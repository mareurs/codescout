# Usage Analysis — 2026-11-02 (codescout project database, 2026-10-25 → 2026-11-02)

**Scope:** two parts. Findings 1 to 7 read one database, `/home/marius/work/claude/codescout/.codescout/usage.db`, rows with `called_at >= '2026-10-25'` (first row 2026-10-26, last 2026-11-02 09:38 UTC). Findings 8 and 9 read every database with 20 or more calls in the same window (15 of them). Finding 11 reads this project's Claude Code transcripts from the four profiles. usage.db keeps 30 days.
**Compared with:** `2026-10-24-usage-analysis.md`, which covered 21 days and 40 projects. The two are not like for like; read the comparison as direction only.
**Instruments:** `usage.db` queries (calls, errors, latency, `lsp_events`), a read-only scan of transcripts and subagent files, and four read-only subagent verifications of the friction log against the code. No cost data.

## Headline numbers

| Measure | This window | 2026-10-24 report |
|---|---|---|
| MCP calls | 31,530 in 175 sessions | 192,411 (all projects) |
| Non-success rate | 3.51% | 3.6% |
| Hard `error` rows | 20 | 41 |
| Overflow rate | 5.94% | 5.2% |

## Tool popularity (top rows)

| Tool | Calls | Avg ms | Max ms | Non-success | Overflows |
|---|---:|---:|---:|---:|---:|
| `run_command` | 14,898 | 11,105 | 1,890,710 | 335 | 1,399 |
| `read_file` | 4,977 | 3 | 415 | 291 | 0 |
| `symbols` | 2,646 | 838 | 45,381 | 16 | 196 |
| `edit_file` | 2,455 | 25 | 2,675 | 182 | 0 |
| `grep` | 2,163 | 150 | 99,824 | 25 | 110 |
| `doc` | 1,257 | 93 | 10,981 | 112 | 138 |
| `create_file` | 1,217 | 5 | 96 | 42 | 0 |
| `edit_code` | 1,074 | 354 | 45,461 | 76 | 1 |

`run_command`'s average and maximum are dominated by `sleep` polls (see Slow calls). Tool names were not folded: the 2026-09-02 rename predates the window.

## Findings

### 1. The two Iron-Law gates and the worktree gate still produce most refusals

Top non-success messages in the window: shell-on-source refusal on `run_command` (36), `edit_file` definition guard (27 for `fn `, 25 for `def `, 9 more in batch form), worktree-activation block (23 on `create_file`, 13 on `edit_file`, 11 on `doc`), `IL3` pipe refusals (5 in the top 20). These are gate refusals, not tool failures; the 2026-10-24 report found the gates recover in one call about 95% of the time. Recovery was not re-measured here.

### 2. `edit_code` hits a null `documentSymbol` answer 20 times

"the language server answered textDocument/documentSymbol with null" for 20 `edit_code` calls (and 2 `references` calls) in the codescout database. Investigated in findings 10 and 13: every failing build already contains the `1b208ad79` fix, and 21 of the 22 failing paths are in the main checkout, not in nested worktrees.

### 3. Rust mux lock failures: 15 in 8 days, falling

15 rust `lsp_events` rows failed with "another mux instance holds the lock" (9, 2, 2, 1, 1 per day from 2026-10-28), against 229 successes, each failed start taking 1,028 to 1,144 ms. Cause not established. Filed as `docs/issues/2026-11-02-a-second-client-that-spawns-a-rust-mux-loses-the-lock-race-and-fails-the-start-instead-of-connecting-to-the-winner.md`; the parent bug (`2026-10-17-mux-connect-path-lock-races.md`) was marked `fixed` while this race stayed open, so it was invisible to the open-bug query.

Other LSP failures in the window: java, 21 "LSP server disconnected" (last 2026-10-31); kotlin, a run of "mux process failed to start" rows on 2026-10-30 whose stored error is the server's stderr banner (`Picked up JAVA_TOOL_OPTIONS`), so the real cause is not in the row. Not investigated.

### 4. `edit_file(replace_all=true)` reports no count

Reproduced on a scratch file: three occurrences replaced, response carries no count. Reproduced on a scratch file: three occurrences replaced, response carries no count. Filed as `docs/issues/archive/2026-11-02-edit-file-replace-all-answers-a-bare-ok-with-no-count-of-the-sites-it-replaced.md` (friction log U-37, U-38). Fixed afterwards on `experiments` at `f77ac83c9`: with `replace_all` the answer now carries `replaced`.

### 5. Writes to the shared catalog from under `/tmp` are refused (11)

"refusing to write an artifact rooted under the system temp dir into the shared persistent catalog" on `doc` (11 rows). The refusal is by design; this window's rows show callers still attempt it. Not investigated further.

### 6. Slow calls are polls, not defects

