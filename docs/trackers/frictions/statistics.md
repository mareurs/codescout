---
kind: tracker
status: active
title: "Frictions — usage statistics (latency, grep, rg and git grep, the git library)"
tags:
  - frictions
  - usage-stats
  - latency
  - grep
  - git
---

# Frictions — usage statistics

This tracker holds the measured facts behind the friction ledger. The ledger is in [index.md](index.md). This file has no entry prefix, so you can edit it directly. Add a dated section when you re-measure.

First pass: 2026-11-11. Five analysts measured one topic each: baseline, latency, `grep`, a `grep` versus `rg` versus `git grep` benchmark, and the git library. I re-ran thirteen of the headline queries myself (section 13). Every other number is an analyst count, and its source file is named. The full reports are in `docs/research/` (section 14).

**Read this first.** Frequency is not friction (TU-7). A large count shows that a gate fired or that a tool is popular. It does not show that an agent struggled. Where this file talks about friction, it uses recovery shape: what the agent did next.

## 1. Headline findings

| # | Finding | Evidence | Section |
|---|---|---|---|
| 1 | `run_command` is 98.0% of all recorded latency. Wait loops are 51.0% of that, and builds and tests are 30.5%. | 785,379 s of 801,645 s | 4.1 |
| 2 | `latency_ms` understates what the agent waits. About 237 ms per call sits outside it for ids above 290000. | recorder gap plus retention sweep | 4.3 |
| 3 | The usage recorder runs a full-table `DELETE` scan after every insert. 143 rows were dropped for `database is locked`. | `EXPLAIN` shows `SCAN tool_calls` | 4.3, 10 |
| 4 | `grep` has p50 16 ms and p99 1.65 s. The slow tail is walks above the project root. | 9,227 calls | 5.5 |
| 5 | `grep` is fast only when its 200-candidate cap ends the walk early. For equal work it is 3.5 times slower than `rg` in repo scope and 26 times slower on a 3.5 GB corpus. | benchmark, 51 combinations | 6.3 |
| 6 | `grep` results equal `rg` results in 51 of 51 combinations. `git grep` differs. | parity run | 6.4 |
| 7 | No zero result of `grep` was provably false. 47.8% of checkable zeros are scope-guess misses. | 418 zeros checked on disk | 5.4 |
| 8 | Keep `git2`. Dropping it saves about 1.07 MiB and 69 CPU-seconds on a cold build. It would make per-call reads 6 to 10 times slower. | benchmark and build runs | 7 |
| 9 | Several instruments are blind. `outcome` hides timeouts. `output_json` exists only in debug mode. Dropped rows are censored. | source and data | 2.2 |

## 2. Method and instrument validity

### 2.1 Corpus

| Item | Value |
|---|---|
| Table | `.codescout/usage.db`, table `tool_calls` |
| Snapshot filter | `id <= 299175` |
| Rows | 118,278 (ids 180898 to 299175, contiguous) |
| Window | 2026-10-12 08:06:52 to 2026-11-10 22:23:51 UTC |
| Retention | rows older than 30 days are deleted on every insert (`src/usage/db.rs:572-581`), so every count here is a floor on all-time use |
| Sessions | 464 `session_id` values (one per server process), 710 `cc_session_id` values |
| Machine | 64 cores, git 2.56.0, rg 15.2.0, hyperfine 2.0.0 |

The database is live. Counts for the same filter drift by a few rows when other sessions write. Use the same `id` bound when you compare.

### 2.2 What each column measures

| Column | Meaning | Blind spot |
|---|---|---|
| `latency_ms` | Time from just before the tool body to just after it. It includes `call_content` post-processing and, for `run_command`, the attribution engine. | It excludes the cross-process write-lock wait, the recorder work after the call, the retention sweep, and MCP transport. |
| `started_at` | Taken with the timer, just before the tool body. | none known |
| `called_at` | Evaluated inside the `INSERT`, after `open_db` and after the write lock. | Lock waits on insert land in the gap between `started_at` and `called_at`. |
| `outcome` | `success`, `recoverable_error` or `error`. It records whether the tool delivered a result. | A `run_command` timeout and a non-zero exit are both `success`. All 4,535 rows of 20 s or more are `success`. |
| `overflowed` | The first text block parses as JSON with an `output_id` key. | It is also true for `run_command(run_in_background)` "Started" envelopes: 3,247 of 7,210 rows. `read_file` buffers (`@file_`) are never recorded. |
| `input_json` | Always recorded since 2026-11-02 (`src/usage/mod.rs:330`). | Empty on 22,463 rows (19.0%). The last empty row is 2026-11-04 16:17. |
| `output_json` | Recorded only when the recorder runs in debug mode (`src/usage/mod.rs:335`, `if self.debug`). | Present on 63.5% of rows: 44.9% of parent rows and 100% of subagent rows. Why subagents always have it is not verified. It looks truncated near 20 kB. |
| `tool_use_id` | Mirrors the presence of `output_json` exactly (75,061 rows). | same |
| `err_family` | Derived label. | NULL on 171 errors (4.4%). |

Other traps:

- Only MCP calls are recorded. Native `Read`, `Grep` and `Bash` calls are invisible.
- A subagent shares its parent's `session_id`. Key on `(session_id, agent_id)`.
- 61.7% of calls come from the main checkout. The rest are claude agent worktrees (16.5%), repo worktrees (10.4%), codex worktrees (6.5%), `/tmp` harness runs (4.0%) and other roots (0.9%). Harness and eval roots are 12.4% of calls.
- Rows dropped for `database is locked` have no row. The slowest recorder waits are censored.
- The string `cs-hint` in `output_json` is usually echoed file content, not a delivered hint.

## 3. Call mix, sessions and time

### 3.1 Tool mix

| Tool | Calls | Share | Error rate | Notes |
|---|---|---|---|---|
| `run_command` | 51,365 | 43.4% | 2.47% | 96.0% of its errors are the two `il3` gates |
| `read_file` | 19,730 | 16.7% | 5.20% | 14.1% immediate-repeat rate after an error |
| `symbols` | 10,569 | 8.9% | 0.66% | |
| `grep` | 9,227 | 7.8% | 0.69% | |
| `edit_file` | 8,485 | 7.2% | 7.84% | |
| `doc` | 6,786 | 5.7% | 5.57% | |
| `edit_code` | 4,492 | 3.8% | 5.59% | |
| `create_file` | 3,833 | 3.2% | 2.66% | |
| `memory` | 1,491 | 1.3% | 5.03% | |
| `references` | 260 | 0.22% | 4.62% | |
| `call_graph` | 130 | 0.11% | 3.08% | |
| `semantic_search` | 14 | 0.01% | 0% | |
| `symbol_at` | 7 | 0.01% | 0% | |

