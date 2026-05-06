# Retrieval Stack Spike — 2026-05-06

## Setup

Spike goal: determine whether Ollama BGE-M3 exposes sparse vectors, or whether
TEI (text-embeddings-inference) is required for the CPU embedder profile.

Target services:
- Qdrant v1.13.0
- Ollama (BGE-M3) — CPU embedder candidate
- TEI cpu-1.6 (BAAI/bge-m3) — fallback CPU embedder
- TEI cpu-1.6 (Qwen/Qwen3-Reranker-0.6B) — reranker

## Findings

| Question | Answer |
|---|---|
| Ollama BGE-M3 sparse output exposed? | **No** — `/api/embeddings` returns dense float array only; no `/embed_sparse` endpoint |
| TEI BGE-M3 sparse output exposed? | **Yes** — `/embed` (dense) + `/embed_sparse` (sparse) both functional |
| Qdrant idf modifier with BGE-M3 sparse? | **Yes** — collection with `sparse_vectors.modifier=idf` + upsert + hybrid query succeed |

**Decision:** Use TEI (`ghcr.io/huggingface/text-embeddings-inference:cpu-1.6`) for the
CPU embedder profile. Ollama does not expose a sparse endpoint required for hybrid search.

## Measurements

| Service | Idle RSS | Cold-start (model load) |
|---|---|---|
| Qdrant v1.13 | ~120 MB | < 5 s |
| TEI BGE-M3 (cpu) | ~1.4 GB | ~90 s (first run, model download) |
| TEI Qwen3-Reranker-0.6B (cpu) | ~1.2 GB | ~60 s (first run) |

Single-query latency (CPU, no GPU):
- Embed: ~180 ms
- Rerank (5 passages): ~220 ms
- Qdrant hybrid query: < 5 ms

## Decisions

1. CPU embedder: **TEI BAAI/bge-m3** (`--dtype float32`)
2. GPU embedder: **TEI Qwen/Qwen3-Embedding-8B** (`--dtype float16`)
3. Reranker CPU: **TEI Qwen/Qwen3-Reranker-0.6B**
4. Reranker GPU: **TEI BAAI/bge-reranker-v2-m3**
5. Qdrant sparse modifier: **Idf** (standard IDF weighting over BGE-M3 sparse outputs)
6. No Ollama service in the compose stack (Ollama dense-only, not needed)
