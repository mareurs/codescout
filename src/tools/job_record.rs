//! The completion record of a background `run_command` job.
//!
//! A background job's outcome lives in the server's memory (`JobState`), where nothing
//! outside the server process can wait on it. This module writes the same outcome to a
//! small JSON file so a watcher that does not share the process can observe completion
//! by reading a file. One record per job: `<server pid>-<token>-<handle>.json`.
//!
//! Records are written atomically (temp file in the same directory, then `rename`), so a
//! reader sees either no file or a complete one, never a half-written record. The
//! directory is owner-only (`0700`): a record carries a redacted command line, which is
//! still nobody else's business on a shared machine.
//!
//! Sidecars sit next to the record and are named by appending to its full path:
//! `<record>.watch` (a watcher is attached) and `<record>.delivered` (the watcher has
//! reported the outcome). Only `*.json` files are records; sidecars and temp files never
//! match that filter, so sweeping cannot mistake one for a record.
//!
//! docs/trackers/bug-fix-session-log.md F-201;
//! docs/superpowers/specs/2026-10-29-background-job-wake-design.md

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::tools::output_buffer::JobState;
use crate::util::redact::redact_credentials;

/// Version of the on-disk record format. A reader that sees a different value must not
/// guess at the fields.
pub const RECORD_SCHEMA: u32 = 1;

/// How long a finished record may sit before `sweep_stale` removes it.
/// cap-class: NOT_A_CAP — retention age for stale job records, not a bound on returned output.
pub const RECORD_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// How long a finished job's log outlives the moment its outcome was delivered (the
/// `.delivered` sidecar's mtime); the agent reads the log right after the wake.
/// cap-class: NOT_A_CAP — retention age for a delivered job's log file, not a bound on returned output.
pub const LOG_GRACE_AFTER_DELIVERY: Duration = Duration::from_secs(60 * 60);

/// How long a finished job's log may sit when its outcome was never delivered or read
/// (measured from the record file's mtime, which is the moment the job ended).
/// cap-class: NOT_A_CAP — retention age for an undelivered job's log file, not a bound on returned output.
pub const LOG_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// Where records live. Test builds get a different directory so that the many existing
/// tests which spawn background jobs never write into the real one.
pub fn default_record_dir() -> PathBuf {
    let name = if cfg!(test) {
        "codescout-bg-test"
    } else {
        "codescout-bg"
    };
    std::env::temp_dir().join(name)
}

/// Lifecycle state as recorded on disk. Mirrors `JobState`, but is its own type: the
/// file format must not change because an in-memory enum gained a variant.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RecordState {
    Running,
    Exited,
    Failed,
}

/// One background job's record. `exit_code` is `None` for a job killed by a signal (no
/// code exists) and for a job still running; `state` is what tells those apart.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct JobRecord {
    pub schema: u32,
    pub handle: String,
    /// Credential-redacted: the record outlives the process that wrote it, so it must
    /// not carry a secret from the command line.
    pub command: String,
    pub server_pid: u32,
    pub job_pid: Option<u32>,
    pub state: RecordState,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    /// The file the job's output goes to. A record written before this field existed has
    /// none, and so does one for a job whose log is not known: absent, never `null`, so a
    /// record that names no log is byte-identical to an old one. `RECORD_SCHEMA` stays 1
    /// on purpose: the wake hook ignores any other schema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_path: Option<String>,
}

impl JobRecord {
    /// A record for a job that has just been spawned by this process.
    pub fn running(handle: &str, command: &str, job_pid: Option<u32>) -> Self {
        Self {
            schema: RECORD_SCHEMA,
            handle: handle.to_string(),
            command: redact_credentials(command).text.into_owned(),
            server_pid: std::process::id(),
            job_pid,
            state: RecordState::Running,
            exit_code: None,
            error: None,
            started_at: chrono::Utc::now().to_rfc3339(),
            finished_at: None,
            log_path: None,
        }
    }

    /// Name the file this job's output goes to. Kept apart from `running` so that
    /// constructor keeps its signature; a record built without this names no log.
    pub fn with_log_path(mut self, path: &Path) -> Self {
        self.log_path = Some(path.display().to_string());
        self
    }

