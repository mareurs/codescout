---
id: f38c0205eaef0829
kind: bug
status: open
title: 'BUG: concurrent calls that carry a workspace pin leave no usage.db row, and the recorder says nothing'
tags:
- cluster/unclassified
opened: 2026-11-09
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
8. **Hypothesis:** the loss came from `/tmp` being at its per-user quota. **Evidence:** `quota -u marius` showed 18.6 GB used of a 19.2 GB hard limit (20,132,659 KB, exactly 80% of 24 GiB; `/tmp` had been resized to 32G on 2026-11-05 but the limit had not followed). The gate, run at the same time, failed 2,101 of 6,307 (LEAN) and 3,689 of 9,196 (DEFAULT) tests with `Disk quota exceeded (os error 122)`, so writes into `/tmp` were failing during the loss window. After the limit was raised to 26,843,545 KB (`sudo setquota -u marius 0 26843545 0 0 /tmp`) and the server was rebuilt and reconnected: 6 of 6 and 8 of 8 concurrent pinned `grep` calls left a row, and `usage-dropped-rows.log` was never created. **Verdict:** consistent, not proven. Two things changed together (the quota and the server process), and no write path from the recorder to `/tmp` has been named. A candidate: sqlite temp files during `open_db` migration and backfill on the 295 MB database, or the pin's first project load. Rejected as a full explanation only if the loss recurs with the quota raised; the new log would then name the failing `?`.
7. **Hypothesis:** the in-process dispatch loses rows too. **Test:** a test through `call_tool_inner` with 6 concurrent `grep` calls, pinned once to the session's own root and once to a second project, on `multi_thread` with 4 workers, asserting 6 rows each. **Verdict:** rejected: it passed (3.48 s). The test was removed because a green result that cannot express the loss is not evidence. What the test lacks, and the live server has: a 295 MB `usage.db` shared by several server processes, a repo with linked worktrees, and a pin that must first load project resources (the first pinned call took 7 to 12 s on the live server).

## Fix
Not started. Two obligations whatever the cause:
- A failed write must leave a trace (a counter in a table that survives, or a log line), because a silent gap makes every rate over `tool_calls` wrong. That is the property this recorder's other refusal rows already keep.
- A pinned call must resolve its project for recording the same way the tool body did, or the row lands in the wrong project's database (see `docs/issues/archive/2026-07-09-residual-workspace-pin-gaps-post-edit-code-fix.md`, finding 4).

## Tests added
None yet. A regression test should issue N concurrent pinned calls through `dispatch_tool_call` and assert N rows.

## Workarounds
Do not pass `workspace=` on concurrent read calls when the usage rows matter. Treat per-tool counts from pinned subagent sessions as lower bounds.

## Resume
Done 2026-11-09: the error is no longer dropped (`note_dropped_row` in `src/usage/mod.rs`, test `a_row_that_cannot_be_written_leaves_a_line_in_the_dropped_rows_log`), and 14 of 14 pinned concurrent calls were recorded after the `/tmp` quota fix. Left: watch `.codescout/usage-dropped-rows.log`; if it stays absent through normal use, the quota explains the loss and this bug can close as environment, with the log kept. If a line appears, its error chain names the failing step.

## References
- `src/usage/mod.rs` `record_content`, `write_content`
- `src/agent/mod.rs` `with_project_at`
- `src/usage/db.rs` `open_db`
- `docs/research/2026-11-09-parallel-tool-calls-transcript-mining.md`
