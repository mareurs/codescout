//! Shared capture + replay helpers for out-of-scope writes.
//!
//! When a write tool targets a path outside the project root, `resolve_write_or_capture`
//! stashes the full tool input and returns a `@ack_*` handle instead of failing.
//! Re-invoking the tool with that handle in `path` hits `maybe_replay_ack`, which
//! approves the directory for the session and returns the original input so the
//! tool can replay the write without re-sending content. Mirrors `run_command`'s
//! dangerous-command gate.

use std::path::PathBuf;

use serde_json::{json, Value};

use super::{RecoverableError, ToolContext};
use crate::tools::output_buffer::looks_like_ack_handle;
use crate::util::path_security::{
    classify_write_path, too_broad_directory_denial, validate_approve_path, WritePathDecision,
};

/// Result of resolving a write target with capture awareness.
#[derive(Debug)]
pub enum WriteOutcome {
    /// Proceed: write to this resolved path.
    Write(PathBuf),
    /// Return this `pending_ack` envelope to the caller verbatim.
    Pending(Value),
}

/// Phase A — if `input["path"]` is an `@ack_*` write handle, approve its
/// directory for the session and return the original stored input. Otherwise
/// return `input` unchanged. MUST run before any path-shape gate in the tool.
pub async fn maybe_replay_ack(
    ctx: &ToolContext,
    input: Value,
    tool_name: &str,
) -> anyhow::Result<Value> {
    let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if !looks_like_ack_handle(path) {
        return Ok(input);
    }
    let stored = ctx.output_buffer.get_pending_write(path).ok_or_else(|| {
        RecoverableError::with_hint(
            "ack handle expired or unknown",
            "Regenerate the write to get a fresh handle.",
        )
    })?;
    if stored.tool_name != tool_name {
        return Err(RecoverableError::with_hint(
            format!(
                "ack handle was minted by '{}', not '{}'",
                stored.tool_name, tool_name
            ),
            format!("Re-invoke {}(path=\"{}\") instead.", stored.tool_name, path),
        )
        .into());
    }
    let root = ctx
        .agent
        .require_project_root_for(ctx.workspace_override.as_deref())
        .await?;
    let security = ctx
        .agent
        .security_config_for(ctx.workspace_override.as_deref())
        .await;
    // Re-validate approvability (deny-list / breadth) before granting.
    validate_approve_path(&stored.approve_dir.to_string_lossy(), &root, &security)
        .map_err(|e| RecoverableError::new(e.to_string()))?;

    ctx.agent
        .add_session_write_root_for(
            ctx.workspace_override.as_deref(),
            stored.approve_dir.clone(),
        )
        .await;
    Ok(stored.input)
}