Total errors: 3,928 (3.32%). 98.9% are `recoverable_error`.

The mix moves with time. From the week of 2026-10-12 to the week of 2026-11-09: `doc` 2.7% to 11.6%, `symbols` 7.5% to 13.8%, `grep` 11.3% to 4.5%, `edit_file` 9.4% to 4.8%. The cause is a hypothesis. The `grep` and `edit_file` shares in mid October are lifted by two subagent-fleet sessions on 2026-10-16 to 2026-10-19.

### 3.2 Sessions and subagents

- p50 session has 71 calls. p90 has 421. The maximum has 8,777.
- The top 1% of sessions (4) hold 26.5% of calls. The top 10% (46) hold 69.4%.
- 35 sessions span more than 24 hours.
- 78 sessions use subagents (603 distinct `agent_id` values, at most 55 in one session).
- Subagents make 33.7% of calls and 43.3% of errors (error rate 4.26% against 2.84% for parents).
- 142 sessions touch more than one root class, because `workspace(activate)` changes `project_root`.

### 3.3 Time shape

- 2026-10-25 has zero rows. The gap is 47.2 h (2026-10-24 15:37 to 2026-10-26 14:48). The cause is not known.
- Saturdays have the highest mean (6,344 calls a day against 3,171 to 3,746), but there are only four, and 2026-10-17 alone is 9,333 calls.
- Hours 06 to 19 UTC hold 4.1% to 7.1% each. Hours 00 to 02 hold 0.2% to 0.4%.
- 64.5% of calls ran on a `dirty` build. There are 141 distinct build shas, and the top ten serve 48.4% of calls. Build age at call time is p50 0.61 days and p90 3.28 days (an upper bound, from commit date). Build and period are confounded, so no build effect is claimed.

## 4. Latency

### 4.1 Where the time goes

Summed `latency_ms` is 801,645 s (222.7 h). Calls overlap across sessions, so this is tool time, not elapsed time.

| Tool | Calls | p50 ms | p90 ms | p99 ms | max ms | Sum s | Share |
|---|---|---|---|---|---|---|---|
| `run_command` | 51,365 | 94 | 10,725 | 290,236 | 3,000,065 | 785,379 | 98.0% |
| `symbols` | 10,569 | 19 | 703 | 11,487 | 57,560 | 6,606 | 0.8% |
| `grep` | 9,227 | 16 | 292 | 1,651 | 386,711 | 1,900 | 0.2% |
| `librarian` | 285 | 165 | 14,593 | 71,842 | 128,955 | 1,741 | 0.2% |
| `references` | 260 | 1,711 | 15,786 | 42,166 | 55,179 | 1,466 | 0.2% |
| `workspace` | 889 | 127 | 1,423 | 34,903 | 54,618 | 1,340 | 0.2% |
| `edit_code` | 4,492 | 62 | 421 | 2,124 | 45,461 | 998 | 0.1% |
| `doc` | 6,786 | 21 | 112 | 1,028 | 10,981 | 523 | 0.1% |
| `index` | 21 | 27,498 | 37,470 | 41,542 | 42,197 | 465 | 0.1% |
| `memory` | 1,491 | 1 | 1,141 | 3,002 | 4,877 | 439 | 0.1% |
| `edit_file` | 8,485 | 10 | 73 | 361 | 5,014 | 303 | 0.0% |
| `call_graph` | 130 | 452 | 5,043 | 27,977 | 39,788 | 296 | 0.0% |
| `read_file` | 19,730 | 0 | 7 | 33 | 21,487 | 93 | 0.0% |
| `create_file` | 3,833 | 2 | 12 | 55 | 5,033 | 48 | 0.0% |
| `semantic_search` | 14 | 844 | 2,079 | 5,521 | 6,032 | 17 | 0.0% |

Without wait loops and rows at their own timeout, `run_command` still holds 373,092 s, which is 95.8% of the remaining time.

`run_command` time by kind (rows with a recorded command; 9,310 rows have none and cannot be sorted):

| Kind | Calls | Sum s | Share of `run_command` time | p50 s | p90 s |
|---|---|---|---|---|---|
| Wait loops (`until`, `while`, `sleep`, `pgrep -f`) | 2,058 | 400,576 | 51.0% | 90 | 540 |
| `cargo`, `nextest`, `rb`, `with-slot` | 3,926 | 239,149 | 30.5% | 19 | 155 |
| Other | 16,044 | 57,054 | 7.3% | 0.06 | 1.7 |
| `python3` scripts | 4,798 | 30,310 | 3.9% | 0.18 | 5.7 |
| `pytest` | 1,608 | 17,713 | 2.3% | 0.65 | 21.9 |
| `git` | 11,111 | 12,058 | 1.5% | 0.08 | 1.7 |
| At own timeout | 176 | 11,712 | 1.5% | 30 | 60 |
| `gate.sh` | 672 | 7,297 | 0.9% | 0.03 | 0.28 |

Wait loops:

- 1,075 of the 2,058 loops (319,692 s) came from subagents. A subagent gets no wake, so a bounded poll is its only option.
- Two sessions hold 54% of all wait-loop time.
- 539 loops poll an `@bg_N` job (114,372 s). `pgrep -f` loops are the documented self-matching trap (216 loops).
- Wait-loop time fell by week: 209,841 s, 101,350 s, 79,217 s, 7,561 s, 2,607 s. The cause (background jobs plus the wake hook) is a hypothesis.

### 4.2 Tails by tool

| Tool | Tail shape | Cause, as far as it is known |
|---|---|---|
| `symbols` | Without a `path`, the project-wide name search has p50 10.6 s (225 calls, 30% of all `symbols` time). With `name` and `path` it has p50 20 ms (5,610 calls). | One `workspace/symbol` leg per language, each with an 8 s budget (`src/tools/symbol/symbols.rs:954`). Since 2026-11-01 the p50 is 3.4 to 3.9 s (85 rows). That `SHORT_LANG_BUDGET` (1.5 s, line 797) caused the drop is a hypothesis. |
| `symbols` | 21 rows between 40.0 and 40.7 s, 2026-10-29 to 2026-11-01. 83 rows between 5.0 and 5.25 s. | The 40 s `PROGRESS_CEILING` (`src/lsp/progress.rs:31`) and the 5 s re-ask budget, while rust-analyzer answers `documentSymbol` null. |
| `references` | p50 1.7 s even warm (later calls p50 1.4 s). First call per session and root: p50 2.6 s, p90 21 s. | The request itself is slow. Three rows sit at 30 s (`LSP request timed out`) and two at 45 s (`FIRST_CALL_SETTLE_WAIT`). |
| `call_graph` | First call p50 1.8 s, p90 17.2 s. Later calls p50 132 ms. | Rust files are slow (14 rows, p50 5.0 s); Python is fast (30 rows, p50 252 ms). |
| `librarian` | The top 20 rows (20 s or more) hold 61% of tool time. `reindex` p50 13.3 s. `doctor` p50 6.9 s, steady. | Last row of 60 s or more is 2026-11-09. |
| `workspace` | `activate` mean 372 ms (430 rows, p50 132 ms). `status` mean 9.4 s. 30 rows of 20 s or more, all between 2026-10-14 and 2026-10-30. | No row above 6 s since 2026-11-01. |
| `index` | `status` p50 30 s. | No slow row after 2026-10-30. |
| `memory` | `write` p50 1,015 ms (209 calls). `read` p50 0 ms. | Cause not investigated. The spike runs from 2026-10-29 to 2026-11-10. |
| `edit_code` | 7 rows of 15 to 19 s with error family `lsp_document_symbol_null`. `rename` p50 988 ms. | The 5 s null window repeats. See FR-12. |
| `read_file` | 98.4% under 33 ms. | Nothing else is slow. |
| `grep` | Section 5.5. | |

