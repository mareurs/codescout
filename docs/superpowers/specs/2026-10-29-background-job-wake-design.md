# Background job wake-up — design

Date: 2026-10-29
Status: draft, not yet approved
Issue: `docs/issues/2026-10-29-codescout-has-no-background-wait-and-notify-mechanism-and-claude-code-monitor-is-removed-by-the-bash-deny.md`

## Intent

A session starts a long command with `run_command(run_in_background=true)`. It must be able to do other work and then hear about the result. Today it cannot.

The operator's rule says to "activate a Monitor" for long tasks. On this machine Claude Code's `Monitor` tool does not exist. The bare `Bash` entry in `permissions.deny` removes it. The operator wants codescout to provide a better mechanism.

## What exists today (verified in the code)

- `spawn_background_command` (`src/tools/run_command/inner.rs`) starts the child. A supervisor task waits on it and calls `buffer.set_job_state`.
- `JobState` (`src/tools/output_buffer.rs`) has three variants: `Running`, `Exited { code }` and `Failed { error }`. The state lives only in server memory.
- Nothing sends the outcome to the client. The model learns it only when a later call names the `@bg_*` handle.
- The server must not push unsolicited `notifications/progress`: they crash Claude Code 2.x (`src/server.rs`, BUG-038).
- The companion plugin has no hook that wakes a session.

## Measurements (2026-10-29, Claude Code 2.1.312, model `haiku`, interactive `tmux`)

All runs used a throwaway config dir. The first four rows used hooks from its `settings.json`. The plugin rows used a throwaway plugin.

| Question | Result |
|---|---|
| Does an `asyncRewake` hook that exits 2 wake an idle session? | Yes. The model started a turn by itself and reported the hook message. |
| Does Claude Code enforce `timeout` on an `asyncRewake` hook? | Yes. A hook with `timeout` 10 that slept 25 s was cancelled, and nothing woke. |
| Does Claude Code accept `timeout` 7200? | Yes. The hook ran and woke the session. A multi-hour wait is not measured. |
| Does the wake-up turn fire the same hook again? | Yes, for `UserPromptSubmit`: the hooks restarted and would loop. A guard is required. |
| Does `PostToolUse` with matcher `mcp__codescout__run_command` fire? | Yes. The hook input has `tool_name`, `tool_input` and `tool_response`. |
| Is `tool_input.run_in_background` visible to the hook? | Yes, as a JSON boolean. |
| Where is the `@bg_*` handle? | In `tool_response[0].text`, a JSON string that the hook must parse a second time. |
| Does a guard on `run_in_background` stop the loop? | Yes. The wake turn read the job with `tail -50 @bg_…`. The hook fired and skipped it. No second arm followed. |
| Does a hook declared in a plugin's `hooks.json` work? | Yes. A throwaway plugin, loaded with `--plugin-dir`, declared the hook with `asyncRewake` and `timeout` 7200. It fired with `CLAUDE_PLUGIN_ROOT` set and woke the idle session. |
| Does it work with `permissions.deny = ["Bash","Write","Edit"]`? | Yes. The same run used that deny list. The model confirmed that no `Bash` tool existed, and it started the job with `run_command`. |
| Can the hook parse the handle from `tool_response`? | Yes. `jq` with `.tool_response[0].text \| fromjson \| .output_id` gave `@bg_00000001`. |
| Do two jobs started in one turn both wake the session? | Yes. Two hooks armed at 11:48:46 and both finished at 11:49:11. The session got two "Stop hook feedback" wake-ups and reported both jobs. |
| Does the re-arm rule (section 4) work? | Yes. With a 45 s deadline and a 150 s job, each of three heartbeats woke the session. The model read the running job, and a new watcher armed 1 to 3 s later. The third read came after the job had ended. It showed the final state, and the hook skipped it. |
| Does a wait of more than two minutes work? | Yes, for 150 s, in the re-arm run. |
| Does a wake-up that arrives mid-turn get lost? | No. The job ended while the model ran foreground commands. Claude Code delivered the wake-up between two tool calls, in the same turn. |
| Does the hook see a dead server? | Yes. The record held the server pid. After `SIGKILL` of that server, the hook woke the session at once. The model reported that the job state was unknown. It then tried `tail -50 @bg_…`, which failed because the server was gone. |
| Does the real companion plugin interfere? | No. The real plugin and the test plugin loaded together, and the wake-up worked. Its `SessionStart` output was in the transcript. |

How the wake arrives: Claude Code shows it as "Stop hook feedback" with the hook's stderr as a system reminder.

Not measured:

- a wait of more than about three minutes. The longest run was 150 s. The `timeout` of 7200 s was accepted but never reached.
- a server that restarts, as opposed to one that dies
- jobs started by a subagent
- the real hook script and the real server record. They do not exist yet, so the runs used a model of them.
- the response-shape guard on an acked background command
- whether a background job outlives its server (inferred from the code)
- whether `PostToolUse` input carries `agent_id` for a subagent call

Review on 2026-10-29 found three gaps between an earlier draft and the code. They are recorded as F-201 in `docs/trackers/bug-fix-session-log.md`, and this version fixes them.

