---
id: a6f259bd8bc32a62
kind: bug
status: open
title: 'BUG: the `cites` schema text and the manual say "Not from a worktree", but the code refuses id allocation on a main ledger, not cites'
owners:
- marius
tags:
- cluster/doc-contradicted-by-code
- librarian
opened: 2026-11-14
severity: low
unverified: 'Read from the code, not run. Not established: that `cites` on a worktree-born ledger works end to end (the born-here test passes no cites; the cite resolver was not read for worktree cases), and whether other prompt or guide text repeats the sentence.'
---

# BUG: the `cites` schema text and the manual say "Not from a worktree", but the code refuses id allocation on a main ledger, not cites

## Summary
The `cites` parameter description ends "Not from a worktree." and `entry-citations.md` says "Not supported from a worktree checkout." The code does not refuse `cites` from a worktree. It refuses any `append_entry` id allocation on a main-checkout ledger or on a worktree shadow of one. A ledger born in the worktree is not refused.

## Class fit
cluster/doc-contradicted-by-code: the schema text and the manual state a rule of the `cites` feature, and the code enforces a different rule, one level up, on the allocation, with a different scope.

## Symptom (Effect)
A reader concludes that `cites` is unavailable in any worktree session. For a ledger born in the worktree the allocation guard does not fire, so `cites` is not blocked by the code there. The text also hides the real remedy: repeat the call with the main row's id and `workspace=<main root>`.

## Reproduction
Not run. Read path:
- `src/librarian/tools/artifact.rs:484`: the `cites` description ends "Not from a worktree."
- `docs/manual/src/concepts/entry-citations.md` line 137: "Not supported from a worktree checkout." Line 98 says the allocation refusal is "on the same grounds it refuses `cites`". The heading at line 96 is "From the main checkout only".
- `src/librarian/tools/append_entry.rs:100` `refuse_allocation_off_the_main_ledger`: refuses when the session is in a worktree and the row is a main-checkout artifact or the session's own shadow of one, and when the row has a worktree fork record. Its doc says a ledger born in the worktree "is neither, and has no main twin to collide with". The refusal text is "id allocation is not supported from a worktree checkout" and names `workspace=<main root>`.
- Test `append_with_cites_from_worktree_is_refused` cites the main tracker's own id and expects the allocation refusal, so the guard, not `cites`, is the source. Test `prose_allocation_is_allowed_on_a_worktree_born_ledger` shows allocation succeeding on a worktree-born ledger (that test passes no `cites`).

## Environment
experiments at fe5370c2f.

## Root cause
The sentence dates from when `cites` and the worktree guard were one change. The guard is on the allocation, for every `append_entry` call, and it has the worktree-born exception. The `cites` text and the manual kept the narrower, older claim.

## Evidence
See Reproduction. The Plan C1 review also noted that the manual heading "From the main checkout only" is broader than its sentence.

## Hypotheses tried
None. Found by the Plan C1 whole-branch review (N-6).

## Fix
Not started. Reword both texts, for example: "`append_entry` refuses id allocation on a main ledger from a worktree session; repeat the call with the main row's id and `workspace=<main root>`." Drop "Not from a worktree" from the `cites` description. Check the prompt surfaces that copy the text.

## Tests added
None.

## Workarounds
Read the refusal text, which names the route.

## Resume
Open. Not started.

## References
- `src/librarian/tools/artifact.rs` (`cites` schema, line 484)
- `src/librarian/tools/append_entry.rs` (`refuse_allocation_off_the_main_ledger`, tests above)
- `docs/manual/src/concepts/entry-citations.md` (lines 96-104, 137)
- `docs/research/2026-11-13-artifact-identity-plan-c1-execution-state.md`
