//! `create_file` tool.

use anyhow::Result;
use serde_json::{json, Value};

use super::{Tool, ToolContext};

pub struct CreateFile;

/// The refusal for a `create_file` whose path is occupied, with the handle that replays the same
/// write over the file the caller was told about. The pre-check and the publish (a file that
/// appeared in between) both return it, so the two refusals cannot differ.
///
/// The text keeps the prefix `file already exists:`: `src/usage/db.rs` classifies on it.
fn existing_path_refusal(
    ctx: &ToolContext,
    input: &Value,
    resolved: &std::path::Path,
) -> anyhow::Error {
    let mut hint = String::from(
        "Use edit_file to modify, or pass overwrite: true to replace. \
             create_file is for new files only.",
    );
    // The refusal discards nothing now: the input is kept under a handle, so
    // replacing the file does not mean sending `content` a second time. The handle
    // remembers the file's size and modification time and refuses a replay over a
    // file that changed since. A path with nothing readable behind it (a dangling symlink)
    // gets no handle: `OverwriteGuard::of` reads through the link.
    // docs/issues/archive/2026-11-08-create-file-existing-path-refusal-makes-the-agent-resend-the-whole-write.md
    if let Some(guard) = crate::tools::output_buffer::OverwriteGuard::of(resolved) {
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
    super::RecoverableError::with_hint(format!("file already exists: {}", resolved.display()), hint)
        .into()
}

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
            return Err(existing_path_refusal(ctx, &input, &resolved));
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
        if overwrite {
            crate::util::fs::write_utf8(&resolved, content)?;
        } else if let Err(e) = crate::util::fs::write_new_utf8(&resolved, content) {
            // The path was free at the check above. `write_new_utf8` returns `AlreadyExists` only
            // when the path was occupied (a creator that raced this call, or a dangling symlink the
            // check above cannot see), and the publish did not replace it. The `symlink_metadata`
            // re-check therefore only catches an occupant that vanished again; in that case the
            // write's own error is returned.
            if e.kind() == std::io::ErrorKind::AlreadyExists
                && std::fs::symlink_metadata(&resolved).is_ok()
            {
                return Err(existing_path_refusal(ctx, &input, &resolved));
            }
            return Err(anyhow::anyhow!(
                "Failed to write {}: {}",
                resolved.display(),
                e
            ));
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;
    use crate::lsp::LspManager;
    use crate::tools::RecoverableError;

    async fn project_ctx() -> (tempfile::TempDir, ToolContext) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        (
            dir,
            ToolContext {
                agent,
                lsp: LspManager::new_arc(),
                output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(
                    20,
                )),
                progress: None,
                peer: None,
                section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                    crate::tools::section_coverage::SectionCoverage::new(),
                )),
                guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(
                    Default::default(),
                )),
                workspace_override: None,
            },
        )
    }

    fn names(dir: &std::path::Path) -> Vec<String> {
        let mut out: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        out.sort();
        out
    }
    /// Staging files (`<name>.<nonce>.tmp`) left in `dir`. The project root also gets files of
    /// its own (`.gitignore`), so a listing of it is not a stable assertion.
    fn staging_leftovers(dir: &std::path::Path) -> Vec<String> {
        names(dir)
            .into_iter()
            .filter(|n| n.ends_with(".tmp"))
            .collect()
    }

    /// The handle a refusal names. The server replays `create_file(ack=<handle>)`; the tests
    /// below replay it with `replay` (the same steps as `CodeScoutServer::replay_ack`).
    fn handle_in(text: &str) -> String {
        let at = text
            .find("@ack_")
            .unwrap_or_else(|| panic!("the refusal names no @ack_ handle: {text}"));
        text[at..at + "@ack_".len() + 8].to_string()
    }

    /// What `CodeScoutServer::replay_ack` does for an overwrite handle: refuse when the file
    /// changed since the refusal, otherwise call again with the kept input plus `overwrite=true`.
    async fn replay(ctx: &ToolContext, handle: &str) -> Result<Value> {
        let kept = ctx
            .output_buffer
            .get_pending_write(handle)
            .expect("the handle must be kept");
        let guard = kept
            .overwrite_guard
            .expect("an existing-path refusal keeps an overwrite guard");
        assert!(guard.still_holds(), "the file changed since the refusal");
        let mut input = kept.input;
        input["overwrite"] = Value::Bool(true);
        CreateFile.call(input, ctx).await
    }

    /// Run `create_file(path, content)` with `planted` written to the path in the window between
    /// the call's own existence check and the publish. Returns the error text of the refusal.
    async fn create_losing_a_race(
        ctx: &ToolContext,
        file: &std::path::Path,
        planted: &'static str,
    ) -> String {
        // The path is free when `call` checks it, so only the publish can notice the file.
        assert!(!file.exists(), "precondition: the path is free");
        let _planted = crate::util::fs::set_before_publish(move |p| {
            std::fs::write(p, planted).unwrap();
        });
        let err = CreateFile
            .call(
                json!({ "path": file.to_str().unwrap(), "content": "MINE" }),
                ctx,
            )
            .await
            .expect_err("a create must not replace a file that appeared after the check");
        assert!(
            err.downcast_ref::<RecoverableError>().is_some(),
            "the refusal is a RecoverableError, as the pre-check's is: {err:#}"
        );
        err.to_string()
    }

    #[tokio::test]
    async fn a_file_planted_after_the_check_is_refused_and_left_unchanged() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("raced.txt");

        let text = create_losing_a_race(&ctx, &file, "PLANTED").await;

        assert!(
            text.starts_with("file already exists:"),
            "usage/db.rs classifies on this prefix: {text}"
        );
        assert!(
            text.contains("@ack_"),
            "the refusal keeps a replay handle: {text}"
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "PLANTED");
        assert_eq!(staging_leftovers(dir.path()), Vec::<String>::new());
    }

    #[tokio::test]
    async fn the_handle_of_a_raced_refusal_replaces_the_planted_file() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("raced.txt");
        let text = create_losing_a_race(&ctx, &file, "PLANTED").await;

        // The seam is cleared by now, so the replay publishes nothing behind its back.
        replay(&ctx, &handle_in(&text)).await.unwrap();

        assert_eq!(std::fs::read_to_string(&file).unwrap(), "MINE");
    }

    #[tokio::test]
    async fn a_normal_create_still_writes_the_file_and_leaves_nothing_else() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("sub").join("new.txt");

        let out = CreateFile
            .call(
                json!({ "path": file.to_str().unwrap(), "content": "FRESH" }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(out, json!("ok"));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "FRESH");
        assert_eq!(names(&dir.path().join("sub")), vec!["new.txt".to_string()]);
    }

    #[tokio::test]
    async fn overwrite_true_still_replaces_an_existing_file() {
        let (dir, ctx) = project_ctx().await;
        let file = dir.path().join("there.txt");
        std::fs::write(&file, "OLD").unwrap();

        CreateFile
            .call(
                json!({ "path": file.to_str().unwrap(), "content": "NEW", "overwrite": true }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(std::fs::read_to_string(&file).unwrap(), "NEW");
        assert_eq!(staging_leftovers(dir.path()), Vec::<String>::new());
    }

    /// A dangling symlink is not "existing" to `Path::exists`, so the old code let the write
    /// replace the link. The publish sees the link and refuses; the pre-check cannot offer a
    /// replay handle for it (`OverwriteGuard::of` reads through the link and finds nothing).
    /// The classifier admits an in-project dangling leaf link (see
    /// `create_file_overwrite_on_a_leaf_symlink_replaces_the_link_and_spares_the_victim`), so
    /// the refusal below is the publish's, not an earlier guard's.
    #[cfg(all(test, unix))]
    #[tokio::test]
    async fn a_dangling_symlink_at_the_path_is_refused_as_occupied_without_a_replay_handle() {
        let (dir, ctx) = project_ctx().await;
        let never = dir.path().join("never-created.txt");
        let link = dir.path().join("dangling.txt");
        std::os::unix::fs::symlink(&never, &link).unwrap();
        assert!(!link.exists(), "precondition: the link dangles");

        let err = CreateFile
            .call(
                json!({ "path": link.to_str().unwrap(), "content": "MINE" }),
                &ctx,
            )
            .await
            .expect_err("a create must not replace a link");

        let text = err.to_string();
        assert!(text.starts_with("file already exists:"), "{text}");
        assert!(
            !text.contains("@ack_"),
            "no handle for a dangling link: {text}"
        );
        assert_eq!(
            std::fs::read_link(&link).unwrap(),
            never,
            "the link is untouched"
        );
        assert!(!never.exists(), "nothing was created through the link");
        assert_eq!(staging_leftovers(dir.path()), Vec::<String>::new());
    }

    #[tokio::test]
    async fn a_file_where_a_parent_directory_should_be_gives_the_writes_own_error() {
        let (dir, ctx) = project_ctx().await;
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, "I am a file").unwrap();
        let target = blocker.join("x.txt");

        let err = CreateFile
            .call(
                json!({ "path": target.to_str().unwrap(), "content": "MINE" }),
                &ctx,
            )
            .await
            .expect_err("no directory can be made under a file");

        assert!(
            err.downcast_ref::<RecoverableError>().is_none(),
            "not the existing-path refusal: {err:#}"
        );
        assert!(
            !err.to_string().contains("file already exists"),
            "the path is not occupied, an ancestor is in the way: {err:#}"
        );
        assert_eq!(std::fs::read_to_string(&blocker).unwrap(), "I am a file");
    }
}
