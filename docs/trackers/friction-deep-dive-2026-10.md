---
id: '49dcc2fa7fec2e8a'
kind: tracker
status: draft
title: Friction deep dive 2026-10 — every error group, where the pain is
---

# Friction deep dive 2026-10 — every error group, where the pain is

Opened 2026-10-26 by the `codescout-kat` session. The operator asked for every error group in `usage.db` to be classified and opened, to find structural issues and bugs. This tracker holds the results, one section per group, and the bugs each group produced.

## Scope and instruments

- **Corpus:** this project's `.codescout/usage.db`, 2026-09-27 04:39 .. 2026-10-26 19:14 UTC. 73,207 calls: 70,467 `success`, 2,737 `recoverable_error`, 13 `error`. 2,750 errors (3.76%). 4,767 successes overflowed.
- **Instrument:** `sqlite3` over `tool_calls`, with `err_family`, `agent_id`, `session_id`, `error_msg`. `scripts/friction-probe.py` (fixed in `56fb01cf4`) for repeat and recovery rates.
- **Blind spots.** `usage.db` sees MCP calls only. The 30-day retention sweep makes every count a floor. There is no data for 2026-10-25. The corpus is this project only (the 2026-10-24 usage report covered 40 projects). Rates are not mix-controlled.
- **Reading rule.** Frequency is not friction (`2026-08-15-tool-usage-investigation:TU-7`). A family counts as friction only with a repeat rate, a recovery cost or a structural cause. A rising or falling trend is a window fact, not a fix receipt, unless a commit boundary shows a flatline.

## Groups

| Group | Families | Errors | Status |
|---|---|---:|---|
| G1 Iron-Law gates | `il3_pipe_to_trimmer` 692, `il3_shell_on_source` 478, `il1_read_overlaps_symbol` 245, `il2_structural_edit` 185 | 1,600 | investigated |
| G2 Edit and write path | `edit_stale_match` 128, `replace_dropped_sibling` 40, `target_already_exists` 37, `ambiguous_old_string` 27, `edit_would_break_syntax` 18, `ast_extent_fail` 9, `write_scope_denied` 2, `destructive_replace_blocked` 1 | 262 | investigated |
| G3 Workspace, worktree and paths | `path_not_found` 143, `worktree_activate_required` 72, `cwd_escapes_root` 9 | 224 | investigated |
| G4 Artifacts, memory, headings | `librarian_managed_artifact` 173, `memory_target_not_found` 23, `heading_not_found` 11, `ambiguous_heading` 1 | 208 | investigated |
| G5 Parameters and schema | `missing_required_param` 70, `unknown_enum_value` 10, `invalid_line_range` 10, `invalid_regex` 6, `mutually_exclusive_params` 3 | 99 | investigated |
| G6 Buffers and `json_path` | `buffer_ref_expired` 30, `json_path_key_miss` 11, `json_path_unsupported` 7, `json_path_wrong_buffer_kind` 5, `json_path_shape_mismatch` 3 | 56 | investigated |
| G7 LSP and symbols | `symbol_not_found` 5, `lsp_index_locked` 4, `lsp_still_starting` 1, `lsp_not_running` 1, `ambiguous_name_path` 1 | 12 | investigated |
| G8 Unclassified (`err_family` NULL) | 289 rows across 10 tools | 289 | investigated |
| G9 Output overflow | 4,767 successful calls that overflowed | — | investigated |

## Findings

Each section states what was measured, the structural cause if one was found, and what it produced. Rates are from this project's `usage.db`. "Repeat" and "recovery" are from `scripts/friction-probe.py --days 14 --calibrate` (2026-10-12..26). Subagents investigated G2 to G7 and the controller verified their claims against source; anything unverified is marked.

### G1 — Iron-Law gates (1,600 errors, 58% of all errors)

**Common shape.** All four gates are cheap to recover from: median calls-to-recovery 1, no unrecovered errors, immediate-repeat 1.2% to 25%. Volume concentrates in a few subagent campaigns. Sessions `e98b5705` (10-16..18) and `500c9b13` (10-17..19) are the top session for three of the four families and also caused the `path_not_found` bursts.