### 4.3 The recorder gap and the retention sweep

The gap is `called_at - started_at - latency_ms`. It is the recorder's own cost before the row is written.

| Measure | Value |
|---|---|
| Gap p50, p90, p99 | 1 ms, 133 ms, 3,738 ms |
| Rows with gap over 1 s | 5,003 (4.2%) |
| Rows with gap over 5 s | 442 |
| Rows with gap over 1 s, by day | 0.2% early in the window, 13.2% on 2026-11-09, 9.8% on 2026-11-08 |
| Mean outside `latency_ms`, ids above 290000 | 236.6 ms per call |

Facts from source and logs:

- `write_record` runs `DELETE FROM tool_calls WHERE called_at < datetime('now','-30 days')` after every insert. `EXPLAIN QUERY PLAN` shows `SCAN tool_calls`. The only index on the table is `idx_tool_calls_tool_use_id`. The database is 295 MB.
- The sweep runs before the call returns, but it appears in neither `latency_ms` nor the gap.
- `usage-lock-holds.log` (from 2026-11-09 20:21) logs holds of 250 ms or more. In 26 hours it holds 879 sweeps (p50 2,659 ms, max 20,004 ms, 2,478 s in total). Nearly all report `deleted=0`. It also holds 398 inserts (p50 1,540 ms) and 73 backfills (p50 2,429 ms).
- The sweeps inside the snapshot window (777) sum to 2,197 s. In the same window the gap sums to 1,547 s over 8,048 rows. So the recorder costs the caller about 465 ms per call there, and 59% of that is the unrecorded sweep.
- A gap histogram has a plateau that ends near 5.0 to 5.1 s. That is the 5 s `busy_timeout`.
- 34 to 36 server processes write to this one database.
- Two rows are not lock waits. Row 212104 has a 9.67 h gap and row 253891 has an 8.4 h gap. Only the same session's rows appear in those windows. Machine suspend is a hypothesis (the monotonic clock stops, the wall clock does not). Exclude gaps over 1 h before you average wall time.
- Commit `e31973932` ("serve the backfill's two scans from partial indexes") fixed only the backfill scans. The retention sweep got its own index afterwards (2026-11-11, `idx_tool_calls_called_at`, not yet committed when this was written). On a copy of the live file the cold `DELETE` took 2,025 ms before and 0.69 ms after. Whether the dropped rows stop is not yet shown. See `docs/issues/2026-11-09-concurrent-pinned-calls-leave-no-usage-row.md`, section Fix.

Not separated by the data: the cost of the scan itself against the cost of lock contention. The timers include the lock wait.

The attribution engine (`run_command` with a non-zero exit) adds more cost. In 1,727 rows since 2026-11-01, `silent` runs take p50 150 ms and `hinted` runs take p50 8,104 ms (177 rows, p90 16.7 s). A failing command that gets a red-attribution hint waits about 8 s for it.

### 4.4 Cold start and LSP starts

First call per session and root, against later calls (p50 / p90 ms):

| Subset | First | Later | p50 penalty |
|---|---|---|---|
| `references` | 2,562 / 21,168 | 1,384 / 8,112 | 1,178 |
| `call_graph` | 1,798 / 17,209 | 132 / 2,193 | 1,666 |
| `symbols` (name and path) | 132 / 2,250 | 17 / 247 | 115 |
| `edit_code` | 94 / 578 | 61 / 394 | 32 |
| Any LSP-class call | 166 / 3,056 | 37 / 513 | 129 |

The p50 penalty is small. The p90 is large. A session id is a server process, and servers are shared, so "first per session" is not "first on a cold server".

`lsp_events` handshake p50 / p90 ms by language: rust 53 / 201, python 182 / 252, javascript 113 / 168, typescript 116 / 175, bash 557 / 1,158, java 2,490 / 3,465, kotlin 2 / 4,455 (bimodal). 339 of 3,376 starts failed. Java failed 206 times (p50 16.5 s, 2026-10-12 to 2026-10-31 only). Do not read the rust `first_response_ms` p50 of 16 ms as a cold cost (memory `research/lsp-cold-start-latency-20261110`).

Of 507 slow LSP-tool calls (5 s or more), 74.0% began within five minutes of a rust LSP start. For ordinary calls the figure is 38.7%. This is an association. Load, restarts and rebuilds would also fit.

### 4.5 Timeouts and budget spikes

- Spikes sit at constants in the code: 20 s (`grep` budget), 30 s (`run_command` default and LSP request), 45 s, 60 s, 120 s, 300 s. At 30 s there are 223 `run_command` rows against 32 in the control windows.
- 208 `run_command` rows ended at their own timeout (32,859 s, 4.2% of `run_command` time). Only 111 carry `"timed_out": true` in `output_json`. All are recorded as `success`. See FR-2.
- No row has error family `tool_timeout`. Outside `run_command`, only `librarian` (6 rows) and `grep` (3 rows) exceed 57.6 s.
- 14 `write_lock_unavailable` rows sit at 5.0 to 5.03 s, the 5 s write-lock timeout. 13 of them are on 2026-11-09.

## 5. The `grep` tool

Source: [the `grep` report](../../research/2026-11-11-usage-stats-grep.md). Coverage: 9,227 rows, 9,047 with input (98.0%), 8,915 with output (96.6%).

### 5.1 Parameters

