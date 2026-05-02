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
        "Grant session-scoped write access to a directory outside the project root. \
         Approval clears on restart or project re-activation. Use before edit_file, \
         create_file, edit_code, or edit_markdown on paths outside the project root. \
         Protected paths (e.g. ~/.ssh) cannot be approved."
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
        let raw = super::require_str_param(&input, "path")?;

        let root = ctx.agent.require_project_root().await.map_err(|_| {
            RecoverableError::new("approve_write: no active project — activate a project first")
        })?;

        let security = ctx.agent.security_config().await;

        if !security.file_write_enabled {
            return Err(RecoverableError::new(
                "approve_write: file writes are disabled for this project",
            )
            .into());
        }

        let resolved = crate::util::path_security::validate_approve_path(raw, &root, &security)
            .map_err(|e| RecoverableError::new(e.to_string()))?;

        ctx.agent.add_session_write_root(resolved.clone()).await;

        Ok(json!({
            "approved": resolved.to_string_lossy(),
            "scope": "this session only"
        }))
    }
}
