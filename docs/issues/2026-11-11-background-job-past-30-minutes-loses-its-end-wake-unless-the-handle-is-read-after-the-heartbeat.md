---
id: '3f165861a11d04b8'
kind: bug
status: mitigated
title: a background job that runs past 30 minutes loses its end wake unless the session reads its handle after the heartbeat
owners:
- marius
tags:
- cluster/hint-composed-without-the-request
closed: 2026-11-11
opened: 2026-11-11
severity: high
unverified: 'SessionStart of wake-missed-jobs.mjs is not measured through the harness (UserPromptSubmit is, see Safety net through the harness). The cliff is not removed: without a read after the heartbeat the end wake is still lost (measured 2026-11-11, see Live repro). Whether a Codex session gets an end wake for its background jobs is not measured (Evidence 6 shows only that those records have no legacy .wake-owner.json).'
---

# BUG: a background job that runs past 30 minutes loses its end wake unless the session reads its handle after the heartbeat

## Summary

The companion wake hook (`wake-on-job-end.mjs`) ends its watcher at 30 minutes with a "still running" wake. After that no watcher exists. A job that ends later sends no wake, unless the session ran a `run_command` whose command text names the `@bg_N` handle between the heartbeat and the end. The spawn hint and the owner's rule say "end your turn, a wake arrives, do not poll". They do not say that this stops being true after 30 minutes. Of 56 armed background jobs that ran 30 minutes or longer, 43 ended with no end wake. Of 833 armed jobs shorter than 30 minutes, all 833 got one.

## Class fit

The spawn hint (`src/tools/run_command/inner.rs:169-171`) promises an end wake from the shape of the response, without the state that decides it: the job's age, and whether a watcher is still alive. The discriminator (the job record's `started_at`, and the `.watch` marker) is available to the server and is not consulted. A hint that is true for a short job and false for a long one is composed without the request it answers.

## Symptom (Effect)

All times are UTC. Local time on this machine is UTC+2.

Failing case, session A (profile `~/.claude-stef`, session id `be7356ea-b0e8-4ba3-8375-9ca446490fb5`):

```
10:53:11.9Z  run_command(run_in_background=true), a long evaluation script; handle @bg_00000011
11:23:12.8Z  wake: "codescout background job @bg_00000011 is still running after 30 min.
             Read it with run_command("tail -50 @bg_00000011") to keep waiting."
11:23:14Z    the session reads the job's OUTPUT DIRECTORY with run_command; the command text has no @bg_ handle
11:23:17Z    the session says it will wait for the end-of-job notice, and ends its turn
11:35:32.8Z  the job ends: record state "exited", exit_code 0 (42 min 20 s)
             no wake arrives
11:43:20Z    the owner types "check" (7 min 48 s after the end); only then does the session notice
```

Working case, same session, same server: `@bg_00000013`, a log copy. Spawned 10:56:03Z, ended 10:57:27.9Z (1 min 25 s), wake "ended: exited 0 after 1m 25s" at 10:57:28.2Z.

A third path, same session: a foreground `run_command` with `timeout_secs=600` was moved to the background by the harness after 120 s (task `kisuqg1j6`). Its completion notice ("MCP task kisuqg1j6 (codescout/run_command) completed") arrived at 11:47:27.6Z. That path does not go through the codescout hook (see Evidence 5).

## Reproduction

Not reproduced live in this investigation. The mechanism follows from the hook code and from the transcripts (Evidence 2 to 4). Shortest recipe, from the design spec's own measurement (`docs/superpowers/specs/2026-10-29-background-job-wake-design.md:39`): with the companion plugin active, set `CODESCOUT_WAKE_DEADLINE_SECS=45`, start `run_command("sleep 150", run_in_background=true)`, end the turn, and when the heartbeat wake arrives do NOT run any command that names the `@bg_N` handle. End the turn again. Expected by this issue: no wake when the job ends at 150 s.

### Live repro, 2026-11-11 (measured)

Session `14053904-5577-4ad4-9053-0a7fb3f09d04`, job `@bg_00000014` (`sleep 1860`), default 1800 s deadline.

