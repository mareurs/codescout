# codescout-embed — Project Overview

## Purpose
Shared embedding primitives library used by `codescout` (the main MCP server) and
`librarian-mcp`. Provides: text chunking, embedding trait abstraction, local CPU
embeddings (ONNX via fastembed), and remote HTTP embeddings (OpenAI-compatible APIs).

## Crate
- **Name:** `codescout-embed`
- **Root:** `crates/codescout-embed`
- **Entry point:** `src/lib.rs`
- **Type:** library crate (no binary)

## Source Files
- `src/lib.rs` — public API surface, `chunk_size_for_model`, `create_embedder_with_config`, `create_embedder`
- `src/embedder.rs` — `Embedder` trait + `Embedding` type alias
- `src/chunker.rs` — `split`, `split_markdown`, `chunk_markdown`, `RawChunk`
- `src/local.rs` — `LocalEmbedder` (fastembed/ONNX; feature-gated: `local-embed`)
- `src/remote.rs` — `RemoteEmbedder` (reqwest; feature-gated: `remote-embed`)

## Key Dependencies
- `anyhow`, `thiserror` — error handling
- `async-trait` — async trait methods
- `tokio` — async runtime
- `serde`, `serde_json` — JSON serialization for HTTP requests/responses
- `fastembed` (optional, `local-embed` feature) — ONNX Runtime embeddings
- `reqwest` (optional, `remote-embed` feature) — HTTP client for remote embeddings

## Feature Flags
- `local-embed` — enables `LocalEmbedder` (downloads ONNX model to `~/.cache/huggingface/hub/`)
- `remote-embed` — enables `RemoteEmbedder` and `probe_ollama`
- Default: neither feature active (chunker + trait only)

## Runtime Requirements
- `local-embed`: ONNX Runtime (bundled by fastembed); first run downloads model (~22–547 MB)
- `remote-embed`: network access to embedding server (Ollama, OpenAI, etc.)
- No LSP or external tooling required
