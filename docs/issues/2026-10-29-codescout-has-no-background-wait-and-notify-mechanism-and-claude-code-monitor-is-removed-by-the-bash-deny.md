---
id: '45d8ce48dc6eb6fd'
kind: bug
status: fixed
title: 'BUG: codescout has no monitor mechanism for long background commands, and Claude Code''s Monitor is unavailable when Bash is denied'
tags:
- cluster/unclassified
closed: 2026-10-29
opened: 2026-10-29
owner: marius
severity: medium
---

# BUG: codescout has no monitor mechanism for long background commands, and Claude Code's Monitor is unavailable when Bash is denied

## Summary
This is a missing capability, filed as an issue on the operator's request. The operator's iron rule says to "activate a Monitor" for long tasks. On this machine Claude Code's `Monitor` tool does not exist, so the rule cannot be followed. Codescout's `run_command` with `run_in_background` returns a `@bg_*` handle, but nothing tells the model when the job ends. The outcome appears only in the response of a later call that names the handle. The operator wants codescout to provide a better monitor itself.

## Class fit
This is a missing feature, not a defect in an existing surface, so no class trait applies. The closest bug-shaped trait is a rule that names a tool the environment cannot supply. Filed `cluster/unclassified` as the sanctioned escape hatch: looked, nothing fits.

## Symptom (Effect)
No `Monitor` tool appears in the session tool list. The session has `ScheduleWakeup`, `TaskStop`, and `run_command(run_in_background=true)`. The last returns an `@bg_*` handle. Nothing re-invokes the session when the job ends. Correction 2026-10-29: an earlier version of this issue said the harness re-invokes the session on completion. The code does not support that. `spawn_background_command` (`src/tools/run_command/inner.rs`) hands the child to a supervisor task that only calls `OutputBuffer::set_job_state`, and no notification goes to the client. The companion plugin had no hook that wakes a session.

## Reproduction
Measured 2026-10-29 by a forked agent, with `claude` 2.1.312 and tiny `-p "hi"` calls in a temporary config dir, same auth and base URL:

| permissions.deny | Bash | Edit | Write | Monitor |
|---|---|---|---|---|
| none | yes | yes | yes | yes |
| `["Bash"]` | no | yes | yes | no |
| `["Write","Edit"]` | yes | no | no | yes |

All three profiles (`~/.claude`, `~/.claude-sdd`, `~/.claude-kat`) set `permissions.deny = ["Bash","Write","Edit"]`. The `--debug-file` log says `Adding 3 deny rule(s) to destination 'userSettings'`. The stream-json init tool list has no `Bash`, `Edit`, `Write` or `Monitor`.

## Environment
Linux, Claude Code 2.1.312, requests routed through llm-proxy at `localhost:8082`. Branch `experiments`.

