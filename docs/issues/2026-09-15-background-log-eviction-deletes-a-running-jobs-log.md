---
id: '7ca1aa2451dd57a8'
kind: bug
status: open
title: Background job log eviction deletes the log of a job that is still running
owners:
- marius
tags:
- run-command
- output-buffer
- cluster/truncated-window-ordered-by-the-wrong-key
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

Not implemented. Subsumed by the job-record proposal for slice 1 of
`docs/trackers/architecture-boundary-measurement.md` (`c61d542269b5c6de`): once a job
carries terminal state, eviction can prefer terminated jobs and refuse to unlink a
live one. A narrower standalone fix is to skip `remove_file` when the job has not been
observed to exit — but with the current `PathBuf` model there is nothing to observe.

## Tests added

None yet. A regression test can sit entirely on `OutputBuffer` with no process: store
`max_pending + 1` background paths against real temp files and assert the first is
still present when its job is marked live.

## Workarounds

Keep fewer than 20 background jobs alive per session, or redirect important background
output to a path you chose yourself rather than relying on the managed log.

## Resume

Decide alongside slice-1 retention policy. Related: the sibling `@bg_` defect
`docs/issues/2026-09-13-background-command-loses-terminal-status.md`
(`7a17adf0a2766a96`) — same missing job record, different visible failure.
