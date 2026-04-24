# librarian-mcp — Conventions

## Rust Patterns

- **Tool trait**: Simpler than codescout's Tool. No `call_content()`, no `OutputGuard`, no `RecoverableError`.
  Tools return `anyhow::Result<Value>`; errors surfaced as `CallToolResult::error` by the server.
- **Mutex discipline**: Catalog lock (`parking_lot::Mutex`) is acquired immediately before SQL and
  dropped immediately after. Never held across `.await`. Embedding calls happen outside the lock.
- **Write tools**: Return meaningful JSON (e.g. `{"id": ..., "updated": true}`), not `json!("ok")`.
  This project has no OutputGuard convention — tools emit compact results directly.
- **Frontmatter round-trips**: `frontmatter::update_in_place()` preserves all existing fields.
  `frontmatter::write()` serializes via serde_yml. `serde(deny_unknown_fields)` on Frontmatter
  enforces strict schema — unknown YAML keys cause parse failure.
- **IDs**: `artifact_id(repo, rel_path)` = sha256(repo + "\n" + rel_path) hex[:16]. Stable across renames
  only if the (repo, rel_path) tuple is unchanged. Collision probability negligible for realistic corpora.
- **Path safety**: `validate_rel_path()` in create.rs blocks absolute paths and `..` traversal.
  `normalize_rel_path()` converts Windows backslashes to `/` for cross-platform ID stability.

## Testing Patterns

- **Inline unit tests**: All modules use `#[cfg(test)] mod tests { ... }` with in-memory catalog
  (`Catalog::open_in_memory()`) and `tempfile::TempDir` for file I/O tests.
- **Integration test**: `tests/mcp_integration.rs` spawns the real `librarian-mcp` binary via
  `assert_cmd::cargo::cargo_bin()`, runs JSON-RPC over stdio with `LIBRARIAN_WORKSPACE` and
  `LIBRARIAN_DB` env vars, checks 11 tools are registered and `artifact_find` returns results.
- **Async tool tests**: `#[tokio::test]` for tools that use async. No separate runtime setup needed.
- **Mock embedder**: Tests create a `MockEmbedder` that implements `codescout_embed::Embedder`,
  wrapped in `EmbeddingService::new(Arc::new(mock))`. Used in semantic search tests.

## Naming & Structure

- Tool structs: PascalCase unit structs (`ArtifactFind`, `ArtifactGet`, `LibrarianReindex`, etc.)
- One tool per file in `src/tools/`; `all_tools()` in `src/tools/mod.rs` returns `Vec<Arc<dyn Tool>>`
- Catalog sub-modules: `artifact`, `find`, `links`, `observations` — one concern per file
- Preview sub-modules: one file per kind (`plan`, `spec`, `memory`, `default`, `headings`, `summary`)
- `#[serde(deny_unknown_fields)]` on all config/frontmatter structs to catch typos early

## Configuration / Environment

- `LIBRARIAN_WORKSPACE` → workspace.toml path (default: `~/.config/librarian/workspace.toml`)
- `LIBRARIAN_DB` → SQLite DB path (default: `~/.local/share/librarian/catalog.db`)
- `LIBRARIAN_EMBED_MODEL` → embedding model spec (e.g. `local:AllMiniLML6V2Q`, `openai:text-embedding-3-small`)
- `LIBRARIAN_EMBED_URL` → custom embedding API endpoint (optional)
- `LIBRARIAN_EMBED_API_KEY` → API key for remote embedder (optional)

## Classification System

Rules are TOML-based, loaded from `workspace.toml` `[[rule]]` sections or a separate rules file.
First-matching glob wins. Unknown-kind artifacts (`kind="unknown"`) are indexed but flagged in
the `unknown_ids` field of the reindex report. Frontmatter `kind`/`status` takes precedence over
rule-matched values; rules provide defaults for files without frontmatter.

## Pre-Commit Requirements

```
cargo fmt && cargo clippy -- -D warnings && cargo test
```
Integration test requires the binary to be built: `cargo build` or `cargo build --release` first.
