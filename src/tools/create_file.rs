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

    /// Opts `create_file` into operator-rule routing so `OP-4`
    /// (`**Serves:** create_file(path~/.claude)`) can reach `route()` at all.
    ///
    /// Covered in the same change as `edit_file` deliberately: `OP-4` names
    /// both in one rule, and opting in only one would leave it half-routable —
    /// a state harder to notice than not routable at all, because the rule
    /// would fire for some writes and silently not for others.
    ///
    /// This is the routing PRECONDITION only; `OP-4`'s `path~` predicate is
    /// matched against the response, which carries no path. See
    /// `docs/issues/archive/2026-08-28-op-4-path-predicate-can-never-fire.md`.
    fn selector_key(&self, input: &serde_json::Value) -> Option<String> {
        crate::tools::core::types::action_selector_key(self.name(), input)
    }

    fn is_write(&self, _input: &Value) -> bool {
        true
    }

    fn description(&self) -> &str {
        "Create a new file with the given content. Refuses to overwrite an existing file \
             unless `overwrite: true` is passed. Creates parent directories as needed."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path", "content"],
            "properties": {
                "path": { "type": "string", "description": "File path (relative or absolute)" },
                "file_path": { "type": "string", "description": "Alias for path" },
                "content": { "type": "string", "description": "Content to write" },
                "overwrite": {
                    "type": "boolean",
                    "default": false,
                    "description": "If true, allow replacing an existing file. Default: false (create_file refuses to overwrite)."
                }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        super::guard_worktree_write(ctx).await?;
        let input = super::maybe_replay_ack(ctx, input, "create_file").await?;
        let path = super::require_str_param_or_hint(
            &input,
            "path",
            crate::fs::PATH_PARAM_ALIASES,
            "create_file(path=\"src/new_file.rs\", content=\"...\"). 'file_path' is also accepted; there is no implicit current file.",
        )?;
        let content = super::require_str_param(&input, "content")?;
        let overwrite = super::parse_bool_param(&input["overwrite"]);
        let resolved =
            match super::resolve_write_or_capture(ctx, "create_file", &input, path).await? {
                super::WriteOutcome::Write(p) => p,
                super::WriteOutcome::Pending(env) => return Ok(env),
            };
        if !overwrite && resolved.exists() {
            return Err(super::RecoverableError::with_hint(
                format!("file already exists: {}", resolved.display()),
                "Use edit_file to modify, or pass overwrite: true to replace. \
                     create_file is for new files only.",
            )
            .into());
        }
        crate::util::fs::write_utf8(&resolved, content)?;
        ctx.lsp.notify_file_changed(&resolved).await;
        ctx.agent
            .invalidate_call_edges_for(ctx.workspace_override.as_deref(), &resolved)
            .await;
        ctx.agent
            .mark_file_dirty_for(ctx.workspace_override.as_deref(), resolved)
            .await;
        Ok(json!("ok"))
    }
}
