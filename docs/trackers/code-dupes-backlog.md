---
id: fd76d88c02358b79
kind: tracker
status: active
title: Code Duplication Backlog (code-dupes)
owners:
- Marius Ailinca
tags:
- code-quality
- duplication
- refactor
topic: null
time_scope: null
---

## Audit scope and methodology

Findings from `code-dupes` (`~/.cargo/bin/code-dupes`, an AST-fingerprint duplicate-code
detector for Rust — installed via `cargo install` but **not** a real `cargo` subcommand;
there is no literal `cargo dedup` alias anywhere in this repo or `~/.cargo/config.toml`).
Not on `PATH` — invoke via the absolute path.

Scanned `src/` on 2026-07-08 at HEAD `b2853b4d`:

```
code-dupes report -p src --exclude-tests --exclude 'tests.rs' --min-lines 15
```

`--exclude-tests` alone misses inline `#[cfg(test)] mod tests { ... }` blocks inside
non-`tests.rs` files (e.g. `mod.rs`, `resolver.rs`) — combining it with a substring
`--exclude 'tests.rs'` was necessary to get a clean, test-free view of production code.
`--exclude` does substring matching on the path, not glob (`**/tests.rs` silently matched
nothing; `tests.rs` matched everything).

Raw filtered scan: 10 exact-duplicate groups (21 units) + 2 near-duplicate groups (4 units)
across 881 code units / 43,427 lines, 1.2% exact / 0.2% near duplication. Every group below
was verified by reading both sides of the pair before being logged — one group
(`has_lsp_config` / `is_idempotent_lsp_method`) turned out to be a coincidental AST-shape
match (same `matches!` arm count, disjoint value domains) and is logged `wontfix` rather
than silently dropped, so it isn't re-investigated on the next scan.

## Per-issue detail

### Issue 1 — Manifest-parser triplication in `library/discovery.rs`
**Symptom:** `try_cargo_toml` / `try_package_json` / `try_pyproject_toml` are a 3-way exact
clone (`src/library/discovery.rs:44-96`) — same exists-check / read-to-string / extract-name
/ extract-version / construct-`DiscoveredLibrary` skeleton, differing only in the manifest
filename, the toml-vs-json extractor function, and the language tag string.
**Root cause:** Copy-paste-adapt when `package.json` and `pyproject.toml` support were added
after `Cargo.toml`.
**Fix:** Extract a shared `try_manifest(dir, filename, extract: fn(&str, &str) -> Option<String>, language) -> Option<DiscoveredLibrary>` (or similar) helper; each `try_*` becomes a thin one-line wrapper.
**Predicted impact:** ~35 duplicated lines removed; new manifest formats (e.g. Gradle,
Go modules) add one wrapper instead of copy-pasting the skeleton.

**Fixed:** 2507e4eb — `try_manifest()` extracted, all three `try_*` reduced to one-liners.

### Issue 2 — `OutputGuard::cap_items` / `cap_files` duplication
**Symptom:** `src/tools/output.rs:125-173` — the two methods are byte-identical control
flow (`Exploring` mode: truncate-and-build-`OverflowInfo`; `Focused` mode: delegate to
`paginate`), differing only in `self.max_results` vs `self.max_files`.
**Root cause:** `cap_files` was added as a copy of `cap_items` with the threshold field
swapped.
**Fix:** Extract a private `cap<T>(&self, items: Vec<T>, hint: &str, threshold: usize) ->
(Vec<T>, Option<OverflowInfo>)` and have both public methods call it with their own
threshold field.
**Predicted impact:** ~25 duplicated lines removed; a future third `cap_*` variant is a
one-liner.

**Fixed:** 991c0a51 — private `cap()` extracted, both public methods now one-liners.

