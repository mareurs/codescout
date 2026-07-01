# Usage Analysis — 2026-07-01

Scope: `stefanini/southpole/MRV-poc` (requested). Three `usage.db` files found
(main + two worktrees).

## What `usage.db` is

Per-project SQLite telemetry codescout writes at `.codescout/usage.db` on every
tool call. Four tables:

| Table | Purpose | Key columns |
|---|---|---|
| `tool_calls` | one row per MCP tool invocation | `tool_name, called_at, latency_ms, outcome (success\|error), overflowed, overflow_tokens, error_msg, err_family, session_id, cc_session_id, input_json, output_json, codescout_sha, project_sha, project_root, friction_target` |
| `lsp_events` | LSP server lifecycle | `language, reason (new_session\|idle_evicted), handshake_ms, first_response_ms, outcome (success\|failed), error` |
| `call_edges` | cached call-graph edges (not usage) | `project_id, caller_sym, callee_sym, file, line, col, source, computed_at` |
| `sqlite_sequence` | SQLite autoincrement bookkeeping | — |

`err_family` is the load-bearing column for analysis: codescout classifies each
error into a stable family (e.g. `il3_shell_on_source`, `json_path_unsupported`)
so recurring friction is countable without regex-matching raw messages. Schema
evolves via auto-applied migrations — the `vertex-ai-search` worktree DB predates
the `err_family` / `lsp_events.outcome` migrations (last written 2026-05-27).

## Cross-Project Summary

Projects scanned: 3 · Total calls: **15,759** · Combined error rate: **~7.7%** · Sessions: 53

| DB | calls | error % | sessions | window |
|---|---|---|---|---|
| MRV-poc (main) | 15,534 | 7.7% | 51 | 2026-06-01 → 07-01 |
| .worktrees/vertex-ai-search | 224 | 6.7% | 1 | 2026-05-27 |
| .worktrees/ui-improvements | 1 | 0% | 1 | 2026-06-06 |

### Top Issues (ranked)

1. **[MRV-poc] `read_file` json_path can't reach dotted object keys** — one file,
   `benchmarks/section-audit/uat_criteria_by_section.json` (top-level keys `1.1`…`2.1.5`),
   drove **278 errors / 461 calls (60% failure)** ≈ **23% of all project errors**.
   `$.2.1.5`, `$["2.1.5"]`, `$['2.1.5']` all fail; the key is unaddressable.
   → Filed **`docs/issues/2026-07-01-read-file-jsonpath-dotted-object-keys-unreachable.md`** (severity high).
2. **[MRV-poc] IL3 `run_command` guard hits: 265** — `il3_shell_on_source` (143) +
   `il3_pipe_to_trimmer` (122). Model habitually reaches for native shell
   grep/sed/cat on source and pipes unbounded output to head/tail/grep.
3. **[MRV-poc] IL1 `read_file` overlaps symbol: 189** (`il1_read_overlaps_symbol`) —
   line-range reads on source redirected to `symbols(include_body=true)`.
4. **[MRV-poc] edit-routing friction: ~105** — `il2_structural_edit` (57, edit_file on
   code → edit_code), `edit_stale_match` (25), `il5_edit_markdown_routing` (23), plus
   `edit_code` `replace_dropped_sibling` (13) + `ast_extent_fail` (10).
5. **[MRV-poc] read/edit on librarian-managed artifacts: ~31** — direct `read_markdown`
   on managed trackers (`corpus-gaps.md`, `eval-pipeline-safety.md`, `retrieval-experiments.md`)
   instead of `artifact(get)`.
6. **[MRV-poc] latency: 470 `run_command` >10s** — mostly legitimate long ML/eval/test
   jobs, but several **busy-wait polling loops** (`for i in $(seq…); sleep`, `sleep 600 && pgrep`)
   that the `Monitor`/background-job pattern would replace. Max single call **876s (14.6 min)**.
7. **LSP healthy** — zero failed starts. Churn worth noting: python 207 new / 187 idle-evicted.

