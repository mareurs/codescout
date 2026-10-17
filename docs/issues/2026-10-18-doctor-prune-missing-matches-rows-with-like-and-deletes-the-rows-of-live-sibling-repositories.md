---
status: fixed
opened: 2026-10-18
closed: 2026-10-18
severity: high
owner: marius
related: [docs/issues/2026-10-17-doc-update-writes-through-a-catalogued-symlink-and-rewrites-a-file-outside-the-root.md]
tags:
- cluster/unclassified
kind: bug
unverified: 'The same LIKE-is-case-insensitive class remains at other call sites that were NOT changed: catalog/gc.rs (detect_move_candidates, plan_rehome, rehome_commits), catalog/artifact.rs delete_orphan_repos, catalog/augmentation.rs resolve_cite_ref and list_stale, indexer.rs backfill_chunk_vectors_with. They use ESCAPE (wildcards are literal) but not an exact compare, so a case-only sibling is still matched; reachability and blast radius of each was not measured.'
---

# BUG: `librarian(doctor, fix=prune_missing)` matches rows with `LIKE` and no `ESCAPE`, so pruning a dead root deletes the rows of live repositories whose path differs only by case or by a `_`/`%` position

## Summary

`prune_dead_root` deleted `artifact` and `commits` rows with `abs_path LIKE '<root>/%'` (`git_root LIKE` for commits),
unescaped and ASCII-case-insensitive. Pruning the dead `/gone/my_app` therefore also deleted every row of the LIVE
`/gone/myXapp` and `/gone/MY_APP`, with their cascaded augmentation, events, links and chunks. `count_dead_root`, the
dry run, used the same predicate, so it counted them too and could not warn. Found by the round-4 review of the
symlink-escape work as "one older data-loss bug in the same class" as the indexer's reclaim (which round 3 fixed).

## Class fit

No existing class names a destructive catalog statement whose scope is a path PREFIX written as a SQL pattern, so the
selection is wider than the string the caller named. It is the same defect as the indexer's reclaim in the symlink-escape
issue (round 3, H1), in a second statement that nothing had swept.

## Symptom (Effect)

Rows of a live repository vanish from the catalog (and with them their augmentation params and events, which exist nowhere
else) after an unrelated dead root is pruned; `find` stops returning them until a reindex re-adds the plain rows.

## Reproduction

`prune_and_count_dead_root_spare_siblings_that_match_a_like_pattern` in `src/librarian/tools/doctor.rs` (red before the fix:
`left: (4, 1) right: (2, 1)` from `count_dead_root`, and `left: 3 right: 1` for the apostrophe/wildcard root), run with
`scripts/with-slot.sh cargo test --lib -- prune_`.

## Environment

Linux, `src/librarian/tools/doctor.rs`, branch `worktree-agent-a66600865e063ffd2` at `7a7bfc9b`.

## Root cause

`prune_dead_root` and `count_dead_root` selected with `abs_path = ?1 OR abs_path LIKE ?2` where `?2` was
`"{root}/%"`. In SQLite `LIKE` `_` and `%` in the pattern are wildcards and ASCII letters match case-insensitively, and no
`ESCAPE` clause was given. Measured 2026-10-18 by the red test above (the counts in Reproduction), not inferred.

## Evidence

The red run is recorded in the Reproduction section; the reviewer's probe P5 (`prune_dead_root_spares_like_pattern_siblings`)
showed the same.

## Hypotheses tried

1. **Hypothesis:** escaping `_`/`%` (as the indexer fix of round 3 did) is enough. **Test:** `MY_APP` against a root `my_app`.
   **Verdict:** rejected: `LIKE` still folds ASCII case, which no escape changes. The fix compares exactly.

## Fix

Commit `a6a293c0` (patch-id `a6a293c00d255ecfab8880b41a1914116edc6722`). `util::path_at_or_under_sql(col, root_param, prefix_param)` builds
`(col = ?root OR substr(col, 1, length(?prefix)) = ?prefix)`, byte-exact, with `_`, `%` and `'` literal; `prune_dead_root`
and `count_dead_root` use it for both tables, and the indexer's reclaim and vectorless count too. A Linux unit test runs the
helper on Windows-shaped strings (`//?/C:/...`).

## Tests added

`prune_and_count_dead_root_spare_siblings_that_match_a_like_pattern`, `prune_dead_root_matches_a_root_with_an_apostrophe_and_wildcards_literally`
(doctor.rs), `path_at_or_under_sql_is_exact_for_windows_shaped_paths` (util.rs).

## Workarounds

Do not run `fix=prune_missing` for a dead root whose name has a live sibling differing by case or by `_`/`%` before the fix.

## Resume

N/A. The unchanged call sites in the frontmatter caveat are the follow-up.

## References

- `docs/issues/2026-10-17-doc-update-writes-through-a-catalogued-symlink-and-rewrites-a-file-outside-the-root.md` (round 3, H1; round 4)
