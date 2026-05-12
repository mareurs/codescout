---
id: '8e09ca67f463027e'
kind: tracker
status: in-progress
title: Retrieval Benchmark — pinned 25-TC log
owners:
- '@mareurs'
tags:
- retrieval
- benchmark
- qdrant
- embedding
topic: null
time_scope: null
---

## Why this tracker exists

The 20-TC numbers scattered across `docs/research/2026-04-03-embedding-model-benchmark.md`
(23–41/60 range) are **not comparable to each other**. Each run used a different codebase
HEAD, different chunking, different fusion config, different embedder protocol — and the
harness recorded none of those. The "best" historical result (41/60 for CodeRankEmbed
hybrid on 2026-05-02) cannot be reproduced from the artifacts that exist.

This tracker is the canonical log going forward. Every run is anchored to:

- A **pinned worktree** at `.worktrees/bench` (detached HEAD at `baseline_sha`)
- A **dedicated qdrant collection** (`bench_<model>_code_chunks`) so models coexist
- A **config block** in the JSON output (model, boost, sparse on/off, project_sha)
- The **25-TC suite** (20 legacy tiers + 5 T5 real-usage-shape from external `usage.db`)

If the table above ever needs a new `baseline_sha`, treat all prior rows as compromised
and start a new section here.

## How to run a bench

### Prerequisites

```bash
# 1. Retrieval stack containers must be up
docker ps --format '{{.Names}}' | grep -E "qdrant|embedder|reranker"
# Expect: codescout-qdrant (:6334), codescout-embedder-gpu (:48081),
#         codescout-embedder-sparse-gpu (:48084), codescout-reranker-gpu (:48083)

# 2. Pinned bench worktree must exist at the baseline commit
git worktree list | grep .worktrees/bench
# If missing: git worktree add --detach .worktrees/bench <baseline_sha>

# 3. Build release binary
cargo build --release
```

### Run 1 — jina-v2 (fusion at default boost=3.0)

```bash
# Sync once (per (model, collection_prefix) combination)
CODESCOUT_QDRANT_COLLECTION_PREFIX=bench_jinav2_ \
CODESCOUT_QDRANT_URL=http://127.0.0.1:6334 \
CODESCOUT_EMBEDDER_URL=http://127.0.0.1:48081 \
CODESCOUT_SPARSE_EMBEDDER_URL=http://127.0.0.1:48084 \
CODESCOUT_RERANKER_URL=http://127.0.0.1:48083 \
CODESCOUT_RETRIEVAL_PROFILE=gpu CODESCOUT_MODEL_DIM=768 \
./target/release/sync_project .worktrees/bench code-explorer

# Then bench (no re-sync needed for boost sweeps on same model)
CODESCOUT_QDRANT_URL=http://127.0.0.1:6334 \
CODESCOUT_EMBEDDER_URL=http://127.0.0.1:48081 \
CODESCOUT_SPARSE_EMBEDDER_URL=http://127.0.0.1:48084 \
CODESCOUT_RERANKER_URL=http://127.0.0.1:48083 \
CODESCOUT_RETRIEVAL_PROFILE=gpu CODESCOUT_MODEL_DIM=768 \
CODESCOUT_BM25_BOOST=3.0 \
CODESCOUT_EMBED_MODEL=jina-embeddings-v2-base-code \
python3 scripts/run-tc-benchmark.py \
  --binary ./target/release/codescout \
  --project-path "$(pwd)/.worktrees/bench" \
  --collection-prefix bench_jinav2_ \
  --label "jina-v2-bm25-3.0" \
  > /tmp/bench-jinav2.json
```

### Run 2 — CodeRankEmbed via llama-server

```bash
# Start llama-server (foreground for sanity, then move to background)
nohup llama-server -m ~/models/CodeRankEmbed-Q4_K_M.gguf \
  --embeddings --port 43300 --host 127.0.0.1 \
  -ngl 99 -c 16384 -b 4096 -ub 4096 --parallel 1 \
  > /tmp/llama-coderank.log 2>&1 &

# Wait for ready
until curl -s http://127.0.0.1:43300/v1/models >/dev/null; do sleep 1; done

# Sync
CODESCOUT_QDRANT_COLLECTION_PREFIX=bench_coderank_ \
CODESCOUT_EMBEDDER_URL=http://127.0.0.1:43300 \
CODESCOUT_EMBEDDER_PROTOCOL=openai \
CODESCOUT_EMBEDDER_MODEL_NAME=coderank \
CODESCOUT_QDRANT_URL=http://127.0.0.1:6334 \
CODESCOUT_SPARSE_EMBEDDER_URL=http://127.0.0.1:48084 \
CODESCOUT_RERANKER_URL=http://127.0.0.1:48083 \
CODESCOUT_RETRIEVAL_PROFILE=gpu CODESCOUT_MODEL_DIM=768 \
./target/release/sync_project .worktrees/bench code-explorer

# Bench
CODESCOUT_EMBEDDER_URL=http://127.0.0.1:43300 \
CODESCOUT_EMBEDDER_PROTOCOL=openai \
CODESCOUT_EMBEDDER_MODEL_NAME=coderank \
CODESCOUT_QDRANT_URL=http://127.0.0.1:6334 \
CODESCOUT_SPARSE_EMBEDDER_URL=http://127.0.0.1:48084 \
CODESCOUT_RERANKER_URL=http://127.0.0.1:48083 \
CODESCOUT_RETRIEVAL_PROFILE=gpu CODESCOUT_MODEL_DIM=768 \
CODESCOUT_BM25_BOOST=3.0 \
CODESCOUT_EMBED_MODEL=CodeRankEmbed-Q4_K_M \
python3 scripts/run-tc-benchmark.py \
  --binary ./target/release/codescout \
  --project-path "$(pwd)/.worktrees/bench" \
  --collection-prefix bench_coderank_ \
  --label "coderank-bm25-3.0" \
  > /tmp/bench-coderank.json
```

