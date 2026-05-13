# Architecture

This page describes how codescout works internally. It is written for users
who want to understand the system, not for contributors adding new tools or
languages (see the [Extending](extending/adding-languages.md) chapter for that).

---

## System Overview

codescout is an MCP server that gives LLMs IDE-grade code intelligence. It
sits between the AI assistant (Claude Code, Cursor, or any MCP-capable client)
and the project's source code, providing 28 tools for navigation, search,
editing, and analysis.

The server is a single Rust binary. It launches language servers, parses source
files with tree-sitter, manages a vector embedding index, and reads git history
-- all behind a uniform MCP tool interface. The AI assistant never interacts
with these backends directly; it calls tools, and codescout handles the
rest.

---

## Component Diagram

```
Claude Code ──MCP──▶ CodeScoutServer
                          │
                    ┌─────┼─────┐
                    ▼     ▼     ▼
                  Agent  Tools  Instructions
                    │     │
              ┌─────┼─────┼─────┐
              ▼     ▼     ▼     ▼
            Config  LSP   AST   Embeddings
              │     │     │     │
              ▼     ▼     ▼     ▼
          project  Language  tree-sitter  SQLite
          .toml    Servers   grammars     index
```

**CodeScoutServer** is the MCP entry point. It holds the Agent, the tool
registry, and the server instructions that get sent to the LLM.

**Agent** manages the active project: root path, configuration, memory store.

**Tools** are stateless structs dispatched by name. All state flows through
`ToolContext`, which holds references to the Agent and the LSP manager.

**Instructions** are markdown text sent to the LLM as part of the MCP server
info. They guide the LLM on how to use the tools effectively.

---

## Request Lifecycle

When the LLM calls a tool, here is what happens:

1. **MCP request arrives.** A JSON-RPC message comes in over stdio (single
   connection) or HTTP/SSE (multi-connection). The `rmcp` crate handles
   protocol framing.

2. **Tool lookup.** `CodeScoutServer::call_tool()` searches the tool
   registry -- a `Vec<Arc<dyn Tool>>` -- for a tool matching the requested
   name. If no match is found, an `invalid_params` MCP error is returned.

3. **Security check.** Before the tool runs, `check_tool_access()` verifies
   that the tool is not disabled by the project's security configuration. For
   example, if `shell_enabled` is false, `run_command` is blocked here.

4. **ToolContext creation.** A `ToolContext` is assembled with clones of the
   `Agent` and `Arc<LspManager>`. This is the only state a tool receives.

5. **Tool execution.** The tool's `call()` method runs with the parsed JSON
   input and the context. Tools are async and can call into LSP servers, read
   files, query the embedding index, or run shell commands.

6. **Result or error.** On success, the tool returns a `serde_json::Value`
   that gets serialized to a `CallToolResult` with text content. On failure,
   the error is wrapped in `CallToolResult::error` -- it is surfaced to the
   LLM as an error message, not as an MCP protocol error. This means tool
   failures are recoverable: the LLM sees the error and can try a different
   approach.

---

## Key Components

### Agent

**Source:** `src/agent.rs`

The Agent manages the active project state. It holds:

- **Project root** -- the filesystem path to the project being explored.
- **Configuration** -- the parsed `project.toml` settings.
- **Memory store** -- the markdown-backed key-value store for persistent
  knowledge.

The Agent is thread-safe via `Arc<RwLock<AgentInner>>`. It is cloned and shared
across all tool calls and, in HTTP mode, across all connections. Calling
`workspace(action: activate)` swaps the inner project state atomically.

### Tool Registry

**Source:** `src/server.rs`

All 22 tools are registered at startup in `CodeScoutServer::from_parts()` as
a `Vec<Arc<dyn Tool>>`. Dispatch is by name: `call_tool()` iterates the vector
and matches on `tool.name()`.

