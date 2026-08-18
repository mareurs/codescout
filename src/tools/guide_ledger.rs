//! Session-scoped guide-hint ledger with disk persistence.
//!
//! Tracks which `get_guide(topic)` topics have already been surfaced to the
//! model in the current Claude Code conversation. The set is persisted to
//! `.codescout/guide_hints/<session_id>.json` so it **survives MCP server
//! restarts** — a `/mcp` reconnect re-spawns the codescout process, which would
//! otherwise reborn an empty in-memory set and re-inject every guide body the
//! conversation already holds. Fix for
//! `docs/issues/archive/2026-06-14-get-guide-reinjects-on-mcp-restart.md`.
//!
//! Keyed by `CLAUDE_CODE_SESSION_ID` (set by Claude Code in the MCP subprocess
//! env since v2.1.154) — per-process, so concurrent CC windows on one project
//! get distinct files and never collide. A `Default`-constructed ledger is
//! ephemeral (no path → no persistence); that is what the many internal/test
//! `ToolContext` builders get for free, so they compile unchanged.

use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

/// Topics emitted this session, each stamped with when it was delivered,
/// optionally backed by a per-session JSON file. Reads go through the in-memory
/// map; mutations write through.
///
/// The stamps are what make expiry (`expire_idle`) and garbage collection
/// expressible. The pre-2026-08-18 shape was a bare `Vec<String>`, which is still
/// read and migrated — see `read_entries`.
#[derive(Debug, Default)]
pub struct GuideLedger {
    /// Per-session file (`<dir>/<session_id>.json`). `None` ⇒ ephemeral.
    path: Option<PathBuf>,
    emitted: BTreeMap<String, DateTime<Utc>>,
    /// One-shot session notices that are NOT guide topics. (Unchanged — see the
    /// original field docs for why this is deliberately separate and unpersisted.)
    notices: HashSet<String>,
}

/// Accepts both on-disk shapes. `untagged` is unambiguous here because a JSON
/// array can only match `Legacy` and a JSON object can only match `Stamped`.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum LedgerFile {
    Stamped(BTreeMap<String, DateTime<Utc>>),
    Legacy(Vec<String>),
}

impl GuideLedger {
    /// Load the persisted ledger for `session_id` under `dir`. Best-effort: a
    /// missing, unreadable or malformed file yields an empty set — degrading to
    /// re-sending a guide, never to suppressing one. `dir = None` ⇒ ephemeral.
    pub fn load(session_id: &str, dir: Option<PathBuf>) -> Self {
        let path = dir.map(|d| d.join(format!("{}.json", sanitize(session_id))));
        let emitted = path.as_deref().map(read_entries).unwrap_or_default();
        Self {
            path,
            emitted,
            notices: HashSet::new(),
        }
    }

    /// The raw stamps, for tests that assert on migration and expiry.
    #[cfg(test)]
    pub fn stamps_for_test(&self) -> Vec<(String, DateTime<Utc>)> {
        self.emitted.iter().map(|(k, v)| (k.clone(), *v)).collect()
    }

    /// Has this topic already been surfaced this session?
    pub fn contains(&self, topic: &str) -> bool {
        self.emitted.contains_key(topic)
    }

    /// Has nothing been surfaced yet this session?
    ///
    /// True at session start and again after [`clear`](Self::clear) (workspace
    /// activate / post-compact re-arm). This is the condition that fires the
    /// session-opening guide from any tool — see
    /// `prompts::SESSION_OPENING_GUIDE`.
    pub fn is_empty(&self) -> bool {
        self.emitted.is_empty()
    }

    /// A ledger that behaves as if the session has already opened.
    ///
    /// Test-only. The session opener (`prompts::SESSION_OPENING_GUIDE`) fires on
    /// the first guide-eligible call of any session and appends a second content
    /// block, so a test asserting on *primary-block shape* must start
    /// mid-session or it measures the opener instead. Guide delivery itself is
    /// covered by `server::guide_hint_tests`.
    #[cfg(test)]
    pub fn mid_session() -> Self {
        let mut ledger = Self::default();
        // Default has no path, so this insert stamps in memory and persists nothing.
        ledger.insert(crate::prompts::SESSION_OPENING_GUIDE.to_string());
        ledger
    }

    /// Record a topic. Returns `true` if newly added (matching
    /// `HashSet::insert`); persists only on a genuine insertion.
    pub fn insert(&mut self, topic: String) -> bool {
        let added = self.emitted.insert(topic, Utc::now()).is_none();
        if added {
            self.persist();
        }
        added
    }

    /// Forget all topics (workspace activate / post-compact re-arm). Persists
    /// by removing the file so a later reload re-arms every guide.
    pub fn clear(&mut self) {
        let was_nonempty = !self.emitted.is_empty();
        self.emitted.clear();
        // Notices re-arm with the guides: an `activate` is precisely the act a
        // worktree notice asks the caller to perform, and a post-compact
        // re-arm means the model no longer remembers being told.
        self.notices.clear();
        if was_nonempty {
            self.persist();
        }
    }

    /// Record a one-shot session notice. Returns `true` the FIRST time this
    /// key is seen and `false` forever after (until [`clear`](Self::clear)),
    /// so the caller can write `if ledger.notice_once(K) { emit }`.
    ///
    /// Notices live beside guide topics, never inside them — see the `notices`
    /// field for why that separation is load-bearing rather than tidy.
    pub fn notice_once(&mut self, key: &str) -> bool {
        self.notices.insert(key.to_string())
    }

