---
id: '6767d5eaafa320c7'
kind: bug
status: open
title: 'BUG: tests that read a usage.db row after a refused call fail when the row is not there yet, 5 tests in 2 files'
owners:
- marius
tags:
- cluster/repro-env-diverges-from-gate-env
- testing
opened: 2026-11-12
related:
- '20927ccfe293c34e'
severity: low
unverified: 'Two failing iterations in 35, five tests, one machine. The recorder writes the row before the response returns (read in src/usage/mod.rs), so the background-write hypothesis is refuted for record_content; I did not read the two server.rs call sites beyond their names, nor make_server_in_root. Whether the failing tests lost the row to a dropped write (the production log shows 214 such drops, all `database is locked` near the 5 s busy timeout, from the real server) or to a failed read that rows_for hides is not established: the captured stderr of the two failing runs is gone. That a stall of the host explains the four simultaneous failures is a reading of their timings and of the 109 s iteration, not a measurement of the host.'
---

# BUG: tests that read a usage.db row after a refused call fail when the row is not there yet, 5 tests in 2 files

## Summary

In two of the 35 iterations of a full-suite stress run (2026-11-12, `cargo nextest run --workspace --no-fail-fast --stress-count`), tests that read a row from `usage.db` right after a refused or unusual call found none. Five tests failed: four in `src/server/exit_rows_tests.rs` (`exactly one row; got []`, about 25.8 s each, all four within 0.3 s of one another, in an iteration that took 109 s instead of 72 s) and one in `src/server/native_context_tests.rs` (`no single usage row for marker-refused: Query returned no rows`, 3.1 s). Each passed in all the other iterations.

## Class fit

The tests pass in a quiet run and fail under load, which is the trait of `repro-env-diverges-from-gate-env`. They differ from the class's usual member in that the load here was not made on purpose: it was an ordinary gate-like run on a host shared with other sessions.

## Symptom (Effect)

Run 1, 15 iterations, iteration 13:

```
FAIL [25.843s] server::exit_rows_tests::a_refused_ack_handle_leaves_one_row_in_its_family
FAIL [25.851s] server::exit_rows_tests::a_configured_off_gate_refusal_leaves_one_row_in_its_own_family
FAIL [25.930s] server::exit_rows_tests::an_unknown_tool_stays_a_protocol_error_and_leaves_one_row
FAIL [26.116s] server::exit_rows_tests::a_refused_write_lock_leaves_one_row
exit_rows_tests.rs:57:5: <what>: exactly one row; got []
```

Run 2, 20 iterations, iteration 14:

```
FAIL [3.111s] server::guide_hint_tests::native_codex_root_is_recorded_for_a_refused_pin
native_context_tests.rs:358:29: no single usage row for marker-refused: Query returned no rows
```

## Reproduction

Not on demand. `scripts/with-slot.sh cargo nextest run --workspace --no-fail-fast --stress-count 15`, then `--stress-count 20` after `cargo clean -p codescout`, on `experiments`, no extra load job. The first run failed in one iteration of 15 (the four tests), the second in one iteration of 20 (the one test).

## Environment

Linux, a host shared with other sessions, `experiments` at `d516da4cb` for both runs; in run 2 the working tree also held the rename-helper change, committed afterwards as `4603d77cf`. Iteration 13 of run 1 took 109.4 s, against 71 to 86 s for the other iterations.

## Root cause

Unknown. Read in the code on 2026-11-12:

- `exit_rows_tests.rs`, `rows_for`: waits up to 100 x 50 ms = 5 s for any row, sleeps 300 ms, reads again. The four tests took 25.8 s, so about 20 s passed before this wait began.
- `native_context_tests.rs`, `native_recorded_session`: one `query_row`, no wait at all.

Both depend on the recorder having written the row by the time they read. I did not read the recorder's write path, so whether the row is written before the response returns or in the background is not established. That the four tests of run 1 ended within 0.3 s of one another fits one stall of the host that was longer than the 5 s window; that is a reading of the timings, not a measurement.


Read on 2026-11-14 (session `5f715d8f`), at the bytes:

