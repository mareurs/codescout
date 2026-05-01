# codescout-embed — Conventions

## Language & Style
- Rust 2021 edition; workspace-level `edition`, `license`, `authors` fields
- `async-trait` for async trait methods (`#[async_trait::async_trait]`)
- `anyhow::Result` throughout; `thiserror` available but not currently used in structs

## Error Handling
- `anyhow::bail!` for user-facing configuration errors with actionable messages
- Error messages include concrete alternatives and example config snippets
- `unwrap()` only in tests or truly infallible paths (e.g. `Client::build`)
- `spawn_blocking` join errors propagated via `map_err` with context

## Feature Gates
- All remote HTTP code under `#[cfg(feature = "remote-embed")]`
- All fastembed/ONNX code under `#[cfg(feature = "local-embed")]`
- `lib.rs` guards module `pub mod` declarations with the same cfg attributes
- Dead-variable suppression via `let _ = &var` under `#[cfg(not(feature = ...))]`

## Naming
- Model strings use prefix convention: `local:ModelName`, `ollama:model-name`, `openai:model-name`
- fastembed model variant names match `fastembed::EmbeddingModel` enum variants exactly (PascalCase)
- Public chunker functions: `split` (generic), `split_markdown` (returns `RawChunk`), `chunk_markdown` (returns `Vec<String>`)

## Testing
- Unit tests in `mod tests` at bottom of each file
- `#[ignore = "requires running Ollama"]` for integration tests needing external services
- Chunker tests verify: coverage of all lines, line number accuracy, overlap behavior, content match
- `local.rs` tests use only `parse_model` (sync, no ONNX needed) — no `#[tokio::test]`
- Remote tests test constructor logic (sync); actual HTTP tests are `#[ignore]`
- Smoke test in `lib.rs` verifies crate compiles: `assert_eq!(2 + 2, 4)`

## Chunk Size Formula
- `chunk_size = (max_tokens × 0.85 × 3) as usize`
- 0.85 factor: 15% headroom for tokenization variance and control tokens
- 3 chars/token: conservative lower bound for code
- Unknown models fall back to 512 tokens
- Not user-configurable — derived from model spec to prevent misconfiguration

## HTTP Safety
- 300-second per-request timeout on embed HTTP client
- 2-second timeout on `probe_ollama`
- 32 MiB cap on response bodies
- API keys forbidden over plaintext HTTP unless loopback host
- Batch size: 32 texts per request
- Retry: 3 attempts, exponential backoff starting at 500ms, only on 5xx

## Workspace Integration
- `edition.workspace = true`, `license.workspace = true`, `authors.workspace = true`
- `anyhow`, `serde`, `serde_json`, `tokio`, `tracing`, `thiserror` from workspace
