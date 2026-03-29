# Architecture

See `docs/ARCHITECTURE.md` for the full component diagram. This memory captures
wiring details NOT covered there.

## Source Tree

```
src/
├── main.rs          # CLI: start (MCP server), index, and dashboard subcommands
├── lib.rs           # Crate root for library/integration use
├── server.rs        # rmcp ServerHandler — bridges Tool trait to MCP, signal handling + graceful LSP shutdown
├── agent.rs         # Orchestrator: active project, config, memory
├── logging.rs       # --debug mode: file logging with rotation (tracing-appender)
├── config/          # ProjectConfig (.codescout/project.toml), modes
├── lsp/             # LSP types, server configs (9 langs), JSON-RPC client
├── ast/             # Language detection (20+ exts), tree-sitter parser
├── git/             # git2: blame, file_log, open_repo
├── embed/           # Chunker, SQLite index, RemoteEmbedder, schema, drift detection
├── library/         # LibraryRegistry, Scope enum, manifest discovery
├── memory/          # Markdown-based MemoryStore (.codescout/memories/)
├── usage/           # UsageRecorder: append-only SQLite call stats (usage.db)
├── prompts/         # LLM guidance: server_instructions.md, onboarding_prompt.md
├── tools/           # Tool implementations by category
│   ├── output.rs          #   OutputGuard: progressive disclosure (exploring/focused)
│   ├── output_buffer.rs   #   OutputBuffer: session-scoped LRU (@cmd_*/@file_* handles)
│   ├── progress.rs        #   ProgressReporter: MCP progress notifications
│   ├── format.rs          #   Shared format helpers (format_line_range, format_overflow, truncate_path)
│   ├── file.rs            #   read_file, list_dir, search_pattern, create_file, find_file, edit_file
│   ├── file_summary.rs    #   Smart per-type summarizers (source, markdown, JSON, TOML, YAML)
│   ├── workflow.rs        #   onboarding, run_command
│   ├── symbol.rs          #   9 LSP-backed tools (find_symbol, list_symbols, goto_definition, hover, remove_symbol, etc.)
│   ├── git.rs             #   git_blame, file_log (not registered; used by dashboard)
│   ├── semantic.rs        #   semantic_search, index_project, index_status
│   ├── github.rs          #   github_identity, github_issue, github_pr, github_file, github_repo
│   ├── library.rs         #   list_libraries, register_library
│   ├── memory.rs          #   memory (action: read/write/list/delete/remember/recall/forget/refresh_anchors)
│   ├── usage.rs           #   GetUsageStats (dashboard API; not an MCP tool)
│   ├── ast.rs             #   list_functions, list_docs (not registered; tree-sitter offline tools)
│   ├── command_summary.rs #   Smart output summarization, terminal filter detection
│   └── config.rs          #   activate_project, project_status
└── util/            # fs helpers, text processing, path security
```

## Tool Dispatch Pipeline (concrete flow)

`rmcp::ServerHandler::call_tool` → `call_tool_inner` (src/server.rs):
1. `find_tool(name)` — linear scan over `Vec<Arc<dyn Tool>>`
2. `check_tool_access(name, &security)` — denormalized match-arm gate in
   `src/util/path_security.rs`. Missing a write tool here bypasses access controls.
3. Build `ToolContext { agent, lsp, output_buffer, progress, peer, section_coverage }`
4. Apply `tool_timeout_secs` from `project.toml` (skipped for `index_project`, `onboarding`)
5. `UsageRecorder::record_content` wraps `tool.call_content()`
6. `route_tool_error`: `RecoverableError` → `isError:false` + JSON error/hint;
   LSP code -32800 (RequestCancelled) → recoverable with Kotlin multi-session hint;
   other errors → `isError:true`
7. `strip_project_root_from_result` removes absolute project prefix from all output text

## Output Routing (Tool trait default impl)

`call_content()` in `src/tools/mod.rs`:
- Small output (< `MAX_INLINE_TOKENS`) → pretty-printed JSON inline
- Large output → stored in `OutputBuffer::store_tool()` as `@tool_xxx` ref (LRU, 50 slots)
  Returns `{ output_id, summary, hint }` where hint points to `json_path` or line range

`OutputGuard` (src/tools/output.rs) enforces two modes:
- `Exploring` (default): cap at 200 items / 200 files, no body inclusion
- `Focused` (`detail_level: "full"`): paginated via offset/limit, includes bodies

## LSP Lifecycle

`LspManager::get_or_start(language, root)` (src/lsp/manager.rs):
- Fast path: cache hit by language key, checks `is_alive()` + workspace root match
- Slow path: watch-channel barrier prevents concurrent duplicate cold-starts for the
  same language. First caller becomes "starter"; others wait on receiver.
- `do_start()` registers `StartingCleanup` guard that removes barrier on any exit path
  (including async cancellation via tool timeout).

## Embedding Pipeline (build_index)