### Run variants

- **Dense-only control:** add `CODESCOUT_DISABLE_SPARSE=1` to the bench command (sync is shared).
- **Boost sweep:** vary `CODESCOUT_BM25_BOOST=<0.5|1.0|2.0|5.0>` on the bench command;
  no re-sync needed (boost is query-time only).
- **New model:** pick a unique `CODESCOUT_QDRANT_COLLECTION_PREFIX` (e.g. `bench_new_`),
  sync into it, then bench. The live `code_chunks` collection is never touched.

### Scoring

Score per TC: 3 if all expected paths in top-5, 2 if all in top-10 or majority in top-5,
1 if at least one in top-10, 0 otherwise. Path match: `r == exp` or
`r.endswith("/" + exp)` or `exp.endswith("/" + r)`. **Caveat:** basename collisions
(`embedder.rs` in two crates) defeat the matcher when the expected list is workspace-
relative — known gap.

## Findings so far (2026-05-12, baseline `ede25e69`)

- **Fusion helps by +2** on both jina-v2 and CodeRankEmbed (sparse on vs off at boost=3.0).
- **Boost sweep on CodeRankEmbed:** 0.5→32, 1.0→33, 2.0→34, 3.0→34, **5.0→35** (peak),
  7.0→34, 10.0→33, 15.0→33, 20.0→33. Peak shifts up vs Phase 6's 3.0 plateau —
  the 25-TC shape (T5 keyword-bags) leans harder on BM25.
- **CodeRankEmbed wins on env-var / identifier-bag queries** (TC-02, TC-11, TC-12)
  where code-specific training surfaces identifier semantics jina misses.
- **CodeRankEmbed query prefix hurts** by 2–4 pts across all boost values when
  doc-side index is plain (no `search_document:` prefix during indexing). Either the
  Q4_K_M quant collapsed the asymmetric subspace or a doc-side prefix is required for
  the prefix to recover. Not retried with re-indexed docs.
- **T5 new tier (real-usage shape) is stuck at 4/15 on both models.** Failures cluster
  on cross-crate basename collisions and class-name-only-no-keyword queries
  (TC-23/24/25). The expected-path matcher is too strict for this tier.
- **Best so far: CodeRankEmbed @ bm25_boost=5.0, no query prefix → 35/75** (46.7%).
## Caveats and known gaps

- **No CodeRankEmbed query prefix.** Historical 41/60 hybrid run used the
  `Represent this query for searching relevant code:` prefix. The retrieval stack's
  `EmbedderHttp` does not add a query-side prefix. Adding `CODESCOUT_QUERY_PREFIX`
  support is the single highest-leverage win for CodeRankEmbed.
- **Scoring matcher needs basename flexibility** for T5. Either rewrite TC-23/24/25
  expected lists to use unambiguous paths, or extend the matcher with crate-aware
  matching.
- **No latency baseline for dense-only p95** in this row set — the harness emitted
  `p95=0` when bench length was too short for the 5% tail. Re-run with `--limit 20` if
  comparing tails matters.

## History

### 2026-05-12 — query prefix experiment (negative result) + extended boost sweep

Added `CODESCOUT_QUERY_PREFIX` env to `EmbedderHttp::embed()` (query side only;
`embed_batch()` doc-side untouched). Tested `"Represent this query for searching
relevant code: "` against CodeRankEmbed across boost ∈ {1, 2, 3, 5}:

| variant | no prefix | with prefix | Δ |
|---|---|---|---|
| dense-only | 32 | 30 | **−2** |
| fusion boost=1.0 | 33 | 30 | **−3** |
| fusion boost=2.0 | 34 | 31 | **−3** |
| fusion boost=3.0 | 34 | 32 | −2 |
| fusion boost=5.0 | **35** | 31 | **−4** |

**Prefix consistently hurts** by 2–4 points. Hypotheses (not yet validated):

1. Q4_K_M quantization may have collapsed the prefix-conditioned subspace.
2. Our docs were indexed without the doc-side training distribution (raw code only).
   If the model was trained with explicit `search_document:` style doc prefix
   (Nomic family convention), then prefix-asymmetry without re-indexing docs WITH
   the doc prefix breaks the asymmetric calibration.
3. Re-indexing docs with `search_document: ` doc-side prefix may recover the win.
   Not tested yet — would require a fresh `bench_coderank_qp_` collection.

Extended boost sweep (no prefix):

| boost | score | p50 ms |
|---|---|---|
| 0.5 | 32 | 143 |
| 1.0 | 33 | 141 |
| 2.0 | 34 | 146 |
| 3.0 | 34 | 152 |
| **5.0** | **35** | 148 |
| 7.0 | 34 | 146 |
| 10.0 | 33 | 150 |
| 15.0 | 33 | 145 |
| 20.0 | 33 | 157 |

**Boost peak is 5.0** on the 25-TC pinned bench. Beyond 5.0, BM25 starts crowding
out the dense candidates that actually carry signal. Plateau is broader than Phase 6
saw (it stopped at 3.0).

### 2026-05-12 — initial pinned bench

Built `.worktrees/bench`, refactored 7 hard-coded collection literals to use
`config.collection(<kind>)` with `CODESCOUT_QDRANT_COLLECTION_PREFIX` override.
Added 5 T5 real-usage-shape TCs sampled from external `usage.db`. First 8 runs
land in the table above. CodeRankEmbed @ boost=5.0 is the current leader at 35/75.
