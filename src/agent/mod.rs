//! Central orchestrator: manages projects, tool registry, and shared state.

pub(crate) mod build_check;
#[cfg(test)]
mod existing_root_tests;
mod write_guard;
#[allow(unused_imports)]
pub(crate) use write_guard::{
    acquire as acquire_write_guard, holder_record_path, open_lock_file, WriteGuard,
};

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::project::ProjectConfig;
use crate::library::registry::LibraryRegistry;
use crate::memory::semantic_store::SemanticMemoryStore;
use crate::memory::MemoryStore;
use crate::util::fs::to_forward_slash;
use crate::workspace::{discover_projects, DiscoveredProject, Project, ProjectState, Workspace};

/// State of the background index-build task spawned by `index_project`.
#[derive(Default, Clone)]
pub enum IndexingState {
    #[default]
    Idle,
    Running {
        done: usize,
        total: usize,
        eta_secs: Option<u64>,
    },
    Done {
        files_indexed: usize,
        files_deleted: usize,
        detail: String,
        total_files: usize,
        total_chunks: usize,
    },
    Failed(String),
}

/// Tracks the indexing lifecycle of a single external library.
#[derive(Debug)]
pub enum LibraryIndexState {
    Idle,
    FetchingSources { command: String },
    Indexing { done: usize, total: usize },
    Done { chunks: usize, version: String },
    Failed(String),
}

#[derive(Clone)]
pub struct Agent {
    pub inner: Arc<RwLock<AgentInner>>,
    /// Tracks the background index-build task. Stored outside AgentInner
    /// so callers only need a brief std::sync lock, not an async RwLock.
    pub indexing: Arc<std::sync::Mutex<IndexingState>>,
    /// Per-session dedup for library nudge hints (e.g. "index this library").
    /// Wrapped in Arc so Agent remains Clone.
    pub nudged_libraries: Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
    /// Limits concurrent embedding API calls to avoid overwhelming the embedding server.
    pub embedding_semaphore: Arc<tokio::sync::Semaphore>,
    /// Per-library indexing state (Idle / FetchingSources / Indexing / Done / Failed).
    pub library_index_states: Arc<std::sync::Mutex<HashMap<String, LibraryIndexState>>>,
    /// Abort handle for the current in-flight sync task (project reindex or
    /// library auto-index). `index(action='cancel')` takes this slot and calls
    /// `.abort()` to stop a running reindex without restarting the MCP server.
    /// Single slot — project and library sync rarely overlap; last-write-wins
    /// if they do. Per researcher MCP finding: dropping a `JoinHandle` does
    /// NOT cancel a task — only `.abort()` (or a `CancellationToken`) will.
    pub active_sync_abort: Arc<std::sync::Mutex<Option<tokio::task::AbortHandle>>>,
    /// Author-side build check: what this session has written, and the state of the
    /// background `cargo check` over it. Same `Arc<Mutex<_>>` shape as `indexing` above,
    /// and per-session for the same reason the server is — see
    /// [`build_check`][crate::agent::build_check] for why it needs no session identity.
    pub(crate) build_check: Arc<std::sync::Mutex<build_check::SessionBuildState>>,
    /// Lazily-constructed semantic memory store (Qdrant-backed).
    /// `OnceCell` so the first caller wins; later callers share the Arc.
    /// Wrapped in `Arc` so `Agent` remains `Clone`.
    pub(crate) semantic_memory: Arc<tokio::sync::OnceCell<Arc<dyn SemanticMemoryStore>>>,
    /// Lazily-constructed dense embedder for memory operations.
    /// Parallel design to `semantic_memory` — first caller builds, others
    /// share the Arc. Swappable in tests via `set_memory_embedder_for_test`
    /// so remember/recall paths can be exercised end-to-end without a live
    /// retrieval stack.
    pub(crate) memory_embedder:
        Arc<tokio::sync::OnceCell<Arc<dyn crate::retrieval::embedder::DenseEmbedder>>>,
    /// Memory embedders by the config that identifies them (url, model, dimension pin, key),
    /// for [`Agent::memory_embedder_for`]. Holds one entry per distinct embedding config the
    /// session's memory calls resolved to, normally one.
    #[allow(clippy::type_complexity)]
    pub(crate) memory_embedders_by_config: Arc<
        std::sync::Mutex<
            HashMap<
                (Option<String>, String, Option<usize>, Option<String>),
                Arc<dyn crate::retrieval::embedder::DenseEmbedder>,
            >,
        >,
    >,
    /// Test-only capture of the `RetrievalClient.embedder` seen by the single
    /// `RetrievalClient::from_env()` call inside `memory_embedder()`, taken at
    /// the exact point that `Arc` is about to be moved into `CodeDenseAdapter`.
    /// Exists solely so
    /// `memory_embedder_is_built_from_the_shared_code_embedder` can
    /// `Arc::ptr_eq` it against the embedder the returned `CodeDenseAdapter`
    /// actually wraps — proving the memory path shares the code-search
    /// embedder INSTANCE from that construction, not merely one with
    /// equivalent behaviour. Never read or written outside `#[cfg(test)]`.
    #[cfg(test)]
    pub(crate) test_seen_client_embedder:
        Arc<tokio::sync::OnceCell<Arc<dyn crate::retrieval::embedder::CodeEmbedder>>>,
    /// Test-only override for [`Agent::code_search`].
    ///
    /// Deliberately NOT the lazy-cache shape of `semantic_memory` / `memory_embedder`:
    /// production builds a fresh client per anchor pass today, keyed on a root that
    /// varies with `workspace_override`, and a shared `OnceCell` would silently pin
    /// the first caller's root for every later one. Test-gated so the production
    /// struct is unchanged.
    #[cfg(test)]
    pub(crate) code_search_override:
        Arc<std::sync::Mutex<Option<Arc<dyn crate::retrieval::search::CodeChunkSearch>>>>,
}

pub struct AgentInner {
    /// Registry of activated workspaces, keyed by canonical workspace root.
    /// `activate` clears the map and inserts the new default, so it drops every
    /// pinned resident; `ensure_resident` adds a pinned workspace alongside without
    /// clearing, so the map can hold several entries. Nothing evicts: a pinned
    /// workspace stays resident until the next `activate` or a restart. Per-`Workspace`
    /// locking and the eviction sweep are Phase 4b, deferred by decision. See
    /// docs/plans/2026-05-30-per-request-workspace-pinning.md § Phase 4b.
    pub workspaces: HashMap<PathBuf, Workspace>,
    /// Canonical root of the workspace that unpinned calls resolve to — the
    /// per-session default (what `activate` sets). Replaces the implicit
    /// "the one workspace" identity of the old single `workspace` slot.
    pub default_workspace_root: Option<PathBuf>,
    pub project_explicitly_activated: bool,
    /// True only after an in-session `activate` call. See
    /// `Agent::is_project_chosen_this_session` for why the startup flag above
    /// cannot answer that question.
    pub project_chosen_this_session: bool,
    pub home_root: Option<PathBuf>,
    /// Last `activate()` as (root, when, caller). Drives the concurrent-activation
    /// guard (`Agent::note_activation`): if a *different* root is activated
    /// under this shared server within a short window, the activate response
    /// carries a `concurrent_activation_warning`. The third element is the
    /// activating CALLER — `IC-17` layer 2, the owner field that lets the
    /// guard compare identity instead of substituting wall-clock proximity for
    /// it. It is the per-call principal (`crate::tools::session_key::current_caller`),
    /// NOT the process's `CLAUDE_CODE_SESSION_ID`: that is one value for the
    /// whole process, so a parent and every subagent it spawns share it and
    /// comparing it can only ever say "same caller". `None` means the caller did
    /// not identify itself, never that it was the same caller. See
    /// docs/issues/archive/2026-05-30-shared-server-global-active-project-race.md
    pub last_activation: Option<(PathBuf, std::time::Instant, Option<String>)>,
    /// The most recent time the default MOVED to a different root, and who moved
    /// it. See [`Displacement`] and `Agent::displaced_default_notice`.
    pub displacement: Option<Displacement>,
    /// caller -> `Displacement::epoch` it was last told about (or caused). Bounded
    /// by `DISPLACEMENT_TOLD_CAP`; clearing it only ever re-tells.
    pub displacement_told: HashMap<String, u64>,
}
/// One move of the default workspace to a different root — the record the
/// harmed party's notice is built from (`Agent::displaced_default_notice`).
///
/// The activation guard (`Agent::note_activation`) speaks to the party that MOVED
/// the slot. The party that is harmed is whoever makes an unpinned call afterwards
/// and resolves against a root it never chose, and it receives nothing. This
/// record is what lets the server tell it, because it names who moved the
/// default and to where.
#[derive(Debug, Clone)]
pub struct Displacement {
    /// 1-based, bumped on every move. A caller is told about each epoch once, so a
    /// NEW move re-arms the notice for everyone (the one-shot-per-conversation
    /// form was spent on the wrong episode — see `worktree_read_notice`).
    pub epoch: u64,
    pub from: Option<PathBuf>,
    pub to: PathBuf,
    /// The caller that moved it. `None` = it did not identify itself, in which case
    /// nobody can be told "someone ELSE moved it".
    pub by: Option<String>,
    pub at: std::time::Instant,
}

/// A notice owed to one caller, not yet spent. Carry `text` in a response, then hand
/// this back to [`Agent::mark_displaced_told`].
#[derive(Debug, Clone)]
pub struct DisplacedNotice {
    pub text: String,
    pub caller: String,
    pub epoch: u64,
}

/// Upper bound on `AgentInner::displacement_told`. A subagent fan-out is tens of
/// principals; past this the map is cleared, which re-tells and never loses a notice.
// cap-class: NOT_A_CAP — bounds the memory of which principals were told a displacement; past it the map clears and everyone is re-told, so no notice is lost and no result is shortened
const DISPLACEMENT_TOLD_CAP: usize = 256;

impl AgentInner {
    /// The workspace that unpinned calls resolve to (the per-session default).
    /// The ambient accessors below route through it. The selector-aware twins
    /// (`Agent::with_project_at`, `project_root_for`, ...) resolve a pinned
    /// workspace instead.
    pub fn default_workspace(&self) -> Option<&Workspace> {
        self.workspaces.get(self.default_workspace_root.as_ref()?)
    }

    /// Mutable twin of `default_workspace`. Clones the key first to avoid a
    /// split borrow of `default_workspace_root` and `workspaces`.
    pub fn default_workspace_mut(&mut self) -> Option<&mut Workspace> {
        let root = self.default_workspace_root.clone()?;
        self.workspaces.get_mut(&root)
    }

    /// Convenience: get `&ActiveProject` from the focused project of the
    /// default workspace.
    pub fn active_project(&self) -> Option<&ActiveProject> {
        self.default_workspace()?.focused_active()?.as_active()
    }

    /// Convenience: get `&mut ActiveProject` from the focused project of the
    /// default workspace.
    pub fn active_project_mut(&mut self) -> Option<&mut ActiveProject> {
        self.default_workspace_mut()?
            .focused_active_mut()?
            .as_active_mut()
    }
    /// Resolve the effective `read_only` for a root about to be activated.
    ///
    /// An explicit request wins at either root: `Some(false)` opens a foreign
    /// root, `Some(true)` protects the home one. Absent a request, home is
    /// read-write and a foreign root is protected.
    ///
    /// Expressed as `unwrap_or` rather than a `match` deliberately. The earlier
    /// form placed a `_ if is_home => false` guard arm ABOVE the explicit case,
    /// so it swallowed `Some(true)` — which made `read_only: true` inert at
    /// EVERY root, a foreign one already defaulting to protected. In this form an
    /// explicit value cannot be shadowed by a default, so that class of edit
    /// cannot recur here. Three copies of the rule existed when this was
    /// extracted, one of them dead; keep this the only one.
    ///
    /// See docs/issues/archive/2026-09-02-read-only-true-is-inert-at-every-root.md
    fn resolve_read_only(read_only: Option<bool>, is_home: bool) -> bool {
        read_only.unwrap_or(!is_home)
    }

    /// Assemble a `Workspace` for `root` from pre-loaded `ProjectResources`,
    /// under the caller's write lock. Reuses an already-resident project's
    /// write/file/dirty locks (so re-activation serializes correctly against
    /// in-flight writers). Pure read of `self` (home_root + workspaces) — it
    /// returns an owned `Workspace` and does not mutate the registry; the
    /// caller decides whether to clear+set-default (`activate`) or insert
    /// alongside (`ensure_resident`).
    fn build_workspace(
        &self,
        root: &Path,
        read_only: Option<bool>,
        res: ProjectResources,
    ) -> Workspace {
        let ProjectResources {
            config,
            config_stamp,
            memory,
            private_memory,
            library_registry,
            head_sha,
            discovered,
            fresh_file_lock,
        } = res;

        let is_home = self
            .home_root
            .as_ref()
            .map(|h| h.as_path() == root)
            .unwrap_or(true);
        let effective_read_only = AgentInner::resolve_read_only(read_only, is_home);

        // Re-activating the same root must keep the SAME write_lock, file_lock,
        // and dirty_files — otherwise an in-flight tool holding the old locks
        // does not serialize against new tools, and two writers can race.
        let existing = self.workspaces.values().find_map(|ws| {
            ws.projects.iter().find_map(|p| match &p.state {
                ProjectState::Activated(ap) if ap.root.as_path() == root => Some((
                    ap.write_lock.clone(),
                    ap.file_lock.clone(),
                    ap.dirty_files.clone(),
                )),
                _ => None,
            })
        });
        let (write_lock, file_lock, dirty_files) = existing.unwrap_or_else(|| {
            (
                Arc::new(tokio::sync::Mutex::new(())),
                fresh_file_lock,
                Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
            )
        });

        let active = ActiveProject {
            root: root.to_path_buf(),
            config,
            config_stamp,
            config_load_error: None,
            memory,
            private_memory,
            library_registry,
            dirty_files,
            read_only: effective_read_only,
            head_sha,
            has_git_remote: probe_has_git_remote(root),
            write_lock,
            file_lock,
            session_write_roots: Arc::new(std::sync::Mutex::new(Vec::new())),
        };

        let mut projects: Vec<Project> = Vec::new();
        let mut root_found = false;
        for dp in discovered {
            if dp.relative_root == std::path::Path::new(".") {
                root_found = true;
                projects.push(Project {
                    discovered: dp,
                    state: ProjectState::Activated(Box::new(active.clone())),
                });
            } else {
                projects.push(Project::new_dormant(dp));
            }
        }
        if !root_found {
            let root_dp = DiscoveredProject {
                id: crate::workspace::ROOT_PROJECT_ID.to_string(),
                relative_root: PathBuf::from("."),
                languages: vec![],
                manifest: None,
            };
            projects.insert(
                0,
                Project {
                    discovered: root_dp,
                    state: ProjectState::Activated(Box::new(active)),
                },
            );
        }

        Workspace::new(root.to_path_buf(), projects)
    }
}

/// A cheap fingerprint of `.codescout/project.toml`, taken when the config it
/// produced was read.
///
/// `None` (the holding field is `Option<ConfigStamp>`) means the file did not exist
/// at load time — a project running on defaults. A file appearing later therefore
/// reads as a change, which is the direction that matters: a project *gaining* a
/// `[security]` section must not be ignored.
///
/// **What it deliberately does not detect.** An edit preserving both mtime and length
/// is invisible here, and on a filesystem with coarse mtime granularity that includes
/// a same-size edit inside one tick. The alternative is hashing the file on every
/// gated call — a read and a parse per tool invocation, where a `stat` is neither.
/// The residual is narrower than the defect it replaces by exactly the width of that
/// coincidence, and it is stated here rather than left for a reader to infer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct ConfigStamp {
    modified: Option<std::time::SystemTime>,
    len: u64,
}

impl ConfigStamp {
    /// Fingerprint a project's config file: one `stat`, no read, no parse.
    ///
    /// **Take this BEFORE the read it describes, never after.** A write landing
    /// between the stat and the read then leaves a stamp describing the OLDER file,
    /// so the next comparison sees a difference and reloads — an extra reload, never
    /// a missed one. Stamping after the read fails the other way: the stamp would
    /// describe bytes the cached config was not built from, and that change would be
    /// swallowed permanently.
    pub(crate) fn of(root: &Path) -> Option<Self> {
        let md = std::fs::metadata(root.join(".codescout").join("project.toml")).ok()?;
        Some(Self {
            modified: md.modified().ok(),
            len: md.len(),
        })
    }
}

/// Active project state.
///
/// **Field-visibility contract:** all fields are `pub(crate)` rather than
/// private to keep `Agent::with_project(|p| ...)` closures ergonomic — they
/// receive `&ActiveProject` and read fields directly. Mutation invariants are
/// not enforced by getters; they are enforced by the borrow contract:
///
/// - External callers go through `Agent::with_project`, which hands out
///   `&ActiveProject` (shared, not mutable) — assignment to any field is a
///   compile error from outside this module.
/// - In-module mutation requires `AgentInner::active_project_mut()` and is
///   limited to a small number of well-named call sites in `agent/mod.rs`
///   (e.g. `activate`, `reload_config_if_project_toml`).
/// - Cross-cutting state (`dirty_files`, `write_lock`, `file_lock`, `session_write_roots`) is
///   `Arc<Mutex<_>>` / `Arc<File>` and self-protects via interior mutability;
///   external access is routed through `Agent` accessor methods such as
///   `mark_file_dirty`, `dirty_file_count`, `dirty_files_arc`, `add_session_write_root`,
///   `session_write_roots_snapshot`.
///
/// If codescout is ever split into multiple crates, fields with cross-field
/// invariants (`read_only`, `config`, `head_sha`/`has_git_remote`) should be
/// reduced to private and exposed through accessors. Until then, the type
/// system already enforces the contract — getters would add boilerplate
/// without adding safety.

#[derive(Clone)]
pub struct ActiveProject {
    pub(crate) root: PathBuf,
    pub(crate) config: ProjectConfig,
    /// Fingerprint of `project.toml` as it stood when `config` above was read.
    ///
    /// **Invariant: this moves with `config`, or not at all.** A config refreshed
    /// without its stamp re-reads on every gated call forever; a stamp refreshed
    /// without its config pins the staleness permanently, which is the defect the
    /// field exists to close. Both writers go through
    /// `ActiveProject::reload_config_from_disk`, and construction takes the pair out
    /// of `ProjectResources`, so neither can be set alone without adding a new site.
    pub(crate) config_stamp: Option<ConfigStamp>,
    /// Why the LAST reload of `project.toml` failed, or `None` when it loaded.
    ///
    /// Set by `reload_config_from_disk` when the file on disk cannot be loaded, cleared by
    /// the next reload that succeeds. It is what makes a failed reload loud: every tool
    /// response and every refused write carries it (`config::project::failed_reload_notice`)
    /// for as long as it is set. The stamp advances on a failure, so without this field a
    /// broken file would be reported once, by the log, and then never again.
    pub(crate) config_load_error: Option<String>,
    pub(crate) memory: MemoryStore,
    pub(crate) private_memory: MemoryStore,
    pub(crate) library_registry: LibraryRegistry,
    /// Tracks files written by tools in this session but not yet re-indexed.
    /// Wrapped in an Arc so index_project can capture it across a tokio::spawn
    /// boundary and clear it on successful completion.
    pub(crate) dirty_files: Arc<std::sync::Mutex<std::collections::HashSet<PathBuf>>>,
    /// When true, file writes are disabled regardless of security config.
    pub(crate) read_only: bool,
    /// Git HEAD SHA of the project at activation time. None for non-git projects.
    pub(crate) head_sha: Option<String>,
    /// Cached at activation: does this project have at least one git remote?
    /// Used by `current_capabilities` to gate GitHub-family tool exposure
    /// without re-opening the repo on every `list_tools` call. Refreshed on
    /// re-activation; does not track remotes added mid-session (rare enough
    /// to not justify invalidation complexity — user can re-activate).
    pub(crate) has_git_remote: bool,
    /// Async mutex serializing writes within this process.
    /// Acquired FIRST in the write-lock order (see agent::write_guard).
    pub(crate) write_lock: Arc<tokio::sync::Mutex<()>>,
    /// Shared file descriptor for the cross-process advisory lock at
    /// `.codescout/write.lock`. The flock is per-open-file-description, so a
    /// single File handle shared by every tool call in this process (via Arc)
    /// is sufficient — in-process ordering is handled by `write_lock` above.
    pub(crate) file_lock: Arc<std::fs::File>,
    /// Session-scoped directories approved for writing outside the project root.
    /// Managed by the `approve_write` tool; cleared on re-activation.
    pub(crate) session_write_roots: Arc<std::sync::Mutex<Vec<PathBuf>>>,
}

impl ActiveProject {
    /// Project name used as the namespace across stores (Qdrant `project_id`
    /// payload, sqlite-vec scoping, etc.). Comes from `project.toml`'s
    /// `[project] name = ...` field.
    pub fn project_id(&self) -> &str {
        &self.config.project.name
    }
    /// True when `.codescout/project.toml` no longer matches the fingerprint taken
    /// when this project's `config` was read.
    ///
    /// One `stat`, no read and no parse, so this is affordable before every gated
    /// call — which is the point. The config is loaded once when a project becomes
    /// resident and cached for the life of the process, and until this existed the
    /// only thing that re-read it was codescout's *own* write tools targeting the
    /// file. An edit made in an editor, by `Bash`, or by any other process was never
    /// picked up, so a project flipped to `file_write_enabled = false` or
    /// `shell_command_mode = "disabled"` on disk kept permitting, silently.
    pub(crate) fn config_is_stale(&self) -> bool {
        ConfigStamp::of(&self.root) != self.config_stamp
    }

    /// Re-read `project.toml` and re-stamp it, together.
    ///
    /// **On a load failure the stamp still advances, and the policy FAILS CLOSED.** Advancing
    /// the stamp anyway means a file that stays broken is parsed once rather than on every
    /// gated call — without it the retry is unbounded and paid by every tool invocation.
    /// Fixing the file changes its fingerprint, so the reload happens then.
    ///
    /// What this used to do, keep the last good config whole and say nothing, is looser than
    /// the file in the one case that matters: a tightening saved in the same edit as an
    /// entry the loader refuses (`file_write_enabled = false` next to a bad
    /// `extra_write_roots`) never applied, and no surface reported it. Now the failure is
    /// recorded in `config_load_error` (every response and refusal carries it until a reload
    /// succeeds) and the security section becomes the stricter of the previous policy and the
    /// file's own — see [`crate::config::project::SecuritySection::tighten_for_unloadable_file`].
    /// A malformed file still cannot relax a gate by falling back to permissive defaults.
    /// docs/issues/2026-10-17-an-invalid-project-toml-reload-is-ignored-silently-so-a-tightening-saved-with-it-never-applies.md
    pub(crate) fn reload_config_from_disk(&mut self) {
        let stamp = ConfigStamp::of(&self.root);
        match crate::config::project::ProjectConfig::load_or_default(&self.root) {
            Ok(fresh) => {
                self.config = fresh;
                self.config_load_error = None;
            }
            Err(e) => {
                let file =
                    crate::config::project::ProjectConfig::read_project_toml_table(&self.root);
                self.config.security.tighten_for_unloadable_file(
                    file.as_ref(),
                    &crate::config::project::GlobalSecurity::load(),
                );
                tracing::warn!(
                    root = %self.root.display(),
                    error = %format!("{e:#}"),
                    "project.toml failed to load; keeping the previous settings, tightened"
                );
                self.config_load_error = Some(format!("{e:#}"));
            }
        }
        self.config_stamp = stamp;
    }

    /// Absolute path to the project root on disk.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// Resolve `raw` to the canonical directory a project would live in, or REFUSE it.
///
/// Every path that makes a project resident (`Agent::new`, `Agent::activate`,
/// `Agent::ensure_resident`, the server's per-call `workspace=` pin) resolves its root
/// through here. They each used to do `canonicalize(..).unwrap_or(raw)`, which turns "that
/// path does not exist" into "that path is the root": a relative or missing pin was kept
/// as-is and the first touch BUILT a project there (`.gitignore` and
/// `.codescout/{memories, private-memories, usage.db, write.lock}`), so a placeholder such as
/// `workspace="x"` left a stray `x/.codescout/` in the server's cwd.
///
/// A root must be an existing DIRECTORY (`is_dir`, not `exists`: a file is no project root
/// either). Nothing is created on the refusal path. A path that exists resolves exactly as it
/// did, so an existing relative path still resolves against the server's cwd, as
/// `workspace(activate)` and `--project` do. `what` names the argument in the refusal.
///
/// docs/issues/2026-10-18-a-workspace-pin-naming-a-path-that-does-not-exist-creates-a-project-there.md
pub(crate) fn existing_project_dir(
    raw: PathBuf,
    what: &str,
) -> std::result::Result<PathBuf, crate::tools::RecoverableError> {
    // The io error kind is the only cheap way to tell "does not exist" from "cannot be
    // reached": a mode-000 parent makes `canonicalize` fail with a permission error, and
    // calling that "not an existing directory" sends the caller looking for a typo.
    let (root, denied) = match std::fs::canonicalize(&raw) {
        Ok(canonical) => (canonical, false),
        Err(e) => (raw, e.kind() == std::io::ErrorKind::PermissionDenied),
    };
    if root.is_dir() {
        return Ok(root);
    }
    // The path is the caller's text and lands in a response, a usage row and a log: capped
    // and escaped (`bounded_for_message`).
    let shown = bounded_for_message(&root.display().to_string());
    if denied {
        return Err(crate::tools::RecoverableError::with_hint(
            format!(
                "{what} '{shown}' cannot be reached: permission denied on the path or one of its parents"
            ),
            "Pass the absolute path of a project root this process may read, or fix the \
             permissions on it. Nothing was created at that path.",
        ));
    }
    // A relative path is resolved against the process's cwd, which the caller does not
    // control, so say where that is: it is the most likely reason a relative path is wrong.
    let cwd_note = if root.is_relative() {
        match std::env::current_dir() {
            Ok(cwd) => format!(
                " (a relative path resolves against the working directory, {})",
                bounded_for_message(&cwd.display().to_string())
            ),
            Err(_) => " (a relative path resolves against the working directory)".into(),
        }
    } else {
        String::new()
    };
    Err(crate::tools::RecoverableError::with_hint(
        format!("{what} '{shown}' is not an existing directory{cwd_note}"),
        "Pass the absolute path of an existing project root. Nothing was created at that path.",
    ))
}

/// A configuration file (the project's `project.toml` or the global `config.toml`) that the
/// loader could not turn into a configuration.
///
/// It is the TYPE that says "this is a configuration failure", set by this module at the one
/// place it calls the loader ([`load_project_config`]). Classifying by the loader's error type
/// or message text would couple the hint to wording another module owns, and the loader's own
/// error QUOTES the offending source line (a syntax error) or value (a type error), so a
/// project.toml that is a link to a secret file would be echoed to whoever pinned it. This
/// type therefore carries only what is safe to show: which file, and the line and column of a
/// syntax error. The original error is deliberately not kept.
#[derive(Debug)]
pub(crate) struct ConfigLoadFailed {
    /// The file that failed.
    pub(crate) path: PathBuf,
    /// The machine-wide config, not the project's own.
    pub(crate) global: bool,
    /// ` (line L, column C)` for a syntax error, digits only.
    pub(crate) location: Option<String>,
}

impl std::fmt::Display for ConfigLoadFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} '{}' could not be loaded{}",
            if self.global {
                "global config"
            } else {
                "project config"
            },
            bounded_for_message(&self.path.display().to_string()),
            self.location.as_deref().unwrap_or("")
        )
    }
}

