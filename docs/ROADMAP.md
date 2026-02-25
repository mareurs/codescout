# Roadmap

See the detailed implementation plan: [`plans/2026-02-25-v1-implementation-plan.md`](plans/2026-02-25-v1-implementation-plan.md)

## Quick Status

| Phase | Description | Sprints | Status |
|-------|-------------|---------|--------|
| 0 | Architecture Foundation (ToolContext) | 0.1 | Not started |
| 1 | Wire Existing Backends | 1.1–1.4 | Not started |
| 2 | Complete File Tools | 2.1 | Not started |
| 3 | LSP Client | 3.1–3.5 | Not started |
| 4 | Tree-sitter AST Engine | 4.1–4.2 | Not started |
| 5 | Polish & v1.0 | 5.1–5.3 | Not started |

## What's Built

- 32 source files, 9 modules, 60 tests passing
- 4/27 tools working: `read_file`, `list_dir`, `search_for_pattern`, `execute_shell_command`
- MCP server over stdio (rmcp)
- Core libraries: chunker, embedding index, memory store, git blame/log, config
