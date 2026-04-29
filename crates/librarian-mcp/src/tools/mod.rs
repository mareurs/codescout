use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;

use crate::catalog::Catalog;
use crate::classify::CompiledRule;
use crate::workspace::WorkspaceConfig;

pub mod find;
pub mod get;
pub mod graph;
pub mod links;
pub mod list_by_kind;
pub mod scope;

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
    ]
}