    /// Best-effort write-through. Persistence is an optimization, not a
    /// correctness requirement — failures are logged at debug, never raised.
    fn persist(&self) {
        let Some(path) = &self.path else { return };
        if self.emitted.is_empty() {
            let _ = std::fs::remove_file(path);
            return;
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match serde_json::to_string(&self.emitted) {
            Ok(json) => {
                if let Err(e) = std::fs::write(path, json) {
                    tracing::debug!("guide ledger persist failed ({}): {e}", path.display());
                }
            }
            Err(e) => tracing::debug!("guide ledger serialize failed: {e}"),
        }
    }
}

/// Read one ledger file, migrating the legacy `Vec<String>` shape on the way.
/// A legacy file has no per-topic stamps, so every topic inherits the file's
/// mtime: the best available evidence of when those guides were delivered, and
/// it keeps a freshly-migrated ledger from looking instantly expired.
fn read_entries(path: &Path) -> BTreeMap<String, DateTime<Utc>> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    match serde_json::from_str::<LedgerFile>(&raw) {
        Ok(LedgerFile::Stamped(m)) => m,
        Ok(LedgerFile::Legacy(topics)) => {
            let stamp = file_mtime(path).unwrap_or_else(Utc::now);
            topics.into_iter().map(|t| (t, stamp)).collect()
        }
        Err(_) => BTreeMap::new(),
    }
}

fn file_mtime(path: &Path) -> Option<DateTime<Utc>> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(DateTime::<Utc>::from(modified))
}

/// Session ids are uuids, but the env value / file fallback is untrusted — keep
/// the basename to a safe charset so it can't escape the directory.
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn ledger_survives_reload_and_isolates_sessions() {
        let dir = tempdir().unwrap();
        let hints_dir = dir.path().join(".codescout").join("guide_hints");

        // First "process": record a topic.
        let mut l = GuideLedger::load("sess-A", Some(hints_dir.clone()));
        assert!(!l.contains("librarian"));
        assert!(l.insert("librarian".to_string()), "first insert is new");
        assert!(
            !l.insert("librarian".to_string()),
            "second insert is a no-op"
        );
        drop(l);

        // Second "process" (simulated /mcp restart): same session reloads from disk.
        let l2 = GuideLedger::load("sess-A", Some(hints_dir.clone()));
        assert!(
            l2.contains("librarian"),
            "ledger must survive reconstruction (the bug)"
        );

        // A concurrent session on the same project sees nothing of A's.
        let l3 = GuideLedger::load("sess-B", Some(hints_dir.clone()));
        assert!(!l3.contains("librarian"), "sessions must be isolated");

        // Clear persists (removes the file) → next reload re-arms (compaction).
        let mut l4 = GuideLedger::load("sess-A", Some(hints_dir.clone()));
        l4.clear();
        drop(l4);
        let l5 = GuideLedger::load("sess-A", Some(hints_dir));
        assert!(!l5.contains("librarian"), "clear must persist");
    }

    #[test]
    fn ephemeral_ledger_is_in_memory_only() {
        // The Default ledger (no path) is what the 30+ test/internal ToolContext
        // builders get — pure in-memory, no files touched.
        let mut l = GuideLedger::default();
        assert!(l.insert("x".to_string()));
        assert!(l.contains("x"));
        l.clear();
        assert!(!l.contains("x"));
    }

    #[test]
    fn a_legacy_vec_file_is_read_and_stamped_from_its_mtime() {
        let dir = tempdir().unwrap();
        let hints = dir.path().join("guide_hints");
        std::fs::create_dir_all(&hints).unwrap();
        // The pre-2026-08-18 on-disk shape: a bare array, no timestamps.
        std::fs::write(
            hints.join("sess-legacy.json"),
            r#"["librarian","tracker-conventions"]"#,
        )
        .unwrap();

        let l = GuideLedger::load("sess-legacy", Some(hints.clone()));
        assert!(
            l.contains("librarian"),
            "legacy topics must survive the shape change"
        );
        assert!(l.contains("tracker-conventions"));

        // Every legacy topic is stamped with the file's mtime, so it is neither
        // instantly expired nor immortal.
        let stamps = l.stamps_for_test();
        assert_eq!(stamps.len(), 2);
        let now = chrono::Utc::now();
        for (_topic, at) in stamps {
            assert!(at <= now, "a stamp from mtime cannot be in the future");
            assert!(
                (now - at).num_seconds() < 60,
                "a file written just now must stamp as just now"
            );
        }
    }

    #[test]
    fn the_new_shape_round_trips_through_disk() {
        let dir = tempdir().unwrap();
        let hints = dir.path().join("guide_hints");

        let mut l = GuideLedger::load("sess-new", Some(hints.clone()));
        assert!(l.insert("librarian".to_string()));
        drop(l);

        // On disk it is now an object, not an array.
        let raw = std::fs::read_to_string(hints.join("sess-new.json")).unwrap();
        assert!(raw.starts_with('{'), "expected a stamped map, got: {raw}");
        assert!(raw.contains("librarian"));

        let l2 = GuideLedger::load("sess-new", Some(hints));
        assert!(l2.contains("librarian"), "the new shape must reload");
    }

    #[test]
    fn a_malformed_file_yields_an_empty_ledger_rather_than_a_panic() {
        let dir = tempdir().unwrap();
        let hints = dir.path().join("guide_hints");
        std::fs::create_dir_all(&hints).unwrap();
        std::fs::write(hints.join("sess-bad.json"), "{not json at all").unwrap();

        let l = GuideLedger::load("sess-bad", Some(hints));
        assert!(!l.contains("librarian"));
        assert!(
            l.is_empty(),
            "an unreadable ledger degrades to re-sending, never to suppressing"
        );
    }
}
