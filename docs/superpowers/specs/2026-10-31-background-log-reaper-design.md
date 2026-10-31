# Background-command log reaper — design

Date: 2026-10-31
Status: draft, not yet approved
Issue: `docs/issues/2026-10-31-background-command-logs-are-never-reclaimed-when-the-server-ends.md` (bug `45b69c70b723a089`)
Size: small change (three server files plus tests; no prompt-surface change)

## Intent

Every `run_command(run_in_background=true)` leaves a `codescout-bg-XXXXXX.log` file in `$TMPDIR`, and nothing removes it once its server has ended. Measured 2026-10-31 on this machine: 18,798 files, 687 MiB, and 151 report-sized files hold 94.6% of it. The operator wants a gate report, or any background output, freed after the agent has acknowledged it, and not kept for ever.

## Goals

- A log is removed after its outcome has been delivered and read, or after a fixed age, whichever comes first.
- The existing pile (logs with no record) is reclaimed once, safely.
- A running job's log is never removed (the 2026-09-15 incident, `docs/issues/archive/2026-09-15-background-log-eviction-deletes-a-running-jobs-log.md`).

## Non-goals and anti-goals

- No change to the wake flow or to what an agent sees. A read of `@bg_N` after the log is gone already answers "background job log unavailable" (`output_buffer.rs:1821` and `:1991`).
- No new tool and no explicit "ack" call. The existing `.delivered` marker is the acknowledgement.
- Not in scope: the orphaned `codescout-mux-*` directories (5.0 GiB in 60 entries) and other `$TMPDIR` content. Their cause is not verified.
- Anti-goal: silencing the companion plugin's wake hook. It ignores any record whose `schema` is not 1 (`claude-plugins/codescout-companion/hooks/wake-on-job-end.mjs:160,230`), so `RECORD_SCHEMA` must stay 1.

## Constraints (read at the bytes, 2026-10-31)

- `spawn_background_command` creates the log with `tempfile::Builder … .tempfile()?` and then `keep()` (`src/tools/run_command/inner.rs:99-104`). The record is written right after, before the supervisor starts (`:126-140`).
- A finished job's log is unlinked when the in-memory handle is evicted (`output_buffer.rs:1711-1717`). A live job's log is never unlinked there (`:1727`).
- `sweep_stale` (`src/tools/job_record.rs:212`) removes a record, and its `.delivered` and `.watch` sidecars, when the record file is at least `RECORD_MAX_AGE` (7 days) old and the job is finished, or the server is gone. It runs once per server, at the first background spawn (`output_buffer.rs:1671-1677`).
- `JobRecord` (`job_record.rs:62-75`) has no log path. `read_record` is plain `serde_json::from_str`, so an extra optional field breaks no reader.
- Job records exist only since 2026-10-29 (commit `09cbba443`). The 10,749 logs older than 7 days have no record at all.
- `pid_alive` is `kill(pid, 0)` on Unix and `true` elsewhere (`output_buffer.rs:92-102`).
- The wake text tells the agent to read the log after the wake, so the log must outlive `.delivered` by a grace period.

## Approaches

| | Approach | Trade-off | Outcome |
|---|---|---|---|
| A | Add `log_path` to `JobRecord` (additive, schema stays 1); the record sweeper removes the log; one separate sweep for the legacy pile | Smallest change. The owner link is data, so a failed record write leaves a log with no owner (the legacy sweep then covers it) | **Chosen by the operator** |
| B | Sweep loose `codescout-bg-*.log` by age, skipping open files; no record change | One rule for all logs; Linux-only open-file check; the file still names no owner | Not chosen |
| C | Put the log beside its record with the same stem so the existing sweeper owns the pair | Ownership by name, whole class gone; changes the log location | Designer's recommendation; the operator chose A |

## Design

### 1. The record names its log

`JobRecord` gains `log_path: Option<String>` with `#[serde(default, skip_serializing_if = "Option::is_none")]`. `RECORD_SCHEMA` stays 1. `spawn_background_command` sets it before the first write. A record written before this change reads back with `None`.

### 2. Lifetimes (the operator's "free after ack")

