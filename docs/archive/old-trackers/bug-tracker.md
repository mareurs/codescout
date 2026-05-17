---
id: null
kind: null
status: archived
title: null
owners: []
tags: []
topic: null
time_scope: null
---
<!--
=========================================================================
RETIRED 2026-05-17 — historical reference only.

This tracker is no longer maintained. The 8 #N rows below have been
migrated to per-file format in docs/issues/ (active) and
docs/issues/archive/ (after fix shipped to master). New bugs go in
docs/issues/<date>-<slug>.md using docs/issues/_TEMPLATE.md — see
CLAUDE.md § Bug Tracking.

Migration commit: 2026-05-17. Body preserved verbatim for `git blame`
continuity. Do not append.
=========================================================================
-->

---
id: '0ed68e66d69ceec0'
kind: tracker
status: active
title: Bug Tracker
owners:
- '@mareurs'
tags:
- bugs
- tracker
topic: null
time_scope: null
---

## Audit scope and methodology

Tracks bugs noticed while working on codescout — its MCP server, its tools, the
companion plugin's hooks, LSP behavior, build scripts, and anything else that
misbehaves. Each issue gets a row in `params.issues`. Substantial investigations
(multi-session work, complex repro, evidence to gather) live in
`docs/issues/<date>-<slug>.md` and are linked via the row's `path` field;
trivial bugs (one-line fix) need only the param row + the fix commit.

The per-bug file skeleton lives at `docs/issues/_TEMPLATE.md`. Use it for any
issue where `path` is set.

## Per-issue detail

Short summaries (under 100 words per issue). Long investigations belong in
the per-bug file, not here.

### #1 — `edit_markdown insert_after` on H1 places content at section end

- **Symptom:** `edit_markdown(action="insert_after", heading="# Title")` on a
  top-level H1 inserts content at the END of the file (EOF area), not
  immediately after the heading line. For files where the H1 wraps the entire
  document, the content lands at the bottom.
- **Root cause:** The "insert_after a heading" semantic targets the END of that
  heading's section. For an H1 spanning the whole doc, the section ends at EOF.
  Defensible but counter-intuitive for top-level headings.
- **Workaround:** Use `action="edit"` with `old_string` matching the heading +
  the next non-blank line; place the new content between them in `new_string`.
- **Fix:** Open. No commit yet.

### #2 — `read_file(@buf, json_path="$.array[N].field")` returns 0 lines for array-element paths

- **Symptom:** `read_file(path="@tool_xxx", json_path="$.symbols[0].body")` returns `lines: 0` even though the buffer contains a populated `body` at `symbols[0]`. Object access on the same buffer (e.g. `json_path="$.context"`) works correctly.
- **Root cause:** Unknown — likely jsonpath dispatch difference between array-index-with-property access vs. plain object property access.
- **Workaround:** Use `read_file(path=@tool_, start_line, end_line)` and parse manually, or `read_file(path="<real-filesystem-path>", force=true)`.
- **Fix:** Open. Promoted from F-1 in `docs/trackers/archive/i1-session-friction.md`.

### #3 — `read_file(@buf, start_line=N, end_line=M)` returns empty content past buffer midpoint

- **Symptom:** Reading `@tool_*` buffer line ranges beyond roughly the buffer midpoint returns empty `content` with no error. The same buffer reads correctly with smaller `start_line`.
- **Root cause:** Unknown. Possible pagination or buffer-chunk offset miscalculation. Buffer total bytes reported correctly; only `start_line` past a threshold triggers the empty response.
- **Workaround:** Read filesystem path directly (`force=true`), or fetch from line 1 in two passes.
- **Fix:** Open. Promoted from F-2 in `docs/trackers/archive/i1-session-friction.md`.

### #4 — `grep(pattern, path="@tool_*")` false-negatives on strings present in the buffer

