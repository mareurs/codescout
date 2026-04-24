# librarian-mcp — Conventions

## Error Handling

- All tool `call()` methods return `anyhow::Result<Value>`.
- Errors propagate to `LibrarianServer::call_tool` which converts them to
  `CallToolResult::error(...)` — so MCP callers receive `isError: true` content,
  not a transport-level error. The server never panics on tool failures.
- `serde_json::Error` downcasting in `call_tool` adds a hint about type mismatch
  for input deserialization failures.
- Use `?` freely; do not swallow errors silently inside tool implementations.

## Tool File Structure

Each tool lives in `src/tools/<name>.rs` and follows this pattern:
1. `pub struct ArtifactFoo;` — unit struct
2. Inner `Args` struct (serde Deserialize) for input parsing
3. `impl Tool for ArtifactFoo` with `name()`, `description()`, `input_schema()`, `call()`
4. `input_schema()` returns a hand-written `json!({...})` (not derived) — use `schemars`
   only for complex nested types if needed
5. `call()` deserializes args with `serde_json::from_value::<Args>(args)?`
6. Unit tests in `#[cfg(test)] mod tests` at the bottom of the same file

Register new tools in `all_tools()` in `src/tools/mod.rs`.

## Locking Discipline

- `ToolContext.catalog` is a `Arc<parking_lot::Mutex<Catalog>>`.
- Lock with `let cat = ctx.catalog.lock();` — `parking_lot` mutex, not std.
- **Never hold the lock across an `.await` point.** Release before any async call
  (embedding, file I/O). The `librarian_context` tool is the canonical example:
  it computes embedding vectors *before* locking, then locks briefly for DB reads.
- Batch DB queries inside a single lock acquisition when possible to minimize
  lock contention.

## Frontmatter Format

YAML delimited by `---` fences. Parsed with `serde_yml` + `deny_unknown_fields`.
Known fields: `id`, `kind`, `status`, `title`, `owners` (list), `tags` (list),
`topic`, `time_scope`. Unknown keys are a parse error.
`frontmatter::update_in_place(doc, |fm| { ... })` is the preferred mutation
primitive — it preserves body content and handles missing frontmatter.

## ID Generation

`ids::artifact_id(repo, rel_path)` — deterministic 16-hex-char ID (truncated hash).
IDs are stable across re-indexing as long as repo name and rel_path don't change.

## Classification Priority

`kind` / `status` resolution order in indexer: frontmatter field → CompiledRule glob
match → default (`"unknown"` / `"draft"`). `confidence` is 1.0 if frontmatter
explicitly sets `kind`, 0.5 if derived from a rule.

## Testing Patterns

- Tests use `tempfile::TempDir` for isolated SQLite DBs; call `Catalog::open(&tmp_path)`.
- `serial_test` crate used where tests share global state (e.g., sqlite-vec init).
- Indexer tests build small fixture repos in temp dirs with synthetic `.md` files.
- Three-query sandwich for cache/staleness tests: baseline → mutate without
  notification → assert stale → trigger invalidation → assert fresh.
- Dev dependencies: `serial_test`, `tempfile`, `assert_cmd`, `predicates`.