    /// Fold an observed terminal `JobState` into the record. A `Running` state leaves
    /// the record untouched: finishing is the act of recording an outcome, and none
    /// has been observed.
    pub fn finish(&mut self, state: &JobState) {
        match state {
            JobState::Running => return,
            JobState::Exited { code } => {
                self.state = RecordState::Exited;
                self.exit_code = *code;
            }
            JobState::Failed { error } => {
                self.state = RecordState::Failed;
                self.error = Some(error.clone());
            }
        }
        self.finished_at = Some(chrono::Utc::now().to_rfc3339());
    }
}

/// `<dir>/<server pid>-<token>-<handle without '@'>.json`. The `@` is dropped because it
/// is meaningful to shells and to codescout's own buffer-ref syntax; the pid and token
/// keep records of two servers (or two runs of one pid) from colliding.
pub fn record_path(dir: &Path, token: &str, handle: &str) -> PathBuf {
    let handle = handle.strip_prefix('@').unwrap_or(handle);
    dir.join(format!("{}-{token}-{handle}.json", std::process::id()))
}

/// Append `suffix` to the full path (`a.json` -> `a.json.watch`), not to the stem.
fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

fn remove_with_sidecars(path: &Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(sidecar(path, ".delivered"));
    let _ = std::fs::remove_file(sidecar(path, ".watch"));
}

fn read_record(path: &Path) -> Option<JobRecord> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Write `record` to `path` atomically, creating the parent directory owner-only.
pub fn write_record(path: &Path, record: &JobRecord) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    create_private_dir(dir)?;
    // Unique per write: two threads of this server write the same record (spawn writes
    // the running one, the supervisor the finished one), and `create_new` below fails
    // on a name that already exists.
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = sidecar(path, &format!(".tmp-{}-{seq}", std::process::id()));
    let json = serde_json::to_vec_pretty(record).map_err(std::io::Error::other)?;
    let result = (|| {
        // O_EXCL: never follow a symlink planted at the temp name.
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        f.write_all(&json)?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(unix)]
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    // `create` succeeds on a directory that already exists, whatever its owner or mode.
    // The name is fixed and predictable in a shared temp dir, so another user may have
    // made it first: refuse anything we do not own or that others can touch.
    let meta = std::fs::symlink_metadata(dir)?;
    // SAFETY: geteuid has no preconditions and cannot fail.
    let euid = unsafe { libc::geteuid() };
    if !meta.is_dir() || meta.uid() != euid || meta.mode() & 0o077 != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!(
                "record directory {} is not an owner-only directory owned by this user",
                dir.display()
            ),
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)
}

/// Delete a record once its outcome has been delivered. Returns `true` only when the
/// record was removed. A record that is still `running`, or whose `.delivered` marker
/// is absent, is kept: deleting it would lose an outcome nobody has seen yet.
pub fn remove_if_delivered(path: &Path) -> bool {
    let finished = matches!(read_record(path), Some(r) if r.state != RecordState::Running);
    if !finished || !sidecar(path, ".delivered").exists() {
        return false;
    }
    remove_with_sidecars(path);
    true
}

/// Remove records that can no longer matter and return how many were removed. A record
/// is stale when its file is at least `max_age` old (mtime) and either it is finished,
/// or it is `running` but its server is gone (`is_alive(server_pid)` is false) so
/// nothing will ever finish it. Unreadable or unparsable files are skipped, never an
/// error: this is housekeeping, and a file it cannot judge is not its to delete.
///
/// A record's log goes with it, through `unlink_log_if_ours` (so only a regular
/// `codescout-bg-*.log` file directly in `log_dir`), so a record is never removed while
/// its log stays behind with nothing left to name it.
pub fn sweep_stale(
    dir: &Path,
    log_dir: &Path,
    max_age: Duration,
    is_alive: impl Fn(u32) -> bool,
) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(age) = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .map(|t| t.elapsed().unwrap_or(Duration::ZERO))
        else {
            continue;
        };
        if age < max_age {
            continue;
        }
        let Some(rec) = read_record(&path) else {
            continue;
        };
        if rec.state == RecordState::Running && is_alive(rec.server_pid) {
            continue;
        }
        if let Some(log) = rec.log_path.as_deref() {
            unlink_log_if_ours(log_dir, log);
        }
        remove_with_sidecars(&path);
        removed += 1;
    }
    removed
}

