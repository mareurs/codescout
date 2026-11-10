---
id: f38c0205eaef0829
kind: bug
status: investigating
title: 'BUG: concurrent calls that carry a workspace pin leave no usage.db row, and the recorder says nothing'
tags:
- cluster/unclassified
opened: 2026-11-09
unverified: The lock holder is not named. 33 drops (2026-11-09 21:35-23:18 UTC) and 2 drops (2026-11-10 11:02 and 11:05 UTC) are all database-is-locked waits of 5.0 s. The retention sweep and the insert are slow on a cold file, but a hold line is statement time including the wait, so it does not show who held the lock. One drop (11:05:34) has no recorded hold at all. About 40 servers still run builds without the lock-hold log.
---

# BUG: concurrent calls that carry a workspace pin leave no usage.db row, and the recorder says nothing

## Summary
When several read calls that each carry `workspace="<abs path>"` run at the same time, most of them leave no row in `.codescout/usage.db`. Every call still answers. The same calls without `workspace=` all leave a row. The recorder drops the error (`let _ =`), so nothing marks the gap.

## Class fit
The recorder is best-effort by design, so a failed write is dropped and the table reads as if the call never happened. No existing class names a failure that is rendered as an absence. The nearest are classes about a failure rendered as an answer, but here the answer is correct and the telemetry is the part that vanishes. Filed unclassified until a second instance shows the pattern.

## Symptom (Effect)
Two measurements on 2026-11-09, one session, one server shared with two other sessions.

Run 1 — four `symbols(name=..., workspace="/home/marius/work/claude/codescout")` calls in one message: two rows (`normalize_err_family`, `replay_ack`). No row for `acquire_write_guard` and none for `partition_batches`. Both answered.

Run 2 — six `grep(mode="files", pattern="zzprobe_<n>_unique", workspace="/home/marius/work/claude/codescout")` calls in one message: one row (`zzprobe_alpha_unique`, id 286311, 7,219 ms). Five calls answered with `0 matches` and left no row.

Row ids around run 2 are consecutive (286305 to 286314), so the missing rows never received an id: the insert did not happen or failed.

Control — six `grep(mode="files", pattern="zzctl_<n>_unique")` calls in one message, no `workspace=`: six rows (ids 286322 to 286327).

## Reproduction
1. Activate no project, or any project. Use a checkout that has linked worktrees.
2. In one assistant message, issue six `grep` calls with `mode="files"`, distinct patterns and `workspace` set to the repo root.
3. Count rows: `sqlite3 -cmd ".timeout 8000" .codescout/usage.db "select count(*) from tool_calls where input_json like '%zzprobe_%'"`.
Expected 6. Observed 1 in run 2.

Not yet isolated: whether a solo pinned call is always recorded (one solo `symbols` with a pin was recorded, id 286279), and whether the loss depends on the number of calls or on other sessions writing at the same time.

## Environment
Linux, codescout stdio server `experiments` at `fe9658883` plus uncommitted peer work, rmcp 1.7.0, tokio multi-thread runtime, WAL mode, `busy_timeout` 5 s. Two other agent sessions were writing to the same `usage.db` in the same minutes (their rows interleave in the id range).

## Root cause
Unknown — see below. What is established:
- `UsageRecorder::record_content` drops the result of the write: `let _ = self.write_content(...)` (`src/usage/mod.rs`, in `record_content`, "Best-effort — never let recording fail the tool call"). A failure leaves no log line I could find.
- `write_content` can fail in two places before the insert: `self.agent.with_project_at(workspace_override, ...)` and `db::open_db(&db_root)`.
- With a pin, `with_project_at` first runs `ensure_resident(root)` and then looks the workspace up with `inner.workspaces.get(&key)`, which fails with `pinned workspace not resident: <path>` if the key is absent (`src/agent/mod.rs`, `with_project_at`).
- `open_db` opens a fresh connection per row, runs `CREATE TABLE IF NOT EXISTS` and migration probes, with a 5 s `busy_timeout` (`src/usage/db.rs`).

