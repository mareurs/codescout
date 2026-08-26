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

/// Bump when the on-disk shape changes. Purely informational today: no reader
/// compares this value or branches on it -- a shape mismatch is silently
/// tolerated (unknown JSON keys are ignored, missing ones default), not
/// version-gated. See the `index-state.json` schema section of
/// `docs/state-protocol.md`. If version-gated degradation is ever built, update
/// this comment to describe the real mechanism rather than the aspiration.
pub const INDEX_STATE_SCHEMA_VERSION: u32 = 4;

/// The on-disk shape of `.codescout/index-state.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexState {
    /// Full git HEAD oid the index was last built against. Empty string when the
    /// project root is not inside a git repo (then there is no HEAD to lag).
    pub last_indexed_commit: String,
    /// RFC3339 timestamp of the sync completion that wrote this state.
    pub last_indexed_at: String,
    pub schema_version: u32,
    /// Project-relative paths this checkout must NOT inherit from the main index.
    /// Forward-slashed, project-relative -- identical in form to
    /// [`crate::retrieval::drift::ChunkRef::file_path`] (the indexer writes it via
    /// `to_forward_slash(rel_path)` at `src/retrieval/sync.rs:207`). Non-empty only
    /// for a worktree delta sync.
    ///
    /// `#[serde(default)]` is load-bearing: without it, a sidecar written before
    /// this field existed fails to parse, and [`read_index_state`] reads a failed
    /// parse as "no sidecar" -- i.e. "never indexed" -- for the whole project.
    #[serde(default)]
    pub dirty_paths: Vec<String>,
    /// The embedding model spec that produced the vectors currently in the store,
    /// in codescout-embed's grammar (`local:`, `openai:`, a bare name, ...).
    ///
    /// This is the *stored* model, which is NOT the configured one. They differ in
    /// exactly the failure the manual's troubleshooting section exists to diagnose,
    /// and reporting the configured value under this name would make a mismatch
    /// invisible by construction -- the two fields a reader is told to compare
    /// would be one value read twice.
    ///
    /// `None` means "not recorded": a sidecar written before this field existed, or
    /// a write by a path that is not the model-migration surface. Absence is not a
    /// mismatch, and must never be reported as one.
    ///
    /// Why it cannot be derived instead: a dimension check already catches a
    /// dimension *change* (`migrate_or_guard_index_dim`), but the common model swaps
    /// share a dimension -- `local:AllMiniLML6V2Q` and `local:BGESmallENV15` are
    /// both 384d, `CodeRankEmbed` and `JinaEmbeddingsV2BaseCode` both 768d -- so
    /// nothing but a stored identity can see them.
    ///
    /// `#[serde(default)]` for the same reason as `dirty_paths` above.
    #[serde(default)]
    pub indexed_with_model: Option<String>,
    /// How many chunks the sync that wrote this state could not embed (0 = clean).
    /// Sourced from `SyncReport.skipped.len()` -- see `flush_pending`'s doc comment
    /// in `src/retrieval/sync.rs` for why a batch failure is recorded rather than
    /// aborting the walk.
    ///
    /// Unlike `indexed_with_model`, this has no `Preserve` case and takes a plain
    /// `&[String]` rather than an enum: every write KNOWS whether THIS sync skipped
    /// anything (there is no "not the surface that would know" writer the way
    /// `sync_worktree` is for the model), so a clean run must record 0 rather than
    /// silently carrying forward a previous run's count.
    ///
    /// `#[serde(default)]` for the same reason as `dirty_paths` above: a sidecar
    /// written before this field existed must parse as "0 skipped", not fail.
    #[serde(default)]
    pub last_sync_skipped_count: usize,
    /// A bounded sample of what was skipped and why, capped at
    /// `SKIPPED_SAMPLE_CAP` -- matching `sync.rs`'s own `INTEGRITY_SAMPLE`
    /// convention (count exact, sample capped) without importing across the
    /// module boundary. Empty when `last_sync_skipped_count == 0`.
    #[serde(default)]
    pub last_sync_skipped_sample: Vec<String>,
}