impl std::error::Error for ConfigLoadFailed {}

/// The `ConfigLoadFailed` in `e`, if that is what it is.
fn config_load_failure(e: &anyhow::Error) -> Option<&ConfigLoadFailed> {
    e.chain()
        .find_map(|cause| cause.downcast_ref::<ConfigLoadFailed>())
}

/// ` (line L, column C)` from a TOML syntax error, or `None`. Only the digits of the first
/// line are read: the rest of that error is the quoted source, which must not be shown.
fn toml_location(e: &anyhow::Error) -> Option<String> {
    let shown = e
        .chain()
        .find_map(|cause| cause.downcast_ref::<toml::de::Error>())?
        .to_string();
    let rest = shown
        .lines()
        .next()?
        .strip_prefix("TOML parse error at line ")?;
    let (line, column) = rest.split_once(", column ")?;
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    (digits(line) && digits(column)).then(|| format!(" (line {line}, column {column})"))
}

/// `ProjectConfig::load_or_default(root)`, with a failure replaced by [`ConfigLoadFailed`].
///
/// Which file failed is found by asking the global loader: if it fails on its own, the global
/// config is the culprit; otherwise the project's `project.toml` is. Only on the error path.
fn load_project_config(root: &Path) -> Result<ProjectConfig> {
    ProjectConfig::load_or_default(root).map_err(|e| {
        let global = crate::config::global::GlobalConfig::load().is_err();
        let path = if global {
            crate::config::global::global_config_path().unwrap_or_default()
        } else {
            root.join(".codescout").join("project.toml")
        };
        anyhow::Error::new(ConfigLoadFailed {
            path,
            global,
            location: toml_location(&e),
        })
    })
}

/// Longest piece of caller-supplied text echoed into a refusal, in bytes.
// cap-class: NOT_A_CAP — bounds the echo of the caller's own input inside an error message, and the cut is marked in the text (`truncated, N bytes in all`); no result is shortened
const ECHO_MAX_BYTES: usize = 512;

/// `s` made safe to put in a message that is also stored in a usage row and written to a log:
/// control characters are escaped (`\u{a}`), so they cannot forge a line or colour, and the
/// text is cut to [`ECHO_MAX_BYTES`] at a character boundary, saying how long it was.
pub(crate) fn bounded_for_message(s: &str) -> String {
    let mut out = String::with_capacity(s.len().min(ECHO_MAX_BYTES) + 32);
    for c in s.chars() {
        let piece = if c.is_control() {
            format!("\\u{{{:x}}}", c as u32)
        } else {
            c.to_string()
        };
        if out.len() + piece.len() > ECHO_MAX_BYTES {
            out.push_str(&format!("… (truncated, {} bytes in all)", s.len()));
            return out;
        }
        out.push_str(&piece);
    }
    out
}

/// The recoverable refusal for a pin whose project could not be made resident, whatever the
/// route: the dispatch check, `ensure_resident` inside a tool body and the write gate all use
/// it, so the same failure reads the same on every path.
///
/// An error that is already a `RecoverableError` (the directory is not there) passes through
/// untouched. A [`ConfigLoadFailed`] points at the file that failed (the project's or the
/// global one) and shows nothing from inside it. Anything else gets the generic hint: a
/// directory this process may not enter used to be blamed on a project.toml never read.
/// docs/issues/2026-10-18-a-workspace-pin-naming-a-path-that-does-not-exist-creates-a-project-there.md
pub(crate) fn pin_resolution_refusal(
    root: &Path,
    e: anyhow::Error,
) -> crate::tools::RecoverableError {
    let e = match e.downcast::<crate::tools::RecoverableError>() {
        Ok(refusal) => return refusal,
        Err(e) => e,
    };
    let pin = bounded_for_message(&root.display().to_string());
    if let Some(config) = config_load_failure(&e) {
        let message = format!("workspace pin '{pin}' could not be resolved: {config}");
        if config.global {
            crate::tools::RecoverableError::with_hint(
                message,
                "Fix the global config named above (it applies to every project), or drop the \
                 workspace= argument to use the active project instead.",
            )
        } else {
            crate::tools::RecoverableError::with_hint(
                message,
                "Fix that project's .codescout/project.toml -- a [project] table with a `name` \
                 is required, and it must be valid and within its size and list limits -- or \
                 drop the workspace= argument to use the active project instead.",
            )
        }
    } else {
        crate::tools::RecoverableError::with_hint(
            format!(
                "workspace pin '{pin}' could not be resolved: {}",
                bounded_for_message(&format!("{e:#}"))
            ),
            "Check that the directory is readable and usable as a project root, or drop the \
             workspace= argument to use the active project instead.",
        )
    }
}

/// Read `workspace.toml` and return the discovery depth and exclude list.
///
/// From a linked worktree, reads through to the MAIN checkout when the worktree has
/// no `workspace.toml` of its own. That file is gitignored, so it never travels into
/// a worktree, and the plain fallback silently dropped `exclude_projects` there —
/// sub-project discovery then walked into every `tests/fixtures/*` (measured 2 -> 9
/// on this repo, `docs/issues/archive/2026-08-15-worktree-memory-set-and-subproject-topology-diverge.md`).
///
/// Read-through, not copy: carrying the settings needs no file-sync obligation. A
/// worktree's own `workspace.toml` still wins, so a deliberate per-worktree
/// configuration is never overridden. Falls back to `(3, vec![])` when neither
/// location has a readable, parseable file.
fn load_discover_settings(root: &std::path::Path) -> (usize, Vec<String>) {
    if let Some(settings) = read_discover_settings(root) {
        return settings;
    }
    if let Some(main) = crate::util::path_security::worktree_main_root(root) {
        if let Some(settings) = read_discover_settings(&main) {
            return settings;
        }
    }
    (3, vec![])
}

/// `Some` only when `root` holds a `workspace.toml` that both reads and parses;
/// `None` collapses "missing" and "unparseable" into one case, as the caller's
/// fallback always has.
fn read_discover_settings(root: &std::path::Path) -> Option<(usize, Vec<String>)> {
    let ws = crate::config::workspace::load_workspace_config(root)?;
    Some((ws.workspace.discovery_max_depth, ws.exclude_projects))
}

/// The most bytes `.codescout/system-prompt.md` may hold before it is not served. The file goes
/// into the `initialize` instructions, i.e. into the model's context; generated prompts are a few
/// KiB (the largest on this machine is under 8 KiB), so a file past this is not a prompt.
// cap-class: NOT_A_CAP — bounds how many bytes of an INPUT file are read; a file over it is treated as absent, no result is shortened and nothing is presented as complete
const MAX_SYSTEM_PROMPT_FILE_BYTES: u64 = 256 * 1024;

/// The project's system prompt as the `initialize` instructions serve it: the text of
/// `.codescout/system-prompt.md` when that is a regular file (a link is never served, as its
/// target can be any readable file), else `default_prompt` (the `system_prompt` of
/// `project.toml`, if any).
///
/// The type check on the path is not enough on its own: the file is then opened, and between the
/// two a FIFO can take its place, and the text had no size bound. The read is
/// `read_regular_text` at [`MAX_SYSTEM_PROMPT_FILE_BYTES`]; a FIFO, device or oversized file is
/// treated as an absent one (the default), logged, its content never echoed. A regular file that
/// fails to read (permissions, not UTF-8) is `None`, as it always was.
fn read_system_prompt_file(
    prompt_file: &std::path::Path,
    default_prompt: Option<String>,
) -> Option<String> {
    match std::fs::symlink_metadata(prompt_file) {
        Ok(m) if m.file_type().is_file() => {
            match crate::util::fs::read_regular_text(prompt_file, MAX_SYSTEM_PROMPT_FILE_BYTES) {
                Ok(text) => Some(text),
                Err(
                    e @ (crate::util::fs::BoundedReadError::NotRegular
                    | crate::util::fs::BoundedReadError::TooLarge { .. }),
                ) => {
                    tracing::warn!("{} ignored: {e}", prompt_file.display());
                    default_prompt
                }
                Err(_) => None,
            }
        }
        // A link or directory is never served: its text goes into the `initialize`
        // instructions. Treated as absent.
        Ok(_) => {
            tracing::warn!("{} is not a regular file; ignored", prompt_file.display());
            default_prompt
        }
        Err(_) => default_prompt,
    }
}

/// Resolve the short git HEAD SHA for a directory. Returns None if not a git
/// repo or if HEAD is unborn (no commits yet).
///
/// Uses libgit2 (no subprocess): on this project's locked-down Windows VDI,
/// every `CreateProcessW` is taxed by EDR injection, and a raw `git rev-parse`
/// with no timeout could hang activation outright. `short_id()` respects
/// `core.abbrev`, matching `git rev-parse --short HEAD` semantics. Mirrors the
/// sibling `probe_has_git_remote`, which already opens a libgit2 repo.
fn resolve_head_sha(root: &Path) -> Option<String> {
    let repo = git2::Repository::open(root).ok()?;
    let head = repo.revparse_single("HEAD").ok()?;
    let short = head.short_id().ok()?;
    short.as_str().map(str::to_string).filter(|s| !s.is_empty())
}

/// Does `root` contain a git repository with at least one configured remote?
/// Used at activation time to cache `has_git_remote` on `ActiveProject`.
fn probe_has_git_remote(root: &Path) -> bool {
    git2::Repository::open(root)
        .ok()
        .and_then(|repo| repo.remotes().ok())
        .map(|remotes| !remotes.is_empty())
        .unwrap_or(false)
}
/// Lock-free I/O products needed to assemble a `Workspace` for a root.
/// Loaded by `Agent::load_project_resources` (outside any lock), then consumed
/// by `AgentInner::build_workspace` under the write lock.
struct ProjectResources {
    config: ProjectConfig,
    /// Fingerprint of the file `config` was read from, taken BEFORE that read.
    /// Travels with it so `build_workspace` cannot assemble one without the other.
    config_stamp: Option<ConfigStamp>,
    memory: MemoryStore,
    private_memory: MemoryStore,
    library_registry: LibraryRegistry,
    head_sha: Option<String>,
    discovered: Vec<DiscoveredProject>,
    fresh_file_lock: Arc<std::fs::File>,
}

/// Derive a `PathSecurityConfig` from an active project: its security config
/// plus library paths, with writes disabled when the project is read-only.
/// Shared by `security_config` (default) and `security_config_for` (pinned).
fn project_security_config(p: &ActiveProject) -> crate::util::path_security::PathSecurityConfig {
    project_security_config_with(p, false)
}

/// [`project_security_config`], optionally with the caller's consent to write past a
/// read-only activation.
///
/// **Consent is a property of one call, never of the project.** A `workspace=` pin on a
/// write call is documented as resolving "that single call", but its consent used to be
/// recorded by flipping `p.read_only` on the registry entry every caller shares
/// (`ensure_resident(root, Some(false))`), so one pinned write lifted a read-only
/// activation that another caller had made on purpose, for everyone, and the lift
/// persisted. Here it is only an input to this derivation, so it cannot outlive the call
/// that supplied it and there is nothing to restore.
/// docs/issues/archive/2026-09-28-a-pinned-write-lifts-a-read-only-activation-for-every-caller.md
///
/// It lifts the READ-ONLY half only. A project whose own `project.toml` turns writes off
/// stays `ConfiguredOff` under consent: that is the durable setting, not a session choice a
/// pin may override (`call_tool_inner_honors_workspace_override_for_security_config`).
fn project_security_config_with(
    p: &ActiveProject,
    write_consent: bool,
) -> crate::util::path_security::PathSecurityConfig {
    let mut config = p.config.security.to_path_security_config();
    config.library_paths = p
        .library_registry
        .all()
        .iter()
        .map(|e| e.path.clone())
        .collect();
    let read_only = p.read_only && !write_consent;
    if read_only {
        config.file_write_enabled = false;
    }
    // Record WHY writes are off, and for which project, so `check_tool_access`
    // can state a cause instead of hedging between two.
    //
    // Precedence is deliberate and is read BEFORE the flag above is mutated:
    // a project whose own config disables writes stays `ConfiguredOff` even
    // when it is also read-only, because that is the cause whose remedy is
    // different. Telling someone to re-activate writable when their
    // project.toml turns writes off is advice that costs a call and fails.
    // docs/issues/archive/2026-08-26-workspace-read-only-flips-mid-session.md
    let cause = crate::util::path_security::WriteBlockCause::classify(
        p.config.security.file_write_enabled,
        read_only,
    );
    config.write_block = cause.map(|cause| crate::util::path_security::WriteBlock {
        root: p.root.clone(),
        cause,
    });
    config
}

// ---------------------------------------------------------------------------
// Lifecycle & activation
// ---------------------------------------------------------------------------
impl Agent {
    pub async fn new(project: Option<PathBuf>) -> Result<Self> {
        // Tests and library users that bypass main() reach here without the
        // crypto provider installed — install it idempotently before any TLS
        // (Qdrant gRPC, dense embedder HTTP) is touched.
        crate::install_default_crypto_provider();

        let (workspace, home_root) = if let Some(raw) = project {
            // Canonicalize so home_root is always an absolute path.  This prevents
            // path-form drift when activate_project(".") later canonicalizes its
            // argument and compares against home_root.
            let root = existing_project_dir(raw, "project root")?;
            // Stamped before the read it describes — see `ConfigStamp::of`.
            let config_stamp = ConfigStamp::of(&root);
            let config = ProjectConfig::load_or_default(&root)?;
            let memory = MemoryStore::open(&root)?;
            let private_memory = MemoryStore::open_private(&root)?;
            let registry_path = root.join(".codescout").join("libraries.json");
            let library_registry = LibraryRegistry::load(&registry_path).unwrap_or_default();
            let home = root.clone();

            let active = ActiveProject {
                root: root.clone(),
                config,
                config_stamp,
                config_load_error: None,
                memory,
                private_memory,
                library_registry,
                dirty_files: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
                read_only: false,
                head_sha: resolve_head_sha(&root),
                has_git_remote: probe_has_git_remote(&root),
                write_lock: Arc::new(tokio::sync::Mutex::new(())),
                file_lock: open_lock_file(&root)
                    .with_context(|| format!("failed to open write.lock for {}", root.display()))?,
                session_write_roots: Arc::new(std::sync::Mutex::new(Vec::new())),
            };

            // Discover sub-projects; root project is always included.
            // Respect depth and exclude settings from workspace.toml if it exists.
            // Walked on a blocking thread — `ignore::WalkBuilder` + manifest
            // reads do synchronous fs I/O that must not stall the Tokio runtime.
            let (discover_depth, discover_exclude) = load_discover_settings(&root);
            let discovered = {
                let root = root.clone();
                let exclude = discover_exclude.clone();
                tokio::task::spawn_blocking(move || {
                    discover_projects(&root, discover_depth, &exclude)
                })
                .await
                .map_err(|e| anyhow::anyhow!("discover_projects task failed: {e}"))?
            };
            let mut projects: Vec<Project> = Vec::new();

            // Find if the root project was discovered (relative_root == ".")
            let mut root_found = false;
            for dp in discovered {
                if dp.relative_root == std::path::Path::new(".") {
                    root_found = true;
                    projects.push(Project {
                        discovered: dp,
                        state: ProjectState::Activated(Box::new(active.clone())),
                    });
                } else {
                    projects.push(Project::new_dormant(dp));
                }
            }

            // If root was not discovered (e.g. no manifest), synthesize it
            if !root_found {
                let root_dp = DiscoveredProject {
                    id: crate::workspace::ROOT_PROJECT_ID.to_string(),
                    relative_root: PathBuf::from("."),
                    languages: vec![],
                    manifest: None,
                };
                projects.insert(
                    0,
                    Project {
                        discovered: root_dp,
                        state: ProjectState::Activated(Box::new(active)),
                    },
                );
            }

            let ws = Workspace::new(root, projects);
            (Some(ws), Some(home))
        } else {
            (None, None)
        };

        // A project provided at startup (via --project or CWD) is treated as explicitly
        // activated — the server operator already chose the write target.
        let project_explicitly_activated = workspace.is_some();
        let default_workspace_root = workspace.as_ref().map(|ws| ws.root.clone());
        let workspaces = match workspace {
            Some(ws) => {
                let mut m = HashMap::new();
                m.insert(ws.root.clone(), ws);
                m
            }
            None => HashMap::new(),
        };

        Ok(Self {
            inner: Arc::new(RwLock::new(AgentInner {
                workspaces,
                default_workspace_root,
                project_explicitly_activated,
                // Startup never counts as a choice, however the root was found.
                project_chosen_this_session: false,
                home_root,
                last_activation: None,
                displacement: None,
                displacement_told: HashMap::new(),
            })),
            indexing: Arc::new(std::sync::Mutex::new(IndexingState::Idle)),
            nudged_libraries: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
            embedding_semaphore: Arc::new(tokio::sync::Semaphore::new(2)),
            library_index_states: Arc::new(std::sync::Mutex::new(HashMap::new())),
            active_sync_abort: Arc::new(std::sync::Mutex::new(None)),
            build_check: Arc::new(std::sync::Mutex::new(
                build_check::SessionBuildState::default(),
            )),
            semantic_memory: Arc::new(tokio::sync::OnceCell::new()),
            memory_embedder: Arc::new(tokio::sync::OnceCell::new()),
            memory_embedders_by_config: Arc::new(std::sync::Mutex::new(HashMap::new())),
            #[cfg(test)]
            test_seen_client_embedder: Arc::new(tokio::sync::OnceCell::new()),
            #[cfg(test)]
            code_search_override: Arc::new(std::sync::Mutex::new(None)),
        })
    }

    /// Activate a project by path, replacing the current workspace as the
    /// per-session default. Pinned workspaces are added via `ensure_resident`
    /// without disturbing the default.
    pub async fn activate(&self, root: PathBuf, read_only: Option<bool>) -> Result<()> {
        // Canonicalize up-front so every downstream consumer (and the registry
        // key) sees the same absolute path. Without this, activate(".") would
        // compare unequal to Agent::new's canonicalized home_root, making
        // is_home false on the first re-activation and flipping to read-only.
        let root = existing_project_dir(root, "project root")?;
        let res = Self::load_project_resources(&root).await?;
        {
            let mut inner = self.inner.write().await;
            // build_workspace computes is_home / read_only and reuses an
            // existing root's locks, all under this write lock (no TOCTOU).
            let ws = inner.build_workspace(&root, read_only, res);
            if inner.home_root.is_none() {
                inner.home_root = Some(root.clone());
            }
            // Clear + reinsert: `activate` replaces the whole registry, pinned
            // residents included (the previous single-slot drop-and-replace).
            // `ensure_resident` adds pinned entries alongside without clearing.
            inner.workspaces.clear();
            inner.workspaces.insert(root.clone(), ws);
            inner.default_workspace_root = Some(root);
            inner.project_explicitly_activated = true;
            inner.project_chosen_this_session = true;
        }
        Ok(())
    }
    /// Load all lock-free I/O for a project root (config, memory, library
    /// registry, sub-project discovery, write-lock file). Shared by `activate`
    /// and `ensure_resident`; the products are assembled into a `Workspace`
    /// under the write lock by `AgentInner::build_workspace`.
    async fn load_project_resources(root: &Path) -> Result<ProjectResources> {
        // Stamped BEFORE the read it describes: a write landing between the two then
        // leaves a stamp describing the OLDER file, so the next gated call reloads.
        // The reverse order swallows that write permanently. See `ConfigStamp::of`.
        let config_stamp = ConfigStamp::of(root);
        let config = load_project_config(root)?;
        let memory = MemoryStore::open(root)?;
        let private_memory = MemoryStore::open_private(root)?;
        let registry_path = root.join(".codescout").join("libraries.json");
        let library_registry = LibraryRegistry::load(&registry_path).unwrap_or_default();
        let head_sha = resolve_head_sha(root);
        let (discover_depth, discover_exclude) = load_discover_settings(root);
        let discovered = {
            let root = root.to_path_buf();
            let exclude = discover_exclude.clone();
            tokio::task::spawn_blocking(move || discover_projects(&root, discover_depth, &exclude))
                .await
                .map_err(|e| anyhow::anyhow!("discover_projects task failed: {e}"))?
        };
        let fresh_file_lock = write_guard::open_lock_file(root)
            .with_context(|| format!("failed to open write.lock for {}", root.display()))?;
        Ok(ProjectResources {
            config,
            config_stamp,
            memory,
            private_memory,
            library_registry,
            head_sha,
            discovered,
            fresh_file_lock,
        })
    }

    /// Ensure `root` is resident in the registry (load + cache on miss) WITHOUT
    /// clearing the registry or changing `default_workspace_root`. Lets a
    /// per-request pinned workspace be resolved alongside the default. Pinned,
    /// non-home workspaces default to read-only.
    ///
    /// Idempotent, and it NEVER changes an already-resident entry. It used to take
    /// `read_only: Option<bool>` and, on `Some(false)`, flip a resident read-only entry to
    /// writable so a pinned write could proceed. That entry is shared by every caller, so one
    /// pinned write lifted a read-only activation another caller had made on purpose, for
    /// everyone, and the lift persisted — the opposite of a pin's documented "resolves that
    /// single call". The pin's consent to write is now applied per call, in
    /// [`Agent::security_config_for_write`], and nothing here records it.
    /// docs/issues/archive/2026-09-28-a-pinned-write-lifts-a-read-only-activation-for-every-caller.md
    ///
    /// Still does not require a full `activate`, which would clear every other resident
    /// workspace — see `Agent::activate`.
    ///
    /// `root` must be an existing directory, or this refuses with a `RecoverableError` before
    /// anything is created ([`existing_project_dir`]). It used to keep a missing path as the
    /// root and build a project there on first touch.
    /// docs/issues/2026-10-18-a-workspace-pin-naming-a-path-that-does-not-exist-creates-a-project-there.md
    pub async fn ensure_resident(&self, root: PathBuf) -> Result<()> {
        let root = existing_project_dir(root, "workspace pin")?;
        if self.inner.read().await.workspaces.contains_key(&root) {
            return Ok(());
        }
        let res = Self::load_project_resources(&root).await?;
        let mut inner = self.inner.write().await;
        // Re-check under the write lock — another caller may have inserted it
        // while we did the lock-free I/O.
        if inner.workspaces.contains_key(&root) {
            return Ok(());
        }
        let ws = inner.build_workspace(&root, None, res);
        inner.workspaces.insert(root, ws);
        Ok(())
    }

    /// Run a closure with a read-lock on the project resolved by an optional
    /// workspace pin. `Some(root)` → that workspace (resident-on-demand);
    /// `None` → the session default. The closure receives the workspace's
    /// focused `&ActiveProject`. Level-2 sub-project pinning within a pinned
    /// workspace is not yet wired (read tools pin at workspace granularity).
    pub async fn with_project_at<F, T>(&self, workspace_override: Option<&Path>, f: F) -> Result<T>
    where
        F: FnOnce(&ActiveProject) -> Result<T>,
    {
        if let Some(root) = workspace_override {
            self.ensure_resident(root.to_path_buf()).await?;
        }
        let inner = self.inner.read().await;
        let ws = match workspace_override {
            Some(root) => {
                let key = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
                inner.workspaces.get(&key).ok_or_else(|| {
                    anyhow::anyhow!("pinned workspace not resident: {}", key.display())
                })?
            }
            None => inner.default_workspace().ok_or_else(|| {
                crate::tools::RecoverableError::with_hint(
                    "No active project. Use activate_project first.",
                    "Call activate_project(\"/path/to/project\") to set the active project.",
                )
            })?,
        };
        let project = ws
            .focused_active()
            .and_then(|p| p.as_active())
            .ok_or_else(|| anyhow::anyhow!("workspace has no active focused project"))?;
        f(project)
    }

    /// Pinned twin of `project_root`: focused root of the workspace named by
    /// `workspace_override` (resident-on-demand), or the default if `None`.
    pub async fn project_root_for(&self, workspace_override: Option<&Path>) -> Option<PathBuf> {
        self.with_project_at(workspace_override, |p| Ok(p.root().to_path_buf()))
            .await
            .ok()
    }

