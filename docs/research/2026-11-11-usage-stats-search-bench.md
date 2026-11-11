---
kind: research
status: active
title: "Usage statistics — codescout grep vs rg vs git grep benchmark"
tags:
  - usage-stats
  - frictions
  - grep
---

Archived analyst report from the 2026-11-11 statistics pass. The summary and the decisions are in [the statistics tracker](../trackers/frictions/statistics.md). The scripts named below are in `docs/research/data/2026-11-11-usage-stats-scripts.tar.gz`. They read pickles and a scratch directory that no longer exist, so treat them as method, not as a rerunnable job.

# codescout `grep` vs `rg` vs `git grep` through `run_command` - benchmark report

Repo: /home/marius/work/claude/codescout (6366 tracked files; `rg --files` = 6830; `rg --files --hidden` = 11967; `rg --files --hidden --no-ignore` = 335112; 701 untracked non-ignored files; `target/` is 95 GB and gitignored). Machine: 64 cores, ripgrep 15.2.0, git 2.56.0, hyperfine 2.0.0. Date 2026-11-11 (UTC 00:28 - 02:30). All numbers are WARM-cache (page cache not droppable).

All numbers below were produced by scripts in `/home/marius/.cache/cc-tmp/claude-1000/-home-marius-work-claude-codescout/2e6121be-ee04-4e7a-974f-3717997500f4/scratchpad/` (prefix `bench_`; raw JSON in `timing/`, `hf/`, `parity2/`, `tok/`, `lp/`). Section 10 lists them.

## 0. Method, and two deviations the reader must know

1. **How the "agent-facing" calls were issued.** I could not script my own MCP session's tool calls (7 repeats x ~50 combos x 4 variants = thousands of calls). `bench_mcp2.py` spawns a second `codescout start -p <repo>` over stdio (same binary, `/home/marius/.cargo/bin/codescout` -> `target/release/codescout`, built 2026-11-10 21:24), calls `workspace(activate)`, sleeps 30 s (settle), then issues the same `grep` and `run_command` tool calls the agent would. The calls are logged in `usage.db` like any other; `bench_collect.py` joins each call to its `latency_ms` by (tool, args) within the driver's session. Without the 30 s settle, the first calls of a fresh process were 2-3 s slower than later ones (startup background work); with it, they are not. Each call alternates tools within a round (cs-lines, cs-files, rg, git grep) so contention is shared.
2. **`latency_ms` is not what the agent waits.** Measured: a `read_file` of a 1-line file has `latency_ms` = 1 but the client waited a median 87 ms and the server process burned 65 ms CPU for the call (`bench_ovh_report.py`); `run_command echo hi` has `latency_ms` 59.5 vs client wall 157 ms. So ~85 ms per call sits outside `latency_ms` for every tool (cause NOT determined - hypothesis: post-call recording/response work; not investigated). Both are reported in the raw JSON (`wall_ms`, `latency_ms`); tables below use `latency_ms` as asked. Fast calls (13 ms) are really ~100 ms to the client.
3. **`usage.db` drops rows under contention** (BONUS): in batch `repo b` (load spiking to 54) 5 of 368 calls had no row (`GROUP MISMATCH calls=8 rows=7 ...`); `bench_collect.py` leaves such groups unjoined. That batch is excluded.
4. `run_command` used `effect=read`, bare output (no `head`), so nothing was bounded for rg/git grep. Output bounding is therefore NOT changing the work. For cs, defaults (limit 50) - which DO change the work (see 3.3).
5. Hyperfine: `-i --warmup 2 --runs 15 --output pipe` (pipe, not /dev/null, because hyperfine warns grep-likes optimise for /dev/null), default shell (`-N` not used, per instruction). `-i` was needed because zero-match runs exit 1.

### Load log (1-minute loadavg before -> after; threshold 12)

The box was saturated at the start (other analysts: gate test runs, a `git_bench`, milvus). I waited for quiet windows (`bench_run_batch.sh` polls /proc/loadavg up to a limit, then runs and records `uptime` before/after).

| batch | before -> after | status |
|---|---|---|
| src a / src b | 78.5 -> 45.6 / 87.4 -> 38.2 | DISCARDED (>12) - medians ran 1.16-1.28x the quiet ones (`bench_ratios.py`) |
| repo a | 14.9 -> 13.5 | discarded (>12) |
| repo b | 8.3 -> 54.5 | DISCARDED (load spike mid-run, 5 usage rows dropped) |
| src c / repo c | 11.5 -> 8.4 / 7.5 -> 10.1 | valid (<12), used only as the second sample |
| globrs a / docs a / reg a | 11.4 -> 11.2 / 11.5 -> 8.3 / 10.0 -> 9.7 | valid, second sample |
| **src q / repo q / globrs q / docs q / reg q (PRIMARY)** | 1.5 -> 3.6 / 3.6 -> 4.9 / 4.9 -> 4.8 / 4.8 -> 3.4 / 7.0 -> 5.4 | valid; all <= 7.0 at start, <= 4.9 at end |
| hyperfine src q / repo q / globrs q / docs q / reg q (PRIMARY) | 2.80 -> 2.98 / 2.98 -> 6.08 / 6.08 -> 8.88 / 8.57 -> 8.14 / 8.14 -> 8.01 | valid |
| hyperfine reg a (older) | 10.65 -> 14.67 | discarded (>12) at end; reg b 11.82 -> 11.58 valid, not used |
| parity, token, gate, CPU, overhead experiments | 17.5 -> 17.4 (parity), 7.8 -> 5.7 (token/gate), 2.7 -> 3.8 (CPU), 2.2 (overhead), 1.8 -> 1.5 (hyp) | parity is not timing-sensitive; the rest valid |