Candidate causes, none measured: a race in `ensure_resident` or in the `workspaces` map when several pinned calls arrive together, or `open_db` contention. Only the first differs between pinned and unpinned calls, which fits the control, but that is an inference.

measured 2026-11-09: run 1, run 2 and the control above, by counting rows after each batch. The mechanism is inferred from the source, not measured.

## Evidence
Run 2 rows after the batch:

```
id|st|latency_ms|outcome|inp
286311|11:55:30.810|7219|success|{"mode":"files","pattern":"zzprobe_alpha_unique",...
```

Control rows (no pin), all six present:

```
286322|11:56:33.083|870|success|zzctl_four_unique
286323|11:56:32.419|1535|success|zzctl_two_unique
286324|11:56:33.695|258|success|zzctl_six_unique
286325|11:56:32.448|1505|success|zzctl_one_unique
286326|11:56:33.430|523|success|zzctl_five_unique
286327|11:56:32.710|1243|success|zzctl_three_unique
```

## Hypotheses tried
1. **Hypothesis:** the missing rows exist with different input text. **Test:** query by time window and by id range without any input filter. **Verdict:** rejected: the ids are consecutive, so no row was written.
2. **Hypothesis:** the unpinned path loses rows too. **Test:** the control batch. **Verdict:** rejected for six calls.
3. **Hypothesis:** `ensure_resident` or the `workspaces` lookup fails under concurrency. **Verdict:** deferred, not tested.
4. **Hypothesis:** `open_db` contention past the 5 s timeout. **Verdict:** deferred, not tested.
5. **Hypothesis:** the loss needs a pin AND concurrency. **Test:** live batches on 2026-11-09, one session. Pinned and concurrent: 2 calls gave 0 rows, 3 gave 1, 4 gave 1 (and 2 in an earlier run), 6 gave 1. Pinned, one call per message: 3 of 3 recorded. Unpinned and concurrent: 6 of 6 and 4 of 4 recorded. **Verdict:** supported: 5 of 19 rows for pinned concurrent calls against 10 of 10 unpinned. Small samples, one server shared with two other sessions.
6. **Hypothesis:** a race in `Agent::ensure_resident` or in the `workspaces` lookup. **Test:** read `ensure_resident` (`src/agent/mod.rs`). It loads without a lock and re-checks under the write lock, so a race there is handled. **Verdict:** not supported by the code, not excluded at runtime.
8. **Hypothesis:** the loss came from `/tmp` being at its per-user quota. **Evidence:** `quota -u marius` showed 18.6 GB used of a 19.2 GB hard limit (20,132,659 KB, exactly 80% of 24 GiB; `/tmp` had been resized to 32G on 2026-11-05 but the limit had not followed). The gate, run at the same time, failed 2,101 of 6,307 (LEAN) and 3,689 of 9,196 (DEFAULT) tests with `Disk quota exceeded (os error 122)`, so writes into `/tmp` were failing during the loss window. After the limit was raised to 26,843,545 KB (`sudo setquota -u marius 0 26843545 0 0 /tmp`) and the server was rebuilt and reconnected: 6 of 6 and 8 of 8 concurrent pinned `grep` calls left a row, and `usage-dropped-rows.log` was never created. **Verdict:** contradicted by hypothesis 9 for the 27 rows the log recorded (none carries a quota error). It may still have caused the earlier losses; that is unproven. Originally "consistent, not proven": two things changed together (the quota and the server process), and no write path from the recorder to `/tmp` has been named. A candidate: sqlite temp files during `open_db` migration and backfill on the 295 MB database, or the pin's first project load. Rejected as a full explanation only if the loss recurs with the quota raised; the new log would then name the failing `?`.
7. **Hypothesis:** the in-process dispatch loses rows too. **Test:** a test through `call_tool_inner` with 6 concurrent `grep` calls, pinned once to the session's own root and once to a second project, on `multi_thread` with 4 workers, asserting 6 rows each. **Verdict:** rejected: it passed (3.48 s). The test was removed because a green result that cannot express the loss is not evidence. What the test lacks, and the live server has: a 295 MB `usage.db` shared by several server processes, a repo with linked worktrees, and a pin that must first load project resources (the first pinned call took 7 to 12 s on the live server).
9. **Hypothesis:** the dropped rows come from SQLite lock contention, not from the quota. **Test:** read `.codescout/usage-dropped-rows.log` (27 lines, first 13:12:49, last 15:49:06, log clock) and the code of `open_db`, `backfill_legacy_rows` and `write_record`. **Result:** all 27 lines carry `database is locked: Error code 5: database is locked`. None names a quota error. They fall in bursts (13:12, 13:19–13:21, 13:59–14:02, 15:41–15:49) and cover pinned and unpinned (`-`) calls, and the tools `symbols`, `read_file`, `run_command`, `memory`, `workspace`, `get_guide`. **Verdict:** the immediate cause is a write that waits longer than the 5 s `busy_timeout`. Which writer holds the lock for more than 5 s is NOT established.
   Facts measured on 2026-11-09, `usage.db` at 295 MB and 109,431 rows:
   - About 48 `codescout start` processes share the file, almost all on a deleted binary (oldest started 2026-10-07, read from `/proc/<pid>/exe` and `ps`).
   - `PRAGMA user_version` is 676504337. Each `open_db` calls `backfill_legacy_rows`, which re-runs its `UPDATE`s when `user_version` differs from the binary's `err_family_fingerprint`. Binaries with different fingerprints could re-stamp it in turn. This is a hypothesis: not observed.
   - `write_record` runs `DELETE FROM tool_calls WHERE called_at < datetime('now','-30 days')` after every insert. `EXPLAIN QUERY PLAN` says `SCAN tool_calls` (the only index is `idx_tool_calls_tool_use_id`). The same predicate as a `SELECT count(*)` takes 0.055 s on the live file when its pages are cached and matches 0 rows. That timing was warm; hypothesis 10 measured it cold, and "cannot hold the lock for 5 s" was wrong for a cold file. The sweep costs a write transaction per call.
