# Background Job Wake-Up Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A session that starts `run_command(run_in_background=true)` is woken, even when idle, when the job ends, with a heartbeat for long jobs.

**Architecture:** codescout writes a JSON completion record per background job and returns its path in the spawn response. A codescout-companion `PostToolUse` hook with `asyncRewake: true` arms on the spawn response, polls the record and exits 2 to wake the session. The record file is the only contract between the two repositories.

**Tech Stack:** Rust (codescout: `serde`, `chrono`, `tokio`), Node.js ESM hook (codescout-companion, no `jq`, no Bash), Bash test scripts.

**Spec:** `docs/superpowers/specs/2026-10-29-background-job-wake-design.md`. Read it first: it holds the measurements and the reasons behind every rule below.

## Global Constraints

- Record directory: `std::env::temp_dir().join("codescout-bg")`, created with mode `0700`.
- Record file name: `<server_pid>-<token>-<handle without "@">.json`, for example `4112-9f3a1c07-bg_00000001.json`. `<token>` is 8 hex characters chosen once per `OutputBuffer`. **Deviation from the spec, my call:** the spec has `<server_pid>-<handle>.json`. One test process builds many `OutputBuffer`s, each with handles from `@bg_00000001`, so pid and handle alone collide. The hook never builds this name; it reads the path from the response.
- Record fields: `schema` (1), `handle`, `command`, `server_pid`, `job_pid`, `state` (`running` | `exited` | `failed`), `exit_code`, `error`, `started_at`, `finished_at` (RFC 3339). No log content and no log path.
- `command` in the record goes through `crate::util::redact::redact_credentials` first.
- Records older than 7 days are swept, except a `running` record whose `server_pid` is alive.
- Hook registration: `PostToolUse`, matcher `mcp__codescout__run_command`, `asyncRewake: true`, `timeout: 7200`.
- Hook deadline: `CODESCOUT_WAKE_DEADLINE_SECS`, default `1800`.
- Hook arm rule: `output_id` starts with `@bg_` **and** the parsed response has no `exit_code` key. Never `tool_input.run_in_background`.
- Hook exit codes: exit 2 only for a deliberate wake-up. Every error path exits 0. This is the companion's fail-open contract (`hooks/lib.mjs` header).
- Test isolation (`docs/conventions/test-env-isolation.md`): no test sets a process env var. Pass the record directory in.
- `tools/list` does not change: no `run_command` description or schema edit. The `tool_surface_under_budget` budget stays as it is. The spawn `hint` must not promise a wake-up.
- Before marking any codescout task complete: `./scripts/gate.sh`. Commits follow `docs/conventions/shared-checkout-commit-sequence.md`. Cite the bug-fix session-log entry F-201 in the commit body of the codescout tasks.

## Review Focus

1. **A dangerous background command, which needs an ack.** The user expects a wake-up as for any other job. Owner: Task 2 (server test `an_acked_background_spawn_carries_a_record`) and Task 3 (hook case `acked replay arms`).
2. **More than `max_pending` live jobs, so a live handle is evicted.** The job's record must still reach `exited`. Owner: Task 2 (`an_evicted_live_job_still_finalises_its_record`).
3. **The codescout server is restarted with `/mcp` while a job runs.** The user expects a message that says the handle is void, not an instruction to read it. Owner: Task 3 (cases `server dead, job alive` and `server dead, job dead`).
4. **The session reads a job that has already ended.** The read must not arm a watcher, so no loop. Owner: Task 4 (case `read of a finished job does not arm`).
5. **The record directory cannot be written, for example when it is owned by another user.** The spawn must still succeed and say that no wake-up will come. Owner: Task 2 (`a_record_write_failure_is_reported_not_fatal`).

---

### Task 1: The job record module (codescout)

