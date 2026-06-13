use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;

use crate::librarian::catalog::Catalog;
use crate::librarian::classify::CompiledRule;
use crate::librarian::workspace::WorkspaceConfig;

pub mod find;
pub mod gather;
pub mod get;
pub mod graph;
pub mod scope;

/// Statuses hidden by default from `find` and `context` listings.
///
/// Single source of truth shared by `find.rs` and `context.rs` so the two
/// surfaces cannot drift — they did once: `retired` was added to `find` but
/// not `context` (see
/// docs/issues/2026-05-25-hidden-statuses-context-missing-retired.md).
///
/// - `archived` / `superseded`: terminal; the file is physically moved to an
///   `archive/` path.
/// - `retired`: terminal but kept in place (MRV in-place redirect — the file
///   stays at its original path so incoming links still resolve, and its body
///   forwards to the canonical successor).
pub(crate) const HIDDEN_STATUSES: &[&str] = &["archived", "superseded", "retired"];

/// A recoverable tool error: the LLM gave bad input and can self-correct.
///
/// When a tool returns this error type, the MCP server serialises it as
/// `isError: false` with a JSON body containing `"error"` and an optional
/// `"hint"`. This prevents Claude Code from aborting sibling parallel tool
/// calls (which it does when it sees `isError: true`).
///
/// Use this for **expected, input-driven failures**: unknown event kind,
/// missing required payload field, intent already resolved, target event
/// not found, etc.
///
/// Keep returning plain `anyhow` errors (→ `isError: true`) for genuine
/// bugs: panics, security violations, IO/database failures.
#[derive(Debug)]
pub struct RecoverableError {
    pub message: String,
    pub hint: Option<String>,
}

impl std::fmt::Display for RecoverableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)?;
        if let Some(h) = &self.hint {
            write!(f, " (hint: {h})")?;
        }
        Ok(())
    }
}

impl std::error::Error for RecoverableError {}

impl RecoverableError {
    /// Construct a recoverable error wrapped in `anyhow::Error` so it can
    /// flow through `Result<_, anyhow::Error>` tool calls via `?`.
    ///
    /// Returns `anyhow::Error` rather than `Self` so call sites read like
    /// the `anyhow!(...)` macro they replace.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(msg: impl Into<String>) -> anyhow::Error {
        anyhow::Error::new(Self {
            message: msg.into(),
            hint: None,
        })
    }

    pub fn with_hint(msg: impl Into<String>, hint: impl Into<String>) -> anyhow::Error {
        anyhow::Error::new(Self {
            message: msg.into(),
            hint: Some(hint.into()),
        })
    }
}

pub struct ToolContext {
    pub catalog: Arc<parking_lot::Mutex<Catalog>>,
    pub workspace: Arc<WorkspaceConfig>,
    pub rules: Arc<Vec<CompiledRule>>,
    pub embedding: Option<Arc<crate::librarian::embedding::EmbeddingService>>,
    /// Resolved at server startup from the process cwd. `None` when the cwd
    /// lies outside every configured workspace root; tools then fall back to
    /// workspace-wide scope and surface a hint in their response.
    pub current_project: Option<Arc<crate::librarian::current_project::CurrentProject>>,
}

/// Candidate "managed roots" an artifact may legitimately live under: the
/// legacy workspace `[[roots]]` entries plus the active project's git root
/// and project root.
///
/// Under the `[[project]]` workspace model the active project is resolved
/// into `current_project` and is usually ABSENT from the legacy `roots`
/// registry. A guard that consults only `workspace.roots` therefore rejects
/// every delete/move performed in such a project — see
/// `docs/issues/2026-06-03-artifact-delete-refuses-in-workspace-artifact.md`.
///
/// `git_root` is listed before `abs_path` so a caller that joins a
/// repo-root-relative path (e.g. `mv`) resolves against the repo root rather
/// than a project subdirectory.
pub(crate) fn managed_roots(ctx: &ToolContext) -> Vec<std::path::PathBuf> {
    let mut roots: Vec<std::path::PathBuf> =
        ctx.workspace.roots.iter().map(|r| r.path.clone()).collect();
    if let Some(cp) = ctx.current_project.as_deref() {
        for candidate in [&cp.git_root, &cp.abs_path] {
            if !roots.iter().any(|r| r == candidate) {
                roots.push(candidate.clone());
            }
        }
    }
    roots
}

/// The first managed root that contains `abs_path`, if any.
///
/// Paths are compared lexically: stored `abs_path` values are
/// canonical-absolute (upsert canonicalizes on write) and `current_project`
/// is canonicalized at the adapter boundary (`adapter.rs`), so a lexical
/// `Path::starts_with` is sound. We deliberately do NOT `canonicalize()`
/// `abs_path` at call time — `delete` tolerates an already-removed file and
/// `std::fs::canonicalize` errors on a missing path.
pub(crate) fn containing_root<'a>(
    roots: &'a [std::path::PathBuf],
    abs_path: &std::path::Path,
) -> Option<&'a std::path::PathBuf> {
    roots.iter().find(|root| abs_path.starts_with(root))
}

#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn input_schema(&self) -> Value;
    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value>;
}

pub mod create;

pub mod update;

pub mod link;

pub mod delete;
pub mod mv;

pub mod event_create;
pub mod state_at;
pub mod workspace_state_at;

pub mod timeline;

pub mod reindex;

pub mod context;

pub mod audit_doc_refs;
pub mod legibility_scan;

pub mod doctor;

pub mod augment;
pub mod goal_aggregation;
pub mod refresh;
pub mod refresh_stale;
pub mod render;
pub mod schema_validate;
pub mod tracker_design;

pub mod artifact;
pub mod artifact_event;
pub mod artifact_refresh;
pub mod librarian;

pub fn all_tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(artifact::Artifact),
        Arc::new(artifact_event::ArtifactEvent),
        Arc::new(augment::ArtifactAugment),
        Arc::new(artifact_refresh::ArtifactRefreshTool),
        Arc::new(librarian::Librarian),
    ]
}
