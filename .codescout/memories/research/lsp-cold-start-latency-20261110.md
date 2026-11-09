# symbols / references latency — where the slow tail comes from (usage.db, last 7 days to 2026-11-10)

Source: `usage.db` of every project under `~/work` (1,025 databases), tool `run_command` excluded. Recorded latencies, not new timings: the machine was saturated when this was written (load average 54 to 63 on 64 cores), so no new measurement was taken, and the recorded values carry whatever load existed when each call ran. Read them as rough.

## Ranking by total time

| tool | calls | total | average | what dominates |
|---|---|---|---|---|
| `references` | 445 | 5,817 s | 13.1 s | 93% (5,401 s) is `hce-data` benchmark testbeds (fresh checkouts, 16.6 s per call). The codescout project itself: 90 calls, 381 s, 4.2 s average. |
| `symbols` | 5,733 | 2,699 s | 471 ms | A tail. In the codescout project 2,289 of 2,854 name searches take under 100 ms; 83 calls take 5 s or more. |
| `librarian` | 131 | 542 s | 4.1 s | Mostly `doctor` (see [[research/doctor-latency-20261109]]). |
| `grep` | 7,106 | 731 s | 103 ms | Addressed earlier. |

## The `symbols` slow tail in the codescout project (81 calls of 5 s or more)

| workspace of the call | slow calls | slow seconds |
|---|---|---|
| fresh `/tmp` checkout | 11 | 297 (about 27 s each) |
| linked worktree | 29 | 222 |
| home, no workspace field | 22 | 212 |
| main checkout, pinned | 19 | 138 |

## Cold start is a cause, not the only one

- A rust-analyzer cold start answers `documentSymbol` with `null` for 8 to 54 s while it scans 451 roots (measured in docs/issues/archive/2026-10-23-documentsymbol-null-window-outlasts-the-5s-cold-budget-because-rust-analyzer-scans-the-nested-worktrees.md).
- Mitigations that exist in `src/lsp/`: `files.excludeDirs` for nested worktrees (`init_options.rs`), and a re-ask wait on `$/progress` with `PROGRESS_CEILING` = 40 s (`progress.rs`). That issue records that no live run showed a window above 5 s failing before the change and passing after.
- Pre-warm is hardcoded to JVM languages: `PREWARM_LANGUAGES = ["java", "kotlin"]` in `src/lsp/mod.rs`. Rust is not pre-warmed on activation.
- Rust started 199 times in 7 days in the codescout database (73 on 2026-11-09).
- Association, with a control: of 107 slow (5 s or more) `symbols`/`references` calls, 67.3% came within 5 minutes of a rust server start; of 3,635 ordinary calls, 37.1% did. So slow calls follow a start about twice as often as the base rate.

## Not established

- The causal share. About a third of slow calls (35 of 107) did not follow a start within 5 minutes. Their cause is unknown: a server restart outside the window, a rebuild that reloaded the server, and machine load from peer sessions would each fit; the data cannot separate them.
- Whether language-server starts for worktree workspaces are recorded in this database or in another; that could change the 67%.
- `lsp_events.first_response_ms` (average 318 ms for rust) does NOT size the cold cost: rust-analyzer answers once at 0.02 s, then turns `null` at 0.62 s (same issue). Do not use it for that.
- What pre-warming rust would cost or save. It trades startup CPU and RAM on every activation for first-call latency; it needs a live rust-analyzer measurement on a quiet machine and a deliberate decision (architecture question), not tuning.

## How to reproduce

Group `tool_calls` by `tool_name` over `called_at >= datetime('now','-7 days')`, per `usage.db`, listing the databases once with `find ~/work -path '*/.codescout/usage.db'` (looping `find` per tool timed out at 180 s). The control: compare the share of slow versus non-slow calls with a row in `lsp_events` (`language='rust'`, `started_at` within 5 minutes before `called_at`).
