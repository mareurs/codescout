//! The pid-keyed handshake that lets a companion hook push a fresh conversation
//! id into an already-running server.
//!
//! MCP `initialize` runs before `SessionStart`, so the hook cannot mint an id the
//! server reads at startup: the server publishes a slot, the hook writes into it.
//! Keyed by server pid because a per-project file collides between two windows on
//! one repo — the 2026-08-16 attribution bug. A pid is a valid rendezvous within
//! one process lifetime even though it is useless as durable identity.
//!
//! Published ONLY from `CodeScoutServer::from_parts_with_env`. `codescout mux`
//! processes are children of a codescout, never of the harness, and never serve
//! guides — publishing from any shared init would mint entries no hook can match.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// One server's slot. `session` is what the server reads back; `hook_at` is how
/// it knows a hook — rather than only itself — has written here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub pid: u32,
    pub ppid: u32,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub cwd: String,
    pub session: Option<String>,
    /// Set by the companion hook. `None` ⇒ no rendezvous is active.
    pub hook_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub struct Rendezvous {
    path: Option<PathBuf>,
}

impl Rendezvous {
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Write this process's slot, and collect slots whose process is gone.
    /// Best-effort throughout: a failure costs the `/clear` refresh, never
    /// correctness — the idle TTL and the next restart both still work.
    pub fn publish(dir: Option<PathBuf>, session: Option<&str>) -> Self {
        let Some(dir) = dir else {
            return Self { path: None };
        };
        if std::fs::create_dir_all(&dir).is_err() {
            return Self { path: None };
        }
        gc(&dir);
        let pid = std::process::id();
        let entry = Entry {
            pid,
            ppid: parent_pid(),
            started_at: chrono::Utc::now(),
            cwd: std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            session: session.map(str::to_string),
            hook_at: None,
        };
        let path = dir.join(format!("{pid}.json"));
        match serde_json::to_string(&entry) {
            Ok(json) => match crate::util::fs::write_utf8(&path, &json) {
                Ok(()) => Self { path: Some(path) },
                Err(e) => {
                    tracing::debug!("rendezvous publish failed ({}): {e}", path.display());
                    Self { path: None }
                }
            },
            Err(e) => {
                tracing::debug!("rendezvous serialize failed: {e}");
                Self { path: None }
            }
        }
    }
}

#[cfg(unix)]
fn parent_pid() -> u32 {
    // SAFETY: getppid takes no arguments and cannot fail.
    unsafe { libc::getppid() as u32 }
}

#[cfg(windows)]
fn parent_pid() -> u32 {
    // No getppid here. The hook walks ancestry itself, so a zero degrades to
    // "never matched" rather than to a WRONG match — the safe direction.
    0
}