## Goals

1. The session can start a job, keep working and receive a wake-up when the job ends.
2. The wake-up reaches an idle session.
3. A job that outlives the hook produces a "still running" wake-up. It must never be lost silently.
4. The mechanism works with `Bash` denied.

## Non-goals

- Streaming matched output lines, as `Monitor` does. Version 1 reports completion and a heartbeat only.
- A wake-up inside `claude -p` or the Agent SDK. Claude Code kills async hooks at teardown there.
- A new MCP tool. The existing `run_command` result and a plugin hook carry everything.
- Jobs that a subagent starts. A subagent's `PostToolUse` hook fires in its own context, and the wake target is not measured. Version 1 makes no promise for them, and the `run_command` description must say so. This is my call.

## Anti-goals

- A hook that prints log content. The log may hold secrets. The server redacts a log when `run_command` reads it back (`a_background_log_is_redacted_when_it_is_read_back` in `src/tools/run_command/tests.rs`). The wake message names the handle, and the model reads the log through `run_command`.
- A wake-up for every `run_command` call, or a hook that re-arms itself on its own wake-up. A re-arm that the model causes by reading a running job (section 4) is allowed.

## Constraints

- Codescout is an MCP server. It cannot push a wake-up to the model.
- The companion plugin lives in a separate repository (`../claude-plugins/codescout-companion/`). Hooks there are `.mjs` files with `*.test.sh` tests.
- Several codescout servers run on one machine. Each numbers handles from `@bg_00000001`, so a handle alone is not a unique key.

## Approaches considered

**A. Completion record in the server, wake-up by a companion hook. Chosen.**
The supervisor writes a record file. An `asyncRewake` `PostToolUse` hook waits on it and exits 2.
- Gains: it works today, it needs no preview flag, and it reaches an idle session.
- Costs: the plugin and the server must agree on one file format. The hook `timeout` is a hard limit, so the hook needs its own earlier deadline.

**B. Codescout declares `claude/channel` and pushes events.**
Per the Claude Code channels reference, this needs `--dangerously-load-development-channels server:codescout` and a dialog in every session. The flag is ignored under `-p`. Claude Code drops events silently when the channel is not loaded. Not chosen for now. It can become a second delivery path that reads the same record.

**C. Report the outcome in the next `run_command` response, plus `ScheduleWakeup`.**
Rejected: it never wakes an idle session, which is goal 2.

## Design (approach A)

### 1. Completion record (codescout)

- Directory: `$TMPDIR/codescout-bg/`, mode `0700`.
- File name: `<server_pid>-<handle>.json`, for example `4112-bg_00000001.json`. **This is my call.** The operator approved "a fixed directory, keyed by handle". The server pid keeps two servers apart.
- The supervisor task writes the file at spawn with `state: "running"`. It rewrites it when the child exits. Every write goes to a temporary file and is renamed into place.
- The supervisor writes the record itself, not through `OutputBuffer::set_job_state`. `set_job_state` silently does nothing once `store_background` has evicted the handle, and eviction drops the oldest handle even when its job is still running. A record written there would never reach `exited`.
- Fields:
  - `schema` (1)
  - `handle`
  - `command`
  - `server_pid`
  - `job_pid`: the pid of the spawned shell, which is also its process group id (`process_group(0)` in `src/platform/unix.rs`)
  - `state`: `running`, `exited` or `failed`
  - `exit_code`: an integer, or `null` for a signal or a failure
  - `error`: set only for `failed`
  - `started_at` and `finished_at` (RFC 3339)
- The record holds no log content and no log path.
- If the server cannot write the record at spawn, the spawn response carries `record_error` and no `record`. Its hint says that no wake-up will come for this job.

### 2. Handle and record path in the spawn response (codescout)

`spawn_background_command` adds a `record` field with the absolute record path to its JSON response. The hook reads it from `tool_response`. It does not derive the path from the handle.

### 3. The hook (companion plugin)

A `PostToolUse` hook for matcher `mcp__codescout__run_command`, with `asyncRewake: true` and `timeout` 7200 (my call).

1. Read stdin. Parse `tool_response[0].text` as JSON. Arm only on the spawn response shape: `output_id` starts with `@bg_` and there is no `exit_code` key. Otherwise go to section 4, or exit 0. This is the guard that stops the wake-up loop: a read of a job always carries `exit_code`.
   - Do not arm on `tool_input.run_in_background`. An acked background command spawns from a replay call whose input is `{"command": "@ack_…"}`, with no such key (test `ack_re_dispatch_honours_run_in_background`). The server also accepts the string `"true"` (`parse_bool_param`).
   - If the hook input carries `agent_id`, exit 0: subagent jobs are out of scope. The builder must first verify that `PostToolUse` input carries `agent_id` for a subagent call.