**Files:**
- Create: `src/tools/job_record.rs`
- Modify: `src/tools/mod.rs` (add `pub mod job_record;` next to `pub mod output_buffer;`)
- Test: `src/tools/job_record.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `crate::tools::output_buffer::JobState`, `crate::util::redact::redact_credentials`.
- Produces:
  - `pub const RECORD_SCHEMA: u32 = 1;`
  - `pub const RECORD_MAX_AGE: std::time::Duration` (7 days)
  - `pub fn default_record_dir() -> PathBuf`
  - `#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)] pub struct JobRecord { schema, handle, command, server_pid: u32, job_pid: Option<u32>, state: RecordState, exit_code: Option<i32>, error: Option<String>, started_at: String, finished_at: Option<String> }`
  - `#[serde(rename_all = "lowercase")] pub enum RecordState { Running, Exited, Failed }`
  - `impl JobRecord { pub fn running(handle: &str, command: &str, job_pid: Option<u32>) -> Self; pub fn finish(&mut self, state: &JobState) }`
  - `pub fn record_path(dir: &Path, token: &str, handle: &str) -> PathBuf`
  - `pub fn write_record(path: &Path, record: &JobRecord) -> std::io::Result<()>`
  - `pub fn remove_if_delivered(path: &Path) -> bool`
  - `pub fn sweep_stale(dir: &Path, max_age: Duration, is_alive: impl Fn(u32) -> bool) -> usize`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_running_record_redacts_its_command_and_names_this_process() {
    let rec = JobRecord::running("@bg_00000001", "TOKEN=ghp_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa make", Some(42));
    assert_eq!(rec.state, RecordState::Running);
    assert_eq!(rec.server_pid, std::process::id());
    assert_eq!(rec.job_pid, Some(42));
    assert!(!rec.command.contains("ghp_aaaa"), "{}", rec.command);
    assert!(rec.finished_at.is_none() && rec.exit_code.is_none());
}

#[test]
fn finish_maps_every_job_state() {
    let mut r = JobRecord::running("@bg_00000001", "true", None);
    r.finish(&JobState::Exited { code: Some(3) });
    assert_eq!((r.state.clone(), r.exit_code), (RecordState::Exited, Some(3)));
    assert!(r.finished_at.is_some());
    let mut s = JobRecord::running("@bg_00000002", "true", None);
    s.finish(&JobState::Exited { code: None });
    assert_eq!((s.state.clone(), s.exit_code), (RecordState::Exited, None));
    let mut f = JobRecord::running("@bg_00000003", "true", None);
    f.finish(&JobState::Failed { error: "boom".into() });
    assert_eq!((f.state.clone(), f.error.as_deref()), (RecordState::Failed, Some("boom")));
}

#[test]
fn record_path_strips_the_at_sign_and_carries_pid_and_token() {
    let p = record_path(Path::new("/d"), "9f3a1c07", "@bg_00000001");
    assert_eq!(p, PathBuf::from(format!("/d/{}-9f3a1c07-bg_00000001.json", std::process::id())));
}

#[test]
fn write_record_round_trips_and_leaves_no_temp_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = record_path(dir.path(), "00000000", "@bg_00000001");
    let rec = JobRecord::running("@bg_00000001", "true", Some(1));
    write_record(&path, &rec).unwrap();
    let back: JobRecord = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(back, rec);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn write_record_creates_its_directory_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let base = tempfile::tempdir().unwrap();
    let dir = base.path().join("codescout-bg");
    write_record(&record_path(&dir, "00000000", "@bg_00000001"), &JobRecord::running("@bg_00000001", "true", None)).unwrap();
    assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
}

#[test]
fn remove_if_delivered_keeps_undelivered_and_running_records() {
    // running + .delivered -> kept; exited without .delivered -> kept;
    // exited + .delivered -> record, .delivered and .watch all removed, returns true
}

#[test]
fn sweep_keeps_young_records_and_live_running_ones() {
    // max_age = 0: an exited record is removed; a running record whose server_pid
    // is_alive -> kept; a running record with a dead pid -> removed; count returned.
}
```

The two tests with comment bodies must assert exactly what their comments state. Unix-only tests take `#[cfg(unix)]`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/with-slot.sh cargo test --lib tools::job_record`
Expected: compile failure, because `job_record` does not exist.

- [ ] **Step 3: Implement the module**