/// Unlink the log a record names, but only when it is certainly one of ours: a regular
/// file (not a symlink, not a directory) whose parent is exactly `log_dir` and whose
/// file name starts with `codescout-bg-` and ends with `.log`. Returns whether a file
/// was removed. A record is data on disk another process could have written or damaged,
/// so its `log_path` is never trusted: anything that fails a check is left alone.
///
/// The parent is compared to `log_dir` LITERALLY, with no `canonicalize` on either side:
/// canonicalizing would follow the very symlinks this rule exists to refuse. A temp dir
/// reached through a symlink therefore makes the log be KEPT, which is the safe
/// direction. The parent check is also what makes the lossy `display()` string stored by
/// `JobRecord::with_log_path` harmless: a path that does not round-trip cannot equal
/// `log_dir`, so it is skipped rather than resolved to some other file.
fn unlink_log_if_ours(log_dir: &Path, log_path: &str) -> bool {
    let path = Path::new(log_path);
    if path.parent() != Some(log_dir) {
        return false;
    }
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    if !(name.starts_with("codescout-bg-") && name.ends_with(".log")) {
        return false;
    }
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_file() => {}
        _ => return false,
    }
    std::fs::remove_file(path).is_ok()
}

/// How long ago `path` was last modified. A modification time in the future (clock
/// skew) is age zero, never "very old"; `None` when the metadata cannot be read.
fn age_of(path: &Path) -> Option<Duration> {
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
    Some(modified.elapsed().unwrap_or(Duration::ZERO))
}

