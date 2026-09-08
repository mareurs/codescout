---
status: open
opened: 2026-09-08
closed:
severity: medium
owner: marius
related:
  - docs/issues/archive/2026-09-01-an-absent-stage-log-makes-the-foreign-index-guard-pass.md
  - docs/issues/archive/2026-09-01-foreign-index-guard-passed-a-peers-staged-deletion.md
kind: bug
tags:
  - cluster/repro-env-diverges-from-gate-env
---

# BUG: the `Shell suites` CI lane is intermittently red at ~25% on one three-assertion block, and CI's two-endpoint sampling makes that read as a regression by whoever pushed last

## Summary

`tests/hooks-discrimination.sh` § *stager wins* fails in CI on three assertions that pass
locally, intermittently, with the file and everything it depends on unchanged:

```
FAIL  cold log + peer status -> unknown, not the passer-by
FAIL  unknown reads as foreign -> refuse
FAIL  refusal names the staged deletion
```

They are one property, not three: the block `rm -f .git/session-stage-log`, has a **passer-by**
run `git status`, and asserts the log stays cold (`owner_of s1.txt` = `-`). When that first
assertion goes the other way the next two follow mechanically — a claimed owner is not foreign,
so the guard does not refuse, so the refusal names nothing.

**Measured 2026-09-08 over the last 12 `experiments` runs: 3 failures, 9 successes (~25%).**
Red at `c7db63eb` (09-07T15:04), `9a1a3905` (07:32) and `5a44c347` (05:42); green at the other
nine, including `ae7fe563` (08:20) which sits *between* two of the reds.

## Symptom (Effect)

**The expensive symptom is not the red lane — it is who the red gets attributed to.** CI
`cancel-in-progress` means only a few commits per hour get a completed run, so the observed
history is a sparse sample of a dense branch. Between the last green (`ae7fe563`) and the red
(`c7db63eb`) sat **16 commits from 4 sessions**. A `success → failure` transition across two
sampled endpoints reads as *"the tip broke it"*, and the tip is whoever pushed last.

That reading was made in this session and held until the distribution was pulled: the pushing
session's commit touches **no shell file at all**, and two of the three reds predate it.

## Reproduction

Not reproducible locally. `bash tests/hooks-discrimination.sh` → **91 passed, 0 failed**, three
consecutive runs, at `9c82bda4` with the same scripts CI used.

In CI it needs no reproduction step — re-run the lane until it flips.

## Environment

- Local: git 2.55.0, passes 3/3.
- CI: `ubuntu-24.04`, image `20260831.293.1` — **identical image on the green run and the red
  one**, so a runner-image roll is refuted, not merely unlikely.

## Root cause

**Unknown.** What is ruled out, each checked rather than assumed:

1. **A change to the test.** `git log ae7fe563..HEAD -- tests/hooks-discrimination.sh` is
   **empty**. Nobody touched it.
2. **A change to what it exercises.** Only one file under `scripts/` changed in the range —
   `scripts/install-hooks.sh` at `3182c61c` — and this test does not use it: `new_repo()` writes
   its own `post-index-change` shim (`:84-88`) pointing straight at
   `scripts/post-index-change-stage-log.sh`.
3. **A runner-image roll.** Same image version on both runs.
4. **The pushing session's commit.** `c7db63eb` touches `docs/**`, `src/retrieval/client.rs` and
   `tests/hook_config.rs`, all doc-comment or prose edits, and no shell file.

The remaining candidate is a **timing dependence** in the property itself. `post-index-change`
fires on *every* index write, and the assertion is that a specific one (a passer-by's
`git status`) does **not** claim. A loaded runner executing a 15-job matrix is exactly the
condition under which an ordering assumption stops holding — but this is a **hypothesis, not a
finding**, and no measurement in this file supports it yet.

## Evidence

`Shell suites` conclusion, last 12 `experiments` runs, read from `conclusion` and never `status`:

| when | commit | shell lane |
|---|---|---|
| 09-07T15:04 | `c7db63eb` | **failure** |
| 09-07T08:20 | `ae7fe563` | success |
| 09-07T07:32 | `9a1a3905` | **failure** |
| 09-07T07:25 | `31ae291a` | success |
| 09-07T07:19 | `e766233f` | success |
| 09-07T06:37 | `4b30601c` | success |
| 09-07T05:53 | `d5b20fbb` | success |
| 09-07T05:49 | `f15aa586` | success |
| 09-07T05:42 | `5a44c347` | **failure** |
| 09-07T05:41 | `e4a01763` | success |
| 09-07T05:40 | `491ed828` | success |
| 09-07T05:06 | `b678e0f3` | success |

## Hypotheses tried

- *"The tip broke it."* **Refuted** by the two earlier reds and by the tip's diff.
- *"A peer's `install-hooks.sh` change broke it."* **Refuted** — the test installs its own shim.
- *"The runner image rolled."* **Refuted** — byte-identical image version.

## Fix

None. Filed for the distribution and the attribution hazard, which are worth more than a fix
attempt against an unreproduced timing bug.

**The cheap first move is not a fix at all:** `Shell suites` should be re-run on a known-green
commit a few times to establish the rate independently of the branch, which separates "flaky
test" from "something in the tree makes it flaky". Nothing in this file distinguishes those.

## Tests added

None. Note the shape: this is a test that **fails intermittently while asserting a real
property**, so muting or looping it would trade a visible flake for an invisible hole — the
property (`the stager wins, not the observer`) is the production failure its own comment
records, and the archived bugs in `related:` are what it was written for.

## Workarounds

Read `conclusion` per *job*, not per run, and pull the **distribution** before attributing a
red to a commit. On this branch `cancel-in-progress` makes completed runs sparse, so two
adjacent samples can span a dozen commits from several sessions.

## References

- `tests/hooks-discrimination.sh:158-165` (the three assertions), `:76-88` (`new_repo`'s own shim)
- `scripts/post-index-change-stage-log.sh`, `scripts/file-provenance.py` (`owner_of`)
- `docs/plans/2026-09-06-stale-ledger-and-shared-state-fix-queue.md` § 3c — the CI-signal
  section this instantiates: *read `conclusion`, never `status`*, and the aggregate-vs-distribution point
- CLAUDE.md § *Testing Discipline* — a count of a defect population must arrive with its unit