`write_record` serializes to `<path>.tmp-<pid>` in the same directory, then calls `std::fs::rename`. It creates the directory with `std::fs::DirBuilder::new().recursive(true).mode(0o700)` on Unix. Timestamps: `chrono::Utc::now().to_rfc3339()`. `remove_if_delivered` deletes only when the record parses as non-`running` **and** `<path>.delivered` exists. It also deletes `<path>.delivered` and `<path>.watch`. `sweep_stale` matches `*.json` files only, judges age by mtime, and removes the sidecar files with the record. Unreadable files are skipped, never an error.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/with-slot.sh cargo test --lib tools::job_record`
Expected: 7 passed.

- [ ] **Step 5: Commit**

Message: `feat(run-command): add the background job completion record` — body cites F-201 and the spec path.

---

### Task 2: Write records from the background supervisor (codescout)

**Files:**
- Modify: `src/tools/output_buffer.rs` (`OutputBuffer` fields, `with_byte_limit`, new `with_record_dir`, `record_dir`, `record_token`, `store_background`)
- Modify: `src/tools/run_command/inner.rs` (`spawn_background_command`)
- Test: `src/tools/run_command/tests.rs`, `src/tools/output_buffer.rs` tests

**Interfaces:**
- Consumes: everything Task 1 produces.
- Produces:
  - `impl OutputBuffer { pub fn with_record_dir(self, dir: PathBuf) -> Self; pub fn record_dir(&self) -> &Path; pub fn record_token(&self) -> &str }`. `with_byte_limit` sets `record_dir = job_record::default_record_dir()` and a random 8-hex `record_token`. `OutputBuffer::new(n)` keeps its signature, because about 200 call sites use it.
  - The spawn response gains `"record": "<absolute path>"`. If the first write fails, it gains `"record_error": "<io error>"` instead, with no `record`, and the `hint` gains: `No completion record could be written, so no wake-up will come for this job.`

- [ ] **Step 1: Write the failing tests**

In `src/tools/run_command/tests.rs`, use a context whose buffer is `OutputBuffer::new(20).with_record_dir(tmp.path().join("bg"))`:

```rust
#[tokio::test]
async fn a_background_spawn_writes_a_running_record_and_returns_its_path() {
    // spawn "sleep 5" in background; result["record"] is a path under the tmp dir;
    // the file parses as JobRecord with state Running, handle == result["output_id"],
    // job_pid Some(_), server_pid == std::process::id(); result has no "exit_code".
}

#[tokio::test]
async fn the_record_reaches_exited_with_the_code_when_the_job_ends() {
    // spawn "exit 3"; poll the record (<= 5 s, 50 ms steps) until state != Running;
    // assert state Exited, exit_code Some(3), finished_at Some(_).
}

#[tokio::test]
async fn an_evicted_live_job_still_finalises_its_record() {
    // max_pending is fixed at 20 (with_byte_limit), so: spawn a real "sleep 1" job first,
    // then store 20 fake Running BackgroundJobs directly with store_background (as the
    // store_jobs helper in this file does). That evicts the real job's handle while it
    // runs. After it ends, its record must read Exited.
}

#[tokio::test]
async fn an_acked_background_spawn_carries_a_record() {
    // store_dangerous("sleep 1", None, 30, true) then call RunCommand with {"command": handle}
    // (as in ack_re_dispatch_honours_run_in_background); assert output_id starts "@bg_",
    // no "exit_code", and "record" names an existing file.
}

#[tokio::test]
async fn a_record_write_failure_is_reported_not_fatal() {
    // with_record_dir(<a path whose parent is a regular FILE>); spawn "true";
    // assert output_id starts "@bg_", result["record"].is_null(),
    // result["record_error"] is a string, hint contains "no wake-up will come".
}
```

In `src/tools/output_buffer.rs` tests:

```rust
#[test]
fn evicting_a_terminated_job_removes_its_record_only_when_delivered() {
    // two exited jobs with records in a tmp record dir, only the first has
    // "<record>.delivered"; force eviction of both by storing past max_pending;
    // the delivered record is gone, the undelivered record still exists.
}
```

Every comment body becomes assertions that state exactly that.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/with-slot.sh cargo test --lib -- record`
Expected: compile errors on `with_record_dir`, then assertion failures on `record`.

- [ ] **Step 3: Implement**

