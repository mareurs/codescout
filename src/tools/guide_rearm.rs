//! One-shot "please re-arm these topics" requests, written by the companion's
//! `SubagentStart` hook for a fresh (non-`fork`) subagent dispatch, and consumed by
//! the ALREADY RUNNING server on its very next request.
//!
//! Why this exists: `GuideLedger::load` runs once at server construction and the
//! in-memory ledger is authoritative for the process's life (`GuideLedger::persist`
//! is deliberately not read-modify-write). A subagent shares its parent's
//! `session_id` — no separate MCP identity exists for it — so the ledger already
//! knows a topic as "delivered" the moment the parent received it, and a genuinely
//! fresh subagent (zero inherited context) can be silently denied a guide it never
//! saw a single byte of. The existing snapshot/restore hook pair
//! (`agent-guide-snapshot.mjs` / `agent-guide-restore.mjs`) only edits the ON-DISK
//! ledger file, which this same process never re-reads — confirmed inert for the
//! live session, useful only for the NEXT reconnect. This module is what reaches the
//! live in-memory ledger instead. docs/issues/2026-08-31-subagents-receive-guides-
//! their-parent-already-holds.md
//!
//! One file per `(server_pid, agent_id)`, not one shared per-pid slot like
//! [`crate::tools::rendezvous::Rendezvous`]'s `<pid>.json`. That file has exactly
//! one writer at a time by design (whole-object overwrite) — two concurrently
//! dispatched subagents writing to the same slot would race last-writer-wins, the
//! exact bug class already hit once for the sibling snapshot/restore mechanism
//! (docs/issues/archive/2026-08-27-concurrent-subagent-restores-discard-parent-guide-marks.md).
//! Distinct per-agent filenames make that race structurally impossible here.
//!
//! No in-memory liveness state to memoize, unlike `Rendezvous`: each request file is
//! written once and consumed exactly once (deleted on read), so there is no repeated
//! stamp to dedupe against and no mtime to track.
//!
//! Gated by the request file's existence and pid match ALONE — not by
//! `Rendezvous::is_active()`. A freshly-written file addressed to this server's own
//! pid is itself stronger, more current proof of a live companion hook than the
//! latched `rendezvous_active` boolean (which can stay true from a stamp made hours
//! earlier — see `inherited_stamp`'s own doc comment in `rendezvous.rs`). No
//! companion installed ⇒ no file is ever written ⇒ `poll` always finds nothing ⇒
//! behavior is byte-identical to today. This still satisfies the project's "only act
//! on verified-live companion evidence, else degrade safely" idiom — via a simpler,
//! strictly stronger check.

use std::path::{Path, PathBuf};

/// The body of one request file. Any other key (e.g. a `created_at` debugging
/// timestamp the hook writes) is ignored by serde's default "extra fields are
/// dropped" behaviour — this struct only needs the one field it acts on.
#[derive(Debug, serde::Deserialize)]
struct RearmRequest {
    topics: Vec<String>,
}

/// Directory of pending re-arm requests. See module docs for the problem and the
/// per-`(pid, agent_id)` filename scheme.
#[derive(Debug, Clone)]
pub struct GuideRearmInbox {
    dir: Option<PathBuf>,
}

impl GuideRearmInbox {
    /// Creates the directory eagerly, mirroring `Rendezvous::publish`'s eager
    /// `create_dir_all` — the hook runs well after server construction and must
    /// never have to `mkdir` this itself. GCs orphaned requests whose pid is
    /// provably dead before returning: a server that crashed or was killed before
    /// polling its own requests would otherwise leave them forever, and a reused
    /// pid could then spuriously "inherit" a stranger's stale request.
    pub fn new(dir: Option<PathBuf>) -> Self {
        let Some(dir) = dir else {
            return Self { dir: None };
        };
        if std::fs::create_dir_all(&dir).is_err() {
            return Self { dir: None };
        }
        gc(&dir);
        Self { dir: Some(dir) }
    }

    /// Consume every pending request addressed to `std::process::id()`, returning
    /// the union of requested topics. Best-effort throughout: a request file that
    /// fails to read or parse is skipped AND removed — never retried, since a
    /// malformed request will not parse differently on the next poll. Presence of
    /// a file addressed to us IS the gate; there is no separate liveness check.
    pub fn poll(&self) -> Vec<String> {
        let Some(dir) = self.dir.as_deref() else {
            return Vec::new();
        };
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let my_pid = std::process::id();
        let mut topics = Vec::new();
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(pid) = pid_prefix(&path) else {
                continue;
            };
            if pid != my_pid {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(req) = serde_json::from_str::<RearmRequest>(&text) {
                    topics.extend(req.topics);
                }
            }
            let _ = std::fs::remove_file(&path);
        }
        topics
    }
}

/// Extracts the leading `<pid>` from a `<pid>-<hash>.json` filename stem. Returns
/// `None` for anything that doesn't start with digits followed by `-` — a
/// corrupt or foreign filename in this directory is simply not ours to touch.
fn pid_prefix(path: &Path) -> Option<u32> {
    let stem = path.file_stem()?.to_str()?;
    let (pid_str, _rest) = stem.split_once('-')?;
    pid_str.parse::<u32>().ok()
}

