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
- **Fix:** Open. Promoted from F-6a in `docs/trackers/artifact-code-linkage-session-log.md`. Blocks F-5 investigation (commits table backfill needs reindex to work).

### #6 — `librarian(reindex, force=true)` fails with embedding dimension mismatch (768 → 1)

- **Symptom:** Calling `librarian(action="reindex", scope="project", force=true)` returns: *"Dimension mismatch for inserted vector for the "embedding" column. Expected 768 dimensions but received 1."* Workspace status confirms `embeddings_model: jina-embeddings-v2-base-code` (768-dim). The embedding pipeline produced a 1-element vector instead of a 768-element one — likely an error sentinel that the writer did not gate against.
- **Root cause:** Unknown. Hypothesis: the embedding service hit an error condition and returned `vec![0.0]` (or similar 1-element fallback) without bubbling the error up. The writer (vec0 INSERT path) trusts the dimension and fails at the SQL layer rather than at the validation layer.
- **Workaround:** None known. Library indexing (0/62 indexed per `workspace(status)`) is blocked by the same code path.
- **Fix:** Open. Promoted from F-6b in `docs/trackers/artifact-code-linkage-session-log.md`. The fix is partly defensive (validate `vec.len() == expected_dim` before INSERT; return diagnostic error early) and partly upstream (find why the embedding service returns a 1-element vector — probably needs an error-propagation fix in `codescout-embed` or the indexer's embed loop).

### #7 — `librarian(reindex, force=true)` cascade-deletes all augmentations (DATA LOSS)

- **Symptom:** Running `librarian(action="reindex", scope="project", force=true)` on this project (a) deletes all rows from `artifact` matching the targets, (b) cascades to delete all `artifact_augmentation` rows, (c) fails on the subsequent embedding INSERT (per #6), but (d) **does not roll back the DELETE**. Net effect: all augmented artifacts in the project lose their augmentation data permanently.
- **Root cause:** The force-DELETE in `src/librarian/tools/reindex.rs::call` is not wrapped in a SQLite transaction. The `DELETE FROM artifact WHERE abs_path LIKE ?1` auto-commits via SQLite's implicit-transaction-per-statement default; the re-walk + embedding INSERT run as later separate statements. When the INSERT fails (F-6b), the prior DELETE survives. Schema declares `artifact_augmentation.artifact_id REFERENCES artifact(id) ON DELETE CASCADE` (`src/librarian/catalog/schema.sql:116`), so cascade-removal of augmentations was always going to happen — but it should only happen if the rebuild succeeds.
- **Workaround:** Reconstruct augmentations from external sources (session transcripts, file content). Post-reconstruction, do NOT run `reindex(force=true)` again until this is fixed.
- **Fix:** Open. Promoted from F-9 in `docs/trackers/artifact-code-linkage-session-log.md`. Three-part fix:
  1. Wrap force-DELETE + re-walk + embedding INSERT in a single SQLite transaction (`BEGIN; ... COMMIT;` or `Transaction::new` in rusqlite). Failure of any later step rolls back the DELETE.
  2. Until #1 ships, document the data-loss risk in the reindex tool description and the `force` field's help text.
  3. Architectural consideration: should the augmentation key be content-derived (`abs_path` or hash) rather than synthetic `id`? Would survive artifact-row recreation by design.

## History

### 2026-05-09 — Tracker bootstrapped

Created from the `audit_issues` archetype via `librarian(tracker_design)`.
Replaces the static `docs/issues/INDEX.md` shipped earlier on `experiments`
(commit b3b063b). Inaugural issue (#1) filed for the `edit_markdown` H1
footgun observed during this very bootstrap session.


### 2026-05-17 — #5, #6 filed (reindex broken)

Both `librarian(reindex)` paths fail on this project — default (#5: UNIQUE constraint) and force (#6: embedding dimension mismatch). Surfaced during artifact-code linkage reconnaissance (session log `docs/trackers/artifact-code-linkage-session-log.md`, F-6). High severity — blocks the `commits` table backfill (which in turn blocks `state_at(commit=...)` per F-5) and library indexing (0/62 indexed).
