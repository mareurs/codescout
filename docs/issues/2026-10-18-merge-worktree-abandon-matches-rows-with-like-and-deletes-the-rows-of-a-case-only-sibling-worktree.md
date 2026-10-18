---
status: fixed
opened: 2026-10-18
closed: 2026-10-18
severity: high
owner: marius
related: [docs/issues/2026-10-18-doctor-prune-missing-matches-rows-with-like-and-deletes-the-rows-of-live-sibling-repositories.md, docs/issues/2026-10-17-doc-update-writes-through-a-catalogued-symlink-and-rewrites-a-file-outside-the-root.md]
tags:
- cluster/unclassified
kind: bug
unverified: 'The merge (non-abandon) path selects the sibling rows too, but I could not make that harmful: with the LIKE predicate restored as a mutation, a real (non-dry) merge of feat_1 left the FEAT_1 row untouched, because reseat_one strips the worktree prefix case-sensitively and skips it. The test for that path is a regression pin, not a reproduced defect. The unchanged LIKE sites that remain are in docs/issues/2026-10-18-the-remaining-like-prefix-sites-are-case-folded-or-unbounded-and-none-is-destructive-today.md.'
---

# BUG: `librarian(merge_worktree, abandon=true)` selects a worktree's rows with `LIKE`, so abandoning `.worktrees/feat_1` deletes the rows of a case-only sibling worktree with their cascade

## Summary

`merge_worktree` chose "every catalog row under the registered worktree root" with `abs_path = ?1 OR abs_path LIKE <escaped root>/%`
(`descendant_path_like`). The escape makes `_` and `%` literal but SQLite's `LIKE` still ignores ASCII case, so a registration
for `/…/.worktrees/feat_1` also selected the rows of `/…/.worktrees/FEAT_1`. With `abandon=true` those rows were DELETED (with
their augmentation, events and links, which exist nowhere else), and the dry run counted them, so it could not warn. Same class as the
doctor prune (round 4, H-2).

## Class fit

No existing class names a destructive catalog statement whose scope is a path prefix written as a SQL pattern, so the selection is wider
than the root the caller named. It is the third statement of the same defect after the indexer's reclaim and the doctor prune.

## Symptom (Effect)

Reviewer probe (round 5): registration `/nonexistent-r4/.worktrees/feat_1`, rows under `feat_1`, `FEAT_1` and `featX1`:
`dry_abandoned=2 abandoned=2 FEAT_1_alive=false`.

## Reproduction

`merge_worktree_abandon_spares_the_rows_of_a_case_only_sibling_worktree` in `src/librarian/tools/link_dir_tests.rs`
(`scripts/with-slot.sh cargo test --lib -- merge_worktree`). Reproduced here by restoring the `LIKE` predicate as a mutation
(`mergelike` in the round-5 probe set), which turns it red.

## Environment

Linux, `src/librarian/tools/merge_worktree.rs`, branch `worktree-agent-a66600865e063ffd2`.

## Root cause

`merge_worktree::call` built its row selection and its `DELETE` from `util::descendant_path_like("?1")`, a `LIKE ... ESCAPE` predicate;
`LIKE` folds ASCII case regardless of `ESCAPE`. Measured by the reviewer's probe above and by the mutation run recorded under Tests added;
not inferred.

## Evidence

The probe output is in the Symptom section.

## Hypotheses tried

1. **Hypothesis:** the escape already protects the sibling. **Verdict:** rejected; it protects `_` and `%`, not case.

## Fix

`0b187952` (patch-id `5e48b25e9e01f0ccb1e18c0a878f8d4b880333eb`): the selection, the abandon `DELETE` and the dry-run count use
`util::path_at_or_under_sql` with the root and its prefix from `util::root_and_prefix`, exact and trailing-separator-safe. One
selection feeds both the abandon and the merge/reseat loop, so the non-abandon path is fixed by the same change.

## Tests added

`merge_worktree_abandon_spares_the_rows_of_a_case_only_sibling_worktree` (dry run counts 1, abandon deletes 1, `FEAT_1` and `featX1`
survive; red under the restored `LIKE`) and `merge_worktree_merge_does_not_reseat_the_rows_of_a_case_only_sibling_worktree` (a
regression pin: a real merge leaves the sibling's row and path alone; it also passes under the restored `LIKE`, see the caveat).

## Workarounds

Do not abandon a worktree whose directory name has a case-only sibling registered or catalogued, before the fix.

## Resume

N/A.

## References

- `docs/issues/2026-10-18-doctor-prune-missing-matches-rows-with-like-and-deletes-the-rows-of-live-sibling-repositories.md`
- `docs/issues/2026-10-17-doc-update-writes-through-a-catalogued-symlink-and-rewrites-a-file-outside-the-root.md` (round 5)
