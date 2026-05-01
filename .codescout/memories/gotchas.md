# Gotchas — codescout Workspace

## Cross-Project

**Embedding dimension mismatch on fixture project memories**
`memory(write)` on fixture projects (java/kotlin/python/rust/typescript) emits
`cross-embed failed: Dimension mismatch (expected 384, received 768)` warnings.
Status is still `ok` — the memory text is saved; only the cross-project embedding index fails.
Cause: fixture project `.codescout/embeddings/project.db` was indexed with 384-dim model
but current encoder runs 768-dim. Non-blocking; just noise in logs.

**Fixture project memory reads fall back to home project**
`memory(action="read", project="java-library", topic="project-overview")` returns
the home project's (code-explorer's) memory if the fixture project has no memory store
or wasn't activated. Always check the returned content matches the expected project.

**Subagents share the MCP server state**
`workspace(action="activate", path=...)` is global state. Any subagent that activates
a different project MUST restore the home project before exiting, or all subsequent
tool calls in the parent session break silently.

## code-explorer Specific

**LSP cold start on first symbol query** — rust-analyzer and other LSPs need 5–30s to
index on first use. Retry logic is built into `LspClient` but the first call may appear
slow. Use `MockLspProvider` in tests to avoid this.

**Kotlin LSP conflict** — Multiple concurrent Kotlin LSP instances can conflict.
See `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md`. TTL is 2h vs 30min
for other languages.

**`symbols()` truncation** — `symbols(path)` caps output via OutputGuard. If symbols
appear missing, use `symbols(path, detail_level="full")` or narrow with `name=` filter.

**`edit_file` gate on multi-line structural edits** — `edit_file` hard-errors on edits
containing definition keywords (`fn`, `class`, `struct`, etc.) on LSP-supported languages.
Use `replace_symbol`, `insert_code`, or `remove_symbol` instead.

**Three-query sandwich for cache invalidation tests** — A two-query test only checks the
happy path. Always: baseline → assert stale → invalidate → assert fresh.
See `did_change_refreshes_stale_symbol_positions` in `src/lsp/client.rs`.

**Prompt surface tripwire** — `server::tests::prompt_surfaces_reference_only_real_tools`
fails if any of the 3 prompt surfaces reference a non-existent tool name. Add to allowlist
or fix the stale reference.

## librarian-mcp Specific

**FilterNode SQL injection** — Never build SQL fragments by hand. Always go through
`filter::compile()` which emits parameterized queries.

**TimeMachine replay requires git history** — `artifact_state_at` replays via git diff;
repos without full history (shallow clones) may miss events.

## codescout-embed Specific

**fastembed `&mut self` in v5** — fastembed 5 changed `embed` to `&mut self`, requiring
`Arc<Mutex<LocalEmbedder>>` even for read-only usage. Don't try to make it `Arc<RwLock>`.

**Remote embedder lazy dimension caching** — `RemoteEmbedder` caches dimensions via
`Arc<AtomicUsize>`. The first embed call populates it; before that, `dimensions()` returns 0.
