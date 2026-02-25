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
| 5 | Polish & v1.0 | 5.1–5.3 | Not started |

## What's Built

- 35 source files, 9 modules, 131 tests passing
- 29/29 tools working (file, workflow, memory, git, config, semantic, symbol, AST)
- LSP client: transport, lifecycle, document symbols, references, definition, rename
- Tree-sitter: symbol extraction + docstrings for Rust, Python, TypeScript, Go
- MCP server over stdio (rmcp)
- 0 tools remaining — all planned tools implemented
