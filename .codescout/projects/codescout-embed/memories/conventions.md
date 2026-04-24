# Conventions — codescout-embed

## Language & Style
- Pure Rust, workspace edition/license/authors inherited via `workspace = true`
- `async_trait` used for all async trait methods — required by the `Embedder`
  trait definition; all impls must carry `#[async_trait::async_trait]`
- Error handling: `anyhow::Result` throughout; `anyhow::bail!` for hard errors,
  `anyhow::anyhow!` for constructing error values
- No `thiserror` custom types in this crate — `anyhow` is sufficient at this layer

## Naming & Public API
- `Embedding = Vec<f32>` — the fundamental output type
- `RawChunk` — pre-embed text chunk with line provenance
- Factory functions are free functions in `lib.rs`, not constructors:
  `create_embedder`, `create_embedder_with_config`, `embed_one`,
  `chunk_size_for_model`
- Backend constructors live in `impl` blocks: `RemoteEmbedder::openai`,
  `::ollama`, `::custom`, `::from_url`; `LocalEmbedder::new`

## Feature Gates
- All `local-embed`-gated code uses `#[cfg(feature = "local-embed")]` at item level
- All `remote-embed`-gated code uses `#[cfg(feature = "remote-embed")]`
- Feature-independent code (chunker, trait, factory fn skeleton) compiles always
- The `let _ = &api_key;` suppression pattern used in `create_embedder_with_config`
  when `remote-embed` is disabled — avoids unused-variable warnings

## Line Number Convention
- All public chunker output uses **1-indexed** line numbers (both `start_line`
  and `end_line`). Internal loop indices are 0-indexed and converted on output:
  `start_line: start_line + 1`.

## Async Patterns
- CPU-heavy ONNX session creation runs on `tokio::task::spawn_blocking`
- `Mutex<fastembed::TextEmbedding>` is std (not tokio) — blocked inside
  `spawn_blocking`; the async `embed` method spawns a new blocking task per call

## Security Invariants
- Never send an API key over plaintext HTTP to a non-loopback host: enforced
  in both `RemoteEmbedder::custom` and `::from_url`
- `OPENAI_API_KEY` env var is the fallback for openai: prefix
- `EMBED_API_KEY` env var is the fallback for custom/from_url paths
- `OLLAMA_HOST` env var overrides the default `http://localhost:11434`

## Testing Approach
- Chunker has an inline `#[cfg(test)] mod tests` with unit tests covering:
  empty input, single chunk, 1-indexed start, overlap invariants,
  line continuity across chunks, markdown heading splits
- Remote embedder tests are in `#[cfg(test)] mod tests` in `remote.rs`:
  covers URL normalisation, loopback/HTTPS security, query prefix detection
- Local embedder tests: gated behind `#[cfg(feature = "local-embed")]` and
  `#[cfg_attr(not(feature = "local-embed"), ignore)]` — only run in CI with
  the feature enabled
- `lib.rs` has a trivial `smoke::crate_builds` test (no network/model needed)
- Tests that require a live server are not present — by design, to keep CI fast

## Model String Format Summary
| Prefix         | Backend              | Notes                              |
|----------------|----------------------|------------------------------------|
| `local:`       | LocalEmbedder (ONNX) | e.g. `local:AllMiniLML6V2Q`        |
| `ollama:`      | RemoteEmbedder       | probes reachability at startup     |
| `openai:`      | RemoteEmbedder       | requires API key                   |
| `custom:`      | — (hard error)       | use `url` field in project.toml    |
| (no prefix)    | local fallback       | tries LocalEmbedder, else error    |
| url + any      | RemoteEmbedder       | url takes priority over prefix     |
