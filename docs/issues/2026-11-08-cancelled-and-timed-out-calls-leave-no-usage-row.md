---
id: d7315a96632ef30f
kind: bug
status: open
title: a call cancelled by Escape or timed out by tool_timeout_secs leaves no usage row
tags:
- cluster/unclassified
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
Not done. Two exits, two designs, and one open vocabulary question.

- **Timeout.** The error text is known where it is built. A typed error (for example `ToolTimedOut`) that `dispatch_tool_call`'s `Err(e)` branch can downcast would let it record once, through `record_refusal_row`, without double-writing the errors `record_content` already wrote. It needs a family (`tool_timeout`) in `normalize_err_family` and `ERR_FAMILIES`.
- **Cancel.** The cancel arm of `race_against_cancel` can `await` before it parks, so it can write the row there, with the input and identity passed in. The call's side effects may have partly run, so the row is worth having.
- **Open:** a cancelled call has no `outcome` today (`success`, `recoverable_error`, `error`). A new `cancelled` value is counted as friction by `write_content` (`outcome != "success"`), and any query that counts non-success rows would change. That decision is the user's.

## Tests added
None. This file records the defect only.

## Workarounds
Read cancellations and timeouts from the harness transcript, not from `usage.db`.

## Resume
Start from `race_against_cancel` and the `Exit` type in `src/server.rs`, and from `src/server/exit_rows_tests.rs` for the test shape. The probe's shape is in Symptom: inject a slow `Tool` into `server.tools`, and spawn `call_tool_inner` for the cancel case.

## References
`src/server.rs` (`race_against_cancel`, `dispatch_tool_call`, `record_refusal_row`); `src/usage/mod.rs` (`record_content`, `write_content`); `docs/issues/archive/2026-11-08-access-gate-refusals-leave-no-usage-row.md`.
