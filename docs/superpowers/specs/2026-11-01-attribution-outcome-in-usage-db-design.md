# Record the red-attribution outcome in `usage.db`, never in the response

Date: 2026-11-01. Size: a small change. Bug: `20927ccfe293c34e`.

## Intent and why

A red `run_command` may carry a `wip_authors` hint. When the engine passes its 30 s ceiling, `wip_author_diagnostic_in` returns `None` (`attribution.rs:584`). The test `a_timed_out_scan_is_silence_and_never_a_verdict` pins that. The silence is on purpose: a missing hint must never read as "nobody wrote it".

The silence has a cost. Nothing records that the scan ran out of time. `usage.db` shows no hint since 2026-10-20: 138 red runs with a diagnostic text, none hinted. The data cannot tell a timeout from a red that names no dirty file. The operator asked to read it and could not decide anything from it.

The operator chose: the timeout goes to the log and to `usage.db` only. The response stays as it is.

## Goals

- Every `run_command` call that starts the engine leaves one outcome and one elapsed time in its `usage.db` row.
- A timeout also writes one `tracing::warn!` line.
- A reader can compute, per day, how many attempts timed out and how close the others came to the ceiling.

## Non-goals and anti-goals

- No change to the response of any call. Not one byte.
- No change to the timeout value, to the engine, or to the cache.
- Anti-goal: a recorded outcome that a reader can take as a statement about ownership. The column says what the engine did, never who wrote a file.

## The design

One new task-local sink, built on the pattern of `usage::observed` (`OBSERVED_SINK`, drained by `record_content` into `observed_changed`).

- New module `src/usage/attribution_outcome.rs`, shaped like `observed.rs`: a sink, and `record(outcome, elapsed_ms)` that replaces any earlier value and does nothing outside a scope.
- Two new nullable columns on `tool_calls`, added by the same probe-and-`ALTER` that `open_db` uses for `observed_changed` (`SELECT … LIMIT 0`, then `ALTER TABLE … ADD COLUMN`):
  - `attribution_outcome TEXT`: one of `hinted`, `silent`, `timed_out`, `tooling_failure`, `spawn_failed`. NULL means the engine did not start (green command, opt-out, no diagnostic text).
  - `attribution_ms INTEGER`: wall time from spawn to the end of the wait.
- `wip_author_diagnostic_in` calls `record` at each exit after the spawn. The `timed_out` branch also logs `tracing::warn!` with the ceiling and the elapsed time.
- `usage/mod.rs` scopes the sink and passes the drained value down to `write_record` through `MeasurementLinkage`, as `observed_changed` does.

`silent` means the engine finished and printed nothing. It covers both an early exit (no dirty file named, 0.1 s) and a full scan with no output. The elapsed time tells them apart.

## Approaches considered

1. **A sink and two columns (chosen).** Same shape as `observed_changed`. A reader needs one query. Cost: a fourth place that grows `write_record` (it already carries nine parameters and an `allow`).
2. **Log line only.** Cheapest. Rejected by the operator: the log is not queryable beside the other calls.
3. **Pack the outcome into `deliveries_json` or `error_msg`.** No migration. Rejected: those columns have other meanings, and a reader would parse a field that holds two things.

## Proof the builder must produce

- Migration test, shaped like `open_db_adds_observed_changed_and_it_round_trips_three_states`: a pre-migration table gains both columns, the old row reads NULL, and each outcome and NULL survive a `write_record` round trip.
- `record` outside a scope is inert. A later `record` replaces an earlier one.
- Attribution tests, one per branch, each asserting the sink value and a plausible `attribution_ms`: `hinted`, `silent`, `timed_out` (through the existing timeout seam, `AttributionEnv.timeout`), `tooling_failure`, `spawn_failed` if a test can force it.
- The existing test that a timeout returns `None` still passes unchanged. A new assertion checks that the response bytes of a timed-out call are identical to those of a call with the sink out of scope.
- Mutant: drop the `record` in the timeout branch. A test must fail.
- `./scripts/gate.sh` in a clean worktree.

## Left to the builder

- The exact enum and column names, as long as the five states stay distinct from NULL.
- Whether `usage::observed` and the new module share a helper. Two sinks with the same shape do not yet justify one. This would be the third use, so extracting is allowed but not required.

## Risks and what I have not shown

- `attribution.rs` sets no `kill_on_drop` (a grep found none). I think a timed-out engine keeps running after the ceiling, and that is how a cold fill makes progress across calls. I have not verified this. If it is true, each timeout leaves a python process running for longer than 30 s.
- The 138-red count uses `-->` and `panicked at` as a stand-in for the engine's own prefilter. It is an approximation.
- One end-to-end sample exists: 18.5 s, binary `a3c21e540`. The `attribution_ms` column is what will turn that into a distribution.