- **Symptom:** `grep` on an `@tool_*` buffer returns `{"matches": [], "total": 0}` for patterns verifiably present in the buffer (confirmed via `read_file` line-range on the same buffer immediately afterward). The tool emits a misleading suggestion: *"Pattern looks like a symbol name. Consider: symbols(name='…')."*
- **Root cause:** Likely two layered causes. (1) `grep` on `@tool_*` may not operate on raw buffer text the way it does on filesystem paths. (2) The symbol-name-suggestion router may intercept queries containing underscores/identifier-shaped tokens before the search runs.
- **Workaround:** Use `read_file(path=@tool_, json_path=…)` for structured fields, or `read_file(@tool_, start_line, end_line)` for sequential inspection. Reserve `grep` for filesystem paths.
- **Fix:** Open. Promoted from F-11 in `docs/trackers/archive/i1-session-friction.md`.

### #5 — `librarian(reindex)` fails with `UNIQUE constraint failed: artifact.abs_path`

- **Symptom:** Calling `librarian(action="reindex", scope="project")` on this project returns: *"UNIQUE constraint failed: artifact.abs_path"*. Default (non-force) path fails immediately; no rows updated.
- **Root cause:** Unknown. Hypothesis: prior walks left rows whose `abs_path` collides with what the current walk tries to insert; the upsert path doesn't reconcile. Possibly path normalization (trailing slash, symlink resolution) producing two rows for the same logical file.
- **Workaround:** None — `force=true` (which deletes rows first) hits #6 instead. The catalog is read-only for reindex until both #5 and #6 are fixed.
- **Fix:** **Fixed by commit `d482ca8a` (2026-05-17).** Root cause: `artifact::upsert` only handled `ON CONFLICT(id)`, not the schema's `abs_path UNIQUE` constraint. Fix: pre-clean `DELETE FROM artifact WHERE abs_path = ?1 AND id != ?2` before the INSERT. Verified post-rebuild — `reindex(scope=project)` now succeeds with `unchanged: 493, backfill_error_count: 0`.

### #6 — `librarian(reindex, force=true)` fails with embedding dimension mismatch (768 → 1)

- **Symptom:** Calling `librarian(action="reindex", scope="project", force=true)` returns: *"Dimension mismatch for inserted vector for the "embedding" column. Expected 768 dimensions but received 1."* Workspace status confirms `embeddings_model: jina-embeddings-v2-base-code` (768-dim). The embedding pipeline produced a 1-element vector instead of a 768-element one — likely an error sentinel that the writer did not gate against.
- **Root cause:** Unknown. Hypothesis: the embedding service hit an error condition and returned `vec![0.0]` (or similar 1-element fallback) without bubbling the error up. The writer (vec0 INSERT path) trusts the dimension and fails at the SQL layer rather than at the validation layer.
- **Workaround:** None known. Library indexing (0/62 indexed per `workspace(status)`) is blocked by the same code path.
- **Fix:** **Defensive-validation layer fixed by commit `d482ca8a` (2026-05-17).** `write_embeddings` now validates dim consistency before any INSERT: (1) non-empty batch, (2) all batch vectors share same length > 0, (3) batch length matches any existing `artifact_vec` row blob length. The original triggering condition (embedder returning 1-elem vectors) was not reproduced live today, but the validation path is verified via the unit test suite (2329 passed). If the embedder fails again with a sentinel value, the error fires before any DELETE, with a diagnostic message naming likely causes. Upstream fix (find why the embedder returns 1-elem vectors) still pending — see codescout-embed crate.

### #7 — `librarian(reindex, force=true)` cascade-deletes all augmentations (DATA LOSS)

