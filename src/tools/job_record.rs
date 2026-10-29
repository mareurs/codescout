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
pub const RECORD_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

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
        }
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
    let tmp = sidecar(path, &format!(".tmp-{}", std::process::id()));
    let json = serde_json::to_vec_pretty(record).map_err(std::io::Error::other)?;
    let result = (|| {
        let mut f = std::fs::File::create(&tmp)?;
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
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
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
pub fn sweep_stale(dir: &Path, max_age: Duration, is_alive: impl Fn(u32) -> bool) -> usize {
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
        remove_with_sidecars(&path);
        removed += 1;
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
        let dir = tempfile::tempdir().unwrap();
        let path = record_path(dir.path(), "00000000", "@bg_00000001");
        let rec = JobRecord::running("@bg_00000001", "true", Some(1));
        write_record(&path, &rec).unwrap();
        let back: JobRecord =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back, rec);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
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
        let dir = tempfile::tempdir().unwrap();
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
        let dir = tempfile::tempdir().unwrap();
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
        assert_eq!(sweep_stale(dir.path(), RECORD_MAX_AGE, |_| false), 0);
        assert!(exited.exists() && live.exists() && dead.exists());

        // max_age = 0: exited removed; running with a live pid kept; dead pid removed.
        let n = sweep_stale(dir.path(), Duration::ZERO, |pid| pid == me);
        assert_eq!(n, 2);
        assert!(!exited.exists() && !sidecar(&exited, ".watch").exists());
        assert!(live.exists());
        assert!(!dead.exists() && !sidecar(&dead, ".watch").exists());
    }
}
