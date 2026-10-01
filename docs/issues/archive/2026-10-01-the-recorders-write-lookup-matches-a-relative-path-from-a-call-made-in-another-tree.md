---
id: 3094869ba182deab
kind: bug
status: fixed
title: The recorder's write lookup matches a relative path from a call made in another tree, so a worktree edit can name its writer as the owner of the same path here
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
---

# BUG: the recorder's write lookup matches a relative path from a call made in another tree, so a worktree edit can name its writer as the owner of the same path here

## Summary

`foreign_writer` in `scripts/post-index-change-stage-log.sh` matches a relative `path`, `rel_path` or
`new_rel_path` against this checkout's path with no regard to WHICH tree the call ran in. A session
editing `src/x.rs` inside a linked worktree (`<repo>.worktrees/<name>`) leaves a row whose relative
path is byte-identical to this checkout's `src/x.rs`. If another session then stages `src/x.rs`
here, the lookup names the worktree's writer as the path's owner (route `named-foreign`) although
the write never touched this tree.

## Symptom (Effect)

Not observed as a refusal. Measured as a population of wrong attributions in both directions (see Evidence): a writer whose rows were all in a worktree was named as the owner of this checkout's path, and nothing refused because of it was found; and a stager's own worktree row would return "mine" at once and hide a peer's real write here (the recorder returns as soon as the stager has a live row of its own), which no triple in the measured window showed but a fixture reproduces.

## Evidence

Measured 2026-10-01, about 17:00 UTC, against the live `.codescout/usage.db`, tree `8bfd3253`, unit = one
successful `edit_file`/`edit_code`/`create_file` row in the last three days: 1620 write rows, 1106 of
them with a relative `path`; 27 of those (2.4%) ran in a tree other than this checkout (the
`workspace` argument when the call carries one, else the `project_root` column), and all 27 name a
path that is tracked here. The trees: `codescout.worktrees/mutation-slot-1`, `port-28-test`, `il3-amp`
(sessions `e41af068` and `0b05903e`; 18 of the rows are `il3-amp`). Whether any of those paths was
staged here inside its three-day window was not searched for, so this is a latent defect with a
measured population, not an incident.

The guard's direction matters: this OVER-attributes a path to a writer who never touched it, a
false refusal of the stager's routine commit. The recorder already prefers the quiet old claim to a
loud wrong one for every other failure to read the record.

## Fix

`scripts/post-index-change-stage-log.sh`, `foreign_writer`: a RELATIVE path (`path`, a doc create's `rel_path`, a move's `new_rel_path`) counts only for a row whose `project_root` is this checkout, compared by **equality** (a worktree can sit under the checkout's directory). An absolute `path` and a doc id match from any tree. A row with a NULL or empty `project_root` is read as this checkout, as every row was before the column. **A table without the column gets no predicate at all**: the column's presence is asked first (`pragma_table_info`), because naming a missing column is a SQL error, which prints nothing and reads as "nobody else wrote it", switching the whole lookup off while every case that carries the column stays green.

Chosen over resolving each relative path against its row's root (which would also place a sub-project's relative path correctly): the data shows no sub-project root, and that resolution would assume a doc `rel_path` is relative to the active project, which the doc tool lets a `repo` argument change. Equality adds nothing the data does not ask for.

The same commit fixes `931c28128081ad46`, the suite's flaky cold-log precondition, which sits in the same file.

## Tests added

`tests/hooks-discrimination.sh` case 25 (a to k, and q), with `mkdb` now creating the CURRENT table shape (so every older case also runs the predicate against NULL rows) and `mkdb_old` creating the shape without the column. Red on the unchanged recorder: 25a, 25f-other, 25g-other, 25j, 25k (5 assertions); the rest pass there and rest on mutation. 25i is the missing-column fallback; 25q is a checkout path holding an apostrophe. Suite 284 passed, 0 failed, three runs in a row.

