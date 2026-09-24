---
id: '24ea8fd4e40fb821'
kind: bug
status: open
title: Three hooks-discrimination cases failed once inside a mutation run and did not reproduce in 12 runs
tags:
- cluster/gate-keyed-on-unobservable-event
- testing
- shared-checkout
- flaky
topic: test suite reliability
opened: 2026-09-16
owner: marius
related: []
severity: low
---

# Three `tests/hooks-discrimination.sh` cases failed once and have not reproduced in 12 runs

## Summary

`tests/hooks-discrimination.sh` is the suite this repo uses as **mutation evidence** for the
shared-checkout git hooks. On 2026-09-16 a single run reported three failures in § 2 / § 2b
that are unrelated to the mutation applied and that have not recurred.

Filed at `n=1` and **not** because the observation is strong. It is filed because of what the
suite is *for*: a flaky failure inflates a mutation's kill count, and the same flakiness in
the other direction would mask a **survival** — which reads as "the test discriminates" when
it does not. A suite used to certify other tests has to be trusted differently from one that
only certifies code.

## Symptom (Effect)

One run of `scripts/mutation-probe.sh` against an unrelated script reported:

```
  FAIL  cold log + peer status -> unknown, not the passer-by
  FAIL  unknown reads as foreign -> refuse
  FAIL  refusal names the staged deletion
  FAIL  a self-mentioning file is not its own citer
passed=128 failed=4
```

Only the fourth is attributable to the mutation. The first three exercise
`scripts/post-index-change-stage-log.sh` ownership resolution and cannot be reached by the
mutated file at all.

**Read at face value it said "your change broke three stage-log cases", which was false.**
That is the cost this record exists to name: the natural next action is to go debug a
working change.

## Reproduction

**REPRODUCED 2026-09-18** — 4 failures in 45 runs (~9%), the same three cases every time, on the
main checkout with **no mutation applied and no probe worktree**. Recorded by sessionId
`3aa55c01-9663-44ca-82d2-48b6b8d76d66`.

| runs | environment | result |
|---|---|---|
| 2 | same mutation, same probe worktree | `131/1` — only the expected case |
| 1 | inert mutation (comment reworded), probe worktree | `132/0` |
| 6 | clean, serial, main checkout | `132/0` each |
| 3 | clean, concurrent with each other | `132/0` each |
| 15 | clean, serial, main checkout (2026-09-18) | **1 failure** — `142/3` |
| 30 | clean, serial, main checkout (2026-09-18) | **3 failures** — `142/3` each; runs 1 and 2 CONSECUTIVE |

The inert-mutation run is the one that matters for triage: it isolates the probe's worktree
as an environment and finds nothing, so *"the worktree is different"* is not the explanation.
The 2026-09-18 batches close the other half — neither the mutation nor the worktree is
required. **Two consecutive failures rule out a uniform per-run coin flip**; whatever gates it
persists across at least one run boundary.

The pass counts differ between the two dates (`132` then `145`) because the suite gained cases
in between. The three FAILING cases are the same three.

### Root cause — § 2b's hypothesis is confirmed, and the three failures are ONE cause

The earlier runs recorded only the `FAIL` lines. Capturing full output surfaced the line that
names it:

```
awk: fatal: cannot open file `.git/session-stage-log': No such file or directory
  FAIL  cold log + peer status -> unknown, not the passer-by
        want '-' got ''
