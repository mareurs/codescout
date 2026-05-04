# Usage Analysis — 2026-05-03

## Cross-Project Summary

Projects scanned: 22  
Total calls: ~9,782 | Error rate: ~4.5% | Sessions: ~144  
Date range: 2026-05-01 → 2026-05-03

### Top Issues

1. **[bk/python-services] 27.3% error rate** — 6/22 calls failed; all file-not-found errors from navigating backend-kotlin monorepo using sub-project-relative paths (e.g. `eduplanner-mcp/Cargo.toml`, `ktor-server/...`) — wrong root assumption
2. **[MRV-poc] run_command max 786s (13 min)** — ML training scripts (`uv run python scripts/_train_lora_v5.py`, eval chunking) hitting default 600s timeout; avg 9.8s per call — heaviest compute load in the workspace
3. **[backend-kotlin] grep overflows 24x** — highest overflow count of any project; broad patterns on large Kotlin/Java codebase
4. **[code-explorer] edit_file blocked 66x + run_command blocked 40x** — enforcement working; agent keeps retrying before getting redirected
5. **[SYSTEMIC] memory topic-not-found across 3 projects** — deployment, eduplanner-site, optaplanner all missing architecture/conventions/development-commands memories; projects activated but not fully onboarded
6. **[claude-plugins] 8.1% error rate (443 calls)** — highest meaningful error rate among active projects

### Worst by Category

| Category | Project | Value |
|---|---|---|
| Error rate (high volume) | claude-plugins | 8.1% (443 calls) |
| Error rate (any volume) | bk/python-services | 27.3% (22 calls) |
| Max latency | MRV-poc | 786,295ms (13 min) |
| Avg latency | MRV-poc | 9,823ms |
| Grep overflows | backend-kotlin | 24 |
| Total calls | code-explorer | 5,050 (52% of all) |

### Systemic Errors (appear in >1 project)

| Error | Projects |
|---|---|
| `shell access to source files is blocked` | code-explorer (40), playground (4), librarian-mcp (2) |
| `Use edit_markdown for markdown files` | code-explorer, playground, eduplanner-mobile, workspace-mcp |
| `edit_file is blocked for source code files` | code-explorer (46), librarian-mcp (1) |
| `memory topic '...' not found` | deployment, eduplanner-site, optaplanner, eduplanner-mobile |
| `file not found: '...'` (monorepo path issues) | bk/python-services, librarian-mcp |

---

## Project: code-explorer

**DB:** `/home/marius/work/claude/code-explorer/.codescout/usage.db`

### Overview

| Metric | Value |
|---|---|
| Total calls | 5,050 |
| Error rate | 4.3% |
| Date range | 2026-05-01 → 2026-05-03 |
| Sessions | 57 |

### Tool Popularity

| Tool | Calls | Avg ms | Max ms | Errors | Overflows |
|---|---|---|---|---|---|
| run_command | 1,671 | 4,867 | 300,012 | 40 | 0 |
| symbols | 1,052 | 984 | 30,522 | 5 | 5 |
| read_file | 607 | 0 | 6 | 26 | 0 |
| grep | 370 | 2 | 30 | 1 | 19 |
| read_markdown | 317 | 0 | 2 | 20 | 0 |
| edit_code | 238 | 78 | 5,210 | 12 | 0 |
| workspace | 163 | 92 | 7,164 | 0 | 0 |
| edit_file | 163 | 12 | 35 | 66 | 0 |
| edit_markdown | 104 | 13 | 28 | 28 | 0 |
| semantic_search | 91 | 438 | 19,201 | 0 | 0 |
| create_file | 46 | 17 | 26 | 0 | 0 |
| artifact_update | 35 | 4 | 11 | 9 | 0 |
| artifact_get | 34 | 0 | 0 | 0 | 0 |
| tree | 32 | 8 | 111 | 0 | 2 |
| memory | 27 | 1 | 23 | 4 | 0 |
| index | 17 | 130 | 827 | 0 | 0 |
| call_graph | 7 | 770 | 2,703 | 4 | 0 |
| artifact_find | 11 | 2 | 12 | 0 | 0 |
| onboarding | 5 | 85 | 426 | 0 | 0 |
| librarian_reindex | 5 | 3,160 | 3,657 | 0 | 0 |

### Error Breakdown

| Tool | Error | Count |
|---|---|---|
| run_command | shell access to source files is blocked | 39 |
| edit_file | blocked — structural edit on source code | 26 |
| edit_file | blocked — source code file | 20 |
| read_file | Use read_markdown for markdown files | 14 |
| edit_file | Use edit_markdown for markdown files | 10 |
| read_markdown | heading not found | 8 |
| edit_markdown | missing 'action' parameter | 3 |
| edit_markdown | old_string not found | 3 |
| artifact_update | malformed frontmatter YAML | 4 |
| memory | dimension mismatch 384 vs 768 | 1 |
| call_graph | callees requires LSP callHierarchy (not available) | 2 |
| call_graph | symbol not found | 2 |