**`il3_pipe_to_trimmer` — 692 errors, 90 sessions, 443 from subagents. Healthy gate; one residual class.**
- Top four sessions hold 302 rows (44%). Daily peak 84 on 10-16 and 111 on 10-17 (see `il3_pipe` per-day counts).
- The refusal text changed with the run-to-completion change (`docs/superpowers/specs/2026-10-21-il3-run-to-completion-pipe-design.md`, `pipe_refusal` in `src/tools/run_command/mod.rs:303`): first row naming "the capture cannot be spliced" is 2026-10-22 15:20. A foreground single pipeline now runs through a capture and is allowed.
- After 2026-10-22 06:00 there are 36 refusals in 12 sessions: **33 are compound commands** (`;`, `&&`, `||` or newline around the pipe), 2 are background runs, 1 other. Example (10-24): `ls docs/issues | grep X ; git log --oneline | head -3; ls docs/issues | wc -l`.
- Reading: the 10-21 change removed the single-pipe class. What remains is the design's stated limit for compound commands. Candidate (not a bug): when the pipe sits in a `;` list, capture the whole command's stdout instead of refusing. Feasibility unverified.

**`il3_shell_on_source` — 478 errors, 66 sessions, 365 from subagents. Healthy gate; one design cost.**
- 103 rows (22%, 24 sessions) carry the "unexpanded shell expansion" note. The usual shape is `S=<scratchpad path>; ... cat > $S/x.py <<'EOF'` or `sed -n ... $S/file.rs`. The gate blocks conservatively; this is pinned by `an_unexpanded_expansion_still_blocks` (`src/util/path_security.rs:4656`). 14 more rows are heredoc writes.
- Most-repeated plain commands are reads of project shell scripts: `sed -n 1,60p scripts/with-slot.sh`, `scripts/mutation-probe.sh`, `cat build.rs | wc -l`.
- Probe: `sed -n 1,5p` of a `.sh` file OUTSIDE the project root ran without refusal, so the gate is project-scoped.
- Candidate (not a bug): resolve a literal `VAR=...;` assignment earlier in the same command before judging the path. Security trade-off unverified.

**`il1_read_overlaps_symbol` — 245 errors, 59 sessions, 145 from subagents. Gate with a 27% override rate.**
- Repeat rate 25.2% against TU-7's 14% (calibration ratio 1.8).
- The next call after a refusal: `read_file force=true` 27% (65), another `read_file` refusal 18% (44), `symbols` 14%, `grep` 14%, `run_command` 10%. Roughly a quarter of agents override at once, so the gate gave them no value.
- Symbols named in samples are shell functions (`usage`, `count_lit`, `cleanup`): the gate treats functions in `.sh` scripts as navigable symbols.

**`il2_structural_edit` — 185 errors, 39 sessions, 151 from subagents.**
- 137 on `.rs`, 44 on `.py`. The message names `edit_code`. Next call: `edit_code` success 28%, `edit_file` success 27%, `run_command` success 17%.
- Some `run_command` follow-ups write source by shell, e.g. `printf '...' >> src/tools/read_file.rs`. Whether the gates see a shell redirect into a source file is **unverified**.

### G2 — edit and write path (262 errors)

Subagent report, controller-verified at source where marked. All eight families recover in a median of 1 call with 0% unrecovered. Repeat rates 0% to 5.6%.

- **`edit_stale_match` (128; 37 sessions; 75% subagent).** Not staleness. A check for a same-path write by another principal in the prior 30 minutes found 2 of 155 failures, so concurrent editing is not the cause. 63 are `edit_file` batch failures. **Verified:** the batch path (`src/tools/edit_file/mod.rs:651-655`) answers `edit[i]: old_string not found` with the hint `Batch aborted — no changes written.`, with no path and no nearest content, while the single-edit path (`not_found_msg`, `mod.rs:276`) returns `Nearest content at lines s-e`. Subagent counts, unverified by the controller: 45 of 63 failing `old_string`s start with whitespace; 27 retries succeeded after a whitespace change.
- **`edit_file` batch ambiguous (part of `ambiguous_old_string`, 27).** **Verified:** `mod.rs:661` says `found N times` with no line numbers; the single path names the lines.
- **`replace_dropped_sibling` (40; 83% subagent).** **Verified wording:** `TargetDropped` (`edit_code.rs:1430`) tells the agent the body must be the complete declaration "not just body statements"; the `SiblingsDropped` text (`edit_code.rs:~400`) ends with "likely a stale LSP range". Subagent counts, unverified: in 26 of 29 `TargetDropped` cases the body did not contain the target's name; 10 of 11 `SiblingsDropped` cases dropped children of the replaced symbol.
- **`target_already_exists` (37; 84% subagent).** 35 of 37 target scratch or `/tmp` paths and 29 followed an earlier write in the same session. Healthy gate; whether retries used `overwrite` is unverified.
- **`edit_would_break_syntax` (18), `write_scope_denied` (2), `destructive_replace_blocked` (1).** Healthy gates that name the corrective call.
- **`ast_extent_fail` (9; 8 sessions).** **Verified:** `edit_code.rs:~1595` answers "the file parses cleanly, but the AST does not pinpoint this symbol's end" and the hint names only the nested-impl case. The subagent reports targets were Python module constants (`_VALID_LABELS`, `INTERRUPT_MARKERS`, `SECTION_SIGNATURES`), an `impl ServerEnv` and a struct field. Why the extractor has no end for them is unverified.

