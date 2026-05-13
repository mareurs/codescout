# Hybrid Dense + Sparse Retrieval

`semantic_search` uses a hybrid retrieval pipeline combining dense vector
search with sparse SPLADE keyword search, fused via Reciprocal Rank Fusion
(RRF) inside Qdrant.

> **History:** Prior to v0.12 the sparse leg was a local Tantivy BM25 index.
> Since v0.12 it is a SPLADE service in the retrieval stack — same conceptual
> shape (lexical complement to dense), different implementation. The Tantivy
> code and `bm25.rs` module have been deleted.

## How it works

1. **Dense leg** — query embedded by the dense embedder service (default
   `localhost:48081`, TEI or OpenAI protocol) and matched against the
   `code_chunks` collection's dense vector field.
2. **Sparse leg** — query embedded by the SPLADE service (default
   `localhost:48084`, TEI protocol) and matched against the same collection's
   sparse vector field.
3. **RRF fusion** — Qdrant fuses the two ranked lists server-side using
   `1/(1+rank)` (note: rank-1 = 0.500, rank-9 ≈ 0.100 — this is Qdrant's
   constant, not the academic `k=60` formula).
4. **Cross-encoder rerank** — top fused candidates are POSTed to the
   reranker service (default `localhost:48083`, `bge-reranker-v2-m3`) for
   pairwise scoring. Final results are sorted by rerank score.

## Behavior

- Hybrid search is always on for project-scope queries — both legs run
  unconditionally when the retrieval stack is reachable.
- Library scope (`scope: "libraries"`) follows the same pipeline against the
  `lib:NAME` project_id namespace.
- If the retrieval stack is unreachable, `semantic_search` returns a
  structured error with stack-inspection hints (see
  `src/tools/semantic/search.rs` error classification).

## Configuration

| Env | Default | Effect |
|---|---|---|
| `CODESCOUT_EMBEDDER_PROTOCOL` | `tei` | `tei` or `openai` (e.g. for Ollama) |
| `CODESCOUT_EMBEDDER_MODEL_NAME` | (empty) | Model id sent in OpenAI-protocol payloads |
| `CODESCOUT_QUERY_PREFIX` | (empty) | Prepended to query text only — for asymmetric models like Nomic |
| `CODESCOUT_RERANKER_PROTOCOL` | `tei` | `tei` or `infinity` |
| `CODESCOUT_RERANKER_MODEL` | (unset) | Override the reranker model id |

## Rebuilding the indexes

The hybrid indexes are rebuilt automatically at the end of every
`index(action="build")` call — no separate "build the sparse index" step.
Both legs share the same chunk set and are upserted into Qdrant atomically.
