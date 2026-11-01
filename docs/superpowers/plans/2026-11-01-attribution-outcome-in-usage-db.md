# Record the red-attribution outcome in `usage.db` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `run_command` call that starts the red-attribution engine leaves its outcome and elapsed time in its `usage.db` row, and a timeout also writes one log line. No response changes.

**Architecture:** A task-local sink, `ATTRIBUTION_SINK`, copies the shape of `usage::observed::OBSERVED_SINK`. `attribution.rs` records into it. `UsageRecorder::record_content` scopes and drains it, and passes the value through `write_content` and `MeasurementLinkage` into two new nullable columns.

**Tech Stack:** Rust, `tokio::task_local!`, `rusqlite`, `tracing`.

**Spec:** `docs/superpowers/specs/2026-11-01-attribution-outcome-in-usage-db-design.md` (commit `cb17344c4`).

## Global Constraints

- The response of every call stays byte-identical. The timeout stays silent in the response.
- `ATTRIBUTION_TIMEOUT_DEFAULT`, the engine, and the history cache do not change.
- Outcome column values are exactly `hinted`, `silent`, `timed_out`, `tooling_failure`, `spawn_failed`. NULL means the engine did not start. No state may share a value with NULL.
- Column names: `attribution_outcome TEXT`, `attribution_ms INTEGER`. Both are nullable and added by the probe-and-`ALTER` pattern in `open_db`.
- Both columns are record-only. Nothing reads them back into behavior.
- Commit style: Conventional Commits, `(20927ccfe293c34e)` in the subject, trailer `Session-Id: 48d211eb-3bd8-4247-975b-631a73b1a87a`. Use the shared-checkout sequence in `docs/conventions/shared-checkout-commit-sequence.md`, in separate calls, `git commit -F <file> -- <pathspec>`, `timeout_secs=300`.
- Run `./scripts/gate.sh` in a clean worktree before the code commit. Targeted tests: `scripts/with-slot.sh cargo test …`.
- Nothing is pushed. Peers' staged and unstaged files stay untouched.

## Review Focus

1. **Response bytes.** A timed-out or hinted call returns the same value with the sink in scope and out of scope. Pinned in Task 3.
2. **NULL is not `silent`.** A green command (exit 0, nothing failure-shaped) and an opt-out start no engine and record nothing. A non-zero exit with plain text DOES start the engine and records `silent` (corrected after the final review: the first text said a red with no diagnostic text records nothing). Pinned in Task 3.
3. **No leak between calls.** A row that recorded `timed_out` must not colour the next call's row. Pinned in Task 2.
4. **Migration.** An old database opens, gains both columns, and a second open does not fail. Pinned in Task 2.
5. **Concurrent calls.** Two calls in flight each keep their own value. Pinned in Task 1.

---

### Task 1: The sink

**Files:**
- Create: `src/usage/attribution_outcome.rs`
- Modify: `src/usage/mod.rs` (add `pub(crate) mod attribution_outcome;` beside `pub(crate) mod observed;`, line 3)
- Test: the `tests` module of the new file

**Interfaces:**
- Produces, for Tasks 2 and 3:
  - `pub(crate) enum Outcome { Hinted, Silent, TimedOut, ToolingFailure, SpawnFailed }`, deriving `Debug, Clone, Copy, PartialEq, Eq`.
  - `impl Outcome { pub(crate) fn as_column(self) -> &'static str }` returning the five exact strings in Global Constraints.
  - `pub(crate) struct Recorded { pub outcome: Outcome, pub elapsed_ms: i64 }`, deriving `Debug, Clone, Copy, PartialEq, Eq`.
  - `tokio::task_local! { pub(crate) static ATTRIBUTION_SINK: RefCell<Option<Recorded>>; }`
  - `pub(crate) fn record(outcome: Outcome, elapsed_ms: i64)`: replaces any earlier value, does nothing outside a scope (`try_with`).

- [ ] **Step 1: Write the failing tests** in the new file's `tests` module, shaped like `observed.rs`'s:
  - `record_outside_a_scope_is_a_silent_no_op`: `record(Outcome::TimedOut, 5)` does not panic.
  - `a_later_record_replaces_an_earlier_one_and_an_unscoped_sink_starts_empty`: inside a scope, the value is `None`, then `Some(Recorded{Hinted, 7})`, then, after a second `record`, the second value.
  - `the_five_columns_values_are_exact_and_distinct`: `as_column` of the five variants equals `["hinted","silent","timed_out","tooling_failure","spawn_failed"]` in that order, and the set has five members.
  - `two_concurrent_scopes_keep_their_own_value`: with `tokio::join!` over two scopes that each `record` a different outcome around a `tokio::task::yield_now().await`, each drains its own.
