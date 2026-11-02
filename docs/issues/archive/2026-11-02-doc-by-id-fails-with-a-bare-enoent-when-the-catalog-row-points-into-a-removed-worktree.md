---
id: c7777ef798c5361f
kind: bug
status: fixed
title: 'BUG: `doc` by id fails with a bare "No such file or directory (os error 2)" when the catalog row points into a worktree that was removed, and the main-checkout copy of the file has another id'
owners:
- marius
tags:
- cluster/unclassified
- worktree-overlay
- librarian
closed: 2026-11-02
opened: 2026-11-02
owner: marius
severity: low
unverified: The call through the server was not repeated on a rebuilt binary, and CI has not been read. The stale rows (13 in this project) are not repaired; their cause is not established.
---

# BUG: `doc` by id fails with a bare "No such file or directory (os error 2)" when the catalog row points into a worktree that was removed, and the main-checkout copy of the file has another id

## Summary

Catalog row `3822f951e9d78ddf` (kind `bug`, status `open`) has `abs_path` `/home/marius/work/claude/codescout.worktrees/fix-g-20261024/docs/issues/2026-10-24-a-worktree-supersedes-of-a-main-file-the-worktree-lacks-fails-after-forking-shadow-rows.md`. That directory does not exist, and it is not in `git worktree list`. `doc(action="get", id="3822f951e9d78ddf")` returns the row with `body_error: "No such file or directory (os error 2)"`. The same file exists in the main checkout and is cataloged under another id, `5ee9a523ac9a7cde` (status `fixed`).

## Class fit

No class on the roster was walked. `cluster/unclassified` is the sanctioned escape.

## Symptom (Effect)

A caller that holds the old id gets a bare operating-system message. It names no path and no worktree, so the caller cannot tell that the row is stale and that another id exists. On 2026-10-30 20:49, session `f844662c` called `doc(action="update", id="3822f951e9d78ddf", patch={status: "fixed", body_edits: [...]})`. The call ended with outcome `error`, not `recoverable_error`, and the message was only "No such file or directory (os error 2)". It is the only row with that message in the codescout usage database since 2026-10-25.

## Reproduction

Reproduced for `get` on 2026-11-02: `doc(action="get", id="3822f951e9d78ddf", heading="## Summary")`. The `update` failure is from the usage database:

```
sqlite3 -readonly .codescout/usage.db "SELECT called_at, outcome, error_msg FROM tool_calls WHERE tool_name='doc' AND error_msg LIKE 'No such file or directory%';"
```

gives one row, 2026-10-30 20:49. `ls` of the worktree directory fails with "No such file or directory".

## Environment

codescout `experiments`, main checkout, catalog on this machine. The row's title and its tags (`worktree-overlay`) are those of a bug about worktree shadow rows.

## Root cause

Unknown. Not checked: how the row came to point into that worktree, why removing the worktree did not delete or re-point the row, whether `doc(find)` lists the stale row next to the main one, and how many other rows point into removed worktrees.

## Evidence

The `get` response and the query above. Row ids and paths are copied from the live response.

## Hypotheses tried

None.

## Fix

Done on `experiments` at `d1b47ab5b`, for the reported effect only (the bare text and the hard error). In `src/librarian/read_door.rs`: `io_at` puts the path into the OS errors of `read_verified` and keeps the kind; `ReadError::into_anyhow` turns `NotFound` into a `LibrarianRecoverableError` whose hint says how to find the live row (`doc(find)` with a `rel_path` filter and `include_archived`). Other OS errors stay hard. Not done: the 13 stale rows stay in the catalog, and why a removed worktree leaves its rows is not addressed here. That is filed apart.


## Fix provenance

- **SHA:** `d1b47ab5bb521e68024d8d231fafdc5e180f2fc7` (`experiments`)
- **patch-id:** `62b31fc06dfb898770a4fe2f3f0b6c08cc30c206`

## Tests added

In `src/librarian/read_door.rs`. Red before the change, then green: `a_missing_file_is_an_io_error_that_names_its_path` and `a_missing_file_reaches_the_tool_boundary_as_a_recoverable_error_with_a_way_on` (it failed with the bare "No such file or directory (os error 2)"). A guard that passed before too: `an_invalid_utf8_file_stays_a_hard_io_error`. Gate in a clean worktree: FMT, CLIPPY, LEAN and DEFAULT all 0, 8,835 tests; `cargo test --lib librarian::` 2,532 passed. Not tested end to end: a `doc(get)` or `doc(update)` call on a row with a missing file through the server; the unit tests cover the door and the conversion at the tool boundary. CI has not been read.

## Workarounds

Use the id of the main-checkout row (`5ee9a523ac9a7cde`).

## Resume

Fixed on `experiments` (`d1b47ab5b`) for the reported effect. The stale rows are not repaired and their cause is not known; see the bug filed for them.

**Count of the stale rows, 2026-11-02** (`~/.local/share/librarian/catalog.db`, read-only; script `os.path.exists` over every `artifact.abs_path`):

- 51 of 6,790 rows point at a file that does not exist. All 51 have `missing_since` set, so the catalog already knows.
- 13 of the 51 are under `/home/marius/work/claude/codescout.worktrees/` (12 in `integration-fixes-20261024`, 1 in `fix-g-20261024`). All 13 carry `missing_since` 2026-10-24 13:15:51. Their statuses: 10 `fixed`, 2 `open`, 1 `mitigated`.
- 3 of the 13 have a row for the same file in the main checkout, found by swapping the worktree prefix: `3822f951e9d78ddf` (open) and `5ee9a523ac9a7cde` (fixed); `717298048575f667` (mitigated) and `fe747d6bdfe6ea5c` (mitigated); `9c90e767b8ae886b` (open) and `13cffc336e09af35` (wontfix). So two stale rows still say `open` for bugs that the main rows say are `fixed` and `wontfix`. The other 10 have no row at the swapped path; I did not look for a renamed or archived file.
- The other 38 are outside this project (for example `whitelabel-crawler` 22).
- `doc(find, filter={rel_path: contains …})` returns only the main row for `9c90e767b8ae886b`'s file, and `librarian(doctor)` for this project reports 0 violations with a path under `codescout.worktrees`. So the stale rows are reachable by id only, which is how the 10-30 call reached one.

## References

- `docs/issues/2026-10-24-a-worktree-supersedes-of-a-main-file-the-worktree-lacks-fails-after-forking-shadow-rows.md` (the bug whose file the stale row describes)