Run-to-run agreement: between the primary (q) batch and the second valid batch the median cell differs by 7% (204 cells compared, `bench_tables.py`); 7 cells differ by more than 50% (mostly registry cells, where the page cache and cs's 20 s walk budget interact).

## 1. How `src/tools/grep.rs` works (cites file:line, read in full)

Parallel or serial: **serial**. `ignore::WalkBuilder::new(&search_path)` is built at `grep.rs:293`, configured `hidden(!include_hidden).git_ignore(true)` (294), glob overrides added through `OverrideBuilder` (295-311), and `wb.build()` (312) - the sequential iterator, not `build_parallel`. Both modes (`if files_mode {` at 317, its loop `clock.expired` at 332; lines loop `'outer: for entry in walker` at 390, `clock.expired` at 399) iterate that one iterator on the async task's thread, one file at a time. `require_git` is left at the crate default (true) and WalkBuilder defaults also give `.ignore`, parent and global gitignore, no symlink following. `.rgignore` is not a WalkBuilder default, so it is NOT honoured (measured: section 4).

File reading: whole-file `std::fs::File` read into a `Vec<u8>` via `read_path_unless_binary` (123-127) / `read_unless_binary` (97-121): sniff the first `BINARY_SNIFF_BYTES = 8192` (70); a NUL in that window = binary, skipped and counted (`skipped_binary`); a declared length over `MAX_FILE_BYTES = 64 MiB` (80) = skipped and counted (`skipped_large`; the rendered text names it and suggests `rg` via `run_command`). No mmap. Then `String::from_utf8_lossy(&bytes)` (356, 423) and `text.lines()` with `regex::Regex::is_match` per line (357, 427). A NUL after byte 8192 does NOT make a file binary (measured: section 4, `nul_late.txt`).

Regex: `build_grep_regex` (1046-1083): `regex::RegexBuilder`, `size_limit`/`dfa_size_limit` 1 MiB, case-insensitive flag, `whole_word` wraps as `\b(?:...)\b`. No look-around (error text in section 6), no multiline (`refuse_native_keys_that_change_the_result`, 796-818). An invalid pattern that does not look regex-like is silently re-compiled as an escaped literal (`is_regex_like`, 1061): measured `foo(` and `a{2` returned `0 matches` with no mention of the fallback.

Symbol enclosure: **tree-sitter, not LSP, and a second read**. Only when `context_lines == 0 && !hit_cap` (block starting at 525, `// Index-aware hits`): for every matched file the file is re-read with `std::fs::read_to_string` and parsed with `crate::ast::parser::extract_symbols_from_source` (545), cached per file, then `enclosing_symbol` (1087-1095) picks the innermost range (551). Cost is therefore proportional to the number of distinct matched source files, and it is skipped when the collection cap fires (measured, section 3.2).

Caps (all measured to fire): `limit` default 50 (251: collection stops at `limit * COLLECTION_OVERSAMPLE = 4` = 200 candidate lines, so the walk stops early for common patterns); `MAX_MATCH_BYTES = 2000` per emitted line/block (1106); `MAX_TOTAL_MATCH_BYTES = 60_000` (1111) - when it fires the result says "Collection stopped at the output byte budget, so the true total is unknown ... a floor, not a count"; `time_budget_ms` default 20000 (`DEFAULT_WALK_BUDGET_MS`, 20; `WalkClock::expired` polled once per walk entry, 41-64) - fires as a "walk cut" notice (measured on the registry); 64 MiB per file. `mode=files` has no match cap and no symbol step, only the walk budget. `cap_grouped` (585) then keeps up to `limit` matches with file diversity and groups by file; the formatted text names the top-3 narrowing paths.

## 2. Patterns, scopes, exact commands

Source of truth: `bench_common.py`. Patterns (label -> pattern): P01 rare, definition in 1 file `fn clamp_match`; P02 common identifier `RecoverableError` (3104 lines/626 files); P03 multi-word `wall-clock budget` (18/10); P04 alternation `RecoverableError|anyhow::bail` (3293/675); P05 anchored `^pub fn` (1319/357); P06 case-insensitive `recoverableerror` (-i); P07 whole-word `Agent` (-w); P08 zero matches `zzqxnomatchzz`; P09 huge `fn ` (25357 lines/1179 files); P10 PCRE look-around `(?<=pub )fn \w+` (rg/git only); P11 `Result<Value>` (316/150); P12 `TODO|FIXME`. Scopes: repo (no path), `src/`, glob `*.rs` (repo-wide), `docs/`, and the non-git corpus `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f` (3.5 GB, 163,487 files) with 8 registry patterns Q1-Q8 (MAP_NORESERVE, `unsafe impl Send`, `TODO|FIXME`, `wall-clock`, zero, `RecoverableError`, -i, PCRE) - P09/P10-style huge outputs were left out of the registry run_command set to avoid multi-hundred-MB captures. Registry rows use `git -C <reg> grep --no-index`, NOT tracked-only semantics.

Command templates (`rg_cmd`, `gg_cmd`, `cs_args`): `rg -n [-i] [-w] [-P] -e PAT [-g '*.rs'] [path]`; `git grep -n [-i] [-w] (-E|-P) -e PAT [-- pathspec]`; `grep(pattern=PAT, [path=..], [glob=..], [ignore_case], [whole_word], [mode="files"])`. Note `git grep -E` is required for alternation (BRE treats `|` literally). All 12 + 8 patterns and the 5 scopes were run.

## 3. Timing

### 3.1 Headline tables (PRIMARY q batches; usage.db latency_ms for agent-facing; hyperfine wall for raw)

Columns are `median / min / p90` in ms. cs = codescout `grep`; "via run_command" = `rg ...`/`git grep ...` through the codescout `run_command` tool (7 repeats after 1 warm-up for all; registry cs rows have only 2 repeats because a call takes 7-20 s). Raw = hyperfine, 15 runs after 2 warm-ups. NOT equal work: `cs lines` stops walking after 200 candidate lines (limit 50 x 4) or 60 KB, so on common patterns it answers a smaller question; `cs files` always does the full walk and counts every matching line, which is the like-for-like comparison with `rg -n` (all lines).


#### Scope `repo`

| pat | cs lines med/min/p90 | cs files med/min/p90 | rg via run_command med/min/p90 | git grep via run_command med/min/p90 | rg raw med/min/p90 | git grep raw med/min/p90 | cs-lines / rg-run_command | cs-lines / rg raw |
|---|---|---|---|---|---|---|---|---|
| P01 rare (definition, 1 file) `fn clamp_match` | 304 / 300 / 306 | 282 / 277 / 285 | 78 / 76 / 78 | 143 / 133 / 145 | 18.3 / 17.3 / 19.6 | 79.3 / 76.0 / 82.1 | 3.90x | 16.6x |
| P02 common identifier `RecoverableError` | 13 / 12 / 14 | 281 / 277 / 285 | 80 / 80 / 86 | 154 / 147 / 157 | 19.2 / 17.8 / 20.1 | 87.0 / 84.2 / 91.5 | 0.16x | 0.7x |
| P03 multi-word text `wall-clock budget` | 454 / 443 / 456 | 281 / 276 / 290 | 78 / 77 / 81 | 139 / 134 / 154 | 18.3 / 17.4 / 19.6 | 78.6 / 76.6 / 85.9 | 5.82x | 24.8x |
| P04 alternation `RecoverableError|anyhow::bail` | 14 / 13 / 14 | 299 / 287 / 302 | 84 / 80 / 86 | 245 / 243 / 248 | 19.6 / 18.3 / 21.2 | 181.1 / 178.1 / 183.2 | 0.17x | 0.7x |
| P05 anchored regex `^pub fn` | 108 / 106 / 111 | 279 / 273 / 282 | 81 / 80 / 81 | 191 / 190 / 194 | 18.6 / 17.5 / 19.9 | 134.4 / 131.8 / 138.8 | 1.33x | 5.8x |
| P06 case-insensitive `recoverableerror` | 15 / 14 / 15 | 314 / 312 / 322 | 85 / 83 / 86 | 158 / 155 / 161 | 20.2 / 18.6 / 21.3 | 92.1 / 89.0 / 95.4 | 0.18x | 0.7x |
| P07 whole-word `Agent` | 15 / 13 / 15 | 306 / 304 / 311 | 83 / 81 / 86 | 151 / 145 / 157 | 18.2 / 17.1 / 19.6 | 84.4 / 80.5 / 89.1 | 0.18x | 0.8x |
| P08 zero matches `zzqxnomatchzz` | 282 / 275 / 284 | 280 / 277 / 282 | 210 / 207 / 213 | 276 / 263 / 287 | 18.0 / 16.8 / 19.4 | 80.0 / 75.5 / 83.5 | 1.34x | 15.7x |
| P09 huge result `fn ` | 5 / 5 / 6 | 284 / 281 / 291 | 296 / 291 / 303 | 382 / 376 / 391 | 19.3 / 18.3 / 22.5 | 92.5 / 85.5 / 96.2 | 0.02x | 0.3x |
| P10 PCRE look-around `(?<=pub )fn \w+` | rejected (look-around) | - | 102 / 95 / 106 | 148 / 141 / 153 | 35.8 / 28.8 / 38.0 | 85.0 / 80.1 / 90.3 | - | - |
| P11 literal w/ metachar-ish `Result<Value>` | 238 / 234 / 240 | 280 / 278 / 287 | 82 / 81 / 85 | 145 / 138 / 147 | 17.8 / 16.9 / 19.1 | 80.3 / 75.5 / 87.4 | 2.90x | 13.4x |
| P12 alternation of words `TODO|FIXME` | 50 / 49 / 51 | 298 / 293 / 302 | 82 / 79 / 86 | 182 / 180 / 191 | 18.5 / 17.0 / 19.6 | 121.6 / 119.3 / 125.3 | 0.61x | 2.7x |

#### Scope `src`

| pat | cs lines med/min/p90 | cs files med/min/p90 | rg via run_command med/min/p90 | git grep via run_command med/min/p90 | rg raw med/min/p90 | git grep raw med/min/p90 | cs-lines / rg-run_command | cs-lines / rg raw |
|---|---|---|---|---|---|---|---|---|
| P01 rare (definition, 1 file) `fn clamp_match` | 60 / 58 / 62 | 41 / 40 / 42 | 68 / 67 / 69 | 83 / 76 / 85 | 8.4 / 6.8 / 9.4 | 22.8 / 14.9 / 27.8 | 0.88x | 7.2x |
| P02 common identifier `RecoverableError` | 11 / 11 / 12 | 40 / 39 / 43 | 68 / 66 / 69 | 83 / 77 / 86 | 8.4 / 7.4 / 8.6 | 25.6 / 19.3 / 29.7 | 0.16x | 1.3x |
| P03 multi-word text `wall-clock budget` | 211 / 205 / 215 | 40 / 39 / 40 | 70 / 68 / 71 | 83 / 77 / 84 | 8.3 / 7.1 / 8.9 | 21.5 / 15.6 / 27.2 | 3.01x | 25.4x |
| P04 alternation `RecoverableError|anyhow::bail` | 11 / 11 / 11 | 43 / 42 / 43 | 69 / 68 / 71 | 88 / 85 / 93 | 8.5 / 7.4 / 9.8 | 27.4 / 21.5 / 32.2 | 0.16x | 1.3x |
| P05 anchored regex `^pub fn` | 16 / 16 / 16 | 40 / 39 / 42 | 70 / 69 / 71 | 84 / 83 / 92 | 8.3 / 7.3 / 9.2 | 25.9 / 20.4 / 30.4 | 0.23x | 1.9x |
| P06 case-insensitive `recoverableerror` | 14 / 12 / 16 | 47 / 46 / 54 | 72 / 69 / 80 | 83 / 78 / 90 | 8.7 / 7.8 / 8.8 | 23.5 / 16.8 / 28.3 | 0.19x | 1.6x |
| P07 whole-word `Agent` | 27 / 26 / 28 | 47 / 46 / 49 | 72 / 71 / 75 | 87 / 82 / 97 | 8.4 / 7.1 / 9.2 | 21.4 / 18.4 / 25.3 | 0.38x | 3.2x |
| P08 zero matches `zzqxnomatchzz` | 42 / 41 / 42 | 41 / 40 / 41 | 203 / 199 / 207 | 215 / 208 / 218 | 8.2 / 7.0 / 8.9 | 24.2 / 18.9 / 28.1 | 0.21x | 5.1x |
| P09 huge result `fn ` | 3 / 3 / 3 | 42 / 40 / 42 | 75 / 74 / 76 | 88 / 86 / 93 | 9.3 / 8.2 / 10.1 | 24.4 / 16.8 / 28.6 | 0.04x | 0.3x |
| P10 PCRE look-around `(?<=pub )fn \w+` | rejected (look-around) | - | 74 / 73 / 75 | 84 / 82 / 88 | 10.9 / 10.3 / 11.8 | 23.3 / 18.8 / 27.7 | - | - |
| P11 literal w/ metachar-ish `Result<Value>` | 585 / 576 / 595 | 41 / 40 / 42 | 74 / 71 / 76 | 88 / 83 / 91 | 8.6 / 7.4 / 9.5 | 24.8 / 16.4 / 28.9 | 7.91x | 67.8x |
| P12 alternation of words `TODO|FIXME` | 142 / 140 / 145 | 43 / 43 / 44 | 72 / 71 / 78 | 88 / 82 / 91 | 8.3 / 7.4 / 10.1 | 23.5 / 17.6 / 26.8 | 1.97x | 17.2x |

#### Scope `globrs`

| pat | cs lines med/min/p90 | cs files med/min/p90 | rg via run_command med/min/p90 | git grep via run_command med/min/p90 | rg raw med/min/p90 | git grep raw med/min/p90 | cs-lines / rg-run_command | cs-lines / rg raw |
|---|---|---|---|---|---|---|---|---|
| P01 rare (definition, 1 file) `fn clamp_match` | 76 / 75 / 77 | 56 / 54 / 56 | 68 / 67 / 72 | 83 / 78 / 85 | 9.4 / 8.1 / 9.6 | 22.1 / 16.1 / 26.5 | 1.12x | 8.1x |
| P02 common identifier `RecoverableError` | 21 / 20 / 21 | 56 / 55 / 57 | 70 / 69 / 76 | 81 / 75 / 87 | 9.4 / 8.4 / 10.2 | 23.0 / 17.3 / 26.8 | 0.30x | 2.2x |
| P03 multi-word text `wall-clock budget` | 222 / 218 / 232 | 55 / 54 / 59 | 72 / 69 / 72 | 83 / 81 / 85 | 9.2 / 8.2 / 9.7 | 23.9 / 15.9 / 29.2 | 3.08x | 24.0x |
| P04 alternation `RecoverableError|anyhow::bail` | 20 / 19 / 22 | 58 / 56 / 61 | 71 / 70 / 72 | 86 / 81 / 91 | 9.5 / 8.1 / 9.8 | 27.6 / 23.6 / 31.8 | 0.28x | 2.1x |
| P05 anchored regex `^pub fn` | 27 / 26 / 27 | 56 / 55 / 59 | 72 / 70 / 72 | 84 / 79 / 93 | 9.7 / 8.7 / 11.4 | 24.7 / 19.7 / 29.7 | 0.38x | 2.8x |
| P06 case-insensitive `recoverableerror` | 23 / 22 / 24 | 63 / 61 / 66 | 72 / 71 / 73 | 82 / 77 / 90 | 10.1 / 9.2 / 10.5 | 24.8 / 17.7 / 29.7 | 0.32x | 2.3x |
| P07 whole-word `Agent` | 36 / 35 / 38 | 62 / 61 / 65 | 72 / 69 / 72 | 86 / 82 / 87 | 9.8 / 9.4 / 10.4 | 25.7 / 20.8 / 27.5 | 0.50x | 3.7x |
| P08 zero matches `zzqxnomatchzz` | 56 / 55 / 56 | 55 / 54 / 56 | 197 / 195 / 200 | 211 / 201 / 220 | 9.4 / 8.1 / 10.5 | 24.5 / 18.3 / 28.7 | 0.28x | 6.0x |
| P09 huge result `fn ` | 11 / 10 / 11 | 57 / 57 / 58 | 77 / 75 / 82 | 90 / 84 / 94 | 10.8 / 10.0 / 12.4 | 24.4 / 18.8 / 27.0 | 0.14x | 1.0x |
| P10 PCRE look-around `(?<=pub )fn \w+` | rejected (look-around) | - | 76 / 75 / 76 | 86 / 81 / 91 | 12.5 / 11.5 / 13.4 | 27.0 / 21.1 / 28.5 | - | - |
| P11 literal w/ metachar-ish `Result<Value>` | 624 / 610 / 633 | 58 / 55 / 60 | 73 / 71 / 75 | 82 / 77 / 85 | 9.6 / 8.3 / 10.2 | 24.6 / 20.3 / 27.2 | 8.55x | 65.0x |
| P12 alternation of words `TODO|FIXME` | 166 / 164 / 169 | 59 / 58 / 61 | 73 / 70 / 74 | 87 / 81 / 91 | 9.5 / 8.7 / 10.2 | 23.0 / 19.9 / 25.5 | 2.27x | 17.4x |

#### Scope `docs`

| pat | cs lines med/min/p90 | cs files med/min/p90 | rg via run_command med/min/p90 | git grep via run_command med/min/p90 | rg raw med/min/p90 | git grep raw med/min/p90 | cs-lines / rg-run_command | cs-lines / rg raw |
|---|---|---|---|---|---|---|---|---|
| P01 rare (definition, 1 file) `fn clamp_match` | 225 / 222 / 228 | 224 / 221 / 227 | 206 / 204 / 214 | 263 / 254 / 269 | 17.2 / 15.8 / 18.3 | 77.6 / 64.2 / 85.8 | 1.09x | 13.1x |
| P02 common identifier `RecoverableError` | 12 / 12 / 13 | 222 / 218 / 224 | 79 / 77 / 81 | 140 / 134 / 147 | 16.9 / 15.5 / 18.3 | 81.3 / 69.8 / 84.2 | 0.15x | 0.7x |
| P03 multi-word text `wall-clock budget` | 223 / 220 / 228 | 226 / 218 / 234 | 77 / 74 / 78 | 137 / 130 / 164 | 16.5 / 15.0 / 18.2 | 76.0 / 67.3 / 79.9 | 2.90x | 13.5x |
| P04 alternation `RecoverableError|anyhow::bail` | 13 / 12 / 13 | 233 / 228 / 239 | 80 / 76 / 82 | 198 / 194 / 202 | 17.6 / 16.8 / 18.5 | 138.4 / 133.7 / 141.7 | 0.16x | 0.7x |
| P05 anchored regex `^pub fn` | 106 / 104 / 113 | 217 / 216 / 221 | 77 / 75 / 81 | 167 / 163 / 169 | 17.0 / 15.8 / 19.5 | 109.4 / 100.6 / 112.9 | 1.38x | 6.2x |
| P06 case-insensitive `recoverableerror` | 14 / 13 / 14 | 248 / 243 / 251 | 82 / 78 / 83 | 139 / 135 / 152 | 18.3 / 17.1 / 19.1 | 82.8 / 73.0 / 85.0 | 0.17x | 0.8x |
| P07 whole-word `Agent` | 13 / 12 / 13 | 239 / 237 / 247 | 78 / 77 / 79 | 138 / 129 / 144 | 16.5 / 15.6 / 17.4 | 77.1 / 68.7 / 82.1 | 0.17x | 0.8x |
| P08 zero matches `zzqxnomatchzz` | 224 / 220 / 233 | 221 / 219 / 224 | 205 / 203 / 207 | 265 / 257 / 272 | 16.5 / 15.5 / 17.1 | 76.1 / 64.5 / 79.9 | 1.09x | 13.6x |
| P09 huge result `fn ` | 42 / 39 / 43 | 220 / 218 / 232 | 248 / 242 / 260 | 308 / 297 / 323 | 17.1 / 16.5 / 18.1 | 79.0 / 70.7 / 86.7 | 0.17x | 2.5x |
| P10 PCRE look-around `(?<=pub )fn \w+` | rejected (look-around) | - | 93 / 91 / 96 | 138 / 132 / 144 | 31.2 / 25.0 / 33.7 | 77.0 / 73.1 / 82.3 | - | - |
| P11 literal w/ metachar-ish `Result<Value>` | 259 / 254 / 264 | 225 / 220 / 230 | 78 / 77 / 80 | 134 / 126 / 139 | 16.8 / 15.2 / 18.0 | 77.7 / 65.9 / 82.7 | 3.32x | 15.4x |
| P12 alternation of words `TODO|FIXME` | 50 / 48 / 52 | 232 / 231 / 238 | 78 / 76 / 80 | 170 / 166 / 174 | 17.2 / 15.7 / 18.9 | 112.0 / 100.1 / 115.6 | 0.64x | 2.9x |

#### Scope `reg`

| pat | cs lines med/min/p90 | cs files med/min/p90 | rg via run_command med/min/p90 | git grep via run_command med/min/p90 | rg raw med/min/p90 | git grep raw med/min/p90 | cs-lines / rg-run_command | cs-lines / rg raw |
|---|---|---|---|---|---|---|---|---|
| Q1 registry: rare-ish (496 files) `MAP_NORESERVE` | 1586 / 1581 / 1591 | 6904 / 6903 / 6904 | 258 / 254 / 263 | 1458 / 1431 / 1471 | 200.5 / 197.7 / 202.9 | 1475.8 / 1460.3 / 1492.2 | 6.15x | 7.9x |
| Q2 registry: multi-word common `unsafe impl Send` | 1416 / 1405 / 1425 | 6886 / 6850 / 6916 | 263 / 259 / 265 | 1459 / 1420 / 1492 | 201.8 / 197.1 / 204.2 | 1479.7 / 1464.3 / 1497.3 | 5.38x | 7.0x |
| Q3 registry: alternation (12301 files) `TODO|FIXME` | 74 / 72 / 76 | 7451 / 7409 / 7485 | 562 / 558 / 576 | 1911 / 1891 / 1925 | 229.9 / 225.7 / 232.4 | 1715.5 / 1699.3 / 1741.8 | 0.13x | 0.3x |
| Q4 registry: text `wall-clock` | 7168 / 7084 / 7234 | 6930 / 6906 / 6950 | 261 / 255 / 264 | 1450 / 1431 / 1457 | 200.0 / 196.3 / 203.6 | 1478.2 / 1452.1 / 1504.3 | 27.46x | 35.8x |
| Q5 registry: zero `zzqxnomatchzz` | 6776 / 6762 / 6787 | 6803 / 6791 / 6813 | 390 / 385 / 397 | 1570 / 1539 / 1608 | 198.9 / 195.7 / 202.8 | 1476.6 / 1449.2 / 1496.1 | 17.37x | 34.1x |
| Q6 registry: rare (33 files) `RecoverableError` | 3662 / 3648 / 3674 | 6862 / 6838 / 6882 | 262 / 257 / 269 | 1440 / 1424 / 1525 | 198.6 / 196.9 / 202.8 | 1477.7 / 1465.4 / 1508.3 | 13.98x | 18.4x |
| Q7 registry: case-insens `recoverableerror` | 3242 / 3228 / 3252 | 7738 / 7709 / 7762 | 269 / 267 / 272 | 1440 / 1433 / 1467 | 210.3 / 208.7 / 212.4 | 1487.8 / 1461.9 / 1532.3 | 12.05x | 15.4x |
| Q8 registry: PCRE `(?<=unsafe )impl Send` | rejected (look-around) | - | 620 / 610 / 628 | 1447 / 1440 / 1463 | 557.6 / 546.6 / 563.7 | 1483.9 / 1464.6 / 1502.6 | - | - |

### 3.2 What explains the cs-lines/cs-files gap (natural experiment, load 1.8 -> 1.5, `bench_out_hyp`)

`context_lines=1` takes the other branch of the call (no symbol step) with otherwise identical walking.

| combo | cs lines (default) | cs lines context_lines=1 | cs files |
|---|---|---|---|
| `Result<Value>` src/ | 579 ms | 18 ms | 40 ms |
| `wall-clock budget` repo | 450 ms | 287 ms | 280 ms |
| `^pub fn` src/ | 15 ms | 5 ms | 39 ms |
| `RecoverableError` repo | 12 ms | 12 ms | 278 ms |
| `TODO|FIXME` repo | 49 ms | 43 ms | 298 ms |

So the tree-sitter symbol step costs ~560 ms for 74 matched files (P11 src) and ~170 ms for P03: MEASURED. The 12 ms rows are the early-exit (200-candidate cap) - the same pattern with mode=files walks everything (278 ms).

### 3.3 CPU cost (cs process + children; `/proc/<pid>/stat` utime+stime+cutime+cstime; 10 ms tick, summed over 7 calls / 3 for registry)

| combo | variant | wall ms/call (client, median) | server+child CPU ms/call (sum of ticks/n; 10 ms tick) | usage.db latency_ms median |
|---|---|---|---|---|
| P02|repo | cs-lines | 99 | 76 | 13 |
| P02|repo | cs-files | 365 | 340 | 283 |
| P02|repo | rg | 178 | 287 | 82 |
| P02|repo | gg | 256 | 634 | 155 |
| P03|repo | cs-lines | 536 | 506 | 457 |
| P03|repo | cs-files | 372 | 346 | 286 |
| P03|repo | rg | 177 | 273 | 80 |
| P03|repo | gg | 238 | 607 | 141 |
| P09|repo | cs-lines | 90 | 67 | 5 |
| P09|repo | cs-files | 366 | 339 | 282 |
| P09|repo | rg | 395 | 531 | 295 |
| P09|repo | gg | 480 | 886 | 385 |
| P08|repo | cs-lines | 365 | 343 | 284 |
| P08|repo | cs-files | 363 | 341 | 283 |
| P08|repo | rg | 305 | 413 | 210 |
| P08|repo | gg | 369 | 749 | 274 |
| P11|src | cs-lines | 667 | 637 | 580 |
| P11|src | cs-files | 125 | 100 | 40 |
| P11|src | rg | 170 | 174 | 73 |
| P11|src | gg | 183 | 224 | 86 |
| P05|src | cs-lines | 101 | 74 | 16 |
| P05|src | cs-files | 123 | 103 | 40 |
| P05|src | rg | 169 | 176 | 73 |
| P05|src | gg | 186 | 279 | 89 |
| Q5|reg | cs-lines | 6830 | 6743 | 6743 |
| Q5|reg | cs-files | 6901 | 6793 | 6808 |
| Q5|reg | rg | 501 | 2573 | 394 |
| Q5|reg | gg | 1672 | 5763 | 1576 |
| Q2|reg | cs-lines | 1504 | 1460 | 1418 |
| Q2|reg | cs-files | 6923 | 6843 | 6844 |
| Q2|reg | rg | 363 | 2460 | 268 |
| Q2|reg | gg | 1534 | 5670 | 1437 |

| combo | rg raw (hyperfine) wall med ms | rg raw CPU med ms (user+sys) | git grep raw wall med | git grep raw CPU med |
|---|---|---|---|---|
| P02 repo | 19.2 | 132.8 | 87.0 | 504.8 |
| P03 repo | 18.3 | 120.4 | 78.6 | 487.3 |
| P09 repo | 19.3 | 138.8 | 92.5 | 503.4 |
| P08 repo | 18.0 | 120.6 | 80.0 | 468.4 |
| P11 src | 8.6 | 17.8 | 24.8 | 64.8 |
| P05 src | 8.3 | 18.7 | 25.9 | 131.0 |
| Q5 reg | 198.9 | 2256.1 | 1476.6 | 6221.6 |
| Q2 reg | 201.8 | 2285.4 | 1479.7 | 6342.6 |

Reading it: cs (single-threaded) uses CPU about equal to its walk time (`cs files` repo: 340 ms CPU for 283 ms latency); rg uses ~133 ms CPU to finish in 19 ms wall (parallel) - 2.6x less CPU and 15x less wall than cs for the same walk; `git grep` (tracked files) uses MORE CPU than either (505 ms) but finishes in 87 ms; on the registry rg 2.26 s CPU/0.2 s wall, git grep --no-index 6.2 s CPU/1.48 s, cs 6.8 s CPU/6.9 s. The server CPU for a `cs lines` call that exits early (P02: 76 ms CPU, 13 ms latency) is dominated by the ~65 ms per-call overhead noted in section 0.

### 3.4 Other measured timing facts

- `run_command` floor: `true` 59 ms, `rg -n clamp_match src/tools/grep.rs` (1 hit) 61 ms. rg raw on src/ is 8 ms, so the tool adds ~55-60 ms.
- **Non-zero exit adds ~130 ms**: `false` 187 ms vs `true` 59 ms; `rg` with no hit on one file 190 ms vs hit 61 ms (`bench_out_exit`). This is why P08 (zero matches) rg/git grep through run_command are 200-280 ms while raw is 18 ms. Cause not investigated (hypothesis: exit-code handling path).
- Huge captured output costs: `rg -n 'fn '` repo (3.1 MB out) 296 ms via run_command vs 19 ms raw.
- Registry cs walk: `cs files` 6.8-7.7 s for a full 3.5 GB / 163k-file walk; the first registry parity attempt, at load 70-95, hit the 20 s budget ("walk cut" in the response) on all 7 non-PCRE patterns (client wall 20.1-29.5 s per call). With `time_budget_ms=300000` and quiet box it completes in ~7 s and equals rg's counts.

Slower-than-2x flags (`bench_flags.json`; cs lines vs rg via run_command, medians): repo P01 3.9x, P03 5.8x, P11 2.9x; src P03 3.0x, P11 7.9x; globrs P03 3.1x, P11 8.6x, P12 2.3x; docs P03 2.9x, P11 3.3x; registry Q1 6.2x, Q2 5.4x, Q4 27.5x, Q5 17.4x, Q6 14.0x, Q7 12.1x. `cs files` vs rg via run_command is >2x slower in repo scope for 9 of 11 patterns (3.4-3.7x), in docs scope for 8 (2.8-3.1x), and 13-29x on the registry; it is faster in src/ and globrs scopes (ratios 0.59 and 0.80 medians).
Per-scope medians of the ratio cs-lines / rg(run_command): repo 0.61, src 0.23, globrs 0.38, docs 0.64, registry 12.05; cs-lines is slower than rg(run_command) in 5/11, 3/11, 4/11, 5/11 and 6/7 patterns. git grep via run_command / rg via run_command: 1.83 repo, 1.19 src, 1.16 globrs, 1.74 docs, 5.42 registry.

Causes: single-threaded walk - MEASURED (CPU ~ wall; hyperfine shows rg using 7x more CPU than wall); symbol enclosure - MEASURED (3.2); "whole-file read + from_utf8_lossy + per-line regex versus rg's SIMD/memchr line finder and mmap" and "docs/ jsonl corpus dominates the walk (docs-only cs-files 222 ms of 280 ms repo)" - HYPOTHESES (the docs fraction is measured, the mechanism is not).

## 4. Result parity

Method: `bench_parity.py` snapshots rg and git grep result sets (A), runs codescout `mode=files` (full per-file counts, `parity2/`) through the driver, snapshots again (B) and only accepts a combo if A == B. An earlier pass with the codescout run hours before the rg run showed a 3-line difference in `src/usage/db.rs` that turned out to be another session editing that file (drift), so everything below is the drift-controlled pass. All 51 non-PCRE combos were stable (no UNSTABLE flag).

Result, per (pattern, scope):
- **codescout vs rg: identical in all 51 combos** (same files, same per-file line counts; 0 only-rg, 0 only-cs, 0 count differences), including case-insensitive, whole-word (`\b(?:..)\b` vs `-w`), glob `*.rs`, docs/ and the registry. The (file,line) sets of lines mode were also equal in 5 spot checks (`bench_lines_parity.py`: `fn clamp_match` 1, `wall-clock budget` 18, `Result<Value>` src 134, `TODO|FIXME` src 10, `Agent` -w in src/tools 323); a 6th (`TODO|FIXME` repo-wide, limit=1000) returned only 67 of rg's 304 because the 60 KB byte budget fired - and said so ("67 matches (capped) ... a floor, not a count").
- **git grep vs rg/cs differs in repo-wide scopes** (examples: `RecoverableError` repo: rg 3104 lines/626 files, git grep 5574/647, cs 3104/626; `Agent -w`: rg 1851, git grep 1938; `fn `: rg 25357, git grep 25396; registry `TODO|FIXME` `--no-index`: rg 31804, git grep 31934). In `src/` and `*.rs` they agree except for untracked files.

Cause classification (`bench_classify.py`, union over all repo-scope combos): every file only git grep finds (52 files) is **tracked + hidden** (e.g. `.buddy/memory/INDEX.md`, `.codescout/audit/ripper-65e654-202609.jsonl`, `.codescout/memories/gotchas.md`); every file only rg/cs find (16 files) is **untracked, not ignored** (e.g. `docs/research/2026-11-11-lsp-capability-audit.md`, `docs/issues/2026-11-07-oracle-capture-undeclared-tests.md`, `docs/research/data/2026-11-06-tmp-exec-diagnosis.json`). `rg --hidden` also searches `.git/` (found `.git/COMMIT_MSG_INTEGRATION`); gitignored dirs (`target`, `.worktrees`, `.claude/worktrees`, confirmed by `git check-ignore -v`) are skipped by rg/cs even with hidden on, so nested worktrees do not leak. Registry: git grep `--no-index` ignores `.gitignore` and searches hidden crate files, rg/cs honour ignore files and skip hidden; 4555 files were counted binary-skipped by cs.

Five concrete files that exactly one tool misses, from a controlled corpus built for this (`bench_corpus/`, a throwaway git repo in the scratchpad; `bench_mkcorpus.py`; pattern `NEEDLE`):

| file | rg (default) | git grep (tracked) | codescout (default) | why |
|---|---|---|---|---|
| `.hidden.txt` (tracked dotfile) | MISSES | finds | MISSES (needs include_hidden) | hidden |
| `tracked_ignored.txt` (tracked, then in .gitignore) | MISSES | finds | MISSES | rg/ignore honour .gitignore even for tracked files |
| `untracked.txt` | finds | MISSES (needs --untracked) | finds | untracked |
| `rgignored.txt` (listed in `.rgignore`) | MISSES | finds | **finds** | `.rgignore` is rg-only; codescout walks with the `ignore` crate defaults and does not read it |
| `nul_late.txt` (NUL at byte 9618) | MISSES (no lines in a recursive walk; named explicitly it prints `binary file matches (found "\0" byte around offset 9618)` on stderr) | finds both lines | **finds both lines** (line 602 contains the NUL) | cs sniffs only the first 8192 bytes; rg treats NUL anywhere in its first buffer as binary |
| `big.txt` (70 MiB) | finds | finds | **MISSES, says so** ("1 file(s) over 64 MiB were not searched ... run `rg` on it through `run_command`") | MAX_FILE_BYTES |
| `nul_early.bin` | MISSES | prints `Binary file nul_early.bin matches` (no line) | MISSES ("1 binary file(s) skipped") | binary |
| `ignored.txt`, `ignore_dot.txt` (`.gitignore` / `.ignore`) | MISSES | finds `ignore_dot.txt` (tracked), not `ignored.txt` (untracked) | MISSES | ignore files |
| `link_file.txt` / `link_dir/` (symlinks) | skipped (found only with `rg -L`) | greps the link blob, no match | skipped | symlinks not followed |
| `nested/n.txt` (embedded git repo, untracked) | finds | MISSES | finds | nested repo is just a directory to rg/cs |

With a glob the picture changes identically for rg and cs: `rg -g '*.txt'` and `grep(glob="*.txt", mode="files")` both re-admitted gitignored and hidden files in the corpus (`ignored.txt`, `ignore_dot.txt`, `tracked_ignored.txt`, `.hidden.txt` appeared in both), because glob overrides outrank ignore files. (In the repo, all 11 non-PCRE glob-scope combos matched cs vs rg exactly.) Invalid/unsupported inputs: `(?<=x)NEEDLE` -> error (section 6); multiline `line1\nNEEDLE` -> `0 matches` with a hidden-path warning (searched line by line); `foo(` and `a{2` -> `0 matches` (literal fallback, not announced).

**Which agrees with an agent's expectation in this repo:** codescout grep and `rg` agree exactly (hidden skipped, gitignore honoured, untracked searched); `git grep` is the odd one - it searches 52 tracked hidden files an agent's `docs/`-style search would not expect, and it misses 16 untracked files a fresh doc may live in (the repo is edited by several agents with uncommitted files, so this matters here).

## 5. Token cost = output size

`bench_tok_report.py`; resp_B = bytes of the tool response the agent sees (median of repeats after warm-up, so no first-call banner); full_out_B = what the command printed in full (rg/git grep: measured by running the command; cs when buffered: size of the drained `@tool_` JSON). ~tok = resp_B / 4.

| search | variant | resp_B | ~tok | full_out_B | lines | buffered? |
|---|---|---|---|---|---|---|
| S1 `fn clamp_match` (1 hit) | cs lines | 231 | 58 | 231 | - | no |
| | cs files | 229 | 57 | 229 | - | no |
| | rg -n | 96 | 24 | 59 | 1 | no |
| | rg --vimgrep | 98 | 24 | 61 | 1 | no |
| | rg -l / -c | 55 / 57 | 14 / 14 | 18 / 20 | 1 | no |
| | git grep -n | 96 | 24 | 59 | 1 | no |
| S2 `wall-clock budget` (18 lines/10 files) | cs lines | 4094 | 1024 | 4094 | - | no |
| | cs files | 761 | 190 | 761 | - | no |
| | rg -n / --vimgrep | 4045 / 4100 | 1011 / 1025 | 3982 / 4037 | 18 | no |
| | rg -l / -c | 513 / 533 | 128 / 133 | 467 / 487 | 10 | no |
| | git grep -n | 4598 | 1150 | 4534 | 19 | no |
| S3 `Result<Value>` src/ (134 lines/74 files) | cs lines (50 shown) | 6625 | 1656 | 6625 | - | no |
| | cs files | 2845 | 711 | 2845 | - | no |
| | rg -n / --vimgrep | 2088 / 2088 | 522 / 522 | 12781 / 13167 | 134 | YES |
| | rg -l / -c | 2265 / 2413 | 566 / 603 | 2155 / 2303 | 74 | no |
| | git grep -n | 2088 | 522 | 12781 | 134 | YES |
| S4 `RecoverableError` repo (3104 lines/626 files) | cs lines (limit 50) | 3286 | 822 | 45534 | - | YES |
| | cs files | 3307 | 827 | 68994 | - | YES |
| | rg -n / --vimgrep | 2086 / 2086 | 522 / 522 | **1,887,422 / 2,254,382** | 3104 / 3189 | YES |
| | rg -l / -c | 1545 / 1702 | 386 / 426 | 39229 / 40568 | 626 | YES |
| | git grep -n | 2086 | 522 | **3,063,270** | 5574 | YES |
| S5 `^pub fn` src/ (612 lines/158 files) | cs lines (50 shown) | 5548 | 1387 | 5548 | - | no |
| | cs files | 5772 | 1443 | 5772 | - | no |
| | rg -n / --vimgrep | 2088 / 2088 | 522 / 522 | 54586 / 55810 | 612 | YES |
| | rg -l / -c | 4519 / 4851 | 1130 / 1213 | 4325 / 4657 | 158 | no |
| | git grep -n | 2088 | 522 | 54586 | 612 | YES |
| S6 `fn ` repo (25357 lines/1179 files) | cs lines (50 shown) | 5438 | 1360 | 5438 | - | no |
| | cs files | 3300 | 825 | 120734 | - | YES |
| | rg -n / --vimgrep | 2085 / 2086 | 521 / 522 | **3,117,156 / 3,599,916** | 25357 / 26231 | YES |
| | rg -l / -c | 1613 / 1618 | 403 / 404 | 64488 / 67421 | 1179 | YES |
| | git grep -n | 2085 | 521 | **3,841,889** | 25397 | YES |

Findings:
- **Per shown match, codescout is not smaller.** S3: cs 6625 B for 50 shown matches = 132 B/match vs rg 12781 B/134 = 95 B/match; S5: 111 vs 89 B/match. The grouping saves the repeated path only when several matches share a file; here most files have 1 match, and the file heading, ` (N)` tally and `[enclosing symbol]` suffix cost more than the path prefix they replace. Every lines-mode response also carries a fixed header (matches/files tally, narrowing hints naming the top-3 files with counts, the hidden-paths warning ~170 B): S2 4094 B vs rg 4045 B for the same 18 hits.
- **Per search, codescout is bounded and rg is not** (full output): S4 rg -n = 1.89 MB (~470k tokens) because jsonl docs have enormous single lines; cs clamps each line to 2000 B and the collection to 60 KB total, so its full buffer is 45.5 KB (11k tokens). git grep -n is worse still (3.06 MB) because it also searches 52 tracked hidden files.
- **What the agent actually sees inline is capped for both**: when output exceeds the ~10 KB inline threshold both tools return a ~2 KB envelope plus a handle. cs returns `{output_id: "@tool_..", summary}` where the summary has "N matches in M files", the cap reason, narrowing candidates with counts and the first matches (3286 B for S4). `run_command` returns the first ~1.9 KB of stdout, a `--- N lines omitted ---` marker around a head/tail window, `stdout: 1900 of 1887422 bytes shown; all of it: output_id` and an `@cmd_` handle (2086 B for S4); the full stream stays in the buffer and costs nothing until it is queried (`grep PATTERN @cmd_x`).
- **versus `rg | head`**: `rg -n "fn " src/ | head -50` is allowed and returns the 50 lines (4286 B) AND an `unfiltered_output` handle (up to 4 MiB of the producer stream; 15588 lines retained for that call) - so the pipe form trims context and keeps the buffer; but when a `;`, `&&` or `||` follows the pipe the gate refuses (section 6) and nothing is kept. cs has no equivalent of "give me everything": at the cap the matches beyond the first 200 candidates / 60 KB are never collected (S4: "61 matches (capped) in 12 files" for a pattern with 3104 matches).
- `rg -l`/`-c` and `cs mode=files` are the cheap shape: `rg -l` is 386-566 tokens even on 626 files; `cs files` inline is 190-1443 tokens but in S6 buffered 120 KB.
- No `⚠ Reads are resolving against ... linked git worktrees` banner after an explicit `workspace(activate)`; without an explicit activation every call (grep and run_command) carried a ~2.2 KB banner listing the repo's linked worktrees (seen in my first driver version: 2436 B vs 231 B for the same one-hit grep) - roughly 550 tokens per call for an agent that never activates.

## 6. Features only one side has, and the gates

Only rg / git grep (cs refuses or lacks): look-around and backrefs via `-P` (cs: `invalid regex: regex parse error: ... look-around, including look-ahead and look-behind, is not supported`, hint `the regex engine has no look-around (?!...) (?<!...) (?=...) (?<=...) - use two greps, or rewrite the pattern without it`; measured through the tool, ok:false); multiline `-U` (`multiline: true` -> `'multiline' is not supported - grep matches one line at a time ... Nothing was searched.`, run through the tool); `--type`, `-z`, `--stats` (G15 ran), `-m`/`--max-count`, `--sort`, `--max-columns`, `-g '!pat'` negation (cs `glob` takes gitignore-syntax globs; negation not tested), `-L` symlink following, `--no-ignore`/`-u`, `-r` replace, `-o`, `-v`, `-w` is available on both, **`git grep REV` / history** (G14 `git grep -n -E ... HEAD -- file` ran), tracked-only semantics, `--untracked`, `--no-index`; files over 64 MiB; (cs did search `~/.cargo/registry/src` and the scratchpad through an absolute `path`, so outside-project paths are not a limit).

Only cs: enclosing symbol on every source hit (tree-sitter, `[LibrarianAdapter/call]`-style suffix); explicit caps with notices (`capped`, "a floor, not a count", `skipped_binary`, `skipped_large`, `walk_cut`); the hidden-path warning (`Hidden paths at the search root were not searched: .buddy/, .cargo/, ... Pass include_hidden=true`) and a "this zero describes what was searched, not the pattern" warning on zero results; narrowing hints that name the top-3 files with their counts; buffer handles for the result and `grep` over `@cmd_/@tool_/@file_` buffers (`grep_in_buffer`, 1218); workspace pinning (`workspace=<abs path>` per call); time budget (`time_budget_ms`); alias repair for native-Grep spellings (`-i`, `-C/-A/-B`, `query`, `regex`); ranked per-file counts; schema-level refusal of `output_mode` values other than content/lines.

Gates when an agent runs rg/git grep through `run_command` (measured, `bench_out_gate*.json`):
- **Not refused or rewritten**: bare `rg -n ...` (output buffered when >10 KB), `rg -l`, `rg --files | head -5`, `rg -P` (allowed), `rg -U`, `rg --stats`, `rg -m 3`, `rg -c --sort path`, `git grep -n/-P/-E ... HEAD`, `grep -rn ... src/` and `grep -rn ... src/ | head`, `rg ... > file`, `rg ... | sort | uniq -c | sort -rn | head`, `rg ... | grep pub | head -3`, `rg ... | cut -c1-60`, `git grep -n ... | wc -l`, `rg ... | wc -l` (these pipe forms return an `unfiltered_output` handle to the producer's full stream, 15588 lines in the examples).
- **Refused**: a pipe followed by a list: `rg -n RecoverableError src/tools/grep.rs | head -3 && echo ok` and `... | head -3 ; echo done` -> `IL3 violation - piped ... to a log-trimmer, and the capture cannot be spliced into this command. BLOCKED.` with fixes "(a) rewrite it as one plain pipeline ending in its filter (b) run_command("rg ...") bare, then query the @cmd_* buffer". The gate text lists `rg`/`fd`/`cargo`/recursive grep as unbounded producers, which is why a pipe needs a capture; its text also says `git` is unbounded only without an output limiter and lists `-n` among the limiters, so by that rule `git grep -n` counts as bounded (that is what the gate text says; I did not find a command it changes).
- A non-zero exit (no match) costs +130 ms (3.4) and returns `exit_code: 1`, not an error.
- Inline cap: results over ~10 KB are buffered (resp ~2 KB), see section 5.

## 7. Verdict (which tool per search class; evidence from this run)

| search class | use | why (measured) |
|---|---|---|
| Common identifier / `fn ` / alternation that has hundreds of hits, repo or src/ | codescout `grep` (lines) | 5-108 ms latency (5-50 ms except `^pub fn` repo-wide at 108) vs 68-296 ms for rg via run_command; response bounded (3-6 KB, <=60 KB buffered); gives enclosing symbols. Caveat: it is a capped sample (first ~200 candidates in walk order, not ranked) and the answer is "a floor". |
| Counting / per-file ranking | codescout `grep mode=files` in src/ (40 ms, same counts as rg), `rg -c`/`-l` anywhere else | cs files in repo scope 280 ms vs rg via run_command 78-83 ms (3.4-3.7x slower); parity exact |
| Rare or zero-hit search over the whole repo or docs/ (full walk needed) | `rg` via run_command | cs 280-454 ms vs rg 78 ms (hit) / 210 ms (miss, because of the non-zero-exit penalty); raw rg 18 ms. In src/ and `*.rs` the two are close (cs 41-76 vs rg 68-72 ms) unless many files match (symbol step) |
| Small source set with a few matches in many files (P11/P03/P12 classes) | `rg`, or codescout with `context_lines=1`/`mode=files` if symbols are not needed | cs lines 142-624 ms (symbol enclosure, measured) vs rg 70-82 ms: 2.0-8.6x slower |
| Large non-git corpus (~/.cargo/registry, 3.5 GB) | `rg` | rg 258-562 ms via run_command (0.2 s raw) vs cs 1.4-7.2 s lines / 6.8-7.7 s files (and a 20 s walk cut under heavy load) vs git grep --no-index 1.4-1.9 s; also 2.7-2.8x less CPU than either (2.26 s vs 6.2-6.8 s) |
| Look-around, multiline, `-P`, `--type`, `-z`, `-v`, replace | `rg -P` / `-U` / git grep -P | cs refuses (tested) |
| History / a REV / tracked-only | `git grep` | only option; but it also searches 52 tracked hidden files and skips 16 untracked ones in this repo |
| Hidden but tracked state (`.codescout/memories`, `.buddy`) | `git grep`, or cs `include_hidden=true` | rg needs `--hidden` and then also reaches `.git/` |
| Huge single-line files (docs/evals jsonl) | codescout `grep` | clamps lines at 2000 B and collection at 60 KB; `rg -n` printed 1.89 MB (S4) |
| Files >64 MiB | `rg` through run_command | cs skips and says so |

Do NOT draw: "codescout grep is faster" in general. It is faster only when its cap ends the walk early; for equal work (`mode=files` vs full `rg`) it is 3.5x slower in repo scope, 0.6x in src/, 26x on the 3.5 GB corpus.

## 8. NOT VERIFIED

- Everything is warm-cache; cold-cache behaviour is unknown (the first registry rg probe took 3.7 s cold vs 0.2 s warm).
- The ~85 ms per-call overhead outside `latency_ms` was measured with the standalone driver's server process (not an interactive session) - its cause is unknown, and an interactive session with the companion hook may differ. Quiet-window numbers are from a box at load 1.5-9; under load 45-90 the same medians ran 1.16-1.28x (src) and registry walks hit the 20 s budget.
- The mechanism behind "non-zero exit costs +130 ms in run_command" is untested.
- Whole-file read vs rg's mmap/SIMD as the cause of the remaining walk gap, and the docs/jsonl share of the walk, are hypotheses.
- `rg -c`/`rg -l` via run_command were sized, not timed; `rg --vimgrep` likewise.
- Parity of the lines-mode (file,line) sets was checked for 5 searches, not all 51; the 51-combo parity is per-file line counts from `mode=files`.
- PCRE combos (P10, Q8) were timed for rg/git grep only; their results were not compared (cs rejects them).
- git grep `--untracked` / `rg --hidden` counts were taken but only summarised; `.rgignore`/`.ignore`/nested repo/symlink behaviours were measured on a throwaway corpus, not in the codescout repo.
- Negated globs (`-g '!x'`) and `--sort` were not run through cs.
- Registry results for cs have 2 repeats (3 in the CPU run), so their p90 is not meaningful.
- CPU numbers for cs/run_command use 10 ms ticks summed over 7 calls (resolution ~1.4 ms/call) and include the ~65 ms/call server overhead; hyperfine `time_cpu` is user+sys of the process tree.
- The usage-recorder row drops (5 of 368 calls under load 54) were seen once; frequency unknown.
- Whether `grep(pattern="foo(")`'s literal fallback is mentioned anywhere in the response was not checked beyond the two observed responses (none).

## 9. Ledger

Ledger checked: none (no bug ledger query was run; the usage.db rows I touched were only my own benchmark calls). Phase 0 memory reading was skipped because the task named the files to read; guides auto-injected by the tools were read.

## 10. Scripts and raw data

All under `/home/marius/.cache/cc-tmp/claude-1000/-home-marius-work-claude-codescout/2e6121be-ee04-4e7a-974f-3717997500f4/scratchpad/`:
`bench_mcp2.py` / `bench_mcp3.py` (stdio MCP driver; v3 adds per-call CPU ticks), `bench_collect.py` (joins usage.db latency_ms), `bench_common.py` (patterns, scopes, command builders), `bench_gen_timing.py` + `bench_run_batch.sh` (agent-facing batches with load wait; outputs `timing/t_<scope>_<run>.joined.json`, `timing/load_*.txt`), `bench_hf.py` (hyperfine; outputs `hf/hf_<scope>_<run>.json`, `hf/load_*.txt`), `bench_summ.py`, `bench_tables.py` -> `bench_tables.md`, `bench_cpu_report.py` -> `bench_cpu_tables.md`, `bench_ratios.py`, `bench_gen_parity2.py` + `bench_parity.py` (A/B drift-controlled parity -> `bench_parity_result.json`), `bench_classify.py`, `bench_lines_parity.py`, `bench_mkcorpus.py` (+ `bench_corpus/`), `bench_gen_tok.py` + `bench_tok_report.py`, gate specs `bench_spec_gate*.json` / outputs `bench_out_gate*.json`, `bench_out_hyp.joined.json`, `bench_out_exit.joined.json`, `bench_out_cpu.joined.json`, `bench_ovh_report.py`.
