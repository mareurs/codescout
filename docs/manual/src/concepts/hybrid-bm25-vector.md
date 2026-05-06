# Hybrid BM25 + Vector Retrieval

`semantic_search` now uses a hybrid retrieval pipeline combining dense vector search with sparse BM25 keyword search, fused via Reciprocal Rank Fusion (RRF).

## How it works

1. **Vector leg** — dense embedding search (unchanged behavior)
2. **BM25 leg** — keyword search over a Tantivy full-text index, built alongside the vector index during `index(action="build")`
3. **RRF fusion** — results from both legs are re-ranked using Reciprocal Rank Fusion (k=60)

The BM25 index uses a code-aware tokenizer that splits on camelCase, snake_case, file paths, and punctuation.

## Behavior

- Hybrid search activates automatically for project-scope queries when a BM25 index exists
- Other scopes (libraries, all) use pure vector search unchanged
- If the BM25 index is absent or corrupted, the tool falls back to pure vector search with a warning

## Building the BM25 index

The BM25 index is rebuilt automatically at the end of every `index(action="build")` call. No extra steps needed.

## Tokenizer

The `CodeTokenizer` splits text by:
- Non-alphanumeric separators (spaces, punctuation, path separators)
- Underscores (`snake_case` → `["snake", "case"]`)
- CamelCase boundaries (`parseModel` → `["parse", "model"]`)