The 12 longest `run_command` rows (all over 1,780 s, up to 1,890 s) are `sleep` loops waiting on the setup-ab evaluation's background work. They inflate the average and the maximum of `run_command` and say nothing about tool latency.

### 7. The friction log held five fixed entries marked open

See `docs/trackers/bug-fix-session-log.md` F-212. 31 entries were archived on 2026-11-02 (commit `26f1b10ff`); 17 stay live: 6 fixed in part, each with its remaining half in the Status line (U-29, U-34, U-40, U-41, U-46, U-47); U-28 flipped to fixed-verified and left live for the next quarterly pass; the other 10 are unchanged (U-12, U-13, U-18, U-32, U-37, U-38, U-48, U-49, U-50, U-51).

### 8. The other databases show the same shape

15 databases have 20 or more calls since 2026-10-25: 69,460 calls, 3.52% non-success (the codescout database is 31,744 of them). The top families across them: `il3_pipe_to_trimmer` 444 (11 databases), `il3_shell_on_source` 334 (11), `il1_read_overlaps_symbol` 254 (13), `il2_structural_edit` 205 (9), `librarian_managed_artifact` on `read_file` 93 (8). 26 further databases were skipped because SQLite could not open them read-only; the ones named in the error were eval-run copies (`es-stage0/critic-a/runs/...`), not real usage. The databases with fewer than 20 calls were not counted.

### 9. Recovery by error family (15 databases)

Share of errors whose next call in the same session and agent succeeds, and share with no success in the next 25 calls:

| Family | Errors | Next call succeeds | No success in 25 calls |
|---|---:|---:|---:|
| `il3_pipe_to_trimmer` | 440 | 95.9% | 0.0% |
| `il2_structural_edit` | 205 | 91.7% | 0.0% |
| `edit_stale_match` | 80 | 92.5% | 0.0% |
| `il3_shell_on_source` | 334 | 89.5% | 0.0% |
| `path_not_found` | 118 | 86.4% | 2.5% |
| `missing_required_param` | 107 | 85.0% | 0.0% |
| (no family) | 256 | 82.4% | 0.0% |
| `il1_read_overlaps_symbol` | 253 | 81.8% | 0.0% |
| `librarian_managed_artifact` | 155 | 81.3% | 0.0% |
| `memory_target_not_found` | 43 | 79.1% | 0.0% |
| `worktree_activate_required` | 87 | 77.0% | 0.0% |

The lowest first-retry success is `worktree_activate_required` (77%). This is not like for like with the 2026-10-24 table, which reported 31.6% of `memory_target_not_found` unrecovered in 25 calls; this run gives 0.0% on 43 errors with a different session key and window, so the two should not be set side by side. Errors that were the last call of a session are excluded here.

### 10. Null `documentSymbol` answers outlive the 2026-10-24 fix

22 rows in the window: 20 `edit_code` and 2 `references` (2 on 10-29, 15 on 10-31, 5 on 11-01), from five builds, **all of which contain `1b208ad79`** (the fix that keeps re-asking while rust-analyzer reports start-up progress). 15 of them are in two sessions on 10-31; finding 13 drills into those. 21 of the 22 failing paths are in the main checkout and one is in a nested `.worktrees/` checkout. An earlier version of this report said the paths were in nested worktrees and that `symbols` rows were included; the drill-down showed both statements were wrong. Both known causes have bug files: `docs/issues/archive/2026-10-23-documentsymbol-null-window-outlasts-the-5s-cold-budget-because-rust-analyzer-scans-the-nested-worktrees.md` (fixed) and the open `docs/issues/2026-10-31-rust-analyzer-watcher-walks-the-directory-symlinks-in-resident-private-without-end-so-documentsymbol-stays-null.md`. No new bug was filed; finding 13 added its evidence to the open bug.

### 11. Transcripts (204 files: 21 parent sessions, 183 subagent files, deduplicated)

- **Parents:** 16,988 tool calls, 94.3% codescout, 5.7% native (mostly `Agent` 248, `SendMessage` 172, `Read` 122, `ToolSearch` 122, `Skill` 92). Native `Read` of source: 23 calls (0.14 per 100).
- **Subagents:** 8,626 calls, 83.1% codescout, 16.9% native. Native `Read` of source: 299 (3.47 per 100); the 10-24 report had 0.45. The reads are spread over many agents, mostly ranged, and mostly in two worktrees. Cause not established. Filed as friction `U-52`.
- **Stop reasons:** `tool_use` 46,122, `end_turn` 2,231, `stop_sequence` 7. No `max_tokens` stop in the window.
- **Hook-output rejections** ("missing required field hookEventName"): at most 1 genuine one since 10-25, against 211 in the 10-24 report, and `emitSkillHint` in the companion's `lib.mjs` now carries a comment about the field. Four more hits dated 2026-11-02 are the text of this audit's own scripts, quoted back into the transcripts the scan reads: an instrument writing into what it measures.
- **Tool-result errors** in transcripts count only hard errors (24 or so), because recoverable refusals come back as normal results. The transcript error rate is therefore not comparable with the database's 3.5%.

