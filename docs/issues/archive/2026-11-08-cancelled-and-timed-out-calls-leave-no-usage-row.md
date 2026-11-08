---
id: 27ed8d5fbc627324
kind: bug
status: fixed
title: a call cancelled by Escape or timed out by tool_timeout_secs leaves no usage row
tags:
- cluster/unclassified
closed: 2026-11-08
opened: 2026-11-08
---

# BUG: a call cancelled by Escape or timed out by tool_timeout_secs leaves no usage row

## Summary
`usage.db` has no row for a call that the caller cancelled (Escape) or that hit the server's `tool_timeout_secs`. The recorder writes after the tool body returns, and both exits drop the future that holds the recorder. The slowest calls, and the ones the caller gave up on, are the ones missing.

## Class fit
No roster class fits without forcing it. The recorder is correct for a call that finishes; the defect is two exits that abandon the call before the recorder's write, so the observation is missing for exactly the calls that took longest. That is an observation gap in two code paths, not a wrong decision, and the roster has no class for it.

## Symptom (Effect)
Measured 2026-11-08 with a temporary probe (not committed), through `call_tool_inner` on the tree at `c849f9a0e`:

```
control   tree                              -> 1 row  (success)
cancel    run_command "sleep 5", cancelled after 1 s
            task_finished = false           -> 0 rows, and still 0 rows 5 s later
timeout   injected tool sleeping 5 s, tool_timeout_secs = 1
            caller got Ok(is_error=true)    "Tool 'slow_probe' timed out after 1s. Increase tool_timeout_secs ..."
                                            -> 0 rows
```

The control shows the reader and the recorder work. The timeout is visible to the caller as an error and invisible to `usage.db`. The cancelled task never finishes: it stays parked.

## Reproduction
1. Set `tool_timeout_secs = 1` in a project's `.codescout/project.toml`. Call a non-exempt tool that takes longer than that (the probe injected one into `server.tools`; `run_command`, `index` and `librarian reindex` are exempt, see `tool_skips_server_timeout`).
2. `sqlite3 -readonly .codescout/usage.db "select tool_name, outcome from tool_calls"`: no row for the call.
3. For cancel: start `run_command "sleep 5"`, cancel its token after 1 s, wait 7 s, and query the same table.

## Environment
codescout on branch `experiments`, tree at `c849f9a0e`, 2026-11-08. The probe ran under `cargo test --lib`, not through a live MCP client.

## Root cause
`tool_call_fut` is `recorder.record_content(.., || tool.call_content(..))`, wrapped in task-locals and handed to `race_against_cancel` (`src/server.rs`). `record_content` writes its row after `f().await` (`src/usage/mod.rs`).

- **Timeout:** `tokio::time::timeout(secs, tool_call_fut)` drops the future when it expires, so `record_content`'s write never runs. The "timed out after Ns" error is built in `race_against_cancel`, outside the recorder.
- **Cancel:** the `select!` drops `tool_call_fut` when the token fires, then parks the task with `pending()`. The write never runs, and the `dispatch_tool_call` frame stays alive, which is why a guard with `Drop` would not fire either.

Read from the source; both outcomes were then measured as above.

## Evidence
See Symptom. In this project's `usage.db` the only rows matching `timed out after` are 2 LSP-internal timeouts (`references`, 2026-10-31), not the server's own timeout, which agrees with the probe.

## Hypotheses tried
1. **Hypothesis:** the recorder skips rows whose outcome is an error. **Test:** the control above, and `tool_calls` already holds `error` and `recoverable_error` rows. **Verdict:** rejected.

## Fix

**Timeout: done, committed as `655dab401`.** The caller's answer is unchanged; the row is new.

- `race_against_cancel` now builds a typed `ToolTimedOut { tool, secs }` instead of a bare `anyhow!`. Its `Display` is the same text the caller has always got.
- `dispatch_tool_call` takes the start time and a clock beside the race. When the race ends in a `ToolTimedOut`, it writes the row through `UsageRecorder::record_abandoned` (`src/usage/mod.rs`), which takes the latency from the caller. A row written from inside `record_content` would say about 0 ms for a call held for the whole timeout. An error the tool returned is not a `ToolTimedOut`, so it is not written twice.
- New family `tool_timeout` in `normalize_err_family` and `ERR_FAMILIES`, decided by the fixed prefix (`Tool '`) and the fixed closing sentence, with a hostile tool name in the test.

**Cancel (Escape): not done, by decision.** The user decided on 2026-11-08 not to record it. Escape is an act of the user, not a decision of codescout: the server did not refuse, fail or time out, so there is no server outcome to record. A design had been agreed earlier (`outcome = 'error'`, a new family `tool_cancelled`, written in the cancel arm of `race_against_cancel` before the task is parked), and it is not built. A reader of `usage.db` must know that a cancelled call has no row; the Workarounds section says where to look.

## Tests added

Timeout, in `src/server/exit_rows_tests.rs`: `a_server_timeout_leaves_one_row_with_the_time_it_waited` injects a tool that sleeps 5 s into `server.tools`, with `tool_timeout_secs = 1`. It asserts the caller still gets `Tool 'slow_test_tool' timed out after 1s.`, then exactly one row (counted again after 300 ms, so a double write fails) with `outcome = 'error'`, `err_family = 'tool_timeout'` and `latency_ms` between 900 and 4000. It failed before the change with no rows and the precondition passing. A `tool_timeout` case with a hostile tool name was added to `usage::db::tests::the_dispatcher_refusals_have_a_family_whatever_their_data_says`.

After: 16 of 16 in the targeted run. `./scripts/gate.sh` 2026-11-08: FMT, CLIPPY, LEAN and DEFAULT exit 0. Its Python lane failed 2 timing tests (`tests/test_phase1b_stage2_shell.py`, a 10 s subprocess timeout) while the machine's load average was 52; that file passes alone (8 of 8), and `python3 scripts/python-tests.py --lane light` passes on its own (1764 passed).

Cancel: none, by decision.

## Workarounds

A timeout has a row now (`err_family = 'tool_timeout'`). For a cancelled call, read the harness transcript: `usage.db` has no row for it.

## Resume

Done for the timeout. Cancel is declined, so there is nothing to resume.

## Fix provenance

- **SHA:** `655dab401` (`experiments`)
- **patch-id:** `3b972669eee9`

The patch-id covers the whole commit. It carries the timeout row only.

## References
`src/server.rs` (`race_against_cancel`, `dispatch_tool_call`, `record_refusal_row`); `src/usage/mod.rs` (`record_content`, `write_content`); `docs/issues/archive/2026-11-08-access-gate-refusals-leave-no-usage-row.md`.
