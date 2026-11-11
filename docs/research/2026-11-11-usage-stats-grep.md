---
kind: research
status: active
title: "Usage statistics — the grep tool and its shell neighbours"
tags:
  - usage-stats
  - frictions
  - grep
---

Archived analyst report from the 2026-11-11 statistics pass. The summary and the decisions are in [the statistics tracker](../trackers/frictions/statistics.md). The scripts named below are in `docs/research/data/2026-11-11-usage-stats-scripts.tar.gz`. They read pickles and a scratch directory that no longer exist, so treat them as method, not as a rerunnable job.

# stats-grep: the `grep` tool and its shell neighbours

Corpus: `.codescout/usage.db`, `tool_calls`, `id <= 299175`, 2026-10-12..2026-11-10 UTC. Read-only.
Scripts (all in the scratchpad, prefix `grep_`): grep_common.py, grep_enrich.py (output parser), grep_q1.py, grep_q1b.py,
grep_q23.py (pattern classifier), grep_q3.py, grep_q3b.py, grep_q3c.py, grep_q4a.py (in-session buckets), grep_q4b.py (filesystem check),
grep_q4c.py, grep_q4d.py, grep_q4e.py, grep_q5.py, grep_q5b.py, grep_q6.py, grep_q6b.py, grep_q7.py, grep_q7b.py, grep_q7d.py, grep_q7e.py, grep_q7f.py, grep_shell.py (shell-command parser).
Pickles: grep_q4a.pkl (per-zero bucket), grep_q4b.pkl (per-zero filesystem verdict), grep_shell_calls.pkl, grep_shell_res.pkl.
Scratchpad = /home/marius/.cache/cc-tmp/claude-1000/-home-marius-work-claude-codescout/2e6121be-ee04-4e7a-974f-3717997500f4/scratchpad/

## 0. Coverage (state next to every number)

| slice | grep rows | non-empty input_json | non-empty output_json |
|---|---|---|---|
| all | 9,227 | 9,047 (98.0%) | 8,915 (96.6%) |
| parent (agent_id NULL) | 3,954 | 3,774 (95.4%) | 3,642 (92.1%) |
| subagent | 5,273 | 5,273 (100%) | 5,273 (100%) |

