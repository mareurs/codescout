# Background-Command Log Reaper Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop `codescout-bg-*.log` files from piling up: a log is removed after its outcome was delivered and read, or after a fixed age, and the existing pile is reclaimed once, without ever removing a running job's log.

**Architecture:** `JobRecord` gains an optional `log_path` (schema stays 1). Two new sweeps in `src/tools/job_record.rs` (one linked to records, one for loose legacy logs) run in `OutputBuffer`'s existing once-per-server housekeeping, which moves onto a detached thread in production and runs inline in tests. A small Linux helper in `src/platform/` lists the files that processes hold open.

**Tech Stack:** Rust 2021 (rustc 1.97), `serde`/`serde_json`, `tempfile` (tests), `std::thread`, Linux `/proc/*/fd`.

**Spec:** `docs/superpowers/specs/2026-10-31-background-log-reaper-design.md` (commit `1ed1d6333`). Bug: `docs/issues/2026-10-31-background-command-logs-are-never-reclaimed-when-the-server-ends.md` (`45b69c70b723a089`). Read both before any task.

## Global Constraints

- `RECORD_SCHEMA` stays `1`. The companion plugin's wake hook returns early for any other schema (`claude-plugins/codescout-companion/hooks/wake-on-job-end.mjs:160,230`).
- JSON field name `log_path`, type `Option<String>`, `#[serde(default, skip_serializing_if = "Option::is_none")]`. A record without the field must still parse.
- Constants in `src/tools/job_record.rs`: `LOG_GRACE_AFTER_DELIVERY = 1 hour`, `LOG_MAX_AGE = 24 hours`; `RECORD_MAX_AGE` stays 7 days. Each new constant carries a `// cap-class: NOT_A_CAP — <reason>` line directly above it, as `RECORD_MAX_AGE` does.
- Never remove a running job's log. A `running` record with a live server keeps its log. A `running` record with a dead server keeps its log until `RECORD_MAX_AGE` (the job may be an orphan that still writes).
- A log is only ever unlinked when it is a regular file (not a symlink), directly inside the log directory (`std::env::temp_dir()` in production), with a name that starts with `codescout-bg-` and ends with `.log`. A record that names any other path is skipped, never followed.
- The legacy sweep takes the log directory as a PARAMETER. `OutputBuffer` holds it in a field that is `Some(std::env::temp_dir())` in production and `None` when `cfg!(test)`. No test may scan the real temp directory.
- Housekeeping never fails a tool call: errors are ignored or logged with `tracing::debug!`.
- No `unsafe`. Windows must still compile: `pid_alive` is `true` there, and the open-file helper returns `None` outside Linux.
- No prompt-surface change (`src/prompts/`, `.codescout/system-prompt.md`): `ONBOARDING_VERSION` stays.
- Run cargo through the leased pool: `CARGO_BUILD_JOBS=16 scripts/with-slot.sh cargo test --lib -- <filter>`. Do not run `./scripts/gate.sh`.
- Shared checkout: commit by pathspec with a `Session-Id:` trailer, per `docs/conventions/shared-checkout-commit-sequence.md`. Each task's code and tests land in one commit.

## Review Focus

1. **Test isolation (the one that could hurt other people).** The housekeeping runs the first time any test spawns a background job. If the legacy sweep ever scanned the real temp directory, `cargo test` would delete other sessions' logs. The `log_dir` field must be `None` in test builds, and a test must prove that a test-built `OutputBuffer` leaves a planted old `codescout-bg-*.log` in the real temp directory alone (Task 4).
2. **A tampered or stale record must not become an arbitrary-file delete.** A record whose `log_path` is a path outside the log directory, a symlink, or a directory must be skipped (Task 2).
3. **A running job's log survives every sweep:** live server, dead server before `RECORD_MAX_AGE`, and a loose log that a process holds open (Tasks 2 and 3).
4. **The wake flow keeps working:** an old record without `log_path` parses; a new record is still `schema: 1`; the plugin test cases 20c/20e premise holds (Task 1).
5. **mtime edge cases:** a future mtime (clock skew) counts as age zero, never as "very old"; a missing `.delivered` sidecar means "not delivered"; an unreadable sidecar mtime means "not delivered" (Task 2).

---

### Task 1: The record names its log

**Files:**
- Modify: `src/tools/job_record.rs` (struct `JobRecord` at L62-75, `impl JobRecord` at L77-111; tests module)
- Modify: `src/tools/run_command/inner.rs` (`spawn_background_command`, L131)
- Test: `src/tools/job_record.rs` (tests module), `src/tools/run_command/tests.rs` (helpers `read_job_record` L13753, `finished_record` L13758)