- **Symptom:** Running `librarian(action="reindex", scope="project", force=true)` on this project (a) deletes all rows from `artifact` matching the targets, (b) cascades to delete all `artifact_augmentation` rows, (c) fails on the subsequent embedding INSERT (per #6), but (d) **does not roll back the DELETE**. Net effect: all augmented artifacts in the project lose their augmentation data permanently.
- **Root cause:** The force-DELETE in `src/librarian/tools/reindex.rs::call` is not wrapped in a SQLite transaction. The `DELETE FROM artifact WHERE abs_path LIKE ?1` auto-commits via SQLite's implicit-transaction-per-statement default; the re-walk + embedding INSERT run as later separate statements. When the INSERT fails (F-6b), the prior DELETE survives. Schema declares `artifact_augmentation.artifact_id REFERENCES artifact(id) ON DELETE CASCADE` (`src/librarian/catalog/schema.sql:116`), so cascade-removal of augmentations was always going to happen — but it should only happen if the rebuild succeeds.
- **Workaround:** Reconstruct augmentations from external sources (session transcripts, file content). Post-reconstruction, do NOT run `reindex(force=true)` again until this is fixed.
- **Fix:** **Fixed by commit `d482ca8a` (2026-05-17).** The destructive `DELETE FROM artifact WHERE abs_path LIKE` block in `reindex.rs::call` was removed. `force=true` is now a no-op pending proper plumbing through `index_repo_sync` (queued as task #31 — "plumb force_rewalk through index_repo_sync"). Verified post-rebuild: ran `reindex(scope=project, force=true)` against catalog with 4 augmentations; post-call augmented count remained 4. No cascade-delete. The destructive failure mode is structurally impossible.

### #8 — `state_at(commit=<short-sha>)` fails: lookup uses exact match not prefix

- **Symptom:** Calling `artifact(action="state_at", commit="d482ca8a")` (any short SHA) returns `commit d482ca8a not indexed; run librarian_reindex`. But the `commits` table IS populated (2931 rows; verified via `sqlite3 catalog.db "SELECT COUNT(*) FROM commits"`), and the full 40-char SHA works: `state_at(commit="d482ca8ac91241a7a96a487e46ca394095019912")` succeeds.
- **Root cause:** `src/librarian/tools/state_at.rs::resolve_cutoff_ts:30` uses `SELECT authored_at FROM commits WHERE hash = ?1` — exact match. Stored hashes are full 40-char; callers (humans + LLMs) pass short SHAs. Match fails → misleading "not indexed" error. The error message implies the table is empty when really the lookup mode is wrong.
- **Workaround:** Pass the full 40-char SHA, or use `timestamp=<unix-ms>` instead.
- **Fix:** Open. Change `=` to `LIKE ?1 || '%'` (or `GLOB ?1 || '*'` for case-sensitive) in `resolve_cutoff_ts`. Add a test with both short and full SHA. Tracks as task #32 in the session log. Note: this was originally misdiagnosed as F-5 ("commits table empty") in `docs/trackers/archive/artifact-code-linkage-session-log.md`; the correction landed post-rebuild verification 2026-05-17.

## History

### 2026-05-09 — Tracker bootstrapped

Created from the `audit_issues` archetype via `librarian(tracker_design)`.
Replaces the static `docs/issues/INDEX.md` shipped earlier on `experiments`
(commit b3b063b). Inaugural issue (#1) filed for the `edit_markdown` H1
footgun observed during this very bootstrap session.


### 2026-05-17 — #5, #6 filed (reindex broken)

Both `librarian(reindex)` paths fail on this project — default (#5: UNIQUE constraint) and force (#6: embedding dimension mismatch). Surfaced during artifact-code linkage reconnaissance (session log `docs/trackers/archive/artifact-code-linkage-session-log.md`, F-6). High severity — blocks the `commits` table backfill (which in turn blocks `state_at(commit=...)` per F-5) and library indexing (0/62 indexed).


### 2026-05-17 — #5, #6, #7 fixed (commit `d482ca8a`)

All three reindex bugs filed earlier this same day landed in one batched
commit. #5 (UNIQUE constraint) fixed via `artifact::upsert` pre-clean.
#6 (dim mismatch) fixed via defensive validation in `write_embeddings`.
#7 (cascade-delete DATA LOSS) fixed by removing the pre-walk DELETE
entirely; `force=true` is now a no-op pending proper plumbing (task
#31). Verified live post-rebuild: `reindex(scope=project)` succeeds;
`reindex(scope=project, force=true)` preserves all 4 augmentations.
