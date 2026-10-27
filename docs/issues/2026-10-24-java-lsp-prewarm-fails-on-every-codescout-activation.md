---
id: '191ec294d551ba58'
kind: bug
status: investigating
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

**Established 2026-10-27: `jdtls` cannot run under the system JDK 27.** `~/.local/bin/jdtls` is a launcher that uses whatever `java` the environment resolves. On 2026-09-30 `pacman -Syu` upgraded `jdk-openjdk 26.0.2.1.u1-1 -> 27.u35-1` (`/var/log/pacman.log:8413`, 13:06:59+0300, which is 10:06:59Z) and repointed `/usr/lib/jvm/default-runtime` to `java-27-openjdk`. The `jdtls` install dates from February and May and did not change.

Run by hand with the same launcher, under JDK 27, the process exits with code 13 and `.metadata/.log` holds `java.lang.IllegalArgumentException: Unsupported class file major version 71` from `org.apache.aries.spifly.dynamic.bundle`. Class-file version 71 is JDK 27, and `jdtls`'s bundled bytecode reader does not know it. The same command under `/usr/lib/jvm/java-21-openjdk` (JAVA_HOME and PATH set) stayed up until my 60-second timeout (exit 124) with 0 occurrences of that error.

**Why it retries on every activation.** `LspManager.startup_failures` is an in-memory map, and the breaker trips at 5 failures within 60 s (`CIRCUIT_BREAKER_MAX_FAILURES`, `CIRCUIT_BREAKER_WINDOW` in `src/lsp/manager.rs`). Each activation is a new process that pre-warms once, so its count reaches 1 and never trips. The failure is never remembered across processes. `grep startup_failures src/` finds no persistence.

**Not established:** the 13 `java` failures between 2026-08-18 and 2026-09-11 (before the JDK upgrade) have no known cause. Nothing here ties them to JDK 27.

## Evidence

Daily java failures (successes are 0 every day): 10-03 5, 10-05 20, 10-13 21, 10-14 31, 10-16 21, 10-23 23 (full table in the 2026-10-24 sweep).


**Correction to the Summary's "0 successes".** That holds for the 2026-10-03..10-24 window only. All-time, `lsp_events` holds 1,961 `java` successes (average handshake 2,198 ms, last at 2026-09-30 09:40:47Z) and 350 failures (average 15,992 ms). The first failure of the current run is 2026-09-30 10:44:46Z, 37 minutes after the JDK 27 upgrade. `started_at` is UTC (`datetime('now')`).

`/usr/lib/jvm` holds `java-21-openjdk` and `java-27-openjdk`. `java -version` resolves to `27.0.1.0.3`.

## Hypotheses tried

- Missing workspace data dir: ruled out. A fresh `-data` directory gets as far as the version error.
- JDK version: confirmed. JDK 27 fails, JDK 21 does not, with the identical command and a fresh data directory.
- Fixture layout: not tested, and not needed to explain the failure.

## Fix

Not started. Three independent options. None is applied.

1. **Machine fix (no code).** Run codescout with JDK 21 on `PATH` or `JAVA_HOME`, or upgrade `jdtls` to a build that reads class-file version 71. This stops the failures here and nothing else.
2. **Remember a failed start across processes.** Skip the pre-warm for a language whose last K starts in this project all failed, and retry on explicit use. The in-memory breaker cannot do this (see Root cause). The `lsp_events` table already records each failed start, so it is one possible store, and the mem-kill marker file next to the mux lock is another.
3. **Let the project name the JDK.** `LspLangOverride` carries only `mux`. An `env` override for the language would let `[lsp.java]` set `JAVA_HOME` per project, and `LspServerConfig` already has an `env` field.

## Tests added

None yet.

## Workarounds

None needed for correctness; the cost is CPU, memory and telemetry noise.

## Resume

Cause is known. What remains is a choice of fix (the Fix section lists three). Decide before coding: a cross-process failure memory is a design question, so use `buddy:architecture-snow-lion` first. Re-check after any JDK or `jdtls` change with `SELECT date(started_at), outcome, count(*) FROM lsp_events WHERE language='java' GROUP BY 1,2 ORDER BY 1 DESC LIMIT 5`.

## References

- `docs/usage-reports/2026-10-24-usage-analysis.md` § LSP
