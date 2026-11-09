//! Request-scoped provenance protection. This is not run_command's telemetry effect.
use super::core::types::ToolContext;
use super::native_provenance_fence::{canonical_checkout, Fence};
use super::{ClientIdentity, RecoverableError};
use rmcp::model::Content;
use serde_json::{json, Map, Value};
use std::future::Future;
use std::path::Path;
use std::sync::Arc;

struct Scope {
    fence: Arc<Fence>,
    coordination: Arc<Fence>,
    trusted_formatter: bool,
    certificate: Option<Value>,
}

tokio::task_local! { static FENCE: Scope; }

fn is_foreground_formatter(name: &str, input: &Value) -> bool {
    name == "run_command"
        && matches!(
            input.get("command").and_then(Value::as_str),
            Some("bash scripts/fmt-mine.sh" | "./scripts/fmt-mine.sh")
        )
        && ["run_in_background", "interactive"].iter().all(|key| {
            input
                .get(*key)
                .is_none_or(|value| value == &Value::Bool(false))
        })
}

fn certificate(
    name: &str,
    input: &Value,
    meta: Option<&Map<String, Value>>,
    client: Option<&ClientIdentity>,
    fence: &Fence,
    coordination: &Fence,
) -> anyhow::Result<Option<Value>> {
    if !is_foreground_formatter(name, input)
        || !client.is_some_and(|client| client.name == "codex-mcp-client")
    {
        return Ok(None);
    }
    let Some(meta) = meta else {
        return Ok(None);
    };
    let Some(proof) = meta.get("codex/nativeExecutionProvenance") else {
        return Ok(None);
    };
    if proof.get("fence_capability") != Some(&Value::Bool(true)) {
        return Ok(None);
    }
    if super::native_provenance_fence::has_unverified_shell(coordination.root())? {
        return Err(RecoverableError::new(
            "UNKNOWN: uncertified shell descendants remain possible in this coordination domain",
        )
        .into());
    }
    let refuse = || {
        RecoverableError::new(
            "native provenance checkpoint is not bound to this formatter invocation/fence",
        )
    };
    let checkpoint = proof.get("checkpoint").ok_or_else(refuse)?;
    let identity = checkpoint.get("identity").ok_or_else(refuse)?;
    let call_id = meta
        .get("codex/toolCallId")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(refuse)?;
    if identity.get("root") != meta.get("sessionId")
        || identity.get("thread") != meta.get("threadId")
        || checkpoint.get("call_id").and_then(Value::as_str) != Some(call_id)
        || proof
            .get("fence")
            .and_then(|value| value.get("root"))
            .and_then(Value::as_str)
            != fence.root().to_str()
        || proof
            .get("fence")
            .and_then(|value| value.get("generation"))
            .and_then(Value::as_u64)
            != Some(fence.generation())
        || proof.get("coordination_generation").and_then(Value::as_u64)
            != Some(coordination.generation())
        || proof.get("coordination_root").and_then(Value::as_str) != coordination.root().to_str()
    {
        return Err(refuse().into());
    }
    Ok(Some(json!({
        "checkpoint":checkpoint, "invocation_id":call_id,
        "fence_root":fence.root(), "fence_generation":fence.generation(),
        "coordination_generation":coordination.generation(), "coordination_root":coordination.root(),
    })))
}

pub(crate) async fn scope<F>(
    ctx: &ToolContext,
    writing: bool,
    name: &str,
    input: &Value,
    meta: Option<&Map<String, Value>>,
    future: F,
) -> anyhow::Result<Vec<Content>>
where
    F: Future<Output = anyhow::Result<Vec<Content>>>,
{
    if !writing && name != "run_command" {
        return future.await;
    }
    let root = ctx
        .agent
        .require_project_root_for(ctx.workspace_override.as_deref())
        .await?;
    let client = ctx.client_identity();
    scope_root(&root, writing, name, input, meta, client.as_ref(), future).await
}