### Issue 3 — `conn_for` duplication across the two sqlite-vec stores
**Symptom:** `SqliteVecSemanticMemoryStore::conn_for` (`src/memory/sqlite_semantic_store.rs:53-80`)
and `SqliteVecCodeStore::conn_for` (`src/retrieval/sqlite_code_store.rs:70-100`) share the
identical lock-cache / register-extension / mkdir / open-connection / cache-insert skeleton;
they differ only in the db filename suffix and the `CREATE TABLE` DDL string.
**Root cause:** The two stores were built independently against the same sqlite-vec
extension pattern.
**Fix:** Extract a shared `open_sqlite_vec_conn(dir, conns: &Mutex<HashMap<..>>, project_id, db_suffix, ddl) -> Result<Arc<Mutex<Connection>>>` helper (module-level or on a small shared
struct); each store's `conn_for` becomes a thin wrapper passing its own suffix + DDL.
**Predicted impact:** ~28 duplicated lines removed; the register/mkdir/cache-insert
boilerplate can't drift between the two stores again.

**Fixed:** 94a2008d — `sqlite_vec_ext::open_conn()` extracted; both stores' `conn_for`
now pass their own db suffix + DDL. All `real_vec0_*` integration tests pass unchanged.

### Issue 4 — `LspClient::incoming_calls` / `outgoing_calls` duplication
**Symptom:** `src/lsp/client.rs:1395-1454` — identical capability-check / build-params /
request / handle-null / deserialize skeleton, differing only in the LSP method string
(`callHierarchy/incomingCalls` vs `.../outgoingCalls`) and the response type
(`CallHierarchyIncomingCall` vs `...OutgoingCall`).
**Root cause:** Call-hierarchy support was added as two near-identical LSP request wrappers.
**Fix:** Lower priority — collapsing this cleanly needs a small generic-over-response-type
helper (`request_call_hierarchy<T: DeserializeOwned>(method, params) -> Result<Vec<T>>`),
which is a bit more invasive than issues 1-3. Worth doing but not in the first pass.
**Predicted impact:** ~25 duplicated lines removed.