    /// Pinned twin of `security_config`: security config of the workspace named
    /// by `workspace_override` (resident-on-demand), or defaults if `None`/none.
    ///
    /// Re-reads `project.toml` first if it has changed on disk since the cached
    /// config was built. See `security_config` for why that check lives at this
    /// seam rather than at `ensure_resident`.
    ///
    /// This is the config for everything EXCEPT deciding whether a write may proceed. A
    /// caller about to decide that must use [`Agent::security_config_for_write`], which is
    /// the one place a pin's consent is applied.
    pub async fn security_config_for(
        &self,
        workspace_override: Option<&Path>,
    ) -> crate::util::path_security::PathSecurityConfig {
        self.security_config_for_call(workspace_override, false)
            .await
    }

    /// The config a WRITE call is gated on: [`Agent::security_config_for`], except that a
    /// `workspace=` pin is the caller's consent to write to the project it names, **for this
    /// one call**.
    ///
    /// Consent lifts a read-only ACTIVATION only. Nothing is written to the shared registry
    /// entry, so it cannot outlive the call and there is nothing to restore; an unpinned
    /// caller, or another pinned caller, still meets the guard that was set. With no pin
    /// there is no consent: the default project's own read-only state binds its callers.
    /// A project that disables writes in its own `project.toml` is not lifted either.
    ///
    /// Both places that decide a write — the server's access gate and `approve_write` — use
    /// this, so the two cannot disagree about what a pin means.
    /// docs/issues/archive/2026-09-28-a-pinned-write-lifts-a-read-only-activation-for-every-caller.md
    pub async fn security_config_for_write(
        &self,
        workspace_override: Option<&Path>,
    ) -> crate::util::path_security::PathSecurityConfig {
        self.security_config_for_call(workspace_override, workspace_override.is_some())
            .await
    }

    /// Why the project this call resolves to could not load its last `project.toml` edit, or
    /// `None` when it loaded. Rendered by [`crate::config::project::failed_reload_notice`] on
    /// the next response and on every refusal.
    ///
    /// Reads the field the gate just refreshed: both callers run it after
    /// `security_config_for*`, which is where a stale file is re-read, so this never reports
    /// the state from before the edit it should describe.
    pub async fn config_load_error_for(&self, workspace_override: Option<&Path>) -> Option<String> {
        self.with_project_at(workspace_override, |p| Ok(p.config_load_error.clone()))
            .await
            .ok()
            .flatten()
    }

    async fn security_config_for_call(
        &self,
        workspace_override: Option<&Path>,
        write_consent: bool,
    ) -> crate::util::path_security::PathSecurityConfig {
        // Fast path: derive under the read lock and learn, in the SAME pass, whether
        // project.toml moved under us. One `stat`, no read, no parse, no write lock.
        let probe = self
            .with_project_at(workspace_override, |p| {
                Ok((
                    project_security_config_with(p, write_consent),
                    p.config_is_stale(),
                ))
            })
            .await;
        match probe {
            Ok((config, false)) => config,
            Ok((_, true)) => {
                // Slow path only: the file really changed. Re-read under the write
                // lock, then re-derive — never reuse the config probed above, which
                // was built from the bytes we just learned are stale.
                let _ = self
                    .with_project_at_mut(workspace_override, |p| {
                        p.reload_config_from_disk();
                        Ok(())
                    })
                    .await;
                self.with_project_at(workspace_override, |p| {
                    Ok(project_security_config_with(p, write_consent))
                })
                .await
                .unwrap_or_default()
            }
            Err(_) => crate::util::path_security::PathSecurityConfig::default(),
        }
    }
    /// Pinned twin of `require_project_root`: focused root of the workspace
    /// named by `workspace_override` (resident-on-demand), or a recoverable
    /// "no active project" error if none resolvable.
    pub async fn require_project_root_for(
        &self,
        workspace_override: Option<&Path>,
    ) -> Result<PathBuf> {
        self.with_project_at(workspace_override, |p| Ok(p.root().to_path_buf()))
            .await
    }

    /// Pinned twin of `mark_file_dirty`: marks a file dirty in the workspace
    /// named by `workspace_override` (resident-on-demand) or the session
    /// default. Silently no-ops if no project resolves, matching the ambient
    /// contract — by the time a write tool calls this it has already resolved
    /// the same pin via `require_project_root_for`, so the workspace is resident.
    ///
    /// Also the author-side build check's trigger, and deliberately so. Every write
    /// path in the tool layer already calls this — nine call sites across
    /// `create_file`, `edit_file` and `edit_code` — so hanging the trigger here means a
    /// future write path cannot acquire the check by remembering to add a line. It
    /// either marks the file dirty, or it has a louder bug than a missing notice.
    /// (CLAUDE.md § Testing Discipline: guard the SITE, not the feature.)
    pub async fn mark_file_dirty_for(&self, workspace_override: Option<&Path>, path: PathBuf) {
        self.note_source_write_for(workspace_override, &path).await;
        let _ = self
            .with_project_at(workspace_override, |p| {
                p.dirty_files
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(path);
                Ok(())
            })
            .await;
    }

    /// Author-side build check: note that this session wrote `path`, and start a
    /// background `cargo check` if this checkout is shared and the debounce has elapsed.
    ///
    /// Silently no-ops when no project resolves, matching `mark_file_dirty_for`'s
    /// contract above. Every other gate — Rust-only, shared-checkout, debounce, cargo
    /// lock — lives in [`build_check`][crate::agent::build_check] so it is testable
    /// without a tool.
    pub async fn note_source_write_for(&self, workspace_override: Option<&Path>, path: &Path) {
        let root = self
            .with_project_at(workspace_override, |p| Ok(p.root.clone()))
            .await;
        if let Ok(root) = root {
            build_check::on_source_write(&self.build_check, &root, path);
        }
    }

    /// The author-side build notice for this response, if one is pending. Consumes it,
    /// so a break is announced once rather than on every later call.
    pub fn take_build_notice(&self) -> Option<String> {
        build_check::pending_notice(&self.build_check)
    }

    /// Pinned twin of `add_session_write_root`.
    pub async fn add_session_write_root_for(
        &self,
        workspace_override: Option<&Path>,
        path: PathBuf,
    ) {
        let _ = self
            .with_project_at(workspace_override, |p| {
                p.session_write_roots
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(path);
                Ok(())
            })
            .await;
    }

    /// Pinned twin of `session_write_roots_snapshot`. Empty Vec if no project resolves.
    pub async fn session_write_roots_snapshot_for(
        &self,
        workspace_override: Option<&Path>,
    ) -> Vec<PathBuf> {
        self.with_project_at(workspace_override, |p| {
            Ok(p.session_write_roots
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone())
        })
        .await
        .unwrap_or_default()
    }

    /// Pinned twin of `dirty_files_arc`. None if no project resolves.
    pub async fn dirty_files_arc_for(
        &self,
        workspace_override: Option<&Path>,
    ) -> Option<Arc<std::sync::Mutex<std::collections::HashSet<PathBuf>>>> {
        self.with_project_at(workspace_override, |p| Ok(p.dirty_files.clone()))
            .await
            .ok()
    }

    /// Mutable twin of `with_project_at`. Runs the closure with a `&mut
    /// ActiveProject` for the workspace named by `workspace_override`
    /// (resident-on-demand) or the session default. For write tools that mutate
    /// `ActiveProject` fields *directly* (e.g. `p.config = …`,
    /// `library_registry.register`) rather than via the `Arc<Mutex>`
    /// interior-mutability fields — those use the read `with_project_at`.
    ///
    /// Holds the single `AgentInner` write lock for the closure's duration; the
    /// closure MUST stay non-blocking (no `.await` on a per-project lock) per
    /// `## Phase 4 — Lock-Ordering Proof`. Moving this onto a per-`Workspace` lock
    /// is Phase 4b, deferred by decision (plan § Phase 4b), so writes to different
    /// workspaces still serialize here.
    pub async fn with_project_at_mut<F, T>(
        &self,
        workspace_override: Option<&Path>,
        f: F,
    ) -> Result<T>
    where
        F: FnOnce(&mut ActiveProject) -> Result<T>,
    {
        if let Some(root) = workspace_override {
            self.ensure_resident(root.to_path_buf()).await?;
        }
        let mut inner = self.inner.write().await;
        let ws = match workspace_override {
            Some(root) => {
                let key = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
                inner.workspaces.get_mut(&key).ok_or_else(|| {
                    anyhow::anyhow!("pinned workspace not resident: {}", key.display())
                })?
            }
            None => {
                let root = inner.default_workspace_root.clone().ok_or_else(|| {
                    crate::tools::RecoverableError::with_hint(
                        "No active project. Use activate_project first.",
                        "Call activate_project(\"/path/to/project\") to set the active project.",
                    )
                })?;
                inner
                    .workspaces
                    .get_mut(&root)
                    .ok_or_else(|| anyhow::anyhow!("default workspace not resident"))?
            }
        };
        let project = ws
            .focused_active_mut()
            .and_then(|p| p.as_active_mut())
            .ok_or_else(|| anyhow::anyhow!("workspace has no active focused project"))?;
        f(project)
    }

    /// Pinned twin of `reload_config_if_project_toml`.
    pub async fn reload_config_if_project_toml_for(
        &self,
        workspace_override: Option<&Path>,
        path: &std::path::Path,
    ) {
        let _ = self
            .with_project_at_mut(workspace_override, |p| {
                let toml_path = p.root.join(".codescout").join("project.toml");
                if path == toml_path {
                    p.reload_config_from_disk();
                }
                Ok(())
            })
            .await;
    }

    /// Window within which activating a *different* root counts as concurrent
    /// contention (a subagent racing the shared slot) rather than a normal
    /// sequential re-activation by one linear session.
    const CONCURRENT_ACTIVATION_WINDOW: std::time::Duration = std::time::Duration::from_secs(5);

    /// Pure decision for the concurrent-activation guard. Returns a warning when
    /// `new_root` rapidly replaces a *different* recently-activated root — the
    /// fingerprint of concurrent multi-workspace use on a single shared server
    /// (parallel subagents that each `activate` a different workspace). Same-root
    /// re-activation and slow sequential switches (outside `window`) are silent.
    ///
    /// So is a switch whose previous activation carries the SAME session id: that is
    /// one linear session using the documented cross-project pattern, and warning on
    /// it was a systematic false positive rather than noise
    /// (`docs/issues/archive/2026-09-02-the-concurrent-activation-guard-substitutes-proximity-for-identity.md`).
    /// Identity only ever silences — an unknown id on either side keeps the old
    /// proximity behaviour.
    /// See docs/issues/archive/2026-05-30-shared-server-global-active-project-race.md
    fn concurrent_switch_warning(
        prev: Option<(&std::path::Path, std::time::Duration, Option<&str>)>,
        new_root: &std::path::Path,
        new_sid: Option<&str>,
        window: std::time::Duration,
    ) -> Option<String> {
        match prev {
            Some((prev_root, since, prev_sid)) if prev_root != new_root && since < window => {
                // IDENTITY MAY ONLY SILENCE A WARNING, NEVER CREATE ONE.
                //
                // Proximity is a proxy for contention: it correlates with it and does
                // not imply it. One session following the documented cross-project
                // pattern (activate foreign -> work -> activate home) inside the window
                // is a SYSTEMATIC false positive, not noise, and a guard that cries on
                // the documented path trains its reader to ignore it.
                //
                // The asymmetry is deliberate. When either side is unidentified, two
                // anonymous callers are indistinguishable from one, so we fall back to
                // the proximity behaviour this guard shipped with: a missed warning on
                // a real race is the harm it exists for, a spurious one merely annoys.
                if let (Some(p), Some(n)) = (prev_sid, new_sid) {
                    if p == n {
                        return None;
                    }
                }
                // Name the OTHER caller when we know it. "another caller" leaves a
                // reader with nobody to ask; a sessionId is an address they can reach
                // (see CLAUDE.md § Reaching a Peer Session).
                let by = match prev_sid {
                    Some(s) => format!(", the previous activation by session {s}"),
                    None => String::new(),
                };
                Some(format!(
                    "active project switched from {} to {} {:?} ago{} — another caller \
                     (e.g. a concurrent subagent) shares this server's single \
                     active-project slot, so reads may resolve against the wrong \
                     workspace. Fix: pass workspace=<absolute path> on each tool call \
                     to pin resolution per-request instead of activating. For fully \
                     independent parallel work, separate client windows also \
                     isolate (separate processes = separate slots).",
                    prev_root.display(),
                    new_root.display(),
                    since,
                    by
                ))
            }
            _ => None,
        }
    }

    /// Record this activation and return a warning if it rapidly replaced a
    /// *different* recently-activated root. Best-effort drift signal — it cannot
    /// prevent the race (the active project is process-global shared state), only
    /// surface it. The real fix is per-request workspace pinning; see the bug file.
    ///
    /// `caller` is the per-call principal (`session_key::current_caller`), and it
    /// is passed in rather than read here on purpose. This used to derive the
    /// identity from `CLAUDE_CODE_SESSION_ID`, which is ONE value for the whole
    /// process — a parent and every subagent share it — so under Claude Code the
    /// "same caller" comparison was always true and the guard was silent on exactly
    /// the race it exists for (measured 2026-10-16: two rapid activations of
    /// different roots, process env set, both returned `None`).
    ///
    /// Also records the move for the harmed party's notice
    /// ([`displaced_default_notice`](Self::displaced_default_notice)): this
    /// function is the one production writer of activation records.
    pub async fn note_activation(
        &self,
        root: &std::path::Path,
        caller: Option<&str>,
    ) -> Option<String> {
        let mut inner = self.inner.write().await;
        let prev = inner
            .last_activation
            .as_ref()
            .map(|(p, at, s)| (p.as_path(), at.elapsed(), s.as_deref()));
        let warning =
            Self::concurrent_switch_warning(prev, root, caller, Self::CONCURRENT_ACTIVATION_WINDOW);
        // What the default was BEFORE this activation: the previous record's root,
        // else the home project the server started on. Re-activating the root that is
        // already the default moves nothing, so it is not a displacement.
        let from = inner
            .last_activation
            .as_ref()
            .map(|(p, _, _)| p.clone())
            .or_else(|| inner.home_root.clone());
        if from.as_deref() != Some(root) {
            let epoch = inner.displacement.as_ref().map_or(1, |d| d.epoch + 1);
            inner.displacement = Some(Displacement {
                epoch,
                from,
                to: root.to_path_buf(),
                by: caller.map(str::to_string),
                at: std::time::Instant::now(),
            });
        }
        inner.last_activation = Some((
            root.to_path_buf(),
            std::time::Instant::now(),
            caller.map(str::to_string),
        ));
        warning
    }

    /// The notice owed to the party HARMED by a move of the default workspace:
    /// whoever makes an unpinned call afterwards and resolves against a root it
    /// did not choose.
    ///
    /// [`note_activation`](Self::note_activation)'s warning is attached to the
    /// response of the call that PERFORMED the switch, so it reaches the switcher.
    /// The harmed party — in the archived incidents the controller, whose own writes
    /// were refused minutes after a subagent activated — received nothing. This is the
    /// delivery to it: `caller` is the party making the unpinned call now.
    ///
    /// Told when ALL of these hold; every other case is silent:
    /// - both sides are identified, and they differ. "Someone ELSE moved it" is a
    ///   claim, so unlike the activation guard — where identity may only silence a
    ///   warning — identity is needed to CREATE this one. A caller with no identity
    ///   might be the mover, and a mover with none cannot be named.
    /// - the default is not home. Back at home nothing is displaced, and the
    ///   overwhelmingly common session (activate home once, subagents read it) never
    ///   sees this. "The default" is where an unpinned call resolves: the FOCUSED
    ///   project of the default workspace, so a by-id project switch counts.
    /// - this caller has not already been told about THIS move. The unit is the move
    ///   (`Displacement::epoch`), not the conversation, so a later move re-arms it.
    ///
    /// **This only PEEKS.** It never marks the caller told: the caller is told when a
    /// response has actually carried the text, and the carrier then calls
    /// [`mark_displaced_told`](Self::mark_displaced_told). Marking here, before a
    /// renderer had decided whether the result could hold it, spent the notice into
    /// result shapes that swallowed it (a bare `"ok"`).
    ///
    /// The REMEDY is the per-call pin, never a re-activation: the harmed party can
    /// perform it, and a re-activation is process-wide. See
    /// `get_guide("workspace-state")` § Per-call workspace pinning.
    pub async fn displaced_default_notice(&self, caller: Option<&str>) -> Option<DisplacedNotice> {
        let caller = caller?;
        let inner = self.inner.read().await;
        let d = inner.displacement.as_ref()?;
        let current = inner
            .default_workspace()
            .and_then(|ws| ws.focused_project_root().ok())
            .or_else(|| inner.default_workspace_root.clone())?;
        // The record must describe the default as it stands: a default moved by some
        // path that did not go through `note_activation` is not ours to attribute.
        if d.to != current || inner.home_root.as_deref() == Some(current.as_path()) {
            return None;
        }
        if d.by.as_deref().is_none_or(|by| by == caller) {
            return None;
        }
        if inner.displacement_told.get(caller) == Some(&d.epoch) {
            return None;
        }
        let home = inner
            .home_root
            .as_ref()
            .map(|h| format!(" (home is {})", h.display()))
            .unwrap_or_default();
        let by = d.by.as_deref().unwrap_or("another caller");
        Some(DisplacedNotice {
            caller: caller.to_string(),
            epoch: d.epoch,
            text: format!(
                "This call carried no workspace= pin, so it resolved against {to}, which \
                 caller {by} made the server's active project {ago:?} ago{home}. If you \
                 did not choose {to}, results and refused writes describe the wrong \
                 workspace. Fix: pass workspace=<absolute path> on each tool call to pin \
                 resolution for yourself. Re-activating is not a fix: activation is \
                 process-wide and moves the default for every caller. If {by} is your own \
                 earlier identity (for example from before a /clear), nobody else moved it.",
                to = d.to.display(),
                ago = d.at.elapsed(),
            ),
        })
    }

    /// Record that `notice` was carried by a response, so its caller is not told this
    /// move again. Bounded by `DISPLACEMENT_TOLD_CAP`; shedding only ever re-tells.
    pub async fn mark_displaced_told(&self, notice: &DisplacedNotice) {
        let mut inner = self.inner.write().await;
        if inner.displacement_told.len() >= DISPLACEMENT_TOLD_CAP {
            inner.displacement_told.clear();
        }
        inner
            .displacement_told
            .insert(notice.caller.clone(), notice.epoch);
    }

    /// Get the active project root, or error if none is set.
    pub async fn require_project_root(&self) -> Result<PathBuf> {
        let inner = self.inner.read().await;
        inner
            .default_workspace()
            .ok_or_else(|| {
                crate::tools::RecoverableError::with_hint(
                    "No active project. Use activate_project first.",
                    "Call activate_project(\"/path/to/project\") to set the active project.",
                )
            })
            .and_then(|ws| {
                ws.focused_project_root().map_err(|_| {
                    crate::tools::RecoverableError::with_hint(
                        "No active project. Use activate_project first.",
                        "Call activate_project(\"/path/to/project\") to set the active project.",
                    )
                })
            })
            .map_err(Into::into)
    }

    /// Switch focus to a project by ID within the current workspace.
    pub async fn switch_focus(&self, project_id: &str) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner
            .default_workspace_mut()
            .ok_or_else(|| anyhow::anyhow!("No active workspace"))?
            .set_focused(project_id)
    }

    /// Promote a Dormant workspace project to Activated in-place.
    /// Unlike `activate()`, this preserves the workspace topology.
    pub async fn activate_within_workspace(
        &self,
        project_id: &str,
        read_only: Option<bool>,
    ) -> Result<()> {
        // --- Phase 1: read-only pass to resolve abs_root and check early-return ---
        // Use a read lock so we don't block other readers while doing the
        // lookup.  We'll re-check under the write lock below.
        let (abs_root, home_root_snapshot) = {
            let inner = self.inner.read().await;
            let ws = inner
                .default_workspace()
                .ok_or_else(|| anyhow::anyhow!("No active workspace"))?;
            let relative_root = ws
                .projects
                .iter()
                .find(|p| p.discovered.id == project_id)
                .map(|p| p.discovered.relative_root.clone())
                .ok_or_else(|| {
                    anyhow::anyhow!("Project '{}' not found in workspace", project_id)
                })?;
            (ws.root.join(&relative_root), inner.home_root.clone())
        };

        // --- Phase 2: blocking I/O outside any lock ---
        // Determine read_only using the snapshot; the write lock below will
        // re-derive this from the live state, so a race here is harmless.
        let is_home_snapshot = home_root_snapshot
            .as_ref()
            .map(|h| *h == abs_root)
            .unwrap_or(false);
        let effective_read_only_snapshot =
            AgentInner::resolve_read_only(read_only, is_home_snapshot);
        let _ = effective_read_only_snapshot; // recomputed under write lock below

        // A sub-project is discovered from a manifest, so its directory existed at discovery;
        // it may be gone since. `open_lock_file` below `create_dir_all`s, so without this a
        // deleted sub-project would be silently re-created as an empty project.
        existing_project_dir(abs_root.clone(), "sub-project root")?;

        // Open the lock file before acquiring the write lock — involves blocking
        // fs I/O (create_dir_all + OpenOptions::open) that must not run on the
        // async executor while holding a write guard.
        let file_lock = write_guard::open_lock_file(&abs_root)
            .with_context(|| format!("failed to open write.lock for {}", abs_root.display()))?;

        // --- Phase 3: write lock to mutate workspace state ---
        let mut inner = self.inner.write().await;

        // Clone home_root before taking a mutable reference into inner.workspace,
        // since RwLockWriteGuard doesn't support split field borrows.
        let home_root = inner.home_root.clone();

        let ws = inner
            .default_workspace_mut()
            .ok_or_else(|| anyhow::anyhow!("No active workspace"))?;

        // Re-resolve root under the write lock to guard against concurrent
        // activate() calls that could have replaced the workspace.
        let relative_root = ws
            .projects
            .iter()
            .find(|p| p.discovered.id == project_id)
            .map(|p| p.discovered.relative_root.clone())
            .ok_or_else(|| anyhow::anyhow!("Project '{}' not found in workspace", project_id))?;

        let abs_root = ws.root.join(&relative_root);

        // Determine read_only: an explicit request wins; absent one, home is rw
        // and a foreign root is ro. Single rule at AgentInner::resolve_read_only.
        let is_home = home_root.as_ref().map(|h| *h == abs_root).unwrap_or(false);
        let effective_read_only = AgentInner::resolve_read_only(read_only, is_home);

        // If already activated, just switch focus and optionally update read_only
        let already_activated = ws
            .projects
            .iter()
            .find(|p| p.discovered.id == project_id)
            .and_then(|p| p.as_active())
            .is_some();
        if already_activated {
            ws.set_focused(project_id)?;
            if let Some(ro) = read_only {
                if let Some(active) = ws.focused_active_mut().and_then(|p| p.as_active_mut()) {
                    active.read_only = ro;
                }
            }
            return Ok(());
        }

        // Load config, memory, library registry for the sub-project
        let config = ProjectConfig::load_or_default(&abs_root)?;
        // Resolve through the workspace, NOT `MemoryStore::open(&abs_root)`. Every
        // branch of the live `memory` tool — write included — routes through
        // `Workspace::memory_dir_for_project`, which places a non-root project's
        // memories at `<workspace_root>/.codescout/projects/<id>/memories`. Opening
        // the sub-project root instead reads `<abs_root>/.codescout/memories`, a
        // directory nothing writes to; and `MemoryStore` creates its directory on
        // open, so the miss also left an empty one behind corroborating itself. The
        // two resolve to the same path for the root project (`relative_root == "."`),
        // which is why this only ever surfaced on sub-projects and why two verify-open
        // passes against the home project could not clear it.
        //
        // `private_memory` deliberately stays project-local: the memory tool reads
        // private topics from `p.private_memory` on BOTH surfaces, so they already
        // agree, and `.codescout/private-memories/` is gitignored by design.
        //
        // docs/issues/archive/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md
        let memory = MemoryStore::from_dir(ws.memory_dir_for_project(project_id))?;
        let private_memory = MemoryStore::open_private(&abs_root)?;
        let registry_path = abs_root.join(".codescout").join("libraries.json");
        let library_registry = LibraryRegistry::load(&registry_path).unwrap_or_default();
        let head_sha = resolve_head_sha(&abs_root);

        let active = ActiveProject {
            root: abs_root.clone(),
            config,
            // This path reuses a config already resolved above rather than reading
            // one, so the stamp is taken here. Any write since that resolve leaves a
            // NEWER file than this stamp describes, which the next gated call sees as
            // a difference and reloads — the safe direction.
            config_stamp: ConfigStamp::of(&abs_root),
            config_load_error: None,
            memory,
            private_memory,
            library_registry,
            dirty_files: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
            read_only: effective_read_only,
            head_sha,
            has_git_remote: probe_has_git_remote(&abs_root),
            write_lock: Arc::new(tokio::sync::Mutex::new(())),
            file_lock,
            session_write_roots: Arc::new(std::sync::Mutex::new(Vec::new())),
        };

        // Promote in-place
        let project_mut = ws
            .projects
            .iter_mut()
            .find(|p| p.discovered.id == project_id)
            .expect("project_mut lookup — invariant: re-resolved from the same ws.projects slice under the write lock above; only activate_within_workspace mutates project list, and it holds this lock");
        project_mut.state = ProjectState::Activated(Box::new(active));

        // Switch focus
        ws.focused = Some(project_id.to_string());

        Ok(())
    }

    /// Resolve root: explicit project ID > file hint > focused project.
    pub async fn resolve_root(
        &self,
        project: Option<&str>,
        file_hint: Option<&std::path::Path>,
    ) -> Result<PathBuf> {
        let inner = self.inner.read().await;
        inner
            .default_workspace()
            .ok_or_else(|| anyhow::anyhow!("No active project"))?
            .resolve_root(project, file_hint)
    }
}

