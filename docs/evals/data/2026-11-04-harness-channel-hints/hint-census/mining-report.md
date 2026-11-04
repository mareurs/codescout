# Hint mining report (read-only; all numbers from commands run 2026-11-04)

Population searched
- .codescout/usage.db tool_calls: 94,686 rows, 2026-10-05 19:16 to 2026-11-04 19:16. 3,331 non-success rows.
- Transcripts: top-level `*.jsonl` of ~/.claude*/projects/-home-marius-work-claude-codescout (213 files over the 4 profiles; mtimes 2026-08-27..2026-11-04 for ~/.claude). Subagent transcripts (subdirs) NOT scanned, so hook-delivery counts are undercounts.
- Trackers read: friction-deep-dive-2026-10 (G2,G3,G4,G9, cross-cutting), codescout-usage-frictions U-50, U-52, codescout-usage-hookify headings/status, tool-usage-patterns T-27, T-34.
- NOT searched: skill-frictions.md, 2026-08-15-tool-usage-investigation.md, memories infra/friction-measurement and gotchas; native-tool calls (invisible to usage.db); wrong-edit / edit-then-test-failure chains; Codex/other-harness sessions.
- Scripts written (scratchpad): mine_retry.py, mine_pnf*.py, mine_wt*.py, mine_poll*.py, mine_dup*.py, mine_il1.py, mine_zero.py, mine_sess.py, mine_grepn.py, hintcount.py, bashhint*.py.

Headline finding
Almost every error family already returns a server-side error with the fix named (il3 pipe/shell, il1 overlap + symbols() call, il2 + edit_code call, grep output_mode -> mode, memory sections list, worktree activate, headings overflow, doc param shapes). A hook hint after such an error repeats text the agent just read, and a PreToolUse hint cannot beat the 1-call cost of the server refusal. Where the agent repeated the same failing call, I checked: after il3_pipe_to_trimmer 615 of 730 next calls are the same tool succeeding; il1 retries: next call same path with force 65, symbols 65, same path without force 41 (of 278). The candidates below are therefore the few cases where the server error does NOT name the cause, or where no error occurs at all.

