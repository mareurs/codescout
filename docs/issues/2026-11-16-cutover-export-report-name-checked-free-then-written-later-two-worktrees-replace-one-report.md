---
id: '4f4b90618909a5e2'
kind: bug
status: open
title: 'BUG: two worktrees that run cutover export together can replace one report, because the report name is checked free before the write'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
opened: 2026-11-16
severity: low
unverified: The failing interleaving was not run. The check and the write were read at the bytes (cutover.rs:681 and :371); the race was not reproduced, and no test pins it.
---

# BUG: two worktrees that run cutover export together can replace one report, because the report name is checked free before the write

## Summary

`codescout cutover export` promises that it never overwrites a report (D-R17). `report_paths` (`src/cli/cutover.rs:654`) picks the first name `cutover-report-<name>-<commit12>[-n]` whose `.md` and `.json` do not exist. The files are written later, in the same function, by `write_catalogued_file`, which replaces a file atomically. Nothing holds the name between the check and the write. Two exports can pick the same name, and the second write replaces the first report.

## Class fit

The class `guard-narrower-than-its-name` fits: the guard is called "a report is never overwritten", but it covers only the moment of the check. The write lock that runs beside it is taken per checkout, and the report directory is shared by every checkout of the machine. This bug differs from the class's other members in that the gap is in time (check, then act), not in the inputs the guard accepts.

## Symptom (Effect)

Two reports are expected and one remains. The report that was lost is the record of that run: which files were written, which were refused, and which findings were reported. The exported files themselves are not affected.

## Reproduction

Not run. The interleaving, from the code:

1. Two worktrees of one repository are at the same commit. Both use the same old catalog, so both have the same report directory.
2. Both run `cutover export`. Each takes `<own git root>/.codescout/write.lock`, so neither waits for the other.
3. Both call `report_paths` before either writes. Both see the name free and return the same pair of paths.
4. Both call `write_catalogued_file`. The later rename replaces the earlier file.

## Environment

Branch `integration/identity-lifecycle-20261114` at `56dd96fe3`. Found by the 2026-11-16 status sweep of the identity session log (F-29, review item M57). It was parked by a ruling in Plan D Task 8 fix round 1.

## Root cause

- `export_run` takes the lock at `src/cli/cutover.rs:324-325` with `lock_root_for(&loaded.start, &loaded.git_root)` (`src/cli/write_lock.rs:32`). The lock file lies in the checkout, so two checkouts hold two different locks.
- The report directory is the parent of the old catalog (`cutover.rs:342`). The old catalog is machine-global, so the directory is shared.
- `report_paths` tests the names with `std::fs::symlink_metadata` (`cutover.rs:681`) and returns. The write is a separate step at `cutover.rs:371`.
- `write_catalogued_file` stages a file and renames it over the target (`src/librarian/util.rs:98-135`), so it never fails on an existing name.

## Evidence

The reads above. The review record is `.superpowers/sdd/2026-11-14-artifact-identity-plan-d-cutover-tool/` (git-ignored, in the main checkout), Task 8 fix round 1 and `final-review.md` § 3. The sweep report is `.superpowers/sdd/2026-11-16-identity-tracker-sweep/sweep-c.md`.

## Hypotheses tried

None. The cause is read from the code.

## Fix

Take the name with an exclusive create, not a check: open `<name>.md` with `create_new` (and the `.json` twin), and on `AlreadyExists` try the next number; then write into the file that this process created. Or keep the check and add a lock on the report directory.

## Tests added

None.

## Workarounds

Run `cutover export` from one checkout at a time. A real run also needs every codescout process stopped, so the case is rare.

## Resume

Open. Low severity: it needs two exports at the same moment from two worktrees of one repository at the same commit.

## References

- Session log `abff63149e3c24e8`, entry F-29.
- Plan D state doc `bb10d700b9552d01`.