2. Read `output_id` and `record`. If `record` is missing, exit 0. At arm time, write a map file `$TMPDIR/codescout-bg/map-<session_id>-<handle>` that holds the record path. `session_id` comes from the hook input. Section 4 needs this map.
3. Poll the record about once per second. Also check that `server_pid` is alive. If the record disappears before delivery, exit 2 with "record vanished; job state unknown".
4. On `exited` or `failed`: create `<record>.delivered` with exclusive create (`O_EXCL`, Node `flag: "wx"`). If the file already exists, exit 0, because another watcher delivered it. Otherwise print one line to stderr and exit 2. The line names the handle, the state, the exit code and the duration. It tells the model to read the output with `run_command("tail -50 <handle>")`.
5. If `server_pid` is gone while the state is `running`:
   - If `job_pid` is still alive, keep watching it. When it exits, wake with "job ended; exit code unknown, because its server ended". At the deadline, wake with "still running as pid N; its server ended".
   - If `job_pid` is gone too, wake at once with "server ended; job state unknown".
   - Every such message must say that the handle is no longer valid. A new server reuses `@bg_00000001` for a different job. The message must not tell the model to read the job with `run_command`.
   - In the test, `SIGTERM` did not stop the server within 12 s, so this check sees only a real exit. Nothing in the server kills background jobs on exit: they run in their own process group. That a job survives is inferred from the code, not measured.
6. At the hook's own deadline (default 30 minutes, my call, set by an environment variable): exit 2 with "still running after N minutes".

### 4. Re-arm after a heartbeat (companion plugin)

After a heartbeat wake-up the model reads the job with `run_command("tail -50 @bg_…")`. Nothing arms a new wait for that call. Rule: the same hook also arms when the command names an `@bg_*` handle whose record is `running` and has no live watcher. The hook finds the record through the map file `map-<session_id>-<handle>` from section 3. A `<record>.watch` file holds the pid of the live watcher.

Do not look the record up by handle alone: every server starts its handles at `@bg_00000001`. Do not rely on a `record` field in the read response either: `SHED_ORDER` can drop the `jobs` list to fit the output budget, and the list shows at most `JOBS_LISTED_MAX` (8) jobs.

If the model does not read the job after a heartbeat, no new watcher arms. That is not a silent loss: the heartbeat already told the model the job is running and named its handle. Measured on 2026-10-29 with a 45 s deadline and a 150 s job. Each heartbeat woke the session. The model read the running job, and a new watcher armed within 1 to 3 s. If the job ends between a heartbeat and the read, the read shows the final state and the hook skips it. The builder must still repeat the measurement with the real hook.

### 5. Cleanup

When the server evicts a log, it deletes the record only if the record is final and has a `.delivered` file. It never deletes a `running` record this way. At server start it deletes records older than seven days (my call), except a `running` record whose `server_pid` is alive. Keep the `.delivered` and `.watch` files with the record.

## Failure modes

| Case | Behavior |
|---|---|
| Job outlives the hook deadline | Heartbeat wake-up. Re-arm by the rule in section 4. |
| Server dies | The hook sees a dead `server_pid`. It keeps watching `job_pid` if that is alive, and otherwise wakes the session at once. The message says the handle is void. |
| Acked background command | The replay call's response has the spawn shape, so the hook arms. |
| Record write fails | The response carries `record_error`. No wake-up comes, and the model is told so. |
| Two jobs finish together | Two hooks, two wake-ups. Measured with two jobs: the session received both. |
| Wake-up while the model works | Measured: Claude Code delivered it between tool calls, in the same turn. Nothing was lost. |
| Hook not installed | Nothing wakes. The `run_command` description must not promise a wake-up. |

## Tests the builder must add

- Server: the record exists at spawn and holds `running`. It holds `exited` with the code after exit. A signal gives `exit_code: null`. A write never leaves a partial file.
- Server: an evicted handle's record still reaches `exited`. A record write failure gives `record_error`. The acked background spawn response has `output_id` `@bg_…` and no `exit_code`.
- Hook: one `*.test.sh` case each for the response-shape guard (spawn, read, acked replay, a pending ack), `agent_id` present, a missing `record`, `exited`, `failed`, two watchers racing on `.delivered`, a dead `server_pid` with a live and with a dead `job_pid`, the deadline, and a re-arm through the map file.
- Integration: repeat the `tmux` measurement above with the plugin hook. Include the profile with `permissions.deny` set.

Follow `docs/conventions/testing-discipline.md` and run `./scripts/mutation-probe.sh` on the new server code.

## Left to the builder

- Exact names, polling interval and record directory permissions on non-Linux platforms.
- The prompt surface: the `run_command` description changes. Check `src/prompts/README.md` for the `ONBOARDING_VERSION` rule. Read `docs/PROGRESSIVE_DISCOVERABILITY.md` before changing the tool.
- Cross-repo commits: cite plugin changes as `<repo>:<sha>`.
- The operator rule in the CLAUDE.md files ("activate a Monitor") should change to say how to use this mechanism. The operator owns that edit.

## Revisit when

- Claude Code channels leave the research preview: add approach B as a second delivery path.
- A measurement shows that two simultaneous jobs lose a wake-up. The one measurement with two jobs lost none.
- Claude Code restores `Monitor` under a `Bash` deny.