Each tool is a zero-size struct implementing the `Tool` trait:

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value>;
}
```

Tools are stateless. All project state lives in the `ToolContext` they receive
on each call. This means tools are trivially shareable across threads and
connections.

### LSP Manager

**Source:** `src/lsp/manager.rs`, `src/lsp/client.rs`, `src/lsp/servers/`

The LSP Manager starts language server processes on demand. When a symbol tool
is called for a file, the manager checks the file's language, looks up the
default server configuration, and either reuses an existing server process or
spawns a new one.

Supported language servers:

| Language            | Server                       |
|---------------------|------------------------------|
| Rust                | `rust-analyzer`              |
| Python              | `pyright-langserver`         |
| TypeScript/JS/TSX   | `typescript-language-server` |
| Go                  | `gopls`                      |
| Java                | `jdtls`                      |
| Kotlin              | `kotlin-language-server`     |
| C/C++               | `clangd`                     |
| C#                  | `OmniSharp`                  |
| Ruby                | `solargraph`                 |

Communication with language servers uses JSON-RPC over stdio. The `LspClient`
struct handles the LSP initialization handshake, sends requests
(`textDocument/documentSymbol`, `textDocument/references`, `textDocument/rename`,
etc.), and parses responses using `lsp-types`.

Server processes are long-lived: once started, they persist for the lifetime of
the MCP session. On shutdown (SIGINT, SIGTERM, or MCP connection close), the
server calls `LspManager::shutdown_all()`, which sends proper LSP
`shutdown`/`exit` messages to each language server for a clean exit. As a safety
net, the `LspClient` Drop implementation also sends SIGTERM to the child process
via `libc::kill`, ensuring cleanup even if the graceful path is bypassed.

### AST Engine

**Source:** `src/ast/`

Tree-sitter provides offline parsing that works without a language server. It
is faster than LSP for simple structural queries and has zero startup cost.

The AST engine is used internally for richer symbol extraction and semantic chunking. It is not exposed as a standalone tool — use `symbols` (LSP-backed) for interactive symbol navigation.

Supported languages for tree-sitter: Rust, Python, TypeScript, Go, Java,
Kotlin. See the [Language Support](language-support.md) page for the full
matrix.

### Embedding Pipeline

**Source:** `src/retrieval/`, `src/embed/ast_chunker.rs`, `crates/codescout-embed/`

The embedding pipeline enables semantic search — finding code by meaning rather
than by name. As of v0.12 it is a network-attached retrieval stack, not a
local-database backend. Four stages, each running in a separate process:

1. **Chunking** (`src/embed/ast_chunker.rs`, `crates/codescout-embed/src/chunker.rs`)
   — Source files are split into overlapping text chunks. For languages with
   tree-sitter support, the chunker uses AST boundaries (functions, classes,
   blocks) to create semantically coherent chunks. For other files, it falls
   back to line-based splitting with configurable `chunk_size` and
   `chunk_overlap`.

2. **Dense embedding** (`src/retrieval/embedder.rs::EmbedderHttp`) — Each chunk
   is POSTed to a dense embedding service over HTTP. The default stack ships
   a TEI-compatible embedder on `localhost:48081`; the same client also speaks
   the OpenAI-compatible protocol when `CODESCOUT_EMBEDDER_PROTOCOL=openai`
   is set, so Ollama, OpenAI, and Anthropic-compatible endpoints all work.

3. **Sparse embedding** (`src/retrieval/embedder.rs`, SPLADE) — In parallel,
   chunks are sent to a sparse SPLADE service on `localhost:48084`. The sparse
   vector captures lexical matches the dense vector misses (rare tokens, exact
   identifiers).

4. **Storage and search** (`src/retrieval/qdrant.rs`) — Both vectors plus chunk
   metadata are upserted into Qdrant's `code_chunks` collection over gRPC
   (`localhost:6334`). Query-time, the same dense + sparse embeddings are
   computed for the query text and Qdrant performs hybrid search with
   Reciprocal Rank Fusion (`1/(1+rank)`) across both legs. Top results are
   then re-ranked by a cross-encoder service on `localhost:48083`
   (TEI-compatible, `bge-reranker-v2-m3` by default; protocol switchable to
   Infinity via `CODESCOUT_RERANKER_PROTOCOL=infinity`).

The index tracks file content hashes. On incremental re-indexing, only files
that changed since the last index build are re-chunked and re-embedded.
Memories live in a sibling Qdrant collection (`memories`) with the same
substrate but a separate schema and lifecycle.

The full stack is provided as `docker-compose.yml` at the repo root with `cpu`
and `gpu` profiles. Users migrating from pre-v0.12 installs run
`codescout migrate-memories` to re-embed legacy `.codescout/embeddings/project.db`
content into Qdrant.
### Memory Store

**Source:** `src/memory/`

A lightweight key-value store backed by markdown files in
`.codescout/memories/`. Topics are path-like strings (e.g.,
`debugging/async-patterns`) that map to files on disk.

The file store supports four operations: write, read, list, and delete.

A second tier — **semantic memory** — stores entries as vector embeddings in
the same `.codescout/embeddings.db`. This enables natural-language recall
(`action: "remember"` / `"recall"` / `"forget"`) in addition to the
key-based file store. Each write also cross-embeds into the semantic store
(best-effort, non-fatal). Memories auto-classify into buckets: `code`,
`system`, `preferences`, `unstructured`.

---

## Transport Modes

codescout supports two transport modes, selected at startup via the
`--transport` flag.

### stdio (default)

```bash
codescout start --project /path/to/project
```

Single connection. Claude Code launches the server as a subprocess and
communicates over stdin/stdout. No authentication is needed because the
connection is local and exclusive.

This is the standard mode for Claude Code integration. The MCP registration
command (`claude mcp add`) sets this up automatically.

### HTTP/SSE

```bash
codescout start --project /path/to/project --transport http --port 8080
```

Multi-connection. The server binds to a port and accepts SSE (Server-Sent
Events) connections. Each connection gets its own `CodeScoutServer` instance
but shares the same Agent and LSP Manager.

An auth token is auto-generated and printed to stderr at startup. Clients must
send it as a `Bearer` token in the `Authorization` header. You can also
provide your own token via `--auth-token`.

Use HTTP mode when the MCP client runs on a different machine, or when
multiple clients need to share a single server.

---

## Storage

All persistent state is split between **per-project local files** and a
**shared retrieval stack** running as containers.

### Per-project (`.codescout/` in the project root)

```
<project-root>/
└── .codescout/
    ├── project.toml      # Configuration
    ├── call_edges.db      # Cross-file call graph cache (SQLite)
    └── memories/          # Markdown knowledge files
        ├── topic-a.md
        └── debugging/
            └── async-patterns.md
