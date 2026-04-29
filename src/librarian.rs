//! Librarian (markdown artifact registry) integration.
//!
//! Codescout embeds the librarian crate and exposes its tools through the
//! same MCP server, so the agent sees one server with both code-symbol
//! tools and artifact tools. The adapter bridges librarian's sync `Tool`
//! trait (blocking rusqlite + parking_lot) to codescout's async trait
//! via `spawn_blocking`.
//!
//! Builder is fallible and best-effort: when no workspace.toml is
//! discoverable from cwd the librarian tools are simply absent — codescout
//! continues to serve its own tools.

use std::sync::Arc;

use anyhow::Result;
use serde_json::Value;

use librarian_mcp::tools::{all_tools as lib_all_tools, ToolContext as LibToolContext};

pub async fn try_build_runtime() -> Option<Arc<LibToolContext>> {
    match librarian_mcp::build_tool_context().await {
        Ok(ctx) => Some(Arc::new(ctx)),
        Err(err) => {
            tracing::info!("librarian disabled: {err:#}");
            None
        }
    }
}

pub fn adapters_for(ctx: Arc<LibToolContext>) -> Vec<Arc<dyn crate::tools::Tool>> {
    lib_all_tools()
        .into_iter()
        .map(|t| {
            let adapter: Arc<dyn crate::tools::Tool> = Arc::new(LibrarianAdapter {
                inner: t,
                ctx: Arc::clone(&ctx),
            });
            adapter
        })
        .collect()
}

struct LibrarianAdapter {
    inner: Arc<dyn librarian_mcp::tools::Tool>,
    ctx: Arc<LibToolContext>,
}

#[async_trait::async_trait]
impl crate::tools::Tool for LibrarianAdapter {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn input_schema(&self) -> Value {
        self.inner.input_schema()
    }

    async fn call(&self, input: Value, _ctx: &crate::tools::ToolContext) -> Result<Value> {
        self.inner.call(&self.ctx, input).await
    }

    fn is_write(&self, _input: &Value) -> bool {
        matches!(
            self.inner.name(),
            "artifact_create"
                | "artifact_update"
                | "artifact_link"
                | "artifact_observe"
                | "artifact_event_create"
                | "librarian_reindex"
        )
    }
}
