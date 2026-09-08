# codescout — Architecture

## Module Structure (src/)

Verified against the tree 2026-09-08. **`src/tools/` is grouped into subdirectories by
concern, not flat** — the earlier version of this section listed a flat layout in which 12 of
17 paths no longer resolved.

```
src/
  server.rs          — CodeScoutServer (MCP ServerHandler), tool registry, request dispatch
  main.rs, lib.rs    — binary entry + library root
  agent/             — Agent, ActiveProject, project state, write locking, per-project config
  tools/
    mod.rs           — the module list; read this first, it is the authoritative index
    core/            — Tool trait, ToolContext (types.rs), OutputGuard (guards.rs),
                       RecoverableError, params.rs, path_strip.rs (PATH_KEYS/ROOT_KEYS
                       allowlist walker), write_ack.rs
    symbol/          — symbols.rs, references.rs, edit_code.rs, symbol_at.rs,
                       call_graph/, call_edges/, list_overview.rs, display.rs
    semantic/        — semantic_search.rs, index.rs
    markdown/        — read_markdown.rs, edit_markdown.rs, frontmatter.rs
    memory/          — per-project markdown memory (read/write/list/remember/recall)
    config/          — workspace activate/status/list  ← the `workspace` tool lives HERE
    edit_file/       — exact string replacement
    run_command/     — shell execution
    file_summary/    — file overview / heading maps
    create_file.rs, read_file.rs, grep.rs, tree.rs, output.rs, output_buffer.rs,
    guide.rs, guide_ledger.rs, library.rs, onboarding.rs (+ ONBOARDING_VERSION),
    probe.rs, peer.rs, rendezvous.rs, approve_write.rs, progress.rs,
    section_coverage.rs, command_summary.rs, edit_repair.rs, file_group.rs,
    format.rs, session_key.rs
```

**Two files were DELETED and must not be re-added to the flat-file list above** (a prior
version of this memory listed both, describing files that no longer exist). Fenced rather
than backticked, because `audit_doc_refs` reads a backticked path-shaped token as a live
citation and cannot tell a mention from a use:

```
src/tools/usage.rs   deleted
src/tools/ast.rs     deleted
```

Confirmed gone 2026-09-08: neither resolves on disk, and neither is declared in
`src/tools/mod.rs`. This is unrelated to the top-level `src/ast/` and `src/usage/`
directories, which are real,
separate modules declared in `src/lib.rs` (`src/usage/` is the MCP-call-only usage.db recorder;
`src/ast/` is unrelated AST tooling) — see the module list below.

```
  librarian/         — artifact catalog + the librarian/artifact tool family
    catalog/         — SQLite store, migrations, augmentation, worktree overlay
    tools/           — one file per verb: artifact.rs, find.rs, get.rs, create.rs,
                       update.rs, mv.rs, delete.rs, graft.rs, link.rs, graph.rs,
                       append_entry.rs, update_entry.rs, augment.rs, event_create.rs,
                       refresh.rs, refresh_stale.rs,
                       gather.rs, reindex.rs, context.rs, doctor.rs, link_scan/,
                       audit_doc_refs/, legibility_scan/, merge_worktree.rs,
                       tracker_design.rs, state_at.rs, workspace_state_at.rs, timeline.rs,
                       schema_validate.rs, constitution_check.rs, goal_aggregation.rs,
                       worktree.rs, scope.rs, render.rs, temp_write_guard.rs
                       (also present, not previously listed: audit_log.rs, librarian.rs, status.rs)
    augmentation_sidecar.rs, classify.rs, filter.rs, freshness.rs, frontmatter.rs,
    ids.rs, indexer.rs, statements.rs, workspace.rs, preview/, prompts/
  lsp/               — LSP client, mux, per-language servers, circuit breaker
  retrieval/         — vector backends, embedding config, reranker, transport
  prompts/           — source.md (TWO slices), builders.rs, source.rs (slice extractor),
                       guide_index.rs, guides/, workspace_onboarding_prompt.md, README.md
  embed/             — embedding integration (delegates to codescout-embed crate)
  operator_rules/    — operator profiles + compiled-in rule ledger
  usage/             — usage.db recorder (MCP calls only — Bash work is invisible to it)
  ast/ symbol/ git/ fs/ config/ memory/ library/ legibility/ peer/ platform/
  migrate/ dashboard/ mcp_resources/ util/ cli/ bin/
```

**Do not navigate from this listing — navigate with `symbols`.** A directory tree in prose
is a dated snapshot, and this one had rotted through a whole reorganisation without anything
failing. `src/tools/mod.rs` is the live index; `symbols(name=X)` finds a symbol wherever it
now lives. This section is here for orientation — which concern owns which subtree — not as
a path source.

## Tool Registry / Count

