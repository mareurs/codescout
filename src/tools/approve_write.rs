//! `approve_write` tool.

use serde_json::{json, Value};

use super::{RecoverableError, Tool, ToolContext};

pub struct ApproveWrite;

#[async_trait::async_trait]
impl Tool for ApproveWrite {
    fn name(&self) -> &str {
        "approve_write"
    }

    /// Additive and in-process: grants a session write root, never revokes one, and the
    /// same path twice leaves the same state. **Not** `readOnlyHint: true` — this is the
    /// call that widens the write sandbox, so a client filtering on that hint to decide
    /// what to auto-approve must not auto-approve it.
    fn annotations(&self) -> Option<rmcp::model::ToolAnnotations> {
        crate::tools::annot::additive_closed()
    }

    fn description(&self) -> &str {
        "Grant write access to a directory outside the project root for this session. \
         Session-scoped — cleared on server restart. Call before edit_file, create_file, \
         or edit_code on paths outside the project. Protected paths (e.g. ~/.ssh) \
         cannot be approved."
    }

    fn is_write(&self, _input: &Value) -> bool {
        true
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute or project-relative path to the directory to approve for writing."
                }
            },
            "required": ["path"]
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        super::guard_worktree_write(ctx).await?;
        // `path` means three different things across its three call sites, so the
        // shared table in params.rs deliberately has no entry for it. BL-3 Class B.
        let raw = super::require_str_param_or_hint(
            &input,
            "path",
            &[],
            "Pass the directory to approve for writing, e.g. path=\"docs/notes\" — absolute \
             or project-relative. Approval is scoped to that directory; the denial you are \
             answering names the path it wanted.",
        )?;

        let root = ctx
            .agent
            .require_project_root_for(ctx.workspace_override.as_deref())
            .await
            .map_err(|_| {
                RecoverableError::new("approve_write: no active project — activate a project first")
            })?;

        let security = ctx
            .agent
            .security_config_for(ctx.workspace_override.as_deref())
            .await;

        if !security.file_write_enabled {
            return Err(RecoverableError::new(
                "approve_write: file writes are disabled for this project",
            )
            .into());
        }

        let resolved = crate::util::path_security::validate_approve_path(raw, &root, &security)
            .map_err(|e| RecoverableError::new(e.to_string()))?;

        ctx.agent
            .add_session_write_root_for(ctx.workspace_override.as_deref(), resolved.clone())
            .await;

        Ok(json!({
            "approved": resolved.to_string_lossy(),
            "scope": "this session only"
        }))
    }
}