// ---------------------------------------------------------------------------
// Project files & status
// ---------------------------------------------------------------------------
impl Agent {
    /// Run a closure with a read-lock on the active project.
    /// Returns an error if no project is active.
    pub async fn with_project<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&ActiveProject) -> Result<T>,
    {
        let inner = self.inner.read().await;
        let project = inner
            .active_project()
            .ok_or_else(|| anyhow::anyhow!("No active project. Use activate_project first."))?;
        f(project)
    }

    /// Mark a file as written-but-not-yet-indexed.
    /// Called by every write tool after modifying a source file.
    pub async fn mark_file_dirty(&self, path: PathBuf) {
        let inner = self.inner.read().await;
        if let Some(p) = inner.active_project() {
            p.dirty_files
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(path);
        }
    }

    /// Append a session-approved write root for the current project.
    pub async fn add_session_write_root(&self, path: PathBuf) {
        let inner = self.inner.read().await;
        if let Some(p) = inner.active_project() {
            p.session_write_roots
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(path);
        }
    }

    /// Return a snapshot of the current session-approved write roots.
    pub async fn session_write_roots_snapshot(&self) -> Vec<PathBuf> {
        let inner = self.inner.read().await;
        match inner.active_project() {
            Some(p) => p
                .session_write_roots
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            None => Vec::new(),
        }
    }

    /// Number of files written in this session but not yet re-indexed.
    pub async fn dirty_file_count(&self) -> usize {
        let inner = self.inner.read().await;
        inner
            .active_project()
            .map(|p| {
                p.dirty_files
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .len()
            })
            .unwrap_or(0)
    }

    /// Drain all files marked dirty by write tools, returning them for re-indexing.
    /// Clears the set so subsequent calls return only newly-dirtied files.
    pub async fn drain_dirty_files(&self) -> Vec<PathBuf> {
        let inner = self.inner.read().await;
        inner
            .active_project()
            .map(|p| {
                let mut set = p.dirty_files.lock().unwrap_or_else(|e| e.into_inner());
                set.drain().collect()
            })
            .unwrap_or_default()
    }

    /// Clone the dirty-files Arc so index_project can capture it across a spawn boundary
    /// and clear it on successful completion.
    pub async fn dirty_files_arc(
        &self,
    ) -> Option<Arc<std::sync::Mutex<std::collections::HashSet<PathBuf>>>> {
        let inner = self.inner.read().await;
        inner.active_project().map(|p| p.dirty_files.clone())
    }

    /// Get the current project status for building server instructions.
    pub async fn project_status(&self) -> Option<crate::prompts::ProjectStatus> {
        // Phase 1: cheap clones under the read lock — no blocking I/O
        let (
            name,
            path,
            project_root,
            languages,
            memory_store,
            alt_memories_dir,
            db_path,
            prompt_file,
            default_prompt,
        ) = {
            let inner = self.inner.read().await;
            let project = inner.active_project()?;
            let prompt_file = project.root.join(".codescout").join("system-prompt.md");
            // Inline path — replaces embed::index::project_db_path during L-01
            // step 8a. The legacy sqlite db at this location indicates the user
            // has not yet migrated to the retrieval stack; activate_project
            // surfaces a separate `legacy_semantic_index` hint when present.
            let db_path = project.root.join(".codescout/embeddings/project.db");
            // The project's OTHER memory layout, when one exists.
            //
            // A sub-project has two, and `project.memory` is whichever the activation
            // path happened to open — workspace-resolved on the bare-id focus switch,
            // project-local on `Agent::new` / `load_project_resources`. So this cannot
            // name a fixed layout; it takes both candidates and keeps the one that is
            // not already `project.memory`'s. For the ROOT project every candidate
            // equals it and this is `None`.
            //
            // Without it the `## Project Status` block reported "None yet" for a
            // sub-project whose memories the activation response, in the same message,
            // listed twelve of.
            // docs/issues/archive/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md
            let alt_memories_dir = {
                let primary = project.memory.dir().to_path_buf();
                let project_local = project.root.join(".codescout").join("memories");
                let workspace_layout = inner.default_workspace().and_then(|ws| {
                    ws.focused
                        .as_deref()
                        .map(|id| ws.memory_dir_for_project(id))
                });
                [Some(project_local), workspace_layout]
                    .into_iter()
                    .flatten()
                    .find(|dir| dir != &primary)
            };
            Some((
                project.config.project.name.clone(),
                to_forward_slash(&project.root),
                project.root.clone(),
                project.config.project.languages.clone(),
                project.memory.clone(),
                alt_memories_dir,
                db_path,
                prompt_file,
                project.config.project.system_prompt.clone(),
            ))
        }?; // lock dropped here

        // Phase 2: blocking filesystem reads off the executor
        let (memories, has_index, system_prompt, worktree) =
            tokio::task::spawn_blocking(move || {
                let mut memories = memory_store.list().unwrap_or_default();
                if let Some(alt) = alt_memories_dir {
                    // `from_dir_readonly`, not `from_dir`: this is a status read and
                    // must not materialise the directory it inspects.
                    memories.extend(
                        crate::memory::MemoryStore::from_dir_readonly(alt)
                            .list()
                            .unwrap_or_default(),
                    );
                    memories.sort();
                    memories.dedup();
                }
                let has_index = db_path.exists();
                let system_prompt = read_system_prompt_file(&prompt_file, default_prompt);
                let worktree = crate::prompts::detect_worktree_info(&project_root);
                (memories, has_index, system_prompt, worktree)
            })
            .await
            .ok()?;

        // Phase 3: workspace summary (acquires its own read-lock)
        let workspace = self.workspace_summary().await;

        Some(crate::prompts::ProjectStatus {
            name,
            path,
            languages,
            memories,
            has_index,
            system_prompt,
            workspace,
            worktree,
        })
    }

    /// Map current `IndexingState` to a short label for external consumers
    /// (e.g. the `project://summary` MCP resource).
    pub fn index_status_label(&self) -> String {
        match &*self.indexing.lock().unwrap() {
            IndexingState::Idle => "idle".into(),
            IndexingState::Running { .. } => "indexing".into(),
            IndexingState::Done { .. } => "indexed".into(),
            IndexingState::Failed(_) => "failed".into(),
        }
    }

    /// Build workspace project summaries for multi-project repos.
    /// Returns None for single-project workspaces.
    pub async fn workspace_summary(&self) -> Option<Vec<crate::prompts::WorkspaceProjectSummary>> {
        let inner = self.inner.read().await;
        let ws = inner.default_workspace()?;
        if ws.projects.len() <= 1 {
            return None;
        }
        let ws_cfg: Option<crate::config::workspace::WorkspaceConfig> =
            crate::config::workspace::load_workspace_config(&ws.root);

        let summaries = ws
            .projects
            .iter()
            .map(|p| {
                let depends_on = ws_cfg
                    .as_ref()
                    .and_then(|cfg| cfg.projects.iter().find(|e| e.id == p.discovered.id))
                    .map(|e| e.depends_on.clone())
                    .unwrap_or_default();
                crate::prompts::WorkspaceProjectSummary {
                    id: p.discovered.id.clone(),
                    root: to_forward_slash(&p.discovered.relative_root),
                    languages: p.discovered.languages.clone(),
                    depends_on,
                }
            })
            .collect();
        Some(summaries)
    }

    /// If `path` is the active project's `.codescout/project.toml`, reload the
    /// in-memory config from disk. Called by `edit_file` after every successful
    /// write so that tools like `semantic_search` see the updated model immediately
    /// without requiring a session restart.
    ///
    /// This is the *proxy* path — it fires on "one of my own write tools targeted
    /// that file", never on "the file changed". `ActiveProject::config_is_stale` is
    /// the observation that covers every other writer; this stays because it
    /// refreshes eagerly at the moment of a known write rather than lazily at the
    /// next gated call, and because it reaches consumers that never ask for a
    /// security config at all.
    pub async fn reload_config_if_project_toml(&self, path: &std::path::Path) {
        let mut inner = self.inner.write().await;
        if let Some(ref mut p) = inner.active_project_mut() {
            let toml_path = p.root.join(".codescout").join("project.toml");
            if path == toml_path {
                p.reload_config_from_disk();
            }
        }
    }

    /// Returns the canonical `project_id` for the session-default workspace's
    /// call-edge cache entries — the focused sub-project id, or `ROOT_PROJECT_ID`.
    /// Delegates to `call_edges_project_id_for(None)`; kept as the ambient entry
    /// point for callers that operate on the default workspace.
    pub async fn call_edges_project_id(&self) -> String {
        self.call_edges_project_id_for(None).await
    }

    /// Pinned twin of `call_edges_project_id`: the call-edge `project_id` of the
    /// workspace named by `workspace_override` (resident-on-demand), or the
    /// session default when `None`. `call_graph` (read + upsert) and
    /// `invalidate_call_edges_for` BOTH resolve `project_id` through here, so they
    /// always agree on the cache namespace under a pin.
    pub async fn call_edges_project_id_for(&self, workspace_override: Option<&Path>) -> String {
        if let Some(root) = workspace_override {
            let _ = self.ensure_resident(root.to_path_buf()).await;
        }
        let inner = self.inner.read().await;
        let ws = match workspace_override {
            Some(root) => {
                let key = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
                inner.workspaces.get(&key)
            }
            None => inner.default_workspace(),
        };
        ws.and_then(|ws| ws.focused.clone())
            .unwrap_or_else(|| crate::workspace::ROOT_PROJECT_ID.to_string())
    }

    /// Invalidate call-edge cache entries for `path`.
    ///
    /// Called alongside `lsp.notify_file_changed` at every write-tool call site
    /// so that call-graph queries see fresh results after a file is modified.
    /// Best-effort: opens the project DB if one exists, then deletes all cached
    /// edges whose ref-site matches `path`. Silently no-ops when:
    /// - no project is active,
    /// - the embed DB does not exist yet (pre-index state),
    /// - or the DB open / DELETE fails (non-fatal degraded mode).
    pub async fn invalidate_call_edges(&self, path: &std::path::Path) {
        let root = {
            let inner = self.inner.read().await;
            inner.active_project().map(|p| p.root.clone())
        };
        let Some(root) = root else { return };

        // Skip invalidation if the call_edges cache file doesn't exist yet —
        // first-time invalidations are no-ops, not errors.
        let cache_db = root.join(".codescout/call_edges.db");
        if !cache_db.exists() {
            return;
        }

        // Derive the canonical project_id the same way the call_graph tool does.
        let project_id = self.call_edges_project_id().await;

        // Spawn blocking so we don't hold the async executor on a sqlite open.
        let path = path.to_path_buf();
        let _ = tokio::task::spawn_blocking(move || {
            let conn = match crate::tools::symbol::call_edges::cache::open_db(&root) {
                Ok(c) => c,
                Err(_) => return,
            };
            let cache = crate::tools::symbol::call_edges::cache::EdgeCache::new(&conn, &project_id);
            let _ = cache.invalidate_file(&path);
        })
        .await;
    }

    /// Pinned twin of `invalidate_call_edges`: invalidates the call-edge cache for
    /// `path` in the workspace named by `workspace_override` (or the default).
    /// Resolves BOTH the DB root and the `project_id` namespace from the pinned
    /// workspace so it agrees with `call_graph`'s pinned upsert/read. Best-effort,
    /// same no-op conditions as the ambient twin.
    pub async fn invalidate_call_edges_for(
        &self,
        workspace_override: Option<&Path>,
        path: &std::path::Path,
    ) {
        let root = self
            .with_project_at(workspace_override, |p| Ok(p.root.clone()))
            .await
            .ok();
        let Some(root) = root else { return };

        let cache_db = root.join(".codescout/call_edges.db");
        if !cache_db.exists() {
            return;
        }

        let project_id = self.call_edges_project_id_for(workspace_override).await;
        let path = path.to_path_buf();
        let _ = tokio::task::spawn_blocking(move || {
            let conn = match crate::tools::symbol::call_edges::cache::open_db(&root) {
                Ok(c) => c,
                Err(_) => return,
            };
            let cache = crate::tools::symbol::call_edges::cache::EdgeCache::new(&conn, &project_id);
            let _ = cache.invalidate_file(&path);
        })
        .await;
    }
}

// ---------------------------------------------------------------------------
// Workspace & discovery
// ---------------------------------------------------------------------------
impl Agent {
    /// Get optional project root (None if no workspace is active).
    ///
    /// Uses the same `focused_project_root()` path as `require_project_root()` so
    /// that read tools and write tools always agree on the project root — even when
    /// the focused project is still `Dormant` (i.e. after `switch_focus` to a
    /// sub-project that hasn't been fully loaded yet).
    pub async fn project_root(&self) -> Option<PathBuf> {
        let inner = self.inner.read().await;
        inner.default_workspace()?.focused_project_root().ok()
    }

    pub async fn is_project_explicitly_activated(&self) -> bool {
        self.inner.read().await.project_explicitly_activated
    }

    /// Did the CALLER choose this project during the session, via `activate`?
    ///
    /// Distinct from [`is_project_explicitly_activated`](Self::is_project_explicitly_activated),
    /// which is also true for a project resolved at startup. That is the right
    /// reading for `--project` — an operator did choose it — but not for the
    /// `current_dir()` fallback in `run_server`, which is a default, not a
    /// choice. Since the fallback fires whenever a cwd resolves, the startup
    /// flag is true before any tool runs in essentially every session.
    ///
    /// Ask THIS when the question is "has the caller picked a tree yet?" — for
    /// worktree ambiguity the startup cwd is no evidence at all: it describes
    /// where the process launched, and a harness that switches worktrees
    /// afterwards never touches it.
    pub async fn is_project_chosen_this_session(&self) -> bool {
        self.inner.read().await.project_chosen_this_session
    }

    /// Return the home project root (the first project activated in this session).
    pub async fn home_root(&self) -> Option<PathBuf> {
        self.inner.read().await.home_root.clone()
    }

    /// True when the active project is the home project (or both are None).
    pub async fn is_home(&self) -> bool {
        let inner = self.inner.read().await;
        match (inner.active_project(), &inner.home_root) {
            (Some(project), Some(home)) => project.root == *home,
            (None, None) => true,
            _ => false,
        }
    }

    /// Return the list of discovered projects from the active workspace.
    /// Returns an empty vec if no workspace is active.
    pub async fn discovered_projects(&self) -> Vec<crate::workspace::DiscoveredProject> {
        let inner = self.inner.read().await;
        inner
            .default_workspace()
            .map(|ws| ws.projects.iter().map(|p| p.discovered.clone()).collect())
            .unwrap_or_default()
    }

