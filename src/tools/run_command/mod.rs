//! RunCommand tool — executes shell commands with buffered output, interactive
//! mode, background tasks, and session-scoped @cmd_* ref buffers.

mod attribution;
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

/// The effect a `run_command` caller DECLARES for its command: `"write"` (the default) or
/// `"read"`, or `None` when `effect` is present but is neither.
///
/// **Absent means write.** Nothing can derive a shell command's effect from its string — that
/// is the halting problem wearing a costume — so the safe reading of silence is the mutating
/// one: a command is a write unless its caller says otherwise
/// (`docs/issues/2026-09-20-run-command-never-overrides-is-write.md`).
///
/// **Recorded, never enforced.** The value lands in `usage.db`'s `effect_class` so telemetry
/// can separate `ls` from `git reset --hard`; it does NOT take the write lock — see
/// [`RunCommand`]'s `is_write`. One definition, read by `call` (which refuses `None`) and by
/// the usage recorder (which then records no class rather than guessing one).
pub(crate) fn declared_effect(input: &Value) -> Option<&'static str> {
    match input.get("effect") {
        None | Some(Value::Null) => Some("write"),
        Some(Value::String(s)) if s == "write" => Some("write"),
        Some(Value::String(s)) if s == "read" => Some("read"),
        Some(_) => None,
    }
}

#[async_trait::async_trait]
impl Tool for RunCommand {
    fn name(&self) -> &str {
        "run_command"
    }

