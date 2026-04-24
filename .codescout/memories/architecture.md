# codescout — Architecture

## Module Structure

```
src/
  server.rs         — CodeScoutServer: MCP ServerHandler, tool registration (29 tools),
                      call_tool_inner dispatch, route_tool_error, WriteGuard acquisition
  agent/            — Agent + ActiveProject: project state, workspace discovery, activation
    mod.rs          — Agent (RwLock<AgentInner>), ActiveProject, activate(), new()
    write_guard.rs  — WriteGuard: dual async-mutex + fs4 cross-process file lock
  tools/            — All tool implementations (Tool trait + every tool module)
    mod.rs          — Tool trait, ToolContext, RecoverableError, OutputGuard constants
    output.rs       — OutputMode (Exploring/Focused), OutputGuard, OverflowInfo, paginate()
    output_buffer.rs — OutputBuffer: @cmd_* / @file_* / @tool_* buffer refs
    symbol/         — LSP-backed symbol tools (find_symbol, list_symbols, replace_symbol, etc.)
    semantic.rs     — SemanticSearch, IndexProject, IndexStatus
    memory.rs       — Memory tool (read/write/list/delete/remember/recall/forget/refresh_anchors)
    onboarding.rs   — Onboarding tool + GatheredContext + ONBOARDING_VERSION
    run_command.rs  — RunCommand + @cmd_* buffer refs
  symbol/           — Domain-level symbol helpers (used by tools/symbol/ and future providers)
    mod.rs          — pub mod edit; pub mod query;
    query.rs        — 16 AST/LSP lookup, classification, validation, JSON-shaping functions
    edit.rs         — text-edit application, symbol editing helpers
  lsp/
    ops.rs          — LspClientOps trait, LspProvider trait (abstraction layer)
    client.rs       — LspClient: JSON-RPC transport, lifecycle, workspace/document symbols
    manager.rs      — LspManager: idle TTL eviction (Kotlin 2h, others 30min)
    mux/            — Kotlin LSP multiplexer (Unix socket proxy, MuxState, ClientTag routing)
  embed/
    index.rs        — build_index(), search_scoped(), search_scoped_vec0(), SQLite schema
    ast_chunker.rs  — Language-aware AST-guided code chunker
    drift.rs        — compute_file_drift(): content-hash + cosine chunk matching
    schema.rs       — CodeChunk, SearchResult types
    mod.rs          — Embedder trait, create_embedder() factory
  memory/           — MemoryStore: file-based .codescout/memories/ + semantic vec_memories
  config/           — ProjectConfig, GlobalConfig, WorkspaceConfig, Mode/Context enums
  ast/              — detect_language(), extract_symbols() via tree-sitter
  git/              — open_repo(), diff_tree_to_tree() (change detection for incremental index)
  library/          — LibraryRegistry, discovery, scope parsing, version staleness
  workspace.rs      — workspace.toml discovery, project topology
  prompts/          — server_instructions.md, onboarding_prompt.md, builders.rs, README.md
  dashboard/        — Axum web dashboard (tool stats, memories browser, index status)
```

## Key Abstractions

1. **`Tool` trait** (`src/tools/mod.rs:543`) — `name/description/input_schema/call/call_content/
   format_compact/availability/json_path_hint`. `call_content()` handles buffer routing by default;
   tools only override `call()`. Write tools return `json!("ok")`.

2. **`OutputGuard`** (`src/tools/output.rs`) — Enforces progressive disclosure: Exploring mode
   (default, cap 200 items) vs Focused mode (full detail, paginated). All variable-output tools
   use `guard.cap_items()` / `guard.cap_files()`.

3. **`RecoverableError`** (`src/tools/mod.rs:190`) — Expected input-driven failures → `isError:false`
   (sibling parallel calls not aborted). Carries `message`, optional `Guidance` (Hint/Warning/MustFollow),
   and structured `extra` fields. `anyhow::bail!` for genuine failures → `isError:true`.

4. **`Agent` / `ActiveProject`** (`src/agent/mod.rs`) — `Agent` holds `RwLock<AgentInner>` wrapping
   `Workspace` of discovered `Project`s. `ActiveProject` carries root, config, MemoryStore, LibraryRegistry,
   dirty_files, write_lock, and file_lock. All tools access via `ctx.agent.with_project()`.

5. **`LspProvider` / `LspClientOps`** (`src/lsp/ops.rs`) — Trait abstraction over real `LspClient` and
   `MockLspClient`. Tools always use the trait; tests swap in mock implementations.

6. **`WriteGuard`** (`src/agent/write_guard.rs`) — Dual-layer write exclusion: in-process async mutex +
   fs4 cross-process file lock on `.codescout/write.lock`. Acquired for the full duration of mutating
   tool calls; dropped on completion or cancellation.

## Data Flow: Tool Call (read path)

1. MCP client → `ServerHandler::call_tool()` → `call_tool_inner()`
2. Resolve tool from registry → `check_tool_access()` (read_only guard)
3. `build_context()` → `ToolContext { agent, lsp, output_buffer, progress, peer }`
4. Race `tool.call_content(input, ctx)` against `cancel_token` + optional timeout
5. Success → `post_process()` (strip project root from paths) → `CallToolResult::success(blocks)`
6. Error → `route_tool_error(e)`: `RecoverableError` → `isError:false`; other → `isError:true`

## Data Flow: Semantic Indexing

1. `IndexProject` tool → `build_index(project_root, force, cb)`
2. Phase 1: `find_changed_files()` — git diff → mtime → SHA-256 fallback chain
3. For each changed file: `detect_language()` → `ast_chunker::split_file()` → `Vec<CodeChunk>`
4. `embed_producer` + `db_writer` run concurrently via `mpsc::channel(1)`:
   - Producer: batches chunks → `embedder.embed_batch()` → sends `GroupReady`
   - Writer: receives → SQLite upsert into `chunks` + `chunk_embeddings` (vec0) + drift report
5. `search_scoped()` → KNN via `vec0` (`search_scoped_vec0`) or pure-Rust cosine fallback

## Design Principles

- **Progressive disclosure**: Compact by default (Exploring), full detail on demand (Focused).
  `OutputGuard` is the single enforcement point — not per-tool logic.
- **Token efficiency**: Tools minimize output; overflow produces actionable hints + `by_file` maps.
  Large results go to `@tool_*` buffers; agents query via `read_file(@ref, json_path=...)`.
- **No echo in write responses**: Mutation tools return `json!("ok")` only.
- **Agent-agnostic**: Error messages name codescout tools, not host-specific tools.
- **Three prompt surfaces**: `server_instructions.md`, `onboarding_prompt.md`, `builders.rs`.
  All must be updated together when tools change. Test: `prompt_surfaces_reference_only_real_tools`.

## Good semantic_search Queries

- `semantic_search("OutputGuard cap_items overflow hint", project="code-explorer")`
- `semantic_search("RecoverableError recoverable isError guidance", project="code-explorer")`
- `semantic_search("LSP client document symbols workspace symbols", project="code-explorer")`
- `semantic_search("incremental embedding index build changed files", project="code-explorer")`
- `semantic_search("write guard cross-process file lock mutex", project="code-explorer")`