    /// Returns per-project memory topic lists for all workspace projects that have memories.
    /// Returns an empty vec for single-project activations (workspace absent or len ≤ 1).
    pub async fn workspace_project_memories(&self) -> Vec<(String, Vec<String>)> {
        let inner = self.inner.read().await;
        let ws = match inner.default_workspace() {
            Some(ws) if ws.projects.len() > 1 => ws,
            _ => return vec![],
        };
        ws.projects
            .iter()
            .filter_map(|p| {
                let dir = ws.memory_dir_for_project(&p.discovered.id);
                // Read-only by construction: this function is documented as
                // "Returns per-project memory topic lists", but `from_dir`'s
                // `create_dir_all` made merely ASKING what each project holds
                // materialise `projects/<id>/memories` for every project in the
                // workspace. `list` yields nothing for a missing directory, so
                // nothing is lost by not creating it.
                let topics = crate::memory::MemoryStore::from_dir_readonly(dir)
                    .list()
                    .unwrap_or_default();
                if topics.is_empty() {
                    None
                } else {
                    Some((p.discovered.id.clone(), topics))
                }
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------
impl Agent {
    /// Get the security config, or defaults if no project is active.
    /// Populates `library_paths` from the active project's library registry.
    ///
    /// **Re-reads `project.toml` when it has changed on disk since the cached config
    /// was built, and this is the seam that closes the staleness rather than
    /// `ensure_resident`.** The config is cached for the life of the process and
    /// `ensure_resident` is where that cache goes stale — but it is not where the
    /// staleness is *served*, and it is not on the path of every gated call. Every
    /// one of those reaches a security decision through `project_security_config`,
    /// which both this and `security_config_for` call, so checking here covers the
    /// gates and checking at `ensure_resident` would not.
    ///
    /// Two cached-config readers are deliberately left out of scope, named so the
    /// next reader does not re-derive them as covered: `current_capabilities`
    /// (`server.rs`) reads `shell_command_mode` directly to decide whether to
    /// *advertise* `run_command`, so a stale read there advertises a tool that then
    /// correctly refuses — degraded, not permissive; and `max_index_bytes` is a
    /// resource bound rather than a gate.
    pub async fn security_config(&self) -> crate::util::path_security::PathSecurityConfig {
        // Fast path: derive and check staleness under one read lock. The check is a
        // single `stat` — no read, no parse — which is what makes it affordable on
        // the path every tool call takes.
        let probe = {
            let inner = self.inner.read().await;
            inner
                .active_project()
                .map(|p| (project_security_config(p), p.config_is_stale()))
        };
        match probe {
            None => crate::util::path_security::PathSecurityConfig::default(),
            Some((config, false)) => config,
            Some((_, true)) => {
                // Slow path. The read lock above is released by the block, then the
                // write lock is taken — sequential, never nested.
                {
                    let mut inner = self.inner.write().await;
                    if let Some(ref mut p) = inner.active_project_mut() {
                        p.reload_config_from_disk();
                    }
                }
                let inner = self.inner.read().await;
                inner
                    .active_project()
                    .map(project_security_config)
                    .unwrap_or_default()
            }
        }
    }

    /// Resolve the per-language `mux` override from the config of the project
    /// named by `workspace_override` (resident-on-demand), or the session default
    /// when `None`. Returns `None` if no project is active or the language has no
    /// override set.
    ///
    /// Takes the pin directly rather than offering an unpinned twin: every call
    /// site sits next to an already-pinned `require_project_root_for`, so an
    /// unpinned variant would only be a footgun (see
    /// `docs/issues/archive/2026-07-09-residual-workspace-pin-gaps-post-edit-code-fix.md`,
    /// finding 5 — this helper resolved a *different* project's LSP config than
    /// the root it was about to be used with).
    pub async fn lsp_mux_override(
        &self,
        workspace_override: Option<&Path>,
        language: &str,
    ) -> Option<bool> {
        self.with_project_at(workspace_override, |p| {
            Ok(p.config.lsp.langs.get(language).and_then(|o| o.mux))
        })
        .await
        .unwrap_or(None)
    }

    /// Get a clone of the library registry, if a project is active.
    pub async fn library_registry(&self) -> Option<LibraryRegistry> {
        self.inner
            .read()
            .await
            .active_project()
            .map(|p| p.library_registry.clone())
    }

    /// Persist the library registry to disk.
    pub async fn save_library_registry(&self) -> Result<()> {
        let inner = self.inner.read().await;
        let project = inner
            .active_project()
            .ok_or_else(|| anyhow::anyhow!("No active project"))?;
        let path = project.root.join(".codescout").join("libraries.json");
        project.library_registry.save(&path)
    }
}

// ---------------------------------------------------------------------------
// Embedding & library indexing
// ---------------------------------------------------------------------------
impl Agent {
    /// Check if we should nudge about a library. Returns true at most once per
    /// session per library, and respects the persistent `nudge_dismissed` flag.
    pub async fn should_nudge(&self, lib_name: &str) -> bool {
        // Check persistent dismissal and indexed status
        let inner = self.inner.read().await;
        if let Some(p) = inner.active_project() {
            if let Some(entry) = p.library_registry.lookup(lib_name) {
                if entry.nudge_dismissed || entry.indexed {
                    return false;
                }
            }
        }
        drop(inner);

        // Check session dedup — insert returns true if the value was NEW
        let mut nudged = self
            .nudged_libraries
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        nudged.insert(lib_name.to_string())
    }

    /// Update the indexing state for a named library.
    pub fn set_library_state(&self, name: &str, state: LibraryIndexState) {
        let mut states = self
            .library_index_states
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        states.insert(name.to_string(), state);
    }

    /// Spawn a background library indexing task if auto_index is enabled and library is not yet indexed.
    pub async fn maybe_auto_index_library(&self, lib_name: &str) {
        let (should_index, root, entry_path, max_index_bytes) = {
            let inner = self.inner.read().await;
            let Some(p) = inner.active_project() else {
                return;
            };
            if !p.config.libraries.auto_index {
                return;
            }
            let Some(entry) = p.library_registry.lookup(lib_name) else {
                return;
            };
            if entry.indexed {
                return;
            }
            (
                true,
                p.root.clone(),
                entry.path.clone(),
                p.config.security.max_index_bytes,
            )
        };
        if !should_index {
            return;
        }
        // A library is walked under the library policy, not the host project's
        // `[ignored_paths]`: the scope guard below and the sync must judge the SAME set.
        let ignore_patterns = crate::retrieval::sync::SyncOpts::for_library(false).ignore_patterns;

        let name = lib_name.to_string();

        // Scope guard. This background path has no interactive user to confirm an
        // oversized/suspicious root, so a giant un-ignored tree must be *skipped*
        // here rather than walked. The streaming indexer keeps any single pass
        // memory-safe, but auto-indexing a dependency tree (e.g. a `.venv` with tens
        // of thousands of files) still wastes embed-server work and pollutes the
        // code index. Mirror the `index` tool's preflight; on "needs confirmation",
        // decline. (docs/issues/archive/2026-06-19-mcp-server-oom-68gb.md)
        let scope_root = entry_path.clone();
        let pf_patterns = ignore_patterns;
        let verdict = tokio::task::spawn_blocking(move || {
            crate::embed::preflight::check_index_scope(&scope_root, max_index_bytes, &pf_patterns)
        })
        .await;
        match verdict {
            Ok(Ok(crate::embed::preflight::PreflightVerdict::Clear)) => {}
            Ok(Ok(crate::embed::preflight::PreflightVerdict::RequiresConfirmation(info))) => {
                tracing::warn!(
                    library = %name,
                    root = %info.root.display(),
                    file_count = info.file_count,
                    approx_bytes = info.approx_bytes,
                    "skipping background auto-index: root exceeds scope guard \
                     (raise security.max_index_bytes to index it)"
                );
                self.set_library_state(
                    &name,
                    LibraryIndexState::Failed(
                        "skipped: root exceeds index scope guard (auto-index declined)".into(),
                    ),
                );
                return;
            }
            Ok(Err(e)) => {
                tracing::warn!(library = %name, error = %e, "skipping background auto-index: scope check failed");
                self.set_library_state(
                    &name,
                    LibraryIndexState::Failed(format!("scope check failed: {e}")),
                );
                return;
            }
            Err(e) => {
                tracing::warn!(library = %name, error = %e, "skipping background auto-index: scope check task error");
                self.set_library_state(
                    &name,
                    LibraryIndexState::Failed(format!("scope check task error: {e}")),
                );
                return;
            }
        }

        let lib_project_id = format!("lib:{}", name);
        self.set_library_state(&name, LibraryIndexState::Indexing { done: 0, total: 0 });

        let self_clone = self.clone();
        let sync_abort_for_task = self.active_sync_abort.clone();
        let sync_abort_for_store = self.active_sync_abort.clone();
        let task = tokio::spawn(async move {
            tracing::info!("Auto-indexing library '{}' in background...", name);
            crate::heartbeat::note_background_op(&format!("auto_index:{name}"));
            let result = async {
                let client =
                    crate::retrieval::client::RetrievalClient::from_env(Some(&root)).await?;
                let opts = crate::retrieval::sync::SyncOpts::for_library(false);
                client
                    .sync_project(&lib_project_id, &entry_path, opts)
                    .await
            }
            .await;
            match result {
                Ok(_report) => {
                    let mut inner = self_clone.inner.write().await;
                    if let Some(p) = inner.active_project_mut() {
                        if let Some(entry) = p.library_registry.lookup_mut(&name) {
                            entry.indexed = true;
                        }
                        let reg_path = p.root.join(".codescout/libraries.json");
                        let _ = p.library_registry.save(&reg_path);
                    }
                    drop(inner);
                    self_clone.set_library_state(
                        &name,
                        LibraryIndexState::Done {
                            chunks: 0,
                            version: String::new(),
                        },
                    );
                }
                Err(e) => {
                    self_clone.set_library_state(&name, LibraryIndexState::Failed(e.to_string()));
                }
            }
            // Clear the abort handle slot — task is done, nothing to cancel.
            *sync_abort_for_task
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = None;
        });
        *sync_abort_for_store
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(task.abort_handle());
    }

    /// Return a human-readable summary string for each tracked library.
    pub fn library_states_summary(&self) -> HashMap<String, String> {
        let states = self
            .library_index_states
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        states
            .iter()
            .map(|(k, v)| {
                let status = match v {
                    LibraryIndexState::Idle => "idle".to_string(),
                    LibraryIndexState::FetchingSources { command } => {
                        format!("fetching_sources: {}", command)
                    }
                    LibraryIndexState::Indexing { done, total } => {
                        format!("indexing: {}/{}", done, total)
                    }
                    LibraryIndexState::Done { chunks, version } => {
                        format!("done: {} chunks (v{})", chunks, version)
                    }
                    LibraryIndexState::Failed(msg) => format!("failed: {}", msg),
                };
                (k.clone(), status)
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Semantic memory store (Qdrant)
// ---------------------------------------------------------------------------
impl Agent {
    /// The semantic memory store a call should use, honouring its `workspace=` pin.
    ///
    /// [`Agent::semantic_memory_store`] is one cell built once from the SESSION DEFAULT
    /// project's root, so under the sqlite-vec backend (whose directory is
    /// `<root>/.codescout/embeddings`) a memory write pinned at project B put B's vectors into
    /// the default project A's `<id>.memories.db`, and after an `activate` every write kept
    /// going to the project the cell was first built for. Under sqlite-vec this resolves the
    /// root of THIS call (its pin, else the current default) and returns a store over that
    /// directory, built per call. The Qdrant backend (one collection, keyed by project id) and a
    /// test double installed in the default cell are the default store, unchanged.
    ///
    /// docs/issues/2026-10-19-a-pinned-projects-memory-vectors-go-into-the-default-projects-embeddings-database.md
    pub async fn semantic_memory_store_for(
        &self,
        workspace_override: Option<&Path>,
    ) -> anyhow::Result<Arc<dyn SemanticMemoryStore>> {
        self.semantic_memory_store_for_backend(
            crate::retrieval::code_store::VectorBackend::resolve(),
            workspace_override,
            |root| {
                Ok(
                    crate::retrieval::config::RetrievalConfig::from_env_and_project(root)?
                        .sqlite_dir,
                )
            },
        )
        .await
    }

    /// [`Agent::semantic_memory_store_for`] with the backend and the root-to-directory resolver
    /// passed in, so a test selects the sqlite-vec arm and the directory rule without setting
    /// `CODESCOUT_VECTOR_BACKEND` or `CODESCOUT_SQLITE_DIR`.
    pub(crate) async fn semantic_memory_store_for_backend(
        &self,
        backend: crate::retrieval::code_store::VectorBackend,
        workspace_override: Option<&Path>,
        sqlite_dir_for: impl Fn(Option<&Path>) -> anyhow::Result<PathBuf>,
    ) -> anyhow::Result<Arc<dyn SemanticMemoryStore>> {
        use crate::memory::sqlite_semantic_store::SqliteVecSemanticMemoryStore;
        use crate::retrieval::code_store::VectorBackend;
        if !matches!(backend, VectorBackend::SqliteVec) {
            return self.semantic_memory_store().await;
        }
        // A test double installed in the default cell stands for every root (the seam would
        // otherwise be bypassed under the lean build, where the backend resolves to sqlite-vec).
        // In production the cell holds a sqlite-vec store whenever this arm is reached, so the
        // check is test-only: `SemanticMemoryStore::as_any` is.
        #[cfg(test)]
        if let Some(installed) = self.semantic_memory.get() {
            if installed
                .as_any()
                .downcast_ref::<SqliteVecSemanticMemoryStore>()
                .is_none()
            {
                return Ok(installed.clone());
            }
        }
        // The root is resolved on EVERY call, from this call's pin or else the CURRENT default,
        // and the store is built fresh over its directory. Neither the default cell nor a cache
        // keyed by root can be used here: the cell is built once and survives an `activate`, and
        // a cached store keeps its sqlite files open, so a worktree removed and recreated at the
        // same path would keep writing into a deleted inode. `at` only allocates; a connection is
        // opened on first use of the store, which is this call's own use.
        let root = match workspace_override {
            Some(pin) => Some(self.require_project_root_for(Some(pin)).await?),
            None => self.project_root().await,
        };
        let dir = sqlite_dir_for(root.as_deref())?;
        Ok(Arc::new(SqliteVecSemanticMemoryStore::at(dir)) as Arc<dyn SemanticMemoryStore>)
    }

    /// Lazily construct (or return cached) the semantic memory store.
    ///
    /// Backend is selected by `CODESCOUT_VECTOR_BACKEND`: Qdrant (server stack,
    /// one network probe + `memories` collection bootstrap) or in-process
    /// sqlite-vec (lite stack, no daemon). Subsequent calls return the cached
    /// `Arc` without further I/O.
    ///
    /// In tests, pre-populate via `set_semantic_memory_store_for_test` to bypass
    /// the env-driven construction path.
    pub async fn semantic_memory_store(&self) -> anyhow::Result<Arc<dyn SemanticMemoryStore>> {
        use crate::retrieval::code_store::VectorBackend;
        self.semantic_memory
            .get_or_try_init(|| async {
                match VectorBackend::resolve() {
                    VectorBackend::SqliteVec => {
                        // Same project root the Qdrant arm below resolves. This arm
                        // used to read the environment itself, which is the split
                        // that arm's comment already called out — and it is also why
                        // the memory store kept writing `<id>.memories.db` into
                        // `$HOME` after the code store stopped.
                        let root = self.project_root().await;
                        let config =
                            crate::retrieval::config::RetrievalConfig::from_env_and_project(
                                root.as_deref(),
                            )?;
                        let store =
                            crate::memory::sqlite_semantic_store::SqliteVecSemanticMemoryStore::at(
                                config.sqlite_dir.clone(),
                            );
                        anyhow::Ok(Arc::new(store) as Arc<dyn SemanticMemoryStore>)
                    }
                    #[cfg(feature = "server-stack")]
                    VectorBackend::Qdrant => {
                        // Same project root `memory_embedder` resolves — without this,
                        // a project.toml-only [embeddings] config would make
                        // memory_embedder project-aware while this stayed env-only,
                        // a split env-only config could not produce (review round-1
                        // I-3).
                        let root = self.project_root().await;
                        let config =
                            crate::retrieval::config::RetrievalConfig::from_env_and_project(
                                root.as_deref(),
                            )?;
                        let qdrant =
                            crate::retrieval::qdrant::QdrantWrap::connect(&config.qdrant_url)
                                .await?;
                        let collection = config.collection("memories");
                        // Both the dimension resolution AND the collection bootstrap
                        // are bound by ONE timeout (review round-2 I3). Resolving the
                        // model's own dimension (below) can, for a local backend,
                        // trigger a first-time ONNX weights download from the HF hub —
                        // that used to sit entirely outside any timeout, reachable from
                        // `main.rs`/`prompts::builders`/the memory tool's `forget` path,
                        // none of which call `memory_embedder()` first to warm the
                        // cache. Bounding it here means a slow/absent download fails
                        // fast and retries on next use, exactly like the Qdrant-hang
                        // case below already did — same fail-soft contract, one timeout
                        // covering both causes.
                        let store = match tokio::time::timeout(
                            crate::retrieval::qdrant::QDRANT_BOOTSTRAP_TIMEOUT,
                            async {
                                // Resolve the model's own dimension rather than trusting an
                                // absent-or-wrong CODESCOUT_MODEL_DIM pin — see
                                // `RetrievalClient::resolve_model_dim` for why this can't
                                // reuse `memory_embedder()`'s already-built instance.
                                let dim =
                                    crate::retrieval::client::RetrievalClient::resolve_model_dim(
                                        &config,
                                    )
                                    .await? as u64;
                                crate::memory::semantic_store::QdrantSemanticMemoryStore::new(
                                    qdrant, collection, dim,
                                )
                                .await
                            },
                        )
                        .await
                        {
                            // Treated exactly like a connect error: it flows out as an
                            // `Err`, so `get_or_try_init` leaves the cell uninitialized
                            // and retries on the next call once the cause (a hung Qdrant,
                            // or a slow/absent model download) clears.
                            Ok(result) => result?,
                            Err(_) => anyhow::bail!(
                                "timed out bootstrapping Qdrant memories collection after {:?} \
                                 (Qdrant reachable but unresponsive, or the configured \
                                 embedding model is still downloading/loading?); semantic \
                                 memory unavailable this session — will retry on next use",
                                crate::retrieval::qdrant::QDRANT_BOOTSTRAP_TIMEOUT
                            ),
                        };
                        anyhow::Ok(Arc::new(store) as Arc<dyn SemanticMemoryStore>)
                    }
                    #[cfg(not(feature = "server-stack"))]
                    VectorBackend::Qdrant => anyhow::bail!(
                        "CODESCOUT_VECTOR_BACKEND=qdrant requires the `server-stack` build \
                         feature. Rebuild with `--features server-stack`, or use the lean lite \
                         stack with CODESCOUT_VECTOR_BACKEND=sqlite-vec."
                    ),
                }
            })
            .await
            .cloned()
    }

    /// Test seam: pre-populate the OnceCell with a stub store so tests don't
    /// hit the network. Fails (silently) if already initialized — call before
    /// any production code path triggers `semantic_memory_store()`.
    #[cfg(test)]
    pub fn set_semantic_memory_store_for_test(
        &self,
        store: Arc<dyn SemanticMemoryStore>,
    ) -> std::result::Result<(), tokio::sync::SetError<Arc<dyn SemanticMemoryStore>>> {
        self.semantic_memory.set(store)
    }

    /// Lazily construct (or return cached) the DEFAULT dense embedder for memory ops: one cell,
    /// built once from the default project's config.
    ///
    /// Production reaches this only through [`Agent::memory_embedder_for`]'s non-sqlite-vec arm
    /// (the Qdrant backend, whose one shared collection needs one embedding space). The
    /// sqlite-vec path builds per resolved config instead, so a pinned project is not embedded
    /// with the default project's model. Tests also use the cell as the seam that installs a
    /// double (`set_memory_embedder_for_test`).
    ///
    /// First call performs `RetrievalClient::from_env()` (one network probe)
    /// and wraps the resulting `client.embedder` (`Arc<dyn CodeEmbedder>`) in
    /// [`crate::retrieval::embedder::CodeDenseAdapter`], rather than building a
    /// second, independent embedder. This means memory recall always rides
    /// whatever backend code search selected — HTTP or the in-process local ONNX
    /// path — instead of having its own selection path that
    /// could drift from code search's. Subsequent calls share the cached `Arc`.
    ///
    /// In tests, pre-populate via [`Agent::set_memory_embedder_for_test`] to
    /// bypass the env-driven construction path.
    pub async fn memory_embedder(
        &self,
    ) -> anyhow::Result<Arc<dyn crate::retrieval::embedder::DenseEmbedder>> {
        self.memory_embedder
            .get_or_try_init(|| async {
                let root = self.project_root().await;
                let client =
                    crate::retrieval::client::RetrievalClient::from_env(root.as_deref()).await?;
                #[cfg(test)]
                let _ = self.test_seen_client_embedder.set(client.embedder.clone());
                let emb = crate::retrieval::embedder::CodeDenseAdapter(client.embedder);
                anyhow::Ok(Arc::new(emb) as Arc<dyn crate::retrieval::embedder::DenseEmbedder>)
            })
            .await
            .cloned()
    }

    /// The dense embedder a memory call should use, following the embedding config of the root
    /// THAT CALL resolves to (its `workspace=` pin, else the current default project).
    ///
    /// [`Agent::memory_embedder`] is one cell built once from the default project's config, so
    /// vectors for a pinned project B were embedded with A's model — and, now that they land in
    /// B's own database ([`Agent::semantic_memory_store_for`]), would mix embedding spaces or
    /// clash on dimension there. Built here from the resolved config exactly as
    /// `RetrievalClient::from_env` builds its embedder (`build_embedder`), and cached by what
    /// identifies an embedder: url, model, dimension pin and key. Projects that share a config
    /// share one embedder. A test double installed in the `memory_embedder` cell wins. Under
    /// the Qdrant backend this is the default embedder; see [`Agent::memory_embedder_for_with`].
    pub async fn memory_embedder_for(
        &self,
        workspace_override: Option<&Path>,
    ) -> anyhow::Result<Arc<dyn crate::retrieval::embedder::DenseEmbedder>> {
        #[cfg(test)]
        if let Some(installed) = self.memory_embedder.get() {
            return Ok(installed.clone());
        }
        self.memory_embedder_for_with(
            crate::retrieval::code_store::VectorBackend::resolve(),
            workspace_override,
            crate::retrieval::config::RetrievalConfig::from_env_and_project,
        )
        .await
    }

    /// [`Agent::memory_embedder_for`] with the backend and the root-to-config resolver passed
    /// in, so a test selects the arm and supplies per-root configs without the process
    /// environment (which on a developer machine pins the embedder for EVERY root).
    ///
    /// Only the sqlite-vec backend follows the root. Under Qdrant the memory store is the one
    /// default-cell collection, sized from the first root's model dimension, so a vector
    /// embedded for another root's model would be rejected (other dimension) or mix embedding
    /// spaces in that collection (same dimension): the default embedder is the only consistent
    /// answer there, exactly as [`Agent::semantic_memory_store_for`] answers with the default
    /// store.
    pub(crate) async fn memory_embedder_for_with(
        &self,
        backend: crate::retrieval::code_store::VectorBackend,
        workspace_override: Option<&Path>,
        resolve: impl Fn(Option<&Path>) -> anyhow::Result<crate::retrieval::config::RetrievalConfig>,
    ) -> anyhow::Result<Arc<dyn crate::retrieval::embedder::DenseEmbedder>> {
        if !matches!(
            backend,
            crate::retrieval::code_store::VectorBackend::SqliteVec
        ) {
            return self.memory_embedder().await;
        }
        let root = match workspace_override {
            Some(pin) => Some(self.require_project_root_for(Some(pin)).await?),
            None => self.project_root().await,
        };
        let config = resolve(root.as_deref())?;
        self.memory_embedder_for_config(&config).await
    }

    /// The cached-by-config half of [`Agent::memory_embedder_for`], split out so a test can hand
    /// it configs directly instead of routing them through the process environment.
    pub(crate) async fn memory_embedder_for_config(
        &self,
        config: &crate::retrieval::config::RetrievalConfig,
    ) -> anyhow::Result<Arc<dyn crate::retrieval::embedder::DenseEmbedder>> {
        let key = (
            config.embedder_url.clone(),
            config.model.clone(),
            config.model_dim,
            config.api_key.clone(),
        );
        let cached = self
            .memory_embedders_by_config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .cloned();
        if let Some(found) = cached {
            return Ok(found);
        }
        let lite = matches!(
            crate::retrieval::code_store::VectorBackend::resolve(),
            crate::retrieval::code_store::VectorBackend::SqliteVec
        );
        let embedder =
            crate::retrieval::client::RetrievalClient::build_embedder(config, lite).await?;
        let built = Arc::new(crate::retrieval::embedder::CodeDenseAdapter(embedder))
            as Arc<dyn crate::retrieval::embedder::DenseEmbedder>;
        // Two callers can build concurrently; the first insert wins and both use it.
        Ok(self
            .memory_embedders_by_config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(key)
            .or_insert(built)
            .clone())
    }

    /// Test seam: pre-populate the embedder cell so tool calls bypass
    /// `RetrievalClient::from_env`. Must be called before the first
    /// `memory_embedder()` invocation; later calls return [`SetError`].
    #[cfg(test)]
    pub fn set_memory_embedder_for_test(
        &self,
        embedder: Arc<dyn crate::retrieval::embedder::DenseEmbedder>,
    ) -> std::result::Result<
        (),
        tokio::sync::SetError<Arc<dyn crate::retrieval::embedder::DenseEmbedder>>,
    > {
        self.memory_embedder.set(embedder)
    }

    /// Resolve the code-chunk search used by memory anchor creation.
    ///
    /// Production behaviour is byte-identical to the inline
    /// `RetrievalClient::from_env(root)` this replaced — a fresh client per call,
    /// built from the same root the caller resolved. Only the indirection is new,
    /// and only so tests can substitute a network-free implementation.
    ///
    /// Not cached, unlike [`Agent::memory_embedder`]. `create_semantic_anchors`
    /// passes a root derived from `ctx.workspace_override`, so a shared `OnceCell`
    /// would pin whichever root arrived first and serve it to every later caller —
    /// a cross-workspace leak in exchange for saving one client construction on a
    /// path that already performs a network search.
    ///
    /// In tests, install via [`Agent::set_code_search_for_test`].
    pub async fn code_search(
        &self,
        root: Option<&std::path::Path>,
    ) -> anyhow::Result<Arc<dyn crate::retrieval::search::CodeChunkSearch>> {
        #[cfg(test)]
        {
            // Cloned out and the guard dropped before any await: a std Mutex must
            // not be held across one.
            let installed = self.code_search_override.lock().unwrap().clone();
            if let Some(stub) = installed {
                return Ok(stub);
            }
        }
        let client = crate::retrieval::client::RetrievalClient::from_env(root).await?;
        Ok(Arc::new(client) as Arc<dyn crate::retrieval::search::CodeChunkSearch>)
    }

    /// Test seam: install a network-free code search so anchor creation never
    /// reaches `RetrievalClient::from_env`.
    ///
    /// Overwrites rather than set-once, unlike the embedder and store seams. Those
    /// initialise a cache, so a second write would be ambiguous and they return
    /// `SetError`; this override is read on every call, so replacing it is
    /// well-defined. That is what lets a test install a counting stub over the
    /// default one `test_ctx_with_project_raw` puts in place.
    #[cfg(test)]
    pub fn set_code_search_for_test(
        &self,
        search: Arc<dyn crate::retrieval::search::CodeChunkSearch>,
    ) {
        *self.code_search_override.lock().unwrap() = Some(search);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "server-stack")]
    use serial_test::serial;
    use tempfile::tempdir;

    /// Canonicalize a path. On macOS this resolves the `/var` → `/private/var`
    /// symlink that `tempfile::tempdir` returns un-canonicalized but production
    /// code paths canonicalize via `std::fs::canonicalize`.
    fn canonical(p: &std::path::Path) -> std::path::PathBuf {
        std::fs::canonicalize(p).expect("path canonicalizes")
    }

    /// Build a minimal `ActiveProject` rooted in a tmpdir, so
    /// `project_security_config` can be exercised for real.
    fn active_project_at(
        root: &std::path::Path,
        read_only: bool,
        writes_in_config: bool,
    ) -> ActiveProject {
        let mut config = crate::config::project::ProjectConfig::load_or_default(root).unwrap();
        config.security.file_write_enabled = writes_in_config;
        let file_lock = Arc::new(std::fs::File::create(root.join(".lock")).unwrap());
        ActiveProject {
            root: root.to_path_buf(),
            config,
            // Test helper: the fixture mutates `config` in memory AFTER loading it, so
            // no on-disk file corresponds to what this project is carrying. `None`
            // reads as "no file at load time", which makes the first staleness check
            // see any real project.toml as a change. Tests that care drive the stamp
            // explicitly; tests that do not are unaffected, because nothing here calls
            // `config_is_stale`.
            config_stamp: None,
            config_load_error: None,
            memory: MemoryStore::from_dir(root.join("mem")).unwrap(),
            private_memory: MemoryStore::from_dir(root.join("priv")).unwrap(),
            library_registry: LibraryRegistry::default(),
            dirty_files: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
            read_only,
            head_sha: None,
            has_git_remote: false,
            write_lock: Arc::new(tokio::sync::Mutex::new(())),
            file_lock,
            session_write_roots: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    /// Pins the WIRING, which nothing else does.
    ///
    /// Every other test of this feature constructs a `PathSecurityConfig` by
    /// hand, so all of them pass with `project_security_config`'s assignment
    /// deleted — measured: 4598 tests green with the wiring removed. That is the
    /// exact shape that let two other features in this repo ship inert under a
    /// green suite, so the derivation gets its own test rather than being
    /// assumed from the unit tests of the thing it derives.
    #[test]
    fn project_security_config_attributes_a_read_only_project_to_its_root() {
        let tmp = tempfile::tempdir().unwrap();
        let p = active_project_at(tmp.path(), true, true);
        let cfg = project_security_config(&p);

        assert!(!cfg.file_write_enabled, "read-only must disable writes");
        let block = cfg
            .write_block
            .expect("the derivation must attribute the block, not leave it None");
        assert_eq!(block.root, tmp.path(), "must name THIS project");
        assert_eq!(
            block.cause,
            crate::util::path_security::WriteBlockCause::ActivatedReadOnly
        );
    }

    /// The other cause, through the same derivation — and the one whose remedy
    /// differs, so mis-attributing it produces confidently wrong advice.
    #[test]
    fn project_security_config_attributes_a_config_disabled_project_to_its_config() {
        let tmp = tempfile::tempdir().unwrap();
        let p = active_project_at(tmp.path(), false, false);
        let cfg = project_security_config(&p);

        assert!(!cfg.file_write_enabled);
        let block = cfg.write_block.expect("must attribute");
        assert_eq!(block.root, tmp.path());
        assert_eq!(
            block.cause,
            crate::util::path_security::WriteBlockCause::ConfiguredOff
        );
    }

    /// A writable project must carry no block at all — otherwise the refusal
    /// path could grow a message for a state that never refuses.
    #[test]
    fn project_security_config_leaves_a_writable_project_unattributed() {
        let tmp = tempfile::tempdir().unwrap();
        let p = active_project_at(tmp.path(), false, true);
        let cfg = project_security_config(&p);
        assert!(cfg.file_write_enabled);
        assert!(cfg.write_block.is_none());
    }

    /// THE LAST env-mutating test helper in the crate — and the only one left on
    /// purpose. Everything else now injects (see `ServerEnv`, `LibrarianEnv`,
    /// `GlobalConfig::load_from_dir`, `ProjectConfig::load_with_global_base`).
    ///
    /// Why this one survives:
    /// - It is `server-stack`-gated, and `server-stack` is NOT a default feature — so
    ///   it does not compile into, and cannot corrupt, the default `cargo test` run.
    /// - Its single consumer
    ///   (`semantic_memory_store_bootstrap_times_out_on_hung_qdrant`) exists precisely
    ///   to exercise the ENV-DRIVEN construction path (`VectorBackend::resolve` +
    ///   `RetrievalConfig::from_env`) against a black-hole Qdrant. Injecting past that
    ///   path would delete the thing under test.
    ///
    /// Closing it properly means threading a `RetrievalConfig` through `Agent` — worth
    /// doing, not done here. Tracked in
    /// `docs/issues/archive/2026-07-13-test-env-access-ub-nonserial-writers-race-build-tool-context.md`.
    /// Do NOT copy this pattern into a default-feature test.
    #[cfg(feature = "server-stack")]
    struct EnvGuard {
        key: &'static str,
        original: Option<std::ffi::OsString>,
    }

    #[cfg(feature = "server-stack")]
    impl EnvGuard {
        fn set<V: AsRef<std::ffi::OsStr>>(key: &'static str, value: V) -> Self {
            let original = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, original }
        }
    }

    #[cfg(feature = "server-stack")]
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.original.take() {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }

    #[tokio::test]
    async fn new_without_project() {
        let agent = Agent::new(None).await.unwrap();
        assert!(agent.require_project_root().await.is_err());
        assert!(agent.project_status().await.is_none());
    }

    #[tokio::test]
    async fn new_with_valid_project() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let root = agent.require_project_root().await.unwrap();
        assert_eq!(root, canonical(dir.path()));
    }

    #[tokio::test]
    async fn activate_sets_project() {
        let agent = Agent::new(None).await.unwrap();
        assert!(agent.require_project_root().await.is_err());

        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        agent
            .activate(dir.path().to_path_buf(), None)
            .await
            .unwrap();

        let root = agent.require_project_root().await.unwrap();
        assert_eq!(root, canonical(dir.path()));
    }

    #[tokio::test]
    async fn activate_replaces_previous_project() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();
        std::fs::create_dir_all(dir1.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir2.path().join(".codescout")).unwrap();

        let agent = Agent::new(Some(dir1.path().to_path_buf())).await.unwrap();
        assert_eq!(
            agent.require_project_root().await.unwrap(),
            canonical(dir1.path())
        );

        agent
            .activate(dir2.path().to_path_buf(), None)
            .await
            .unwrap();
        assert_eq!(
            agent.require_project_root().await.unwrap(),
            canonical(dir2.path())
        );
    }
    #[tokio::test]
    async fn activate_registers_default_workspace_by_canonical_root() {
        // Pins the Phase-1 registry invariant: after activate(root), the default
        // resolves to that canonical root, the registry is keyed by it, and the
        // focused project's root matches. The resolution invariant is durable
        // through Phase 3 (multi-residence); only the single-entry assertion is
        // Phase-1-specific (clear + reinsert on activate).
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let root = canonical(dir.path());

        let agent = Agent::new(None).await.unwrap();
        agent
            .activate(dir.path().to_path_buf(), None)
            .await
            .unwrap();

        {
            let inner = agent.inner.read().await;
            assert_eq!(
                inner.default_workspace_root.as_deref(),
                Some(root.as_path()),
                "default_workspace_root must be the canonical activated root"
            );
            assert!(
                inner.workspaces.contains_key(&root),
                "registry must be keyed by the canonical root"
            );
            assert_eq!(
                inner.workspaces.len(),
                1,
                "Phase 1: single-entry registry (clear + reinsert on activate)"
            );
        }

        // The focused project resolves through the default workspace to the root.
        let p_root = agent
            .with_project(|p| Ok(p.root().to_path_buf()))
            .await
            .unwrap();
        assert_eq!(p_root, root);

        // Re-activating the same root keeps the single-entry invariant.
        agent
            .activate(dir.path().to_path_buf(), None)
            .await
            .unwrap();
        {
            let inner = agent.inner.read().await;
            assert_eq!(
                inner.workspaces.len(),
                1,
                "re-activate same root: still one entry"
            );
            assert_eq!(
                inner.default_workspace_root.as_deref(),
                Some(root.as_path())
            );
        }
    }
    #[tokio::test]
    async fn require_project_root_for_resolves_pin_over_default() {
        // Phase 3: the pinned accessors resolve workspace A even when the
        // default is B — the Level-1 resolution every migrated read tool relies
        // on, tested at the accessor seam where they all converge. Proves:
        // pin resolves A, default stays B, both become resident (multi-residence).
        let dir_a = tempdir().unwrap();
        let dir_b = tempdir().unwrap();
        std::fs::create_dir_all(dir_a.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir_b.path().join(".codescout")).unwrap();
        let root_a = canonical(dir_a.path());
        let root_b = canonical(dir_b.path());

        let agent = Agent::new(Some(dir_b.path().to_path_buf())).await.unwrap();

        // Default (unpinned) resolves B.
        assert_eq!(agent.require_project_root().await.unwrap(), root_b);

        // Pinned to A resolves A (activate-on-miss), via both _for accessors.
        assert_eq!(
            agent.require_project_root_for(Some(&root_a)).await.unwrap(),
            root_a
        );
        assert_eq!(
            agent.project_root_for(Some(&root_a)).await,
            Some(root_a.clone())
        );

        // The pin did NOT mutate the default — unpinned calls still resolve B.
        assert_eq!(agent.require_project_root().await.unwrap(), root_b);

        // A and B are both resident now (multi-residence); default is still B.
        let inner = agent.inner.read().await;
        assert!(
            inner.workspaces.contains_key(&root_a),
            "pinned workspace A must be resident"
        );
        assert!(
            inner.workspaces.contains_key(&root_b),
            "default workspace B must remain resident"
        );
        assert_eq!(
            inner.default_workspace_root.as_deref(),
            Some(root_b.as_path())
        );
    }

    #[tokio::test]
    async fn lsp_mux_override_resolves_pin_over_default() {
        // BUG (docs/issues/archive/2026-07-09-residual-workspace-pin-gaps-post-edit-code-fix.md,
        // finding 5): lsp_mux_override read the config via the plain, unpinned
        // with_project. Every call site sits one line below an already-pinned
        // require_project_root_for — so a pinned call started an LSP server at
        // workspace A's ROOT but with workspace B's MUX CONFIG.
        let dir_a = tempdir().unwrap();
        let dir_b = tempdir().unwrap();
        for dir in [&dir_a, &dir_b] {
            std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        }
        // A and B disagree about rust's mux setting — so the value returned is
        // direct evidence of WHICH project's config was consulted.
        std::fs::write(
            dir_a.path().join(".codescout").join("project.toml"),
            "[project]\nname = \"a\"\n\n[lsp.rust]\nmux = true\n",
        )
        .unwrap();
        std::fs::write(
            dir_b.path().join(".codescout").join("project.toml"),
            "[project]\nname = \"b\"\n\n[lsp.rust]\nmux = false\n",
        )
        .unwrap();
        let root_a = canonical(dir_a.path());

        // Default (unpinned) workspace is B.
        let agent = Agent::new(Some(dir_b.path().to_path_buf())).await.unwrap();

        assert_eq!(
            agent.lsp_mux_override(None, "rust").await,
            Some(false),
            "unpinned call must read the session-default project B's config"
        );
        assert_eq!(
            agent.lsp_mux_override(Some(&root_a), "rust").await,
            Some(true),
            "pinned call must read the PINNED project A's config, not the default B's"
        );
    }

    #[tokio::test]
    async fn a_write_pin_consents_for_the_call_and_records_nothing() {
        // FINDING (docs/issues/archive/2026-07-09-edit-code-write-path-ignores-workspace-pin.md,
        // "Live-verification finding"): a pinned WRITE to a non-home root, which is read-only by
        // default, must be able to proceed without a full `activate` (which clears every other
        // resident workspace). That is still required. It used to be met by
        // `ensure_resident(root, Some(false))` flipping the SHARED entry to writable, so one
        // pinned write lifted a read-only activation for every caller
        // (docs/issues/archive/2026-09-28-a-pinned-write-lifts-a-read-only-activation-for-every-caller.md).
        // It is met now by `security_config_for_write`, which applies the pin's consent to that
        // one derivation and writes nothing.
        let dir_a = tempdir().unwrap();
        let dir_b = tempdir().unwrap();
        std::fs::create_dir_all(dir_a.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir_b.path().join(".codescout")).unwrap();
        let root_a = canonical(dir_a.path());

        let agent = Agent::new(Some(dir_b.path().to_path_buf())).await.unwrap();

        // First touch (read-oriented default): A becomes resident, read-only.
        agent.ensure_resident(root_a.clone()).await.unwrap();
        let cfg = agent.security_config_for(Some(&root_a)).await;
        assert!(
            !cfg.file_write_enabled,
            "control: a fresh pin must default to read-only"
        );

        // The pinned WRITE is let through for its own call ...
        let write_cfg = agent.security_config_for_write(Some(&root_a)).await;
        assert!(
            write_cfg.file_write_enabled && write_cfg.write_block.is_none(),
            "a pinned write must be consented to, or a pin can never write without a full activate"
        );

        // ... and that consent is NOT recorded anywhere. This is the assertion that fails if
        // the shared entry is mutated again: the entry, and the config any other caller
        // derives from it, are exactly what they were.
        let still_read_only = agent
            .with_project_at(Some(&root_a), |p| Ok(p.read_only))
            .await
            .unwrap();
        assert!(
            still_read_only,
            "a pinned write must not change the shared entry"
        );
        let after = agent.security_config_for(Some(&root_a)).await;
        assert!(
            !after.file_write_enabled,
            "another caller's config must still be read-only after a pinned write"
        );
        assert_eq!(
            after.write_block.map(|b| b.cause),
            Some(crate::util::path_security::WriteBlockCause::ActivatedReadOnly),
            "and must still attribute the block to the read-only activation"
        );
    }

    /// Consent needs a PIN. Without one the call runs against the default project, whose own
    /// read-only state binds its callers: the pin is the named, deliberate choice of target,
    /// and an unpinned write has made no choice.
    #[tokio::test]
    async fn no_pin_means_no_write_consent() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let root = canonical(dir.path());
        let agent = Agent::new(Some(root.clone())).await.unwrap();
        agent.activate(root, Some(true)).await.unwrap();

        let cfg = agent.security_config_for_write(None).await;
        assert!(
            !cfg.file_write_enabled,
            "an unpinned write must not be consented to under an explicit read-only activation"
        );
    }

    /// The pin lifts the read-only ACTIVATION, never a project's own `project.toml`. That
    /// setting is durable and a pin is a per-call convenience, so consent must not reach it.
    /// Paired with the case above: "consent lifts read-only" alone is satisfied by a
    /// derivation that ignores every write block, and only this case says it does not.
    #[tokio::test]
    async fn a_write_pin_does_not_lift_a_project_that_disables_writes_in_its_config() {
        let home = tempdir().unwrap();
        let locked = tempdir().unwrap();
        std::fs::create_dir_all(home.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(locked.path().join(".codescout")).unwrap();
        std::fs::write(
            locked.path().join(".codescout").join("project.toml"),
            "[project]\nname = \"locked\"\n\n[security]\nfile_write_enabled = false\n",
        )
        .unwrap();
        let locked_root = canonical(locked.path());
        let agent = Agent::new(Some(home.path().to_path_buf())).await.unwrap();
        agent.ensure_resident(locked_root.clone()).await.unwrap();

        let cfg = agent.security_config_for_write(Some(&locked_root)).await;
        assert!(
            !cfg.file_write_enabled,
            "a project whose own config turns writes off must stay off under a pin"
        );
        assert_eq!(
            cfg.write_block.map(|b| b.cause),
            Some(crate::util::path_security::WriteBlockCause::ConfiguredOff),
            "and must be attributed to its config, whose remedy is different"
        );
    }

    #[tokio::test]
    async fn require_project_root_error_message() {
        let agent = Agent::new(None).await.unwrap();
        let err = agent.require_project_root().await.unwrap_err();
        assert!(
            err.to_string().contains("No active project"),
            "error should mention no active project: {}",
            err
        );
    }
    #[test]
    fn concurrent_switch_warning_flags_rapid_foreign_switch() {
        use std::time::Duration;
        let a = std::path::Path::new("/tmp/cc-wt-a");
        let b = std::path::Path::new("/tmp/cc-wt-b");
        let window = Duration::from_secs(5);

        // First activation (no prior) → silent.
        assert!(Agent::concurrent_switch_warning(None, a, None, window).is_none());

        // Rapid switch to a DIFFERENT root → warning (the subagent-race signature).
        // The message must recommend per-request pinning as the primary fix and
        // separate windows as the fallback — both are guidance contracts.
        let w = Agent::concurrent_switch_warning(
            Some((a, Duration::from_millis(200), None)),
            b,
            None,
            window,
        );
        assert!(w.as_deref().is_some_and(|s| {
            s.contains("workspace=<absolute path>") && s.contains("separate client windows")
        }));

        // Same-root re-activation → silent (normal return-home / re-activate).
        assert!(Agent::concurrent_switch_warning(
            Some((a, Duration::from_millis(200), None)),
            a,
            None,
            window
        )
        .is_none());

        // Different root but OUTSIDE the window (slow sequential switch) → silent.
        assert!(Agent::concurrent_switch_warning(
            Some((a, Duration::from_secs(60), None)),
            b,
            None,
            window
        )
        .is_none());
    }

    /// The false positive the bug is about: ONE session following the documented
    /// cross-project pattern (activate foreign -> work -> activate home) fast
    /// enough to land inside the window. Proximity alone cannot tell it from a
    /// two-caller race; identity can.
    #[test]
    fn a_single_session_switching_its_own_slot_is_not_a_race() {
        use std::time::Duration;
        let a = std::path::Path::new("/tmp/cc-wt-a");
        let b = std::path::Path::new("/tmp/cc-wt-b");
        let window = Duration::from_secs(5);

        assert!(
            Agent::concurrent_switch_warning(
                Some((a, Duration::from_millis(200), Some("sid-one"))),
                b,
                Some("sid-one"),
                window,
            )
            .is_none(),
            "a session switching its OWN slot is not concurrent use — this is the \
             documented cross-project pattern, and warning on it trains the reader \
             to ignore the guard"
        );
    }

    /// The case the guard exists for must survive, and it must now NAME the other
    /// caller — a warning that says "another caller" leaves the reader with no one
    /// to ask.
    #[test]
    fn a_switch_by_a_different_session_still_warns_and_names_the_other_caller() {
        use std::time::Duration;
        let a = std::path::Path::new("/tmp/cc-wt-a");
        let b = std::path::Path::new("/tmp/cc-wt-b");
        let window = Duration::from_secs(5);

        let w = Agent::concurrent_switch_warning(
            Some((a, Duration::from_millis(200), Some("sid-one"))),
            b,
            Some("sid-two"),
            window,
        )
        .expect("a genuine two-caller race must still warn");

        assert!(
            w.contains("sid-one"),
            "the warning must name the OTHER caller, not merely assert one exists: {w}"
        );
        // Discriminator: kills an implementation that interpolates the CALLER's own
        // id, which would satisfy the assertion above.
        assert!(
            !w.contains("sid-two"),
            "it must name the previous holder, not the caller being warned: {w}"
        );
    }

    /// IDENTITY MAY ONLY SILENCE A WARNING, NEVER CREATE ONE.
    ///
    /// When either side is unidentified, two anonymous callers are
    /// indistinguishable from one, so the guard must fall back to the proximity
    /// behaviour it had before. A missed warning on a real race is the harm this
    /// guard exists for; a spurious one merely annoys.
    #[test]
    fn an_unidentified_caller_falls_back_to_proximity_and_still_warns() {
        use std::time::Duration;
        let a = std::path::Path::new("/tmp/cc-wt-a");
        let b = std::path::Path::new("/tmp/cc-wt-b");
        let window = Duration::from_secs(5);

        for (prev_sid, new_sid) in [
            (None, None),
            (Some("sid-one"), None),
            (None, Some("sid-one")),
        ] {
            assert!(
                Agent::concurrent_switch_warning(
                    Some((a, Duration::from_millis(200), prev_sid)),
                    b,
                    new_sid,
                    window,
                )
                .is_some(),
                "unidentified ({prev_sid:?} -> {new_sid:?}) must keep the old \
                 proximity warning: identity can only silence, never create"
            );
        }
    }

    // ---- the party HARMED by a move of the default workspace ----------------------
    //
    // `note_activation`'s guard speaks to whoever MOVED the slot. These cover the other
    // half: whoever makes an unpinned call afterwards and resolves against a root it
    // never chose. `docs/issues/archive/2026-09-24-residual-route-activation-warning-to-the-harmed-party.md`.

    /// A server whose home project is a fresh tempdir, plus a function to move the
    /// default the way the activate handler does (`activate`, then `note_activation`).
    async fn displaced_fixture() -> (tempfile::TempDir, std::path::PathBuf, Agent) {
        let home = tempdir().unwrap();
        let home_root = home.path().canonicalize().unwrap();
        let agent = Agent::new(Some(home_root.clone())).await.unwrap();
        (home, home_root, agent)
    }

    async fn move_default(agent: &Agent, to: &std::path::Path, by: Option<&str>) {
        agent.activate(to.to_path_buf(), None).await.unwrap();
        agent.note_activation(to, by).await;
    }

    /// What the notice machinery does for a caller whose response CARRIED the notice:
    /// peek, then mark spent. Production does exactly this in `call_content` (and the
    /// gate-refusal path), only with a render in between.
    async fn told(agent: &Agent, caller: Option<&str>) -> Option<String> {
        let n = agent.displaced_default_notice(caller).await?;
        agent.mark_displaced_told(&n).await;
        Some(n.text)
    }

    /// Peeking must not spend the notice: a response that could not carry it leaves it
    /// owed, and the next one that can carry it does. Marking inside the peek spent it
    /// on a bare `"ok"` that had nowhere to put it.
    #[tokio::test]
    async fn a_notice_is_owed_until_a_response_carries_it() {
        let (_h, _home, agent) = displaced_fixture().await;
        let foreign = tempdir().unwrap();
        let foreign_root = foreign.path().canonicalize().unwrap();
        move_default(&agent, &foreign_root, Some("sess-1/agent-a")).await;

        let first = agent
            .displaced_default_notice(Some("sess-1"))
            .await
            .expect("owed");
        assert!(
            agent
                .displaced_default_notice(Some("sess-1"))
                .await
                .is_some(),
            "an uncarried notice is still owed"
        );
        agent.mark_displaced_told(&first).await;
        assert!(
            agent
                .displaced_default_notice(Some("sess-1"))
                .await
                .is_none(),
            "carried and marked: spent"
        );
        // Spending is per caller: a sibling is unaffected.
        assert!(agent
            .displaced_default_notice(Some("sess-1/agent-b"))
            .await
            .is_some());
    }

    /// The notice must not claim what is not known: it names the mover as a fact, says
    /// re-activating is process-wide rather than asserting someone relies on the
    /// default, and admits the mover may be the reader's own earlier identity.
    #[tokio::test]
    async fn the_notice_text_claims_only_what_the_record_shows() {
        let (_h, _home, agent) = displaced_fixture().await;
        let foreign = tempdir().unwrap();
        let foreign_root = foreign.path().canonicalize().unwrap();
        move_default(&agent, &foreign_root, Some("sess-1/agent-a")).await;
        let n = told(&agent, Some("sess-1")).await.unwrap();
        assert!(n.contains("process-wide"), "{n}");
        assert!(
            !n.contains("under sess-1/agent-a"),
            "must not assert the mover still relies on the default: {n}"
        );
        assert!(
            n.contains("/clear"),
            "must admit the mover may be the reader's own earlier identity: {n}"
        );
    }

    /// The incident shape (E-5 of the archived 2026-08-23 bug): a SUBAGENT activates a
    /// foreign root; the PARENT's next unpinned call resolves against it and, until
    /// now, was told nothing.
    #[tokio::test]
    async fn the_parent_is_told_when_a_subagent_moved_the_default_off_home() {
        let (_h, home, agent) = displaced_fixture().await;
        let foreign = tempdir().unwrap();
        let foreign_root = foreign.path().canonicalize().unwrap();
        move_default(&agent, &foreign_root, Some("sess-1/agent-a")).await;

        let n = told(&agent, Some("sess-1"))
            .await
            .expect("the party resolving against a root a subagent chose must be told");
        assert!(
            n.contains(&foreign_root.display().to_string()),
            "must name the root the call resolved against: {n}"
        );
        assert!(
            n.contains("sess-1/agent-a"),
            "must name WHO moved it, so the reader has someone to reach: {n}"
        );
        assert!(
            n.contains(&home.display().to_string()),
            "must name the home the default was moved off: {n}"
        );
        // The remedy has to be one the harmed party can perform: a per-call pin. It
        // must NOT send them to re-activate, which would move the default under the
        // subagent that is relying on it.
        assert!(
            n.contains("workspace=<absolute path>"),
            "must prescribe the per-call pin: {n}"
        );
        // Once per move: a second unpinned call is not told again.
        assert!(
            told(&agent, Some("sess-1")).await.is_none(),
            "the same move is reported to the same caller once"
        );
    }

    /// Absence assertions are monotone under removal, so this is paired with the test
    /// above: that one proves the notice exists, this one proves it is addressed.
    #[tokio::test]
    async fn the_party_that_moved_the_default_is_never_told_it_did() {
        let (_h, _home, agent) = displaced_fixture().await;
        let foreign = tempdir().unwrap();
        let foreign_root = foreign.path().canonicalize().unwrap();
        move_default(&agent, &foreign_root, Some("sess-1")).await;

        assert!(
            told(&agent, Some("sess-1")).await.is_none(),
            "the switcher chose this root; telling it someone ELSE moved it is false"
        );
        // The same principal on a later call is still the mover, however long after.
        assert!(told(&agent, Some("sess-1")).await.is_none());
        // ...and a different principal is still owed it: the silence above is
        // identity, not "the notice is broken".
        assert!(
            told(&agent, Some("sess-1/agent-b")).await.is_some(),
            "positive control: another principal must still be told"
        );
    }

    /// Back at home nothing is displaced, however many moves it took to get there.
    #[tokio::test]
    async fn nothing_is_reported_once_the_default_is_back_at_home() {
        let (_h, home, agent) = displaced_fixture().await;
        let foreign = tempdir().unwrap();
        let foreign_root = foreign.path().canonicalize().unwrap();
        move_default(&agent, &foreign_root, Some("sess-1/agent-a")).await;
        assert!(
            told(&agent, Some("sess-1")).await.is_some(),
            "positive control: displaced, and the parent is owed the notice"
        );

        move_default(&agent, &home, Some("sess-1/agent-a")).await;
        assert!(
            told(&agent, Some("sess-1")).await.is_none(),
            "the default is home again: there is nothing to tell"
        );
        // Re-activating home by the PARENT over an already-home default is no move.
        move_default(&agent, &home, Some("sess-1")).await;
        assert!(told(&agent, Some("sess-1/agent-b")).await.is_none());
    }

    /// A NEW move re-arms the notice. The one-shot-per-conversation form was spent on
    /// the wrong episode once already (`worktree_read_notice`), so the unit of
    /// delivery is the move, not the conversation.
    #[tokio::test]
    async fn each_new_move_is_reported_again_naming_its_own_mover() {
        let (_h, _home, agent) = displaced_fixture().await;
        let f1 = tempdir().unwrap();
        let f2 = tempdir().unwrap();
        let (r1, r2) = (
            f1.path().canonicalize().unwrap(),
            f2.path().canonicalize().unwrap(),
        );

        move_default(&agent, &r1, Some("sess-1/agent-a")).await;
        let first = told(&agent, Some("sess-1")).await.unwrap();
        assert!(first.contains("sess-1/agent-a"));

        move_default(&agent, &r2, Some("sess-1/agent-b")).await;
        let second = told(&agent, Some("sess-1"))
            .await
            .expect("a second move is a new episode and must be reported");
        assert!(
            second.contains("sess-1/agent-b"),
            "names the NEW mover: {second}"
        );
        assert!(
            second.contains(&r2.display().to_string()),
            "names the NEW root: {second}"
        );
        assert!(
            !second.contains("sess-1/agent-a"),
            "must not attribute the new move to the previous mover: {second}"
        );
    }

    /// Identity is needed to CREATE this notice (unlike the activation guard, where it
    /// may only silence one): "someone else moved it" is a claim, and with either side
    /// unidentified it could be false. Silence is the honest answer.
    #[tokio::test]
    async fn an_unidentified_party_is_never_told_someone_else_moved_it() {
        let (_h, _home, agent) = displaced_fixture().await;
        let foreign = tempdir().unwrap();
        let foreign_root = foreign.path().canonicalize().unwrap();

        // Unidentified mover: no one can be told it was someone else.
        move_default(&agent, &foreign_root, None).await;
        assert!(told(&agent, Some("sess-1")).await.is_none());

        // Unidentified reader of an identified move: it may BE the mover.
        let other = tempdir().unwrap();
        let other_root = other.path().canonicalize().unwrap();
        move_default(&agent, &other_root, Some("sess-1/agent-a")).await;
        assert!(told(&agent, None).await.is_none());
        // Positive control: the same move does reach an identified reader.
        assert!(told(&agent, Some("sess-1")).await.is_some());
    }

    /// The told-map cannot grow without bound over a long session of subagent fan-outs,
    /// and shedding it only ever re-tells.
    #[tokio::test]
    async fn the_told_map_is_bounded() {
        let (_h, _home, agent) = displaced_fixture().await;
        let foreign = tempdir().unwrap();
        let foreign_root = foreign.path().canonicalize().unwrap();
        move_default(&agent, &foreign_root, Some("sess-1/mover")).await;

        for i in 0..(DISPLACEMENT_TOLD_CAP * 2) {
            let caller = format!("sess-1/agent-{i}");
            assert!(
                told(&agent, Some(&caller)).await.is_some(),
                "each distinct caller is told once"
            );
        }
        let told = agent.inner.read().await.displacement_told.len();
        assert!(
            told <= DISPLACEMENT_TOLD_CAP,
            "told-map holds {told} entries, cap is {DISPLACEMENT_TOLD_CAP}"
        );
    }

    /// Re-activating the root that is ALREADY the default moves nothing, so it must not
    /// transfer the blame: the harmed party is owed the name of whoever actually
    /// moved it, not of the last caller to touch the slot.
    #[tokio::test]
    async fn re_activating_the_current_root_does_not_change_who_moved_it() {
        let (_h, _home, agent) = displaced_fixture().await;
        let foreign = tempdir().unwrap();
        let foreign_root = foreign.path().canonicalize().unwrap();
        move_default(&agent, &foreign_root, Some("sess-1/agent-a")).await;
        move_default(&agent, &foreign_root, Some("sess-1/agent-b")).await;

        let n = told(&agent, Some("sess-1"))
            .await
            .expect("still displaced, and the parent has not been told");
        assert!(n.contains("sess-1/agent-a"), "names the real mover: {n}");
        assert!(!n.contains("sess-1/agent-b"), "agent-b moved nothing: {n}");
    }

    /// A default moved by a path that never went through `note_activation` is not ours
    /// to attribute: the record describes an OLDER move, and naming its mover for the
    /// current root would be a false statement.
    #[tokio::test]
    async fn a_default_moved_without_a_record_is_not_attributed_to_the_last_recorded_mover() {
        let (_h, _home, agent) = displaced_fixture().await;
        let (f1, f2) = (tempdir().unwrap(), tempdir().unwrap());
        let (r1, r2) = (
            f1.path().canonicalize().unwrap(),
            f2.path().canonicalize().unwrap(),
        );
        move_default(&agent, &r1, Some("sess-1/agent-a")).await;
        assert!(
            told(&agent, Some("sess-1")).await.is_some(),
            "positive control: the recorded move is reportable"
        );

        // Moved with no `note_activation`.
        agent.activate(r2, None).await.unwrap();
        assert!(
            told(&agent, Some("sess-1/agent-b")).await.is_none(),
            "the record names {r1:?}, the default is elsewhere: say nothing"
        );
    }

    /// The activation guard compares the per-call caller it is GIVEN. It used to read
    /// `CLAUDE_CODE_SESSION_ID` — one value per process, shared by a parent and all its
    /// subagents — and so was silent on the very race it exists for.
    ///
    /// Env-independent by construction: nothing here reads or sets `environ`.
    #[tokio::test]
    async fn note_activation_tells_a_parent_from_its_subagent() {
        let a = std::path::Path::new("/tmp/cc-na-a");
        let b = std::path::Path::new("/tmp/cc-na-b");

        let agent = Agent::new(None).await.unwrap();
        assert!(agent.note_activation(a, Some("sess-1")).await.is_none());
        let w = agent
            .note_activation(b, Some("sess-1/agent-a"))
            .await
            .expect("parent then subagent, rapidly, different roots: a real race");
        assert!(w.contains("sess-1"), "names the previous holder: {w}");

        // The documented single-session cross-project pattern stays silent.
        let agent = Agent::new(None).await.unwrap();
        assert!(agent.note_activation(a, Some("sess-1")).await.is_none());
        assert!(
            agent.note_activation(b, Some("sess-1")).await.is_none(),
            "one caller switching its own slot is not a race"
        );
    }

    #[tokio::test]
    async fn with_project_errors_when_none() {
        let agent = Agent::new(None).await.unwrap();
        let result = agent.with_project(|_p| Ok(42)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn with_project_runs_closure() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();

        let name = agent
            .with_project(|p| Ok(p.config.project.name.clone()))
            .await
            .unwrap();
        // Default config uses directory name
        assert!(!name.is_empty());
    }

    /// A memory write pinned at project B must put B's vectors under B's own
    /// `.codescout/embeddings`, not under the session default A's. The sqlite-vec arm is selected
    /// by argument, so the test neither sets `CODESCOUT_VECTOR_BACKEND` nor touches the real home;
    /// both roots are temp dirs, and the directory rule is the env-free `default_sqlite_dir`,
    /// so an exported `CODESCOUT_SQLITE_DIR` cannot redirect the stores or make this skip.
    ///
    /// docs/issues/2026-10-19-a-pinned-projects-memory-vectors-go-into-the-default-projects-embeddings-database.md
    #[tokio::test]
    async fn a_pinned_projects_memory_vectors_go_into_its_own_embeddings_dir() {
        use crate::retrieval::code_store::VectorBackend;

        let (_d, default_root) = temp_project("default-proj");
        let (_p, pinned_root) = temp_project("pinned-proj");
        let agent = Agent::new(Some(default_root.clone())).await.unwrap();
        agent.ensure_resident(pinned_root.clone()).await.unwrap();

        let store = agent
            .semantic_memory_store_for_backend(
                VectorBackend::SqliteVec,
                Some(&pinned_root),
                default_sqlite_dir,
            )
            .await
            .unwrap();
        upsert_probe(store.as_ref(), "pinned-proj").await;

        assert_eq!(
            memory_dbs(&pinned_root).len(),
            1,
            "the pinned project's memory vectors must land under ITS embeddings dir"
        );
        assert!(
            memory_dbs(&default_root).is_empty(),
            "the session default project must not receive the pinned project's vectors: {:?}",
            memory_dbs(&default_root)
        );
    }

    /// A temp project root (canonical) whose `project.toml` names it `name`.
    fn temp_project(name: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        std::fs::write(
            dir.path().join(".codescout").join("project.toml"),
            format!("[project]\nname = \"{name}\"\n"),
        )
        .unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        (dir, root)
    }

    /// The `*.memories.db` files under `<root>/.codescout/embeddings`.
    fn memory_dbs(root: &std::path::Path) -> Vec<String> {
        std::fs::read_dir(root.join(".codescout").join("embeddings"))
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .filter(|n| n.ends_with(".memories.db"))
                    .collect()
            })
            .unwrap_or_default()
    }

    async fn upsert_probe(store: &dyn SemanticMemoryStore, project_id: &str) {
        use crate::retrieval::memory_payload::SemanticMemory;
        store
            .upsert(
                &SemanticMemory {
                    project_id: project_id.into(),
                    bucket: "note".into(),
                    title: "t".into(),
                    content: "content".into(),
                    anchors: Vec::new(),
                    created_at: "2026-10-24T00:00:00Z".into(),
                    updated_at: "2026-10-24T00:00:00Z".into(),
                },
                &[1.0_f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            )
            .await
            .unwrap();
    }

    #[cfg(feature = "remote-embed")]
    #[tokio::test]
    async fn memory_embedder_follows_the_resolved_roots_embedding_config() {
        // The resolver reads each root's own `[embeddings]` through the pure project loader
        // (an EMPTY global layer, no `CODESCOUT_EMBED_*` overlay) and fixes everything else the
        // key does not read, so no process environment decides the verdict. The environment of a
        // developer machine pins one embedder for every root, which is exactly what made the
        // earlier env-reading version of this test return early and still report `ok`.
        let resolver =
            |root: Option<&Path>| -> anyhow::Result<crate::retrieval::config::RetrievalConfig> {
                let project = crate::config::project::ProjectConfig::load_with_global_base(
                    root.expect("a project root"),
                    toml::Value::Table(toml::map::Map::new()),
                )?;
                let mut config =
                    crate::retrieval::config::RetrievalConfig::from_env_and_project(None)?;
                config.embedder_url = project.embeddings.url.clone();
                config.model = project.embeddings.model_or_default();
                config.model_dim = None;
                config.api_key = None;
                Ok(config)
            };
        let with_model = |name: &str, model: &str| {
            let (dir, root) = temp_project(name);
            std::fs::write(
                root.join(".codescout").join("project.toml"),
                format!(
                    "[project]\nname = \"{name}\"\n[embeddings]\nurl = \"http://unused.invalid/v1\"\nmodel = \"{model}\"\n"
                ),
            )
            .unwrap();
            (dir, root)
        };
        let (_da, a) = with_model("proj-a", "model-a");
        let (_db, b) = with_model("proj-b", "model-b");
        let (_dc, c) = with_model("proj-c", "model-a");
        let agent = Agent::new(Some(a.clone())).await.unwrap();
        agent.ensure_resident(b.clone()).await.unwrap();
        agent.ensure_resident(c.clone()).await.unwrap();

        use crate::retrieval::code_store::VectorBackend::SqliteVec;
        let for_a = agent
            .memory_embedder_for_with(SqliteVec, None, resolver)
            .await
            .unwrap();
        let for_b = agent
            .memory_embedder_for_with(SqliteVec, Some(&b), resolver)
            .await
            .unwrap();
        let for_c = agent
            .memory_embedder_for_with(SqliteVec, Some(&c), resolver)
            .await
            .unwrap();
        assert!(
            !Arc::ptr_eq(&for_a, &for_b),
            "a pinned project with another model must not be embedded with the default's"
        );
        assert!(
            Arc::ptr_eq(&for_a, &for_c),
            "projects with an identical embedding config share one embedder"
        );
    }

    /// Under Qdrant the store is one default-cell collection sized from the first root's model,
    /// so the embedder must be the default one whatever root the call names: the resolver is
    /// never consulted. (No test seam in `memory_embedder_for_with`: the double is installed in
    /// the default cell, which IS the default embedder.)
    #[tokio::test]
    async fn the_qdrant_backend_embeds_with_the_default_embedder_for_every_root() {
        use crate::retrieval::code_store::VectorBackend;
        use crate::retrieval::embedder::DenseEmbedder;

        struct Stub;
        #[async_trait::async_trait]
        impl DenseEmbedder for Stub {
            async fn embed(&self, _t: &str) -> anyhow::Result<Vec<f32>> {
                Ok(vec![1.0])
            }
            async fn embed_document(&self, _t: &str) -> anyhow::Result<Vec<f32>> {
                Ok(vec![1.0])
            }
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }
        let (_da, a) = temp_project("proj-a");
        let (_db, b) = temp_project("proj-b");
        let agent = Agent::new(Some(a.clone())).await.unwrap();
        agent.ensure_resident(b.clone()).await.unwrap();
        let default: Arc<dyn DenseEmbedder> = Arc::new(Stub);
        agent
            .set_memory_embedder_for_test(default.clone())
            .map_err(|_| ())
            .expect("install the default embedder");

        let got = agent
            .memory_embedder_for_with(VectorBackend::Qdrant, Some(&b), |_| {
                panic!("the Qdrant backend must not resolve a per-root embedding config")
            })
            .await
            .unwrap();
        assert!(
            Arc::ptr_eq(&got, &default),
            "a pinned root must get the default embedder under Qdrant"
        );
    }

    /// The directory rule with no environment: `CODESCOUT_SQLITE_DIR` is not consulted.
    fn default_sqlite_dir(root: Option<&Path>) -> anyhow::Result<PathBuf> {
        crate::retrieval::config::resolve_sqlite_dir(None, root)
    }

    #[cfg(feature = "remote-embed")]
    #[tokio::test]
    async fn memory_embedders_are_cached_by_embedding_config() {
        let (_d, root) = temp_project("proj-cfg");
        let agent = Agent::new(Some(root.clone())).await.unwrap();
        let config_with = |model: &str| {
            let mut c =
                crate::retrieval::config::RetrievalConfig::from_env_and_project(Some(&root))
                    .unwrap();
            c.embedder_url = Some("http://unused.invalid/v1".into());
            c.model = model.into();
            c
        };
        let a = agent
            .memory_embedder_for_config(&config_with("model-a"))
            .await
            .unwrap();
        let b = agent
            .memory_embedder_for_config(&config_with("model-b"))
            .await
            .unwrap();
        let a_again = agent
            .memory_embedder_for_config(&config_with("model-a"))
            .await
            .unwrap();
        assert!(
            !Arc::ptr_eq(&a, &b),
            "another model must get its own embedder"
        );
        assert!(
            Arc::ptr_eq(&a, &a_again),
            "an identical config shares one embedder"
        );
    }

    /// The default project changes under a running session (`activate`): the store cell
    /// is built once from the root at first use and never reset, so the next memory write
    /// — pinned at the new default or not — wrote into the PREVIOUS project's directory.
    /// Both calls here go through one `Agent`, as one session does.
    #[tokio::test]
    async fn memory_vectors_follow_the_default_project_across_an_activate() {
        use crate::retrieval::code_store::VectorBackend;

        let (_da, a) = temp_project("proj-a");
        let (_db, b) = temp_project("proj-b");
        let agent = Agent::new(Some(a.clone())).await.unwrap();

        let first = agent
            .semantic_memory_store_for_backend(VectorBackend::SqliteVec, None, default_sqlite_dir)
            .await
            .unwrap();
        upsert_probe(first.as_ref(), "proj-a").await;
        assert_eq!(
            memory_dbs(&a).len(),
            1,
            "precondition: A's write lands under A"
        );

        agent.activate(b.clone(), None).await.unwrap();

        for (label, pin) in [
            ("unpinned", None),
            ("pinned at the new default", Some(b.as_path())),
        ] {
            let store = agent
                .semantic_memory_store_for_backend(
                    VectorBackend::SqliteVec,
                    pin,
                    default_sqlite_dir,
                )
                .await
                .unwrap();
            upsert_probe(store.as_ref(), "proj-b").await;
            assert_eq!(
                memory_dbs(&b).len(),
                1,
                "{label}: after activating B, B's vectors must land under B"
            );
            assert_eq!(
                memory_dbs(&a).len(),
                1,
                "{label}: after activating B, A must receive nothing more: {:?}",
                memory_dbs(&a)
            );
        }
    }

    #[cfg(feature = "server-stack")]
    #[tokio::test]
    #[serial]
    async fn semantic_memory_store_bootstrap_times_out_on_hung_qdrant() {
        // Regression for docs/issues/archive/2026-06-24-qdrant-hang-wedges-mcp-startup.md:
        // a reachable-but-unresponsive Qdrant (TCP accepts, no reply) must not
        // block semantic_memory_store() anywhere near the client's 120s operation
        // timeout. Black-hole listener mirrors the bug file's own `socat`/`nc -l`
        // standalone repro.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local_addr");
        tokio::spawn(async move {
            loop {
                if let Ok((_stream, _)) = listener.accept().await {
                    // Accept and never respond — simulates a wedged Qdrant.
                    std::future::pending::<()>().await;
                }
            }
        });

        let _backend = EnvGuard::set("CODESCOUT_VECTOR_BACKEND", "qdrant");
        let _url = EnvGuard::set("CODESCOUT_QDRANT_URL", format!("http://{addr}"));
        // Review round-2 I3: pin the embedder to a remote/HTTP backend so
        // `resolve_model_dim` (now wrapped in the SAME timeout this test
        // measures — see `semantic_memory_store`) takes its instant,
        // zero-I/O branch regardless of ambient `[embeddings]`/env config.
        // Without this, a host with a local model configured would make
        // this test perform a real ONNX load (or fail if weights are
        // absent) before ever reaching the black-hole Qdrant listener —
        // `result.is_err()` would still pass, but for the wrong reason,
        // and the timeout guard this test exists to pin would silently
        // stop being exercised.
        let _embedder_url = EnvGuard::set("CODESCOUT_EMBEDDER_URL", "http://unused.invalid");

        let agent = Agent::new(None).await.unwrap();
        let start = std::time::Instant::now();
        let result = agent.semantic_memory_store().await;
        let elapsed = start.elapsed();

        assert!(
            result.is_err(),
            "expected a bootstrap-timeout error against a black-hole Qdrant"
        );
        assert!(
            elapsed < std::time::Duration::from_secs(30),
            "semantic_memory_store() took {elapsed:?} against a black-hole Qdrant \
             — bootstrap timeout guard regressed (unbounded case blocks up to 120s)"
        );
    }

    /// Memory recall must ride the same embedder instance code search uses. If this
    /// regresses, memory silently keeps its own HTTP embedder and a local model
    /// configured for code search would not reach memory at all.
    ///
    /// Proves INSTANCE identity, not merely equivalent behaviour. `memory_embedder()`
    /// performs exactly one `RetrievalClient::from_env()` call; `Agent::test_seen_client_embedder`
    /// (test-only) captures a clone of `client.embedder` at the exact point it is about
    /// to be moved into `CodeDenseAdapter` — see `memory_embedder`'s body. If
    /// `memory_embedder` is ever changed to build its own independent `EmbedderHttp`
    /// instead of using `client.embedder`, the two `Arc`s diverge and `Arc::ptr_eq`
    /// below catches it. (Verified by sabotage: temporarily swapping the
    /// `CodeDenseAdapter(client.embedder)` line for
    /// `CodeDenseAdapter(Arc::new(EmbedderHttp::new(...)))` makes this test fail;
    /// reverting makes it pass again.)
    ///
    /// **Gated on `remote-embed` since 2026-08-30, and the asymmetry with the two
    /// `selection_tests` this shares a regression with is deliberate.** Those two
    /// assert a claim that survives a lean build — *the guard must not be what
    /// rejects this config* — so they stay ungated and branch only on which error
    /// arrives. This test's subject is `Arc::ptr_eq` on an `EmbedderHttp`
    /// **instance**, and a build with no HTTP transport has no such instance to
    /// share. There is no lean-meaningful version of the claim, so gating removes
    /// nothing; gating the other two would have deleted the guard's
    /// non-over-firing proof from the configuration where a mis-widened guard is
    /// hardest to see. `2c6f2677` turned all three red at once, which is what made
    /// one uniform remedy look right for all three.
    #[cfg(feature = "remote-embed")]
    #[tokio::test]
    async fn memory_embedder_is_built_from_the_shared_code_embedder() {
        use crate::retrieval::embedder::{CodeDenseAdapter, DenseEmbedder};

        // The embedder is resolved from CONFIGURATION, so this test supplies it instead of
        // inheriting whatever the developer's shell exports. It inherited it until
        // 2026-08-26, which held every CI `Test` lane red for a week while the local gate
        // read green — one failure among 4326 passes, and never reproducible locally.
        // docs/issues/archive/2026-08-26-ci-test-lanes-red-because-one-test-reads-ambient-embedder-config.md
        //
        // A project.toml and not an env var: mutating env in a default-feature test is what
        // `EnvGuard`'s doc comment in this module warns against. `url` is what does the work
        // — it selects `build_embedder`'s HTTP branch, which only CONSTRUCTS, no network.
        // The model is named explicitly even though that branch ignores it: leaving it to
        // `default_embed_model()` is what produced the ambient dependency in the first
        // place, and `guard_local_model_with_url` rejects a `local-dir:` model against a
        // url, so a remote name states the intent and cannot drift into that pair.
        //
        // Verified by control, not assumed: delete the `[embeddings]` block and this test
        // fails again with the original panic. (A `local:` model here would NOT fail — that
        // guard covers `local-dir:` only, on purpose; see its doc comment.)
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        std::fs::write(
            dir.path().join(".codescout/project.toml"),
            "[project]\nname = \"embedder-wiring\"\n\n[embeddings]\n\
             model = \"openai:text-embedding-3-small\"\nurl = \"http://127.0.0.1:1\"\n",
        )
        .unwrap();

        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let mem: std::sync::Arc<dyn DenseEmbedder> = agent.memory_embedder().await.unwrap();

        let seen_client_embedder = agent
            .test_seen_client_embedder
            .get()
            .expect("memory_embedder must capture client.embedder before returning")
            .clone();

        let adapter = mem
            .as_any()
            .downcast_ref::<CodeDenseAdapter>()
            .expect("memory_embedder must return a CodeDenseAdapter");

        assert!(
            std::sync::Arc::ptr_eq(&adapter.0, &seen_client_embedder),
            "memory_embedder's returned adapter must wrap the SAME embedder Arc the \
                 RetrievalClient it built holds — got two different instances"
        );
    }

    #[tokio::test]
    async fn project_status_returns_none_without_project() {
        let agent = Agent::new(None).await.unwrap();
        assert!(agent.project_status().await.is_none());
    }

    #[tokio::test]
    async fn project_status_returns_some_with_project() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();

        let status = agent.project_status().await;
        assert!(status.is_some());
        let status = status.unwrap();
        assert!(!status.name.is_empty());
        let canonical_dir = canonical(dir.path());
        // status.path is forward-slash normalized (RepoPath convention); the
        // raw canonicalized PathBuf renders with native separators on Windows.
        assert!(status
            .path
            .contains(&crate::util::fs::to_forward_slash(&canonical_dir)));
    }

    #[tokio::test]
    async fn agent_is_clone_safe() {
        // Agent wraps Arc<RwLock<...>> so clones share state
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(None).await.unwrap();
        let agent2 = agent.clone();

        agent
            .activate(dir.path().to_path_buf(), None)
            .await
            .unwrap();
        // Clone should see the activation
        let root = agent2.require_project_root().await.unwrap();
        assert_eq!(root, canonical(dir.path()));
    }

    #[tokio::test]
    async fn activate_creates_empty_library_registry() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();

        let reg = agent.library_registry().await.unwrap();
        assert!(
            reg.all().is_empty(),
            "fresh project should have empty library registry"
        );
    }

    #[tokio::test]
    async fn library_registry_none_without_project() {
        let agent = Agent::new(None).await.unwrap();
        assert!(agent.library_registry().await.is_none());
    }

    #[tokio::test]
    async fn project_status_reads_system_prompt_file() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join(".codescout");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("project.toml"),
            "[project]\nname = \"test\"\n",
        )
        .unwrap();
        std::fs::write(config_dir.join("system-prompt.md"), "Always use pytest.\n").unwrap();

        let agent = Agent::new(None).await.unwrap();
        agent
            .activate(dir.path().to_path_buf(), None)
            .await
            .unwrap();
        let status = agent.project_status().await.unwrap();
        assert_eq!(
            status.system_prompt.as_deref(),
            Some("Always use pytest.\n")
        );
    }

    #[tokio::test]
    async fn project_status_falls_back_to_toml_system_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join(".codescout");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("project.toml"),
            "[project]\nname = \"test\"\nsystem_prompt = \"From TOML\"\n",
        )
        .unwrap();

        let agent = Agent::new(None).await.unwrap();
        agent
            .activate(dir.path().to_path_buf(), None)
            .await
            .unwrap();
        let status = agent.project_status().await.unwrap();
        assert_eq!(status.system_prompt.as_deref(), Some("From TOML"));
    }

