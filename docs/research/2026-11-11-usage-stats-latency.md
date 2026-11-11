---
kind: research
status: active
title: "Usage statistics — latency (definition, tails, cold start, timeouts, budgets, recorder gap)"
tags:
  - usage-stats
  - frictions
  - latency
---

Archived analyst report from the 2026-11-11 statistics pass. The summary and the decisions are in [the statistics tracker](../trackers/frictions/statistics.md). The scripts named below are in `docs/research/data/2026-11-11-usage-stats-scripts.tar.gz`. They read pickles and a scratch directory that no longer exist, so treat them as method, not as a rerunnable job.

# codescout latency analysis (snapshot id <= 299175, 118,278 rows, 2026-10-12 08:06 to 2026-11-10 22:23 UTC)

Scripts (all in this directory, python3 -I, read-only sqlite): lat_load.py (builds lat_rows.pkl), lat_q1.py lat_q1b..f.py (definition/gap/logs/attribution), lat_q2.py lat_q2a..c.py (where time goes, wait loops, timeout detection), lat_q3.py lat_q3b..d.py (tails), lat_q4.py lat_q4b.py (cold start, lsp_events), lat_q5.py (segments), lat_q6.py (timeouts), lat_q7.py lat_q7b..d.py (budgets/spikes). Each writes lat_<name>.out which is appended raw below.

Conventions: wait loop = run_command whose command text matches `\b(until|while|sleep)\b|pgrep -f` (2,058 rows; only rows with input_json can be classified: 42,055 of 51,365 run_command rows). "At timeout" = output contains `"timed_out": true` (output_json exists only on 35,202 run_command rows) OR latency within [T, T+300 ms] where T = input timeout_secs (default 30 s when input present but no timeout_secs): 208 rows, heuristic. wall_ms = (called_at - started_at); gap = wall_ms - latency_ms.

## 1. Definition (verified in code)

- started_at: `UsageRecorder::record_content` (src/usage/mod.rs:89-167) takes `started_at = db::now_timestamp()` and `Instant::now()` together, immediately before `f().await`.
- latency_ms = `start.elapsed()` right after `f().await` (mod.rs:138). f is the closure built in `dispatch_tool_call` (src/server.rs ~2195-2210): `observations::server::dispatch_at(&root)` then `tool.call_content(input,&ctx).await`. So latency_ms INCLUDES the tool body, call_content's own post-processing/buffering/guide hints, and (run_command) the red-attribution engine run.
- latency_ms EXCLUDES: (a) the cross-process write-lock wait `acquire_write_guard_if_writing` (server.rs ~2161-2190, runs BEFORE record_content; a refused wait is recorded separately by `record_refusal_row` with the wait as latency, rows with err_family write_lock_unavailable), (b) everything the recorder does after f: `write_content` -> `with_project_at`, `worktree_main_root`, `db::open_db` (CREATE TABLEs, migration probes, backfill_legacy_rows), then the INSERT; (c) post-processing in dispatch_tool_call after the race (route_tool_error, guide/refusal text), (d) the retention sweep (see below), (e) MCP transport/client side.
- called_at = `strftime('%Y-%m-%d %H:%M:%f','now')` evaluated INSIDE the INSERT statement (db.rs write_record ~478-545), i.e. a completion instant taken after open_db and after the INSERT acquired its write lock. The db.rs comment (lines ~133-141) says exactly this. Empirical check (lat_q1e.out): for 320 `insert` lines of usage-lock-holds.log matched to rows, called_at lies within 50 ms of the END of the logged hold for 224 and within 50 ms of its start for 35; e.g. hold 1538 ms -> called_at - hold_start 1530 ms, gap 1531 ms. So INSERT lock wait lands in the gap.
- So gap = recorder overhead before called_at = with_project_at + worktree_main_root + open_db (+ its 5 s busy_timeout waits) + INSERT lock wait. It is independent of the tool's own latency (gap p99 is 3.4-3.8 s in every latency bucket from <10 ms to >10 s: lat_q1.out last lines).
- NOT in either column: the retention sweep. `write_record` runs `DELETE FROM tool_calls WHERE called_at < datetime('now','-30 days') [AND id NOT IN pika_observations]` AFTER the INSERT (db.rs ~568-590) on EVERY call, and `record_content` is awaited before the result is returned, so the sweep delays the response but appears in neither latency_ms nor called_at-started_at. EXPLAIN QUERY PLAN (read-only) = `SCAN tool_calls` (only index on the table is idx_tool_calls_tool_use_id; no index on called_at). DB is 295 MB / 72,041 pages of 4 KB, WAL mode. A read-only equivalent `select count(*) ... called_at < datetime('now','-30 days')` took 1.83 s cold and 0.073 s warm (twice) with load average 24-39 on 64 cores.
- Gap numbers (all rows): p50 1 ms, p90 133 ms, p99 3,737 ms; 99,456 rows < 10 ms; 5,003 rows > 1 s; 442 rows > 5 s. Excluding the two suspend-like rows (below) the gap sums to about 18,357 s over 118,278 rows (mean 155 ms). Per-tool gap p50/p90/p99 are in the table in lat_q1.out below: p50 is 1-2 ms for every tool; p90 is 329-330 ms for read_file/symbols, 152 grep, 138 workspace/create_file, 4-6 ms for edit_file/edit_code/librarian; p99 3.2-4.6 s for every tool with n>=130.
- Gap tail is time-clustered, not tool-specific: gap>1 s rows per day rise from ~0.2% (10-12..10-15, 10-20..10-23) to 13.2% on 2026-11-09 and 9.8% on 11-08 (lat_q1c.out). The gap histogram at 100 ms resolution has a plateau that ends near 5.0-5.1 s (44 rows in 5000-5099 ms, then 16 in 5100-5199) = the 5 s `busy_timeout` signature. HYPOTHESIS (consistent with the code and logs, not proven per row): many concurrent server processes (34 distinct pids in the lock-holds log) serialise on usage.db.
- id>290000: avg latency 1,296.8 ms vs avg wall 1,533.4 ms: 236.6 ms/call outside latency_ms. That is the recorder gap, dominated by rows with gap>1 s on 11-09/11-10; it does NOT include the sweep.
- Two rows are NOT lock waits: id 212104 (run_command `sleep 299; grep ...`, started 2026-10-20 17:59:10, called_at 2026-10-21 03:44:27, latency 299,183 ms, gap 34,818,025 ms = 9.67 h) and id 253891 (`./scripts/gate.sh`, started 2026-11-02 19:59:07, called_at 2026-11-03 04:32:36, latency 481,049 ms, gap 30,327,977 ms = 8.4 h). In both windows only the same session's own rows appear (6 and 11 rows total; no rows from any session between the last row before and first row after, 17:59:04 -> 03:48:26 and 19:58:54 -> 04:32:41). HYPOTHESIS: machine suspend; `Instant` (CLOCK_MONOTONIC on Linux) does not advance during suspend while the wall clock does. NOT verified (no suspend log checked). These two rows alone are 65,146 s of the 83,503 s total gap and inflate every wall-clock average; use latency_ms or exclude gap>1h.

### usage-lock-holds.log (/home/marius/work/claude/codescout/.codescout/) - lat_q1d.out
- Writer: `note_lock_hold` (src/usage/db.rs ~452-475). Line = started_at, pid, build sha, kind (backfill | insert | retention_sweep), ms, detail; written only when ms >= 250 (LOCK_HOLD_LOG_MS). The timers wrap `conn.execute`, so ms is wait-for-lock PLUS execution, not only time holding the lock (the doc comment says "held"). Log covers only 2026-11-09 20:21 -> 11-10 22:27 (1,227 lines, 34 pids): 343 insert (p50 1,540 ms, p90 3,552, max 6,528), 73 backfill (p50 2,429, max 12,444), 811 retention_sweep (p50 2,686, p90 4,785, max 20,004; sum 2,296 s; all but ~11 report deleted=0). 929 lines >= 1 s, 83 >= 5 s.
- Does it explain the gap? INSERT holds yes (call-by-call match above); backfill holds are in open_db so also inside the gap. Retention-sweep holds (2,197 s in the 777 sweeps inside the snapshot window, 643 of them >= 1 s) are NOT in the gap but ARE paid by the caller before the response returns. In the same window (8,048 rows from 11-09 20:21) the summed gap is 1,547 s. So the recorder costs the caller about (1,547 + 2,197)/8,048 = 465 ms per call there, and 59% of that is the unrecorded sweep. Caveat: both logs only exist for the last ~26 h of the corpus and log only >= 250 ms events.
### usage-dropped-rows.log - lat_q1d.out
- Writer: `UsageRecorder::note_dropped_row` (src/usage/mod.rs ~218-260): a row whose write failed. 165 lines, all `database is locked: Error code 5` (95 new-format lines with pid/build/wait_ms: wait_ms min 5,004, p50 5,010, max 5,776 = the 5 s busy_timeout; 70 older 4-field lines), 2026-11-09 13:12 -> 11-10 22:27 (104 on 11-09, 61 on 11-10), tools run_command 42, symbols 39, read_file 33, grep 24, doc 8. Those calls have NO row in tool_calls, so they are invisible to every statistic here (survivorship: the slowest recorder waits are censored). They do not explain the gap of surviving rows; they are its failure tail (5 s timeout hit).
### attribution_ms / attribution_outcome - lat_q1f.out
- Writer: src/tools/run_command/attribution.rs ~656-720 (clock starts after the gates, before engine extraction; every state records `ms()`); `attribution_outcome::record` (src/usage/attribution_outcome.rs:73) stores into a task-local sink drained by record_content. Outcomes: hinted | silent | timed_out | tooling_failure | spawn_failed; NULL = engine did not start. It runs INSIDE f, so attribution_ms is a component of latency_ms (ratio ams/lat p50 0.56, p90 0.91, never > latency).
- Rows: 1,727 run_command rows (first 2026-11-01 14:42; 7.9% of the 21,813 run_command rows since). attribution_ms p50 155, p90 1,617, p99 17,162, max 30,012 ms; sum 1,873 s = 9.7% of the latency of those rows, 0.24% of all run_command latency. By outcome: silent 1,549 rows p50 150 ms (early exit ~0.1 s; sum 295 s); hinted 177 rows p50 8,104 ms, p90 16,704, max 29,859 (sum 1,547 s); timed_out 1 (30,012 ms; the 30 s ceiling). Hinted runs are the cost: a failing command is delayed about 8 s median to attach a red-attribution hint.


## 2. Where time goes (tables are the raw output of lat_q2.py, appended in full below as "RAW lat_q2.out")

Headline: run_command is 785,379 s of 801,645 s total latency (98.0%); every other tool together is 16,266 s (4.5 h). Total recorded latency 222.7 h over 29 days (calls overlap in time across sessions/subagents, so this is summed tool time, not elapsed time). Excluding wait loops and timeouts removes 412,288 s: run_command still holds 373,092 s = 95.8% of the remaining 389,357 s.
run_command time by kind (n, sum_s, share of run_command time): wait loops 2,058 / 400,576 s / 51.0% (p50 90 s, p90 540 s, p99 1,780 s); cargo/nextest/rb/with-slot 3,926 / 239,149 s / 30.5% (p50 19 s, p90 155 s, p99 494 s); other 16,044 / 57,054 s / 7.3%; python3 scripts 4,798 / 30,310 s / 3.9%; pytest 1,608 / 17,713 s / 2.3%; git 11,111 / 12,058 s / 1.5%; at-timeout 176 / 11,712 s / 1.5%; gate.sh 672 / 7,297 s / 0.9%. Rows with no input recorded (9,310; 7,053 s) cannot be classified. Categories are first-match regexes, so "cargo" includes foreground test/clippy runs, which are inherent; the `gate.sh` row count is low in time because it is usually launched in background (run_in_background rows: 2,596, p50 28 ms, max 22,263 ms).
Wait loops (lat_q2c.out): @bg_N polling 539 rows / 114,372 s (p50 118 s); `sleep N` without until/while 1,414 rows / 324,821 s (p50 118 s) [categories overlap slightly]; pgrep 216 / 27,414 s; file/grep polling 400 / 34,187 s. Subagents carry 1,075 of 2,058 loops and 319,692 s; parents 983 loops / 80,884 s. Two sessions (500c9b13 112,577 s; e98b5705 102,323 s) hold 54% of all wait-loop time; 172 distinct sessions have at least one. By week the wait-loop time falls 209,841 s (W1, 827 loops) -> 101,350 (W2) -> 79,217 (W3) -> 7,561 (W4) -> 2,607 (W5, partial). HYPOTHESIS: the drop follows adoption of background jobs + wake hook; not tested.
Inherent vs avoidable (labels, not measured): builds/tests/clippy/gate in the foreground are inherent in wall time but their p50 19 s means many are cheap; wait loops that poll a job the same session started (@bg_N polling, `sleep N; tail`) are avoidable in principle with the wake hook (parents) but a subagent gets no wake (per CLAUDE.md), so for subagents a bounded poll is the only option; `pgrep -f` loops are the documented self-matching anti-pattern (progress-disclosure guide: the loop finds itself) and are avoidable.

## 3. Tails (per tool, except run_command)

Raw per-tool top-1% / top-20 listings with ids and inputs are in lat_q3.out and lat_q3c.out (by action) and lat_q3d.out below. Summary:

| tool | n | p50 / p99 / max (ms) | tail anatomy (verified from rows) |
|---|---|---|---|
| symbols | 10,569 | 19 / 11,487 / 57,560 | By mode (lat_q3c.out): name+path 5,610 rows p50 20 ms; path-only overview 1,458 p50 42; NO-path project-wide `name` 225 rows p50 10,583 ms, p90 12,335, sum 1,993 s (30% of all symbols time from 2% of calls). Code (src/tools/symbol/symbols.rs ~794, ~954) runs one `workspace/symbol` leg per language with PER_LANG_BUDGET = 8 s plus a tree-sitter pass; its own comment says 214 of 222 such calls waited the whole budget. The p50 by period (5-day buckets, lat_q7.out): 10-15..10-24 11.1-11.5 s (117 rows, 100% >= 8 s); 10-25..10-31 7.6 / 9.0 s (23 rows); since 11-01 3.4-3.9 s (85 rows, 32 < 3 s). The code now has SHORT_LANG_BUDGET = 1.5 s after a recent full timeout; that the drop is due to it is a HYPOTHESIS (commit date not checked). Second mode: 21 symbols rows in [40.0, 40.7] s on 10-29..11-01 = `PROGRESS_CEILING` 40 s (src/lsp/progress.rs:31) exhausted while rust-analyzer answers documentSymbol null; 83 rows in [5.0, 5.25) s = the 5 s cold re-ask budget (src/lsp/progress.rs header). Six consecutive 33-56 s calls in one session on 11-09 12:23 (/tmp stock checkout) and 3 on 11-10 11:05 (worktree). 54 of the 105 tail rows are from subagents; 61 from worktree roots. No symbols row exceeds 57.6 s (the 60 s server tool timeout never fired for it). |
| references | 260 | 1,711 / 42,166 / 55,179 | Slow even warm (later calls p50 1,384, p90 8,112 ms); first call per (session,root) p50 2,562 / p90 21,168 ms. Rust-file calls (194) p50 2,336 / p90 20,833; python (31) p50 1,230 / p90 2,236. Spikes: 3 rows at 29.9-30.0 s with error `LSP request timed out` (30,013-32,454 ms) and 2 rows at 45.0-45.1 s (FIRST_CALL_SETTLE_WAIT 45 s). |
| call_graph | 130 | 452 / 27,977 / 39,788 | First call p50 1,798 / p90 17,209 vs later p50 132 / p90 2,193. Rust path (14 rows) p50 5,044 / p90 27,896; python 30 rows p50 252. |
| librarian | 285 | 165 / 71,842 / 128,955 | Top 20 (>= 20 s) hold 61% of tool time; 9 are `reindex` (25 reindex calls p50 13.3 s, p90 58 s, max 127 s), 9 have no input recorded; 9 of the 20 on 2026-10-30 (reindex/audit_doc_refs: audit_doc_refs p50 85 ms but max 128,955). `doctor` 55 calls p50 6.9 s, p90 14.3 s, max 21.6 s (steady, see memory research/doctor-latency-20261109). 8 of 20 overflowed. Last >=60 s row 11-09. |
| workspace | 889 | 127 / 34,903 / 54,618 | `activate` 430 rows p50 132 / p90 1,070 / max 2,403 ms, mean 372 ms (160 s / 430): activate does NOT average 1.5 s; the 1.5 s workspace mean (1,340 s / 889) comes from `status` (mean 9.4 s) and the 400 no-input rows (652 s); `status` 56 rows p50 654, p90 31,270, max 48,789; 30 rows >= 20 s (sum 951 s), ALL on 2026-10-14..10-30, none since: workspace max since 11-01 is 5,969 ms. |
| index | 21 | 27,498 / 41,542 / 42,197 | `status` (9) p50 30.1 s, `verify` (3) p50 27.5 s, `build` p50 140 ms; all 15 calls >= 20 s are 10-13..10-30; 11-05 status 207/266 ms and 11-07 verify 2,245 ms: slow behaviour appears gone after 10-30. |
| doc | 6,786 | 21 / 1,028 / 10,981 | Tail (67 rows >= 1,039 ms): update 19, move 19 (move p50 395 ms), find 14 (find p50 85 ms); 4 recoverable_error rows at ~5,010-5,028 ms (3 carry err_family write_lock_unavailable = `held` latency at the 5 s write lock timeout); worst: update id 239942 10,981 ms with a 179 ms gap. 30 distinct sessions; no concentration. |
| memory | 1,491 | 1 / 3,002 / 4,877 | `write` 209 rows p50 1,015 ms, p90 1,726, max 4,538 (sum 229 s) while read p50 0 ms: every memory write costs ~1 s (spike bin 1,000-1,750 ms: 68+39+28 rows, from 10-29 on; cause not investigated; HYPOTHESIS: embedding/sync on write). |
| edit_code | 4,492 | 62 / 2,124 / 45,461 | Tail 44 rows: insert 26, rename 10 (rename p50 988 ms), replace 6; 22 recoverable_error (7 of them 15-19 s `lsp_document_symbol_null` 'null on every attempt for 5000 ms' error, 10-24..11-01; 11 rows in bin 3,000-3,250 ms are errors); 12 of 44 from subagents; slowest: insert 45,461 ms (10-29 22:20), remove 34,384 ms (10-30). |
| read_file | 19,730 | 0 / 33 / 21,487 | 98.4% under 33 ms (histogram: 10,567 rows < 1 ms, 4,044 1-3 ms, 3,822 3-10 ms, 1,089 10-33 ms, 164 33-100 ms, 41 100-1000 ms, 3 >= 1 s). The 44 rows >= 100 ms: 13 are recoverable_error (11 `il1_read_overlaps_symbol`, whose body consults symbols/LSP before refusing), 18 non-main roots; the three >= 1 s (21,487 / 7,364 / 4,572 ms) are single rows, two within one minute of each other (10-17 07:08) with high concurrent gaps; HYPOTHESIS machine/disk stall, weak association (20% of slow reads have a non-run_command row with gap>1 s within +-3 s vs 8.3% of 2,000 random fast reads). No by-kind difference (md heading p90 7, rs p90 10, buffer-ref p90 3). Nothing else is slow about read_file. |
| edit_file | 8,485 | 10 / 361 / 5,014 | 15 rows >= 1 s; 2 of them 5,014 ms `write_lock_unavailable` (11-09 11:05) = 5 s write_lock_timeout_secs (src/config/project.rs:296); rest 1.7-2.7 s on 7 sessions. |
| create_file | 3,833 | 2 / 55 / 5,033 | 6 rows >= 1 s; 4 are 5.0-5.03 s `write_lock_unavailable` (11-09 07:53..21:38). |
| grep | 9,227 | 16 / 1,651 / 386,711 | 7 rows at 19.9-20.1 s = `time_budget_ms` 20 s; 3 rows >= 99 s: 386,711 ms (id 210368, path=/home/marius/work/claude, mode files, 10-20), 135,109 ms (path=/home/marius, `**/*.d.ts`, include_hidden, 10-19), 99,824 ms (id 226057, no input recorded): the 20 s budget did not bound these (gap 1-5 ms, so not recorder delay); HYPOTHESIS: walk is synchronous and the budget is checked between files (cf. the code comment in callers_check.rs:151 "its files-mode walk is synchronous"). NOT verified. |
| tree, get_guide, approve_write, semantic_search, symbol_at | 173/452/69/14/7 | semantic_search p50 844 ms; symbol_at p50 648 ms (n=7); tree p99 947 ms | negligible time (sums 15, 2, 0, 17, 13 s). |
| run_command (for completeness) | 51,365 | 94 / 290,236 / 3,000,065 | see section 2. |

