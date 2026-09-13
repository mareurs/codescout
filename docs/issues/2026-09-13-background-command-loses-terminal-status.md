---
id: '7a17adf0a2766a96'
kind: bug
status: open
title: Background command completion is not observable through its log handle
owners:
- marius
tags:
- agent-harness
- run-command
- cluster/unclassified
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

Not implemented. Candidate: retain job lifecycle state and expose terminal result/cancellation through a job handle, independent of log buffering. Even before that boundary, do not assert a running state that was not observed.

## Tests added

Recorded live characterization and control; no production regression or fix yet.

## Workarounds

Wrap important background commands to print explicit exit markers, and distinguish wrapper/read exit codes from each underlying command. This does not provide general cancellation or lifecycle state.

## Resume

Evaluate as part of the agent-harness architecture review; implementation requires a separately approved runtime change. No fix SHA or patch-id exists.
