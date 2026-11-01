---
id: 3cfa77a5bec68d96
kind: bug
status: fixed
title: 'BUG: config_propagation''s worktree walk follows directory symlinks, so with the resident-private links present the gate''s cargo test never finishes'
tags:
- cluster/unclassified
closed: 2026-11-01
opened: 2026-11-01
owner: marius
severity: high
---

## Summary

`discover_linked_worktrees` in `tests/config_propagation.rs` follows directory symlinks, because it tests `path.is_dir()`, which resolves links. `.codescout/resident-private/` now holds 1,876 directory symlinks that point back into the same tree (bug `35bcf086deeb7cbe`). With them present, the walk does not end, so `no_linked_worktree_points_at_a_gitdir_that_is_gone` and `the_walk_finds_every_worktree_git_itself_reports` run for good and the gate's `cargo test --workspace` never finishes.

## Class fit

A walker that follows links over a tree that holds cycles: the same mechanism as rust-analyzer's watcher spin in `35bcf086deeb7cbe`, met here by a test of ours. The walk was written to avoid `ignore::WalkBuilder`'s gitignore pruning and lost that walker's default of not following links along with it.

## Symptom (Effect)

On 2026-11-01 a gate run (`./scripts/gate.sh`, session `76494b3d-4b99-49c6-9924-4fd3a7cee027`) printed both tests as "has been running for over 60 seconds" and stayed there for more than 30 minutes. The `config_propagation` test binary in gate slot 1 ran 32 minutes at 193% CPU. The run was stopped by hand.

## Reproduction

A Python copy of the walk (same pruned names, same "`.git` is a file" stop), run from the repo root with a 60 s budget:

```
follow=False FINISHED dirs_visited=19340 after 1.0s
follow=True STOPPED dirs_visited>=1431639 queue=101 after 60.0s
```

Example link: `.codescout/resident-private/fixed-checker-transfer-20261101/review/mutant06/matched-workflow-20261031 -> /home/marius/work/claude/codescout/.codescout/resident-private/matched-workflow-20261031`.

## Environment

`experiments`, Linux, the shared checkout with the effort-steering experiment trees under `.codescout/resident-private/`.

## Root cause

`if !path.is_dir() { continue; }` follows a symlink to its target, so each directory link is queued and walked. Links that point back into the tree make the walk revisit it without end. Not established: which of the links closes the loop. Two earlier gates the same day passed these tests, so the loop probably formed with links created later that day; the record of `35bcf086deeb7cbe` gives the link counts.

## Fix

`9f544c4dc`: the walk tests `entry.file_type()`, which does not follow links, and skips anything that is not a real directory. A linked worktree that git reports sits at its real path, which a real-directory walk still reaches when it is under the root.

## Tests added

`a_directory_symlink_is_not_followed` in `tests/config_propagation.rs`: a real worktree `wt/` beside a link `a/up -> ..`. Red first: the walk reported `wt` 41 times, through `a/up` up to 40 levels deep, where the kernel's link-depth limit stopped it. Green after the fix.

- The whole `config_propagation` binary passes 6 of 6 in 1.07 s on the real tree, including the two tests that hung. The git-backed positive control, `the_walk_finds_every_worktree_git_itself_reports`, still finds every worktree `git worktree list` reports, so skipping links lost none.
- Mutant (`scripts/mutation-probe.sh`): restoring `if !path.is_dir()` is killed by the new test.
- Gate after the fix: `PYTHON_LIGHT=0 CLIPPY=0 LEAN=0 DEFAULT=0`; the `DEFAULT` lane ran these tests to the end. `FMT=1` was one line in an unrelated new file of the same session, since formatted.

## Resume

Fixed in `9f544c4dc`. Nothing left here. The symlinks themselves are bug `35bcf086deeb7cbe`.

## Fix provenance

- **SHA:** `9f544c4dc` (`experiments`)
- **patch-id:** `eb2c04ce8ec569573a09dbf02a61355e55f1255e`
