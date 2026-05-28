# codescout-embed — Project Overview

## Purpose

Shared embedding primitives crate used by both `codescout` (the main MCP server) and
`librarian-mcp`. Provides text chunking, the `Embedder` trait, and concrete backends for
local CPU embeddings (fastembed/ONNX) and remote HTTP embeddings (OpenAI-compatible APIs,
Ollama). Also provides `chunk_size_for_model` to derive safe chunk sizes from model specs.

## Package

- Crate: `codescout-embed` v0.1.0
- Path: `crates/codescout-embed/`
- Manifest: `crates/codescout-embed/Cargo.toml`

## Features (opt-in)

- `remote-embed` — HTTP-based embedding via OpenAI-compatible APIs (Ollama, OpenAI, llama.cpp,
  etc.). Pulls in `reqwest` (0.13, rustls-no-provider) + `rustls` (ring provider).
- `local-embed` — CPU-based ONNX embedding via `fastembed` v5. Downloads models to
  `~/.cache/huggingface/hub/` on first use (22 MB–547 MB depending on model).

Neither feature is enabled by default; callers choose what they need.

## Key Dependencies

- `anyhow` — error propagation throughout
- `async-trait` — `Embedder` trait is `async`
- `serde` / `serde_json` — HTTP request/response serialisation
- `tokio` — async runtime (workspace dependency)
- `reqwest` (optional, remote) — HTTP client with TLS, per-request 300s timeout
- `rustls` (optional, remote) — TLS with ring provider, installed once via `Once`
- `fastembed` v5 (optional, local) — ONNX Runtime wrapping HuggingFace models

## Supported Local Models

| Model ID | Dims | Size | Notes |
|---|---|---|---|
| `local:AllMiniLML6V2Q` | 384 | ~22 MB | Recommended default, quantized |
| `local:NomicEmbedTextV15Q` | 768 | ~158 MB | Higher quality, quantized |
| `local:NomicEmbedTextV15` | 768 | ~547 MB | Full f32 precision |
| `local:JinaEmbeddingsV2BaseCode` | 768 | ~300 MB | Code-specialized |
| `local:AllMiniLML6V2` | 384 | — | Full f32 precision |
| `local:BGESmallENV15Q` | 384 | — | Deprecated — GPU-only, crashes on CPU |
| `local:BGESmallENV15` | 384 | — | Full f32 precision |

## Model Spec Format

`create_embedder_with_config` and `chunk_size_for_model` accept a model spec string:
- `local:<ModelId>` — local ONNX backend
- `ollama:<model-name>` — Ollama running at `$OLLAMA_HOST` (default `localhost:11434`)
- `openai:<model-name>` — OpenAI API (needs `api_key` or `OPENAI_API_KEY`)
- `custom:` — **removed**; use explicit `url` field instead
- Bare string — treated as a local model name; `custom:` emits a migration hint
