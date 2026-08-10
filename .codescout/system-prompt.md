# codescout — Code Explorer Guidance

## Entry Points

- `src/server.rs::CodeScoutServer::from_parts` — all tools registered here; start for tool inventory
- `src/tools/core/types.rs` — `Tool` trait + `ToolContext`; read before adding or modifying any tool
- `src/agent/mod.rs::Agent::new` — project activation and state wiring
- `crates/codescout-embed/src/lib.rs` — embedding factory + chunk size formula
- `src/librarian/` — SQLite artifact catalog (find.rs, get.rs, update.rs, events.rs)

## Key Abstractions

- `Tool` trait + `ToolContext` (`src/tools/core/`) — every tool implements `call()`; `call_content()` is the MCP entry point
- `OutputGuard` (`src/tools/output.rs`) — enforces exploring/focused two-mode progressive disclosure
- `RecoverableError` — maps to `isError: false`; prevents sibling parallel tool call abort
- `Agent` / `ActiveProject` (`src/agent/mod.rs`) — project state; tools access via `ctx.agent.with_project()`
- `CodeScoutServer` (`src/server.rs`) — MCP `ServerHandler`; all `CallToolRequest`s flow through `call_tool_inner()`

## Search Tips

- Good queries: "OutputGuard cap_items", "route_tool_error", "RecoverableError", "strip_paths_in_value"
- codescout-embed: "Embedder trait backend", "chunk_size_for_model", "RemoteEmbedder batching"
- Librarian: "FilterNode compile SQL", "artifact find hidden statuses", "audit_doc_refs"
- Avoid: "tool", "error", "file" (too broad)
- For a specific tool: `symbols("src/tools/<category>.rs")` + `symbols(name=..., include_body=true)`
- Fixture projects have no semantic index — use `grep(pattern, path="tests/fixtures/<name>/src")` or `symbols(path=...)` directly
- `symbols(path)` routes to LSP when available; to verify a tree-sitter extractor fix, use `edit_code` on the target symbol — LSP output masks AST extractor bugs

## Navigation Strategy

1. New task on a tool → `symbols("src/tools/<file>.rs")` + `symbols(name=..., include_body=true)`
2. Cross-cutting change → `semantic_search` across `src/` + check all 3 prompt surfaces
3. Before any refactor → `call_graph(symbol, path, direction="callers")` for blast radius; `direction="callees"` for flow tracing
4. Bug in symbol editing → check `docs/issues/` for open trackers first
5. LSP behavior question → `symbols("src/lsp/")` then targeted body reads
6. Embedding question → `symbols("crates/codescout-embed/src/")` first
7. Fixture inspection → `symbols("tests/fixtures/<lang>-library/src/")` — read-only targets

## Project Rules

- `cargo fmt && cargo clippy -- -D warnings && cargo test` before every completion — use `cargo test`, NOT `--lib` (integration tests live in `tests/`)
- Dashboard tests require `--features dashboard`; `cargo test --lib` silently skips them
- Write tools return `json!("ok")` only — never echo content back
- `RecoverableError` for expected failures, `anyhow::bail!` for genuine bugs
- Use `edit_code` for all structural code edits; `edit_markdown` for `.md` files
- Tool rename/addition: update all 3 prompt surfaces + bump `ONBOARDING_VERSION` only for `onboarding_prompt` surface changes
- Subagents MUST restore home project after activating a different workspace project
