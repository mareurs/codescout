---
id: b11f090a939e45aa
kind: bug
status: open
title: 'BUG: a write-lock refusal row records latency_ms 0 for a call that waited up to write_lock_timeout_secs'
tags:
- cluster/unclassified
opened: 2026-11-09
---

## Summary
A write tool that waits for the write lock and is then refused leaves a `usage.db` row with `latency_ms = 0` and `started_at` equal to the end of the wait. The row hides the time the call was held, up to `write_lock_timeout_secs` (default 5 s).

## Class fit
No roster class fits without forcing it. The recorder is correct for the calls it sees. The row for this exit is written afterwards, from a path that has no start clock, so one measurement is missing in one code path. That is an observation gap, not a wrong decision, and the roster has no class for it.

## Symptom (Effect)
`latency_ms` for a `write_lock_unavailable` row is 0. The server timeout row (`tool_timeout`) records the time the call was held; this row does not.

## Reproduction
1. Hold `.codescout/write.lock` from another process for 14 s: `python3 -I -c "import fcntl,time; f=open('.codescout/write.lock','a'); fcntl.flock(f,fcntl.LOCK_EX); time.sleep(14)"`.
2. Call `create_file(path=..., content=..., workspace=<root>)`. The call is refused: "another codescout instance is writing to this project".
3. Read the newest row: `sqlite3 -readonly .codescout/usage.db "SELECT id, latency_ms, started_at, called_at FROM tool_calls WHERE tool_name='create_file' ORDER BY id DESC LIMIT 1"`.

## Environment
codescout on `experiments`, HEAD `931b975c5`, rebuilt server reconnected with `/mcp` on 2026-11-09.

## Root cause
`dispatch_tool_call` returns `Exit::unrecorded(...)` when `acquire_write_guard_if_writing` refuses (`src/server.rs`, the `Ok(Err(result))` arm). `record_unrecorded` then writes the row with no start clock. The wait happens inside `acquire_write_guard_if_writing`, before that point, so the wait is not in the row.

## Evidence
Measured 2026-11-09 in this project's `usage.db`, three refusals under a held lock:
- row 282413: `latency_ms` 0, `started_at` 07:36:03.724, `called_at` 07:36:03.725.
- row 282432: `latency_ms` 0, `started_at` 07:37:33.823, `called_at` 07:37:33.824.
- row 282441: `latency_ms` 0, `started_at` 07:38:04.390. The holder took the lock at 07:37:55.9 (`date`), and it still held the lock at that time. The row is stamped 8.5 s after the lock was taken.

Not measured: the exact wait of each call. The call start is not visible from outside the server. The code path shows a wait of up to `write_lock_timeout_secs`; no measurement shows that this wait happened in these three calls.

## Hypotheses tried
None. Cause read from `src/server.rs` only.

## Fix
Unknown. Likely: take a clock before `acquire_write_guard_if_writing` and carry it in `Unrecorded` so `record_unrecorded` can write `started_at` and `latency_ms`. Only the write-lock exit has a real wait; the other `Unrecorded` exits return at once.

## Tests added
N/A — not fixed. `a_refused_write_lock_leaves_one_row` asserts the family and the outcome; it does not assert `latency_ms`.

## Workarounds
Read `called_at` of the row beside `write_lock_timeout_secs`. The wait is not recoverable from the row alone.

## Resume
Add a start clock to `Unrecorded`, set it before the lock wait, and extend `a_refused_write_lock_leaves_one_row` to assert `latency_ms >= 900` as the timeout test does.

## References
- `docs/issues/archive/2026-11-08-access-gate-refusals-leave-no-usage-row.md`
- `docs/issues/archive/2026-11-08-cancelled-and-timed-out-calls-leave-no-usage-row.md`
