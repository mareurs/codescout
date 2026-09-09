# codescout — Code Explorer Guidance

## Entry Points

- `src/server.rs::CodeScoutServer::from_parts` (:306) — tool registry; start here for tool inventory
- `src/tools/core/types.rs` — `Tool` trait (:770), `ToolContext` (:59); read before adding/modifying a tool
- `src/tools/mod.rs` — the LIVE module index; `src/tools/` is grouped (`symbol/`, `semantic/`, `markdown/`, `memory/`, `config/`, `edit_file/`, `run_command/`), not flat
- `src/agent/mod.rs::Agent::new` (:465) — project activation and workspace discovery
- `src/librarian/tools/` — one file per doc verb: `find.rs`, `get.rs`, `update.rs`, `event_create.rs`, `augment.rs`, `doctor.rs`
- `crates/codescout-embed/src/lib.rs` — embedding factory + chunk-size formula

## Key Abstractions

- `Tool` + `ToolContext` (`src/tools/core/`) — every tool implements `call()`; `call_content()` is the MCP entry point
- `OutputGuard` (`src/tools/output.rs`) — exploring/focused two-mode progressive disclosure
- `RecoverableError` (`src/tools/core/`) — `isError: false`; sibling parallel calls survive
- `Agent` / `ActiveProject` (`src/agent/mod.rs`) — project state; tools access via `ctx.agent.with_project()`
- `CodeScoutServer` (`src/server.rs`) — MCP `ServerHandler`; every call flows through `call_tool_inner()` (:1055)

## Search Tips

- Good queries: "OutputGuard cap_items", "route_tool_error", "RecoverableError", "chunk_size_for_model"
- Avoid: "tool", "error", "file" — too broad
- Locate by name, not by path: `symbols(name=X)` finds a symbol wherever it now lives — the tree has been regrouped before
- Fixture projects (`tests/fixtures/*`) have no semantic index — use `grep`/`symbols(path=...)` there instead

## Navigation Strategy

1. Know the name → `symbols(name=X)`, then `symbols(name_path=..., include_body=true)` for the body
2. Know only the concept → `semantic_search(query)`; exact string → `grep(pattern, glob=...)`
3. Who calls it → `references(symbol, path)`, never `grep`
4. Before any structural edit → `call_graph(symbol, path, direction="callers")` for blast radius; `direction="callees"` to trace flow
5. Bug work → `doc(action="find", kind="bug", filter={"status": {"in": ["open","taken","investigating","zombie"]}})` before filing a new one
6. Markdown → `read_file` (heading-addressed) / `edit_file` (heading+action); a librarian-managed tracker needs `doc(action="update", patch={body_edits:[...]})` instead

## Project Rules

- The gate is FOUR commands and the ORDER is load-bearing: `./scripts/fmt-mine.sh` → `cargo clippy --workspace --all-targets --features local-embed -- -D warnings` → `cargo test --workspace --no-default-features` (lean, THIRD) → `cargo test --workspace` (default, LAST, chained with `;` not `&&`). Detail: memory `development-commands`
- `cargo build` (dev) does not refresh the live MCP binary — only `cargo build --release`/`cargo rb`, then `/mcp`
- Write tools return `json!("ok")` only — never echo content back
- `RecoverableError` for expected failures, `anyhow::bail!` for genuine bugs
- Tool rename/addition touches FOUR prompt surfaces (two `source.md` slices, `builders.rs`, this file); bump `ONBOARDING_VERSION` only for the `onboarding_prompt` slice
- Subagents MUST restore the home project after activating a different workspace project
- Cite a fix by SHA **and** patch-id — `experiments` is rebased routinely and the SHA alone dies
