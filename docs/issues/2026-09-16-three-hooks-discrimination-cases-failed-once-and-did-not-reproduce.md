---
id: '24ea8fd4e40fb821'
kind: bug
status: zombie
title: Three hooks-discrimination cases failed once inside a mutation run and did not reproduce in 12 runs
tags:
- cluster/unclassified
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

**None.** Not reproduced in 12 subsequent runs:

| runs | environment | result |
|---|---|---|
| 2 | same mutation, same probe worktree | `131/1` — only the expected case |
| 1 | inert mutation (comment reworded), probe worktree | `132/0` |
| 6 | clean, serial, main checkout | `132/0` each |
| 3 | clean, concurrent with each other | `132/0` each |

The inert-mutation run is the one that matters for triage: it isolates the probe's worktree
as an environment and finds nothing, so *"the worktree is different"* is not the explanation.

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
2. **Re-run on failure before reporting**, in the probe rather than in the reader. The
   discriminator that resolved this instance was a second run, and it was reached by
   judgement; a suite that self-re-runs a failing case once and reports *"failed 1 of 2
   attempts"* makes intermittency visible instead of leaving it to be noticed.
3. **Leave it.** Defensible at this evidence level, and the reason this is `zombie` rather
   than `open`: there is nothing here to work, only something to watch for.

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
