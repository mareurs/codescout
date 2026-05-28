---
title: "Usage Analysis — 2026-05-27"
status: draft
kind: report
---

# Usage Analysis — 2026-05-27

Scope: all `usage.db` files under `~/work` with ≥20 tool calls.
Combined DB built at `/tmp/usage-combined.db` for cross-project aggregation.

## Cross-Project Summary

- **Projects scanned:** 34 (≥20 calls each); 44 total DBs (10 below threshold)
- **Total tool calls:** 101,051
- **Combined error rate:** 5.6% (5,654 errors)
- **Total overflows:** 466
- **Sessions:** 957
- **Date range:** 2026-05-01 → 2026-05-28

### Top Tensions, Bugs, and Improvements

Ranked by signal-to-action ratio (highest leverage first):

1. **[CLAUDE.md / system prompt] Native-tool habit is the #1 friction.** 1,950+ errors are
   policy redirects: `run_command` "shell access blocked" (812 across 16 projects), `read_file` →
   "use read_markdown" (390 across ~12 projects), `edit_file` → "use edit_markdown" (356), and
   `edit_file` → "use symbol tools" (403). These aren't bugs in codescout — they're the model
   reaching for native CC tools out of habit. The redirects work, but every one costs a turn.
   **Improvement candidate:** strengthen the `server_instructions` surface anti-pattern section
   to lead with these four redirects + their failure shape, since they dominate real-session
   pushback.

2. **[infra] Kotlin LSP cold-start dominates language-server cost.** Avg 5.3s, p100 62.5s for
   new sessions; eviction restarts average 24s (`lru_evicted`) and 11.7s (`idle_evicted`). Java
   is also slow (avg 2.2s, max 6.3s, 374 starts). Compared to Rust 17ms / TS-JS ~90ms / Python
   341ms, JVM startup is the bottleneck.
   **Improvement candidate:** lifting the Kotlin LSP eviction policy, or warming a single
   Kotlin LSP at codescout startup when a `.kt` file is present, would reclaim significant
   wall time. Tracks `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md`.

3. **[ux] `run_command` 30s default timeout clipping** — 389 calls hit ~30s, 102 at ~60s,
   51 at ~120s, 20 at ~300s, 4 at the 900s ceiling. Many of these are real long-running ops
   (uv pytest, mrv ingest, builds) where the user passed a `timeout_secs` that still didn't
   suffice, or didn't pass one at all. Suggest the LLM defaults to `run_in_background=true`
   for known-slow command patterns (pytest, build, ingest, eval).
   **Improvement candidate:** add a hint in the run_command timeout error pointing toward
   `run_in_background=true` when the command matches known-long patterns.

4. **[bug] LSP server disconnected** — 27 `edit_code` errors (2 projects) + 16 `symbols` errors
   (3 projects). The Kotlin LSP issue is the prime suspect, but `edit_code` failing here is
   actionable: the user-facing error message gives no recovery path. The fix is to retry once
   or surface "restart codescout / wait N seconds".

5. **[overflow] `grep` is the dominant overflow tool** — 314 overflows (100% of overflowed
   calls). Sample patterns show broad alternations (`^from|^import`,
   `create_embedder|LocalEmbedder|RemoteEmbedder|embed\.rs`). The 50-row default limit may be
   too tight for codebases with many matches; combined with the lack of
   `by_file` distribution in some grep responses, the model can't easily narrow.
   **Improvement candidate:** when grep overflows, include a top-3 `by_file` distribution so
   the model knows whether to narrow path or pattern.

6. **[ux] `read_file` "both start_line and end_line are required"** — 26× across 7 projects.
   Small papercut: `start_line` alone should default `end_line = start_line + 50` (or similar)
   rather than error. Low-cost UX fix.

7. **[ops] semantic_search `dense tei status`** — 53 errors, 45 of which are in a single bench
   worktree on 2026-05-12. The embedding service was down during one bench run; not an ongoing
   bug, but evidence that the LLM has no good fallback when TEI is unavailable.

## Per-Project Drill-Downs (top 10 by volume)

### claude/code-explorer (29,050 calls, 5.5% err, 133 ovfl, 336 sessions)

The codescout dev project itself. Heavy `run_command` (build/test/clippy), `symbols`,
`edit_code`. 373 `edit_file` errors — mostly markdown/symbol redirects.

Notable: high session count (336) means many short turns. `2c6a550c` (247 calls) and
`89359f47` (145 calls / 22 errors / 15.2%) stand out.

### stefanini/southpole/MRV-poc (24,375 calls, 5.4% err, 86 ovfl, 156 sessions)

