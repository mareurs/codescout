---
id: d71ec1b8c7edbe18
kind: bug
status: fixed
title: a refusal by the access gate leaves no usage row, so usage.db cannot see it
tags:
- cluster/unclassified
closed: 2026-11-08
opened: 2026-11-08
unverified: Not run on a live MCP server built from 6cb2aa615, only through call_tool_inner. A call cancelled by Escape and a call that hits tool_timeout_secs are not covered and leave no row today; that is read from race_against_cancel, not measured, and is not fixed here.
---

# BUG: a refusal by the access gate leaves no usage row, so usage.db cannot see it

## Summary
`call_tool_inner` answers a gate refusal (a write under a read-only activation, a write with `file_write_enabled = false`, indexing disabled) with `return Ok(*err)` before it builds the usage recorder. Those refusals never reach `tool_calls`. Every analysis that reads `usage.db` is blind to them.

## Class fit
No roster class fits without forcing it. The instrument (the usage recorder) is correct for the calls it sees; the defect is that one early-return path skips it. That is an observation gap in one code path, not a wrong decision, and the roster has no class for it.

## Symptom (Effect)
Measured live 2026-11-08, on a server rebuilt from commit `a96963c96`. A fresh directory was activated with `read_only=true`, then `create_file` was called twice and refused both times (`File writes are disabled: …`). The directory's own `usage.db` afterwards:

```
1|workspace|success|||177
2|create_file|success|||252      <- the ack replay, not a refusal
3|read_file|success|||19
```

Two refusals, no rows. A write refused this way is invisible to `analyze-usage` and to any query over `tool_calls`.

Earlier, 2026-11-08: no row with `error_msg LIKE 'File writes are disabled%'` in any `usage.db` under `~/work` larger than 100 KB.

## Reproduction
1. `workspace(action="activate", path=<fresh dir containing .codescout/>, read_only=true)`.
2. `create_file(path="x.txt", content="c")`. It is refused.
3. `sqlite3 -readonly <dir>/.codescout/usage.db "select tool_name, outcome from tool_calls"`. The refused call has no row.

## Environment
codescout on branch `experiments`, built from `a96963c96`, 2026-11-08. MCP stdio.

## Root cause
In `CodeScoutServer::call_tool_inner` (`src/server.rs`, lines as of `a96963c96`): the gate call `check_tool_access` is at line 1742, its refusal branch ends with `return Ok(*err)` at line 1774, and the recorder is built later, `UsageRecorder::new` at line 1852. The refusal returns before the recorder exists. Read this session. The two live refusals above agree with it.

The sibling early return for an unusable `workspace=` pin does record: `refuse_pin` builds a recorder and writes the row (`src/server.rs`, `UsageRecorder::new` at line 961), and says why in its doc comment ("a refused call that left none would have been invisible").

Which refusals take this path: the ones `check_tool_access` returns, that is a write block (`ActivatedReadOnly`, `ConfiguredOff`) and `indexing_enabled = false`. The worktree write guard runs inside the tool, so it is recorded (108 `worktree_activate_required` rows in this project's `usage.db`).

## Evidence
See Symptom. Both measurements are above; the second one is the live run.

## Hypotheses tried
1. **Hypothesis:** the recorder skips rows whose outcome is an error. **Test:** `tool_calls` holds `recoverable_error` and `error` rows in this and other projects. **Verdict:** rejected.

## Fix

Done in 6cb2aa615.

`call_tool_inner` is now a thin wrapper over `dispatch_tool_call`, which returns an `Exit` (`src/server.rs`). Every `return` in the dispatcher picks `Exit::Recorded` (the recorder wrote, or `refuse_pin` wrote the row itself) or `Exit::Unrecorded`; the wrapper writes the row for the second kind through `record_unrecorded`, then returns the answer unchanged. A new early return does not compile until it chooses. A guard with `Drop` was rejected: `Drop` cannot await, so a test could not read the row deterministically.

Exits now covered: the access gate, the write-lock refusal, and an unknown tool (still a protocol error). `refuse_pin` and `record_unrecorded` share one writer, `record_refusal_row`. The rendezvous poll is consumed once per call, so the result is handed on in `CallIdentity::rendezvous` and not polled twice.

Seven families were added to `normalize_err_family` and `ERR_FAMILIES` (`src/usage/db.rs`): `write_blocked_read_only`, `write_blocked_by_config`, `write_blocked_cause_unknown`, `indexing_disabled`, `write_lock_unavailable`, `ack_handle_refused`, `unknown_tool`. Each is decided by the message's fixed prefix.

Not covered, on purpose: a call cancelled by Escape and a call that hits `tool_timeout_secs`. Both drop the future that holds the recorder, and the cancel arm parks the task instead of returning. That needs its own change, and the claim that they leave no row today is read from `race_against_cancel`, not measured.

A call with no project to hold the row is skipped, as `refuse_pin` already did.

## Fix provenance

- **SHA:** `6cb2aa615` (`experiments`)
- **patch-id:** `cc8686a70e4a`

## Tests added

`src/server/exit_rows_tests.rs`, 5 tests through the real dispatch. Each reads the row back with a second connection and counts again after 300 ms, so a double write fails too:

- a read-only gate refusal leaves one row, family `write_blocked_read_only`
- a configured-off gate refusal leaves one row, family `write_blocked_by_config`
- a refused write lock leaves one row, family `write_lock_unavailable`
- an unknown tool stays a protocol error and leaves one row, family `unknown_tool`
- a refused `ack` handle leaves one row, family `ack_handle_refused`

Plus `usage::db::tests::the_dispatcher_refusals_have_a_family_whatever_their_data_says`, with hostile data (`heading Fix not found`, `index is locked`) in each message.

All 5 failed before the change, for the reason named: 0 rows, and the ack row with no family. After: 15 of 15 in the targeted run. `./scripts/gate.sh` 2026-11-08: FMT, CLIPPY, LEAN, DEFAULT and Python all exit 0, on the third run. The first run's LEAN lane failed 2 `prompts` tests; they passed (140 of 140) on the next run of the same tree, and the files involved (`src/prompts/source.md` and its fixture) were another session's uncommitted edits.

## Workarounds
Count refusals from the harness transcript, not from `usage.db`.

## Resume
Start from `refuse_pin` and its test in `src/server.rs`. A regression test: refuse one write under a read-only activation, then assert one `tool_calls` row with `outcome != 'success'`.

## References
`src/server.rs` (`call_tool_inner`, `refuse_pin`, `check_tool_access`); `src/usage/db.rs` (`normalize_err_family`); `docs/issues/archive/2026-11-08-read-only-refusal-makes-the-agent-resend-the-whole-write.md`; F-219 in `docs/trackers/bug-fix-session-log.md`.
