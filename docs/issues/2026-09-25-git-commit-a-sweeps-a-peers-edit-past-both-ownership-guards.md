---
id: '3c394db3801157d0'
kind: bug
status: open
title: git commit -a sweeps a peer's unstaged edit into your commit, and both ownership guards pass it
tags:
- cluster/gate-keyed-on-unobservable-event
closed: null
opened: 2026-09-25
owner: marius
related:
- docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md
severity: medium
---

# BUG: `git commit -a` sweeps a peer's unstaged edit into your commit, and both ownership guards pass it

## Summary

On this shared checkout, `git commit -a` commits every modified tracked file in the working
tree, a peer's included. **Both pre-commit guards built to stop exactly that exit 0**, and the
commit prints only git's own summary. The cause is not a gap in either guard's logic. Both are
keyed on an index they never see: git hands the hooks `GIT_INDEX_FILE=.git/index.lock` for an
`-a` commit, and each guard skips or misreads that index.

- `scripts/pre-commit-unreviewed-content.sh:94-97` acts only when the hook's index is named
  `next-index-*`, the temporary index a pathspec commit builds, and otherwise exits 0.
- `scripts/pre-commit-foreign-index.sh:268-272` classifies a pair as foreign only when the stage log
  holds a row naming another owner. The stage-log recorder skips any `GIT_INDEX_FILE` other
  than `$git_dir/index` (`scripts/post-index-change-stage-log.sh:102-110`), so the pairs `-a` stages
  are never recorded, and a pair with no row falls through to "mine".

This is the `git commit -a` instance family (Instances 1-3) of
`docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md`, which records
the capture but not why the guards are silent on it. Filed separately because it has its own
mechanism and a cheap, specific fix.

## Symptom (Effect)

Session A edits `a.txt`. Session B edits the tracked file `b.txt` and does not stage it. A runs
`git commit -a -m ...`: `rc=0`, and HEAD contains `a.txt` and **`b.txt`**. B's work is now
published under A's message, and nothing was printed that a reader could have acted on.
`scripts/commit-mine.sh` refuses `-a`, but that protects only sessions using the helper.

## Reproduction

A scratch script in session `e4fbc7ef`'s scratchpad (not retained). It builds a throwaway repo
with copies of `scripts/pre-commit-unreviewed-content.sh`, `scripts/pre-commit-foreign-index.sh`
and `scripts/post-index-change-stage-log.sh` installed as real hooks. Its pre-commit hook echoes
`GIT_INDEX_FILE`. Sessions A and B are synthetic ids passed as `CLAUDE_CODE_SESSION_ID`.

- **Arm 1.** A stages a seed file, which puts one row of A's in the stage log. A edits
  `a.txt`, B edits `b.txt` and does not stage it, and A runs `git commit -a -m sweep`.
- **Arm 2 (control).** The same state, except B **stages** `b.txt`, and A runs a bare
  `git commit -m bare`.

Observed 2026-09-25:

```
ARM 1  hook: GIT_INDEX_FILE=<repo>/.git/index.lock
       rc=0   HEAD files: a.txt b.txt seed.txt
       log rows before: 1   after: 1   (only A's seed row)
ARM 2  hook: GIT_INDEX_FILE=.git/index
       rc=1   Refusing a bare commit: the index holds paths staged by another session.
```

Arm 2 is what makes arm 1's silence a finding rather than a broken fixture: the same guard, in
the same repo layout, is live and refuses when B's pair was recorded. It was reproduced
independently of the batch-A verifier, which found it first with its own scripts.

## Environment

`experiments` @ `8e274b32`, Linux, git as installed on this host. Guards copied from HEAD.

## Root cause

A gate keyed on an event it cannot observe (`IC-2`). Both guards' condition is *"did a session
stage this content, and which one?"*, which the stage log answers only for staging that goes
through `$git_dir/index`. A `commit -a` stages into `index.lock` inside the commit itself, so
the event happens outside the recorder's view, and each guard falls back to a proxy.
Unreviewed-content's proxy is "a pathspec commit is the only kind that commits unstaged
content", and foreign-index's is "no row means mine". Both proxies return a pass, not an error.

## Evidence

The reproduction above, plus the three line ranges cited in § Summary, read at `8e274b32`.

## Hypotheses tried

1. **The guards run on `-a` and judge it correct.** Rejected: unreviewed-content exits at
   `:97` before judging anything, and foreign-index judges an index whose extra pairs have no
   row.
2. **The recorder records `-a`'s staging and the guard mis-attributes it.** Rejected: the log
   holds 1 row before and after the commit.

## Fix

Not implemented. Candidates, neither built:

- **Extend unreviewed-content to `index.lock`.** Any path whose entry in the hook's
  `index.lock` differs from `.git/index` was staged by the commit itself, so its content was
  never reviewed. That is the same comparison the guard already makes for `next-index-*`
  (`:104-109`). A bare commit's hooks see `.git/index` (arm 2), so this does not reach the
  ordinary commit form. `--include` and `--interactive` commits likely take the same route and
  should be probed before relying on it.
- **Make foreign-index refuse, not default, when the hook's index is not `.git/index`.** This
  is weaker: it would also refuse A's own `-a` over files only A touched.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*

## Tests added

None yet. The reproduction's two arms are the shape a regression test in `tests/commit-mine.sh`
(or a sibling suite) would take. Arm 2 is the control that makes arm 1 non-vacuous.

## Workarounds

Never `git commit -a` on this checkout. Stage by path, check the index, read the diff, and
commit by pathspec, or with `scripts/commit-mine.sh` (which refuses `-a`).

## Resume

Build the first candidate in § Fix, with arms 1 and 2 as its test. Probe `--include` and
`--interactive` first, since they may share `index.lock`.

## References

- `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md`, Instances 1-3.
- `docs/conventions/shared-checkout-commit-sequence.md`.
- `docs/issues/archive/2026-09-01-two-correct-pre-commit-guards-have-an-empty-intersection.md`
  (the empty intersection `commit-mine.sh` was built for).
