---
kind: bug
status: fixed
tags:
- cluster/guard-narrower-than-its-name
closed: 2026-09-16
opened: 2026-09-15
owner: marius
related: []
severity: high
---

# Background-log eviction unlinks the log of a job whose liveness is explicitly UNKNOWN

## Summary

`JobState::is_running()` — as the predicate was named and written at filing time — is
`matches!(self, JobState::Running)`. Its own doc comment, two lines above, states the
property it is supposed to answer:

> Whether the job **may still be writing** to its log. Used by eviction, which **must not
> unlink a file a live process still holds open**.

`JobState::Failed` does not satisfy `matches!(self, Running)`, so `is_running()` returns
`false` for it — and eviction treats that as "safe to unlink". But `Failed` is the state
that means *liveness is unknown*: its own renderer (`:150`) prints
`format!("unobservable: {error}")`.

The conservative answer for an unobservable job is `true`. The code returns `false`, so
the one state where the invariant cannot be established is the state where it is assumed.

## Symptom (Effect)

A long background command is running. Its supervisor's `child.wait()` returns `Err`
(EINTR, ECHILD, reaped elsewhere), so the state becomes `JobState::Failed`. **The process
is still alive and still writing to its log.**

On the next background spawn past the retention limit, `store_background`'s victim search
(`:690-696`) picks the first job where `!j.state.is_running()` — which now matches this
one — and `std::fs::remove_file(&job.log_path)` runs at `:702`. The live process keeps the
unlinked inode open and writes into a file no path reaches. The handle then reports the log
as unavailable.

**The failure is silent in the direction that matters.** `remove_file`'s own error is
logged at `tracing::debug!` (`:703`), and the unlink here *succeeds* — there is no error to
log. Nothing tells the caller their output went to an unreachable inode.

## Reproduction

**Reproduced deterministically — and the earlier "not reproduced live" note was answering
the wrong question.** Reaching `JobState::Failed` through a real `wait()` failure is indeed
not available on demand. But nothing about the defect needs it: the defect is in eviction's
*classification* of that state, so seeding the state directly and tripping eviction
reproduces the consequence exactly, with no runtime raciness at all.

1. Store `TEST_MAX_PENDING` jobs whose state is `JobState::Failed`.
2. Store one more, tripping the eviction branch.
3. Assert the oldest job's `log_path` still exists.

Observed red on 2026-09-16 against the unmodified production path:

```
thread '...eviction_never_unlinks_the_log_of_an_unobservable_job' panicked at
src/tools/output_buffer.rs:2041:9:
an unobservable job's log must survive eviction of its handle
```

**The generalisation worth keeping, because it changes what gets filed:** a
state-classification defect is reproducible through its **classifier**, even when the
state's natural producer is unreachable on demand. Reasoning from the bytes was right
about the defect and wrong about the reproduction, and the cost of that reading is the
regression test — a bug filed as unreproducible does not get one.
## Root cause

`is_running()` answers a **two**-state question over a **four**-state enum, and the two
states it does not name are assigned to the branch that is unsafe for one of them:

| `JobState` | may still be writing? | `is_running()` | safe to unlink? |
|---|---|---|---|
| `Running` | yes | `true` | no — correctly retained |
| `Exited { code: Some }` | no | `false` | yes — correct |
| `Exited { code: None }` | no | `false` | yes — correct |
| **`Failed { error }`** | **UNKNOWN** | **`false`** | **unestablished — unlinked anyway** |

The predicate is not wrong about the three states it was written for. It has no branch for
the fourth, and the default it falls into is destructive.

## Why the class is `cluster/guard-narrower-than-its-name`

The doc comment names the property — *may still be writing* — and everything downstream
reasons with that name. The implementation covers a **subset** of it (`Running` only). The
uncovered remainder is exactly the state whose name (`unobservable`) says the property
cannot be established, and it is protected by nothing. That is IC-14's claim without
modification.

Deliberately **not** `cluster/record-asserts-an-unchecked-completion` (IC-8), which was
weighed: `Failed` does assert a completion nothing re-checked, but the defect here is not
that the record goes unverified — it is that a named property covers less than its name.
The IC-8 reading would send a fixer toward re-checking job state; the IC-14 reading sends
them to the predicate, which is where the three-line fix is.

## Fix

