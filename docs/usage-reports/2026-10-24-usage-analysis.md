# Usage Analysis — 2026-10-24 (21-day window, 2026-10-03 → 2026-10-24)

**Window:** `called_at >= '2026-10-03' AND < '2026-10-25'` (UTC). usage.db retention is 30 days, so older data is gone.
**Instruments:** four, each with a blind spot stated in § Blind spots.

| Instrument | What it covered | Detail file |
|---|---|---|
| `usage.db` (56 DBs changed since 10-03; 40 with ≥20 calls) | 192,411 MCP calls | `scratchpad/sweep.json`, `agg.txt`, `recovery3.txt` |
| Claude Code transcripts (4 profiles) | 518 parent sessions, 1,501 subagent files, 176,455 tool calls | `scratchpad/transcripts/FINDINGS.md` |
| git history + bug ledger | 965 commits, 132 bug files dated in the window | `scratchpad/repo/FINDINGS.md` |
| source + live replay | verification of each claim marked "verified" below | this file |

Tool-name folding: not needed. The 2026-09-02 collapse predates the window.

## Headline numbers

| Measure | Value | Source |
|---|---|---|
| MCP calls | 192,411 (Claude Code 158,304; Codex 34,663) | usage.db |
| Error rate (`outcome != 'success'`) | 3.6%; hard `error` only 41 rows (0.02%) | usage.db |
| Weekly error rate | W1 3.78% · W2 3.44% · W3 3.56% | usage.db |
| Overflow rate | 5.2% (10,073 calls); `run_command` 6,781 | usage.db |
| Subagent share of calls | 55% (106,418 of 192,372 with the column) | usage.db `agent_id` |
| Codescout share of tool calls (transcripts) | 91.5%; native tools 2.9% | transcripts |
| Native `Read` of source files, per 100 calls | parents 0.05, subagents 0.45 | transcripts |
| After a codescout error, next call stays in codescout | 98.4% (switch to native: 40 of 6,299) | transcripts |
| Fix commits that repair code written in the window | ~42% (range 34–47%); 43% repair an earlier fix | git blame, 302 fixes |
| Bugs filed in the window | 132 (20 high); 105 fixed, 23 open; 65% are tool bugs | ledger |
| Estimated cost | $9,567 at list prices (Sonnet $6,768; Opus $2,799); cache reads 68% | transcripts |

## Comparison with the 2026-09-10 sweep

The two sweeps are **not like-for-like**: that one had a 3-day window and 6 projects; this one has 21 days and 40 projects. Read the table as direction only.

| 2026-09-10 improvement candidate | Then | Now (21-day window) | Reading |
|---|---|---|---|
| `append_entry` unpushed-commits refusal | 19 / 3 days | 0 | gone (verified by query) |
| `worktree_activate_required` | 19 / 3 days (codescout) | 56 total; 1.62 → 0.28 per 1k calls from W0 to W3 | falling |
| `missing 'path'` | 21 / 3 days | 25 / 21 days | falling |
| `file_path` alias in `read_file` | 147 / 3 days in 3 projects | 17 in 21 days (all projects) | large drop; cause not checked |
| IL-3 source gate (`il3_shell_on_source`) | 75 / 3 days (codescout) | 1,248 in 21 days (all projects), 4.8 → 9.3 → 6.9 → 7.0 per 1k | **not falling** |
| IL-3 pipe limiter (`il3_pipe_to_trimmer`) | ~20 / 3 days | 1,457 in 21 days, 8.7 → 9.0 → 10.3 → 8.7 per 1k | **not falling** |
| Recoverable-error rate | 5.0% | 3.6% | lower, window differs |

The 09-10 report's decision still stands: the Iron-Law gates are healthy by recovery (next call succeeds 95%), so their volume is a cost, not a malfunction (see `infra/friction-measurement`, TU-7).

## Findings, ranked by measured cost

### 1. `grep` silently drops native-Grep parameters (bug filed, high)

4,966 of 23,222 recorded `grep` calls (21.4%) carried a key that the schema does not declare. 4,960 returned `success` with no notice. Dropped keys include `-i` (82), `multiline` (11), `-A/-B/-C`/`context` (269), `head_limit` (593) and `output_mode` (4,693).

Verified by replay: the same search returned **43 matches with `ignore_case=true` and 37 without** (what a `-i` call actually gets). The dropped lines are real hits.
→ `docs/issues/2026-10-24-grep-silently-drops-native-grep-parameters-so-a-case-insensitive-search-runs-case-sensitive.md`

The same scan found smaller cases: `tree.depth` (11), `edit_code.reindent` (95) and `.new_body` (4).

### 2. Companion hook output is rejected 211 times (NOT filed; other repo)

