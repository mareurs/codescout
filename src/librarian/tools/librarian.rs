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
         action: context | reindex | tracker_design | workspace_state_at | audit_doc_refs | legibility_scan | doctor. \
         context: pack topic/anchor neighbourhood into a markdown bundle. \
         reindex: re-scan and classify markdown artifacts. \
         tracker_design: return teaching prompt + archetype library (call BEFORE artifact(create) for trackers). \
         workspace_state_at: time-travel snapshot of all artifacts at a commit/timestamp. \
         audit_doc_refs: scan markdown for stale code refs (file paths, symbols, \
         line refs, link targets, module paths). Surfaces broken references \
         against current filesystem + LSP symbol index. Manual cadence — run \
         when a doc-heavy PR is about to merge or when drift is suspected. \
         Output is an `audit_issues` tracker. \
         legibility_scan: rank code-legibility refactor candidates from usage.db \
         friction + the AST symbol index. Writes/updates the legibility-backlog \
         tracker — open targets ranked by observed cost (tier 1 biting-now, tier 2 \
         latent), auto-closing refactored ones with a before/after delta. \
         write=false for a dry-run JSON. \
         doctor: read-only catalog drift scanner. Checks abs_path columns for \
         absolute-form, forward-slash form, NTFS ADS colons, '..' segments, \
         and missing files on disk; checks commits.git_root for forward-slash \
         form. Returns a JSON report with per-check violation counts. Manual \
         cadence — run after large refactors or when downstream LIKE queries \
         return empty."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["action"],
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["context", "reindex", "tracker_design", "workspace_state_at", "audit_doc_refs", "legibility_scan", "doctor"],
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
                    "description": "audit_doc_refs: glob patterns to restrict scan (default: docs/**/*.md, CLAUDE.md, **/README.md). Default scan excludes docs/agents/** — pass an explicit list to include those files."
                },
                "emit_tracker": { "type": "boolean", "default": true, "description": "audit_doc_refs: create/update an audit_issues tracker artifact with results" },
                "tracker_id": { "type": "string", "description": "audit_doc_refs: existing tracker id to update (creates new if omitted)" },
                "fail_on": { "type": "string", "default": "never", "description": "audit_doc_refs: exit_code 1 when findings reach this severity (high | med | low | never)" },
                "write": { "type": "boolean", "default": true, "description": "legibility_scan: reconcile the backlog tracker (false = dry-run JSON only)" },
                "project": { "type": "string", "description": "legibility_scan: project root path; defaults to active project. Scopes the recorder lane." },
                "limit": { "type": "integer", "description": "legibility_scan: cap candidates returned/written" }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let action = args["action"].as_str().ok_or_else(|| {
            RecoverableError::new(
                "action required — one of: context, reindex, tracker_design, workspace_state_at, audit_doc_refs, legibility_scan, doctor",
            )
        })?;
        match action {
            "context"            => super::context::call(ctx, args).await,
            "reindex"            => super::reindex::call(ctx, args).await,
            "tracker_design"     => super::tracker_design::call(ctx, args).await,
            "workspace_state_at" => super::workspace_state_at::call(ctx, args).await,
            "audit_doc_refs"     => super::audit_doc_refs::call(ctx, args).await,
            "legibility_scan"    => super::legibility_scan::call(ctx, args).await,
            "doctor"             => super::doctor::call(ctx, args).await,
            other => Err(RecoverableError::new(format!(
                "unknown action '{other}' — expected one of: context, reindex, tracker_design, workspace_state_at, audit_doc_refs, legibility_scan, doctor"
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
        use crate::librarian::current_project::CurrentProject;
        use crate::librarian::workspace::Root;
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        // Write a minimal markdown file so the scanner has something to scan.
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::write(root.join("docs/readme.md"), "# hello\n").unwrap();
        let ctx = ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap())),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![Root {
                    name: "r".into(),
                    path: root.clone(),
                }],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: Some(Arc::new(CurrentProject {
                abs_path: root.clone(),
                git_root: root,
                umbrella: None,
            })),
        };
        let result = crate::librarian::tools::audit_doc_refs::call(&ctx, serde_json::json!({}))
            .await
            .unwrap();
        assert_eq!(result["exit_code"], 0);
        assert!(result["findings"].is_array());
    }

    #[tokio::test]
    async fn legibility_scan_action_routes() {
        let ctx = mk_ctx();
        let args = serde_json::json!({ "action": "legibility_scan", "write": false });
        // No active project in mk_ctx → RecoverableError, NOT "unknown action".
        let err = Librarian.call(&ctx, args).await.unwrap_err();
        let msg = format!("{err}");
        assert!(!msg.contains("unknown action"), "should route, got: {msg}");
    }

}