- [ ] **Step 2: Run:** `scripts/with-slot.sh cargo test --lib attribution_outcome`. Expected: FAIL to compile (module missing).
- [ ] **Step 3: Implement** the interface above. Put a module doc beside the one in `observed.rs`: record-only, a statement about what the engine did and never about who wrote a file, the same no-`tokio::spawn` caveat.
- [ ] **Step 4: Run** the same command. Expected: 4 passed.

### Task 2: The columns and the drain

**Files:**
- Modify: `src/usage/db.rs` (`open_db` migration near line 256; `MeasurementLinkage` line 344; `write_record` INSERT line 411)
- Modify: `src/usage/mod.rs` (`record_content` line 86; `write_content` line 144)
- Test: `src/usage/db.rs` tests, `src/usage/mod.rs` `content_tests`

**Interfaces:**
- Consumes: Task 1's `Recorded`, `ATTRIBUTION_SINK`, `Outcome::as_column`.
- Produces:
  - `MeasurementLinkage` gains `pub attribution_outcome: Option<&'a str>` and `pub attribution_ms: Option<i64>`. The struct stays `Copy` and keeps its `Default`.
  - `write_content` gains a last parameter `attribution: Option<crate::usage::attribution_outcome::Recorded>`. The existing `#[allow(clippy::too_many_arguments)]` covers it.

- [ ] **Step 1: Write the failing tests.**
  - In `db.rs`, `open_db_adds_the_attribution_columns_and_they_round_trip`, copied in shape from `open_db_adds_observed_changed_and_it_round_trips_three_states`: a pre-migration table gains both columns; the old row reads NULL for both; six rows written through `write_record` (each of the five `as_column` values with an `attribution_ms`, and one with `None`) read back unchanged; a second `open_db` on the same path returns `Ok`.
  - In `mod.rs` `content_tests`, `record_content_stores_the_attribution_sink_value_and_null_when_the_engine_did_not_start`, shaped like `record_content_stores_the_observed_sink_value_and_null_when_unobserved`: a closure that calls `record(Outcome::TimedOut, 31_000)` stores `timed_out` and `31000`; a closure that records nothing stores NULL in both.
  - `the_attribution_sink_does_not_leak_into_the_next_call`, shaped like `the_observed_sink_does_not_leak_into_the_next_call`: call one records, call two does not, row two reads NULL.
- [ ] **Step 2: Run:** `scripts/with-slot.sh cargo test --lib usage::`. Expected: FAIL (columns missing, fields missing).
- [ ] **Step 3: Implement.**
  - Migration: one probe per column (`SELECT attribution_outcome FROM tool_calls LIMIT 0`, then `ALTER TABLE tool_calls ADD COLUMN …`). Put the meaning of each state in a comment on this migration, as `observed_changed` does, because a reader of the database lands there. State plainly: a value is what the engine did, never who wrote a file, and `silent` covers an early exit and a full scan with no output, which `attribution_ms` separates.
  - `write_record`: add the two columns and placeholders `?25`, `?26`; bind `measurement.attribution_outcome` and `measurement.attribution_ms`.
  - `record_content`: add a third `.scope(RefCell::new(None), …)` for `ATTRIBUTION_SINK` around the existing two, drain it inside, and pass it to `write_content`.
  - `write_content`: map the `Recorded` to `attribution_outcome: Some(r.outcome.as_column())` and `attribution_ms: Some(r.elapsed_ms)` in the `MeasurementLinkage` literal.
- [ ] **Step 4: Run** the same command. Expected: all `usage::` tests pass, including the three new ones.

### Task 3: Record at the engine's exits

**Files:**
- Modify: `src/tools/run_command/attribution.rs` (`wip_author_diagnostic_in`, line 538; add `finished_outcome` beside `engine_outcome`, line 433)
- Test: the `tests` module of the same file

**Interfaces:**
- Consumes: Task 1's `record` and `Outcome`.
- Produces: `fn finished_outcome(success: bool, answer: &Option<String>) -> Outcome`. A non-successful exit gives `ToolingFailure`; a successful one gives `Hinted` when `answer` is `Some`, else `Silent`. No string matching.

