# librarian-mcp — Project Overview

## Purpose

librarian-mcp is a standalone Rust MCP server that acts as a workspace artifact registry.
It indexes markdown documents across multiple git repositories, parses YAML frontmatter to
classify and catalog them, and exposes 11 MCP tools for LLM agents to discover, read, write,
and traverse relationships between artifacts (specs, plans, memories, ADRs, etc.).

## Tech Stack

- **Language:** Rust (2021 edition, async with tokio)
- **MCP protocol:** rmcp 1.3 (server, macros, transport-io, schemars)
- **Storage:** rusqlite + sqlite-vec (vec0 virtual tables for KNN semantic search, float[768])
- **Embedding:** codescout-embed sibling crate (local ONNX / remote OpenAI-compat API)
- **Markdown parsing:** pulldown-cmark (frontmatter stripping, H1 extraction)
- **YAML frontmatter:** serde_yml
- **File walking:** ignore + walkdir (respects .gitignore)
- **Glob classification rules:** globset
- **SHA-256 content hashing:** sha2
- **CLI:** clap (derive)

## Key Dependencies

- `codescout-embed` (workspace crate) — embedding abstraction, also used for semantic search
- `rusqlite` — all persistent storage (WAL mode, foreign keys on)
- `sqlite-vec` — vec0 virtual table extension, registered as SQLite auto-extension at startup

## Runtime Requirements

- `LIBRARIAN_WORKSPACE` env var → path to `workspace.toml` (default: `~/.config/librarian/workspace.toml`)
- `LIBRARIAN_DB` env var → path to SQLite catalog DB (default: `~/.local/share/librarian/catalog.db`)
- Optional: `LIBRARIAN_EMBED_MODEL` / `LIBRARIAN_EMBED_URL` / `LIBRARIAN_EMBED_API_KEY` for semantic search

## Binary Layout

- `crates/librarian-mcp/src/main.rs` — entry point; clap CLI with `import-codescout` and `reindex` subcommands
- `crates/librarian-mcp/src/lib.rs` — `run_stdio_server()`, `import_codescout()`, `reindex_cli()`
- `crates/librarian-mcp/src/` — all modules
- `crates/librarian-mcp/tests/mcp_integration.rs` — subprocess MCP integration test

## Workspace Config Format (`workspace.toml`)

```toml
[[roots]]
name = "repo-a"
path = "/home/user/work/repo-a"

[[rule]]
glob = "**/docs/specs/*.md"
kind = "spec"
status = "active"

[ignore]  # optional glob patterns
```