    #[tokio::test]
    async fn project_status_file_takes_precedence_over_toml() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join(".codescout");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("project.toml"),
            "[project]\nname = \"test\"\nsystem_prompt = \"From TOML\"\n",
        )
        .unwrap();
        std::fs::write(config_dir.join("system-prompt.md"), "From file\n").unwrap();

        let agent = Agent::new(None).await.unwrap();
        agent
            .activate(dir.path().to_path_buf(), None)
            .await
            .unwrap();
        let status = agent.project_status().await.unwrap();
        assert_eq!(status.system_prompt.as_deref(), Some("From file\n"));
    }

    /// A cloned repository can ship `.codescout/system-prompt.md` as a link to a secret;
    /// the file's text goes into the `initialize` instructions, i.e. into the model's
    /// context. A link is skipped, never served, and the TOML `system_prompt` (or nothing)
    /// is used instead, exactly as for an absent file.
    #[cfg(unix)]
    #[tokio::test]
    async fn project_status_never_serves_a_system_prompt_symlink_target() {
        let sandbox = tempfile::tempdir().unwrap();
        let secret = sandbox.path().join("home/.ssh/id_rsa");
        std::fs::create_dir_all(secret.parent().unwrap()).unwrap();
        std::fs::write(&secret, "SECRET-PROMPT-BYTES\n").unwrap();
        for (toml_prompt, expected) in [
            ("", None),
            ("system_prompt = \"From TOML\"\n", Some("From TOML")),
        ] {
            let root = sandbox
                .path()
                .join(format!("project-{}", expected.is_some()));
            let config_dir = root.join(".codescout");
            std::fs::create_dir_all(&config_dir).unwrap();
            std::fs::write(
                config_dir.join("project.toml"),
                format!("[project]\nname = \"test\"\n{toml_prompt}"),
            )
            .unwrap();
            std::os::unix::fs::symlink(&secret, config_dir.join("system-prompt.md")).unwrap();

            let agent = Agent::new(None).await.unwrap();
            agent.activate(root.clone(), None).await.unwrap();
            let status = agent.project_status().await.unwrap();

            assert_eq!(
                status.system_prompt.as_deref(),
                expected,
                "a linked system-prompt.md must be treated as absent"
            );
        }
    }

    #[tokio::test]
    async fn project_not_explicitly_activated_without_project() {
        let agent = Agent::new(None).await.unwrap();
        assert!(!agent.is_project_explicitly_activated().await);
    }

    #[tokio::test]
    async fn activate_sets_explicitly_activated() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(None).await.unwrap();
        agent
            .activate(dir.path().to_path_buf(), None)
            .await
            .unwrap();
        assert!(agent.is_project_explicitly_activated().await);
    }

    #[tokio::test]
    async fn new_with_project_sets_explicitly_activated() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        assert!(agent.is_project_explicitly_activated().await);
    }

    #[tokio::test]
    async fn home_root_set_from_initial_project() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        assert_eq!(agent.home_root().await, Some(canonical(dir.path())));
    }

    #[tokio::test]
    async fn home_root_none_without_project() {
        let agent = Agent::new(None).await.unwrap();
        assert_eq!(agent.home_root().await, None);
    }

    #[tokio::test]
    async fn home_root_set_on_first_activate() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(None).await.unwrap();
        agent
            .activate(dir.path().to_path_buf(), None)
            .await
            .unwrap();
        assert_eq!(agent.home_root().await, Some(canonical(dir.path())));
    }

    #[tokio::test]
    async fn home_root_not_changed_by_second_activate() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();
        std::fs::create_dir_all(dir1.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir2.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir1.path().to_path_buf())).await.unwrap();
        agent
            .activate(dir2.path().to_path_buf(), None)
            .await
            .unwrap();
        assert_eq!(agent.home_root().await, Some(canonical(dir1.path())));
    }

    #[tokio::test]
    async fn is_home_true_when_at_home() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        assert!(agent.is_home().await);
    }

    #[tokio::test]
    async fn is_home_false_after_switching() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();
        std::fs::create_dir_all(dir1.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir2.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir1.path().to_path_buf())).await.unwrap();
        agent
            .activate(dir2.path().to_path_buf(), None)
            .await
            .unwrap();
        assert!(!agent.is_home().await);
    }

    #[tokio::test]
    async fn is_home_true_after_returning() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();
        std::fs::create_dir_all(dir1.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir2.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir1.path().to_path_buf())).await.unwrap();
        agent
            .activate(dir2.path().to_path_buf(), None)
            .await
            .unwrap();
        assert!(!agent.is_home().await);
        agent
            .activate(dir1.path().to_path_buf(), None)
            .await
            .unwrap();
        assert!(agent.is_home().await);
    }

    /// Changes the process cwd for the life of the guard and restores it on drop, INCLUDING when the
    /// test panics: the cwd is process-global, so a panic that skipped the restore would leave every
    /// later test in the binary running somewhere else.
    struct CwdGuard {
        original: PathBuf,
    }

    impl CwdGuard {
        fn change_to(dir: &std::path::Path) -> Self {
            let original = std::env::current_dir().unwrap();
            std::env::set_current_dir(dir).unwrap();
            Self { original }
        }
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original);
        }
    }

    /// `#[serial]`: this test CHANGES the process cwd, which every ambient-cwd reader sees. It
    /// serializes only against the tests that opt in (`util::path_security`'s ambient-classification
    /// tests do; tests elsewhere that reach `WriteEnv::ambient()` through a tool do not), which is
    /// the documented limit of `#[serial]` (`docs/conventions/test-env-isolation.md`). The cwd
    /// itself cannot be injected: resolving a relative `--project` against it is what is tested.
    #[tokio::test]
    #[serial_test::serial]
    async fn new_with_relative_path_canonicalizes_home_root() {
        let dir = tempdir().unwrap();
        let canonical = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();

        // Simulate --project with a relative path by constructing one that
        // points to the same directory.  We use the tempdir's last component
        // as a relative path from its parent.
        let parent = canonical.parent().unwrap();
        let rel = canonical.file_name().unwrap();

        // The guard restores the cwd when it drops, panic or not.
        let cwd_guard = CwdGuard::change_to(parent);
        let agent = Agent::new(Some(PathBuf::from(rel))).await;
        drop(cwd_guard);
        let agent = agent.unwrap();

        // home_root must be the canonical absolute path, not the relative input.
        let home = agent.home_root().await.unwrap();
        assert!(
            home.is_absolute(),
            "home_root should be absolute, got: {}",
            home.display()
        );
        assert_eq!(home, canonical);

        // is_home should be true when re-activating the same directory
        // (simulates activate_project(".") which canonicalizes).
        agent.activate(canonical.clone(), None).await.unwrap();
        assert!(
            agent.is_home().await,
            "is_home must be true after re-activating the same directory"
        );
    }

    /// The guard's whole purpose: a panic while the cwd is changed must not leave it changed.
    ///
    /// Runs in a CHILD process (`platform::test_child`): the test changes the process cwd, and in
    /// the child the shared test binary never sees that change. (Unix only, like the helper.)
    #[cfg(unix)]
    #[test]
    fn the_cwd_guard_restores_the_cwd_when_the_guarded_code_panics() {
        crate::platform::test_child::run(
            module_path!(),
            "the_cwd_guard_restores_the_cwd_when_the_guarded_code_panics",
            &[],
            || {
                let original = std::env::current_dir().unwrap();
                let elsewhere = tempdir().unwrap();
                let elsewhere = elsewhere.path().canonicalize().unwrap();

                let outcome = std::panic::catch_unwind(|| {
                    let _guard = CwdGuard::change_to(&elsewhere);
                    assert_eq!(
                        std::env::current_dir().unwrap(),
                        elsewhere,
                        "fixture: moved"
                    );
                    panic!("the guarded code fails");
                });

                assert!(outcome.is_err(), "fixture: the closure must have panicked");
                assert_eq!(
                    std::env::current_dir().unwrap(),
                    original,
                    "the guard must put the cwd back during the unwind"
                );
            },
        );
    }

    #[tokio::test]
    async fn active_project_has_private_memory() {
        let dir = tempdir().unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        agent
            .with_project(|p| {
                p.private_memory.write("pref", "verbose")?;
                assert_eq!(p.private_memory.read("pref")?, Some("verbose".to_string()));
                // private is isolated from shared
                assert_eq!(p.memory.read("pref")?, None);
                Ok(())
            })
            .await
            .unwrap();
    }

    /// F12 through the real entry point. A cloned repository whose `.gitignore` is a link
    /// to a secret must not get the secret's bytes copied into a regular project file by
    /// merely being activated. `home/` is a fixture directory under the temp root, so
    /// nothing outside the sandbox is read or written.
    #[cfg(unix)]
    #[tokio::test]
    async fn activation_never_copies_a_gitignore_symlink_target_into_the_project() {
        let sandbox = tempdir().unwrap();
        let secret = sandbox.path().join("home/.ssh/id_rsa");
        std::fs::create_dir_all(secret.parent().unwrap()).unwrap();
        std::fs::write(&secret, "PRIVATE\n").unwrap();
        let root = sandbox.path().join("project");
        std::fs::create_dir_all(&root).unwrap();
        std::os::unix::fs::symlink(&secret, root.join(".gitignore")).unwrap();

        let agent = Agent::new(Some(root.clone())).await.unwrap();
        drop(agent);

        let meta = std::fs::symlink_metadata(root.join(".gitignore")).unwrap();
        assert!(
            meta.file_type().is_symlink(),
            ".gitignore was replaced by a regular file: {:?}",
            std::fs::read_to_string(root.join(".gitignore"))
        );
        assert_eq!(std::fs::read_to_string(&secret).unwrap(), "PRIVATE\n");
    }

    /// Regression test: after switch_focus to a sub-project, project_root() must
    /// return the sub-project root (same as require_project_root), not None.
    ///
    /// Uses the three-query sandwich:
    ///   1. Baseline: both methods agree on root
    ///   2. switch_focus to Dormant sub-project
    ///   3. Assert project_root() == sub-project root (not None — the bug)
    #[tokio::test]
    async fn project_root_matches_require_project_root_after_switch_focus() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();

        // Create a sub-project with a package.json so discover_projects picks it up
        let sub = root.join("packages").join("api");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(
            sub.join("package.json"),
            r#"{"name":"api","scripts":{"build":"tsc"}}"#,
        )
        .unwrap();

        let agent = Agent::new(Some(root.clone())).await.unwrap();

        // Step 1: baseline — both methods agree on root
        let pr = agent.project_root().await;
        let rpr = agent.require_project_root().await.unwrap();
        assert!(
            pr.is_some(),
            "project_root() must be Some before switch_focus"
        );
        assert_eq!(
            pr.unwrap(),
            rpr,
            "project_root() and require_project_root() must agree before switch_focus"
        );

        // Step 2: switch focus to the Dormant sub-project
        agent.switch_focus("api").await.unwrap();

        // Step 3: both methods must still agree — and return the sub-project root.
        // Before the fix, project_root() returned None here (Dormant bug).
        let pr_after = agent.project_root().await;
        let rpr_after = agent.require_project_root().await.unwrap();
        assert!(
            pr_after.is_some(),
            "project_root() must not be None after switch_focus (Dormant-project bug)"
        );
        assert_eq!(
            pr_after.unwrap(),
            rpr_after,
            "project_root() and require_project_root() must agree after switch_focus"
        );
        assert!(
            rpr_after.ends_with("packages/api"),
            "focused root must be the sub-project: {:?}",
            rpr_after
        );
    }

    #[tokio::test]
    async fn activate_non_home_defaults_to_read_only() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();
        std::fs::create_dir_all(dir1.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir2.path().join(".codescout")).unwrap();

        let agent = Agent::new(Some(dir1.path().to_path_buf())).await.unwrap();
        agent
            .activate(dir2.path().to_path_buf(), None)
            .await
            .unwrap();

        let config = agent.security_config().await;
        assert!(
            !config.file_write_enabled,
            "non-home project should be read-only by default"
        );
    }
    /// The silent half of
    /// `docs/issues/archive/2026-09-14-an-out-of-band-project-toml-edit-never-invalidates-the-cached-config.md`:
    /// `project.toml` is read once when a project becomes resident and cached for the
    /// life of the process, and until `config_is_stale` existed only codescout's OWN
    /// write tools re-read it. An edit made in an editor, by `Bash`, or by any other
    /// process was never picked up — so a project flipped to
    /// `file_write_enabled = false` on disk kept ACCEPTING WRITES, with no error on
    /// any surface. That is the direction an operator cares about most, because it
    /// fails toward permitting.
    ///
    /// **The write below MUST be `std::fs::write`, and that is the whole test.**
    /// Driving it through `edit_file` would call `reload_config_if_project_toml`,
    /// which refreshes the config by an entirely different route, so the test would
    /// pass against the broken code and prove nothing. Anyone "tidying" this onto the
    /// tool API deletes the coverage and leaves the assertion green.
    #[tokio::test]
    async fn an_out_of_band_project_toml_edit_reaches_the_next_gated_call() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let toml = dir.path().join(".codescout").join("project.toml");

        std::fs::write(
            &toml,
            "[project]\nname = \"t\"\n\n[security]\nfile_write_enabled = true\n",
        )
        .unwrap();

        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        assert!(
            agent.security_config().await.file_write_enabled,
            "control: the project must START writable, or the flip below proves nothing"
        );

        // `true` -> `false` changes the file's LENGTH as well as its mtime, and that
        // is load-bearing rather than incidental: the stamp is (mtime, len), so on a
        // filesystem with coarse mtime granularity a same-length rewrite inside one
        // tick would be invisible. Keep these two values different in length.
        std::fs::write(
            &toml,
            "[project]\nname = \"t\"\n\n[security]\nfile_write_enabled = false\n",
        )
        .unwrap();

        assert!(
            !agent.security_config().await.file_write_enabled,
            "an out-of-band edit must reach the gate: before the fix the project kept \
             accepting writes for the life of the process, printing nothing"
        );
    }

    /// `security_config_for` is the PINNED twin and a SEPARATE code path — the
    /// staleness check is implemented once in each, so the test above cannot cover
    /// this one. `CLAUDE.md` § *Testing Discipline*: mutate once per guarded SITE,
    /// not once per feature; here applied before the mutation rather than after.
    #[tokio::test]
    async fn an_out_of_band_edit_reaches_the_pinned_security_config_too() {
        let home = tempdir().unwrap();
        let pinned = tempdir().unwrap();
        std::fs::create_dir_all(home.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(pinned.path().join(".codescout")).unwrap();
        let toml = pinned.path().join(".codescout").join("project.toml");

        // `shell_command_mode` rather than `file_write_enabled`, deliberately: a
        // pinned non-home workspace defaults to read-only, and
        // `project_security_config` forces `file_write_enabled = false` for a
        // read-only project regardless of config — so that field would assert
        // vacuously here, passing whether or not the reload happened.
        // `shell_command_mode` passes through untouched, and it is also the sharpest
        // case in the bug's § Scope.
        std::fs::write(
            &toml,
            "[project]\nname = \"p\"\n\n[security]\nshell_command_mode = \"warn\"\n",
        )
        .unwrap();

        let agent = Agent::new(Some(home.path().to_path_buf())).await.unwrap();
        assert_eq!(
            agent
                .security_config_for(Some(pinned.path()))
                .await
                .shell_command_mode,
            "warn",
            "control: the pinned project must START permissive"
        );

        std::fs::write(
            &toml,
            "[project]\nname = \"p\"\n\n[security]\nshell_command_mode = \"disabled\"\n",
        )
        .unwrap();

        assert_eq!(
            agent
                .security_config_for(Some(pinned.path()))
                .await
                .shell_command_mode,
            "disabled",
            "a project flipped to disabled on disk kept EXECUTING shell commands for \
             the life of the process — the permissive-direction failure"
        );
    }

    #[tokio::test]
    async fn activate_non_home_with_read_only_false_is_writable() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();
        std::fs::create_dir_all(dir1.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir2.path().join(".codescout")).unwrap();

        let agent = Agent::new(Some(dir1.path().to_path_buf())).await.unwrap();
        agent
            .activate(dir2.path().to_path_buf(), Some(false))
            .await
            .unwrap();

        let config = agent.security_config().await;
        assert!(
            config.file_write_enabled,
            "explicit read_only=false should enable writes"
        );
    }

    #[tokio::test]
    async fn activate_home_defaults_to_writable() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();
        std::fs::create_dir_all(dir1.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir2.path().join(".codescout")).unwrap();

        let agent = Agent::new(Some(dir1.path().to_path_buf())).await.unwrap();

        // Switch away (read-only)
        agent
            .activate(dir2.path().to_path_buf(), None)
            .await
            .unwrap();
        assert!(!agent.security_config().await.file_write_enabled);

        // Return home
        agent
            .activate(dir1.path().to_path_buf(), None)
            .await
            .unwrap();
        assert!(
            agent.security_config().await.file_write_enabled,
            "home project should always be writable"
        );
    }

    #[tokio::test]
    async fn activate_home_with_read_only_true_is_honoured() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();

        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();

        // The home root, explicitly asked to be protected. Until 2026-09-02 the
        // `_ if is_home => false` guard arm sat ABOVE the explicit case and
        // swallowed this, which made `read_only: true` inert at EVERY root —
        // home came back writable, and a foreign root was already protected by
        // default, so no root could observe the flag. See
        // docs/issues/archive/2026-09-02-read-only-true-is-inert-at-every-root.md
        agent
            .activate(dir.path().to_path_buf(), Some(true))
            .await
            .unwrap();

        let config = agent.security_config().await;
        assert!(
            !config.file_write_enabled,
            "explicit read_only=true must protect the home root; if this fails, \
             an is_home branch is shadowing the caller's explicit request again"
        );
    }

    /// The whole domain of the read-only rule, at its single site.
    ///
    /// Six rows, and the discriminating one is `Some(true) + home`: before the
    /// rule was extracted it returned `false`, agreeing with `None + home`, and
    /// that agreement is what made the flag inert. A foreign-root-only test is
    /// monotone under the defect — deleting the explicit case entirely leaves
    /// both foreign rows correct — so the home rows are the coverage here.
    #[test]
    fn resolve_read_only_covers_its_whole_domain() {
        // An explicit request wins at either root.
        assert!(!AgentInner::resolve_read_only(Some(false), true));
        assert!(AgentInner::resolve_read_only(Some(true), true));
        assert!(!AgentInner::resolve_read_only(Some(false), false));
        assert!(AgentInner::resolve_read_only(Some(true), false));
        // Absent a request: home is read-write, a foreign root is protected.
        assert!(!AgentInner::resolve_read_only(None, true));
        assert!(AgentInner::resolve_read_only(None, false));
    }

    /// Pins the agreement between `activate_within_workspace`'s two branches.
    ///
    /// The already-activated branch applies `read_only` directly rather than
    /// through the shared rule, so it honoured `Some(true)` even while the other
    /// branch swallowed it. That disagreement is what established the swallow as
    /// a defect rather than intent — this test exists so the two cannot drift
    /// apart again. The root project is the case that reaches this branch:
    /// `Agent::new` activates it, so it is never the not-yet-activated path.
    #[tokio::test]
    async fn activate_within_workspace_honours_read_only_true_on_the_root_project() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();

        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        assert!(
            agent.security_config().await.file_write_enabled,
            "home starts writable — otherwise this test asserts nothing"
        );

        agent
            .activate_within_workspace(crate::workspace::ROOT_PROJECT_ID, Some(true))
            .await
            .unwrap();

        assert!(
            !agent.security_config().await.file_write_enabled,
            "read_only=true on the root project must protect it"
        );
    }

    #[tokio::test]
    async fn first_activate_is_writable() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();

        let agent = Agent::new(None).await.unwrap();
        agent
            .activate(dir.path().to_path_buf(), None)
            .await
            .unwrap();

        let config = agent.security_config().await;
        assert!(
            config.file_write_enabled,
            "first activated project should be writable (becomes home)"
        );
    }

    #[tokio::test]
    async fn workspace_summary_returns_projects_with_depends_on() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();

        // Create two sub-projects
        let sub_a = root.join("packages").join("api");
        let sub_b = root.join("packages").join("web");
        std::fs::create_dir_all(&sub_a).unwrap();
        std::fs::create_dir_all(&sub_b).unwrap();
        std::fs::write(
            sub_a.join("package.json"),
            r#"{"name":"api","scripts":{"build":"tsc"}}"#,
        )
        .unwrap();
        std::fs::write(
            sub_b.join("package.json"),
            r#"{"name":"web","scripts":{"build":"tsc"}}"#,
        )
        .unwrap();

        let agent = Agent::new(Some(root)).await.unwrap();
        let summary = agent.workspace_summary().await;
        assert!(
            summary.is_some(),
            "multi-project workspace should have summary"
        );
        let projects = summary.unwrap();
        assert!(projects.len() >= 2, "should have at least 2 sub-projects");
        // Each entry should have depends_on field (even if empty)
        for p in &projects {
            let _ = &p.depends_on;
        }
    }

    #[tokio::test]
    async fn workspace_summary_returns_none_for_single_project() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let summary = agent.workspace_summary().await;
        assert!(
            summary.is_none(),
            "single-project workspace should return None"
        );
    }

    #[tokio::test]
    async fn activate_within_workspace_promotes_dormant() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();

        // Create a sub-project
        let sub = root.join("packages").join("api");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(
            sub.join("package.json"),
            r#"{"name":"api","scripts":{"build":"tsc"}}"#,
        )
        .unwrap();

        let agent = Agent::new(Some(root.clone())).await.unwrap();

        // Before: sub-project is Dormant — active_project() returns None after switch_focus
        agent.switch_focus("api").await.unwrap();
        let is_dormant = {
            let inner = agent.inner.read().await;
            inner.active_project().is_none()
        };
        assert!(
            is_dormant,
            "sub-project should be Dormant before activate_within_workspace"
        );

        // Switch back to home first
        agent
            .switch_focus(crate::workspace::ROOT_PROJECT_ID)
            .await
            .unwrap();

        // Now use activate_within_workspace
        agent.activate_within_workspace("api", None).await.unwrap();

        // After: with_project works
        let name = agent
            .with_project(|p| Ok(p.config.project.name.clone()))
            .await
            .unwrap();
        assert!(
            !name.is_empty(),
            "should have loaded config for sub-project"
        );

        // Workspace topology preserved — all original projects still exist
        let project_count = {
            let inner = agent.inner.read().await;
            inner.default_workspace().unwrap().projects.len()
        };
        assert!(
            project_count >= 2,
            "workspace should still have all projects"
        );
    }

    #[tokio::test]
    async fn activate_within_workspace_unknown_id_errors() {
        let dir = tempdir().unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let result = agent.activate_within_workspace("nonexistent", None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn activate_populates_head_sha() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        // Init a git repo so there's a HEAD to read.
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "--allow-empty", "-m", "init"])
            .current_dir(dir.path())
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .output()
            .unwrap();

        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let sha = agent
            .with_project(|p| Ok(p.head_sha.clone()))
            .await
            .unwrap();
        assert!(sha.is_some(), "head_sha should be set for a git project");
        assert!(
            sha.as_ref().unwrap().len() >= 7,
            "SHA should be at least 7 chars"
        );
    }

    #[tokio::test]
    async fn head_sha_none_for_non_git_project() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let sha = agent
            .with_project(|p| Ok(p.head_sha.clone()))
            .await
            .unwrap();
        assert!(sha.is_none(), "head_sha should be None for non-git project");
    }

    #[tokio::test]
    async fn drain_dirty_files_clears_set_and_returns_paths() {
        use std::path::PathBuf;

        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();

        let a = PathBuf::from("/proj/src/a.rs");
        let b = PathBuf::from("/proj/src/b.rs");
        agent.mark_file_dirty(a.clone()).await;
        agent.mark_file_dirty(b.clone()).await;

        let mut drained = agent.drain_dirty_files().await;
        drained.sort();
        assert_eq!(drained, vec![a, b]);

        // Set must be empty after drain
        assert!(agent.drain_dirty_files().await.is_empty());
    }

    /// THE WIRING, not the rules. Every other `build_check` test drives a pure function
    /// directly; this is the only assertion that the trigger is REACHED. Six mutations
    /// killed six tests in `agent::build_check` and none of them would have noticed the
    /// call below going missing — a module can be correct, tested, and unreachable, which
    /// is exactly how `ListFunctions`/`ListDocs` carried a passing suite for months while
    /// no agent could reach a line of it (CLAUDE.md § Testing Discipline).
    ///
    /// Delete `self.note_source_write_for(...)` from `mark_file_dirty_for` and this reds.
    /// Nothing else in the suite does.
    ///
    /// Asserts on `edits` rather than on the resulting state deliberately: `edits` is
    /// recorded before the `enabled` gate, so this test reads nothing from the
    /// environment. Branching on `CODESCOUT_NO_BUILD_CHECK` would make it a skip-guard,
    /// and a skip-guard is monotone under the deletion it exists to catch.
    #[tokio::test]
    async fn marking_a_rust_file_dirty_reaches_the_build_check() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();

        let f = dir.path().join("src/written_by_me.rs");
        agent.mark_file_dirty_for(None, f.clone()).await;

        let edits = agent
            .build_check
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .edits
            .clone();
        assert!(
            edits.iter().any(|p| p.ends_with("src/written_by_me.rs")),
            "mark_file_dirty_for must reach the build check — the trigger hangs off this \
             one method precisely so the nine write-tool call sites cannot each forget it. \
             recorded: {edits:?}"
        );
    }

    /// The extension filter is reached THROUGH the wiring, not merely present in the
    /// module. Without this, `note_write`'s `.rs` check could be inverted and only a
    /// unit test would notice — while the shipped path recorded every markdown edit and
    /// queued a `cargo check` for each.
    #[tokio::test]
    async fn marking_a_non_rust_file_dirty_records_no_build_edit() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();

        agent
            .mark_file_dirty_for(None, dir.path().join("docs/notes.md"))
            .await;

        let edits = agent
            .build_check
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .edits
            .clone();
        assert!(
            edits.is_empty(),
            "a non-Rust write must not arm a cargo check: {edits:?}"
        );
    }

    #[tokio::test]
    async fn session_write_roots_empty_by_default() {
        let dir = tempdir().unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let roots = agent.session_write_roots_snapshot().await;
        assert!(roots.is_empty());
    }

    #[tokio::test]
    async fn add_session_write_root_visible_in_snapshot() {
        let dir = tempdir().unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let extra = dir.path().join("extra");
        agent.add_session_write_root(extra.clone()).await;
        let roots = agent.session_write_roots_snapshot().await;
        assert_eq!(roots, vec![extra]);
    }

    #[tokio::test]
    async fn session_write_roots_cleared_on_reactivation() {
        let dir = tempdir().unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let extra = dir.path().join("extra");
        agent.add_session_write_root(extra.clone()).await;
        // Snapshot shows the root
        let roots = agent.session_write_roots_snapshot().await;
        assert!(
            !roots.is_empty(),
            "root should be visible before re-activation"
        );
        // Re-activate same project
        agent
            .activate(dir.path().to_path_buf(), None)
            .await
            .unwrap();
        // Snapshot is now empty — re-activation created a fresh ActiveProject
        let roots_after = agent.session_write_roots_snapshot().await;
        assert!(
            roots_after.is_empty(),
            "session roots must clear on re-activation"
        );
    }

    /// Build `<tmp>/main` with a `workspace.toml`, plus a linked worktree at
    /// `<tmp>/main/.worktrees/feat` whose `.git` is the `gitdir:` pointer file git
    /// writes. Returns (tmpdir, main_root, worktree_root); the tmpdir must stay
    /// alive for the paths to exist.
    fn worktree_fixture(main_toml: &str) -> (tempfile::TempDir, PathBuf, PathBuf) {
        let tmp = tempdir().unwrap();
        let main = tmp.path().join("main");
        std::fs::create_dir_all(main.join(".codescout")).unwrap();
        std::fs::create_dir_all(main.join(".git")).unwrap();
        std::fs::write(main.join(".codescout").join("workspace.toml"), main_toml).unwrap();

        let wt = main.join(".worktrees").join("feat");
        std::fs::create_dir_all(&wt).unwrap();
        std::fs::write(
            wt.join(".git"),
            format!("gitdir: {}/.git/worktrees/feat\n", main.display()),
        )
        .unwrap();
        (tmp, main, wt)
    }

    /// `.codescout/workspace.toml` is gitignored, so it never travels into a linked
    /// worktree. Before this read-through, `load_discover_settings` fell back to
    /// `(3, vec![])` there — dropping `exclude_projects` and letting sub-project
    /// discovery walk into every `tests/fixtures/*` (measured 2 -> 9 on this repo).
    ///
    /// `discovery_max_depth` is deliberately 5, not the default 3: with 3 the depth
    /// assertion would pass against the fallback and prove nothing.
    #[test]
    fn discover_settings_read_through_to_the_main_checkout_from_a_worktree() {
        let (_tmp, _main, wt) = worktree_fixture(
            "exclude_projects = [\"fixtures\"]\n[workspace]\nname = \"t\"\ndiscovery_max_depth = 5\n",
        );

        let (depth, exclude) = load_discover_settings(&wt);

        assert_eq!(depth, 5, "worktree must inherit the main checkout's depth");
        assert_eq!(
            exclude,
            vec!["fixtures".to_string()],
            "worktree must inherit the main checkout's exclude_projects"
        );
    }

    /// Read-through is a fallback, not an override: a worktree that has its own
    /// `workspace.toml` keeps it. Without this, "inherit from main" would silently
    /// outrank a deliberate per-worktree configuration.
    #[test]
    fn a_worktrees_own_workspace_toml_still_wins_over_the_main_checkouts() {
        let (_tmp, _main, wt) = worktree_fixture(
            "exclude_projects = [\"fixtures\"]\n[workspace]\nname = \"t\"\ndiscovery_max_depth = 5\n",
        );
        std::fs::create_dir_all(wt.join(".codescout")).unwrap();
        std::fs::write(
            wt.join(".codescout").join("workspace.toml"),
            "exclude_projects = [\"local-only\"]\n[workspace]\nname = \"t\"\ndiscovery_max_depth = 7\n",
        )
        .unwrap();

        let (depth, exclude) = load_discover_settings(&wt);

        assert_eq!(depth, 7);
        assert_eq!(exclude, vec!["local-only".to_string()]);
    }

    /// The read-through must key on being a linked worktree, not merely on the file
    /// being absent — otherwise a plain project would start reading configuration
    /// out of whatever directory happened to sit above it.
    #[test]
    fn discover_settings_fall_back_to_defaults_outside_a_worktree() {
        let tmp = tempdir().unwrap();
        let plain = tmp.path().join("plain");
        std::fs::create_dir_all(plain.join(".codescout")).unwrap();
        // A real main checkout: `.git` is a DIRECTORY, so there is no gitdir pointer.
        std::fs::create_dir_all(plain.join(".git")).unwrap();

        let (depth, exclude) = load_discover_settings(&plain);

        assert_eq!(depth, 3);
        assert!(exclude.is_empty());
    }
}
/// `.codescout/system-prompt.md` is served into the `initialize` instructions. The path is
/// checked first (a link or FIFO is never served), but the text had no size bound.
/// docs/issues/2026-10-20-reads-of-repository-files-that-remain-unbounded-after-the-config-loader-fix.md
#[cfg(all(test, unix))]
mod system_prompt_read_tests {
    use super::*;
    use crate::util::fs::special_file_fixtures::{make_fifo, within_deadline};