10. **Hypothesis:** the lock is held for seconds by a full-table scan on a cold file, and the holder is the backfill or the retention sweep. **Test (2026-11-09, one hour, two samplers one minute apart, plus timings on a `.backup` copy of the 295 MB file):**
   - Copy, same `SELECT count(*) ... WHERE called_at < datetime('now','-30 days')` run four times in a row: 5.6 s, 2.4 s, 0.51 s, 0.05 s. The `UPDATE ... WHERE project_root IS NULL` and the `DELETE` sweep, both matching 0 rows, took 2.4 s and 2.6 s on first run. A scan is I/O bound while pages are cold, and the first run exceeds the 5 s `busy_timeout`.
   - `fincore` on the live file, once a minute for 58 minutes: resident bytes swing between 0 and 281 MB of 295 MB, with 0 or 16-20 KB at 21:35, 21:51-21:53 and 21:57-21:59 (local). The page cache is 5.9 GB of 128 GB, so the file is evicted completely, several times an hour.
   - `user_version` sampled once a minute for 61 minutes: 52 samples `676504337`, 2 samples `1965217055` (21:05, 21:06), 7 samples `1877607761` (21:22, 21:25, 21:27, 21:30, 21:35, 21:40, 21:44). At least three fingerprints are alive and stamp the file in turn. 49 to 51 server processes, no new start at those instants. Samples show state, not transitions, so 9 of 61 is a lower bound. Each binary whose fingerprint differs re-runs `backfill_legacy_rows` and stamps its own value.
   - `usage-dropped-rows.log` grew from 27 to 55 lines. 12 of the 28 new lines carry the new columns: pid 1059053 (5 lines, `wait_ms` 5007 to 5082) and pid 899302 (7 lines, `wait_ms` 5005 to 5737), build `7be425b48`. Every one waited the whole `busy_timeout`: none failed at once, so none is a snapshot or recovery error. The other 16 new lines have 4 columns and come from older binaries. The log clock is UTC; local is UTC+2.
   **Verdict:** supported as a mechanism, not proven as the cause. NOT shown: (a) who holds the lock: the pid on a line is the waiter; (b) that the backfill or the sweep is the holder; (c) a link between low residency and a drop (3 of 12 drop minutes had a low reading before them; three zero-residency stretches had no drop); (d) a link between a visible `user_version` change and a drop (3 of 12 drop minutes). Observer effect: the 12 new-column lines fall between 21:07 and 21:15 local, when this session ran large greps, a `.backup` of the live file and scans of the copy. After 21:15 every new line came from an older binary.
