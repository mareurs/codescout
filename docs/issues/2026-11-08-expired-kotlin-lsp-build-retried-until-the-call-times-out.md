---
id: '9176f317573ff01b'
kind: bug
status: zombie
title: An expired Kotlin LSP build is retried ~58 times in one symbols call
owners:
- marius
tags:
- cluster/unclassified
last_observed: 2026-11-04
opened: 2026-11-08
severity: medium
unverified: Why the circuit-breaker did not stop ~58 starts is not established (no project_root in lsp_events; 2026-11-03 logs rotated). That the 2026-11-03 directory held ~58 files is not verified. That called_at marks call end is assumed. The fix for the per-file restarts has no measurement on a real failing server.
---

# BUG: an expired Kotlin LSP build is retried ~58 times in one `symbols` call (104.6 s), and the error names the wrong cause

## Summary
On 2026-11-03 a `symbols` call on backend-kotlin took 104.6 s, the slowest codescout call of the week. In the same 104 s window `lsp_events` records about 58 failed Kotlin starts, one every ~1.5 s. The server printed `This build of kotlin-server has expired`, which is a permanent failure, but codescout kept starting it. The error text that reaches the caller points at a mux lock instead.

## Class fit
No roster class fits cleanly. The defect is an unbounded retry of a permanent failure plus an error hint that names a different cause, which is not a doc contradicted by code, a config that is additive, or an assertion satisfiable by accident. Parked in `cluster/unclassified` until a class for retry-without-classification exists.

## Symptom (Effect)
The `symbols` call of 2026-11-03 19:17:54 ended after 104587 ms. Input:

```
{"include_body":true,"name":"withInnovaplanErrorContext","path":".worktrees/room-bookings/ktor-server/src/main/kotlin/edu/planner/integration/innovaplan"}
```

The error stored in `lsp_events` for the start of 2026-11-03 19:16:11 (verbatim, shortened to the relevant lines):

```
INFO mux::server_stderr: This build of kotlin-server has expired.
INFO mux::server_stderr: The IDE will now close.
INFO mux::server_stderr: Please download a new build from https://github.com/kotlin/kotlin-lsp
Error: failed to read message during initialize handshake
Caused by:
    EOF while reading message headers — hint: Check that another codescout mux isn't already running for this workspace and that the lock-file directory is writable; ...
```

## Reproduction
Not yet reproducible: the build has since been replaced (no failed Kotlin start after 2026-11-04 11:09). Best lead: start `kotlin-lsp` with a build past its expiry date, then call `symbols(name=..., path=<dir>)` on a Kotlin project and time it.

## Environment
- Project: `/home/marius/work/mirela/backend-kotlin`, worktree `.worktrees/room-bookings`.
- Date: 2026-11-03. Branch of codescout at that time: unknown.
- Transport: mux (`mux process failed to start`).

## Root cause
Partly established. The loop below drives the repeated starts; why the circuit-breaker did not stop them is Unknown — see Hypotheses tried.

Mechanism (read 2026-11-09, `search_files_restricted` in `src/tools/symbol/symbols.rs`): the branch of `symbols(name, path=<dir>)` calls `ctx.lsp.get_or_start` once per file inside a `for` loop. On `Err` it recorded the file as unread and went on to the next file, so a server that cannot start paid one failed start per file. A stub provider reproduces it (5 starts for 5 files). Not verified: that the 2026-11-03 directory held about 58 files. The worktree is gone, and the same directory holds 78 `.kt` files in the main tree today.

Facts read in code: `CIRCUIT_BREAKER_MAX_FAILURES = 5` and `CIRCUIT_BREAKER_WINDOW = 60 s` (`src/lsp/manager.rs:789-792`); the counter `startup_failures` is keyed by `LspKey` (language plus `project_root`) (`src/lsp/manager.rs:99`); failures inside the `cold_start_until` grace are not counted (`src/lsp/manager.rs:1645-1655`). With one key, 58 starts in ~100 s should not pass the breaker. Nothing in the code classifies "build has expired" as permanent.