/// Remove request files whose server process is gone. Same dead-pid check as
/// `rendezvous::gc`; worst case on a pid-reuse collision is one harmless extra
/// re-arm delivered to the new occupant of that pid, never new starvation — the
/// same pre-existing, accepted risk class as `rendezvous.rs`'s own `<pid>.json`.
fn gc(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(pid) = pid_prefix(&path) else {
            continue;
        };
        if !crate::platform::process_alive(pid) {
            let _ = std::fs::remove_file(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_request(dir: &Path, pid: u32, agent_hash: &str, topics: &[&str]) {
        let path = dir.join(format!("{pid}-{agent_hash}.json"));
        let body = serde_json::json!({
            "topics": topics,
            "created_at": "2026-09-11T00:00:00Z",
        });
        std::fs::write(&path, body.to_string()).unwrap();
    }

    #[test]
    fn poll_with_no_directory_returns_empty_and_never_panics() {
        let inbox = GuideRearmInbox { dir: None };
        assert_eq!(inbox.poll(), Vec::<String>::new());
    }

    #[test]
    fn poll_with_an_empty_directory_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = GuideRearmInbox::new(Some(dir.path().to_path_buf()));
        assert_eq!(inbox.poll(), Vec::<String>::new());
    }

    #[test]
    fn poll_returns_topics_from_a_request_addressed_to_this_pid() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = GuideRearmInbox::new(Some(dir.path().to_path_buf()));
        let my_pid = std::process::id();
        write_request(
            dir.path(),
            my_pid,
            "abc123",
            &["librarian", "progressive-disclosure"],
        );

        let mut topics = inbox.poll();
        topics.sort();
        assert_eq!(
            topics,
            vec![
                "librarian".to_string(),
                "progressive-disclosure".to_string()
            ]
        );
    }

    #[test]
    fn poll_ignores_a_request_addressed_to_a_different_pid() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = GuideRearmInbox::new(Some(dir.path().to_path_buf()));
        // A dead-but-plausible pid: written directly (bypassing `new`'s own GC,
        // which already ran) so the file exists for `poll` to examine.
        write_request(dir.path(), 999_999_999, "abc123", &["librarian"]);

        assert_eq!(inbox.poll(), Vec::<String>::new());
    }

    #[test]
    fn poll_consumes_a_request_exactly_once() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = GuideRearmInbox::new(Some(dir.path().to_path_buf()));
        let my_pid = std::process::id();
        write_request(dir.path(), my_pid, "abc123", &["librarian"]);

        assert_eq!(inbox.poll(), vec!["librarian".to_string()]);
        assert_eq!(
            inbox.poll(),
            Vec::<String>::new(),
            "the second poll must find nothing — the first must have deleted the file"
        );
    }

    #[test]
    fn poll_unions_topics_from_concurrent_request_files_for_the_same_pid() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = GuideRearmInbox::new(Some(dir.path().to_path_buf()));
        let my_pid = std::process::id();
        write_request(dir.path(), my_pid, "agentone", &["librarian"]);
        write_request(dir.path(), my_pid, "agenttwo", &["workspace-state"]);

        let mut topics = inbox.poll();
        topics.sort();
        assert_eq!(
            topics,
            vec!["librarian".to_string(), "workspace-state".to_string()],
            "two concurrently-dispatched subagents' requests must not clobber each other"
        );
    }

    #[test]
    fn a_malformed_request_file_is_skipped_and_removed_rather_than_retried_forever() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = GuideRearmInbox::new(Some(dir.path().to_path_buf()));
        let my_pid = std::process::id();
        let path = dir.path().join(format!("{my_pid}-badjson.json"));
        std::fs::write(&path, "{ not valid json").unwrap();

        assert_eq!(inbox.poll(), Vec::<String>::new());
        assert!(
            !path.exists(),
            "a request that will never parse must be removed, not retried on every future poll"
        );
    }

    #[test]
    fn a_non_json_file_in_the_directory_is_left_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = GuideRearmInbox::new(Some(dir.path().to_path_buf()));
        let stray = dir.path().join("README.txt");
        std::fs::write(&stray, "not ours").unwrap();

        assert_eq!(inbox.poll(), Vec::<String>::new());
        assert!(stray.exists());
    }

    #[test]
    fn gc_removes_a_request_file_whose_pid_is_dead() {
        let dir = tempfile::tempdir().unwrap();
        // Construction runs gc(); seed the dead-pid file first so `new` reaps it.
        write_request(dir.path(), 999_999_999, "abc123", &["librarian"]);
        let _inbox = GuideRearmInbox::new(Some(dir.path().to_path_buf()));

        let remaining: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert!(
            remaining.is_empty(),
            "a request addressed to a pid that is provably dead must not survive construction"
        );
    }

    #[test]
    fn gc_keeps_a_request_file_whose_pid_is_alive() {
        let dir = tempfile::tempdir().unwrap();
        let my_pid = std::process::id();
        write_request(dir.path(), my_pid, "abc123", &["librarian"]);
        let inbox = GuideRearmInbox::new(Some(dir.path().to_path_buf()));

        // Still there after construction's own gc — `poll` (not gc) is what
        // consumes a live pid's own request.
        assert_eq!(inbox.poll(), vec!["librarian".to_string()]);
    }
}