Reconciliation: the brief says 9,158 grep calls / 61 errors; at id<=299175 I count 9,227 rows / 64 non-success (the DB is live and my bound includes other analysts' greps). The brief's 443 bare zeros is 426 here (output_json exactly `[{"type":"text","text":"0 matches"}]`, 156 parent / 270 subagent, all with input).
Input missing: 180 rows (2026-10-13/14/21/23/27/28/29/30/31, 11-01..11-04), output missing 312 (mostly parent, 11-03..11-10). output_json looks truncated at ~20.2k chars (max length 20,252), so byte maxima are censored; p99 (10.3 kB) is below that.
Output shape: output_json is the formatted text, not the tool's JSON. 369 rows are `{output_id, summary, hint}` overflow envelopes (the summary is parsed instead). Single-file results have no "N matches" header, only `path (N)`; I parse both. 152 rows have no parseable header (errors, `{`...).
Splits by parent/subagent and by date change little for grep (zero rate 8.8% parent vs 7.5% subagent; weekly zero rate 7.6-9.7%; p50 latency 12 vs 18 ms); where a split matters it is stated.

## 1. Parameters (of 9,047 rows with input)

| parameter | rows | share | notes |
|---|---|---|---|
| pattern | 9,047 | 100% | length p50 40, p90 95, p99 217, max 2,221 chars |
| path | 7,489 | 82.8% | class: file 3,843 / subdir 2,464 / root 1,774 (incl. absent) / abs outside project 710 / buffer ref `@...` 256 |
| workspace | 4,424 | 48.9% | 773 parent rows, 3,651 subagent rows (subagents pin; 786 of them = main checkout, the rest worktrees) |
| context_lines | 4,181 | 46.2% | values: 2 (993), 3 (839), 1 (610), 0 (445), 6 (421), 4 (298), 12 (173), 8 (94), 14 (73), 30 (52) |
| glob | 2,736 | 30.2% | see below |
| limit | 2,290 | 25.3% | 30 (344), 20 (328), 40 (323), 60 (216), 80 (188), 12 (171), 10 (134), 15 (112), 6 (82), 100 (60), 8 (55), 25 (52). Default 50 is rarely typed. |
| mode (declared, lines\|files) | 1,796 | 19.9% | files 774, lines 536, **content 482 (not in enum, accepted as lines)**, count 4 |
| output_mode (NOT declared) | 1,622 | 17.9% | content 1,539 (accepted, harmless), count 32, files_with_matches 23, lines 14, files 14 |
| head_limit (NOT declared) | 778 | 8.6% | values 40/30/20/12/60, 40% as strings; silently ignored (section 8) |
| ignore_case | 299 | 3.3% | True 298 |
| include_hidden | 216 | 2.4% | True 204 |
| context (NOT declared; aliased) | 155 | 1.7% | |
| -n, -i, -C, -A, -B, -o | 44, 23, 17, 16, 3, 5 | <=0.5% each | `-i` -C -A -B `context` are aliased with a "corrected to" note (98 notes seen); -n -o are dropped silently |
| time_budget_ms | 11 | 0.1% | 1, 100 (x2), 15000, 20000, 60000 (x2), 120000-120003 (probe rows by an analyst) |
| whole_word | 3 | 0.03% | essentially unused |
| multiline | 3 | 0.03% | refused (2 errors) |
| offset, max_matches | 6, 3 | | undeclared, ignored |

glob shapes (2,736 rows with glob): string 2,421, array 315 (len 2: 64, 3: 73, 4: 60, 5: 43, 6: 22; max len 18). Contains `/`: 1,480 (54%); contains `**`: 1,050 (38%); brace set `{a,b}`: 341 (12%); negation `!...`: 137 (5%); leading `**/`: 72; plain `*.ext`: 880 (32%); absolute path: 0 detected as globs (the unsatisfiable-absolute-glob guard returns an error, so none reach success rows).
mode x output_mode: 19 rows pass both (e.g. mode=files + output_mode=content).

## 2. Pattern classes (measured; rule below)

Rule (grep_q23.py `classify`, first match wins, applied to the `pattern` string):
1. len > 150 -> very_long
2. contains an unescaped `|` outside `[...]` -> alternation; sub-class `alternation_literal` if, after deleting `\b`/`\B` and `\<punct>` escapes, no regex metacharacter `^$\[](){}+*?` remains, else `alternation_regex`
3. contains any of `^ $ \ [ ] ( ) { } + * ?` -> regex_meta
4. contains whitespace -> multiword_text
5. `^[A-Za-z_]\w*$` -> identifier
6. `/` or a file extension and only `[\w.\-*/]` -> path_like
7. `^[A-Za-z_][\w:.\-/]*$` -> qualified_identifier (e.g. `BL-65`, `nested-fence`)
8. else other_literal

Result (rows with input = 9,047; zero% is over rows that have output; overflow% over all rows of the class):

| class | n | share | zero% | overflowed% | capped% | latency p50 / p90 ms | output bytes p50 / p90 |
|---|---|---|---|---|---|---|---|
| alternation_literal | 3,936 | 43.5% | 6.7% (260) | 6.2% | 4.5% | 17 / 286 | 1,825 / 5,949 |
| alternation_regex | 2,153 | 23.8% | 8.4% (178) | 5.4% | 4.0% | 16 / 287 | 1,381 / 5,344 |
| regex_meta (no alternation) | 901 | 10.0% | 6.2% (56) | 4.0% | 3.8% | 10 / 237 | 1,254 / 4,557 |
| identifier (single word) | 765 | 8.5% | 13.9% (106) | 3.7% | 6.2% | 20 / 355 | 1,292 / 5,532 |
| multiword_text | 650 | 7.2% | 8.8% (56) | 1.4% | 2.0% | 6 / 193 | 1,048 / 3,064 |
| very_long (>150) | 244 | 2.7% | 2.1% (5) | 6.6% | 1.2% | 71 / 406 | 1,562 / 6,799 |
| qualified_identifier | 234 | 2.6% | 9.5% (22) | 3.4% | 5.6% | 16 / 660 | 1,384 / 4,480 |
| other_literal | 111 | 1.2% | 11.1% (12) | 3.6% | 9.3% | 24 / 275 | 890 / 4,895 |
| path_like | 53 | 0.6% | 17.0% (9) | 0.0% | 5.7% | 169 / 809 | 493 / 6,098 |

Correction of the prior estimate: 6,352 of 9,047 patterns (70.2%) contain a `|`; 4,716 contain a space (the "about 4,700 plain multi-word text" figure is this raw space count, and mostly alternations like `foo bar|baz`). Plain multi-word text (no `|`, no regex metacharacter) is **650 (7.2%)**: 258 parent, 392 subagent. Agents write alternations of literals: 43.5% of all greps are `a|b|c`-style identifier OR-lists, 6.1k alternations in total; single-identifier greps are 8.5%.
Identifiers (the class symbols/references would serve) have the highest zero rate of the large classes (13.9%) and path_like the highest overall (17%, n=53 only).

## 3. Result shape (rows with output; 8,915; parseable header 8,764)

| measure | value |
|---|---|
| zero ("0 matches", any trailer) | 704 = 7.9% of rows with output (parent 316, subagent 388) |
| of which exactly bare `0 matches` | 426 (60.5% of zeros) |
| of which with a scope warning/notice | 278: completeness_warning 252 (starved-glob clause 87, hidden clause 127 [+7 with another clause], gitignore clause 16-17, walk-error clause 22-26), stale-workspace preamble on a bare zero 17, walk-cut notice 2, skipped_large 1 |
| match-count buckets (n) | 0: 704, 1: 1,262, 2-5: 2,409, 6-20: 2,636, 21-50: 1,159, 51+: 594 (unparsed 151) |
| n percentiles | p50 6, p90 41, p99 215, max 45,418 (files-mode count) |
| capped header `N matches (capped)` | 382 rows (4.3%); `byte budget` cap rows 26; "floor, not a count" hint 98 |
| "Showing N of M" truncation | 540 rows (6.2% of rows with output) |
| hidden-paths note on a NON-zero result (`Hidden paths at the search root were not searched: ...`) | 288 of 8,060 non-zero rows (3.6%) |
| binary files skipped note | 146 rows |
| "walk cut:" notice (real, line-start) | 10 rows: 7 at ~20.0 s (ids 245666 255329 258240 262260 279892 294209 295234), 1 at 15 s (262748), 2 tests (238991 at time_budget_ms=1, 282421 at 100 ms) |
| literal fallback (invalid regex tried as literal) | 2 |
| stale-workspace `Reads are resolving against ...` preamble | 418 rows (383 parent), 40 of them on zeros |
| `X is not a parameter of grep - corrected to Y` note | 98 rows: context 52, -i 16, -C 15, -A 14, -B 1 |
| overflowed=1 | 473 of 9,227 = 5.1% (parent 201, subagent 272); 369 of them are envelope rows |
| overflow_tokens | n=473, p50 3,352, p90 6,747, max 235,205 |
| output_json bytes | p50 1,485, p90 5,476, p99 10,329, max 20,320 (censored); mean 2,288; total 20.4 MB over 8,915 rows |
| bytes by context | context_lines=0: p50 1,026 / p90 4,992; context_lines>0: p50 2,157 / p90 6,148 |

Zero rate by scope (rows with parseable output): path=file 4.6% (175/3,780); root 9.2% (158/1,724); subdir 11.8% (275/2,338); abs outside 11.1% (76/683); buffer 8.4%; with glob 10.5% vs without 7.0%; mode=files 15.2% (109/717) vs lines 7.4%; ignore_case=true 14.1% (41/290); include_hidden=true 9.9% (19/192). Weekly zero rate 7.6%-9.7%, no trend.

## 4. Zeros: true or false

Population: 704 zero rows, all with input. Principal = (session_id, agent_id), next 3 calls of any tool, ordered by id.

### 4a. In-session evidence (grep_q4a.py)
Rule: a follow-up counts as "related" if it is a grep (pattern), symbols (name) or a shell search command (run_command with grep/rg/git grep/find/fd) whose token set (words >=3 chars, minus stopwords) overlaps the zero's pattern tokens by >=1 token and >=34% of either set. Hit detection: grep not-zero; symbols not empty; run_command exit 0 with non-empty stdout (or a buffered output).

| bucket | all 704 | bare 426 | warned 278 | parent 316 | subagent 388 |
|---|---|---|---|---|---|
| SCOPE MISS: related grep with changed path/glob/include_hidden/workspace found hits | 81 (11.5%) | 39 | 42 | 33 | 48 |
| pattern miss: same scope, reformulated pattern or flags found hits | 18 (2.6%) | 14 | 4 | 7 | 11 |
| hit via another tool (run_command search 29, symbols 5) | 34 (4.8%) | 23 | 11 | 14 | 20 |
| => zero followed by a search that found hits | 133 (18.9%) | 76 | 57 | | |
| LIKELY TRUE: related search also empty | 38 (5.4%) | 18 | 20 | 22 | 16 |
| UNKNOWN: no related search in next 3 | 413 (58.7%) | 273 | 140 | 195 | 218 |
| UNKNOWN: related search but its output missing/errored | 116 (16.5%) | 57 | 59 | | |
| UNKNOWN: no next call | 4 | 2 | 2 | | |

What agents do next (first following call, all 704): grep 343, run_command 211, read_file 57, symbols 30, edit_file 22, doc 13.
By warning kind (zero rows; followed by hit-finding search = scope_miss+pattern_miss+other tool): bare 76/426 (17.8%); hidden clause 20/127 (15.7%); starved-glob clause 25/87 (28.7%); walk-error clause 4/22; gitignore clause 5/16.
Caveat: the "scope miss" label means the agent's next search widened scope and hit; it does not prove the first zero was wrong for the scope asked (see 4b: it was right). Sample of 14 scope-miss pairs read by hand (seed 7): all 14 widened or changed path/glob, but only 4 kept the identical pattern (e.g. 263489 `fn project_root_for` path=src/agent.rs, then path=src found it); 10 also rewrote the pattern (broader or different alternatives), so this bucket mixes pure scope misses with re-tries of a changed question.

### 4b. Filesystem check (grep_q4b.py): ALL 418 zero rows run in the main checkout, not a 40-row sample
284 further zeros ran in worktrees that no longer exist (not checkable) and 2 in a still-existing worktree.
Method per row, on the CURRENT tree (`rg` 15.2 in the same scope): (1) `rg --files` with the row's glob rooted at the row's path (the tool roots globs at `path`; plain `rg -g` anchors at the cwd, which first gave a wrong "false zero" class - fixed by running rg with cwd = the path dir) -> number of files admitted; (2) `rg -c -e PATTERN` in that scope (honouring ignore_case / whole_word / include_hidden); (3) widened `rg -l -i --hidden --no-ignore` over the repo minus .git/target/worktrees/.codescout/audit; (4) for matches in-scope, `git log --diff-filter=A` and `git log -1` on the file vs the call time, plus a pickaxe (`git log --since=<call> -S<literal>|-G<regex>`, `git diff HEAD`) to see whether the text changed after the call.

| verdict | rows | % of 418 | meaning |
|---|---|---|---|
| starved: scope admits no file now (glob rooted at path matches nothing) | 51 | 12.2% | **all 51 carried the "No file under ... passed the glob filter" warning at call time**; static count of the relative-glob-repeats-path-prefix shape over all 704 zeros: 61, all 61 warned |
| zero true for the scope; term exists elsewhere in code (src/tests/scripts/...) | 135 | 32.3% | scope guess too narrow, tool answer correct |
| zero true for the scope; term exists only in docs/records elsewhere | 56 | 13.4% | same |
| zero true for the scope; match only in hidden/gitignored files | 9 | 2.2% | include_hidden / gitignore class; 9/9 had no related follow-up |
| absent everywhere now (true zero) | 70 | 16.7% | 45 literal patterns pickaxed, 6 of 45 had the literal changed after the call (see below) |
| tree changed: match present now but literal added/removed after call | 36 | 8.6% | cannot say |
| path never in git history (ignored/untracked path) | 32 | 7.7% | cannot date |
| buffer path (`@...`) expired | 18 | 4.3% | not checkable |
| path missing now but in git history | 6 | 1.4% | tree changed |
| match is outside the repo (unversioned) | 4 | 1.0% | cannot date |
| **FALSE: stated scope contains the match now AND file untouched since the call** | **0** | 0% | one candidate (241483), but its matching files are untracked now, so unprovable |

Conclusion (labelled): in this repo and window, the grep tool did not return a provable false negative for the scope it was given. The zeros that waste turns are scope-guess misses (about half of the checkable zeros, 200/418 = 47.8%), and the tool's own warning caught every starved-glob zero that could be checked. The residual risk is the walk-cut/hidden-prune class, which the data cannot confirm (284 worktree zeros unchecked, 18 buffers).
Hidden-prune class: `completeness_warning` hidden clause appeared on 127 zeros; the example below shows the only starved zero whose cause was a hidden directory:

5 verbatim examples (row id | pattern | glob/path | verdict):
1. 186448 | `librarian|tracker` | path `src/prompts`, glob `src/prompts/*.md` | starved (glob repeats the path prefix), tool warned "No file under ... passed the glob filter"; follow-up in session unknown
2. 291400 | `"progressive-disclosure"` | path `src`, glob `src/**/*.rs` | starved, warned (same shape, 2026-11-09, after the ADR fix shipped: the shape still recurs)
3. 294826 | `call-graph-generic-call|fmt-mine-is-refused` | path none, glob `.codescout/memories/bug-triage-2026-11-09.md` | starved by hidden dir (no include_hidden); warned
4. 263489 | `fn project_root_for` | path `src/agent.rs` | zero true for scope (function lives elsewhere); next call grep path=`src` found it -> scope miss
5. 273804 | `MUTATION \(temporary\)` | path `src` | match exists only in `.buddy/.../cs_tool_log.jsonl` (hidden); next 3 calls had no related search
Also 257595 `fn per_user_mux_dir` (path src) absent everywhere now (true zero; follow-up reformulated and hit), 286323 `zzctl_two_unique` tree changed (analyst probe string later written into docs/issues).

Unsatisfiable-glob class (static, all 704 zeros): relative glob that starts with the path prefix: 61 rows, 61 warned (100%). Hidden-dir glob/path without include_hidden: 4 rows with glob `.codescout/...` (warned). Zeros with no warning and a starved scope: 0 found among the 418 checked (the 51 starved were all warned).
Warning coverage of zeros: completeness_warning present on 252/704 (35.8%); 426/704 (60.5%) bare. ADR clause 2 ("stay quiet on trustworthy"): the 426 bare zeros are quiet; of the 418 filesystem-checked rows 70 are demonstrably true zeros, 200 scope-guess misses that a bare zero cannot help with.

## 5. Latency (grep_q5.py, grep_q5b.py)

| slice | n | p50 | p90 | p99 | max | >=1 s |
|---|---|---|---|---|---|---|
| all grep | 9,227 | 16 ms | 292 | 1,651 | 386,711 | 220 (2.4%) |
| parent / subagent | 3,954 / 5,273 | 12 / 18 | 257 / 310 | 1,608 / 1,704 | | 2.4% / 2.4% |
| before / after 2026-10-30 (time budget commit 59d870c9d) | 4,970 / 4,257 | 16 / 15 | 291 / 292 | 1,487 / 1,712 | 386,711 / 20,009 | 2.0% / 2.8% |
| path = file | 3,843 | 2 | 25 | 144 | 1,459 | 0% |
| path = subdir | 2,464 | 54 | 348 | 1,220 | 2,966 | 1.5% |
| path = root/absent | 1,774 | 153 | 906 | 3,180 | 12,133 | 9.0% |
| root, no glob | 198 | 998 | 2,473 | 7,285 | 12,133 | 50.0% |
| root, with glob | 1,576 | 124 | 556 | 2,007 | 5,735 | 3.8% |
| abs path outside project | 710 | 5 | 146 | 20,001 | 386,711 | 3.0% |
| buffer | 256 | 1 | 12 | 51 | 108 | 0 |
| mode=files | 840 | 114 | 1,178 | 11,072 | 386,711 | 12.1% |
| mode=lines | 8,207 | 11 | 243 | 1,229 | 20,009 | 1.4% |
| include_hidden=true | 204 | 122 | 1,651 | 19,582 | 135,109 | 21.1% |
| context_lines=0 / >0 | 5,311 / 3,736 | 46 / 3 | 439 / 73 | 2,150 / 483 | | 3.7% / 0.5% |
| result zero / 1-5 / 6-50 / 51+ | | 16 / 5 / 27 / 152 | 283 / 143 / 315 / 742 | 6,766 / 1,065 / 1,639 / 3,730 | | 4.4 / 1.1 / 2.5 / 6.6% |
| overflowed | 473 | 38 | 665 | 3,440 | 386,711 | 6.6% |
(by pattern class: see section 2; by class p99: identifier 3,196, qualified_identifier 4,966, multiword 2,436; no class shows regex backtracking, the slowest rows are walks.)

Time budget (default 20,000 ms, `DEFAULT_WALK_BUDGET_MS`, commit 59d870c9d 2026-10-30): 10 rows end with the "walk cut" notice; 7 at ~20.0 s, every one with an absolute path ABOVE the project (`/home/marius/work/claude`, `/home/marius`, `/home/marius/work`, setup-ab-data, claude-plugins) and walks of 45k-110k files read before the cut. No row after the budget exceeded 20,009 ms. Three rows before the budget ran 99.8 s (226057, no input), 135.1 s (208396, `path=/home/marius`, `**/*.d.ts`, include_hidden, mode=files) and 386.7 s (210368, path `/home/marius/work/claude`, mode=files, 45,418 matches in 6,124 files). 33 rows are >= 3 s: 19 root scope, 13 absolute outside, 1 no input. Why slow: walk size (files read), not regex; the notice itself states files_read. Regex backtracking: Rust regex is non-backtracking, none observed (hypothesis, not tested).
Compare symbols (grep_q5b.py, 10,569 rows, 7,296 with input): name search 5,836 calls p50 23 ms, p90 738, p99 12,206, max 57,560; name+path 5,611 calls p50 20 ms; **name with no path (project-wide) 225 calls p50 10,583 ms, p90 12,335, 100% >=1 s**; path-only overview 1,458 calls p50 42 ms. Project-wide grep (root, no glob) p50 998 ms is 10x faster than project-wide symbols search at the median (different work: LSP workspace-symbol vs text walk; hypothesis for why agents grep identifiers).

## 6. Chains (grep_q6.py, grep_q6b.py; 9,047 grep rows with input, next 1-10 calls of the same principal)

| measure | value |
|---|---|
| first call after a grep | grep 2,764 (30.6%), run_command 2,028 (22.4%), read_file 1,956 (21.6%), symbols 827 (9.1%), edit_file 631 (7.0%), doc 220, edit_code 201, create_file 170 |
| non-zero result whose hit file is read/symbols/edited/referenced within next 3 calls (path match) | 2,857 of 7,452 parseable non-zero results = 38.3% |
| next 3 calls re-run the same pattern | 309 (3.4%); of those with a changed path/glob (narrowing/widening) 279 |
| same pattern as the immediately preceding grep (any params) | 231 |
| immediate repeat with byte-identical input (same principal, previous call grep) | **0** of 9,047 (surprising; not independently verified that the recorder does not collapse duplicates) |
| shell grep-type command in next 3 calls | not reported: my first regex counted `| grep` post-filters (1,821, invalid). Use section 7 for shell search counts. |
| identifier-class greps (765): followed by references/call_graph naming the identifier within 3 / 10 calls | 1 / 2 (0.1% / 0.3%): CONFIRMED rare |
| ... followed by symbols naming it within 3 | 17 (2.2%) |
| ... followed by an edit_file/edit_code whose input contains the identifier within 20 calls | 216 = 28.2%; placebo (identifier drawn at random from another identifier grep, 20 draws) 1.4%; |
| any edit within 20 calls after an identifier grep | 52.0% vs 48.3% base rate over all 118,278 calls (49.0% after any grep); so the edit signal is the identifier match, not "being in an edit phase" |

## 7. Substitution: shell searches in run_command (grep_q7*.py, grep_shell.py)

Coverage: run_command 51,365 rows; 42,055 with input; 35,202 with output. Subagent rows are complete; parent rows lack output for most `rg` calls (rg: 291 calls, only 61 with output). Parser: splits on unquoted `| ; && || newline`, shlex per segment, strips env/time/timeout/xargs/sudo; excludes my own commands (scratchpad paths). 9,781 calls contain a search-family segment. The shell parser is a heuristic.

### 7.1 Families, sessions, parent/subagent (call-level, priority git grep > rg > grep -r > grep FILE > find > fd > grep as filter > grep on @buffer)

| family | calls | % of run_command | sessions | parent | subagent | with output |
|---|---|---|---|---|---|---|
| git grep | 581 | 1.1% | 52 | 133 | 448 | 581 |
| rg | 291 | 0.6% | 29 | 237 | 54 | 61 |
| grep -r/-R (recursive) | 527 | 1.0% | 86 | 127 | 400 | 527 |
| grep on file args, not recursive | 4,333 | 8.4% | 202 | 1,602 | 2,731 | 4,321 |
| find | 397 | 0.8% | 96 | 255 | 142 | 307 |
| fd | 2 | 0.0% | 2 | 0 | 2 | 2 |
| grep as a pipe filter (`cmd | grep`) | 2,208 | 4.3% | 164 | 1,076 | 1,132 | 2,207 |
| grep on `@cmd_/@tool_/@file_/@bg_` buffers (sanctioned) | 1,442 | 2.8% | 165 | 1,003 | 439 | 1,398 |
Search-for-content calls that the grep tool could in principle replace (git grep + rg + grep -r + grep FILE) = 5,732 = 11.2% of all run_command calls; vs 9,227 grep tool calls.

Weekly trend (Monday start; run_command total | git grep | rg | grep -r | grep FILE | find | filter | buffer; grep tool calls):
| week | run_command | git grep | rg | grep -r | grep FILE | find | filter | buffer | grep tool |
|---|---|---|---|---|---|---|---|---|---|
| 10-12 | 11,220 | 299 | 42 | 163 | 1,838 | 64 | 680 | 303 | 3,041 |
| 10-19 | 5,065 | 28 | 9 | 89 | 640 | 43 | 271 | 158 | 1,222 |
| 10-26 | 14,552 | 161 | 8 | 98 | 880 | 109 | 586 | 378 | 2,090 |
| 11-02 | 13,965 | 37 | 99 | 113 | 699 | 119 | 458 | 389 | 2,111 |
| 11-09 (4 days) | 6,563 | 56 | 133 | 64 | 275 | 62 | 213 | 214 | 763 |
rg grew from 8 (week 10-26) to 99 and 133 (237 of 291 calls are parent-session calls: the human-driven main agent, not subagents). git grep is a subagent habit (448 of 581), peaking in the week of 10-12 (299).

### 7.2 Flag shares per family (share of calls of that family; flags parsed after shlex; short clusters expanded)
- git grep (581): pathspec after `--` 554 (95%), `-n` 503 (87%), `-E` 339 (58%), alternation pattern 304 (52%), **a revision argument 293 (50%)** (commit shas 15+ distinct, HEAD, branch names; parent 30, subagent 263), `-i` 147 (25%), `-F` 96 (17%), `-e` 71 (12%), `-c` 49 (8%), `-l` 48 (8%), `-A/-B/-C` 38 (7%), `-o` 27 (5%), `-I` 12, pathspec exclusion `':!path'` 158 (27%), `--cached` 2, `-w` 2, `-P` 1.
- rg (291): `-n` 187 (64%), alternation 163 (56%), `-g/--glob` 76 (26%), `--files` 64 (22%), `-m` 30 (10%), `-e` 27, `-A/-B/-C` 22 (8%), `--no-heading` 20, `-l` 14, `--no-ignore/-u` 10 (3%), `-i` 9, `--hidden` 8 (3%), `--type/-t` 7 (2%), `-P` 5 (2%), `-F` 4, `-U/-z` 3, `-o` 3, `-c` 2, `-r` replace 2, `-g '!x'` negation 12 (4%).
- grep -r (527): `-r` 100%, `-n` 378 (72%), `--include` 240 (46%), `-l` 238 (45%), alternation 234 (44%), `-E` 136 (26%), bare identifier 116 (22%), `--exclude-dir` 30 (6%), `-i` 36 (7%), `-F` 14, `-o` 13, `-A/-B/-C` 9, `-I` 22, `-w` 2, `-v` 1.
- grep FILE (4,333): alternation 2,414 (56%), `-n` 2,148 (50%), `-E` 1,865 (43%), `-c` 583 (13%), `-A/-B/-C` 499 (12%), bare identifier 379 (9%), `-i` 203 (5%), `-o` 184 (4%), `-m` 89 (2%), `-l` 79 (2%), `-v` 68 (2%), `-a` 69, `-H` 90, `-q` 21, `-P` 4 (look-around 1), `-w` 1.
- find (397): flags in expression: `-name` etc. not counted by my flag map; 106 (27%) use time/size/exec/printf (`-newermt`, `-mtime`, `-size`, `-exec`), 291 (73%) name/type/path only.
- grep as filter (2,208): `-E` 1,316 (60%), `-c` 279 (13%), `-i` 264 (12%), `-v` 225 (10%), `-n` 85; buffer greps (1,442): `-E` 1,057 (73%), `-n` 549 (38%), `-A/-B/-C` 124 (9%), `-c` 114 (8%), `-o` 65 (5%).

### 7.3 Result behaviour (calls with output_json)
| family | calls | with output | output bytes p50 / p90 / max | buffered (`output_id`) | overflowed | exit 1 (no match) | piped to head/tail | bounded by -m |
|---|---|---|---|---|---|---|---|---|
| git grep | 581 | 581 | 1,066 / 4,520 / 16,616 | 58 (10%) | 58 | 60 | 48 (8%) | 0 |
| rg | 291 | 61 | 1,835 / 4,533 / 8,340 | 6 (10%) | 43 | 3 | 8 (3%) | 32 |
| grep -r | 527 | 527 | 903 / 4,240 / 16,616 | 11 (2%) | 11 | 26 | 263 (50%) | 10 |
| grep FILE | 4,333 | 4,321 | 769 / 3,741 / 18,042 | 166 (4%) | 169 | 278 | 1,410 (33%) | 114 |
| find | 397 | 307 | 1,047 / 3,770 / 16,099 | 13 (4%) | 24 | 24 | 104 (26%) | 0 |
| grep filter | 2,208 | 2,207 | 736 / 2,971 / 18,765 | 77 (3%) | 77 | 148 | 808 (37%) | 22 |
| grep @buffer | 1,442 | 1,398 | 860 / 6,385 / 16,534 | 11 (1%) | 13 | 99 | 85 (6%) | 17 |
Outputs are the same size class as the grep tool (grep tool p50 1,485 B); recursive greps are bounded with `| head` half the time (the grep tool bounds itself via `limit`).

### 7.4 What only rg / git grep can do, and what the grep tool could serve (exclusive, heuristic, over 6,128 search-for-content + find calls; grep_q7e.py)
| category | calls | share | verdict |
|---|---|---|---|
| A. `git grep ... <rev>` search at a revision | 293 | 4.8% | grep tool has no revision parameter: UNSERVABLE (the clearest capability gap; 263 of 293 by subagents) |
| B. PCRE look-around/backref (`-P` with `(?<`, `(?=`, `(?!`, `\1`) | 1 | 0.0% | UNSERVABLE (Rust regex); `-P` otherwise appears 10 times and is mostly plain |
| C. multiline (`-U`, `-z`) | 3 | 0.0% | UNSERVABLE (tool refuses `multiline`) |
| D. lifts gitignore (`--no-ignore`, `-u`, `--no-index`, `--untracked`) | 10 | 0.2% | UNSERVABLE: the tool's gitignore clause says no argument lifts it |
| E. `find` by time/size/exec/printf | 106 | 1.7% | no tool |
| F. `find` by name/type/path | 291 | 4.7% | nearest is `tree` or grep mode=files (not a file-name search): capability gap |
| G. `rg --files` listing | 60 | 1.0% | no equivalent |
| H. output forms `-o`, `-v`, `-r`, `--json`, `--stats` | 295 | 4.8% | not offered |
| I. target inside a gitignored path | 316 | 5.2% | prune by gitignore: probably not served (uncertain for explicit roots) |
| J. target is an absolute path outside the repo (scratch/tmp 32% of grep-FILE calls, sibling repos 11%, ~/.claude* 3%) | 2,377 | 38.8% | grep(path=abs) can serve (710 grep rows do); why the shell is chosen is NOT VERIFIED |
| K. bare identifier / `fn|struct|def name` | 305 | 5.0% | symbols/references/grep serve |
| L. plain regex on a visible in-repo target | 2,071 | 33.8% | grep tool serves; `-n/-i/-E/-l/-c/-A/-B/-C/-m` map to context_lines/ignore_case/mode=files/limit |
Tool-only shapes that the grep tool CAN serve and the shell sometimes uses: `--type` (7 rg calls; glob `*.rs` serves), pathspec `--` (path/glob), `-g '!x'` and `':!x'` (grep glob negation `!x`), tracked-only scope (grep also sees untracked, a superset).

### 7.5 Targets the grep tool would skip (grep_q7e.py; a call can hit several)
| family | in-repo visible | in-repo hidden dir (not ignored) | in-repo gitignored path | outside repo (any) | cwd default |
|---|---|---|---|---|---|
| git grep 581 | 518 (89%) | 35 (6%) | 3 (1%) | 24 (4%) | 35 (6%) |
| rg 291 | 152 (52%) | 8 (3%) | 24 (8%) | 116 (40%) | 9 (3%) |
| grep -r 527 | 298 (57%) | 39 (7%) | 34 (6%) | 228 (43%) | 24 (5%) |
| grep FILE 4,333 | 1,657 (38%) | 55 (1%) | 275 (6%) | 2,261 (52%) | 133 (3%) |
| find 397 | 50 (13%) | 9 (2%) | 40 (10%) | 314 (79%) | 7 (2%) |
Flags that lift the filters on purpose: rg `--no-ignore/-u` 10 + `--hidden` 8; git grep `--cached` 2; grep -r on `.codescout/`, `.claude/worktrees`, `.superpowers` paths. Of the grep-tool's own 9,047 rows only 216 (2.4%) set include_hidden, and the tool has no way to lift gitignore.

### 7.6 Param confusion tie-in (FR-11)
`grep` rows with err_family=param_not_applied: 41 (brief: 38): `output_mode` = count 24, files_with_matches 11, files 6 ("'output_mode' is not a parameter of grep - 'count' was not applied - hint ..."); 31 parent, 10 subagent; 2026-10-27..11-10. Recovery in the next 3 calls: grep success 33, run_command 6, other 2; in the 3 rows I read, the next call was the same grep with `mode:"files"`. So the error teaches in one call (healthy by the brief's definition), cost = one wasted call. Meanwhile `output_mode:"content"` (1,539 rows) and `mode:"content"` (482 rows) are accepted silently, and `head_limit` (778) is dropped silently.

## 8. Actionable list (labelled)
1. head_limit is silently ignored (778 rows, 8.6% of grep calls; not in aliases; code comment calls it harmless). Evidence: among 721 rows with head_limit and no limit, 167 (23.2%) returned more matches than head_limit (mean excess 37), 69 overflowed. Defect-or-design: DESIGN QUESTION (alias to `limit` as was done for -i/-C/-A). Confidence: high on facts, medium on cost.
2. The prior "4,700 plain multi-word" is wrong by 7x; 70.2% of patterns are alternations (43.5% pure literal OR-lists), plain multi-word text 650 (7.2%). MEASUREMENT FIX; high.
3. Zeros: no provable false negative in 418 checkable zeros; 47.8% are scope-guess misses, 12.2% starved globs all warned (51/51; static shape 61/61), 16.7% absent everywhere. A bare zero is trustworthy by every test available; what is missing is a verified count for the 284 worktree zeros. MEASUREMENT GAP (not a defect). Medium-high.
4. Relative-glob-repeats-path-prefix keeps recurring (61 zeros of 704, still on 2026-11-09/10 after the 2026-08-27 fix): the warning works (recovery in-session 28.7% by the next 3 calls), but a pre-flight predicate (ADR shape 1) is cheaper than a wasted call. DESIGN QUESTION; medium (recurrence verified, cost = 1 call).
5. No revision parameter: 293 git grep calls at a commit/branch (50% of git grep; 263 subagents). CAPABILITY GAP; high on count (verified samples), medium that the grep tool is the right place.
6. Root-scope greps without glob are slow (p50 998 ms, 50% >= 1 s) and 7 walks hit the 20 s budget, all with absolute paths above the project; three pre-budget walks took 100-387 s. Project-wide `symbols(name)` without path is 10.6 s p50. LATENCY/DESIGN; high on numbers.
7. Identifier greps feed edits (28.2% edit the same identifier within 20 calls vs 1.4% placebo) while references/call_graph follow only 2/765: the confirmation of the earlier "rare" claim; no defect, a discoverability observation. high on numbers, hypothesis on cause.
8. 38.8% of shell content searches target absolute paths outside the repo (scratch, siblings, ~/.claude*), 5.2% gitignored paths; the grep tool serves absolute paths (710 rows) but agents still use the shell for these. WHY is NOT VERIFIED (hypothesis: the files are outputs of the same shell command or the agent never tries grep(path=abs)).

## 9. NOT VERIFIED
- Whether the 284 zeros that ran in deleted worktrees were true (their trees are gone) and the 18 buffer-path zeros.
- Intent behind a zero: "scope miss" is inferred from the next 3 calls; 75.7% of zeros (533/704) have no usable related follow-up.
- That the `rg` emulation equals the tool's walker exactly (.rgignore/.ignore, binary and 8 KB sniff, MAX_FILE_BYTES); I did not run the codescout grep tool to compare per row.
- Zero immediate byte-identical repeats (0 of 9,047) may be real or a recorder artefact.
- Regex backtracking as a latency cause (not tested, only walk size observed); the 99.8 s row 226057 has no input.
- Servability classes (7.4) and revision detection are heuristics over shlex parses; the 4,333 "grep FILE" bucket may include log filtering of a file produced by the same command.
- output_json bytes are capped near 20 kB by the recorder (not verified), so max/p99 of large outputs are lower bounds.
- Why agents choose rg/git grep/find (flags suggest -E, -n, -l, -c, context, rev, `--include`, pathspec exclusion; no run_command guidance text was inspected).
- Rows with missing input (180) and missing output (312) are excluded from the statistics that need them; parent subsets are thinner for rg.
