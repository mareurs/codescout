# codescout-embed — Conventions

## Error Handling

- Uses `anyhow::bail!` throughout (no `RecoverableError` — this is a library crate).
- Empty/whitespace-only batch → `bail!` immediately (do not pass to server; old sentinel
  `vec![vec![0.0; 1]]` approach was removed — it silently corrupted the dim-check downstream).
- `custom:` model prefix → hard error with migration hint to use `url` field.
- Missing `OPENAI_API_KEY` → descriptive error naming the env var.
- Ollama unreachable → descriptive error with fallback options listed.
- Unknown local model → `bail!` listing all 7 supported `local:` model IDs.

## Async Patterns

- `LocalEmbedder::new()` is async; heavy ONNX work is on `tokio::task::spawn_blocking`.
- `LocalEmbedder::embed()` also uses `spawn_blocking` (fastembed 5 `embed()` is `&mut self`).
- Mutex (std, blocking) wraps the fastembed model — serialises concurrent embed calls.
- `RemoteEmbedder` is fully async (`reqwest` + tokio).

## Feature Gating

- All `local.rs` code is `#[cfg(feature = "local-embed")]`.
- All `remote.rs` code is `#[cfg(feature = "remote-embed")]`.
- `lib.rs` has fine-grained `cfg` blocks per resolver step in `create_embedder_with_config`.
- `chunk_size_for_model` is feature-free — knows about local model token counts but
  avoids importing local.rs to keep it unconditionally available.

## Naming Conventions

- `model_spec` — full spec string including prefix (e.g. `"local:AllMiniLML6V2Q"`)
- `model_id` / `bare_model` / `bare` — model name after stripping prefix
- `chunk_size` — in characters
- `chunk_overlap` — in characters
- `max_tokens` — token budget; `chunk_markdown` converts to chars via `× 4`
- `RawChunk` — pre-embedding chunk with line provenance; used by indexing pipeline
- `Embedding` = `Vec<f32>`; `Vec<Embedding>` = one vector per input text

## Testing Approach

- Pure logic tests (chunker): `#[test]` in `mod tests` at bottom of `chunker.rs`.
  No `#[ignore]` needed — no I/O.
- Remote embedder tests: `#[tokio::test] #[ignore = "requires running Ollama"]` for live
  HTTP tests; only `probe_ollama_errors_when_unreachable` runs in CI (uses port 1).
- Local model tests: `#[test] #[ignore = "requires fastembed model download"]`.
- Smoke test in `lib.rs::smoke::crate_builds` — `#[test] fn crate_builds()` — trivially
  passes; exists to confirm the crate compiles with no features.
- No integration test directory — all tests are in-module.
- Test constants: `const MODEL: &str = "nomic-embed-text"` for Ollama tests.

## HTTP Client Configuration

- Per-request timeout: 300 seconds (handles slow GPU-discovery Ollama startups).
- `probe_ollama` uses a separate 2-second client for fast connectivity check.
- `rustls-no-provider` feature on reqwest — caller must install ring provider via `Once`.
- Response body capped at 32 MiB before JSON decode.

## Batch Processing Constants (`remote.rs`)

- `BATCH_SIZE = 32` texts per HTTP request
- `MAX_RETRIES = 3` (0, 500ms, 1000ms backoff; doubles each retry)
- `INITIAL_BACKOFF_MS = 500`
- 4xx errors are NOT retried; 5xx and network errors are.
