---
status: open
opened: 2026-10-18
severity: low
owner: marius
related: [docs/issues/2026-10-18-doctor-prune-missing-matches-rows-with-like-and-deletes-the-rows-of-live-sibling-repositories.md, docs/issues/2026-10-18-merge-worktree-abandon-matches-rows-with-like-and-deletes-the-rows-of-a-case-only-sibling-worktree.md]
tags:
- cluster/unclassified
kind: bug
---

# BUG: the remaining `LIKE` path-prefix call sites fold ASCII case or have no `/` boundary; none is destructive today, and none was changed

## Summary

Rounds 3-5 of the symlink-escape work replaced `LIKE` with the exact `util::path_at_or_under_sql` at every site that DELETES or
reseats rows (indexer reclaim, doctor prune/count, `merge_worktree`). Four other sites still select rows with `LIKE`. They are
listed here accurately (an earlier note of mine said they all use `ESCAPE`; they do not), with what each does with the match, so
nobody has to re-derive which of them matter.

## Class fit

Same defect as the doctor-prune and `merge_worktree` issues (a path prefix written as a SQL pattern, so the selection is wider than the
named root), at sites where the consequence is a wrong listing, a wrong vector filing or a wrong citation target rather than a
deletion. Kept in one file because none is destructive and the fix is the same helper.

## Symptom (Effect)

Not observed live. Read from the code on 2026-10-18 (inferred from the lines below, not run), after the round-5 review's measurements:

| site | predicate | escaped? | what the match does |
|---|---|---|---|
| `list_stale`, `src/librarian/catalog/augmentation.rs:2924-2927` | `a.abs_path LIKE '<prefix>/%'` | no `ESCAPE`: `_`/`%` in the prefix are wildcards, and case folds | lists a sibling project's stale augmentations (read-only) |
| `delete_orphan_repos`, `src/librarian/catalog/artifact.rs:521-537` | `abs_path LIKE '<root>/%'` (scope and active lists) | no `ESCAPE` | DELETES rows "under scope and not under active"; both callers (`tools/reindex.rs:320`, `librarian/mod.rs:607`) pass the same list as scope and active, so the set is empty and it deletes nothing today |
| `backfill_chunk_vectors_with`, scope `a.abs_path LIKE ?4` at `src/librarian/indexer.rs:1416`, bound from `src/cli/backfill_chunks.rs:72` | `"<abs_path>%"` | no `ESCAPE` and no `/` boundary: `/x/app` also matches `/x/app2/...` and `/x/application/...` | add-only: embeds a sibling's artifacts and can file their vectors under the current project |
| `resolve_cite_ref`, `src/librarian/catalog/augmentation.rs:2481-2483` | `abs_path LIKE '%/<escaped raw>'` | escaped, case-folded | resolves an `append_entry` cite to a row whose path differs only in case; feeds an `entry_cite` write |

`plan_rehome` and `rehome_commits` (`src/librarian/catalog/gc.rs`) strip the prefix case-sensitively in Rust and are safe by the
reviewer's reading; I did not re-derive that. `descendant_path_like` is also used by `catalog/worktree.rs` (`covering_conn`) and
`tools/worktree.rs` (`shadow_main_pairs`), which fold case; not examined beyond noting the predicate.

## Reproduction

Not yet reproducible as a failing test; the first step of a fix is a test per site with a case-only or `_` sibling, as
`prune_and_count_dead_root_spare_siblings_that_match_a_like_pattern` does.

## Environment

Linux, branch `worktree-agent-a66600865e063ffd2`.

## Root cause

`LIKE` is ASCII-case-insensitive and treats `_`/`%` in the pattern as wildcards unless escaped; an `ESCAPE` clause fixes only the
second. Inferred from the lines in Symptom, plus the measured behaviour at the three fixed sites.

## Evidence

See Symptom.

## Hypotheses tried

N/A.

## Fix

Not done, by scope. Route each site through `path_at_or_under_sql` and `root_and_prefix` (for the backfill, also pass the root and the
`/`-terminated prefix instead of `"<root>%"`).

## Tests added

N/A: nothing was changed (justification: the sites are not destructive today and were outside the round's list).

## Workarounds

None needed today for the destructive site (it deletes nothing). For the backfill, run it from a project whose directory name is not a
prefix of a sibling's.

## Resume

Pick one site, write its red test, switch it to the helper, mutation-probe the switch.

## References

- `docs/issues/2026-10-18-doctor-prune-missing-matches-rows-with-like-and-deletes-the-rows-of-live-sibling-repositories.md`
- `docs/issues/2026-10-18-merge-worktree-abandon-matches-rows-with-like-and-deletes-the-rows-of-a-case-only-sibling-worktree.md`