| Parameter | Rows | Share | Note |
|---|---|---|---|
| `pattern` | 9,047 | 100% | p50 length 40 characters |
| `path` | 7,489 | 82.8% | file 3,843, subdirectory 2,464, root 1,774, absolute outside the project 710, buffer 256 |
| `workspace` | 4,424 | 48.9% | subagents pin the workspace |
| `context_lines` | 4,181 | 46.2% | |
| `glob` | 2,736 | 30.2% | 315 of them are arrays. 12% use a brace set. 5% negate. |
| `limit` | 2,290 | 25.3% | The default 50 is rarely typed. |
| `mode` | 1,796 | 19.9% | `files` 774, `lines` 536, `content` 482 (not in the enum; accepted as `lines`) |
| `output_mode` | 1,622 | 17.9% | Not declared. `content` 1,539 is accepted. 41 rows with `count`, `files_with_matches` or `files` failed with `param_not_applied`. |
| `head_limit` | 778 | 8.6% | Not declared. Silently ignored (section 5.7). |
| `ignore_case` | 299 | 3.3% | |
| `include_hidden` | 216 | 2.4% | |
| `whole_word`, `multiline`, `time_budget_ms` | 3, 3, 11 | under 0.2% | `multiline` is refused. |

Aliased with a "corrected to" note (98 rows): `context`, `-i`, `-C`, `-A`, `-B`. Dropped without a note: `-n`, `-o`, `head_limit`, `offset`, `max_matches`.

### 5.2 Pattern classes

70.2% of patterns (6,352 of 9,047) contain a `|`. Plain multi-word text is 650 patterns (7.2%). An earlier estimate of "about 4,700 plain multi-word" was a raw space count and was wrong by about 7 times.

| Class | Share | Zero rate | p50 / p90 latency ms |
|---|---|---|---|
| Alternation of literals | 43.5% | 6.7% | 17 / 286 |
| Alternation with regex | 23.8% | 8.4% | 16 / 287 |
| Regex without alternation | 10.0% | 6.2% | 10 / 237 |
| Single identifier | 8.5% | 13.9% | 20 / 355 |
| Plain multi-word text | 7.2% | 8.8% | 6 / 193 |
| Longer than 150 characters | 2.7% | 2.1% | 71 / 406 |
| Qualified identifier | 2.6% | 9.5% | 16 / 660 |
| Path-like | 0.6% | 17.0% | 169 / 809 |

### 5.3 Result shape

- 704 results are zero (7.9% of rows with output). 426 of them are a bare `0 matches`. 278 carry a warning or notice.
- 382 results (4.3%) are capped. 540 (6.2%) say "Showing N of M".
- 288 non-zero results (3.6%) carry the hidden-paths note. 146 report skipped binary files.
- Output bytes: p50 1,485, p90 5,476. 20.4 MB in total over 8,915 rows.
- 473 calls (5.1%) overflow into a buffer. Read-back of `grep` buffers is the lowest of any tool: 8.9% (42 of 472).
- Walk-cut notices: 10 rows (7 at about 20.0 s).

### 5.4 Zeros: true or false

Method: for the 418 zero results that ran in the main checkout, check the current tree with `rg` in the same scope. The 284 zeros from deleted worktrees cannot be checked.

| Verdict | Rows | Share of 418 |
|---|---|---|
| The glob rooted at the path admits no file (**all 51 carried the "No file under … passed the glob filter" warning at call time**) | 51 | 12.2% |
| True for the scope. The term exists elsewhere in code. | 135 | 32.3% |
| True for the scope. The term exists only in docs or records. | 56 | 13.4% |
| True for the scope. The match is only in hidden or ignored files. | 9 | 2.2% |
| Absent everywhere now | 70 | 16.7% |
| The tree changed after the call | 36 | 8.6% |
| Path never in git history | 32 | 7.7% |
| Buffer path expired, path gone, or match outside the repo | 28 | 6.7% |
| **False: the stated scope contains the match now and the file is untouched** | **0** | 0% |

Reading: in this repo and window, `grep` returned no provable false negative for the scope it was given. One more row (241483) matched only files that are untracked now, so it cannot be proven either way. The zeros that waste turns are scope-guess misses (200 of 418, 47.8%). The 284 unchecked zeros and the walk-cut class remain unproven.

What the agent did next (704 zeros): 18.9% were followed by a search that found hits. 11.5% widened scope and hit. 2.6% reformulated the pattern. 4.8% found hits through another tool. 58.7% had no related search in the next three calls.

The shape "relative glob repeats the path prefix" caused 61 zeros. All 61 were warned. It still occurred on 2026-11-09 and 2026-11-10, after the 2026-08-27 ADR fix.

### 5.5 Latency by scope

| Slice | Calls | p50 ms | p90 ms | p99 ms | 1 s or more |
|---|---|---|---|---|---|
| All | 9,227 | 16 | 292 | 1,651 | 2.4% |
| `path` is a file | 3,843 | 2 | 25 | 144 | 0% |
| `path` is a subdirectory | 2,464 | 54 | 348 | 1,220 | 1.5% |
| Root, no `path` | 1,774 | 153 | 906 | 3,180 | 9.0% |
| Root, no glob | 198 | 998 | 2,473 | 7,285 | 50.0% |
| Absolute path outside the project | 710 | 5 | 146 | 20,001 | 3.0% |
| `mode=files` | 840 | 114 | 1,178 | 11,072 | 12.1% |
| `include_hidden=true` | 204 | 122 | 1,651 | 19,582 | 21.1% |
| `context_lines=0` / above 0 | 5,311 / 3,736 | 46 / 3 | 439 / 73 | 2,150 / 483 | 3.7% / 0.5% |

- The 20 s walk budget (`DEFAULT_WALK_BUDGET_MS`, commit `59d870c9d`, 2026-10-30) produced seven clean cuts at 19.9 to 20.1 s. All used an absolute path above the project root. None exceeded 20,009 ms after the budget.
- Three earlier rows ran 99.8 s, 135.1 s and 386.7 s. Each used a path above the project root. All three predate the budget, and none is recorded after 2026-10-29.
- Walk size, not regex cost, explains the tail. Rust's regex engine does not backtrack. That claim is a hypothesis for these rows.
- Project-wide `grep` without a glob has p50 998 ms. Project-wide `symbols(name)` has p50 10.6 s. They do different work, but that gap is a plausible reason agents grep for identifiers.

### 5.6 What agents do after a `grep`

- Next call: `grep` 30.6%, `run_command` 22.4%, `read_file` 21.6%, `symbols` 9.1%, `edit_file` 7.0%.
- 38.3% of non-zero results have a hit file read, edited, or opened with `symbols` within three calls.
- 28.2% of identifier greps are followed by an edit that names the same identifier within 20 calls. A random-identifier control gives 1.4%.
- Only 2 of 765 identifier greps are followed by `references` or `call_graph`. This repeats the finding in FR-4: agents do not move from text search to the LSP tools.
- 0 of 9,047 calls repeat the previous call byte for byte. That may be real or a recorder artifact. It is not verified.