/// Unlink the logs of finished jobs and return how many were removed. The record stays
/// (`sweep_stale` owns it). For each finished record naming a log, the log goes when its
/// `.delivered` sidecar is at least `grace` old, or the record file is at least `max_age`
/// old. A missing or unreadable `.delivered` sidecar means "not delivered"; a record whose
/// metadata cannot be read is skipped.
///
/// A `running` record is never acted on, whatever `is_alive` says: with a live server the
/// job is writing its log, and with a dead server the job may be an orphan that still
/// writes, so that log waits for `RECORD_MAX_AGE`, where `sweep_stale` removes it with the
/// record. `_is_alive` is therefore not consulted; it stays in the signature so the two
/// sweeps are called alike.
pub fn sweep_logs(
    record_dir: &Path,
    log_dir: &Path,
    grace: Duration,
    max_age: Duration,
    _is_alive: impl Fn(u32) -> bool,
) -> usize {
    let Ok(entries) = std::fs::read_dir(record_dir) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(record_age) = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .map(|t| t.elapsed().unwrap_or(Duration::ZERO))
        else {
            continue;
        };
        let Some(rec) = read_record(&path) else {
            continue;
        };
        if rec.state == RecordState::Running {
            continue;
        }
        let Some(log) = rec.log_path.as_deref() else {
            continue;
        };
        let delivered_long_enough =
            age_of(&sidecar(&path, ".delivered")).is_some_and(|a| a >= grace);
        if (delivered_long_enough || record_age >= max_age) && unlink_log_if_ours(log_dir, log) {
            removed += 1;
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::output_buffer::JobState;
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    #[test]
    fn a_running_record_redacts_its_command_and_names_this_process() {
        let rec = JobRecord::running(
            "@bg_00000001",
            "TOKEN=ghp_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa make",
            Some(42),
        );
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
        assert_eq!(
            (r.state.clone(), r.exit_code),
            (RecordState::Exited, Some(3))
        );
        assert!(r.finished_at.is_some());
        let mut s = JobRecord::running("@bg_00000002", "true", None);
        s.finish(&JobState::Exited { code: None });
        assert_eq!((s.state.clone(), s.exit_code), (RecordState::Exited, None));
        let mut f = JobRecord::running("@bg_00000003", "true", None);
        f.finish(&JobState::Failed {
            error: "boom".into(),
        });
        assert_eq!(
            (f.state.clone(), f.error.as_deref()),
            (RecordState::Failed, Some("boom"))
        );
    }

    #[test]
    fn record_path_strips_the_at_sign_and_carries_pid_and_token() {
        let p = record_path(Path::new("/d"), "9f3a1c07", "@bg_00000001");
        assert_eq!(
            p,
            PathBuf::from(format!(
                "/d/{}-9f3a1c07-bg_00000001.json",
                std::process::id()
            ))
        );
    }

    #[test]
    fn write_record_round_trips_and_leaves_no_temp_file() {
        let dir = private_tempdir();
        let path = record_path(dir.path(), "00000000", "@bg_00000001");
        let rec = JobRecord::running("@bg_00000001", "true", Some(1));
        write_record(&path, &rec).unwrap();
        let back: JobRecord =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back, rec);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    /// The ten fields an old (pre-`log_path`) writer produced. A record written before the
    /// log was named must still parse: the plugin and the sweep read records of any age.
    #[test]
    fn a_record_without_log_path_still_parses() {
        let old = r#"{
            "schema": 1,
            "handle": "@bg_00000001",
            "command": "true",
            "server_pid": 4242,
            "job_pid": 4243,
            "state": "running",
            "exit_code": null,
            "error": null,
            "started_at": "2026-10-29T00:00:00+00:00",
            "finished_at": null
        }"#;
        let rec: JobRecord = serde_json::from_str(old).unwrap();
        assert_eq!(rec.log_path, None);
        assert_eq!(rec.handle, "@bg_00000001");
    }

    /// The wake hook ignores any record whose `schema` is not 1, so naming the log must not
    /// bump it: a bump would silence the wake for every new job.
    #[test]
    fn a_record_with_log_path_round_trips_and_stays_schema_1() {
        let dir = private_tempdir();
        let path = record_path(dir.path(), "00000000", "@bg_00000001");
        let rec = JobRecord::running("@bg_00000001", "true", Some(1))
            .with_log_path(Path::new("/x/codescout-bg-abc.log"));
        write_record(&path, &rec).unwrap();
        let back = read_record(&path).expect("record reads back");
        assert_eq!(back.log_path.as_deref(), Some("/x/codescout-bg-abc.log"));
        assert_eq!(back.schema, 1);
        assert_eq!(back, rec);
    }

    /// A record that names no log writes no `log_path` key at all (not `null`), so its bytes
    /// are what an old writer would have produced.
    #[test]
    fn a_record_without_log_path_serialises_no_log_path_key() {
        let rec = JobRecord::running("@bg_00000001", "true", None);
        assert_eq!(rec.log_path, None);
        let json = serde_json::to_string(&rec).unwrap();
        assert!(!json.contains("log_path"), "{json}");
    }

    #[cfg(unix)]
    #[test]
    fn write_record_creates_its_directory_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let base = tempfile::tempdir().unwrap();
        let dir = base.path().join("codescout-bg");
        write_record(
            &record_path(&dir, "00000000", "@bg_00000001"),
            &JobRecord::running("@bg_00000001", "true", None),
        )
        .unwrap();
        assert_eq!(
            std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }

    /// A temp dir the record-directory check accepts: `tempdir()` follows the umask (0755
    /// here), and `write_record` refuses a directory other users can reach.
    fn private_tempdir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        dir
    }

    fn sidecar(path: &Path, ext: &str) -> PathBuf {
        let mut s = path.as_os_str().to_owned();
        s.push(ext);
        PathBuf::from(s)
    }

    /// Write a record in `state` plus both sidecars; returns the record path.
    fn seed(dir: &Path, token: &str, handle: &str, state: RecordState, delivered: bool) -> PathBuf {
        let path = record_path(dir, token, handle);
        let mut rec = JobRecord::running(handle, "true", None);
        rec.state = state;
        write_record(&path, &rec).unwrap();
        std::fs::write(sidecar(&path, ".watch"), b"").unwrap();
        if delivered {
            std::fs::write(sidecar(&path, ".delivered"), b"").unwrap();
        }
        path
    }

    #[test]
    fn remove_if_delivered_keeps_undelivered_and_running_records() {
        let dir = private_tempdir();
        // running + .delivered -> kept
        let running = seed(
            dir.path(),
            "00000001",
            "@bg_00000001",
            RecordState::Running,
            true,
        );
        assert!(!remove_if_delivered(&running));
        assert!(running.exists() && sidecar(&running, ".delivered").exists());
        // exited without .delivered -> kept
        let undelivered = seed(
            dir.path(),
            "00000002",
            "@bg_00000002",
            RecordState::Exited,
            false,
        );
        assert!(!remove_if_delivered(&undelivered));
        assert!(undelivered.exists() && sidecar(&undelivered, ".watch").exists());
        // exited + .delivered -> record, .delivered and .watch all removed, returns true
        let done = seed(
            dir.path(),
            "00000003",
            "@bg_00000003",
            RecordState::Exited,
            true,
        );
        assert!(remove_if_delivered(&done));
        assert!(!done.exists());
        assert!(!sidecar(&done, ".delivered").exists());
        assert!(!sidecar(&done, ".watch").exists());
    }

    #[test]
    fn sweep_keeps_young_records_and_live_running_ones() {
        let dir = private_tempdir();
        let exited = seed(
            dir.path(),
            "00000001",
            "@bg_00000001",
            RecordState::Exited,
            false,
        );
        let live = seed(
            dir.path(),
            "00000002",
            "@bg_00000002",
            RecordState::Running,
            false,
        );
        let dead = seed(
            dir.path(),
            "00000003",
            "@bg_00000003",
            RecordState::Running,
            false,
        );
        // Distinguish live from dead by server_pid: rewrite the dead record with pid 7.
        let mut rec: JobRecord =
            serde_json::from_str(&std::fs::read_to_string(&dead).unwrap()).unwrap();
        rec.server_pid = 7;
        write_record(&dead, &rec).unwrap();
        let me = std::process::id();

        // A real max_age keeps every young record, whatever its state.
        assert_eq!(
            sweep_stale(dir.path(), dir.path(), RECORD_MAX_AGE, |_| false),
            0
        );
        assert!(exited.exists() && live.exists() && dead.exists());

        // max_age = 0: exited removed; running with a live pid kept; dead pid removed.
        let n = sweep_stale(dir.path(), dir.path(), Duration::ZERO, |pid| pid == me);
        assert_eq!(n, 2);
        assert!(!exited.exists() && !sidecar(&exited, ".watch").exists());
        assert!(live.exists());
        assert!(!dead.exists() && !sidecar(&dead, ".watch").exists());
    }
    #[cfg(unix)]
    #[test]
    fn write_record_refuses_a_pre_existing_directory_others_can_reach() {
        use std::os::unix::fs::PermissionsExt;
        for mode in [0o755, 0o777] {
            let base = tempfile::tempdir().unwrap();
            let dir = base.path().join("codescout-bg");
            std::fs::create_dir(&dir).unwrap();
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(mode)).unwrap();
            let path = record_path(&dir, "00000000", "@bg_00000001");
            let err = write_record(&path, &JobRecord::running("@bg_00000001", "true", None))
                .expect_err("a directory open to others must be refused");
            assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied, "{err}");
            assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "mode {mode:o}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn write_record_does_not_follow_a_symlink_at_the_temp_name() {
        let dir = tempfile::tempdir().unwrap();
        let victim = dir.path().join("victim");
        std::fs::write(&victim, "precious").unwrap();
        let sub = dir.path().join("bg");
        let path = record_path(&sub, "00000000", "@bg_00000001");
        let rec = JobRecord::running("@bg_00000001", "true", None);
        write_record(&path, &rec).unwrap(); // creates `sub` 0700
                                            // Plant a symlink at every temp name the next writes could pick.
                                            // (The counter is process-wide, so plant far past anything the suite reaches.)
        for seq in 0..5000 {
            let tmp = sidecar(&path, &format!(".tmp-{}-{seq}", std::process::id()));
            let _ = std::os::unix::fs::symlink(&victim, tmp);
        }
        assert!(
            write_record(&path, &rec).is_err(),
            "O_EXCL must hit the planted name"
        );
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "precious");
    }

    #[test]
    fn concurrent_writers_of_one_record_both_succeed_and_leave_no_temp_file() {
        let dir = private_tempdir();
        let path = record_path(dir.path(), "00000000", "@bg_00000001");
        std::thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| {
                    for _ in 0..50 {
                        let rec = JobRecord::running("@bg_00000001", "true", None);
                        write_record(&path, &rec).unwrap();
                    }
                });
            }
        });
        let back: JobRecord =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back.handle, "@bg_00000001");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn sweep_skips_what_it_cannot_judge_and_a_missing_directory_is_zero() {
        let dir = tempfile::tempdir().unwrap();
        let garbage = dir.path().join("x.json");
        std::fs::write(&garbage, "not json {").unwrap();
        let json_dir = dir.path().join("dir.json");
        std::fs::create_dir(&json_dir).unwrap();
        assert_eq!(
            sweep_stale(dir.path(), dir.path(), Duration::ZERO, |_| false),
            0
        );
        assert!(garbage.exists() && json_dir.exists());
        assert_eq!(
            sweep_stale(
                &dir.path().join("absent"),
                dir.path(),
                Duration::ZERO,
                |_| false
            ),
            0
        );
    }

    #[test]
    fn remove_if_delivered_is_false_for_a_missing_or_malformed_record() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("none.json");
        std::fs::write(sidecar(&missing, ".delivered"), b"").unwrap();
        assert!(!remove_if_delivered(&missing));
        let bad = dir.path().join("bad.json");
        std::fs::write(&bad, "garbage").unwrap();
        std::fs::write(sidecar(&bad, ".delivered"), b"").unwrap();
        assert!(!remove_if_delivered(&bad));
        assert!(bad.exists() && sidecar(&bad, ".delivered").exists());
    }
    /// Like `seed`, plus a `codescout-bg-<handle>.log` in `log_dir` that the record names.
    /// Returns (record path, log path).
    fn seed_with_log(
        dir: &Path,
        log_dir: &Path,
        token: &str,
        handle: &str,
        state: RecordState,
        delivered: bool,
    ) -> (PathBuf, PathBuf) {
        let log = log_dir.join(format!(
            "codescout-bg-{}.log",
            handle.trim_start_matches('@')
        ));
        std::fs::write(&log, b"output").unwrap();
        let path = record_path(dir, token, handle);
        let mut rec = JobRecord::running(handle, "true", None).with_log_path(&log);
        rec.state = state;
        write_record(&path, &rec).unwrap();
        std::fs::write(sidecar(&path, ".watch"), b"").unwrap();
        if delivered {
            std::fs::write(sidecar(&path, ".delivered"), b"").unwrap();
        }
        (path, log)
    }

    fn set_mtime(path: &Path, when: std::time::SystemTime) {
        std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(when)
            .unwrap();
    }

    fn ago(d: Duration) -> std::time::SystemTime {
        std::time::SystemTime::now() - d
    }

    const HOUR: Duration = Duration::from_secs(60 * 60);

    #[test]
    fn sweep_logs_removes_the_log_one_grace_after_delivery_and_keeps_the_record() {
        let dir = private_tempdir();
        let logs = tempfile::tempdir().unwrap();
        let (rec, log) = seed_with_log(
            dir.path(),
            logs.path(),
            "00000001",
            "@bg_00000001",
            RecordState::Exited,
            true,
        );
        set_mtime(&sidecar(&rec, ".delivered"), ago(2 * HOUR));
        let n = sweep_logs(
            dir.path(),
            logs.path(),
            LOG_GRACE_AFTER_DELIVERY,
            LOG_MAX_AGE,
            |_| false,
        );
        assert_eq!(n, 1);
        assert!(!log.exists());
        assert!(rec.exists());
        assert!(sidecar(&rec, ".delivered").exists() && sidecar(&rec, ".watch").exists());
    }

    #[test]
    fn sweep_logs_keeps_a_delivered_log_inside_the_grace() {
        let dir = private_tempdir();
        let logs = tempfile::tempdir().unwrap();
        let (rec, log) = seed_with_log(
            dir.path(),
            logs.path(),
            "00000001",
            "@bg_00000001",
            RecordState::Exited,
            true,
        );
        set_mtime(&sidecar(&rec, ".delivered"), ago(HOUR / 6));
        let n = sweep_logs(
            dir.path(),
            logs.path(),
            LOG_GRACE_AFTER_DELIVERY,
            LOG_MAX_AGE,
            |_| false,
        );
        assert_eq!(n, 0);
        assert!(log.exists() && rec.exists());
    }

    #[test]
    fn sweep_logs_removes_an_undelivered_finished_log_after_max_age_and_keeps_it_before() {
        let dir = private_tempdir();
        let logs = tempfile::tempdir().unwrap();
        let (old_rec, old_log) = seed_with_log(
            dir.path(),
            logs.path(),
            "00000001",
            "@bg_00000001",
            RecordState::Exited,
            false,
        );
        set_mtime(&old_rec, ago(LOG_MAX_AGE + HOUR));
        let (young_rec, young_log) = seed_with_log(
            dir.path(),
            logs.path(),
            "00000002",
            "@bg_00000002",
            RecordState::Failed,
            false,
        );
        set_mtime(&young_rec, ago(LOG_MAX_AGE - HOUR));
        let n = sweep_logs(
            dir.path(),
            logs.path(),
            LOG_GRACE_AFTER_DELIVERY,
            LOG_MAX_AGE,
            |_| false,
        );
        assert_eq!(n, 1);
        assert!(!old_log.exists() && old_rec.exists());
        assert!(young_log.exists() && young_rec.exists());
    }

    #[test]
    fn sweep_logs_never_touches_a_running_jobs_log_with_a_live_server() {
        let dir = private_tempdir();
        let logs = tempfile::tempdir().unwrap();
        // Old record and old .delivered: both finished-record rules would fire if the
        // state were not consulted.
        let (rec, log) = seed_with_log(
            dir.path(),
            logs.path(),
            "00000001",
            "@bg_00000001",
            RecordState::Running,
            true,
        );
        set_mtime(&rec, ago(10 * LOG_MAX_AGE));
        set_mtime(&sidecar(&rec, ".delivered"), ago(10 * LOG_MAX_AGE));
        let me = std::process::id();
        let n = sweep_logs(
            dir.path(),
            logs.path(),
            Duration::ZERO,
            Duration::ZERO,
            |pid| pid == me,
        );
        assert_eq!(n, 0);
        assert!(log.exists() && rec.exists());
    }

    #[test]
    fn sweep_logs_leaves_a_dead_servers_running_log_to_the_record_rule() {
        let dir = private_tempdir();
        let logs = tempfile::tempdir().unwrap();
        let (rec, log) = seed_with_log(
            dir.path(),
            logs.path(),
            "00000001",
            "@bg_00000001",
            RecordState::Running,
            true,
        );
        let mut r: JobRecord =
            serde_json::from_str(&std::fs::read_to_string(&rec).unwrap()).unwrap();
        r.server_pid = 7;
        write_record(&rec, &r).unwrap();
        set_mtime(&rec, ago(2 * LOG_MAX_AGE));
        set_mtime(&sidecar(&rec, ".delivered"), ago(2 * LOG_MAX_AGE));
        // sweep_logs leaves it...
        let n = sweep_logs(
            dir.path(),
            logs.path(),
            LOG_GRACE_AFTER_DELIVERY,
            LOG_MAX_AGE,
            |_| false,
        );
        assert_eq!(n, 0);
        assert!(log.exists() && rec.exists());
        // ...and sweep_stale before RECORD_MAX_AGE leaves it too...
        assert_eq!(
            sweep_stale(dir.path(), logs.path(), RECORD_MAX_AGE, |_| false),
            0
        );
        assert!(log.exists() && rec.exists());
        // ...until the record itself goes, and then the log goes with it.
        set_mtime(&rec, ago(RECORD_MAX_AGE + HOUR));
        assert_eq!(
            sweep_stale(dir.path(), logs.path(), RECORD_MAX_AGE, |_| false),
            1
        );
        assert!(!log.exists() && !rec.exists());
    }

    #[cfg(unix)]
    #[test]
    fn sweep_logs_skips_a_log_path_outside_the_log_dir_a_symlink_and_a_directory() {
        let dir = private_tempdir();
        let logs = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let old = ago(2 * LOG_MAX_AGE);
        let finished_naming = |token: &str, handle: &str, named: &Path| -> PathBuf {
            let path = record_path(dir.path(), token, handle);
            let mut rec = JobRecord::running(handle, "true", None).with_log_path(named);
            rec.state = RecordState::Exited;
            write_record(&path, &rec).unwrap();
            set_mtime(&path, old);
            path
        };

        // 1. a well-named file, but in a different directory
        let elsewhere = outside.path().join("codescout-bg-out.log");
        std::fs::write(&elsewhere, b"x").unwrap();
        finished_naming("00000001", "@bg_00000001", &elsewhere);

        // 2. a symlink inside log_dir whose target is a file in the other directory
        let target = outside.path().join("target.txt");
        std::fs::write(&target, b"precious").unwrap();
        let link = logs.path().join("codescout-bg-link.log");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        finished_naming("00000002", "@bg_00000002", &link);

        // 3. a directory with a log-like name
        let a_dir = logs.path().join("codescout-bg-x.log");
        std::fs::create_dir(&a_dir).unwrap();
        finished_naming("00000003", "@bg_00000003", &a_dir);

        // 4. a regular file in log_dir with the wrong name
        let wrong = logs.path().join("notes.log");
        std::fs::write(&wrong, b"x").unwrap();
        finished_naming("00000004", "@bg_00000004", &wrong);

        let n = sweep_logs(
            dir.path(),
            logs.path(),
            LOG_GRACE_AFTER_DELIVERY,
            LOG_MAX_AGE,
            |_| false,
        );
        assert_eq!(n, 0);
        assert!(elsewhere.exists(), "a file outside log_dir was removed");
        assert!(link.symlink_metadata().is_ok(), "the symlink was removed");
        assert!(target.exists(), "the symlink target was removed");
        assert!(a_dir.is_dir(), "the directory was removed");
        assert!(wrong.exists(), "a wrongly named file was removed");
    }

    #[test]
    fn sweep_logs_treats_a_future_mtime_as_age_zero() {
        let dir = private_tempdir();
        let logs = tempfile::tempdir().unwrap();
        let future = std::time::SystemTime::now() + 3 * LOG_MAX_AGE;
        // undelivered: the record's mtime is in the future
        let (rec_a, log_a) = seed_with_log(
            dir.path(),
            logs.path(),
            "00000001",
            "@bg_00000001",
            RecordState::Exited,
            false,
        );
        set_mtime(&rec_a, future);
        // delivered: the .delivered mtime is in the future
        let (rec_b, log_b) = seed_with_log(
            dir.path(),
            logs.path(),
            "00000002",
            "@bg_00000002",
            RecordState::Exited,
            true,
        );
        set_mtime(&sidecar(&rec_b, ".delivered"), future);
        let n = sweep_logs(
            dir.path(),
            logs.path(),
            LOG_GRACE_AFTER_DELIVERY,
            LOG_MAX_AGE,
            |_| false,
        );
        assert_eq!(n, 0);
        assert!(log_a.exists() && log_b.exists());
    }

    #[test]
    fn sweep_logs_treats_a_missing_delivered_sidecar_as_not_delivered() {
        let dir = private_tempdir();
        let logs = tempfile::tempdir().unwrap();
        let (rec, log) = seed_with_log(
            dir.path(),
            logs.path(),
            "00000001",
            "@bg_00000001",
            RecordState::Exited,
            false,
        );
        assert!(!sidecar(&rec, ".delivered").exists());
        // A zero grace would remove a log whose delivery age was taken as zero.
        let n = sweep_logs(dir.path(), logs.path(), Duration::ZERO, LOG_MAX_AGE, |_| {
            false
        });
        assert_eq!(n, 0);
        assert!(log.exists() && rec.exists());
    }

    #[test]
    fn sweep_stale_removes_the_log_with_its_record_and_survives_a_record_whose_log_is_gone() {
        let dir = private_tempdir();
        let logs = tempfile::tempdir().unwrap();
        let (rec_a, log_a) = seed_with_log(
            dir.path(),
            logs.path(),
            "00000001",
            "@bg_00000001",
            RecordState::Exited,
            true,
        );
        let (rec_b, log_b) = seed_with_log(
            dir.path(),
            logs.path(),
            "00000002",
            "@bg_00000002",
            RecordState::Exited,
            false,
        );
        std::fs::remove_file(&log_b).unwrap();
        let n = sweep_stale(dir.path(), logs.path(), Duration::ZERO, |_| false);
        assert_eq!(n, 2);
        assert!(!rec_a.exists() && !log_a.exists());
        assert!(!rec_b.exists());
    }
}
