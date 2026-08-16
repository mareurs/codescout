#!/usr/bin/env bash
# Fetch/build the GGUF model files the retrieval stack bind-mounts into its
# dense (and, on the gpu/amd profiles, reranker) containers.
#
#   ./scripts/fetch-models.sh          # dense only
#   ./scripts/fetch-models.sh --gpu    # dense + reranker — gpu profile
#   ./scripts/fetch-models.sh --amd    # same as --gpu; the amd compose profile was
#                                        removed 2026-07-27 but the flag is kept
#                                        because the reranker GGUF is identical and
#                                        the profile is restorable from git history.
#
# Idempotent: an existing target file is left alone. Honours CODESCOUT_MODEL_DIR
# (same variable docker-compose.yml bind-mounts), defaulting to ./models.
set -euo pipefail

MODEL_DIR="${CODESCOUT_MODEL_DIR:-./models}"
LLAMA_IMAGE="ghcr.io/ggml-org/llama.cpp:full"
DENSE_GGUF="CodeRankEmbed-Q4_K_M.gguf"
RERANK_GGUF="bge-reranker-v2-m3-Q4_K_M.gguf"

# Create the bind-mount target ourselves. If docker gets there first (by
# starting a container before the model exists) it creates the directory as
# root:root, and every write below then fails with EACCES.
mkdir -p "$MODEL_DIR"
MODEL_DIR="$(cd "$MODEL_DIR" && pwd)"

# `--user` keeps container-written files owned by the invoking user; without it
# the converted GGUFs land as root:root and the next run cannot overwrite them.
llama() {
  docker run --rm --user "$(id -u):$(id -g)" -v "$MODEL_DIR:/models" "$LLAMA_IMAGE" "$@"
}

fetch_dense() {
  if [ -f "$MODEL_DIR/$DENSE_GGUF" ]; then
    echo "dense:    $DENSE_GGUF present — skipping"
    return
  fi
  # There is no published GGUF repo for this model. `nomic-ai/CodeRankEmbed-GGUF`
  # (which earlier revisions of the docs pointed at) returns HTTP 401 and does
  # not appear in HuggingFace search — see
  # docs/issues/archive/2026-07-25-coderankembed-gguf-source-404.md. Build the quant from
  # the official, ungated safetensors repo instead. Q4_K_M is deliberate: it is
  # the benchmarked champion (37) and beats f16 — see
  # docs/manual/src/concepts/retrieval-stack.md § Dense embedder.
  local src="$MODEL_DIR/CodeRankEmbed"
  local base="https://huggingface.co/nomic-ai/CodeRankEmbed/resolve/main"
  echo "dense:    fetching nomic-ai/CodeRankEmbed source (~550 MB)"
  mkdir -p "$src"
  for f in config.json model.safetensors tokenizer.json tokenizer_config.json \
           special_tokens_map.json vocab.txt; do
    curl -L --fail -sS -o "$src/$f" "$base/$f"
  done

  echo "dense:    converting to f16 GGUF"
  llama --convert /models/CodeRankEmbed --outtype f16 \
        --outfile /models/CodeRankEmbed-f16.gguf

  echo "dense:    quantizing to Q4_K_M"
  llama --quantize /models/CodeRankEmbed-f16.gguf "/models/$DENSE_GGUF" Q4_K_M

  echo "dense:    built $DENSE_GGUF ($(stat -c%s "$MODEL_DIR/$DENSE_GGUF") bytes; expect ~90 MB)"
}

fetch_reranker() {
  if [ -f "$MODEL_DIR/$RERANK_GGUF" ]; then
    echo "reranker: $RERANK_GGUF present — skipping"
    return
  fi
  # This one IS published as GGUF, so it is a plain download.
  echo "reranker: downloading gpustack/bge-reranker-v2-m3-GGUF (~419 MB)"
  curl -L --fail -sS -o "$MODEL_DIR/$RERANK_GGUF" \
    "https://huggingface.co/gpustack/bge-reranker-v2-m3-GGUF/resolve/main/$RERANK_GGUF"
  echo "reranker: downloaded ($(stat -c%s "$MODEL_DIR/$RERANK_GGUF") bytes)"
}

fetch_dense
case "${1:-}" in
  --gpu|--amd) fetch_reranker ;;
  "")          ;;
  *)           echo "usage: $0 [--gpu|--amd]" >&2; exit 2 ;;
esac

echo
echo "model dir: $MODEL_DIR ($(du -sh "$MODEL_DIR" | cut -f1))"
echo "The CodeRankEmbed/ source dir and CodeRankEmbed-f16.gguf are build"
echo "intermediates — safe to delete once the Q4_K_M file exists (they are only"
echo "needed to re-quantize at a different level)."
