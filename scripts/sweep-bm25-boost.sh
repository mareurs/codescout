#!/usr/bin/env bash
# Sweep CODESCOUT_BM25_BOOST values and print score table.
set -euo pipefail

BINARY="${1:-./target/release/codescout}"
# The corpus defaults to THIS repo, not a path on the author's machine:
# run-tc-benchmark.py's expected-file lists name codescout's own tree
# (src/lsp/client.rs, src/lsp/manager.rs, src/tools/output.rs, ...), so scores are
# only meaningful against this checkout. NOTE: five paths in those expected
# lists no longer exist, so some TCs cannot score at any boost value — see
# docs/issues/2026-08-16-tc-benchmark-ground-truth-cites-deleted-files.md before
# reading a sweep result as a retrieval measurement.
# The previous default pointed at
# ".../code-explorer" -- codescout's pre-rename name -- and had resolved to
# nothing since the rename.
PROJECT_PATH="${2:-$(cd "$(dirname "$0")/.." && pwd)}"
BOOSTS="${3:-0.25 0.5 1.0 1.5 2.0 3.0 5.0}"

export CODESCOUT_QDRANT_URL="http://127.0.0.1:6334"
# Host ports as published by docker-compose.yml. The container-internal ports
# (8080/80/8080) are not reachable from here -- this script runs a host binary.
export CODESCOUT_EMBEDDER_URL="http://127.0.0.1:48081"
export CODESCOUT_SPARSE_EMBEDDER_URL="http://127.0.0.1:48084"
export CODESCOUT_RERANKER_URL="http://127.0.0.1:48083"
export CODESCOUT_MODEL_DIM="768"

echo "boost  score  p50ms  p95ms"
echo "-----  -----  -----  -----"

for boost in $BOOSTS; do
    out=$(CODESCOUT_BM25_BOOST="$boost" python3 scripts/run-tc-benchmark.py \
        --binary "$BINARY" \
        --project-path "$PROJECT_PATH" 2>/dev/null)
    score=$(echo "$out" | python3 -c "import sys,json; d=json.load(sys.stdin)['aggregate']; print(f\"{d['total']}/{d['max']}\")")
    p50=$(echo "$out" | python3 -c "import sys,json; d=json.load(sys.stdin)['aggregate']; print(d['p50_latency_ms'])")
    p95=$(echo "$out" | python3 -c "import sys,json; d=json.load(sys.stdin)['aggregate']; print(d['p95_latency_ms'])")
    echo "$boost  $score  $p50  $p95"
done