async fn scope_root<F>(
    root: &Path,
    writing: bool,
    name: &str,
    input: &Value,
    meta: Option<&Map<String, Value>>,
    client: Option<&ClientIdentity>,
    future: F,
) -> anyhow::Result<Vec<Content>>
where
    F: Future<Output = anyhow::Result<Vec<Content>>>,
{
    if !writing && name != "run_command" {
        return future.await;
    }
    #[cfg(not(unix))]
    return future.await;
    #[cfg(unix)]
    {
        // Cwd does not confine a shell's write targets. A per-user coordination
        // lease closes effects from another checkout or server/profile as well.
        let domain = super::native_provenance_fence::coordination_root()?;
        let formatter = is_foreground_formatter(name, input);
        let native_proof = client.is_some_and(|client| client.name == "codex-mcp-client")
            && meta
                .and_then(|meta| meta.get("codex/nativeExecutionProvenance"))
                .and_then(|proof| proof.get("fence_capability"))
                == Some(&Value::Bool(true));
        let trusted_formatter = formatter && native_proof;
        // Scope decision (bug 312cde2f4698874b): the approved design motivates exclusivity by
        // native checkpoint validity through the formatter's last write, so only a formatter
        // that can use a checkpoint takes the leases exclusively. Before this change every
        // foreground formatter did, which also kept concurrent writers away from formatters
        // without a proof; that extra behavior is removed deliberately, and they now share the
        // leases like any other command. `trusted_formatter` is only eligibility: an eligible
        // call is still refused below when its proof does not validate.
        let acquire = |root: &Path| {
            if trusted_formatter {
                Fence::try_acquire(root)
            } else {
                Fence::try_participate(root)
            }
        };
        let mut coordination = acquire(&domain).map_err(|error| {
            RecoverableError::new(format!(
                "native provenance coordination refused dispatch: {error}"
            ))
        })?;
        let root = canonical_checkout(root).or_else(|_| root.canonicalize())?;
        let mut fence = acquire(&root).map_err(|error| {
            RecoverableError::new(format!("native provenance fence refused dispatch: {error}"))
        })?;
        if trusted_formatter {
            super::trusted_formatter::verify(&root, input).map_err(|error| {
                RecoverableError::new(format!(
                    "UNKNOWN: trusted formatter closure is unavailable: {error}"
                ))
            })?;
        }
        let certificate = certificate(name, input, meta, client, &fence, &coordination)?;
        coordination.bump()?;
        fence.bump()?;
        if name == "run_command" && !trusted_formatter {
            super::native_provenance_fence::mark_unverified_shell(&domain)?;
        }
        FENCE
            .scope(
                Scope {
                    fence: Arc::new(fence),
                    coordination: Arc::new(coordination),
                    trusted_formatter,
                    certificate,
                },
                future,
            )
            .await
    }
}

/// Called before every supported shell spawn, including detached supervisors.
pub(crate) fn apply_to_shell(command: &mut tokio::process::Command) -> anyhow::Result<()> {
    command
        .env_remove("CODESCOUT_NATIVE_PROVENANCE")
        .env_remove("CODESCOUT_PROVENANCE_FENCE_FD")
        .env_remove("CODESCOUT_PROVENANCE_FENCE_ROOT")
        .env_remove("CODESCOUT_PROVENANCE_FENCE_GENERATION")
        .env_remove("CODESCOUT_PROVENANCE_COORDINATOR_FD")
        .env_remove("CODESCOUT_PROVENANCE_COORDINATOR_ROOT")
        .env_remove("CODESCOUT_PROVENANCE_COORDINATOR_GENERATION");
    FENCE
        .try_with(|scope| -> anyhow::Result<()> {
            if scope.trusted_formatter {
                super::trusted_formatter::apply_environment(command.as_std_mut());
            }
            scope
                .coordination
                .apply_to_command_named(command.as_std_mut(), "CODESCOUT_PROVENANCE_COORDINATOR")?;
            scope.fence.apply_to_command(command.as_std_mut())?;
            if let Some(certificate) = &scope.certificate {
                command.env(
                    "CODESCOUT_NATIVE_PROVENANCE",
                    serde_json::to_string(certificate)?,
                );
            }
            Ok(())
        })
        .unwrap_or(Ok(()))
}

#[cfg(all(test, unix))]
#[path = "provenance_fence_context_tests.rs"]
mod tests;