Heavy ETL/eval workload. 274 `edit_file` errors (Python codebase, hitting the
`debug_enforce_symbol_tools` gate). Source of every 900s timeout clip (3 of 4).
Long-running `uv run mrv ingest` / `uv run pytest` calls dominate the >10s bucket.

### mirela/backend-kotlin (12,974 calls, 5.6% err, 84 ovfl, 91 sessions)

Kotlin LSP-heavy. 166 `edit_file` errors. Marathon session `92308a19`: 394 calls / 64
errors / 6 overflows — drill-down candidate via `cc.py tool-calls`.

### stefanini/invest-europe/lang-pal-engine (8,498 calls, 4.5% err, 65 ovfl, 43 sessions)

106 `edit_file` errors. One 1,001s `uv run pytest -m permutation` call — eval test
duration, not a bug. Tighter `timeout_secs` + `run_in_background` would help.

### claude/claude-plugins (5,460 calls, 6.6% err, 18 ovfl, 73 sessions)

Highest error rate among top-5 projects. Session `ee0362c3` worst: 69 calls / 18 errors
/ 26.1%. Plugin dev workflow shows model uncertainty (likely tool-name guessing).

### claude/code-explorer/.worktrees/bench (3,983 calls, 1.2% err, 1 ovfl, 72 sessions)

Single-day bench session (2026-05-12). 45 `dense tei` errors during embedding-service
outage. Excellent error rate (1.2%) when infrastructure was working.

### mirela/eduplanner-ui (3,742 calls, 3.9% err, 39 ovfl, 38 sessions)

TS/JS LSP — fast, healthy. 39 `edit_file` errors (markdown redirects mostly).

### stefanini/southpole/MRV-poc/.worktrees/reviewer-ui (3,278 calls, 6.2% err, 4 ovfl, 10 sessions)

54 `edit_file` errors. Smaller session count, longer sessions — focused worktree work.

### claude/researcher (1,403 calls, 4.9% err, 2 ovfl, 28 sessions)

Healthy. 22 `edit_file` errors. Session `0608fbea` (67 calls / 9 errors / 13.4%) worth
inspecting if you want a deeper drill.

### stefanini/southpole/MRV-poc/.worktrees/gcp-native-retrieval (1,184 calls, 5.7% err, 1 ovfl, 6 sessions)

Recent worktree (2026-05-18 onward). Patterns mirror parent MRV-poc.

## Other Projects (20–1,162 calls)

| Project | Calls | Err% | Ovfl | Sessions | Note |
|---|---|---|---|---|---|
| mirela/deployment | 1,163 | 6.4 | 1 | 25 | 1 × 900s build clip |
| stefanini/southpole/tools/workspace-mcp | 1,132 | 6.4 | 7 | 13 | |
| claude/topictracker | 1,021 | 2.5 | 0 | 6 | Cleanest project |
| mirela/backend-kotlin/ktor-server | 900 | **11.0** | 8 | 6 | Sub-project, Kotlin LSP impact |
| mirela/backend-kotlin/.worktrees/weekly-pattern | 593 | 3.7 | 0 | 1 | |
| claude/playground | 335 | 6.3 | 2 | 13 | |
| stefanini/southpole/MRV-poc/.worktrees/vertex-ai-search | 224 | 6.7 | 0 | 1 | |
| stefanini/invest-europe/pal/.worktrees/phase1-openapi/pal-api | 167 | 4.8 | 0 | 1 | |
| claude/opencode | 162 | 0.6 | 9 | 2 | |
| stefanini/extenda/extenda-buddy-smartstore | 162 | 3.1 | 0 | 3 | |
| stefanini/southpole/P-C AI Project | 151 | 7.3 | 0 | 6 | |
| claude/claude-plugins/buddy | 144 | **11.1** | 2 | 1 | Single session, high err |
| ionut/hotel | 142 | 4.2 | 1 | 3 | |
| personal/home/ha | 135 | 6.7 | 0 | 1 | |
| mirela/backend-kotlin/python-services | 123 | **15.4** | 0 | 2 | Highest err% of >100-call group |
| mirela | 90 | 4.4 | 3 | 3 | |
| stefanini/invest-europe/lang-pal-engine/docling-worker | 80 | **18.8** | 0 | 2 | Highest err% overall |
| mrv-vertex-probe | 79 | 0.0 | 0 | 1 | |
| mirela/optaplanner | 75 | **10.7** | 0 | 3 | |
| mirela/eduplanner-site | 67 | **10.4** | 0 | 2 | |
| stefanini/utils/tempo | 49 | 6.1 | 0 | 2 | |
| mirela/eduplanner-mobile | 39 | 5.1 | 0 | 3 | |
| personal/phone | 37 | 0.0 | 0 | 1 | |
| stefanini/southpole/verra-mrv-scraper | 34 | 0.0 | 0 | 1 | |

