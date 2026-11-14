---
id: '133504302d03e590'
kind: bug
status: open
title: 'BUG: a link added on a branch and on main merges as one row but two identical relation lines'
owners:
- marius
tags:
- cluster/unclassified
- librarian
opened: 2026-11-14
severity: low
unverified: 'Read from the review, not reproduced. Not established: the exact OR IGNORE statement in merge_worktree; read it before the fix.'
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
Decide with the operator's choice on the link format (execution-state document, decision I-2): sorted insertion makes identical lines meet at one place and merge as one. Plan E removes the row merge, which leaves only the text rule.

## Resume
Open. Not started. Found by the Plan C2 whole-branch review (M-9).

## References
- `src/librarian/tools/merge_worktree.rs`
- `src/librarian/catalog/link_text.rs` (`splice_relation`)
- `docs/research/2026-11-14-artifact-identity-plan-c2-execution-state.md`
