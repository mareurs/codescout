# Architecture

## Layer Structure
```
CLI (src/main.rs)
  └─ Server (src/server.rs) — rmcp ServerHandler, tool dispatch
       └─ Tools (src/tools/*.rs) — 31 tool implementations
            └─ Agent (src/agent.rs) — orchestrator: active project, config, embedder cache
                 ├─ LSP (src/lsp/) — language server management
                 ├─ Embed (src/embed/) — semantic search pipeline
                 ├─ AST (src/ast/) — tree-sitter parsing
                 ├─ Git (src/git/) — git2 operations
                 ├─ Library (src/library/) — third-party dependency navigation
                 ├─ Memory (src/memory/) — persistent markdown-based knowledge
                 ├─ Config (src/config/) — project.toml + modes
                 └─ Usage (src/usage/) — tool call telemetry
```

## Key Abstractions

| Abstraction | File | Role |
|---|---|---|
| `Tool` trait | `src/tools/mod.rs:167` | Interface all 31 tools implement: `name()`, `description()`, `input_schema()`, `call()` |
| `ToolContext` | `src/tools/mod.rs:35` | Shared context passed to every tool call: `agent`, `lsp`, `output_buffer` |
| `OutputGuard` | `src/tools/output.rs:35` | Progressive disclosure enforcer: Exploring (compact, 200 cap) vs Focused (full, paginated) |
| `RecoverableError` | `src/tools/mod.rs:54` | Non-fatal errors with hints — `isError: false` so sibling parallel calls aren't aborted |
| `Agent` | `src/agent.rs:17` | Orchestrator holding active project, config, embedder cache (`Arc<Mutex<AgentInner>>`) |
| `ActiveProject` | `src/agent.rs:29` | Current project state: root path, config, memory store, library registry |
| `LspManager` | `src/lsp/manager.rs:18` | Manages LSP server lifecycle per language; implements `LspProvider` |
| `LspClientOps` trait | `src/lsp/ops.rs:9` | Abstract LSP operations (document_symbols, workspace_symbol, etc.) |
| `LspProvider` trait | `src/lsp/ops.rs:57` | Provides LSP clients by language; testable via `MockLspProvider` |
| `CodeExplorerServer` | `src/server.rs:40` | rmcp `ServerHandler` — bridges Tool trait to MCP protocol |
| `Embedder` trait | `src/embed/mod.rs:33` | Abstraction for embedding backends (remote HTTP or local ONNX) |
| `Scope` enum | `src/library/scope.rs:3` | Controls search scope: Project, Libraries, All, or Lib(name) |
| `PathSecurityConfig` | `src/util/path_security.rs:40` | Path validation — blocks reads to sensitive dirs, validates writes |

## Data Flow
1. MCP request → `CodeExplorerServer::call_tool()` dispatches by name
2. Tool's `call(Value, &ToolContext)` executes, using `ToolContext.agent` for state
3. LSP tools: `ToolContext.lsp` → `LspManager` → `LspClient` (JSON-RPC over stdio)
4. Semantic tools: `Agent` → `Embedder` → SQLite index (cosine similarity via sqlite-vec)
5. Results pass through `OutputGuard` for mode-appropriate truncation
6. Errors routed through `route_tool_error()`: `RecoverableError` → soft, other → fatal

## Design Patterns
- **Trait-based tool dispatch:** All tools share one interface, registered as `Vec<Arc<dyn Tool>>`
- **Two-mode output:** `OutputGuard::from_input()` reads `detail_level` param, enforces caps
- **Recoverable vs fatal errors:** Input-driven failures are recoverable (with hints), system failures are fatal
- **Interior mutability:** `Agent` wraps `AgentInner` in `Arc<Mutex<>>` for shared state across tools
- **LSP testability:** `LspClientOps` + `LspProvider` traits enable `MockLspClient` for unit tests
