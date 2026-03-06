# Architecture

See `docs/ARCHITECTURE.md` and `CLAUDE.md § Project Structure` for the full layer diagram.
This memory captures what the code reveals beyond those docs.

## Key Abstractions

| Type | File | Role |
|---|---|---|
| `Tool` trait | `src/tools/mod.rs:228` | Core tool abstraction; `call_content()` handles buffering |
| `CodeScoutServer` | `src/server.rs` | rmcp ServerHandler; dispatches all 28 tools |
| `Agent` / `AgentInner` | `src/agent.rs` | Project state orchestrator; RwLock-guarded |
| `LspManager` / `LspClient` | `src/lsp/manager.rs`, `client.rs` | Per-language LSP pool |
| `OutputGuard` | `src/tools/output.rs` | Progressive disclosure enforcer |

## Data Flow: Tool Call

```
MCP client → call_tool(name, args)
  → CodeScoutServer::call_tool_inner()
    → security check (path_security::check_tool_access)
    → ToolContext { agent, lsp, output_buffer, progress }
    → timeout (project.toml: tool_timeout_secs)
    → UsageRecorder::record_content(tool_name, || tool.call_content(input, ctx))
      → tool.call(input, ctx) → Result<Value>
      → size check: > MAX_INLINE_TOKENS?
          yes → OutputBuffer::store_tool() → @tool_* ref + hint JSON
          no  → pretty-printed JSON inline
    → route_tool_error(e):
        RecoverableError → isError:false, {ok:false, error, hint?}
        LSP -32800       → isError:false, specialized hint
        other            → isError:true (fatal)
    → strip_project_root_from_result()  ← all absolute paths removed
```

## Data Flow: Semantic Search

```
SemanticSearch::call()
  → agent.get_or_create_embedder(model)  ← cached by model name
  → embed_one(query) → Vec<f32>
  → embed::index::search_scoped(conn, query_vec, scope)
      → pure-Rust cosine similarity (sqlite-vec NOT active)
  → OutputGuard::cap_items() + staleness check
```

## Error Routing (3-way)

`route_tool_error` in `src/server.rs:262`:
1. `RecoverableError` → `isError:false` — sibling parallel calls NOT aborted by Claude Code
2. LSP `code -32800` (RequestCancelled) → also `isError:false` with Kotlin-specific hint
3. Everything else → `isError:true` (fatal)

## LSP Concurrency Pattern

`LspManager::get_or_start()` uses a watch-channel barrier: first caller becomes "starter"
and sends tx, concurrent callers clone the rx and wait. On starter success, waiters read
the cache. On failure, a waiter becomes the new starter. `StartingCleanup` RAII guard
removes the barrier on any exit. See `src/lsp/manager.rs:78`.

## Testability Seam

`LspProvider` and `LspClientOps` traits (`src/lsp/ops.rs`) allow `MockLspClient` /
`MockLspProvider` injection in tests without live LSP servers. All symbol tools receive
`ctx.lsp: Arc<dyn LspProvider>`.

## Invariants

| Rule | Why it exists |
|---|---|
| Write tools call `validate_write_path()` before any I/O | Violations must return `RecoverableError`, not crash or silently write outside project root |
| `RecoverableError` for expected failures, `anyhow::bail!` for genuine tool failures | MCP `isError:true` aborts sibling parallel calls in Claude Code; wrong classification breaks parallel workflows |
| All write tools return `json!("ok")` — never echo content back | Echoing wastes tokens with zero information gain (caller already sent the content) |
| `OutputGuard` used for all variable-length output | Per-tool truncation logic diverges; `OutputGuard` is the single enforcement point |

## Strong Defaults

| Default | When to break it |
|---|---|
| `detail_level: "exploring"` (compact, capped at 200) | LLM knows exactly what it wants → pass `detail_level: "full"` |
| Absolute paths stripped from all output | Never — strip happens in `call_tool_inner`, after the tool runs |
| System prompt from `.codescout/system-prompt.md` | Falls back to `project.toml::project.system_prompt`; TOML field exists but file takes precedence |
