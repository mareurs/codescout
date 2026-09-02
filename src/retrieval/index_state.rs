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
pub const INDEX_STATE_SCHEMA_VERSION: u32 = 5;

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
    /// Which process wrote this file, and what code it was running.
    ///
    /// Shared per-project state has no lock and no ownership: whoever syncs last
    /// wins. That is adjudicable only if a reader can tell WHO won. Measured on
    /// this host 2026-08-28: seven concurrent `codescout start` processes, six of
    /// them executing binaries that had already been unlinked -- one of which had
    /// been running for nine minutes, zombified by a routine `cargo rb` rather
    /// than by age. A rebuild invalidates every already-running server instantly,
    /// so this is a property of the ordinary edit-build loop, not of long
    /// sessions.
    ///
    /// `None` means a sidecar written before this field existed. Absence is "not
    /// recorded", never "written by the current build" -- the same rule
    /// `indexed_with_model` states above.
    /// docs/issues/archive/2026-08-26-zombie-servers-on-deleted-binaries-stamp-stale-config-into-shared-state.md
    #[serde(default)]
    pub written_by: Option<WriterProvenance>,
}

/// Identity of the process that wrote an [`IndexState`], and of the code it was
/// running.
///
/// Two of the three fields are portable and carry most of the diagnostic value on
/// their own: a sidecar stamped with a `git_sha` different from the reading
/// binary's own `env!("CODESCOUT_GIT_SHA")` proves a different build wrote it, on
/// every platform, with no `/proc` walk. `exe_deleted` is the Linux-only bonus,
/// not a gate -- which is why this ships without an answer for the non-Linux
/// `/proc/self/exe` question that had been treated as blocking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WriterProvenance {
    /// `git rev-parse --short HEAD` at build time, baked in by `build.rs` as
    /// `CODESCOUT_GIT_SHA`. Literally `"unknown"` when the build had no git.
    pub git_sha: String,
    /// Whether that build came from a dirty working tree, in which case `git_sha`
    /// does not fully identify the code. `build.rs` bakes `"0"`/`"1"`/`"unknown"`;
    /// `"1"` is the only true, matching `Commands::Version`'s own reading.
    pub git_dirty: bool,
    /// The writer's pid, so a reader can ask whether it is even still alive.
    pub pid: u32,
    /// Whether the writing process's own executable had already been unlinked.
    ///
    /// `None` off Linux, where `/proc/self/exe` does not exist. `None` means
    /// "could not tell" and must never be read as "not deleted" -- the same
    /// distinction `indexed_with_model` draws between absence and mismatch.
    pub exe_deleted: Option<bool>,
}

/// Snapshot this process's identity for [`WriterProvenance`].
pub fn current_writer() -> WriterProvenance {
    WriterProvenance {
        git_sha: env!("CODESCOUT_GIT_SHA").to_string(),
        git_dirty: env!("CODESCOUT_GIT_DIRTY") == "1",
        pid: std::process::id(),
        exe_deleted: exe_is_deleted(),
    }
}

/// The pure half of [`exe_is_deleted`]: does this resolved `/proc/self/exe` target
/// report an unlinked binary?
///
/// Split out 2026-09-02 so the *predicate* can be tested without depending on whether
/// this process's own executable happens to still exist. It does not always: on a
/// shared checkout a peer's `cargo build` unlinks the running test binary mid-suite,
/// which falsifies the premise of `a_live_binary_does_not_report_itself_deleted` and
/// made it fail while reporting an inverted predicate — a true statement about the
/// environment, misattributed to the code. The three cases the doc comment above
/// describes in prose are fixtures against this function now.
#[cfg(target_os = "linux")]
fn path_reports_deleted(p: &std::path::Path) -> bool {
    p.to_string_lossy().ends_with(" (deleted)") && !p.exists()
}

