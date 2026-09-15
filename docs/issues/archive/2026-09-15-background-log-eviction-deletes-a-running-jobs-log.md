---
id: 52f03908f97a948a
kind: bug
status: fixed
title: Background job log eviction deletes the log of a job that is still running
owners:
- marius
tags:
- run-command
- output-buffer
- cluster/truncated-window-ordered-by-the-wrong-key
closed: 2026-09-15
opened: 2026-09-15
severity: medium
---

# BUG: Background job log eviction deletes the log of a job that is still running

## Summary

`OutputBuffer::store_background` (`src/tools/output_buffer.rs:602-626`) evicts the
oldest background job when at capacity and calls `std::fs::remove_file` on its log
path. There is **no liveness check**. The eviction key is insertion order; the
criterion that matters is whether the owning job has exited.

`max_pending` is `20` (`src/tools/output_buffer.rs:152`), and is shared with
`pending_acks`. So the 21st `run_in_background` call in a session unlinks the 1st
job's log while that job may still be writing to it.

## Symptom (Effect)

The evicted job keeps writing to an unlinked inode — its output is unreachable and
the disk space is not reclaimed until the process exits. A later read of that handle
takes the `!log_path.exists()` branch at `src/tools/output_buffer.rs:730` and returns
`background job log unavailable`, whose hint reads *"Check if the process is still
running; its log file no longer exists."* The message invites the reader to suspect
their own job, when the cause is an unrelated 21st background command.

FIFO makes this systematically hit the wrong job: the oldest background job is on
average the longest-running one, which is precisely the job that was backgrounded
*because* it takes long, and therefore the one most likely still writing.

## Reproduction

**Not executed — derived from source.** Stated so the next reader can run it rather
than re-derive it:

1. `run_command(command="sleep 600 > /dev/null", run_in_background=true)` — keep the
   returned `@bg_` handle.
2. Issue 20 further `run_in_background` calls.
3. `run_command("cat @bg_<first>")` -> expect `background job log unavailable`.

The structural claim — that eviction is unconditional on liveness — is verified by
inspection of the complete function body; there is no branch that consults job state,
because no job state exists to consult. The end-to-end runtime effect is not measured.

## Environment

Read at worktree HEAD `bb6a2cb1`, branch `experiments`, 2026-09-15.

## Root cause

`background_jobs` is typed `HashMap<String, PathBuf>`
(`src/tools/output_buffer.rs:136`). A background job **is** a path; there is no
record of process state, so eviction has nothing to consult even in principle. The
defect is in the data model, not in the eviction branch.

## Evidence

`src/tools/output_buffer.rs:602-626` (eviction + `remove_file`), `:136`
(`background_jobs` type), `:152` (`max_pending: 20`), `:730` (the misleading
log-unavailable hint).

## Hypotheses tried

None — found by reading during the architecture-boundary slice-1 design review, not
by a failing call.

## Fix

**FIXED 2026-09-15** in `f098069a` — patch-id `8658a129d49444e33a6fce955a2f8b498bb743d1`.

Fixed as predicted, by the type rather than at the eviction branch: once `background_jobs` holds a `BackgroundJob` carrying `JobState`, eviction has a liveness predicate to consult, which it previously could not have had even in principle.

`store_background` now takes the oldest **terminated** job and unlinks its log. When every retained job is still running there is no safe file to delete, so it drops the oldest **handle** and leaves the log on disk for the owning process and the OS tempdir. Losing addressability is recoverable; unlinking a live log is not.

**Residue, stated rather than silently accepted:** a live job whose handle was evicted leaks its log file, because the supervisor's `set_job_state` no-ops on a handle that is gone and nothing then owns cleanup. That needs >20 concurrent live background jobs to reach, and it is strictly better than the corruption it replaces.
## Tests added

`eviction_never_unlinks_a_live_jobs_log` and `eviction_prefers_a_terminated_job_over_an_older_running_one` (`src/tools/output_buffer.rs`).

**Confirmed by an observed RED.** Mutating the liveness predicate back to FIFO (`is_none_or(|_| true)`) killed both — `a LIVE job's log must survive eviction of its handle` and `the terminated job should have been evicted`. Run in an isolated worktree. Gate green on all four lanes.

The fixture helper carries an annotation saying why its state is `Exited`: a `Running` default would route every unrelated `@bg_` test through the live-job eviction branch and mask a regression there.
## Workarounds

Keep fewer than 20 background jobs alive per session, or redirect important background
output to a path you chose yourself rather than relying on the managed log.

## Resume

Decide alongside slice-1 retention policy. Related: the sibling `@bg_` defect
`docs/issues/archive/2026-09-13-background-command-loses-terminal-status.md`
(`b9935bb5470a799c`) — same missing job record, different visible failure, fixed in
the same commit.