### 5.7 Findings about the `grep` tool

| # | Finding | Evidence | Kind |
|---|---|---|---|
| 1 | `head_limit` is silently ignored. | 778 calls (8.4% of all `grep` rows). Of 721 rows with `head_limit` and no `limit`, 167 (23.2%) returned more matches than `head_limit`, and 69 overflowed. The code comment calls the drop harmless (`src/tools/grep.rs:785-790`). | Design question: alias it to `limit`. |
| 2 | `grep` has no revision parameter. | 293 `git grep` calls name a revision (50% of `git grep` calls, 263 from subagents). | Capability gap. |
| 3 | The glob-repeats-the-path-prefix shape recurs. | 61 zeros, all warned. | A pre-flight check is cheaper than a wasted call. |
| 4 | An invalid regex becomes a literal without a note. | `foo(` and `a{2` returned `0 matches` and no mention of the fallback (benchmark, two observed responses). | Possible gap against the ADR on negative results. |
| 5 | Root-scope walks are slow and the walk is serial. | Section 5.5. `grep.rs:312` is `wb.build()`; there is no `build_parallel`. | Latency |

## 6. Shell search, and `grep` against `rg` and `git grep`

Source: [the benchmark report](../../research/2026-11-11-usage-stats-search-bench.md) and the shell section of the `grep` report.

### 6.1 How agents search through the shell

9,781 `run_command` calls contain a search. Families (call level, parsed with `shlex`; a heuristic):

| Family | Calls | Share of `run_command` | Parent / subagent |
|---|---|---|---|
| `git grep` | 581 | 1.1% | 133 / 448 |
| `rg` | 291 | 0.6% | 237 / 54 |
| `grep -r` | 527 | 1.0% | 127 / 400 |
| `grep` on file arguments | 4,333 | 8.4% | 1,602 / 2,731 |
| `find` | 397 | 0.8% | 255 / 142 |
| `grep` as a pipe filter | 2,208 | 4.3% | |
| `grep` on a buffer (`@cmd_`, `@tool_`) | 1,442 | 2.8% | |

`rg` is growing: 8, 99 and 133 calls in the last three weeks. 237 of its 291 calls come from parent sessions. `git grep` is mainly a subagent habit (448 of 581). The search-for-content calls that `grep` could replace (`git grep`, `rg`, `grep -r`, `grep FILE`) total 5,732, which is 11.2% of `run_command` calls.

Searches the `grep` tool cannot serve (heuristic, over 6,128 calls): a search at a revision (293), `find` by time or size (106), `find` by name (291), `rg --files` (60), output forms `-o`, `-v`, `-r`, `--json` and `--stats` (295), and targets inside ignored paths (316). 38.8% of shell searches target an absolute path outside the repo, which `grep(path=abs)` can serve. Why agents still use the shell for those is not verified.

### 6.2 Benchmark method

- A second `codescout start` process served real `grep` and `run_command` calls over stdio, after a 30 s settle. `latency_ms` was joined from `usage.db`. Raw timings used `hyperfine` (15 runs, 2 warm-ups).
- Scopes: repo, `src/`, a glob (`*.rs`), `docs/`, and `~/.cargo/registry/src` (3.5 GB, 163,487 files). 12 patterns on the repo scopes and 8 on the registry.
- The load average was above 12 on some batches. Those were discarded. The primary batches ran at 1.5 to 9.
- All numbers are warm-cache.
- About 85 ms per call sits outside `latency_ms` for every tool. A one-line `read_file` showed 1 ms `latency_ms` and 87 ms client wall time. The cause is unknown.
- A non-zero exit costs about 130 ms through `run_command` (`false` 187 ms, `true` 59 ms). This is why a zero-match `rg` takes 200 ms or more. The cause is unknown.

### 6.3 Timing (median `latency_ms`, quiet batch)

| Search | `grep` lines | `grep` files | `rg` via `run_command` | `git grep` via `run_command` | `rg` raw | `git grep` raw |
|---|---|---|---|---|---|---|
| Repo, rare (`fn clamp_match`) | 304 | 282 | 78 | 143 | 18 | 79 |
| Repo, common identifier | 13 | 281 | 80 | 154 | 19 | 87 |
| Repo, `fn ` (huge) | 5 | 284 | 296 | 382 | 19 | 93 |
| Repo, zero matches | 282 | 280 | 210 | 276 | 18 | 80 |
| `src/`, `Result<Value>` | 585 | 41 | 74 | 88 | 8.6 | 25 |
| `src/`, zero matches | 42 | 41 | 203 | 215 | 8.2 | 24 |
| `docs/`, multi-word | 223 | 226 | 77 | 137 | 17 | 76 |
| Registry, zero matches | 6,776 | 6,803 | 390 | 1,570 | 199 | 1,477 |
| Registry, `TODO\|FIXME` | 74 | 7,451 | 562 | 1,911 | 230 | 1,716 |

Reading the table:

- `grep` lines is fast only when the 200-candidate cap ends the walk early (`limit` 50 times 4). It then answers a smaller question. The `grep` files column is the equal-work comparison with `rg -n`.
- For equal work, `grep` files is 3.5 times slower than `rg` in repo scope, 0.6 times in `src/`, and 26 times slower on the registry.
- `grep` lines is more than 2 times slower than `rg` through `run_command` in 16 cells of 51.
- Measured causes. First, the walk is serial: `grep` CPU is about its wall time (340 ms CPU for 283 ms), while `rg` uses 133 ms CPU to finish in 19 ms. Second, the enclosing-symbol step: `Result<Value>` in `src/` takes 579 ms by default and 18 ms with `context_lines=1`. The step re-reads each matched file and parses it with tree-sitter (`grep.rs:525-551`, analyst read).
- Hypotheses, not measured: whole-file reads against `rg`'s memory maps, and the share of the walk spent on `docs/` jsonl files.
- CPU for the same repo search: `rg` 133 ms, `git grep` 505 ms, `grep` 340 ms. On the registry: `rg` 2.26 s, `git grep` 6.2 s, `grep` 6.8 s.
- On the registry under load 70 to 95, all seven non-PCRE patterns hit the 20 s walk cut.

### 6.4 Parity

- `grep` and `rg` agree in all 51 non-PCRE combinations (same files, same per-file line counts). The line sets of lines mode agreed in 5 of 5 spot checks. This holds only for the combinations tested, not for every pattern.
- `git grep` differs in repo-wide scopes. It finds 52 tracked hidden files that `rg` and `grep` skip (for example `.buddy/memory/INDEX.md`, `.codescout/memories/gotchas.md`). It misses 16 untracked files that `rg` and `grep` find (for example the LSP capability audit written in the same session).
- On the throwaway corpus:

