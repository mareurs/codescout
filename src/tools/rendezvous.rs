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
/// it knows a hook — rather than only itself — has written for this conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub pid: u32,
    pub ppid: u32,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub cwd: String,
    pub session: Option<String>,
    /// Written by the companion hook, or carried forward by [`Rendezvous::publish`]
    /// from a predecessor slot for the SAME conversation. `None` ⇒ no rendezvous is
    /// active.
    ///
    /// The inheritance is what makes this field mean "a hook has stamped a slot for
    /// this CONVERSATION" rather than "for this PROCESS". Hook installation is a
    /// property of the conversation; keying it to a process lost the fact on every
    /// `/mcp` reconnect. See
    /// docs/issues/archive/2026-08-19-mcp-reconnect-leaves-rendezvous-inactive-so-activate-clears-the-ledger.md
    pub hook_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub struct Rendezvous {
    path: Option<PathBuf>,
    /// Last mtime we parsed at. `None` ⇒ never read.
    last_mtime: Option<std::time::SystemTime>,
    /// The session we currently believe we are serving.
    current: Option<String>,
    /// Set once a hook has written here.
    active: bool,
}

impl Rendezvous {
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Write this process's slot, and collect slots whose process is gone.
    /// Best-effort throughout: a failure costs the `/clear` refresh, never
    /// correctness — the idle TTL and the next restart both still work.
    ///
    /// `current` is seeded with the session we just wrote, which is what makes
    /// the FIRST [`poll`](Self::poll) quiet: it re-reads the file it wrote,
    /// finds the same session, and reports no change. Seeding it `None` instead
    /// would re-arm the ledger on every server start.
    pub fn publish(dir: Option<PathBuf>, session: Option<&str>) -> Self {
        let inert = || Self {
            path: None,
            last_mtime: None,
            current: None,
            active: false,
        };
        let Some(dir) = dir else {
            return inert();
        };
        if std::fs::create_dir_all(&dir).is_err() {
            return inert();
        }
        // BEFORE `gc`, which is the only window in which the predecessor still
        // exists: a `/mcp` reconnect kills the old server and starts this one, so
        // by the time `gc` reaps dead-process slots the evidence is gone.
        let inherited = inherited_stamp(&dir, session);
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
            hook_at: inherited,
        };
        let path = dir.join(format!("{pid}.json"));
        match serde_json::to_string(&entry) {
            Ok(json) => match crate::util::fs::write_utf8(&path, &json) {
                Ok(()) => Self {
                    path: Some(path),
                    last_mtime: None,
                    current: entry.session,
                    active: inherited.is_some(),
                },
                Err(e) => {
                    tracing::debug!("rendezvous publish failed ({}): {e}", path.display());
                    inert()
                }
            },
            Err(e) => {
                tracing::debug!("rendezvous serialize failed: {e}");
                inert()
            }
        }
    }

    /// Has a companion hook written for this CONVERSATION — into our slot, or into a
    /// predecessor slot this one inherited from across a `/mcp` reconnect?
    ///
    /// Phase C gates its re-arm predicate on this: without a rendezvous the
    /// server cannot detect a conversation change, so the blunt
    /// clear-on-every-activate behaviour has to stay. Shipping the precise
    /// predicate ungated would remove the accidental mitigation for `/clear`
    /// without supplying the real one. Public — and covered by this module's
    /// own tests — ahead of that consumer: `CodeScoutServer::poll_rendezvous`
    /// now copies it onto `GuideLedger::rendezvous_active` on every request.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// The session we currently believe we are serving, if any — the same value
    /// [`poll`](Self::poll) just re-keyed to when it returns `Some`, and the value
    /// seeded at [`publish`](Self::publish) otherwise.
    ///
    /// This is the per-call correction for usage telemetry: unlike a caller's own
    /// construction-time snapshot, this field is updated by every `poll()` that
    /// observes a fresh stamp, so reading it after a poll reflects the CURRENT
    /// conversation rather than the one the process was born with.
    /// docs/issues/2026-08-20-telemetry-session-id-frozen-while-the-ledger-re-keys-per-call.md
    pub fn current(&self) -> Option<&str> {
        self.current.as_deref()
    }

    /// Returns the new session id ONLY when it changed.
    ///
    /// Called on every guide-eligible request, so the unchanged path must stay
    /// cheap: one `metadata` call, and no read or parse unless the mtime moved.
    /// That is a cost budget, not an optimisation.
    ///
    /// Every I/O or parse failure yields `None` — a missing, truncated or
    /// corrupt slot costs the `/clear` refresh, never correctness.
    pub fn poll(&mut self) -> Option<String> {
        let path = self.path.as_deref()?;
        let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
        if self.last_mtime == Some(mtime) {
            return None;
        }
        // Commit the mtime only once the read AND parse both succeed. A poll
        // landing mid-write (the hook writes non-atomically) can see a
        // truncated file at this mtime; if we recorded it anyway, a truncated
        // and a completed write sharing one mtime tick would make the server
        // silently never see the completed stamp. Leaving `last_mtime`
        // unchanged on failure makes the next poll retry this same mtime.
        let entry: Entry = std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())?;
        self.last_mtime = Some(mtime);
        if entry.hook_at.is_some() {
            self.active = true;
        }
        let session = entry.session?;
        // A repeated stamp of the SAME session must be silent: every
        // `SessionStart` stamps, `resume` included, and re-arming there would
        // punish resuming a conversation.
        if self.current.as_deref() == Some(session.as_str()) {
            return None;
        }
        self.current = Some(session.clone());
        Some(session)
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