Measured 2026-11-08: `sqlite3 backend-kotlin/.codescout/usage.db` on `lsp_events` and `tool_calls`. 111 failed Kotlin starts from 2026-11-01 to 2026-11-04 (1, 7, 76, 27 per day), none after 2026-11-04 11:09. The first failed start of the burst (19:16:11) is ~104 s before the call ended (19:17:54). That `called_at` marks the end of the call is assumed, not verified.

## Evidence
### `lsp_events` of the burst
About 58 rows between 2026-11-03 19:16:11 and 19:17:54, all `kotlin new_session failed`, `handshake_ms` between 1292 and 1853.

### Logs
Not available: `debug.log*` in backend-kotlin start on 2026-11-08, so the 2026-11-03 logs are rotated away. No `circuit-breaker` line exists in them.

## Hypotheses tried
1. **Several `LspKey`s were involved** (one per worktree root), so each had its own counter. Test: count distinct keys. Not testable: `lsp_events` has no `project_root` column and the logs are gone. Status: open.
2. **The cold-start grace suppressed counting.** Test: none run. Status: open. Weak: no start succeeded in the window, and the grace is set only on success.
3. **The call re-entered `get_or_start` after the window expired.** 58 starts in ~100 s is more than 5 per 60 s, so this alone does not explain it. Status: unlikely.
4. **One start per file in `search_files_restricted`.** Test: a stub provider that counts start requests, 3 `.rs` and 2 `.py` files. Result: 5 starts before the change, 2 after. Confirmed as the driver of repeated starts. It does not explain why the breaker stayed closed.

## Fix
Partly done. The per-file restarts are fixed in `search_files_restricted`: a language whose start failed is not started again inside the call, and its remaining files are listed as unread with the same reason. Commit `14fc2d669` on `experiments` (patch-id `08cbd9b89b0392553db036b88846fd563236103a`). The classification is done in commit `64aec28c2` on `experiments`: `mux_failure_is_expired_build` (both "This build of" and "has expired" must appear) gives `mux_failure_report` its own hint, and `get_or_start` returns that error as it is, beside the index-contention guard and before the generic refusal whose hint ("Retry in a moment ... fuser") named a different cause. Not done: a retry circuit-breaker keyed on this classification, and the open question of why the existing breaker did not stop ~58 starts. The hint does not carry the download URL; the server's own stderr, kept in the message, does. In backend-kotlin's usage.db 492 of 724 recorded Kotlin start failures carry the expiry text; the other 232 have another cause and keep the generic hint.

## Tests added
`a_restricted_search_starts_a_failing_server_once_per_language_not_once_per_file` in `src/tools/symbol/tests.rs`. It failed first with `["python","python","rust","rust","rust"]` against `["python","rust"]` and passes now. It also pins the "and N more files" warning. It does not cover the circuit-breaker. For the classification (commit `64aec28c2`, `src/lsp/manager.rs`): `mux_failure_is_expired_build_detects_the_servers_own_message`, `an_expired_build_report_names_the_expiry_and_not_the_lock` (also pins that the classification survives `RecoverableError`'s Display, which is what `get_or_start` re-reads) and `get_or_start_returns_an_expired_build_error_without_a_direct_fallback` (a fake mux prints the expiry: one spawn, the cause in the error, one failed start recorded). All three failed on assertions first, with the detection stubbed to false. The last one pins the guard, not the production refusal after it, because a test runner is the one place the direct fallback is still allowed.

## Workarounds
Install a current kotlin-lsp build. After this, no failed start was recorded.


## Status: zombie
The user reported on 2026-11-08 that the expired kotlin-lsp build was a one-time problem and was resolved. `lsp_events` agrees: no failed Kotlin start after 2026-11-04 11:09. The root cause of the unbounded retry (why the circuit-breaker did not stop ~58 starts) is still not established. Re-open if a failed-start burst appears again in `lsp_events` (query: `outcome='failed'` grouped by day), for any language.

## Resume
First measure which hypothesis holds: add `project_root` to `lsp_events` or log the `LspKey` on each failure, then reproduce with an expired build.

## References
- `src/lsp/manager.rs:99`, `:197`, `:789`, `:1628-1690`
- Slow-call scan of 2026-11-01..2026-11-08 (this session)
