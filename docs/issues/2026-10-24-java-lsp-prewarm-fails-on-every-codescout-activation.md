---
id: '191ec294d551ba58'
kind: bug
status: fixed
title: Java LSP pre-warm fails on every activation of the codescout project (235 of 235 starts, ~16.6 s each)
tags:
- cluster/unclassified
closed: 2026-10-27
fix_patch_id: f1c9570384ec1dbc4850ed5d44e5cd495ed38bb3
fix_sha: ce424fa47 (experiments)
opened: 2026-10-24
owner: marius
severity: low
unverified: No rebuilt binary has been observed skipping a real pre-warm, and no real activation has been run end to end. This project's project.toml does not set [lsp.java.env], so jdtls still fails here. The 3-failure and 6-hour thresholds are untested guesses.
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

Fixed on `experiments` in `ce424fa47` (patch-id `f1c9570384ec1dbc4850ed5d44e5cd495ed38bb3`). Two of the three options below were built. The third is a machine change that stays with the operator.

1. **Remember a failed start across processes. Built.** `usage::db::lsp_start_history` counts a language's `outcome='failed'` rows since its latest success and ages the newest. `prewarm_lsp_background` skips a language after `PREWARM_BACKOFF_AFTER_FAILURES` (3) consecutive failed starts whose newest is under `PREWARM_BACKOFF_WINDOW_SECS` (6 h) old, so a deterministic failure costs one attempt per window. Only the optional warm-up is skipped: a tool call that needs the server still starts it. An unreadable history means "pre-warm as before". The history is read from the activated project's `usage.db`, and `LspManager::record_lsp_failure` writes to the manager's own `project_root`. The two are the same store only while the manager's root is the project being pre-warmed. If they differ, the back-off does not engage and the old behaviour remains.
2. **Let the project name the JDK. Built.** `[lsp.<language>.env]` in `project.toml` adds environment variables to that language's server. `server_config_for` in `src/lsp/manager.rs` reads them from the config of the workspace root being started, not from the session's active project. A key the server already sets is kept and a warning names it, because kotlin's `JAVA_TOOL_OPTIONS` carries the per-workspace `-Duser.home`. Documented in `docs/manual/src/configuration/project-toml.md`. Verified by hand that `JAVA_HOME=/usr/lib/jvm/java-21-openjdk` alone keeps `jdtls` up.
3. **Machine fix. Not applied.** This project's `.codescout/project.toml` has no `[lsp.java.env]`, and the system `java` is still JDK 27. Until the operator sets `JAVA_HOME` there (or upgrades `jdtls`), `jdtls` still fails here. The back-off stops the retries and does not make it start.

The numbers 3 and 6 h are guesses, not measurements.

## Tests added

- `lsp_start_history_counts_failed_starts_since_the_last_success` (`src/usage/db.rs`). Counts one language only, restarts at a success, and ages the newest failure. RED was `left: 0, right: 2`.
- `prewarm_tests::a_language_whose_recent_starts_all_failed_is_not_pre_warmed` and `the_pre_warm_still_runs_when_the_history_does_not_justify_skipping` (`src/lsp/mod.rs`). They drive `prewarm_lsp_background` with a provider that counts starts and always fails. They cover the threshold, another language's failures, a success in between, and a failure older than the window. RED was `left: 1, right: 0`.
- `lsp_section_parses_a_per_language_env` (`src/config/project.rs`). Key order, and `mux` still parses beside `env`.
- `a_project_env_override_reaches_the_server_config_and_never_replaces_a_built_in_key` (`src/lsp/manager.rs`). It checks the real `server_config_for`, the kotlin built-in key kept, and a missing or unparseable `project.toml`. RED was `got []`.

The full `./scripts/gate.sh` was green (`FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`, `PYTHON_LIGHT=0`). It ran in an isolated worktree holding `HEAD` plus this patch, because a peer's uncommitted edit to `src/tools/grep.rs` fails the format check in the shared tree. The staged diff and the gated patch have the same patch-id.

## Workarounds

Until the fix runs here: set `[lsp.java.env] JAVA_HOME = "/usr/lib/jvm/java-21-openjdk"` in `.codescout/project.toml` (needs the new build), or set `JAVA_HOME` to JDK 21 in the environment of the process that runs codescout. `JAVA_HOME` alone keeps `jdtls` up by hand. Putting JDK 21 first on the `PATH` without `JAVA_HOME` was not tested.

## Resume

Done in code. To close the caveat: rebuild, run `/mcp`, activate this project twice, and read `SELECT started_at, outcome FROM lsp_events WHERE language='java' ORDER BY id DESC LIMIT 5`. After 3 failed rows, the next activation should add none. Then set `[lsp.java.env]` and confirm a `success` row appears.

## References

- `docs/usage-reports/2026-10-24-usage-analysis.md` § LSP