/// The newest stamp any slot for `session` carries.
///
/// This is what makes a `/mcp` reconnect survivable. The server publishes a fresh
/// slot at construction with no stamp, and the ONLY writer of `hook_at` is the
/// companion's `SessionStart` hook — which does not fire on a reconnect. Without
/// this, `is_active()` reports false for the rest of the conversation with no path
/// back, and `ActivateProject::call` then takes its blunt `GuideLedger::clear()`
/// branch on every activation, re-sending every guide the conversation already holds.
///
/// Matched on the SESSION id, never on pid or cwd: a pid is useless as durable
/// identity (see this module's own doc) and cwd would wrongly inherit between two
/// conversations in one repo — the 2026-08-16 attribution bug, in a new place.
///
/// Absent a companion the scan finds nothing, so a hookless client keeps the blunt
/// behaviour exactly as before. That default is load-bearing: the keyed tier carries
/// no idle TTL (`GuideLedger::load` sets `idle_ttl: None`), so for such a client the
/// blunt clear is the ONLY thing standing between a `/clear` and permanent guide
/// starvation.
///
/// docs/issues/archive/2026-08-19-mcp-reconnect-leaves-rendezvous-inactive-so-activate-clears-the-ledger.md
fn inherited_stamp(dir: &Path, session: Option<&str>) -> Option<chrono::DateTime<chrono::Utc>> {
    let session = session?;
    let mut newest: Option<chrono::DateTime<chrono::Utc>> = None;
    for entry in std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        // Best-effort per slot: an unreadable or half-written neighbour costs this
        // one inheritance, never the publish.
        let Some(e) = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<Entry>(&t).ok())
        else {
            continue;
        };
        if e.session.as_deref() != Some(session) {
            continue;
        }
        if let Some(at) = e.hook_at {
            newest = Some(newest.map_or(at, |n: chrono::DateTime<chrono::Utc>| n.max(at)));
        }
    }
    newest
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

    #[cfg(unix)]
    #[test]
    fn publish_records_the_parent_pid_the_hook_matches_on() {
        // The hook selects entries whose ppid is on its own ancestry. A zero or
        // missing ppid makes every entry unmatchable — the feature silently dies.
        //
        // Unix only, because `parent_pid()` is `#[cfg(windows)] -> 0` BY DESIGN: there is
        // no `getppid` there, and its doc comment argues a zero degrades to "never
        // matched" rather than to a WRONG match. This assertion therefore contradicted
        // shipped intent on Windows and failed on all three lanes. The sibling below pins
        // the Windows contract instead of leaving it to a skip.
        let dir = tempfile::tempdir().unwrap();
        Rendezvous::publish(Some(dir.path().to_path_buf()), None);
        let e = entry_at(dir.path(), std::process::id()).unwrap();
        assert_ne!(e.ppid, 0, "ppid must be recorded");
    }

    /// The Windows contract, pinned rather than skipped: `parent_pid()` returns 0 there
    /// on purpose, so the rendezvous never matches on ppid and the hook's ancestry walk
    /// is the only path. That is a real, accepted cost — and writing it down is what
    /// stops the next reader "fixing" the zero without also updating the matcher, or
    /// deleting this file's Unix assertion as platform-flaky.
    ///
    /// If someone does implement a Windows `getppid` equivalent, this fails, and that is
    /// the signal to re-unify the two rather than a mystery.
    #[cfg(windows)]
    #[test]
    fn publish_records_a_zero_parent_pid_on_windows_by_design() {
        let dir = tempfile::tempdir().unwrap();
        Rendezvous::publish(Some(dir.path().to_path_buf()), None);
        let e = entry_at(dir.path(), std::process::id()).unwrap();
        assert_eq!(
            e.ppid, 0,
            "Windows has no getppid here; 0 is deliberate and means 'never matched', \
             which is the safe direction versus a wrong match"
        );
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

    /// Rewrite an entry the way the companion hook does: new session, hook_at set.
    fn stamp_as_hook(path: &std::path::Path, session: &str) {
        stamp_as_hook_at(path, session, 2_000_000_000);
    }

    /// `stamp_as_hook` with an explicit mtime, so a test can hold the mtime
    /// still while changing the bytes underneath it.
    fn stamp_as_hook_at(path: &std::path::Path, session: &str, mtime_secs: i64) {
        let mut e: Entry = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        e.session = Some(session.to_string());
        e.hook_at = Some(chrono::Utc::now());
        std::fs::write(path, serde_json::to_string(&e).unwrap()).unwrap();
        // mtime resolution is coarse on some filesystems; make the change visible
        // deterministically rather than by sleeping.
        filetime::set_file_mtime(path, filetime::FileTime::from_unix_time(mtime_secs, 0)).unwrap();
    }

    #[test]
    fn poll_reports_a_session_written_by_the_hook() {
        let dir = tempfile::tempdir().unwrap();
        let mut r = Rendezvous::publish(Some(dir.path().to_path_buf()), Some("old"));
        stamp_as_hook(r.path().unwrap(), "new");
        assert_eq!(r.poll().as_deref(), Some("new"));
        assert!(r.is_active(), "a hook stamp activates the rendezvous");
    }

    #[test]
    fn poll_returns_none_when_nothing_changed() {
        // Kills a mutation that re-arms on every call — which would wipe the
        // ledger continuously and defeat the entire feature while looking fine.
        let dir = tempfile::tempdir().unwrap();
        let mut r = Rendezvous::publish(Some(dir.path().to_path_buf()), Some("old"));
        stamp_as_hook(r.path().unwrap(), "new");
        assert_eq!(r.poll().as_deref(), Some("new"));
        assert_eq!(
            r.poll(),
            None,
            "a second poll with no new write must be quiet"
        );
    }

    #[test]
    fn poll_ignores_a_stamp_repeating_the_session_we_already_have() {
        // Every SessionStart stamps, including `resume` on an unchanged
        // conversation. Re-arming there would punish resuming a session.
        let dir = tempfile::tempdir().unwrap();
        let mut r = Rendezvous::publish(Some(dir.path().to_path_buf()), Some("same"));
        stamp_as_hook(r.path().unwrap(), "same");
        assert_eq!(r.poll(), None);
    }

    #[test]
    fn poll_does_not_re_read_when_the_mtime_is_unchanged() {
        // The mtime check is a COST BUDGET, not an optimisation: `poll` runs on
        // every guide-eligible request, so the unchanged path must be one
        // `metadata` call with no read and no parse.
        //
        // Proven by making the file LIE: a third session id written back under
        // the mtime we already recorded must stay invisible. Nothing else in
        // this file kills a dropped short-circuit — the other tests all end in
        // `None` either way, because a re-read finds the same session.
        let dir = tempfile::tempdir().unwrap();
        let mut r = Rendezvous::publish(Some(dir.path().to_path_buf()), Some("old"));
        stamp_as_hook(r.path().unwrap(), "new");
        assert_eq!(r.poll().as_deref(), Some("new"));

        stamp_as_hook_at(r.path().unwrap(), "newer", 2_000_000_000);
        assert_eq!(
            r.poll(),
            None,
            "an unchanged mtime must short-circuit BEFORE the read"
        );
    }
    #[test]
    fn poll_retries_a_torn_write_once_repaired_at_the_same_mtime() {
        // The hook writes its stamp non-atomically. A poll landing mid-write
        // can read a truncated file at that mtime; if `poll` burned the mtime
        // anyway, the eventual completed write sharing that same mtime tick
        // would stay silently invisible until the slot moves to a new mtime.
        let dir = tempfile::tempdir().unwrap();
        let mut r = Rendezvous::publish(Some(dir.path().to_path_buf()), Some("old"));
        stamp_as_hook(r.path().unwrap(), "new");
        assert_eq!(r.poll().as_deref(), Some("new"));

        // A poll lands mid non-atomic write: malformed JSON at a fresh mtime.
        std::fs::write(r.path().unwrap(), "{{{{").unwrap();
        filetime::set_file_mtime(
            r.path().unwrap(),
            filetime::FileTime::from_unix_time(3_000_000_000, 0),
        )
        .unwrap();
        assert_eq!(r.poll(), None, "a torn read must not report a session");

        // The write completes and repairs the file, but lands at the SAME
        // mtime the torn read already saw (coarse mtime resolution can tick
        // both writes into one bucket). This is the regression: without the
        // fix, `poll` already burned this mtime on the failed parse and the
        // repaired stamp is lost until the slot is rewritten again.
        let repaired = Entry {
            pid: std::process::id(),
            ppid: 0,
            started_at: chrono::Utc::now(),
            cwd: String::new(),
            session: Some("newer".to_string()),
            hook_at: Some(chrono::Utc::now()),
        };
        std::fs::write(r.path().unwrap(), serde_json::to_string(&repaired).unwrap()).unwrap();
        filetime::set_file_mtime(
            r.path().unwrap(),
            filetime::FileTime::from_unix_time(3_000_000_000, 0),
        )
        .unwrap();

        assert_eq!(
            r.poll().as_deref(),
            Some("newer"),
            "a failed parse must not burn the mtime, or the repaired stamp is \
             lost until the next mtime tick"
        );
    }

    #[test]
    fn an_unstamped_entry_is_not_active_and_polls_quiet() {
        // The no-companion path. Must degrade, never mis-fire.
        let dir = tempfile::tempdir().unwrap();
        let mut r = Rendezvous::publish(Some(dir.path().to_path_buf()), Some("s"));
        assert!(!r.is_active());
        assert_eq!(r.poll(), None);
        // AFTER the poll too: `active` is only ever written inside `poll`, so a
        // pre-poll assertion alone cannot see an unconditional `active = true`
        // there — measured, not assumed (mutation M2b stayed green without this
        // line). Phase C gates its re-arm predicate on `is_active`, so a slot
        // that no hook ever touched reporting `true` would hand Phase C a
        // rendezvous it does not have.
        assert!(
            !r.is_active(),
            "reading an entry no hook has stamped must not activate it"
        );
    }

    #[test]
    fn a_path_less_rendezvous_polls_quiet_and_is_never_active() {
        // No servers dir (or a publish failure) ⇒ the server simply never learns
        // about a conversation change. The anonymous-tier idle TTL is what
        // catches `/clear` then, one interval late.
        let mut r = Rendezvous::publish(None, Some("s"));
        assert!(!r.is_active());
        assert_eq!(r.poll(), None);
    }

    #[test]
    fn a_corrupt_or_deleted_entry_polls_quiet_rather_than_panicking() {
        let dir = tempfile::tempdir().unwrap();
        let mut r = Rendezvous::publish(Some(dir.path().to_path_buf()), Some("s"));
        std::fs::write(r.path().unwrap(), "{{{{").unwrap();
        assert_eq!(r.poll(), None);
        std::fs::remove_file(r.path().unwrap()).unwrap();
        assert_eq!(r.poll(), None);
    }

    /// What a previous server for a conversation left behind, optionally stamped.
    fn seed_predecessor(dir: &std::path::Path, pid: u32, session: &str, stamped: bool) {
        let e = Entry {
            pid,
            ppid: 1,
            started_at: chrono::Utc::now(),
            cwd: "/tmp".to_string(),
            session: Some(session.to_string()),
            hook_at: stamped.then(chrono::Utc::now),
        };
        std::fs::write(
            dir.join(format!("{pid}.json")),
            serde_json::to_string(&e).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn publish_inherits_the_stamp_from_a_predecessor_slot_for_the_same_conversation() {
        let dir = tempfile::tempdir().unwrap();
        let dead = a_dead_pid();
        seed_predecessor(dir.path(), dead, "sess-1", true);

        let r = Rendezvous::publish(Some(dir.path().to_path_buf()), Some("sess-1"));

        assert!(
            r.is_active(),
            "a reconnect must not lose the fact that a hook is installed for this \
             conversation"
        );
        let mine = entry_at(dir.path(), std::process::id()).unwrap();
        assert!(
            mine.hook_at.is_some(),
            "the stamp is carried into the new slot, so a SECOND reconnect inherits too"
        );
        assert!(
            entry_at(dir.path(), dead).is_none(),
            "the predecessor is still collected — the scan must run BEFORE gc, not \
             instead of it"
        );
    }

    #[test]
    fn publish_does_not_inherit_a_stamp_from_a_different_conversation() {
        let dir = tempfile::tempdir().unwrap();
        seed_predecessor(dir.path(), a_dead_pid(), "someone-elses-session", true);

        let r = Rendezvous::publish(Some(dir.path().to_path_buf()), Some("sess-1"));

        assert!(
            !r.is_active(),
            "inheriting across conversations would let one window's hook vouch for \
             another's — the 2026-08-16 attribution bug in a new place"
        );
        assert!(entry_at(dir.path(), std::process::id())
            .unwrap()
            .hook_at
            .is_none());
    }

    #[test]
    fn publish_stays_inactive_when_no_predecessor_was_stamped() {
        let dir = tempfile::tempdir().unwrap();
        seed_predecessor(dir.path(), a_dead_pid(), "sess-1", false);

        let r = Rendezvous::publish(Some(dir.path().to_path_buf()), Some("sess-1"));

        assert!(
            !r.is_active(),
            "a client with no companion keeps the blunt clear: the keyed tier carries no \
             idle TTL, so that default is the only thing between a /clear and permanent \
             guide starvation"
        );
    }

    #[test]
    fn a_reconnect_keeps_the_rendezvous_active() {
        // The real sequence: the server publishes, the SessionStart hook stamps, that
        // server dies on `/mcp`, and a new one publishes. Only the pid differs between
        // the two — and a test cannot change its own — so the first slot is renamed onto
        // a dead pid to stand in for "that process is gone".
        let dir = tempfile::tempdir().unwrap();
        let r1 = Rendezvous::publish(Some(dir.path().to_path_buf()), Some("sess-1"));
        assert!(!r1.is_active(), "unstamped at first publish, as before");
        stamp_as_hook(r1.path().unwrap(), "sess-1");

        let dead = a_dead_pid();
        std::fs::rename(
            dir.path().join(format!("{}.json", std::process::id())),
            dir.path().join(format!("{dead}.json")),
        )
        .unwrap();

        let r2 = Rendezvous::publish(Some(dir.path().to_path_buf()), Some("sess-1"));

        assert!(
            r2.is_active(),
            "before this fix a reconnect reported inactive for the rest of the \
             conversation, and every workspace(activate) then cleared the whole guide \
             ledger — ~59-67 KB of guides re-sent into a context already holding them"
        );
    }
}