```

This directory is created automatically when a project is first activated.
Add `.codescout/` to your `.gitignore` — it contains machine-local state
(call-graph cache, memory notes) that should not be committed.

The `project.toml` file is an exception: you may want to commit it so that
team members share the same configuration. See
[Project Configuration](configuration/project-toml.md) for details.

> **Note:** Pre-v0.12 installs had `.codescout/embeddings/project.db` (a
> sqlite-vec store). That file is no longer created or read. To migrate its
> contents into Qdrant, run `codescout migrate-memories` once; you can then
> delete the legacy file. The active-project banner surfaces a "⚠ LEGACY
> INDEX" hint when it detects one.

### Shared retrieval stack (Qdrant + embedding services)

| Service | Default port | Role |
|---|---|---|
| Qdrant | `:6334` (gRPC) | Vector storage for `code_chunks` and `memories` collections |
| Dense embedder | `:48081` (HTTP) | TEI- or OpenAI-protocol text → dense vector |
| Cross-encoder reranker | `:48083` (HTTP) | TEI or Infinity-protocol pairwise reranking |
| Sparse SPLADE | `:48084` (HTTP) | TEI-protocol text → sparse vector |

The repo ships a `docker-compose.yml` with `cpu` and `gpu` profiles that
brings up all four. The stack is shared across projects on a machine — there
is no per-project Qdrant instance.
## Tech Stack

| Crate | Purpose |
|-------|---------|
| `rmcp` | MCP protocol implementation (stdio and SSE transports) |
| `lsp-types` | LSP type definitions |
| `tree-sitter` + language grammars | Offline AST parsing |
| `git2` | Git operations (blame, log, diff) |
| `qdrant-client` | gRPC client for Qdrant vector storage |
| `rusqlite` | SQLite — `call_edges.db` cache + legacy memory migration reader |
| `reqwest` (rustls + ring) | HTTP client for embedder / reranker / sparse services |
| `rustls` | TLS via the `ring` crypto provider (small binary footprint) |
| `fastembed` | Local CPU embeddings (optional, `local-embed` feature — not the default substrate) |
| `tokio` | Async runtime |
| `clap` | CLI argument parsing |
| `serde` / `serde_json` | JSON serialization |
| `tracing` | Structured logging |
| `libc` | POSIX signals for LSP process cleanup |
## Further Reading

- [Progressive Disclosure](concepts/progressive-disclosure.md) -- how output
  volume is controlled across all tools.
- [Project Configuration](configuration/project-toml.md) -- all settings in
  `project.toml`.
- [Embedding Backends](configuration/embedding-backends.md) -- configuring
  Ollama, OpenAI, or local embeddings.
- [Semantic Search Concepts](concepts/semantic-search.md) — how the embedding
  pipeline works, similarity scoring, and when to reach for semantic vs symbol search
- [Dashboard](concepts/dashboard.md) — visual UI for project health, tool usage
  stats, index status, and memory browsing
- The full internal architecture with contributor-level detail is in
  `docs/ARCHITECTURE.md` in the repository root.