In `spawn_background_command`:
1. Spawn as today.
2. Take `child.id()` for `job_pid`.
3. Call `store_background`.
4. Compute `record_path(buffer.record_dir(), buffer.record_token(), &ref_id)` and write `JobRecord::running(...)`.
5. The supervisor task owns a clone of the record and its path. After `child.wait()` it calls `record.finish(&state)` and `write_record`. It does this **before** `buffer.set_job_state`, and never through it: `set_job_state` silently does nothing for an evicted handle. A failed final write is logged with `tracing::warn!`.

In `store_background`, in the terminated-victim branch only, call `job_record::remove_if_delivered(&record_path(...victim...))` next to the existing `remove_file(&job.log_path)`. Never in the live-victim branch.

On the first `store_background` of a buffer, run `sweep_stale(record_dir, RECORD_MAX_AGE, pid_alive)` once (a `std::sync::Once` field). `pid_alive` is `libc::kill(pid, 0) == 0`. **My call:** the spec says "at server start". The first spawn is the first moment the directory matters, and this keeps the sweep out of server startup.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/with-slot.sh cargo test --lib -- record` and `scripts/with-slot.sh cargo test --lib tools::output_buffer`
Expected: all pass, including the existing eviction tests.

- [ ] **Step 5: Gate and commit**

Run: `./scripts/gate.sh`. Expected: green. Run `./scripts/mutation-probe.sh` on `src/tools/job_record.rs` and on the `spawn_background_command` changes. Survivors are listed in the commit body or killed with a test.
Message: `feat(run-command): background jobs write a completion record the companion can wait on`, citing F-201.

---

### Task 3: The wake hook: arm and watch (codescout-companion)

**Files:**
- Create: `../claude-plugins/codescout-companion/hooks/wake-on-job-end.mjs`
- Create: `../claude-plugins/codescout-companion/hooks/wake-on-job-end.test.sh`

**Interfaces:**
- Consumes: the spawn response (`output_id`, `record`, no `exit_code`) and the record JSON from Task 1.
- Produces: exported pure helpers, for the test and for Task 4:
  - `export function spawnOf(input) -> { handle, record } | null`
  - `export function wakeMessage(kind, ctx) -> string`, where `kind` is one of `exited`, `failed`, `vanished`, `heartbeat`, `server-ended-job-ended`, `server-ended-job-running`, `server-ended-unknown`
  - `export async function watch({ handle, record, deadlineSecs, pollMs }) -> { exit: 0 | 2, message?: string }`
  - The default entry point reads stdin with `readInput()` from `./lib.mjs`, decides, and writes `message` to stderr before `process.exit(exit)`.

Behavior, in order:
1. If `process.env.CLAUDE_PROJECT_DIR` is unset, exit 0. The hook then never blocks a host that ignores `asyncRewake`. **Verify in Task 5** that Claude Code sets it for plugin hooks.
2. If the input has a non-empty `agent_id`, exit 0. Subagent jobs are out of scope.
3. `spawnOf`: parse `tool_response[0].text` as JSON. Return `null` unless `output_id` starts with `@bg_`, the object has no `exit_code` key, and `record` is a string.
4. `watch` polls every `pollMs` (default 1000):
   - The record is missing: wake with `vanished`.
   - The state is `exited` or `failed`: create `<record>.delivered` with `writeFileSync(..., { flag: 'wx' })`. On `EEXIST`, exit 0. Otherwise wake with that state's message.
   - `server_pid` is dead and `job_pid` is alive: keep polling `job_pid` only. When it dies, wake with `server-ended-job-ended`. At the deadline, wake with `server-ended-job-running`.
   - `server_pid` and `job_pid` are both dead: wake with `server-ended-unknown`.
   - The deadline has passed: delete `<record>.watch` and wake with `heartbeat`.
   - At start, `watch` writes its own pid to `<record>.watch`.
5. A pid is alive when `process.kill(pid, 0)` does not throw `ESRCH`.

Message rules (exact copy for the server-ended kinds):
- `exited` / `failed`: `codescout background job <handle> ended: <exited N | terminated by signal | failed: error> after <duration>. Read it with run_command("tail -50 <handle>").`
- `heartbeat`: `codescout background job <handle> is still running after <M> min. Read it with run_command("tail -50 <handle>") to keep waiting.`
- `server-ended-*`: start with `The codescout server that ran <handle> has ended, so <handle> is no longer valid and may now name a different job. Do not read it with run_command.` Then add one of: `The job (pid N) has also ended; its exit code is unknown.` / `The job is still running as pid N.` / `The job's state is unknown.`