```
12:32:30Z  spawn; .wake-owner.json, .watch and the session map exist (the hook armed itself)
13:02:34Z  heartbeat wake, with the NEW text from claude-plugins 1.21.4 ("This watcher has ended ... no end wake arrives")
           no run_command naming the handle was run; only a foreground sleep and a path-based ls
13:03:30Z  record: state exited, finished_at 13:03:30Z; no .delivered, no .watch
13:04:18Z  no end wake had arrived
```

So the mechanism holds: with no read after the heartbeat, the end wake is lost. The safety net, run by hand with this session's id on the real state directory, then reported the job once (`ended: exited 0 after 31m 0s`) and wrote `.delivered`.

Observed, not explained: the heartbeat of this session carried the 1.21.4 text although the session was started before the release. The cache of the profile was replaced in place by `release.sh` at 14:3x local time. Not checked whether a resumed session resolves the plugin root at each call.

### Safety net through the harness, 2026-11-11 (measured)

After `/reload-plugins` (the plugin is a directory source, re-read from `~/work/claude/claude-plugins/codescout-companion`), job `@bg_00000016` (`sleep 45`) was started and its watcher process was killed (pid 1230003), so the job ended with no wake. At the next user prompt the harness ran `wake-missed-jobs.mjs` at `UserPromptSubmit` and injected: `ended: exited 0 after 45s`. The session had not read the job. So the hook is registered and works end to end for `UserPromptSubmit`. `SessionStart` is not measured.

## Environment

- Linux, three Claude Code profiles (`~/.claude`, `~/.claude-kat`, `~/.claude-stef`), Claude Code 2.1.322 in session A.
- codescout-companion 1.21.3 installed in `~/.claude-stef`; its `hooks/wake-on-job-end.mjs` is byte-identical to the repo copy (`diff -q`, 2026-11-11). Versions 1.20.25 to 1.21.2 in the same cache are identical too; 1.20.22 to 1.20.24 differ and were not read.
- `codescout-wake.json` exists in none of the four profile directories, so the hook takes the legacy path (the v1 block at `hooks/wake-on-job-end.mjs:254-267` is skipped).
- Every record's `.wake-owner.json` reads `{"version":1,"engine":"legacy","target":null}`.

## Root cause

Verified by reading `claude-plugins/codescout-companion/hooks/wake-on-job-end.mjs` and `hooks/hooks.json`, and by the records and transcripts in Evidence. Not run live (inferred from the code and consistent with every transcript; not measured at runtime).

1. One watcher process is started per spawn response: `hooks.json:149-151` registers the hook with `asyncRewake: true`, `timeout: 7200`; `main()` takes the spawn path at `wake-on-job-end.mjs:268-272`.
2. The watcher has a deadline. `deadlineSecsOf` returns 1800 s unless `CODESCOUT_WAKE_DEADLINE_SECS` is set, capped at 7000 s (`wake-on-job-end.mjs:117-120`).
3. At the deadline, while the job still runs, the watcher returns `finish(2, wakeMessage('heartbeat', ...))` (`wake-on-job-end.mjs:180-184`). The process exits 2, which is the wake, and `finish` removes the `.watch` marker (`wake-on-job-end.mjs:133-141`). The heartbeat does not write `.delivered` (only `deliver()` does, via `claimDelivery`, `wake-on-job-end.mjs:106-112`). So a job whose last wake was a heartbeat has no `.watch` and no `.delivered` file.
4. After that, a new watcher starts only through `rearmTarget` (`wake-on-job-end.mjs:214-244`), which runs on a later PostToolUse of `run_command`. It requires: a tool response that parses as JSON with `exit_code` (a read, line 219); a command whose text contains an `@bg_xxxxxxxx` handle (line 220-222); a map file for this session and handle (line 223-226); a record still `running` (line 229); no `.delivered` (line 233); no live `.watch` owner (lines 234-240; the checks sit at lines 219, 221, 224-226, 229, 233 and 240). A command that reads the job's log or output directory by path, or that sleeps, or a turn that just ends, arms nothing.
5. The design says this on purpose: "If the model does not read the job after a heartbeat, no new watcher arms. That is not a silent loss: the heartbeat already told the model the job is running and named its handle." (`docs/superpowers/specs/2026-10-29-background-job-wake-design.md:145`). A self re-arm by the hook is a stated non-goal (same file, line 76).
6. The heartbeat text is the only place that tells the session what to do ("Read it with run_command("tail -50 @bg_N") to keep waiting", `wake-on-job-end.mjs:80-81`). The spawn hint says "end your turn: a wake message arrives when the job ends" with no limit (`src/tools/run_command/inner.rs:169-171`). `src/prompts/guides/project-activation-bootstrap.md:19-20` says "end your turn ... Do not poll". The guide `src/prompts/guides/progressive-disclosure.md:135-136` says "After 30 minutes a still-running job wakes you once ... Reading it again keeps the wait going", which is the only statement of the rule and does not say that without a read the end wake is lost. In session A the owner's own rule (end the turn, do not poll) and the heartbeat text ask for opposite things; the session did a different third thing (read the output directory).