Claude Code rejected hook output 211 times with "missing required field hookEventName": 150 before `edit_code`, 61 before `Agent`. **Verified in source:** `emitSkillHint` in `claude-plugins/codescout-companion/hooks/lib.mjs` emits `hookSpecificOutput` with no `hookEventName`, while the helpers at lines 34, 45 and 57 set it. Effect: the reconnaissance hint and the pre-dispatch hint never reach the model. The transcript agent reports the bug is on `main` and in the installed cache 1.20.19; I did not verify the cache version.
→ The fix is in a different repo. I did not write there. It needs a bug file in `claude-plugins`.

### 3. Gates are refused, then worked around, not obeyed

- `read_file` symbol redirect (~790 refusals): 38% re-send with `force=true`, 31% switch to `symbols`. In the usage.db, 10,728 of 14,888 source-file `force=true` reads came **before** any refusal in that process. `force=true` is now the habit, not the escape.
- `edit_file` definition guard (611): 46% reshape the same call, 40% switch to `edit_code`.
- IL-3 pipe: after the 10-22 run-to-completion feature, the remaining refusals are mostly a list after the pipe (`cmd | head -5 && echo done`: 89 of ~150 sampled since 2026-10-22 13:00). The feature does not splice that shape. This was sampled, not counted over the full window.
- `acknowledge_risk=true`: 1,349 uses; 1,077 came after a refusal and 272 were pre-emptive.

### 4. Recovery quality by error family (usage.db, 6,986 errors)

| Family | n | next call succeeds | unrecovered in 25 calls | median calls to recover |
|---|---:|---:|---:|---:|
| `memory_target_not_found` | 98 | 67.3% | **31.6%** | 2 |
| `librarian_managed_artifact` | 322 | 89.1% | **12.7%** | 3 |
| `symbol_not_found` | 20 | 95% | 15% | 2 |
| `il3_pipe_to_trimmer` | 1,460 | 95.7% | 0.3% | 1 |
| `il3_shell_on_source` | 1,248 | 95.2% | 2.2% | 2 |
| `edit_stale_match` | 352 | 93.2% | 0.0% | 2 |

`memory_target_not_found` is the worst. Its largest group is git worktrees of projects that do not commit `.codescout/memories/`: the session sees **zero** memories ("no memory topics exist yet"), not a diverged set. I added this measurement to the open bug `docs/issues/2026-09-24-residual-worktree-memory-resolution-choice.md`, which still says severity low.

The "identical retry" column of my recovery script is **not reported**. One run gave 0 for every family after I added a NULL guard, and I did not find why. Codex rows have NULL `input_json`, so an unguarded run counted NULL = NULL as a retry.

### 5. Output overflow is the largest context cost

| Tool | Overflows | Tokens in overflow | Avg tokens | Buffer read afterwards |
|---|---:|---:|---:|---:|
| `symbols` | 1,452 | 6.27 M | 4,315 | 18.1% |
| `grep` | 1,040 | 4.45 M | 4,283 | 8.2% |
| `doc` | 531 | 4.21 M | 7,928 | 74.0% |
| `librarian` | 131 | 1.74 M | 13,262 | 76.2% |
| `run_command` | 6,812 | 1.00 M | 146 | 45–62% |

`doc(get)` is 303 of the 531 `doc` overflows. `librarian(doctor)` is 45 of 131. For `symbols` and `grep`, most buffers are never read: the model reads the inline summary and re-queries.

### 6. Latency

`run_command` p50 97 ms, p90 29 s, p99 290 s, max 3,000 s; it is 322 of the ~327 hours of summed latency, which is mostly the commands themselves (cargo, tests). Slow non-shell calls: `librarian(reindex)` up to 187 s (the worst on `mirela/backend-kotlin`), `grep` up to 387 s (two scans of `/home/marius/work/claude` and `/home/marius`), `symbols` up to 82 s on Kotlin worktrees.

### 7. LSP

- **Java:** 235 starts, **235 failed**, avg 16.6 s each, all in the codescout project (bug filed).
- **Kotlin:** 455 failed starts across projects, mostly "mux holds its lock but its socket is unreachable" (132 in `backend-kotlin`) and "another mux instance holds the lock" (rust, 66+). 268 Kotlin starts succeeded, max handshake 11 s.
- A `edit_code` failure with `KaNotUnderContentRootModuleImpl` (Kotlin, 2 calls) is a worktree layout problem, not a model error.

### 8. Wrong implementations (git, 302 code-touching fix commits)

