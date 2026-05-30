//! `approve_write` tool.

use serde_json::{json, Value};

use super::{RecoverableError, Tool, ToolContext};

pub struct ApproveWrite;

#[async_trait::async_trait]
impl Tool for ApproveWrite {
    fn name(&self) -> &str {
        "approve_write"
    }

    fn description(&self) -> &str {
        "Grant write access to a directory outside the project root for this session. \
         Session-scoped — cleared on server restart. Call before edit_file, create_file, \
         edit_code, or edit_markdown on paths outside the project. Protected paths (e.g. ~/.ssh) \
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
        let raw = super::require_str_param(&input, "path")?;

        let root = ctx.agent.require_project_root_for(ctx.workspace_override.as_deref()).await.map_err(|_| {
            RecoverableError::new("approve_write: no active project — activate a project first")
        })?;

        let security = ctx.agent.security_config_for(ctx.workspace_override.as_deref()).await;

        if !security.file_write_enabled {
            return Err(RecoverableError::new(
                "approve_write: file writes are disabled for this project",
            )
            .into());
        }

        let resolved = crate::util::path_security::validate_approve_path(raw, &root, &security)
            .map_err(|e| RecoverableError::new(e.to_string()))?;

        ctx.agent.add_session_write_root_for(ctx.workspace_override.as_deref(), resolved.clone()).await;

        Ok(json!({
            "approved": resolved.to_string_lossy(),
            "scope": "this session only"
        }))
    }
}
