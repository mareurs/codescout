---
id: bad49b228663df9a
kind: bug
status: open
title: 'BUG: the catalog keeps the rows of a removed worktree (13 in this project), two of them still `open` for bugs the main checkout calls `fixed` and `wontfix`, and neither `doc(find)` nor `doctor` shows them'
owners:
- marius
tags:
- cluster/unclassified
- worktree-overlay
- librarian
opened: 2026-11-02
owner: marius
severity: low
unverified: Counts are from one read of one catalog on one machine. The cause is not established, and the ten rows without a main twin were not traced.
---

# BUG: the catalog keeps the rows of a removed worktree (13 in this project), two of them still `open` for bugs the main checkout calls `fixed` and `wontfix`, and neither `doc(find)` nor `doctor` shows them

## Summary

Removing a linked worktree leaves its artifact rows in the catalog. `~/.local/share/librarian/catalog.db` has 51 of 6,790 rows whose file does not exist. All 51 carry `missing_since`. 13 are under this project's removed worktrees (`codescout.worktrees/integration-fixes-20261024`: 12, `fix-g-20261024`: 1), all marked missing at 2026-10-24 13:15:51.

## Class fit

No class on the roster was walked. `cluster/unclassified` is the sanctioned escape.

## Symptom (Effect)

- Two stale rows say `open`: `3822f951e9d78ddf` and `9c90e767b8ae886b`. The rows for the same files in the main checkout say `fixed` (`5ee9a523ac9a7cde`) and `wontfix` (`13cffc336e09af35`). A caller that holds the old id sees the wrong status.
- `doc(find)` returns only the main row. `librarian(doctor)` for this project reports 0 violations with a path under `codescout.worktrees`. So only a remembered id reaches a stale row, and nothing tells anyone it exists.
- A call by such an id used to fail with a bare "No such file or directory (os error 2)". That message is fixed in `d1b47ab5b` (bug `c7777ef798c5361f`). The rows themselves are not.

## Reproduction

```
sqlite3 -readonly ~/.local/share/librarian/catalog.db "select substr(id,1,16), status, datetime(missing_since/1000,'unixepoch'), abs_path from artifact where abs_path like '%/codescout.worktrees/%' and missing_since is not null"
```

gives the 13 rows. Each file was checked with `os.path.exists` in a script.

## Environment

codescout `experiments`, catalog on this machine, 2026-11-02. The 38 other missing-file rows are in other projects (for example 22 in `whitelabel-crawler`).

## Root cause

Not established. `missing_since` is set by `reconcile_missing_since` (`src/librarian/catalog/gc.rs`) and `find` hides a row whose `missing_since` is older than a grace cutoff. Not checked: why nothing then removes or re-points the row, whether `merge_worktree` or `doc(graft)` was run for these worktrees, and what the rows of the other 38 have in common.

## Evidence

The query above; `doc(get)` on `3822f951e9d78ddf` (its `abs_path` is in the removed `fix-g-20261024`); the swapped-prefix match of three of the 13 to main rows. Ten of the 13 have no row at the swapped path; I did not look for a renamed or archived file.

## Hypotheses tried

None.

## Fix

Not done. Options, none chosen: `librarian(doctor, fix=prune_missing)` drops the rows under a dead root; `fix=reseat_worktree` reseats rows with no collision to their main path. Both write to the shared catalog, so each starts as a dry run and needs a decision.

### Dry runs, 2026-11-03

**Neither doctor fix reaches these 13 rows.** Both worktrees still have an ACTIVE overlay registration, and the librarian guide § Worktree overlay calls `reseat_worktree` the legacy fallback for rows with no active registration.

- `librarian(doctor, fix=prune_missing)` in batch mode listed 9 dead roots. It marks `integration-fixes-20261024` (12 rows) and `fix-g-20261024` (1 row) `would_skip: "active worktree registration"`. The only rows it would delete belong to other projects: `whitelabel-crawler` (22 artifact rows, 97 commit rows) and `eduplanner-mobile/docs/trackers` (1 row). So `prune_missing` does not fix this bug, and its batch mode would delete rows in projects this bug does not name.
- `librarian(merge_worktree, root=…/integration-fixes-20261024, dry_run=true)` would merge 12 shadows into their main twins. It reports 11 status conflicts, and in every one `main` and `worktree` agree (`fixed` or `mitigated`, base `open`). `9c90e767b8ae886b` maps to `13cffc336e09af35` with no conflict listed, so its `open` is the fork-time base, not a worktree change. The merge is delta-only, so it should not reopen the main row. Nothing is remapped and nothing is suspicious.
- `librarian(merge_worktree, root=…/fix-g-20261024, dry_run=true)` merges nothing. It reports a `reseat_collision`: shadow `3822f951e9d78ddf` against main `5ee9a523ac9a7cde`. So this root needs `abandon=true`, which drops the shadow and marks the registration abandoned.

**Proposed fix (needs the operator, because it writes the shared catalog and cannot be undone through git):**

1. `librarian(action="merge_worktree", root="/home/marius/work/claude/codescout.worktrees/integration-fixes-20261024")`
2. `librarian(action="merge_worktree", root="/home/marius/work/claude/codescout.worktrees/fix-g-20261024", abandon=true)`
3. Re-run the reproduction query: expect 0 rows.

Still not explained: why the overlay registrations stayed active after the worktrees were removed. A worktree removal that does not merge or abandon its registration would leave this state every time.

## Tests added

N/A. Nothing is fixed.

## Workarounds

Use the main-checkout id. Find it with `doc(find, filter={rel_path: {contains: ...}}, include_archived=true)`.

## Resume

Found while fixing `c7777ef798c5361f`. Only filed.

## References

- `docs/issues/archive/2026-11-02-doc-by-id-fails-with-a-bare-enoent-when-the-catalog-row-points-into-a-removed-worktree.md` (the error message half, fixed)
- `src/librarian/catalog/gc.rs` (`reconcile_missing_since`, `visibility_sql`)
