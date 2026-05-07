#!/usr/bin/env python3
"""
codescout retrieval benchmark harness — 20-TC suite.

Usage:
    CODESCOUT_RETRIEVAL_BACKEND=legacy python3 scripts/run-tc-benchmark.py \
        --binary ./target/release/codescout \
        --project-path /path/to/project \
        > /tmp/legacy.json

    CODESCOUT_RETRIEVAL_BACKEND=stack python3 scripts/run-tc-benchmark.py \
        --binary ./target/release/codescout \
        --project-path /path/to/project \
        > /tmp/stack.json
"""
import argparse
import json
import os
import statistics
import subprocess
import sys
import time
from typing import Any

# ---------------------------------------------------------------------------
# Test case definitions (from docs/research/2026-04-03-embedding-model-benchmark.md)
# ---------------------------------------------------------------------------

TEST_CASES = [
    # Tier 1: Direct symbol/keyword lookup (1-5)
    # Style: short keyword-dense queries matching real usage patterns
    {
        "id": "TC-01", "tier": 1,
        "query": "RecoverableError",
        "expected": ["src/tools/mod.rs", "src/server.rs", "docs/FEATURES.md"],
    },
    {
        "id": "TC-02", "tier": 1,
        "query": "CODESCOUT_EMBEDDER_URL prefix backend local remote ONNX",
        "expected": [
            "src/embed/mod.rs",
            "docs/manual/src/configuration/embeddings.md",
            "docs/manual/src/configuration/embedding-backends.md",
        ],
    },
    {
        "id": "TC-03", "tier": 1,
        "query": "LSP client implementation",
        "expected": ["src/lsp/client.rs", "src/lsp/ops.rs", "src/lsp/manager.rs"],
    },
    {
        "id": "TC-04", "tier": 1,
        "query": "run_command shell dangerous command output_id stderr",
        "expected": [
            "src/tools/run_command/mod.rs",
            "docs/manual/src/concepts/shell-integration.md",
            "docs/manual/src/concepts/output-buffers.md",
        ],
    },
    {
        "id": "TC-05", "tier": 1,
        "query": "OutputGuard progressive disclosure capping",
        "expected": ["src/tools/output.rs", "docs/PROGRESSIVE_DISCOVERABILITY.md"],
    },
    # Tier 2: Symbol + concept composition (6-12)
    {
        "id": "TC-06", "tier": 2,
        "query": "tool_calls usage.db latency outcome session_id record",
        "expected": [
            "src/usage/db.rs",
            "src/usage/mod.rs",
            "src/tools/usage.rs",
        ],
    },
    {
        "id": "TC-07", "tier": 2,
        "query": "parse_all_headings compute_section_end heading boundary markdown",
        "expected": [
            "src/tools/markdown/edit_markdown.rs",
            "src/tools/file_summary/file_summary.rs",
        ],
    },
    {
        "id": "TC-08", "tier": 2,
        "query": "embedding dimension mismatch vec0 schema migration",
        "expected": ["src/embed/index.rs", "src/embed/schema.rs"],
    },
    {
        "id": "TC-09", "tier": 2,
        "query": "dangerous command detection deny block path_security run_command",
        "expected": ["src/util/path_security.rs", "src/tools/run_command/mod.rs"],
    },
    {
        "id": "TC-10", "tier": 2,
        "query": "OutputGuard overflow hint cap_items by_file narrow suggestion",
        "expected": [
            "src/tools/output.rs",
            "docs/PROGRESSIVE_DISCOVERABILITY.md",
            "src/prompts/server_instructions.md",
        ],
    },
    {
        "id": "TC-11", "tier": 2,
        "query": "rename_symbol workspace_edit textDocument LSP references sites",
        "expected": ["src/tools/symbol/edit_code.rs", "src/lsp/ops.rs"],
    },
    {
        "id": "TC-12", "tier": 2,
        "query": "CODESCOUT_EMBEDDER_URL model_prefix local remote backend factory",
        "expected": [
            "src/embed/mod.rs",
            "docs/manual/src/configuration/embeddings.md",
        ],
    },
    # Tier 3: Multi-symbol cross-cutting (13-17)
    {
        "id": "TC-13", "tier": 3,
        "query": "LSP circuit breaker crash recovery restart client manager",
        "expected": ["src/lsp/client.rs", "src/lsp/manager.rs", "docs/manual/src/troubleshooting.md"],
    },
    {
        "id": "TC-14", "tier": 3,
        "query": "RecoverableError anyhow bail call_content dispatch isError routing",
        "expected": ["src/tools/mod.rs", "src/server.rs", "src/usage/mod.rs"],
    },
    {
        "id": "TC-15", "tier": 3,
        "query": "force_reindex vec0 recreate dimension migration build_index",
        "expected": ["src/embed/index.rs", "src/embed/mod.rs"],
    },
    {
        "id": "TC-16", "tier": 3,
        "query": "semantic_search embedding KNN vec0 ranked results query flow",
        "expected": [
            "src/tools/semantic/semantic_search.rs",
            "src/embed/index.rs",
            "src/embed/mod.rs",
        ],
    },
    {
        "id": "TC-17", "tier": 3,
        "query": "companion plugin PreToolUse hook Read Grep block routing codescout",
        "expected": [
            "docs/manual/src/concepts/routing-plugin.md",
            "docs/manual/src/getting-started/companion-plugin.md",
        ],
    },
    # Tier 4: Architectural insight (18-20)
    {
        "id": "TC-18", "tier": 4,
        "query": "parse_all_headings compute_section_end code_block tracking same path",
        "expected": [
            "src/tools/markdown/edit_markdown.rs",
            "src/tools/file_summary/file_summary.rs",
            "docs/TODO-tool-misbehaviors.md",
        ],
    },
    {
        "id": "TC-19", "tier": 4,
        "query": "project activate LSP lifecycle ActiveProject tool_context server wiring",
        "expected": [
            "src/agent/mod.rs",
            "src/lsp/manager.rs",
            "src/server.rs",
        ],
    },
    {
        "id": "TC-20", "tier": 4,
        "query": "prompt_surfaces_reference_only_real_tools server_instructions onboarding consistency",
        "expected": [
            "src/prompts/server_instructions.md",
            "src/prompts/onboarding_prompt.md",
            "src/server.rs",
        ],
    },
]