---

## Project: MRV-poc (main)

**DB:** `/home/marius/work/stefanini/southpole/MRV-poc/.codescout/usage.db`

### Overview
`total_calls: 15534 · error_pct: 7.7 · from 2026-06-01 to 2026-07-01 · sessions: 51`

### Tool Popularity
| tool | calls | avg_ms | max_ms | errors | overflows |
|---|---|---|---|---|---|
| run_command | 5431 | 9353 | 876575 | 266 | 111 |
| read_file | 2544 | 1 | 1002 | 524 | 31 |
| symbols | 2218 | 133 | 8760 | 9 | 78 |
| grep | 1429 | 25 | 1803 | 5 | 19 |
| read_markdown | 1215 | 0 | 9 | 143 | 11 |
| edit_file | 706 | 5 | 2238 | 118 | 0 |
| edit_markdown | 489 | 1 | 24 | 42 | 0 |
| edit_code | 410 | 28 | 753 | 32 | 0 |
| create_file | 329 | 0 | 12 | 27 | 0 |
| artifact | 329 | 7 | 118 | 15 | 41 |
| memory | 157 | 170 | 3167 | 5 | 0 |
| workspace | 85 | 3374 | 8823 | 1 | 0 |
| tree | 71 | 12 | 101 | 0 | 3 |
| references | 34 | 375 | 1950 | 6 | 0 |
| semantic_search | 16 | 1478 | 2241 | 0 | 1 |
| librarian | 10 | 451 | 2976 | 2 | 1 |
| index | 10 | 3798 | 7962 | 0 | 0 |
| (others: approve_write, artifact_augment, artifact_event, symbol_at, onboarding, get_guide, call_graph) | | | | | |

### Error Breakdown — by `err_family`
| tool | err_family | n |
|---|---|---|
| read_file | il1_read_overlaps_symbol | 189 |
| read_file | json_path_unsupported | 168 |
| read_file | (uncategorized: path-segment-not-found) | 165 |
| read_markdown | (managed-artifact + non-.md + missing-heading) | 143 |
| run_command | il3_shell_on_source | 143 |
| run_command | il3_pipe_to_trimmer | 122 |
| edit_file | il2_structural_edit | 57 |
| edit_markdown | (missing heading / stale) | 33 |
| edit_file | edit_stale_match | 25 |
| edit_file | il5_edit_markdown_routing | 23 |
| create_file | (exists / path) | 22 |
| edit_code | replace_dropped_sibling | 13 |
| edit_code | ast_extent_fail | 10 |

