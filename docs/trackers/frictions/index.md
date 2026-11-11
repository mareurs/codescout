---
id: be1321a7959b5b23
kind: tracker
status: active
title: Frictions ledger — where agents lose calls to codescout
tags:
- frictions
- tool-usage
- ledger
entry_prefix: FR
entry_high_water_FR: 16
---

# Frictions ledger — where agents lose calls to codescout

One entry per friction. An entry is a **measured** place where agents lose calls, work or trust
to a codescout tool. The index below holds one line per friction. The full evidence lives in the
entry's own file, `docs/trackers/frictions/FR-N-<slug>.md`.

## What counts as a friction

**Count is not friction.** A gate that fires and is obeyed on the very next call is healthy
teaching (`2026-08-15-tool-usage-investigation:TU-7`). Admit an entry only when at least one of
these holds, and say which one in the entry:

- the agent repeats the same call after an error (immediate repeat);
- the agent needs three or more calls to recover, or never recovers;
- the agent routes around the tool, for example with `run_command` sed, cat or grep;
- the error text is wrong, misleading, or gives no corrective hint;
- the call succeeded but the output misled (a false zero, a hollow or truncated result);
- the agent passes a parameter the tool does not declare;
- the tool refuses something it should allow, or two tools disagree about the same write;
- latency makes agents retry or give up.

Healthy high-volume gates are listed in the section **Healthy — not frictions**, so nobody
re-litigates them.

## Vocabularies

**Kind:** `wrong-guidance` · `misleading-output` · `gate-too-strict` · `gate-too-loose` ·
`param-confusion` · `missing-capability` · `silent-failure` · `discoverability` ·
`hint-ineffective` · `latency` · `other`.

**Status:** `open` (measured, not yet fixed) · `fixing` (a fix is in flight, name the branch) ·
`fixed-verified` (fix shipped and a re-measurement shows the flatline) · `wontfix` (decision
recorded in the entry) · `not-a-friction` (re-measurement showed healthy recovery).

**Severity** (anchored on recovery cost, not on volume): `low` — recovers in one call;
`med` — needs two or more calls, or routes around the tool; `high` — silent wrong result,
lost work, or no recovery.

## Rules for entries

1. **Evidence or nothing.** Every number names the SQL (or script) that produced it, the window
   in UTC, and the corpus. `usage.db` records MCP calls only. `input_json` is empty on about 19%
   of rows and `output_json` on about 37% (mostly parent-session rows), so every rate is partial.
2. **Cite the recovery.** For each entry, sample at least five rows and say what the next calls
   were: fixed on the next call, needed two or more calls, routed around, abandoned.
3. **A friction that is also a defect gets a bug file** in `docs/issues/` (`CLAUDE.md` § Bug
   Tracking). The entry cites the bug file and the bug file cites the entry.
4. **A fix cites its entry** in the commit message and sets `status: fixed-verified` only after
   a re-measurement. The re-measurement uses `scripts/friction-probe.py` and the commit SHA
   (`codescout_sha` is a column of `tool_calls`).
5. **Language:** English.

## First mining pass — 2026-11-11

**Method.** Three Sonnet investigators (transcripts, `usage.db` adoption, a code and LSP-capability audit) and ten Haiku miners, one per slice of `usage.db`: `replace`, other `edit_code`, `edit_file`, `read_file`, `run_command`, search tools, `doc`/`memory`/`create_file`, LSP tools with `lsp_events`, hint follow-through, refactor sequences. Instruments: `scripts/friction-probe.py` and direct SQL, read-only. Corpus: 117,391 calls from 2026-10-12 to 2026-11-10 UTC, 462 sessions. Transcripts: 1,487 files from 4 profiles, 2026-09-19 to 2026-11-10.

**What the lead re-verified before writing.** `edit_code.rs:239` against the three target-aware call sites; the `references.rs:493` warning text and `corruption_verdict`; the `cs-hint` artifact (sampled rows); counts for FR-5 to FR-9 and FR-15; `timed_out` rows. Everything else is the miners' count and says so in its detail file.

**Corrections made during the pass.** Error rows with a `— hint:` text are about 3,065 of 3,924 (78%), not 2,360. The "about 360 success rows with a `cs-hint`" figure counts echoed file content, not advisories (FR-3).

**Against the starting hypotheses.**

- *Rename is missing.* It is not: `edit_code(action="rename")` ran 57 times with 0 errors (104 times in transcripts). The two-step `replace` then `rename` shows up in 8 sessions and agents recover. Real gaps are in rename's reach (no dry-run, no `at_line`, resource operations dropped), which are capability gaps and not frictions.
- *`replace` and refactors hide friction.* `replace` refusals are 6.1% of 1,779 calls and mostly healthy. The one defect is FR-13. Refactor sequences show 59 route-arounds after a failed `edit_code` (26% of failures), 923 verification re-reads (FR-10) and 7 renames followed by a failing build or test.
- *The complex LSP tools are rarely used.* True (FR-4), and cost and trust look like stronger drivers than awareness.
- *Agents reach for LSP operations codescout lacks.* Not found in the transcripts: no complaint and no hallucinated LSP tool name. The nearest finds are a hallucinated `glob` tool (11 calls, 4 sessions) and tools that disappear when a project has no LSP. The corpus is the codescout repo itself, so it under-represents outside agents.

