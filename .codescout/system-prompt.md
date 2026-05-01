# codescout — Code Explorer Guidance

## Entry Points

- `src/server.rs::CodeScoutServer::from_parts` — all tools registered here; start for tool inventory
- `src/tools/mod.rs` — `Tool` trait definition; read before adding or modifying any tool
- `src/agent.rs::Agent::new` — project activation and state wiring
- `crates/codescout-embed/src/lib.rs` — embedding factory + chunk size formula
- `crates/librarian-mcp/src/tools/mod.rs` — librarian ToolContext + all librarian tools
## Key Abstractions

- `Tool` trait (`src/tools/mod.rs`) — name/description/schema/call/call_content/format_compact
- `OutputGuard` (`src/tools/output.rs`) — progressive disclosure; every tool with variable output uses it
- `RecoverableError` (`src/tools/mod.rs`) — recoverable vs fatal error routing
- `LspProvider` / `LspClientOps` (`src/lsp/ops.rs`) — LSP abstraction; `MockLspClient` for tests
- `Agent` / `ActiveProject` (`src/agent.rs`) — project state; all tools access via `ctx.agent.with_project()`
- `Embedder` trait (`crates/codescout-embed/src/embedder.rs`) — local ONNX or remote HTTP backend
## Search Tips

- Good queries: "OutputGuard cap_items", "route_tool_error", "RecoverableError", "strip_project_root"
- codescout-embed: "Embedder trait backend", "chunk_size_for_model", "RemoteEmbedder batching"
- librarian-mcp: "FilterNode compile SQL", "TimeMachine state_at", "index_repo_sync pipeline"
- Avoid: "tool", "error", "file" (too broad)
- For a specific tool: `symbols("src/tools/<category>.rs")` + `symbols(name=..., include_body=true)`
- For LSP flow: `semantic_search("get_or_start", project_id="code-explorer")`
- For call relationships: `call_graph(symbol, path)` — traces callers and callees; use before refactoring to assess impact
## Navigation Strategy

1. New task on a tool → `symbols("src/tools/<file>.rs")` + read body ranges
2. Cross-cutting change → `semantic_search` across `src/` + check all 3 prompt surfaces
3. Impact analysis before refactoring → `call_graph(symbol, path)` to trace callers/callees
4. Bug in symbol editing → read `docs/TODO-tool-misbehaviors.md` first
5. LSP behavior question → `symbols("src/lsp/client.rs")` then targeted reads
6. Embedding question → `symbols("crates/codescout-embed/src/")` first
7. Librarian question → `symbols("crates/librarian-mcp/src/")` first
8. Fixture inspection → `symbols("tests/fixtures/<lang>-library/src/")` — read-only targets
## Project Rules

- `cargo fmt && cargo clippy -- -D warnings && cargo test` before every completion
- Write tools return `json!("ok")` only — never echo content back
- `RecoverableError` for expected failures, `anyhow::bail!` for genuine bugs
- Use `edit_code` for all structural code edits (replaces old `replace_symbol`, `insert_code`, `remove_symbol`, `rename_symbol`)
- Read `docs/PROGRESSIVE_DISCOVERABILITY.md` before adding any tool with variable-length output
- When renaming tools: update all 3 prompt surfaces (see `CLAUDE.md § Prompt Surface Consistency`)
- GitHub tools shell to `gh` CLI — not HTTP; `sqlite-vec` is fully active (vec0 virtual tables with KNN search)
- Subagents MUST restore home project after activating a different workspace project