| File | `rg` | `git grep` | `grep` | Why |
|---|---|---|---|---|
| tracked dotfile | misses | finds | misses | hidden |
| tracked, then ignored | misses | finds | misses | ignore files apply |
| untracked | finds | misses | finds | `git grep` is tracked-only |
| listed in `.rgignore` | misses | finds | finds | `.rgignore` is `rg`-only |
| NUL at byte 9,618 | misses | finds | finds | `grep` sniffs only the first 8,192 bytes |
| 70 MiB file | finds | finds | misses, and says so | `MAX_FILE_BYTES` is 64 MiB |
| NUL at the start | misses | prints "Binary file matches" | misses | binary |

### 6.5 Output size (a proxy for token cost)

- A search for `RecoverableError` across the repo prints 1.89 MB through `rg -n` (huge jsonl lines) and 3.06 MB through `git grep -n`. `grep` buffers 45.5 KB in full, because it clamps each line to 2,000 bytes and the whole collection to 60 KB.
- What the agent sees inline is capped for both: about 2 KB plus a buffer handle (`rg` 2,086 B, `grep` 3,286 B).
- Per shown match `grep` is larger: 132 against 95 bytes per match for 134 hits in `src/`.
- `rg ... | head -50` is allowed and still returns an `unfiltered_output` handle (up to 4 MiB). A `;` or `&&` after a pipe is refused by gate IL3.
- Without an explicit `workspace(activate)`, every call carried a 2.2 KB banner about linked worktrees. That is about 550 tokens a call.

### 6.6 Which tool for which search

| Search | Use | Why |
|---|---|---|
| Common identifier, `fn `, alternations with many hits | `grep` lines | 5 to 108 ms, bounded output, enclosing symbols. It is a capped sample, not a count. |
| Counting or per-file ranking in `src/` | `grep` with `mode="files"` | 40 ms, same counts as `rg` |
| Rare or zero-hit search of the whole repo or `docs/` | `rg` through `run_command` | 78 ms against 280 to 454 ms |
| Few matches spread over many files, and you do not need symbols | `rg`, or `grep` with `context_lines=1` | The symbol step costs 170 to 560 ms. |
| A large non-git corpus | `rg` | 258 to 562 ms against 1.4 to 7.2 s |
| Look-around, multiline, `-P`, `-v`, replace | `rg` | `grep` refuses them |
| History, a revision, tracked-only | `git grep` | The only option |
| Huge single-line files | `grep` | Clamps lines at 2,000 bytes |
| Files over 64 MiB | `rg` | `grep` skips them and says so |

Do not conclude that `grep` is faster in general. It is faster only when its cap ends the walk early.

## 7. The git library

Source: [the git library report](../../research/2026-11-11-usage-stats-git-library.md).

**Decision: keep `git2`.** Do not drop it for the CLI. Do not move to `gix` now.

### 7.1 Inventory

- `git2 0.20` is in `Cargo.toml` since the initial commit (2026-02-25). Features: no default features, vendored libgit2.
- 97 lines contain `git2::`: 32 in production, 62 in tests, 3 in comments. 18 production functions open a libgit2 repository. One of them, `git::diff_tree_to_tree`, has no production caller. Its only callers are four tests in `src/git/mod.rs` (lines 116, 128, 151 and 162). A reference lookup of the crate function also returned `src/librarian/tools/doctor.rs:8362`, but that line calls the `git2` method of the same name, not the crate function.
- `Command::new("git")` appears on 5 production lines and 20 test lines in `src/`, and on 22 test lines in `tests/`.
- No ADR chooses libgit2. The recorded reason is rule WIN-14 (commit `dd3bff8f7`): on a locked-down Windows VDI, every process spawn is taxed by EDR injection. The per-spawn cost is not measured anywhere.
- Hot paths: the provenance fence spawns one `git rev-parse` per `run_command` or write call (Unix). The observation store opens libgit2 at least four times per call. `run_command` observation spawns five `git` processes per read-declared call. Activation opens libgit2 twice.

### 7.2 Speed (median ms, warm, load about 10)

| Operation | libgit2 | CLI | Ratio |
|---|---|---|---|
| Find the repository root | 0.17 | 1.47 | CLI 8.6 times slower |
| HEAD id | 0.26 | 1.73 | 6.7 times |
| Remote list | 0.17 | 1.75 | 10 times |
| Worktree list (39 worktrees) | 0.38 | 4.31 | 11 times |
| Path ignored check | 0.25 | 2.89 | 11.6 times |
| Ahead and behind | 1.47 | 3.83 | 2.6 times |
| Read and iterate the index | 5.80 | 3.90 | CLI 1.5 times faster |
| Status with untracked files | 51.1 | 22.5 | CLI 2.3 times faster |
| Tree diff, 5 commits | 9.4 | 4.0 | CLI 2.4 times faster |
| Tree diff, 200 commits, with renames | 263.5 | 76.5 | CLI 3.4 times faster |
| Revwalk, last 200 commits (`Sort::TIME`) | 196.5 | 3.9 | CLI 50 times faster |
| Revwalk, all commits, ids only | 193.8 | 15.0 | CLI 13 times faster |
| Blame of one file | 1,980 | 19.5 | CLI 100 times faster |

- `Sort::TIME` costs a constant 195 ms for 1 to 5,000 commits. Without it, 200 commits take 5.2 ms. The mechanism (an eager full walk) is a hypothesis.
- Turning off strict hash verification helps by 27% to 38% and still leaves libgit2 30 to 37 times slower than the CLI on the 200-commit walk.
- `gix` 0.81 in process: find root 0.23, HEAD 0.34, revwalk of 200 commits 1.25, all commits 3.25, status 22.6, tree diff 9.05 (200 commits). It matches CLI speed, in process.
- Pure spawn cost: 1.24 to 1.40 ms per `git` process on this machine. Under EDR on Windows the cost is unknown.

### 7.3 Build and size

| Option | Release wall | Release CPU | Binary growth | Packages |
|---|---|---|---|---|
| `git2` | 22.8 s | 84.1 s | +1.07 MiB (2.6% of the 43.8 MB binary) | 51 |
| `gix` 0.81 | 19.3 s | 121.9 s | +3.34 MiB (about 8%) | 156 |

`git2`'s marginal build cost is about 69 CPU-seconds (release) and 39 (dev), once per cold target directory. It does not need `cmake` or OpenSSL, only a C compiler. CI already builds it on Linux, macOS and Windows.

### 7.4 Semantics and risk

- Linked worktrees work (this repo has about 40).
- Sparse checkout: libgit2 `statuses` reports every out-of-cone file as deleted. The CLI is clean.
- A sparse index, reftable ref storage and SHA-256 repositories all fail to open in libgit2. Every site then takes its "not a repository" branch, with misleading text.
- A partial clone fails when blame needs a missing blob.
- libgit2 never runs hooks and ignores fsmonitor. Production code only reads, so neither matters today.
- Zero rows in `usage.db` and zero documents name a libgit2 defect.

