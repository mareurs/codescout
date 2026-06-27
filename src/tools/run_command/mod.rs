//! RunCommand tool — executes shell commands with buffered output, interactive
//! mode, background tasks, and session-scoped @cmd_* ref buffers.

mod inner;
mod interactive;
mod output;

use crate::tools::output_buffer::looks_like_ack_handle;
use inner::run_command_inner;
use interactive::run_command_interactive;
use output::format_run_command;

use super::{parse_bool_param, Tool, ToolContext};
use serde_json::{json, Value};

pub struct RunCommand;

/// Extract a u64 from a JSON value that may be a Number or a numeric String.
fn get_timeout_u64(v: &Value) -> Option<u64> {
    match v {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.parse::<u64>().ok(),
        _ => None,
    }
}

/// Parse the timeout from run_command input with leniency for:
/// - wrong key name (`timeout` instead of `timeout_secs`)
/// - millisecond values passed as `timeout_secs` (value > 86_400)
///
/// Returns `(resolved_seconds, optional_hint_for_agent)`.
fn parse_timeout_input(input: &Value) -> (u64, Option<String>) {
    // Canonical key: timeout_secs
    if let Some(v) = get_timeout_u64(&input["timeout_secs"]) {
        if v == 0 {
            return (
                30,
                Some("timeout_secs: 0 is invalid — using default of 30s.".to_string()),
            );
        }
        if v > 86_400 {
            let converted = v / 1_000;
            return (
                converted,
                Some(format!(
                    "timeout_secs: {v} looks like milliseconds — converted to {converted}s. \
                     Use timeout_secs with a value in seconds."
                )),
            );
        }
        return (v, None);
    }

    // Fallback: wrong key name `timeout`
    if let Some(v) = get_timeout_u64(&input["timeout"]) {
        if v == 0 {
            return (
                30,
                Some(
                    "Unknown parameter 'timeout' — use timeout_secs. \
                     Value 0 is invalid, using default of 30s."
                        .to_string(),
                ),
            );
        }
        if v >= 1_000 {
            let converted = v / 1_000;
            return (
                converted,
                Some(format!(
                    "Unknown parameter 'timeout' — use timeout_secs. \
                     Converted {v}ms → {converted}s."
                )),
            );
        }
        // v < 1000 → already seconds
        return (
            v,
            Some(format!(
                "Unknown parameter 'timeout' — use timeout_secs. \
                 Interpreted {v} as seconds."
            )),
        );
    }

    // Neither key present
    (30, None)
}

#[async_trait::async_trait]
impl Tool for RunCommand {
    fn name(&self) -> &str {
        "run_command"
    }

    fn relevant_guide_topic(&self) -> Option<&str> {
        Some("progressive-disclosure")
    }

    fn description(&self) -> &str {
        "Run a shell command in the project root. Large output is buffered as @cmd_* refs."
    }

    fn long_docs(&self) -> Option<&str> {
        Some(
            "## Output buffering\n\
             \n\
             Short output (< 50 lines) is returned inline.\n\
             Long output is stored as `@cmd_xxxx` and a smart summary is returned.\n\
             Query the buffer in a follow-up: `run_command(\"grep FAILED @cmd_xxxx\")`.\n\
             Never pipe output inline — use the buffer ref instead.\n\
             \n\
             ## Key parameters\n\
             \n\
             - `command`: shell command string. May reference `@cmd_*` buffer refs.\n\
             - `cwd`: subdirectory relative to project root.\n\
             - `timeout_secs`: default 30; raise for long builds.\n\
             - `run_in_background=true`: detach and return immediately.\n\
             - `interactive=true`: spawn with stdin/stdout for REPLs.\n\
             - `acknowledge_risk=true`: bypass the dangerous-command gate (use the `@ack_*` \
             handle from the rejection response instead).\n\
             \n\
             ## Dangerous commands\n\
             \n\
             Commands matching destructive patterns (rm -rf, dd, mkfs, …) are blocked.\n\
             The rejection response contains an `@ack_*` handle — pass it as `acknowledge_risk` \
             to proceed after the user confirms.\n\
             \n\
             ## Tips\n\
             \n\
             - `cargo test` → buffer ref → `grep FAILED @cmd_xxx` to find failures.\n\
             - `cargo build` → buffer ref → `grep error @cmd_xxx` to find errors.\n\
             - Add trusted commands to `shell_allow_always` in `project.toml [security]`.",
        )
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["command"],
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command. May reference @cmd_* buffers (e.g. grep FAILED @cmd_abc)."
                },
                "timeout_secs": { "type": "integer", "default": 30, "description": "Max seconds (default 30)." },
                "cwd": { "type": "string", "description": "Subdirectory relative to project root." },
                "acknowledge_risk": { "type": "boolean", "description": "Bypass dangerous-command check. Prefer @ack_* handle from the rejected response." },
                "run_in_background": { "type": "boolean", "description": "Detach and return immediately. Use for long-running or backgrounded (&) commands." },
                "interactive": { "type": "boolean", "description": "Spawn process with interactive stdin/stdout. Elicits input after each output chunk. Use for REPLs, prompts, and interactive CLIs." }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        use super::output_buffer::OutputBuffer;