- [ ] **Step 1: Write the failing test script**

Follow the style of `hooks/cs-liveness.test.sh`: `pass` / `fail` counters, `mktemp -d`, and a non-zero exit when any case fails. Run the hook with `CLAUDE_PROJECT_DIR=$TMP`, `CODESCOUT_WAKE_DEADLINE_SECS=3`, and a test-only `CODESCOUT_WAKE_POLL_MS=100` read only by the entry point. Cases, each asserting the exit code and a stderr substring:
- `spawn arms, record exited 0` gives exit 2 and `ended: exited 0`.
- `record failed` gives exit 2 and `failed:`.
- `read response (has exit_code) does not arm` gives exit 0 and empty stderr.
- `acked replay arms`: input `{"tool_input":{"command":"@ack_x"}, "tool_response":[{"text":"{\"output_id\":\"@bg_00000001\",\"record\":\"…\"}"}]}` gives exit 2.
- `pending ack does not arm`: the response has `pending_ack` and no `output_id`. Exit 0.
- `agent_id present does not arm`: exit 0.
- `no CLAUDE_PROJECT_DIR does not arm`: exit 0.
- `record vanishes`: delete the record after 0.5 s. Exit 2 and `vanished`.
- `two watchers race`: two hooks on one record, then the record turns `exited`. Exactly one exits 2.
- `heartbeat`: the record stays `running` with a live `server_pid` (`$$`). Exit 2 within 5 s, `still running`, and `<record>.watch` is removed.
- `server dead, job alive`: `server_pid` is a dead pid and `job_pid` is a `sleep 1` started by the test. Exit 2, `no longer valid` and `has also ended`.
- `server dead, job dead`: both pids dead. Exit 2 and `state is unknown`. The stderr has no `tail -50`.

- [ ] **Step 2: Run it to verify it fails**

Run: `bash ../claude-plugins/codescout-companion/hooks/wake-on-job-end.test.sh`
Expected: every case fails, because the hook does not exist.

- [ ] **Step 3: Implement `wake-on-job-end.mjs`**

Wrap the whole entry point in `try { … } catch { process.exit(0) }`.

- [ ] **Step 4: Run it to verify it passes**

Run: the same command. Expected: all 12 cases pass, and the script exits 0.

- [ ] **Step 5: Commit (companion repo)**

Message: `feat(wake): wake the session when a codescout background job ends`. The body cites `codescout:<sha of Task 2>` and the spec path.

---

### Task 4: Re-arm after a heartbeat (codescout-companion)

**Files:**
- Modify: `../claude-plugins/codescout-companion/hooks/wake-on-job-end.mjs`
- Modify: `../claude-plugins/codescout-companion/hooks/wake-on-job-end.test.sh`

**Interfaces:**
- Consumes: `spawnOf`, `watch` from Task 3; `guideLedgerPath`'s state-home resolution from `hooks/lib.mjs`. Reuse its `XDG_STATE_HOME` logic, do not copy it.
- Produces:
  - `export function mapPath(sessionId, handle) -> string`: `<state home>/codescout-companion/wake/map-<sessionId>-<handle>`.
  - Arming a spawn now also writes `mapPath(session_id, handle)` with the record path as content.
  - `export function rearmTarget(input) -> { handle, record } | null`.

`rearmTarget` returns a target only when all of these hold:
- The response has an `exit_code` key, so it is a read.
- `tool_input.command` names an `@bg_[0-9a-f]{8}` handle.
- `mapPath(session_id, handle)` exists and holds a record path.
- That record exists with state `running`.
- `<record>.watch` is absent, or names a dead pid.

Then `watch` runs as in Task 3. A map file is not a record, so a missing map never wakes.

