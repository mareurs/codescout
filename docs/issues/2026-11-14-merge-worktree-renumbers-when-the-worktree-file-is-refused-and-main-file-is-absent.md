---
id: '75e128d67e85d17b'
kind: bug
status: open
title: 'BUG: merge_worktree renumbers an entry when the worktree file is refused and main''s file is absent'
owners:
- marius
tags:
- cluster/unclassified
- librarian
opened: 2026-11-14
severity: low
unverified: 'Read from the code, not reproduced. Not established: that a merge can reach this state in practice (a shadow row whose main twin has no file on disk), nor whether a refused worktree file in that state ever defines an id by heading. No test covers the combination.'
---

# BUG: merge_worktree renumbers an entry when the worktree file is refused and main's file is absent

## Summary
`renumbers_defined_by_heading` (`src/librarian/tools/merge_worktree.rs:542`) decides which renumbered ids a heading defines. When the worktree's shadow file is present but the read door refuses it, and main's file is not on disk, it returns an empty list. An empty list means nothing is held, so the merge renumbers the row. The function's own doc says it "fails closed" when the source cannot be read.

## Class fit
cluster/unclassified: a guard documented as failing closed that fails open in one corner. The "main file absent defines nothing" arm was written for a params-only ledger and also catches a source nobody could read.

## Symptom (Effect)
A worktree file that defines `F-n` by heading, which the door refuses (for example a symlink to a file outside the repository), plus a main file that is not on disk: the merge renumbers `F-n` in params. If the refused file does define it, the entry is split between params and markdown. No tool renumbers an entry in both stores afterwards.

## Reproduction
Not reproduced. Read path, `merge_worktree.rs` lines 582-586:
- `present` is true, so `source` is `None` (the door refused) and `unreadable_by` is `WorktreeFileRefused`.
- `door.text(&main_row.abs_path)` returns `ReadError::Io` with `NotFound`.
- The arm `(Vec::new(), unreadable_by)` returns no held ids, with `source = worktree_file_refused`.

Existing tests: `a_refused_shadow_file_never_falls_through_to_the_branch_tip` and `a_dangling_shadow_link_is_a_refused_file_not_an_absent_one` both have main's file present. No test has a refused worktree file and an absent main file.

## Environment
experiments at fe5370c2f.

## Root cause
The `NotFound` arm for main's file assumes that no main file means no heading to protect ("a params-only ledger merges as before"). That holds when the worktree source was read. When the source was not read, an absent main file is not evidence that the ids are undefined.

## Evidence
- Doc comment on `renumbers_defined_by_heading`: "FAILS CLOSED when the source cannot be read ... A main file that is not on disk defines nothing, so a params-only ledger merges as before."
- The Plan C1 review recorded this as a deferred finding under Task 4: "a present worktree file the door refuses plus a missing main file lets the renumber proceed (same as old code)".

## Hypotheses tried
None. Found by the Plan C1 whole-branch review (N-3), read from the code.

## Fix
Not started. For a source that was present but refused (`WorktreeFileRefused`), treat an absent main file like an unreadable one: hold every renumbered id (`remap.keys()`), report source `unreadable` or `worktree_file_refused`, and name the exits in `remedy`. Keep the empty-list arm for a source that was absent and has no branch tip only if that case is judged safe. Add the missing test.

## Tests added
None.

## Workarounds
Before a merge, make the worktree's file a readable regular file inside the repository.

## Resume
Open. Not started.

## References
- `src/librarian/tools/merge_worktree.rs` (`renumbers_defined_by_heading`, `DecidedBy`)
- `docs/research/2026-11-13-artifact-identity-plan-c1-execution-state.md`