**Interfaces:**
- Produces: `JobRecord.log_path: Option<String>` and `JobRecord::with_log_path(self, path: &Path) -> Self` (stores `path.display().to_string()`). `JobRecord::running` keeps its signature and sets `log_path: None`.

- [ ] **Step 1: Write the failing tests** in `job_record.rs` tests:
  - `a_record_without_log_path_still_parses`: parse a JSON string with the ten old fields and assert `log_path == None`.
  - `a_record_with_log_path_round_trips_and_stays_schema_1`: `JobRecord::running(...).with_log_path(Path::new("/x/codescout-bg-abc.log"))`, write then read it; assert `log_path == Some("/x/codescout-bg-abc.log")` and `schema == 1`.
  - `a_record_without_log_path_serialises_no_log_path_key`: the serialised JSON of `JobRecord::running(...)` does not contain `"log_path"`.
  And in `src/tools/run_command/tests.rs`: `a_background_job_record_names_its_real_log`: spawn a background `true`, read the record (existing helpers), assert `log_path` is `Some`, that the file exists, and that its name starts with `codescout-bg-` and ends with `.log`.
- [ ] **Step 2: Run to see them fail:** `CARGO_BUILD_JOBS=16 scripts/with-slot.sh cargo test --lib -- tools::job_record a_background_job_record_names_its_real_log`. Expected: compile errors for `log_path`/`with_log_path` (that is the failure).
- [ ] **Step 3: Implement** the field, the builder method, and in `spawn_background_command` build the record as `JobRecord::running(&ref_id, resolved_command, job_pid).with_log_path(&log_path)`. The `log_path` variable already exists (L103).
- [ ] **Step 4: Run again.** Expected: all listed tests pass, and every other test in `tools::job_record` still passes.
- [ ] **Step 5: Commit** `src/tools/job_record.rs src/tools/run_command/inner.rs src/tools/run_command/tests.rs`: `feat(run-command): the job record names its log (45b69c70b723a089)`.

### Task 2: Lifetimes and the record-linked sweep

**Files:**
- Modify: `src/tools/job_record.rs` (constants near L36; `sweep_stale` L212; new `sweep_logs`; tests module)
- Modify: `src/tools/output_buffer.rs` (only the `sweep_stale` call site at L1672)

**Interfaces:**
- Consumes: `JobRecord::log_path` (Task 1), `read_record`, `sidecar`, `remove_with_sidecars`, `RecordState`.
- Produces:
  - `pub const LOG_GRACE_AFTER_DELIVERY: Duration`, `pub const LOG_MAX_AGE: Duration` (values in Global Constraints).
  - `fn unlink_log_if_ours(log_dir: &Path, log_path: &str) -> bool`: applies the safety rule in Global Constraints (regular file, not a symlink, parent equals `log_dir`, `codescout-bg-` prefix, `.log` suffix); returns whether a file was removed.
  - `pub fn sweep_logs(record_dir: &Path, log_dir: &Path, grace: Duration, max_age: Duration, is_alive: impl Fn(u32) -> bool) -> usize`: returns the number of logs removed.
  - `sweep_stale` gains a `log_dir: &Path` parameter after `dir` and unlinks `rec.log_path` (through `unlink_log_if_ours`) before `remove_with_sidecars`. Update its five existing call sites (one in `output_buffer.rs`, four in the `job_record.rs` tests).

- [ ] **Step 1: Write the failing tests** (use the existing `private_tempdir`, `seed`; add a helper `seed_with_log(dir, log_dir, token, handle, state, delivered) -> (PathBuf record, PathBuf log)` that also writes a `codescout-bg-<handle>.log` file in `log_dir` and a record naming it; set file mtimes with `std::fs::File::set_modified`). Names:
  - `sweep_logs_removes_the_log_one_grace_after_delivery_and_keeps_the_record`
  - `sweep_logs_keeps_a_delivered_log_inside_the_grace`
  - `sweep_logs_removes_an_undelivered_finished_log_after_max_age_and_keeps_it_before`
  - `sweep_logs_never_touches_a_running_jobs_log_with_a_live_server`
  - `sweep_logs_leaves_a_dead_servers_running_log_to_the_record_rule`
  - `sweep_logs_skips_a_log_path_outside_the_log_dir_a_symlink_and_a_directory` (one record names a file in a SECOND temp dir outside `log_dir`, never a real system path; another names a symlink inside `log_dir` whose target is a file in that second dir; another a directory named `codescout-bg-x.log`; assert all of them, and the symlink target, still exist)
  - `sweep_logs_treats_a_future_mtime_as_age_zero`
  - `sweep_logs_treats_a_missing_delivered_sidecar_as_not_delivered`
  - `sweep_stale_removes_the_log_with_its_record_and_survives_a_record_whose_log_is_gone`
