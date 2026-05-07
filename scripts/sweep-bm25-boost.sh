#!/usr/bin/env bash
# Sweep CODESCOUT_BM25_BOOST values and print score table.
set -euo pipefail

BINARY="${1:-./target/release/codescout}"
PROJECT_PATH="${2:-/home/marius/work/claude/code-explorer}"
BOOSTS="${3:-0.25 0.5 1.0 1.5 2.0 3.0 5.0}"

export CODESCOUT_QDRANT_URL="http://127.0.0.1:6334"
export CODESCOUT_EMBEDDER_URL="http://127.0.0.1:8081"
export CODESCOUT_SPARSE_EMBEDDER_URL="http://127.0.0.1:8084"
export CODESCOUT_RERANKER_URL="http://127.0.0.1:8083"
export CODESCOUT_MODEL_DIM="768"
export CODESCOUT_RETRIEVAL_BACKEND="stack"

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
