---
title: Usage Analysis — 2026-04-02
date: 2026-04-02
topic: telemetry
summary: "Cross-project analysis of `usage.db` from codescout (self, Rust) and backend-kotlin (client)."
status: complete
---

# Usage Analysis — 2026-04-02

Cross-project analysis of `usage.db` from **codescout** (self, Rust) and **backend-kotlin** (client, Kotlin/Python/TS).

**Data window: 2026-03-26 → 2026-04-02 (7 days)**

## Dataset

| Project | Total Calls | Errors | Success Rate |
|---------|-------------|--------|-------------|
| codescout | 3,279 | 175 | 94.7% |
| backend-kotlin | 3,064 | 159 | 94.8% |

## Tool Volume Distribution

### codescout (Rust project, self-development)

| Tool | Calls | Avg Latency | Error % | Overflows |
|------|------:|------------:|--------:|----------:|
| run_command | 807 | 4,763ms | 1.7% | 0 |
| find_symbol | 716 | 176ms | 2.0% | 0 |
| read_file | 437 | <1ms | 9.8% | 2 |
| grep | 273 | 1ms | 0.0% | 25 |
| read_markdown | 264 | <1ms | 3.8% | 9 |
| list_symbols | 163 | 82ms | 1.8% | 2 |
| replace_symbol | 115 | 62ms | 7.0% | 0 |
| edit_file | 105 | 1ms | 35.2% | 0 |
| edit_markdown | 102 | <1ms | 36.3% | 0 |
| memory | 77 | 263ms | 6.5% | 0 |
| activate_project | 64 | 26ms | 0.0% | 0 |
| list_dir | 49 | <1ms | 0.0% | 0 |
| insert_code | 47 | 53ms | 6.4% | 0 |
| semantic_search | 20 | 207ms | 0.0% | 0 |
| create_file | 12 | <1ms | 0.0% | 0 |
| onboarding | 9 | 218ms | 0.0% | 0 |
| remove_symbol | 7 | 59ms | 14.3% | 0 |
| project_status | 5 | 11ms | 0.0% | 0 |
| glob | 3 | 2ms | 0.0% | 2 |
| index_status | 1 | 45ms | 0.0% | 0 |
| find_references | 1 | 913ms | 0.0% | 0 |

### backend-kotlin (client project, Kotlin/Python/TS)

| Tool | Calls | Avg Latency | Error % | Overflows |
|------|------:|------------:|--------:|----------:|
| run_command | 542 | 2,870ms | 3.0% | 0 |
| read_file | 496 | <1ms | 6.0% | 19 |
| find_symbol | 473 | 1,334ms | 1.1% | 0 |
| grep | 283 | 5ms | 0.0% | 7 |
| read_markdown | 235 | <1ms | 2.1% | 4 |
| list_symbols | 216 | 429ms | 11.1% | 2 |
| edit_file | 190 | <1ms | 15.8% | 0 |
| list_dir | 129 | 1ms | 0.0% | 2 |
| memory | 106 | 241ms | 0.9% | 0 |
| activate_project | 102 | 7ms | 0.0% | 0 |
| semantic_search | 79 | 259ms | 0.0% | 0 |
| edit_markdown | 53 | <1ms | 37.7% | 0 |
| create_file | 49 | <1ms | 0.0% | 0 |
| replace_symbol | 43 | 20ms | 32.6% | 0 |
| glob | 30 | 11ms | 0.0% | 1 |
| remove_symbol | 11 | 1ms | 90.9% | 0 |
| onboarding | 8 | 307ms | 0.0% | 0 |
| index_status | 8 | 32ms | 0.0% | 0 |
| insert_code | 4 | 2ms | 25.0% | 0 |
| find_references | 3 | 10ms | 100.0% | 0 |
| index_project | 2 | <1ms | 0.0% | 0 |
| project_status | 1 | 22ms | 0.0% | 0 |
| hover | 1 | 1ms | 0.0% | 0 |

## Error Counts (absolute)

### codescout