- [ ] **Step 2: Run to see them fail:** `CARGO_BUILD_JOBS=16 scripts/with-slot.sh cargo test --lib -- tools::job_record`. Expected: compile errors for the missing items.
- [ ] **Step 3: Implement** per the Interfaces block. `sweep_logs` reads each `*.json` record (same filter as `sweep_stale`), decides per the design's section 3: finished record with a `log_path`: unlink the log when the `.delivered` sidecar's mtime is at least `grace` old, or the record file's mtime is at least `max_age` old; never act on `running` records. A file whose metadata cannot be read is skipped. Do not remove the record in `sweep_logs`.
- [ ] **Step 4: Run again.** Expected: every listed test passes and all earlier `tools::job_record` tests still pass.
- [ ] **Step 5: Check the cap-class gate:** `CARGO_BUILD_JOBS=16 scripts/with-slot.sh cargo test -p codescout --test result_caps every_cap_constant_is_classified`. Expected: it passes, OR (while bug `6fda4eca9da69652` is unfixed on the base branch) it fails listing ONLY `src/lsp/watch_fanout.rs:20 FANOUT_MAX_ENTRIES`. If it lists `job_record.rs`, a new constant lacks its `cap-class` line.
- [ ] **Step 6: Commit** `src/tools/job_record.rs src/tools/output_buffer.rs`: `feat(run-command): sweep a finished job's log one hour after delivery or a day after the job ended (45b69c70b723a089)`. (`output_buffer.rs` changes only the `sweep_stale` call site here.)

### Task 3: The legacy sweep and the open-file helper

**Files:**
- Modify: `src/platform/mod.rs` (new `open_file_paths`, near `process_alive` L86), `src/tools/job_record.rs` (new `recorded_log_paths`, `sweep_loose_logs`; tests)

**Interfaces:**
- Produces:
  - `pub fn open_file_paths() -> Option<std::collections::HashSet<std::path::PathBuf>>` in `crate::platform`: on Linux, the set of targets of `/proc/[0-9]*/fd/*` symlinks that can be read (entries it cannot read are skipped); `None` when `/proc` cannot be listed and on every other platform.
  - `pub fn recorded_log_paths(record_dir: &Path) -> std::collections::HashSet<PathBuf>`: the `log_path` of every record that has one.
  - `pub fn sweep_loose_logs(log_dir: &Path, recorded: &HashSet<PathBuf>, min_age: Duration, open: Option<&HashSet<PathBuf>>) -> usize`: removes each regular, non-symlink file directly in `log_dir` named `codescout-bg-*.log` that is not in `recorded`, is at least `min_age` old (mtime; future counts as zero), and is not in `open`. With `open == None` the caller passes the larger age; the function itself only compares ages.

- [ ] **Step 1: Write the failing tests** in `job_record.rs` tests (temp dirs only): `sweep_loose_logs_removes_an_old_log_nobody_names`, `..._keeps_a_young_log`, `..._keeps_a_log_a_record_names`, `..._keeps_a_log_a_process_holds_open` (open a `File` in the test, build the `open` set with the path, assert kept; Linux only: `#[cfg(target_os = "linux")]` and a second test that `open_file_paths()` contains the path of a file the test holds open), `..._ignores_other_names_symlinks_and_directories`, `recorded_log_paths_collects_every_named_log_and_skips_garbage`.
- [ ] **Step 2: Run to see them fail.** Expected: compile errors.
- [ ] **Step 3: Implement** the three items. Keep `open_file_paths` free of `unsafe`; use `std::fs::read_dir("/proc")`, filter all-digit names, `read_dir(format!("/proc/{pid}/fd"))`, `read_link` each. Non-Linux: a `#[cfg(not(target_os = "linux"))]` version returning `None`.
- [ ] **Step 4: Run again.** Expected: all listed tests pass.
- [ ] **Step 5: Commit** `src/platform/mod.rs src/tools/job_record.rs`: `feat(run-command): reclaim the loose background logs that no record names (45b69c70b723a089)`.

### Task 4: Wire the housekeeping, prove test isolation, close the bug

**Files:**
- Modify: `src/tools/output_buffer.rs` (`OutputBuffer` struct near L489, `with_byte_limit` L534-569, `store_background` L1671-1677, tests), `docs/issues/2026-10-31-background-command-logs-are-never-reclaimed-when-the-server-ends.md` (through `doc(action="update")`)

