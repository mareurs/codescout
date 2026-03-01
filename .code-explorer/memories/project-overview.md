# code-explorer

## Purpose
Rust MCP server that gives LLMs IDE-grade code intelligence — symbol-level navigation, semantic search, git integration. Designed for Claude Code as the host agent. Inspired by Serena.

## Tech Stack
- **Language:** Rust (edition 2021, MSRV 1.75)
- **Async runtime:** tokio (full features)
- **MCP protocol:** rmcp (Rust MCP SDK) — server, macros, stdio + SSE transport
- **LSP types:** lsp-types 0.97
- **AST parsing:** tree-sitter (Rust, Python, Go, TS, Java, Kotlin grammars)
- **Embedding storage:** rusqlite (bundled) + sqlite-vec for cosine similarity
- **Embedding backends:** reqwest (remote/Ollama) or fastembed (local ONNX)
- **Git:** git2 (libgit2 bindings)
- **CLI:** clap 4 (derive)
- **Dashboard:** axum 0.8 + tower-http (optional "dashboard" feature)

## Runtime Requirements
- Rust toolchain (stable, 1.75+)
- LSP servers for target languages (install via `./scripts/install-lsp.sh`)
- For semantic search: Ollama with an embedding model, or local-embed feature
- For dashboard: enabled by default via "dashboard" feature flag

## Feature Flags
- `remote-embed` (default) — HTTP-based embedding via Ollama/OpenAI API
- `local-embed` — CPU embedding via fastembed/ONNX
- `dashboard` (default) — web UI on port 8099
- `e2e` / `e2e-rust` / `e2e-python` etc. — integration tests requiring real LSP servers
