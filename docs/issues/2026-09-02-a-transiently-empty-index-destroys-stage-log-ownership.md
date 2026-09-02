---
id: a25f881ebbae5be2
kind: bug
status: open
title: 'BUG: a transiently-empty index permanently destroys stage-log ownership, so your own paths read as foreign'
tags:
- cluster/shared-resource-carries-no-owner
closed: null
opened: 2026-09-02
owner: marius
related:
- docs/issues/archive/2026-09-02-foreign-index-refusal-names-a-cause-no-route-produces.md
severity: medium
unverified: no fix and no regression test — root cause and minimal trigger are reproduced, but nothing is changed and tests/hooks-discrimination.sh has no case asserting ownership survives an index-emptying operation, which is why the behaviour shipped.
---

# BUG: a transiently-empty index permanently destroys stage-log ownership, so your own paths read as foreign

## Summary

`post-index-change-stage-log.sh` rebuilds the whole log from `git diff --cached --raw` on
every index write. Any operation that transiently empties the staged set — `git stash`, a
`reset`, a failed `pre-commit` run's stash cycle — therefore rewrites the log to **empty**,
and the ownership rows are gone for good. The restore repopulates them with owner `-`,
because carry-over has nothing left to carry. `pre-commit-foreign-index.sh` then refuses the
author's own retry, listing their own paths under `theirs:`.

**This qualifies `IC-17`'s Mechanism status**, which records the git index as an *owned*
resource citing these scripts. It is owned until any index-emptying operation, after which it
silently is not.

## Symptom (Effect)

Reported by a peer (sessionId `953b5e77`) after a `ledger-counts` refusal: `git commit` failed,
and the retry was refused by `foreign-index` listing all ten of their own staged paths under
`theirs:`, with `Staged by: (unrecorded) — not a staging command`.

## Reproduction

Throwaway repo, hook installed from `experiments`, `CLAUDE_CODE_SESSION_ID=SESS-OWNER`:

```
$ git add -- a.txt b.txt c.txt && cat .git/session-stage-log
SESS-OWNER	7898192	a.txt	named
SESS-OWNER	6178079	b.txt	named
SESS-OWNER	f2ad6c7	c.txt	named

$ git stash -q --include-untracked
$ cat .git/session-stage-log          # <- EMPTY. three rows gone.

$ git stash pop -q
$ cat .git/session-stage-log
-	0000000	base.txt	not-staging
```

`git stash` is the minimal trigger and needs no hook failure at all. The peer's route was a
failed `pre-commit`, whose stash/restore cycle does the same thing.

## Environment

`experiments`, shared checkout, hooks installed. Reproduced in an isolated throwaway repo —
never against the shared index.

## Root cause

The log is a **projection of the current index**, not a durable ownership record:

1. `: > "$tmp"` truncates.
2. The `while` loop writes one row per pair returned by `git diff --cached --raw`.
3. `mv -f "$tmp" "$log"` replaces atomically.

Ownership survives only via the carry-over lookup, which reads **the log that step 1 just
discarded a copy of**. So when the staged set is empty, step 2 writes nothing, and the prior
rows are not preserved anywhere. The next write has no prior state to consult.

**Predates the route column** — `99d5acac` introduced the truncate-and-rebuild shape and
`fa9b3aff` refined what a cold log may claim. Neither is wrong about *claiming*: the file's
comment correctly argues that rebuilding from `--raw` must not let the current writer claim
every staged pair, "so claiming all of them for the current writer hands a session its peers'
work". That reasoning is about **attribution**, and it does not require **discarding** rows
for pairs that are no longer staged. The two got coupled because one code path does both.

**What the route column did change is diagnosability**, and that is how this was found: the
row now reads `not-staging` rather than a bare `-`, which is what let the peer see that the
index write came from an unattributable parent rather than from a peer's staging.

Measured 2026-09-02: reproduction above, run against the `experiments` copy of the hook.

## Evidence

### The failure direction is safe, and that is exactly what makes it expensive

`-` is not equal to any real sessionId, so the paths read as foreign and the guard
**over-refuses**. That is the correct direction and the design says so deliberately. But at
the point of use it is **indistinguishable from a real capture**: the author is told their own
ten paths belong to someone else. The printed remedy (`git commit -- <paths>`) works, and its
"none of the staged paths look like yours" line is accurate about the log while being wrong
about the world.

### Why carry-over cannot fix this on its own

Carry-over already works for the case it was built for — a *cold* log, where rows were never
written. It cannot help here because the rows were written and then deleted. Repairing this
means not deleting them: retain rows for pairs absent from the current staged set, with
whatever pruning keeps the file bounded, and keep the claiming rule exactly as it is.

## Hypotheses tried

1. **Hypothesis** — the route column or the `pre-staged` split introduced this.
   **Test** — `git log -S` on the truncate and the `--raw` rebuild.
   **Verdict** — rejected. Both predate today (`99d5acac`, `fa9b3aff`). Today's change made
   the state *legible*, not worse.

2. **Hypothesis** — it needs a hook failure.
   **Test** — bare `git stash` / `git stash pop` in a throwaway repo, no hooks failing.
   **Verdict** — rejected. `git stash` alone destroys the rows. The failed commit is one route
   among several.

## Fix

Not implemented, and not attempted today. Direction: **stop truncating.** Preserve rows whose
`(blob, path)` is absent from the current staged set instead of dropping them, so a
transiently empty index cannot erase ownership; keep the existing rule that the current writer
may only claim pairs argv NAMED.

Two things a fix must not break, both currently load-bearing:

- The claiming rule (`fa9b3aff`) — a cold log must still claim only what argv named.
- Boundedness. The log currently self-limits by tracking only staged pairs. Retention needs a
  prune, and the prune must not be "drop what is not staged", which is the bug.

**Do not fix it by making the guard trust `-` more.** Over-refusing is the correct direction;
the defect is upstream, in what the recorder forgets.

## Tests added

None yet. `tests/hooks-discrimination.sh` § 8 covers the route values but has no case for
ownership surviving an index-emptying operation — which is the assertion this bug wants, and
its absence is why the behaviour shipped. A regression test is a stash/pop cycle asserting the
owner is unchanged.

## Workarounds

`git commit -- <explicit paths>` — the pathspec form ignores the shared index and is what the
refusal already prints. Re-`git add` the paths after any stash/reset to re-establish
ownership before committing.

## Resume

Write the regression case first, in `tests/hooks-discrimination.sh` § 8: stage two paths as
`SESS-A`, `git stash`, `git stash pop`, assert `owner_of` is still `SESS-A`. Confirm it REDs
against the current script before changing anything — per `CLAUDE.md` § *Testing Discipline*,
an observed red against the production path, not the fixture's inputs.

Then change the write loop in `scripts/post-index-change-stage-log.sh` to carry forward
unmatched prior rows. Re-run the full suite (69/69 at time of filing) and re-run mutations M6
and M7, which guard the prose replacement and the diagnostic-only invariant.

## References

- `scripts/post-index-change-stage-log.sh` — the truncate-and-rebuild loop.
- `scripts/pre-commit-foreign-index.sh` — reads the log; correctly over-refuses on `-`.
- `docs/issues/archive/2026-09-02-foreign-index-refusal-names-a-cause-no-route-produces.md` —
  the route column that made this diagnosable (`689ceffb`, `d7eb09c2`).
- `docs/trackers/issue-clusters.md` `IC-17` — whose Mechanism status records the index as
  owned; this is the hole in that claim.
- Observation and the pre-commit route: sessionId `953b5e77`. Reproduction, minimal trigger
  and root cause: this session.