- 52% of fixes repair mostly in-window code; median age of repaired code 5.9 days; 44% repair code under one day old. W3 is the worst week (60%, median 0.9 days). A hand check of 40 in-window fixes found 32 (80%) real: 19 review rounds or incomplete first attempts, 13 new defects.
- Most-fixed files: `read_file.rs` 36 (verified: 36 `fix` commits in the window), `server.rs` 34, `cap_probe.rs` 34, `run_command/output.rs` 29, `path_security.rs` 28.
- No `Revert` commits (verified: 0). One fix functionally reverts the IL-3 prompt text (A-41 measured a regression).
- Case study: the IL-3 capture feature (10-21) had 7 same-day fixes, then red CI on dash and BSD (fixed), a Windows build failure (open) and a `head` capture that looks complete but is partial (open).
- Bug families: symlink-following 12, capped scans reporting false zeros 8 (12 in the class "capped result presented as complete"), backfill chain 7 in 6 days.
- Most severe: `doctor` prune and `merge_worktree` deleted catalog rows of live sibling repos; `edit_file` and the librarian read deny-listed files through symlinks; LSP mux reads were not cancel-safe.
- Caveat from the agent: blame finds the lines a fix touched, not the cause; much of the in-window share is same-session iteration, not shipped regressions.

### 9. What users corrected, and what agents admitted

- 2,357 genuine human prompts. The keyword selector has 24.8% precision, so it misses most corrections; the agent's estimate is ~5% of prompts, with a wide margin. 13.3% are bare approvals.
- 38 hand-labelled corrections: misunderstood intent 9, stalling or pushing work back to the user 7, unverified claims 7, wrong approach 6, over-scoping 5, ignored available knowledge 2, "fixed" but broken 2. **None blames a codescout tool response.**
- 140 self-admitted errors (1.03 per 1,000 turns; parents 2.80, subagents 0.03). Largest group: factual claims made without checking (17 of 56 labelled). 3 trace to confusing codescout API surface; none to wrong output.

### 10. Other transcript findings (from the agent; not all re-verified)

- 190 calls to non-existent tool names, all by Sonnet 5.5, 154 in subagents, all failed: bare codescout names (`run_command` 89, `read_file` 30) and 32 invented `*_placeholder` names.
- 332 edits refused because the project was activated read-only; 41% targeted scratchpad or `/tmp`. The suggested fix (`workspace=`) created a `.codescout/` folder and `.gitignore` in the scratchpad. **Verified:** `scratchpad/transcripts/.codescout` and `.gitignore` exist. Related: open bug `2026-10-18-a-workspace-pin-naming-a-path-that-does-not-exist-creates-a-project-there` (fixed in `9bd0df3df`; the stray directories I found in the repo were created by builds before that fix).
- `grep`→`semantic_search` hint: 581 firings, followed by `semantic_search` 0 times; at least 46% of the patterns were code identifiers. `semantic_search` was called 21 times in 3 weeks (29 in usage.db).
- "Parallel writes" hint: 1,437 firings, 97% accurate, no behaviour change; it cites BUG-021 (wontfix).
- `read_file` refusing `end_line` without `start_line`: 89 refusals (usage.db) / 91 (transcripts), not covered by any open bug.

## Telemetry defects found (all filed)

| Defect | Evidence | Bug |
|---|---|---|
| `friction-probe.py` counts only `outcome='error'` | reports 8 errors where 2,038 exist | `2026-10-24-friction-probe-counts-only-hard-errors-…` |
| `codescout_dirty` is 1 on 99.7% of rows | 80,336 of 80,597 since 09-24 | `2026-10-24-codescout-dirty-is-always-one-…` |
| `err_family` keys on retired `read_markdown`; NULL head is 785 rows | 146 `read_file` overflow rows plus 6 new guard messages | `2026-10-24-err-family-overflow-branch-keys-on-retired-…` |
| Java LSP pre-warm fails every time | 235 of 235 | `2026-10-24-java-lsp-prewarm-fails-on-every-…` |
| Codex runs do not record `input_json` | 99.7% of Codex rows have none | not filed; a launch-config choice (`codescout start` without `--debug`) |

Because of the last row, any input-based measure (undeclared keys, `force` habit, retries) covers Claude Code only.

## Blind spots

- usage.db sees MCP calls only; native tools come from transcripts only.
- 18.7% of transcript files are second copies of a session in another profile; the agent kept the longest copy.
- 367 of 518 parent sessions are scripted `sdk` runs, so "per session" figures mix human and scripted work.
- Several usage.db DBs are worktree or fixture copies (`es-stage0`, `codescout.worktrees/fix-*`); their calls are small and included.
- `observed_changed` is set on 854 rows, 10 true; too few to use.
- Cost uses list prices and matches Claude Code's per-session records in 70 of 71 Opus and 400 of 456 Sonnet sessions.
- Window mix: W2 (10-10..10-16) has 37.9k Claude calls, against 60k in W1 and W3, so weekly comparisons across W2 are weak.

## Not done

- No session drill-down with `lf.py` (Langfuse); the transcript agent gave cost and stop reasons instead (1 `max_tokens` stop, 272 compactions, 46 rate limits).
- No before/after test of any change. `friction-probe.py --split-at` cannot give one until the `codescout_dirty` bug is fixed.
- No `lsp_events` first-response latency analysis beyond handshake times.
