# codescout-embed — Conventions

## Error Handling
- All errors use `anyhow`; no custom error enums with `thiserror`
- `anyhow::bail!()` for input-driven failures (unknown model, unreachable server, wrong config)
- `.map_err(|e| anyhow::anyhow!(...))` for converting foreign errors with context
- Error messages are user-facing and actionable: always suggest alternatives or next steps
- 4xx HTTP errors from embedding server → immediate bail (no retry); 5xx → retry with backoff

## Async Patterns
- `async-trait` crate for all async trait methods
- CPU-heavy work (ONNX session init, fastembed embed) always goes through `tokio::task::spawn_blocking`
- `Arc<Mutex<fastembed::TextEmbedding>>` because fastembed v5 requires `&mut self` on embed
- `Arc<AtomicUsize>` for cached_dims in `RemoteEmbedder` so clones share the lazily-populated value

## Feature Gating
- `remote-embed` and `local-embed` are both opt-in; the core (`Embedder` trait, `RawChunk`, splitters) has no feature dependencies
- `lib.rs` uses many `#[cfg(feature = "...")]` guards in `create_embedder_with_config`
- Feature-gated modules (`local`, `remote`) are conditionally compiled with `#[cfg(feature)]` on `pub mod`

## Public API Surface (lib.rs re-exports)
- `Embedder`, `Embedding` — trait + type alias
- `RawChunk`, `split`, `split_markdown`, `chunk_markdown` — chunking primitives
- `chunk_size_for_model` — model-aware chunk size calculator
- `create_embedder`, `create_embedder_with_config` — factory functions
- `embed_one` — convenience wrapper for single-query embed

## Testing
- Chunker tests are pure unit tests (no async, no network) — fast and always run
- Remote tests use `#[tokio::test]` and require a live Ollama instance; they test: nonzero dims, batch consistency, semantic similarity ordering, large batch handling, URL normalization, API key precedence
- Local tests cover `parse_model` name mapping — no network needed but require `local-embed` feature
- `#[test] fn crate_builds()` smoke test in `lib.rs` (just `2+2==4`)

## Naming Conventions
- Model specs use `prefix:model-name` format: `local:`, `ollama:`, `openai:`; bare model name = local fallback
- fastembed variant names are CamelCase matching the enum: `AllMiniLML6V2Q`, `NomicEmbedTextV15Q`
- Ollama/OpenAI model names are kebab-case strings: `nomic-embed-text`, `text-embedding-3-small`

## Key Invariants
- `RawChunk.start_line` is always 1-indexed
- `RemoteEmbedder.dimensions()` returns 0 before first embed — callers must handle this
- Empty/whitespace-only input strings are replaced with zero-vectors; never sent to server
- `chunk_size_for_model` is not user-configurable — derived from model spec to prevent misconfiguration