**FIXED 2026-09-16** in `82d0f58e` — patch-id `f7cdad388c3c1dd81945b1604703fcf5f39116e4`.

Applied 2026-09-16. Two changes, and the second is not cosmetic.

**1. The predicate answers its documented question, as an exhaustive match:**

```rust
pub fn may_still_be_writing(&self) -> bool {
    match self {
        JobState::Running | JobState::Failed { .. } => true,
        JobState::Exited { .. } => false,
    }
}
```

**2. Renamed from `is_running`, which was half the defect rather than a detail.** Fixing
only the behaviour ships a method named `is_running` that returns `true` for a job that is
not running — the same class with its polarity reversed, and a live trap for the next
caller who asks the *name's* question instead of the doc comment's. The name is what made
the original mistake available: classifying `Failed` under a predicate called
`is_running` invites the answer for *"not running"* where the site needs the answer for
*"may still be writing"*.

**The rename was safe to make, and that is a measured fact rather than a judgement.**
`references` reports exactly **one** call site — the eviction victim search — and the old
name occurred nowhere else in the tree outside markdown prose. **Enumerating the callers is
the step this file originally skipped, and it is what decides between the narrow fix and
the rename:** at one call site the rename costs two lines, and this file's own "narrowest
change" framing was written without that number in hand.

`store_background` needed no change, which is the other thing the caller enumeration
bought. Its `None` branch — *every retained job is live, so drop the oldest HANDLE and
leave the file on disk* — is already the correct landing place for a retained `Failed`
job. The fix therefore converts an unlink into an existing, deliberate degradation rather
than introducing a new path for the unobservable case.
## Tests added

Two, in `src/tools/output_buffer.rs`, both asserting the **consequence** (a file still on
disk) rather than the predicate — as this file demanded before either was written:

- `eviction_never_unlinks_the_log_of_an_unobservable_job` — fills the buffer with `Failed`
  jobs, trips eviction, asserts the evicted handle's log survives.
- `eviction_takes_a_newer_exited_job_over_older_unobservable_ones` — **exists because the
  first is monotone under "stop unlinking anything, ever"**, a fix that would leak one log
  file per background command while satisfying it completely. Nineteen `Failed` jobs sit in
  front of the single `Exited` one in the queue, so the victim search must skip past them
  and still reclaim it.

**Both were observed red against unmodified production code, for their own distinct
reasons** — the first on the stranded inode, the second on the victim search taking a
`Failed` job — and green after. `bg_job_failed` carries a fixture annotation naming what
breaks if its state is changed, since an `Exited` fixture would route both tests through
the already-correct branch and they would pass without discriminating.

**No mutation run, deliberately: the pre-fix production code WAS the mutation for this
site, and both tests killed it.** That is the evidence a probe run would be trying to
manufacture, obtained from the real path rather than a synthetic one. The third arm
(`Exited => false`) is pinned by the pre-existing
`eviction_prefers_a_terminated_job_over_an_older_running_one`, which reds if eviction ever
stops reclaiming an observed exit.
## Provenance

Found by `/code-review` on 2026-09-15 while it was pointed at the wrong target — it was
given PR #20 and reviewed the local working tree instead
(`embedder-stack-ops-session-log:F-3`). The finding was real; the review was not of the
thing it was asked about. Confirmed at the bytes on 2026-09-15 after the code had landed,
by sessionId `f5f48b42-6d84-482e-84a4-8eaebb0ce60f`.

## References

- `src/tools/output_buffer.rs:157` — `may_still_be_writing`: the predicate and the doc
  comment that always named the right property
- `src/tools/output_buffer.rs:170` — `Failed` renders as `unobservable: {error}`, the
  state's own admission that its liveness was never established
- `src/tools/output_buffer.rs:698` — `store_background`: the victim search, the unlink,
  and the `None` branch that a retained unobservable job now lands in
- `src/tools/run_command/inner.rs:111` — where the state is set, and where the child is
  **not** killed. That is what makes the stranded inode a live hazard rather than a
  theoretical one: nothing on that path stops the process writing
- [`docs/issues/archive/2026-09-15-background-log-eviction-deletes-a-running-jobs-log.md`](archive/2026-09-15-background-log-eviction-deletes-a-running-jobs-log.md)
  — the **absence** of any liveness check, which adding this call fixed. This file was the
  residual hole that fix left open, not a re-file of it