## 4. Cold start (raw tables in lat_q4.out below)

First call per (session_id, project_root) of THAT tool vs later calls (p50 / p90, ms):
| subset | first n | first p50/p90 | later n | later p50/p90 | p50 penalty | mean penalty |
|---|---|---|---|---|---|---|
| references | 109 | 2,562 / 21,168 | 151 | 1,384 / 8,112 | 1,178 | 3,673 |
| call_graph | 41 | 1,798 / 17,209 | 89 | 132 / 2,193 | 1,666 | 4,908 |
| symbol_at | 4 | 970 / 3,447 | 3 | 2 / 5,380 | n too small | |
| edit_code | 312 | 94 / 578 | 4,180 | 61 / 394 | 32 | 95 |
| workspace (all actions) | 403 | 259 / 1,588 | 486 | 78 / 1,198 | 181 | 209 |
| workspace activate | 278 | 172 / 1,159 | 152 | 92 / 861 | 79 | 180 |
| symbols (name given) | 400 | 152 / 4,170 | 5,435 | 20 / 599 | 132 | 656 |
| symbols (name+path) | 395 | 132 / 2,250 | 5,215 | 17 / 247 | 115 | 383 |
| symbols (path only) | 276 | 176 / 1,810 | 1,185 | 27 / 690 | 150 | 436 |
| any LSP-class call | 471 | 166 / 3,056 | 10,028 | 37 / 513 | 129 | 647 |
Reading: the cold penalty at p50 is 0.03-1.7 s; it is the p90 that is large (references 21 s, call_graph 17 s). references is slow warm too (p50 1.4 s, ordinal 2-6 p50 1.7/1.7/1.2/1.1/0.5 s), so most of its time is the request itself, not startup. Caveat: "first per (session_id, project_root)" is not "first on a cold server": a session id is a server process, servers are shared across worktrees/mux, and symbols could have warmed the server before the first references.
lsp_events (this DB only; window 10-12..11-10; lat_q4.out has the by-reason table): handshake_ms p50/p90 by language (success rows): rust 53/201 (n=791), python 182/252 (598), javascript 113/168 (187), typescript 116/175 (143), bash 557/1,158 (198), css 295/478, html 491/760, java 2,490/3,465 (435), kotlin 2/4,455 (448, bimodal). first_response_ms p50/p90: rust 16/124 (n=679, max 40,043), python 370/521 (455, max 5,413), javascript 530/748, typescript 500/616, bash 36/92; java/kotlin have no first_response (n=0 / 1). The memory research/lsp-cold-start-latency-20261110 says first_response_ms does NOT size the cold cost for rust-analyzer (it answers once at 0.02 s then null at 0.62 s) - so do not read "rust first_response p50 16 ms" as cold cost. Failed starts: 339 of 3,376 events; java new_session failed 206 at p50 16,494 ms (error `LSP server disconnected`; total handshake time 3,437.6 s, 2026-10-12..10-31 only), kotlin failed 70 (113 s), rust failed 63 (p50 31 ms), error text for some: "mux process failed to start: ... another mux instance holds the lock". Whether failed starts block a tool call is NOT verified (java/kotlin are prewarmed in the background per the memory).
Association with starts (lat_q4b.out): of 507 symbols/references/call_graph/symbol_at/edit_code calls >= 5 s, 74.0% began within 5 min after a rust lsp start vs 38.7% of 14,951 ordinary calls (python 47.4% vs 41.2%, java 40.0% vs 30.6%, kotlin 45.5% vs 26.4%). Rust starts: 92 on 10-17, 75 on 11-09, 51 on 10-16 and 10-31. Same direction as the memory's 67.3% vs 37.1%. Cause not established (load, restarts, rebuilds would also fit).

## 5. Segments (raw tables in lat_q5.out, lat_q6.out first table)

Key reads (cells n / p50 / p90 ms; run_command excluding wait loops and timeouts):
- Parent vs subagent: run_command parent 33,958 / 76 / 1,969 vs subagent 15,173 / 126 / 55,129; symbols 8,343 / 14 / 655 vs 2,226 / 51 / 2,026; edit_code 33/223 vs 125/604 (p50/p90); doc and memory are parent work. Of run_command rows >= 10 s (excl. wait/timeout): parent 1,238 rows hold 63,916 s (52% build/test commands) vs subagent 2,478 rows hold 285,692 s (75% build/test): subagents account for 78% of non-wait long run_command time.
- Root class: main checkout 73,021 rows; worktrees 34,399; other-under-codescout 5,068; /tmp checkouts 4,741; other repos 1,049. symbols p90: main 734, worktree 614, tmp 1,025, other-under-codescout 3,406. run_command p90: main 2,007, worktree 49,184, other-under-codescout 78,844.
- Week (UTC Mon-start; W5 partial 11-09/10): run_command p90 35,358 / 33,744 / 3,307 / 2,668 / 1,849 ms for W1..W5; symbols p90 553 / 5,035 / 805 / 601 / 745; workspace p50 552 / 673 / 191 / 93 / 125; memory p90 1 / 5 / 1,803 / 1,156 / 135 (the memory-write ~1 s appears from W3, ~10-29, to W4; W5 p50 10 ms); build/test share of long run_command time per week: 82% / 85% / 37% / 69% / 55%.
- codescout_dirty: no meaningful tool differences except run_command p90 3,157 (clean, n 17,097) vs 7,834 (dirty, n 32,034) and edit_code p50 31 vs 91 - confounded by time period and by what was being built; not interpreted.
- Hour of day UTC (table in lat_q5.out): load peaks 07-12 UTC (6,800-8,400 rows/h; busiest 8,421 at 11) and the gap tail concentrates where load is high (rows with gap>1 s: 634 at 11 UTC, 496 at 15, 442 at 16; gap p90 731 ms at 03 UTC with only 1,858 rows, an outlier whose cause I did not examine) while non-run_command p90 stays 120-410 ms. run_command p90 by hour is dominated by rare long builds (e.g. 19,530 ms at 20 UTC).
- Top 5 shas (rows): 3923da8c (8,777; 10-17..10-19) run_command p50 193, symbols p50 61; 0007d926 (8,558) 87/34; 0eb6190ec (8,436; 10-30..11-04) 92/13; 2acc553ad (7,630) 83/26; aebb0e76f (6,150; 11-04..11-07, the only clean one) 66/9. 141 distinct shas, each build covers a different date range, so build and period are confounded; no build effect is claimed.

## 6. Timeout boundary (raw: lat_q6.out)
Rows within +-100 ms of a round value (n; time): 20,000 ms: run_command 22 (441 s), grep 7 (140 s), references 1, librarian 1; 30,000 ms: run_command 223 (6,697 s), references 3 (90 s), index 1, symbols 1; 60,000 ms: run_command 47 (2,822 s); 120,000: run_command 56 (6,723 s); 300,000: run_command 4 (1,200 s). Control: the adjacent 100-300 ms windows hold 29 / 32 / 31 / 23 / 2 rows over all tools, so there is a real spike at 30 s (223 vs 32), 60 s, 120 s, and at 20 s only for grep/run_command. Total time in these windows is 0.06% + 0.84% + 0.35% + 0.84% + 0.15% of all latency, about 2.3% (18,200 s).
run_command rows at their OWN timeout (flag or [T,T+300 ms]; heuristic): 208 rows, 32,859 s = 4.2% of run_command time; by timeout_secs the biggest are 30 s default (154 rows, 57 flagged, 4,627 s), 3000 s (2 rows, 6,000 s), 1500 s (3, 4,501 s), 1800 (2, 3,600), 2900 (1), 900 (3, 2,700), 1200 (2, 2,400); only 111 rows carry an explicit `"timed_out": true` in output_json and only 32 of the 208 are wait loops. A run_command timeout is recorded as outcome=success (all 4,535 rows >= 20 s are success with err_family NULL), so outcome cannot find them; the output text `Command timed out after N seconds` + `timed_out: true` can. Server-level `tool_timeout_secs` (60 s): zero `tool_timeout` rows exist; outside run_command only librarian (6 rows 59-129 s; reindex is exempt from the server timeout per references.rs:277-279) and grep (3 rows: 99,824 / 135,109 / 386,711 ms, ids 226057 / 208396 / 210368, dates 10-29 / 10-19 / 10-20) exceed 57.6 s; grep is not in the exemption list, so those three outlived a 60 s timeout. HYPOTHESIS: the grep files-mode walk is synchronous and blocks the executor, so tokio's timeout cannot fire until it ends; not verified (note the recorder only started recording timed-out calls in the 2026-11-08 fix, docs/issues/archive/2026-11-08-cancelled-and-timed-out-calls-leave-no-usage-row.md, so absence before then is weak evidence). Other refusals: 14 `write_lock_unavailable` rows (create_file 9, doc 3, edit_file 2; 13 on 11-09, 1 on 11-10), latency 5,006-5,033 ms (the 5 s write_lock_timeout_secs), 40 s total.

## 7. Budgets (raw: lat_q7.out, lat_q7c.out)
- grep 20 s `time_budget_ms`: 7 rows in [19.9,20.1) s and none in [19,19.9) or [20.1,25): a sharp spike = budget exhausted (verified shape; the responses were not read). But 3 rows exceed it by 5-19x (above), so the budget is not a hard bound.
- symbols 20 s (path+name `time_budget_ms`): 0 rows in [19,20.5); of 5,610 name+path rows 26 >= 19 s and 85 in [5,19) s: no budget-exhaustion spike; the spikes are at 5.0 s, ~8-12 s (project-wide leg: 8 s PER_LANG_BUDGET plus tree-sitter) and 40.0-40.2 s (PROGRESS_CEILING). Explicit time_budget_ms values were passed on only 10 rows (60000 x2, 100 x2, 1, 15000, 20000, 120000-120003).
- references: FIRST_CALL_SETTLE_WAIT 45 s -> 2 rows in [44.9,45.1) s (+3 in [40,44), 1 in [46,59)); 30 s LSP request -> 3 rows in [29.9,30.1) s, all with `LSP request timed out`; tool timeout 60 s: 0 rows. Counts are tiny (n=260 total), so spikes are real (control windows are empty) but rare: 5 rows, about 180 s.
- call_graph: 1 row at 30.1-31 s; no spike. edit_code: 7 rows 15-19 s (all `lsp_document_symbol_null`, 5 s null window repeated), 1 at 45.1-46 s. librarian: 1 row at 19.9-20.1 s. workspace: 11 rows in [21,25) s, 8 in [31,40) s (all before 10-30).
- Spike bins from the generic scan (lat_q7c.out): other than symbols 5.0 s and 40.0 s, the largest non-run_command bins are at 1,000-1,250 ms (symbols 90, memory 68, grep 61), which have no round-number tie; no further round-number spike with n >= 6 exists.


## 8. Most actionable latency observations (max 8)

