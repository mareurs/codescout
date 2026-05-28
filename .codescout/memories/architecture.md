# codescout — Architecture

## Module Structure (src/)

```
src/
  server.rs          — CodeScoutServer (MCP ServerHandler), tool registry, request dispatch
  agent/             — Agent, ActiveProject, project state, write locking, per-project config
  tools/
    core/            — Tool trait, ToolContext, OutputGuard, RecoverableError
    symbols.rs       — symbol navigation (LSP + tree-sitter)
    references.rs    — find all references via LSP
    call_graph.rs    — transitive call graph (BFS, configurable direction + depth)
    edit_code.rs     — structural code editing (LSP-aware; action=replace/insert/remove/rename)
    edit_file.rs     — exact string replacement in files
    edit_markdown.rs — heading-addressed markdown editing
    create_file.rs   — new file creation
    semantic_search.rs — vector search (Qdrant + optional cross-encoder reranker)
    memory.rs        — per-project markdown memory (read/write/list/semantic)
    librarian.rs     — artifact registry operations (find/get/update/event/refresh)
    onboarding.rs    — project onboarding + system prompt generation + ONBOARDING_VERSION
    workspace.rs     — workspace activate/status/list
    index.rs         — semantic index build/status/cancel
    tree.rs          — filesystem exploration (glob + recursive listing)
    grep.rs          — regex search across files
    run_command.rs   — shell command execution
    output.rs        — OutputGuard (progressive disclosure enforcement)
  lsp/               — LSP client, mux, per-language servers, circuit breaker
  librarian/         — SQLite artifact catalog (find.rs, get.rs, update.rs, events.rs, refresh.rs)
  prompts/           — source.md (two surfaces), builders.rs, source.rs (slice extractor)
  embed/             — embedding integration (delegates to codescout-embed crate)
```

## Key Abstractions

- **`CodeScoutServer`** (`server.rs`) — MCP `ServerHandler` impl; owns the tool registry;
  all `CallToolRequest`s flow through `call_tool_inner()`
- **`Tool` trait + `ToolContext`** (`tools/core/types.rs`) — every tool implements `call()`;
  `call_content()` is the MCP entry point (handles output buffer routing)
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
5. Calls `tool.call_content()` → `tool.call()` + buffer routing
6. Success → `CallToolResult::success`; Error → `route_tool_error()`:
   - `RecoverableError` → `isError: false` with structured JSON guidance
   - Other errors → `isError: true`
7. Post-process: strip project root prefixes, log duration

## Prompt Surfaces

Three surfaces, two editable via `src/prompts/source.md`:
- **`server_instructions`** slice — injected at every MCP session start; no cache, no version bump needed
- **`onboarding_prompt`** slice — drives stored per-project system prompt; bump `ONBOARDING_VERSION` in `onboarding.rs` to refresh
- **`build_system_prompt_draft()`** in `builders.rs` — generated per-project context; also version-gated

Test `server::tests::prompt_surfaces_reference_only_real_tools` catches stale tool names across all three surfaces at build time.