| Tool | Errors |
|------|-------:|
| read_file | 43 |
| edit_markdown | 37 |
| edit_file | 37 |
| find_symbol | 14 |
| run_command | 14 |
| read_markdown | 10 |
| replace_symbol | 8 |
| memory | 5 |
| list_symbols | 3 |
| insert_code | 3 |

### backend-kotlin

| Tool | Errors |
|------|-------:|
| read_file | 30 |
| edit_file | 30 |
| list_symbols | 24 |
| edit_markdown | 20 |
| run_command | 16 |
| replace_symbol | 14 |
| remove_symbol | 10 |
| read_markdown | 5 |
| find_symbol | 5 |
| find_references | 3 |

## LSP Latency (new_session handshake, last 7 days)

| Language | codescout | backend-kotlin |
|----------|----------:|---------------:|
| Rust | 15ms | 15ms |
| Python | 376ms | 366ms |
| TypeScript | 85ms | 87ms |
| JavaScript | 85ms | 87ms |
| Java | 2,345ms | 2,493ms |
| CSS | 225ms | — |
| HTML | 438ms | — |

No Kotlin LSP starts in either project in the last 7 days.

### LSP Eviction

- codescout: 29 idle evictions (Java 6, JS 6, Python 6, TS 6, Rust 4) + 1 LRU
- backend-kotlin: 5 idle + 2 LRU — much lower churn

## Investigation Items

### P0 — `edit_markdown` broken everywhere

- codescout: **36.3%** error rate (37/102 calls)
- backend-kotlin: **37.7%** error rate (20/53 calls)
- Consistent across projects → tool logic bug, not LSP-dependent
- Investigate error messages to identify root cause

### P1 — `edit_file` error rate spiked

- codescout: **35.2%** (37/105) — all-time was 9.3%, so this is a significant regression
- backend-kotlin: **15.8%** (30/190)
- Highest absolute error count in backend-kotlin (tied with read_file at 30)
- Investigate: did recent changes regress? Or has usage shifted to harder edit patterns?

### P2 — Structural edit tools unreliable on non-Rust LSPs

Error rates codescout → backend-kotlin:
- `replace_symbol`: 7.0% → **32.6%**
- `remove_symbol`: 14.3% → **90.9%** (10 of 11 calls failed)
- `find_references`: 0% (1 call) → **100%** (3/3 failed)
- `list_symbols`: 1.8% → **11.1%**
- `insert_code`: 6.4% → **25.0%**

No Kotlin LSP was active this week, so these failures are from Python/TS/JS LSPs — the problem is broader than just Kotlin.

### P3 — `read_file` error rate elevated

- codescout: **9.8%** (43/437) — all-time was 2.2%
- backend-kotlin: **6.0%** (30/496)
- Highest absolute error source in codescout (43 errors)
- Need to check: how many are intentional source-file blocks vs genuine failures?

### P4 — Dead tools (zero or near-zero usage)

Last 7 days across both projects:
- `goto_definition`: 0 calls
- `hover`: 1 call total
- `rename_symbol`: 0 calls
- `find_references`: 4 calls total, 75% error rate
- These tools are effectively unused. Either fix reliability first or improve prompt surfacing.

### P5 — `semantic_search` underused but reliable

- codescout: 20 calls (0.6%), backend-kotlin: 79 calls (2.6%)
- **0% error rate** in both projects — reliability is not the blocker
- Agents prefer `grep`/`find_symbol` even when semantic would be more appropriate
- Consider better prompt guidance for when to reach for semantic search

## Positive Signals

- `grep`: **0.0% error rate** across 556 combined calls — bulletproof
- `find_symbol`: 2.0% / 1.1% error rate — reliable core navigation tool
- `run_command`: handles heavy volume (1,349 combined), low error rate
- `semantic_search`: 0% errors — the tool works, just needs more adoption
- `memory`: 183 combined calls — agents actively leveraging project memory
- `create_file`: 0% error rate, 61 calls — write path is clean for new files
- Progressive disclosure working: overflow flag fires only on read/search tools as designed