(Projects with <20 calls omitted entirely: 10 DBs, mostly tests/fixtures and probes.)

## LSP Events

`lsp_events` data (2,614 rows total):

| Language | Reason | Starts | Avg handshake | Max handshake | Avg first response | Projects |
|---|---|---:|---:|---:|---:|---:|
| **kotlin** | lru_evicted | 3 | **24,035ms** | 62,453ms | — | 1 |
| **kotlin** | idle_evicted | 11 | **11,774ms** | 58,629ms | 413 | 2 |
| **kotlin** | new_session | 32 | **5,331ms** | 58,890ms | 371 | 3 |
| java | idle_evicted | 29 | 2,321 | 5,677 | — | 1 |
| java | lru_evicted | 29 | 2,244 | 2,383 | 446 | 1 |
| java | new_session | 374 | 2,188 | 6,277 | 839 | 5 |
| html | * | 238 | ~395 | 811 | ~16 | up to 9 |
| python | * | 624 | ~340 | 1,183 | ~400 | up to 19 |
| bash | * | 304 | ~285 | 1,634 | ~24 | up to 14 |
| css | * | 226 | ~190 | 651 | ~18 | up to 9 |
| javascript | * | 284 | ~90 | 270 | ~460 | up to 12 |
| typescript | * | 220 | ~88 | 361 | ~440 | up to 7 |
| jsx/tsx | * | 132 | ~88 | 112 | ~440 | up to 4 |
| **rust** | * | 106 | **16ms** | 61 | 4 | up to 6 |

**Reads in one phrase:** Kotlin/Java handshakes are 50-1000× slower than every other
language. Rust LSP cold start (rust-analyzer) is the fastest.

## Drilldown Candidates (Step 7)

Sessions matching trigger criteria (>50 calls, >10% errors, or >5 overflows):

| project | session prefix | calls | errs | err% | ovfl | drill? |
|---|---|---|---|---|---|---|
| mirela/backend-kotlin | 92308a19 | **394** | 64 | 16.2 | 6 | ✓ marathon |
| claude/code-explorer | 2c6a550c | 247 | 33 | 13.4 | 0 | ✓ |
| claude/code-explorer | 89359f47 | 145 | 22 | 15.2 | 0 | ✓ |
| stefanini/southpole/MRV-poc | cf26c267 | 101 | 18 | 17.8 | 0 | ✓ |
| claude/code-explorer | ddd18a18 | 104 | 16 | 15.4 | 0 | ✓ |
| claude/claude-plugins | ee0362c3 | 69 | 18 | **26.1** | 0 | ✓ worst err% |
| stefanini/southpole/MRV-poc | 918077ec | 76 | 18 | 23.7 | 0 | ✓ |

To drill any of these, run:

```bash
python3 ~/.claude/skills/analyze-usage/scripts/cc.py tool-calls <session_id> --project <project_path>
```

(Pull the verified path via `find ~/.claude/projects -name "<prefix>*"` first — the
project-path decoder mangles names with dashes; see common-mistakes in the skill.)

## Improvement Backlog (one-line summary)

| ID | Surface | Action | Effort |
|---|---|---|---|
| I-1 | `src/prompts/source.md` server_instructions | Lead anti-pattern section with the 4 dominant redirects (run_command-block / read_markdown / edit_markdown / symbol-tools) | low |
| I-2 | `src/lsp/*` kotlin | Sticky single-instance LSP / longer idle timeout / startup-warm | med |
| I-3 | `run_command` error message | When latency ≈ timeout_secs, hint toward `run_in_background=true` | low |
| I-4 | `edit_code` / `symbols` "LSP disconnected" | Single retry-after-reconnect + better recovery hint | med |
| I-5 | `grep` overflow response | Include top-3 `by_file` distribution so model can narrow | low |
| I-6 | `read_file` "both start_line and end_line required" | Default `end_line = start_line + 50` when start_line alone is passed | low |
| I-7 | `semantic_search` TEI status error | Graceful fallback to sparse-only / clearer "service unavailable" surface | med |

## Data Sources

- Per-project DBs: `find ~/work -path "*/.codescout/usage.db"`
- Combined scratch DB: `/tmp/usage-combined.db` (tagged by `project` column)
- Methodology + queries: `.claude/skills/analyze-usage/SKILL.md`
