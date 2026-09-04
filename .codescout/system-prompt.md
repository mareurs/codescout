# codescout — Code Explorer Guidance

## Entry Points

- `src/server.rs::CodeScoutServer::from_parts` (:307) — all tools registered here; start for tool inventory
- `src/tools/core/types.rs` — `Tool` trait + `ToolContext`; read before adding or modifying any tool
- `src/tools/mod.rs` — the LIVE module index. `src/tools/` is grouped (`symbol/`, `semantic/`, `markdown/`, `memory/`, `config/`, `edit_file/`, `run_command/`), not flat
- `src/agent/mod.rs::Agent::new` (:444) — project activation and state wiring
- `src/librarian/tools/` — one file per doc verb: `find.rs`, `get.rs`, `update.rs`, `event_create.rs`, `augment.rs`, `doctor.rs`, `link_scan/`
- `crates/codescout-embed/src/lib.rs` — embedding factory + chunk size formula

## Key Abstractions

- `Tool` + `ToolContext` (`src/tools/core/`) — every tool implements `call()`; `call_content()` is the MCP entry point
- `OutputGuard` (`src/tools/output.rs`) — enforces exploring/focused two-mode progressive disclosure
- `RecoverableError` (`src/tools/core/`) — maps to `isError: false`; prevents sibling parallel-call abort
- `Agent` / `ActiveProject` (`src/agent/mod.rs`) — project state; tools access via `ctx.agent.with_project()` (:1107)
- `CodeScoutServer` (`src/server.rs`) — MCP `ServerHandler`; every call flows through `call_tool_inner()` (:1014)

## Search Tips

- Good queries: "OutputGuard cap_items", "route_tool_error", "RecoverableError", "strip_paths_in_value", "FilterNode compile SQL", "chunk_size_for_model"
- Avoid: "tool", "error", "file" — too broad
- **Locate by name, not by path**: `symbols(name=X)` finds a symbol wherever it now lives. Guessing `src/tools/<file>.rs` fails — the tree has been regrouped
- Fixture projects have no semantic index — use `grep(pattern, path="tests/fixtures/<name>/src")` or `symbols(path=...)`
- To verify a tree-sitter extractor fix, use `edit_code` on the target: `symbols(path)` routes to LSP and masks AST extractor bugs

## Navigation Strategy

1. Know the name → `symbols(name=X)`; then `symbols(name_path=..., include_body=true)` for the body
2. Know only the concept → `semantic_search(query)`; exact string → `grep(pattern, glob=...)`
3. Who calls it → `references(symbol, path)`, never `grep`
4. Before any refactor → `call_graph(symbol, path, direction="callers")` for blast radius; `direction="callees"` to trace flow
5. Bug or regression work → `doc(action="find", kind="bug", filter={"status": {"in": ["open", "taken", "investigating", "zombie"]}})` before filing anything new — `status="open"` alone hides `taken` (a live session holds it; check before starting), `investigating` (worked, no live owner) and `zombie` (recurring-but-unconfirmed — a "has this come back?" check, not a task to pick up)
6. Markdown → `read_file` (heading-addressed) / `edit_file` (heading+action); librarian-managed trackers refuse direct edits, use `doc(action="update", patch={body_edits: [...]})`

## Project Rules

- **The gate is FOUR commands and the ORDER is load-bearing**: `cargo fmt` → `cargo clippy --workspace --all-targets --features local-embed -- -D warnings` → `cargo test --workspace --no-default-features` (lean, THIRD) → `cargo test --workspace` (default, LAST). The bare narrow forms pass trees CI fails; the lean lane last would leave a librarian-less binary in the shared `target/`. Detail: memory `development-commands`
- Dashboard tests require `--features dashboard`; `cargo test --lib` silently skips them
- Write tools return `json!("ok")` only — never echo content back
- `RecoverableError` for expected failures, `anyhow::bail!` for genuine bugs
- Tool rename/addition: update all 4 prompt surfaces (including `.codescout/system-prompt.md`); bump `ONBOARDING_VERSION` only for `onboarding_prompt` changes
- Subagents MUST restore the home project after activating a different workspace project
- Cite a fix by SHA **and** patch-id — `experiments` is rebased routinely and the SHA dies
