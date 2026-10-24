---
id: '191ec294d551ba58'
kind: bug
status: open
title: Java LSP pre-warm fails on every activation of the codescout project (235 of 235 starts, ~16.6 s each)
tags:
- cluster/unclassified
opened: 2026-10-24
owner: marius
severity: low
---

# BUG: Java LSP pre-warm fails on every activation of the codescout project (235 of 235, ~16.6 s each)

## Summary

`lsp_events` in the codescout `usage.db` records **235 `java` starts in 2026-10-03..10-24, all `outcome='failed'`, error `LSP server disconnected`**, average handshake 16,624 ms, max 24,047 ms, 0 successes, every day of the window. `debug.log` shows the caller is the activation pre-warm: `LSP pre-warm for java skipped: LSP server disconnected` (2026-10-24T03:33:35Z), preceded by `Starting LSP server: jdtls []` and `LSP server spawned pid=… binary=jdtls`. `jdtls` resolves to `/home/marius/.local/bin/jdtls`. The project lists `java` among its languages (fixture files), so each activation spawns a JVM that dies.

## Class fit

No class in the roster fits a start-up that fails deterministically and is retried on every activation; it is not a lazy warm-up billing a caller (the pre-warm is off the request path) and not a cap or a selector defect.

## Symptom (Effect)

About 65 minutes of cumulative JVM start-up (235 × 16.6 s) over three weeks, plus the memory of each JVM, on a machine that hit `No space left on device` / `Disk quota exceeded` several times in the same window. The failures also dominate the `lsp_events` failure signal for this project (235 of 359 failed starts), which hides real failures behind a constant.

## Reproduction

`workspace(action="activate", path="/home/marius/work/claude/codescout")`, then `sqlite3 .codescout/usage.db "SELECT outcome, error, handshake_ms FROM lsp_events WHERE language='java' ORDER BY id DESC LIMIT 3"`.

## Environment

Linux, jdtls at `~/.local/bin/jdtls`, `/usr/bin/java`.

## Root cause

Unknown — under investigation. Not established: why jdtls disconnects (missing workspace data dir, JDK version, or the fixture layout), and whether pre-warm should skip a language whose last N starts all failed.

## Evidence

Daily java failures (successes are 0 every day): 10-03 5, 10-05 20, 10-13 21, 10-14 31, 10-16 21, 10-23 23 (full table in the 2026-10-24 sweep).

## Hypotheses tried

None yet.

## Fix

Not started. Two independent options: fix the jdtls launch; and make pre-warm back off (skip a language after K consecutive failed starts in this project, re-try on explicit use).

## Tests added

None yet.

## Workarounds

None needed for correctness; the cost is CPU, memory and telemetry noise.

## Resume

Start at the pre-warm call site (`grep "LSP pre-warm for" src/`), then run jdtls by hand with the arguments `src/lsp/` passes and read its stderr.

## References

- `docs/usage-reports/2026-10-24-usage-analysis.md` § LSP
