use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use super::{RecoverableError, Tool, ToolContext};

pub struct ArtifactEvent;

#[async_trait]
impl Tool for ArtifactEvent {
    fn name(&self) -> &'static str {
        "artifact_event"
    }

    fn description(&self) -> &'static str {
        "Artifact event log. action: create | list. \
         Events are immutable append-only records anchored to git commits — \
         distinct from field patches (use artifact(update) for those)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["action"],
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "list"],
                    "description": "Operation: create appends an event; list returns events newest-first."
                },
                "artifact_id": { "type": "string", "description": "create/list: artifact id" },
                "kind": {
                    "type": "string",
                    "description": "create: event kind (note, reviewed, status_change, field_patch, superseded_by, external_signal, intent, verdict)"
                },
                "payload": { "description": "create: event payload (any JSON)" },
                "anchor_commit": { "type": "string", "description": "create: git commit to anchor event to" },
                "head_commit": { "type": "string", "description": "create: HEAD commit at write time" },
                "parent_event_id": { "type": "string", "description": "create: parent event id for threading" },
                "author": { "type": "string", "description": "create: event author" },
                "also_mutates": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "create: additional artifact ids mutated by this event"
                },
                "resolves_intent_event_id": { "type": "string", "description": "create: intent event id this verdict resolves" },
                "source": {
                    "type": "object",
                    "description": "create: external signal source {uri, kind, payload?}",
                    "properties": {
                        "uri": { "type": "string" },
                        "kind": { "type": "string" },
                        "payload": {}
                    },
                    "required": ["uri", "kind"]
                },
                "kinds": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "list: filter to these event kinds"
                },
                "limit": { "type": "integer", "default": 50, "description": "list: max results" },
                "since": { "type": "integer", "format": "int64", "description": "list: return events after this ms epoch" },
                "until": { "type": "integer", "format": "int64", "description": "list: return events before this ms epoch" }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let action = args["action"]
            .as_str()
            .ok_or_else(|| RecoverableError::new("action required — one of: create, list"))?;
        match action {
            "create" => super::event_create::call(ctx, args).await,
            "list" => super::timeline::call(ctx, args).await,
            other => Err(RecoverableError::new(format!(
                "unknown action '{other}' — expected one of: create, list"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;
    use crate::workspace::WorkspaceConfig;
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
        let err = ArtifactEvent
            .call(
                &mk_ctx(),
                serde_json::json!({"action": "bogus", "artifact_id": "x"}),
            )
            .await
            .unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }

    #[tokio::test]
    async fn list_action_routes_correctly() {
        let v = ArtifactEvent
            .call(
                &mk_ctx(),
                serde_json::json!({"action": "list", "artifact_id": "nonexistent"}),
            )
            .await
            .unwrap();
        // timeline returns array even for unknown ids
        assert!(v.is_array() || v["events"].is_array());
    }
}
