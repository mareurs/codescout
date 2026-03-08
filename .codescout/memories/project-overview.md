# codescout

## Purpose
Rust MCP server giving LLMs IDE-grade code intelligence: symbol navigation via LSP,
semantic search via embeddings, file ops, git integration, persistent memory, and GitHub
integration. Designed for use with Claude Code; a companion routing plugin (in
`../claude-plugins/code-explorer-routing/`) enforces that LLMs use codescout tools
rather than raw shell reads on source files.

## Tech Stack
- **Language:** Rust 1.75+ (MSRV enforced in CI)
- **MCP SDK:** `rmcp 0.1` (stdio + SSE transports)
- **LSP:** JSON-RPC clients for 9 languages via `lsp-types 0.97`
- **AST:** `tree-sitter` with grammars for Rust/Python/TypeScript/Go/Java/Kotlin
- **Embeddings:** SQLite via `rusqlite` (bundled); `remote-embed` (reqwest, Ollama/OpenAI-compatible) or `local-embed` (fastembed ONNX) — feature flags
- **GitHub:** shells to `gh` CLI subprocess — requires `gh` installed and authenticated
- **Dashboard:** `axum 0.8` (behind `dashboard` feature, opt-in CLI subcommand)

## Runtime Requirements
- Rust stable ≥ 1.75
- An LSP server per language used (rust-analyzer, pyright, typescript-language-server, etc.)
- For semantic search: Ollama or compatible embedding API (or `local-embed` feature)
- For GitHub tools: `gh` CLI installed and authenticated (`gh auth login`)
- No required env vars — all config is per-project in `.codescout/project.toml`

## Key Feature Flags
- `default`: remote-embed + dashboard
- `local-embed`: ONNX-based local embeddings (downloads model ~20–300MB on first use)
- `e2e-*`: integration tests requiring real LSP servers installed
