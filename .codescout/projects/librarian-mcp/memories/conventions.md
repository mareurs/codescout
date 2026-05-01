# librarian-mcp — Conventions

## Language & Framework Patterns

- **Async runtime:** Tokio. `build_tool_context()` is `async`; `Tool::call()` is sync.
  Heavy async work (embedding) uses `tokio::spawn` + `futures::future::join_all`.
- **Error handling:**
  - `RecoverableError::new(msg)` / `::with_hint(msg, hint)` for expected, input-driven
    failures (bad args, not found, etc.) → tool returns `isError: false` in MCP
  - `anyhow::bail!` for genuine bugs or unexpected infra failures → `isError: true`
  - `map_tool_result()` in `server.rs` routes these; every tool returns `Result<Value>`
- **Write tools return `json!("ok")`** — never echo content back (no-echo principle)
- **Mutation tools** (`create`, `update`) modify the on-disk `.md` file first, then
  re-index; the file is the source of truth, the catalog is derived.

## Naming

- Catalog modules: free functions (`upsert`, `get`, `delete`, `insert`, `find`) — no
  method dispatch on `Catalog`; `Catalog` passed as `&Catalog` parameter
- Tool structs: `ArtifactXxx` or `LibrarianXxx`; inner `Args` struct holds schemars schema
- Inner test helpers: `mk_ctx()`, `art(id)`, `ev(id, kind, ts)` — consistent across files

## Testing Patterns

- Unit tests inline (`#[cfg(test)]` mod) in each source file — very thorough
- `Catalog::open_in_memory()` used in all unit tests; no temp files needed for catalog
- Integration test in `tests/mcp_integration.rs` spawns the real binary, runs JSON-RPC
  handshake, verifies `tools/list` returns 15 tools and `artifact_find` works end-to-end
- Test fixture: `tests/fixtures/repo_a/` — 3 markdown files for integration smoke tests
- `serial_test` crate used where tests share global state
- Three-query sandwich pattern for cache-invalidation tests (baseline → stale assert → invalidate → fresh assert)

## Classifier Rules

- Rules are TOML `[[rule]]` with `glob`, `kind`, `status?`, `time_scope?` fields
- First-match wins across the three-layer stack:
  1. Project-local: `<project>/.codescout/librarian.toml`
  2. Workspace: `workspace.toml [[rule]]`
  3. Built-in defaults: `classify::DEFAULT_RULES_TOML` (covers changelogs, issues, specs, plans, ADRs, etc.)
- `confidence=1.0` when kind comes from frontmatter; `0.5` when inferred from rules

## Schema / IDs

- `artifact.id` = `artifact_id(repo, rel_path)` — deterministic, hash-based ULID-style string
- Times are **millisecond epoch integers** — not ISO-8601
- `tags` and `owners` stored as JSON arrays in TEXT columns; `json_each()` used in SQL for `contains` operator
- `artifact_vec` is a `vec0` virtual table (sqlite-vec) with 768-dim float embeddings

## Environment Variables

| Variable | Purpose |
|---|---|
| `LIBRARIAN_WORKSPACE` | Path to `workspace.toml` |
| `LIBRARIAN_DB` | Path to `catalog.db` |
| `LIBRARIAN_EMBED_MODEL` | Embedder model name (enables semantic search) |
| `LIBRARIAN_EMBED_URL` | Remote embedder base URL |
| `LIBRARIAN_EMBED_API_KEY` | API key for remote embedder |
| `LIBRARIAN_CWD` | Override for cwd used in project resolution |

## CLI Subcommands

- `librarian-mcp` (no args) — stdio MCP server mode
- `librarian-mcp reindex [--repo=...] [--force]` — CLI reindex for seeding/CI
- `librarian-mcp import-codescout` — import codescout workspace projects into `workspace.toml`

## Preview System

- `preview::extract(kind, row, body, ctx)` routes to kind-specific renderers:
  `plan`, `spec`, `memory`, `default` (fallback), `headings`, `summary`
- Used by `artifact_get` and `librarian_context` to produce compact LLM-friendly snippets
