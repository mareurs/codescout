---
id: '056847ab0ed7d3e9'
kind: bug
status: open
title: 'BUG: cutover maps an old document id to an unrelated file when the rename destination was later deleted and the path reused'
owners:
- marius
tags:
- cluster/unclassified
opened: 2026-11-16
severity: low
unverified: The wrong mapping was not run. It follows from the code of head_path (history.rs:112-137); no test builds the history that triggers it.
---

# BUG: cutover maps an old document id to an unrelated file when the rename destination was later deleted and the path reused

## Summary

`HistoryIds::head_path` (`src/librarian/cutover/history.rs:112`) turns an old hex id into the path that the document has at `HEAD`. It follows git renames from the id's own path and stops at the first path that `HEAD` holds. It never asks whether that path is still the same document. If the destination of a rename was deleted later and a different file was added at that path, the id maps to the different file, and the cutover rewrites the citation to point at it.

## Class fit

No class in the roster states this mechanism: a resolver follows a chain of renames and trusts the last path without checking that the object at that path kept its identity. It is closest to the traits of "a value correct in one frame is published under a name that states another", but the roster text does not name path reuse in history. The class `cluster/unclassified` is used until a class review.

## Symptom (Effect)

A citation by an old hex id is rewritten to `[[key]]` of a file that is not the document it cited. The rewrite is silent: the report does not list it as a guess.

## Reproduction

Not run. History that triggers it, from the code:

1. Commit 1 adds `a.md`.
2. Commit 2 renames `a.md` to `b.md` (git sees a rename above the 90% similarity threshold). `renames["a.md"]` holds `(position, "b.md")`.
3. Commit 3 deletes `b.md`.
4. Commit 4 adds a new, unrelated `b.md`.
5. A document at `HEAD` cites the old id of `a.md`. `head_path(id)` finds that `a.md` is not in `HEAD`, follows the rename to `b.md`, finds `b.md` in `HEAD`, and returns `Ok("b.md")`.

## Environment

Branch `integration/identity-lifecycle-20261114` at `56dd96fe3`. Found by the 2026-11-16 status sweep of the identity session log (F-28, review item T4-M10). It is parked in the Plan D final review's park list (`final-review.md` § 3); no `Ruling:` line names it.

## Root cause

- `load` (`history.rs:73-82`) records every rename edge and every deleted path in `seen`, but keeps no record of a deletion after a rename.
- `head_path` returns at `history.rs:118` when `self.head.contains(&path)`. That test only says the path exists in `HEAD`. It does not say that the file at the path descends from the id's document.

## Evidence

The code at the lines above. The review record is `.superpowers/sdd/2026-11-14-artifact-identity-plan-d-cutover-tool/final-review.md` § 3 (git-ignored, in the main checkout). The sweep report is `.superpowers/sdd/2026-11-16-identity-tracker-sweep/sweep-c.md`. The existing test `a_deleted_files_id_never_maps_to_another_file_with_its_key` (`history.rs`) covers a deleted file, not a deleted rename destination.

## Hypotheses tried

None. The cause is read from the code.

## Fix

Record, per path, the position of its last deletion. In `head_path`, accept a path from the rename chain only when no deletion of that path comes after the rename edge that reached it. Then report the id as unresolved (`Err(origin)`), as the code already does for a chain that does not reach `HEAD`.

## Tests added

None.

## Workarounds

Read the cutover report for citations by old hex id and check the target of each rewrite when the repository history deletes and re-adds a path.

## Resume

Open. Low severity: it needs a rename, then a deletion of the destination, then a reuse of the same name, and a citation by the old id.

## References

- Session log `abff63149e3c24e8`, entry F-28.
- Plan D state doc `bb10d700b9552d01`.
