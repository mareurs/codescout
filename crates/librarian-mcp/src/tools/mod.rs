use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;

use crate::catalog::Catalog;
use crate::classify::CompiledRule;
use crate::workspace::WorkspaceConfig;

pub mod find;
pub mod gather;
pub mod get;
pub mod graph;
pub mod links;
pub mod list_by_kind;
pub mod scope;

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
    pub embedding: Option<Arc<crate::embedding::EmbeddingService>>,
    /// Resolved at server startup from the process cwd. `None` when the cwd
    /// lies outside every configured workspace root; tools then fall back to
    /// workspace-wide scope and surface a hint in their response.
    pub current_project: Option<Arc<crate::current_project::CurrentProject>>,
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

pub mod observe;

pub mod event_create;
pub mod state_at;
pub mod workspace_state_at;

pub mod timeline;

pub mod reindex;

pub mod context;

pub mod augment;
pub mod refresh;
pub mod refresh_commit;
pub mod render;
pub mod schema_validate;
pub mod tracker_create;
pub mod update_params;

pub fn all_tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(find::ArtifactFind),
        Arc::new(get::ArtifactGet),
        Arc::new(list_by_kind::ArtifactListByKind),
        Arc::new(links::ArtifactLinks),
        Arc::new(graph::ArtifactGraph),
        Arc::new(create::ArtifactCreate),
        Arc::new(update::ArtifactUpdate),
        Arc::new(link::ArtifactLink),
        Arc::new(observe::ArtifactObserve),
        Arc::new(event_create::ArtifactEventCreate),
        Arc::new(timeline::ArtifactTimeline),
        Arc::new(state_at::ArtifactStateAt),
        Arc::new(workspace_state_at::WorkspaceStateAt),
        Arc::new(reindex::LibrarianReindex),
        Arc::new(context::LibrarianContext),
        Arc::new(augment::ArtifactAugment),
        Arc::new(update_params::ArtifactUpdateParams),
        Arc::new(refresh::ArtifactRefresh),
        Arc::new(refresh_commit::ArtifactRefreshCommit),
        Arc::new(tracker_create::TrackerCreate),
    ]
}
