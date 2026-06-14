//! Session-scoped guide-hint ledger with disk persistence.
//!
//! Tracks which `get_guide(topic)` topics have already been surfaced to the
//! model in the current Claude Code conversation. The set is persisted to
//! `.codescout/guide_hints/<session_id>.json` so it **survives MCP server
//! restarts** — a `/mcp` reconnect re-spawns the codescout process, which would
//! otherwise reborn an empty in-memory set and re-inject every guide body the
//! conversation already holds. Fix for
//! `docs/issues/2026-06-14-get-guide-reinjects-on-mcp-restart.md`.
//!
//! Keyed by `CLAUDE_CODE_SESSION_ID` (set by Claude Code in the MCP subprocess
//! env since v2.1.154) — per-process, so concurrent CC windows on one project
//! get distinct files and never collide. A `Default`-constructed ledger is
//! ephemeral (no path → no persistence); that is what the many internal/test
//! `ToolContext` builders get for free, so they compile unchanged.

use std::collections::HashSet;
use std::path::PathBuf;

/// In-memory set of emitted guide topics, optionally backed by a per-session
/// JSON file. Reads go through the in-memory set; mutations write through.
#[derive(Debug, Default)]
pub struct GuideLedger {
    /// Per-session file (`<dir>/<session_id>.json`). `None` ⇒ ephemeral.
    path: Option<PathBuf>,
    emitted: HashSet<String>,
}

impl GuideLedger {
    /// Load the persisted ledger for `session_id` under `dir` (the
    /// `.codescout/guide_hints` directory). Best-effort: a missing/unreadable
    /// or malformed file yields an empty set. `dir = None` ⇒ ephemeral.
    pub fn load(session_id: &str, dir: Option<PathBuf>) -> Self {
        let path = dir.map(|d| d.join(format!("{}.json", sanitize(session_id))));
        let emitted = path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
            .map(|v| v.into_iter().collect())
            .unwrap_or_default();
        Self { path, emitted }
    }

    /// Has this topic already been surfaced this session?
    pub fn contains(&self, topic: &str) -> bool {
        self.emitted.contains(topic)
    }

    /// Record a topic. Returns `true` if newly added (matching
    /// `HashSet::insert`); persists only on a genuine insertion.
    pub fn insert(&mut self, topic: String) -> bool {
        let added = self.emitted.insert(topic);
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
        if was_nonempty {
            self.persist();
        }
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
        let topics: Vec<&String> = self.emitted.iter().collect();
        match serde_json::to_string(&topics) {
            Ok(json) => {
                if let Err(e) = std::fs::write(path, json) {
                    tracing::debug!("guide ledger persist failed ({}): {e}", path.display());
                }
            }
            Err(e) => tracing::debug!("guide ledger serialize failed: {e}"),
        }
    }
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
}
