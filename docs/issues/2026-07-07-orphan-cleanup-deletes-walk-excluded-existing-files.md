---
status: fixed
opened: 2026-07-07
closed: 2026-07-07
severity: high
owner: marius
related: [2026-07-07-reindex-reports-success-but-catalog-find-get-empty.md]
tags: [librarian, catalog, data-loss-risk, ignore-rules]
kind: bug
---

# BUG: `index_repo_sync`'s orphan-cleanup deletes catalog rows for files the walker skipped, even when the file still exists on disk

## Summary
`index_repo_sync`'s end-of-walk cleanup deletes every `artifact` row under the walked root whose
id was "not seen in this walk" — treating "the walker didn't visit this path" as equivalent to
"the file no longer exists". Those are not the same thing: `ignore::WalkBuilder::standard_filters(true)`
also skips paths matched by `.gitignore`, `.git/info/exclude`, or a global excludesfile, none of
which mean the file was deleted. Any project with such an ignore rule over a directory that is
still tracked in the catalog had every row under that directory silently deleted on **every single
reindex call** — this was the actual root cause of a separate, long-running Mercury BOM bug report
(`docs/issues/2026-07-07-reindex-reports-success-but-catalog-find-get-empty.md`), where
`docs/trackers/*.md` files were being wiped from the catalog on every reindex because Mercury
BOM's own `.git/info/exclude` lists `/docs/trackers/` (a local-only, never-published directory on
their publish-branch workflow).

## Symptom (Effect)
```
# Mercury BOM, live reproduction, 2026-07-07:
artifact(find, scope="project")  →  {"count": 11, "items": [... none under docs/trackers/ ...]}
artifact(find, filter={"rel_path":{"contains":"trackers"}}, scope="project")  →  {"count": 0, "items": []}
```
`docs/trackers/` has 11 real `.md` files on disk (confirmed via `list_dir`), including
`bom-requirements.md`, which had a working catalog row (id `7dc4b76a0c852674`) earlier in the
Mercury BOM bug's own session (its Evidence E1). By the time of this reproduction, zero rows exist
for anything under `docs/trackers/`.

## Reproduction
1. Commit: `4d5ead8b` (branch `experiments`, before this fix)
2. Any repo with a `.git/info/exclude` (or `.gitignore`) entry covering a directory whose files
   are already catalogued.
3. Run `librarian(action="reindex", force=true, scope="project")`.
4. `artifact(find)` for that directory returns `count: 0` — the rows are gone, even though the
   files are still on disk.
5. Minimal unit repro (added as a regression test, see Tests added): index a file, then re-index
   with an `ignore` glob that now matches the same still-existing file — the row is deleted.

## Environment
- codescout v0.15.0, `experiments` branch
- Reproduced live against Mercury BOM's real `.codescout` catalog and `.git/info/exclude`
- Not platform-specific — pure catalog-logic bug, reproduces identically on any OS

## Root cause
[src/librarian/indexer.rs](../../src/librarian/indexer.rs) (pre-fix) — the orphan-cleanup at the
end of `index_repo_sync` ran:
```sql
DELETE FROM artifact WHERE abs_path LIKE '<root>/%' AND id NOT IN (<seen_ids>)
```
`seen_ids` only contains ids for files the `ignore::WalkBuilder` actually visited this pass.
`standard_filters(true)` makes the walker skip anything matched by `.gitignore`/
`.git/info/exclude`/a global excludesfile — none of which imply deletion. The cleanup conflated
"not walked this pass" with "gone from disk", so it deleted rows for files that were simply
ignore-excluded, on every single reindex, forever, silently (no error, no warning — `reindex`
reports success with a plausible-looking `removed` count).

A previous session already suspected an ignore-related cause and added
`[ignored_paths] force_include = [...]` to Mercury BOM's `.codescout/project.toml` as an attempted
fix — that config key does not exist anywhere in codescout's source (confirmed via full-repo
grep); it has always been a silent no-op.

## Evidence
See `docs/issues/2026-07-07-reindex-reports-success-but-catalog-find-get-empty.md` Evidence E12
for the full live-reproduction transcript (find/get calls, `.git/info/exclude` contents, the
`force_include` grep-for-zero-matches check).

## Hypotheses tried
1. **Hypothesis:** The symptom is a restart-reverts-catalog-state issue (the sibling bug's
   Hypothesis 4).
   **Test:** N/A here — that hypothesis remains plausible as a separate, additional phenomenon,
   but is not required to explain this specific project's `find`-returns-empty symptom; this
   mechanism alone is 100% sufficient and reproduces deterministically on every reindex.
   **Verdict:** superseded as the primary explanation for Mercury BOM's specific symptom, not
   necessarily disproven as a separate issue.

## Fix
[src/librarian/indexer.rs](../../src/librarian/indexer.rs) — before deleting a "not seen"
candidate row, check `std::path::Path::new(&abs_path).exists()`; only delete if the file is
genuinely gone. Implemented as: SELECT candidate `(id, abs_path)` pairs matching the old WHERE
clause, then delete row-by-row only for candidates that fail the existence check. Correct
regardless of WHY the walker skipped a path (ignore rules, permission errors, or genuine deletion
all resolve correctly now).

**What this fix does NOT do:** it does not make ignore-excluded files visible to future reindexes
— the walker still skips them, so they won't be (re-)discovered or (re-)embedded. It only stops
the destructive side effect for already-catalogued rows. See the sibling bug's E12 for the
follow-up options (remove the exclude in the affected repo, or implement `force_include` for
real).

Local commit: `f48e50ed` on `experiments` (not pushed).

## Tests added
`index_does_not_delete_still_existing_file_newly_matched_by_ignore` —
[src/librarian/indexer.rs](../../src/librarian/indexer.rs): indexes a file normally, then
re-indexes with an `ignore` glob newly matching that same file; asserts the row survives and
`removed == 0`. The existing `index_removes_deleted_files` test (genuine on-disk deletion) was
re-verified unchanged/still passing — the fix only narrows the delete condition, doesn't disable
it.

## Workarounds
For repos hit by this before the fix: re-running `reindex` after upgrading to a build with this
fix will NOT retroactively restore already-deleted rows (nothing deletes further, but nothing
recreates what's gone either, since the walker still skips ignore-excluded paths). To restore
visibility for an ignore-excluded-but-wanted directory: temporarily remove the relevant
`.gitignore`/`.git/info/exclude` entry, run `reindex(force=true, reembed=true)` once to
repopulate, then decide whether to keep it un-excluded or wait for a real `force_include`
implementation.

## Resume
Consider implementing `[ignored_paths] force_include` as an actual feature (parse the config,
thread matching globs into the `WalkBuilder`'s override-ignore mechanism or a post-walk
supplemental scan) so the already-documented, already-attempted-by-users config key does what
people reasonably expect it to do. Scope: separate follow-up, not done as part of this fix.

## References
- [docs/issues/2026-07-07-reindex-reports-success-but-catalog-find-get-empty.md](2026-07-07-reindex-reports-success-but-catalog-find-get-empty.md) — the bug this root-causes, Evidence E12
- [src/librarian/indexer.rs](../../src/librarian/indexer.rs) — the fixed orphan-cleanup logic
