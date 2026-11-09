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

    /// `destructive` stays at its `true` default: `overwrite: true` replaces an existing
    /// file.
    fn annotations(&self) -> Option<rmcp::model::ToolAnnotations> {
        crate::tools::annot::writer_closed()
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
            "required": ["content"],
            // NO top-level anyOf/oneOf/allOf — the Anthropic Messages API rejects them
            // and Claude Code drops the whole tool client-side. The path alternation is
            // enforced in call() below, not here. See
            // no_tool_schema_declares_a_top_level_combinator (src/server.rs).
            "properties": {
                "path": { "type": "string", "description": "File path (relative or absolute)" },
                "content": { "type": "string", "description": "Content to write" },
                "overwrite": {
                    "type": "boolean",
                    "default": false,
                    "description": "Allow replacing an existing file. Default false; large reductions also require force=true."
                },
                "force": {
                    "type": "boolean",
                    "default": false,
                    "description": "Allow a >50% byte or line reduction when the existing file is at least 200 bytes. Requires overwrite=true."
                }
            }
        })
    }

    fn param_aliases(&self) -> crate::tools::param_alias::AliasMap {
        crate::fs::PATH_PARAM_ALIAS_MAP
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        super::guard_worktree_write_for_target(ctx, &input, super::WorktreeWriteTarget::File)
            .await?;
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
            let mut hint = String::from(
                "Use edit_file to modify, or pass overwrite: true to replace. \
                     create_file is for new files only.",
            );
            // The refusal discards nothing now: the input is kept under a handle, so
            // replacing the file does not mean sending `content` a second time. The handle
            // remembers the file's size and modification time and refuses a replay over a
            // file that changed since.
            // docs/issues/archive/2026-11-08-create-file-existing-path-refusal-makes-the-agent-resend-the-whole-write.md
            if let Some(guard) = crate::tools::output_buffer::OverwriteGuard::of(&resolved) {
                let (len, age) = (guard.len, guard.age_secs());
                let handle = ctx.output_buffer.store_overwrite_replayable_write(
                    "create_file".to_string(),
                    input.clone(),
                    guard,
                );
                let age = age
                    .map(|s| format!(", last modified {s}s ago"))
                    .unwrap_or_default();
                hint.push_str(&format!(
                    " To replace it with the content you just sent, call \
                         create_file(ack=\"{handle}\") and send nothing else. The existing file \
                         is {len} bytes{age}; the replay is refused if it changes before then."
                ));
            }
            return Err(super::RecoverableError::with_hint(
                format!("file already exists: {}", resolved.display()),
                hint,
            )
            .into());
        }
        if overwrite && resolved.exists() && !super::parse_bool_param(&input["force"]) {
            let original = std::fs::read_to_string(&resolved)?;
            if let Some(report) = crate::util::shrink_guard::check(&original, content) {
                let message = format!("shrink guard: overwriting '{}' {}", path, report.describe());
                let mut hint = String::from(
                    "Read the existing file before replacing it. If the reduction is intentional, \
                 repeat create_file(path=..., content=..., overwrite=true, force=true).",
                );
                // Like the existing-path refusal, this one keeps the input under a handle, so
                // agreeing to the reduction does not mean sending `content` a second time. The
                // replay adds `force=true` and is refused if the file changed since.
                // docs/issues/archive/2026-11-08-create-file-shrink-guard-refusal-makes-the-agent-resend-the-whole-write.md
                if let Some(mut guard) = crate::tools::output_buffer::OverwriteGuard::of(&resolved)
                {
                    guard.force = true;
                    let handle = ctx.output_buffer.store_overwrite_replayable_write(
                        "create_file".to_string(),
                        input.clone(),
                        guard,
                    );
                    hint.push_str(&format!(
                        " Or, to apply the content you just sent as it is, call \
                         create_file(ack=\"{handle}\") and send nothing else; the replay is \
                         refused if the file changes before then."
                    ));
                }
                return Err(super::RecoverableError::with_hint(message, hint).into());
            }
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
