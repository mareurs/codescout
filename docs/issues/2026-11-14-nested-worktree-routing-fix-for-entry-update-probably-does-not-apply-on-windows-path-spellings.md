---
id: '24f1f923d02614a6'
kind: bug
status: open
title: The nested-worktree routing fix for entry update and readme get probably does not apply on Windows path spellings
tags:
- cluster/unclassified
opened: 2026-11-14
severity: low
unverified: Windows behaviour not tested. The mismatch is read from std's path parsing by one reviewer; no run on Windows, no test.
---

# BUG: the nested-worktree routing fix for entry update and readme get probably does not apply on Windows path spellings

## Summary

Lifecycle tools part 1 routes `doc(update)` on an entry, and `doc(get)` on a collection readme, by deriving a repo-relative path from a catalog row's `abs_path`. For a row inside a nested worktree (`<main>/.worktrees/x/docs/trackers/...`) the fix `5bfeeee11` narrows the root with `util::door_root`. On Windows the catalog stores `//?/C:/..` while a native root is `\\?\C:\..`; the final review of the fix wave read that `Path::strip_prefix` probably does not match them. If so, the narrowing never applies there and the nested-worktree entry silently takes the generic update path. Code is on `integration/identity-lifecycle-20261114` (`18e0ad3c2`), not yet on `experiments`.

## Class fit

No reserved class matches: this is a platform-specific path-spelling gap in a routing helper, found by reading. `cluster/unclassified` is used so the counts that promotion reads are not bent.

## Symptom (Effect)

On Windows, a main-checkout session that updates an entry in a nested worktree writes an undeclared status with no `status_since` (generic path), and a readme get in that case returns prose with no table, with no message. No data loss. No worse than before the lifecycle tools branch.

## Reproduction

Unverified, not run, and not runnable on this Linux host. The reviewer read `mod.rs` lines 401-409 and `util.rs` `descends_without_link` against std's `parse_prefix`. No test covers the Windows spelling for this narrowing.

## Environment

Windows, catalog abs_path in the `//?/C:/..` spelling, root in the `\\?\C:\..` spelling. Branch `integration/identity-lifecycle-20261114` at `18e0ad3c2`.

## Root cause

Probable, unconfirmed: `Path::strip_prefix` on mixed verbatim spellings. The helper `windows_comparable` in `tools/mod.rs` exists for this and is used by `containing_root` and `rel_path_below`, but the narrowing step through `door_root` and `descends_without_link` is not shown to use it.

## Evidence

- Final-review re-review of the fix wave, Minor 3 (from reading std's `parse_prefix`; the reviewer could not check it on this host).
- The Task 9 review found the same class in `root_and_rel_path` (Important) and it was fixed with `windows_comparable`.

## Hypotheses tried

None run.

## Fix

Not started. Route the narrowing through `windows_comparable`, then add a unit test with both spellings (a pure string test needs no Windows host if the comparison is made on normalised strings).

## Tests added

N/A: no fix yet.

## Workarounds

On Windows, update an entry through a session whose root is the worktree itself.

## Resume

Open. Parked in the memory `tracker-lifecycle-parked-items` (§ Parked from tools part 1) and in session log entry F-5 (`tracker-organization-session-log`).

## References

- docs/trackers/tracker-organization-session-log.md (F-5)
- docs/superpowers/plans/2026-11-14-tracker-lifecycle-tools-part-1.md (Tasks 9 and 10)
