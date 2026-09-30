---
id: fe8affc7141081ef
kind: bug
status: open
title: 'BUG: commit-mine exits 0 and leaves a coupled path behind when the stage-log recorder cannot attribute a hunk-staged file'
owners:
- marius
tags:
- cluster/authorship-unrecoverable-after-the-fact
- commit-hooks
- shared-checkout
closed: ''
opened: 2026-09-30
severity: medium
---

# BUG: commit-mine exits 0 and leaves a coupled path behind when the stage-log recorder cannot attribute a hunk-staged file

## Summary

`scripts/commit-mine.sh` commits the staged paths the stage log attributes to the caller and leaves
the rest, exiting 0. The recorder (`scripts/post-index-change-stage-log.sh`) attributes a staged
`(blob, path)` pair only when the index write comes from a recognised staging command; a pair first
seen any other way is stamped `-`, which reads as foreign. The two ordinary ways to stage *some
hunks of a file a peer is also editing*, `git add -p` and `git update-index --cacheinfo`, both
produce `-` rows. So the caller's own hunk is reported "not yours", left in the index, and the
commit lands without it, exit 0.

## Symptom (Effect)

2026-09-30, session `3e2b9cc8`. A change touched four whole files of mine plus two hunks in
`src/tools/symbol/edit_code.rs`, which another live session had uncommitted hunks in.
`scripts/commit-mine.sh -F msg` printed `commit-mine: leaving staged — not yours:
src/tools/symbol/edit_code.rs (staged by -)` and `committed 4 path(s) of yours; left 1 staged`,
exit 0. The commit (`c64e0c65`) contained the four insert tests and not the `do_insert` guard they
exercise, so that commit was red until the follow-up (`9d2049dd`). Nothing in the output said the
left-behind path was coupled to what was committed, and `-` is called "not yours" although it was.

## Reproduction

Stage one hunk of a file by `git add -p`, or stage a snapshot with `git update-index --cacheinfo`,
then `scripts/commit-mine.sh -m x`. The path is reported `staged by -` and left. Restaging
byte-identical content does not help: it is not an index write, so the hook does not fire again
and the `-` row stays. `git diff --cached` shows exactly the intended hunks throughout.

## Root cause

Measured from `post-index-change-stage-log.sh`, not assumed: it claims a pair for the running
session only during `git add`, `git rm --cached` and `git commit`, and records `-` for a pair first
seen during any other write, deliberately ("Unknown reads as foreign to everyone, so the guard
over-refuses"). `git add -p` applies its hunks through a helper process and `update-index` is not
on the list, so neither is a staging command to the recorder. The over-refusal is intended; the
missing pieces are that `commit-mine` reports an unknown owner as "not yours", and exits 0 without
saying that what it left may be something its commit depends on.

## Fix

Not decided. What worked, recorded because nothing else names it: build a snapshot of the file
(`HEAD` plus only your hunks) in a scratch work tree, link the repo's `scripts/` into it (the
`post-index-change` hook resolves `scripts/` relative to the work tree and is silently skipped
otherwise), and run `GIT_DIR=<repo>/.git GIT_WORK_TREE=<scratch> git add <path>`. That is a real
`git add` of content that never exists on disk, so the recorder claims it, and the peer's bytes in
the shared file are never touched; the documented "entangled single file" construction instead
rewrites the working file for the seconds between its steps.

Candidate changes, for the operator: have `commit-mine` distinguish `-` (unknown) from a named
foreign owner in its message; make it exit non-zero, or warn prominently, when it leaves a staged
path behind while committing others; and describe the scratch-work-tree route in
`docs/conventions/shared-checkout-commit-sequence.md` § *The entangled single file*.

## Severity

Medium. Commits are local here, so the partial commit was repaired before any push, but on a
checkout where committing is publishing, a commit that carries tests without their code is a red
other sessions run.

## Tests added

None.

## Workarounds

The scratch-work-tree `git add` above, then `scripts/commit-mine.sh`. Or wait until the peer commits
the file and stage it whole.

## References

- `docs/conventions/shared-checkout-commit-sequence.md` § *The entangled single file*
- `scripts/commit-mine.sh`, `scripts/post-index-change-stage-log.sh`