/// Remove slots whose process is gone. Marked cleanup, not discovery — see
/// `src/lsp/mux/mod.rs:68-72` (R-45) for why that distinction is written down.
fn gc(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(pid) = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        if pid == 0 {
            // A stray `0.json` can only be garbage: `std::process::id()` is
            // never 0, and `process_alive(0)` is unconditionally true on unix
            // because `kill(0, 0)` addresses the caller's process GROUP, not
            // process number zero — so the liveness check below would never
            // collect it. Collect it here explicitly rather than letting it
            // live forever.
            let _ = std::fs::remove_file(&path);
            continue;
        }
        if !crate::platform::process_alive(pid) {
            let _ = std::fs::remove_file(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry_at(dir: &std::path::Path, pid: u32) -> Option<Entry> {
        let text = std::fs::read_to_string(dir.join(format!("{pid}.json"))).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// A pid that is definitely not running. Pid 0 does NOT work: `kill(0, 0)`
    /// targets the caller's process GROUP and succeeds, so `process_alive(0)`
    /// is true on unix (measured 2026-08-18: `kill -0 0` exits 0).
    fn a_dead_pid() -> u32 {
        #[cfg(windows)]
        let mut child = std::process::Command::new("cmd")
            .args(["/C", "exit"])
            .spawn()
            .unwrap();
        #[cfg(unix)]
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        pid
    }

    #[test]
    fn publish_writes_an_entry_named_for_this_process() {
        let dir = tempfile::tempdir().unwrap();
        let r = Rendezvous::publish(Some(dir.path().to_path_buf()), Some("sess-1"));
        let me = std::process::id();
        let e = entry_at(dir.path(), me).expect("entry must exist under our own pid");
        assert_eq!(e.pid, me);
        assert_eq!(e.session.as_deref(), Some("sess-1"));
        assert!(
            e.hook_at.is_none(),
            "a freshly published entry is unstamped"
        );
        assert!(r.path().is_some());
    }

    #[test]
    fn publish_records_the_parent_pid_the_hook_matches_on() {
        // The hook selects entries whose ppid is on its own ancestry. A zero or
        // missing ppid makes every entry unmatchable — the feature silently dies.
        let dir = tempfile::tempdir().unwrap();
        Rendezvous::publish(Some(dir.path().to_path_buf()), None);
        let e = entry_at(dir.path(), std::process::id()).unwrap();
        assert_ne!(e.ppid, 0, "ppid must be recorded");
    }

    #[test]
    fn publish_with_no_directory_is_inert_and_never_panics() {
        let r = Rendezvous::publish(None, Some("sess-1"));
        assert!(r.path().is_none());
    }
    /// M10: both filesystem-failure branches in `publish` (the `create_dir_all`
    /// early return and the `write_utf8` match arm) were unguarded — flipping
    /// either to `Some(path)` kept the suite green. `path()` is exactly what
    /// Task 5's read side consumes, so a bogus `Some` is a lie: the read side
    /// would stat and parse a file that was never written.
    ///
    /// Two independent scenarios, one per branch, both proven with a real
    /// filesystem operation (not assumed): root and some exotic filesystems
    /// ignore mode bits, so each scenario checks its own precondition and
    /// degrades to a no-op rather than asserting on an untriggered condition.
    #[cfg(unix)]
    #[test]
    fn publish_degrades_to_none_on_filesystem_failure() {
        use std::os::unix::fs::PermissionsExt;

        // Branch A (create_dir_all fails): `dir` does not exist yet and its
        // parent is unwritable, so creating it fails outright.
        let parent = tempfile::tempdir().unwrap();
        std::fs::set_permissions(parent.path(), std::fs::Permissions::from_mode(0o500)).unwrap();
        let candidate = parent.path().join("servers");
        if std::fs::create_dir(&candidate).is_ok() {
            let _ = std::fs::set_permissions(parent.path(), std::fs::Permissions::from_mode(0o700));
            return;
        }
        let r1 = Rendezvous::publish(Some(candidate), Some("sess"));
        assert!(
            r1.path().is_none(),
            "a create_dir_all failure must degrade to None, never claim a path \
             that was never written"
        );
        let _ = std::fs::set_permissions(parent.path(), std::fs::Permissions::from_mode(0o700));

        // Branch B (write_utf8 fails): `dir` already exists — create_dir_all
        // succeeds trivially — but the directory itself is unwritable, so
        // writing the entry file into it fails.
        let existing = tempfile::tempdir().unwrap();
        std::fs::set_permissions(existing.path(), std::fs::Permissions::from_mode(0o500)).unwrap();
        let probe = existing.path().join("probe");
        if std::fs::write(&probe, "x").is_ok() {
            let _ = std::fs::remove_file(&probe);
            let _ =
                std::fs::set_permissions(existing.path(), std::fs::Permissions::from_mode(0o700));
            return;
        }
        let r2 = Rendezvous::publish(Some(existing.path().to_path_buf()), Some("sess"));
        assert!(
            r2.path().is_none(),
            "a write failure inside an existing-but-unwritable directory must \
             degrade to None, never claim a path that was never written"
        );
        let _ = std::fs::set_permissions(existing.path(), std::fs::Permissions::from_mode(0o700));
    }

    #[test]
    fn publish_collects_entries_whose_process_is_gone() {
        let dir = tempfile::tempdir().unwrap();
        let dead = a_dead_pid();
        std::fs::write(
            dir.path().join(format!("{dead}.json")),
            format!(
                r#"{{"pid":{dead},"ppid":1,"started_at":"2026-01-01T00:00:00Z","cwd":"/","session":null,"hook_at":null}}"#
            ),
        )
        .unwrap();
        Rendezvous::publish(Some(dir.path().to_path_buf()), None);
        assert!(
            !dir.path().join(format!("{dead}.json")).exists(),
            "a dead pid's entry must be collected"
        );
    }
    #[test]
    fn publish_collects_a_stray_pid_zero_file() {
        // `process_alive(0)` is unconditionally true (POSIX `kill(0, 0)`
        // addresses the caller's process GROUP, not process number zero), so
        // pid 0 needs its own explicit collection path in `gc` rather than
        // falling through the liveness check, which would never fire for it.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("0.json"),
            r#"{"pid":0,"ppid":1,"started_at":"2026-01-01T00:00:00Z","cwd":"/","session":null,"hook_at":null}"#,
        )
        .unwrap();
        Rendezvous::publish(Some(dir.path().to_path_buf()), None);
        assert!(
            !dir.path().join("0.json").exists(),
            "a stray 0.json must be collected, not left immortal"
        );
    }

    #[test]
    fn publish_keeps_the_entry_of_a_live_process() {
        // NOTE: this does NOT kill a "gc collects the whole directory" mutation —
        // `publish()` unconditionally rewrites ITS OWN pid's entry right after
        // `gc()` runs, so the final state here is identical whether gc preserved
        // this file or deleted it and the rewrite recreated it. What this test
        // actually kills is a "skip if a file already exists" mutation to the
        // write step: the pre-seeded `session: "old"` must become `"new"`.
        // `gc_does_not_remove_the_entry_of_an_unrelated_live_process` below is
        // the one that proves gc itself preserves a live entry, using a pid that
        // publish() does not also unconditionally rewrite.
        let dir = tempfile::tempdir().unwrap();
        let live = std::process::id();
        std::fs::write(
            dir.path().join(format!("{live}.json")),
            format!(r#"{{"pid":{live},"ppid":1,"started_at":"2026-01-01T00:00:00Z","cwd":"/","session":"old","hook_at":null}}"#),
        )
        .unwrap();
        Rendezvous::publish(Some(dir.path().to_path_buf()), Some("new"));
        let e = entry_at(dir.path(), live).expect("a live pid's entry must survive");
        assert_eq!(
            e.session.as_deref(),
            Some("new"),
            "our own entry is rewritten"
        );
    }
    #[test]
    fn gc_does_not_remove_the_entry_of_an_unrelated_live_process() {
        // Kills a mutation that drops the `process_alive` check in `gc` and
        // removes every numeric-stem `.json` file unconditionally. That mutation
        // passes every other test in this file: the empty-dir tests never seed a
        // pre-existing file for gc to look at, `publish_collects_entries_...`
        // wants the file gone either way, and `publish_keeps_the_entry_of_a_live_
        // process` uses OUR OWN pid, whose entry gets rewritten unconditionally
        // by `publish()` after `gc()` runs regardless of what gc did to it. A pid
        // that is alive but is NOT our own is the only way to observe gc's
        // liveness check in isolation.
        let dir = tempfile::tempdir().unwrap();

        #[cfg(unix)]
        let mut other = std::process::Command::new("sleep")
            .arg("5")
            .spawn()
            .unwrap();
        #[cfg(windows)]
        let mut other = std::process::Command::new("cmd")
            .args(["/C", "timeout /T 5 /NOBREAK >NUL"])
            .spawn()
            .unwrap();
        let other_pid = other.id();

        std::fs::write(
            dir.path().join(format!("{other_pid}.json")),
            format!(
                r#"{{"pid":{other_pid},"ppid":1,"started_at":"2026-01-01T00:00:00Z","cwd":"/","session":null,"hook_at":null}}"#
            ),
        )
        .unwrap();

        Rendezvous::publish(Some(dir.path().to_path_buf()), None);

        let survived = dir.path().join(format!("{other_pid}.json")).exists();
        let _ = other.kill();
        let _ = other.wait();

        assert!(
            survived,
            "gc must not remove a live, unrelated process's entry"
        );
    }

    #[test]
    fn publish_ignores_non_json_and_unparseable_files() {
        // The directory is shared; a stray file must never panic or be deleted.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("notes.txt"), "hello").unwrap();
        std::fs::write(dir.path().join("garbage.json"), "{{{{").unwrap();
        // M4: a numeric-stemmed but non-`.json` file, keyed to a genuinely DEAD
        // pid. If the `.json` extension filter in `gc` were deleted, `gc` would
        // parse this stem as a pid, find it dead, and remove the file. Neither
        // `notes.txt` nor `garbage.json` would catch that mutation: both are
        // skipped by the stem-parse guard instead, for an unrelated reason
        // (neither stem is numeric) — this file is what actually exercises the
        // extension check.
        let dead = a_dead_pid();
        std::fs::write(dir.path().join(format!("{dead}.txt")), "not json").unwrap();

        Rendezvous::publish(Some(dir.path().to_path_buf()), None);

        assert!(
            dir.path().join("notes.txt").exists(),
            "non-json, non-numeric-stem file must be left alone"
        );
        assert!(
            dir.path().join("garbage.json").exists(),
            "a .json file with a non-numeric stem must be left alone"
        );
        assert!(
            dir.path().join(format!("{dead}.txt")).exists(),
            "a numeric-stemmed but non-.json file must be left alone, even for a dead pid"
        );
    }
}
