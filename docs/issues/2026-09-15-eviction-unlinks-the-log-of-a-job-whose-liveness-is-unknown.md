---
status: open
opened: 2026-09-15
closed:
severity: high
owner: marius
related: []
tags: [cluster/guard-narrower-than-its-name]
kind: bug
---

# Background-log eviction unlinks the log of a job whose liveness is explicitly UNKNOWN

## Summary

`JobState::is_running()` (`src/tools/output_buffer.rs:140`) is
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

Not reproduced live — `JobState::Failed` requires `wait()` to fail, which is not reachable
on demand. **Stated as unreproduced rather than asserted**: the reasoning below is from the
bytes, and the defect is a state-classification error visible in three lines that do not
need a runtime to read.

1. Background a long-running command.
2. Force its supervisor's `wait()` to error (the state transition, not the process exit).
3. Spawn background commands until the retention limit evicts.
4. Read the first job's `@bg_*` handle.

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

Not applied. The narrowest change is to make the predicate answer its documented question:

```rust
pub fn is_running(&self) -> bool {
    !matches!(self, JobState::Exited { .. })
}
```

which retains `Failed` and keeps the two `Exited` arms evictable, or equivalently an
explicit `Running | Failed { .. } => true`. The second form is preferable: it makes the
`Failed` decision visible at the site rather than implied by a negation, so a fifth variant
added later must be classified rather than silently inheriting a default.

## Tests added

None yet. A regression test must assert the **consequence**, not the predicate: seed a job
in `Failed`, force an eviction, and assert its `log_path` still exists. A test asserting
`is_running(&Failed) == true` is monotone under a re-introduced default arm and would not
discriminate.

## Provenance

Found by `/code-review` on 2026-09-15 while it was pointed at the wrong target — it was
given PR #20 and reviewed the local working tree instead
(`embedder-stack-ops-session-log:F-3`). The finding was real; the review was not of the
thing it was asked about. Confirmed at the bytes on 2026-09-15 after the code had landed,
by sessionId `f5f48b42-6d84-482e-84a4-8eaebb0ce60f`.

## References

- `src/tools/output_buffer.rs:138-142` — the doc comment and the predicate
- `src/tools/output_buffer.rs:150` — `Failed` renders as `unobservable: {error}`
- `src/tools/output_buffer.rs:690-706` — the victim search and the unlink
- `docs/issues/2026-09-15-background-log-eviction-deletes-a-running-jobs-log.md` — the
  **absence** of any liveness check, which the `is_running` call fixed. This file is the
  residual hole that fix leaves open, not a re-file of it
