# codescout

## Purpose
Rust MCP server giving Claude Code (and any MCP-compatible LLM client) IDE-grade code
intelligence: symbol navigation via LSP, semantic search via SQLite embeddings, structural
code editing, git integration, and project memory. Inspired by Serena.

## Tech Stack
- **Language:** Rust 2021 (MSRV 1.75)
- **MCP SDK:** rmcp 0.1 (stdio + HTTP/SSE transports)
- **Async runtime:** tokio (full features)
- **LSP protocol:** lsp-types 0.97 with custom JSON-RPC transport
- **Semantic search:** rusqlite + sqlite-vec (bundled) — NOTE: sqlite-vec extension NOT loaded;
  currently uses pure-Rust cosine similarity (see gotchas)
- **AST parsing:** tree-sitter grammars for Rust, Python, TypeScript, Go, Java, Kotlin
- **Git:** git2 0.19
- **HTTP (GitHub tools):** shell `gh` CLI subprocess — not direct HTTP API

## Runtime Requirements
- LSP servers must be installed separately per language (rust-analyzer, pyright, etc.)
- `gh` CLI required for GitHub tools
- Embedding index: optional; requires an OpenAI-compatible endpoint or local fastembed
  (`--features local-embed`) for semantic search
- Default embed model: `mxbai-embed-large-v1`

## Feature Flags
- `remote-embed` (default): HTTP OpenAI-compatible embeddings
- `local-embed`: fastembed ONNX Runtime (downloads ~20-300MB model on first use)
- `dashboard` (default): Axum web UI (`codescout dashboard --project .`)

## Tool Count
28 tools registered (not 23 — CLAUDE.md is stale). See `src/server.rs::from_parts()`.