**read_file is the error hotspot: 524 errors / 2544 calls = 20.6%.** 579 calls used
`json_path`; the dotted-key file alone accounts for 278 of the errors (see Top Issue #1).

### Overflow Tools
| tool | overflow_calls |
|---|---|
| run_command | 111 |
| symbols | 78 |
| artifact | 41 |
| read_file | 31 |
| grep | 19 |
| read_markdown | 11 |
| tree | 3 |
| semantic_search / librarian | 1 each |

### Latency Buckets (success only, top by >10s)
| tool | total | <100ms | <1s | <10s | ≥10s |
|---|---|---|---|---|---|
| run_command | 5165 | 2946 | 708 | 1041 | 470 |
| workspace | 84 | 37 | 4 | 43 | 0 |
| symbols | 2209 | 1801 | 311 | 97 | 0 |
| semantic_search | 16 | 0 | 2 | 14 | 0 |

Only `run_command` has ≥10s calls (470). Every other tool is sub-10s; read/symbols/grep are near-instant.

### Slow Commands (>10s, top)
| latency | command (truncated) |
|---|---|
| 876575 | `… RETRIEVAL_STACK=gcp-full XLS…` (eval run) |
| 840151 | `for i in $(seq 1 28); do grep -q "\[done;" @bg_… ; sleep` (**busy-wait poll**) |
| 600078 | `sleep 600 && pgrep -f "_build_figure_index"` (**busy-wait poll**) |
| 600011 / 600010 | `uv run python scripts/_build_figure_index.py` (10-min timeout) |
| 600009 / 600008 | `while ps -p … ; sleep` / `until ! pgrep … ; sleep` (**busy-wait poll**) |
| 420009 | `until ! pgrep -f "pytest -q" ; sleep` (**busy-wait poll**) |
| 408424 / 398616 / 389953 | `uv run python scripts/_gen_all_sections_dev.py …` (generation) |

Pattern: long-running Python/ML jobs are legitimate, but the `for i in seq … sleep` /
`sleep 600 && pgrep` / `until ! pgrep … sleep` loops are foreground busy-waits that block a
tool slot for minutes — candidates for the `Monitor` tool or background jobs + notification.

### Session Summary (top by calls)
| session | calls | errors | err% | overflows |
|---|---|---|---|---|
| 3ba95070 | 2422 | 206 | 8.5 | 0 |
| 28ebe8ed | 1911 | 165 | 8.6 | 72 |
| ca6d1d98 | 1561 | 64 | 4.1 | 0 |
| 3c78fed3 | 1109 | 136 | 12.3 | 62 |
| 7e4868cd | 1107 | 64 | 5.8 | 0 |
| 5f130154 | 934 | 87 | 9.3 | 0 |
| a6cb1474 | 666 | 19 | 2.9 | 0 |
| d7255246 | 616 | 126 | **20.5** | 46 |
| 301b6142 | 568 | 28 | 4.9 | 43 |
| 1682edec | 435 | 45 | 10.3 | 0 |

Flag: **d7255246** (20.5% error, 46 overflows) and **3c78fed3** (12.3%, 62 overflows) are
drill-down candidates (claude-traces `cc.py tool-calls`) if a deeper root-cause is wanted.

### LSP Events (success)
Healthy — no failed starts (query I empty). Selected:
| language | reason | starts | avg_hs_ms | max_hs_ms |
|---|---|---|---|---|
| java | new_session | 1 | 1864 | 1864 |
| html | new_session | 65 | 435 | 1433 |
| python | new_session | 207 | 279 | 754 |
| python | idle_evicted | 187 | 253 | 638 |
| rust | new_session | 1 | 15 | 15 |

Note the eviction churn: python alone had 187 idle-evictions + 207 restarts. Handshakes are
fast (<500ms typical), so churn is overhead, not a failure.

---

## Project: vertex-ai-search (worktree)

**DB:** `…/MRV-poc/.worktrees/vertex-ai-search/.codescout/usage.db`
Stale single session (2026-05-27), 224 calls, 6.7% error (15 errors). Pre-migration schema
(no `err_family`). Top tools: run_command 140 (3 err), read_markdown 19 (4 err),
create_file 13, symbols 12, read_file 11 (3 err), grep 9. Nothing actionable beyond the
main-DB patterns.

## Project: ui-improvements (worktree)

**DB:** `…/MRV-poc/.worktrees/ui-improvements/.codescout/usage.db`
1 call, 0 errors — activation-only. Summary row only.

---

## Recommendations

1. **Fix the json_path dotted-key gap** (filed bug, severity high) — biggest single error
   source and a genuine tool limitation, not model error. Support `["key"]`/`['key']` and
   bracket-aware tokenization in `parse_json_path_segments`.
2. **IL3/IL1 volume (454 combined)** is the guards *working*, but the recurrence suggests a
   prompt-surface nudge: the model still reaches for shell grep/sed/cat on source and
   read_file line-ranges over symbols. Candidate for a `docs/trackers/tool-usage-patterns.md`
   (T-N) entry feeding `src/prompts/source.md`.
3. **Busy-wait polling loops** in run_command — surface the `Monitor` / background-job pattern
   for "wait until job done" so multi-minute foreground waits stop blocking tool slots.
4. **LSP idle-eviction churn** — python evict/restart cadence is high; if idle TTL is tunable,
   a longer TTL for the dominant language could cut restart overhead (non-urgent).
