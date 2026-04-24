# codescout-embed

## Purpose
Shared embedding primitives crate used by both `codescout` (main server) and
`librarian-mcp`. Provides: a backend-agnostic `Embedder` trait, two concrete
backends (local ONNX via fastembed, remote OpenAI-compatible HTTP), a
language-aware text chunker, and factory functions to construct the right
backend from a model string.

## Tech Stack
- **Rust**, edition from workspace
- **fastembed** (v5, optional feature `local-embed`) — ONNX Runtime via
  HuggingFace model hub; models cached in `~/.cache/huggingface/hub/`
- **reqwest** (v0.13, optional feature `remote-embed`) — async HTTP client for
  `/v1/embeddings` compatible endpoints
- **async-trait** — required for async methods in the `Embedder` trait
- **anyhow / thiserror** — error handling
- **tokio** — async runtime (from workspace)

## Features (Cargo)
- `local-embed` — enables `LocalEmbedder` (fastembed/ONNX)
- `remote-embed` — enables `RemoteEmbedder` (reqwest HTTP)
- Both are optional; the crate compiles with neither (only chunker + trait)

## Source Files
- `src/lib.rs` — public API: re-exports, `chunk_size_for_model`, `embed_one`,
  `create_embedder`, `create_embedder_with_config`
- `src/embedder.rs` — `Embedding` type alias + `Embedder` trait definition
- `src/chunker.rs` — `RawChunk`, `split`, `split_markdown`, `chunk_markdown`
- `src/local.rs` — `LocalEmbedder` (fastembed, ONNX, CPU)
- `src/remote.rs` — `RemoteEmbedder` (OpenAI-compatible HTTP API)

## Runtime Requirements
- **local-embed**: First use downloads chosen model (22MB–300MB) to
  `~/.cache/huggingface/hub/`. No server needed.
- **remote-embed**: Requires a running embedding server (Ollama, OpenAI API,
  or any `/v1/embeddings`-compatible endpoint). Ollama host defaults to
  `http://localhost:11434`; override with `OLLAMA_HOST` env var.
  OpenAI requires `OPENAI_API_KEY` or explicit `api_key` config.

## Key Consumers
- `src/embed/mod.rs` (code-explorer main crate) — re-exports and uses
  `create_embedder_with_config` to build the embedder on demand
- `crates/librarian-mcp` — uses `Embedder` trait for document indexing
