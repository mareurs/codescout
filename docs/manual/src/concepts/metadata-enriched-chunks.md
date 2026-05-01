# Metadata-Enriched Chunks
Every code chunk stored in the semantic index now carries a short searchable header prepended to its embedding input. Headers encode file path, container context, and symbol name:

    src/embed/index.rs :: impl IndexStore :: fn build_index(force: bool)

This information was previously invisible to the embedding model — chunks were embedded as raw code bodies with no location context. Multi-concept keyword queries (the dominant query shape in real usage) now match on file path, container, and symbol name in addition to body content, giving them more surface area to match on.

## What changes

- Chunks have a new `metadata` column populated during indexing.
- Embedding input is `metadata + "\n" + content` when metadata is present.
- Search results are unchanged — users still see raw code content. The header is an embedding-only signal.
- Unknown languages and markdown files have `metadata = NULL` and embed only the body (no behavior change there).

## When it helps

- Queries that mention a file path or module name (`"embed index build"`)
- Queries that mention a struct/class name alongside a concept (`"IndexStore force rebuild"`)
- 3–10 word keyword queries — the dominant shape in production traffic

## When it won't help

- Queries that don't map to any code structure (cross-file architectural questions)
- Bare symbol lookups — use `symbols` instead, it's exact

## Schema migration

On first index after upgrading, the existing `chunks` and `chunk_embeddings` tables are dropped and rebuilt. Expect one reindex delay; thereafter indexing is incremental as usual.