```

End to end:

1. The case (`tests/hooks-discrimination.sh:187`) runs `rm -f .git/session-stage-log`, then
   `git status` as peer B, **expecting `post-index-change` to recreate the log**.
2. `post-index-change` fires on index **writes** (`man githooks`, quoted at
   `scripts/post-index-change-stage-log.sh:26`). `git status` rewrites the index only when it
   has stat information to refresh — sometimes it has none. No write, no hook, no log.
3. `owner_of()` (`tests/hooks-discrimination.sh:100`) awks that path with **no existence
   guard**, so it fatals and yields `''` where the case wants `-`. → failure 1.
4. `guard "$B"` then reaches `scripts/pre-commit-foreign-index.sh:153`'s
   `[ -s "$log" ] || exit 0` and exits silently → failures 2 and 3 (`missing: EXIT=1`, and no
   `del.txt` in the refusal).

So these are not three flaky cases. They are **one absent file observed at three points**,
which is why they have only ever failed together. § 2b predicted exactly this shape — *"a hook
that does not fire, or fires late, would produce exactly this shape"* — and called it a
direction to probe rather than a finding. It is now a finding.

**Noted, not asserted:** step 4 means the foreign-index guard **fails open whenever its state
file is absent**. That may well be deliberate — a fresh clone has no log and must not refuse
every commit. Deciding it needs the author's intent and a measurement of how often a real
checkout sits in that state; neither was done here.

## Environment

`experiments`, 2026-09-16. Shared checkout, six live sessions, so the machine was under
concurrent load from peers' builds and test runs at the observed instant. Three concurrent
runs of this suite alone did not reproduce it, which bounds but does not clear that
hypothesis — the load that mattered may not have been this suite.

## Root cause

**Unknown, and deliberately not guessed.** The cluster tag is `cluster/unclassified` for that
reason: assigning a claim-shaped class here would be a hypothesis wearing a taxonomy.

What is known: the three cases share a subject (`.git/session-stage-log` ownership
resolution) and each depends on a `post-index-change` hook firing as a side effect of a git
command. A hook that does not fire, or fires late, would produce exactly this shape — a row
missing from the log, read as *unknown*, resolving differently. That is a direction to
probe, not a finding.

## Fix

**Not designed.** Directions:

1. **Make the three cases assert their own precondition**, per § 7's rule that a case which
   does not establish the log state first can pass — or fail — for entirely the wrong reason.
   If the hypothesis above is right, this converts a confusing failure into a clear one that
   names the missing row.

   **SHIPPED 2026-09-18.** Two changes to `tests/hooks-discrimination.sh`:
   `owner_of()` (and `route_of()`, same shape, not on the observed path) gained an existence
   guard returning the sentinel `NO-LOG`; and a `log_recreated()` assertion now runs between
   the `git status` and the three cases that depend on it.

   Before / after on a real non-firing hook:

   ```
   -- before
   awk: fatal: cannot open file `.git/session-stage-log': No such file or directory
     FAIL  cold log + peer status -> unknown, not the passer-by
           want '-' got ''

   -- after
     FAIL  precondition: peer status recreated the stage log
           post-index-change did not fire on the preceding git command, so
           .git/session-stage-log was never recreated. The cases below read an
           absent file; their failures are downstream of this one, not independent.
     FAIL  cold log + peer status -> unknown, not the passer-by
           want '-' got 'NO-LOG'
   ```

   **THE FLAKE RATE IS UNCHANGED, AND THAT IS NOT A SHORTFALL OF THE FIX — it is what the fix
   was for.** 5 failures in 35 runs after (~14%) against 4 in 45 before (~9%); combined 9/80,
   the same order, and the difference is well inside the noise of two small samples. Nothing in
   a test can make `git status` rewrite the index. What changed is that a failing run now names
   its own cause instead of sending a reader to debug an unrelated change — do not read the
   unchanged rate as the repair having missed.

   Failures per bad run went 3 -> 4, deliberately. The downstream cases are NOT skipped when the
   precondition fails: a skip shrinks the reported case count on exactly the runs where
   something went wrong, which is a capped result presented as complete (`IC-13`). The count
   stays honest and the first failure explains the rest.

   Verified directly rather than by assertion-existence: `owner_of` discriminates all four
   states (absent -> `NO-LOG`, row -> id, no row -> `''`, `-` -> `-`; the first and third both
   answered `''` before, which was the whole defect), `log_recreated` was observed both RED and
   GREEN, `awk: fatal` no longer occurs at all, baseline is 146/0, and the gate is green.
2. **Re-run on failure before reporting**, in the probe rather than in the reader. The
   discriminator that resolved this instance was a second run, and it was reached by
   judgement; a suite that self-re-runs a failing case once and reports *"failed 1 of 2
   attempts"* makes intermittency visible instead of leaving it to be noticed.
3. ~~**Leave it.**~~ **Withdrawn 2026-09-18.** This read *"defensible at this evidence level,
   and the reason this is `zombie` rather than `open`: there is nothing here to work, only
   something to watch for."* Both halves are now false: it reproduces at ~9% on a clean main
   checkout, and the root cause is named above. Direction 1 is the repair — and it was the
   right call before the cause was known, because a case asserting its own precondition would
   have reported *"the hook did not recreate the log"* on the very first failure instead of
   `want '-' got ''`.

## Workarounds

**Re-run before believing a mutation's extra kills.** A mutation that reports failures
outside the code it touched has said something about the suite, not only about the code.
Cheap, and it is what separated the two readings here.

## Resume

`zombie`, which is this repo's status for *recurring-but-unconfirmed* — a "has this come
back?" check rather than a task to pick up. **Do not open it on this evidence.** Do add an
instance here if another run produces failures in cases the change under test cannot reach,
and note the count: at two independent observations this stops being noise and the fix
directions above become worth costing.

Observed while mutation-testing `scripts/pre-commit-orphaned-citations.sh`; the mutation
itself behaved correctly and its verdict was confirmed by the re-runs above.

## Absorbed duplicate — 2026-09-24

`docs/issues/archive/2026-09-08-the-shell-suites-lane-is-flaky-and-ci-endpoint-sampling-misattributes-it.md` described these same three assertions from the CI side and is now `superseded` by this file. Its CI measurement belongs here. Over the Shell suites jobs on `experiments` from 2026-09-01 to 2026-09-24 there were 177 jobs: 166 success, 8 failure, 3 cancelled. **All 8 failures are exactly these three assertions**: 09-04, 09-06, 09-07 ×3, 09-09, 09-11, and 09-14T17:40 (run 34876146168, the latest). After that come 20 successes and 3 cancellations. Only 2 CI runs have landed since `aa831668`, so its precondition message has not yet been seen in CI. Unit and window from the open-bug sweep (`deep-agent-workflow-observations:DWF-7`); runs before 09-01 were not re-examined.
