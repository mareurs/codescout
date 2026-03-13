# codescout — Code Explorer Guidance

## Entry Points
- `src/server.rs::CodeScoutServer::from_parts` — all 29 tools registered here; start for tool inventory
- `src/tools/mod.rs:239` — `Tool` trait definition; read before adding or modifying any tool
- `src/agent.rs::Agent::new` — project activation and state wiring

## Key Abstractions
- `Tool` trait (`src/tools/mod.rs`) — name/description/schema/call/call_content/format_compact
- `OutputGuard` (`src/tools/output.rs`) — progressive disclosure; every tool with variable output uses it
- `RecoverableError` (`src/tools/mod.rs:78`) — recoverable vs fatal error routing
- `LspProvider` / `LspClientOps` (`src/lsp/ops.rs`) — LSP abstraction; `MockLspClient` for tests
- `Agent` / `ActiveProject` (`src/agent.rs`) — project state; all tools access via `ctx.agent.with_project()`

## Search Tips
- Good queries: "OutputGuard cap_items", "route_tool_error", "RecoverableError", "strip_project_root"
- Avoid: "tool", "error", "file" (too broad)
- For a specific tool implementation: `list_symbols("src/tools/<category>.rs")`
- For LSP flow: `search_pattern("get_or_start", path="src/lsp")`

## Navigation Strategy
1. New task on a tool → `list_symbols("src/tools/<file>.rs")` + `read_file` line ranges for bodies
2. Cross-cutting change → `search_pattern` across `src/` + check all 3 prompt surfaces
3. Bug in symbol editing → read `docs/TODO-tool-misbehaviors.md` first
4. LSP behavior question → `list_symbols("src/lsp/client.rs")` then targeted `read_file`

## Project Rules
- `cargo fmt && cargo clippy -- -D warnings && cargo test` before every completion
- Write tools return `json!("ok")` only — never echo content back
- `RecoverableError` for expected failures, `anyhow::bail!` for genuine bugs
- Read `docs/PROGRESSIVE_DISCOVERABILITY.md` before adding any tool with variable-length output
- When renaming tools: update all 3 prompt surfaces (see `CLAUDE.md § Prompt Surface Consistency`)
- GitHub tools shell to `gh` CLI — not HTTP; `sqlite-vec` is present but NOT active (pure-Rust cosine)
