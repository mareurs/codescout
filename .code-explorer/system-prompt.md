# code-explorer — Code Explorer Guidance

## Entry Points
- `src/main.rs` — CLI entry: `Cli` struct, `Commands` enum (Start, Index, Dashboard)
- `src/server.rs` — MCP server: `CodeExplorerServer`, `run()`, `route_tool_error()`
- `src/tools/mod.rs` — `Tool` trait (line 167), `ToolContext`, `RecoverableError`
- `src/agent.rs` — `Agent` orchestrator, `ActiveProject` state

## Key Abstractions
- `Tool` trait (`src/tools/mod.rs:167`) — interface for all 31 tools
- `OutputGuard` (`src/tools/output.rs:35`) — progressive disclosure: Exploring vs Focused
- `LspClientOps` / `LspProvider` (`src/lsp/ops.rs`) — testable LSP abstraction
- `Embedder` trait (`src/embed/mod.rs:33`) — embedding backend abstraction
- `RecoverableError` (`src/tools/mod.rs:54`) — soft errors with hints

## Search Tips
- "tool dispatch" or "call_tool" → `src/server.rs`
- "progressive disclosure" or "output mode" → `src/tools/output.rs`
- "embedding pipeline" or "semantic index" → `src/embed/`
- "symbol navigation" or "LSP" → `src/tools/symbol.rs` + `src/lsp/`
- Avoid: "data", "utils", "handler" (too generic)

## Navigation Strategy
1. `read_memory("architecture")` — understand the layer structure
2. `list_symbols("src/tools/")` — see all tool implementations
3. For a specific tool: `find_symbol("ToolName", include_body=true)`
4. For cross-cutting concerns: `semantic_search("your concept")`
5. For LSP internals: start at `src/lsp/ops.rs` (traits), then `src/lsp/client.rs`

## Project Rules
- Write responses must return `json!("ok")` — never echo content back
- Use `RecoverableError` for input-driven failures, `anyhow::bail!` for system failures
- All tools must go through `OutputGuard` for output sizing
- Read `docs/PROGRESSIVE_DISCOVERABILITY.md` before adding/modifying tools
- Check/update `docs/TODO-tool-misbehaviors.md` during every work session
