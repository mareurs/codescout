---
kind: research
status: active
title: "Usage statistics — baseline (call mix, sessions, output size, errors, builds, recording)"
tags:
  - usage-stats
  - frictions
---

Archived analyst report from the 2026-11-11 statistics pass. The summary and the decisions are in [the statistics tracker](../trackers/frictions/statistics.md). The scripts named below are in `docs/research/data/2026-11-11-usage-stats-scripts.tar.gz`. They read pickles and a scratch directory that no longer exist, so treat them as method, not as a rerunnable job.

# codescout usage baseline (analyst: STATS-BASELINE)

Corpus: `.codescout/usage.db` table `tool_calls`, rows `id <= 299175` = 118,278 rows, 2026-10-12 08:06:52 to 2026-11-10 22:23:51 UTC (ids 180898..299175, contiguous: 299175-180898+1 = 118,278, so no id gaps from the sweep inside the snapshot). Read-only (`sqlite3` URI `mode=ro` via `python3 -I`). All scripts: `/home/marius/.cache/cc-tmp/claude-1000/-home-marius-work-claude-codescout/2e6121be-ee04-4e7a-974f-3717997500f4/scratchpad/base_*.py` (common helpers `base_common.py`; raw outputs `base_s*.out`; probe JSON `base_probe.json`).
Logs (`usage-dropped-rows.log`, `usage-lock-holds.log`) are NOT id-bounded: they are live files, read at analysis time and still growing; where bounded to the snapshot it says so.


## 9. MOST USEFUL FINDINGS (max 8; evidence in the sections named)