What a read buys is exactly one more window of 30 minutes (Evidence 4, job `@bg_00000014` of session C).

Premise for a later session to re-check before working this bug: that the watcher process is gone after the heartbeat. After the fact, the record of `@bg_00000011` has no `.watch` file (consistent). It was not checked at 11:23 to 11:35Z while the job ran.

## Evidence

All paths are under `$TMPDIR/codescout-bg/` (here `/home/marius/.cache/cc-tmp/codescout-bg/`) unless noted. The records are shared by all sessions on the machine. Session transcripts were scanned for the strings `codescout background job @bg_` and the spawn response `record` path; no project names or command texts are quoted here.

### 1. The record of the failing job

`209236-b92666c9-bg_00000011.json` (mtime 13:35:32.8 local):

```
"handle": "@bg_00000011", "server_pid": 209236, "state": "exited", "exit_code": 0, "error": null,
"started_at": "2026-11-11T10:53:11.898968997+00:00", "finished_at": "2026-11-11T11:35:32.799946741+00:00"
```

Siblings: `.wake-owner.json` present (armed at 10:53:11.99Z, so the hook did start); no `.delivered`; no `.watch`. Compare `209236-b92666c9-bg_00000013.json.delivered` (content `95565`, the watcher pid), mtime 12:57:28.13 local = 10:57:28.13Z, 0.2 s after that job's `finished_at`.

### 2. The same session, long jobs, spawn to end (session A, profile `~/.claude-stef`)

Transcript lines (JSONL, from the spawn response): spawn result L35193, heartbeat L35636 (a user-role message, `origin.kind` = `task-notification`, `promptSource` = `system`), the next tool call L35639 (command reads an output directory and `date`; no `@bg_` in it), assistant text L35646, human "check" L35652, harness task notice L35743. The heartbeat arrived while the session was idle (previous event: an `away_summary` at 11:14:03Z, L35633), and started a turn. So an idle session is woken by this hook.

How a wake arrives in the transcript (three forms, all from the same hook): a user-role message when the session is idle; an `attachment` of type `queued_command` when the session is mid-turn (for example L2602, "ended: exited 0 after 47s"); a `queue-operation` `enqueue`. A scan that looks only at user messages misses the mid-turn ones.

The label "Stop hook feedback / blocking error from command PostToolUse:mcp__codescout__run_command" is the harness's wording for an `asyncRewake` exit 2 of the PostToolUse hook. It is not a Stop hook, and it does not mean the wake needs a tool call to fire.

### 3. All records on the machine, 2026-11-04 to 2026-11-11 (scan on 2026-11-11; 1037 records)

Armed means the record has a `.wake-owner.json` (the hook started for it). Ended means `state` is `exited` or `failed`.

| armed and ended | has `.delivered` (end wake sent) | no `.delivered` |
|---|---|---|
| shorter than 30.0 min | 833 | 0 |
| 30.0 min or longer | 13 | 43 |

