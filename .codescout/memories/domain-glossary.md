# Domain Glossary — codescout Workspace

## Core Terms

**MCP (Model Context Protocol)** — JSON-RPC protocol over stdio or HTTP/SSE; codescout implements an MCP server that exposes tools to LLM clients.

**Tool** — A zero-size struct implementing the `Tool` trait; represents one capability exposed over MCP (e.g. `replace_symbol`, `semantic_search`).

**RecoverableError** — An expected, input-driven failure that routes to `isError: false`; sibling parallel MCP tool calls survive. Contrast with `anyhow::bail!` (`isError: true`).

**OutputGuard** — Controls progressive disclosure: `exploring` mode (default, capped at 200 items) vs `focused` mode (full detail, paginated). Enforced per-tool via `OutputGuard`.

**OutputBuffer** — 50-slot LRU store for large tool outputs; returns `@tool_*` or `@cmd_*` ref instead of inline. Agents read back with `read_file("@tool_*")`.

**ActiveProject** — Runtime state for one activated project: root path, config, memory store, write lock, advisory flock.

**Agent** — `Arc<RwLock<AgentInner>>` holding the current `ActiveProject`; all tools access via `ctx.agent.with_project(...)`.

**Anchor / Anchor Sidecar** — `.anchors.toml` file tracking which source files a memory topic references; used for staleness detection.

**KNN Search** — K-nearest-neighbor vector search using sqlite-vec `vec0` virtual tables; used by `semantic_search`.

**AST Chunker** — tree-sitter-based splitter that chunks code at symbol boundaries for embedding; falls back to line-based chunking.

**Embedder** — Trait in `codescout-embed`; two backends: `LocalEmbedder` (ONNX/fastembed) and `RemoteEmbedder` (OpenAI-compatible HTTP).

**Library Registry** — Read-only navigation target for third-party crates/packages registered via `library(action="register")`.

**TimeMachine** (librarian-mcp) — Event log allowing replay of artifact state at any past git commit or timestamp.

**FilterNode** (librarian-mcp) — Recursive JSON filter AST compiled to injection-safe SQL fragments for artifact queries.

## Fixture Shared Domain

All 5 test fixtures (java/kotlin/python/rust/typescript) use these types intentionally:

- **`Book`** — Core domain entity (record/dataclass/struct/data class/class)
- **`Genre`** — Enum for book categories
- **`Searchable`** — Interface/trait defining `search_text()` and `relevance()`
- **`Catalog<T: Searchable>`** — Generic container with `add`, `search`, `stats`
- **`SearchResult`** — Sealed class / discriminated union / enum with `Found`, `NotFound`, `Error` variants
