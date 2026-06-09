//! Index freshness sidecar — `.codescout/index-state.json`.
//!
//! Records the git commit the semantic index was last built against, so that
//! out-of-process consumers (the codescout-companion session-start hook) and
//! `index(action="status")` can detect when the working tree has moved ahead
//! via *external* git operations — `checkout`, `pull`, a HEAD change — that the
//! on-edit reindex never observes. This complements "Auto-Reindex on Edit"
//! (which re-embeds files edited *through* codescout's own write tools, drained
//! lazily at the next `semantic_search`); the two cover disjoint change sources.
//!
//! Design: O-1 in
//! `docs/trackers/2026-06-09-index-freshness-signal-for-consumers.md`.
//! Fail-soft everywhere: a missing or unreadable sidecar must never break
//! indexing or status — callers degrade to "freshness unknown" and omit the
//! `git_sync` field rather than render a misleading "up to date".

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Bump when the on-disk shape changes. Consumers compare and degrade gracefully
/// on a version they don't recognise.
pub const INDEX_STATE_SCHEMA_VERSION: u32 = 1;

/// The on-disk shape of `.codescout/index-state.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexState {
    /// Full git HEAD oid the index was last built against. Empty string when the
    /// project root is not inside a git repo (then there is no HEAD to lag).
    pub last_indexed_commit: String,
    /// RFC3339 timestamp of the sync completion that wrote this state.
    pub last_indexed_at: String,
    pub schema_version: u32,
}

fn state_path(root: &Path) -> PathBuf {
    root.join(".codescout").join("index-state.json")
}

/// Full git HEAD oid for the repo enclosing `root`, or `None` when `root` is not
/// in a git repo / the repo has no commits yet.
fn head_commit_full(root: &Path) -> Option<String> {
    let repo = git2::Repository::discover(root).ok()?;
    let head = repo.head().ok()?;
    let commit = head.peel_to_commit().ok()?;
    Some(commit.id().to_string())
}

/// Write the freshness sidecar, recording the current HEAD as the indexed commit.
///
/// Fail-soft by contract: the only failures are filesystem errors (the
/// `.codescout` dir already exists in any indexed project), and callers
/// log-and-continue. A non-git root records an empty commit, which
/// [`git_sync_status`] reads as "freshness indeterminate".
pub fn write_index_state(root: &Path) -> std::io::Result<()> {
    let state = IndexState {
        last_indexed_commit: head_commit_full(root).unwrap_or_default(),
        last_indexed_at: chrono::Utc::now().to_rfc3339(),
        schema_version: INDEX_STATE_SCHEMA_VERSION,
    };
    std::fs::create_dir_all(root.join(".codescout"))?;
    let body = serde_json::to_string_pretty(&state).map_err(std::io::Error::other)?;
    std::fs::write(state_path(root), body)
}

/// Read the sidecar, or `None` when it is absent / unparseable.
pub fn read_index_state(root: &Path) -> Option<IndexState> {
    let raw = std::fs::read_to_string(state_path(root)).ok()?;
    serde_json::from_str(&raw).ok()
}

/// The `git_sync` envelope for `index(action="status")`, comparing the recorded
/// indexed commit to current HEAD.
///
/// Returns `None` when freshness is indeterminate — no sidecar, a non-git root,
/// or HEAD unreadable — so the caller omits `git_sync` rather than claim a state
/// it cannot back. Shape:
/// `{ status: "up_to_date" | "behind", behind_commits, last_indexed_commit, head_commit }`.
pub fn git_sync_status(root: &Path) -> Option<Value> {
    let state = read_index_state(root)?;
    if state.last_indexed_commit.is_empty() {
        return None;
    }
    let head = head_commit_full(root)?;
    let short = |s: &str| s.chars().take(8).collect::<String>();

    if head == state.last_indexed_commit {
        return Some(json!({
            "status": "up_to_date",
            "behind_commits": 0,
            "last_indexed_commit": short(&state.last_indexed_commit),
            "head_commit": short(&head),
        }));
    }

    // HEAD has moved off the indexed commit — the index is stale. Best-effort
    // count of how far; 0 when the recorded commit is no longer resolvable
    // (e.g. rebased away), but status stays "behind" because we *know* it differs.
    let behind = behind_count(root, &head, &state.last_indexed_commit).unwrap_or(0);
    Some(json!({
        "status": "behind",
        "behind_commits": behind,
        "last_indexed_commit": short(&state.last_indexed_commit),
        "head_commit": short(&head),
    }))
}

/// Commits reachable from `head` but not from `indexed` (git2 `graph_ahead_behind`'s
/// `ahead` term) — i.e. how many commits the index is lagging behind HEAD.
fn behind_count(root: &Path, head: &str, indexed: &str) -> Option<u64> {
    let repo = git2::Repository::discover(root).ok()?;
    let head_oid = git2::Oid::from_str(head).ok()?;
    let indexed_oid = git2::Oid::from_str(indexed).ok()?;
    let (ahead, _behind) = repo.graph_ahead_behind(head_oid, indexed_oid).ok()?;
    Some(ahead as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    /// Commit `file`=`content` into the repo at `root` (init-ing it on first call),
    /// returning the new commit oid. Each call advances HEAD by one commit.
    fn commit(root: &Path, file: &str, content: &str, msg: &str) -> git2::Oid {
        let repo = git2::Repository::open(root)
            .or_else(|_| git2::Repository::init(root))
            .unwrap();
        fs::write(root.join(file), content).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(file)).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents)
            .unwrap()
    }

    // Three-query cache-invalidation sandwich (CLAUDE.md testing pattern):
    // baseline fresh → mutate HEAD → assert STALE → reindex → assert fresh.
    #[test]
    fn git_sync_tracks_external_head_movement() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        // commit A, then record the index against it.
        commit(root, "a.txt", "A", "first");
        write_index_state(root).unwrap();

        // (1) baseline — index is at HEAD.
        let gs = git_sync_status(root).unwrap();
        assert_eq!(gs["status"], "up_to_date");
        assert_eq!(gs["behind_commits"], 0);

        // (2) HEAD moves ahead via an "external" commit; sidecar untouched.
        commit(root, "b.txt", "B", "second");

        // (3) STALE proof — without this assertion the test would not prove the
        //     freshness signal actually fires (it would only test the happy path).
        let gs = git_sync_status(root).unwrap();
        assert_eq!(gs["status"], "behind");
        assert_eq!(gs["behind_commits"], 1);

        // (4) reindex rewrites the sidecar at the new HEAD.
        write_index_state(root).unwrap();

        // (5) fresh again.
        let gs = git_sync_status(root).unwrap();
        assert_eq!(gs["status"], "up_to_date");
        assert_eq!(gs["behind_commits"], 0);
    }

    #[test]
    fn non_git_root_yields_no_git_sync() {
        let tmp = tempfile::tempdir().unwrap();
        // Writing succeeds (records an empty commit); freshness is indeterminate.
        write_index_state(tmp.path()).unwrap();
        let state = read_index_state(tmp.path()).unwrap();
        assert_eq!(state.last_indexed_commit, "");
        assert!(git_sync_status(tmp.path()).is_none());
    }

    #[test]
    fn missing_sidecar_yields_no_git_sync() {
        let tmp = tempfile::tempdir().unwrap();
        commit(tmp.path(), "a.txt", "A", "first");
        // No write_index_state call → no sidecar on disk.
        assert!(git_sync_status(tmp.path()).is_none());
    }
}