### 12. Filed from this audit (nothing fixed)

- `docs/issues/2026-11-02-a-second-client-that-spawns-a-rust-mux-loses-the-lock-race-and-fails-the-start-instead-of-connecting-to-the-winner.md` (id `8e6268a0577ff543`)
- `docs/issues/archive/2026-11-02-edit-file-replace-all-answers-a-bare-ok-with-no-count-of-the-sites-it-replaced.md` (`0a2b075281ab6c82`; fixed at `f77ac83c9`)
- `docs/issues/archive/2026-11-02-119-non-success-rows-carry-no-err-family-after-the-three-shape-fix.md` (`5da4c4cdf95bf3c4`; fixed at `423d812b4`, measured afterwards: 52 NULL rows of 1,130 instead of 119 of 1,113)
- `docs/issues/2026-11-02-call-graph-fails-with-a-bare-disk-io-error-and-the-rows-record-no-input.md` (`cd85dbae70c76670`)
- friction `U-52` in `docs/trackers/codescout-usage-frictions.md`
- evidence added to the open bug `35bcf086deeb7cbe` (the null `documentSymbol` window), under its Resume section
- `docs/issues/archive/2026-11-02-doc-by-id-fails-with-a-bare-enoent-when-the-catalog-row-points-into-a-removed-worktree.md` (`c7777ef798c5361f`; fixed at `d1b47ab5b`)
- `docs/issues/2026-11-02-the-catalog-keeps-the-rows-of-a-removed-worktree-two-still-open-for-bugs-main-calls-fixed-and-wontfix.md` (`bad49b228663df9a`): the stale rows themselves, filed while fixing the error message
- `docs/issues/2026-11-02-audit-doc-refs-over-the-whole-repo-exits-1-on-11-high-findings-five-cited-artifact-ids-resolve-to-nothing.md` (`9db8908e81bc4b26`): found when the audit was run over the whole repository; the runs on touched files, used for the earlier commits of this report, exit 0

### 13. Session drill-down (usage database only)

**Selection.** The database holds 15 sessions with more than 50 calls since 10-25. Four of them have a non-success rate above 10%: `9b43f888` (14 of 53, 26.4%), `d9be3870` (9 of 63), `7c400569` (10 of 72) and `74cfdeb6` (12 of 108). I read the call sequence of `9b43f888` and `74cfdeb6`. For `d9be3870` and `7c400569` I read only the error families. I did not read the two largest sessions in finding 13; finding 14 covers them.

**Transcripts.** I searched the project directories of all four profiles for the full ids of `9b43f888` and `74cfdeb6`. The only file that names them is this audit's own session. So the native tool calls, the cost and the stop reasons of both sessions are not available. Only the codescout calls are known.

**Findings.**

- **Both sessions started at 12:33 on 10-31 and hold 15 of the 22 null `documentSymbol` rows.** In `74cfdeb6`, 9 of 12 non-success rows are `edit_code` inserts on `attribution.rs`, over 11 min 12 s. In `9b43f888`, 6 of 14 are `edit_code` inserts on `create.rs`, over 86 min. Evidence is in the open bug `35bcf086deeb7cbe`.
- **The error text promises about a minute.** The text says a repo with many nested worktrees can keep the answer null "for about a minute after start-up. Retry shortly." `74cfdeb6` retried 9 times over 11 minutes, with `sleep 90`, `sleep 60`, `sleep 120` and a `pgrep` between the tries. The text does not say what to do when the minute has passed. Filed as evidence in the same bug; no new file.
- **The failing paths are in the main checkout.** The earlier text of this report, which said they are in nested worktrees, was wrong.
- **Most failing rows have no `err_family`.** 10 of the 14 non-success rows in `9b43f888` and 9 of the 12 in `74cfdeb6` have none. They are the population of bug `5da4c4cdf95bf3c4`. In `9b43f888` the untagged rows also include three `doc` refusals (an undeclared `cluster/` tag, `extra.opened` "yesterday" and `extra.opened` null) and one `grep` notice about `output_mode`.
- **`d9be3870` (10-27, 11:16 to 11:25):** 6 of its 9 non-success rows are `write_scope_denied` (5) and `shrink_guard_blocked` (1). I did not read why the writes were refused.
- **`7c400569` (10-31):** 10 non-success rows in 8 tool-and-family pairs, none above 2 rows. It has 7 overflows in 72 calls. I did not read it further.

### 14. Session drill-down: the two largest sessions

