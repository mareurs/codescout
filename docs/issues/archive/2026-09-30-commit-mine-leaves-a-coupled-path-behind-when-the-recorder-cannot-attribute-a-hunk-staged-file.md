---
id: 43a50b8d8af976be
kind: bug
status: fixed
title: 'BUG: commit-mine exits 0 and leaves a coupled path behind when the stage-log recorder cannot attribute a hunk-staged file'
owners:
- marius
tags:
- cluster/authorship-unrecoverable-after-the-fact
- commit-hooks
- shared-checkout
closed: 2026-09-30
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

**Reproduced again 2026-09-30 in a throwaway repository** that copies this repo's hooks and scripts
(nothing on the shared checkout): session A stages one whole file with `git add` and a snapshot of a
second with `git update-index --cacheinfo`, while a peer's hunk sits in that file's working copy. The
stage log reads `-  <blob>  shared.txt  pre-staged`; `commit-mine` prints `leaving staged — not
yours: shared.txt (staged by -)`, commits only the whole file, closes with `left 1 staged for their
owners`, exit 0. In the real log, 19 of 1003 rows are `-`, 17 of them route `not-staging`.

## Root cause

Measured from `post-index-change-stage-log.sh`, not assumed: it claims a pair for the running
session only during `git add`, `git rm --cached` and `git commit`, and records `-` for a pair first
seen during any other write, deliberately ("Unknown reads as foreign to everyone, so the guard
over-refuses"). `git add -p` applies its hunks through a helper process and `update-index` is not
on the list, so neither is a staging command to the recorder. The over-refusal is intended; the
missing pieces are that `commit-mine` reports an unknown owner as "not yours", and exits 0 without
saying that what it left may be something its commit depends on.

## Fix

Chosen 2026-09-30, from three candidates. `commit-mine` now tells an unattributed `-` leftover
apart from a named peer's, the way `pre-commit-foreign-index.sh` already does for the same rows
("unrecorded", "frequently a peer's", and deliberately not ackable): a separate `UNATTRIBUTED`
block saying the path can be the caller's own hunk, that the commit does not contain it, and the
three answers the caller can act on (stage it so the recorder attributes it, leave it, or ask);
and a closing line that counts named and unattributed leftovers separately, with "for their
owners" said only where an owner is named.

**Not chosen: a non-zero exit, or a refusal, when a `-` path is left.** The guard refuses to let
`-` be acked, since an ack naming nobody covers an unbounded set, so a refusal with no ack would
let any peer's `-` leftover block every other session's `commit-mine`, and an ack would
contradict that decision. The exit code stays 0, which `tests/commit-mine.sh` F1 already pins for
a named peer.

The third candidate, documenting the route that makes a hunk attributable, is in
`docs/conventions/shared-checkout-commit-sequence.md` § *The entangled single file*. Verified in a
throwaway repository before it was written down: a real `git add` of HEAD-plus-your-hunks against a
scratch work tree is logged `named`, `commit-mine` takes it, the committed file holds only the
caller's hunk, and the peer's bytes in the working file are untouched.

## Severity

Medium. Commits are local here, so the partial commit was repaired before any push, but on a
checkout where committing is publishing, a commit that carries tests without their code is a red
other sessions run.

## Tests added

In `tests/commit-mine.sh`: F10 (only an unattributed leftover), F11 (a named and an unattributed
leftover in one run, counted apart) and F12 (nothing left behind: the ordinary closing line is
unchanged). F10's fixture asserts its row really is `-`, so it cannot pass against a path the
recorder attributed after all. Red before the change (8 failures), 53 passed and 0 failed after.
One mutation per site on the final bytes, read off the suite's summary against that baseline
because `mutation-probe.sh` gives no verdict for a non-cargo runner: the `-` split deleted is
45/8; the closing line's unattributed tail deleted is 51/2; the "for their owners" clause made
unconditional is 52/1, killed by F12 alone; the remedy pointer deleted is 52/1. CI runs this suite
as its own job; the local four-command gate does not.

## Fix provenance

- **SHA:** `1f033e975ab4ce9dfc6168f2c3b77c686fe7c63a` (`experiments`)
- **patch-id:** `a44d7c2bd112f3d35688e60d87e5e96ff8b65c41` (`git show <sha> | git patch-id --stable`)

Verified on `experiments` 2026-09-30 by running `tests/commit-mine.sh` directly (53 passed, 0
failed) and the four mutations above. The local four-command gate does not run this suite, so its
green is silent about this change; CI's own job for it is the lane that does. The `commit-mine`
the live sessions run is the script on disk, so the new message is in effect for the next commit.

## Workarounds

The scratch-work-tree `git add` above, then `scripts/commit-mine.sh`. Or wait until the peer commits
the file and stage it whole.

## References

- `docs/conventions/shared-checkout-commit-sequence.md` § *The entangled single file*
- `scripts/commit-mine.sh`, `scripts/post-index-change-stage-log.sh`
