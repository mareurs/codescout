//! Central orchestrator: manages projects, tool registry, and shared state.

mod write_guard;
#[allow(unused_imports)]
pub(crate) use write_guard::{acquire as acquire_write_guard, open_lock_file, WriteGuard};

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
    /// Phase 1: holds at most one entry — `activate` clears and reinserts,
    /// mirroring the previous single-slot drop-and-replace, so behavior is
    /// unchanged. Phase 3 lifts the clear-on-activate to enable true
    /// multi-workspace residence + eviction. See
    /// docs/plans/2026-05-30-per-request-workspace-pinning.md.
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
    /// Last `activate()` as (root, when). Drives the concurrent-activation
    /// guard (`Agent::note_activation`): if a *different* root is activated
    /// under this shared server within a short window, the activate response
    /// carries a `concurrent_activation_warning`. See
    /// docs/issues/archive/2026-05-30-shared-server-global-active-project-race.md
    pub last_activation: Option<(PathBuf, std::time::Instant)>,
}

impl AgentInner {
    /// The workspace that unpinned calls resolve to (the per-session default).
    /// Phase 1 this is the single live workspace; the ambient accessors below
    /// route through it. Phase 2+ adds selector-aware twins alongside.
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

    /// Absolute path to the project root on disk.
    pub fn root(&self) -> &Path {
        &self.root
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
    let ws_path = crate::config::workspace::workspace_config_path(root);
    let content = std::fs::read_to_string(&ws_path).ok()?;
    let ws = toml::from_str::<crate::config::workspace::WorkspaceConfig>(&content).ok()?;
    Some((ws.workspace.discovery_max_depth, ws.exclude_projects))
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
    let mut config = p.config.security.to_path_security_config();
    config.library_paths = p
        .library_registry
        .all()
        .iter()
        .map(|e| e.path.clone())
        .collect();
    if p.read_only {
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
    // docs/issues/2026-08-26-workspace-read-only-flips-mid-session.md
    let cause = crate::util::path_security::WriteBlockCause::classify(
        p.config.security.file_write_enabled,
        p.read_only,
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
            let root = std::fs::canonicalize(&raw).unwrap_or(raw);
            let config = ProjectConfig::load_or_default(&root)?;
            let memory = MemoryStore::open(&root)?;
            let private_memory = MemoryStore::open_private(&root)?;
            let registry_path = root.join(".codescout").join("libraries.json");
            let library_registry = LibraryRegistry::load(&registry_path).unwrap_or_default();
            let home = root.clone();

            let active = ActiveProject {
                root: root.clone(),
                config,
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
            })),
            indexing: Arc::new(std::sync::Mutex::new(IndexingState::Idle)),
            nudged_libraries: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
            embedding_semaphore: Arc::new(tokio::sync::Semaphore::new(2)),
            library_index_states: Arc::new(std::sync::Mutex::new(HashMap::new())),
            active_sync_abort: Arc::new(std::sync::Mutex::new(None)),
            semantic_memory: Arc::new(tokio::sync::OnceCell::new()),
            memory_embedder: Arc::new(tokio::sync::OnceCell::new()),
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
        let root = std::fs::canonicalize(&root).unwrap_or(root);
        let res = Self::load_project_resources(&root).await?;
        {
            let mut inner = self.inner.write().await;
            // build_workspace computes is_home / read_only and reuses an
            // existing root's locks, all under this write lock (no TOCTOU).
            let ws = inner.build_workspace(&root, read_only, res);
            if inner.home_root.is_none() {
                inner.home_root = Some(root.clone());
            }
            // Phase 1: single-entry default registry — clear + reinsert mirrors
            // the previous single-slot drop-and-replace. ensure_resident adds
            // pinned entries alongside without clearing.
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
        let config = ProjectConfig::load_or_default(root)?;
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
    /// non-home workspaces default to read-only. Idempotent — EXCEPT that
    /// passing `Some(false)` on an already-resident, currently-read-only entry
    /// upgrades it to writable (never downgrades an already-writable entry).
    /// This lets a write-tool call pin a workspace it was never separately
    /// `activate`d into without requiring a full `activate` (which would clear
    /// every other resident workspace — see `Agent::activate`).
    pub async fn ensure_resident(&self, root: PathBuf, read_only: Option<bool>) -> Result<()> {
        let root = std::fs::canonicalize(&root).unwrap_or(root);
        {
            let mut inner = self.inner.write().await;
            if let Some(ws) = inner.workspaces.get_mut(&root) {
                if read_only == Some(false) {
                    if let Some(p) = ws.focused_active_mut().and_then(|p| p.as_active_mut()) {
                        p.read_only = false;
                    }
                }
                return Ok(());
            }
        }
        let res = Self::load_project_resources(&root).await?;
        let mut inner = self.inner.write().await;
        // Re-check under the write lock — another caller may have inserted it
        // while we did the lock-free I/O.
        if let Some(ws) = inner.workspaces.get_mut(&root) {
            if read_only == Some(false) {
                if let Some(p) = ws.focused_active_mut().and_then(|p| p.as_active_mut()) {
                    p.read_only = false;
                }
            }
            return Ok(());
        }
        let ws = inner.build_workspace(&root, read_only, res);
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
            self.ensure_resident(root.to_path_buf(), None).await?;
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
    pub async fn security_config_for(
        &self,
        workspace_override: Option<&Path>,
    ) -> crate::util::path_security::PathSecurityConfig {
        self.with_project_at(workspace_override, |p| Ok(project_security_config(p)))
            .await
            .unwrap_or_default()
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
    pub async fn mark_file_dirty_for(&self, workspace_override: Option<&Path>, path: PathBuf) {
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
    /// Phase 4a holds the single `AgentInner` write lock for the closure's
    /// duration; the closure MUST stay non-blocking (no `.await` on a per-project
    /// lock) per `## Phase 4 — Lock-Ordering Proof`. Phase 4b moves this onto the
    /// per-`Workspace` lock.
    pub async fn with_project_at_mut<F, T>(
        &self,
        workspace_override: Option<&Path>,
        f: F,
    ) -> Result<T>
    where
        F: FnOnce(&mut ActiveProject) -> Result<T>,
    {
        if let Some(root) = workspace_override {
            self.ensure_resident(root.to_path_buf(), None).await?;
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
                    if let Ok(fresh) =
                        crate::config::project::ProjectConfig::load_or_default(&p.root)
                    {
                        p.config = fresh;
                    }
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
    /// See docs/issues/archive/2026-05-30-shared-server-global-active-project-race.md
    fn concurrent_switch_warning(
        prev: Option<(&std::path::Path, std::time::Duration)>,
        new_root: &std::path::Path,
        window: std::time::Duration,
    ) -> Option<String> {
        match prev {
            Some((prev_root, since)) if prev_root != new_root && since < window => Some(format!(
                "active project switched from {} to {} {:?} ago — another caller \
                 (e.g. a concurrent subagent) shares this server's single \
                 active-project slot, so reads may resolve against the wrong \
                 workspace. Fix: pass workspace=<absolute path> on each tool call \
                 to pin resolution per-request instead of activating. For fully \
                 independent parallel work, separate client windows also \
                 isolate (separate processes = separate slots).",
                prev_root.display(),
                new_root.display(),
                since
            )),
            _ => None,
        }
    }

    /// Record this activation and return a warning if it rapidly replaced a
    /// *different* recently-activated root. Best-effort drift signal — it cannot
    /// prevent the race (the active project is process-global shared state), only
    /// surface it. The real fix is per-request workspace pinning; see the bug file.
    pub async fn note_activation(&self, root: &std::path::Path) -> Option<String> {
        let mut inner = self.inner.write().await;
        let prev = inner
            .last_activation
            .as_ref()
            .map(|(p, at)| (p.as_path(), at.elapsed()));
        let warning =
            Self::concurrent_switch_warning(prev, root, Self::CONCURRENT_ACTIVATION_WINDOW);
        inner.last_activation = Some((root.to_path_buf(), std::time::Instant::now()));
        warning
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
                let system_prompt = if prompt_file.exists() {
                    std::fs::read_to_string(&prompt_file).ok()
                } else {
                    default_prompt
                };
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
            std::fs::read_to_string(crate::config::workspace::workspace_config_path(&ws.root))
                .ok()
                .and_then(|s| toml::from_str(&s).ok());

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
    pub async fn reload_config_if_project_toml(&self, path: &std::path::Path) {
        let mut inner = self.inner.write().await;
        if let Some(ref mut p) = inner.active_project_mut() {
            let toml_path = p.root.join(".codescout").join("project.toml");
            if path == toml_path {
                if let Ok(fresh) = crate::config::project::ProjectConfig::load_or_default(&p.root) {
                    p.config = fresh;
                }
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
            let _ = self.ensure_resident(root.to_path_buf(), None).await;
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
    pub async fn security_config(&self) -> crate::util::path_security::PathSecurityConfig {
        let inner = self.inner.read().await;
        match inner.active_project() {
            Some(p) => project_security_config(p),
            None => crate::util::path_security::PathSecurityConfig::default(),
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
        let (should_index, root, entry_path, max_index_bytes, ignore_patterns) = {
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
                p.config.ignored_paths.patterns.clone(),
            )
        };
        if !should_index {
            return;
        }

        let name = lib_name.to_string();

        // Scope guard. This background path has no interactive user to confirm an
        // oversized/suspicious root, so a giant un-ignored tree must be *skipped*
        // here rather than walked. The streaming indexer keeps any single pass
        // memory-safe, but auto-indexing a dependency tree (e.g. a `.venv` with tens
        // of thousands of files) still wastes embed-server work and pollutes the
        // code index. Mirror the `index` tool's preflight; on "needs confirmation",
        // decline. (docs/issues/archive/2026-06-19-mcp-server-oom-68gb.md)
        let scope_root = entry_path.clone();
        let pf_patterns = ignore_patterns.clone();
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
                     (raise security.max_index_bytes or set [ignored_paths] to index it)"
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
                let opts = crate::retrieval::sync::SyncOpts {
                    ignore_patterns: ignore_patterns.clone(),
                    ..Default::default()
                };
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

    /// Lazily construct (or return cached) the dense embedder for memory ops.
    ///
    /// First call performs `RetrievalClient::from_env()` (one network probe)
    /// and wraps the resulting `client.embedder` (`Arc<dyn CodeEmbedder>`) in
    /// [`crate::retrieval::embedder::CodeDenseAdapter`], rather than building a
    /// second, independent embedder. This means memory recall always rides
    /// whatever backend code search selected — HTTP or (once the local ONNX
    /// path lands) in-process — instead of having its own selection path that
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
    async fn ensure_resident_upgrades_read_only_pin_to_writable() {
        // FINDING (docs/issues/archive/2026-07-09-edit-code-write-path-ignores-workspace-pin.md,
        // "Live-verification finding"): ensure_resident's non-home default is
        // read-only, and every internal caller passed None — so a workspace
        // pin could never become writable without a full `activate` (which
        // clears every other resident workspace). ensure_resident(root,
        // Some(false)) must upgrade an already-resident, read-only entry in
        // place instead of no-op'ing on the idempotence check.
        let dir_a = tempdir().unwrap();
        let dir_b = tempdir().unwrap();
        std::fs::create_dir_all(dir_a.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir_b.path().join(".codescout")).unwrap();
        let root_a = canonical(dir_a.path());

        let agent = Agent::new(Some(dir_b.path().to_path_buf())).await.unwrap();

        // First touch (read-oriented default): A becomes resident, read-only.
        agent.ensure_resident(root_a.clone(), None).await.unwrap();
        let read_only_before = agent
            .with_project_at(Some(&root_a), |p| Ok(p.read_only))
            .await
            .unwrap();
        assert!(read_only_before, "fresh pin must default to read-only");

        // Upgrade: the SAME already-resident entry must flip to writable.
        agent
            .ensure_resident(root_a.clone(), Some(false))
            .await
            .unwrap();
        let read_only_after = agent
            .with_project_at(Some(&root_a), |p| Ok(p.read_only))
            .await
            .unwrap();
        assert!(
            !read_only_after,
            "ensure_resident(Some(false)) must upgrade an already-resident \
             read-only entry to writable"
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
        assert!(Agent::concurrent_switch_warning(None, a, window).is_none());

        // Rapid switch to a DIFFERENT root → warning (the subagent-race signature).
        // The message must recommend per-request pinning as the primary fix and
        // separate windows as the fallback — both are guidance contracts.
        let w = Agent::concurrent_switch_warning(Some((a, Duration::from_millis(200))), b, window);
        assert!(w.as_deref().is_some_and(|s| {
            s.contains("workspace=<absolute path>") && s.contains("separate client windows")
        }));

        // Same-root re-activation → silent (normal return-home / re-activate).
        assert!(
            Agent::concurrent_switch_warning(Some((a, Duration::from_millis(200))), a, window)
                .is_none()
        );

        // Different root but OUTSIDE the window (slow sequential switch) → silent.
        assert!(
            Agent::concurrent_switch_warning(Some((a, Duration::from_secs(60))), b, window)
                .is_none()
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

    #[tokio::test]
    async fn new_with_relative_path_canonicalizes_home_root() {
        let dir = tempdir().unwrap();
        let canonical = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();

        // Simulate --project with a relative path by constructing one that
        // points to the same directory.  We use the tempdir's last component
        // as a relative path from its parent.
        let parent = canonical.parent().unwrap();
        let rel = canonical.file_name().unwrap();

        // Save and restore CWD so the test doesn't affect others.
        let orig_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(parent).unwrap();
        let agent = Agent::new(Some(PathBuf::from(rel))).await.unwrap();
        std::env::set_current_dir(&orig_cwd).unwrap();

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