### G8 — unclassified (`err_family` NULL), 289 rows

Not one problem: ten tools and many small message shapes. Counts (rows / sessions): `edit_code` other 81/30; `read_file` section exceeds inline threshold 48/30; `read_file` other parameter shape 46/27; `doc` other shape or validation 43/18; `doc` fixed bug declares no fix anchor (`move` or `update`) 32/17; `edit_file` shape 15/9; `librarian` other 12/7; LSP `documentSymbol` null 4/2.

The largest new-family candidates, in order: `section_exceeds_inline_threshold` (48), `fixed_bug_no_fix_anchor` (32), `edit_code` shrink guard (23 rows, 10 sessions, 16 from subagents; the next call is `edit_code` success 7, with `force=1` 4). Adding them to `ERR_FAMILIES` (`src/usage/db.rs`) re-classifies history on next open. An existing bug file covers a related NULL head (`docs/issues/2026-10-24-err-family-overflow-branch-keys-on-retired-read-markdown…`).

Open: whether `section exceeds inline threshold` is friction or the designed progressive-disclosure step. It costs a second call in 30 sessions.

### G9 — output overflow

4,767 of 70,467 successful calls overflowed (6.8%): `run_command` 3,318 (11.2% of its calls; 2,124 from subagents), `symbols` 590 (10.8%), `grep` 424 (5.2%), `doc` 239 (8.8%), `librarian` 112 (48.1% of 233 calls), `read_file` 74 (0.6%). Per 1,000 calls by week: 58.0, 74.4, 66.1, 59.9 (w2..w5).

After a `run_command` overflow (10-12..10-26, about 1,800 rows) the next call in the session queries the `@cmd_*` buffer in 13.0% of cases; the next call is another `run_command` in 52.0%. This next-call measure undercounts later retrieval (`docs/PROBES.md` sizes the same blind spot for `read_file` at 22% next-call against 42% by session end), so it bounds nothing. Whether overflow output was wasted is **not established**.

### G3 — workspace, worktree and paths (224 errors)

- **`path_not_found` (143; 31 sessions; 71% subagent; repeat 33%, recovery median 1, p90 4).** Four subagent `read_file` bursts of relative `docs/issues/*.md` paths make 87 of the 113 `read_file` errors: `500c9b13` 32, `e98b5705` 24, `5b936994` 18, `726b843c` 13. Two causes, both traced:
  - **A, a race on the shared active-project slot (verified in `usage.db`).** In `5b936994`, ten subagents activated ten different worktrees between 03:08:14.027 and 03:08:17.894 on 10-24. Unpinned reads by agents `aa685925`, `a4d168bd` and `aa922711` then searched the worktree of agent `a1ec5bbc`. Agent `aac3dcf3` pinned `workspace=` and searched its own tree. The cause is the companion hook `subagent-guidance.mjs` (lines 44-52), which tells every subagent to `activate` its own root, against `get_guide("workspace-state")` ("Brief subagents to pin; do not brief them to activate") and the 2026-10-16 decision that unpinned concurrent use is unsupported. 246 `workspace(activate)` calls in the window, 143 from subagents.
  - **B, a base mismatch (subagent finding, partly verified).** The pinned agent's reads still failed: the parent briefed paths from the main checkout, and the worktree HEAD predates the archive move `8f480f02f`.
  - **Hint gap (verified).** The `read_file` not-found hint (`src/tools/read_file.rs`, `read_file_text`) says "call workspace(action='status')" and never names the `workspace=` pin that `guard_worktree_write` already names.
  - **`@bg_*` handles (verified, see G6).** 9 `read_file` calls on `@bg_*` handles failed as `path_not_found`.
