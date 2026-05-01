# Project Overview — codescout

## Purpose
codescout is an MCP (Model Context Protocol) server that gives LLMs IDE-grade code intelligence. It exposes symbol-level navigation, semantic search, and code editing tools so agents can work with large codebases semantically rather than reading raw files.

## Version & Repository
- Version: 0.10.0 (as of 2026-05-01)
- Repo: https://github.com/mareurs/codescout
- Binary: `codescout` (src/main.rs); Library: `codescout` (src/lib.rs)

## Tech Stack
- **Language:** Rust (MSRV 1.75), async with tokio
- **MCP SDK:** rmcp (stdio + HTTP/SSE transports)
- **LSP:** lsp-types, custom JSON-RPC client in src/lsp/
- **AST:** tree-sitter (Rust, Python, TypeScript, Go, Java, Kotlin, Bash, HTML, CSS)
- **Git:** git2
- **Vector store:** rusqlite (bundled SQLite) + sqlite-vec (vec0 KNN tables)
- **Error handling:** anyhow + custom RecoverableError
- **Serialization:** serde, serde_json, toml
- **HTTP (dashboard/transport):** axum, tower-http (feature-gated)
- **Embeddings:** codescout-embed crate (local ONNX via fastembed, or remote OpenAI-compatible)

## Runtime Requirements
- LSP servers installed for languages used (rust-analyzer, pyright, typescript-language-server, etc.)
- For local embeddings: first run downloads model (~20–300 MB) to ~/.cache/huggingface/hub/
- Optional Ollama or OpenAI API key for remote embeddings

## Key Features
1. Symbol navigation (list, find, replace, insert, remove, rename) via LSP + AST
2. Semantic code search via embeddings (sqlite-vec KNN)
3. Project memory with staleness tracking (anchor sidecars)
4. Multi-project workspace support
5. Progressive disclosure output (OutputGuard, OutputBuffer @ref system)
6. Cross-process write serialization (advisory file lock + in-process async mutex)
7. Dashboard (Axum HTTP server, opt-in via `codescout dashboard`)
8. Library registry (read-only navigation of third-party dependencies)

## Workspace Structure
- `src/` — main codescout binary/library
- `crates/codescout-embed/` — embedding engine (local + remote)
- `crates/librarian-mcp/` — optional librarian sub-server (workspace doc index)
- `tests/` — integration, regression, LSP, e2e, cross-process tests
- `tests/fixtures/` — multi-language fixture projects (Rust, Python, TS, Java, Kotlin)
