#!/usr/bin/env python3
"""Chunk × Model retrieval matrix orchestrator.

Runs N cells of (model × chunk_size [× sparse_mode]) against a corpus, scoring
each on a TC suite. Per cell:

  1. Drop the Qdrant `code_chunks` collection (dim varies per model).
  2. Run sync_project with cell-specific env (model URL, dim, chunk size).
  3. Run run-tc-benchmark.py with cell-specific env.
  4. Append a row to results.tsv.

Containers must already be running (docker-compose.matrix.yml).

Usage:
    scripts/chunk-model-matrix.py \\
        --corpus /home/marius/work/claude/code-explorer/.worktrees/retrieval-stack \\
        --tc-suite scripts/run-tc-benchmark.py \\
        --cells jb,bs,js,cr  --chunks 1200,3000  --out results.tsv

    # Dry-run one cell:
    scripts/chunk-model-matrix.py --cells jb --chunks 3000 --dry-run

Cell IDs:
    jb = jina-base (GPU TEI 8090, 768d)
    cr = CodeRankEmbed (AMD llama-server 43300, 768d, OpenAI proto)
    bs = bge-small-en-v1.5 (CPU TEI 8092, 384d)
    js = jina-small-en (CPU TEI 8093, 512d)
"""
from __future__ import annotations

import argparse
import json
import os
import shlex
import subprocess
import sys
import time
from dataclasses import dataclass

# ---------------------------------------------------------------------------
# Cell catalog
# ---------------------------------------------------------------------------

@dataclass
class Model:
    cell: str
    label: str
    url: str
    dim: int
    protocol: str          # "tei" or "openai"
    model_name: str = ""   # required when protocol == "openai"

MODELS: dict[str, Model] = {
    "jb": Model("jb", "jina-v2-base-code",       "http://127.0.0.1:8090",  768, "tei"),
    "cr": Model("cr", "CodeRankEmbed",           "http://127.0.0.1:43300", 768, "openai", "CodeRankEmbed"),
    "bs": Model("bs", "bge-small-en-v1.5",       "http://127.0.0.1:8092",  384, "tei"),
    "js": Model("js", "jina-v2-small-en",        "http://127.0.0.1:8093",  512, "tei"),
}

SPARSE_URL = "http://127.0.0.1:8091"
QDRANT_REST_URL = "http://127.0.0.1:6333"


# ---------------------------------------------------------------------------
# Cell ops
# ---------------------------------------------------------------------------

def drop_collection() -> None:
    """Delete the `code_chunks` collection so it gets recreated with the right dim."""
    subprocess.run(
        ["curl", "-fsS", "-X", "DELETE", f"{QDRANT_REST_URL}/collections/code_chunks"],
        check=False, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )


def collection_stats() -> tuple[int, int]:
    """Return (points_count, disk_bytes) for code_chunks; 0/0 if missing."""
    r = subprocess.run(
        ["curl", "-fsS", f"{QDRANT_REST_URL}/collections/code_chunks"],
        capture_output=True, text=True,
    )
    if r.returncode != 0:
        return 0, 0
    try:
        info = json.loads(r.stdout)
        result = info.get("result", {})
        points = int(result.get("points_count") or 0)
        # Qdrant doesn't expose disk size in REST; approximate via segments
        return points, 0
    except (json.JSONDecodeError, ValueError):
        return 0, 0


def cell_env(model: Model, chunk_target: int, *, disable_sparse: bool, bm25_boost: float) -> dict[str, str]:
    env = os.environ.copy()
    env.update({
        "CODESCOUT_EMBEDDER_URL":         model.url,
        "CODESCOUT_SPARSE_EMBEDDER_URL":  SPARSE_URL,
        "CODESCOUT_MODEL_DIM":            str(model.dim),
        "CODESCOUT_EMBEDDER_PROTOCOL":    model.protocol,
        "CODESCOUT_EMBEDDER_MODEL_NAME":  model.model_name,
        "CODESCOUT_CHUNK_TARGET":         str(chunk_target),
        "CODESCOUT_BM25_BOOST":           f"{bm25_boost}",
        "CODESCOUT_DISABLE_SPARSE":       "1" if disable_sparse else "0",
        # Belt-and-suspenders: matrix runs always use the stack backend.
    })
    return env


def run_sync(env: dict[str, str], corpus: str, project_id: str, sync_bin: str) -> tuple[bool, float]:
    t0 = time.monotonic()
    r = subprocess.run(
        [sync_bin, corpus, project_id],
        env=env, capture_output=True, text=True, timeout=3600,
    )
    dt = time.monotonic() - t0
    if r.returncode != 0:
        sys.stderr.write(f"[sync FAILED] stderr tail:\n{r.stderr[-1000:]}\n")
    return r.returncode == 0, dt