- **`worktree_activate_required` (72; 32 sessions; 33% subagent).** The message names both remedies and agents learned them (subagent: 15 `create_file` retries carried a pin, 11 sessions activated). Volume fell 42, 19, 9, 2 per week. **Verified:** the guard (`src/tools/core/guards.rs:47`) takes only the context and never inspects the write target, and 36 of the 72 rows (19 sessions) have an absolute target outside the project root. Bug filed.
- **`cwd_escapes_root` (9; 9 sessions).** Healthy; every agent recovered in one call by `cd X && cmd`. Subagent: the message omits `workspace=`. Unverified by the controller.

### G4 — artifacts, memory, headings (208 errors)

- **`librarian_managed_artifact` (173; 58 sessions; 34% subagent; repeat 14%).** `edit_file` 101, `read_file` 72. Subagent breakdown: `edit_file` stamped 84, ledger 12, augmented 5; `read_file` ledger 52, augmented 20. Top paths: `docs/trackers/issue-clusters.md` 27 (21 reads), `bug-fix-session-log.md` 7, then several trackers with 2 to 3 each; the rest are bug files.
  - Ledger and augmented refusals are right.
  - **Verified:** `edit_file` passes `Access::FrontmatterWrite` unconditionally (`src/tools/edit_file/mod.rs:803-811`, comment "the conservative value, not a claim"), so a plain `old_string` edit in the BODY of a stamped file is refused as if it touched the frontmatter. Subagent count, unverified: 56 refusals in 29 sessions carried a plain text edit. This is a documented design trade-off (`docs/issues/archive/2026-09-01-artifact-create-stamps-an-id-that-guard-locks-the-file.md`), so it is a **candidate, not filed**: compute whether the match span touches the frontmatter and pass `BodyWrite` if not.
  - **Verified:** hints print a literal `<id>`, and a ledger READ refusal has no read route (reproduced on `docs/templates/session-log.md`). Bug filed.
- **`memory_target_not_found` (23; 13 sessions).** Subagent: 14 are "no sections matched" (agents pass `sections=["Commit"]` and the heading is "Pre-Commit Requirements"; match is exact), 8 are "topic not found" in worktree subagents (the worktree lists 16 topics, main 26). The second is the same worktree cause as G3. Both unverified by the controller; candidate, not filed.
- **`heading_not_found` (11), `ambiguous_heading` (1).** Healthy; the message lists the headings or names `occurrence=N`.

### G5 — parameters and schema (99 errors)

- **`missing_required_param` (70; 31 sessions; 57% subagent; recovery 67 of 70 in a median of 1 call).** The 2026-09-10 alias plan shipped (`docs/superpowers/plans/2026-09-10-parameter-alias-collapse.md`, `normalize_params`). Of the 70, none that kept `input_json` carried `file_path`, `relative_path`, `file`, `regex`, `query` or `rel_path`: every path miss omitted the key. So aliases cannot repair them.
  - **Verified:** `edit_file` `missing 'old_string'` is 41 rows in 20 sessions; in 25 `edits` is a string that starts `[{"new_string">` (invalid JSON); the unparseable string is dropped by `optional_array_param` and the call falls to the single-edit path. Bug filed. Peak 10-16..18.
  - Subagent, unverified: item-level `body` is ignored in `edits[]` and `body_edits[]` (`src/tools/markdown/edit_markdown.rs:881` reads only `content`; verified), 7 rows; a deterministic repair under the repair-and-continue ADR. Whether the call fails or silently drops the field was not checked. Candidate, not filed.
  - The "doubling" in the last three days is 10 rows on 10-24, 8 from one session (`cb06fa74`), so it is a burst, not a drift.
- **`invalid_regex` (6).** **Verified:** the hint is a constant that says "escape metacharacters" although all six errors are look-around. Bug filed.
- **`invalid_line_range` (10; 80% subagent).** All 10 have `start_line` above `end_line` with `force=true`; the hint says only "1-indexed" (`read_markdown.rs:623`, subagent). Possible `limit` confusion, unverified. Candidate, not filed.
- **`unknown_enum_value` (10).** Subagent: a catch-all label that holds unknown id (4), unknown repo, field, topic and action. Classifier not read by the controller. Candidate, not filed.
- **`mutually_exclusive_params` (3).** Healthy.

### G6 — buffers and `json_path` (56 errors)