        let command = super::require_str_param(&input, "command")?;
        let (timeout_secs, timeout_hint) = parse_timeout_input(&input);
        let acknowledge_risk = parse_bool_param(&input["acknowledge_risk"]);
        let run_in_background = parse_bool_param(&input["run_in_background"]);
        let interactive = parse_bool_param(&input["interactive"]);
        let cwd_param = input["cwd"].as_str();
        let root = ctx
            .agent
            .require_project_root_for(ctx.workspace_override.as_deref())
            .await?;
        let security = ctx
            .agent
            .security_config_for(ctx.workspace_override.as_deref())
            .await;

        // --- Interactive mode: elicitation-driven stdin loop ---
        if interactive {
            return run_command_interactive(
                command,
                cwd_param,
                timeout_secs,
                &root,
                &security,
                ctx,
            )
            .await;
        }

        // --- Early dispatch: @ack_* handle ---
        if looks_like_ack_handle(command) {
            let stored = ctx.output_buffer.get_dangerous(command).ok_or_else(|| {
                super::RecoverableError::with_hint(
                    "ack handle expired or unknown",
                    "Re-run the original command to get a fresh handle.",
                )
            })?;
            return run_command_inner(
                &stored.command,
                &stored.command,
                stored.timeout_secs,
                true, // acknowledge_risk
                stored.cwd.as_deref(),
                false, // buffer_only
                false, // run_in_background — ack re-dispatch is always foreground
                &root,
                &security,
                ctx,
            )
            .await;
        }

        // --- Step 1: Resolve @cmd_ buffer references ---
        // IL3 gate runs BEFORE resolve_refs so buffer refs are still visible —
        // `grep PATTERN @cmd_xxx | sort` is allowed (buffer-op), but
        // `cargo test | grep FAILED` is not. See `detect_il3_violation`.
        if let Some(hint) = crate::util::path_security::detect_il3_violation(command) {
            return Err(super::RecoverableError::new(hint).into());
        }
        let (resolved_command, temp_files, buffer_only, refreshed_handles) =
            ctx.output_buffer.resolve_refs(command)?;

        // Helper: run inner logic then always clean up temp files.
        let mut result = run_command_inner(
            command,
            &resolved_command,
            timeout_secs,
            acknowledge_risk,
            cwd_param,
            buffer_only,
            run_in_background,
            &root,
            &security,
            ctx,
        )
        .await;

        OutputBuffer::cleanup_temp_files(&temp_files);

        // Inject refresh indicator into stdout when any @file_* handle was auto-refreshed.
        if !refreshed_handles.is_empty() {
            if let Ok(ref mut val) = result {
                let prefix: String = refreshed_handles
                    .iter()
                    .map(|id| {
                        format!(
                            "↻ {} refreshed from disk (file changed since last read)\n",
                            id
                        )
                    })
                    .collect();
                // Note: silently skips injection if "stdout" is absent (e.g. pending_ack
                // shape or buffered-output summary). These cases are extremely unlikely
                // to co-occur with a @file_* refresh, but worth noting.
                if let Some(stdout) = val["stdout"].as_str() {
                    val["stdout"] = serde_json::json!(format!("{}{}", prefix, stdout));
                }
            }
        }

        // Attach timeout hint when the timeout parameter was auto-corrected.
        if let Some(ref hint) = timeout_hint {
            if let Ok(ref mut val) = result {
                val["timeout_hint"] = json!(hint);
            }
        }

        result
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_run_command(result))
    }
}

#[cfg(test)]
mod tests;
