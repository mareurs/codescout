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
         gather: collect context for an augmented artifact (no write — synthesize then \
         artifact(update, commit_refresh=true)). \
         list_stale: augmented artifacts stale past threshold_hours (default 24h), oldest-first."
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
        // Best-effort: identity enrichment must never fail a tool call; a failed
        // stamp degrades the row to verb=NULL, which audit_log surfaces honestly.
        if let Err(e) = ctx
            .catalog
            .lock()
            .set_audit_verb(&format!("artifact_refresh.{action}"))
        {
            tracing::warn!("audit verb stamp failed: {e}");
        }
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
    use crate::librarian::tools::TestToolContextBuilder;

    fn mk_ctx() -> ToolContext {
        TestToolContextBuilder::new(Catalog::open_in_memory().unwrap()).build()
    }

    /// Site 4 of 4 for the `IC-15` param probe — see
    /// `crate::tools::param_probe`.
    ///
    /// `gather` needs an id; a well-formed but nonexistent one keeps the failure after
    /// deserialisation. `list_stale` needs nothing, so its baseline succeeds — which is fine
    /// and is why the probe compares outcomes rather than asserting an error: an honoured key
    /// still diverges, because the ill-typed value fails the type check on the way in.
    #[tokio::test]
    async fn every_action_labelled_schema_key_is_honored_by_that_action() {
        use crate::tools::param_probe::{assert_all_honored, assert_required_are_advertised, Spec};

        fn required(action: &str) -> serde_json::Map<String, Value> {
            let mut m = serde_json::Map::new();
            if action == "gather" {
                m.insert("id".into(), json!("0000000000000000"));
            }
            m
        }

        let spec = Spec {
            actions: &["gather", "list_stale"],
            accepts_any_json: &[],
            required,
        };

        assert_all_honored(
            "artifact_refresh",
            &ArtifactRefreshTool.input_schema(),
            &spec,
            3,
            |args| async move { ArtifactRefreshTool.call(&mk_ctx(), args).await },
        )
        .await;

        // Reverse direction, site 4 of 4 — see `param_probe::assert_required_are_advertised`.
        // Reuses the same `required` table rather than restating it: the point of the check is
        // that the two representations agree, so a second copy would defeat it.
        assert_required_are_advertised(
            "artifact_refresh",
            &ArtifactRefreshTool.input_schema(),
            &spec,
        );
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

    #[tokio::test]
    async fn dispatch_stamps_the_audit_verb() {
        let ctx = mk_ctx();
        // list_stale is read-only; the stamp happens at dispatch regardless of verb kind
        let _ = ArtifactRefreshTool
            .call(
                &ctx,
                serde_json::json!({"action": "list_stale", "scope": "all"}),
            )
            .await;
        let verb: Option<String> = ctx
            .catalog
            .lock()
            .conn
            .query_row("SELECT verb FROM audit_ctx", [], |r| r.get(0))
            .unwrap();
        assert_eq!(verb.as_deref(), Some("artifact_refresh.list_stale"));
    }
}
