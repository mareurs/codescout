# codescout — Project Overview

## Purpose

codescout is a Rust MCP server that gives LLMs IDE-grade code intelligence. It exposes
symbol-level navigation, semantic search, persistent memory, and multi-language LSP
integration so AI coding agents can navigate and edit code without reading full files.
Works with Claude Code, Cursor, GitHub Copilot, and any MCP-capable client.

## Tech Stack

- **Language:** Rust (edition 2021, MSRV 1.75)
- **Async:** tokio (full features)
- **MCP protocol:** rmcp (transport-io, server, macros, elicitation, schemars)
- **LSP types:** lsp-types 0.97
- **AST parsing:** tree-sitter (Rust, Python, TypeScript, Go, Java, Kotlin + HTML/CSS/Bash)
- **Embeddings:** codescout-embed sibling crate (local ONNX via fastembed, remote via OpenAI-compat API)
- **Vector store:** SQLite + sqlite-vec (vec0 virtual tables, KNN cosine search)
- **Git:** git2
- **HTTP server:** axum (dashboard feature)
- **CLI:** clap

## Key Dependencies

- `codescout-embed` (workspace crate) — local + remote embedding backends
- `librarian-mcp` (workspace crate) — markdown artifact registry (separate binary)
- `rusqlite` (bundled) — all persistent storage
- `rmcp` — MCP SDK used for server registration and tool dispatch

## Runtime Requirements

- Rust 1.75+
- Language-specific LSP servers installed separately (rust-analyzer, pyright, etc.)
- Optional: Ollama or GPU for non-default embedding models

## Binary / Crate Layout

- `src/` — main `codescout` binary + library crate
- `crates/codescout-embed/` — embedding abstraction (local ONNX + remote API)
- `crates/librarian-mcp/` — standalone librarian-mcp binary (markdown artifact registry)
- `tests/` — integration tests + LSP symbol tests + e2e tests
- `tests/fixtures/` — language fixture projects (rust, python, typescript, java, kotlin)

## Version

v0.9.0 (see Cargo.toml). Licensed MIT. Public repo: github.com/mareurs/codescout.