`build_index(root, force)` in `src/embed/index.rs`:
1. `find_changed_files()`: git diff → mtime → SHA-256 fallback
2. `ast_chunker::split_file()`: AST-aware chunking per language, respects chunk size config
3. Concurrent embedding with semaphore cap=4 (`JoinSet` over `Embedder::embed()`)
4. Single SQLite transaction: delete old chunks, insert new, upsert file hash
5. Drift detection (if enabled): cosine distance old→new embeddings → `drift_report` table
6. High-drift files → mark memory anchors stale

**sqlite-vec**: Extension loading is commented out. Pure-Rust cosine search loads ALL
chunk embeddings into memory for each query — known perf issue for large indexes.

## Memory Architecture (two tiers, one DB)

- **File store**: Markdown in `.codescout/memories/`, CRUD via `MemoryStore`
- **Semantic store**: Vector embeddings in `.codescout/embeddings.db` (tables separate
  from code chunks). `remember`/`recall`/`forget` actions on the `memory` tool.
  Auto-classification via `classify_bucket()` in `src/memory/classify.rs`.
- **Anchor sidecars**: `.anchors.toml` alongside each memory tracks source file paths
  referenced in content. `project_status` checks SHA-256 of anchored files to surface
  stale memories. Regenerated on each `write`; cleared via `refresh_anchors` action.

## LSP Multiplexer (Kotlin-specific)

The `mux` subcommand (`--hidden` in CLI) spawns a Unix-socket-based multiplexer for
languages that support only one server instance per workspace (Kotlin's kotlin-lsp).

Flow:
- `LspManager::do_start(language, root)` for mux-enabled languages calls
  `ensure_mux_running(language, root)` in `src/lsp/mux/process.rs`
- Mux socket path: `/tmp/codescout-<lang>-mux-<workspace_hash>.sock`
- Lock path: `/tmp/codescout-<lang>-mux-<workspace_hash>.lock`
- Auto-terminates after `idle_timeout` (default 300s) with no connected clients
- Multiple codescout instances share one kotlin-lsp process via the mux socket

Kotlin-specific: passes `--system-path=/tmp/codescout-mux-kotlin-lsp` and
`GRADLE_USER_HOME=/tmp/codescout-mux-gradle` to isolate from per-PID paths.

## ToolContext Fields

`ToolContext` (src/tools/mod.rs):
| Field | Type | Description |
|-------|------|-------------|
| `agent` | `Agent` | Shared state: project config, memories, registry |
| `lsp` | `Arc<dyn LspProvider>` | Mock-swappable LSP access |
| `output_buffer` | `Arc<OutputBuffer>` | Session LRU store for @tool_* refs |
| `progress` | `Option<Arc<ProgressReporter>>` | MCP progress notifications |
| `peer` | `Option<Peer<RoleServer>>` | MCP peer for elicitation; None in tests |
| `section_coverage` | `Arc<Mutex<SectionCoverage>>` | Tracks markdown sections read in session |

`ctx.elicit::<T>(message)` sends an MCP elicitation request to the user. Returns
`Ok(Some(T))` on response, `Ok(None)` if client doesn't support it, or
`RecoverableError` if user declined/cancelled.

## Unregistered Tool Structs

Several tool structs exist in code but are NOT registered in `from_parts`:
- `IndexLibrary` (src/tools/library.rs) — ghost tool, never wired up
- `WriteMemory`, `ReadMemory`, `ListMemories`, `DeleteMemory` (src/tools/memory.rs)
  — internal structs, `Memory` mega-dispatcher wraps them
- `ListFunctions`, `ListDocs` (src/tools/ast.rs) — tree-sitter offline tools, used by dashboard
- `GetUsageStats` (src/tools/usage.rs) — dashboard API only

## Server Instructions

Pre-computed at construction in `from_parts` via `build_server_instructions()`.
For stdio: reflects state at startup, never refreshed mid-session.
For HTTP/SSE: each connection gets fresh instructions.
Custom instructions loaded from `.codescout/system-prompt.md` if present.

## Invariants

| Rule | Why it exists |
|---|---|
| Write tools must appear in `check_tool_access` match arm | Missing entry bypasses access gate silently |
| `RecoverableError` for expected failures, `bail!` for bugs | Controls whether Claude Code aborts sibling parallel calls |
| Write tools return `json!("ok")` | Echoing content wastes tokens with zero info gain |
| `call_content()` is the MCP entry point, NOT `call()` | `call_content` handles buffer routing; `call` is the pure logic layer |

## Strong Defaults

| Default | When to break it |
|---|---|
| `OutputGuard::Exploring` (200 item cap) | Use `detail_level: "full"` when you need all items |
| LSP for symbol resolution | Use AST tools (`ListFunctions`, `ListDocs`) for offline/no-LSP scenarios |
| Remote embeddings (Ollama) | Use `local-embed` feature when no Ollama available |
