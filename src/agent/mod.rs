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
}

pub struct AgentInner {
    pub workspace: Option<Workspace>,
    pub project_explicitly_activated: bool,
    pub home_root: Option<PathBuf>,
    /// Last `activate()` as (root, when). Drives the concurrent-activation
    /// guard (`Agent::note_activation`): if a *different* root is activated
    /// under this shared server within a short window, the activate response
    /// carries a `concurrent_activation_warning`. See
    /// docs/issues/2026-05-30-shared-server-global-active-project-race.md
    pub last_activation: Option<(PathBuf, std::time::Instant)>,
}

impl AgentInner {
    /// Convenience: get `&ActiveProject` from the focused workspace project.
    pub fn active_project(&self) -> Option<&ActiveProject> {
        self.workspace.as_ref()?.focused_active()?.as_active()
    }

    /// Convenience: get `&mut ActiveProject` from the focused workspace project.
    pub fn active_project_mut(&mut self) -> Option<&mut ActiveProject> {
        self.workspace
            .as_mut()?
            .focused_active_mut()?
            .as_active_mut()
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

/// Read `workspace.toml` (if present) and return the discovery depth and exclude list.
/// Falls back to defaults (depth=3, no excludes) when the file is missing or unparseable.
fn load_discover_settings(root: &std::path::Path) -> (usize, Vec<String>) {
    let ws_path = crate::config::workspace::workspace_config_path(root);
    if let Ok(content) = std::fs::read_to_string(&ws_path) {
        if let Ok(ws) = toml::from_str::<crate::config::workspace::WorkspaceConfig>(&content) {
            return (ws.workspace.discovery_max_depth, ws.exclude_projects);
        }
    }
    (3, vec![])
}

/// Resolve the short git HEAD SHA for a directory. Returns None if not a git repo.
fn resolve_head_sha(root: &Path) -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
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

        Ok(Self {
            inner: Arc::new(RwLock::new(AgentInner {
                workspace,
                project_explicitly_activated,
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
        })
    }

    /// Activate a project by path, replacing the current workspace.
    pub async fn activate(&self, root: PathBuf, read_only: Option<bool>) -> Result<()> {
        // Canonicalize up-front so every downstream consumer sees the same
        // absolute path. Without this, activate(".") would compare unequal
        // to Agent::new's canonicalized home_root, making is_home return
        // false on the very first re-activation and flipping the project
        // to read-only unexpectedly.
        let root = std::fs::canonicalize(&root).unwrap_or(root);
        // Load all resources outside any lock — I/O is independent of is_home.
        let config = ProjectConfig::load_or_default(&root)?;
        let memory = MemoryStore::open(&root)?;
        let private_memory = MemoryStore::open_private(&root)?;
        let registry_path = root.join(".codescout").join("libraries.json");
        let library_registry = LibraryRegistry::load(&registry_path).unwrap_or_default();
        let head_sha = resolve_head_sha(&root);

        // Discover sub-projects before acquiring the write lock.
        // Respect depth and exclude settings from workspace.toml if it exists.
        // Walked on a blocking thread — see Agent::new for rationale.
        let (discover_depth, discover_exclude) = load_discover_settings(&root);
        let discovered = {
            let root = root.clone();
            let exclude = discover_exclude.clone();
            tokio::task::spawn_blocking(move || discover_projects(&root, discover_depth, &exclude))
                .await
                .map_err(|e| anyhow::anyhow!("discover_projects task failed: {e}"))?
        };

        // Open the lock file before acquiring the write lock — involves blocking
        // fs I/O (create_dir_all + OpenOptions::open) that must not run on the
        // async executor while holding a write guard. This fresh handle may be
        // discarded if we find we're re-activating the same root (in which case
        // the existing file_lock is reused for correct serialization).
        let fresh_file_lock = write_guard::open_lock_file(&root)
            .with_context(|| format!("failed to open write.lock for {}", root.display()))?;

        {
            let mut inner = self.inner.write().await;

            // Compute is_home and effective_read_only under the write lock so
            // there is no TOCTOU window between checking home_root and using the
            // result.  (Previously is_home was read under a short read lock, then
            // the lock was dropped while I/O ran, then a write lock was acquired —
            // a concurrent activate() could have changed home_root in between.)
            let is_home = inner.home_root.as_ref().map(|h| *h == root).unwrap_or(true);
            let effective_read_only = match read_only {
                Some(false) => false,
                _ if is_home => false,
                _ => true,
            };

            // Re-activating the same root must keep the SAME write_lock,
            // file_lock, and dirty_files — otherwise an in-flight tool holding
            // the old locks does not serialize against new tools using the new
            // locks, and two writers can race on the same project. Scan the
            // current workspace for an already-activated project at this root.
            let existing = inner.workspace.as_ref().and_then(|ws| {
                ws.projects.iter().find_map(|p| match &p.state {
                    ProjectState::Activated(ap) if ap.root == root => Some((
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
                root: root.clone(),
                config,
                memory,
                private_memory,
                library_registry,
                dirty_files,
                read_only: effective_read_only,
                head_sha,
                has_git_remote: probe_has_git_remote(&root),
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

            let ws = Workspace::new(root.clone(), projects);

            if inner.home_root.is_none() {
                inner.home_root = Some(root);
            }
            inner.workspace = Some(ws);
            inner.project_explicitly_activated = true;
        }
        Ok(())
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
    /// See docs/issues/2026-05-30-shared-server-global-active-project-race.md
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
                 workspace. For parallel multi-workspace work use separate Claude \
                 Code windows (separate processes = separate slots).",
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
            .workspace
            .as_ref()
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
            .workspace
            .as_mut()
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
                .workspace
                .as_ref()
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
        let effective_read_only_snapshot = match read_only {
            Some(false) => false,
            _ if is_home_snapshot => false,
            _ => true,
        };
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
            .workspace
            .as_mut()
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

        // Determine read_only: explicit > home (always rw) > default (ro)
        let is_home = home_root.as_ref().map(|h| *h == abs_root).unwrap_or(false);
        let effective_read_only = match read_only {
            Some(false) => false,
            _ if is_home => false,
            _ => true,
        };

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
        let memory = MemoryStore::open(&abs_root)?;
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
            .workspace
            .as_ref()
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
            Some((
                project.config.project.name.clone(),
                project.root.display().to_string(),
                project.root.clone(),
                project.config.project.languages.clone(),
                project.memory.clone(),
                db_path,
                prompt_file,
                project.config.project.system_prompt.clone(),
            ))
        }?; // lock dropped here

        // Phase 2: blocking filesystem reads off the executor
        let (memories, has_index, system_prompt, worktree) =
            tokio::task::spawn_blocking(move || {
                let memories = memory_store.list().unwrap_or_default();
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
        let ws = inner.workspace.as_ref()?;
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
                    root: p.discovered.relative_root.display().to_string(),
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

    /// Returns the canonical project_id used for call-edge cache entries.
    ///
    /// This is the focused sub-project name (e.g. `"codescout"`) when a
    /// workspace is active, or `ROOT_PROJECT_ID` otherwise. Must match the
    /// value used by the `call_graph` tool when it upserts edges — both sides
    /// call this method so they always agree.
    pub async fn call_edges_project_id(&self) -> String {
        let inner = self.inner.read().await;
        inner
            .workspace
            .as_ref()
            .and_then(|ws| ws.focused.clone())
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
        inner.workspace.as_ref()?.focused_project_root().ok()
    }

    pub async fn is_project_explicitly_activated(&self) -> bool {
        self.inner.read().await.project_explicitly_activated
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
            .workspace
            .as_ref()
            .map(|ws| ws.projects.iter().map(|p| p.discovered.clone()).collect())
            .unwrap_or_default()
    }

    /// Returns per-project memory topic lists for all workspace projects that have memories.
    /// Returns an empty vec for single-project activations (workspace absent or len ≤ 1).
    pub async fn workspace_project_memories(&self) -> Vec<(String, Vec<String>)> {
        let inner = self.inner.read().await;
        let ws = match inner.workspace.as_ref() {
            Some(ws) if ws.projects.len() > 1 => ws,
            _ => return vec![],
        };
        ws.projects
            .iter()
            .filter_map(|p| {
                let dir = ws.memory_dir_for_project(&p.discovered.id);
                let topics = crate::memory::MemoryStore::from_dir(dir)
                    .ok()?
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
            Some(p) => {
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
                config
            }
            None => crate::util::path_security::PathSecurityConfig::default(),
        }
    }

    /// Resolve the per-language `mux` override from the active project's config.
    /// Returns `None` when no project is active or no override is set for the language.
    pub async fn lsp_mux_override(&self, language: &str) -> Option<bool> {
        self.with_project(|p| Ok(p.config.lsp.langs.get(language).and_then(|o| o.mux)))
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
        let (should_index, _root, entry_path) = {
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
            (true, p.root.clone(), entry.path.clone())
        };
        if !should_index {
            return;
        }

        let name = lib_name.to_string();
        let lib_project_id = format!("lib:{}", name);
        self.set_library_state(&name, LibraryIndexState::Indexing { done: 0, total: 0 });

        let self_clone = self.clone();
        let sync_abort_for_task = self.active_sync_abort.clone();
        let sync_abort_for_store = self.active_sync_abort.clone();
        let task = tokio::spawn(async move {
            tracing::info!("Auto-indexing library '{}' in background...", name);
            let result = async {
                let client = crate::retrieval::client::RetrievalClient::from_env().await?;
                let opts = crate::retrieval::sync::SyncOpts::default();
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
    /// Lazily construct (or return cached) the Qdrant-backed semantic memory store.
    ///
    /// First call performs `RetrievalClient::from_env()` (one network probe to the
    /// embedder) and bootstraps the `memories` collection. Subsequent calls return
    /// the cached `Arc` without further I/O.
    ///
    /// In tests, pre-populate via `set_semantic_memory_store_for_test` to bypass
    /// the env-driven construction path.
    pub async fn semantic_memory_store(&self) -> anyhow::Result<Arc<dyn SemanticMemoryStore>> {
        self.semantic_memory
            .get_or_try_init(|| async {
                let client = crate::retrieval::client::RetrievalClient::from_env().await?;
                let collection = client.config.collection("memories");
                let dim = client.config.model_dim as u64;
                let store = crate::memory::semantic_store::QdrantSemanticMemoryStore::new(
                    client.qdrant,
                    collection,
                    dim,
                )
                .await?;
                anyhow::Ok(Arc::new(store) as Arc<dyn SemanticMemoryStore>)
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
    /// and wraps the resulting `EmbedderHttp` in [`HttpDenseEmbedder`].
    /// Subsequent calls share the cached `Arc`.
    ///
    /// In tests, pre-populate via [`Agent::set_memory_embedder_for_test`] to
    /// bypass the env-driven construction path.
    pub async fn memory_embedder(
        &self,
    ) -> anyhow::Result<Arc<dyn crate::retrieval::embedder::DenseEmbedder>> {
        self.memory_embedder
            .get_or_try_init(|| async {
                let client = crate::retrieval::client::RetrievalClient::from_env().await?;
                let emb = crate::retrieval::embedder::HttpDenseEmbedder::new(client.embedder);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// Canonicalize a path. On macOS this resolves the `/var` → `/private/var`
    /// symlink that `tempfile::tempdir` returns un-canonicalized but production
    /// code paths canonicalize via `std::fs::canonicalize`.
    fn canonical(p: &std::path::Path) -> std::path::PathBuf {
        std::fs::canonicalize(p).expect("path canonicalizes")
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
        let w = Agent::concurrent_switch_warning(Some((a, Duration::from_millis(200))), b, window);
        assert!(w
            .as_deref()
            .is_some_and(|s| s.contains("separate Claude Code windows")));

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
        assert!(status.path.contains(canonical_dir.to_str().unwrap()));
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
    async fn activate_home_always_writable() {
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
            inner.workspace.as_ref().unwrap().projects.len()
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
}
