# codescout-embed — Project Overview

## Purpose
Shared embedding primitives library used by both `codescout` (the MCP server) and
`librarian-mcp`. Provides text chunking, embedding vector generation, and a unified
factory for selecting the right backend at runtime.

## Tech Stack
- **Language:** Rust (async via `tokio`, trait object dispatch)
- **Async trait:** `async-trait` for object-safe async methods
- **Remote backend:** `reqwest` (optional, `remote-embed` feature) — OpenAI-compatible HTTP API
- **Local backend:** `fastembed` v5 (optional, `local-embed` feature) — ONNX Runtime + HuggingFace model hub
- **Error handling:** `anyhow` throughout; no `thiserror` custom error types

## Optional Features
- `remote-embed` — enables `RemoteEmbedder` and `probe_ollama`; unlocks Ollama, OpenAI, and any custom OpenAI-compatible endpoint
- `local-embed` — enables `LocalEmbedder`; downloads ONNX models to `~/.cache/huggingface/hub/` on first use

## Key Dependencies
- `anyhow`, `thiserror`, `serde`, `serde_json`, `tokio`, `tracing`
- `reqwest` (optional), `fastembed` (optional)

## Runtime Requirements
- **Remote mode:** accessible HTTP server implementing `/v1/embeddings`
- **Local mode:** models downloaded on first use (~22MB for AllMiniLML6V2Q, up to ~547MB for NomicEmbedTextV15)
- No runtime requirements for the chunker (pure Rust, no features needed)

## Default Model
`AllMiniLML6V2Q` (384-dimensional, quantized, ~22MB) — the conservative default when no prefix or URL is specified and `local-embed` is enabled.

## Supported Local Models
- `AllMiniLML6V2Q` / `AllMiniLML6V2` — 384d, recommended default
- `NomicEmbedTextV15Q` / `NomicEmbedTextV15` — 768d, higher quality
- `JinaEmbeddingsV2BaseCode` — 768d, code-specialized
- `BGESmallENV15Q` / `BGESmallENV15` — 384d (deprecated: GPU-only, crashes on CPU)