New constants, each with a `// cap-class: NOT_A_CAP — <reason>` line above it, as `RECORD_MAX_AGE` has. `tests/result_caps.rs` fails otherwise, which is exactly bug `6fda4eca9da69652`.

| Constant | Value | Meaning |
|---|---|---|
| `LOG_GRACE_AFTER_DELIVERY` | 1 hour | the log goes this long after the `.delivered` marker; the agent reads it right after the wake |
| `LOG_MAX_AGE` | 24 hours | the log goes this long after the job finished, if never delivered or read |
| `RECORD_MAX_AGE` | 7 days (unchanged) | records and sidecars |

### 3. Record-linked sweep

A new function `sweep_logs(dir, grace, max_age, is_alive)`, called right after `sweep_stale` in the same once-per-server housekeeping:

- For a record that is finished and has a `log_path`: unlink the log when the `.delivered` sidecar is older than the grace, or the record file is older than `LOG_MAX_AGE`. The record itself stays until `RECORD_MAX_AGE`.
- For a record that is `running` with a live server: never touch its log.
- For a record that is `running` with a dead server: leave its log to the existing record rule (the job may be an orphan that still writes). At `RECORD_MAX_AGE` the record goes, and its log goes with it only when the log was last written no later than the record; a log written after its record (always the case for a running job's log, and for a name `tempfile` handed to a new job) is kept and left to the legacy sweep below.
- `sweep_stale` also unlinks `rec.log_path` when it removes a record, under the same rule: a log whose mtime is later than its record's is not that record's own and is kept, so it becomes a loose log that the legacy sweep judges by the open-file scan and its own age in the same pass.
- Every record-linked unlink (in `sweep_logs` too) requires the log's mtime to be no later than its record's: a finished job's last log write comes before the supervisor writes the `finished` record.
- Both record-linked sweeps run only when the record directory is owner-only (the check `write_record` applies), because another user could pre-create it and plant a record naming one of our logs.
- A file that cannot be judged (unreadable record, log path outside `temp_dir()`, symlink) is skipped.

### 4. Legacy sweep

A new function `sweep_loose_logs(temp_dir, recorded, age, is_open)`: for each regular file named `codescout-bg-*.log` directly in `temp_dir()` (not a symlink) that no record names (`recorded` is the set of `log_path` values from all records) and whose mtime is at least `LOG_MAX_AGE` old: unlink it unless a process holds it open. On Linux the open set comes from one scan of `/proc/*/fd`. Where it cannot be built, the age rises to `RECORD_MAX_AGE`.

Expected effect on the 687 MiB measured today: the files older than 24 hours (about 448 MiB by the age buckets) go on the first sweep, and the rest ages out within a day. This is an estimate from the age buckets, not a measured run.

### 5. Where it runs

In the existing `record_sweep.call_once` closure, on a detached thread, so the first background spawn does not wait for a scan of tens of thousands of directory entries. Errors are ignored: housekeeping never fails a tool call.

## Testing

Unit tests in `job_record.rs` with a temp directory and injected clocks/`is_alive`/`is_open`: a finished record with an old `.delivered` loses its log and keeps its record; a finished record under the grace keeps both; a log older than `LOG_MAX_AGE` goes; a running record with a live server keeps its log; a running record with a dead server keeps its log until `RECORD_MAX_AGE`; removing a record removes its log; a symlink and an unreadable record are skipped; a loose old log with no record goes; a loose young log stays; a loose old log that a test holds open stays (Linux); a loose log named by a record stays. One integration test: a spawned background job writes a record whose `log_path` is the real log. `tests/result_caps.rs` must stay green.

## Left to the builder

- The exact names and signatures, within the shapes above.
- How `/proc/*/fd` is scanned (a helper under `src/platform/`).
- Whether the detached thread is `std::thread` or `spawn_blocking`.

## My calls (the operator may change any of them)

- 1 hour grace after delivery, 24 hours maximum age, 7 days fallback without an open-file check.
- The legacy sweep uses the open-file check, because the 2026-09-15 incident makes "never remove a live log" the one hard rule.
- A detached thread for the housekeeping.
- No prompt-surface change, so `ONBOARDING_VERSION` stays.