### 7.5 Decision matrix

| Cluster | Verdict |
|---|---|
| Per-call and activation reads (observation store, policy, HEAD, remotes, index state) | **Keep `git2`.** 6 to 10 times faster than the CLI. |
| `run_command` observation (`observe.rs`) | **Keep the CLI.** libgit2 status is 2.3 times slower and gives false-dirty on sparse checkouts. |
| Provenance fence `canonical_checkout` | Optional: move to `git2`. About 1.3 ms saved per call. Effort S. |
| `doc move` (`git grep`), `doc delete`, `merge_worktree` | **Keep the CLI.** libgit2 has no `grep`. |
| `librarian doctor`, `gc`, `reindex`, `gather` history walks and blame | Keep for now. These are the only places libgit2 is badly slower. A `Sort::NONE` or CLI fix is cheaper than a `gix` migration. Measure doctor with its real pathspec first. |
| Dashboard `git_info` | **Fix a bug** (section 7.6). |
| `git::diff_tree_to_tree` | **Remove**: dead in production. Effort S. |

### 7.6 Defects found

- **Dashboard `git_info` reports a clean checkout as dirty.** `dirty` is `repo.statuses(None)...!s.is_empty()` (`src/dashboard/api/project.rs:45-52`). `statuses(None)` includes ignored files. On a scratch repo with `target/` ignored, the CLI is empty and libgit2 returns one `IGNORED` entry. On a sparse checkout it also reports out-of-cone files as deleted. The bug exists only with the `dashboard` feature. It is not yet in `docs/issues/`.
- **Dead code:** `git::diff_tree_to_tree`.

## 8. Output size and context cost

152.4 MB of output is stored over 75,061 rows (63.5% of rows). Byte counts include JSON escaping and are not tokens.

| Tool | Share of stored bytes | p50 / p99 bytes |
|---|---|---|
| `run_command` | 34.7% | 641 / 12,035 |
| `read_file` | 31.0% | 3,362 / 9,980 |
| `grep` | 13.4% | 1,485 / 10,329 |
| `symbols` | 7.5% | 2,634 / 11,115 |
| `doc` | 4.7% | 885 / 16,197 |
| `workspace` | 3.0% | 6,554 / 25,765 |
| `get_guide` | 3.0% | 11,854 / 70,981 |

- The 15 largest outputs are all `get_guide tracker-conventions` (67 to 71 KB).
- `librarian` overflows on 33.7% of calls, with median 21,423 tokens. 13.4 million overflow tokens in total.
- Read-back of a buffer handle in a later call: `@cmd` 36.1%, `doc` 63.9%, `librarian` 80.0%, `symbols` 20.3%, `grep` 8.9%. A handle named later is not proof of retrieval.
- Parent rows for `symbols`, `doc` and `memory` mostly lack `output_json`, so their size statistics describe subagent calls.

## 9. Errors and recovery

- 3,928 errors (3.32%). 78.4% carry a `— hint:` (3,081). There are 1,135 distinct hint texts.
- Five gate and teaching families are 51.8% of errors: `il3_pipe_to_trimmer` 722, `il3_shell_on_source` 498, `il1_read_overlaps_symbol` 356, `il2_structural_edit` 255, `librarian_managed_artifact` 202. Do not read them as friction.
- After an error, the next call of the same principal succeeds 87.1% of the time with a hint and 93.3% without (mostly `run_command`). The difference is confounded by tool mix.
- 7.3% of errors are an immediate repeat of the same signature. By tool: `read_file` 14.1%, `doc` 10.8%, `call_graph` 1 of 4.
- Median calls to recovery is 1. 3 errors (0.1%) are unrecovered within 25 calls.
- 20.3% of errors sit in runs of two or more (longest run 10).
- Highest immediate-repeat families (3 or more repeats): `il1_read_overlaps_symbol` 21.8%, `unknown_enum_value` 18.2%, `path_not_found` 16.8%, `worktree_activate_required` 15.2%.
- Delivered hints: 5,564 rows (4.7%) carry a delivery record (`guide-sections` 3,745, `session-opener` 1,660, `operator-rules` 159). Error rows never carry one, because an error never reaches the delivery fan-out.

## 10. Recording health

- 143 rows were dropped for `database is locked` by the end of the snapshot (194 by the time of the last read of the log). Each waited about 5 s (p50 5,008 ms). That is about 1.2% of attempted calls in the 33 hours after the first drop.
- Drops are not limited to one tool: `run_command` 56, `read_file` 44, `symbols` 41, `grep` 26.
- Both logs start on 2026-11-09. Earlier days have no log to explain the gap.
- `observed_changed` is 1 on 234 of 19,204 read-declared `run_command` rows (all `success`). A 1 can be a peer's edit on a shared checkout.
- A benchmark batch under load 54 lost 5 of 368 rows. So drops also happen on a benchmark run.

## 11. Candidates for the ledger

These are findings that the ledger does not yet hold. Each needs a decision, then an entry or a bug file.

| Candidate | Evidence | Kind | Size |
|---|---|---|---|
| `grep` silently ignores `head_limit` | 778 calls; 23.2% over the limit | design | S |
| `grep` has no revision parameter | 293 `git grep` calls at a revision | capability | M |
| The recorder serialises on one database and scans on every insert | section 4.3 | bug or performance. The scan is fixed (2026-11-11, index on `called_at`). Lock contention between server processes is still open, and the drops are not yet shown to stop. | M |
| Dashboard `git_info` is dirty for ignored files | section 7.6 | bug (needs `docs/issues/` file) | S |
| `git::diff_tree_to_tree` is dead code | section 7.1 | cleanup | S |
| `grep` turns an invalid regex into a literal without a note | 2 observed responses | possible ADR gap | S |
| `Sort::TIME` pays a 195 ms floor for 1 to 20 commits (`gather`, `doctor`) | section 7.2 | performance | S |
| `rg` through `run_command`: +130 ms on a non-zero exit, +85 ms on every call | section 6.2 | measurement, cause unknown | M |
| `memory(write)` costs about 1 s | 209 calls | performance, cause unknown | S |
| Path-less `symbols(name)` is slow | p50 10.6 s, now 3.4 to 3.9 s | watch | S |
| Three `grep` walks outlived the 60 s server timeout | none since 2026-10-29 | resolved by the 20 s budget (not proven) | - |

## 12. Not verified

Tool and data:

