# Usage Analysis — 2026-11-02 (codescout project database, 2026-10-25 → 2026-11-02)

**Scope:** one database, `/home/marius/work/claude/codescout/.codescout/usage.db`, rows with `called_at >= '2026-10-25'` (first row 2026-10-26, last 2026-11-02 09:38 UTC). The `analyze-usage` skill finds about 150 databases under `~/work`; the other ones were not read. usage.db keeps 30 days.
**Compared with:** `2026-10-24-usage-analysis.md`, which covered 21 days and 40 projects. The two are not like for like; read the comparison as direction only.
**Instruments:** `usage.db` queries (calls, errors, latency, `lsp_events`) and four read-only subagent verifications of the friction log against the code. No transcripts, no cost data.

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

### 2. `edit_code` hits a null `documentSymbol` answer 19 times

"the language server answered textDocument/documentSymbol with null" for 19 `edit_code` calls. Not investigated here. A related fix (`1b208ad79`, keep re-asking while rust-analyzer reports start-up progress) is in history, so whether these are before or after it was not checked.

### 3. Rust mux lock failures: 15 in 8 days, falling

15 rust `lsp_events` rows failed with "another mux instance holds the lock" (9, 2, 2, 1, 1 per day from 2026-10-28), against 229 successes, each failed start taking 1,028 to 1,144 ms. Cause not established. Filed as `docs/issues/2026-11-02-a-second-client-that-spawns-a-rust-mux-loses-the-lock-race-and-fails-the-start-instead-of-connecting-to-the-winner.md`; the parent bug (`2026-10-17-mux-connect-path-lock-races.md`) was marked `fixed` while this race stayed open, so it was invisible to the open-bug query.

Other LSP failures in the window: java, 21 "LSP server disconnected" (last 2026-10-31); kotlin, a run of "mux process failed to start" rows on 2026-10-30 whose stored error is the server's stderr banner (`Picked up JAVA_TOOL_OPTIONS`), so the real cause is not in the row. Not investigated.

### 4. `edit_file(replace_all=true)` reports no count

Reproduced on a scratch file: three occurrences replaced, response carries no count. Filed as `docs/issues/2026-11-02-edit-file-replace-all-answers-a-bare-ok-with-no-count-of-the-sites-it-replaced.md` (friction log U-37, U-38).

### 5. Writes to the shared catalog from under `/tmp` are refused (11)

"refusing to write an artifact rooted under the system temp dir into the shared persistent catalog" on `doc` (11 rows). The refusal is by design; this window's rows show callers still attempt it. Not investigated further.

### 6. Slow calls are polls, not defects

The 12 longest `run_command` rows (all over 1,780 s, up to 1,890 s) are `sleep` loops waiting on the setup-ab evaluation's background work. They inflate the average and the maximum of `run_command` and say nothing about tool latency.

### 7. The friction log held five fixed entries marked open

See `docs/trackers/bug-fix-session-log.md` F-212. 31 entries were archived on 2026-11-02 (commit `26f1b10ff`); 17 stay live: 6 fixed in part, each with its remaining half in the Status line (U-29, U-34, U-40, U-41, U-46, U-47); U-28 flipped to fixed-verified and left live for the next quarterly pass; the other 10 are unchanged (U-12, U-13, U-18, U-32, U-37, U-38, U-48, U-49, U-50, U-51).

## Blind spots

- **One database.** A friction that shows only in another project's database is not here.
- **Retention.** 30 days; the window is 8.
- **`usage.db` records only codescout MCP calls.** Native harness tools are not recorded, so a zero means "not recorded", never "none happened".
- **No recovery analysis.** The next-call-succeeds rate by error family was not recomputed.
- **Subagent findings** on the friction log are model reports. Only the cited commits were checked to exist; the code claims were not re-run, except where this file says it reproduced or queried something.

## Not done

Transcript analysis, cost and stop reasons (`claude-traces`), the other 150 databases, per-session drill-down, a recovery table by error family, and a check of the 19 null `documentSymbol` answers against the build date of `1b208ad79`.