    fn default() -> Option<String> {
        Some("From TOML".to_string())
    }

    /// Past the limit is "not a prompt": the project.toml default is used, as for an absent
    /// file. The file starts with real text and is then extended sparse, so only the length
    /// check can refuse it (a read would serve the text and the zeros).
    #[test]
    fn an_oversized_prompt_file_is_ignored_for_the_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("system-prompt.md");
        std::fs::write(&path, "Always use pytest.\n").unwrap();
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(MAX_SYSTEM_PROMPT_FILE_BYTES + 1).unwrap();
        drop(file);
        assert_eq!(read_system_prompt_file(&path, default()), default());
        assert_eq!(read_system_prompt_file(&path, None), None);
    }

    /// The same, through the real call site that builds the instructions.
    #[tokio::test]
    async fn project_status_ignores_an_oversized_system_prompt_file() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join(".codescout");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("project.toml"),
            "[project]\nname = \"test\"\nsystem_prompt = \"From TOML\"\n",
        )
        .unwrap();
        let path = config_dir.join("system-prompt.md");
        std::fs::write(&path, "Always use pytest.\n").unwrap();
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(MAX_SYSTEM_PROMPT_FILE_BYTES + 1).unwrap();
        drop(file);

        let agent = Agent::new(None).await.unwrap();
        agent
            .activate(dir.path().to_path_buf(), None)
            .await
            .unwrap();
        let status = agent.project_status().await.unwrap();
        assert_eq!(status.system_prompt.as_deref(), Some("From TOML"));
    }

    /// A FIFO, and a link to one, are not served and do not hold the call.
    #[test]
    fn a_fifo_or_a_link_to_one_is_ignored_for_the_default_and_never_a_hang() {
        for linked in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("system-prompt.md");
            if linked {
                let fifo = dir.path().join("elsewhere-pipe");
                make_fifo(&fifo);
                std::os::unix::fs::symlink(&fifo, &path).unwrap();
            } else {
                make_fifo(&path);
            }
            let got = within_deadline("read_system_prompt_file on a FIFO", move || {
                read_system_prompt_file(&path, default())
            });
            assert_eq!(got, default(), "linked={linked}");
        }
    }

    /// Control: the other cases give what they always gave.
    #[test]
    fn regular_missing_empty_and_invalid_prompt_files_read_as_before() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("system-prompt.md");

        assert_eq!(
            read_system_prompt_file(&path, default()),
            default(),
            "missing"
        );

        std::fs::write(&path, "Always use pytest.\n").unwrap();
        assert_eq!(
            read_system_prompt_file(&path, default()).as_deref(),
            Some("Always use pytest.\n")
        );

        std::fs::write(&path, "").unwrap();
        assert_eq!(
            read_system_prompt_file(&path, default()).as_deref(),
            Some("")
        );

        // An unreadable regular file is `None` (not the default), as `.ok()` made it.
        std::fs::write(&path, [0xff, 0xfe]).unwrap();
        assert_eq!(
            read_system_prompt_file(&path, default()),
            None,
            "invalid UTF-8"
        );

        // A file of exactly the limit is still served.
        std::fs::write(&path, "x".repeat(MAX_SYSTEM_PROMPT_FILE_BYTES as usize)).unwrap();
        assert_eq!(
            read_system_prompt_file(&path, default()).map(|s| s.len()),
            Some(MAX_SYSTEM_PROMPT_FILE_BYTES as usize)
        );

        // A link to a regular file is never served.
        std::fs::remove_file(&path).unwrap();
        let real = dir.path().join("real.md");
        std::fs::write(&real, "SECRET").unwrap();
        std::os::unix::fs::symlink(&real, &path).unwrap();
        assert_eq!(read_system_prompt_file(&path, default()), default(), "link");
    }
}
