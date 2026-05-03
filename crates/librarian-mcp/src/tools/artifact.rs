use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use super::{RecoverableError, Tool, ToolContext};

pub struct Artifact;

#[async_trait]
impl Tool for Artifact {
    fn name(&self) -> &'static str {
        "artifact"
    }

    fn description(&self) -> &'static str {
        "Artifact CRUD and query. action: find | get | create | update | link | graph | state_at. \
         Defaults: scope=project (current sub-project only), archived/superseded hidden when \
         filter does not constrain status. Shortcut params kind/status expand to eq-filters \
         and combine with filter via AND. \
         Trackers are artifacts with kind=tracker — augmented documents that auto-refresh their \
         body via a persistent prompt; call librarian(tracker_design) before creating one."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["action"],
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["find", "get", "create", "update", "link", "graph", "state_at"],
                    "description": "Operation to perform"
                },
                "filter": {
                    "type": "object",
                    "description": "find: filter AST (and/or/not + eq/ne/in/nin/gt/lt/gte/lte/contains/prefix leaves)"
                },
                "kind": {
                    "type": "string",
                    "description": "find: shortcut eq-filter on kind. create: artifact kind (spec/plan/adr/tracker/…)"
                },
                "status": {
                    "type": "string",
                    "description": "find: shortcut eq-filter on status (disables archived-hide). create/update: set status."
                },
                "semantic": {
                    "type": "string",
                    "description": "find: natural-language query for semantic search (requires embedder)"
                },
                "scope": {
                    "type": "string",
                    "enum": ["project", "repo", "umbrella", "all"],
                    "default": "project",
                    "description": "find: scope for listing. Defaults to current sub-project."
                },
                "augmented": {
                    "type": "boolean",
                    "description": "find: filter to augmented (true) or non-augmented (false) artifacts"
                },
                "include_archived": { "type": "boolean", "default": false },
                "limit": { "type": "integer", "default": 50, "maximum": 500 },
                "offset": { "type": "integer", "default": 0, "maximum": 100000 },
                "id": {
                    "type": "string",
                    "description": "get/update/graph: artifact id"
                },
                "include_links": { "type": "boolean", "default": false, "description": "get: include link edges" },
                "links_direction": {
                    "type": "string",
                    "enum": ["out", "in", "both"],
                    "description": "get: filter links by direction (default: both)"
                },
                "links_rel": { "type": "string", "description": "get: filter links to this rel type" },
                "include_observations": { "type": "boolean", "default": false },
                "full": { "type": "boolean", "default": false, "description": "get: include full body" },
                "heading": { "type": "string", "description": "get: fetch one section by heading" },
                "headings": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "get: fetch multiple sections by heading"
                },
                "start_line": { "type": "integer", "description": "get: 1-indexed start of line slice" },
                "end_line": { "type": "integer", "description": "get: 1-indexed inclusive end of line slice" },
                "rel_path": { "type": "string", "description": "create: relative path for new file" },
                "repo": { "type": "string", "description": "create: workspace root name" },
                "title": { "type": "string", "description": "create: artifact title" },
                "body": { "type": "string", "description": "create: markdown body" },
                "owners": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "create/update: owner list"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "create/update: tag list"
                },
                "augment": {
                    "type": "object",
                    "description": "create: attach augmentation atomically. Pass prompt + optional params.",
                    "properties": {
                        "prompt": { "type": "string" },
                        "params": { "type": "object" }
                    },
                    "required": ["prompt"]
                },
                "patch": {
                    "type": "object",
                    "description": "update: fields to change (body, title, status, topic, owners, tags)"
                },
                "addBlocks": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "update: task IDs this artifact blocks"
                },
                "addBlockedBy": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "update: task IDs that block this artifact"
                },
                "owner": { "type": "string", "description": "update: set owner field" },
                "commit_refresh": {
                    "type": "boolean",
                    "description": "update: atomically record a completed refresh cycle"
                },
                "activeForm": { "type": "string", "description": "update: present-continuous label shown in spinner" },
                "src_id": { "type": "string", "description": "link: source artifact id" },
                "dst_id": { "type": "string", "description": "link: destination artifact id" },
                "rel": { "type": "string", "description": "link: relation type (supersedes, implements, …)" },
                "depth": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 3,
                    "description": "graph: BFS depth (1–3)"
                },
                "rels": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "graph: filter edges to these rel types"
                },
                "include_events": {
                    "type": "boolean",
                    "default": false,
                    "description": "graph: also walk event and source nodes via event_edges"
                },
                "artifact_id": { "type": "string", "description": "state_at: artifact id" },
                "commit": { "type": "string", "description": "state_at: git commit hash as time-travel cutoff" },
                "timestamp": {
                    "type": "integer",
                    "format": "int64",
                    "description": "state_at: unix epoch ms as time-travel cutoff"
                }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let action = args["action"].as_str().ok_or_else(|| {
            RecoverableError::new(
                "action required — one of: find, get, create, update, link, graph, state_at",
            )
        })?;
        match action {
            "find"     => super::find::call(ctx, args).await,
            "get"      => super::get::call(ctx, args).await,
            "create"   => super::create::call(ctx, args).await,
            "update"   => super::update::call(ctx, args).await,
            "link"     => super::link::call(ctx, args).await,
            "graph"    => super::graph::call(ctx, args).await,
            "state_at" => super::state_at::call(ctx, args).await,
            other => Err(RecoverableError::new(format!(
                "unknown action '{other}' — expected one of: find, get, create, update, link, graph, state_at"
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
        let err = Artifact
            .call(&mk_ctx(), serde_json::json!({"action": "bogus"}))
            .await
            .unwrap_err();
        assert!(
            err.downcast_ref::<RecoverableError>().is_some(),
            "expected RecoverableError, got: {err}"
        );
    }

    #[tokio::test]
    async fn missing_action_returns_recoverable_error() {
        let err = Artifact
            .call(&mk_ctx(), serde_json::json!({}))
            .await
            .unwrap_err();
        assert!(
            err.downcast_ref::<RecoverableError>().is_some(),
            "expected RecoverableError, got: {err}"
        );
    }

    #[tokio::test]
    async fn find_action_routes_correctly() {
        let v = Artifact
            .call(&mk_ctx(), serde_json::json!({"action": "find"}))
            .await
            .unwrap();
        assert!(v["count"].is_number());
    }
}