/// Phase B — resolve `raw_path` for writing. On an outside-root rejection,
/// stash the full input and return a `pending_ack` envelope instead of failing.
pub async fn resolve_write_or_capture(
    ctx: &ToolContext,
    tool_name: &str,
    input: &Value,
    raw_path: &str,
) -> anyhow::Result<WriteOutcome> {
    let root = ctx
        .agent
        .require_project_root_for(ctx.workspace_override.as_deref())
        .await?;
    let security = ctx
        .agent
        .security_config_for(ctx.workspace_override.as_deref())
        .await;
    let session_roots = ctx
        .agent
        .session_write_roots_snapshot_for(ctx.workspace_override.as_deref())
        .await;

    match classify_write_path(raw_path, &root, &security, &session_roots) {
        WritePathDecision::Allowed(p) => {
            // The classifier resolves the parent and keeps the leaf as written, so an in-root
            // leaf link to a deny-listed file is "allowed" here while `read_file` refuses the
            // same link. The edit tools read the target through it (error hints echo it, a
            // unique match copies it into the project), so apply the read deny-list to what
            // the link resolves to. docs/issues/2026-10-17-edit-file-on-a-leaf-symlink-to-a-deny-listed-path-reads-and-copies-the-file.md
            crate::fs::validate_leaf_link_target(
                &p,
                &root,
                &security,
                &session_roots,
                crate::fs::LinkTarget::ReadableByTools,
            )
            .map_err(|e| RecoverableError::new(e.to_string()))?;
            Ok(WriteOutcome::Write(p))
        }
        WritePathDecision::Denied(msg) => Err(RecoverableError::new(msg).into()),
        WritePathDecision::OutsideRoot { resolved } => {
            let dir = resolved
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| resolved.clone());
            // `approve_write` refuses a directory that is too broad: say what is true of THIS
            // write (the same text `validate_write_path` gives) instead of the approval's own
            // refusal, which reads as advice about a different call.
            if let Some(denial) = too_broad_directory_denial(raw_path, &dir) {
                return Err(RecoverableError::new(denial).into());
            }
            // Pre-validate so a minted handle is guaranteed replayable.
            if let Err(e) = validate_approve_path(&dir.to_string_lossy(), &root, &security) {
                return Err(RecoverableError::new(e.to_string()).into());
            }
            let handle = ctx.output_buffer.store_pending_write(
                tool_name.to_string(),
                input.clone(),
                dir.clone(),
            );
            Ok(WriteOutcome::Pending(json!({
                "pending_ack": handle,
                "reason": format!("'{}' is outside the project root", raw_path),
                "hint": format!(
                    "{}(path=\"{}\") to write it and approve {} for this session",
                    tool_name,
                    handle,
                    dir.display()
                ),
            })))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;
    use serde_json::json;

    async fn ctx_with_project() -> (tempfile::TempDir, crate::tools::ToolContext) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let ctx = crate::tools::ToolContext {
            agent,
            lsp: crate::lsp::LspManager::new_arc(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
            guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
            workspace_override: None,
        };
        (dir, ctx)
    }

    #[tokio::test]
    async fn capture_outside_path_returns_pending_handle() {
        let (_dir, ctx) = ctx_with_project().await;
        let input = json!({ "path": "/var/ce_ack_test/plan.md", "content": "BIG" });
        let outcome =
            resolve_write_or_capture(&ctx, "create_file", &input, "/var/ce_ack_test/plan.md")
                .await
                .unwrap();
        match outcome {
            WriteOutcome::Pending(env) => {
                let handle = env["pending_ack"].as_str().expect("pending_ack");
                assert!(handle.starts_with("@ack_"), "got: {env}");
                assert!(env["reason"]
                    .as_str()
                    .unwrap()
                    .contains("outside the project root"));
                // Content preserved verbatim in the buffer.
                let stored = ctx.output_buffer.get_pending_write(handle).unwrap();
                assert_eq!(stored.input, input);
                assert_eq!(stored.tool_name, "create_file");
            }
            other => panic!("expected Pending, got Write: {other:?}"),
        }
    }

    #[tokio::test]
    async fn in_project_path_resolves_to_write() {
        let (dir, ctx) = ctx_with_project().await;
        let p = dir.path().join("plan.md");
        let input = json!({ "path": p.to_str().unwrap(), "content": "x" });
        let outcome = resolve_write_or_capture(&ctx, "create_file", &input, p.to_str().unwrap())
            .await
            .unwrap();
        assert!(
            matches!(outcome, WriteOutcome::Write(_)),
            "got: {outcome:?}"
        );
    }

    #[tokio::test]
    async fn replay_approves_dir_and_returns_stored_input() {
        let (_dir, ctx) = ctx_with_project().await;
        // approve_dir must be approvable: a fresh tempdir (under system temp,
        // not /, $HOME, or a denied path).
        let ext = tempfile::tempdir().unwrap();
        let stored_input = json!({
            "path": ext.path().join("plan.md").to_str().unwrap(),
            "content": "preserved content"
        });
        let handle = ctx.output_buffer.store_pending_write(
            "create_file".to_string(),
            stored_input.clone(),
            ext.path().to_path_buf(),
        );

        let replayed = maybe_replay_ack(&ctx, json!({ "path": handle }), "create_file")
            .await
            .unwrap();
        assert_eq!(replayed, stored_input, "replay returns original input");

        let roots = ctx.agent.session_write_roots_snapshot().await;
        assert!(
            roots.iter().any(|r| r == ext.path()),
            "approve_dir should be a session write root now: {roots:?}"
        );
    }

    #[tokio::test]
    async fn replay_unknown_handle_errors() {
        let (_dir, ctx) = ctx_with_project().await;
        let err = maybe_replay_ack(&ctx, json!({ "path": "@ack_deadbeef" }), "create_file")
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("expired or unknown"), "got: {err}");
    }

    #[tokio::test]
    async fn replay_cross_tool_handle_rejected() {
        let (_dir, ctx) = ctx_with_project().await;
        let ext = tempfile::tempdir().unwrap();
        let handle = ctx.output_buffer.store_pending_write(
            "create_file".to_string(),
            json!({ "path": ext.path().join("p").to_str().unwrap(), "content": "c" }),
            ext.path().to_path_buf(),
        );
        let err = maybe_replay_ack(&ctx, json!({ "path": handle }), "edit_file")
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("minted by"), "got: {err}");
    }

    #[tokio::test]
    async fn non_handle_path_passes_through_unchanged() {
        let (_dir, ctx) = ctx_with_project().await;
        let input = json!({ "path": "src/main.rs", "content": "x" });
        let out = maybe_replay_ack(&ctx, input.clone(), "create_file")
            .await
            .unwrap();
        assert_eq!(out, input);
    }

    // ---- a LEAF that is itself a symlink: the ack flow follows the classifier -------------------
    //
    // `docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md`
    //
    // The outside targets below do not exist and are never created: a DANGLING link names them, and
    // the classifier judges the path a write through it would create. They sit under `/var`, which
    // the ambient environment these tests run in (`resolve_write_or_capture` reads it) does not
    // cover; the system temp dir does, so a tempdir could not stand in for "outside".

    #[cfg(unix)]
    fn plant_leaf_link(project: &std::path::Path, target: &str) -> String {
        let link = project.join("leaf");
        std::os::unix::fs::symlink(target, &link).unwrap();
        link.to_str().unwrap().to_string()
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_leaf_link_to_an_unapproved_outside_path_captures_an_ack_for_the_targets_directory() {
        let (dir, ctx) = ctx_with_project().await;
        let link = plant_leaf_link(dir.path(), "/var/ce_ack_leaf_capture/victim.txt");
        let input = json!({ "path": link, "content": "x" });
        let outcome = resolve_write_or_capture(&ctx, "create_file", &input, &link)
            .await
            .unwrap();
        match outcome {
            WriteOutcome::Pending(env) => {
                let hint = env["hint"].as_str().expect("hint");
                assert!(
                    hint.contains("/var/ce_ack_leaf_capture"),
                    "the ack must name the TARGET's directory, got: {env}"
                );
                assert!(
                    env["pending_ack"]
                        .as_str()
                        .is_some_and(|h| h.starts_with("@ack_")),
                    "got: {env}"
                );
            }
            other => panic!("a link out of the roots must be captured, got: {other:?}"),
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_leaf_link_into_an_approved_directory_is_written_after_the_ack_is_replayed() {
        let (dir, ctx) = ctx_with_project().await;
        let link = plant_leaf_link(dir.path(), "/var/ce_ack_leaf_replay/victim.txt");
        let input = json!({ "path": link, "content": "x" });
        let handle = match resolve_write_or_capture(&ctx, "create_file", &input, &link)
            .await
            .unwrap()
        {
            WriteOutcome::Pending(env) => env["pending_ack"].as_str().unwrap().to_string(),
            other => panic!("expected an ack before approval, got: {other:?}"),
        };
        let replayed = maybe_replay_ack(&ctx, json!({ "path": handle }), "create_file")
            .await
            .unwrap();
        assert_eq!(replayed, input);
        match resolve_write_or_capture(&ctx, "create_file", &input, &link)
            .await
            .unwrap()
        {
            WriteOutcome::Write(p) => assert!(
                p.ends_with("leaf"),
                "the write is handed the link itself, got: {p:?}"
            ),
            other => panic!("after approval the write must proceed, got: {other:?}"),
        }
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn a_leaf_link_to_a_deny_listed_path_is_refused_and_offers_no_ack() {
        // `/etc/gshadow` is on the default deny list. Whether or not it exists on this host the
        // link is refused, and a hard denial must never mint an `@ack_*` handle: the error is
        // the whole answer.
        let (dir, ctx) = ctx_with_project().await;
        let link = plant_leaf_link(dir.path(), "/etc/gshadow");
        let input = json!({ "path": link, "content": "x" });
        let err = resolve_write_or_capture(&ctx, "create_file", &input, &link)
            .await
            .expect_err("a link to a deny-listed path must be refused");
        let msg = err.to_string();
        assert!(msg.contains("protected location"), "got: {msg}");
        assert!(
            !msg.contains("@ack_") && !msg.contains("approve_write(path="),
            "a hard denial must not point at an approval, got: {msg}"
        );
    }
}