42 of the 43 are matched to 11 transcripts in all three profiles (`~/.claude-stef`: 5 sessions, `~/.claude-kat`: 4, `~/.claude`: 2). The last one (exit 143, a job killed from outside) is not matched to a transcript. Durations 30.1 min to 770 min. Also: 8 armed records are still `running` with no `.delivered` (long-lived servers whose heartbeat fired and were never read again), and 125 ended records have no `.wake-owner.json` (see Not verified, point 4). Re-run the scan before relying on the numbers; the directory is a live cache.

### 4. Per-session check: heartbeat, end wake, reads of the handle

For each armed job of 30 min or longer in the transcripts of 11 sessions: was a heartbeat wake received, was an end wake received, and did the session run a `run_command` naming the handle after the heartbeat.

- 13 armed jobs of 30 min or longer have `.delivered` (Evidence 3). Five of them are matched to the 11 transcripts here, and all five had at least one read of the handle after the heartbeat (session A, a job of an earlier server: 7 reads; session C and three others: 1 to 2 reads). The other 8 are not checked.
- Every job with no end wake had a heartbeat and 0 reads after it, with one exception that confirms the mechanism: session C (profile `~/.claude-kat`), job `@bg_00000014` of server 1975418: spawned 2026-11-10T19:03:29Z; heartbeat "after 30 min" at 19:33:29Z; the session read `tail -3 @bg_00000014` at 19:35:03Z; a second heartbeat "after 62 min" came at 20:05:05Z (30 min after the read); no further read; the job ended at 23:49:xx with no end wake. One read bought exactly one more window.
- Session A has 13 such jobs (from 2026-11-09 23:48Z to 2026-11-11 11:35Z), including `@bg_00000011`. Several are servers or jobs the session killed on purpose (exit 137); whether the session was waiting on each of them is not known.

### 5. The harness-moved task

L35703 to L35707: the foreground call returned `MCP tool "codescout/run_command" is still running after 120s. It was moved to the background as task kisuqg1j6 and keeps running; you'll receive a notification with the result when it completes.` The completion notice came at L35743 from the harness (`origin.producer` `session-task`, with a `runId`; the text starts `<task-notification><task-id>kisuqg1j6`). By code reading, the codescout hook does nothing for this response: `spawnOf` needs JSON with an `output_id` that starts with `@bg_` (`wake-on-job-end.mjs:41-55`) and `rearmTarget` needs JSON with `exit_code` (`:219`); the response text is plain prose, so both return null and `main()` exits 0 (`:268-269`). Not run. So the two paths are independent: the harness path wakes the session by itself; the codescout path wakes it only as described above.

### 6. The ended records with no `.wake-owner.json` (measured 2026-11-11, resume point 4)

Re-scan on 2026-11-11 (records started since 2026-11-04): 1067 ended, 919 armed, **148 unarmed** (125 at the first scan; the count grows with the live cache). Each unarmed record was searched by file name in the transcripts of the four Claude Code profiles and in `~/.codex`.

| where the spawn is found | records |
|---|---|
| a Claude Code subagent transcript (the hook exits 0 for `agent_id`, by design) | 105 |
| a Codex rollout (`~/.codex/sessions`); 40 of them are jobs of one `codescout start` whose parent is `codex app-server` | 43 |
| a Claude Code main-session transcript | 0 |
| nowhere | 0 |

(The first scan of the transcripts gave "no transcript found" for the 43 Codex records, because it did not search `~/.codex`. A sample of 4 was then found there; all 43 were not individually searched there.) None of the 148 has a session map or a `.delivered`. So the unarmed records are not a second defect of the Claude Code main-session path. Whether a Codex session receives an end wake for these jobs (that harness has its own hook, `background-wake/codex-hook.mjs`) is a different question and is not measured here.

## Hypotheses tried