def run_benchmark(env: dict[str, str], corpus: str, codescout_bin: str,
                  tc_suite: str | None) -> dict | None:
    cmd = [
        "python3", "scripts/run-tc-benchmark.py",
        "--binary", codescout_bin,
        "--project-path", corpus,
    ]
    if tc_suite:
        cmd += ["--tc-suite", tc_suite]
    r = subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=600)
    if r.returncode != 0:
        sys.stderr.write(f"[bench FAILED] stderr tail:\n{r.stderr[-800:]}\n")
        return None
    try:
        return json.loads(r.stdout)
    except json.JSONDecodeError:
        sys.stderr.write(f"[bench JSON PARSE FAILED] stdout tail:\n{r.stdout[-800:]}\n")
        return None


# ---------------------------------------------------------------------------
# Matrix runner
# ---------------------------------------------------------------------------

def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--corpus", required=True, help="Path to project to index/search")
    ap.add_argument("--project-id", default=None,
                    help="Qdrant project_id payload filter (default: basename of corpus)")
    ap.add_argument("--tc-suite", default=None,
                    help="Optional TC suite JSON. Omit to use built-in 20-TC suite.")
    ap.add_argument("--cells", default="jb,cr,bs,js",
                    help="Comma list of model cell IDs (default: all four)")
    ap.add_argument("--chunks", default="600,1200,3000",
                    help="Comma list of chunk targets")
    ap.add_argument("--bm25-boost", type=float, default=1.0,
                    help="BM25 boost (default 1.0)")
    ap.add_argument("--disable-sparse", action="store_true",
                    help="Run cells with sparse leg disabled (control)")
    ap.add_argument("--out", default="results.tsv",
                    help="Output TSV path (appends rows)")
    ap.add_argument("--sync-bin", default="./target/release/sync_project")
    ap.add_argument("--codescout-bin", default="./target/release/codescout")
    ap.add_argument("--dry-run", action="store_true",
                    help="Run only the first cell of the matrix (smoke test)")
    args = ap.parse_args()

    project_id = args.project_id or os.path.basename(os.path.normpath(args.corpus))

    cell_ids = [c.strip() for c in args.cells.split(",") if c.strip()]
    chunks = [int(c) for c in args.chunks.split(",") if c.strip()]

    cells: list[tuple[Model, int]] = [
        (MODELS[cid], ct) for cid in cell_ids for ct in chunks
    ]
    if args.dry_run:
        cells = cells[:1]

    sys.stderr.write(f"[matrix] {len(cells)} cell(s) over corpus={args.corpus} project_id={project_id}\n")

    # TSV header (only if file doesn't exist)
    write_header = not os.path.exists(args.out)
    out = open(args.out, "a")
    if write_header:
        out.write("cell_id\tmodel\tdim\tchunk\tboost\tsparse\tcorpus\tpoints\tsync_s\tscore\tmax\tp50_ms\tp95_ms\tts\n")
        out.flush()

    for model, chunk_target in cells:
        cell_id = f"{model.cell}_c{chunk_target}_b{args.bm25_boost}{'_ns' if args.disable_sparse else ''}"
        ts = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
        sys.stderr.write(f"\n=== CELL {cell_id} ({model.label}, chunk={chunk_target}) ===\n")

        drop_collection()
        env = cell_env(model, chunk_target,
                       disable_sparse=args.disable_sparse,
                       bm25_boost=args.bm25_boost)

        ok, sync_secs = run_sync(env, args.corpus, project_id, args.sync_bin)
        if not ok:
            out.write(f"{cell_id}\t{model.label}\t{model.dim}\t{chunk_target}\t{args.bm25_boost}\t"
                      f"{'off' if args.disable_sparse else 'on'}\t{project_id}\t0\t{sync_secs:.1f}\t-\t-\t-\t-\t{ts}\n")
            out.flush()
            continue

        points, _ = collection_stats()
        sys.stderr.write(f"  synced {points} points in {sync_secs:.1f}s\n")

        bench = run_benchmark(env, args.corpus, args.codescout_bin, args.tc_suite)
        if bench is None:
            out.write(f"{cell_id}\t{model.label}\t{model.dim}\t{chunk_target}\t{args.bm25_boost}\t"
                      f"{'off' if args.disable_sparse else 'on'}\t{project_id}\t{points}\t{sync_secs:.1f}\t-\t-\t-\t-\t{ts}\n")
            out.flush()
            continue

        agg = bench.get("aggregate", {})
        score = agg.get("total", "-")
        maxv = agg.get("max", "-")
        p50 = agg.get("p50_latency_ms", "-")
        p95 = agg.get("p95_latency_ms", "-")
        sys.stderr.write(f"  score={score}/{maxv}  p50={p50}ms  p95={p95}ms\n")

        out.write(f"{cell_id}\t{model.label}\t{model.dim}\t{chunk_target}\t{args.bm25_boost}\t"
                  f"{'off' if args.disable_sparse else 'on'}\t{project_id}\t{points}\t{sync_secs:.1f}\t"
                  f"{score}\t{maxv}\t{p50}\t{p95}\t{ts}\n")
        out.flush()

    out.close()
    sys.stderr.write(f"\n[matrix] complete -> {args.out}\n")


if __name__ == "__main__":
    main()