**Fixed:** 859a1f5c — extracted `send_call_hierarchy_request<T>()`; each caller still builds its
own typed params (the two params structs aren't interchangeable) and delegates. Verified
against a live rust-analyzer via `call_hierarchy_outgoing_returns_calls`.

### Issue 5 — `open_intents` / `orphan_verdicts` duplication
**Symptom:** `src/librarian/catalog/events.rs:135-168` — same
`prepare` / `query_map(row_to_event)` / `collect` skeleton, differing only in the SQL
`WHERE` predicate (unresolved `intent` vs orphan `verdict`).
**Root cause:** Two independent "find events without a matching edge" queries written
back-to-back.
**Fix:** Lower priority — the SQL predicates are different enough (`NOT IN (SELECT
dst_event_id ...)` vs `NOT IN (SELECT src_event_id ...)`) that a shared helper would need a
predicate parameter; worth it but marginal given only 2 call sites.
**Predicted impact:** ~15 duplicated lines removed.

**Fixed:** 010872f9 — extracted `events_where_no_resolves_edge()`, passing the edge subquery
text whole rather than parameterizing a column name: `dst_event_id` needs an explicit
`IS NOT NULL` guard that `src_event_id` doesn't, so the two subqueries were never truly
identical, just the surrounding Rust skeleton was.

### Issue 6 — Dashboard route handler duplication (`get_lsp` / `get_usage`)
**Symptom:** `src/dashboard/api/lsp.rs:15-56` and `src/dashboard/api/usage.rs:12-53` share
an identical "check usage.db exists → open it → run a query → shape `{available, ...}`
JSON, logging on each failure path" skeleton.
**Root cause:** Both dashboard endpoints were built from the same starting template.
**Fix:** Extract a shared `with_usage_db<T>(state, query: impl FnOnce(&Connection) -> Result<T>) -> Json<Value>`-style helper (exact shape TBD — needs to preserve each endpoint's
custom "not available" messages).
**Predicted impact:** ~35 duplicated lines removed; likely to recur as more dashboard
panels are added (`dashboard/routes.rs` already has 4 near-identical
`*_returns_not_available_without_db` stubs at the test layer).

**Fixed:** a1c1b236 — extracted `usage_stats_response()` into a new `dashboard/api/common.rs`.
Caught mid-fix: `dashboard` is a non-default Cargo feature not enabled by the standard
`cargo test`/`cargo clippy` commands (nor by `cargo rb`'s `server-stack`), so my first
compile/test pass silently checked *nothing* in this module — re-ran everything with
`--features dashboard` explicitly. Also found zero existing tests for either handler;
added 3 characterization tests (db-missing, query-success, query-failure) on
`usage_stats_response` before trusting the refactor.

**Architecture review (architecture-snow-lion, 2026-07-08):** checked whether `common.rs`
is a premature 2-user abstraction against this project's own rule-of-three convention.
The "4 near-identical stubs" cited above are test functions in `routes.rs::tests`, not
production handlers. The one real candidate third user, `get_errors`
(`dashboard/api/errors.rs`), genuinely diverges in shape: no `"reason"` field on
missing-db (`{"available": false, "errors": []}` instead), and it collapses
open-failure/query-failure into one case rather than two distinct messages. `get_index`
queries Qdrant, not `usage.db`, and isn't a candidate at all.
**Decision:** keep `usage_stats_response` as-is, a 2-user private helper — do not widen it
to absorb `get_errors`. Alternatives considered: (a) generalize with a configurable
error-shape param — rejected, adds parameters for one confirmed need and risks silently
changing `get_errors`'s response shape inside a "refactor"; (b) revert `get_lsp`/`get_usage`
back to inline duplication — rejected, those two were byte-identical already (not a
speculative guess at shared shape), unlike the registry-abstraction case the rule-of-three
memory warns against. Revisit-when: a handler is asked to match this exact skeleton —
until then `get_errors` staying divergent is evidence *for* the project's rule-of-three
caution, not against the extraction that already exists. Confidence: high.

### Issue 7 — Duplicated frontmatter-patch closure in `librarian/tools/update.rs::call`
**Symptom:** Inside the single `call` function (`src/librarian/tools/update.rs:164-398`),
the exact same closure body — apply `status`/`title`/`owners`/`tags`/`topic`/`time_scope`
then `merge_extra` — is written out twice verbatim: once inside the `fm_changing` branch
under `body_edits`, once in the plain-patch `else` branch with no body change at all.
**Root cause:** The `body_edits` frontmatter-touch-up path was added after the plain-patch
path and the closure was copied rather than shared.
**Fix:** Extract `fn apply_frontmatter_patch(fm: &mut Frontmatter, patch: &UpdatePatch)` and
call it from both `update_in_place` closures.
**Predicted impact:** ~20 duplicated lines removed inside a single function — purely
mechanical, lowest risk of the whole backlog (both call sites are in the same function,
same test suite covers both).

**Fixed:** 99608519 — turned out to be a 3-way duplicate, not 2-way: the full-body-overwrite
branch had the same block inline (not wrapped in a closure), which is why code-dupes only
flagged 2 of the 3 copies. `apply_frontmatter_patch()` now covers all three call sites.

### Issue 8 — Lazy-regex boilerplate repeated 5x in `command_summary.rs`
**Symptom:** `test_re`, `build_re`, `cargo_test_result_re`, `rust_error_code_re`, and
`warning_re` in `src/tools/command_summary.rs` each repeat
`static RE: OnceLock<Regex> = OnceLock::new(); RE.get_or_init(|| Regex::new(...).expect(...))`
around a different literal pattern.
**Root cause:** No shared "cached regex" helper existed when the first of these was written.
**Fix:** Either a `cached_regex!(pattern, expect_msg)` declarative macro, or a small
`fn cached_regex(cell: &'static OnceLock<Regex>, pattern: &str, what: &str) -> &'static
Regex` helper (the latter needs `cell` passed by the caller since each needs its own
`static`, so a macro is likely cleaner here).
**Predicted impact:** Cosmetic/low duplication-line-count win but removes a repeated,
easy-to-get-wrong pattern (a typo'd `.expect()` message is the only thing that currently
distinguishes the 5 call sites).

**Fixed:** 169dd7d3 — `cached_regex_fn!` macro; each accessor is now a one-line invocation
with its own `static` storage. Note: `edit_code`'s structural-replace guard correctly
refused to turn a `fn` into a macro invocation (not a recognizable function shape) —
worked around via `edit_code remove` + `edit_code insert` instead of fighting the guard.

### Issue 9 — `OutputBuffer::store_dangerous` / `store_pending_write` near-duplication
**Symptom:** `src/tools/output_buffer.rs` — both methods share the identical
lock-inner / mint-pending-handle / insert-into-`pending_acks` / push-to-`pending_order` /
return-id skeleton, differing only in which `PendingAck` variant they construct.
**Root cause:** Two "register a pending ack, return a handle" call sites.
**Fix:** Extract a private `fn store_pending(&self, ack: PendingAck) -> String` and have
both public methods build their variant and call it.
**Predicted impact:** ~10 duplicated lines removed; low priority, near-duplicate not exact.

**Fixed:** 5e290c4e — extracted `store_pending(ack: PendingAck)`.

### Issue 10 — Systemic per-language extractor duplication in `ast/parser.rs`
**Symptom:** `extract_enum_variants` (Rust, `enum_variant_list`/`enum_variant`) and
`extract_java_enum_constants` (Java, `enum_body`/`enum_constant`) are structurally
identical modulo tree-sitter node-kind strings. The raw scan also flagged
`java_docstrings`/`kotlin_docstrings` and `go_docstrings`/`typescript_jsdoc` as similar
pairs — this looks like a systemic pattern across `ast/parser.rs`'s per-language
extractors, not a one-off.
**Root cause:** Each language's tree-sitter grammar was wired up independently, copying the
nearest existing extractor as a starting point.
**Fix:** Out of scope for this backlog's first pass — collapsing this properly means
auditing every per-language extractor in the file (not just the enum pair) and designing a
`(container_kind, item_kind)`-parameterized helper without breaking any single language's
parsing. Needs its own scoped pass, not an opportunistic fix.
**Predicted impact:** Unknown until the full audit is done — likely the largest single
duplication reduction in the file, but also the highest risk (touches parsing correctness
for every supported language).

### Issue 11 — `has_lsp_config` / `is_idempotent_lsp_method` (wontfix — false positive)
**Symptom:** `code-dupes` flagged `src/lsp/servers/mod.rs::has_lsp_config` and
`src/lsp/client.rs::is_idempotent_lsp_method` as 95%-similar.
**Root cause:** Both are single `matches!(x, "a" | "b" | ...)` allowlist expressions with a
similar arm count — the fingerprinter matches on AST shape, not on the (completely
disjoint) string values being matched (language names vs LSP method names). No shared
logic exists to extract.
**Fix:** None — logged so a future scan doesn't re-surface it as new.
**Predicted impact:** N/A.

## History

### 2026-07-08 — Tracker created
Initial `code-dupes` scan run against `src/` at HEAD `b2853b4d`. Backlog seeded with 11
findings (10 actionable, 1 wontfix false-positive). Issues 1, 2, 3, and 7 picked for the
first fix pass (cleanest, lowest-risk mechanical extractions).


### 2026-07-08 — First fix pass: issues 1, 2, 3, 7
Fixed the 4 cleanest/lowest-risk mechanical extractions (commits 2507e4eb, 991c0a51,
94a2008d, 99608519). Full verification: `cargo fmt` + `cargo clippy -- -D warnings` clean,
`cargo test --lib` 2957 passed / 0 failed / 6 ignored (unrelated). Net diff: -45 lines
across 6 files. Issues 4, 5, 6, 8, 9 remain open; issue 10 needs its own scoped pass;
issue 11 is wontfix (false positive).


### 2026-07-08 — Second fix pass: issues 4, 5, 6, 8, 9
Fixed the remaining open issues except 10 (commits 859a1f5c, 010872f9, a1c1b236,
169dd7d3, 5e290c4e). Verification: `cargo fmt` + `cargo clippy -- -D warnings` clean on
both default features and `--features dashboard`; `cargo test --lib` 2957/0/6 (default),
2973/0/6 (`--features dashboard`, includes 3 new characterization tests for issue 6).
Only issue 10 remains open (needs its own scoped pass); issue 11 stays wontfix.

Worth flagging for next time: issue 6 lives entirely behind a non-default Cargo feature
(`dashboard`) that neither the standard `cargo test`/`cargo clippy` invocations nor
`cargo rb` enable — easy to "verify" a change against that module without actually
compiling it. Always grep for `#[cfg(feature = ...)]` above a module's `pub mod` line
before trusting a green check on code under it.