Recency caution (Heuristic 12): several families are campaign-driven (two subagent campaigns 10-16..19 per friction-deep-dive cross-cutting #1). I give last-7d (>= 2026-10-28) counts where I computed them.

## Candidates (ranked)

### 1. Relative read in a worktree that does not contain the file
- Trigger: PostToolUse on mcp read_file/symbols with error text `file not found: '<rel>' (searched <ROOT>/<rel>)` where ROOT matches a git worktree (`.claude/worktrees/agent-<id>/`, `.codex/worktrees/<h>/<repo>/`, `.worktrees/<n>/`). All fields are in the error string plus hook input (agent_id).
- Hint (two variants, pick by check):
  - A (file only absent from this worktree): "`<rel>` is not in this worktree (HEAD <sha7>) but exists in the main checkout: `read_file("<MAIN>/<rel>")`."
  - B (searched root is another agent's worktree): "This read searched agent-<X>'s worktree, not yours: repeat it with `workspace="<own worktree abs path>"`."
- Evidence (measured): 92 error rows since 10-06 whose searched root is a worktree (read_file mostly; symbols/others in the family); 61 of them name a rel path that exists in the main checkout now; 9 of the 61 since 10-28 (e.g. ids 259676, 260847 in a codex worktree). Earlier burst: 79 read_file errors, 33 agents (ids 190255-190260, agent a5de56cff4863b268 searched agent-X's tree: variant B; agent aa532b30b45f11948 ids 190266-190268 failed, then read the same files by absolute main-root path, ids 190298, 190300, success). Of the 79: 54 searched the agent's own worktree (variant A), 25 another agent's tree (variant B; matches friction-deep-dive G3 cause A, a race on the shared active-project slot). Measured: absolute main-root reads from agents whose input carried a worktree `workspace=` succeeded (6 of 6 in a narrow query); read_file on absolute worktree paths with no `workspace=` succeeded 298 times, so the absolute-path remedy works. Inferred: files existed in main when the agent asked (I checked existence now, not then).
- Emit-time check: parse ROOT and rel from the error; `test -e <MAIN>/<rel>` and `! test -e <ROOT>/<rel>`; for B compare the id in ROOT to hook-input agent_id and check `<MAIN>/.claude/worktrees/agent-<agent_id>` exists. For A also `git -C ROOT cat-file -e HEAD:<rel>` fails.
- Cost: one call. False positive: main copy is stale or deleted on purpose in the worktree's branch; the agent reads old content. Mitigate: wording "main checkout copy"; emit for reads only, never for writes. Recency: 9 since 10-28, 0 variant-A bursts since 10-30; low-frequency now.
- Note: the tracker already identified the hint gap server-side (read_file not-found hint omits the `workspace=` pin). A server fix is the cleaner owner; the hook is a stopgap.

### 2. Polling a running background job
- Trigger: PostToolUse on mcp run_command whose command names `@bg_N` and whose output contains `"jobs":[{"handle":"@bg_N","state":"running"...}]` for that handle (seen in output_json of ids 176228, 176240, 177926), at the 3rd such poll; or a foreground `sleep N;` (N >= 30) in the same session as a live @bg job.
- Hint: "@bg_N is still running. Stop polling and end your turn; you will be woken when it ends, then read it with `run_command("tail -50 @bg_N")`."
- Evidence (measured): 978 polls of a running @bg handle; 109 handles polled >= 3 times in 55 sessions. Split at 10-30 (wake design date): before 833 polls, 90 handles >= 3, 1,248 foreground sleeps, 399 polls by subagents; since 10-30: 145 polls, 16 handles >= 3, 56 sleeps, 22 by subagents (670 background jobs started since 10-30 vs 1,127 before). Foreground `sleep N` run_command overall: 1,311 calls in 132 sessions, 349,621 s of blocked wall time (1,222 with N >= 30), 56 of them since 10-30. get_guide progressive-disclosure already says "Do not poll".
- Emit-time check: state running is in the response; the wake claim is TRUE only when (a) the caller is not a subagent (progressive-disclosure: "No wake arrives ... when a subagent started the job"; hook input has agent_id) and (b) the wake hook armed this handle (receipt store of wake-on-job-end). If either fails, do NOT emit the "you will be woken" sentence; for subagents the only true advice is "poll with one sleep per check, not a loop" which is weaker.
- Cost: zero calls (ending the turn). False positive: harness without the wake (non-Claude-Code) leaves the agent waiting forever: hence check (b).
- Recency: dropped 5x after 10-30; remaining value is modest.

### 3. `while|until ... pgrep -f '<pattern>'` wait loops (self-matching)
- Trigger: PreToolUse (or PostToolUse after timeout) on run_command whose command matches `(while|until)\b` and `pgrep -f` with a pattern containing no bracket class like `[e]val`.
- Hint: "`pgrep -f` matches this shell's own command line, so this loop never ends. Wait with `run_command("tail --pid=<PID> -f /dev/null", run_in_background=true)` or use `pgrep -f "[x]rest"`."
- Evidence (measured): 23 calls, 11 sessions, 19,596 s total latency, 11 timed out (ids 197597 3000 s, 182288 3000 s, 200653 2900 s, 201208 1500 s); only 2 since 10-21. The self-match claim is from get_guide("progressive-disclosure"); I did not run the loop to reproduce it. Inferred: the 12 non-timed-out ones ended some other way.
- Emit-time check: regex on command text; the pattern-has-no-bracket-class test is mechanical. Caveat: a pattern that cannot match its own shell (e.g. `^cargo` anchored, or pgrep -x) is a false positive; require the pattern to be a substring of the command text itself (true by construction when self-match applies: check `command.includes(pattern)`).
- Cost: one call (pid lookup + tail --pid). Risk: agent wrong-pid; low. Highest avoidable cost per instance (up to 3000 s), rare.

### 4. Zero-hit line search through run_command (extend suspicious-zero-hint)
- Trigger: companion suspicious-zero-hint.mjs only fires for tool_name === 'Bash' (verified in source line `if ((input.tool_name || '') !== 'Bash') process.exit(0)`). Agents now run shell via mcp run_command; extend the matcher and read `stdout`/`exit_code` from the run_command envelope.
- Hint: existing text ("That zero came from a line-oriented search ... phrase may wrap across a line; try `grep -Pzo`/`rg -U` or open the file"). It delivered 100 times in 13 top-level transcripts (counted from attachments).
- Evidence (measured): run_command with a simple quoted grep/rg and no pipe: 1,956; zero hits (exit 1, empty stdout) 91; multi-word phrase subset 12 (last 7d: 4); 9 of the 12 followed by another search within 2 calls (ids 170338, 170439, 170441). The 12 is a narrow regex count; the real number is higher.
- Emit-time check: the hook's own conservative parser (silent when unsure) and the empty-stdout test.
- Cost: zero or one call. Risk: the hint says "may", not "is"; under a declaration the text must stay conditional or the agent will assume wrapping.
- Frequency is low: include only because it is a verbatim reuse of audited logic.

### 5. Concrete find call after a librarian-managed refusal (weak)
- Trigger: error text `is a librarian-managed artifact`, path in hook input.
- Hint: `doc(action="find", filter={"rel_path": {"contains": "<path>"}})` then `doc(action="get", id=<id>, heading="## X")`.
- Evidence (measured): 187 errors (read_file 93, edit_file 94), 79 in the last 7d (read 47, edit 32); repeat of same family within 3 calls: 18 (edit), 7 (read). Tracker G4 verified the server hint prints a literal `<id>` placeholder and that a ledger read refusal has no read route. I hit this myself this session (read_file on codescout-usage-frictions.md).
- Emit-time check: path from the error; the find call is exact for any path. Savings versus the current server message are about one call; a server fix (print the real id) dominates. Ranked low; listed because the placeholder defect is verified.

## Existing buddy hooks (cs_heuristics.py via hook_helpers.handle_cs_tool_use)
- How often each fired: buddy does not log fires. The only per-session record is .buddy/<session>/cs_tool_log.jsonl, a rolling 50-entry tool log (`MAX_ENTRIES = 50`), deleted on some resets; it holds no hint events. So fire counts come from transcripts: unique hook_additional_context attachments (dedup by uuid), top-level transcripts only:
  - native bash cat/head/tail/sed on source: 2,428 deliveries in 62 transcripts
  - grep for concept phrase: 418 in 78
  - structural edit_file: 354 in 61
  - parallel write: 234 in 47
  - forgot-restore after activate_project: 0
  - ignored @cmd buffer: 0
  (Raw-text search for the forgot-restore and ignored-buffer strings found them only in this session's own text.)
- Stale tool names: CONFIRMED. forgot-restore keys on `mcp__codescout__activate_project` (and its message says `activate_project('.')`); usage.db has 0 calls of activate_project/find_symbol and 275 `workspace(action=activate)` calls; 0 `activate_project|find_symbol` tool_use names in the stef transcripts; the current codescout tool list has no such tools. So forgot-restore cannot ever fire (0 deliveries is consistent). The native-bash hint text says "Use `read_file` or `find_symbol`": `find_symbol` does not exist; it was delivered 2,428 times. Not verified: why ignored-buffer has 0 deliveries (it needs the PostToolUse tool_response to be a dict with `output_id`; I did not capture a live payload).
- Partially stale: structural-edit check reads top-level `new_string`/`path`. edit_file inputs: 7,825 calls, 5,312 with top-level new_string, 1,631 with `edits[]` (1,460 with edits[0].new_string), so the check is blind to the batch form. The server already refuses structural edit_file (il2_structural_edit 230 errors, 91 in last 7d), so this hint is redundant with the server error and cites BUG-027.
- Precision of the native-bash hint (my regex classification of 1,350 deliveries I could map back to their Bash command): 604 start with cat/sed/head/tail on a file (484 sed, 115 cat); 491 are a pipeline that merely ends in `| head/tail/sed` (e.g. `grep -n ... file.rs | head`), which the regex `\b(cat|head|tail|sed)\b.*?(\S+\.\w+)` also matches; 255 other (multi-line scripts). So roughly a third or more of the deliveries are not source reads. Extensions hit: .rs 826, .py 338, .json 62, .toml 56, .yml 36, .yaml 32. Whether agents then complied: NOT measured.

## Rejected candidates
1. il3_pipe_to_trimmer / il3_shell_on_source (730 / 513 errors; 208 / 159 in last 7d). Server refusal already carries the rewrite; cost is exactly the one refused call either way. 189 of the 208 recent pipe errors are the `cmd | tail; echo $?` compound shape; the largest caller (22 errors) spans 2 days, so no tight retry loop. A hook adds no saving unless it rewrites input, which is not a hint.
2. Prior-art grep before filing a docs/issues file (hookify H-14). The trigger (a new bug file) is checkable but "nearest prior art" needs judgement and the surface is a pre-commit script; the tracker itself says it needs a second datapoint.
3. `grep -n @tool_x` numbers the buffer, not the source (T-27). Checkable by handle ledger, but only one verified instance of a source-producing buffer in 92 `grep -n @tool_*` calls (35 in last 7d), most buffers were librarian/doc output. Also considered and dropped: a chained `cmd; echo $?` hint (T-34; 751 cargo+echo$? commands, but agents read the marker, and the tracker lists it as an envelope defect for the server), and a "verify callers" retirement note (not a candidate).

## Observations for the decision
- The existing edit_code `verify callers` hint (943 deliveries, 0.1% follow-through) is the template of what NOT to endorse: advice with a cost the agent cannot see a reason to pay.
- Anything in this list whose claim depends on harness state (wake armed, file exists in main, agent's own worktree) is only safe under a declaration if the hook computes that state at emit time; candidates 1-3 pass, candidate 2 passes only for non-subagent callers.
- Identical-command repeats: run_command repeated identically within 5 calls 953 times; the large clusters are `scripts/with-slot.sh` runs with a write between (131), `git checkout -- <file>` (98, same output), `sleep` polls (156). Not hint-worthy beyond candidate 2.
