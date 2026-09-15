---
id: d44b0a9aa38738f5
kind: bug
status: fixed
title: BackgroundKillGuard holds a raw pid across drop(child), and its SAFETY comment states the wrong worst case
owners:
- marius
tags:
- run-command
- cancellation
- cluster/addressing-without-an-escape-hatch
closed: 2026-09-15
opened: 2026-09-15
severity: low
---

# BUG: BackgroundKillGuard holds a raw pid across drop(child), and its SAFETY comment states the wrong worst case

## Summary

`spawn_background_command` (`src/tools/run_command/inner.rs:89-144`) captures
`child.id()`, then **drops the `Child`**, then arms a `BackgroundKillGuard` holding
only that raw pid for a 5-second cancellation window. Dropping the tokio `Child`
releases the process to tokio's orphan reaper, so the pid can be reaped and recycled
by the OS while the guard is still armed against it.

The guard's `SAFETY` comment (`:43-45`) reads: *"Worst case the PID was reaped and we
kill nothing (ESRCH), which is a no-op."* That enumerates reaped-and-gone but omits
reaped-and-reused. In the reuse case the `libc::kill(pid, SIGKILL)` at `:47` targets an
unrelated process.

## Symptom (Effect)

If a `run_in_background` call is cancelled within its 5-second warm-up **and** the
spawned command already exited **and** the pid has been recycled, codescout SIGKILLs a
process it never spawned. On a shared checkout that plausibly means a peer session's
`cargo` or `rust-analyzer`.

No occurrence has been observed. The defect recorded here with confidence is the
**incorrect worst-case reasoning in the SAFETY comment**, which is what would stop the
next reader from noticing the gap.

## Reproduction

**Not reproduced.** Requires pid wraparound inside a 5-second window, which is not
reliably forceable at default `pid_max`. A deterministic unit-level reproduction would
need the kill target to be injectable so a test can assert the guard refuses to fire
on a pid it can no longer prove it owns.

## Environment

Read at worktree HEAD `bb6a2cb1`, branch `experiments`, 2026-09-15. Unix arm; the
Windows arm at `:49-56` calls `crate::platform::terminate_process(pid)` at `:55` and
has the same shape.

## Root cause

A pid is an address with no disambiguator across time: two processes share one token,
and nothing in the retained value distinguishes them. The code holds the pid precisely
across the interval where that ambiguity becomes reachable — after `drop(child)`
surrenders the handle that *would* have been unambiguous.

The ordering is deliberate (`drop(child)` is what detaches the job) so this is a real
tension, not an oversight in sequencing: retaining the `Child` is exactly what the
slice-1 supervisor proposal would do.

## Evidence

`src/tools/run_command/inner.rs:31-59` (`BackgroundKillGuard` + its `Drop`),
`:114-119` (`child.id()`, `drop(child)`, guard armed), `:121` (5s window).

## Hypotheses tried

None — found by reading during the architecture-boundary slice-1 design review.

## Fix

**FIXED 2026-09-15** in `f098069a` — patch-id `8658a129d49444e33a6fce955a2f8b498bb743d1`.

Fixed by **deletion**, and as a side effect rather than deliberately. Slice 1 needed the `Child` retained to get at the exit status at all, so a supervisor task now owns it for the job's whole life. With nothing dropped there is no detached interval, no 5s warm-up to cancel inside, and no reason to hold a bare pid — `BackgroundKillGuard` and its `Drop` impl are gone entirely, along with the SAFETY comment whose worst case was wrong.

This is the third remedy available to `IC-6` after *escape* and *disambiguator*: **stop using the ambiguous address.**

**Behavior change this carries, recorded because it is a real loss and not only a removal:** cancelling a `run_in_background` call within its first 5 seconds no longer SIGKILLs the child. Backgrounding now means the job runs. That is defensible — it is what the caller asked for — but it is a change, not a no-op.
## Tests added

**None, and that is a gap rather than an oversight — read it before archiving.**

The remedy was deleting the code, so there is nothing left to assert against and no mutation that could produce an observed RED. This file is therefore archived on a weaker basis than its siblings, which carry killed mutations: the defect is gone because its implementation is gone, verified by reading the diff, not by a guard.

**What would re-introduce it**, since no test will say so: any future change that kills or signals a background child by a pid retained across the point where the `Child` handle is released. If that is ever added back, the disambiguator must come with it — kill through the `Child`, or through the process group (`shell_command_configured` already puts the child in its own group via `process_group(0)` on unix), never a bare recycled pid.
## Workarounds

None needed at observed frequency.

## Resume

Fold into slice 1 of `docs/trackers/architecture-boundary-measurement.md`
(`c61d542269b5c6de`). If slice 1 is declined, downgrade to a comment correction.