1. Usage is concentrated in a few shell-and-file tools; the semantic tools are almost unused. run_command 43.4%, read_file 16.7%, symbols 8.9%, grep 7.8%, edit_file 7.2%, doc 5.7% = 89.8% of 118,278 calls; semantic_search 14 calls (0.01%), symbol_at 7, index 21, call_graph 130, references 260. Confidence: high (section 1; 100% populated). The latest week (Nov 9-10) shows symbols 13.8% and doc 11.6% rising while grep fell 11.3% -> 4.5% (trend; cause is a HYPOTHESIS).
2. Half of all errors are the project's own gates firing, and they teach in one call. il3_pipe_to_trimmer 722 + il3_shell_on_source 498 + il1_read_overlaps_symbol 356 + il2_structural_edit 255 + librarian_managed_artifact 202 = 51.8% of errors; 96.0% of run_command's errors are the two il3 families. Overall: 3.32% of calls error, 98.9% of errors are recoverable_error, 78.4% carry a `— hint:`, next call after an error succeeds 87.1% (hint) / 93.3% (no hint, mostly run_command), immediate-repeat 7.3% (principal-keyed 7.0%), median calls-to-recovery 1, 0.1% unrecovered within 25 calls, 20.3% of errors sit in runs of 2+ (longest 10). Highest repeat rates by tool: read_file 14.1%, doc 10.8%, call_graph 1 of 4. Confidence: high (sections 4, 8). Do not read the family counts as friction (TU-7).
3. A few huge sessions dominate the corpus; subagents are a third of calls and an outsized share of errors. 464 sessions; p50 71 calls, p90 421, max 8,777; top 1% of sessions (4) hold 26.5% of calls, top 10% hold 69.4%. 78 sessions use subagents (603 agent_ids; max 55 in one session); subagents make 33.7% of calls, 43.3% of errors (error rate 4.26% vs 2.84% parent). Per-session statistics are therefore dominated by a handful of multi-day sessions (35 sessions span > 24 h). Confidence: high (section 2).
4. Only 61.7% of calls come from the main checkout. 16.5% claude agent worktrees (98.8% subagent), 10.4% repo worktrees, 6.5% codex worktrees, 4.0% /tmp harness, 0.9% other; roots matching eval/probe/codex patterns (setup-ab, /tmp/*, ~/.codex/worktrees) are 12.4% of calls (14,642). Tool mix differs by root (codex: symbols 23.6% vs 7.3% on main). Confidence: high for counts; the class rule is mine (section 0). Aggregating without separating roots mixes interactive work, subagent fleets and harness runs.
5. "overflowed" overstates output overflow and understates read_file buffering. 7,210 overflowed rows, of which 3,247 (45%) are `run_command(run_in_background)` "Started" envelopes; real `@cmd_` buffers 2,050, and 1,913 `@tool_` buffers (symbols 842, doc 484, grep 473, librarian 96). read_file buffers via `@file_` outlines that no column records (20.1% of read_file rows with output mention one). Read-back (handle named by a later call in the same session): `@cmd` 36.1%, doc 63.9%, librarian 80.0%, symbols 20.3%, grep 8.9% (named, not proven resolved). librarian overflows on 33.7% of calls with median 21,423 tokens. Confidence: high for counts, medium for the read-back reading (section 3, 0).
6. Context cost is led by run_command (34.7%), read_file (31.0%) and grep (13.4%) of 152.4 MB of stored output (63.5% of rows; subagent-heavy). get_guide is the biggest single output class by call (p50 11.9 KB, p99 71 KB, 15 largest outputs are all get_guide tracker-conventions at 67-71 KB) but only 3.0% of bytes. Confidence: medium (coverage gap; byte counts include JSON escaping) (section 3).
7. The usage recorder itself is under write-lock pressure since Nov 9. The retention sweep runs a DELETE after every insert; in the hold log 860 sweeps held the writer a median 2.65 s (max 20.0 s) and almost all deleted 0 rows; ~1.2% of calls in the 33 h after the first drop line left no row (143 `database is locked` drops by snapshot end, wait_ms p50 5,008). Confidence: high for the counts, the cause of sweep slowness NOT verified (section 7).
8. Coverage and build identity limits for every other statistic. output_json exists on 44.9% of parent rows (100% of subagent rows), input_json empty on 19.0% (last 2026-11-04), tool_use_id exactly mirrors output_json presence. 64.5% of calls were served by a `dirty` build; 141 shas, top 10 = 48.4% of calls; build age at call (commit date, upper bound) p50 0.61 d, p90 3.28 d, 33.8% older than 1 d, 0.1% older than 7 d. Confidence: high (sections 6, 7).

## NOT VERIFIED

- Why Oct 25 has zero rows (47 h gap); why the retention DELETE is slow (index/scan plan not inspected); whether 2026-10-12..14 empty-input/empty-output days reflect debug mode being off.
- That a handle "named later" was actually resolved by the buffer resolver (the column records mentions only); read-back percentages are upper bounds on retrieval.
- Token cost: sizes are stored-JSON bytes, not tokens; overflow_tokens exists only for non-run_command overflows.
- Whether deliveries (guide sections, session-opener) changed agent behaviour: only counts and a confounded next-call comparison were computed; no causal claim.
- Latency, and the `grep` tool internals: out of slice.
- Drop-rate estimate (~1.2%) assumes drops belong to the same DB as the stored rows; the logs are live and were read at different moments (182, 186, 193 lines).
- Whether `agent_id` NULL always means the parent conversation (pre-agent-tracking rows have started_at set on every row of the snapshot, so none of the snapshot's rows predate agent tracking by that test).

## 0. Definitions and coverage (read first)

- error = `outcome <> 'success'` = `recoverable_error` (3,883) + `error` (45) = 3,928 (friction-probe on the live DB at run time said 3,930: rows added after the snapshot). Source: `classify_content_result` (src/usage/mod.rs:429): a `RecoverableError` is `recoverable_error`; any other `Err` is `error`.
- `overflowed` = the response's first text block parses as JSON with an `output_id` key (src/usage/mod.rs:429-470). So it is TRUE for (a) real buffered output envelopes (`@tool_*`, `@cmd_*`) AND (b) `run_command(run_in_background=true)` "Started" envelopes (`@bg_*`). Of 7,210 overflowed rows, 3,247 are `@bg_` job handles (all `run_command`; 2,564 of them carry `run_in_background` in input_json, 682 have empty input_json, 1 is an `@ack_` replay). Do not read run_command's 10.3% overflow rate as output volume.
- `emitted_output_id` is set only when overflowed (mod.rs:323). `read_output_ids` = buffer handles NAMED in a call's arguments (mod.rs: extract_read_output_ids doc comment) - "was this handle named later", NOT "was this result retrieved". A `run_command("tail @bg_x")` poll counts as a "read" of a bg job.
- `output_json` is recorded ONLY in debug mode (mod.rs: "output_json below stays debug-only"); that is why parent rows have it on 44.9% (35,164/78,381) and subagent rows on 100% (39,897/39,897). `tool_use_id` is present on exactly the same 75,061 rows (identical count; both coincide with output_json presence). `input_json` is recorded always since 2026-11-02 (bug 20d94ebc7deec094); empty on 22,463 rows (19.0%), last empty row 2026-11-04 16:17:53.
- `output_json` holds what the caller received inline (for an overflowed call, the small envelope, not the buffered body). Byte sizes below are `length(CAST(output_json AS BLOB))` of the stored JSON-wrapped text (includes escaping), not tokens.
- session = `session_id` (per MCP server process). principal = (session_id, agent_id); agent_id NULL = parent/main conversation.
- project_root class rule (script `base_common.classify_root`, first match wins): `main_checkout` = exactly `/home/marius/work/claude/codescout`; `codex_worktree` = contains `/.codex/worktrees/`; `claude_agent_worktree` = contains `/.claude/worktrees/` (Claude Code subagent worktrees, `agent-<hash>`); `repo_worktree` = under `/codescout/.worktrees/` or `/codescout.worktrees/`; `tmp_harness` = starts with `/tmp/`; `other` = everything else (38 roots, all review/test scratch worktrees under `~/.cache/...`, `~/work/claude/gate-*`). Worktree calls are written into THIS db by design (src/usage/mod.rs: `worktree_main_root`), tagged by their own root.
- "harness" rows (eval/probe/other-tool): roots containing `setup-ab` (2,243), starting `/tmp/` (4,741; 9 roots, 8 are `/tmp/codescout-*` or `/tmp/native-*`), or under `~/.codex/worktrees` (7,658) = 14,642 rows = 12.4% of calls. Codex is a different harness (Codex threads), not an eval, but is not Claude Code usage.

## 1. CALL MIX (script base_s1.py)

Raw output below. Coverage: calls/outcome/overflow/agent_id/project_root are 100% populated (no NULLs), so section 1 is not affected by the input/output gaps.

```
# S1 call mix, N= 118278
outcome values Counter({'success': 114350, 'recoverable_error': 3883, 'error': 45})
overflowed values Counter({0: 111068, 1: 7210})
| tool | n | share% | success | recov_err | hard_err | err% | recov% of calls | hard% | overflow n | overflow% |
|---|---|---|---|---|---|---|---|---|---|---|
| run_command | 51365 | 43.43 | 50094 | 1270 | 1 | 2.47 | 2.47 | 0.00 | 5297 | 10.31 |
| read_file | 19730 | 16.68 | 18705 | 1023 | 2 | 5.20 | 5.18 | 0.01 | 0 | 0.00 |
| symbols | 10569 | 8.94 | 10499 | 67 | 3 | 0.66 | 0.63 | 0.03 | 842 | 7.97 |
| grep | 9227 | 7.80 | 9163 | 64 | 0 | 0.69 | 0.69 | 0.00 | 473 | 5.13 |
| edit_file | 8485 | 7.17 | 7820 | 663 | 2 | 7.84 | 7.81 | 0.02 | 0 | 0.00 |
| doc | 6786 | 5.74 | 6408 | 367 | 11 | 5.57 | 5.41 | 0.16 | 484 | 7.13 |
| edit_code | 4492 | 3.80 | 4241 | 235 | 16 | 5.59 | 5.23 | 0.36 | 2 | 0.04 |
| create_file | 3833 | 3.24 | 3731 | 96 | 6 | 2.66 | 2.50 | 0.16 | 0 | 0.00 |
| memory | 1491 | 1.26 | 1416 | 75 | 0 | 5.03 | 5.03 | 0.00 | 0 | 0.00 |
| workspace | 889 | 0.75 | 886 | 3 | 0 | 0.34 | 0.34 | 0.00 | 10 | 1.12 |
| get_guide | 452 | 0.38 | 450 | 2 | 0 | 0.44 | 0.44 | 0.00 | 0 | 0.00 |
| librarian | 285 | 0.24 | 280 | 5 | 0 | 1.75 | 1.75 | 0.00 | 96 | 33.68 |
| references | 260 | 0.22 | 248 | 11 | 1 | 4.62 | 4.23 | 0.38 | 3 | 1.15 |
| tree | 173 | 0.15 | 173 | 0 | 0 | 0.00 | 0.00 | 0.00 | 3 | 1.73 |
| call_graph | 130 | 0.11 | 126 | 1 | 3 | 3.08 | 0.77 | 2.31 | 0 | 0.00 |
| approve_write | 69 | 0.06 | 68 | 1 | 0 | 1.45 | 1.45 | 0.00 | 0 | 0.00 |
| index | 21 | 0.02 | 21 | 0 | 0 | 0.00 | 0.00 | 0.00 | 0 | 0.00 |
| semantic_search | 14 | 0.01 | 14 | 0 | 0 | 0.00 | 0.00 | 0.00 | 0 | 0.00 |
| symbol_at | 7 | 0.01 | 7 | 0 | 0 | 0.00 | 0.00 | 0.00 | 0 | 0.00 |
TOTAL {'success': 114350, 'recoverable_error': 3883, 'error': 45} err% 3.3209895331338033 overflow 7210

## weekly share% top8 (week start Monday UTC); first/last weeks partial
| week | calls | run_command | read_file | symbols | grep | edit_file | doc | edit_code | create_file | other |
|---|---|---|---|---|---|---|---|---|---|---|
| 2026-10-12 | 26923 | 41.7 | 17.6 | 7.5 | 11.3 | 9.4 | 2.7 | 4.6 | 2.7 | 2.5 |
| 2026-10-19 | 11325 | 44.7 | 15.6 | 7.2 | 10.8 | 9.8 | 2.9 | 3.7 | 2.5 | 2.7 |
| 2026-10-26 | 30922 | 47.1 | 15.9 | 8.4 | 6.8 | 7.8 | 4.0 | 3.5 | 3.9 | 2.7 |
| 2026-11-02 | 32176 | 43.4 | 18.0 | 8.7 | 6.6 | 5.1 | 7.9 | 3.3 | 3.0 | 4.0 |
| 2026-11-09 | 16932 | 38.8 | 14.8 | 13.8 | 4.5 | 4.8 | 11.6 | 4.1 | 3.6 | 4.1 |

## parent vs subagent
parent 78381 66.3% err 2229 2.84% ov 4923
subagent 39897 33.7% err 1699 4.26% ov 2287
| tool | parent n | parent share% | subagent n | subagent share% | subagent fraction of tool% |
|---|---|---|---|---|---|
| run_command | 35034 | 44.7 | 16331 | 40.9 | 31.8 |
| read_file | 11877 | 15.2 | 7853 | 19.7 | 39.8 |
| symbols | 8343 | 10.6 | 2226 | 5.6 | 21.1 |
| grep | 3954 | 5.0 | 5273 | 13.2 | 57.1 |
| edit_file | 4275 | 5.5 | 4210 | 10.6 | 49.6 |
| doc | 6328 | 8.1 | 458 | 1.1 | 6.7 |
| edit_code | 2674 | 3.4 | 1818 | 4.6 | 40.5 |
| create_file | 2803 | 3.6 | 1030 | 2.6 | 26.9 |
| memory | 1343 | 1.7 | 148 | 0.4 | 9.9 |
| workspace | 728 | 0.9 | 161 | 0.4 | 18.1 |
| get_guide | 273 | 0.3 | 179 | 0.4 | 39.6 |
| librarian | 274 | 0.3 | 11 | 0.0 | 3.9 |
| references | 177 | 0.2 | 83 | 0.2 | 31.9 |
| tree | 83 | 0.1 | 90 | 0.2 | 52.0 |
| call_graph | 126 | 0.2 | 4 | 0.0 | 3.1 |
| approve_write | 57 | 0.1 | 12 | 0.0 | 17.4 |
| index | 20 | 0.0 | 1 | 0.0 | 4.8 |
| semantic_search | 7 | 0.0 | 7 | 0.0 | 50.0 |
| symbol_at | 5 | 0.0 | 2 | 0.0 | 28.6 |

## roots
| class | calls | share% | errors | err% | subagent share% |
|---|---|---|---|---|---|
| main_checkout | 73021 | 61.7 | 2360 | 3.23 | 17.4 |
| claude_agent_worktree | 19484 | 16.5 | 898 | 4.61 | 98.8 |
| repo_worktree | 12325 | 10.4 | 407 | 3.30 | 56.9 |
| codex_worktree | 7658 | 6.5 | 119 | 1.55 | 0.1 |
| tmp_harness | 4741 | 4.0 | 113 | 2.38 | 0.0 |
| other | 1049 | 0.9 | 31 | 2.96 | 87.7 |
harness-ish calls Counter({'codex worktree': 7658, '/tmp/*': 4741, 'setup-ab': 2243}) 14642

## top tools per root class (share% within class)
main_checkout run_command 45.9, read_file 16.9, doc 7.6, symbols 7.3, grep 7.2, edit_file 5.1
tmp_harness run_command 43.6, symbols 16.7, read_file 16.0, edit_file 6.3, create_file 5.2, doc 4.7
repo_worktree run_command 37.8, read_file 15.6, edit_file 12.2, symbols 11.7, grep 9.8, edit_code 7.3
other run_command 30.6, grep 22.4, read_file 19.8, edit_file 9.4, symbols 9.3, create_file 4.9
claude_agent_worktree run_command 40.0, read_file 18.0, edit_file 13.8, grep 12.6, edit_code 6.1, symbols 5.5
codex_worktree run_command 39.4, symbols 23.6, read_file 13.3, doc 9.2, edit_code 6.2, create_file 3.0
```

Notes:
- `read_file` shows 0 overflow rows, but it DOES buffer: its oversized-file answer is an outline text plus `Buffer: @file_xxxx` (not a JSON `output_id` envelope), so `overflowed` and `emitted_output_id` never record it. 2,223 of the 11,043 read_file rows that have output_json (20.1%) contain an `@file_` handle (query: `output_json like '%@file_%'`); 1,282 later read_file calls name an `@file_` path. Blind spot of the overflow column, not absence of overflow.
- Hard `error` is 45 rows (1.1% of errors). 3,883 / 3,928 = 98.9% of errors are `recoverable_error`.
- Weekly table: weeks start Monday UTC; the 11-09 week has only 2 days (Mon, Tue). The 10-19 week has a 47 h hole (Oct 25, section 5).
- Trend (weekly share, first week to last week): doc 2.7 -> 11.6%, symbols 7.5 -> 13.8%, grep 11.3 -> 4.5%, edit_file 9.4 -> 4.8%, run_command 41.7 -> 38.8%. HYPOTHESIS (not tested): doc growth tracks tracker/librarian-heavy work; grep/edit_file share in mid October is lifted by the Oct 16-19 subagent-fleet sessions (grep is 57.1% subagent, edit_file 49.6%).


## 2. SESSIONS AND AGENTS (scripts base_s2.py, base_s9.py)

Coverage: session_id, agent_id, cc_session_id fully populated.

```
session_id null 0 cc_session_id null 0
agent_id set 39897 started_at null 0
session_id: count=464 p50=71.0 p90=420.79999999999995 p99=4085.560000000005 max=8777 mean=254.9; singletons=20; <=10 calls=86
principal (session_id,agent_id) all: count=1070 p50=49.0 p90=191.30000000000007 p99=907.2199999999966 max=8400 mean=110.5; singletons=45; <=10 calls=215
parent principals (agent_id NULL): count=456 p50=68.0 p90=262.5 p99=2142.599999999996 max=8400 mean=171.9; singletons=19; <=10 calls=87
subagent principals: count=614 p50=36.5 p90=140.0 p99=501.65000000000043 max=1038 mean=65.0; singletons=26; <=10 calls=128
distinct agent_ids 603
subagents per session hist (top) [(0, 386), (1, 31), (2, 6), (3, 6), (4, 6), (5, 2), (6, 3), (7, 3), (8, 2), (9, 2), (10, 2), (13, 1), (16, 1), (17, 1), (20, 1)] max 55
sessions with >=1 subagent 78
subagent calls 39897 33.7% errors 1699 all errors 3928 43.3%
distinct cc_session_id 710 cc with >1 session_id 72
session duration minutes: p50 32.3 p90 1056.1 p99 5456.8 max 8195.8 ; >24h: 35 ; >=1h: 202
subagent principal span min: p50 6.7 p90 70.6 max 1858.0

| day | sessions started | sessions active | principals active |
| 2026-10-12 | 3 | 3 | 13 |
| 2026-10-13 | 8 | 9 | 22 |
| 2026-10-14 | 11 | 16 | 20 |
| 2026-10-15 | 2 | 7 | 7 |
| 2026-10-16 | 12 | 18 | 54 |
| 2026-10-17 | 3 | 7 | 70 |
| 2026-10-18 | 4 | 8 | 33 |
| 2026-10-19 | 2 | 5 | 24 |
| 2026-10-20 | 4 | 6 | 19 |
| 2026-10-21 | 5 | 7 | 25 |
| 2026-10-22 | 2 | 4 | 7 |
| 2026-10-23 | 10 | 13 | 13 |
| 2026-10-24 | 15 | 16 | 53 |
| 2026-10-26 | 4 | 6 | 9 |
| 2026-10-27 | 12 | 15 | 25 |
| 2026-10-28 | 12 | 14 | 31 |
| 2026-10-29 | 37 | 41 | 74 |
| 2026-10-30 | 41 | 45 | 68 |
| 2026-10-31 | 35 | 39 | 95 |
| 2026-11-01 | 26 | 32 | 53 |
| 2026-11-02 | 18 | 22 | 29 |
| 2026-11-03 | 13 | 17 | 26 |
| 2026-11-04 | 28 | 30 | 79 |
| 2026-11-05 | 13 | 19 | 45 |
| 2026-11-06 | 10 | 15 | 15 |
| 2026-11-07 | 26 | 31 | 104 |
| 2026-11-08 | 16 | 19 | 34 |
| 2026-11-09 | 70 | 76 | 100 |
| 2026-11-10 | 22 | 27 | 41 |

## sessions by dominant root class (sessions, calls)
main_checkout 387 70961
tmp_harness 17 6981
repo_worktree 26 12081
claude_agent_worktree 8 21435
codex_worktree 26 6820
sessions touching >1 root class 142
sessions harness (setup-ab or /tmp/* or ~/.codex/worktrees): Counter({'none': 382, 'some': 79, 'all-harness': 3})
harness calls 14642 12.4%

## top 10 sessions by calls
500c9b13-6cc 8777 2026-10-17 05:57 2026-10-19 14:16 claude_agent_worktree subagents 50
0e844aa1-d9e 8400 2026-10-30 06:53 2026-11-04 16:17 main_checkout subagents 0
e98b5705-06e 8399 2026-10-16 14:36 2026-10-18 11:16 claude_agent_worktree subagents 55
3d9ab02a-7c6 5794 2026-11-04 17:21 2026-11-07 09:45 main_checkout subagents 0
cb06fa74-029 4771 2026-10-24 03:09 2026-10-29 19:45 tmp_harness subagents 0
ac7fb73b-fd1 3683 2026-11-09 18:48 2026-11-10 22:23 codex_worktree subagents 0
879d21b0-831 2345 2026-11-09 08:25 2026-11-10 18:08 main_checkout subagents 0
8f842cf1-bf7 2020 2026-11-04 08:23 2026-11-09 21:44 main_checkout subagents 26
5d96a3b1-8b6 1977 2026-10-14 06:25 2026-10-16 14:44 repo_worktree subagents 0
2f5f98b2-efe 1808 2026-10-16 04:01 2026-10-21 03:51 main_checkout subagents 23

## top 10 principals by calls
0e844aa1-d9e  8400 133 main_checkout
3d9ab02a-7c6  5794 134 main_checkout
cb06fa74-029  4771 100 main_checkout
ac7fb73b-fd1  3683 82 main_checkout
879d21b0-831  2345 42 main_checkout
5d96a3b1-8b6  1977 48 main_checkout
06e2ecbc-cca  1802 37 main_checkout
2f5f98b2-efe  1331 35 main_checkout
7b6cc957-6a6  1142 24 main_checkout
e98b5705-06e a66600865e06 1038 46 claude_agent_worktree
top 1% sessions (4) hold 26.5% calls
top 5% sessions (23) hold 55.9% calls
top 10% sessions (46) hold 69.4% calls
top 20% sessions (92) hold 80.5% calls
```

Additional (base_s9.py): harness sessions by category (a session counts once per category it touches): `/tmp/codescout-*` 27 sessions (4,727 calls), `~/.codex/worktrees` 47 sessions (7,658 calls), `setup-ab` 9 sessions (2,243 calls), other `/tmp/*` 3 sessions (14 calls). Sessions with >= 50% of their calls in harness roots: 15 `/tmp/codescout-*` (6,968 calls), 25 codex (3,137), 3 setup-ab (2,703), 2 other (13). Many sessions are mixed (the "sessions harness" line in the output above: 79 with some harness calls, 3 all-harness): a session's `project_root` changes with `workspace(activate)`/pins, e.g. session cb06fa74 (4,771 calls) spans `/tmp/codescout-real-task-20261024` (3,572 calls) and a codex worktree. `cc_session_id` and `session_id` disagree: 72 of 710 cc_session_ids map to >1 session_id.

Reading the table: "sessions started" uses the first call inside the window, so sessions that began before 2026-10-12 08:06 count as started on 10-12. The brief's trap holds: the largest session (500c9b13) has 50 and e98b5705 has 55 distinct agent_ids interleaving inside one session_id.


## 3. OUTPUT SIZE = CONTEXT COST (scripts base_s3.py, base_s3b.py, base_s3c.py)

Coverage: output_json exists on 75,061 of 118,278 rows (63.5%): parent 35,164 of 78,381 (44.9%), subagent 39,897 of 39,897. Per-tool coverage is in the table. Tools with low coverage (symbols 34%, doc 37%, memory 23%, call_graph 3%) are the ones parents call most, so their size statistics describe mostly subagent calls. overflow_tokens/overflowed/emitted_output_id are populated for all rows (not debug-gated).

```
rows with output_json 75061 63.5% parent 35164 / 78381 subagent 39897 / 39897
output_json null vs empty: (43217, 0, 0, 0)
total output bytes 152386653
| tool | rows w/ output | cov% of tool rows | p50 | p90 | p99 | max | total bytes | share% |
|---|---|---|---|---|---|---|---|---|
| run_command | 35202 | 69 | 641 | 3881 | 12035 | 21226 | 52852770 | 34.7 |
| read_file | 11043 | 56 | 3362 | 9624 | 9980 | 14268 | 47165849 | 31.0 |
| grep | 8915 | 97 | 1485 | 5476 | 10329 | 20320 | 20401928 | 13.4 |
| symbols | 3572 | 34 | 2634 | 6989 | 11115 | 18719 | 11451836 | 7.5 |
| doc | 2485 | 37 | 885 | 8622 | 16197 | 19360 | 7202619 | 4.7 |
| workspace | 522 | 59 | 6554 | 18685 | 25765 | 27275 | 4510599 | 3.0 |
| get_guide | 201 | 44 | 11854 | 67309 | 70981 | 71403 | 4496081 | 3.0 |
| edit_file | 7440 | 88 | 146 | 286 | 945 | 3877 | 1385020 | 0.9 |
| edit_code | 2602 | 58 | 218 | 386 | 1672 | 18961 | 754741 | 0.5 |
| create_file | 2216 | 58 | 215 | 400 | 1810 | 4953 | 656252 | 0.4 |
| memory | 341 | 23 | 352 | 6175 | 8752 | 10904 | 602906 | 0.4 |
| librarian | 155 | 54 | 1239 | 6638 | 15638 | 18066 | 464137 | 0.3 |
| references | 175 | 67 | 640 | 3404 | 4998 | 7252 | 199228 | 0.1 |
| tree | 126 | 73 | 314 | 2966 | 8772 | 9707 | 142929 | 0.1 |
| semantic_search | 13 | 93 | 5261 | 9751 | 10664 | 10713 | 61343 | 0.0 |
| index | 15 | 71 | 1185 | 4294 | 5497 | 5497 | 26152 | 0.0 |
| approve_write | 28 | 41 | 143 | 219 | 684 | 852 | 5159 | 0.0 |
| call_graph | 4 | 3 | 569 | 2358 | 3003 | 3075 | 4400 | 0.0 |
| symbol_at | 6 | 86 | 394 | 614 | 731 | 744 | 2704 | 0.0 |

## largest 15 outputs
253980 get_guide 71403 agent success '{"topic":"tracker-conventions"}'
278738 get_guide 70981 agent success '{"topic":"tracker-conventions"}'
278739 get_guide 70981 agent success '{"topic":"tracker-conventions"}'
278743 get_guide 70981 agent success '{"topic":"tracker-conventions"}'
278754 get_guide 70981 agent success '{"topic":"tracker-conventions"}'
278955 get_guide 70981 agent success '{"topic":"tracker-conventions"}'
278956 get_guide 70981 agent success '{"topic":"tracker-conventions"}'
278957 get_guide 70981 agent success '{"topic":"tracker-conventions"}'
253624 get_guide 70647 agent success '{"topic":"tracker-conventions"}'
253682 get_guide 70647 agent success '{"topic":"tracker-conventions"}'
233772 get_guide 70640 agent success '{"topic":"tracker-conventions"}'
250578 get_guide 67751 parent success '{"topic":"tracker-conventions"}'
267912 get_guide 67329 parent success '{"topic":"tracker-conventions"}'
278745 get_guide 67329 agent success '{"topic":"tracker-conventions"}'
278749 get_guide 67329 agent success '{"topic":"tracker-conventions"}'

## overflow_tokens
rows with overflow_tokens>0 1913 sum 13362884 overflowed=1 rows 7210 overflowed=1 & tokens null/0 5297 tokens>0 & overflowed=0 0
| tool | overflow rows | sum tokens | p50 | p90 | max | overflow rate% of tool calls |
|---|---|---|---|---|---|---|
| doc | 484 | 4459744 | 4898 | 28503 | 73666 | 7.1 |
| symbols | 842 | 3537617 | 3510 | 6002 | 37016 | 8.0 |
| librarian | 96 | 2973195 | 21423 | 68986 | 79863 | 33.7 |
| grep | 473 | 2317063 | 3352 | 6747 | 235205 | 5.1 |
| workspace | 10 | 39887 | 4020 | 4745 | 4753 | 1.1 |
| edit_code | 2 | 16712 | 8356 | 11198 | 11909 | 0.0 |
| tree | 3 | 10432 | 3284 | 4271 | 4518 | 1.7 |
| references | 3 | 8234 | 2828 | 2828 | 2828 | 1.2 |

## buffers
rows with emitted_output_id 7210 rows with read_output_ids 9583
emitted by tool [('run_command', 5297), ('symbols', 842), ('doc', 484), ('grep', 473), ('librarian', 96), ('workspace', 10), ('references', 3), ('tree', 3), ('edit_code', 2)]
read-back by tool [('run_command', 5362), ('read_file', 3232), ('grep', 265), ('create_file', 194), ('doc', 189), ('memory', 148), ('edit_file', 145), ('edit_code', 48)]
sample emitted ['@bg_0000036b', '@bg_00000370', '@bg_00000055', '@bg_0000005a', '@tool_2f49e611'] sample read ['["@cmd_2fa3e688"]', '["@cmd_2fa4745a"]', '["@cmd_2fa4745a"]', '["@cmd_2fa53199"]', '["@cmd_2fa5ef79"]']
emitted prefixes Counter({'@bg_0': 3247, '@cmd_': 2050, '@tool': 1913})
ov rows with emitted id 1913 emitted rows overflowed 7210
distinct emitted 4678 distinct read ids 3225 emitted ids read>=1 1895 read ids not in emitted set 1330
```

Overflowed rows with output (inline envelope size versus what sits behind the handle), base_s3c.py. `buffered_bytes` parsed on 1,121 rows only (the symbols/grep/doc/librarian envelopes carry it; the run_command envelope does not). Also: non-overflowed outputs over 10,000 chars exist (run_command 196, doc 142, workspace 120, get_guide 113, read_file 103, symbols 40): not everything large is buffered.

```
overflowed rows with output 4504
buffered_bytes parsed 1121
| tool (overflowed rows with output) | n | inline bytes p50 | max | buffered_bytes p50 | p90 | max | sum buffered |
| run_command | 3383 | 317 | 21226 | - | - | - | - |
| grep | 456 | 3458 | 20320 | 13393.5 | 26944.5 | 940823 | 8497005 |
| symbols | 433 | 3444 | 18719 | 14641.0 | 25177.800000000003 | 148067 | 7645772 |
| doc | 163 | 4453 | 17027 | 19252.0 | 79338.40000000001 | 188869 | 6058338 |
| librarian | 59 | 1536 | 15638 | 102215.0 | 287703.0 | 316707 | 8766750 |
| workspace | 6 | 4954 | 4979 | 18321.5 | 18995.0 | 19014 | 103065 |
| edit_code | 2 | 647 | 683 | 33426.5 | 44795.7 | 47638 | 66853 |
| references | 2 | 3482 | 3482 | 11315.0 | 11315.0 | 11315 | 22630 |
['[{"type":"text","text":"{\\n  \\"output_id\\": \\"@bg_0000036b\\",\\n  \\"hint\\": \\"Started; outcome not yet observed. Read output with run_command(\\\\\\"tail -50 @bg_0000036b\\\\\\") — that response carries this job\'s state.\\"\\n}"}]', '[{"type":"text","text":"{\\n  \\"output_id\\": \\"@bg_00000370\\",\\n  \\"hint\\": \\"Started; outcome not yet observed. Read output with run_command(\\\\\\"tail -50 @bg_00000370\\\\\\") — that response carries this job\'s state.\\"\\n}"}]']
non-overflow outputs >10000 chars by tool: Counter({'run_command': 196, 'doc': 142, 'workspace': 120, 'get_guide': 113, 'read_file': 103, 'symbols': 40, 'grep': 11, 'librarian': 2, 'semantic_search': 2, 'edit_code': 1, 'memory': 1})
bg rows with output 1943 p50 bytes 315.0
```

Buffer read-back (base_s3b.py). "read back" = a LATER call in the same session whose arguments name that handle (named, not necessarily resolved). Handles are keyed by (session_id, handle) because `@bg_*` counters restart per server process (4,678 distinct handle strings vs 7,130 (session,handle) pairs). The "next call" table looks at the next call of the same principal after the emitting call. For `bg` rows "reads the buffer" mostly means polling the job with `tail @bg_x`.

```
emitted (session,handle) keys 7130 rows 7210
| emitter tool | handle kind | buffers | read back >=1 | % | total reads | read by same principal |
|---|---|---|---|---|---|---|
| run_command | @bg_ | 3247 | 2196 | 67.6 | 5056 | 2194 |
| run_command | @cmd | 2050 | 740 | 36.1 | 1559 | 740 |
| symbols | @tool | 788 | 160 | 20.3 | 273 | 160 |
| grep | @tool | 472 | 42 | 8.9 | 59 | 42 |
| doc | @tool | 465 | 297 | 63.9 | 551 | 297 |
| librarian | @tool | 90 | 72 | 80.0 | 187 | 72 |
| workspace | @tool | 10 | 1 | 10.0 | 1 | 1 |
| references | @tool | 3 | 1 | 33.3 | 2 | 1 |
| tree | @tool | 3 | 0 | 0.0 | 0 | 0 |
| edit_code | @tool | 2 | 0 | 0.0 | 0 | 0 |

read-back calls by (tool,handle kind) [(('run_command', '@bg'), 4406), (('read_file', '@file'), 1282), (('read_file', '@cmd'), 1080), (('read_file', '@tool'), 842), (('run_command', '@cmd'), 535), (('memory', '@bg'), 357), (('run_command', '@ack'), 246), (('doc', '@bg'), 242), (('run_command', '@tool'), 213), (('edit_file', '@bg'), 122), (('grep', '@cmd'), 114), (('create_file', '@ack'), 108), (('grep', '@bg'), 94), (('create_file', '@bg'), 69), (('create_file', '@cmd'), 68), (('run_command', '@file'), 48), (('edit_code', '@bg'), 45), (('grep', '@tool'), 44), (('edit_file', '@ack'), 40), (('create_file', '@tool'), 36)]
read (session,handle) keys 4957 not emitted in-snapshot by same session 1447 Counter({'@file': 700, '@ack': 378, '@cmd': 193, '@bg': 125, '@tool': 51})

| emitter | kind | n | next call reads the buffer | buffer read within next 5 | next identical re-run | next same tool diff input | next other tool (no read) | last call of principal |
|---|---|---|---|---|---|---|---|---|
| run_command | bg | 3247 | 935 | 1703 | 291 | 971 | 1026 | 24 |
| run_command | overflow | 2050 | 555 | 711 | 143 | 774 | 576 | 2 |
| symbols | overflow | 842 | 58 | 143 | 100 | 160 | 520 | 4 |
| doc | overflow | 484 | 113 | 245 | 22 | 57 | 290 | 2 |
| grep | overflow | 473 | 30 | 40 | 0 | 181 | 261 | 1 |
| librarian | overflow | 96 | 45 | 63 | 4 | 1 | 41 | 5 |
| workspace | overflow | 10 | 1 | 1 | 0 | 1 | 8 | 0 |
| references | overflow | 3 | 0 | 0 | 0 | 0 | 3 | 0 |
| tree | overflow | 3 | 0 | 0 | 0 | 0 | 3 | 0 |
| edit_code | overflow | 2 | 0 | 0 | 0 | 1 | 1 | 0 |

sample read_file reading buffers inputs:
180996 ["@cmd_2fa3e688"] {"end_line":1520,"path":"@cmd_2fa3e688","start_line":1303}
180998 ["@cmd_2fa4745a"] {"end_line":60,"force":true,"path":"@cmd_2fa4745a","start_line":12}
180999 ["@cmd_2fa4745a"] {"end_line":560,"force":true,"path":"@cmd_2fa4745a","start_line":360}
181015 ["@cmd_2fa5ef79"] {"path":"@cmd_2fa5ef79","start_line":1,"end_line":280}
```

Notes: `overflow_tokens` is populated only for the 1,913 non-run_command overflow rows (all 5,297 run_command overflow rows have NULL/0 tokens: 3,247 bg handles + 2,050 `@cmd_` buffers). 13,362,884 tokens summed. Handles read but never emitted in-snapshot by the same session (1,447 of 4,957 handle pairs: `@file` 700, `@ack` 378, `@cmd` 193, `@bg` 125, `@tool` 51) come from read_file buffers (not recorded as emitted), ack handles, handles from rows older than the snapshot, or handles merely quoted in prose.


## 4. ERRORS AND RETRY SHAPE (scripts base_s4.py, base_s4b.py; probe JSON base_probe.json)

Coverage: outcome/error_msg/err_family fully populated for error rows. friction-probe was run on the live DB (118,303 rows, 3,930 errors at run time; my snapshot has 3,928); my own re-computation on the snapshot matches it (286 immediate repeats; identical run histogram when keyed by session_id).
Per TU-7: counts below are NOT friction ranks; the largest families are mostly the project's own gates being obeyed. Retry shape is the signal.

```
bg check: (3247, 2564)
run_in_background in input anywhere (run_command): (2596, 2564)
errors 3928
outcome Counter({'recoverable_error': 3883, 'error': 45})
with '— hint:' 3081 78.4% recoverable with hint 3081 hard with hint 0
error_msg null/empty 0
other 'hint:' variant w/o em-dash form 1 ['IL3 violation — piped `python3 -I -c "\nimport sqlite3,json,re,collections\ncon=sqlite3.connect(\'file:']
distinct hint texts 1135 top share: hints appearing >=10 times: 47 singletons 918
hint texts normalised (digits->#, quotes content trimmed) distinct: 994
top 10 hint texts:
151 edit_code(symbol, path, action='insert', body=..., position=...) — inserts before or after a named symbol — or, to change only a modifier or
132 Call workspace(action='activate', path="/home/marius/work/claude/codescout") to write to the main repo, or pass workspace="<abs path>" on th
129 Check the path with tree, or use tree with `glob` to locate the file. If the root above is not the project you meant, a subagent sharing thi
126 use read_file(path, start_line, end_line), symbols(path), symbols(name=..., include_body=true), or grep(regex) instead. Re-run with acknowle
95 Check the path with tree, or use tree with `glob` to locate the file. If the root above is not the project you meant, a subagent sharing thi
67 Use tree to explore the directory structure, or symbols(path) to list symbols in a file or directory.
47 No exact or whitespace-normalized match. Copy the actual bytes shown (or from read_file) and retry.
46 Frontmatter on this file is catalog-indexed, so edit it through the catalog:
• doc(action="update", id="<id>", patch={status: "...", tags: [
45 Batch aborted — no changes written.
41 To rename: edit_code(action="rename", symbol=..., new_name=...) — then replace the body in a second call if it also changed. To replace only
| tool | errors | with hint | hint% | hard |
| run_command | 1271 | 548 | 43 | 1 |
| read_file | 1025 | 1023 | 100 | 2 |
| edit_file | 665 | 660 | 99 | 2 |
| doc | 378 | 298 | 79 | 11 |
| edit_code | 251 | 235 | 94 | 16 |
| create_file | 102 | 91 | 89 | 6 |
| memory | 75 | 75 | 100 | 0 |
| symbols | 70 | 67 | 96 | 3 |
| grep | 64 | 64 | 100 | 0 |
| references | 12 | 11 | 92 | 1 |
| librarian | 5 | 2 | 40 | 0 |
| call_graph | 4 | 1 | 25 | 3 |
| workspace | 3 | 3 | 100 | 0 |
| get_guide | 2 | 2 | 100 | 0 |
| approve_write | 1 | 1 | 100 | 0 |
errors w/o hint 847 by tool [('run_command', 723), ('doc', 80), ('edit_code', 16), ('create_file', 11), ('edit_file', 5), ('librarian', 3), ('call_graph', 3), ('symbols', 3)]
hard errors: [(('create_file', 'File writes are disabled: the active project is /home/marius/work/clau'), 4), (('edit_code', "write denied: '/home/marius/.cache/codescout-resident/codescout/eviden"), 4), (('call_graph', 'disk I/O error'), 3), (('symbols', 'unsupported language'), 3), (('edit_code', "write denied: '/home/marius/.cache/codescout-resident/codescout/semant"), 3), (('create_file', 'Failed to write /tmp/codescout-real-task-20261024/tools/reliable-agent'), 2), (('doc', 'unknown id `6e78851b29cdb20b`'), 2), (('edit_code', 'Disk quota exceeded (os error 122)'), 2), (('edit_code', "write denied: '/home/marius/work/claude/claude-plugins/effort-steering"), 1), (('edit_code', "write denied: '/home/marius/.codex/worktrees/3940/codescout/tools/reli"), 1), (('edit_file', 'No such file or directory (os error 2)'), 1), (('edit_code', 'Failed to canonicalize path for didOpen: "/home/marius/work/claude/cod'), 1)]

err_family distinct 46 NULL 171 4.4%
NULL by tool [('doc', 78), ('read_file', 26), ('edit_file', 18), ('run_command', 13), ('edit_code', 10), ('librarian', 4), ('create_file', 4), ('references', 4)] NULL by outcome Counter({'recoverable_error': 154, 'error': 17})
NULL top 60-char prefixes:
10 'heading, action, frontmatter and heading-addressed edits[] a'
9 'doc(action="update") patch must be a JSON object mapping fie'
8 'a bug needs exactly one `cluster/…` tag; this one has none —'
8 'a bug needs a `## Class fit` section with at least 40 letter'
7 'parameter `include_body` was removed; use `full: true` for t'
7 'leaf must have exactly one field, got 2 — hint: Each leaf ha'
4 'unsupported language'
3 'disk I/O error'
3 'json_path parameter is only supported for JSON files — hint:'
3 'unknown workspace action: post_compact — hint: Valid actions'
3 'LSP request timed out after 30s: textDocument/references — h'
2 'both start_line and end_line are required — hint: Provide bo'
2 'registration for `/home/marius/work/claude/codescout/.worktr'
2 'start_line/end_line are body-relative (frontmatter_lines: 13'
2 'edits could not be read as an array of edit objects: it is a'

| err_family | n | share% of errors | tools | hard n |
| il3_pipe_to_trimmer | 722 | 18.4 | run_command:722 | 0 |
| il3_shell_on_source | 498 | 12.7 | run_command:498 | 0 |
| il1_read_overlaps_symbol | 356 | 9.1 | read_file:356 | 0 |
| path_not_found | 310 | 7.9 | read_file:224, symbols:66, edit_file:7 | 2 |
| il2_structural_edit | 255 | 6.5 | edit_file:255 | 0 |
| librarian_managed_artifact | 202 | 5.1 | read_file:112, edit_file:90 | 0 |
| missing_required_param | 191 | 4.9 | doc:89, edit_file:56, edit_code:34 | 0 |
| None | 171 | 4.4 | doc:78, read_file:26, edit_file:18 | 17 |
| edit_stale_match | 163 | 4.1 | edit_file:147, doc:16 | 0 |
| read_file_overflow_threshold | 157 | 4.0 | read_file:157 | 0 |
| worktree_activate_required | 132 | 3.4 | edit_file:41, create_file:36, doc:34 | 0 |
| unknown_enum_value | 77 | 2.0 | doc:61, edit_file:10, memory:3 | 7 |
| memory_target_not_found | 68 | 1.7 | memory:68 | 0 |
| json_path_key_miss | 66 | 1.7 | read_file:66 | 0 |
| invalid_line_range | 52 | 1.3 | read_file:38, doc:14 | 0 |
| shrink_guard_blocked | 46 | 1.2 | edit_code:39, create_file:4, memory:3 | 0 |
| replace_would_rename | 41 | 1.0 | edit_code:41 | 0 |
| param_not_applied | 41 | 1.0 | grep:41 | 0 |
| target_already_exists | 36 | 0.9 | create_file:36 | 0 |
| replace_dropped_sibling | 30 | 0.8 | edit_code:30 | 0 |
| heading_not_found | 28 | 0.7 | read_file:17, doc:8, edit_file:3 | 0 |
| lsp_document_symbol_null | 26 | 0.7 | edit_code:22, references:4 | 0 |
| ambiguous_old_string | 25 | 0.6 | edit_file:21, doc:4 | 0 |
| buffer_ref_expired | 25 | 0.6 | run_command:11, grep:10, read_file:4 | 0 |
| write_scope_denied | 21 | 0.5 | edit_code:13, create_file:5, edit_file:3 | 14 |
| unanchored_archive_refused | 21 | 0.5 | doc:21 | 0 |
| cwd_escapes_root | 20 | 0.5 | run_command:20 | 0 |
| artifact_rooted_in_temp | 18 | 0.5 | doc:18 | 0 |
| ast_extent_fail | 17 | 0.4 | edit_code:17 | 0 |
| edit_would_break_syntax | 17 | 0.4 | edit_code:15, edit_file:2 | 0 |
| write_lock_unavailable | 14 | 0.4 | create_file:9, doc:3, edit_file:2 | 0 |
| invalid_param_shape | 13 | 0.3 | doc:13 | 0 |
| mutually_exclusive_params | 12 | 0.3 | edit_file:7, doc:5 | 0 |
| symbol_not_found | 10 | 0.3 | edit_code:9, references:1 | 0 |
| invalid_regex | 9 | 0.2 | grep:9 | 0 |
| json_path_unsupported | 7 | 0.2 | read_file:7 | 0 |
| json_path_wrong_buffer_kind | 7 | 0.2 | read_file:7 | 0 |
| extra_models_reserved_field | 5 | 0.1 | doc:5 | 0 |
| write_blocked_read_only | 5 | 0.1 | create_file:4, doc:1 | 5 |
| lsp_index_locked | 3 | 0.1 | references:2, edit_file:1 | 0 |
| ack_replay_file_changed | 3 | 0.1 | create_file:3 | 0 |
| ambiguous_heading | 2 | 0.1 | doc:1, edit_file:1 | 0 |
| destructive_replace_blocked | 2 | 0.1 | doc:2 | 0 |
| json_path_shape_mismatch | 1 | 0.0 | read_file:1 | 0 |
| symbols_pattern_is_regex | 1 | 0.0 | symbols:1 | 0 |
| ambiguous_name_path | 1 | 0.0 | edit_code:1 | 0 |
| ack_handle_refused | 1 | 0.0 | edit_file:1 | 0 |
```

Probe families (immediate repeat = the very next call in the same session is an error with the same signature; calls-to-recovery = calls up to and including the next success in the same session, cap 25, unrecovered excluded), then my own per-tool recomputation keyed two ways (session_id as the probe does; and (session_id, agent_id) principal, which is the more correct key because subagents interleave inside one session_id):

```
probe (live DB at run time, not id-bounded): calls 118303 errors 3930 err% 3.32 unclassified 171
probe immediate-repeat overall (families n>=5 only): 286 / 3930 7.3%
| err_family | errors | immediate repeats | rate | via msg proxy |
| json_path_wrong_buffer_kind | 7 | 3 | 0.429 | 0 |
| artifact_rooted_in_temp | 18 | 7 | 0.389 | 0 |
| write_scope_denied | 21 | 5 | 0.238 | 0 |
| unanchored_archive_refused | 21 | 5 | 0.238 | 0 |
| il1_read_overlaps_symbol | 357 | 78 | 0.218 | 0 |
| unknown_enum_value | 77 | 14 | 0.182 | 0 |
| path_not_found | 310 | 52 | 0.168 | 0 |
| mutually_exclusive_params | 12 | 2 | 0.167 | 0 |
| worktree_activate_required | 132 | 20 | 0.152 | 0 |
| write_lock_unavailable | 15 | 2 | 0.133 | 0 |
| lsp_document_symbol_null | 26 | 3 | 0.115 | 0 |
| librarian_managed_artifact | 202 | 18 | 0.089 | 0 |
| heading_not_found | 28 | 2 | 0.071 | 0 |
| missing_required_param | 191 | 12 | 0.063 | 0 |
| invalid_line_range | 52 | 3 | 0.058 | 0 |
| json_path_key_miss | 66 | 3 | 0.045 | 0 |
| shrink_guard_blocked | 46 | 2 | 0.043 | 0 |
| ambiguous_old_string | 25 | 1 | 0.04 | 0 |
| il2_structural_edit | 255 | 9 | 0.035 | 0 |
| replace_dropped_sibling | 30 | 1 | 0.033 | 0 |
| <unclassified> | 171 | 5 | 0.029 | 171 |
| target_already_exists | 36 | 1 | 0.028 | 0 |
| edit_stale_match | 163 | 4 | 0.025 | 0 |
| read_file_overflow_threshold | 157 | 4 | 0.025 | 0 |
| il3_shell_on_source | 498 | 12 | 0.024 | 0 |
| replace_would_rename | 41 | 1 | 0.024 | 0 |
| param_not_applied | 41 | 1 | 0.024 | 0 |
| il3_pipe_to_trimmer | 722 | 15 | 0.021 | 0 |
| memory_target_not_found | 68 | 1 | 0.015 | 0 |
| buffer_ref_expired | 25 | 0 | 0.0 | 0 |
| cwd_escapes_root | 20 | 0 | 0.0 | 0 |
| edit_would_break_syntax | 17 | 0 | 0.0 | 0 |
| ast_extent_fail | 17 | 0 | 0.0 | 0 |
| invalid_param_shape | 13 | 0 | 0.0 | 0 |
| symbol_not_found | 10 | 0 | 0.0 | 0 |
| invalid_regex | 9 | 0 | 0.0 | 0 |
| json_path_unsupported | 7 | 0 | 0.0 | 0 |
| extra_models_reserved_field | 5 | 0 | 0.0 | 0 |
| write_blocked_read_only | 5 | 0 | 0.0 | 0 |
| lsp_index_locked | 3 | 0 | 0.0 | 0 |
| ack_replay_file_changed | 3 | 0 | 0.0 | 0 |
| ambiguous_heading | 2 | 0 | 0.0 | 0 |
| destructive_replace_blocked | 2 | 0 | 0.0 | 0 |
| json_path_shape_mismatch | 1 | 0 | 0.0 | 0 |
| symbols_pattern_is_regex | 1 | 0 | 0.0 | 0 |
| ambiguous_name_path | 1 | 0 | 0.0 | 0 |
| ack_handle_refused | 1 | 0 | 0.0 | 0 |

| err_family | errors | recovered | unrecovered(25) | %unrec | median | p90 | mean |
| il3_pipe_to_trimmer | 722 | 722 | 0 | 0.0 | 1 | 1 | 1.07 |
| il3_shell_on_source | 498 | 497 | 1 | 0.2 | 1 | 1 | 1.15 |
| il1_read_overlaps_symbol | 357 | 357 | 0 | 0.0 | 1 | 2 | 1.34 |
| path_not_found | 310 | 310 | 0 | 0.0 | 1 | 2 | 1.4 |
| il2_structural_edit | 255 | 255 | 0 | 0.0 | 1 | 1 | 1.09 |
| librarian_managed_artifact | 202 | 202 | 0 | 0.0 | 1 | 2 | 1.32 |
| missing_required_param | 191 | 191 | 0 | 0.0 | 1 | 2 | 1.17 |
| <unclassified> | 171 | 171 | 0 | 0.0 | 1 | 1 | 1.12 |
| edit_stale_match | 163 | 163 | 0 | 0.0 | 1 | 1 | 1.04 |
| read_file_overflow_threshold | 157 | 157 | 0 | 0.0 | 1 | 1 | 1.04 |
| worktree_activate_required | 132 | 132 | 0 | 0.0 | 1 | 2 | 1.35 |
| unknown_enum_value | 77 | 77 | 0 | 0.0 | 1 | 3 | 1.58 |
| memory_target_not_found | 68 | 68 | 0 | 0.0 | 1 | 1 | 1.06 |
| json_path_key_miss | 66 | 66 | 0 | 0.0 | 1 | 1 | 1.09 |
| invalid_line_range | 52 | 52 | 0 | 0.0 | 1 | 1 | 1.06 |
| shrink_guard_blocked | 46 | 46 | 0 | 0.0 | 1 | 1 | 1.09 |
| replace_would_rename | 41 | 41 | 0 | 0.0 | 1 | 1 | 1.1 |
| param_not_applied | 41 | 41 | 0 | 0.0 | 1 | 1 | 1.05 |
| target_already_exists | 36 | 36 | 0 | 0.0 | 1 | 1 | 1.06 |
| replace_dropped_sibling | 30 | 30 | 0 | 0.0 | 1 | 2 | 1.27 |
| heading_not_found | 28 | 28 | 0 | 0.0 | 1 | 1 | 1.07 |
| lsp_document_symbol_null | 26 | 25 | 1 | 3.8 | 1 | 2 | 1.2 |
| ambiguous_old_string | 25 | 25 | 0 | 0.0 | 1 | 1 | 1.08 |
| buffer_ref_expired | 25 | 25 | 0 | 0.0 | 1 | 1 | 1.04 |
| write_scope_denied | 21 | 20 | 1 | 4.8 | 1 | 3 | 1.55 |
| unanchored_archive_refused | 21 | 21 | 0 | 0.0 | 1 | 2 | 1.43 |
| cwd_escapes_root | 20 | 20 | 0 | 0.0 | 1 | 1 | 1.0 |
| artifact_rooted_in_temp | 18 | 18 | 0 | 0.0 | 1 | 4 | 1.78 |
| edit_would_break_syntax | 17 | 17 | 0 | 0.0 | 1 | 1 | 1.06 |
| ast_extent_fail | 17 | 17 | 0 | 0.0 | 1 | 1 | 1.06 |
| write_lock_unavailable | 15 | 15 | 0 | 0.0 | 1 | 2 | 1.2 |
| invalid_param_shape | 13 | 13 | 0 | 0.0 | 1 | 1 | 1.0 |
| mutually_exclusive_params | 12 | 12 | 0 | 0.0 | 1 | 2 | 1.17 |
| symbol_not_found | 10 | 10 | 0 | 0.0 | 1 | 2 | 1.1 |
| invalid_regex | 9 | 9 | 0 | 0.0 | 1 | 3 | 1.22 |
| json_path_wrong_buffer_kind | 7 | 7 | 0 | 0.0 | 1 | 3 | 1.57 |
| json_path_unsupported | 7 | 7 | 0 | 0.0 | 1 | 2 | 1.14 |
| extra_models_reserved_field | 5 | 5 | 0 | 0.0 | 1 | 1 | 1.0 |
| write_blocked_read_only | 5 | 5 | 0 | 0.0 | 1 | 1 | 1.0 |
| lsp_index_locked | 3 | 3 | 0 | 0.0 | 1 | 1 | 1.0 |
| ack_replay_file_changed | 3 | 3 | 0 | 0.0 | 1 | 1 | 1.0 |
| ambiguous_heading | 2 | 2 | 0 | 0.0 | 1 | 1 | 1.0 |
| destructive_replace_blocked | 2 | 2 | 0 | 0.0 | 1 | 1 | 1.0 |
| json_path_shape_mismatch | 1 | 1 | 0 | 0.0 | 1 | 1 | 1.0 |
| symbols_pattern_is_regex | 1 | 1 | 0 | 0.0 | 1 | 1 | 1.0 |
| ambiguous_name_path | 1 | 1 | 0 | 0.0 | 1 | 1 | 1.0 |
| ack_handle_refused | 1 | 1 | 0 | 0.0 | 1 | 1 | 1.0 |
overall recovered 3927 unrecovered 3
error_runs {'run_length_histogram': {'1': 3132, '2': 263, '3': 43, '4': 16, '5': 8, '6': 2, '8': 1, '9': 1, '10': 1}, 'runs_total': 3467, 'runs_len_ge_2': 335, 'errors_inside_runs_ge_2': 798, 'errors_total': 3930, 'pct_errors_in_a_run': 20.3, 'longest_run': 10}

## keyed by session_id (probe convention), id<=299175: errors 3928, immediate repeat 286 (7.3%), unrecovered within 25 calls 3 (0.1%), recovery calls median 1.0 p90 2.0 mean 1.18
runs hist {1: 3132, 2: 262, 3: 43, 4: 16, 5: 8, 6: 2, 8: 1, 9: 1, 10: 1} runs>=2 334 errors in runs>=2 796 20.3%
| tool | errors | imm repeat | rate% | unrecovered(25) | rec median | rec p90 |
| run_command | 1271 | 27 | 2.1 | 1 | 1.0 | 1.0 |
| read_file | 1025 | 145 | 14.1 | 0 | 1.0 | 2.0 |
| edit_file | 665 | 42 | 6.3 | 1 | 1.0 | 2.0 |
| doc | 378 | 41 | 10.8 | 0 | 1.0 | 2.0 |
| edit_code | 251 | 15 | 6.0 | 1 | 1.0 | 2.0 |
| create_file | 102 | 8 | 7.8 | 0 | 1.0 | 2.0 |
| memory | 75 | 2 | 2.7 | 0 | 1.0 | 1.0 |
| symbols | 70 | 3 | 4.3 | 0 | 1.0 | 1.0 |
| grep | 64 | 1 | 1.6 | 0 | 1.0 | 1.0 |
| references | 12 | 1 | 8.3 | 0 | 1.0 | 1.0 |
| librarian | 5 | 0 | 0.0 | 0 | 1.0 | 1.6 |
| call_graph | 4 | 1 | 25.0 | 0 | 1.0 | 1.7000000000000002 |
| workspace | 3 | 0 | 0.0 | 0 | 1.0 | 1.0 |
| get_guide | 2 | 0 | 0.0 | 0 | 1.0 | 1.0 |
| approve_write | 1 | 0 | 0.0 | 0 | 1.0 | 1.0 |

## keyed by (session_id, agent_id) principal: errors 3928, immediate repeat 275 (7.0%), unrecovered within 25 calls 11 (0.3%), recovery calls median 1.0 p90 2.0 mean 1.16
runs hist {1: 3157, 2: 256, 3: 44, 4: 16, 5: 8, 6: 1, 8: 1, 9: 1} runs>=2 327 errors in runs>=2 771 19.6%
| tool | errors | imm repeat | rate% | unrecovered(25) | rec median | rec p90 |
| run_command | 1271 | 19 | 1.5 | 5 | 1.0 | 1.0 |
| read_file | 1025 | 137 | 13.4 | 1 | 1.0 | 2.0 |
| edit_file | 665 | 46 | 6.9 | 3 | 1.0 | 2.0 |
| doc | 378 | 40 | 10.6 | 0 | 1.0 | 2.0 |
| edit_code | 251 | 15 | 6.0 | 2 | 1.0 | 2.0 |
| create_file | 102 | 8 | 7.8 | 0 | 1.0 | 2.0 |
| memory | 75 | 4 | 5.3 | 0 | 1.0 | 1.0 |
| symbols | 70 | 3 | 4.3 | 0 | 1.0 | 1.0 |
| grep | 64 | 1 | 1.6 | 0 | 1.0 | 1.0 |
| references | 12 | 1 | 8.3 | 0 | 1.0 | 1.0 |
| librarian | 5 | 0 | 0.0 | 0 | 1.0 | 1.6 |
| call_graph | 4 | 1 | 25.0 | 0 | 1.0 | 1.7000000000000002 |
| workspace | 3 | 0 | 0.0 | 0 | 1.0 | 1.0 |
| get_guide | 2 | 0 | 0.0 | 0 | 1.0 | 1.0 |
| approve_write | 1 | 0 | 0.0 | 0 | 1.0 | 1.0 |
```

Derived (arithmetic from the tables above): the five gate/teach families il3_pipe_to_trimmer 722 + il3_shell_on_source 498 + il1_read_overlaps_symbol 356 + il2_structural_edit 255 + librarian_managed_artifact 202 = 2,033 = 51.8% of errors; the two il3 families alone are 1,220 of run_command's 1,271 errors (96.0%).
Hint share: 3,081 of 3,928 errors (78.4%) carry `— hint:` (all recoverable_error; 0 of the 45 hard errors do). The brief's "about 3,065" is slightly lower; 3,081 is the snapshot figure. 847 errors have no hint: run_command 723, doc 80, edit_code 16, create_file 11, edit_file 5, others 12.
err_family IS NULL: 171 errors (4.4%): doc 78, read_file 26, edit_file 18, run_command 13, edit_code 10; 154 recoverable + 17 hard. Top 60-char prefixes are in the output above (a bucket, not a cause).


## 5. TIME SERIES (script base_s5.py)

Coverage: called_at is the completion time, UTC, fully populated. Day 2026-10-12 is partial (starts 08:06), 2026-11-10 partial (ends 22:23).

```
| day | wkday | calls | sessions | err% | distinct tools | subagent% | top root class (share%) | top session (share%) |
|---|---|---|---|---|---|---|---|---|
| 2026-10-12 | Mon | 505 | 3 | 3.4 | 11 | 61 | main_checkout (97) | 9c826e41 (97) |
| 2026-10-13 | Tue | 3247 | 9 | 3.0 | 18 | 19 | main_checkout (84) | 9c826e41 (29) |
| 2026-10-14 | Wed | 2910 | 16 | 2.8 | 19 | 3 | main_checkout (51) | 5d96a3b1 (54) |
| 2026-10-15 | Thu | 382 | 7 | 3.7 | 10 | 0 | main_checkout (99) | d424bc65 (49) |
| 2026-10-16 | Fri | 5595 | 18 | 5.0 | 17 | 66 | claude_agent_worktree (49) | e98b5705 (58) |
| 2026-10-17 | Sat | 9333 | 7 | 4.6 | 18 | 95 | claude_agent_worktree (82) | 500c9b13 (55) |
| 2026-10-18 | Sun | 4951 | 8 | 3.1 | 17 | 93 | claude_agent_worktree (83) | 500c9b13 (55) |
| 2026-10-19 | Mon | 2258 | 5 | 2.7 | 16 | 85 | claude_agent_worktree (56) | 500c9b13 (42) |
| 2026-10-20 | Tue | 2023 | 6 | 3.7 | 14 | 71 | claude_agent_worktree (67) | 6b17822c (68) |
| 2026-10-21 | Wed | 1740 | 7 | 2.5 | 15 | 68 | repo_worktree (52) | bb9e4979 (59) |
| 2026-10-22 | Thu | 550 | 4 | 4.2 | 9 | 36 | repo_worktree (50) | bb9e4979 (47) |
| 2026-10-23 | Fri | 989 | 13 | 2.7 | 13 | 0 | main_checkout (93) | 74145174 (31) |
| 2026-10-24 | Sat | 3765 | 16 | 4.0 | 13 | 40 | main_checkout (41) | 5b936994 (27) |
| 2026-10-25 | Sun | 0 | 0 | - | 0 | - | - | - |
| 2026-10-26 | Mon | 632 | 6 | 3.5 | 11 | 26 | main_checkout (100) | e0833726 (60) |
| 2026-10-27 | Tue | 3391 | 15 | 3.2 | 15 | 18 | main_checkout (75) | cb06fa74 (29) |
| 2026-10-28 | Wed | 2603 | 14 | 3.3 | 17 | 27 | main_checkout (74) | cb06fa74 (52) |
| 2026-10-29 | Thu | 6668 | 41 | 3.2 | 15 | 19 | main_checkout (55) | cb06fa74 (26) |
| 2026-10-30 | Fri | 4660 | 45 | 3.7 | 14 | 38 | main_checkout (73) | e4a5a540 (22) |
| 2026-10-31 | Sat | 6307 | 39 | 4.0 | 16 | 24 | main_checkout (69) | 0e844aa1 (26) |
| 2026-11-01 | Sun | 6661 | 32 | 3.3 | 14 | 16 | main_checkout (79) | 0e844aa1 (25) |
| 2026-11-02 | Mon | 3819 | 22 | 2.6 | 16 | 10 | main_checkout (93) | 0e844aa1 (32) |
| 2026-11-03 | Tue | 3393 | 17 | 1.6 | 13 | 10 | main_checkout (81) | 0e844aa1 (64) |
| 2026-11-04 | Wed | 7167 | 30 | 3.6 | 14 | 28 | main_checkout (80) | 0e844aa1 (18) |
| 2026-11-05 | Thu | 5084 | 19 | 2.9 | 16 | 20 | main_checkout (69) | 3d9ab02a (47) |
| 2026-11-06 | Fri | 3738 | 15 | 2.4 | 14 | 0 | main_checkout (94) | 3d9ab02a (57) |
| 2026-11-07 | Sat | 5969 | 31 | 3.7 | 19 | 33 | main_checkout (84) | 4b1c0778 (25) |
| 2026-11-08 | Sun | 3006 | 19 | 3.1 | 17 | 35 | main_checkout (69) | 109592b1 (29) |
| 2026-11-09 | Mon | 10385 | 76 | 2.7 | 16 | 10 | main_checkout (75) | 879d21b0 (14) |
| 2026-11-10 | Tue | 6547 | 27 | 2.6 | 16 | 8 | main_checkout (57) | ac7fb73b (49) |

first day partial: first call 2026-10-12 08:06:52.291 last day partial: 2026-11-10 last call 2026-11-10 22:23:51.553

## hour of day UTC (calls)
| hour | calls | share% |
| 00 | 417 | 0.4 |
| 01 | 370 | 0.3 |
| 02 | 252 | 0.2 |
| 03 | 1858 | 1.6 |
| 04 | 2792 | 2.4 |
| 05 | 3652 | 3.1 |
| 06 | 5935 | 5.0 |
| 07 | 6828 | 5.8 |
| 08 | 8385 | 7.1 |
| 09 | 6768 | 5.7 |
| 10 | 8223 | 7.0 |
| 11 | 8421 | 7.1 |
| 12 | 7918 | 6.7 |
| 13 | 7908 | 6.7 |
| 14 | 7184 | 6.1 |
| 15 | 6670 | 5.6 |
| 16 | 6003 | 5.1 |
| 17 | 4846 | 4.1 |
| 18 | 5701 | 4.8 |
| 19 | 6919 | 5.8 |
| 20 | 3431 | 2.9 |
| 21 | 3385 | 2.9 |
| 22 | 3265 | 2.8 |
| 23 | 1147 | 1.0 |

## weekday (calls; note number of each weekday in window differs)
| Mon | 17599 | 14.9 | days in window 5 | mean/day 3520 |
| Tue | 18601 | 15.7 | days in window 5 | mean/day 3720 |
| Wed | 14420 | 12.2 | days in window 4 | mean/day 3605 |
| Thu | 12684 | 10.7 | days in window 4 | mean/day 3171 |
| Fri | 14982 | 12.7 | days in window 4 | mean/day 3746 |
| Sat | 25374 | 21.5 | days in window 4 | mean/day 6344 |
| Sun | 14618 | 12.4 | days in window 4 | mean/day 3654 |

daily calls: days with data 29 p50 3738.0 p90 6767.799999999999 max 10385 min 382
busiest 8 hours (UTC): [('2026-10-17 07', 1402), ('2026-10-16 14', 1362), ('2026-11-09 11', 1243), ('2026-10-24 03', 1240), ('2026-11-09 10', 1119), ('2026-10-17 12', 1062), ('2026-11-09 19', 1018), ('2026-10-17 08', 950)]
0 2026-10-24 15:37:47.688 2026-10-26 14:48:43.677
gaps>=6h between consecutive calls: [(47.2, '2026-10-24 15:37', '2026-10-26 14:48'), (17.1, '2026-10-15 10:25', '2026-10-16 03:34'), (13.7, '2026-10-21 14:58', '2026-10-22 04:37'), (13.7, '2026-10-19 17:50', '2026-10-20 07:30'), (12.0, '2026-10-18 17:03', '2026-10-19 05:05'), (11.9, '2026-11-03 19:21', '2026-11-04 07:16'), (11.7, '2026-10-23 15:22', '2026-10-24 03:04'), (11.2, '2026-11-08 20:03', '2026-11-09 07:16'), (11.0, '2026-11-06 20:17', '2026-11-07 07:16'), (10.3, '2026-10-28 18:44', '2026-10-29 04:59'), (10.1, '2026-11-05 19:58', '2026-11-06 06:03'), (10.1, '2026-10-22 16:38', '2026-10-23 02:47')] n 23
```

Explanations (checked against the daily table's top root class / top session columns, and for hours by a direct query):
- 2026-10-25 (Sun): zero rows; the gap is 47.2 h (2026-10-24 15:37 to 10-26 14:48). Cause not determinable from the data.
- Oct 16-19 (5,595 / 9,333 / 4,951 / 2,258 calls, subagent share 66-95%): claude_agent_worktree roots; sessions e98b5705 (8,399 calls, 55 subagent ids) and 500c9b13 (8,777 calls, 50 subagent ids) - a subagent-fleet campaign. The busiest hour, 2026-10-17 07 UTC (1,402 calls), is session 500c9b13 across several `agent-*` worktrees (verified by query).
- 2026-10-24 03 UTC (1,240 calls, 4th busiest hour): mixed sessions (9fba5b92 main checkout 269, cb06fa74 codex worktree 162, 5b936994 two `fix-*` worktrees 214), not one harness.
- Nov 9 peak day (10,385 calls, 76 sessions active, 70 started that day): main checkout 75%, many short sessions; on Nov 10 codex session ac7fb73b is 49% of calls.
- Saturdays have the highest mean calls/day (6,344 vs 3,171-3,746 other weekdays) but 4 Saturdays only; Oct 17 alone is 9,333 of the 25,374 Saturday calls.
- Hours 06-19 UTC are 4.1-7.1% each; 00-02 UTC 0.2-0.4%; 23 UTC 1.0%.


## 6. BUILD IDENTITY (script base_s6.py)

Coverage: codescout_sha, codescout_dirty, project_sha 100% populated. `codescout_sha` is the build provenance (BuildProvenance) of the codescout server binary that served the call, plus a dirty bit; `project_sha` is the HEAD of the project the call ran against (457 distinct).

```
codescout_sha null/empty 0 dirty values Counter({1: 76335, 0: 41943}) project_sha null/empty 0
distinct codescout_sha 141 distinct project_sha 457

| codescout_sha | calls | share% | sessions | dirty calls | first seen | last seen | commit date (git, committer) |
|---|---|---|---|---|---|---|---|
| 3923da8c | 8777 | 7.4 | 1 | 8777 | 2026-10-17 05:57 | 2026-10-19 14:16 | 2026-10-16T17:57:05+03:00 |
| 0007d926 | 8558 | 7.2 | 4 | 8558 | 2026-10-16 13:27 | 2026-10-18 11:16 | 2026-10-16T16:21:49+03:00 |
| 0eb6190ec | 8436 | 7.1 | 2 | 8436 | 2026-10-30 06:49 | 2026-11-04 16:17 | 2026-10-30T08:02:16+02:00 |
| 2acc553ad | 7630 | 6.5 | 9 | 7630 | 2026-10-23 09:58 | 2026-10-29 19:45 | 2026-10-23T11:55:57+03:00 |
| aebb0e76f | 6150 | 5.2 | 8 | 0 | 2026-11-04 17:17 | 2026-11-07 09:45 | 2026-11-04T18:54:20+02:00 |
| 69ffdd12e | 4908 | 4.1 | 9 | 0 | 2026-11-09 18:34 | 2026-11-10 22:23 | 2026-11-09T18:41:48+02:00 |
| f7d65b877 | 3547 | 3.0 | 18 | 0 | 2026-11-06 10:36 | 2026-11-09 08:09 | 2026-11-06T10:44:34+02:00 |
| a52d68284 | 3090 | 2.6 | 6 | 3090 | 2026-11-09 08:22 | 2026-11-10 18:08 | 2026-11-09T10:17:52+02:00 |
| 223b83aa | 3077 | 2.6 | 11 | 3077 | 2026-10-14 06:25 | 2026-10-16 14:44 | 2026-10-14T03:42:57+03:00 |
| 59e43edc | 3024 | 2.6 | 8 | 3024 | 2026-10-16 03:50 | 2026-10-21 03:51 | 2026-10-16T06:45:58+03:00 |
top10 share 48.3581054803091
shas resolvable in git: 141 / 141 calls covered 118278 100.0%
age (days) of commit date vs call time, resolvable rows 118278 p50 0.61 p90 3.28 max 7.9 negative 0
calls served by build whose commit is older than 1d at call time: 39946 (33.8% of resolvable)
calls served by build whose commit is older than 3d at call time: 12403 (10.5% of resolvable)
calls served by build whose commit is older than 7d at call time: 91 (0.1% of resolvable)
calls served by build whose commit is older than 14d at call time: 0 (0.0% of resolvable)

| week | calls | dirty calls | dirty% | distinct shas |
| 2026-10-12 | 26923 | 26921 | 100.0 | 14 |
| 2026-10-19 | 11325 | 11325 | 100.0 | 15 |
| 2026-10-26 | 30922 | 24254 | 78.4 | 51 |
| 2026-11-02 | 32176 | 8335 | 25.9 | 43 |
| 2026-11-09 | 16932 | 5500 | 32.5 | 30 |
overall dirty 76335 64.53862933089839

project_sha top5 [('5a8b146', 10250), ('317746e', 6426), ('aebb0e7', 4869), ('32d0c71', 4453), ('299395e', 2296)] distinct 457
sample project_sha ['918b3d5', '18a007a']
sha lengths Counter({9: 130, 8: 11})
```

Caveats: commit dates are the committer date of each sha resolved in this repo (all 141 shas resolve, 100% of calls). A build can be compiled after its commit date, so "age" is an UPPER bound of binary age. `experiments` is rebased routinely, so committer dates can differ from original authoring dates. A `dirty` build contains uncommitted changes, so its sha does not identify its code. The 4 biggest shas (3923da8c, 0007d926, 0eb6190ec, 2acc553ad) are dirty builds each served by only 1-9 long-lived server processes (sessions column).


## 7. RECORDING HEALTH (scripts base_s7a.py, base_s7b.py)

Definitions found in code:
- `effect_class` (src/usage/db.rs open_db migration): what a `run_command` caller DECLARED - `read` or `write` (write = default when it said nothing). NULL for all other tools. A declaration, not an observation.
- `observed_changed` (src/usage/observed.rs; db.rs migration comment): `1` = a before/after snapshot of git-visible state (HEAD + `git status` paths with size/mtime) DIFFERED during a run_command that declared `read`; `0` = no difference seen (NOT proof it changed nothing; blind to gitignored paths, /tmp, $HOME); NULL = not observed (write-declared, background, buffer-handle commands, errors, held-for-ack, non-git repo, git over 2 s, other tools). `1` on a shared checkout can be a peer's edit: a falsifier of "quiet", not attribution.
- `attribution_outcome` / `attribution_ms` (src/usage/attribution_outcome.rs; db.rs migration comment): what the red-attribution engine did on a run_command with a NON-ZERO exit: `hinted` (printed a hint), `silent` (finished, printed nothing), `timed_out`, `tooling_failure`, `spawn_failed`; NULL = engine did not start (exit 0 with nothing failure-shaped, opt-out, or row older than the migration; first non-NULL row 2026-11-01 14:42). ms = wall time of the engine run.
- `deliveries_json` (src/usage/deliveries.rs): NULL = the post-phase fan-out never ran (tool returned Err, tool overrides call_content, or pre-migration); `[]` = ran, nothing delivered; `[{engine, ledger_keys, blocks:[{sha256,bytes}], hint}]` = one record per contributing engine.
- `usage-dropped-rows.log` (src/usage/mod.rs note_dropped_row): tab-separated `started_at, tool, pin (- if none), error chain, pid, build(+dirty), wait_ms`; older 4-field lines (no pid/build/wait_ms) coexist (71 of 186 read). `usage-lock-holds.log` (src/usage/db.rs note_lock_hold): `started_at, pid, build, kind (backfill|insert|retention_sweep), ms, detail`, written only when one write statement held the DB >= 250 ms (LOCK_HOLD_LOG_MS). Both logs start 2026-11-09 (issue 2026-11-09-concurrent-pinned-calls-leave-no-usage-row): they cover ~1.5 days, not the corpus.
- Retention: `write_record` runs `DELETE FROM tool_calls WHERE called_at < datetime('now','-30 days')` (sparing rows referenced by `pika_observations`) after EVERY insert (db.rs write_record). The live table's min id is 180898 = the snapshot's min id, so everything earlier is gone; every count here is a floor on all-time and the window is ~30 days.

Output of base_s7a.py (logs; unbounded, read at analysis time) and base_s7b.py (DB, snapshot-bounded):

```
dropped lines 194 fields histogram Counter({7: 123, 4: 71})
range 2026-11-09 13:12:49.991 2026-11-10 22:34:45.740
reasons [('database is locked: Error code #: database is locked', 194)]
tools [('run_command', 56), ('read_file', 44), ('symbols', 41), ('grep', 26), ('doc', 8), ('get_guide', 5), ('workspace', 4), ('edit_file', 4), ('memory', 2), ('create_file', 2), ('edit_code', 2)]
per day [('2026-11-09', 104), ('2026-11-10', 90)]
pin class Counter({'no-pin': 106, 'main_checkout': 48, 'codex_worktree': 22, 'repo_worktree': 18})
wait_ms (n=123): p50 5008.0 p90 5027.6 max 5776
builds [('ba103456c', 52), ('5b40efc48', 26), ('f34fbc8b3', 22), ('7be425b48', 12), ('ea44079c4+dirty', 6)]
lines with 4 fields day range 2026-11-09 2026-11-10
lines with 7 fields day range 2026-11-09 2026-11-10

lock-hold lines 1350 range 2026-11-09 20:21:44.486 2026-11-10 22:35:16.233 fields Counter({6: 1350})
overall p50 2410 p90 4476 p99 9042 max 20004; n>=1000ms 1027, >=5000ms 89, >=30000ms 0
| kind | n | p50 ms | p90 | max | total s |
| retention_sweep | 879 | 2659 | 4692 | 20004 | 2478 |
| insert | 398 | 1540 | 3740 | 6528 | 724 |
| backfill | 73 | 2429 | 6409 | 12444 | 209 |
insert [('run_command', 148), ('read_file', 89), ('symbols', 69), ('grep', 32), ('edit_file', 22), ('edit_code', 7)]
backfill [('from=# to=# update_scan_ms=#', 73)]
retention_sweep [('deleted=#', 879)]
min ms logged 251
top holds:
2026-11-10 17:13:36.021	126708	5b40efc48	retention_sweep	20004	deleted=0
2026-11-09 22:50:30.578	1719246	f34fbc8b3	retention_sweep	14779	deleted=0
2026-11-10 22:25:32.158	3621727	ba103456c	retention_sweep	12953	deleted=0
2026-11-10 17:20:14.837	3148557	5b40efc48	backfill	12444	from=1877607761 to=676504337 update_scan_ms=10475
2026-11-10 22:34:45.051	275057	ba103456c	retention_sweep	11753	deleted=0
2026-11-10 17:12:23.005	3148557	5b40efc48	retention_sweep	11203	deleted=0
2026-11-10 16:10:12.351	1975418	f34fbc8b3	retention_sweep	11061	deleted=0
2026-11-10 11:45:09.941	2829334	5b40efc48	retention_sweep	10739	deleted=0
insert holds by tool:  [('run_command', 148, 1745, 5030), ('read_file', 89, 1338, 4916), ('symbols', 69, 838, 6528), ('grep', 32, 1844, 5085), ('edit_file', 22, 2462, 4957), ('edit_code', 7, 2245, 3942), ('references', 7, 1839, 3955), ('doc', 6, 1538, 3701), ('tree', 5, 335, 1753), ('create_file', 4, 1994, 3552)]
holds per day [('2026-11-09', 670), ('2026-11-10', 680)]
```

```
empty input: total 22463 19.0% empty output 43217 36.5%
input_json null vs '' (22463, 0)
| week | calls | parent | empty input parent% | empty input sub% | empty output parent% | empty output sub% |
| 2026-10-12 | 26923 | 8749 | 47.5 | 0.0 | 47.5 | 0.0 |
| 2026-10-19 | 11325 | 5076 | 32.6 | 0.0 | 32.6 | 0.0 |
| 2026-10-26 | 30922 | 23802 | 49.7 | 0.0 | 49.7 | 0.0 |
| 2026-11-02 | 32176 | 25350 | 19.0 | 0.0 | 60.1 | 0.0 |
| 2026-11-09 | 16932 | 15404 | 0.0 | 0.0 | 67.0 | 0.0 |
daily empty input% / parent empty-output%: [('2026-10-12', 3, 7), ('2026-10-13', 38, 47), ('2026-10-14', 81, 83), ('2026-10-15', 0, 0), ('2026-10-16', 10, 28), ('2026-10-17', 0, 7), ('2026-10-18', 0, 1), ('2026-10-19', 0, 0), ('2026-10-20', 0, 0), ('2026-10-21', 19, 59), ('2026-10-22', 1, 1), ('2026-10-23', 29, 29), ('2026-10-24', 27, 46), ('2026-10-26', 11, 15), ('2026-10-27', 48, 59), ('2026-10-28', 61, 83), ('2026-10-29', 51, 63), ('2026-10-30', 21, 34), ('2026-10-31', 32, 42), ('2026-11-01', 32, 38), ('2026-11-02', 35, 41), ('2026-11-03', 64, 82), ('2026-11-04', 18, 51), ('2026-11-05', 0, 72), ('2026-11-06', 0, 88), ('2026-11-07', 0, 57), ('2026-11-08', 0, 10), ('2026-11-09', 0, 57), ('2026-11-10', 0, 82)]
last empty input row 2026-11-04 16:17:53.417

rows with tool_use_id 75061 63.5% first 2026-10-12 08:06:52.291 distinct 75061
tool_use_id by week {'2026-10-12': (22763, 26923), '2026-10-19': (9668, 11325), '2026-10-26': (19081, 30922), '2026-11-02': (16932, 32176), '2026-11-09': (6617, 16932)}
tool_use_id by agent Counter({'sub': 39897, 'parent': 35164})
sample tool_use_id ['toolu_01FEro1gx1AdvkoLcd4n9NUD', 'toolu_014qSPtHwUA7YoWL8uNJSYd2']
coverage after first appearance 75061 / 118278 63.5%

effect_class by tool Counter({('run_command', 'write'): 32161, ('run_command', 'read'): 19204}) non-null 51365 run_command rows 51365
run_command effect_class Counter({'write': 32161, 'read': 19204})
first effect_class row 2026-10-12 08:06:52.291
run_command (effect_class, observed_changed): Counter({('write', None): 32161, ('read', 0): 11425, ('read', None): 7545, ('read', 1): 234})
observed_changed non-run_command non-null 0
observed_changed=1 by outcome Counter({'success': 234})

attribution_outcome Counter({None: 116551, 'silent': 1549, 'hinted': 177, 'timed_out': 1})
by tool non-null Counter({'run_command': 1727})
silent 1549 ms p50 150.0 p90 247.0 max 5762
hinted 177 ms p50 8104.0 p90 16703.600000000002 max 29859
timed_out 1 ms p50 30012.0 p90 30012.0 max 30012
first attribution row 2026-11-01 14:42:31.771
run_command errors 1271
```

Bounded counts: dropped-log lines with timestamp <= snapshot end (2026-11-10 22:23:51.553): 143 (the file had 193 lines at last read; it grows while other sessions run). Rows stored in the snapshot from the first drop line (2026-11-09 13:12:49) to the end: 11,946, so about 1.2% (143 / (143 + 11,946)) of attempted calls in that ~33 h window left no row (approximation: drops are attributed to the DB they would have been written to, pins can route elsewhere). Every dropped line read has the one reason `database is locked: Error code 5: database is locked`; wait_ms p50 5,008 (the 5 s busy_timeout).
Retention sweep (hold log): in the s7a read, 860 of 1,315 hold lines are `retention_sweep` (p50 2,654 ms, max 20,004 ms; 2,404 s of 3,302 s total logged hold time). In a later read of the log, 862 sweep lines had `deleted=0` and only 10 deleted 1-5 rows. So the sweep is a no-op DELETE that holds the single writer lock for seconds: it is the largest holder class. Why it is slow (e.g. no index on called_at) is NOT verified.
Observed_changed: of 19,204 read-declared run_commands, 11,425 are 0, 234 are 1 (all on success rows), 7,545 NULL; write-declared 32,161 all NULL.


## 8. HINTS AND GUIDES (scripts base_s8.py, base_s4.py, base_s9.py)

Coverage: deliveries_json is NULL on 3,980 rows = all 3,928 error rows + 52 success rows (an Err never reaches the fan-out), so error rows never carry a delivery record. 5,564 rows (4.7%) have a non-empty delivery; 108,734 are `[]`; no row has more than one engine entry. The `cs-hint` substring in output_json is NOT counted (echoed file content).

```
deliveries_json null 3980 []  108734 non-empty 5564
null by outcome Counter({'recoverable_error': 3883, 'success': 52, 'error': 45})
rows with non-empty deliveries 5564 rows with >1 engine entry 0

| engine | deliveries | sessions | bytes delivered (sum) | hint=true | hint=false |
| guide-sections | 3745 | 350 / 464 | 22663127 | 3745 | 0 |
| session-opener | 1660 | 446 / 464 | 5458640 | 1660 | 0 |
| operator-rules | 159 | 48 / 464 | 36031 | 0 | 159 |

| engine | ledger_key | deliveries | sessions | principals |
| session-opener | project-activation-bootstrap | 1660 | 446 | 976 |
| guide-sections | progressive-disclosure | 869 | 226 | 541 |
| guide-sections | symbol-navigation | 802 | 236 | 506 |
| guide-sections | librarian#Artifact Model | 336 | 135 | 169 |
| guide-sections | librarian#docs/trackers/ — Backing Store, Not a Docs Folder | 336 | 135 | 169 |
| guide-sections | librarian#Choosing a mode — anti-patterns | 328 | 130 | 166 |
| guide-sections | librarian#The shrink guard, `force`, and `patch`'s accepted keys | 328 | 130 | 166 |
| guide-sections | tracker-conventions#Bug frontmatter and status vocabulary | 290 | 135 | 164 |
| guide-sections | librarian#Filter Syntax | 255 | 122 | 164 |
| guide-sections | tracker-conventions#Claiming and releasing a bug | 243 | 116 | 142 |
| guide-sections | tracker-conventions#The unverified caveat, made queryable | 243 | 116 | 142 |
| guide-sections | tracker-conventions#Querying with the librarian | 240 | 118 | 138 |
| guide-sections | tracker-conventions#Bug files (docs/issues/) | 156 | 71 | 71 |
| guide-sections | tracker-conventions#Trigger rules and capture discipline | 156 | 71 | 71 |
| guide-sections | tracker-conventions#A `## Resume` states its own currency | 124 | 56 | 64 |
| operator-rules | op:OP-3 | 122 | 34 | 35 |
| guide-sections | workspace-state | 109 | 79 | 88 |
| guide-sections | tracker-conventions#Detecting these fields | 82 | 35 | 35 |
| guide-sections | tracker-conventions#Entry-level standard — the shape INSIDE a tracker | 82 | 35 | 35 |
| guide-sections | tracker-conventions#Required fields | 82 | 35 | 35 |
| guide-sections | tracker-conventions#Required fields — Rests on, and detecting the lines | 82 | 35 | 35 |
| guide-sections | tracker-conventions#Entry headings — the definition rule | 80 | 33 | 33 |
| guide-sections | tracker-conventions#Entry ids | 80 | 33 | 33 |
| guide-sections | tracker-conventions#Entry ids — where the counter lives | 80 | 33 | 33 |
| guide-sections | librarian#librarian(action=...) — Reference | 71 | 44 | 49 |
| guide-sections | tracker-conventions#<pointer> | 70 | 23 | 48 |
| guide-sections | tracker-conventions#Entry ids — refusals: main checkout only, and unpushed ledgers | 54 | 26 | 26 |
| guide-sections | tracker-conventions#Archiving a bug or tracker — re-point the citations | 46 | 32 | 39 |
| guide-sections | tracker-conventions#Archiving a bug or tracker — stage both halves of the move | 46 | 32 | 39 |
| guide-sections | tracker-conventions#Archiving a bug or tracker — through the catalog, never git mv | 46 | 32 | 39 |
| guide-sections | tracker-conventions#Reading move's inbound_path_citations | 44 | 30 | 37 |
| guide-sections | tracker-conventions#Verifying the citation sweep | 44 | 30 | 37 |
| guide-sections | tracker-conventions#Why a green gate proves nothing about the sweep | 44 | 30 | 37 |
| operator-rules | op:OP-4 | 37 | 19 | 36 |
| guide-sections | progressive-disclosure#Output budgets | 37 | 19 | 31 |
| guide-sections | progressive-disclosure#Reading a buffer | 37 | 19 | 31 |
| guide-sections | tracker-conventions#A prefix has one owner per repository | 36 | 11 | 11 |
| guide-sections | tracker-conventions#Declaring a ledger | 36 | 11 | 11 |
| guide-sections | tracker-conventions#Make the tracker guarded | 36 | 11 | 11 |
| guide-sections | tracker-conventions#Tracker artifacts (docs/trackers/) | 36 | 11 | 11 |
| guide-sections | librarian#Reach for augmentation — don't hand-maintain the table | 34 | 22 | 22 |
| guide-sections | progressive-disclosure#Background jobs: when no wake arrives, and waiting on another process | 34 | 20 | 29 |
| guide-sections | tracker-conventions#Entry ids — refusals: main checkout only (prose ledgers) | 26 | 7 | 7 |
| guide-sections | librarian#doctor repairs — what each `fix=` mode does | 25 | 19 | 21 |
| guide-sections | progressive-disclosure#Background jobs: start, end the turn, read after the wake | 21 | 16 | 16 |
| guide-sections | progressive-disclosure#Reading the stderr of a buffered command: the `.err` suffix | 21 | 16 | 16 |
| guide-sections | librarian#Archiving / Moving Trackers | 19 | 11 | 11 |
| guide-sections | librarian#doc — Event Log | 9 | 7 | 7 |
| guide-sections | librarian#Changing ONE entry — don't hand-build the array | 6 | 3 | 3 |
| guide-sections | tracker-conventions#Declaring an augmentation | 6 | 5 | 5 |
| guide-sections | tracker-conventions#Declaring an augmentation — why the sidecar exists, and the check's remedy | 6 | 5 | 5 |
| guide-sections | progressive-disclosure#<preamble> | 6 | 5 | 5 |
| guide-sections | librarian#Augmentation Lifecycle | 5 | 4 | 4 |
| guide-sections | librarian#Worktree overlay | 5 | 3 | 3 |
| guide-sections | tracker-conventions#Citing an entry — a third shape, and mentioning without citing | 5 | 5 | 5 |
| guide-sections | tracker-conventions#Citing an entry — bare, or qualified | 5 | 5 | 5 |
| guide-sections | librarian#<preamble> | 2 | 2 | 2 |
| guide-sections | librarian#Refreshing an augmented artifact — the cycle and the stale check | 1 | 1 | 1 |
| guide-sections | tracker-conventions#<preamble> | 1 | 1 | 1 |
| guide-sections | tracker-conventions#Cross-linking (edges are derived — cite in prose) | 1 | 1 | 1 |
distinct ledger keys 61
deliveries by tool [('doc', 1982), ('run_command', 1091), ('symbols', 883), ('workspace', 839), ('read_file', 220), ('grep', 143), ('memory', 137), ('librarian', 102), ('create_file', 55), ('get_guide', 54)]

sessions with >=1 guide-sections key 350 keys per session p50 5.0 p90 16.0 max 38
guide topic -> sessions: [('progressive-disclosure', 247), ('symbol-navigation', 236), ('librarian', 225), ('tracker-conventions', 204), ('workspace-state', 79)]
sessions with session-opener 446
explicit get_guide topics (rows w/ input) [('error-handling', 96), ('librarian', 70), ('tracker-conventions', 53), ('symbol-navigation', 32), ('progressive-disclosure', 31), ('workspace-state', 27), ('iron-laws-detail', 22), ('untrusted-content', 2), (None, 2), ('worktree-merge', 1), ('librarian-runtime', 1), ('nonexistent-topic', 1)] sessions {'librarian': 39, 'tracker-conventions': 30, 'error-handling': 28, 'workspace-state': 22, 'progressive-disclosure': 21, 'symbol-navigation': 21, 'iron-laws-detail': 11, 'untrusted-content': 2, 'worktree-merge': 1, 'librarian-runtime': 1, None: 1, 'nonexistent-topic': 1}
get_guide total rows 452

errors 3928 w/ hint 3081
first seen per engine {'session-opener': '2026-10-12 08:31:43.517', 'guide-sections': '2026-10-12 08:43:28.575', 'operator-rules': '2026-10-16 14:39:18.333'}
non-empty deliveries on error rows 0
```

Error-hint next-call outcome (base_s9.py; next call of the same principal after an error; descriptive only, confounded by tool mix because no-hint errors are mostly run_command):

```
live DB: rows (118546, 180898, 299443, '2026-10-12 08:06:52.291', '2026-11-10 22:33:04.086')
snapshot rows older than id 180898: 0
pika_observations table? 1
| scope | hint? | errors | next call success% | next call error same tool% | next error other tool% |
| all | hint | 3081 | 87.1 | 9.9 | 2.7 |
| all | nohint | 847 | 93.3 | 4.5 | 2.0 |
| doc | hint | 298 | 84.2 | 15.1 | 0.7 |
| doc | nohint | 80 | 80.0 | 17.5 | 2.5 |
| edit_code | hint | 235 | 84.3 | 11.9 | 3.4 |
| edit_code | nohint | 16 | 75.0 | 18.8 | 6.2 |
| edit_file | hint | 660 | 87.0 | 9.7 | 3.0 |
| edit_file | nohint | 5 | 60.0 | 0.0 | 20.0 |
| read_file | hint | 1023 | 84.5 | 13.8 | 1.7 |
| read_file | nohint | 2 | 50.0 | 0.0 | 50.0 |
| run_command | hint | 548 | 92.3 | 2.7 | 4.2 |
| run_command | nohint | 723 | 96.1 | 2.6 | 1.1 |
hint-prefix(50) distinct 952 top3 prefixes cover 507 top10 1008 top 47(>=10 each) 1775
sessions touching category / calls in category: {'/tmp/other': 3, '/tmp/codescout-*': 27, 'codex worktree': 47, 'setup-ab': 9} {'/tmp/other': 14, '/tmp/codescout-*': 4727, 'codex worktree': 7658, 'setup-ab': 2243}
sessions >=50% harness calls by dominant category (sessions): {'/tmp/other': 2, '/tmp/codescout-*': 15, 'codex worktree': 25, 'setup-ab': 3} calls in those sessions {'/tmp/other': 13, '/tmp/codescout-*': 6968, 'codex worktree': 3137, 'setup-ab': 2703}
setup-ab roots: ['/home/marius/work/claude/codescout/.worktrees/setup-ab-eval']
codex roots: [('/home/marius/.codex/worktrees/615e/codescout', 3104), ('/home/marius/.codex/worktrees/cf66/codescout', 2706), ('/home/marius/.codex/worktrees/3940/codescout', 688), ('/home/marius/.codex/worktrees/9369/codescout', 386), ('/home/marius/.codex/worktrees/04e2/codescout', 288), ('/home/marius/.codex/worktrees/ca35/codescout', 210), ('/home/marius/.codex/worktrees/e646/codescout', 170), ('/home/marius/.codex/worktrees/add2/codescout', 99), ('/home/marius/.codex/worktrees/b541/codescout', 7)]
tmp roots calls: [('/tmp/codescout-real-task-20261024', 3572), ('/tmp/codescout-stock-observations-20261109', 510), ('/tmp/codescout-checked-acceptance-20261027', 195), ('/tmp/codescout-albert-hardening-review-20261014', 169), ('/tmp/codescout-r02-review-20261014', 113), ('/tmp/codescout-memory-review-20261014', 104), ('/tmp/codescout-tmp-exec-01a15ca9', 64), ('/tmp/native-history-events-20261011', 13), ('/tmp/claude-1000/-home-marius-work-claude-codescout/1e54b0a7-88de-4ced-ad52-e807a774cded/scratchpad/wt', 1)]
codex session count 47
```

Hint texts: 1,135 distinct strings after `— hint:` (994 after replacing digit runs with `#`; 918 occur once; 47 occur >= 10 times). Many embed paths/ids, so the distinct count overstates distinct advice. Top 10 hint texts are in the base_s4.out section.
