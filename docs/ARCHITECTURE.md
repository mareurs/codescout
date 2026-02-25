# Architecture

## Overview

code-explorer is an MCP server that gives LLMs IDE-grade code intelligence. It exposes symbol-level tools so agents can navigate and edit code semantically.

```
┌────────────────────────────────────────────────────────┐
│              MCP Layer (rmcp)                           │
│   CodeExplorerServer → registered tools (27)           │
└────────────────────────────────────────────────────────┘
                          ↓
┌────────────────────────────────────────────────────────┐
│              Agent / Orchestrator                       │
│   ProjectManager, ToolRegistry, ConfigSystem           │
└────────────────────────────────────────────────────────┘
          ↓                    ↓                  ↓                  ↓
┌─────────────────┐  ┌──────────────────┐  ┌──────────────┐  ┌──────────────────┐
│  LSP Client     │  │  AST Engine      │  │  Git Engine  │  │  Embedding       │
│  (30+ langs)    │  │  (tree-sitter)   │  │  (git2-rs)   │  │  Engine          │
└─────────────────┘  └──────────────────┘  └──────────────┘  └──────────────────┘
          ↓                    ↓                                        ↓
┌────────────────────────────────────────────────────────────────────────────────┐
│                         Storage / Index Layer                                   │
│   SymbolIndex, EmbeddingIndex (sqlite-vec), MemoryStore, IncrementalCache      │
└────────────────────────────────────────────────────────────────────────────────┘
```

## Components

### MCP Server (`src/server.rs`)

Bridges the internal `Tool` trait to rmcp's `ServerHandler`. All tools are registered as `Vec<Arc<dyn Tool>>` and dispatched dynamically in `call_tool`.

- Stdio transport via `rmcp::transport::stdio()`
- Tool errors returned as `CallToolResult::error` (surfaces to LLM, doesn't crash)
- HTTP transport planned but not yet implemented

### Agent (`src/agent.rs`)

Central orchestrator holding active project state behind `RwLock`. Manages:
- Active project root and config
- Memory store reference
- Project detection and activation

### Config (`src/config/`)

- `project.rs` — `ProjectConfig` loaded from `.code-explorer/project.toml` or sensible defaults. Holds embeddings config, ignored paths, project metadata.
- `modes.rs` — `Mode` (Planning/Editing/Interactive/OneShot) and `Context` (Agent/DesktopApp/IdeAssistant) enums.

### LSP Client (`src/lsp/`)

- `symbols.rs` — Language-agnostic `SymbolInfo`/`SymbolKind` types with `From<lsp_types::SymbolKind>`
- `servers/mod.rs` — Default LSP server configs for 9 languages (rust-analyzer, pyright, typescript-language-server, gopls, jdtls, kotlin-language-server, clangd, omnisharp, solargraph)
- `client.rs` — `LspClient` stub, needs tower-lsp/jsonrpc implementation

### AST Engine (`src/ast/`)

- `mod.rs` — `detect_language()` supporting 20+ file extensions; `extract_symbols()` delegates to parser
- `parser.rs` — Stub returning empty vec, awaiting tree-sitter grammar integration

### Git Engine (`src/git/`)

- `mod.rs` — `open_repo()`, `head_short_sha()`, `file_log()` returning `Vec<CommitSummary>` via git2
- `blame.rs` — `blame_file()` returning `Vec<BlameLine>` with author, date, SHA, line content

### Embedding Engine (`src/embed/`)

Inspired by [cocoindex-code](../cocoindex-code/) — embedded semantic search with zero external services.

- `schema.rs` — `CodeChunk` and `SearchResult` data types
- `chunker.rs` — Language-aware recursive text splitter tracking 1-indexed line numbers. Handles overlap via character-count estimation.
- `index.rs` — SQLite schema (`files`, `chunks`, `chunk_embeddings`), CRUD operations, pure-Rust cosine similarity search, `build_index()` for incremental project indexing
- `remote.rs` — `RemoteEmbedder` supporting OpenAI, Ollama, and custom API endpoints
- `mod.rs` — `Embedder` trait, `create_embedder()` factory, `embed_one()` helper

**sqlite-vec**: Extension loading is commented out (TODO). Pure-Rust cosine fallback works but loads all embeddings into memory.

### Memory (`src/memory/`)

Markdown-based persistent store in `.code-explorer/memories/`. Supports nested topics (path-like), directory traversal protection, CRUD operations.

### Tools (`src/tools/`)

Each tool implements the `Tool` trait (`name`, `description`, `input_schema`, `async call`). Organized by category:

| Category | File | Tools | Status |
|----------|------|-------|--------|
| File | `file.rs` | read_file, list_dir, search_for_pattern | Working |
| Workflow | `workflow.rs` | execute_shell_command, onboarding, check_onboarding | 1/3 working |
| Symbol | `symbol.rs` | find_symbol, find_referencing_symbols, get_symbols_overview, replace_symbol_body, insert_before/after_symbol, rename_symbol | Stubs (need LSP) |
| AST | `ast.rs` | list_functions, extract_docstrings | Stubs (need tree-sitter) |
| Git | `git.rs` | git_blame, git_log, git_diff | Stubs (backing funcs exist) |
| Semantic | `semantic.rs` | semantic_search, index_project, index_status | Stubs (backing funcs exist) |
| Memory | `memory.rs` | write_memory, read_memory, list_memories, delete_memory | Stubs (MemoryStore exists) |
| Config | `config.rs` | activate_project, get_current_config | Stubs (Agent exists) |

### Utilities (`src/util/`)

- `fs.rs` — `find_ancestor_with()`, `detect_project_root()`, `read_utf8()`, `write_utf8()`
- `text.rs` — `truncate()`, `count_lines()`, `extract_lines()`

## Tech Stack

| Component | Crate(s) |
|-----------|----------|
| Async runtime | `tokio` |
| MCP protocol | `rmcp` (with `transport-io`, `server`, `macros`) |
| LSP types | `lsp-types` |
| AST parsing | `tree-sitter` (grammar integration pending) |
| Git | `git2` |
| Serialization | `serde`, `serde_json`, `toml` |
| Regex | `regex` |
| File walking | `walkdir`, `ignore`, `globset` |
| Error handling | `anyhow` |
| Logging | `tracing`, `tracing-subscriber` |
| CLI | `clap` |
| Embeddings (cloud) | `reqwest` (feature-gated: `remote-embed`) |
| Vector store | `rusqlite` (bundled SQLite) |
| Hashing | `sha2`, `hex` |
| Schema gen | `schemars` |

## Design Principles

- **Symbol-first**: Operate at symbol/AST level, not raw text
- **Language-agnostic**: Uniform interface across all supported languages
- **Offline-first**: All features work without external APIs
- **Composable tools**: Small focused tools that combine well
- **Fail gracefully**: LSP down → tree-sitter → text fallback
- **Token-efficient**: Return minimal context; let the agent request more

## Reference Projects

- `serena-as-reference/` — Python MCP server for code intelligence (tool API patterns, LSP integration, memory system)
- `../cocoindex-code/` — Python embedding MCP server (chunking strategy, sqlite-vec schema, incremental indexing)