1. **Hypothesis:** the hook only fires after a tool call, so an idle session gets nothing (the label says `PostToolUse`). **Test:** read what arrived while idle. **Verdict:** rejected. The heartbeat at 11:23:12Z started a turn in an idle session (Evidence 2), and 833 of 833 short armed jobs got an end wake. **Evidence:** 2, 3.
2. **Hypothesis:** the job ended while the session was mid-turn, and the wake was lost. **Test:** the design spec measured a mid-turn wake (spec line 41) and transcripts show `queued_command` attachments. **Verdict:** rejected for session A: no turn was running between 11:23:17Z and 11:43:20Z (turn duration 4.5 s, then idle). **Evidence:** 2.
3. **Hypothesis:** the handle number was reused by another server, so the wake went to another job. **Test:** the record path carries the server pid (209236); the wake of `@bg_00000013` came from the same server; no second record for `@bg_00000011` exists in 209236. **Verdict:** rejected for A. The reuse is real in general (map files are keyed by session and handle only, `wake-on-job-end.mjs:193-199`), and it is why a map-file lookup named no session for 4 of the 43 (a later server in the same session reused the handle and overwrote the map entry); 3 of those 4 were matched in session A's transcript instead. **Evidence:** 1.
4. **Hypothesis:** the watcher lived on and failed to deliver (a crash, a `claimDelivery` race). **Test:** `.delivered` and `.watch` of `@bg_00000011`. **Verdict:** deferred, unlikely. Both files are absent, and the code path that ends the watcher at 30 minutes explains the absence without a failure. Not observed while the job ran. **Evidence:** 1.
5. **Hypothesis:** the cause is the heartbeat ending the only watcher, with a re-arm that depends on the session. **Test:** correlate, for every armed job of 30 min or longer in 11 transcripts, the heartbeat, the reads of the handle after it, and the end wake. **Verdict:** confirmed as the best-supported cause (no counterexample in the transcripts read; one job shows the one-window effect). Still read from code rather than measured live. **Evidence:** 3, 4.

## Fix

Three parts, one commit per repo. The cliff itself is not removed (a wake needs a live `asyncRewake` process; after the heartbeat none exists, and only a new PostToolUse can start one), so the status is `mitigated`.

1. **Spawn hint and guide (codescout).** The spawn hint in `src/tools/run_command/inner.rs` and the guide `src/prompts/guides/progressive-disclosure.md` state the 30-minute limit and the one read that re-arms the watcher. The guide also says the rule applies to a `tail --pid` wait job.
2. **Heartbeat text (claude-plugins).** `wakeMessage('heartbeat')` in `wake-on-job-end.mjs` says the watcher has ended, names `run_command("tail -1 @bg_N")`, and says that without it no end wake arrives.
3. **Safety net (claude-plugins).** New hook `wake-missed-jobs.mjs`, on `UserPromptSubmit` and `SessionStart`. It reads the map files of THIS session, and reports each job that has `state` `exited` or `failed`, no `.delivered` and no live `.watch` owner. The report claims `.delivered`, so each end is reported once. For a job of a server that has ended, it does not offer a read of the handle.

Not done, by choice: raising the default deadline (it moves the cliff from 30 to about 117 minutes; jobs of up to 770 minutes exist in the data), and a self re-arm by the hook (a detached child cannot wake the harness).

## Fix provenance

- **SHA:** `7adb23323` (`experiments`)
- **patch-id:** `89fdf1e02365`
- **SHA:** `c93a23b` (`claude-plugins` `main`)
- **patch-id:** `e65e13cacfe9`

## Tests added

- `claude-plugins/codescout-companion/hooks/wake-on-job-end.test.sh`, test 24: the heartbeat names the one read, says an end wake is lost without it, and says to end the turn. Red before the change (3 failures), 62 passed after.
- `claude-plugins/codescout-companion/hooks/wake-missed-jobs.test.sh` (new, 24 checks): a missed end is reported once and claims `.delivered`; silent for a delivered end, a running job, a live watcher, another session's job, a missing, junk or schema-2 record, and no session id; a stale `.watch` pid does not hide a miss; a dead server gets no read hint; two missed jobs in one message. Mutation check: removing the live-watcher guard turned 2 checks red.
- `src/tools/run_command/tests.rs`, `run_in_background_returns_bg_handle`: the spawn hint states the 30-minute limit and its consequence.
- Gate: `./scripts/gate.sh` green on 2026-11-11 (FMT, CLIPPY, LEAN, DEFAULT and Python all exit 0).

