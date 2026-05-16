use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use super::{RecoverableError, Tool, ToolContext};

pub struct ArtifactRefreshTool;

#[async_trait]
impl Tool for ArtifactRefreshTool {
    fn name(&self) -> &'static str {
        "artifact_refresh"
    }

    fn description(&self) -> &'static str {
        "Augmentation lifecycle. action: gather | list_stale. \
         gather: collect context for an augmented artifact (does NOT write — synthesize then call \
         artifact(update, commit_refresh=true) to write back). \
         list_stale: list augmented artifacts whose last refresh is older than threshold_hours \
         (default 24h), oldest-first."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["action"],
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["gather", "list_stale"],
                    "description": "gather: collect context for one artifact. list_stale: list stale augmented artifacts."
                },
                "id": { "type": "string", "description": "gather: artifact id" },
                "threshold_hours": {
                    "type": "integer",
                    "default": 24,
                    "description": "list_stale: hours since last refresh to consider stale (default 24)"
                },
                "scope": {
                    "type": "string",
                    "enum": ["project", "repo", "umbrella", "all"],
                    "default": "project",
                    "description": "list_stale: scope (default project)"
                },
                "limit": {
                    "type": "integer",
                    "default": 10,
                    "maximum": 50,
                    "description": "list_stale: max results (default 10, max 50)"
                }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let action = args["action"]
            .as_str()
            .ok_or_else(|| RecoverableError::new("action required — one of: gather, list_stale"))?;
        match action {
            "gather" => super::refresh::call(ctx, args).await,
            "list_stale" => super::refresh_stale::call(ctx, args).await,
            other => Err(RecoverableError::new(format!(
                "unknown action '{other}' — expected one of: gather, list_stale"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::Catalog;
    use crate::librarian::workspace::WorkspaceConfig;
    use std::sync::Arc;

    fn mk_ctx() -> ToolContext {
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap())),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: None,
        }
    }

    #[tokio::test]
    async fn unknown_action_returns_recoverable_error() {
        let err = ArtifactRefreshTool
            .call(&mk_ctx(), serde_json::json!({"action": "bogus"}))
            .await
            .unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }

    #[tokio::test]
    async fn list_stale_action_routes_correctly() {
        let v = ArtifactRefreshTool
            .call(
                &mk_ctx(),
                serde_json::json!({"action": "list_stale", "scope": "all"}),
            )
            .await
            .unwrap();
        assert!(v.is_array() || v["items"].is_array());
    }
}