# ---------------------------------------------------------------------------
# Scoring (max 3 per TC, max 60 total)
# ---------------------------------------------------------------------------

def score_tc(top10: list[str], expected: list[str]) -> int:
    """
    3 — all expected files in top-5
    2 — all expected files in top-10, OR majority in top-5
    1 — at least one expected file in top-10
    0 — none
    """
    top5 = set(top10[:5])
    top10_set = set(top10)

    def matches(exp: str, result_set: set[str]) -> bool:
        for r in result_set:
            if r == exp or r.endswith("/" + exp) or exp.endswith("/" + r):
                return True
        return False

    in_top5 = sum(1 for e in expected if matches(e, top5))
    in_top10 = sum(1 for e in expected if matches(e, top10_set))

    if in_top5 == len(expected):
        return 3
    if in_top10 == len(expected) or in_top5 >= (len(expected) + 1) // 2:
        return 2
    if in_top10 >= 1:
        return 1
    return 0


# ---------------------------------------------------------------------------
# Minimal MCP stdio client
# ---------------------------------------------------------------------------

class McpClient:
    def __init__(self, proc: subprocess.Popen) -> None:
        self._proc = proc
        self._next_id = 1

    def _send(self, msg: dict[str, Any]) -> None:
        line = json.dumps(msg) + "\n"
        self._proc.stdin.write(line.encode())
        self._proc.stdin.flush()

    def _recv(self, req_id: int) -> dict[str, Any]:
        while True:
            raw = self._proc.stdout.readline()
            if not raw:
                raise RuntimeError("MCP server closed stdout unexpectedly")
            try:
                msg = json.loads(raw)
            except json.JSONDecodeError:
                continue  # skip non-JSON lines (e.g. qdrant version warnings on stdout)
            if msg.get("id") == req_id:
                return msg

    def initialize(self) -> None:
        req_id = self._next_id
        self._next_id += 1
        self._send({
            "jsonrpc": "2.0",
            "id": req_id,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "benchmark-harness", "version": "1.0"},
            },
        })
        self._recv(req_id)
        self._send({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}})

    def call_tool(self, name: str, arguments: dict[str, Any]) -> dict[str, Any]:
        req_id = self._next_id
        self._next_id += 1
        self._send({
            "jsonrpc": "2.0",
            "id": req_id,
            "method": "tools/call",
            "params": {"name": name, "arguments": arguments},
        })
        resp = self._recv(req_id)
        if "error" in resp:
            raise RuntimeError(f"MCP error: {resp['error']}")
        return resp.get("result", {})

    def activate_project(self, path: str) -> None:
        self.call_tool("workspace", {"action": "activate", "path": path})

    def semantic_search(self, query: str, limit: int = 10) -> list[str]:
        result = self.call_tool("semantic_search", {"query": query, "limit": limit})
        content = result.get("content", [])
        if not content:
            return []
        text = content[0].get("text", "{}")
        try:
            data = json.loads(text)
        except json.JSONDecodeError:
            return []
        # Resolve output buffer reference — read_file paginates; loop until complete
        if "output_id" in data:
            ref_id = data["output_id"]
            raw_parts: list[str] = []
            start_line = 1
            for _ in range(50):  # safety cap
                buf_result = self.call_tool("read_file", {
                    "path": ref_id,
                    "start_line": start_line,
                    "end_line": start_line + 99,
                })
                buf_content = buf_result.get("content", [])
                if not buf_content:
                    break
                try:
                    envelope = json.loads(buf_content[0].get("text", "{}"))
                except json.JSONDecodeError:
                    break
                raw_parts.append(envelope.get("content", ""))
                shown = envelope.get("shown_lines")
                if envelope.get("complete", True):
                    break
                if isinstance(shown, list) and len(shown) == 2:
                    start_line = shown[1] + 1
                else:
                    break
            try:
                data = json.loads("".join(raw_parts))
            except json.JSONDecodeError:
                return []
        items = data.get("results", data) if isinstance(data, dict) else data
        if isinstance(items, list):
            return [
                item.get("file_path", "") if isinstance(item, dict) else ""
                for item in items
            ]
        return []

    def close(self) -> None:
        try:
            self._proc.stdin.close()
            self._proc.wait(timeout=5)
        except Exception:
            self._proc.kill()


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> None:
    parser = argparse.ArgumentParser(description="codescout 20-TC retrieval benchmark")
    parser.add_argument("--binary", default="./target/release/codescout",
                        help="Path to codescout binary")
    parser.add_argument("--project-path", required=True,
                        help="Absolute path to the project to activate")
    parser.add_argument("--tc-suite",
                        help="Optional path to a JSON file with custom TCs (overrides built-in TEST_CASES). "
                             "Each TC must have: id, query, expected_files (list); optional tier (default 1).")
    parser.add_argument("--limit", type=int, default=10,
                        help="Top-N results to retrieve per query (default: 10)")
    args = parser.parse_args()

    backend = os.environ.get("CODESCOUT_RETRIEVAL_BACKEND", "legacy")

    test_cases = TEST_CASES
    if args.tc_suite:
        with open(args.tc_suite) as fh:
            raw = json.load(fh)
        test_cases = [
            {"id": t["id"], "tier": t.get("tier", 1),
             "query": t["query"], "expected": t["expected_files"]}
            for t in raw
        ]
        print(f"[INFO] loaded {len(test_cases)} TCs from {args.tc_suite}", file=sys.stderr)

    env = os.environ.copy()
    proc = subprocess.Popen(
        [args.binary, "start"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        env=env,
    )

    client = McpClient(proc)
    try:
        client.initialize()
        client.activate_project(args.project_path)
    except Exception as exc:
        print(json.dumps({"error": f"MCP init failed: {exc}"}), file=sys.stdout)
        client.close()
        sys.exit(1)

    tc_results = []
    latencies: list[float] = []

    for tc in test_cases:
        t0 = time.monotonic()
        try:
            top10 = client.semantic_search(tc["query"], limit=args.limit)
        except Exception as exc:
            top10 = []
            print(f"[WARN] {tc['id']} failed: {exc}", file=sys.stderr)
        latency_ms = (time.monotonic() - t0) * 1000
        latencies.append(latency_ms)

        sc = score_tc(top10, tc["expected"])
        tc_results.append({
            "id": tc["id"],
            "tier": tc["tier"],
            "query": tc["query"],
            "score": sc,
            "top10_files": top10[:10],
            "expected_files": tc["expected"],
            "latency_ms": round(latency_ms, 1),
        })
        print(f"  {tc['id']} score={sc}/3  {latency_ms:.0f}ms", file=sys.stderr)

    client.close()

    latencies_sorted = sorted(latencies)
    p50 = statistics.median(latencies_sorted)
    p95 = latencies_sorted[int(len(latencies_sorted) * 0.95) - 1] if latencies_sorted else 0

    aggregate_score = sum(r["score"] for r in tc_results)
    output = {
        "backend": backend,
        "project_path": args.project_path,
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "aggregate": {
            "total": aggregate_score,
            "max": len(test_cases) * 3,
            "p50_latency_ms": round(p50, 1),
            "p95_latency_ms": round(p95, 1),
        },
        "test_cases": tc_results,
    }
    print(json.dumps(output, indent=2))


if __name__ == "__main__":
    main()