**Capability audit.** The code-level LSP audit (what the client wires, what each server offers, rename's limits, candidate features with sizes) is in `docs/research/2026-11-11-lsp-capability-audit.md`. It is supply, not demand: no friction entry rests on it.


**Measurements.** The statistics behind these entries (latency, `grep`, `rg` and `git grep`, the git library) are in [statistics.md](statistics.md). The analyst reports are in `docs/research/2026-11-11-usage-stats-*.md`.

## Index

| id | date | sev | kind | tool | status | title |
|---|---|---|---|---|---|---|
| FR-16 | 2026-11-11 | med | discoverability | semantic_search | open | **`semantic_search` is almost unused** — 14 calls against 9,189 `grep`; 4 of 14 hit a missing worktree index |
| FR-15 | 2026-11-11 | low | gate-too-strict | run_command | open | **The `il3` gate refuses scratchpad writes with an unexpanded `$VAR` path** — 96 rows; possible regression of a fixed bug |
| FR-14 | 2026-11-11 | med | latency | run_command | open | **Wait loops are two thirds of the slowest `run_command` calls** — 344 of 512; 166 ended at their timeout |
| FR-13 | 2026-11-11 | med | misleading-output | edit_code | open | **A rename of a Python method inside `replace` is reported as a dropped definition** — 2 rows confirmed of 30 |
| FR-12 | 2026-11-11 | med | hint-ineffective | edit_code | open | **`edit_code` was retried ten times on a null `documentSymbol` while the hint said "Retry shortly"** — 22 rows, 8 sessions |
| FR-11 | 2026-11-11 | low | param-confusion | edit_code, edit_file, grep, doc | open | **Agents pass a sibling tool's parameter name** — 143 rows across four tools; recovery is quick |
| FR-10 | 2026-11-11 | low | other | edit_code, edit_file | open | **Edit acknowledgements do not confirm what is now in the file** — 9% of edits are re-read; hypothesis is weak |
| FR-9 | 2026-11-11 | low | hint-ineffective | read_file | open | **The `il1` source-read gate is followed by a plain repeat in a fifth of cases** — 75 of 327 |
| FR-8 | 2026-11-11 | med | wrong-guidance | edit_file, read_file | open | **Managed-artifact refusals lead with what is allowed and end in shell reads** — 200 rows |
| FR-7 | 2026-11-11 | med | discoverability | read_file, symbols, edit_file | open | **`path_not_found` never names an existing neighbour** — 309 rows; agents route around the hint |
| FR-6 | 2026-11-11 | med | gate-too-strict | edit_code | open | **`edit_code` still uses the target-blind worktree guard** — edit_code.rs:239; the 2026-10-26 fix reached three of four write tools |
| FR-5 | 2026-11-11 | med | misleading-output | symbols | open | **`symbols` answers "0 matches" as success when the language server failed** — 71 rows, warning inside the text |
| FR-4 | 2026-11-11 | med | other | references, call_graph, symbol_at | open | **The LSP-backed tools are used in a minority of edit sessions** — 14% call `references`/`call_graph` before the first edit |
| FR-3 | 2026-11-11 | high | silent-failure | usage.db | open | **`usage.db` cannot show what the agent saw** — 19% empty inputs, 37% empty outputs, hook advisories unrecorded |
| FR-2 | 2026-11-11 | high | silent-failure | run_command | open | **`run_command` timeouts and failing commands are recorded as `success`** — the probe cannot see them |
| FR-1 | 2026-11-11 | med | misleading-output | references | open | **`references` blames a warming index when the name only matches same-named locals** — warning text at references.rs:493 |

## Healthy — not frictions

These gates fire often and the agent recovers on the next call. Do not rank them by count (`2026-08-15-tool-usage-investigation:TU-7`). Counts are from `usage.db`, 2026-10-12 to 2026-11-10 UTC, re-counted where marked in the entry's detail file.

| Gate or family | Rows | Evidence that it teaches |
|---|---|---|
| `il2_structural_edit` (`edit_file` refuses a new `fn`/`def` body) | 255, 83 sessions | Next call: `edit_code` success 80, `edit_file` success 93, repeat error 16 |
| `replace_would_rename` (`edit_code` replace cannot rename) | 41, 25 sessions | The message names `edit_code(action="rename")`; 21 of 41 renamed, then replaced, within 3 calls |
| `shrink_guard_blocked` | 31 to 39 | 18 of 31 forced the replace within 3 calls and the success carries a warning. In the 3 cases checked the re-read that the hint asks for was skipped (idea: show the lost lines in the refusal; hypothesis) |
| `il1_read_overlaps_symbol` | 354 | 55% got the content in one call. The residue is FR-9 |
| `il3_pipe_to_trimmer` | 719, 125 sessions | Next call was a successful `run_command` in 598 (83%); `git log --oneline | head` is the commonest false alarm |
| `read_file_overflow_threshold` | 156 | 55 narrowed the next read |
| `create_file` `target_already_exists` | 36 | 34 recovered on the next call |
| `doc(update)` `missing_required_param` (`body_edits` without heading or action) | 88 | 75 recovered on the next call |
| `write_blocked_read_only` | 4 | Each recovered in one call with `workspace=`; the agent did not carry the lesson to its next call |
| `@bg_N.err` suffix refused | 8 | The bare handle is used on the next call |
| `grep` `invalid_regex` (look-around) | 9 | The error names the cause; 8 of 9 next calls are `grep` |
| `grep` and `symbols` overflow | 467 and 836 | Agents re-run with a narrower scope more often than they query the `@tool` buffer (176 against 31 for `grep`). The narrowing hint works as designed; the buffer is a low-salience affordance |

## Watchlist — single observations and unproven cases

Not admitted as entries, because the evidence is one sequence, a sample, or cannot yet tell a defect from a healthy case. Promote an item when a re-measurement meets the rules above.

| Item | Tool | Evidence | Why it waits |
|---|---|---|---|
| Bare `0 matches` from `grep` | `grep` | 443 rows with no scope named; next call the same `grep` 196, `run_command` 140 | ADR 2026-08-27 clause 2: a trustworthy negative must stay quiet. The data cannot tell a true zero from a false one. Admit only with a proven false zero. |
| `write_scope_denied` on `edit_code` while `edit_file` on the same path succeeded | `edit_code` | 13 rows, 9 sessions; row 296252 against row 296251, 0.5 s apart | No scope-guard difference found in the call sites; whether row 296251 wrote the file is unverified (see FR-6). |
| `memory` has no section-edit action | `memory` | 1 sequence, 6 calls for one edit (rows 294827 to 294839) | One sequence. |
| `memory` section miss has no did-you-mean | `memory` | 50 rows, 30 sessions; 44 recovered on the next call | Recovers in one call. |
| `doc(create)` in a temp-rooted workspace | `doc` | 18 rows, 6 sessions; 5 refusals in 3 s in one session (rows 226850 to 226856); the hint names `CODESCOUT_ALLOW_TEMP_WORKSPACE=1`, which an agent cannot set mid-session | Inputs are empty, so the intent is unknown. The gate protects the shared catalog. |
| `doc` unknown id returns `outcome=error` with no hint | `doc` | 7 rows (row 229158) | Small; placeholder ids. |
| `cwd_escapes_root` against `cd` inside the command | `run_command` | 20 rows; row 290879 moved the path into `cd` and ran | Sampled only. |
| `references` on a directory path says "unsupported language" with no hint | `references` | 1 row (229882) | One row. |
| `workspace(action="post_compact")` | `workspace` | 3 rows; the guide documents `post_compact=true`, the agent passed it as the action | Small. |
| `symbol_not_found` did-you-mean drops the `tests/` prefix | `edit_code` | 1 row sampled (204001) | One row. |
| `at_line` misses on `remove` | `edit_code` | 5 rows (for example 192677) | May be the open bug `docs/issues/2026-11-10-symbols-stale-ranges-after-source-formatting.md`; unverified. |
| `call_graph` "disk I/O error" | `call_graph` | 3 rows under `/tmp/codescout-*` roots | Cause unknown; related mitigated bug of 2026-11-02. |
| Language-server start failures that agents rarely see | `lsp_events` | kotlin expired build 423, java disconnect 347, rust mux lock 107, kotlin mux lock 142 | Known bugs (see FR-3); no agent-side friction measured because `lsp_events` has no `session_id`. |
| `force=true` after a shrink refusal without the re-read | `edit_code`, `create_file` | 25 forced after a refusal; 3 verified with no re-read | Content restoration cannot be checked in `usage.db`. |

## FR-1 — `references` blames a warming index when the name only matches same-named locals

**Valid:** dated 2026-11-10

**Kind:** misleading-output · **Tool:** `references` · **Severity:** med

**Status:** open

**Fix:** committed in `5d0ee2421` (patch-id `655d6d953b36bfcec6582a981e21b51229116bdb`), 2026-11-11. The zero-with-scan-hits warning now names what was proven, not a cause. Not yet seen on a rebuilt server.

**Confidence:** code path verified, outcomes sampled

18 of 248 successful `references` calls (2026-10-12 to 2026-11-10 UTC) return this warning: "LSP returned 0 references outside the definition file, but `<sym>` appears as a whole word in N+ other source file(s) … the reference index may still be warming after a reindex." The text is built at `src/tools/symbol/references.rs:493`. In 12 of 12 symbols checked, the other-file hits were same-named locals, doc comments, tests or fields. So the LSP answer was right and the stated cause was false. ADR 2026-08-27 clause 3 says: claim only what is proven.

**Detail:** [references-zero-warning-blames-a-warming-index.md](references-zero-warning-blames-a-warming-index.md)

## FR-2 — `run_command` timeouts and failing commands are recorded as `success`

**Valid:** dated 2026-11-10

**Kind:** silent-failure (instrument) · **Tool:** `run_command`, `usage.db` · **Severity:** high

**Status:** open

**Fix (instrument, partial):** committed in `71af942f1` (patch-id `15e4b2dcea6d18427202725674902b5ad952c1d8`), 2026-11-11. `scripts/friction-probe.py` now reads `exit_code` and `timed_out` from the stored result (an MCP content list) and reports them beside the error counts. Measured: 2,132 of 36,014 readable `run_command` successes failed (2,033 non-zero exits, 99 timeouts; 5.9%), a floor. `outcome` is unchanged. The schema change that records the facts on every row is in `8b9e1c7f2` (see FR-3), so a server without `--debug` is counted too once it runs that build. `analyze-usage` and the pika do not read the columns yet.

`outcome` records whether the tool delivered a result, not whether the command passed. All 314 `run_command` rows whose output contains `timed_out` have `outcome='success'` (149 of them with `timed_out: true`). A text-pattern match also finds at least 1,734 success rows with a non-zero `exit_code`. Example: row 221568 ran `cargo test`, which exited 101, and the row says `success`. So `scripts/friction-probe.py` cannot see failed builds, failed tests or timeouts.

**Detail:** [run-command-timeouts-and-nonzero-exits-are-recorded-as-success.md](run-command-timeouts-and-nonzero-exits-are-recorded-as-success.md)

## FR-3 — `usage.db` cannot show what the agent saw

**Valid:** dated 2026-11-10

**Kind:** silent-failure (instrument) · **Tool:** `usage.db` · **Severity:** high

**Status:** open

**Fix (one gap, scoped 2026-11-11):** committed in `8b9e1c7f2` (patch-id `aefd69646d344650973f082ca820dea0ea6e8fca`). Every row now records `output_bytes`, `command_exit` and `command_timed_out`, with or without `--debug`; NULL means not recorded. `output_json` stays debug-only (operator decision of 2026-11-02). Measured before the change: in the last 5 days 21,485 of 41,997 rows (51%) had no output, all without an `agent_id`; the four running servers without `--debug` were started by Codex, and the `sdd`, `kat` and `stef` Claude profiles pass `--debug`. Checked live on a rebuilt server (2026-11-11, build `340d9609b`): `sh -c 'exit 7'` wrote `command_exit` 7 with `command_timed_out` NULL (row 315925), and a 2 s timeout wrote `command_exit` NULL with `command_timed_out` 1 (row 315927); both rows read `outcome` `success`, and the probe's column section counted both as failed. Only servers that run this build write the columns, so Codex and the other profiles keep NULL until they restart on it. Old rows are not backfilled. A `client_name` column (the `clientInfo.name` of the handshake) was added in `f453c1013` (patch-id `217ce750175f4dedffeb55455a1206ed70a2d20d`); checked live on build `d0e1d3d9b` (2026-11-11): row 316355 reads `claude-code`, `output_bytes` 3237, `command_exit` 0, while the 470 rows of 7 older servers in the same 30 minutes read NULL. The name Codex sends is not yet seen. The probe breaks calls down by `client_name` (`8c7390444`; first reading of the last day: `<null>` 18,354 calls from older builds, `claude-code` 26). Not done, on purpose: `session_id` and `project_root` on `lsp_events`. Its 1,034 failed starts all lie between 2026-06-11 and 2026-11-04 13:46, with none in the 7 days since, and the server's session id is a fresh UUID per construction, so recording it means plumbing a new value through the LSP manager. Revisit if failed starts return. Rows of a refused `workspace=` pin keep NULL. Still open: hook advisories, `lsp_events` identity (`session_id`, `project_root`), probe-row tagging, and a column that names the harness.

`input_json` is empty on 22,463 of 117,422 rows (19%) and `output_json` on 43,074 (37%). Both gaps sit on parent-session rows; subagent rows are complete. The input gap is 0% from 2026-11-05. The `cs-hint` text in success rows is echoed file content, not advisories: hooks deliver advisories through `additionalContext`, which `usage.db` does not store. Probe stand-in rows ("[cs-hint] stand-in") sit in the corpus. `lsp_events` has no `session_id` or `project_root`, so LSP failures cannot join to sessions. Every rate in this ledger is therefore partial.

**Detail:** [usage-db-recording-gaps-limit-every-friction-rate.md](usage-db-recording-gaps-limit-every-friction-rate.md)

## FR-4 — The LSP-backed tools are used in a minority of edit sessions

**Valid:** dated 2026-11-10

**Kind:** other (adoption) · **Tool:** `references`, `call_graph`, `symbol_at` · **Severity:** med

**Status:** open

107 of 462 sessions (23%) called `references`, `call_graph` or `symbol_at`. Of 250 sessions with an edit, 75 (30%) called `references` or `call_graph` at some point, and 35 (14%) did so before the first edit. For subagents the before-first-edit share is 11% (24 of 224). The corpus holds 260 `references`, 130 `call_graph` and 7 `symbol_at` calls against 50,904 `run_command` calls. Cost and trust look like stronger drivers than unawareness: `references` has a p90 of 15.7 s and 43 of 260 calls took 10 s or more, and 5 of 7 `symbol_at` successes say "no definition resolvable". The `symbol-navigation` guide reached 233 sessions.

**Fix (one part only):** `cf3595fb2` (patch-id `e9cc7aeb4f8bb69739e365ed6f077f381b683d46`) makes the client declare `textDocument.callHierarchy` to `typescript-language-server`, in `LspClient::initialize` and in the mux handshake. Verified on 2026-11-12 with an `initialize` probe of four installed servers: `callHierarchyProvider` is absent from `typescript-language-server` without the declaration and `true` with it; pyright, gopls and rust-analyzer return `true` either way. So only that server is sent it. kotlin-lsp and jdtls were not probed and keep what they have always seen. This removes one cause of tree-sitter-only `call_graph` on TypeScript. It does not address adoption or the cost of the first call, and it does not show that the LSP edges are better than the tree-sitter edges for TypeScript. Both stay open: the latency side is a decision about pre-warming rust-analyzer (see memory `research/lsp-cold-start-latency-20261110`), which needs a measurement on a quiet machine. Live check: after a rebuild, the edges that `call_graph` returns for a TypeScript symbol should carry `source: "lsp"` where they carry `source: "ts"` today (`src/tools/symbol/call_graph/mod.rs:602`). Not yet run.

**Detail:** [lsp-tools-are-used-by-a-minority-of-edit-sessions.md](lsp-tools-are-used-by-a-minority-of-edit-sessions.md)

## FR-5 — `symbols` answers "0 matches" as success when the language server failed

**Valid:** dated 2026-11-10

**Kind:** misleading-output · **Tool:** `symbols` · **Severity:** med

**Status:** open

**Checked 2026-11-11, no change:** the zero already carries a warning that says "this 0 is not evidence the symbol is absent" (`src/tools/symbol/symbols.rs`, `display.rs`), so the negative result names its scope. Turning it into an error is a design choice, not a defect fix; it is not made.

71 successful `symbols` calls (2026-10-16 to 2026-11-09) return "0 matches" plus a warning that the language server gave no symbols. 46 say it answered `documentSymbol` with null, 19 say another mux instance holds the lock, 5 say it could not read the file for `didOpen`. The outcome is `success`. In the next 3 calls, 29 of 70 sessions use `run_command`, 25 use `grep` and 24 use `symbols` again. So agents catch the false zero, but they pay calls. The mux-lock cause is a known, mitigated bug.

**Detail:** [symbols-zero-matches-hides-a-failed-language-server.md](symbols-zero-matches-hides-a-failed-language-server.md)

## FR-6 — `edit_code` still uses the target-blind worktree guard

**Valid:** dated 2026-11-10

**Kind:** gate-too-strict · **Tool:** `edit_code` · **Severity:** med

**Status:** open

**Fix:** committed in `824ecf61c` (patch-id `c9e15516f2c71db5f935cadeee1503c1da5ff71b`), 2026-11-11. `edit_code` guards the write target, not the session, against the worktree rule. Not yet seen on a rebuilt server.

`edit_code` refuses writes with `worktree_activate_required` even when the target lies outside every worktree. 20 rows, 15 sessions, 2026-10-15 to 2026-11-09. Post-fix examples: row 262016 (2026-11-04, a file under `/tmp/codex-wake-review/`) and row 269790 (2026-11-06). Cause, verified at the source: `edit_code.rs:239` calls `guard_worktree_write(ctx)`, the session-level guard, while `create_file.rs:62`, `edit_file/mod.rs:538` and `approve_write.rs:48` call the target-aware `guard_worktree_write_for_target`. The 2026-10-26 fix covered three of the four write tools. The next call after the 20 refusals was `workspace(activate)` 8 times, `run_command` 4 times (once a `cp` into `/tmp`), and another blocked `edit_code` 4 times.

**Detail:** [edit-code-worktree-guard-ignores-the-write-target.md](edit-code-worktree-guard-ignores-the-write-target.md)

## FR-7 — `path_not_found` never names an existing neighbour

**Valid:** dated 2026-11-10

**Kind:** discoverability · **Tool:** `read_file`, `symbols`, `edit_file`, `doc`, `run_command` · **Severity:** med

**Status:** open

**Fix:** committed in `5bccb5bdf` (patch-id `435d16659a685a781431dbd04c8bdea81a4348d8`), 2026-11-11. The hint of `read_file`, `read_markdown` and the read-path resolver now names the nearest existing directory and the entries beside it. Checked live once on a rebuilt server: a read of `src/tools/run_command.rs` named `run_command/`. Not covered: `edit_file`, `doc` and `run_command` not-found errors, and the removed-worktree twin of a path. Status stays open until `usage.db` shows the effect.

`path_not_found` errors never name an existing neighbour. 309 rows: `read_file` 224, `symbols` 65, `edit_file` 7, `run_command` 7, `doc` 5, `references` 1. The causes are guessed doc slugs (82 `read_file` rows), a file that became a directory (`src/tools/run_command.rs` is now `src/tools/run_command/`), and paths under removed worktrees (17 `read_file` rows). The hint says "Check the path with tree" or "Use tree to explore". Of 7 `symbols` rows with a visible input, 0 show a corrected path next; all 7 went to `read_file`, `grep` or `run_command`. Agents recover, but by routing around, not by following the hint.

**Detail:** [path-not-found-errors-name-no-existing-neighbour.md](path-not-found-errors-name-no-existing-neighbour.md)

## FR-8 — Managed-artifact refusals lead with what is allowed and end in shell reads

**Valid:** dated 2026-11-10

**Kind:** wrong-guidance · **Tool:** `edit_file`, `read_file` · **Severity:** med

**Status:** open

**Fix (edit_file half):** committed in `61d06f320` (patch-id `5e64ca01601625ae67bb11b2adf2810f22f315b6`), 2026-11-11. The stamped-file refusal now opens on what is refused and gives the body route before the frontmatter route. The 110 `read_file` refusals and the shell-read route are unchanged: the hint already names `doc(get)`, and the guard cannot see the shell. Checked live on a rebuilt server (2026-11-11): a plain `old_string`/`new_string` `edit_file` call on a stamped bug file was refused with the new opening, the body route before the frontmatter route and the closing line that reads are never refused; nothing was written. Whether the 14 immediate retries per 90 refusals drop is not yet measured.

Librarian-managed refusals cost 200 rows: `edit_file` 90 (40 sessions) and `read_file` 110 (68 sessions). The `edit_file` message opens with "Reads and BODY edits are allowed directly, but only via a call that can prove it stays in the body", then refuses the plain `old_string`/`new_string` form the agent just used. 14 of the `edit_file` refusals were immediately retried as `edit_file`. For the 110 `read_file` refusals, the next call was `doc(get)` 36 times, the same `read_file` 21 times and a shell read 25 times (for example `sed -n '60,300p' docs/trackers/…`, row 182655). The shell route makes the guard advisory. This session met the same refusal when it read `docs/trackers/frictions/index.md`.

**Detail:** [managed-artifact-refusals-lead-with-what-is-allowed-and-end-in-shell-reads.md](managed-artifact-refusals-lead-with-what-is-allowed-and-end-in-shell-reads.md)

## FR-9 — The `il1` source-read gate is followed by a plain repeat in a fifth of cases

**Valid:** dated 2026-11-10

**Kind:** hint-ineffective · **Tool:** `read_file` · **Severity:** low

**Status:** open

**Checked 2026-11-11, no change:** the idea of this entry already exists. The refusal in `src/tools/read_file.rs` names up to three overlapping symbols and builds a ready `symbols(name=..., include_body=true)` call, and it orders the two escapes by measured thresholds. A comment there records that a stronger always-loaded text was measured and refuted. The 75 repeats are an agent habit, not missing text.

The source-read gate `il1_read_overlaps_symbol` fired 354 times (35% of all `read_file` errors). Of 327 rows with a visible input, 181 (55%) got the content in one call: `symbols` 96, `read_file` with `force=true` 85. The residue is 75 plain `read_file` repeats that failed again, 43 of them on the same file. The gate itself is healthy; the repeat shows that the hint text is not always read.

**Detail:** [il1-gate-is-followed-by-a-plain-repeat-in-a-fifth-of-cases.md](il1-gate-is-followed-by-a-plain-repeat-in-a-fifth-of-cases.md)

## FR-10 — Edit acknowledgements do not confirm what is now in the file

**Valid:** dated 2026-11-10

**Kind:** other (response shape) · **Tool:** `edit_code`, `edit_file` · **Severity:** low

**Status:** open

After 923 of 10,685 successful edits that carry a path (9%; 188 principals, 108 sessions), the agent reads the same path again within 2 calls. 494 of those re-reads use `read_file` with `force=true`. The acknowledgement is `{status, wrote_to, rel_path}` only. Separately, `edit_stale_match` fired 147 times on `edit_file` (49 sessions), and for 73 of 143 path-bearing rows the agent had made a successful edit to that file after its last read. One fact weakens the obvious fix: the median acknowledgement is 158 bytes for edits that were re-read and 148 bytes for the rest, so acknowledgement size does not predict a re-read.

**Checked 2026-11-11, no change.** The acknowledgement size gives no support to a fix (see Detail). The stale-match half already has a repair path: the `edit_file` miss message names the nearest content and its line span (`not_found_msg`, in place since `857f9fc56`, 2026-06-04, so it was live for the whole window). Measured on `usage.db`, 30 days to 2026-11-11, filter `tool_name='edit_file'`, `outcome<>'success'`, `error_msg LIKE '%old_string not found%'`: 160 misses with a following call in the same session. The next call was `edit_file` for 83 (77 succeeded, 6 failed), `grep` 28, `read_file` 23, `run_command` 12, `edit_code` 8, `symbols` 5, `create_file` 1. So 48% of misses end with a successful retry in one call. A miss costs one call, not lost work. The count differs from the 147 above because the filter differs. Not shown: whether the retries used the nearest-content text.

**Detail:** [edit-acknowledgements-do-not-confirm-what-is-now-in-the-file.md](edit-acknowledgements-do-not-confirm-what-is-now-in-the-file.md)

## FR-11 — Agents pass a sibling tool's parameter name

**Valid:** dated 2026-11-10

**Kind:** param-confusion · **Tool:** `edit_code`, `edit_file`, `grep`, `doc` · **Severity:** low

**Status:** open

**Fix (edit_code part):** committed in `54e9194fa` (patch-id `445e47d5da2926863d220e23341990b56d4637cd`), 2026-11-11. `edit_code` accepts `code`, `new_body`, `new_code` and `new_source` for `body`. Also in `c2b8a572d`: `grep` honours `head_limit` as the result cap when `limit` is absent.

Agents pass the parameter name of a sibling tool, 143 rows in all. `edit_code`: 34 `missing_required_param` rows (29 sessions) with `code`, `new_body`, `new_code` or `new_source` for `body`, and `name` or `name_path` for `symbol`. `edit_file`: 56 rows (26 sessions) with `body` for `new_string` and `old` for `old_string`. `grep`: 38 `param_not_applied` rows with `output_mode` (23 `count`, 9 `files_with_matches`, 6 `files`) for `mode`. `doc(update)`: 15 rows with custom patch keys (`closed`, `unverified`, `heading`, `patch_note`). Recovery is quick: 71% of the `edit_file` rows and 20 of 34 `edit_code` rows were followed by a success. The errors are actionable. The recurrence shows that the name is the agent's default.

**Detail:** [agents-pass-a-sibling-tools-parameter-name.md](agents-pass-a-sibling-tools-parameter-name.md)

## FR-12 — `edit_code` was retried ten times on a null `documentSymbol`

**Valid:** dated 2026-11-10

**Kind:** hint-ineffective · **Tool:** `edit_code` · **Severity:** med

**Status:** open

**Fix:** committed in `ae0672af7` (patch-id `8f28b3431acea84317d3d2dd6cac6c78ec9d10e2`), 2026-11-11. A repeated null for one file changes the advice from the third call. Not run against a live rust-analyzer. The hint text is shared by `references`, `symbols` and `edit_code`.

22 `lsp_document_symbol_null` rows (21 insert, 1 replace; 8 sessions; 2026-10-24 to 2026-11-01). One session retried the same `insert` ten times between 12:41 and 13:12 UTC on 2026-10-31 (ids 240585 to 240698), on `src/tools/run_command/attribution.rs`. Between the retries it ran `run_command` sleeps of 60, 90 and 120 seconds and `pgrep`. `edit_file` was used four times to route around and each worked. The error says "Retry shortly" and never changes after repeats. The causes are known and fixed or mitigated (a null window that outlasts the 5 s budget, and a rust-analyzer watcher that walks symlinked directories). The remaining friction is the advice.

**Detail:** [edit-code-retried-the-same-insert-ten-times-on-a-null-document-symbol.md](edit-code-retried-the-same-insert-ten-times-on-a-null-document-symbol.md)

## FR-13 — A rename of a Python method inside `replace` is reported as a dropped definition

**Valid:** dated 2026-11-10

**Kind:** misleading-output · **Tool:** `edit_code` (replace) · **Severity:** med

**Status:** open

**Fix:** committed in `ba299e6bf` (patch-id `70afc992b3b2d4dc4c3bd177475fae5060fcf7f5`), 2026-11-11. The real shape was a body that declares several symbols, not a rename: the refusal now says so and routes the extras to `insert`. The ledger's Python `pre_set` hypothesis did not reproduce.

A rename hidden inside a `replace` body on a Python method is reported as "dropped the symbol definition", not as a rename. Two rows are confirmed (223327 on 2026-10-28 and 232089 on 2026-10-29), inside a family of 30 `replace_dropped_sibling` rows (12 sessions). In row 223327 the body declares `def test_errored_run_with_no_model_work_is_invalid` for the symbol `IntegrityTest/test_errored_run_is_invalid`, which is a pure rename. The same cause on Rust gets the right message ("`replace` cannot rename", row 248940). Of 3 sampled rows, 0 recovered with a `replace` on the same symbol: 2 went to remove plus insert and 1 to `edit_file`. The archived bug of 2026-09-13 fixed the rename message, and this Python shape still falls through. The mechanism is a hypothesis, not reproduced.

**Detail:** [replace-rename-of-a-python-method-is-reported-as-a-dropped-definition.md](replace-rename-of-a-python-method-is-reported-as-a-dropped-definition.md)

## FR-14 — Wait loops are two thirds of the slowest `run_command` calls

**Valid:** dated 2026-11-10

**Kind:** latency · **Tool:** `run_command` · **Severity:** med

**Status:** open

**Fix (one shape):** committed in `22e322857` (patch-id `ab32a999bfd43e79b4f4105d129c86b64c0baead`), 2026-11-11. `run_command` refuses a `pgrep -f` wait loop, which can never end. 29 calls ran one (15 of them 25 s or longer). Other wait shapes (`while kill -0`, `until` on a file, plain `sleep`) are not covered, and the 344 slow loops of this entry are probably mostly those.

344 of the 512 slowest `run_command` rows (the slowest 1%) are wait loops that use `until`, `while`, `sleep` or `pgrep`. 166 of them ended at their timeout, and all 344 are recorded as `success` (FR-2). Build and test rows are 125 of the 512 and show no errors. Example: row 197597, 3,000,065 ms, `while pgrep -f "cargo test --lib$" …; do sleep 10; done`. Two timed-out rows poll with `pgrep -f "cargo test --lib symlinked_temp_dir"`, which matches its own command line (a trap the progressive-disclosure guide already records). Overall latency: p50 94 ms, p90 10,897 ms, p99 291,621 ms. The wake-hook guidance says to start long jobs with `run_in_background` and to end the turn, not to poll.

**Detail:** [run-command-wait-loops-are-two-thirds-of-the-slowest-calls.md](run-command-wait-loops-are-two-thirds-of-the-slowest-calls.md)

## FR-15 — The `il3` gate refuses scratchpad writes with an unexpanded `$VAR` path

**Valid:** dated 2026-11-10

**Kind:** gate-too-strict · **Tool:** `run_command` · **Severity:** low

**Status:** open

**Fix (advice only):** committed in `0e1073e13` (patch-id `77e03a9222e115de427255e29b34ae3229c208f1`), 2026-11-11. The verdict is unchanged, by design: an unresolved path is blocked conservatively. A clause that only writes such a path now gets write advice (`create_file`, or the path written out) and not "use read_file". The regression question is settled by the code: the block is intended.

The `il3_shell_on_source` gate refuses scratchpad writes whose path starts with an unexpanded `$VAR`. 96 rows, 36 sessions, 2026-10-13 to 2026-11-09, which is after the closing of the bug of 2026-09-10 that covered the same mechanism. Example: row 291458, `cat > $S/opt.sh <<EOF`, refused with "this clause contains an unexpanded shell expansion … blocked conservatively". Of 5 sampled rows, 5 recovered on the next call; only 8 of 96 were followed by an `acknowledge_risk` rerun. Whether this is a regression of the fixed bug or a heredoc-only gap is not verified, because no code was read.

**Detail:** [il3-gate-refuses-scratchpad-writes-with-an-unexpanded-variable-path.md](il3-gate-refuses-scratchpad-writes-with-an-unexpanded-variable-path.md)

## FR-16 — `semantic_search` is almost unused

**Valid:** dated 2026-11-10

**Kind:** discoverability · **Tool:** `semantic_search` · **Severity:** med

**Status:** open

**Checked 2026-11-11, no change:** in a worktree with no index the tool answers with empty results and a hint to run `index(action="build")`, which is a correct negative result. No row shows an agent misled. Every cause of the low use is still a hypothesis.

`semantic_search` has 14 calls in the window, against 9,189 `grep` calls. 4 of the 14 returned "No index for worktree project" with a hint to run `index(action="build")` (rows 199006, 201497, 206247, 207525), all in worktree sessions. The usage data shows no `index` call after any of the four; this was not checked in full. About 4,700 of the 9,189 `grep` patterns are plain multi-word text, by a crude filter, so concept-shaped queries probably go to `grep`. The cause of the avoidance is a hypothesis. The empty result is actionable, but only if it is read.

**Detail:** [semantic-search-is-almost-unused.md](semantic-search-is-almost-unused.md)

## Template for new entries
