---
id: b9935bb5470a799c
kind: bug
status: fixed
title: Background command completion is not observable through its log handle
owners:
- marius
tags:
- agent-harness
- run-command
- cluster/unclassified
closed: 2026-09-15
opened: 2026-09-13
severity: medium
---

# BUG: Background command completion is not observable through its log handle

## Summary

run_command background responses say Process running even after an immediate failure, and retain no terminal process status. A log-read exit code describes the reader, not the original job.

## Symptom (Effect)

`run_command(command="sh -c 'exit 7'", run_in_background=true)` returned a background handle and `Process running`. A subsequent tail of that handle returned `exit_code: 0` and no stdout. The control command `sh -c 'exit 7'; job_exit=$?; printf 'FIXTURE_EXIT=%s\\n' "$job_exit"` surfaced `FIXTURE_EXIT=7` but still said Process running.

## Reproduction

Run the two commands above through MCP in a disposable workspace, then read each returned handle with run_command tail. These exact calls and responses are preserved in `.codescout/measurements/architecture-boundary/2026-09-13/workflow-contracts.json`.

## Environment

Measured 2026-09-13, Linux, attached MCP workspace status reported git_sha 408709ea, git_dirty true, pid 2178312, exe_deleted true. Current worktree source was inspected separately; do not equate it to the serving executable.

## Root cause

`src/tools/run_command/inner.rs` function `spawn_background_command` drops Child, waits five seconds, stores a log path in output_buffer, and unconditionally writes Process running. This source shape was read through symbols; the runtime behavior was measured with the reproduction above. There is no exit code in the observed background response.

## Evidence

See workflow-contracts.json and docs/trackers/architecture-boundary-measurement.md. The explicit-marker control distinguishes an actual failing command from an empty-log interpretation.

## Hypotheses tried

Missing terminal observation rather than command success: confirmed by explicit exit-7 marker control. The log-reader exit code is not the child exit code.

## Fix

**FIXED 2026-09-15** in `f098069a` — patch-id `8658a129d49444e33a6fce955a2f8b498bb743d1`.

Shipped as slice 1 of `docs/trackers/architecture-boundary-measurement.md`. The candidate in the original filing was right about the shape and understated the cause: the exit status is not merely unrecorded, it is **destroyed**. `drop(child)` hands the process to tokio's orphan reaper, and there is no other channel the status exists on — so this could never have been a smaller fix than a job record.

- `OutputBuffer::background_jobs` holds `BackgroundJob { log_path, command, state }` instead of a bare `PathBuf`. `JobState` is `Running | Exited { code } | Failed`, and `Exited { code: None }` reports a signal death as absent rather than as `0`.
- A supervisor task owns the `Child`, awaits `wait()`, and writes the observed state. It outlives the tool call deliberately, so cancelling the call does not discard the outcome.
- The status reaches the caller through the **response envelope** (`OutputBuffer::job_states_in` attaches a `jobs` array to any command naming the handle). It cannot travel the `@bg_` channel: that resolves by textual substitution to a filename, which is exactly why a `tail @bg_x` returned the reader's exit code.
- The unconditional `Process running.` string is gone, and so is the 5s warm-up. The call returns at spawn time, which makes the running state true **by construction** at emit time rather than a claim made after a wait.
## Tests added

`a_failed_background_job_reports_its_exit_code_through_the_envelope` and `the_readers_exit_code_and_the_jobs_outcome_are_reported_separately` (`src/tools/run_command/tests.rs`). The second exists because the reader's exit code and the job's outcome are different numbers that must not be conflated — `cat` succeeds while the job it reads failed, the exact pair that made this defect invisible.

**Both confirmed by an observed RED, not by existing.** Mutating the supervisor's `status.code()` to `Some(0)` killed the first with `last seen: "exited 0"` — the defect verbatim. Mutating `job_states_in(command)` to `job_states_in("")` killed the second. Run in an isolated worktree, so no red was published to the shared tree. Gate green on all four lanes (`FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`).
## Workarounds

Wrap important background commands to print explicit exit markers, and distinguish wrapper/read exit codes from each underlying command. This does not provide general cancellation or lifecycle state.

## Resume

Evaluate as part of the agent-harness architecture review; implementation requires a separately approved runtime change. No fix SHA or patch-id exists.