- Cold-cache behaviour of every benchmark. All numbers are warm.
- The cause of the 85 ms per call outside `latency_ms`, and of the 130 ms non-zero-exit cost.
- Whether the retention scan or lock contention makes the sweep slow (the timers include the lock wait).
- Whether machine suspend caused the 9.7 h and 8.4 h gaps.
- Whether the walk is the reason three `grep` calls outlived the 60 s timeout.
- That `SHORT_LANG_BUDGET` caused the drop in project-wide `symbols` time.
- Why subagent rows always carry `output_json`.
- Whether failed java and kotlin starts block tool calls.
- The body of the 40 s and 5 s `symbols` rows. Their outcome is `success`.
- Why agents choose the shell for `grep`-servable searches (hypotheses only).

Benchmark limits:

- Registry rows for `grep` have two repeats each.
- Lines-mode parity covers 5 searches, not all 51. PCRE results were not compared.
- The benchmark ran in a standalone server, not an interactive session.
- 284 zeros from deleted worktrees and 18 expired buffers could not be checked.
- Shell-search families come from a `shlex` heuristic.

Git library:

- The EDR cost per spawn on the Windows VDI. No Windows or macOS run was made.
- `safe.directory` behaviour (no foreign-owned repository was available).
- Why `Sort::TIME` costs a constant 195 ms.
- `doctor` with its real pathspec, and libgit2's share of the real gate compile time.
- `gix` blame, rename-tracking diff, worktree list and reftable support.

## 13. SQL recipes

Run read-only: `sqlite3 -readonly .codescout/usage.db`. Each recipe below was run on 2026-11-11 and matched the figure in this file, except where a note says otherwise.

```sql
-- R1 corpus size and error rate (118278 rows, ids 180898-299175, 3928 errors)
SELECT COUNT(*), MIN(id), MAX(id), SUM(outcome<>'success') FROM tool_calls WHERE id<=299175;

-- R2 call mix
SELECT tool_name, COUNT(*) n, ROUND(100.0*COUNT(*)/(SELECT COUNT(*) FROM tool_calls WHERE id<=299175),2) share_pct
FROM tool_calls WHERE id<=299175 GROUP BY tool_name ORDER BY n DESC;

-- R3 recorder gap percentiles (p50 1, p90 133, p99 3738)
WITH g AS (SELECT (julianday(called_at)-julianday(started_at))*86400000.0 - latency_ms AS gap
           FROM tool_calls WHERE id<=299175 AND started_at IS NOT NULL),
     o AS (SELECT gap, ROW_NUMBER() OVER (ORDER BY gap) rn, COUNT(*) OVER () n FROM g)
SELECT ROUND(MIN(CASE WHEN rn>=n*0.5  THEN gap END)) p50,
       ROUND(MIN(CASE WHEN rn>=n*0.9  THEN gap END)) p90,
       ROUND(MIN(CASE WHEN rn>=n*0.99 THEN gap END)) p99 FROM o;

-- R4 latency percentiles for one tool (grep: p50 16, p90 292, p99 1652; the analyst rounded p99 to 1651)
WITH o AS (SELECT latency_ms v, ROW_NUMBER() OVER (ORDER BY latency_ms) rn, COUNT(*) OVER () n
           FROM tool_calls WHERE id<=299175 AND tool_name='grep')
SELECT MIN(CASE WHEN rn>=n*0.5  THEN v END) p50, MIN(CASE WHEN rn>=n*0.9  THEN v END) p90,
       MIN(CASE WHEN rn>=n*0.99 THEN v END) p99 FROM o;

-- R5 share of summed latency by tool (run_command 785379 s, 98.0%)
SELECT tool_name, SUM(latency_ms)/1000 sum_s FROM tool_calls WHERE id<=299175 GROUP BY tool_name ORDER BY sum_s DESC;

-- R6 run_command timeouts recorded as success (123 rows with this loose match; the analyst's strict match gave 111)
SELECT outcome, COUNT(*) FROM tool_calls
WHERE id<=299175 AND tool_name='run_command' AND output_json LIKE '%timed_out%: true%' GROUP BY outcome;

-- R7 bare zero-match grep results (426)
SELECT COUNT(*) FROM tool_calls
WHERE id<=299175 AND tool_name='grep' AND output_json='[{"type":"text","text":"0 matches"}]';

-- R8 grep calls that pass head_limit (778; 8.4% of all grep rows, 8.6% of rows with input)
SELECT COUNT(*) FROM tool_calls WHERE id<=299175 AND tool_name='grep' AND input_json LIKE '%"head_limit"%';

-- R9 errors that carry a hint (3928 and 3081)
SELECT COUNT(*), SUM(error_msg LIKE '%— hint:%') FROM tool_calls WHERE id<=299175 AND outcome<>'success';

-- R10 error families
SELECT err_family, COUNT(*) n FROM tool_calls WHERE id<=299175 AND outcome<>'success' GROUP BY err_family ORDER BY n DESC;

-- R11 project-wide symbols(name) with no path (225)
SELECT COUNT(*) FROM tool_calls
WHERE id<=299175 AND tool_name='symbols' AND input_json LIKE '%"name"%' AND input_json NOT LIKE '%"path"%';

-- R12 coverage gaps (22463 without input, 43217 without output)
SELECT SUM(input_json IS NULL OR input_json=''), SUM(output_json IS NULL OR output_json='') FROM tool_calls WHERE id<=299175;

-- R13 retention sweep plan (expect SCAN tool_calls)
EXPLAIN QUERY PLAN DELETE FROM tool_calls WHERE called_at < datetime('now','-30 days');
```

Wait-loop, shell-search, zero-result and parity analyses are Python, not SQL. They are in the script archive (section 14).

## 14. Evidence files

| File | Content |
|---|---|
| [baseline report](../../research/2026-11-11-usage-stats-baseline.md) | call mix, sessions, output size, errors, time series, builds, recording health, hints |
| [latency report](../../research/2026-11-11-usage-stats-latency.md) | definition of `latency_ms`, tails, cold start, timeouts, budgets, raw tables |
| [`grep` report](../../research/2026-11-11-usage-stats-grep.md) | parameters, patterns, zeros, latency, chains, shell substitution |
| [benchmark report](../../research/2026-11-11-usage-stats-search-bench.md) | `grep` against `rg` against `git grep`: timing, CPU, parity, tokens, gates |
| [git library report](../../research/2026-11-11-usage-stats-git-library.md) | inventory, benchmark, semantics, build cost, removal cost |
| `docs/research/data/2026-11-11-usage-stats-scripts.tar.gz` | the analysts' scripts, the `git2` and `gix` benchmark crates, and the SQL recipes. The scripts read scratch pickles that no longer exist. |

The ledger entries that these statistics support: FR-2 (timeouts as success), FR-3 (recording gaps), FR-4 (LSP tools used rarely), FR-14 (wait loops), FR-16 (`semantic_search`).
