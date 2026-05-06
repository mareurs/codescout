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
    # Tier 1: Direct Concept (1-5)
    {
        "id": "TC-01", "tier": 1,
        "query": "RecoverableError",
        "expected": ["src/tools/mod.rs", "src/server.rs", "docs/FEATURES.md"],
    },
    {
        "id": "TC-02", "tier": 1,
        "query": "embedding model configuration",
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
        "query": "run_command shell execution",
        "expected": [
            "src/tools/workflow.rs",
            "docs/manual/src/concepts/shell-integration.md",
            "docs/manual/src/concepts/output-buffers.md",
        ],
    },
    {
        "id": "TC-05", "tier": 1,
        "query": "OutputGuard progressive disclosure capping",
        "expected": ["src/tools/output.rs", "docs/PROGRESSIVE_DISCOVERABILITY.md"],
    },
    # Tier 2: Two-Concept Composition (6-12)
    {
        "id": "TC-06", "tier": 2,
        "query": "how are tool calls recorded in the usage database",
        "expected": [
            "src/usage/db.rs",
            "src/usage/mod.rs",
            "docs/plans/2026-04-02-usage-traceability-design.md",
        ],
    },
    {
        "id": "TC-07", "tier": 2,
        "query": "section boundary detection in markdown editing",
        "expected": ["src/tools/markdown.rs", "src/tools/file_summary.rs"],
    },
    {
        "id": "TC-08", "tier": 2,
        "query": "dimension mismatch when switching embedding models",
        "expected": ["src/embed/index.rs", "src/embed/schema.rs"],
    },
    {
        "id": "TC-09", "tier": 2,
        "query": "dangerous command detection and safety checks",
        "expected": ["src/util/path_security.rs", "src/tools/workflow.rs"],
    },
    {
        "id": "TC-10", "tier": 2,
        "query": "how overflow hints guide the agent to narrow results",
        "expected": [
            "src/tools/output.rs",
            "docs/PROGRESSIVE_DISCOVERABILITY.md",
            "src/prompts/server_instructions.md",
        ],
    },
    {
        "id": "TC-11", "tier": 2,
        "query": "renaming a symbol across all references in the codebase",
        "expected": ["src/tools/symbol.rs", "src/lsp/ops.rs"],
    },
    {
        "id": "TC-12", "tier": 2,
        "query": "how the embedding URL and model prefix determine which backend is used",
        "expected": [
            "src/embed/mod.rs",
            "docs/manual/src/configuration/embeddings.md",
        ],
    },
    # Tier 3: Multi-Concept Cross-Cutting (13-17)
    {
        "id": "TC-13", "tier": 3,
        "query": "what happens when an LSP server crashes mid-request and how does the circuit breaker recover",
        "expected": ["src/lsp/client.rs", "src/lsp/manager.rs", "docs/manual/src/troubleshooting.md"],
    },
    {
        "id": "TC-14", "tier": 3,
        "query": "how does the tool dispatch pipeline handle both recoverable errors and fatal failures differently",
        "expected": ["src/tools/mod.rs", "src/server.rs", "src/usage/mod.rs"],
    },
    {
        "id": "TC-15", "tier": 3,
        "query": "end-to-end force re-indexing flow including dimension migration and vec0 table recreation",
        "expected": ["src/embed/index.rs", "src/embed/mod.rs"],
    },
    {
        "id": "TC-16", "tier": 3,
        "query": "how a semantic search query flows from input through embedding to KNN ranked results",
        "expected": [
            "src/tools/semantic.rs",
            "src/embed/index.rs",
            "src/embed/mod.rs",
        ],
    },
    {
        "id": "TC-17", "tier": 3,
        "query": "how does the companion plugin route native Read and Grep calls to codescout MCP tools",
        "expected": [
            "docs/manual/src/concepts/routing-plugin.md",
            "docs/manual/src/getting-started/companion-plugin.md",
        ],
    },
    # Tier 4: Architectural Insight (18-20)
    {
        "id": "TC-18", "tier": 4,
        "query": "why heading detection in parse_all_headings and compute_section_end must use the same code block tracking",
        "expected": [
            "src/tools/markdown.rs",
            "src/tools/file_summary.rs",
            "docs/TODO-tool-misbehaviors.md",
        ],
    },
    {
        "id": "TC-19", "tier": 4,
        "query": "relationship between project activation, LSP server lifecycle, and tool context wiring",
        "expected": [
            "src/agent/mod.rs",
            "src/lsp/manager.rs",
            "src/server.rs",
        ],
    },
    {
        "id": "TC-20", "tier": 4,
        "query": "how to keep the three prompt surfaces consistent when tools are renamed or behavior changes",
        "expected": [
            "src/prompts/server_instructions.md",
            "src/prompts/onboarding_prompt.md",
            "src/tools/workflow.rs",
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
            msg = json.loads(raw)
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
    parser.add_argument("--limit", type=int, default=10,
                        help="Top-N results to retrieve per query (default: 10)")
    args = parser.parse_args()

    backend = os.environ.get("CODESCOUT_RETRIEVAL_BACKEND", "legacy")

    env = os.environ.copy()
    proc = subprocess.Popen(
        [args.binary],
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

    for tc in TEST_CASES:
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
            "max": 60,
            "p50_latency_ms": round(p50, 1),
            "p95_latency_ms": round(p95, 1),
        },
        "test_cases": tc_results,
    }
    print(json.dumps(output, indent=2))


if __name__ == "__main__":
    main()