1. Retention sweep + open_db on every call, serialised across ~34 server processes. Evidence: DELETE ... WHERE called_at < now-30d is `SCAN tool_calls` (no called_at index; 295 MB db); 777 sweeps >= 250 ms in the last 26 h sum 2,197 s (p50 2.7 s, all but 11 delete 0 rows), unrecorded in latency_ms and in the gap; gap >1 s on 13.2% of rows on 11-09; 5 s plateau; 165 dropped rows with `database is locked` at 5,004-5,776 ms. Avoidable: yes in principle (index or run the sweep rarely). Confidence: high that the sweep scans and is unrecorded; medium-high that it is the main contributor to the lock waits (timers include lock-wait, so scan cost vs contention is not separated).
2. latency_ms understates what the agent waits: three components sit outside it (write-lock wait before record_content, open_db+INSERT lock wait, retention sweep). Gap p99 3.7 s on every tool, including read_file p50 0 ms; 236.6 ms/call outside latency_ms for id>290000. Two rows (id 212104, 253891) show a 9.7 h and 8.4 h gap with the same session's neighbouring rows 5 s apart: use latency_ms or drop gap>1h before averaging wall time. Confidence: high (code + log joins).
3. run_command wait loops are 51% of run_command time (400,576 s, 2,058 loops), 54% of that in two sessions, 80% by subagents (319,692 s); 539 loops poll an @bg_N job (114,372 s). Loop time fell from 209,841 s in W1 to 2,607 s in W5. Avoidable for parents (wake hook), only partly for subagents (no wake). Builds/tests (cargo etc., 239,149 s, 30.5%) are inherent. Confidence: high on the numbers, medium on the cause of the decline (hypothesis).
4. symbols(name) with no path: 225 calls, p50 10.6 s, 1,993 s total (30% of all symbols time). Code gives the cause (8 s PER_LANG_BUDGET LSP leg per language); p50 3.4-3.9 s since 11-01 (n=85) after the short-budget logic. Avoidable: pass `path` (name+path p50 20 ms). Confidence: high for the 8 s cause (code comment + histogram mode at 10-12 s); the improvement attribution is a hypothesis.
5. 40.0-40.2 s cluster: 21 symbols + 3 references + 29 run_command rows in [40.0,40.7] s; the symbols/edit_code ones on 10-29..11-01 equal PROGRESS_CEILING (src/lsp/progress.rs:31) exhausted while rust-analyzer answers documentSymbol null; plus 83 symbols rows at 5.0-5.25 s (cold re-ask budget). Time: about 840 s + 415 s. Avoidable only by warm servers or by returning the tree-sitter result instead of waiting (design choice). No symbols 40 s row after 11-01 09:51. Confidence: medium-high (timing matches constants; error text for 5 s edit_code rows read, 40 s rows' outcomes are success and were not inspected).
6. grep outliers past the budget and the 60 s server timeout: 386.7 s, 135.1 s, 99.8 s on broad paths (/home/marius/work/claude, /home/marius), none after 10-29; plus 7 clean 20 s budget hits since 11-01. Avoidable: yes (scope the path). The three could indicate a timeout that cannot fire (hypothesis). Confidence: medium (3 rows).
7. Foreground-slow LSP tools are slow warm too: references p50 1.4 s warm, p90 8.1 s; first call p90 21 s; call_graph first p50 1.8 s vs 132 ms later; 74% of >=5 s LSP-tool calls begin within 5 min of a rust-analyzer start vs 38.7% base. Mostly inherent to rust-analyzer per request; the start-correlated part is avoidable only with prewarm (memory says rust is not prewarmed). Confidence: medium (association, not cause).
8. memory(write) costs about 1.0 s per call (209 calls, p50 1,015 ms, 229 s) while reads are 0 ms; the memory-write spike bins (1.0-1.75 s) run 10-29 to 11-10 (lat_q7c.out). Cause not investigated. Also java LSP failed starts: 206 with handshake p50 16.5 s (3,438 s), 10-12..10-31 only. Confidence: high on the numbers, low on cause (hypotheses: embedding on write; background start attempts).

## NOT VERIFIED
- Suspend as the cause of the 9.7 h / 8.4 h gaps (no system suspend log read).
- That the sweep scan (versus lock contention) is what makes retention_sweep log lines 2.7 s; the timer includes lock wait. Only the plan (full SCAN) and a 1.83 s cold / 0.073 s warm read-only scan timing were verified.
- That usage-lock-holds.log / usage-dropped-rows.log cover the earlier 17 days of the corpus (they begin 2026-11-09 20:21 and 13:12); the gap on earlier days has no log to attribute it.
- Whether strftime('now') in the INSERT is evaluated after the write lock is acquired (inferred from 320 joined log lines, 224 within 50 ms of hold end).
- Server tool timeout (60 s) not firing on the 3 grep rows; grep not being in the timeout exemption list was taken from the doc comment at references.rs:277-279, not from `tool_skips_server_timeout` itself.
- Whether the symbols name-only improvement since 11-01 is caused by SHORT_LANG_BUDGET (commit/deploy date not checked).
- What outcome (partial result vs error) the 40 s symbols rows and 5 s symbols rows returned (outcome=success; response bodies mostly absent: output_json exists on 3,572 of 10,569 symbols rows).
- Whether failed java/kotlin LSP starts block tool calls.
- Wait-loop and timeout classification is regex/heuristic and limited to rows with input_json (42,055 of 51,365 run_command rows; 9,310 unclassified); `sleep` anywhere in a command counts as a wait loop (e.g. `sleep 2; cmd` pipelines).
- Per-hour and per-sha numbers confound period, load and build; no causal reading.
- Rows dropped for `database is locked` are absent from tool_calls, so slowest recorder waits are censored.
- lsp_events cold start per language: first_response_ms does not measure rust-analyzer cold cost (memory research/lsp-cold-start-latency-20261110); no per-call link between a tool call and its lsp_events row exists (no project_root column).
- Lock wait on the write guard (before record_content) is not in the DB except as write_lock_unavailable rows (14); successful but delayed writes are in neither column.


## RAW lat_q1.out
```
overall gap stats p50/p90/p99/max [1, 133, 3737, 34818025]
neg gaps (<-5ms): 0 min 0.0
gap>1000: 5003  >5000: 442 >60000 2
sum gap s: 83503 sum lat s: 801645
id>290000 avg lat, avg wall: 1296.7844141689372 1533.3973841961854

per tool: n, gap p50 p90 p99 max, sumgap_s, share of total gap, lat p50
run_command 51365 [1, 22, 3333, 34818025] 71298 85.4% 94
read_file 19730 [1, 329, 4138, 36138] 4140 5.0% 0
symbols 10569 [2, 330, 3946, 27724] 2224 2.7% 19
grep 9227 [1, 152, 3835, 26535] 1692 2.0% 16
edit_file 8485 [1, 6, 3532, 36574] 1122 1.3% 10
doc 6786 [1, 106, 3732, 31013] 975 1.2% 21
create_file 3833 [1, 138, 4499, 24335] 765 0.9% 2
edit_code 4492 [1, 6, 3582, 27835] 663 0.8% 62
memory 1491 [1, 81, 3194, 15553] 182 0.2% 1
workspace 889 [1, 138, 3918, 6297] 147 0.2% 127
get_guide 452 [2, 531, 4539, 5013] 129 0.2% 0
references 260 [1, 166, 3862, 10020] 51 0.1% 1711
tree 173 [2, 333, 4609, 7969] 48 0.1% 1
librarian 285 [1, 4, 3175, 11230] 31 0.0% 165
call_graph 130 [1, 199, 3806, 4536] 30 0.0% 452
approve_write 69 [1, 3, 1769, 3141] 4 0.0% 0
semantic_search 14 [2, 400, 567, 587] 1 0.0% 844
index 21 [1, 140, 339, 385] 1 0.0% 27498
symbol_at 7 [2, 275, 643, 684] 1 0.0% 648

per outcome:
success 114350 [1, 134, 3737, 34818025] 82953
recoverable_error 3883 [1, 130, 3738, 10382] 543
error 45 [1, 4, 3475, 6088] 6

gap buckets (all):
-1000000000.0 0 0
0 10 99456
10 50 1785
50 100 2448
100 250 6716
250 500 1458
500 1000 1412
1000 2000 1643
2000 5000 2918
5000 1000000000000.0 442

gap by day-bucket (id range) p50/p90:
180000 19102 [1, 80, 3236, 5145]
200000 20000 [1, 41, 3235, 34818025]
220000 20000 [1, 136, 3235, 17275]
240000 20000 [1, 113, 2678, 30327977]
260000 20000 [1, 80, 3039, 15553]
280000 19176 [2, 1009, 6743, 38994]

big-gap(>1000ms) rows by tool: [('run_command', 1679), ('read_file', 1134), ('symbols', 575), ('grep', 528), ('edit_file', 319), ('doc', 227), ('create_file', 196), ('edit_code', 177), ('memory', 50), ('workspace', 40), ('get_guide', 33), ('tree', 14)]
by root: [('/home/marius/work/claude/codescout', 2858), ('/tmp/codescout-real-task-20261024', 181), ('/tmp/codescout-stock-observations-20261109', 149), ('/home/marius/.codex/worktrees/cf66/codescout', 139), ('/home/marius/.codex/worktrees/615e/codescout', 98), ('/home/marius/work/claude/codescout.worktrees/il3-prompt-wording', 61)]
lat in 0 10 38755 [1, 163, 3432, 36138]
lat in 10 100 45034 [1, 122, 3824, 36574]
lat in 100 1000 22289 [1, 121, 3834, 38994]
lat in 1000 10000 6559 [2, 181, 4239, 26535]
lat in 10000 1000000000000.0 5641 [2, 126, 3836, 34818025]
```

## RAW lat_q1c.out
```
2026-10-20 17:59:10 2026-10-21 03:44:28 rows completing inside window (all tools): 6 [(212099, '2026-10-20 17:59:12.375', 'edit_file'), (212100, '2026-10-20 17:59:16.899', 'run_command'), (212101, '2026-10-20 17:59:23.097', 'run_command'), (212102, '2026-10-21 03:44:15.794', 'run_command'), (212103, '2026-10-21 03:44:20.736', 'run_command'), (212104, '2026-10-21 03:44:27.726', 'run_command')]
 last before 2026-10-20 17:59:04.374 first after 2026-10-21 03:48:26.015
2026-11-02 19:59:07 2026-11-03 04:32:37 rows completing inside window (all tools): 11 [(253881, '2026-11-02 19:59:08.432', 'run_command'), (253882, '2026-11-02 19:59:36.051', 'run_command'), (253883, '2026-11-02 19:59:37.518', 'run_command'), (253884, '2026-11-02 19:59:38.251', 'run_command'), (253885, '2026-11-02 20:00:16.475', 'read_file'), (253886, '2026-11-02 20:00:17.097', 'run_command')]
 last before 2026-11-02 19:58:54.250 first after 2026-11-03 04:32:41.000
day: n, gap>1000 count, share, gap sum s (excl >1h)
2026-10-12 505 1 0.2% 5
2026-10-13 3247 2 0.1% 31
2026-10-14 2910 7 0.2% 69
2026-10-15 382 0 0.0% 1
2026-10-16 5595 207 3.7% 692
2026-10-17 9333 386 4.1% 1221
2026-10-18 4951 201 4.1% 632
2026-10-19 2258 115 5.1% 340
2026-10-20 2023 5 0.2% 26
2026-10-21 1740 2 0.1% 20
2026-10-22 550 0 0.0% 2
2026-10-23 989 2 0.2% 9
2026-10-24 3765 306 8.1% 931
2026-10-26 632 0 0.0% 7
2026-10-27 3391 27 0.8% 181
2026-10-28 2603 81 3.1% 398
2026-10-29 6668 219 3.3% 1018
2026-10-30 4660 65 1.4% 203
2026-10-31 6307 296 4.7% 888
2026-11-01 6661 241 3.6% 707
2026-11-02 3819 48 1.3% 220
2026-11-03 3393 15 0.4% 100
2026-11-04 7167 276 3.9% 896
2026-11-05 5084 122 2.4% 402
2026-11-06 3738 26 0.7% 128
2026-11-07 5969 421 7.1% 1257
2026-11-08 3006 294 9.8% 916
2026-11-09 10385 1372 13.2% 5939
2026-11-10 6547 266 4.1% 1120
```

## RAW lat_q1d.out
```
lock-holds lines 1350 first 2026-11-09 20:21:44.486 last 2026-11-10 22:35:16.233
field counts Counter({6: 1350})
insert 398 p50 1541 p90 3740 max 6528 sum_s 724.4
backfill 73 p50 2429 p90 6422 max 12444 sum_s 208.8
retention_sweep 879 p50 2659 p90 4694 max 20004 sum_s 2478.1
by day [('2026-11-09', 670), ('2026-11-10', 680)]
insert by tool [(('insert', 'run_command'), 148), (('insert', 'read_file'), 89), (('insert', 'symbols'), 69), (('insert', 'grep'), 32), (('insert', 'edit_file'), 22), (('insert', 'edit_code'), 7), (('insert', 'references'), 7), (('insert', 'doc'), 6)]
distinct pids 36
holds >=1000ms: 1027  >=5000: 89
backfill samples
['2026-11-09 20:53:04.449', '4185695', '510500039', 'backfill', '2346', 'from=1965217055 to=676504337 update_scan_ms=2207']

dropped lines 194 first 2026-11-09 13:12:49.991 last 2026-11-10 22:34:45.740 field counts Counter({7: 123, 4: 71})
by day [('2026-11-09', 104), ('2026-11-10', 90)]
by tool [('run_command', 56), ('read_file', 44), ('symbols', 41), ('grep', 26), ('doc', 8), ('get_guide', 5), ('workspace', 4), ('edit_file', 4), ('memory', 2), ('create_file', 2)]
by error [('database is locked: Error code 5: database is lock', 194)]
wait_ms n 123 p50 5008 min 4927 max 5776
sample ['2026-11-10 22:34:45.740', 'run_command', '-', 'database is locked: Error code 5: database is locked', '275057', 'ba103456c', '5025']
roots [('-', 106), ('/home/marius/work/claude/codescout', 65), ('/home/marius/.codex/worktrees/e646/codescout', 11), ('/home/marius/.codex/worktrees/cf66/codescout', 10), ('/home/marius/.codex/worktrees/add2/codescout', 1), ('cout.worktrees/formatter-exclusive-with-proof', 1)]
```

## RAW lat_q1e.out
```
insert lines 320 matched 320
called_at - hold.start vs hold ms (first 12): [(1530, 1538, 1531), (329, 335, 330), (329, 434, 330), (529, 535, 530), (328, 334, 330), (103, 334, 105), (1630, 1638, 1631), (1730, 1753, 1732), (1229, 1258, 1232), (2, 1938, 4), (2, 2139, 4), (1029, 1037, 1030)]
called_at within 50ms of hold END: 224  within 50ms of hold START: 35
holds>=1s matched: 195 gap>=0.5*hold: 168
sweeps 777 sum_s 2197 >=1s 643 deleted values [('deleted=0', 766), ('deleted=1', 4), ('deleted=5', 3), ('deleted=3', 2)]
rows in same window 8048 sum gap s 1547
sweep ms by tool? n/a
```

## RAW lat_q1f.out
```
rc rows 51365 with attribution_ms 1727 first row with attribution 2026-11-01 14:42:31.771
rc rows since first attribution row 21813 share with attribution 0.07917297024710035
attribution_ms p50/p90/p99/max [155, 1617, 17162] 30012 sum_s 1873
sum attribution_ms / sum latency (those rows) =  0.0972  / all rc latency 0.00238
outcome counts Counter({'silent': 1549, 'hinted': 177, 'timed_out': 1})
  hinted 177 p50 8104 p90 16704 max 29859 sum_s 1547
  silent 1549 p50 150 p90 247 max 5762 sum_s 295
  timed_out 1 p50 30012 p90 30012 max 30012 sum_s 30
ratio ams/lat p50 p90 0.559 0.908  rows ams>lat 0
ams>=1000: 189 time_s 1603 ; of rows with lat<2000 how many have ams>=500: 25
rows lat<500 with attribution 1040 ams p50 146.0 p90 216.0
```

## RAW lat_q2.out
```
rc rows: 51365 with cmd text 42055 wait 2058 at_timeout(flag or lat within [T,T+300ms]) 208 flag 100 both 32

### ALL rows (latency_ms, ms)  (rows 118278, total 801645 s = 222.68 h)

| tool | n | p50 | p75 | p90 | p95 | p99 | max | sum_s | share |
|---|---|---|---|---|---|---|---|---|---|
| run_command | 51365 | 94 | 475 | 10725 | 77041 | 290236 | 3000065 | 785379 | 98.0% |
| symbols | 10569 | 19 | 100 | 703 | 2739 | 11487 | 57560 | 6606 | 0.8% |
| grep | 9227 | 16 | 90 | 292 | 527 | 1651 | 386711 | 1900 | 0.2% |
| librarian | 285 | 165 | 4464 | 14593 | 30080 | 71842 | 128955 | 1741 | 0.2% |
| references | 260 | 1711 | 5626 | 15786 | 29931 | 42166 | 55179 | 1466 | 0.2% |
| workspace | 889 | 127 | 723 | 1423 | 2214 | 34903 | 54618 | 1340 | 0.2% |
| edit_code | 4492 | 62 | 167 | 421 | 668 | 2124 | 45461 | 998 | 0.1% |
| doc | 6786 | 21 | 43 | 112 | 358 | 1028 | 10981 | 523 | 0.1% |
| index | 21 | 27498 | 33314 | 37470 | 38923 | 41542 | 42197 | 465 | 0.1% |
| memory | 1491 | 1 | 272 | 1141 | 1505 | 3002 | 4877 | 439 | 0.1% |
| edit_file | 8485 | 10 | 31 | 73 | 164 | 361 | 5014 | 303 | 0.0% |
| call_graph | 130 | 452 | 1904 | 5043 | 10765 | 27977 | 39788 | 296 | 0.0% |
| read_file | 19730 | 0 | 3 | 7 | 11 | 33 | 21487 | 93 | 0.0% |
| create_file | 3833 | 2 | 6 | 12 | 19 | 55 | 5033 | 48 | 0.0% |
| semantic_search | 14 | 844 | 1568 | 2079 | 3475 | 5521 | 6032 | 17 | 0.0% |
| tree | 173 | 1 | 12 | 40 | 108 | 947 | 9773 | 15 | 0.0% |
| symbol_at | 7 | 648 | 2831 | 5312 | 6018 | 6584 | 6725 | 13 | 0.0% |
| get_guide | 452 | 0 | 1 | 3 | 6 | 17 | 1850 | 2 | 0.0% |
| approve_write | 69 | 0 | 0 | 1 | 3 | 4 | 5 | 0 | 0.0% |

### EXCLUDING run_command wait loops and rows at timeout  (rows 116044, total 389357 s = 108.15 h)

| tool | n | p50 | p75 | p90 | p95 | p99 | max | sum_s | share |
|---|---|---|---|---|---|---|---|---|---|
| run_command | 49131 | 87 | 352 | 5599 | 31618 | 162467 | 2189500 | 373092 | 95.8% |
| symbols | 10569 | 19 | 100 | 703 | 2739 | 11487 | 57560 | 6606 | 1.7% |
| grep | 9227 | 16 | 90 | 292 | 527 | 1651 | 386711 | 1900 | 0.5% |
| librarian | 285 | 165 | 4464 | 14593 | 30080 | 71842 | 128955 | 1741 | 0.4% |
| references | 260 | 1711 | 5626 | 15786 | 29931 | 42166 | 55179 | 1466 | 0.4% |
| workspace | 889 | 127 | 723 | 1423 | 2214 | 34903 | 54618 | 1340 | 0.3% |
| edit_code | 4492 | 62 | 167 | 421 | 668 | 2124 | 45461 | 998 | 0.3% |
| doc | 6786 | 21 | 43 | 112 | 358 | 1028 | 10981 | 523 | 0.1% |
| index | 21 | 27498 | 33314 | 37470 | 38923 | 41542 | 42197 | 465 | 0.1% |
| memory | 1491 | 1 | 272 | 1141 | 1505 | 3002 | 4877 | 439 | 0.1% |
| edit_file | 8485 | 10 | 31 | 73 | 164 | 361 | 5014 | 303 | 0.1% |
| call_graph | 130 | 452 | 1904 | 5043 | 10765 | 27977 | 39788 | 296 | 0.1% |
| read_file | 19730 | 0 | 3 | 7 | 11 | 33 | 21487 | 93 | 0.0% |
| create_file | 3833 | 2 | 6 | 12 | 19 | 55 | 5033 | 48 | 0.0% |
| semantic_search | 14 | 844 | 1568 | 2079 | 3475 | 5521 | 6032 | 17 | 0.0% |
| tree | 173 | 1 | 12 | 40 | 108 | 947 | 9773 | 15 | 0.0% |
| symbol_at | 7 | 648 | 2831 | 5312 | 6018 | 6584 | 6725 | 13 | 0.0% |
| get_guide | 452 | 0 | 1 | 3 | 6 | 17 | 1850 | 2 | 0.0% |
| approve_write | 69 | 0 | 0 | 1 | 3 | 4 | 5 | 0 | 0.0% |

excluded run_command rows: 2234 sum_s 412288 of rc sum 785379
 wait only 2026 379428 s;  timeout only 176 11712 s; both 32 21148 s

### run_command by category (all rc rows; wait/timeout flagged separately)
| category | n | p50 | p90 | p99 | sum_s | share of rc time |
|---|---|---|---|---|---|---|
| WAITLOOP | 2058 | 90022 | 540091 | 1780088 | 400576 | 51.0% |
| cargo build/test/clippy/nextest | 3926 | 19050 | 154582 | 493539 | 239149 | 30.5% |
| other | 16044 | 60 | 1694 | 82370 | 57054 | 7.3% |
| python3 script | 4798 | 177 | 5719 | 139706 | 30310 | 3.9% |
| pytest/python tests | 1608 | 654 | 21870 | 169409 | 17713 | 2.3% |
| git | 11111 | 80 | 1699 | 10498 | 12058 | 1.5% |
| AT-TIMEOUT | 176 | 30044 | 60062 | 975055 | 11712 | 1.5% |
| gate.sh | 672 | 33 | 281 | 275879 | 7297 | 0.9% |
| no-input-recorded | 9310 | 71 | 628 | 26343 | 7053 | 0.9% |
| docker/other build | 509 | 238 | 3758 | 30987 | 1162 | 0.1% |
| sqlite3 | 1092 | 138 | 1870 | 12566 | 1094 | 0.1% |
| curl/ssh/net | 61 | 251 | 8028 | 38928 | 202 | 0.0% |

rc overall p50 p90 p99 max [94, 10725, 290236] 3000065
slowest 512: wait 344 to 16 either 349 no cmd 0
```

## RAW lat_q2c.out
```
| wait-loop kind | n | sum_s | p50_s |
|---|---|---|---|
| polls a @bg_ job (until/while ... @bg_N) | 539 | 114372 | 118 |
| pgrep -f / pgrep | 216 | 27414 | 1 |
| polls a file/grep (until grep -q / [ -f ]) not @bg | 400 | 34187 | 0 |
| plain sleep N only (no until/while) | 1414 | 324821 | 118 |
total 2058 400576
parent vs subagent wait loops: {False: (983, 80884), True: (1075, 319692)}
top sessions by wait-loop time s [('500c9b13', 112577), ('e98b5705', 102323), ('c2439466', 34015), ('8edd247a', 21338), ('6b17822c', 19160)] distinct sessions 172
run_in_background rows 2596 p50 28 max 22263
[]
wait loops by week {1: (827, 209841), 2: (357, 101350), 3: (513, 79217), 4: (247, 7561), 5: (114, 2607)}
```

## RAW lat_q3.out
```

=== librarian: n=285, tail size 20, threshold>= 20022 ms; tail time 1066s = 61% of tool time
 outcomes {'success': 20} overflowed 8
 sessions [('e4a5a540', 2), ('577dfc36', 2), ('4a3cffec', 2), ('e98b5705', 1), ('3d54d0a5', 1)] distinct 17
 agent(sub) 1 / 20  roots Counter({'main': 19, 'wt/other': 1})
 action [('reindex', 9), (None, 9), ('audit_doc_refs', 1), ('doctor', 1)]  has_input 11
   233743 10-30 08:09 128955 succ gap 1 | {"action":"audit_doc_refs","path":"docs/trackers/resume-queue-index.md"}
   233652 10-30 08:00 127341 succ gap 20 | {"action":"reindex","path":"docs/trackers/friction-deep-dive-2026-10.md"}
   203503 10-18 05:44 111498 succ gap 1 | {"action":"reindex"}
   184576 10-13 23:00 64288 succ gap 2 | {"action":"reindex"}
   235602 10-30 17:22 60339 succ gap 1 | 
   235568 10-30 17:13 58953 succ gap 1 | 
   190226 10-16 14:34 55211 succ gap 3 | 
   235569 10-30 17:14 55007 succ gap 1 | 

=== workspace: n=889, tail size 20, threshold>= 24644 ms; tail time 720s = 54% of tool time
 outcomes {'success': 20} overflowed 0
 sessions [('3fb0fe28', 3), ('1854a32b', 3), ('7bd1f3c0', 2), ('8c9fc832', 1), ('824d0089', 1)] distinct 15
 agent(sub) 1 / 20  roots Counter({'main': 20})
 action [(None, 10), ('status', 10)]  has_input 10
   235219 10-30 12:59 54618 succ gap 1 | 
   235222 10-30 13:00 52470 succ gap 1 | 
   208957 10-19 10:50 48789 succ gap 2 | {"action":"status"}
   235230 10-30 13:04 45054 succ gap 1 | 
   229262 10-29 17:05 39212 succ gap 2 | {"action":"status"}
   226583 10-29 10:16 38983 succ gap 2 | {"action":"status"}
   235223 10-30 13:00 37679 succ gap 2 | 
   210494 10-20 09:17 36947 succ gap 2 | {"action":"status"}

=== doc: n=6786, tail size 67, threshold>= 1039 ms; tail time 146s = 28% of tool time
 outcomes {'success': 63, 'recoverable_error': 4} overflowed 6
 sessions [('879d21b0', 10), ('2e2f8719', 9), ('3d9ab02a', 7), ('ac7fb73b', 5), ('7b6cc957', 4)] distinct 30
 agent(sub) 2 / 67  roots Counter({'main': 60, 'wt/other': 7})
 action [('update', 19), ('move', 19), ('find', 14), (None, 8), ('create', 6), ('event_create', 1)]  has_input 59
   239942 10-31 10:47 10981 succ gap 179 | {"action":"update","id":"c351f4a6969588c7","patch":{"body_edits":[{"action":"replace","heading":"## Fix","content":"## Fix\n\nDone 2026-10-31 in 16446
   268124 11-05 15:57 5028 reco gap 1653 | {"action":"update","id":"241d6eb64c605b37","patch":{"body_edits":[{"heading":"## Tests added","action":"replace","content":"Two through-grade methods 
   286013 11-09 11:44 5028 reco gap 5 | {"action":"update","id":"ac1441d0f2ebb384","patch":{"body_edits":[{"heading":"## Execution ledger","action":"insert_after","content":"\n- RED observed
   295102 11-10 11:17 5011 reco gap 1045 | {"action":"update","id":"5dc9a421a504e0cd","workspace":"/home/marius/work/claude/codescout","patch":{"body_edits":[{"heading":"## Fixed oracle diagnos
   288947 11-09 17:10 5010 reco gap 2 | {"action":"update","id":"f04ab8ce49bf87e3","patch":{"body_edits":[{"heading":"### Shared embedding transport addendum — 2026-11-09","action":"insert_a
   280142 11-08 10:00 4872 succ gap 4 | {"action":"event_create","id":"89f4456f348addf5","event":{"kind":"note","payload":{"text":"Author job5c closed exit0 with valid frozen output, cost US
   239787 10-31 10:33 4141 succ gap 1 | 
   241478 10-31 16:23 4081 succ gap 1 | {"action":"move","id":"ecb3508a7f51f803","new_rel_path":"docs/issues/archive/2026-10-26-the-worktree-write-guard-ignores-the-target-and-refuses-writes

=== index: n=21, tail size 20, threshold>= 140 ms; tail time 465s = 100% of tool time
 outcomes {'success': 20} overflowed 0
 sessions [('cb2b0864', 6), ('7bd1f3c0', 2), ('46d0d45e', 2), ('5d96a3b1', 2), ('e4a5a540', 1)] distinct 12
 agent(sub) 1 / 20  roots Counter({'main': 20})
 action [('status', 9), (None, 6), ('verify', 3), ('build', 2)]  has_input 14
   223554 10-28 05:13 42197 succ gap 154 | {"action":"status"}
   194998 10-17 07:02 38923 succ gap 1 | 
   223673 10-28 06:34 37470 succ gap 140 | {"action":"status"}
   223571 10-28 05:16 37349 succ gap 1 | {"action":"status"}
   233590 10-30 07:54 33591 succ gap 2 | {"action":"verify"}
   230338 10-29 20:01 33314 succ gap 1 | {"action":"status"}
   229408 10-29 17:16 30084 succ gap 1 | {"action":"status"}
   212385 10-21 04:41 29800 succ gap 385 | 

=== memory: n=1491, tail size 20, threshold>= 2886 ms; tail time 69s = 16% of tool time
 outcomes {'success': 20} overflowed 0
 sessions [('0e844aa1', 7), ('09de233d', 2), ('9f2efa08', 2), ('cb06fa74', 2), ('3d9ab02a', 2)] distinct 10
 agent(sub) 2 / 20  roots Counter({'main': 20})
 action [(None, 11), ('write', 9)]  has_input 9
   245367 11-01 11:07 4877 succ gap 1650 | 
   227357 10-29 12:06 4538 succ gap 2957 | {"action":"write","topic":"research/cross-harness-session-messaging","content":"# Cross-harness session messaging and wake paths (measured 2026-10-29)
   237195 10-31 04:46 4090 succ gap 1 | {"action":"write","content":"# Setup A/B eval (codescout vs bare vs full) — status as of 2026-10-31 06:50\n\nBranch `research/setup-ab-eval-20261027` 
   221602 10-27 08:12 3986 succ gap 123 | {"action":"write","topic":"bug-triage-2026-10-27","content":"# Open-bug triage, 2026-10-27 (session c78f48d5, branch `experiments`)\n\nRule from the u
   247692 11-01 14:22 3763 succ gap 1631 | {"action":"write","topic":"research/gate-speed-20261101","content":"# Gate speed measurements (2026-11-01)\n\nQuestion: what in `./scripts/gate.sh` is
   238539 10-31 08:06 3676 succ gap 8 | 
   209735 10-19 14:50 3586 succ gap 1 | {"action":"write","topic":"catalog-sql-hazards","content":"# Catalog SQL & Migration Hazards\n\nDiscovered the hard way during entry-graph Stage 2 (20
   231804 10-29 22:25 3568 succ gap 2 | 

=== edit_code: n=4492, tail size 44, threshold>= 2154 ms; tail time 354s = 35% of tool time
 outcomes {'success': 22, 'recoverable_error': 22} overflowed 2
 sessions [('74cfdeb6', 9), ('500c9b13', 7), ('9b43f888', 6), ('e98b5705', 4), ('97a050d1', 2)] distinct 20
 agent(sub) 12 / 44  roots Counter({'main': 30, 'wt/other': 14})
 action [('insert', 26), ('rename', 10), ('replace', 6), ('remove', 1), (None, 1)]  has_input 43
   231712 10-29 22:20 45461 succ gap 35 | {"action":"insert","body":"/// A worktree can delete only what it holds. A topic only the main checkout holds is refused by\n/// name, because answeri
   235929 10-30 19:04 34384 succ gap 180 | {"action":"remove","path":"src/librarian/tools/reindex.rs","symbol":"UPSERT_BATCH"}
   235936 10-30 19:05 24119 succ gap 1 | {"action":"insert","body":"/// Write `(chunk_id, artifact_id, vector)` tuples to `store` in batches, stopping at the first\n/// store error. Both ids 
   249296 11-01 19:36 16041 reco gap 1 | {"action":"insert","body":"\n/// Windows spells one location several ways: `canonicalize` answers `\\\\?\\C:\\Users\\runneradmin\\…`\n/// while a call
   245001 11-01 10:38 15498 reco gap 2 | {"action":"insert","body":"\n    /// A result WITH matches skipped `.github/` too, and said nothing: the reader got a partial\n    /// list that read 
   240698 10-31 12:52 15318 reco gap 1 | {"action":"insert","body":"\n    /// An environment that points both history scans of the engine at an empty directory, so a\n    /// test that needs 
   216924 10-24 04:22 15300 reco gap 1 | {"action":"insert","body":"    /// The dispatch boundary rewrites aliases before `call()` runs (`call_content`\n    /// → `normalize_params`). A direc
   240964 10-31 14:08 15292 reco gap 2 | {"action":"insert","body":"    /// `call` used to run `create_dir_all(parent)` before the validators that can still refuse, so a\n    /// refused crea

=== symbols: n=10569, tail size 105, threshold>= 11537 ms; tail time 2388s = 36% of tool time
 outcomes {'success': 105} overflowed 11
 sessions [('500c9b13', 27), ('5b936994', 11), ('e8e6aebc', 8), ('cef93915', 6), ('0e844aa1', 6)] distinct 33
 agent(sub) 54 / 105  roots Counter({'wt/other': 61, 'main': 44})
 action [(None, 105)]  has_input 92
   294933 11-10 11:05 57560 succ gap 184 | {"name":"CachedResolver","path":".worktrees/ecc-resolver-followup-20261110/src/tools/symbol","workspace":"/home/marius/work/claude/codescout"}
   294932 11-10 11:05 57558 succ gap 4 | {"name":"bfs","path":".worktrees/ecc-resolver-followup-20261110/src/tools/symbol","include_body":true,"workspace":"/home/marius/work/claude/codescout"
   286621 11-09 12:23 56393 succ gap 1 | {"path":"src/observations/server.rs","name":"ObservationContext/from_meta","include_body":true,"workspace":"/tmp/codescout-stock-observations-20261109
   286620 11-09 12:23 51278 succ gap 9 | {"path":"src/tools/invocation_context.rs","name":"codex_fields","include_body":true,"workspace":"/tmp/codescout-stock-observations-20261109"}
   286619 11-09 12:23 47364 succ gap 6 | {"path":"src/observations/server.rs","name":"observe_write","include_body":true,"workspace":"/tmp/codescout-stock-observations-20261109"}
   244173 11-01 09:29 45381 succ gap 9 | 
   286618 11-09 12:23 42382 succ gap 1 | {"path":"src/observations/server.rs","name":"LocalInvocationId","include_body":true,"workspace":"/tmp/codescout-stock-observations-20261109"}
   238881 10-31 08:40 41285 succ gap 2 | 

=== read_file: n=19730, tail size 197, threshold>= 33 ms; tail time 52s = 56% of tool time
 outcomes {'success': 142, 'recoverable_error': 55} overflowed 0
 sessions [('500c9b13', 29), ('e98b5705', 24), ('cb06fa74', 15), ('4b1c0778', 12), ('3d9ab02a', 8)] distinct 65
 agent(sub) 92 / 197  roots Counter({'main': 102, 'wt/other': 95})
 action [(None, 197)]  has_input 167
   195322 10-17 07:08 21487 succ gap 631 | {"path":"docs/PROGRESSIVE_DISCOVERABILITY.md","workspace":"/home/marius/work/claude/codescout/.claude/worktrees/agent-ab12363de3a0d43f1"}
   254456 11-03 06:29 7364 succ gap 1 | {"path":"docs/RELEASE.md","heading":"## Citing a fix"}
   195318 10-17 07:08 4572 succ gap 80 | {"end_line":3135,"force":true,"path":"src/librarian/adapter.rs","start_line":3040,"workspace":"/home/marius/work/claude/codescout/.claude/worktrees/ag
   194127 10-17 06:01 679 succ gap 445 | {"end_line":8230,"force":true,"path":"src/librarian/tools/doctor.rs","start_line":8060,"workspace":"/home/marius/work/claude/codescout/.claude/worktre
   216077 10-24 03:32 659 reco gap 3740 | {"heading":"## Main taxonomy","path":"docs/TAXONOMY.md"}
   217992 10-24 08:44 570 succ gap 174 | {"path":"docs/issues/2026-10-24-onboarding-preferences-section-reads-the-default-projects-memory-store-not-the-pinned-one.md","force":true,"workspace"
   194880 10-17 07:00 563 succ gap 81 | {"path":"docs/adrs/2026-10-16-the-reasoning-for-a-class-member-lives-in-the-members-own-bug-file.md","workspace":"/home/marius/work/claude/codescout",
   225428 10-28 16:07 415 succ gap 831 | 

=== edit_file: n=8485, tail size 84, threshold>= 364 ms; tail time 66s = 22% of tool time
 outcomes {'recoverable_error': 2, 'success': 82} overflowed 0
 sessions [('500c9b13', 32), ('e98b5705', 14), ('8f842cf1', 4), ('6b17822c', 4), ('f844662c', 3)] distinct 23
 agent(sub) 64 / 84  roots Counter({'wt/other': 60, 'main': 24})
 action [(None, 84)]  has_input 84
   285192 11-09 11:05 5014 reco gap 1574 | {"new_string":"                found.sort_unstable();\n                return (found, false /* MUTATION */);","old_string":"                found.sort
   285193 11-09 11:05 5014 reco gap 2 | {"new_string":"                found.sort_unstable();\n                return (found, false /* MUTATION */);","old_string":"                found.sort
   239508 10-31 10:18 2675 succ gap 3834 | {"new_string":"            crate::workspace::ProjectState::Activated(active) => active.config.project.name.clone(),\n            crate::workspace::Pro
   204587 10-18 07:37 2218 succ gap 2 | {"new_string":"#[cfg(test)]\n#[path = \"server/config_reload_tests.rs\"]\nmod config_reload_tests;\n\n#[cfg(test)]\n#[path = \"server/protected_config
   218676 10-24 12:01 2005 succ gap 2 | {"path":"/home/marius/.cache/cc-tmp/claude-1000/-home-marius-work-claude-codescout/74be4cd2-79e7-4b0a-a59c-95610e1f5c4a/scratchpad/prune-merged.sh","o
   207053 10-18 13:14 1752 succ gap 2 | {"new_string":"            None => return Ok(None),","old_string":"            None | Some(Value::Null) => return Ok(None),","path":"src/server.rs","w
   206744 10-18 12:36 1379 succ gap 2 | {"new_string":"        // A `workspace=` pin that is not an existing directory is refused HERE, before anything\n        // resolves it: every later c
   193368 10-16 18:40 1378 succ gap 2 | {"new_string":"        std::fs::write(\n            sub.join(\"package.json\"),\n            r#\"{\"name\":\"api\",\"scripts\":{\"build\":\"tsc\"}}\"#

=== create_file: n=3833, tail size 38, threshold>= 56 ms; tail time 31s = 64% of tool time
 outcomes {'recoverable_error': 4, 'success': 34} overflowed 0
 sessions [('500c9b13', 9), ('9fba5b92', 6), ('3d9ab02a', 5), ('4a3d0b5a', 3), ('06e2ecbc', 3)] distinct 16
 agent(sub) 17 / 38  roots Counter({'main': 33, 'wt/other': 5})
 action [(None, 38)]  has_input 35
   285053 11-09 10:53 5033 reco gap 843 | {"workspace":"/home/marius/work/claude/codescout","path":"/tmp/verify-default-ecc-20261109.py","content":"\"\"\"Read native skill discovery and verify
   291376 11-09 21:38 5012 reco gap 431 | {"content":"docs(issues): record the follow-up note to the fence owner (bug 312cde2f4698874b)\n\nThe follow-up went to Codex thread 01a1b59b at 2026-1
   284104 11-09 10:07 5008 reco gap 5193 | {"content":"fix(doc): refuse dry_run=true instead of dropping it and performing the write\n\n`doc(action=\"update\", dry_run=true, ...)` wrote the fil
   282746 11-09 07:53 5006 reco gap 3 | {"content":"x\n","path":"target/lock-probe.txt","workspace":"/home/marius/work/claude/codescout"}
   207329 10-18 13:58 3391 succ gap 3332 | {"content":"\n// ---- REVIEW2 (scratch only): layouts the author's table lacks -------------------------------\n\nconst REL_SITES: &[(&str, &str)] = &
   269239 11-05 19:00 2320 succ gap 7 | {"path":"/tmp/external-memory-cache-01a15ca9-20261105.py","content":"import hashlib,json,pathlib,shutil,subprocess,sys,time,tomllib\nBASE=pathlib.Path
   206627 10-18 12:06 924 succ gap 231 | {"content":"docs(issues): close the Qdrant-host retry gap after the fix landed in 3f1986461\n\nThe record now holds the fix (SHA and patch-id), the fo
   269849 11-06 07:38 679 succ gap 6 | {"path":"/tmp/qualify-final-tmp-exec-01a15ca9-20261106.py","content":"import importlib.util,pathlib\np='/tmp/qualify-executable-tmp-01a15ca9-20261106.

=== grep: n=9227, tail size 92, threshold>= 1659 ms; tail time 1034s = 54% of tool time
 outcomes {'success': 92} overflowed 11
 sessions [('500c9b13', 18), ('e98b5705', 13), ('0cf1f7af', 6), ('10451d0d', 5), ('f079f887', 3)] distinct 41
 agent(sub) 56 / 92  roots Counter({'main': 52, 'wt/other': 40})
 action [(None, 92)]  has_input 91
   210368 10-20 09:07 386711 succ gap 1 | {"pattern":"\\bJev\\b|\\bjev\\b|JEV","mode":"files","path":"/home/marius/work/claude","limit":20}
   208396 10-19 08:12 135109 succ gap 5 | {"pattern":"SessionMessagesArgs","path":"/home/marius","glob":"**/*.d.ts","mode":"files","include_hidden":true,"limit":5}
   226057 10-29 06:50 99824 succ gap 1 | 
   262260 11-04 18:02 20009 succ gap 1 | {"glob":"*.py","path":"/home/marius/work","pattern":"def link_credentials|link_credentials\\(","limit":20}
   279892 11-08 08:45 20003 succ gap 1 | {"mode":"files","path":"/home/marius","glob":"sessions/*.json","include_hidden":true,"pattern":"6821c9b3-d613-4fb6-ac7f-ebdfb38e3cdd"}
   245666 11-01 11:28 20002 succ gap 1 | {"head_limit":"25","mode":"files","path":"/home/marius/work/claude","glob":"!**/resident-private/**","pattern":"resident-private","workspace":"/home/m
   294209 11-10 09:09 20002 succ gap 2 | {"glob":["**/*"],"limit":15,"mode":"files","path":"/home/marius/work/claude/claude-plugins","pattern":"subagent-guidance"}
   258240 11-04 08:40 20001 succ gap 127 | {"pattern":"subagent output matched instruction-shaped","path":"/home/marius/work/claude","include_hidden":true,"limit":20}

=== references: n=260, tail size 20, threshold>= 21113 ms; tail time 658s = 45% of tool time
 outcomes {'success': 15, 'recoverable_error': 5} overflowed 1
 sessions [('500c9b13', 3), ('ac7fb73b', 2), ('bd703e6b', 2), ('cf34db4d', 2), ('b9bb978e', 1)] distinct 15
 agent(sub) 4 / 20  roots Counter({'main': 12, 'wt/other': 8})
 action [(None, 20)]  has_input 20
   206354 10-18 11:43 55179 succ gap 23 | {"symbol":"MemoryStore/read","path":"src/memory/mod.rs","workspace":"/home/marius/work/claude/codescout/.claude/worktrees/agent-af66a5c7595cbbc58"}
   292706 11-10 01:11 45003 succ gap 1 | {"workspace":"/home/marius/.codex/worktrees/cf66/codescout","path":"src/tools/run_command/attribution.rs","symbol":"AttributionEnv","limit":100}
   292710 11-10 01:22 45002 succ gap 2 | {"workspace":"/home/marius/.codex/worktrees/cf66/codescout","path":"src/tools/run_command/attribution.rs","symbol":"AttributionEnv","limit":100}
   231277 10-29 21:36 40196 succ gap 1 | {"path":"src/util/path_security.rs","symbol":"worktree_main_root","workspace":"/home/marius/work/claude/codescout"}
   229156 10-29 16:59 40115 reco gap 2 | {"symbol":"resolve_head_sha","path":"src/agent/mod.rs"}
   229157 10-29 16:59 40037 reco gap 4 | {"symbol":"probe_has_git_remote","path":"src/agent/mod.rs"}
   186736 10-14 16:31 33921 succ gap 0 | {"path":"src/librarian/workspace.rs","symbol":"load"}
   225812 10-29 05:14 32547 succ gap 2 | {"path":"src/retrieval/index_state.rs","symbol":"head_commit_full"}

=== call_graph: n=130, tail size 20, threshold>= 2930 ms; tail time 220s = 75% of tool time
 outcomes {'success': 20} overflowed 0
 sessions [('879d21b0', 2), ('60ee9512', 2), ('a38ec0e8', 2), ('500c9b13', 1), ('b10f3069', 1)] distinct 17
 agent(sub) 3 / 20  roots Counter({'wt/other': 10, 'main': 10})
 action [(None, 20)]  has_input 12
   208692 10-19 10:11 39788 succ gap 5 | {"direction":"callers","max_depth":3,"path":"src/lsp/manager.rs","symbol":"LspManager/get_or_start_via_mux_waiting","workspace":"/home/marius/work/cla
   285155 11-09 11:01 30329 succ gap 2132 | {"workspace":"/home/marius/work/claude/codescout","path":"src/librarian/tools/create.rs","symbol":"call","direction":"callers","max_depth":2}
   247930 11-01 14:45 22219 succ gap 5 | {"symbol":"handle_successful_output_with","path":"src/tools/run_command/output.rs","direction":"callers","max_depth":6}
   285852 11-09 11:37 21317 succ gap 0 | {"symbol":"undo_failed_create","path":"src/librarian/tools/create.rs","direction":"both","max_depth":1,"workspace":"/home/marius/work/claude/codescout
   185158 10-14 08:12 17209 succ gap 0 | 
   203001 10-18 00:57 12597 succ gap 2 | {"direction":"callers","max_depth":3,"path":"src/librarian/catalog/augmentation.rs","symbol":"allocate_entry_id","workspace":"/home/marius/.cache/cc-t
   187219 10-14 18:37 11219 succ gap 1 | 
   285252 11-09 11:07 10211 succ gap 1 | {"symbol":"call","path":"src/librarian/tools/create.rs","direction":"callers","max_depth":1,"workspace":"/home/marius/work/claude/codescout"}

=== tree: n=173, tail size 20, threshold>= 35 ms; tail time 14s = 95% of tool time
 outcomes {'success': 20} overflowed 0
 sessions [('500c9b13', 3), ('4b1c0778', 3), ('5cf07215', 2), ('41fea53e', 2), ('09de233d', 2)] distinct 13
 agent(sub) 12 / 20  roots Counter({'main': 16, 'wt/other': 4})
 action [(None, 20)]  has_input 20
   298050 11-10 21:12 9773 succ gap 2 | {"workspace":"/home/marius/.codex/worktrees/cf66/codescout","path":"/home/marius/.cache/codescout-resident/codescout","glob":"*shared-public*","limit"
   193974 10-17 06:00 2878 succ gap 80 | {"glob":"**/tracker-conventions*","workspace":"/home/marius/work/claude/codescout/.claude/worktrees/agent-a666e4cdd398d0f53"}
   238617 10-31 08:21 196 succ gap 2133 | {"glob":"src/librarian/tools/update*.rs","workspace":"/home/marius/work/claude/codescout"}
   194884 10-17 07:00 175 succ gap 3 | {"glob":"docs/issues/**/2026-10-16-the-growth-gate-accepts*","workspace":"/home/marius/work/claude/codescout"}
   277811 11-07 19:00 145 succ gap 4 | {"glob":"tests/*build*"}
   277301 11-07 18:32 137 succ gap 1 | {"glob":"docs/issues/archive/2026-06-02-preflight*","workspace":"/home/marius/work/claude/codescout"}
   277863 11-07 19:05 120 succ gap 2132 | {"glob":"{scripts/*,.pre-commit-config.yaml,.buddy/**,AGENTS.md,CLAUDE.local.md,.claude/skills/**}","include_hidden":true,"workspace":"/home/marius/wo
   298302 11-10 21:57 117 succ gap 0 | {"glob":"docs/trackers/*.md","limit":80}

=== get_guide: n=452, tail size 20, threshold>= 7 ms; tail time 2s = 89% of tool time
 outcomes {'success': 20} overflowed 0
 sessions [('e98b5705', 7), ('500c9b13', 2), ('5b936994', 2), ('2f5f98b2', 2), ('67b8b982', 1)] distinct 11
 agent(sub) 16 / 20  roots Counter({'main': 14, 'wt/other': 6})
 action [(None, 20)]  has_input 19
   200508 10-17 16:06 1850 succ gap 2 | {"topic":"tracker-conventions"}
   197785 10-17 12:04 80 succ gap 445 | {"topic":"error-handling"}
   194417 10-17 06:18 20 succ gap 630 | {"topic":"error-handling"}
   217904 10-24 08:36 19 succ gap 2763 | 
   190998 10-16 14:48 18 succ gap 2045 | {"topic":"librarian"}
   287107 11-09 13:07 16 succ gap 3173 | {"topic":"librarian"}
   190527 10-16 14:41 12 succ gap 40 | {"topic":"error-handling"}
   215753 10-24 03:13 12 succ gap 4859 | {"topic":"tracker-conventions"}

=== semantic_search: n=14, tail size 14, threshold>= 22 ms; tail time 17s = 100% of tool time
 outcomes {'success': 14} overflowed 0
 sessions [('828a9e6e', 2), ('66c7411c', 1), ('2f5f98b2', 1), ('c2439466', 1), ('cf34db4d', 1)] distinct 13
 agent(sub) 7 / 14  roots Counter({'main': 10, 'wt/other': 4})
 action [(None, 14)]  has_input 13
   186447 10-14 14:26 6032 succ gap 2 | {"query":"onboarding sets up librarian for a new project, creates docs directories and trackers","mode":"full","limit":8}
   188823 10-16 08:27 2098 succ gap 2 | {"limit":5,"query":"reindex attaches augmentation from committed sidecar when catalog row absent, params empty"}
   222813 10-27 19:13 2034 succ gap 2 | {"query":"semantic memory remember storage backend qdrant collection for memories","limit":6}
   223992 10-28 07:49 1652 succ gap 2 | {"query":"index collection name keyed by project root or project id; per-project vector index scope","limit":8}
   184749 10-14 06:27 1315 succ gap 1 | 
   237429 10-31 05:11 1166 succ gap 331 | {"query":"Probe memory for timing the write path. The reindex skip keeps a marker per chunk. The embed queue walks every stored chunk row. replace_chu
   182029 10-13 06:33 983 succ gap 1 | {"limit":5,"mode":"full","query":"register codescout MCP server with Codex CLI config.toml"}
   277721 11-07 18:59 706 succ gap 1 | {"query":"how the per-call session id for a Codex native call is resolved from MCP request meta"}

=== symbol_at: n=7, tail size 7, threshold>= 2 ms; tail time 13s = 100% of tool time
 outcomes {'success': 7} overflowed 0
 sessions [('8498202d', 4), ('6b17822c', 1), ('e98b5705', 1), ('5d96a3b1', 1)] distinct 4
 agent(sub) 2 / 7  roots Counter({'main': 5, 'wt/other': 2})
 action [(None, 7)]  has_input 6
   278448 11-07 20:49 6725 succ gap 2 | {"col":36,"line":289,"path":"src/tools/grep.rs","workspace":"/home/marius/work/claude/codescout"}
   212657 10-21 05:11 4370 succ gap 1 | {"line":2955,"path":"src/server.rs","workspace":"/home/marius/work/claude/codescout/.claude/worktrees/agent-ab5bd161dcd760c57"}
   198792 10-17 13:00 1292 succ gap 684 | {"path":"src/server.rs","line":16035,"identifier":"move","workspace":"/home/marius/work/claude/codescout/.claude/worktrees/agent-ad70a7016cb7773af"}
   184777 10-14 06:31 648 succ gap 2 | 
   278440 11-07 20:49 199 succ gap 1 | {"path":"src/tools/grep.rs","line":289,"column":"35","workspace":"/home/marius/work/claude/codescout"}
   278441 11-07 20:49 2 succ gap 2 | {"col":32,"line":289,"path":"src/tools/grep.rs","workspace":"/home/marius/work/claude/codescout"}
   278445 11-07 20:49 2 succ gap 2 | {"col":36,"line":289,"path":"src/tools/grep.rs","workspace":"/home/marius/work/claude/codescout"}
```

## RAW lat_q3c.out
```
SYMBOLS modes
  name+path n=5610 p50=20 p90=308 p99=5952 max=57560 sum_s=2419
  path-only(overview) n=1458 p50=42 p90=763 p99=6892 max=33471 sum_s=665
  no-input n=3276 p50=12 p90=662 p99=10376 max=45381 sum_s=1528
  name-only(project-wide) n=225 p50=10583 p90=12335 p99=14268 max=29850 sum_s=1993
   ('substr-name', 'body', 'scoped') n=4249 p50=19 p90=312 p99=5426 max=57558 sum_s=1568
   ('substr-name', 'body', 'proj') n=159 p50=10583 p90=12298 p99=14217 max=15513 sum_s=1395
   ('exact-namepath', 'body', 'scoped') n=1124 p50=17 p90=230 p99=5867 max=56393 sum_s=549
   ('exact-namepath', 'body', 'proj') n=41 p50=10924 p90=12904 p99=23343 max=29850 sum_s=399
   ('substr-name', 'nobody', 'scoped') n=220 p50=51 p90=1016 p99=36383 max=57560 sum_s=249
   ('substr-name', 'nobody', 'proj') n=24 p50=9754 p90=11472 p99=11912 max=12024 sum_s=198
   ('exact-namepath', 'nobody', 'scoped') n=17 p50=60 p90=5223 p99=34963 max=40597 sum_s=52
   ('exact-namepath', 'nobody', 'proj') n=1 p50=1600 p90=1600 p99=1600 max=1600 sum_s=2
  path-only file n=1396 p50=39 p90=752 p99=5255 max=33471 sum_s=574
  path-only dir/glob n=62 p50=171 p90=1032 p99=22765 max=25903 sum_s=91
 tail modes Counter({'name-only(project-wide)': 56, 'name+path': 30, 'no-input': 13, 'path-only(overview)': 6}) tail with workspace param 72
 tail time share by mode {'name+path': 1119, 'path-only(overview)': 157, 'no-input': 397, 'name-only(project-wide)': 716}
 tail clusters (10-min buckets with >=4): [('2026-10-17 07:0', 12), ('2026-10-18 09:0', 4), ('2026-10-24 03:1', 6), ('2026-10-24 03:4', 4), ('2026-10-24 08:4', 7), ('2026-10-29 21:4', 4), ('2026-11-09 12:2', 6)]
 symbols lat in [19900,20300] 0 [11000,20000) 87 >20300 41

WORKSPACE by action
  activate n=430 p50=132 p90=1070 p99=2164 max=2403 sum_s=160
  no-input n=400 p50=74 p90=1770 p99=36304 max=54618 sum_s=652
  status n=56 p50=654 p90=31270 p99=43522 max=48789 sum_s=527
  post_compact n=3 p50=0 p90=0 p99=0 max=0 sum_s=0

LIBRARIAN by action
  reindex n=25 p50=13277 p90=58176 p99=123539 max=127341 sum_s=657
  no-input n=111 p50=96 p90=14219 p99=58579 max=60339 sum_s=538
  doctor n=55 p50=6909 p90=14334 p99=19112 max=21560 sum_s=383
  audit_doc_refs n=27 p50=85 p90=217 p99=95636 max=128955 sum_s=132
  link_scan n=15 p50=1527 p90=2596 p99=2977 max=3007 sum_s=25
  context n=1 p50=2381 p90=2381 p99=2381 max=2381 sum_s=2
  merge_worktree n=39 p50=11 p90=98 p99=652 max=786 sum_s=2
  tracker_design n=8 p50=6 p90=24 p99=26 max=26 sum_s=0
  status n=3 p50=1 p90=3 p99=3 max=3 sum_s=0
  frobnicate n=1 p50=2 p90=2 p99=2 max=2 sum_s=0

 DOC by action
   find n=631 p50=85 p90=512 p99=1591 max=2397 sum_s=139
   update n=2335 p50=23 p90=58 p99=776 max=10981 sum_s=138
   move n=235 p50=395 p90=892 p99=1858 max=4081 sum_s=105
   no-input n=1452 p50=19 p90=73 p99=681 max=4141 sum_s=75
   create n=622 p50=27 p90=64 p99=762 max=4073 sum_s=41
   get n=1342 p50=3 p90=21 p99=76 max=1019 sum_s=14
   event_create n=18 p50=14 p90=32 p99=4051 max=4872 sum_s=5
   append_entry n=111 p50=26 p90=59 p99=146 max=371 sum_s=4

 MEMORY by action
   write n=209 p50=1015 p90=1726 p99=3968 max=4538 sum_s=229
   no-input n=548 p50=1 p90=1315 p99=3178 max=4877 sum_s=205
   delete n=108 p50=20 p90=45 p99=174 max=250 sum_s=3
   recall n=9 p50=53 p90=203 p99=232 max=235 sum_s=1
   read n=579 p50=0 p90=3 p99=13 max=43 sum_s=1
   remember n=5 p50=72 p90=138 p99=143 max=144 sum_s=0
   refresh_anchors n=17 p50=3 p90=6 p99=10 max=10 sum_s=0
   list n=12 p50=0 p90=14 p99=26 max=27 sum_s=0

 INDEX by action
   status n=9 p50=30084 p90=38415 p99=41819 max=42197 sum_s=230
   no-input n=6 p50=28672 p90=34362 p99=38467 max=38923 sum_s=172
   verify n=3 p50=27498 p90=32372 p99=33469 max=33591 sum_s=63
   build n=3 p50=140 p90=196 p99=209 max=210 sum_s=0

 EDIT_CODE by action
   insert n=1610 p50=96 p90=523 p99=3189 max=45461 sum_s=525
   replace n=1781 p50=46 p90=307 p99=1015 max=8881 sum_s=245
   remove n=246 p50=103 p90=470 p99=992 max=34384 sum_s=82
   rename n=57 p50=988 p90=2476 p99=7627 max=12216 sum_s=81
   no-input n=798 p50=34 p90=185 p99=629 max=5074 sum_s=65

 EDIT_FILE by action
   noaction n=7363 p50=12 p90=83 p99=386 max=5014 sum_s=294
   no-input n=606 p50=4 p90=19 p99=79 max=349 sum_s=6
   edit n=195 p50=3 p90=18 p99=56 max=144 sum_s=1
   insert_after n=111 p50=4 p90=19 p99=58 max=105 sum_s=1
   replace n=146 p50=2 p90=9 p99=24 max=66 sum_s=1
   insert_before n=51 p50=3 p90=22 p99=37 max=38 sum_s=0
   insert n=4 p50=0 p90=2 p99=3 max=3 sum_s=0
   remove n=4 p50=1 p90=1 p99=1 max=1 sum_s=0

 CREATE_FILE by action
   noaction n=3062 p50=2 p90=12 p99=61 max=5033 sum_s=44
   no-input n=771 p50=3 p90=12 p99=35 max=89 sum_s=4

 GREP by action
   noaction n=9047 p50=16 p90=295 p99=1656 max=386711 sum_s=1789
   no-input n=180 p50=10 p90=149 p99=1085 max=99824 sum_s=111

 REFERENCES by action
   noaction n=227 p50=1962 p90=19064 p99=43752 max=55179 sum_s=1381
   no-input n=33 p50=1402 p90=5036 p99=16877 max=19187 sum_s=84

 CALL_GRAPH by action
   noaction n=45 p50=458 p90=11643 p99=35626 max=39788 sum_s=182
   no-input n=85 p50=445 p90=2595 p99=12177 max=17209 sum_s=114

 TREE by action
   noaction n=145 p50=2 p90=44 p99=1698 max=9773 sum_s=15
   no-input n=28 p50=0 p90=4 p99=10 max=11 sum_s=0

 GET_GUIDE by action
   noaction n=338 p50=0 p90=3 p99=17 max=1850 sum_s=2
   no-input n=114 p50=0 p90=0 p99=5 max=19 sum_s=0
```

## RAW lat_q3d.out
```
name-only symbols lat histogram (s): [(1, 14), (2, 21), (3, 16), (4, 5), (5, 4), (6, 2), (7, 3), (8, 17), (9, 15), (10, 56), (11, 35), (12, 23), (13, 9), (14, 3), (15, 1), (29, 1)]
first-day/last-day 2026-10-16 08:28:11.701 2026-11-10 22:09:13.576 sessions 47 subagent 171 scope in input Counter({None: 225}) with workspace 157
by week Counter({'2026-10-2': 97, '2026-11-1': 54, '2026-10-3': 43, '2026-11-0': 31})
sample inputs: [(10047, '{"include_body":true,"name":"annotate_write_root"}'), (10019, '{"include_body":true,"name":"live_ids_or_disabled"}'), (10634, '{"name":"git_ignore_in_effect","include_body":true,"workspace":"/home/marius/work/claude/c')]
grep 210368 2026-10-20 09:07:10.825 386711 gap 1 2f5f98 {"pattern":"\\bJev\\b|\\bjev\\b|JEV","mode":"files","path":"/home/marius/work/claude","limit":20}
grep 208396 2026-10-19 08:12:41.525 135109 gap 5 2f5f98 {"pattern":"SessionMessagesArgs","path":"/home/marius","glob":"**/*.d.ts","mode":"files","include_hidden":true,"limit":5
grep 226057 2026-10-29 06:50:45.396 99824 gap 1 bc64d1 
grep 262260 2026-11-04 18:02:31.850 20009 gap 1 461383 {"glob":"*.py","path":"/home/marius/work","pattern":"def link_credentials|link_credentials\\(","limit":20}
grep 279892 2026-11-08 08:45:15.427 20003 gap 1 03fa7b {"mode":"files","path":"/home/marius","glob":"sessions/*.json","include_hidden":true,"pattern":"6821c9b3-d613-4fb6-ac7f-
grep 245666 2026-11-01 11:28:20.106 20002 gap 1 f079f8 {"head_limit":"25","mode":"files","path":"/home/marius/work/claude","glob":"!**/resident-private/**","pattern":"resident
grep 294209 2026-11-10 09:09:40.469 20002 gap 2 de741a {"glob":["**/*"],"limit":15,"mode":"files","path":"/home/marius/work/claude/claude-plugins","pattern":"subagent-guidance
grep 258240 2026-11-04 08:40:53.025 20001 gap 127 05bfb8 {"pattern":"subagent output matched instruction-shaped","path":"/home/marius/work/claude","include_hidden":true,"limit":
grep >=5000ms 23  >=20000 10 [19900,20300] 7
edit_file 285192 11-09 11:05 5014 reco gap 1574 write_lock_unavailable {"new_string":"                found.sort_unstable();\n                return (found, false /* MUTAT
edit_file 285193 11-09 11:05 5014 reco gap 2 write_lock_unavailable {"new_string":"                found.sort_unstable();\n                return (found, false /* MUTAT
edit_file 239508 10-31 10:18 2675 succ gap 3834 None {"new_string":"            crate::workspace::ProjectState::Activated(active) => active.config.projec
edit_file 204587 10-18 07:37 2218 succ gap 2 None {"new_string":"#[cfg(test)]\n#[path = \"server/config_reload_tests.rs\"]\nmod config_reload_tests;\n
edit_file 218676 10-24 12:01 2005 succ gap 2 None {"path":"/home/marius/.cache/cc-tmp/claude-1000/-home-marius-work-claude-codescout/74be4cd2-79e7-4b0
edit_file 207053 10-18 13:14 1752 succ gap 2 None {"new_string":"            None => return Ok(None),","old_string":"            None | Some(Value::Nu
edit_file >=1000ms 15 >=5000 2 distinct sessions 7 [('2026-10-18 07', 2), ('2026-11-09 11', 2), ('2026-10-16 18', 1)]
create_file 285053 11-09 10:53 5033 reco gap 843 write_lock_unavailable {"workspace":"/home/marius/work/claude/codescout","path":"/tmp/verify-default-ecc-20261109.py","cont
create_file 291376 11-09 21:38 5012 reco gap 431 write_lock_unavailable {"content":"docs(issues): record the follow-up note to the fence owner (bug 312cde2f4698874b)\n\nThe
create_file 284104 11-09 10:07 5008 reco gap 5193 write_lock_unavailable {"content":"fix(doc): refuse dry_run=true instead of dropping it and performing the write\n\n`doc(ac
create_file 282746 11-09 07:53 5006 reco gap 3 write_lock_unavailable {"content":"x\n","path":"target/lock-probe.txt","workspace":"/home/marius/work/claude/codescout"}
create_file 207329 10-18 13:58 3391 succ gap 3332 None {"content":"\n// ---- REVIEW2 (scratch only): layouts the author's table lacks ---------------------
create_file 269239 11-05 19:00 2320 succ gap 7 None {"path":"/tmp/external-memory-cache-01a15ca9-20261105.py","content":"import hashlib,json,pathlib,shu
create_file >=1000ms 6 >=5000 4 distinct sessions 6 [('2026-11-09 10', 2), ('2026-10-18 13', 1), ('2026-11-05 19', 1)]
memory write lat histogram 100ms buckets [(0, 5), (1, 39), (2, 17), (3, 41), (4, 48), (5, 24), (6, 15), (7, 3), (8, 4), (9, 2), (11, 4), (12, 2), (14, 1), (15, 2), (16, 1), (18, 1)]
```

## RAW lat_q3b.out
```
read_file n 19730 lat bucket counts:
 [0,1) 10567
 [1,3) 4044
 [3,10) 3822
 [10,33) 1089
 [33,100) 164
 [100,1000) 41
 [1000,1000000000.0) 3
read_file by kind: n p50 p90 p99 max
  other 2982 0 7 30 369
  no-input 4955 0 5 27 415
  md-whole/map 3353 1 8 34 21487
  buffer-ref 2193 0 3 12 80
  json 1212 0 5 18 174
  rs 4389 1 10 56 4572
  md-heading 646 0 7 23 7364
read_file lat>=100: 44  of which a non-rc row within +-3s has gap>=1000: 9
baseline fast read_file (2000 sample) same condition: 166
rf lat>=100 outcomes Counter({'success': 31, 'recoverable_error': 13}) errfam [(None, 31), ('il1_read_overlaps_symbol', 11), ('librarian_managed_artifact', 1), ('read_file_overflow_threshold', 1)]
rf lat>=100 by root Counter({'main': 26, 'other': 18})
rf slow day [('2026-10-17', 9), ('2026-10-18', 5), ('2026-10-31', 5), ('2026-10-24', 4), ('2026-10-27', 4), ('2026-10-16', 2)]
```

## RAW lat_q4.out
```
| tool(subset) | first n | first p50 | first p90 | later n | later p50 | later p90 | penalty p50 | penalty mean |
|---|---|---|---|---|---|---|---|---|
| references | 109 | 2562 | 21168 | 151 | 1384 | 8112 | 1178 | 3673 |
| call_graph | 41 | 1798 | 17209 | 89 | 132 | 2193 | 1666 | 4908 |
| symbol_at | 4 | 970 | 3447 | 3 | 2 | 5380 | 968 | -616 |
| edit_code | 312 | 94 | 578 | 4180 | 61 | 394 | 32 | 95 |
| workspace | 403 | 259 | 1588 | 486 | 78 | 1198 | 181 | 209 |
| workspace activate | 278 | 172 | 1159 | 152 | 92 | 861 | 79 | 180 |
| symbols(name, any) | 400 | 152 | 4170 | 5435 | 20 | 599 | 132 | 656 |
| symbols(name+path) | 395 | 132 | 2250 | 5215 | 17 | 247 | 115 | 383 |
| symbols(path only) | 276 | 176 | 1810 | 1185 | 27 | 690 | 150 | 436 |
| edit_code insert | 239 | 115 | 590 | 1371 | 94 | 502 | 21 | 40 |
| edit_code replace | 179 | 77 | 573 | 1602 | 42 | 289 | 34 | 109 |
| ANY LSP-class call (refs/cg/sym_at/edit_code/symbols name+path) | 471 | 166 | 3056 | 10028 | 37 | 513 | 129 | 647 |

references/call_graph/symbol_at by path extension (all calls, p50/p90):
  ('references', 'rs') 194 2336 20833
  ('call_graph', 'noext/none') 86 418 2550
  ('references', 'noext/none') 33 1402 5036
  ('references', 'py') 31 1230 2236
  ('call_graph', 'py') 30 252 2989
  ('call_graph', 'rs') 14 5044 27896
  ('symbol_at', 'rs') 6 746 5548
  ('symbol_at', 'noext/none') 1 648 648
  ('references', 'dir') 1 0 0
  ('references', 'ts') 1 82 82

references by ordinal within (session,root): {1: (109, 2562, 21168), 2: (64, 1715, 11932), 3: (35, 1703, 7672), 4: (18, 1223, 7884), 5: (10, 1142, 8767), 6: (24, 508, 2175)}
references hist (s): [(0, 183), (5, 34), (10, 15), (15, 6), (20, 7), (25, 2), (30, 7), (40, 3), (45, 2), (55, 1)]
references in [44000,46500]: 2  [29900,30300] 3

lsp_events (all rows, filtered to snapshot window):
| language | reason | outcome | n | handshake p50 | handshake p90 | first_response n | first_response p50 | first_response p90 | max first_resp |
|---|---|---|---|---|---|---|---|---|---|
| bash | idle_evicted | success | 42 | 512 | 863 | 24 | 34 | 70 | 106 |
| bash | lru_evicted | success | 6 | 1027 | 1394 | 0 | 0 | 0 | - |
| bash | new_session | success | 150 | 556 | 1172 | 64 | 37 | 92 | 203 |
| c | new_session | success | 1 | 59 | 59 | 0 | 0 | 0 | - |
| css | idle_evicted | success | 19 | 299 | 490 | 2 | 18 | 19 | 19 |
| css | lru_evicted | success | 8 | 382 | 504 | 0 | 0 | 0 | - |
| css | new_session | success | 91 | 289 | 458 | 5 | 19 | 21 | 21 |
| html | idle_evicted | success | 20 | 530 | 772 | 2 | 9 | 10 | 10 |
| html | lru_evicted | success | 6 | 616 | 824 | 0 | 0 | 0 | - |
| html | new_session | success | 92 | 476 | 714 | 5 | 11 | 12 | 12 |
| java | idle_evicted | success | 50 | 2686 | 3690 | 0 | 0 | 0 | - |
| java | new_session | failed | 206 | 16494 | 17226 | 0 | 0 | 0 | - |
| java | new_session | success | 385 | 2468 | 3412 | 0 | 0 | 0 | - |
| javascript | idle_evicted | success | 52 | 114 | 154 | 32 | 532 | 633 | 827 |
| javascript | lru_evicted | success | 7 | 126 | 159 | 1 | 836 | 836 | 836 |
| javascript | new_session | success | 128 | 113 | 170 | 42 | 524 | 750 | 1285 |
| kotlin | new_session | failed | 70 | 1430 | 2960 | 0 | 0 | 0 | - |
| kotlin | new_session | success | 448 | 2 | 4455 | 1 | 989 | 989 | 989 |
| python | idle_evicted | success | 209 | 180 | 247 | 177 | 368 | 516 | 766 |
| python | lru_evicted | success | 10 | 248 | 273 | 1 | 734 | 734 | 734 |
| python | new_session | success | 379 | 182 | 250 | 277 | 371 | 520 | 5413 |
| rust | new_session | failed | 63 | 31 | 1057 | 0 | 0 | 0 | - |
| rust | new_session | success | 791 | 53 | 201 | 679 | 16 | 124 | 40043 |
| typescript | idle_evicted | success | 30 | 118 | 169 | 11 | 543 | 617 | 935 |
| typescript | lru_evicted | success | 6 | 154 | 172 | 0 | 0 | 0 | - |
| typescript | new_session | success | 107 | 116 | 179 | 12 | 451 | 572 | 947 |

per language overall:
| bash | 198 | hs p50 557 p90 1158 | fr n=88 p50 36 p90 92 max 203 |
| c | 1 | hs p50 59 p90 59 | fr n=0 p50 0 p90 0 max - |
| css | 118 | hs p50 295 p90 478 | fr n=7 p50 19 p90 21 max 21 |
| html | 118 | hs p50 491 p90 760 | fr n=7 p50 10 p90 12 max 12 |
| java | 435 | hs p50 2490 p90 3465 | fr n=0 p50 0 p90 0 max - |
| javascript | 187 | hs p50 113 p90 168 | fr n=75 p50 530 p90 748 max 1285 |
| kotlin | 448 | hs p50 2 p90 4455 | fr n=1 p50 989 p90 989 max 989 |
| python | 598 | hs p50 182 p90 252 | fr n=455 p50 370 p90 521 max 5413 |
| rust | 791 | hs p50 53 p90 201 | fr n=679 p50 16 p90 124 max 40043 |
| typescript | 143 | hs p50 116 p90 175 | fr n=23 p50 500 p90 616 max 947 |
[('failed', 339), ('success', 3037)]
[('LSP server disconnected', 206), ("mux process failed to start: Error: another mux instance holds the lock\n\nCaused by:\n    Resource temporarily unavailable (os error 11) — hint: The workspace's LSP index is locked by another running server (RocksDB/mux lock). A stale or concurrent LSP for this workspace holds it. Close other sessions on this workspace, or locate the holder with `fuser <lsp-ho
```

## RAW lat_q4b.out
```
rust: slow(>=5s) n=507 within 5min after a rust lsp start: 375 (74.0%); ordinary n=14951: 5791 (38.7%)
python: slow(>=5s) n=506 within 5min after a python lsp start: 240 (47.4%); ordinary n=14945: 6157 (41.2%)
java: slow(>=5s) n=492 within 5min after a java lsp start: 197 (40.0%); ordinary n=14829: 4531 (30.6%)
kotlin: slow(>=5s) n=492 within 5min after a kotlin lsp start: 224 (45.5%); ordinary n=14829: 3919 (26.4%)
[('10-17', 92), ('11-09', 75), ('10-16', 51), ('10-31', 51), ('10-29', 48), ('10-18', 47)]
events by language/day total 3376
java failed [(206, 3437.576, '2026-10-12 10:24:42', '2026-10-31 15:54:02')]
kotlin failed [(70, 112.74)]
[('LSP server disconnected', 355), ("mux process failed to start: Error: another mux instance holds the lock\n\nCaused by:\n    Resource temporarily unavailable (os error 11) — hint: The workspace's LSP index is locked by another running server (RocksDB/mux lock). A stale or concurrent LSP for this workspace holds it. Close other sessions on this workspace, or locate the holder with `fuser <lsp-ho
```

## RAW lat_q5.out
```

### Parent vs subagent (cells: n / p50 / p90 ms; run_command excludes wait loops and timeouts)

| segment | run_command | symbols | grep | read_file | edit_file | doc | edit_code | create_file | memory | workspace |
|---|---|---|---|---|---|---|---|---|---|---|
| parent(agent_id NULL) | 33958 / 76 / 1969 | 8343 / 14 / 655 | 3954 / 12 / 257 | 11877 / 0 / 6 | 4275 / 8 / 58 | 6328 / 21 / 107 | 2674 / 33 / 223 | 2803 / 2 / 12 | 1343 / 1 / 1173 | 728 / 130 / 1379 |
| subagent | 15173 / 126 / 55129 | 2226 / 51 / 2026 | 5273 / 18 / 310 | 7853 / 0 / 8 | 4210 / 12 / 93 | 458 / 26 / 179 | 1818 / 125 / 604 | 1030 / 1 / 11 | 148 / 0 / 5 | 161 / 114 / 1788 |

### Root class (cells: n / p50 / p90 ms; run_command excludes wait loops and timeouts)

| segment | run_command | symbols | grep | read_file | edit_file | doc | edit_code | create_file | memory | workspace |
|---|---|---|---|---|---|---|---|---|---|---|
| main checkout | 32274 / 80 / 2007 | 5347 / 23 / 734 | 5226 / 14 / 258 | 12318 / 0 / 6 | 3697 / 9 / 62 | 5544 / 21 / 135 | 1694 / 37 / 341 | 2608 / 3 / 14 | 1315 / 2 / 1195 | 765 / 113 / 1174 |
| other repo | 301 / 66 / 6552 | 98 / 32 / 2066 | 235 / 30 / 299 | 208 / 0 / 4 | 99 / 18 / 80 | - | 27 / 130 / 573 | 51 / 0 / 3 | - | - |
| other under codescout | 1754 / 138 / 78844 | 349 / 60 / 3406 | 767 / 20 / 357 | 902 / 1 / 10 | 852 / 15 / 73 | 41 / 43 / 133 | 229 / 94 / 420 | 65 / 1 / 9 | 12 / 1 / 3 | 4 / 138 / 213 |
| tmp checkout | 2067 / 79 / 633 | 791 / 16 / 1025 | 39 / 10 / 64 | 759 / 1 / 6 | 299 / 4 / 18 | 225 / 20 / 80 | 217 / 38 / 98 | 247 / 2 / 10 | 21 / 0 / 3 | 13 / 1414 / 1939 |
| worktree (.worktrees/.claude/.codex) | 12735 / 108 / 49184 | 3984 / 15 / 614 | 2960 / 17 / 320 | 5543 / 1 / 8 | 3538 / 11 / 94 | 976 / 21 / 84 | 2325 / 81 / 494 | 862 / 1 / 7 | 143 / 0 / 2 | 107 / 754 / 2111 |

root class counts Counter({'main checkout': 73021, 'worktree (.worktrees/.claude/.codex)': 34399, 'other under codescout': 5068, 'tmp checkout': 4741, 'other repo': 1049})

### Week (UTC, Mon-start) (cells: n / p50 / p90 ms; run_command excludes wait loops and timeouts)

| segment | run_command | symbols | grep | read_file | edit_file | doc | edit_code | create_file | memory | workspace |
|---|---|---|---|---|---|---|---|---|---|---|
| W1 (from 2026-10-12) | 10332 / 77 / 35358 | 2015 / 34 / 553 | 3041 / 15 / 296 | 4727 / 0 / 7 | 2536 / 11 / 106 | 730 / 18 / 112 | 1245 / 145 / 603 | 738 / 1 / 6 | 104 / 0 / 1 | 170 / 552 / 2206 |
| W2 (from 2026-10-19) | 4680 / 107 / 33744 | 819 / 46 / 5035 | 1222 / 23 / 348 | 1772 / 1 / 11 | 1106 / 11 / 93 | 332 / 25 / 241 | 419 / 113 / 575 | 284 / 2 / 31 | 100 / 1 / 5 | 101 / 673 / 2158 |
| W3 (from 2026-10-26) | 13979 / 83 / 3307 | 2595 / 20 / 805 | 2090 / 10 / 219 | 4925 / 0 / 7 | 2407 / 9 / 59 | 1233 / 27 / 173 | 1069 / 44 / 290 | 1214 / 2 / 11 | 322 / 1 / 1803 | 225 / 191 / 1913 |
| W4 (from 2026-11-02) | 13706 / 83 / 2668 | 2807 / 14 / 601 | 2111 / 14 / 273 | 5798 / 0 / 5 | 1625 / 9 / 60 | 2530 / 19 / 66 | 1070 / 27 / 244 | 981 / 2 / 12 | 691 / 1 / 1156 | 216 / 93 / 940 |
| W5 (from 2026-11-09) | 6434 / 100 / 1849 | 2333 / 13 / 745 | 763 / 43 / 450 | 2508 / 1 / 7 | 811 / 13 / 75 | 1961 / 21 / 221 | 689 / 33 / 160 | 616 / 4 / 15 | 274 / 10 / 135 | 177 / 125 / 443 |

### codescout_dirty (cells: n / p50 / p90 ms; run_command excludes wait loops and timeouts)

| segment | run_command | symbols | grep | read_file | edit_file | doc | edit_code | create_file | memory | workspace |
|---|---|---|---|---|---|---|---|---|---|---|
| dirty=0 | 17097 / 92 / 3157 | 4597 / 13 / 606 | 2658 / 17 / 278 | 6751 / 0 / 7 | 2357 / 10 / 56 | 3040 / 19 / 78 | 1758 / 31 / 227 | 1515 / 2 / 13 | 724 / 2 / 1058 | 332 / 106 / 770 |
| dirty=1 | 32034 / 85 / 7834 | 5972 / 27 / 864 | 6569 / 15 / 299 | 12979 / 0 / 7 | 6128 / 10 / 79 | 3746 / 22 / 167 | 2734 / 91 / 514 | 2318 / 2 / 12 | 767 / 1 / 1255 | 557 / 180 / 2040 |

### Hour of day UTC: load vs p90
| hour | rows | non-rc p50 | non-rc p90 | rc(ex wait/to) p50 | rc p90 | gap p90 | rows with gap>1s |
|---|---|---|---|---|---|---|---|
| 00 | 417 | 5 | 59 | 97 | 8964 | 3 | 0 |
| 01 | 370 | 3 | 55 | 70 | 7978 | 2 | 0 |
| 02 | 252 | 0 | 104 | 62 | 271 | 2 | 0 |
| 03 | 1858 | 7 | 412 | 112 | 6792 | 731 | 160 |
| 04 | 2792 | 5 | 299 | 78 | 3825 | 135 | 91 |
| 05 | 3652 | 3 | 166 | 58 | 888 | 55 | 30 |
| 06 | 5935 | 6 | 234 | 79 | 4331 | 109 | 199 |
| 07 | 6828 | 9 | 265 | 98 | 11726 | 137 | 340 |
| 08 | 8385 | 7 | 232 | 69 | 4754 | 89 | 271 |
| 09 | 6768 | 7 | 196 | 93 | 10729 | 104 | 273 |
| 10 | 8223 | 7 | 176 | 99 | 4394 | 150 | 392 |
| 11 | 8421 | 8 | 238 | 95 | 6335 | 430 | 634 |
| 12 | 7918 | 6 | 190 | 89 | 4041 | 135 | 312 |
| 13 | 7908 | 5 | 142 | 90 | 5543 | 126 | 190 |
| 14 | 7184 | 6 | 141 | 72 | 4416 | 81 | 184 |
| 15 | 6670 | 7 | 171 | 94 | 5257 | 330 | 496 |
| 16 | 6003 | 5 | 137 | 87 | 3160 | 331 | 442 |
| 17 | 4846 | 6 | 119 | 99 | 9536 | 111 | 179 |
| 18 | 5701 | 4 | 122 | 69 | 3740 | 79 | 70 |
| 19 | 6919 | 5 | 120 | 117 | 4657 | 153 | 348 |
| 20 | 3431 | 9 | 180 | 82 | 19530 | 5 | 72 |
| 21 | 3385 | 9 | 215 | 93 | 8329 | 35 | 122 |
| 22 | 3265 | 9 | 170 | 120 | 3993 | 104 | 171 |
| 23 | 1147 | 4 | 78 | 137 | 8229 | 6 | 27 |

### Top 5 codescout_sha by frequency (p50 ms per tool; n)
distinct shas 141
| sha | dirty | rows | first..last | run_command | symbols | grep | read_file | edit_file | doc | edit_code |
|---|---|---|---|---|---|---|---|---|---|---|
| 3923da8c | 1 | 8777 | 2026-10-17..2026-10-19 | 193 (n3330) | 61 (n489) | 31 (n1143) | 1 (n1426) | 19 (n1207) | 73 (n35) | 236 (n463) |
| 0007d926 | 1 | 8558 | 2026-10-16..2026-10-18 | 87 (n2907) | 34 (n515) | 15 (n1139) | 1 (n1752) | 12 (n955) | 49 (n100) | 125 (n471) |
| 0eb6190ec | 1 | 8436 | 2026-10-30..2026-11-04 | 92 (n3834) | 13 (n750) | 11 (n54) | 0 (n2170) | 6 (n160) | 20 (n664) | 19 (n158) |
| 2acc553ad | 1 | 7630 | 2026-10-23..2026-10-29 | 83 (n3445) | 26 (n804) | 47 (n369) | 1 (n1446) | 7 (n409) | 20 (n270) | 40 (n208) |
| aebb0e76f | 0 | 6150 | 2026-11-04..2026-11-07 | 66 (n2333) | 9 (n929) | 20 (n51) | 0 (n1086) | 6 (n119) | 19 (n694) | 19 (n301) |
```

## RAW lat_q6.out
```
### run_command (ex wait/timeout) time composition: n, sum_s, share>10s rows
| segment | n | sum_s | n>=10s | time in >=10s rows s | build/test share of n>=10s time |
|---|---|---|---|---|---|
| parent | 33958 | 80372 | 1238 | 63916 | 52% |
| subagent | 15173 | 292720 | 2478 | 285692 | 75% |
| W1 2026-10-12 | 10332 | 170706 | 1471 | 166467 | 82% |
| W2 2026-10-19 | 4680 | 64147 | 665 | 61976 | 85% |
| W3 2026-10-26 | 13979 | 82436 | 808 | 75882 | 37% |
| W4 2026-11-02 | 13706 | 41665 | 520 | 34146 | 69% |
| W5 2026-11-09 | 6434 | 14139 | 252 | 11136 | 55% |

### Rows within 100 ms of a round timeout (|lat-T|<=100), per tool; time held

| T ms | tool | n | time_s | share of all-latency |
|---|---|---|---|---|
| 20000 | run_command | 22 | 441 | 0.055% |
| 20000 | grep | 7 | 140 | 0.017% |
| 20000 | references | 1 | 20 | 0.002% |
| 20000 | librarian | 1 | 20 | 0.002% |
| 20000 | (neighbour windows 100<|d|<=300 ms, all tools) | 29 | | |
| 30000 | run_command | 223 | 6697 | 0.835% |
| 30000 | references | 3 | 90 | 0.011% |
| 30000 | index | 1 | 30 | 0.004% |
| 30000 | symbols | 1 | 30 | 0.004% |
| 30000 | (neighbour windows 100<|d|<=300 ms, all tools) | 32 | | |
| 60000 | run_command | 47 | 2822 | 0.352% |
| 60000 | (neighbour windows 100<|d|<=300 ms, all tools) | 31 | | |
| 120000 | run_command | 56 | 6723 | 0.839% |
| 120000 | (neighbour windows 100<|d|<=300 ms, all tools) | 23 | | |
| 300000 | run_command | 4 | 1200 | 0.150% |
| 300000 | (neighbour windows 100<|d|<=300 ms, all tools) | 2 | | |

### run_command rows at their OWN timeout (flag or [T,T+300ms]), by timeout_secs
| timeout_secs | n | flagged timed_out | time_s | of which wait-loop |
|---|---|---|---|---|
| 3000 | 2 | 2 | 6000 | 2 |
| 30 | 154 | 57 | 4627 | 9 |
| 1500 | 3 | 3 | 4501 | 3 |
| 1800 | 2 | 2 | 3600 | 1 |
| 2900 | 1 | 1 | 2900 | 1 |
| 900 | 3 | 3 | 2700 | 1 |
| 1200 | 2 | 2 | 2400 | 1 |
| 1700 | 1 | 1 | 1700 | 1 |
| 120 | 10 | 9 | 1201 | 3 |
| 600 | 2 | 2 | 1200 | 1 |
| 60 | 12 | 9 | 721 | 1 |
| 130 | 3 | 0 | 390 | 3 |
| 290 | 1 | 1 | 290 | 1 |
| 180 | 1 | 1 | 180 | 0 |
| 90 | 2 | 2 | 180 | 0 |
| 150 | 1 | 0 | 150 | 1 |
| 20 | 3 | 1 | 60 | 1 |
| 45 | 1 | 0 | 45 | 0 |
| 3 | 2 | 2 | 6 | 2 |
| 5 | 1 | 1 | 5 | 0 |
| 2 | 1 | 1 | 2 | 0 |
total at-timeout rc rows 208 time_s 32859 = share of rc time 4.2%
timed_out flagged rows (output_json present): 111 out of rc rows with output_json 35202
wait-loop share among at-timeout: 32 / 208  with cmd text: 208

### Server/lock refusals (err_family) 
| family | tool | n | p50 | max | sum_s |
|---|---|---|---|---|---|
| write_lock_unavailable | create_file | 9 | 0 | 5033 | 20 |
| write_lock_unavailable | doc | 3 | 5011 | 5028 | 10 |
| write_lock_unavailable | edit_file | 2 | 5014 | 5014 | 10 |
family totals {'tool_timeout': 0, 'write_lock_unavailable': 14}
Counter({'2026-11-09': 13, '2026-11-10': 1})
[('references', 3, 30013, 32454)]
[]
```

## RAW lat_q7.out
```
symbols name-only by 5-day bucket: n p50 p90 share>=8000ms
  2026-10-3 65 11455 13037 1.00
  2026-10-4 52 11110 12903 1.00
  2026-10-5 8 7559 11133 0.38
  2026-10-6 15 9024 12139 0.53
  2026-11-0 20 3412 8950 0.25
  2026-11-1 62 3932 9503 0.40
  2026-11-2 3 8508 8763 0.67
since 11-01: n 85 p50 3806.0 rows<3000 32

symbols n=10569: [0,0.1)s:7923  [0.1,0.5)s:1178  [0.5,1)s:623  [1,2)s:186  [2,5)s:260  [5,8)s:134  [8,10)s:45  [10,12)s:136  [12,15)s:37  [15,19)s:6  [19,19.9)s:0  [19.9,20.1)s:0  [20.1,21)s:3  [21,25)s:1  [25,29)s:3  [29,29.9)s:1  [29.9,30.1)s:1  [30.1,31)s:0  [31,40)s:3  [40,44)s:23  [44,44.9)s:0  [44.9,45.1)s:0  [45.1,46)s:1  [46,59)s:5  [59,59.9)s:0  [59.9,60.1)s:0  [60.1,70)s:0  >=70s:0

grep n=9227: [0,0.1)s:7035  [0.1,0.5)s:1701  [0.5,1)s:271  [1,2)s:153  [2,5)s:44  [5,8)s:9  [8,10)s:1  [10,12)s:0  [12,15)s:2  [15,19)s:1  [19,19.9)s:0  [19.9,20.1)s:7  [20.1,21)s:0  [21,25)s:0  [25,29)s:0  [29,29.9)s:0  [29.9,30.1)s:0  [30.1,31)s:0  [31,40)s:0  [40,44)s:0  [44,44.9)s:0  [44.9,45.1)s:0  [45.1,46)s:0  [46,59)s:0  [59,59.9)s:0  [59.9,60.1)s:0  [60.1,70)s:0  >=70s:3

references n=260: [0,0.1)s:18  [0.1,0.5)s:26  [0.5,1)s:24  [1,2)s:74  [2,5)s:41  [5,8)s:27  [8,10)s:7  [10,12)s:8  [12,15)s:7  [15,19)s:4  [19,19.9)s:2  [19.9,20.1)s:1  [20.1,21)s:1  [21,25)s:5  [25,29)s:1  [29,29.9)s:0  [29.9,30.1)s:3  [30.1,31)s:0  [31,40)s:5  [40,44)s:3  [44,44.9)s:0  [44.9,45.1)s:2  [45.1,46)s:0  [46,59)s:1  [59,59.9)s:0  [59.9,60.1)s:0  [60.1,70)s:0  >=70s:0

call_graph n=130: [0,0.1)s:23  [0.1,0.5)s:45  [0.5,1)s:7  [1,2)s:24  [2,5)s:17  [5,8)s:6  [8,10)s:0  [10,12)s:2  [12,15)s:1  [15,19)s:1  [19,19.9)s:0  [19.9,20.1)s:0  [20.1,21)s:0  [21,25)s:2  [25,29)s:0  [29,29.9)s:0  [29.9,30.1)s:0  [30.1,31)s:1  [31,40)s:1  [40,44)s:0  [44,44.9)s:0  [44.9,45.1)s:0  [45.1,46)s:0  [46,59)s:0  [59,59.9)s:0  [59.9,60.1)s:0  [60.1,70)s:0  >=70s:0

symbol_at n=7: [0,0.1)s:2  [0.1,0.5)s:1  [0.5,1)s:1  [1,2)s:1  [2,5)s:1  [5,8)s:1  [8,10)s:0  [10,12)s:0  [12,15)s:0  [15,19)s:0  [19,19.9)s:0  [19.9,20.1)s:0  [20.1,21)s:0  [21,25)s:0  [25,29)s:0  [29,29.9)s:0  [29.9,30.1)s:0  [30.1,31)s:0  [31,40)s:0  [40,44)s:0  [44,44.9)s:0  [44.9,45.1)s:0  [45.1,46)s:0  [46,59)s:0  [59,59.9)s:0  [59.9,60.1)s:0  [60.1,70)s:0  >=70s:0

edit_code n=4492: [0,0.1)s:2812  [0.1,0.5)s:1299  [0.5,1)s:285  [1,2)s:48  [2,5)s:32  [5,8)s:2  [8,10)s:2  [10,12)s:1  [12,15)s:1  [15,19)s:7  [19,19.9)s:0  [19.9,20.1)s:0  [20.1,21)s:0  [21,25)s:1  [25,29)s:0  [29,29.9)s:0  [29.9,30.1)s:0  [30.1,31)s:0  [31,40)s:1  [40,44)s:0  [44,44.9)s:0  [44.9,45.1)s:0  [45.1,46)s:1  [46,59)s:0  [59,59.9)s:0  [59.9,60.1)s:0  [60.1,70)s:0  >=70s:0

semantic_search n=14: [0,0.1)s:3  [0.1,0.5)s:2  [0.5,1)s:3  [1,2)s:3  [2,5)s:2  [5,8)s:1  [8,10)s:0  [10,12)s:0  [12,15)s:0  [15,19)s:0  [19,19.9)s:0  [19.9,20.1)s:0  [20.1,21)s:0  [21,25)s:0  [25,29)s:0  [29,29.9)s:0  [29.9,30.1)s:0  [30.1,31)s:0  [31,40)s:0  [40,44)s:0  [44,44.9)s:0  [44.9,45.1)s:0  [45.1,46)s:0  [46,59)s:0  [59,59.9)s:0  [59.9,60.1)s:0  [60.1,70)s:0  >=70s:0

librarian n=285: [0,0.1)s:125  [0.1,0.5)s:30  [0.5,1)s:9  [1,2)s:20  [2,5)s:30  [5,8)s:15  [8,10)s:12  [10,12)s:8  [12,15)s:9  [15,19)s:7  [19,19.9)s:0  [19.9,20.1)s:1  [20.1,21)s:0  [21,25)s:2  [25,29)s:2  [29,29.9)s:0  [29.9,30.1)s:0  [30.1,31)s:1  [31,40)s:3  [40,44)s:0  [44,44.9)s:0  [44.9,45.1)s:0  [45.1,46)s:0  [46,59)s:6  [59,59.9)s:0  [59.9,60.1)s:0  [60.1,70)s:2  >=70s:3

workspace n=889: [0,0.1)s:368  [0.1,0.5)s:237  [0.5,1)s:145  [1,2)s:81  [2,5)s:21  [5,8)s:4  [8,10)s:0  [10,12)s:2  [12,15)s:1  [15,19)s:0  [19,19.9)s:0  [19.9,20.1)s:0  [20.1,21)s:0  [21,25)s:11  [25,29)s:2  [29,29.9)s:2  [29.9,30.1)s:0  [30.1,31)s:3  [31,40)s:8  [40,44)s:0  [44,44.9)s:0  [44.9,45.1)s:1  [45.1,46)s:0  [46,59)s:3  [59,59.9)s:0  [59.9,60.1)s:0  [60.1,70)s:0  >=70s:0

index n=21: [0,0.1)s:1  [0.1,0.5)s:4  [0.5,1)s:0  [1,2)s:0  [2,5)s:1  [5,8)s:0  [8,10)s:0  [10,12)s:0  [12,15)s:0  [15,19)s:0  [19,19.9)s:0  [19.9,20.1)s:0  [20.1,21)s:0  [21,25)s:4  [25,29)s:3  [29,29.9)s:1  [29.9,30.1)s:1  [30.1,31)s:0  [31,40)s:5  [40,44)s:1  [44,44.9)s:0  [44.9,45.1)s:0  [45.1,46)s:0  [46,59)s:0  [59,59.9)s:0  [59.9,60.1)s:0  [60.1,70)s:0  >=70s:0
 grep within +-1000ms of 20000: n=7; per-100ms bucket (from 19000): [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
 symbols within +-1000ms of 20000: n=3; per-100ms bucket (from 19000): [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 0]
 references within +-1000ms of 30000: n=3; per-100ms bucket (from 29000): [0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
 references within +-1000ms of 45000: n=2; per-100ms bucket (from 44000): [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
 symbols within +-1000ms of 8000: n=39; per-100ms bucket (from 7000): [1, 0, 3, 0, 0, 3, 4, 0, 2, 1, 0, 1, 1, 2, 1, 5, 4, 0, 4, 7, 0]
 call_graph within +-1000ms of 30000: n=1; per-100ms bucket (from 29000): [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]

symbols name+path n 5610 >=19000 26 max 57560  [5000,19000) 85
symbols/grep >21s: 41 Counter({'symbols': 38, 'grep': 3})  with gap>1s (rows delayed outside latency): 3
   210368 grep 386711 {"pattern":"\\bJev\\b|\\bjev\\b|JEV","mode":"files","path":"/home/marius/work/claude","limit":20}
   208396 grep 135109 {"pattern":"SessionMessagesArgs","path":"/home/marius","glob":"**/*.d.ts","mode":"files","include_hidden":true
   226057 grep 99824 
   294933 symbols 57560 {"name":"CachedResolver","path":".worktrees/ecc-resolver-followup-20261110/src/tools/symbol","workspace":"/hom
   294932 symbols 57558 {"name":"bfs","path":".worktrees/ecc-resolver-followup-20261110/src/tools/symbol","include_body":true,"workspa
   286621 symbols 56393 {"path":"src/observations/server.rs","name":"ObservationContext/from_meta","include_body":true,"workspace":"/t
explicit time_budget_ms used: Counter({60000: 2, 100: 2, 1: 1, 15000: 1, 20000: 1, 120000: 1, 120001: 1, 120002: 1, 120003: 1})
```

## RAW lat_q7b.out
```
37
230838 10-29 20:57:10 40112 b9bb97 par /home/marius/work/claude/codescout | src/util/path_security.rs | denied_paths_with
231357 10-29 21:41:37 40023 9f2efa par /home/marius/work/claude/codescout |  | 
231360 10-29 21:41:58 40163 7a8c8b par /home/marius/work/claude/codescout |  | 
231383 10-29 21:42:29 29933 7a8c8b par /home/marius/work/claude/codescout |  | 
231384 10-29 21:42:29 27558 9f2efa par /home/marius/work/claude/codescout |  | 
233123 10-30 06:31:08 40090 abc8d5 par /home/marius/work/claude/codescout | src/agent/mod.rs | Agent/semantic_memory_st
233124 10-30 06:31:09 40198 abc8d5 par /home/marius/work/claude/codescout | src/agent/mod.rs | Agent/memory_embedder_fo
233125 10-30 06:31:09 40096 abc8d5 par /home/marius/work/claude/codescout | src/tools/memory/mod.rs | cross_embed_memory
234736 10-30 12:00:08 28248 41b193 par /home/marius/work/claude/codescout | src/librarian/tools/get.rs | 
235275 10-30 15:50:07 32744 41b193 par /home/marius/work/claude/codescout | src/tools/core/param_alias.rs | 
235699 10-30 18:36:21 40066 31a26c par /home/marius/work/claude/codescout | c/librarian/tools/librarian.rs | every_action_labelled_sc
235700 10-30 18:36:21 40170 31a26c par /home/marius/work/claude/codescout | c/librarian/tools/librarian.rs | Librarian/call
236388 10-30 20:25:13 40051 97a050 par /home/marius/work/claude/codescout | src/librarian/tools/reindex.rs | call
237241 10-31 04:50:13 40096 e4d293 par /home/marius/work/claude/codescout | src/operator_rules/route.rs | tests/a_rule_with_severa
237609 10-31 06:07:53 40031 e4d293 par /home/marius/work/claude/codescout | src/librarian/tools/create.rs | call
238871 10-31 08:39:04 40098 0e844a par /home/marius/work/claude/codescout |  | 
238881 10-31 08:40:14 41285 0e844a par /home/marius/work/claude/codescout |  | 
238882 10-31 08:40:16 40218 0e844a par /home/marius/work/claude/codescout |  | 
239857 10-31 10:41:34 40104 0e844a par /home/marius/work/claude/codescout |  | 
240532 10-31 12:34:39 40115 74cfde par /home/marius/work/claude/codescout | ols/run_command/attribution.rs | materialize_into
240533 10-31 12:34:39 40149 74cfde par /home/marius/work/claude/codescout | ols/run_command/attribution.rs | engine_std_command
240908 10-31 13:58:20 40066 74cfde par /home/marius/work/claude/codescout | ols/run_command/attribution.rs | no_history
240918 10-31 14:01:01 40078 74cfde par /home/marius/work/claude/codescout | ols/run_command/attribution.rs | python3_present
243606 11-01 08:54:47 40131 b867d3 par /home/marius/work/claude/codescout | src/lsp/client.rs | LspClient/workspace_symb
243621 11-01 08:55:33 40140 b867d3 par /home/marius/work/claude/codescout | src/lsp/client.rs | LspClient/request
244173 11-01 09:29:57 45381 0e844a par /home/marius/work/claude/codescout |  | 
244412 11-01 09:51:56 40597 b867d3 par /home/marius/work/claude/codescout | ols/run_command/attribution.rs | tests/python3_present
286616 11-09 12:23:30 33467 cef939 par /codescout-stock-observations-20261109 | ipts/operation_observations.py | merge
286617 11-09 12:23:35 33471 cef939 par /codescout-stock-observations-20261109 | c/observations/server_tests.rs | 
286618 11-09 12:23:39 42382 cef939 par /codescout-stock-observations-20261109 | src/observations/server.rs | LocalInvocationId
286619 11-09 12:23:44 47364 cef939 par /codescout-stock-observations-20261109 | src/observations/server.rs | observe_write
286620 11-09 12:23:48 51278 cef939 par /codescout-stock-observations-20261109 | rc/tools/invocation_context.rs | codex_fields
286621 11-09 12:23:53 56393 cef939 par /codescout-stock-observations-20261109 | src/observations/server.rs | ObservationContext/from_
287800 11-09 14:03:09 29850 681c20 par /home/marius/work/claude/codescout |  | CreateFile/call
294920 11-10 11:04:42 25903 959d00 par /home/marius/work/claude/codescout | 10/src/tools/symbol/call_edges | 
294932 11-10 11:05:48 57558 959d00 par /home/marius/work/claude/codescout | owup-20261110/src/tools/symbol | bfs
294933 11-10 11:05:48 57560 959d00 par /home/marius/work/claude/codescout | owup-20261110/src/tools/symbol | CachedResolver
```

## RAW lat_q7c.out
```
spike bins (non-run_command, 250ms bins, n>=6): tool, bin start ms, n, outcomes, date range, #sessions
symbols 1000 90 {'succ': 90} 2026-10-16 2026-11-10 41
symbols 5000 83 {'succ': 83} 2026-10-16 2026-11-10 39
memory 1000 68 {'succ': 68} 2026-10-31 2026-11-10 14
grep 1000 61 {'succ': 61} 2026-10-14 2026-11-10 32
symbols 2000 58 {'succ': 58} 2026-10-14 2026-11-10 34
symbols 2500 48 {'succ': 48} 2026-10-13 2026-11-10 21
grep 1250 44 {'succ': 44} 2026-10-17 2026-11-10 24
memory 1250 39 {'succ': 39} 2026-10-30 2026-11-10 11
grep 1500 35 {'succ': 35} 2026-10-17 2026-11-10 25
workspace 1000 34 {'succ': 34} 2026-10-13 2026-11-09 29
symbols 2250 33 {'succ': 33} 2026-10-13 2026-11-10 20
symbols 1750 33 {'succ': 33} 2026-10-13 2026-11-09 24
symbols 1250 32 {'succ': 32} 2026-10-13 2026-11-09 21
symbols 1500 31 {'succ': 31} 2026-10-14 2026-11-10 22
edit_code 1000 30 {'succ': 28, 'reco': 2} 2026-10-13 2026-11-08 12
memory 1500 28 {'succ': 28} 2026-10-29 2026-11-09 10
symbols 10250 27 {'succ': 27} 2026-10-13 2026-10-31 7
symbols 10500 27 {'succ': 27} 2026-10-13 2026-10-30 10
references 1000 26 {'succ': 25, 'reco': 1} 2026-10-17 2026-11-10 19
references 1250 25 {'succ': 25} 2026-10-14 2026-11-10 17
symbols 2750 22 {'succ': 22} 2026-10-14 2026-11-10 17
symbols 10750 21 {'succ': 21} 2026-10-17 2026-10-30 7
workspace 1250 20 {'succ': 20} 2026-10-16 2026-11-07 19
symbols 40000 20 {'succ': 20} 2026-10-29 2026-11-01 10
doc 1000 19 {'succ': 19} 2026-10-24 2026-11-10 11
symbols 3250 18 {'succ': 18} 2026-10-14 2026-11-09 10
symbols 3500 18 {'succ': 18} 2026-10-17 2026-11-10 14
symbols 10000 17 {'succ': 17} 2026-10-13 2026-11-10 7
workspace 1750 17 {'succ': 17} 2026-10-16 2026-11-08 12
doc 1250 16 {'succ': 16} 2026-10-16 2026-11-10 11
workspace 2000 15 {'succ': 15} 2026-10-13 2026-11-05 10
symbols 3000 15 {'succ': 15} 2026-10-24 2026-11-10 14
symbols 11000 14 {'succ': 14} 2026-10-17 2026-11-09 8
symbols 3750 14 {'succ': 14} 2026-10-17 2026-11-10 12
references 1500 13 {'succ': 13} 2026-10-13 2026-11-09 11
grep 1750 13 {'succ': 13} 2026-10-16 2026-11-09 8
symbols 11500 13 {'succ': 13} 2026-10-17 2026-10-29 6
symbols 4000 13 {'succ': 13} 2026-10-29 2026-11-10 7
grep 2000 12 {'succ': 12} 2026-10-16 2026-11-09 9
symbols 12000 12 {'succ': 12} 2026-10-17 2026-10-24 4
edit_code 3000 12 {'reco': 11, 'succ': 1} 2026-10-24 2026-11-08 4
grep 2250 11 {'succ': 11} 2026-10-17 2026-11-09 7
memory 1750 11 {'succ': 11} 2026-10-29 2026-11-09 6
symbols 8750 11 {'succ': 11} 2026-11-04 2026-11-10 6
references 1750 10 {'succ': 10} 2026-10-16 2026-11-10 9
workspace 1500 10 {'succ': 10} 2026-10-16 2026-10-28 10
symbols 9000 10 {'succ': 10} 2026-10-31 2026-11-10 6
call_graph 1250 9 {'erro': 1, 'succ': 8} 2026-10-14 2026-11-10 9
symbols 11250 9 {'succ': 9} 2026-10-17 2026-11-09 4
librarian 1250 9 {'succ': 9} 2026-10-21 2026-11-04 6
symbols 8500 9 {'succ': 9} 2026-11-04 2026-11-10 5
doc 1500 8 {'succ': 8} 2026-10-13 2026-11-10 7
symbols 5750 8 {'succ': 8} 2026-10-16 2026-11-09 8
symbols 5250 8 {'succ': 8} 2026-10-17 2026-11-05 6
symbols 11750 8 {'succ': 8} 2026-10-17 2026-10-24 5
symbols 4500 8 {'succ': 8} 2026-10-17 2026-11-09 6
references 2000 8 {'succ': 8} 2026-10-18 2026-11-08 6
references 5000 8 {'succ': 6, 'reco': 2} 2026-10-18 2026-11-09 7
memory 2750 8 {'succ': 8} 2026-10-19 2026-11-06 5
symbols 4250 8 {'succ': 8} 2026-10-24 2026-11-10 8
doc 1750 8 {'succ': 8} 2026-10-29 2026-11-09 7
call_graph 1000 7 {'succ': 7} 2026-10-13 2026-11-06 5
librarian 6750 7 {'succ': 7} 2026-10-14 2026-11-03 3
grep 2750 7 {'succ': 7} 2026-10-16 2026-11-10 6
edit_code 1250 7 {'succ': 7} 2026-10-17 2026-11-08 6
edit_code 2000 7 {'succ': 7} 2026-10-17 2026-11-10 5
symbols 7500 7 {'succ': 7} 2026-10-18 2026-11-10 5
memory 2500 7 {'succ': 7} 2026-10-29 2026-11-01 3
grep 20000 7 {'succ': 7} 2026-11-01 2026-11-10 7
symbols 12250 6 {'succ': 6} 2026-10-17 2026-10-24 2
librarian 2000 6 {'succ': 6} 2026-10-17 2026-11-04 5
edit_code 1750 6 {'succ': 6} 2026-10-17 2026-11-09 5
memory 3000 6 {'succ': 6} 2026-10-19 2026-11-04 4
call_graph 2250 6 {'succ': 6} 2026-10-31 2026-11-10 2
symbols 6750 6 {'succ': 6} 2026-11-09 2026-11-10 2

rows in [40000,40700]: 53 Counter({'run_command': 29, 'symbols': 21, 'references': 3}) dates 2026-10-13 2026-11-10 outcomes Counter({'success': 51, 'recoverable_error': 2}) sessions 28 time_s 2131
edit_code 15-19s 7 Counter({'lsp_document_symbol_null': 7}) Counter({'recoverable_error': 7}) 2026-10-24 2026-11-01
the language server answered textDocument/documentSymbol with null for /home/marius/work/claude/codescout/src/tools/grep.rs on every attempt for 5000 ms — hint: null means the server has not answered,
the language server answered textDocument/documentSymbol with null for /home/marius/work/claude/codescout/src/librarian/tools/create.rs on every attempt for 5028 ms — hint: null means the server has n
rows in [5000,5120] non-rc 85 Counter({'symbols': 69, 'doc': 4, 'create_file': 4, 'references': 2, 'edit_file': 2, 'call_graph': 2, 'workspace': 1, 'edit_code': 1}) [(None, 76), ('write_lock_unavailable', 8), ('lsp_document_symbol_null', 1)]
```

## RAW lat_q7d.out
```
workspace 30 dates [('2026-10-14', 3), ('2026-10-15', 1), ('2026-10-16', 1), ('2026-10-17', 2), ('2026-10-19', 7), ('2026-10-20', 1), ('2026-10-24', 1), ('2026-10-29', 5), ('2026-10-30', 9)] sum_s 951
index 15 dates [('2026-10-13', 1), ('2026-10-14', 2), ('2026-10-16', 1), ('2026-10-17', 2), ('2026-10-21', 1), ('2026-10-28', 5), ('2026-10-29', 2), ('2026-10-30', 1)] sum_s 462
librarian 20 dates [('2026-10-13', 1), ('2026-10-16', 1), ('2026-10-17', 1), ('2026-10-18', 1), ('2026-10-21', 1), ('2026-10-24', 1), ('2026-10-30', 9), ('2026-10-31', 1), ('2026-11-01', 1), ('2026-11-07', 1), ('2026-11-09', 2)] sum_s 1066
references 22 dates [('2026-10-14', 1), ('2026-10-17', 2), ('2026-10-18', 2), ('2026-10-19', 1), ('2026-10-29', 5), ('2026-10-31', 2), ('2026-11-05', 1), ('2026-11-07', 2), ('2026-11-09', 4), ('2026-11-10', 2)] sum_s 699
call_graph 4 dates [('2026-10-19', 1), ('2026-11-01', 1), ('2026-11-09', 2)] sum_s 114
Counter({'status': 17, None: 13}) parent? Counter({False: 26, True: 4})
workspace {'2026-10a': (193, 548, 48789), '2026-10b': (271, 640, 54618), '2026-11a': (425, 103, 5969)}
index {'2026-10a': (6, 28529, 38923), '2026-10b': (12, 30084, 42197), '2026-11a': (3, 266, 2245)}
librarian {'2026-10a': (77, 99, 111498), '2026-10b': (80, 2597, 128955), '2026-11a': (128, 96, 46584)}
[('2026-10-29', 'status', 30084), ('2026-10-29', 'status', 33314), ('2026-10-30', 'verify', 33591), ('2026-11-05', 'status', 266), ('2026-11-05', 'status', 207), ('2026-11-07', 'verify', 2245)]
librarian>=20s dates [('2026-10-13', 1), ('2026-10-16', 1), ('2026-10-17', 1), ('2026-10-18', 1), ('2026-10-21', 1), ('2026-10-24', 1), ('2026-10-30', 9), ('2026-10-31', 1), ('2026-11-01', 1), ('2026-11-07', 1), ('2026-11-09', 2)] actions Counter({'reindex': 9, None: 9, 'doctor': 1, 'audit_doc_refs': 1})
```
