#!/usr/bin/env bash
# Sweep CODESCOUT_BM25_BOOST on the current best cell (cr@1200).
# Indexes once, then re-runs benchmark per boost value.
set -euo pipefail

cd "$(dirname "$0")/.."

CORPUS=/home/marius/work/claude/code-explorer/.worktrees/retrieval-stack
BIN_SYNC=./target/release/sync_project
BIN_CC=./target/release/codescout
OUT=results-bm25-cr1200.tsv

export CODESCOUT_RETRIEVAL_BACKEND=stack
export CODESCOUT_EMBEDDER_URL=http://127.0.0.1:43300
export CODESCOUT_SPARSE_EMBEDDER_URL=http://127.0.0.1:8091
export CODESCOUT_MODEL_DIM=768
export CODESCOUT_EMBEDDER_PROTOCOL=openai
export CODESCOUT_EMBEDDER_MODEL_NAME=CodeRankEmbed
export CODESCOUT_CHUNK_TARGET=1200

echo "[1/2] Drop+reindex with cr@1200..."
curl -fsS -X DELETE "http://127.0.0.1:6333/collections/code_chunks" >/dev/null || true
"$BIN_SYNC" "$CORPUS" retrieval-stack
PTS=$(curl -fsS http://127.0.0.1:6333/collections/code_chunks | python3 -c "import sys,json; print(json.load(sys.stdin)['result']['points_count'])")
echo "  indexed $PTS points"

echo
echo "[2/2] Sweeping bm25_boost..."
echo -e "boost\tscore\tp50_ms\tp95_ms\tts" > "$OUT"

for BOOST in 0.25 0.5 1.0 1.5 2.0 3.0; do
  export CODESCOUT_BM25_BOOST=$BOOST
  TS=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  echo "  boost=$BOOST"
  RAW=$(python3 scripts/run-tc-benchmark.py --binary "$BIN_CC" --project-path "$CORPUS" 2>/dev/null)
  SCORE=$(echo "$RAW" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['aggregate']['total'])")
  P50=$(echo "$RAW"   | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['aggregate']['p50_latency_ms'])")
  P95=$(echo "$RAW"   | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['aggregate']['p95_latency_ms'])")
  echo -e "$BOOST\t$SCORE/60\t$P50\t$P95\t$TS" | tee -a "$OUT"
done

echo
echo "results -> $OUT"
column -t -s $'\t' "$OUT"
