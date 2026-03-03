# code-explorer Architecture

## Layer Structure

```
MCP Layer (rmcp) → 23 registered tools
        ↓
Agent / Orchestrator (Arc<RwLock<AgentInner>>)
Active project state: root, config, memory, library_registry
        ↓
LSP Client | AST Engine | Git Engine | Embedding Engine
        ↓
Storage: SymbolIndex, EmbeddingIndex (sqlite-vec), MemoryStore, IncrementalCache
```

## Tool Registry (23 tools)

| Category | File | Tools |
|---|---|---|
| File (6) | `src/tools/file.rs` | read_file, list_dir, search_pattern, create_file, find_file, edit_file |
| Workflow (2) | `src/tools/workflow.rs` | onboarding, run_command |
| Symbol (9) | `src/tools/symbol.rs` | find_symbol, list_symbols, goto_definition, hover, find_references, replace_symbol, remove_symbol, insert_code, rename_symbol |
| Semantic (2) | `src/tools/semantic.rs` | semantic_search, index_project |
| Library (1) | `src/tools/library.rs` | list_libraries |
| Memory (1) | `src/tools/memory.rs` | memory (action: read/write/list/delete) |
| Config (2) | `src/tools/config.rs` | activate_project, project_status |

**Not registered as MCP tools** (internal use):
- `src/tools/git.rs` — git_blame, file_log (used by dashboard)
- `src/tools/ast.rs` — list_functions, list_docs (tree-sitter, used internally by symbol tools)
- `src/tools/usage.rs` — get_usage_stats (surfaced via dashboard only)

## Key Abstractions

- `Tool` trait (`src/tools/mod.rs:167`) — name, description, input_schema, async call
- `OutputGuard` (`src/tools/output.rs:35`) — Exploring (compact, ≤200 items) vs Focused (full, paginated)
- `RecoverableError` (`src/tools/mod.rs:54`) — isError:false + hint; sibling calls not aborted
- `LspClientOps` / `LspProvider` (`src/lsp/ops.rs`) — testable LSP abstraction
- `Embedder` trait (`src/embed/mod.rs:33`) — embedding backend abstraction
- `ProjectStatus` tool — combines old `get_config` + `index_status` + usage summary

## Agent Locking

`Arc<RwLock<AgentInner>>` — multiple readers OK, exclusive write lock for project switches.

## Error Routing (`src/server.rs`)

- `RecoverableError` → `isError: false` + `{"error":"…","hint":"…"}`
- `anyhow::bail!` → `isError: true` (fatal)

## Test Count

932 tests passing (900 unit + 10 integration + 22 other).
