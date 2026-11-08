---
id: '34d094297c76ca4c'
kind: bug
status: open
title: a refusal by the access gate leaves no usage row, so usage.db cannot see it
tags:
- cluster/unclassified
opened: 2026-11-08
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
Not done. Candidate: record the gate refusal the way `refuse_pin` records a pin refusal (default project, best-effort, blocking pool, short budget), so the row carries the refusal text and an `err_family`. Open points: `normalize_err_family` in `src/usage/db.rs` has no family for these texts yet, and a refusal must not make the caller wait on SQLite's busy timeout.

## Tests added
None. This file records the defect only.

## Workarounds
Count refusals from the harness transcript, not from `usage.db`.

## Resume
Start from `refuse_pin` and its test in `src/server.rs`. A regression test: refuse one write under a read-only activation, then assert one `tool_calls` row with `outcome != 'success'`.

## References
`src/server.rs` (`call_tool_inner`, `refuse_pin`, `check_tool_access`); `src/usage/db.rs` (`normalize_err_family`); `docs/issues/archive/2026-11-08-read-only-refusal-makes-the-agent-resend-the-whole-write.md`; F-219 in `docs/trackers/bug-fix-session-log.md`.
