---
kind: bug
status: fixed
tags:
- cluster/unclassified
closed: 2026-10-24
opened: 2026-10-18
owner: marius
related:
- docs/issues/2026-10-18-doctor-prune-missing-matches-rows-with-like-and-deletes-the-rows-of-live-sibling-repositories.md
- docs/issues/2026-10-18-merge-worktree-abandon-matches-rows-with-like-and-deletes-the-rows-of-a-case-only-sibling-worktree.md
severity: low
unverified: '`plan_rehome`, `rehome_commits` and the `descendant_path_like` users (`covering_conn`, `shadow_main_pairs`) were not examined and not changed, as this issue itself said. KNOWN GAP in the `delete_orphan_repos` whole-volume guard (`bounds_no_territory`): the verbatim UNC prefix is matched case-sensitively, so a scope spelled `//?/unc/server/share` is NOT recognised as a whole share and is treated as an ordinary directory root. That misclassification fails OPEN (the guard does not skip it). Both current callers pass the same list as scope and active roots, so nothing extra is deleted today, but a future caller with different lists could. The opposite misclassification (a POSIX path starting with `//` read as a UNC root) fails closed. The commit message of 37cfe5a40 says every misclassification fails closed; that is wrong for the lowercase-UNC case. The Windows-shaped roots are tested as strings on Linux only.'
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

Fixed at all four sites. They now use the exact `path_at_or_under_sql` / `root_and_prefix` helpers (the root itself plus `root/...`, byte-exact, with a `/` boundary): `list_stale`, `delete_orphan_repos`, `backfill_chunk_vectors` / `backfill_chunk_vectors_with` (the parameter is now `scope_root`, a plain path), and `resolve_cite_ref` (byte-exact suffix match `substr(abs_path, -length(?)) = '/' || raw`). `delete_orphan_repos` ignores a scope that is a whole filesystem, volume or share (`bounds_no_territory`: `/`, a drive, `//?/C:`, `//server`, `//server/share`, `//?/UNC/server/share`). Behaviour changes, all in CHANGELOG.md: a row equal to a scope root is now in scope; a case or wildcard variant of an active root is no longer kept (deliberate, pinned by a test); a cite that matched only by case now fails to resolve; the `backfill-chunks` JSON key `scope.abs_path_like` is renamed `scope.abs_path_root`.

- Fix SHA (on `experiments`; merged 2026-10-24 by fast-forward, tip `e0255583b`): `37cfe5a40144c661ca585dfc29d38484d6ef7a20`
- patch-id: `1f019d71b303f7f8ae0988f31890bd6bb2e9a128`

An independent Opus review found no way for either caller of `delete_orphan_repos` to delete more than before.

## Fix provenance

- **SHA:** `37cfe5a40144c661ca585dfc29d38484d6ef7a20` (`experiments`)
- **patch-id:** `1f019d71b303f7f8ae0988f31890bd6bb2e9a128`

## Tests added

Each fails on the old code: `augmentation::tests::list_stale_scopes_to_the_project_path_exactly`, `artifact::tests::delete_orphan_repos_treats_roots_as_paths_not_like_patterns`, `artifact::tests::delete_orphan_repos_refuses_a_scope_that_is_a_whole_filesystem_or_volume`, `artifact::tests::a_root_with_no_component_below_its_prefix_is_not_a_bounded_scope`, `augmentation::tests::resolve_cite_ref_does_not_fold_case_in_a_rel_path`, `indexer::tests::a_scoped_backfill_does_not_reach_a_sibling_whose_path_merely_matches_the_root`. Gate on the merged tree: exit 0.

## Workarounds

None needed today for the destructive site (it deletes nothing). For the backfill, run it from a project whose directory name is not a
prefix of a sibling's.

## Resume

Pick one site, write its red test, switch it to the helper, mutation-probe the switch.

## References

- `docs/issues/2026-10-18-doctor-prune-missing-matches-rows-with-like-and-deletes-the-rows-of-live-sibling-repositories.md`
- `docs/issues/2026-10-18-merge-worktree-abandon-matches-rows-with-like-and-deletes-the-rows-of-a-case-only-sibling-worktree.md`