- **`buffer_ref_expired` (30; 20 sessions).** **Verified by reproduction 2026-10-26:** `grep` and `read_file` refuse a live `@bg_*` handle (`buffer reference not found` and `file not found`) while `run_command("tail -3 @bg_x")` returns its log. The guide says every handle is addressable by any path-accepting tool. Bug filed. Subagent: 13 `grep` rows on `@bg_*`, none succeeded; 5 rows are `@cmd_deadbeef` test probes on the live server; 6 are `@file_*` (cause unverified).
- **`json_path_*` (26).** Subagent: the 7 "unsupported" are forward slices or filters, all on or before 10-06 and none since; 5 of 11 key misses are `$.items[*].rel_path` on `doc(find)` buffers, whose items expose `abs_path` and no `rel_path`. Hints list the real keys. Healthy; the `rel_path` omission is a candidate, not filed.

### G7 — LSP and symbols (12 errors)

Scattered, 1 to 5 each, mostly subagent, recovery in 1 call. `lsp_index_locked` (4) is the "another mux instance holds the lock" race in worktree workspaces. `symbol_not_found` (5): 4 are `edit_code` insert with a path into a nested `mod tests`. Subagent: 4 `documentSymbol`-null rows on 10-23 and 10-24 (build `2acc553ad`, before the fix commits `1b208ad79` and `a9e198d07`); builds with both fixes first appear 10-26 14:48 and carry 14 LSP-tool calls in the window, so a **flat-line is not established**.

## Bugs filed from this pass

Nine bug files, all opened 2026-10-26. Each cites the controller's own check; subagent-only numbers are marked there.

| Bug id | Severity | What |
|---|---|---|
| `2f6b6a8bcc9150c7` | medium | The companion's subagent bootstrap tells worktree subagents to `activate`; they race on the shared slot (G3) |
| `d29988a6e87331f7` | medium | `grep` and `read_file` refuse a live `@bg_*` handle with a false message (G6) |
| `c74560d325f6ea3d` | medium | `edit_file` drops an unparseable `edits` string and reports `missing 'old_string'` (G5) |
| `fc18b0e65a75bf8b` | low | The `read_file` not-found hint does not name the `workspace=` pin (G3) |
| `56dcc467826e17ae` | low | Librarian-guard refusals print `<id>`, and a ledger read has no read route (G4) |
| `09102de754efd8ae` | low | `edit_file` batch misses carry no path, nearest content or lines (G2) |
| `ecb3508a7f51f803` | low | `guard_worktree_write` ignores the write target (G3) |
| `30c21b0aa5cb249d` | low | Three recurring error shapes carry no `err_family`; the oversized-section shape is already being fixed by a peer's uncommitted `db.rs` change, two shapes remain (G8) |
| `637ac39f6a9e7f73` | low | The grep invalid-regex hint advises escaping when the cause is look-around (G5) |

Not filed, because the controller did not verify them or because they are documented design choices: plain-text `edit_file` on a stamped file (G4), memory `sections` exact match (G4), item-level `body` (G5), `invalid_line_range` hint (G5), `unknown_enum_value` catch-all (G5), `doc(find)` items lacking `rel_path` (G6), `TargetDropped` and `SiblingsDropped` wording (G2), Python-constant `ast_extent_fail` (G2), the IL-3 compound-command and `$VAR` design costs (G1).

## Cross-cutting patterns

1. **Subagent campaigns drive the volume, not steady use.** Subagents made 33% to 84% of every family that has a figure (lowest `worktree_activate_required` 33% and `librarian_managed_artifact` 34%; highest `target_already_exists` 84%). Two sessions (`500c9b13`, `e98b5705`, 10-16..19) are the top session in `il3_pipe`, `il3_shell`, `il1`, `edit_stale_match`, `path_not_found` and `missing_required_param`. A weekly rate over a corpus like this moves with one dispatch.
2. **The worktree-subagent setup is the one structural source.** The bootstrap hook prescribes the unsupported `activate` pattern; the parent briefs paths from another base; the not-found hint does not name the pin. Together they produce `path_not_found`, `memory_target_not_found` and `cwd_escapes_root`.
3. **Gates cost one call and teach.** The four Iron-Law gates and the edit-path gates recover in a median of 1 call with 0% unrecovered. The 10-21 pipe change removed the single-pipeline class; what remains is compound commands and unexpanded `$VAR` paths.
4. **Hints composed without the request** recur: the `workspace=` pin (G3), the `<id>` placeholder (G4), the batch edit misses (G2), the regex hint (G5). A shared habit, so one sweep of hint sites against the "names the remedy" test would find more.
5. **An instrument gap:** `err_family` NULL grew because new guards were added without families (G8).

Open: the share of overflow output that was never read (G9) is not established; the next-call measure does not bound it.