### Overflow Tools

| Tool | Overflow Calls |
|---|---|
| grep | 19 |
| read_file | 6 |
| symbols | 5 |
| tree | 2 |

### Latency Buckets

| Tool | Total | <100ms | <1s | <10s | >10s |
|---|---|---|---|---|---|
| run_command | 1,631 | 853 | 274 | 273 | 231 |
| symbols | 1,047 | 832 | 78 | 136 | 1 |
| semantic_search | 91 | 62 | 24 | 4 | 1 |
| workspace | 163 | 157 | 4 | 2 | 0 |
| edit_code | 226 | 192 | 33 | 1 | 0 |
| librarian_reindex | 5 | 0 | 0 | 5 | 0 |

### Slow Commands

| Latency (ms) | Command |
|---|---|
| 300,012 | `RUSTFLAGS="" cargo build --release 2>&1 \| tail -10` |
| 266,793 | `cargo clean && cargo build --release 2>&1 \| tail -5` |
| 235,166 | `cargo build --release` |
| 146,051 | `cargo build --release 2>&1` |
| 140,753 | `cargo build --release 2>&1 \| tail -5` |
| 137,783 | `RUSTFLAGS="" cargo test 2>&1 \| tail -40` |
| 127,286 | `cargo build --release 2>&1 \| tail -5` |
| 127,275 | `cargo test 2>&1` |
| 123,344 | `cargo build --release` |
| 112,641 | `cargo test 2>&1 \| tail -5` |
| 108,434 | `cargo check 2>&1 \| tail -3` |
| 106,039 | `RUSTFLAGS="" cargo clippy -- -D warnings 2>&1 \| tail -20` |

### Session Summary (top 15)

| Session | Calls | Errors | Tools Used |
|---|---|---|---|
| e3a3430b | 561 | 23 | read_markdown, read_file, symbols, edit_code, edit_file, run_command, grep, tree, create_file, workspace, semantic_search, edit_markdown |
| cd580050 | 406 | 10 | workspace, read_markdown, read_file, symbols, edit_file, create_file, edit_code, run_command, grep, edit_markdown, semantic_search |
| 4529ad6a | 340 | 10 | read_markdown, symbols, read_file, edit_code, run_command, edit_file, artifact_find, artifact_get, edit_markdown, tree, create_file, workspace, grep |
| e4b9338d | 334 | 10 | read_markdown, workspace, symbols, grep, edit_code, run_command, read_file, edit_file, semantic_search |
| 19862929 | 319 | 17 | read_markdown, read_file, symbols, edit_code, edit_file, grep, run_command, workspace, tree, create_file, edit_markdown |
| 0e3361ef | 290 | 15 | read_markdown, run_command, symbols, read_file, edit_file, create_file, grep, tree, call_graph, edit_code, memory |
| a6c5f4e8 | 276 | 19 | workspace, read_markdown, symbols, grep, create_file, run_command, edit_file, edit_code, read_file, edit_markdown |
| 9c893a88 | 268 | 7 | read_markdown, artifact_find, grep, run_command, artifact_update, edit_markdown, symbols, read_file, edit_code, workspace, edit_file |
| 849e7ff5 | 234 | 3 | workspace, read_markdown, symbols, grep, edit_code, run_command, read_file |
| f4558224 | 173 | 5 | symbols, workspace, run_command, grep, tree, edit_code, read_file, edit_file, read_markdown, edit_markdown |

### LSP Events

| Language | Reason | Starts | Avg Handshake (ms) | Max Handshake (ms) | Avg First Response (ms) |
|---|---|---|---|---|---|
| kotlin | lru_evicted | 1 | 4,135 | 4,135 | — |
| rust | lru_evicted | 15 | 16 | 17 | 5 |
| rust | new_session | 14 | 15 | 16 | 4 |

---

## Project: backend-kotlin

**DB:** `/home/marius/work/mirela/backend-kotlin/.codescout/usage.db`

### Overview

| Metric | Value |
|---|---|
| Total calls | 1,745 |
| Error rate | 5.3% |
| Date range | 2026-05-01 → 2026-05-03 |
| Sessions | 25 |

### Tool Popularity