/// What a sidecar write should do about [`IndexState::indexed_with_model`].
///
/// An explicit two-state choice rather than an `Option<&str>`, because this file has
/// already produced one silent-wipe defect of exactly this shape: `write_index_state`
/// delegates to `write_index_state_with_dirty(root, &[])`, which clears a worktree's
/// recorded `dirty_paths` -- a HUMAN RULING and a regression test
/// (`sync_project_preserves_existing_dirty_paths_never_clears_via_plain_write_index_state`)
/// exist solely to keep callers off that path. `None` would read as "no model" and be
/// mistaken for "clear it"; `Preserve` cannot be misread.
#[derive(Debug, Clone, Copy)]
pub enum ModelStamp<'a> {
    /// Record this model. For the writer that actually performed the embedding and
    /// therefore knows which model produced the vectors.
    Record(&'a str),
    /// Leave whatever is already stored. For a writer that is not the
    /// model-migration surface -- `sync_worktree` documents itself as one
    /// (`src/retrieval/sync.rs`: *"a worktree delta is never the surface a model
    /// migration happens on"*), and it has no config to read a model from anyway.
    Preserve,
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
///
/// **Test-only in practice, and left that way on purpose.** Every production writer
/// goes through [`write_index_state_with_dirty`] — a HUMAN RULING requires it,
/// because this wrapper passes `&[]` and would clear a worktree's recorded
/// `dirty_paths`. That semantics is unchanged here. The model is `Preserve`d rather
/// than cleared, since nothing about this wrapper's contract says a caller with no
/// dirty set also intends to forget which model built the index.
pub fn write_index_state(root: &Path) -> std::io::Result<()> {
    write_index_state_with_dirty(root, &[], ModelStamp::Preserve, &[])
}

/// As [`write_index_state`], additionally recording the paths a worktree's delta
/// sync must not inherit from the main index, and what to do with the stored model
/// spec. See the worktree sync mode in `sync.rs`.
///
/// `model` is a required argument, deliberately. A defaulting convenience wrapper is
/// what produced the `dirty_paths` wipe this file already carries a HUMAN RULING
/// about, so a new field gets no default and the compiler names every caller that
/// has to decide.
///
/// `skipped` is likewise required, plainly `&[String]` rather than an enum: pass
/// the chunks THIS sync could not embed (`SyncReport.skipped`), or `&[]` when
/// nothing was skipped or the calling path cannot yet know (see
/// `sync_worktree`'s call site, which writes before its embed pass runs).
pub fn write_index_state_with_dirty(
    root: &Path,
    dirty: &[String],
    model: ModelStamp<'_>,
    skipped: &[String],
) -> std::io::Result<()> {
    // Read-back-and-preserve for the model, mirroring what `sync_project` already
    // does by hand for `dirty_paths`. Done INSIDE the writer rather than at each
    // call site because every caller that is not the embedding path wants the same
    // thing, and the one that does want to change it says so via `Record`.
    let indexed_with_model = match model {
        ModelStamp::Record(m) => Some(m.to_string()),
        ModelStamp::Preserve => read_index_state(root).and_then(|s| s.indexed_with_model),
    };
    // Cap matches `sync.rs`'s own `INTEGRITY_SAMPLE` (20): count exact, sample
    // capped. Defined locally rather than imported -- this module has no other
    // dependency on `sync.rs`, and the value only needs to agree in spirit, not
    // by shared constant.
    const SKIPPED_SAMPLE_CAP: usize = 20;
    let state = IndexState {
        last_indexed_commit: head_commit_full(root).unwrap_or_default(),
        last_indexed_at: chrono::Utc::now().to_rfc3339(),
        schema_version: INDEX_STATE_SCHEMA_VERSION,
        dirty_paths: dirty.to_vec(),
        indexed_with_model,
        last_sync_skipped_count: skipped.len(),
        last_sync_skipped_sample: skipped.iter().take(SKIPPED_SAMPLE_CAP).cloned().collect(),
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

    #[test]
    fn dirty_paths_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        write_index_state_with_dirty(root, &["src/a.rs".to_string()], ModelStamp::Preserve, &[])
            .unwrap();
        let st = read_index_state(root).expect("sidecar should exist");
        assert_eq!(st.dirty_paths, vec!["src/a.rs".to_string()]);
    }

    #[test]
    fn last_sync_skipped_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        let skipped = vec![
            "src/a.rs:10 -- embedder timeout".to_string(),
            "src/b.rs:5 -- too large".to_string(),
        ];
        write_index_state_with_dirty(root, &[], ModelStamp::Preserve, &skipped).unwrap();
        let st = read_index_state(root).expect("sidecar should exist");
        assert_eq!(st.last_sync_skipped_count, 2);
        assert_eq!(st.last_sync_skipped_sample, skipped);
    }

    #[test]
    fn a_clean_sync_records_zero_skipped_not_a_stale_carry_over() {
        // Unlike `indexed_with_model` (which has a genuine "leave it" case via
        // `ModelStamp::Preserve`), a skip count has none: every write KNOWS whether
        // THIS sync skipped anything, so a clean run must record 0 -- never silently
        // carry forward a previous run's count.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        write_index_state_with_dirty(
            root,
            &[],
            ModelStamp::Preserve,
            &["src/a.rs:1 -- boom".to_string()],
        )
        .unwrap();
        write_index_state_with_dirty(root, &[], ModelStamp::Preserve, &[]).unwrap();
        let st = read_index_state(root).expect("sidecar should exist");
        assert_eq!(st.last_sync_skipped_count, 0);
        assert!(st.last_sync_skipped_sample.is_empty());
    }

    #[test]
    fn last_sync_skipped_sample_is_capped_but_count_stays_exact() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        let skipped: Vec<String> = (0..30).map(|i| format!("file{i}.rs -- reason")).collect();
        write_index_state_with_dirty(root, &[], ModelStamp::Preserve, &skipped).unwrap();
        let st = read_index_state(root).expect("sidecar should exist");
        assert_eq!(
            st.last_sync_skipped_count, 30,
            "count must stay exact even when the sample is capped"
        );
        assert_eq!(
            st.last_sync_skipped_sample.len(),
            20,
            "sample capped -- counts stay exact, matching sync.rs's own INTEGRITY_SAMPLE convention"
        );
    }

    #[test]
    fn sidecar_written_before_dirty_paths_existed_still_parses() {
        // Back-compat: an existing .codescout/index-state.json has no dirty_paths
        // key. It must read as an empty list, not fail the whole parse and silently
        // make every project look unindexed.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        std::fs::write(
            root.join(".codescout").join("index-state.json"),
            r#"{"last_indexed_commit":"abc","last_indexed_at":"2026-08-01T00:00:00Z","schema_version":1}"#,
        )
        .unwrap();
        let st = read_index_state(root).expect("old sidecar must still parse");
        assert!(st.dirty_paths.is_empty());
        assert_eq!(st.last_indexed_commit, "abc");
    }

    #[test]
    fn sidecar_written_before_last_sync_skipped_existed_still_parses() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        std::fs::write(
            root.join(".codescout").join("index-state.json"),
            r#"{"last_indexed_commit":"abc","last_indexed_at":"2026-08-01T00:00:00Z","schema_version":1}"#,
        )
        .unwrap();
        let st = read_index_state(root).expect("old sidecar must still parse");
        assert_eq!(st.last_sync_skipped_count, 0);
        assert!(st.last_sync_skipped_sample.is_empty());
    }

    /// `Record` stores the model; a later `Preserve` write must not erase it.
    ///
    /// This is the whole hazard [`ModelStamp`] exists for. `sync_worktree` writes with
    /// `Preserve`, and a worktree delta happens far more often than a full reindex — so
    /// if `Preserve` cleared the field, the model record would survive only until the
    /// next worktree sync and `index(action="status")` would go back to reporting
    /// nothing, with no error anywhere. Exactly the shape of the `dirty_paths` wipe this
    /// file already carries a HUMAN RULING about.
    #[test]
    fn preserve_does_not_erase_a_recorded_model() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        write_index_state_with_dirty(root, &[], ModelStamp::Record("local:BGESmallENV15"), &[])
            .unwrap();
        assert_eq!(
            read_index_state(root)
                .unwrap()
                .indexed_with_model
                .as_deref(),
            Some("local:BGESmallENV15")
        );

        write_index_state_with_dirty(root, &["src/a.rs".to_string()], ModelStamp::Preserve, &[])
            .unwrap();
        let st = read_index_state(root).unwrap();
        assert_eq!(
            st.indexed_with_model.as_deref(),
            Some("local:BGESmallENV15"),
            "a Preserve write must carry the model forward"
        );
        assert_eq!(
            st.dirty_paths,
            vec!["src/a.rs".to_string()],
            "and must still record its own dirty set"
        );
    }

    /// `Record` overwrites, because a reindex under a new model makes the old record
    /// false. The field must track the vectors actually in the store, not the first
    /// model ever used.
    #[test]
    fn record_replaces_a_previously_recorded_model() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        write_index_state_with_dirty(root, &[], ModelStamp::Record("local:AllMiniLML6V2Q"), &[])
            .unwrap();
        write_index_state_with_dirty(root, &[], ModelStamp::Record("CodeRankEmbed"), &[]).unwrap();

        assert_eq!(
            read_index_state(root)
                .unwrap()
                .indexed_with_model
                .as_deref(),
            Some("CodeRankEmbed")
        );
    }

    /// A sidecar written before this field existed must still parse, and read as "not
    /// recorded" rather than failing.
    ///
    /// `read_index_state` treats a parse failure as "no sidecar" — "never indexed" for
    /// the whole project — so a missing `#[serde(default)]` here would not be a small
    /// bug. Same reasoning as the `dirty_paths` case above.
    #[test]
    fn sidecar_written_before_the_model_field_existed_still_parses() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        std::fs::write(
            root.join(".codescout").join("index-state.json"),
            r#"{"last_indexed_commit":"abc","last_indexed_at":"2026-08-01T00:00:00Z","schema_version":2,"dirty_paths":[]}"#,
        )
        .unwrap();

        let st = read_index_state(root).expect("must parse, not read as absent");
        assert_eq!(st.indexed_with_model, None, "absence is not a mismatch");
        assert_eq!(st.last_indexed_commit, "abc");
    }
}
