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

    async fn call(&self, input: Value, ctx: &crate::tools::ToolContext) -> Result<Value> {
        let active_root: Option<std::path::PathBuf> = {
            let inner = ctx.agent.inner.read().await;
            inner.active_project().map(|p| p.root.clone())
        };
        let lib_ctx = self.derive_ctx(active_root.as_deref());
        self.inner.call(&lib_ctx, input).await
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

impl LibrarianAdapter {
    /// Build a fresh `LibToolContext` for a single tool call, using the
    /// host's currently-active project to derive `current_project`. The
    /// catalog/workspace/rules/embedding stay shared with the boot-time ctx.
    fn derive_ctx(&self, active: Option<&std::path::Path>) -> Arc<LibToolContext> {
        let current_project = active.and_then(|p| match std::fs::canonicalize(p) {
            Ok(abs_path) => {
                let git_root = librarian_mcp::current_project::lookup_git_root(&abs_path)
                    .unwrap_or_else(|| abs_path.clone());
                let umbrella =
                    librarian_mcp::current_project::lookup_umbrella(&abs_path, &self.ctx.workspace);
                Some(Arc::new(librarian_mcp::current_project::CurrentProject {
                    abs_path,
                    git_root,
                    umbrella,
                }))
            }
            Err(err) => {
                tracing::warn!("active project path unresolvable: {} ({err})", p.display());
                None
            }
        });

        Arc::new(LibToolContext {
            catalog: Arc::clone(&self.ctx.catalog),
            workspace: Arc::clone(&self.ctx.workspace),
            rules: Arc::clone(&self.ctx.rules),
            embedding: self.ctx.embedding.clone(),
            current_project,
        })
    }
}