| Tool | Calls | Avg ms | Max ms | Errors | Overflows |
|---|---|---|---|---|---|
| run_command | 608 | 3,647 | 300,013 | 11 | 0 |
| read_file | 237 | 0 | 8 | 20 | 2 |
| symbols | 235 | 1,067 | 43,904 | 4 | 0 |
| read_markdown | 188 | 0 | 0 | 3 | 0 |
| grep | 159 | 3 | 208 | 0 | 24 |
| memory | 89 | 1 | 214 | 0 | 0 |
| tree | 64 | 0 | 5 | 0 | 1 |
| edit_file | 55 | 0 | 17 | 22 | 0 |
| workspace | 53 | 17 | 18 | 0 | 0 |
| edit_markdown | 27 | 36 | 100 | 0 | 0 |
| edit_code | 18 | 24 | 26 | 3 | 0 |

### Error Breakdown

| Tool | Error | Count |
|---|---|---|
| edit_file | blocked — structural/source/markdown | 20 |
| read_file | Use read_markdown for markdown files | 10 |
| run_command | shell access to source files is blocked | 8 |
| read_markdown | heading not found / file not found | 3 |
| edit_code | failed edits | 3 |
| semantic_search | model mismatch | 1 |

### Overflow Tools

| Tool | Overflow Calls |
|---|---|
| grep | 24 |
| read_file | 2 |
| tree | 1 |

### Slow Commands

| Latency (ms) | Command |
|---|---|
| 300,013 | `python3 scripts/test_chat.py 2>&1 \| tail -40` |
| 300,009 | `cd python-services && python3 ../scripts/test_chat.py 2>&1` |
| 147,490 | `cd python-services && python -m intent_classifier.training.train --stage a 2>&1 \| tail -10` |
| 120,017 | `sleep 120 && tail -10 @bg_0000000d` |
| 118,318 | `cd python-services && python -m intent_classifier.training.train --stage b 2>&1 \| tail -10` |
| 57,942 | `cd python-services && python -m intent_classifier.training.export_onnx ...` |
| 44,279 | `cd ktor-server && ./gradlew test 2>&1 \| tail -20` |
| 43,180 | `cd ktor-server && ./gradlew compileKotlin 2>&1 \| tail -15` |

### LSP Events

| Language | Reason | Starts | Avg Handshake (ms) | Max Handshake (ms) |
|---|---|---|---|---|
| javascript | new_session | 2 | 86 | 89 |
| typescript | new_session | 2 | 86 | 85 |
| rust | new_session | 2 | 16 | 16 |

---

## Project: MRV-poc

**DB:** `/home/marius/work/stefanini/southpole/MRV-poc/.codescout/usage.db`

### Overview

| Metric | Value |
|---|---|
| Total calls | 1,108 |
| Error rate | 2.1% |
| Date range | 2026-05-01 → 2026-05-03 |
| Sessions | 9 |

### Tool Popularity

| Tool | Calls | Avg ms | Max ms | Errors | Overflows |
|---|---|---|---|---|---|
| run_command | 593 | 9,823 | 786,295 | 9 | 0 |
| symbols | 111 | 226 | 6,398 | 0 | 0 |
| read_file | 77 | 0 | 0 | 1 | 0 |
| grep | 73 | 2 | 30 | 0 | 0 |

### Error Breakdown

| Tool | Error | Count |
|---|---|---|
| run_command | timeout / process errors | 9 |

### Slow Commands

| Latency (ms) | Command |
|---|---|
| 786,295 | `uv run python scripts/_audit_v2_misses.py --fixture ... --pool 300` |
| 600,013 | `uv run python scripts/eval_chunking.py -k 10 --rerank --bm25 ...` |
| 600,011 | `uv run python scripts/eval_chunking.py ...` |
| 300,012 | `uv run python scripts/_train_lora_v5.py --phase perm_mine` |
| 288,992 | `uv run python scripts/_train_lora_v5.py --phase perm_train` |
| 178,382 | `uv run python scripts/eval_chunking.py -k 10 --rerank ...` |

### LSP Events

| Language | Reason | Starts | Avg Handshake (ms) | Max Handshake (ms) | Avg First Response (ms) |
|---|---|---|---|---|---|
| python | new_session | 21 | 442 | 754 | 585 |
| python | idle_evicted | 10 | 369 | 383 | 276 |

---

## Project: researcher

**DB:** `/home/marius/work/claude/researcher/.codescout/usage.db`

### Overview

| Metric | Value |
|---|---|
| Total calls | 456 |
| Error rate | 3.1% |
| Date range | 2026-05-02 → 2026-05-03 |
| Sessions | 6 |

### Tool Popularity

| Tool | Calls | Avg ms | Max ms | Errors |
|---|---|---|---|---|
| symbols | 185 | 4 | 100 | 0 |
| run_command | 102 | 1,322 | 58,010 | 5 |
| read_file | 45 | 0 | 3 | 5 |
| read_markdown | 42 | 0 | 0 | 2 |
| grep | 24 | 2 | 30 | 0 |
| workspace | 18 | 6 | 35 | 0 |

### LSP Events