/// Whether this process's executable has been unlinked since it started.
///
/// Measured 2026-08-28 by deleting a running binary's file and having it read its
/// own `/proc/self/exe`: the kernel appends `" (deleted)"` to the link target, and
/// `std::env::current_exe()` does NOT strip it.
///
/// Both halves of the predicate earn their place. `!exists()` alone is already
/// correct in all three real cases -- live binary (path exists), unlinked one
/// (the suffixed path does not), and a file genuinely NAMED `"x (deleted)"` that
/// is alive (that path does exist). The suffix check is what keeps a stat that
/// fails for an unrelated reason, such as permissions, from reporting a live
/// binary as deleted; conjoined, the conservative answer is always "not deleted".
#[cfg(target_os = "linux")]
fn exe_is_deleted() -> Option<bool> {
    let p = std::fs::read_link("/proc/self/exe").ok()?;
    Some(path_reports_deleted(&p))
}

#[cfg(not(target_os = "linux"))]
fn exe_is_deleted() -> Option<bool> {
    None
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
        written_by: Some(current_writer()),
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

    /// Goes through the REAL writer, not a hand-built `IndexState`.
    ///
    /// That distinction is the whole point of the test. A fixture that constructs
    /// the provenance it then asserts on would pass whether or not
    /// `write_index_state_with_dirty` ever calls `current_writer()` — the same
    /// class that let `link_scan`'s cross-repo bucket ship inert under a green
    /// test, and that let this file's own `indexed_with_model` reader outlive its
    /// producer by three and a half months.
    #[test]
    fn the_writer_stamps_this_build_and_this_process() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_index_state_with_dirty(root, &[], ModelStamp::Record("m"), &[]).unwrap();

        let w = read_index_state(root)
            .expect("sidecar must parse")
            .written_by
            .expect("the writer must stamp itself, not leave None");

        assert_eq!(
            w.git_sha,
            env!("CODESCOUT_GIT_SHA"),
            "the stamp must be THIS build's sha, so a reader can compare against its own"
        );
        assert_eq!(
            w.pid,
            std::process::id(),
            "and THIS process's pid, so a reader can ask whether the writer still lives"
        );
    }

    /// Back-compat parse, in the same family as the three sibling
    /// `sidecar_written_before_*_existed_still_parses` tests above — the repo
    /// already treats this shape as worth pinning once per added field, because
    /// `read_index_state` reads a failed parse as "no sidecar", i.e. "never
    /// indexed" for the whole project. A parse regression here would not raise
    /// anywhere; it would silently present every pre-existing project as
    /// unindexed.
    ///
    /// Scoped honestly, after measuring what it does and does not catch: it does
    /// NOT pin the `#[serde(default)]` attribute — serde already maps a missing
    /// field to `None` for any `Option<T>`, so removing the attribute leaves this
    /// test green. Nor does it pin optionality; making `written_by` required is a
    /// compile error at three independent sites (the `Default` bound
    /// `#[serde(default)]` induces, the `.as_ref()` in `IndexStatus::call`, and the
    /// two fixture literals in `semantic_search.rs`). What it does pin is the
    /// round trip itself: a real on-disk document with no `written_by` key parses,
    /// and the fields that WERE recorded survive the schema addition.
    #[test]
    fn a_sidecar_written_before_written_by_existed_still_parses() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".codescout")).unwrap();
        fs::write(
            state_path(root),
            r#"{"last_indexed_commit":"abc","last_indexed_at":"2026-01-01T00:00:00Z",
                "schema_version":4,"indexed_with_model":"CodeRankEmbed"}"#,
        )
        .unwrap();

        let st = read_index_state(root).expect("a pre-field sidecar must still parse");
        assert_eq!(st.written_by, None, "absent means not recorded");
        assert_eq!(
            st.indexed_with_model.as_deref(),
            Some("CodeRankEmbed"),
            "and the fields that WERE recorded must survive the added field"
        );
    }

    /// Pins the direction of the deleted-binary predicate against ground truth,
    /// whichever way that truth points.
    ///
    /// **It used to assert `Some(false)` unconditionally**, on the premise that "the
    /// test binary is running from a file that exists". That premise is not the test's
    /// to make on a shared checkout: a peer's `cargo build` unlinks
    /// `target/debug/deps/codescout-<hash>` mid-suite, and this then failed saying the
    /// predicate was inverted — a true statement about the *environment*, misattributed
    /// to the code, alongside 13 `retrieval::sync` failures from the same cause.
    ///
    /// So it now compares the answer to the filesystem rather than to a constant. That
    /// makes it a WIRING test — `exe_is_deleted` reads `/proc/self/exe` and delegates to
    /// [`path_reports_deleted`] — and inversion is caught by
    /// [`path_reports_deleted_discriminates_all_three_real_cases`], which needs no
    /// assumption about this process at all. The pair covers what the single assertion
    /// covered, minus the dependency on who else is building.
    ///
    /// docs/issues/2026-09-02-a-peer-build-unlinks-the-test-binary-and-reds-fourteen-tests.md
    #[cfg(target_os = "linux")]
    #[test]
    fn a_live_binary_does_not_report_itself_deleted() {
        let answer = exe_is_deleted();
        assert!(
            answer.is_some(),
            "on Linux /proc/self/exe exists, so the check must return a definite \
             answer; None would mean 'could not tell'"
        );

        let exe = std::fs::read_link("/proc/self/exe").expect("linux has /proc/self/exe");
        let truth = path_reports_deleted(&exe);
        assert_eq!(
            answer,
            Some(truth),
            "exe_is_deleted must report what the filesystem says about its own \
             executable ({exe:?}). If truth is true here, a concurrent `cargo build` \
             unlinked this test binary mid-run — that is the environment, and the \
             predicate agreeing with it is CORRECT, not a regression."
        );
    }

    /// The predicate itself, over the three real cases — no dependency on this process.
    ///
    /// This is where inversion is actually caught. Each case is a distinct decision and
    /// the conjunction is what makes the third one right:
    ///
    /// - a live path (no suffix, exists) → not deleted;
    /// - a `" (deleted)"`-suffixed path that does NOT exist → deleted;
    /// - a file genuinely **named** `"x (deleted)"` that DOES exist → not deleted.
    ///
    /// Drop the `!p.exists()` conjunct and case 3 flips; drop the suffix check and a
    /// stat failing for an unrelated reason (permissions) reports a live binary as
    /// deleted. Neither mutation is caught by the wiring test above, which is why this
    /// is a separate test rather than an extra assertion in it.
    #[cfg(target_os = "linux")]
    #[test]
    fn path_reports_deleted_discriminates_all_three_real_cases() {
        let dir = tempfile::tempdir().unwrap();

        let live = dir.path().join("codescout");
        std::fs::write(&live, b"x").unwrap();
        assert!(
            !path_reports_deleted(&live),
            "a live binary path is not deleted"
        );

        let unlinked = dir.path().join("codescout (deleted)");
        assert!(
            !unlinked.exists(),
            "fixture precondition: this path must NOT exist, or case 2 is not the \
             case it claims to be"
        );
        assert!(
            path_reports_deleted(&unlinked),
            "a suffixed path that does not exist is the unlinked-binary case"
        );

        // Load-bearing: a real file whose NAME ends in " (deleted)". Deleting this
        // fixture leaves both remaining cases passing under a predicate that ignores
        // `!p.exists()` — the conjunction would then be untested and could be dropped
        // silently.
        let named = dir.path().join("weird (deleted)");
        std::fs::write(&named, b"x").unwrap();
        assert!(
            !path_reports_deleted(&named),
            "a file genuinely NAMED '… (deleted)' that exists is alive; the \
             conjunction with !p.exists() is what keeps this from being a false \
             positive"
        );
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