**Mutation:** 86 sites in the recorder and guard, re-run after the last byte change; 85 killed, each by a test that names it (the new ones: the predicate dropped from each of the three relative spellings, NULL and empty roots excluded, root matched as a prefix, root compared with the path, root not quoted, and the column check made always true, never true, or aimed at another column). The survivor is the SQLite busy timeout, tuning, annotated.

## Fix provenance

- **SHA:** `65d4e5dd512bd6e2184016e00ec23963a4c8a238` (`experiments`)
- **patch-id:** `892b172ceb8ec395699d8d5ca798a00751c01716`

Gate FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0; `tests/commit-mine.sh` 53/0, `tests/install-hooks-check-population.sh` 33/0, `tests/pre-push-foreign-session-guard.sh` 136/0. Residual: the recorder run inside a linked worktree reads that worktree's own (absent) `usage.db` and so records nothing there; a relative path resolved against a sub-project root is not handled and was never seen; and a row spelled with a different root than `git rev-parse --show-toplevel` would be excluded.

## How the tree of a call is recorded

Re-derived 2026-10-01, before the fix. `tool_calls.project_root` is written from `with_project_at(workspace_override, ...)` (`src/usage/mod.rs`), so it is the root the call operated in AFTER the call's own `workspace` pin: 4405 of 4405 rows with a `workspace` argument over seven days agree with it. There is no `cwd` column and no need to read `workspace` out of `input_json`. A call made in a linked worktree is filed in the MAIN checkout's `usage.db` (`worktree_main_root`) and tagged with the worktree's own root. Every row since 2026-09-01 carries a non-empty `project_root` in this database, but the column is added by a migration in `open_db` (the `friction_target` step), so a database no codescout has opened since is a real shape. Roots seen in 30 days: the main checkout, `<checkout>/.worktrees/<n>` (a path UNDER the checkout) and `<checkout>.worktrees/<n>` (a sibling). Sub-project roots (`crates/...`) never appear, so a relative path resolved against a sub-project is not a case the data shows.

The recorder run inside a linked worktree reads `<worktree>/.codescout/usage.db`, which does not exist (the telemetry went to the main checkout's), so there it records nothing and every staging is the legacy claim. That is a separate gap, not this defect.

## Measurement

**Population, unit, instant, tree.** The replay of the previous record, extended: non-merge commits with a `Session-Id` trailer over the three days to 2026-10-01 19:44 UTC at tree `3d3d2b45`, commits of more than 60 paths excluded: 211 commits, 620 `(commit, path)` pairs, 36 `(commit, path, writer)` triples with a foreign writer live under the own-commit rule, 18 of them still refused under the shipped text rule.

**Direction 1, a wrong owner named for a path of this checkout:** 4 of the 18 refusals (22%) name a writer all of whose live rows were made in a worktree: `src/util/path_security.rs` twice (`il3-amp`, `port-28-test`) and `src/symbol/edit.rs` twice (`mutation-slot-1`), by two sessions. Labelled by what the commit did with the writer's text: 3 ABSENT (the commit did not take it; the writer's text was never here) and 1 PRE-COMMITTED. None was a capture. Two of the sessions were running `mutation-probe.sh`, so ordinary mutation work on this checkout produces the rows.

**Direction 2, a real write here missed or hidden:** the predicate loses none: all 4 CAPTURED triples keep their writer, because their rows ran in this checkout. The masking case (the stager's own live row from another tree returning "mine" while a peer's write here is live) was searched for in the same population and found in 0 triples; it is pinned by a fixture (case 25k) and was red before the fix. A row spelled with a different root than `git rev-parse --show-toplevel` would be excluded and is the one way the predicate can lose a write: every root in this database is spelled identically (canonicalised by `Agent::new`), so none was found.

**On the live database**, `foreign_writer` for `src/symbol/edit.rs` named `e41af068` before and names nothing after, for a stager of `a520c25a`.

## References

- `docs/issues/2026-09-07-the-stage-log-records-the-stager-so-git-add--A-makes-you-the-owner.md` — the parent record.
- `docs/issues/archive/2026-10-01-the-stage-log-recorder-reads-a-subagents-composite-session-id-as-a-peer.md` — found in the same pass, by the same population measurement.