- **The row is written before the response returns.** `UsageRecorder::record_content` (`src/usage/mod.rs:101-179`) awaits the tool future, then awaits `write_content`, which opens the database and calls `db::write_record`, and only then returns the result. Its two callers in `src/server.rs` are at lines 1022 and 2187 (I read the call names, not the code around them). So the hypothesis that the recorder writes in the background is refuted for this path, and a fixed wait in the tests is not needed for that reason.
- **A failed write leaves a trace.** `note_dropped_row` (`src/usage/mod.rs:238-279`) emits `warn!("usage row dropped: the call answered but its row was not written")` and appends one line to `<home project>/.codescout/usage-dropped-rows.log`. `open_db` sets a 5 s `busy_timeout` (`src/usage/db.rs:20`).
- **That failure mode is real in production.** The repo's own `.codescout/usage-dropped-rows.log` has 214 lines, from 2026-11-09 13:12 to 2026-11-12 08:55 (UTC stamps in the line). Every line says `database is locked: Error code 5`. The wait before failure has a median of 5006 ms, p90 5020 ms and max 5776 ms, which is the 5 s busy timeout. By tool: run_command 66, read_file 45, symbols 42, grep 30, doc 9, get_guide 5, edit_file 5, workspace 4. At least one line has an empty `wait_ms` field (the minimum printed empty). These are the real server's writes against the repo's database, not test writes.
- **The tests use a private database.** Each test's server is built by `make_server_with_project_toml`, which returns a `tempfile::TempDir`, so no other test shares its `usage.db`. A lock therefore needs a second connection inside the same test process (for example a background task) or a stall of the host. I did not read `make_server_in_root`, and I did not look for such a second connection.
- **`rows_for` cannot tell "no row" from "read failed".** `src/server/exit_rows_tests.rs:24-49` turns a failed open, a failed prepare and a failed query into an empty list. `got []` therefore does not say which of the two happened. rusqlite 0.39 gives a new connection a 5000 ms busy timeout by default (`rusqlite-0.39.0/src/inner_connection.rs:118`), so a read error also needs a lock longer than 5 s. My first guess, that this read connection had a zero timeout, was wrong.
- **The evidence that would decide it is gone.** nextest prints the captured stderr of a failing test, and that would hold the `warn!` line if a write had been dropped. None of the 1221 retained `~/.cache/cc-tmp/codescout-bg-*.log` files contains either failure text, so I cannot check.

## Evidence

The panic lines above, from the logs of the two runs (`@bg_00000024`, `@bg_00000028`, session `5f715d8f`). Archived records on the same area: `2026-11-08-access-gate-refusals-leave-no-usage-row`, `2026-11-08-cancelled-and-timed-out-calls-leave-no-usage-row`, `2026-11-09-write-lock-refusal-row-records-zero-latency-for-a-call-that-waited`.

## Hypotheses tried

None. Not tried: that a stall of the host explains iteration 13; that the recorder writes in the background.


Tried on 2026-11-14, by reading code and the production log (no new test run): that the recorder writes in the background (refuted for `record_content`, see Root cause); that the read connection has no busy timeout (refuted, the default is 5000 ms). Not tried: that a second connection in the test process holds the lock; that a stall of the host explains the drops; a run that keeps the captured stderr of a failure.

## Fix

Not fixed.

## Tests added

`01a93680a` (patch-id `51f8d3d56cef`) adds diagnostics, not a fix: `rows_for` keeps its last read error and, when it ends with no row, prints it with the dropped-rows log; `native_recorded_session` puts the log in its panic text; the shared helper is `test_support::dropped_rows_log`. The test `the_empty_row_diagnosis_names_the_read_error_and_the_dropped_rows_log` pins the message for three cases (no log, a two-line log, an unreadable log). Gate green on that commit: FMT 0, CLIPPY 0, LEAN 10048 passed, DEFAULT 10142 passed, Python 1836 passed. Not shown: that the message appears in a real failure (nextest prints captured stderr for a failing test, which I did not exercise), and the new test was not mutation-tested; its assertions name each of the three parts of the message.

## Workarounds

Rerun the gate. A failure in only these tests, with a pass on the rerun, is this bug.

## Resume

Read the recorder's write path (where the `tool_calls` row is inserted relative to the response). If it is written in the background, a fixed wait is the wrong shape for both tests and the fix is a bounded poll in `native_recorded_session`, as `rows_for` has, with a window longer than 5 s. If it is written before the response, then neither test should ever see an empty table, and the failure points at something else (a second connection reading before commit, or a lost write).


Update 2026-11-14: the first branch above is settled, the row is written before the response returns, so a fixed wait is not the fix. What remains is whether the write failed (a dropped row, 5 s lock) or the read failed. The next failure decides it if its captured output is kept: search it for `usage row dropped`. A cheaper step is a test-side change that makes the panic text carry the evidence: have `rows_for` and `native_recorded_session` return the read error, and print the tempdir's `.codescout/usage-dropped-rows.log` when no row is found. That change is not made. It would cost one gate run and would turn the next failure into a measurement.


Update 2026-11-14 (after `01a93680a`): the diagnostics are in place. On the next failure of one of these five tests, read the failing test's captured output first. `rows_for(<tool>): no row after 5.3 s; last read error: …; usage-dropped-rows.log: …` separates the cases: a read error means the read failed; `absent` with `none, every read succeeded…` means the row was never written and the recorder did not report a failed write; a logged `database is locked` line means the write was dropped. Keep that output in the bug before anything else.

## References

- `src/server/exit_rows_tests.rs`: `rows_for`, `assert_one_row`
- `src/server/native_context_tests.rs`: `native_recorded_session`
- `20927ccfe293c34e` (the same class: tests with a wall-clock window under load)
