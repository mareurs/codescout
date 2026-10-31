---
id: '45b69c70b723a089'
kind: bug
status: open
title: 'BUG: background-command logs are never reclaimed when the server ends, so codescout-bg-*.log piles up (18,798 files, 687 MiB)'
owners:
- marius
tags:
- cluster/shared-resource-carries-no-owner
- run_command
- background-job
- disk-leak
topic: background job logs
opened: 2026-10-31
severity: medium
unverified: That an MCP restart is the main leak path is inferred from the code (logs of a dead server are never swept), not measured by tracing which logs belonged to which server.
---

# BUG: background-command logs are never reclaimed when the server ends, so codescout-bg-*.log piles up (18,798 files, 687 MiB)

## Summary

`run_command(run_in_background=true)` writes the job's output to `$TMPDIR/codescout-bg-XXXXXX.log`. The server deletes that file only when it evicts a finished job from its in-memory handle list. When the server ends (an MCP restart, a crash, a kill), every log it held stays on disk. The sweeper that cleans old job records cannot reach the logs, because a record does not name its log. Every gate, test and build run that an agent starts in the background leaves one such file.

## Class fit

IC-17 (a shared resource carries no owner). The log file's name has only a random tail, and the job record that names the job holds no log path. So no sweeper can link a log to an owner, and enumerating the records does not find the logs.

## Symptom (Effect)

measured 2026-10-31, `$TMPDIR=/home/marius/.cache/cc-tmp` (btrfs), one machine with four Claude Code profiles:

```
codescout-bg-*.log: 18798 entries, 700.6 MiB allocated
  <1h: 47 files 2.2 MiB | <1d: 3612 files 250.8 MiB | <3d: 2852 files 246.2 MiB | <7d: 1538 files 129.7 MiB | >=7d: 10749 files 71.7 MiB
codescout-bg/*.json (records): 1572 entries, 6.1 MiB, none older than 3 days
```

By size (687.1 MiB of apparent size):

| Size | Files | MiB |
|---|---|---|
| under 4 KiB | 17430 | 0.5 |
| 4–64 KiB | 1150 | 20.5 |
| 64 KiB–1 MiB | 67 | 16.5 |
| 1–10 MiB (the size of one full gate run) | 135 | 299.3 |
| 10 MiB or more (largest 53 MiB) | 16 | 350.4 |

So 151 report-sized files hold 94.6% of the space. One full gate report is 1,346,249 bytes (a red run of 15,869 lines).

## Reproduction

1. Start a background command: `run_command("echo x", run_in_background=true)`. Note the log under `$TMPDIR`.
2. End the MCP server (for example `/mcp` restart, or kill it).
3. The log is still there. No later server run removes it.

## Environment

codescout `experiments` at `d89b2c0b7`, Linux, `TMPDIR=/home/marius/.cache/cc-tmp`.

## Root cause

Read 2026-10-31:

- `src/tools/run_command/inner.rs:99-104` creates the log with `tempfile::Builder::new().prefix("codescout-bg-").suffix(".log").tempfile()?` and then calls `log_tmp.keep()?`, which turns off the automatic delete.
- `src/tools/output_buffer.rs:1711-1717` removes a log (`std::fs::remove_file(&job.log_path)`) only when the buffer evicts a finished job from its bounded in-memory list. `:1727` states that a live job is evicted without unlinking its log.
- `src/tools/job_record.rs:36` sets `RECORD_MAX_AGE` to 7 days, and `:212` `sweep_stale` removes old finished records and records whose server is gone. It handles records only. Job records exist only since 2026-10-29 (commit `09cbba443`), which is why the oldest record is 3 days old: nothing has had time to expire yet. The 10,749 logs older than 7 days predate the records and have none, so a record-based sweep can never reach them.
- `JobRecord` (`job_record.rs:62-75`) has the fields `schema, handle, command, server_pid, job_pid, state, exit_code, error, started_at, finished_at`. It has no log path, so the sweeper could not unlink a log even if it tried.
- A grep for `codescout-bg` over `src/` finds only these sites and tests: no startup sweep of `codescout-bg-*.log` exists.

measured 2026-10-31: the census above (`python3` over `glob('~/.cache/cc-tmp/codescout-bg-*.log')`). The statement that an MCP restart is the main leak path is inferred from the code, not measured: I did not trace which of the 18,798 logs belonged to a server that had ended.

## Evidence

The census quoted under Symptom. The scripts are session scratch files, not kept; the numbers are the evidence.

## Hypotheses tried

1. **Hypothesis:** the logs are in `/tmp`, so a reboot cleans them. **Test:** `TMPDIR` is `~/.cache/cc-tmp` on btrfs. **Verdict:** rejected here; on a machine whose `TMPDIR` is a tmpfs, the logs cost RAM until a reboot.
2. **Hypothesis:** eviction frees them in normal use. **Test:** read `output_buffer.rs:1711-1727`. **Verdict:** true while the server lives and the job is finished; false for everything a dead server held.

## Fix

Not fixed. Options, in order of cost:

1. Put the log path into `JobRecord` (a schema bump) so that `sweep_stale` unlinks the log together with its record. This covers only logs created after the change: the logs that exist now have no record, so it needs option 2 as well.
2. Sweep orphan `codescout-bg-*.log` files by mtime at server start and on the existing housekeeping pass, with an age shorter than the 7 days used for records (a log is a copy of output the agent has already read).
3. Delete a log when the wake has been delivered and the output read (the `.delivered` sidecar exists today), as an explicit acknowledgement.

A live job's log must never be removed: its mtime moves while it writes, so an age test is safe, but a job that is silent for a long time needs the `is_alive(server_pid)` test the record sweep already has.

## Tests added

None yet. A test needs a temp `TMPDIR` with an old orphan log, a young log, and a log of a live job, and must assert that only the old orphan goes.

## Workarounds

Delete old logs by hand, for example those older than 7 days that no live server uses. The reclaimable amount on this machine today is small by count but not by size for the 151 large files. This should not run while a long job is in flight whose log has not changed for days.

## Resume

Decide between Fix options 1–3. Option 1 plus option 2 together make a tested change in `src/tools/job_record.rs` and `src/tools/run_command/inner.rs`.

## References

- `src/tools/run_command/inner.rs:99-104`
- `src/tools/output_buffer.rs:1711-1727`
- `src/tools/job_record.rs:36,62-75,212`
- `docs/superpowers/specs/2026-10-29-background-job-wake-design.md` (the wake and record design)
