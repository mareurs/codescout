# codescout — Architecture

## Module Structure (src/)

Verified against the tree 2026-08-31. **`src/tools/` is grouped into subdirectories by
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
    create_file.rs, read_file.rs, grep.rs, tree.rs, ast.rs, output.rs, output_buffer.rs,
    guide.rs, guide_ledger.rs, library.rs, onboarding.rs (+ ONBOARDING_VERSION),
    usage.rs, probe.rs, peer.rs, rendezvous.rs, approve_write.rs, progress.rs,
    section_coverage.rs, command_summary.rs, edit_repair.rs, file_group.rs,
    format.rs, session_key.rs
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
## Key Abstractions

- **`CodeScoutServer`** (`server.rs`) — MCP `ServerHandler` impl; owns the tool registry;
  all `CallToolRequest`s flow through `call_tool_inner()`
- **`Tool` trait + `ToolContext`** (`tools/core/types.rs`) — every tool implements `call()`;
  `call_content()` is the MCP entry point (handles output buffer routing, and — since the
  2026-08-09 field-aware-path-strip work — the `PATH_KEYS`/`ROOT_KEYS` allowlist walk over
  the typed `Value`, strictly before buffering/formatting)
- **`Agent` / `ActiveProject`** (`agent/mod.rs`) — project state (config, memory, write lock);
  tools access it via `with_project(|p| ...)`
- **`OutputGuard`** (`tools/output.rs`) — enforces two-mode progressive disclosure:
  Exploring (compact, capped at 200 items) / Focused (full detail, paginated)
- **`RecoverableError`** — maps to `isError: false`; prevents sibling parallel tool call abort;
  all other errors map to `isError: true`

## Data Flow: MCP Tool Call

1. `ServerHandler::call_tool()` receives `CallToolRequest`
2. `call_tool_inner()` resolves tool by name, checks access, parses JSON
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

Three surfaces, two editable via `src/prompts/source.md`:
- **`server_instructions`** slice — injected at every MCP session start; no cache, no version bump needed
- **`onboarding_prompt`** slice — drives stored per-project system prompt; bump `ONBOARDING_VERSION` in `onboarding.rs` to refresh
- **`build_system_prompt_draft()`** in `builders.rs` — generated per-project context; also version-gated

Test `server::tests::prompt_surfaces_reference_only_real_tools` catches stale tool names across all three surfaces at build time.
