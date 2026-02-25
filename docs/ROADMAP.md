# Roadmap

See the detailed implementation plan: [`plans/2026-02-25-v1-implementation-plan.md`](plans/2026-02-25-v1-implementation-plan.md)

## Quick Status

| Phase | Description | Sprints | Status |
|-------|-------------|---------|--------|
| 0 | Architecture Foundation (ToolContext) | 0.1 | **Done** |
| 1 | Wire Existing Backends | 1.1–1.4 | **Done** |
| 2 | Complete File Tools | 2.1 | **Done** |
| 3 | LSP Client | 3.1–3.5 | **Done** |
| 4 | Tree-sitter AST Engine | 4.1–4.2 | **Done** |
| 5 | Polish & v1.0 | 5.1–5.3 | **In progress** |

## What's Built

- 30 tools across 8 categories (file, workflow, symbol, AST, git, semantic, memory, config)
- LSP client with transport, lifecycle, document symbols, references, definition, rename
- Tree-sitter symbol extraction + docstrings for Rust, Python, TypeScript, Go, Java, Kotlin
- Embedding pipeline: chunker, SQLite index, remote + local embedders
- Git integration: blame, log, diff via git2
- Persistent memory store with markdown-based topics
- Progressive disclosure output (exploring/focused modes via OutputGuard)
- MCP server over stdio (rmcp)
- 232 tests (227 passing, 5 ignored)

## What's Next

- HTTP/SSE transport (in addition to stdio)
- Additional tree-sitter grammars
- Additional LSP server configurations
- sqlite-vec integration for vector similarity (currently pure-Rust cosine)
- Companion Claude Code plugin: `code-explorer-routing`
