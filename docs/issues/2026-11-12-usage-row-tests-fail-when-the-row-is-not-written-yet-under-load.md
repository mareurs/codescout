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
unverified: Two failing iterations in 35, five tests, one machine. The recorder's write path was not read, so whether the row is written before the response is not established. That a stall of the host explains the four simultaneous failures is a reading of their timings and of the 109 s iteration, not a measurement of the host.
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

## Evidence

The panic lines above, from the logs of the two runs (`@bg_00000024`, `@bg_00000028`, session `5f715d8f`). Archived records on the same area: `2026-11-08-access-gate-refusals-leave-no-usage-row`, `2026-11-08-cancelled-and-timed-out-calls-leave-no-usage-row`, `2026-11-09-write-lock-refusal-row-records-zero-latency-for-a-call-that-waited`.

## Hypotheses tried

None. Not tried: that a stall of the host explains iteration 13; that the recorder writes in the background.

## Fix

Not fixed.

## Tests added

None. N/A: not investigated.

## Workarounds

Rerun the gate. A failure in only these tests, with a pass on the rerun, is this bug.

## Resume

Read the recorder's write path (where the `tool_calls` row is inserted relative to the response). If it is written in the background, a fixed wait is the wrong shape for both tests and the fix is a bounded poll in `native_recorded_session`, as `rows_for` has, with a window longer than 5 s. If it is written before the response, then neither test should ever see an empty table, and the failure points at something else (a second connection reading before commit, or a lost write).

## References

- `src/server/exit_rows_tests.rs`: `rows_for`, `assert_one_row`
- `src/server/native_context_tests.rs`: `native_recorded_session`
- `20927ccfe293c34e` (the same class: tests with a wall-clock window under load)