## Workarounds

- After a "still running after 30 min" wake, run one command that names the handle (for example `run_command("tail -1 @bg_N")`) before ending the turn. This re-arms a watcher for another 30 minutes. Repeat after each heartbeat.
- A command that reads the job's log through a file path does not re-arm.
- Starting a wait-on-pid job (`tail --pid=<pid> -f /dev/null` as a background job) does not help: it is a job too, with the same 30-minute window.
- Use `timeout_secs` in the foreground for a job that fits the harness limit; the harness moves it to a task and notifies on completion (Evidence 5).

## Resume

Settle the open points in this order. All are read-only.

1. Count again: `python3 -I` over `$TMPDIR/codescout-bg/*.json` for armed ended jobs of 30 min or longer without `.delivered`, and list the session of each via `~/.local/state/codescout-companion/wake/map-<session>-<handle>` (file content is the record path). Compare with the 43 above; separate servers killed on purpose (exit 137) from jobs a session was waiting on.
2. Live repro with `CODESCOUT_WAKE_DEADLINE_SECS=45` and a 150 s job, no read after the heartbeat. Expect: heartbeat at 45 s, no end wake at 150 s, no `.watch` between them. This turns the root cause from "read from code" into "measured".
3. Check whether the 13 delivered jobs of 30 min or longer that were not read here (only 5 were matched to a transcript) each had a read after the heartbeat.
4. The 125 ended records without `.wake-owner.json`: sample 5 and find their spawn in a transcript. The hook exits 0 for a subagent (`agent_id` present, `wake-on-job-end.mjs:251`) and for a non-Claude-Code harness, so many may be expected; this was not checked. Do not treat these as part of this bug until checked.
5. The 607-minute job `@bg_0000000b` of session B (profile `~/.claude`) had its first and only recorded heartbeat at 606 min, not at 30 min (heartbeat at 2026-11-06T06:03:05Z for a job spawned 2026-11-05T19:56Z). A machine suspend that paused the watcher's timer would explain it; not checked (`journalctl` for suspend entries on that night). If true, a suspend delays the heartbeat, and the end wake of a job that ends during a suspend needs its own check.
6. The owner counts 3 failures; 43 records fit the pattern. Ask which 3 the owner means; `@bg_00000011` is one.

## References

- Hook: `claude-plugins/codescout-companion/hooks/wake-on-job-end.mjs`, `claude-plugins/codescout-companion/hooks/hooks.json:149-151`, `claude-plugins/codescout-companion/background-wake/intake.mjs` (`selectEngine`, owner sidecar).
- Server side: `src/tools/job_record.rs` (the record), `src/tools/run_command/inner.rs:167-171` (spawn hint).
- Guides: `src/prompts/guides/project-activation-bootstrap.md:19-20`, `src/prompts/guides/progressive-disclosure.md:123-136`.
- Design: `docs/superpowers/specs/2026-10-29-background-job-wake-design.md` (section 4, line 139-145; non-goal at line 76).
- Related issues: `docs/issues/2026-11-10-wake-hook-promotion-effect-on-polling-is-unmeasured.md` (sessions poll although a wake exists; this issue is a cause of that polling that was not named there), `docs/issues/2026-11-01-the-wake-plugins-v1-engine-never-writes-delivered-so-the-log-reaper-grace-never-fires.md`.
- Sessions (transcripts under `~/.claude*/projects/*/<id>.jsonl`): A `be7356ea-b0e8-4ba3-8375-9ca446490fb5` (`~/.claude-stef`), C `fbe38184-5a8d-4bc3-89dd-fdd3024c909a` (`~/.claude-kat`), B `74fdade4-0164-427c-82cc-8b933b74ec4c` (`~/.claude`).