    fn relevant_guide_topic(&self, _result: &Value) -> Option<&str> {
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
             - `effect`: `\"read\"` or `\"write\"` (default) — what the command does to files. \
             Recorded in telemetry so reads and writes are separable; it takes no lock.\n\
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
                "interactive": { "type": "boolean", "description": "Spawn process with interactive stdin/stdout. Elicits input after each output chunk. Use for REPLs, prompts, and interactive CLIs." },
                "effect": { "type": "string", "enum": ["read", "write"], "default": "write", "description": "What the command does to files. Recorded in telemetry; takes no lock." }
            }
        })
    }

    /// **Deliberately `false` for every input, including `effect: "write"` — an operator
    /// ruling (2026-09-24), not an omission.** `run_command` was the one tool with no
    /// override, and the natural repair is to return the declared effect. It was measured and
    /// rejected: the server holds the cross-process write lock for the WHOLE call and refuses
    /// waiters after `write_lock_timeout_secs` (5), while `run_command`'s p95 is 43 s — so a
    /// default-write lock turns one session's foreground `cargo test` into every peer's
    /// refused edits. The declared effect is recorded instead ([`declared_effect`], usage.db
    /// `effect_class`); shell writes stay unserialized against the edit tools, exactly as
    /// before. Pinned by `run_command_never_takes_the_write_lock_whatever_it_declares`.
    /// docs/issues/2026-09-20-run-command-never-overrides-is-write.md
    fn is_write(&self, _input: &Value) -> bool {
        false
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        use super::output_buffer::OutputBuffer;

        let command = super::require_str_param(&input, "command")?;
        if declared_effect(&input).is_none() {
            return Err(super::RecoverableError::with_hint(
                format!("unknown `effect`: {}", input["effect"]),
                "`effect` is \"read\" or \"write\" (the default when omitted). It is recorded in \
                 telemetry and takes no lock, so declare what the command does to files.",
            )
            .into());
        }
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
                stored.run_in_background,
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

        // Attach a truncation notice when the command read a buffer that holds only a
        // PREFIX of what was captured. This is the half that closes the observable:
        // the in-buffer sentinel is visible to `tail`/`wc`/a slice, but a `grep -c`
        // over the missing tail returns `0` and shows the caller nothing at all — and
        // `0` is byte-identical to genuinely absent. Reported as a field rather than
        // injected into stdout, because a count or a hash is a value the caller will
        // parse, and prepending prose to it corrupts the thing they asked for.
        //
        // BUG docs/issues/archive/2026-08-27-unfiltered-output-lines-counts-the-source-not-the-buffer.md
        let truncation_notices = ctx.output_buffer.truncation_notices_in(command);
        if !truncation_notices.is_empty() {
            if let Ok(ref mut val) = result {
                val["buffer_truncated"] = serde_json::json!(truncation_notices);
            }
        }

        // Background job state travels in the ENVELOPE, and it has to.
        //
        // A `@bg_*` handle is resolved by textual substitution into the shell
        // command, so it can only ever expand to a FILENAME — the channel cannot
        // carry a status, and the exit code the shell hands back belongs to the
        // reader (`tail`, `cat`) rather than to the job. That is the whole reason
        // a backgrounded failure read as success: the caller asked `tail` how it
        // went. Attaching state here is what makes the supervisor's observation
        // reachable; without it the job record would be written and never read.
        //
        // docs/issues/archive/2026-09-13-background-command-loses-terminal-status.md
        let job_states = ctx.output_buffer.job_states_in(command);
        if !job_states.is_empty() {
            if let Ok(ref mut val) = result {
                val["jobs"] = serde_json::json!(job_states
                    .iter()
                    .map(|(id, state, cmd)| {
                        serde_json::json!({
                            "handle": id,
                            "state": state.summary(),
                            "command": cmd,
                        })
                    })
                    .collect::<Vec<_>>());
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

    /// Hidden from `list_tools` when `security.shell_command_mode = "disabled"`.
    ///
    /// Purely an exposure trim — it saves the agent this tool's description and
    /// schema, and stops it reaching for a door that is bolted. The refusal in
    /// `run_command_inner` (step 3) is the enforcement, and it stays: this gate
    /// reads the SESSION-DEFAULT project, while `call` reads the
    /// `workspace`-pinned one, so a pinned call into a shell-disabled project
    /// never passes through here at all. See `Availability::RequiresShell`.
    ///
    /// Note the side effect of hiding rather than merely refusing: `run_command`
    /// is the only door to `@cmd_*` buffers, so an agent that cannot see the
    /// tool also cannot query one. That falls out of the choice to hide rather
    /// than from the `buffer_only` bypass inside `call`, which still exempts
    /// buffer queries from the refusal itself.
    fn availability(&self, _caps: &crate::tools::ToolCapabilities) -> crate::tools::Availability {
        crate::tools::Availability::RequiresShell
    }
}

#[cfg(test)]
mod tests;

/// #56 lives here rather than in `tests.rs` on purpose: that file carries a live peer's
/// uncommitted work, and a path commit takes the whole working-tree file.
#[cfg(test)]
mod effect_tests {
    use super::*;
    use crate::tools::Tool;

    async fn ctx() -> (tempfile::TempDir, ToolContext) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        (
            dir,
            ToolContext {
                agent,
                lsp: crate::lsp::LspManager::new_arc(),
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

    /// `effect` is the caller's declaration of what the command does to files. Absent means
    /// WRITE — a shell command is a write unless someone says otherwise, because nothing can
    /// derive it from the command string (`docs/issues/2026-09-20-run-command-never-overrides-is-write.md`).
    /// `None` for anything else, so `call` can refuse it and the recorder records no class
    /// rather than guessing one.
    #[test]
    fn declared_effect_defaults_to_write_and_accepts_only_read_or_write() {
        assert_eq!(
            declared_effect(&json!({"command": "ls"})),
            Some("write"),
            "absent must mean write"
        );
        assert_eq!(
            declared_effect(&json!({"command": "ls", "effect": "read"})),
            Some("read")
        );
        assert_eq!(
            declared_effect(&json!({"command": "ls", "effect": "write"})),
            Some("write")
        );
        assert_eq!(
            declared_effect(&json!({"command": "ls", "effect": "delete"})),
            None
        );
        assert_eq!(
            declared_effect(&json!({"command": "ls", "effect": true})),
            None
        );
    }

    /// An `effect` outside the two classes is refused before anything runs, naming both valid
    /// values — silently treating `"delete"` as a write would record a class nobody declared.
    /// Load-bearing: `echo` succeeds when the refusal is missing, so a pass here cannot be the
    /// command failing for some other reason.
    #[tokio::test]
    async fn run_command_refuses_an_effect_it_does_not_know() {
        let (_dir, ctx) = ctx().await;
        let err = RunCommand
            .call(json!({"command": "echo hi", "effect": "delete"}), &ctx)
            .await
            .expect_err("an unknown effect must be refused, not run");
        let rec = err
            .downcast_ref::<crate::tools::RecoverableError>()
            .expect("recoverable, so sibling calls survive");
        let text = format!("{} {}", rec.message, rec.hint().unwrap_or_default());
        assert!(
            text.contains("\"read\"") && text.contains("\"write\""),
            "must name both valid values: {text}"
        );
    }
}