| Language | Reason | Starts | Avg Handshake (ms) |
|---|---|---|---|
| javascript | new_session | 3 | 86 |
| typescript | new_session | 3 | 82 |

---

## Project: claude-plugins

**DB:** `/home/marius/work/claude/claude-plugins/.codescout/usage.db`

### Overview

| Metric | Value |
|---|---|
| Total calls | 443 |
| Error rate | 8.1% |
| Date range | 2026-05-02 |
| Sessions | 12 |

### Tool Popularity (top)

| Tool | Calls | Errors |
|---|---|---|
| symbols | ~120 | 1 |
| run_command | ~85 | 5 |
| read_file | ~60 | 12 |
| edit_file | ~50 | 18 |
| grep | ~40 | 0 |
| edit_markdown | ~30 | 0 |

---

## Project: eduplanner-ui

**DB:** `/home/marius/work/mirela/eduplanner-ui/.codescout/usage.db`

### Overview

| Metric | Value |
|---|---|
| Total calls | 348 |
| Error rate | 4.6% |
| Date range | 2026-05-01 → 2026-05-03 |
| Sessions | 8 |

---

## Project: librarian-mcp

**DB:** `/home/marius/work/claude/code-explorer/crates/librarian-mcp/.codescout/usage.db`

### Overview

| Metric | Value |
|---|---|
| Total calls | 121 |
| Error rate | 6.6% |
| Date range | 2026-05-02 |
| Sessions | 2 |

### Tool Popularity

| Tool | Calls | Avg ms | Max ms | Errors | Overflows |
|---|---|---|---|---|---|
| symbols | 35 | 205 | 6,066 | 1 | 0 |
| grep | 24 | 0 | 2 | 0 | 1 |
| read_file | 21 | 0 | 4 | 3 | 0 |
| run_command | 20 | 2,962 | 23,409 | 2 | 0 |
| workspace | 7 | 17 | 18 | 0 | 0 |
| tree | 5 | 0 | 0 | 0 | 0 |

### Error Breakdown

| Tool | Error | Count |
|---|---|---|
| run_command | shell access to source files is blocked | 2 |
| edit_file | multi-line edit contains symbol definition — use symbol tools | 1 |
| read_file | Use read_markdown for markdown files | 1 |
| read_file | file not found | 2 |
| semantic_search | model mismatch (local vs CodeRankEmbed) | 1 |

### Slow Commands

| Latency (ms) | Command |
|---|---|
| 23,409 | `cargo build -p librarian-mcp 2>&1` |
| 14,889 | `cargo test -p librarian-mcp -- merge_params 2>&1` |
| 14,096 | `cargo clippy -p librarian-mcp -- -D warnings 2>&1` |

---

## Project: playground

**DB:** `/home/marius/work/claude/playground/.codescout/usage.db`

### Overview

| Metric | Value |
|---|---|
| Total calls | 150 |
| Error rate | 6.0% |
| Date range | 2026-05-02 → 2026-05-03 |
| Sessions | 7 |

### Error Breakdown

| Tool | Error | Count |
|---|---|---|
| run_command | shell access to source files is blocked | 4 |
| read_file | Use read_markdown for markdown files | 2 |
| edit_file | write denied — outside project root | 2 |
| read_file | file not found | 1 |

### Slow Commands

| Latency (ms) | Command |
|---|---|
| 18,157 | `find / -name "claude" -type f -executable 2>/dev/null \| grep -v proc \| head -5` |

---

## Smaller Projects

Projects with <100 calls. Abbreviated — overview + notable errors only.

| Project | Calls | Error % | Sessions | Notable |
|---|---|---|---|---|
| optaplanner | 65 | 12.3% | 2 | `edit_markdown` write-denied to cross-project paths (backend-kotlin, eduplanner-mobile) |
| eduplanner-site | 67 | 10.4% | 2 | memory topics missing — 5 architecture/conventions topics not found |
| deployment | 86 | 3.5% | ~5 | memory topics missing (architecture, conventions) |
| eduplanner-mobile | 23 | 8.7% | 2 | `edit_file` blocked ×2 |
| verra-mrv-scraper | 34 | 0% | 1 | clean |
| bk/python-services | 22 | 27.3% | 1 | all errors = file-not-found using monorepo paths from wrong root |
| P-C AI Project | 19 | 0% | 1 | clean |
| workspace-mcp | 14 | 7.1% | 1 | `edit_file` → edit_markdown redirect |
| bk/eduplanner-mcp | 7 | 0% | 2 | clean |
| bk/ktor-server | 6 | 0% | 1 | clean |
| ionut/hotel | 3 | 0% | 1 | clean |
| rust-library fixture | 5 | 0% | 1 | clean |
| claude (root) | 9 | 0% | 1 | clean |
| mirela (root) | 1 | 0% | 1 | clean |