11. **Hypothesis:** the 7-column drops can be matched to a line in `usage-lock-holds.log` (kinds `backfill`, `insert`, `retention_sweep`; interval `[started_at, started_at + ms]`, started_at is taken before the statement). **Test (2026-11-10, read of both logs; 674 hold lines after 2026-11-09 20:21 UTC, 33 drops with 7 columns, 16 older-format lines excluded):**
   - 32 of 33 drops have at least one hold overlapping the 10 s before the drop; for 28 of 33 the drop instant lies inside one hold. Control: 5000 random 10 s windows in the same span (21:35-23:19 UTC) overlap a hold in 50.5% of cases, so the match is above the base rate but the span was chosen because drops happened in it. Holds of 5 s or more: 54 inside the span, 1 outside it.
   - Holds by kind in the log: 490 `retention_sweep` (median 3152 ms, p90 5124 ms, max 14779 ms; 497 of 501 later lines say `deleted=0`, four say `deleted=2` to `5`), 179 `insert` (median 1748 ms, max 5085 ms), 5 `backfill` (701 to 6359 ms). No slow `backfill` lies near a drop except pid 1752622 at 21:38:44 (6359 ms), which comes after the drop at 21:38:38 from the same pid.
   - 20 of 33 drops have an overlapping hold from the waiter's own pid: one process competes with itself over several connections.
   - Waiters by build: `f34fbc8b3` 22, `ea44079c4+dirty` 6, `f6d96e59f` 5. Three waiters were still alive and are named by `/proc`: pid 1752622 (session b3485581, profile `.claude-kat`), pid 3531871 (session 732f1c09, `.claude`), pid 3504791 (session 677278c4, `.claude`). Pids 1719246 and 3517646 are named by `diagnostic-189e.log` and `diagnostic-daa8.log` (version 0.15.0) but not by session. Pids 1686255, 1528621 and 1683570 are named nowhere.
   - Operator action 2026-11-10 08:45 UTC: SIGTERM and then SIGKILL on the three live waiters' servers (1752622, 3531871, 3504791); the three sessions restarted their servers (pids 2725400, 2795639, 944490 with builds `fd7f7a029`, `5b40efc48`, `01dcbaf5e`).
   - Two new drops after 11 hours with none: 11:02:20 UTC (`get_guide`) and 11:05:34 UTC (`run_command`), both from pid 2829334, build `5b40efc48`, `wait_ms` 5010 and 5013. Pid 2829334 is a Codex-launched server (parent `codex app-server-daemon`), started 11:01:32 UTC; its rows in `usage.db` belong to Codex thread 01a1bf68-e650-7f20-911e-b53f86597c27 (inferred, not proven). For the 11:02:20 drop, pid 944490 (build `01dcbaf5e`) logged an `insert` of 2141 ms (until 11:02:19.65) and a `retention_sweep` of 2412 ms (until about 11:02:22.07), and pid 2829334 logged two `insert` lines for `get_guide` of 1749 ms and 4841 ms from 11:02:20.425. For the 11:05:34 drop no hold line of any pid overlaps the 10 s before it.
   **Verdict:** supported that the drops come with slow statements on the same file; NOT shown: (a) who holds the lock: a hold line's `ms` is statement time and includes waiting for the lock, so a line cannot tell wait from work; (b) the cause of any single drop; (c) why a sweep that deletes nothing takes seconds (cold cache remains a hypothesis from hypothesis 10); (d) anything about processes that run builds without the lock-hold log: about 40 servers still run older binaries, and the 11:05:34 drop has no hold in any instrumented process, so its holder is either uninstrumented or not one of the three measured statements. The kill of three servers did not stop the drops.

