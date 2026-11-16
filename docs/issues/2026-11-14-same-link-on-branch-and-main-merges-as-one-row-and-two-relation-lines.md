---
id: '133504302d03e590'
kind: bug
status: fixed
title: 'BUG: a link added on a branch and on main merges as one row but two identical relation lines'
owners:
- marius
tags:
- cluster/unclassified
- librarian
closed: 2026-11-16
opened: 2026-11-14
severity: low
---

# BUG: a link added on a branch and on main merges as one row but two identical relation lines

## Summary
`merge_worktree` folds a shadow's link rows onto main with an `OR IGNORE` insert. If a worktree and main both add the same link, the rows merge into one. Since Plan C2 each link also writes a relation line into the source document, so the text holds the line twice, or git reports a merge conflict at the end of `## Relations`.

## Class fit
cluster/unclassified: rows and text merge by different rules (row merge ignores duplicates; text merge appends).

## Symptom (Effect)
After the merge, the source document has two identical `**Label:** [[key]]` lines, or the merge stops with a conflict in `## Relations`. The catalog has one row.

## Reproduction
Not reproduced. Read path: in a worktree session and in main, call `doc(link)` for the same source, target and rel; merge the branch with `merge_worktree` and git.

## Environment
experiments at 5fc3680f4.

## Root cause
`splice_relation` is idempotent only within one document state. The two branches each add the line to their own copy. The row merge does not know about the text.

## Fix

Fixed by Plan E2 Task 9. `splice_relation` (`src/librarian/catalog/link_text.rs`) now puts a new relation line before the first declaring line of the `## Relations` home whose text (trailing `\r` removed) is greater, with one blank line after it, and appends only when none is greater. Two branches that add the same line at the same place therefore produce identical text, and a three-way merge takes it once. Different lines at one place still conflict. Test: `the_same_link_added_on_two_branches_merges_as_one_line` (real `splice_relation` for ours and theirs, `git merge-file -p` exits 0; an always-append mutation conflicts). Guards: `a_line_takes_its_sorted_place_and_leaves_the_next_heading_alone`, `a_line_that_sorts_last_still_appends`, a CRLF case and a fenced-block case.

## Fix provenance

- **SHA:** `dfb931dbb` (`integration/identity-lifecycle-20261114`; reaches `experiments` with the next integration merge)
- **patch-id:** `b2a26cd3773115a316877e85bb2cbb5b4577fe33`

## Resume

N/A. Fixed 2026-11-16 by Plan E2 Task 9. The CRLF known limit: in a CRLF document whose greater line is the last line and has no terminator, the inserted line gets LF; the append path writes LF as before.

## References
- `src/librarian/tools/merge_worktree.rs`
- `src/librarian/catalog/link_text.rs` (`splice_relation`)
- `docs/research/2026-11-14-artifact-identity-plan-c2-execution-state.md`
