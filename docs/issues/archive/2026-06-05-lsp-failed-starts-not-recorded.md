---
status: fixed
opened: 2026-06-05
closed: 2026-06-05
severity: medium
owner: marius
related: []
tags: [lsp, observability, usage-db, telemetry, kotlin]
kind: bug
---

# BUG: failed LSP starts record no `lsp_events` row — a chronically-failing server is invisible to usage analytics

## Summary
`lsp_events` only records *completed* LSP handshakes. When a server dies during
`initialize` (e.g. an expired kotlin-lsp build → "LSP server disconnected"),
`do_start` returns `Err` and writes **nothing**. A server that fails every start
produces zero `lsp_events` rows, so `analyze-usage` and the dashboard see a
silent absence rather than a failure signal — the only trace is
`tool_calls.error_msg='LSP server disconnected'` plus the debug log.

## Symptom (Effect)
Investigating a dead kotlin LSP in `~/work/mirela/backend-kotlin` (2026-06-05):
every `symbols` on a `.kt` file errored

```
LSP server disconnected
```

and the debug log showed the server self-closing on launch. Yet `lsp_events` had
**no kotlin row** for the failing window — the most recent kotlin row predated the
failures by days. The failure was only discoverable by digging the debug log.

## Reproduction
1. Point a language at a command that exits on launch (or an expired LSP build).
2. Trigger any LSP-backed tool (`symbols` on that language).
3. `SELECT * FROM lsp_events WHERE language='<lang>'` → no row for the failed attempt.
   `SELECT * FROM tool_calls WHERE error_msg LIKE '%LSP server disconnected%'` → the only trace.

Pre-fix HEAD: the `experiments` commit immediately before this change.

## Environment
codescout v0.15.0; any LSP language; observed with kotlin-lsp `262.4739.0`
(expired build) on Arch Linux, MCP stdio transport.

## Root cause
`LspManager::do_start` (`src/lsp/manager.rs`) records an `lsp_events` row via
`write_lsp_event` **only in the `Ok(new_client)` arm**. The `Err(e)` arm handled
the circuit-breaker and returned `Err` without writing any row. `lsp_events` also
had no `outcome`/`error` column, so there was no representation for a failed start.

## Evidence
- `do_start` Ok-arm-only recording: `src/lsp/manager.rs` (success path calls
  `write_lsp_event`; the `Err` arm did not write anything).
- Live triage: `~/work/mirela/backend-kotlin/.codescout/usage.db` — `tool_calls`
  had `symbols`→`LSP server disconnected` errors at 13:34–13:41 UTC 2026-06-05;
  `lsp_events`'s last kotlin row was 2026-05-26.
- Debug log: `.codescout/debug.log` — `lsp_stderr: This build of intellij-server
  has expired.` (×3).

## Hypotheses tried
1. **Hypothesis:** the row simply wasn't flushed. **Test:** read `do_start`.
   **Verdict:** rejected — there is no failure write path at all (Ok-arm only).

## Fix
Implemented on `experiments` (cite the master-side SHA here after cherry-pick):
- Schema migration in `open_db` (`src/usage/db.rs`): `ALTER TABLE lsp_events ADD
  COLUMN outcome TEXT NOT NULL DEFAULT 'success'` + `error TEXT`, via the same
  ALTER-probe pattern `tool_calls` uses. `outcome` defaults to `'success'`, so the
  unchanged `write_lsp_event` INSERT and every pre-existing row stay correct.
- New `write_lsp_failure(conn, language, reason, handshake_ms, error)`
  (`src/usage/db.rs`) — inserts an `outcome='failed'` row with the error string and
  time-to-failure as `handshake_ms`.
- `do_start` `Err` arm (`src/lsp/manager.rs`) now records a failure row
  (best-effort, `spawn_blocking`, project-root-gated), independent of the
  cold-start-grace / circuit-breaker logic (those gate the breaker, not observability).
- `query_lsp_stats` aggregate now computes success metrics via `CASE WHEN
  outcome='success'` plus a per-language `failures` count, so success stats stay
  clean AND a fail-only language still appears (starts=0, failures>0). `recent` and
  `lsp_percentile` filter `outcome='success'`. The response gained a per-language
  `failures` field + a `recent_failures` list.
- Observability surfaces (consumers of the new data):
  - `analyze-usage` skill (`.claude/skills/analyze-usage/SKILL.md`): query H now
    filters `outcome='success'`; new query I lists failed starts; report + count updated.
  - Dashboard (`--features dashboard`): per-language Failures column + a "Failed
    Starts" panel (`src/dashboard/static/{dashboard.js,index.html}`).

## Tests added
- `usage::db::tests::write_lsp_failure_records_failed_outcome` (`src/usage/db.rs`)
- `usage::db::tests::write_lsp_event_defaults_outcome_to_success` (`src/usage/db.rs`)
- `usage::db::tests::query_lsp_stats_excludes_failed_starts` (`src/usage/db.rs`)
- `lsp::manager::tests::do_start_records_failure_event_when_start_fails`
  (`src/lsp/manager.rs`) — bogus binary → asserts an `outcome='failed'` row; needs
  no LSP installed, so it runs everywhere.
- `usage::db::tests::query_lsp_stats_surfaces_fail_only_language` (`src/usage/db.rs`)
  — a language with only a failed start still appears (starts=0, failures=1).
- `dashboard::routes::tests::lsp_response_exposes_failures_and_recent_failures`
  (`src/dashboard/routes.rs`) — `/api/lsp` JSON exposes per-language `failures` +
  `recent_failures`; runs only under `--features dashboard`.

## Workarounds
Query the failure signal directly (now that it is recorded):
```sql
SELECT language, error, COUNT(*) AS failures, MAX(started_at) AS last
FROM lsp_events WHERE outcome='failed' GROUP BY language, error ORDER BY failures DESC;
```

## Resume
N/A — fixed and verified. `cargo test --lib` green (2640) and the dashboard contract
test green under `--features dashboard`; clippy clean. Both originally-noted follow-ups
are DONE in this work stream: analyze-usage query I added; dashboard Failures column +
Failed Starts panel added. NOTE: dashboard code is feature-gated — test/lint it with
`--features dashboard`; default `cargo test --lib` skips it (see codescout memory
`dashboard-feature-gated-tests`).

## References
- Live triage: `~/work/mirela/backend-kotlin/.codescout/{usage.db,debug.log}`.
- Separate but related: kotlin-lsp build-expiry workaround (JetBrains kotlin-lsp
  issue #217, faketime) — that is why the failures were occurring.
- Touched: `src/usage/db.rs`, `src/lsp/manager.rs`.