`CodeScoutServer::from_parts_with_env` (`src/server.rs:312`) builds the tool list. **19 tools
are always registered** (`ReadFile`, `Tree`, `Grep`, `CreateFile`, `EditFile`, `RunCommand`,
`Onboarding`, `ApproveWrite`, `Symbols`, `References`, `SymbolAt`, `CallGraph`, `EditCode`,
`Memory`, `SemanticSearch`, `Index`, `Workspace`, `Library`, `GetGuide`), plus **`doc` and
`librarian`** from `crate::librarian::adapters_for(lib_ctx)` when the `librarian` feature is
compiled in (default) and enabled at runtime — bringing the default advertised surface to
**21 tools**. Two more are runtime-conditional and NOT part of the baseline count: `peer`
(Unix-only, opt-in via `CODESCOUT_PEER_ENABLED`) and the debug-only `__probe_description_cap__`
(`CODESCOUT_PROBE=1`). Verified 2026-09-08 against `src/server.rs:312-380`.

A 2026-09-02 "tool-surface collapse" retired several call forms that must never reappear in a
prompt surface: `find_symbol`, `list_symbols`, `replace_symbol`, `insert_code`, `rename_symbol`,
`search_pattern`, `read_markdown`, `edit_markdown`, `artifact_augment`, `artifact_event`,
`artifact_refresh`, and the tool name `artifact` **when written as a call form with an opening
paren** — the bare noun is still correct prose, which is why the canonical list carries that one
entry with its paren attached. Canonical list: `DEPRECATED_TOOL_NAMES`, `src/prompts/mod.rs:1949`.

Do not "improve" the sentence above by writing the paren form literally. `reader_docs_contain_no_retired_call_forms`
scans `.codescout/` and cannot tell a mention from an invocation claim, so naming the retired
form exactly is what trips it — writing it without the paren is the documented escape.

## Key Abstractions

- **`CodeScoutServer`** (`server.rs`) — MCP `ServerHandler` impl; owns the tool registry;
  all `CallToolRequest`s flow through `call_tool_inner()` (`src/server.rs:1055`)
- **`Tool` trait** (`tools/core/types.rs:770`) **+ `ToolContext`** (`tools/core/types.rs:59`) —
  every tool implements `call()`; `call_content()` is the MCP entry point (handles output
  buffer routing, and — since the 2026-08-09 field-aware-path-strip work — the
  `PATH_KEYS`/`ROOT_KEYS` allowlist walk over the typed `Value`, strictly before
  buffering/formatting)
- **`Agent` / `ActiveProject`** (`agent/mod.rs`) — project state (config, memory, write lock);
  tools access it via `with_project(|p| ...)` (`agent/mod.rs:1169`); `with_project_at` /
  `with_project_at_mut` are the workspace-override variants
- **`OutputGuard`** (`tools/output.rs`) — enforces two-mode progressive disclosure:
  Exploring (compact, capped at 200 items) / Focused (full detail, paginated)
- **`RecoverableError`** — maps to `isError: false`; prevents sibling parallel tool call abort;
  all other errors map to `isError: true`
- **`WriteGuard`** (`agent/write_guard.rs`) — cross-process file lock for writes; names the
  current holder (path, elapsed hold time) on contention rather than a bare "someone holds it"

## Data Flow: MCP Tool Call

1. `ServerHandler::call_tool()` receives `CallToolRequest`
2. `call_tool_inner()` (`src/server.rs:1055`) resolves tool by name, checks access, parses JSON
3. Builds `ToolContext` (Agent, LspManager, output buffer, progress reporter)
4. Acquires write guard if mutating
5. Calls `tool.call_content()` → `tool.call()`, then the `PATH_KEYS`/`ROOT_KEYS` allowlist
   walk over the result `Value` (`src/tools/core/path_strip.rs`), then buffer routing —
   every downstream consumer sees already-relative values
6. Success → `CallToolResult::success`; Error → `route_tool_error()`:
   - `RecoverableError` → `isError: false` with structured JSON guidance
   - Other errors → `isError: true`
7. Post-process: append the once-per-activation `[codescout] paths are relative to <root>`
   banner (no text rewriting any more — that moved to step 5), log duration

## Prompt Surfaces

Four surfaces exist; three editable via `src/prompts/source.md` + `builders.rs`, and a fourth
generated-and-committed file:
- **`server_instructions`** slice — injected at every MCP session start; no cache, no version bump needed
- **`onboarding_prompt`** slice — drives stored per-project system prompt; bump `ONBOARDING_VERSION` in `onboarding.rs` to refresh
- **`build_system_prompt_draft()`** in `builders.rs` — generated per-project context; also version-gated
- **`.codescout/system-prompt.md`** — tracked in git, generated once by `onboarding` (workspace
  flow: `src/prompts/workspace_onboarding_prompt.md`), injected into every codescout session in
  this repo at project activation. Sweep it whenever the other three are swept;
  `onboarding(refresh_prompt=true)` regenerates it.

Tests `server::tests::prompt_surfaces_reference_only_real_tools`,
`prompts::tests::claude_md_contains_no_deprecated_tool_names`, and
`prompts::tests::claude_md_gate_lists_its_four_commands_in_the_load_bearing_order` catch stale
tool names / a moved gate sentence across these surfaces at build time.
