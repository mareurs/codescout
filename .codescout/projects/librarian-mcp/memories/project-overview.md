# librarian-mcp — Project Overview

## What It Does

librarian-mcp is a standalone MCP server that turns a collection of markdown
repositories into a queryable artifact catalog. It walks `.md` files, classifies
them by kind (spec, plan, memory, adr, doc, …), stores rich metadata in SQLite,
exposes 15 tools over JSON-RPC, and optionally embeds documents for semantic
search.

## Primary Value

- **Workspace-wide or project-scoped artifact discovery** — find specs, plans,
  ADRs, roadmaps, etc. across multiple repos via a composable filter AST.
- **TimeMachine event log** — append-only event stream (note, reviewed,
  status_change, intent, verdict, superseded_by …) lets you replay artifact state
  at any past commit or timestamp.
- **Context packing** — `librarian_context` collects and token-caps the most
  relevant artifacts for an LLM prompt in one call.
- **Semantic search** — optional vector embeddings via `codescout-embed`
  (remote or local), stored in `sqlite-vec` vec0 virtual tables.

## Tech Stack

- **Language:** Rust (async, tokio)
- **MCP transport:** rmcp 1.3 (stdio, JSON-RPC 2.0)
- **Storage:** SQLite via rusqlite; `sqlite-vec` for 768-dim float embeddings
- **Embeddings:** `codescout-embed` crate (remote / local, opt-in)
- **Config format:** TOML (`workspace.toml`) + optional per-project
  `.codescout/librarian.toml` classifier overrides
- **Markdown parsing:** `pulldown-cmark` (headings, frontmatter extraction)
- **Frontmatter:** YAML via `serde_yml`

## Runtime Requirements

- `LIBRARIAN_WORKSPACE` — path to `workspace.toml` (defaults to
  `~/.config/librarian/workspace.toml`)
- `LIBRARIAN_DB` — path to catalog SQLite file (defaults to
  `~/.local/share/librarian/catalog.db`)
- `LIBRARIAN_EMBED_MODEL` — optional; enables semantic search
- `LIBRARIAN_EMBED_URL`, `LIBRARIAN_EMBED_API_KEY` — optional embedder config
- `LIBRARIAN_CWD` — optional override for cwd-based project detection

## Key Dependencies

- `codescout-embed` (sibling crate, features: remote-embed + local-embed)
- `rmcp`, `rusqlite`, `sqlite-vec`, `git2` (vendored libgit2)
- `walkdir`, `ignore`, `globset`, `sha2`, `ulid`, `chrono`, `serde_yml`