Both are in the database only; no transcript was read. Both are dominated by the three Iron-Law gates, which is what finding 1 reports for the whole window.

**`8edd247a`** (1,224 calls, 86 non-success, 7.0%; 10-31 12:34 to 11-01 20:43, working in the `.worktrees/setup-ab-eval` checkout):

- 744 of the calls are `run_command`. The 15 slowest are `sleep` polls of the setup-ab evaluation (up to 1,890 s). That agrees with finding 6.
- Non-success rows by family: `il3_pipe_to_trimmer` 24, `il2_structural_edit` 19 (the ten most-hit paths, 17 of the 19 rows, are Python files under `tools/setup-ab-eval`), `worktree_activate_required` 16, `il3_shell_on_source` 14, `il1_read_overlaps_symbol` 4.
- Next-call success after a refusal: `il2` 18 of 19, `il3_pipe_to_trimmer` 23 of 24, `il3_shell_on_source` 12 of 14, `worktree_activate_required` **9 of 16**. The last family is the slowest to recover, as in the 15-database table (finding 9). 12 of its 16 rows fall in the first 65 minutes (12:34 to 13:39); the rest are on 11-01.

**`f844662c`** (575 calls, 44 non-success, 7.7%; 10-30 20:31 to 22:22, in the main checkout):

- Non-success rows by family: `il3_pipe_to_trimmer` 8, `il3_shell_on_source` 8, `edit_file` on a managed document (`librarian_managed_artifact`) 7, `il1_read_overlaps_symbol` 7, `missing_required_param` 4, and 2 `doc` failures.
- The 7 `librarian_managed_artifact` rows are `edit_file` calls on bug files and on two `docs/conventions/` files, and 5 of the 7 were followed by a successful call.
- One of the 2 `doc` failures is a hard error, "No such file or directory (os error 2)", from `doc(update)` by an id whose catalog row points into a removed worktree. It is the only row with that message since 10-25. Filed as `docs/issues/archive/2026-11-02-doc-by-id-fails-with-a-bare-enoent-when-the-catalog-row-points-into-a-removed-worktree.md` (`c7777ef798c5361f`), fixed afterwards at `d1b47ab5b`: the error now names the path and is recoverable. The other is a recoverable `body_edits` refusal ("old_string not found in section").
- The catalog behind that error: 51 of 6,790 rows point at a missing file, all with `missing_since` set, and 13 are in this project's removed worktrees (all marked missing at 2026-10-24 13:15:51; two of them still say `open` while the main-checkout row says `fixed` or `wontfix`). `doc(find)` and `librarian(doctor)` do not show them, so only a remembered id reaches one. The counts are in the bug file's Resume.

## Blind spots

- **Databases.** Findings 1 to 7 use one database. Findings 8 and 9 use the 15 with 20 or more calls and skip 26 that SQLite could not open read-only.
- **Retention.** 30 days; the window is 8.
- **`usage.db` records only codescout MCP calls.** Native harness tools are not recorded, so a zero means "not recorded", never "none happened". The transcript scan covers native tools, for this project's directory only: sessions started from a worktree path have their own project directory and were not read.
- **Transcript errors.** Recoverable refusals return as normal results, so transcripts show about 24 hard errors against thousands of refusals in the database.
- **Recovery table.** Defined here as the next call of the same session and agent; the earlier report used another key, so the two are not comparable.
- **Subagent findings** on the friction log are model reports. Only the cited commits were checked to exist; the code claims were not re-run, except where this file says it reproduced or queried something.
- **Instrument contamination.** The hook-rejection count includes this audit's own text in the transcripts it reads; the table above removes it by hand and says so.
- **`input_json` is missing from 36.7% of the rows in the window.** A server records a call's input only when it was started with `--debug`, and both forms run on this machine. Every figure here that reads `input_json` (the slow-command lists of findings 6 and 14, the paths of the refused `il2` edits, the per-session sequences of findings 13 and 14) counts only rows that had it, and none of them said so when it was written. Filed as `docs/issues/2026-11-02-usage-db-stores-a-calls-input-only-for-a-debug-server-so-over-a-third-of-recent-rows-have-none.md` (`2108589a1b5fab62`). Counts that use only `tool_name`, `outcome`, `err_family`, `error_msg` and the session columns are not affected. The operator decided on 2026-11-02 to record the input always; `a72175ca5` does that for builds that contain it. This report's figures cover the period before, so the gap stays true of them.

## Not done

Cost and stop-reason analysis from the proxy (`claude-traces`, Langfuse; needs the proxy keys, which I did not read), the native-tool side of every drilled session (no transcript was found for the two sessions of finding 13, and none was read for those of finding 14), the 26 unreadable eval-copy databases, the transcripts of worktree-started sessions, user corrections and git-history measures (wrong implementations, fix-on-fix rate) from the 10-24 report, and the cause of U-52.
