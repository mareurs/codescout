#!/usr/bin/env bash
# Run the 20-TC retrieval benchmark (Qdrant stack only since Phase 7).
#
# Usage:
#   ./scripts/run-tc-benchmark.sh > /tmp/stack.json
#
# Override binary or project path:
#   CODESCOUT_BINARY=./target/debug/codescout \
#   CODESCOUT_PROJECT_PATH=/other/project \
#   ./scripts/run-tc-benchmark.sh > /tmp/results.json

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

BINARY="${CODESCOUT_BINARY:-${REPO_ROOT}/target/release/codescout}"
PROJECT_PATH="${CODESCOUT_PROJECT_PATH:-${REPO_ROOT}/.worktrees/bench}"

if [[ ! -x "${BINARY}" ]]; then
    echo "codescout binary not found at ${BINARY}" >&2
    echo "Run: cargo build --release" >&2
    exit 1
fi

exec python3 "${SCRIPT_DIR}/run-tc-benchmark.py" \
    --binary "${BINARY}" \
    --project-path "${PROJECT_PATH}" \
    "$@"
