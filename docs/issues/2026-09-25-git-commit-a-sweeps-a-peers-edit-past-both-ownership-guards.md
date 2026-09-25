---
id: '3c394db3801157d0'
kind: bug
status: taken
title: git commit -a sweeps a peer's unstaged edit into your commit, and both ownership guards pass it
tags:
- cluster/gate-keyed-on-unobservable-event
claimed_at: 2026-09-25
claimed_by: e4fbc7ef-27b7-4707-8469-ccdffa8e4e92
closed: null
opened: 2026-09-25
owner: marius
related:
- docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md
severity: medium
unverified: '`git commit -p` / `--interactive` were not probed: they need a TTY, and a scripted `script(1)` probe hung on the prompt. They likely take `index.lock` (the -a/-i route), which the guard now examines, but that is unmeasured.'
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

**Built 2026-09-25: the first candidate.** `scripts/pre-commit-unreviewed-content.sh` now examines `index.lock`
as well as `next-index-*`, with the same per-path blob comparison against `.git/index`. Its refusal is
form-aware, and the `-a` branch's remedy deliberately does **not** print `git add <the list>` as the pathspec
branch does: for `-a` the list is every path the commit would sweep, so that line would stage a peer's file for
them.

Which index each form hands the hook was measured on git 2.55 before the change, by echoing `GIT_INDEX_FILE` from
a real hook:

| form | hook's index | examined |
|---|---|---|
| bare `commit`, `--amend` | `.git/index` | no, and inert if examined: an index compared with itself finds nothing |
| `-a`, `-i <paths>`, `--amend -a` | `index.lock` | **yes, since this fix** |
| `-o <paths>`, `-- <paths>` | `next-index-<pid>.lock` | yes, as before |

**Foreign-index is unchanged, deliberately.** The second candidate would also refuse A's own `-a` over files only
A touched. Now that unreviewed-content refuses any `-a` that sweeps unstaged content, it would add nothing.

`scripts/pre-commit-run.sh`'s comment on which index each shape uses was corrected in the same commit. Its run
label (*refuse a pathspec commit carrying unstaged content*) was kept verbatim, because about ten records quote
it as a name.

Fix SHA: *(recorded when archived)*
Patch-id: *(recorded when archived)*

## Tests added

`tests/hooks-discrimination.sh` § 4b (the section heading is "== -a / -i commits (index.lock)"). It has 12
assertions, driven through REAL commits and a pre-commit shim, because the defect was the guard not recognising the
index name git actually uses. A hand-copied `index.lock` would pass whether or not git still used that name. Suite
at 163/0.

Each bound was mutated with `scripts/mutation-probe.sh`, reading the suite's own `passed=/failed=` line:

| mutation | killed by |
|---|---|
| M1: delete the `index.lock` arm (the fix reverted; the observed red) | 6 cases: the `-a` sweep refusal, its file name, HEAD unchanged, the peer warning, and the `-i` refusal and its file name |
| M2: restore the pathspec remedy in the `-a` branch | `-a remedy does not tell you to stage the swept list` |
| M3: drop the blob comparison, refusing every temporary-index commit | the fully-staged control (2 assertions) |
| M4: `*) exit 0` to examine every index | `no GIT_INDEX_FILE -> silent` |

**M4 first SURVIVED.** The bare-commit case alone cannot kill it, because git hands a bare commit `.git/index`, and
an index compared with itself finds nothing. The unset-variable case was added to kill it, and the bare case's
comment now says it guards behaviour, not that arm. All four were re-run after that edit.

## Workarounds

Never `git commit -a` on this checkout. Stage by path, check the index, read the diff, and
commit by pathspec, or with `scripts/commit-mine.sh` (which refuses `-a`).

## Resume

Archive once the gate is green, recording the fix SHA and patch-id, and repoint the citations of this path: the
guard's header and refusal text, the test comment, and `e421be689a23ae2a`'s re-verification note.

## References

- `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md`, Instances 1-3.
- `docs/conventions/shared-checkout-commit-sequence.md`.
- `docs/issues/archive/2026-09-01-two-correct-pre-commit-guards-have-an-empty-intersection.md`
  (the empty intersection `commit-mine.sh` was built for).