## Root cause
Claude Code applies the Bash permission rules to `Monitor`. The tools reference (https://code.claude.com/docs/en/tools-reference.md) says `Monitor` follows the Bash rules. The permissions page (https://code.claude.com/docs/en/permissions.md) says a bare `Bash` deny removes the tool from the model's context. Measured by the A/B test above: the bare `Bash` deny alone removes `Monitor`.

Ruled out, with evidence: llm-proxy (`STRIP_TOOLS` in `/proc/9498/environ` and `/home/marius/agents/llm-proxy/.env:19` has no `Monitor`; stripping is by exact name, `src/passthrough.rs:79-83`; `~/.buddy/proxy-overrides` is empty), the version (above the documented 2.1.310 floor), and feature-flag fetching (debug log shows `mcp runtime arm: v2 (source: growthbook)`).

Not explained: `CronCreate` appears in the model's tool list although `STRIP_TOOLS` names it. Inferred, not measured: the proxy edits the API request after Claude Code builds the list. Also not explained: `ToolSearch` is unavailable in this session.

## Evidence
Open upstream issues report the same symptom for other causes: https://github.com/anthropics/claude-code/issues/104194 (open, no cause known) and https://github.com/anthropics/claude-code/issues/86171 (a telemetry-off flag). Neither explains this machine, because the A/B test removes `Monitor` with the deny rule alone. The researcher found no replacement for `Monitor` in Anthropic's docs.

## Probe: what else each deny value removes (2026-10-29)
Measured with `claude` 2.1.312, five tiny `-p "hi"` calls in fresh config dirs, same auth and base URL. The script read the stream-json init event. Only `env` was copied from the `~/.claude-kat` profile, and `CLAUDE_CODE_ENABLE_TODO_TOOLS` was dropped.

| permissions.deny | Removed from the init tool list | Added |
|---|---|---|
| none | nothing | nothing |
| `["Bash"]` | `Bash`, `Monitor` | `Glob`, `Grep` |
| `["Edit"]` | `Edit` | nothing |
| `["Write"]` | `Write` | nothing |
| `["Bash","Write","Edit"]` | `Bash`, `Monitor`, `Write`, `Edit` | `Glob`, `Grep` |

Findings:
- `Bash` and `Monitor` are the only tools the deny set removes beyond `Edit` and `Write` themselves.
- A bare `Edit` deny does not remove `Write` or `NotebookEdit`. A bare `Write` deny does not remove `Edit` or `NotebookEdit`. `NotebookEdit` stays in every case, so it is a write path that the deny set does not close.
- Without `Bash`, Claude Code adds the dedicated `Glob` and `Grep` tools. The tools reference says they are absent by default on Linux.
- The agent list (5), the skill list (19) and the slash-command count (52) are the same in all five cases. The deny set hides no agent and no skill at list level.
- `LSP`, `PowerShell` and the `Task*` tools are absent in the `none` case too, so the deny set does not cause their absence.
- `ToolSearch` is present in every case. The Reproduction section says it is unavailable in this session, so that statement is stale.

Behavioral probe, same day, `--model haiku`, `--permission-mode bypassPermissions` in a temporary working directory, one run per case. Bypass mode skips prompts but still applies deny rules.

| Case | deny | Result |
|---|---|---|
| `NotebookEdit` writes `n.ipynb` | `["Bash","Write","Edit"]` | The cell source changed to `PROBE_WRITTEN`. The first call failed with "File has not been read yet"; the model then used `Read` and retried. |
| Skill with `` !`cmd` `` injection | none | The harness expanded all three forms (`A: !`cmd``, a bare `` !`cmd` `` line, and a fenced `` ```! `` block). The model made no tool call. |
| Same skill | `["Bash"]` | The invocation returned an empty result with no tool call. No error text reached the result. |
| Subagent (`general-purpose`) runs `echo` and writes `sub.txt` | none | The subagent used `Bash` and `Write`. `sub.txt` exists. |
| Same subagent task | `["Bash","Write","Edit"]` | The subagent had neither tool. It called only `ToolSearch`. `sub.txt` does not exist. |

Findings:
- `NotebookEdit` is a working write path under the full deny set. The deny set does not stop file writes.
- A skill that uses shell injection does nothing while `Bash` is denied. Such skills are unusable on this machine.
- The deny set applies to subagents. The built-in `statusline-setup` agent lists `Edit` as a tool, so it is probably unusable too. I did not test that agent.

Not valid, kept as a warning: the first skill probe used `RESULT=!`echo ...$((6*7))``. The harness did not expand that form, and the model could compute `42` without a shell. Use output the model cannot guess, such as `$RANDOM` and `$$`.

Not measured: `/tasks`, the `!` prompt prefix, command hooks, and the `statusline-setup` agent.

## Hypotheses tried
1. llm-proxy strips `Monitor`. Refuted: see Root cause.
2. Version or feature-flag gate. Refuted: see Root cause.
3. Bare `Bash` deny removes `Monitor`. Confirmed by the A/B test.

## Fix

Implemented 2026-10-29 on `experiments` (codescout) and `main` (claude-plugins). Pushed to `origin/experiments` (codescout, through `e312bdc20`) by another session on the operator's say-so. Released 2026-10-29: the server rebuilt, and the companion bumped to 1.20.22 and installed in all four profiles. Design: `docs/superpowers/specs/2026-10-29-background-job-wake-design.md`. Plan: `docs/superpowers/plans/2026-10-29-background-job-wake.md`.

In short: the background supervisor writes a JSON completion record (`$TMPDIR/codescout-bg/<server_pid>-<token>-<handle>.json`) and the spawn response carries its path as `record`. A companion-plugin `PostToolUse` hook on `mcp__codescout__run_command`, with `asyncRewake: true`, waits on that record and exits 2, which wakes an idle session. A heartbeat wake-up covers long jobs, and a read of a still-running job re-arms the watcher.

Commits, codescout: `09cbba443`, `dc8092c79` (record module and its fix), `5392b1d17`, `bdd533f5d` (supervisor writes records). Commits, claude-plugins: `32d8a06`, `2615b82` (hook), `872b634`, `2317650`, `fb79f46` (re-arm), `5ed4847` (registration). Review gaps in an earlier design draft are recorded as F-201 in `docs/trackers/bug-fix-session-log.md`.

## Tests added

- `src/tools/job_record.rs`: 12 unit tests (record shape and redaction, atomic and concurrent writes, a refused untrusted directory, a symlink at the temp name, sweep and delivered-removal rules).
- `src/tools/run_command/tests.rs` and `src/tools/output_buffer.rs`: a running record at spawn, an exited record with the code, an evicted live job still finalising its record, an acked background spawn carrying a record, a failed first write giving `record_error`, delivered and undelivered eviction, the once-per-buffer sweep, `pid_alive`.
- `codescout-companion/hooks/wake-on-job-end.test.sh` in claude-plugins: 44 assertions (arm on the spawn response shape, acked replay, guards, vanished record, two racing watchers, heartbeat, dead server with a live and a dead job, re-arm, no loop, `.watch` ownership).
- End-to-end, 2026-10-29, `haiku`, throwaway config with `Bash`, `Write` and `Edit` denied, the real companion plugin and a debug build of this branch: a short job, a 70 s job with three heartbeat re-arms and a final wake-up, an acked `rm -rf` background command, and a subagent's job (no arm, as designed). All passed.

## Workarounds

Until the fix ships: start the job with `run_command(run_in_background=true)`, then use `ScheduleWakeup` at 1200 s or more as a fallback, and read the job with `run_command("tail -50 @bg_…")` when it fires. No completion notice exists today.

## Resume

Closed 2026-10-29 as fixed, on the operator's word. Verified live after the release: the rebuilt server wrote a record for `@bg_00000001`, and a 20 s job `@bg_00000002` woke the idle session in the `~/.claude-stef` profile. The other three profiles have 1.20.22 installed with the hook registered; their wake was not exercised. Still open for the operator: updating the operator rule "activate a Monitor" in the CLAUDE.md files, and a clean gate re-run (below). Still unmeasured: a wait of several hours and a server that restarts. Gate, run 2026-10-29 on the committed tree: Python, rustfmt and clippy green. The two cargo test lanes were red with 30 library failures, none in `job_record`, `output_buffer` or `run_command`. 27 are an artifact of the run: a long `TMPDIR` (set because `/tmp` hit a disk quota) breaks unix socket paths (`path must be shorter than SUN_LEN`) for the `lsp`, `peer` and `util::fs` socket tests and one `memory` size test; all 246 of them pass with a short `TMPDIR`. The other 3 are not from this work: two OP-2 rule tests (`operator_rules::route`, `server::every_triggered_rule_names_a_tool_that_can_deliver_it`, tied to the uncommitted `src/operator_rules/profiles.rs`) and `prompts::reader_docs_contain_no_retired_call_forms` (the gitignored `.codescout/resident-private` tree). They were not re-run on a clean tree. The gate must be re-run once the machine's `/tmp` quota and those peer files are settled.

Counterexample, 2026-10-29, profile `~/.claude-stef`: a session that started before the install got no wake. The session started at 07:24 local with companion 1.20.21, which has no `hooks/wake-on-job-end.mjs`. The 1.20.22 directory was created at 17:21 local. After `/mcp` the rebuilt server (pid 3350735) wrote a correct record for that session's job `@bg_00000001` (`state: exited`, exit 0, finished 19:37:30 local). No `.delivered` marker exists beside it, and the other sessions' records have one. The session was never woken and a user had to prompt it. A `/mcp` reconnect restarts the MCP server, not the plugin hooks. Retest, same session, 20:35 local: after the operator ran `/reload-plugins --force` (the companion re-read from `~/work/claude/claude-plugins/codescout-companion`), a 20 s job `@bg_00000002` woke the idle session, and its `.delivered` marker appeared 0.15 s after the record. So the same session went from no wake to a wake with a plugin reload and no restart. That Claude Code reads hooks only at session start was NOT tested directly; a reload fixing it fits the evidence. Consequence if true: a session that outlives the plugin update gets no wake until it restarts, and nothing tells it so.

Known limit, not measured: if an old server outlives `/mcp` long enough to finalise its record, the hook's wake text says `tail -50 @bg_N`, and that call goes to the new server, where `@bg_N` may be a different job. Still unmeasured: a wait of several hours.

## Follow-up 2026-10-29: a process that `run_command` did not start has no wake, and the guide gives no pattern for it

Found by the operator in a session on `ie-pal-engine` (pasted, not read from the transcript). The agent waited on an evaluation run with its own loop: `while pgrep -f "..."`. The loop matched its own command line, so it never ended. The agent then ran `pkill -f "while pgrep -f"`. That killed its own shell, and the call returned `exit_code: -1`. The analysis in the same command never ran.

**Verified 2026-10-29, claude-plugins session:**

- `pgrep -f` matches the shell that carries the pattern. A `bash -c` command that searched for a marker string counted 2 processes: the outer shell and its subshell. No other process carried the marker.
- `codescout-companion/hooks/wake-on-job-end.mjs` arms only on a `run_command` spawn response: `output_id` starts with `@bg_`, a `record` path exists, and there is no `exit_code` (`spawnOf`, line 36). Any other call exits 0.
- The hook skips subagents (`input.agent_id`, line 247) and exits when `CLAUDE_PROJECT_DIR` is unset (line 244).
- The hook checks liveness with `kill(pid, 0)`, so it does not have the self-match fault.
- The `progressive-disclosure` guide, section "Background jobs", says "Do not poll with sleeps or repeated status calls". It names no pattern for a process that `run_command` did not start. No guide, spec or issue in this repo mentions the `pgrep -f` or `pkill -f` self-match.

**Not established:** how the evaluation run in that session started, and whether the hook was installed and armed there. The paste does not show either. The loop may have ended for this reason alone, or other causes may have added to it.

**Suggested change, untested:** add to the "Background jobs" section a rule for a process that `run_command` did not start. Wait on its pid, for example `tail --pid=<pid> -f /dev/null` started with `run_in_background`, so the wait has a handle and a record. Warn that `pgrep -f` and `pkill -f` match the agent's own shell; a pattern such as `[w]hile pgrep` avoids that. A change to this guide changes the prompt surface: read `src/prompts/README.md` for the `ONBOARDING_VERSION` rule first.

**Status:** this issue stays `fixed`. The fix covers `run_in_background` jobs. This follow-up was an open gap in the guide, not a regression of that fix. It is closed in the next subsection.

### Resolved 2026-10-29, session `codescout-de`

- **Guide.** Commit `7503e23c8` adds the rule to "Background jobs" in `progressive-disclosure.md`: wait on the pid with `tail --pid=<pid> -f /dev/null` started with `run_in_background`, and match by name with a bracket class such as `[e]val_run`. The test `progressive_disclosure_guide_states_the_background_job_recipe` pins the new strings. No `ONBOARDING_VERSION` bump: the guide is a `get_guide` topic, not the `onboarding_prompt` slice or `build_system_prompt_draft()`.
- **Exit code.** Commit `58f75546f`: a command that a signal kills now carries `terminated_by_signal: <n>` beside `exit_code: -1`, on the main path and the interactive path. An ordinary non-zero exit has no such key. The test is `a_signal_killed_command_names_the_signal_beside_the_minus_one`. The cause was `status.code().unwrap_or(-1)`, which gives `-1` when a signal ends the process. The filter-run site in `inner.rs` (`run_filter_on_capture`) still maps a signal to `-1`. I left it, because it runs a filter on a captured stream, not the user's command.
- **Measured.** `pgrep -fc <marker>` inside a `bash -c` that carries the marker counted 1, and that process was the shell itself. A bracket pattern on a command line without the plain form counted 0. `pkill -f <marker>` in the same shape killed its own shell: the line after it did not run, and the response was `exit_code: -1`.
- **Measured, the pid wait.** An orphan `setsid sleep 45`, outside the shell's process tree, was waited on with `run_command("tail --pid=<pid> -f /dev/null", run_in_background=true)`. The job's record stayed `running` while the orphan lived. The companion hook then woke the idle session with `exited 0 after 43s`. The exit code is `0` whatever the waited process did, and the guide says so.
- **Checked, not a defect.** `source.md:154` tells the model to poll `index(action='status')` every 15 seconds. `index(action='build')` returns `status: "started"` and runs in the background, so the poll matches the tool. No wake exists for an index build. That is a missing capability, not a defect of this class.
- **Not done.** No guard was added for `pkill -f`. The dangerous-command list has `kill -9` but not `pkill -f`, and a guard is a judgement call. The root cause in the `ie-pal-engine` session is still not established: only a paste existed.

## Related research
Cross-harness wake paths (Claude Code inbox socket, Codex `codex queue`, channels, hooks, Pi gaps), measured 2026-10-29: codescout memory `research/cross-harness-session-messaging`. The peer session's design for this bug is `docs/superpowers/specs/2026-10-29-background-job-wake-design.md`.

## References
- https://code.claude.com/docs/en/tools-reference.md
- https://code.claude.com/docs/en/permissions.md
- https://github.com/anthropics/claude-code/issues/104194
- https://github.com/anthropics/claude-code/issues/86171
- Operator rule: "Long Tasks — Activate a Monitor, Do Not Poll" in the user CLAUDE.md files.