## Fix
Not started. Two obligations whatever the cause:
- A failed write must leave a trace (a counter in a table that survives, or a log line), because a silent gap makes every rate over `tool_calls` wrong. That is the property this recorder's other refusal rows already keep.
- A pinned call must resolve its project for recording the same way the tool body did, or the row lands in the wrong project's database (see `docs/issues/archive/2026-07-09-residual-workspace-pin-gaps-post-edit-code-fix.md`, finding 4).

## Tests added
None yet. A regression test should issue N concurrent pinned calls through `dispatch_tool_call` and assert N rows.

## Workarounds
Do not pass `workspace=` on concurrent read calls when the usage rows matter. Treat per-tool counts from pinned subagent sessions as lower bounds.

## Resume
Done 2026-11-09: the error is no longer dropped (`note_dropped_row` in `src/usage/mod.rs`, test `a_row_that_cannot_be_written_leaves_a_line_in_the_dropped_rows_log`), and 14 of 14 pinned concurrent calls were recorded after the `/tmp` quota fix. Superseded the same day: the log did record 27 drops, all `database is locked` (hypothesis 9), so "close as environment" no longer applies. Left: name the writer that holds the lock for more than 5 s. Done the same day: each log line now names the waiter's pid, build and `wait_ms` (`4d9b3fbd5`); hypothesis 10 holds the first readings. Left: name the holder. The log cannot, because it records the waiter. Candidate: time and log, per `open_db` call, whether `backfill_legacy_rows` ran and how long it held the write lock, and the same for the retention `DELETE` in `write_record`, then read which of them was running when a waiter timed out. Fix candidates, not chosen before the holder is named: an index on `called_at` or a rarer retention sweep, backfill off the hot path (once per process, or when the stamp differs by more than a build), and a longer `busy_timeout`. A restamp loop between binaries of different fingerprints is a defect on its own account, because it re-runs a table scan per flip; it is filed as bug 1a64131278d421ce.
Status set to `investigating` on 2026-11-10 (no live owner): hypothesis 11 holds the readings, and no fix is chosen. Re-check on or after 2026-11-14, when the servers that still run older binaries have been replaced: (1) count 7-column lines in `usage-dropped-rows.log` newer than 2026-11-10 11:06 UTC; (2) for each, list the hold lines of `usage-lock-holds.log` that overlap the 10 s before it, as `.codescout/usage-lock-holds.log` columns are `started_at, pid, build, kind, ms, detail`; (3) count how many drops have no hold at all, because that count measures the holders that still escape the log. If the drops stopped, record the last drop and the build mix, and set `zombie` with `last_observed`. If they continue with holds of the same pid and kind, the next measurement is one that separates the wait from the work inside a statement, then the fix candidates above. A drop with no overlapping hold on an all-instrumented population would point to a lock holder outside the three measured statements.

## References
- `src/usage/mod.rs` `record_content`, `write_content`
- `src/agent/mod.rs` `with_project_at`
- `src/usage/db.rs` `open_db`
- `docs/research/2026-11-09-parallel-tool-calls-transcript-mining.md`