- [ ] **Step 1: Add the failing cases**
- `read of a running job after heartbeat re-arms`: map present, record running, no watch file. The hook stays alive, then exits 2 when the test flips the record to `exited`.
- `read of a finished job does not arm`: record exited. Exit 0 at once.
- `read while a watcher is alive does not arm`: the watch file holds `$$`. Exit 0.
- `read with no map does not arm`: exit 0.
- `spawn writes the map`: after a spawn case, the map file exists under `$XDG_STATE_HOME`.

- [ ] **Step 2: Run to verify the new cases fail**

Run: `bash ../claude-plugins/codescout-companion/hooks/wake-on-job-end.test.sh`. Expected: the 5 new cases fail, and the 12 old ones pass.

- [ ] **Step 3: Implement `mapPath`, `rearmTarget` and the map write**

- [ ] **Step 4: Run to verify all pass**

Expected: 17 passed.

- [ ] **Step 5: Commit (companion repo)**

Message: `feat(wake): re-arm the watcher when the session reads a still-running job`.

---

### Task 5: Register, release and measure end to end

**Files:**
- Modify: `../claude-plugins/codescout-companion/hooks/hooks.json` (a new `PostToolUse` entry)
- Modify: `../claude-plugins/codescout-companion/.claude-plugin/plugin.json` (version `1.20.21` → `1.21.0`)
- Modify: `../claude-plugins/docs/trackers/version-bump-checklist.md` (follow it)
- Modify: `docs/issues/2026-10-29-codescout-has-no-background-wait-and-notify-mechanism-and-claude-code-monitor-is-removed-by-the-bash-deny.md` (`## Fix`, `## Tests added`, `## Resume`)

**Interfaces:**
- Consumes: the hook from Tasks 3–4 and a codescout binary with Task 2.

- [ ] **Step 1: Register the hook**

Add this entry to `PostToolUse` in `hooks.json`:

```json
{ "matcher": "mcp__codescout__run_command",
  "hooks": [{ "type": "command",
              "command": "node ${CLAUDE_PLUGIN_ROOT}/hooks/wake-on-job-end.mjs",
              "asyncRewake": true, "timeout": 7200 }] }
```

- [ ] **Step 2: Install the codescout build**

Run: `./scripts/rb.sh`. Expected: the release binary is installed in `~/.cargo/bin/codescout`.

- [ ] **Step 3: Repeat the spec measurements with the real parts**

Use the throwaway method from the spec: an empty `CLAUDE_CONFIG_DIR` with a copied `.credentials.json`, `permissions.deny = ["Bash","Write","Edit"]`, `claude --model haiku --plugin-dir ../claude-plugins/codescout-companion` in `tmux`, the prompt typed first, then `C-m`. Delete the credential copy afterwards. Record each result:
- (a) A spawn of `sleep 20; echo done` wakes the idle session, and the model reads it with `tail -50`.
- (b) With `CODESCOUT_WAKE_DEADLINE_SECS=45` exported before `claude` starts, a `sleep 150` job gives heartbeats and re-arms, and ends in one final wake-up.
- (c) A dangerous background command (`rm -rf <scratch dir>/x && sleep 10`), acked, wakes the session.
- (d) Check `CLAUDE_PROJECT_DIR` in the hook environment: run (a) once with a temporary `env > <scratch>/hook-env.txt` line added at the top of the hook, then remove the line.
- (e) Check `agent_id`: a subagent spawns a background job. The hook must not arm, so no watch file appears.

Expected: (a)–(c) wake the session. (d) shows `CLAUDE_PROJECT_DIR`. (e) arms nothing. If (d) or (e) fails, stop and report: the guard in Task 3 is wrong.

- [ ] **Step 4: Release the companion**

Follow `../claude-plugins/docs/trackers/version-bump-checklist.md`. Per the operator's CLAUDE.md, apply the update to all four profiles: `~/.claude`, `~/.claude-sdd`, `~/.claude-kat` and `~/.claude-stef`. Check that each profile's `installPath` lies under its own `cache/`.

- [ ] **Step 5: Close the loop in codescout**

Update the issue: `## Fix` names the codescout and companion commits, `## Tests added` lists the tests from Tasks 1–4, and `## Resume` says "shipped; the operator rule 'activate a Monitor' in the CLAUDE.md files is the operator's to update". Set the F-201 status to `fixed-verified` with `doc(action="update_entry", …)`. Commit per the shared-checkout sequence.
