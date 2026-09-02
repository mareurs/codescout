---
id: 1e5b89d4cf5483f2
kind: bug
status: fixed
title: 'BUG: a transiently-empty index permanently destroys stage-log ownership, so your own paths read as foreign'
tags:
- cluster/shared-resource-carries-no-owner
closed: 2026-09-02
opened: 2026-09-02
owner: marius
related:
- docs/issues/archive/2026-09-02-foreign-index-refusal-names-a-cause-no-route-produces.md
severity: medium
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

Fixed on `experiments` at `5e522fa4`, patch-id
`d5e178bd1b0c2ea6449d77ec75770d5d68a8bd07`. (The SHA is positional and dies when
`experiments` is rebased; the patch-id is a content hash of the diff and survives rebase and
cherry-pick. Cite the pair.)

Implemented 2026-09-02. The write loop still emits one row per currently-staged pair; a
second pass now **carries forward** prior rows whose `(blob, path)` is absent from that set,
capped by `STAGE_LOG_MAX_RETAINED` (default 1000, env-overridable so the suite can reach the
bound). Retained rows are copied verbatim — owner *and* route — and marked with a fifth
tab-separated field, `retained`.

The claiming rule is untouched: retention **preserves** an owner and never assigns one, so a
retained `-` stays `-`.

Two things the implementation had to get right that the plan did not anticipate, both found by
an observed RED rather than by reading:

- **`NR == FNR` is the wrong discriminator here**, and wrong *only* in the case this fix
  exists for. `$tmp` is empty exactly when the index is transiently empty, and an empty first
  file makes `NR == FNR` true for the second file's first record — which would file the prior
  log as the staged set and retain nothing, silently, in the stash case alone. Keyed on
  `FILENAME` instead.
- **The log runs newest-first, not oldest-first.** Each pass writes the staged block and
  appends retention after it, so the head of the retained block is the recently-unstaged.
  Keeping the tail satisfies the cap while evicting exactly the row a stash/pop is about to
  ask for; a six-cycle trace showed the newest row dropped. The cap keeps the head.

**A regression the fix introduced, and the invariant that constrained the repair.** Retention
makes the carry-over lookup visible to pairs that *left* the index, and carry-over runs before
the claiming branch — so a retained row suppressed a fresh explicit claim by another session.
Caught by § 6's `a deletion patch (+++ /dev/null) is claimed`, which went from green to red.
The blanket repair — let a `names_path` claim beat carry-over — **breaks § 2's headline
invariant**, because one `git add -- unchanged.txt changed.txt` names a pair it does not alter
and the namer would steal it from the stager. So the override is scoped to *retained* rows,
which is what the fifth field is for: a retained row is a claim about the past, and a staging
op that explicitly names the path is what is putting the pair back.

`scripts/pre-commit-foreign-index.sh`'s MECHANISM comment documented the row as three fields
(`<owner>\t<blob>\t<path>`) and had already gone stale when the route column landed; updated
to the current five and to say the guard ignores the marker.
## Tests added

`tests/hooks-discrimination.sh` § 9, five cases. Confirmed RED against the unmodified script
first (6 failures, 73 pre-existing passes), then green at 79/79.

- ownership survives **while the index is empty** — the root cause, not merely the round trip
- ownership survives a full stash/pop cycle, for both staged paths
- the **route** survives with the owner; a row restored as `not-staging` would still read as
  unattributable to the guard, so restoring the owner alone is no repair
- retention does not invent an owner for a pair the claiming rule left unowned
- the cap is honoured **and** evicts the oldest, keeping the newest

**The stasher is a different session from the stager, and that is the fixture's load-bearing
detail.** With one session doing both, a "fix" that simply let the restoring writer claim every
staged pair would satisfy the owner assertion while destroying the claiming rule — the
assertion is monotone under that mutation. Splitting the sessions makes it RED.

The cap assertion carries a lower bound as well as an upper one. `rows <= 4` alone passed
vacuously at **0 rows** against the unfixed script — an assertion that cannot fail, which is
`cluster/assertion-that-cannot-fail` in the test written to close a different class.
## Workarounds

`git commit -- <explicit paths>` — the pathspec form ignores the shared index and is what the
refusal already prints.

**Re-`git add`ing the paths does NOT re-establish ownership, and this file said it did.**
Falsified 2026-09-02 by direct experiment: `git add` of already-staged, byte-identical content
is not an index write, so `post-index-change` never fires and the row stays `-` /
`not-staging`. Only a *content change* triggers the hook and reclaims the pair — verified by
appending a byte to a staged file, after which the row came back as the author's. That is why
the peer's retry was refused: there was no route back short of modifying their own work.

The wrong workaround is worth recording rather than deleting. It is plausible, it is what any
reader would try first, and it fails **silently** — the log is simply unchanged, with no error
to read.
## Resume

Fixed; no resume owed. The regression cases and the retention pass are in place and the suite
is green at 79/79.

What is deliberately **not** addressed here, and stays with `IC-17`: retention gives the git
index a durable ownership record, but the working tree's *unstaged* state still has none, and
that gap has no adjacent git primitive to extend. See the class's `Mechanism status`.
## References

- `scripts/post-index-change-stage-log.sh` — the truncate-and-rebuild loop.
- `scripts/pre-commit-foreign-index.sh` — reads the log; correctly over-refuses on `-`.
- `docs/issues/archive/2026-09-02-foreign-index-refusal-names-a-cause-no-route-produces.md` —
  the route column that made this diagnosable (`689ceffb`, `d7eb09c2`).
- `docs/trackers/issue-clusters.md` `IC-17` — whose Mechanism status records the index as
  owned; this is the hole in that claim.
- Observation and the pre-commit route: sessionId `953b5e77`. Reproduction, minimal trigger
  and root cause: this session.
