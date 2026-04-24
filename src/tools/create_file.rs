//! `create_file` tool.

use anyhow::Result;
use serde_json::{json, Value};

use super::{Tool, ToolContext};

pub struct CreateFile;

#[async_trait::async_trait]
impl Tool for CreateFile {
    fn name(&self) -> &str {
        "create_file"
    }

    fn is_write(&self, _input: &Value) -> bool {
        true
    }

    fn description(&self) -> &str {
        "Create or overwrite a file with the given content. Creates parent directories as needed."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path", "content"],
            "properties": {
                "path": { "type": "string", "description": "File path (relative or absolute)" },
                "content": { "type": "string", "description": "Content to write" }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        super::guard_worktree_write(ctx).await?;
        let path = super::require_str_param(&input, "path")?;
        let content = super::require_str_param(&input, "content")?;
        let root = ctx.agent.require_project_root().await?;
        let security = ctx.agent.security_config().await;
        let resolved = crate::util::path_security::validate_write_path(path, &root, &security)?;
        crate::util::fs::write_utf8(&resolved, content)?;
        ctx.lsp.notify_file_changed(&resolved).await;
        ctx.agent.mark_file_dirty(resolved).await;
        Ok(json!("ok"))
    }
}