- [ ] **Step 1: Write the failing tests.** Each test that drives the engine wraps the call in `ATTRIBUTION_SINK.scope(RefCell::new(None), async { … })` and drains the sink inside it.
  - `finished_outcome_separates_hinted_silent_and_tooling_failure`: pure, three cases.
  - `a_timed_out_scan_records_timed_out_and_still_says_nothing`: the 1 ms fixture of `a_timed_out_scan_is_silence_and_never_a_verdict`. The sink holds `Outcome::TimedOut` and `elapsed_ms >= 1`; the return value is `None`.
  - `an_engine_that_answers_records_hinted`: the fixture of `a_red_naming_a_dirty_file_reaches_the_engine`, with its python3 skip rule. The sink holds `Hinted`.
  - `an_engine_with_nothing_to_say_records_silent`: a dirty fixture repo and a red that names a path that is not dirty. The sink holds `Silent`.
  - `an_engine_that_cannot_be_extracted_records_spawn_failed`: `wip_author_diagnostic_in` with the relative base `Path::new("attribution-outcome-relative-base-probe")` (refused, nothing created; see `a_relative_base_is_refused`). The sink holds `SpawnFailed`. Check that the path does not exist afterwards.
  - `a_call_that_starts_no_engine_records_nothing`: a green command with nothing failure-shaped, and the opt-out case, each leave the sink `None`.
  - `the_response_is_the_same_in_and_out_of_a_scope`: the timed-out case and, where python3 is present, the hinted case return equal values with the sink in scope and out of scope.
- [ ] **Step 2: Run:** `scripts/with-slot.sh cargo test --lib run_command::attribution`. Expected: the new tests FAIL (sink empty) or do not compile (`finished_outcome` missing).
- [ ] **Step 3: Implement in `wip_author_diagnostic_in`.**
  - Start an `Instant` after the opt-out gate and before `materialize_into`. This is my call: extraction takes a few ms, and an unrecorded first step would leave `spawn_failed` with no time.
  - `materialize_into` returns `None`, or `spawn` errs: `record(SpawnFailed, ms)`, then return `None` as now.
  - Split the `timeout` match. `Err(_)` (the ceiling passed): `record(TimedOut, ms)`, then `tracing::warn!` with the ceiling in ms and the elapsed ms, in words that say the missing hint is not evidence about who wrote the files. Return `None`. `Ok(Err(_))` (the wait itself failed): `record(ToolingFailure, ms)`, log a warn, return `None` as now.
  - Finished: compute `engine_outcome(...)` once, `record(finished_outcome(out.status.success(), &answer), ms)`, return `answer`.
- [ ] **Step 4: Run** the same command. Expected: all attribution tests pass, including the unchanged `a_timed_out_scan_is_silence_and_never_a_verdict`.
- [ ] **Step 5: Mutants.** Each must fail at least one test; restore after each.
  - Delete the `record(TimedOut, …)` call: `a_timed_out_scan_records_timed_out_and_still_says_nothing` fails.
  - Swap `Hinted` and `Silent` in `finished_outcome`: its test and `an_engine_that_answers_records_hinted` fail.
  - Return the `warn!` text in the response: `the_response_is_the_same_in_and_out_of_a_scope` fails.
- [ ] **Step 6: Gate and commit.** Run `./scripts/gate.sh` in a clean worktree (`git worktree add --detach … HEAD`, apply the diff, never `--force` on removal). Every lane must exit 0. Commit Tasks 1 to 3 in one commit: `feat(usage): record the red-attribution outcome and its time in usage.db, never in the response (20927ccfe293c34e)`.

### Task 4: Record it in the bug file

**Files:**
- Modify: `docs/issues/2026-10-31-eight-lib-tests-fail-under-16-to-1-cpu-oversubscription-through-wall-clock-windows.md`

- [ ] **Step 1:** With `doc(action="update", id="20927ccfe293c34e", patch={body_edits:[…]})`, add what was measured this session and what this change adds: the warm phase table (12.8 s; shell `write_targets` 5.81 s), the whole-call timings (2.8 to 5.4 s with a floor, 13.5 to 14.7 s without), the end-to-end sample (18.5 s on `a3c21e540`), the `usage.db` finding (0 hints since 2026-10-20 against 138 red runs with a diagnostic text, and why it cannot be read), and the commit SHA and patch-id of Task 3's commit. State what is not shown: the daily distribution, until the columns have data.
- [ ] **Step 2:** Commit with the shared-checkout sequence: `docs(issues): 20927ccfe293c34e records the attribution outcome columns and the measurements behind them`.

## After the plan (operator steps, not tasks)

- `./scripts/rb.sh`, then `/mcp`.
- Run a red that names a dirty file. Then read the row: `select attribution_outcome, attribution_ms from tool_calls where tool_name='run_command' order by id desc limit 1;`. Expected: `hinted` and a time in ms.
- After a few days: `select substr(called_at,1,10) d, attribution_outcome, count(*), max(attribution_ms) from tool_calls where attribution_outcome is not null group by 1,2;`.
