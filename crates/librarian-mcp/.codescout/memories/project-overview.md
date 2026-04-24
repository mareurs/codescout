# librarian-mcp — Project Overview

Workspace-wide markdown artifact registry exposed as an MCP server. Scans multiple
git repos for `.md` files, extracts YAML frontmatter, stores metadata in SQLite with
sqlite-vec for KNN embedding search, and serves 11 MCP tools for query, traversal,
context-packing, and round-trip writes. Designed as long-term memory / context
retrieval for LLM agents; file-on-disk is the source of truth.

## Tech Stack

- Language: Rust (async, tokio runtime)
- MCP framework: `rmcp` v1.3 (`server`, `macros`, `transport-io`, `schemars` features)
- Storage: `rusqlite` + `sqlite-vec` (vec0 virtual table, FLOAT[768] embeddings)
- Embedding: `codescout-embed` workspace crate (remote + local backends)
- Markdown: `pulldown-cmark` (body parsing), `serde_yml` (YAML frontmatter)
- File walking: `walkdir` / `ignore` (respects .gitignore)
- Hashing: `sha2` (SHA-256 per-file content fingerprint for change detection)
- Locking: `parking_lot::Mutex` wrapping the `Catalog` connection

## Runtime Requirements

- `LIBRARIAN_WORKSPACE` — path to workspace TOML config (default: `~/.config/librarian/workspace.toml`)
- `LIBRARIAN_DB` — path to SQLite database (default: `~/.local/share/librarian/catalog.db`)
- `LIBRARIAN_EMBED_MODEL` — embedding model name; if absent, embedding is skipped silently
- `LIBRARIAN_EMBED_URL` — optional: override embed endpoint URL
- `LIBRARIAN_EMBED_API_KEY` — optional: API key for remote embedding backend

## CLI Commands

- `librarian-mcp` (no args) — run stdio MCP server
- `librarian-mcp import-codescout` — seed workspace config from codescout project
- `librarian-mcp reindex` — offline full re-scan without starting the server

## 11 MCP Tools

artifact_find, artifact_get, artifact_list_by_kind, artifact_links, artifact_graph,
artifact_create, artifact_update, artifact_link, artifact_observe, librarian_reindex,
librarian_context
