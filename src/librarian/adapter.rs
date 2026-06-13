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

use crate::librarian::tools::{all_tools as lib_all_tools, ToolContext as LibToolContext};

pub async fn try_build_runtime() -> Option<Arc<LibToolContext>> {
    match crate::librarian::build_tool_context().await {
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
    inner: Arc<dyn crate::librarian::tools::Tool>,
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

    fn is_write(&self, input: &Value) -> bool {
        let action = input.get("action").and_then(Value::as_str);
        match self.inner.name() {
            // CRUD tool — mutating actions only; find/get/graph/state_at are reads.
            "artifact" => matches!(
                action,
                Some("create" | "update" | "move" | "delete" | "link")
            ),
            // Append-only event log: `create` writes, `list` reads.
            "artifact_event" => action == Some("create"),
            // Always attaches/replaces/merges an augmentation row.
            "artifact_augment" => true,
            // gather / list_stale are both read-only — the write-back is
            // artifact(update, commit_refresh=true), classified under "artifact".
            "artifact_refresh" => false,
            // reindex rewrites the catalog; audit_doc_refs emits a tracker unless
            // emit_tracker=false; legibility_scan reconciles the backlog unless
            // write=false; context/tracker_design/workspace_state_at/doctor read.
            "librarian" => match action {
                Some("reindex") => true,
                Some("audit_doc_refs") => {
                    input.get("emit_tracker").and_then(Value::as_bool) != Some(false)
                }
                Some("legibility_scan") => {
                    input.get("write").and_then(Value::as_bool) != Some(false)
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn relevant_guide_topic(&self) -> Option<&str> {
        Some("librarian")
    }
}

impl LibrarianAdapter {
    /// Build a fresh `LibToolContext` for a single tool call, using the
    /// host's currently-active project to derive `current_project`. The
    /// catalog/workspace/rules/embedding stay shared with the boot-time ctx.
    fn derive_ctx(&self, active: Option<&std::path::Path>) -> Arc<LibToolContext> {
        let current_project = active.and_then(|p| match std::fs::canonicalize(p) {
            Ok(abs_path) => {
                let git_root = crate::librarian::current_project::lookup_git_root(&abs_path)
                    .unwrap_or_else(|| abs_path.clone());
                let umbrella = crate::librarian::current_project::lookup_umbrella(
                    &abs_path,
                    &self.ctx.workspace,
                );
                Some(Arc::new(
                    crate::librarian::current_project::CurrentProject {
                        abs_path,
                        git_root,
                        umbrella,
                    },
                ))
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
