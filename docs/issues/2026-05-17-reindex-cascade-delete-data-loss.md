---
status: fixed
opened: 2026-05-17
closed: 2026-05-17
severity: critical
owner: marius
related: []
tags: ["librarian", "reindex", "data-loss", "cascade-delete", "transaction"]
kind: bug
---

# BUG: `librarian(reindex, force=true)` cascade-deleted all augmentations (DATA LOSS)

## Summary

Running `librarian(reindex, force=true)` on this project deleted all rows from `artifact` matching the targets, cascaded to delete all `artifact_augmentation` rows, then failed on the subsequent embedding INSERT (per the dim-mismatch bug). The DELETE was not wrapped in a transaction and the failure did not roll it back. Net effect: all augmented artifacts in the project lost their augmentation data permanently. Fixed by commit `d482ca8a` (2026-05-17) — removed the destructive DELETE block entirely.

## Symptom (Effect)

After `librarian(action="reindex", scope="project", force=true)`:

1. `artifact` rows matching the targets were deleted.
2. `artifact_augmentation` rows cascaded-deleted via the schema's `REFERENCES artifact(id) ON DELETE CASCADE`.
3. The subsequent embedding INSERT failed with the dim-mismatch error (bug-tracker.md #6).
4. **No rollback** — the DELETE survived the INSERT failure.

Net effect: 4 augmented artifacts in the project lost their augmentation data permanently. Recovered manually from the session transcript.

## Reproduction

Pre-fix: any call to `librarian(reindex, force=true)` on a project where the embedder hit any error during write. The DELETE auto-committed via SQLite's implicit-transaction-per-statement default; the re-walk + embedding INSERT ran as later separate statements.

## Environment

- Date observed: 2026-05-17
- Tool: `mcp__codescout__librarian(reindex, force=true)`
- Component: `src/librarian/tools/reindex.rs::call`
- Schema: `src/librarian/catalog/schema.sql:116` declares `artifact_augmentation.artifact_id REFERENCES artifact(id) ON DELETE CASCADE`

## Root cause

The force-DELETE in `reindex.rs::call` was not wrapped in a SQLite transaction. The `DELETE FROM artifact WHERE abs_path LIKE ?1` auto-committed via SQLite's implicit-transaction-per-statement default. The re-walk + embedding INSERT ran as later separate statements. When the INSERT failed (cascade trigger: dim mismatch — see #6), the prior DELETE survived.

Schema declared `artifact_augmentation.artifact_id REFERENCES artifact(id) ON DELETE CASCADE` (`src/librarian/catalog/schema.sql:116`), so cascade-removal of augmentations was always going to happen — but it should only happen if the rebuild succeeded.

## Evidence

- Reproduced live during the artifact-code-linkage session: 4 augmentations vanished from `artifact(find, augmented=true)` after `reindex(force=true)` failed.
- Recovery: re-augmented all 4 from session transcript data — see session log F-9.
- Post-fix verification: ran `reindex(scope=project, force=true)` against catalog with 4 augmentations; post-call augmented count remained 4.

## Hypotheses tried

1. **Hypothesis:** Wrap the DELETE + walk + INSERT in a single SQLite transaction. **Verdict:** Considered but rejected — the right fix is to remove the DELETE entirely and re-do `force` semantics as a proper rewalk pass (no destructive intermediate state). **Evidence link:** see Fix.
2. **Hypothesis:** Remove the destructive DELETE; let `force=true` be a no-op pending proper plumbing through `index_repo_sync`. **Verdict:** Confirmed — adopted as the fix. **Evidence link:** see Fix.
3. **Follow-up hypothesis:** Plumb `force_rewalk` through `index_repo_sync` so `force=true` re-walks every file regardless of cached hash. **Verdict:** Confirmed — shipped in follow-up commit `2f085f45` (2026-05-17). **Evidence link:** see Fix.

## Fix

**Cascade-delete eliminated (commit `d482ca8a`, 2026-05-17):** The destructive `DELETE FROM artifact WHERE abs_path LIKE` block in `reindex.rs::call` was removed entirely. `force=true` is now a no-op pending proper plumbing.

**Force semantic restored (commit `2f085f45`, 2026-05-17):** `force_rewalk: bool` plumbed through `index_repo_sync`. Hash-equal early-return gated on `!force_rewalk`. `force=true` now re-walks regardless of cached hashes — no DELETE, no cascade.

Verified post-rebuild: ran `reindex(scope=project, force=true)` against catalog with 4 augmentations; post-call augmented count remained 4. No cascade-delete. The destructive failure mode is structurally impossible.

## Tests added

`force_wipes_then_reindexes` test in `src/librarian/tools/reindex.rs` rewritten three times across the fix cycle:

1. (Original) Asserted destructive behavior.
2. (Post-d482ca8a) Inverted to no-op assertions.
3. (Post-2f085f45) Final form — asserts `added=0, updated=1, unchanged=0` (proper rewalk semantic).

## Workarounds

Pre-fix: do NOT run `reindex(force=true)`. Use `reindex(scope=project)` only (default, non-force path) — also failed but at least non-destructively.

## Resume

N/A — fixed. After commits `d482ca8a` and `2f085f45` land on master, move this file to `docs/issues/archive/`.

## References

- Originally tracked as **#7** in `docs/issues/bug-tracker.md` (retired after migration to per-file system).
- Session log: `docs/trackers/archive/artifact-code-linkage-session-log.md` F-9 (incident) and F-9 fix-verified note.
- Fix commits: `d482ca8a` + `2f085f45` on `experiments`.
- Related: bug-tracker.md #5 (UNIQUE), #6 (dim mismatch) — same commit cluster.
- Schema reference: `src/librarian/catalog/schema.sql:116`.
