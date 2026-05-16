use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use super::{RecoverableError, Tool, ToolContext};

pub struct Librarian;

#[async_trait]
impl Tool for Librarian {
    fn name(&self) -> &'static str {
        "librarian"
    }

fn description(&self) -> &'static str {
        "Workspace-level librarian operations. \
         action: context | reindex | tracker_design | workspace_state_at | audit_doc_refs. \
         context: pack topic/anchor neighbourhood into a markdown bundle. \
         reindex: re-scan and classify markdown artifacts. \
         tracker_design: return teaching prompt + archetype library (call BEFORE artifact(create) for trackers). \
         workspace_state_at: time-travel snapshot of all artifacts at a commit/timestamp. \
         audit_doc_refs: scan markdown for stale code refs (file paths, symbols, \
         line refs, link targets, module paths). Surfaces broken references \
         against current filesystem + LSP symbol index. Manual cadence — run \
         when a doc-heavy PR is about to merge or when drift is suspected. \
         Output is an `audit_issues` tracker."
    }

fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["action"],
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["context", "reindex", "tracker_design", "workspace_state_at", "audit_doc_refs"],
                    "description": "Operation to perform"
                },
                "topic": { "type": "string", "description": "context: subject for semantic/LIKE search across titles and topics" },
                "anchor_id": { "type": "string", "description": "context: artifact id to anchor the bundle (uses link graph)" },
                "max_tokens": { "type": "integer", "default": 4000, "description": "context: approximate token budget" },
                "include_archived": { "type": "boolean", "default": false },
                "scope": {
                    "type": "string",
                    "enum": ["project", "repo", "umbrella", "all"],
                    "default": "project",
                    "description": "context/reindex/workspace_state_at/audit_doc_refs: scope. Defaults to active project."
                },
                "repo": { "type": "string", "description": "reindex: restrict to a specific workspace root" },
                "force": { "type": "boolean", "description": "reindex: wipe rows for targeted scope before re-walking" },
                "intent": { "type": "string", "description": "tracker_design: free-form intent (optional)" },
                "commit": { "type": "string", "description": "workspace_state_at: git commit hash as time-travel cutoff. Exactly one of commit or timestamp required." },
                "timestamp": { "type": "integer", "format": "int64", "description": "workspace_state_at: unix epoch ms as cutoff. Exactly one of commit or timestamp required." },
                "kinds": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "workspace_state_at: filter by artifact kinds"
                },
                "freshness_filter": {
                    "type": "array",
                    "items": { "type": "string", "enum": ["fresh", "stale", "unknown", "superseded"] },
                    "description": "workspace_state_at: only return artifacts matching these freshness values"
                },
                "paths": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "audit_doc_refs: glob patterns to restrict scan (default: docs/**/*.md, CLAUDE.md, **/README.md)"
                },
                "emit_tracker": { "type": "boolean", "default": true, "description": "audit_doc_refs: create/update an audit_issues tracker artifact with results" },
                "tracker_id": { "type": "string", "description": "audit_doc_refs: existing tracker id to update (creates new if omitted)" },
                "severity_overrides": { "type": "object", "description": "audit_doc_refs: map of ref_kind -> severity override" },
                "fail_on": { "type": "string", "default": "never", "description": "audit_doc_refs: exit_code 1 when findings reach this severity (high | med | low | never)" }
            }
        })
    }

async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let action = args["action"].as_str().ok_or_else(|| {
            RecoverableError::new(
                "action required — one of: context, reindex, tracker_design, workspace_state_at, audit_doc_refs",
            )
        })?;
        match action {
            "context"            => super::context::call(ctx, args).await,
            "reindex"            => super::reindex::call(ctx, args).await,
            "tracker_design"     => super::tracker_design::call(ctx, args).await,
            "workspace_state_at" => super::workspace_state_at::call(ctx, args).await,
            "audit_doc_refs"     => super::audit_doc_refs::call(ctx, args).await,
            other => Err(RecoverableError::new(format!(
                "unknown action '{other}' — expected one of: context, reindex, tracker_design, workspace_state_at, audit_doc_refs"
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
        let err = Librarian
            .call(&mk_ctx(), serde_json::json!({"action": "bogus"}))
            .await
            .unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }

    #[tokio::test]
    async fn tracker_design_routes_correctly() {
        let v = Librarian
            .call(&mk_ctx(), serde_json::json!({"action": "tracker_design"}))
            .await
            .unwrap();
        assert!(v["archetypes"].is_array());
    }

    #[tokio::test]
    async fn audit_doc_refs_action_routes() {
        let ctx = mk_ctx();
        let result = crate::librarian::tools::audit_doc_refs::call(&ctx, serde_json::json!({}))
            .await
            .unwrap();
        assert_eq!(result["exit_code"], 0);
        assert!(result["findings"].is_array());
    }
}