**Interfaces:**
- Consumes: `sweep_stale`, `sweep_logs`, `recorded_log_paths`, `sweep_loose_logs`, `open_file_paths`, the constants (Tasks 1-3).
- Produces:
  - Field `log_dir: Option<PathBuf>` on `OutputBuffer`, set by `fn default_log_dir() -> Option<PathBuf>` (`None` when `cfg!(test)`, else `Some(std::env::temp_dir())`).
  - `#[cfg(test)] pub fn with_log_dir(mut self, dir: PathBuf) -> Self` for tests.
  - `fn run_housekeeping(record_dir: &Path, log_dir: Option<&Path>)`: in order: `sweep_stale(record_dir, log_dir_or_fallback, RECORD_MAX_AGE, pid_alive)`, `sweep_logs(...)`, then if `log_dir` is `Some`: the loose sweep with `recorded_log_paths(record_dir)`, `open_file_paths()`, and `min_age = LOG_MAX_AGE` when the open set is `Some`, else `RECORD_MAX_AGE`. When `log_dir` is `None`, `sweep_stale` and `sweep_logs` still run, using `std::env::temp_dir()` as the guard directory.
  - `store_background`'s `record_sweep.call_once` closure calls `run_housekeeping` inline when `cfg!(test)`, otherwise on a detached `std::thread::Builder::new().name("bg-housekeeping".into())` thread (clone the two paths into it; ignore a failed spawn).

- [ ] **Step 1: Write the failing tests** in `output_buffer.rs` tests: `housekeeping_removes_an_old_loose_log_and_keeps_a_young_one` (calls `run_housekeeping` directly on temp dirs), `housekeeping_keeps_a_running_jobs_logs`, `a_test_built_output_buffer_never_scans_the_real_temp_dir` (plant an old `codescout-bg-reaper-isolation-<uuid>.log` in `std::env::temp_dir()`, set its mtime to 30 days ago, run a `store_background` on a default test `OutputBuffer`, assert the planted file still exists, then delete it), and `store_background_runs_the_housekeeping_once` using `with_log_dir` on a temp dir with an old loose log (assert it is gone after the first `store_background`).
- [ ] **Step 2: Run to see them fail.** Expected: compile errors for `run_housekeeping`, `with_log_dir`.
- [ ] **Step 3: Implement** the field, constructor default, `with_log_dir`, `run_housekeeping`, and the `call_once` change.
- [ ] **Step 4: Run the focused tests:** `CARGO_BUILD_JOBS=16 scripts/with-slot.sh cargo test --lib -- tools::job_record tools::output_buffer tools::run_command`. Expected: all pass. (The `run_command` filter covers the background-job tests that call `store_background`.)
- [ ] **Step 5: Run the cap-class gate** as in Task 2 Step 5, and `CARGO_BUILD_JOBS=16 scripts/with-slot.sh cargo clippy --lib --features local-embed -- -D warnings`. Expected: clippy clean; the cap-class result as described there.
- [ ] **Step 6: Commit** `src/tools/output_buffer.rs`: `feat(run-command): reap background logs at the first background spawn of each server (45b69c70b723a089)`.
- [ ] **Step 7: Update the bug record** through `doc(action="update", id="45b69c70b723a089", patch={status: "fixed", ...})` with the fix SHAs (labelled `experiments`) and the patch-ids (`git show <sha> | git patch-id --stable`), the focused test names, and a line saying that the reclaimed amount was NOT measured on this machine (state the expected 448 MiB as an estimate). Do not archive it until the gate has run.

---

## Self-review

**Spec coverage.** Design §1 → Task 1. §2 constants → Task 2. §3 record-linked sweep → Task 2. §4 legacy sweep → Task 3. §5 where it runs → Task 4. Testing section → tests listed in each task. Non-goals: no prompt-surface change (Global Constraints), no new tool, no wake-flow change (Review Focus 4). Anti-goal "silencing the plugin" → Global Constraints and Task 1 tests.

**Type consistency.** `sweep_stale(record_dir, log_dir, max_age, is_alive)`; `sweep_logs(record_dir, log_dir, grace, max_age, is_alive)`; `sweep_loose_logs(log_dir, recorded, min_age, open)`; `run_housekeeping(record_dir, log_dir: Option<&Path>)`. `unlink_log_if_ours(log_dir, log_path)` is used by `sweep_stale` and `sweep_logs` only.

**Proportion.** About 4 tasks and 25 named tests against a design of 94 lines; each step names a signature or a test, none carries a body.
