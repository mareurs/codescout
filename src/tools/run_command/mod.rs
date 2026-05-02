//! RunCommand tool — executes shell commands with buffered output, interactive
//! mode, background tasks, and session-scoped @cmd_* ref buffers.

use std::path::Path;

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
        let root = ctx.agent.require_project_root().await?;
        let security = ctx.agent.security_config().await;

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

fn format_run_command(result: &Value) -> String {
    let mut s = if result["output_id"].is_string() {
        let exit = result["exit_code"].as_i64().unwrap_or(0);
        let check = if exit == 0 { "✓" } else { "✗" };
        let output_id = result["output_id"].as_str().unwrap_or("");
        match result["type"].as_str() {
            Some("test") => {
                let passed = result["passed"].as_u64().unwrap_or(0);
                let failed = result["failed"].as_u64().unwrap_or(0);
                let ignored = result["ignored"].as_u64().unwrap_or(0);
                let mut s = format!("{check} exit {exit} · {passed} passed");
                if failed > 0 {
                    s.push_str(&format!(" · {failed} FAILED"));
                }
                if ignored > 0 {
                    s.push_str(&format!(" · {ignored} ignored"));
                }
                s.push_str(&format!("  (query {output_id})"));
                s
            }
            Some("build") => {
                let errors = result["errors"].as_u64().unwrap_or(0);
                if errors > 0 {
                    format!("{check} exit {exit} · {errors} errors  (query {output_id})")
                } else {
                    format!("{check} exit {exit}  (query {output_id})")
                }
            }
            _ => format!("{check} exit {exit}  (query {output_id})"),
        }
    } else if result["timed_out"].as_bool().unwrap_or(false) {
        "✗ timed out".to_string()
    } else {
        let exit = result["exit_code"].as_i64().unwrap_or(0);
        let stdout_lines = result["stdout"]
            .as_str()
            .map(|s| s.lines().count())
            .unwrap_or(0);
        let check = if exit == 0 { "✓" } else { "✗" };
        format!("{check} exit {exit} · {stdout_lines} lines")
    };

    // Append timeout hint after all branch logic so it covers every output shape.
    if let Some(hint) = result["timeout_hint"].as_str() {
        s.push_str(&format!("\n⚠ timeout: {hint}"));
    }

    s
}

/// Returns true when `command` is a bare `@ack_<8hex>` handle.
fn looks_like_ack_handle(command: &str) -> bool {
    let s = command.trim();
    if !s.starts_with("@ack_") {
        return false;
    }
    let suffix = &s[5..]; // after "@ack_"
    suffix.len() == 8 && suffix.chars().all(|c| c.is_ascii_hexdigit())
}

/// Reassemble a buffered command summary with a stable, reader-friendly field order.
///
/// Dynamic field appending (`obj["key"] = val`) always places fields last, which
/// caused `output_id` (the buffer reference) to land after `stdout`/`failures`/
/// `first_error` (the bulk content). Correct order:
///   type → exit_code → output_id → [counts] → [content]
fn rebuild_buffered_summary(raw: Value, output_id: &str) -> Value {
    // These are large text fields — always go last.
    const CONTENT_FIELDS: &[&str] = &["stdout", "failures", "first_error"];

    let mut map = serde_json::Map::new();

    // 1. Status identity
    if let Some(v) = raw.get("type") {
        map.insert("type".into(), v.clone());
    }
    if let Some(v) = raw.get("exit_code") {
        map.insert("exit_code".into(), v.clone());
    }

    // 2. Buffer reference — most action-relevant, agent needs this to query results
    map.insert("output_id".into(), json!(output_id));

    // 3. Type-specific compact fields (counts, not content)
    let raw_obj = raw.as_object().expect("summary is always an object");
    for (k, v) in raw_obj {
        if !["type", "exit_code"].contains(&k.as_str()) && !CONTENT_FIELDS.contains(&k.as_str()) {
            map.insert(k.clone(), v.clone());
        }
    }

    // 4. Content fields last — bulk payload
    for field in CONTENT_FIELDS {
        if let Some(v) = raw_obj.get(*field) {
            map.insert((*field).into(), v.clone());
        }
    }

    Value::Object(map)
}

/// Interactive mode: spawn a process with piped stdin/stdout/stderr, then drive it
/// via MCP elicitation in a loop until the process exits or the user cancels.
///
/// Design notes (spike — E-3):
/// - Uses a 150 ms settle window to batch initial output before the first elicit.
/// - On each elicit round-trip we collect whatever is available (non-blocking drain),
///   show it to the user, and send their input back to the process.
/// - Empty input = user wants to cancel; we kill the process and return accumulated output.
/// - If elicitation is unavailable (no peer), returns a RecoverableError guiding the
///   caller to use the non-interactive path.
///
/// Latency concern (noted for spike evaluation):
///   Each stdin→stdout round-trip requires one MCP elicitation request+response, which
///   adds roughly the Claude Code UI round-trip latency (~1-3 s) per interaction step.
///   This is acceptable for slow interactive CLIs (setup wizards, REPLs with human
///   think-time) but unusable for high-frequency interactive programs.
async fn run_command_interactive(
    command: &str,
    cwd_param: Option<&str>,
    _timeout_secs: u64,
    root: &Path,
    security: &crate::util::path_security::PathSecurityConfig,
    ctx: &ToolContext,
) -> anyhow::Result<Value> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::process::Command;

    // Gate: elicitation must be available.
    if ctx.peer.is_none() {
        return Err(super::RecoverableError::with_hint(
            "interactive mode requires elicitation support",
            "The MCP client does not support elicitation. Use run_command without interactive: true.",
        )
        .into());
    }

    // Dangerous command check — block in interactive mode to keep the spike focused.
    if let Some(reason) = crate::util::path_security::is_dangerous_command(command, security) {
        return Err(super::RecoverableError::with_hint(
            format!("interactive mode blocked dangerous command: {reason}"),
            "Remove the dangerous pattern or use the non-interactive path with acknowledge_risk: true.",
        )
        .into());
    }

    // Resolve working directory.
    let work_dir = if let Some(rel) = cwd_param {
        let candidate = root.join(rel);
        candidate.canonicalize().map_err(|e| {
            super::RecoverableError::with_hint(
                format!("cwd '{rel}' is not a valid directory: {e}"),
                "Provide a relative path to an existing subdirectory of the project.",
            )
        })?
    } else {
        root.to_path_buf()
    };

    // Spawn with piped stdin/stdout/stderr.
    let (shell, shell_args) = crate::platform::shell_command(command);
    let mut child = Command::new(shell)
        .args(&shell_args)
        .current_dir(&work_dir)
        .env("GIT_PAGER", "cat")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().expect("stdin piped");
    let mut stdout_reader = tokio::io::BufReader::new(child.stdout.take().expect("stdout piped"));
    let mut stderr_reader = tokio::io::BufReader::new(child.stderr.take().expect("stderr piped"));

    let mut accumulated_output = String::new();

    // Drain available output from stdout+stderr using a settle window.
    // We use two separate buffers to avoid the double-borrow-of-mut-buf compiler error
    // when both futures reference the same buffer slice simultaneously.
    //
    // Loop structure: alternate trying stdout vs stderr within the settle timeout;
    // break out of the loop when both are silent for `settle_ms` ms (timeout fires).
    //
    // Note: this is an inner async fn — Rust supports these as non-capturing closures.
    // We cannot use a closure here because async closures that borrow mutable state across
    // await points are not yet stable (rust-lang/rust#62290).
    async fn drain_with_settle(
        stdout_reader: &mut tokio::io::BufReader<tokio::process::ChildStdout>,
        stderr_reader: &mut tokio::io::BufReader<tokio::process::ChildStderr>,
        settle_ms: u64,
    ) -> String {
        let settle = std::time::Duration::from_millis(settle_ms);
        let mut output = String::new();
        // Two independent buffers — one per reader — avoids the E0499 double-borrow.
        let mut out_buf = [0u8; 4096];
        let mut err_buf = [0u8; 4096];

        loop {
            tokio::select! {
                result = tokio::time::timeout(settle, stdout_reader.read(&mut out_buf)) => {
                    match result {
                        Ok(Ok(n)) if n > 0 => {
                            output.push_str(&String::from_utf8_lossy(&out_buf[..n]));
                        }
                        _ => break, // timeout or EOF
                    }
                }
                result = tokio::time::timeout(settle, stderr_reader.read(&mut err_buf)) => {
                    match result {
                        Ok(Ok(n)) if n > 0 => {
                            output.push_str(&String::from_utf8_lossy(&err_buf[..n]));
                        }
                        _ => break, // timeout or EOF
                    }
                }
            }
        }
        output
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
    struct InteractiveInput {
        /// Text to send to the process stdin (leave empty to cancel and kill the process)
        input: String,
    }
    rmcp::elicit_safe!(InteractiveInput);

    // Interaction loop.
    let mut round = 0u32;
    const MAX_ROUNDS: u32 = 50; // guard against runaway loops
    loop {
        if round >= MAX_ROUNDS {
            let _ = child.kill().await;
            accumulated_output.push_str("\n[interactive: max rounds reached, process killed]");
            break;
        }
        round += 1;

        // Read post-spawn / post-input output with 150 ms settle.
        let chunk = drain_with_settle(&mut stdout_reader, &mut stderr_reader, 150).await;
        if !chunk.is_empty() {
            accumulated_output.push_str(&chunk);
        }

        // Check whether the process already exited.
        match child.try_wait() {
            Ok(Some(status)) => {
                let code = status.code().unwrap_or(-1);
                // Drain any remaining output after exit.
                let tail = drain_with_settle(&mut stdout_reader, &mut stderr_reader, 50).await;
                if !tail.is_empty() {
                    accumulated_output.push_str(&tail);
                }
                return Ok(json!({
                    "exit_code": code,
                    "stdout": accumulated_output,
                    "interactive_rounds": round,
                }));
            }
            Ok(None) => {} // still running
            Err(e) => {
                accumulated_output.push_str(&format!("\n[interactive: wait error: {e}]"));
                break;
            }
        }

        // Elicit next input from the user.
        let display_output = if accumulated_output.len() > 4000 {
            // Show only the tail to keep the elicitation dialog readable.
            &accumulated_output[crate::tools::floor_char_boundary(
                &accumulated_output,
                accumulated_output.len() - 4000,
            )..]
        } else {
            &accumulated_output
        };
        let prompt = format!(
            "Process output (round {round}):\n```\n{display_output}\n```\n\nEnter input to send to stdin, or leave empty to cancel:"
        );

        let elicited = ctx.elicit::<InteractiveInput>(prompt).await?;

        match elicited {
            None => {
                // Elicitation unavailable mid-session (shouldn't happen — we checked at entry).
                let _ = child.kill().await;
                accumulated_output
                    .push_str("\n[interactive: elicitation unavailable, process killed]");
                break;
            }
            Some(InteractiveInput { input }) if input.is_empty() => {
                // User cancelled.
                let _ = child.kill().await;
                accumulated_output.push_str("\n[interactive: cancelled by user]");
                break;
            }
            Some(InteractiveInput { mut input }) => {
                // Send input to the process (append newline if missing).
                if !input.ends_with('\n') {
                    input.push('\n');
                }
                if let Err(e) = stdin.write_all(input.as_bytes()).await {
                    accumulated_output
                        .push_str(&format!("\n[interactive: stdin write error: {e}]"));
                    let _ = child.kill().await;
                    break;
                }
            }
        }
    }

    // Final drain after loop exit.
    let tail = drain_with_settle(&mut stdout_reader, &mut stderr_reader, 50).await;
    if !tail.is_empty() {
        accumulated_output.push_str(&tail);
    }

    Ok(json!({
        "exit_code": -1,
        "stdout": accumulated_output,
        "interactive_rounds": round,
        "note": "process killed or loop exited before natural termination",
    }))
}

/// RAII guard: deletes a named temp file when dropped.
struct TmpfileGuard(String);

impl Drop for TmpfileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// RAII guard: aborts a spawned task when dropped.
struct AbortOnDrop(tokio::task::JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

fn resolve_work_dir(root: &Path, cwd_param: Option<&str>) -> anyhow::Result<std::path::PathBuf> {
    if let Some(rel) = cwd_param {
        let candidate = root.join(rel);
        let canonical = candidate.canonicalize().map_err(|e| {
            super::RecoverableError::with_hint(
                format!("cwd '{}' is not a valid directory: {}", rel, e),
                "Provide a relative path to an existing subdirectory of the project.",
            )
        })?;
        let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let tmp = crate::platform::temp_dir();
        let canonical_tmp = tmp.canonicalize().unwrap_or(tmp);
        let under_project = canonical.starts_with(canonical_root.as_path());
        let under_tmp = canonical.starts_with(canonical_tmp.as_path());
        if !under_project && !under_tmp {
            return Err(super::RecoverableError::with_hint(
                format!("cwd '{}' escapes project root", rel),
                "The cwd must be a subdirectory within the project, or a path under the \
                 platform temp directory.",
            )
            .into());
        }
        Ok(canonical)
    } else {
        Ok(root.to_path_buf())
    }
}

async fn spawn_background_command(
    resolved_command: &str,
    work_dir: &Path,
    ctx: &ToolContext,
) -> anyhow::Result<Value> {
    let log_tmp = tempfile::Builder::new()
        .prefix("codescout-bg-")
        .suffix(".log")
        .tempfile()?;
    let log_path = log_tmp.path().to_path_buf();
    let (log_file, _) = log_tmp.keep()?;
    let log_stderr = log_file.try_clone()?;

    let (shell, shell_args) = crate::platform::shell_command(resolved_command);
    let child = tokio::process::Command::new(shell)
        .args(&shell_args)
        .current_dir(work_dir)
        .env("GIT_PAGER", "cat")
        .stdout(std::process::Stdio::from(log_file))
        .stderr(std::process::Stdio::from(log_stderr))
        .spawn()?;

    // Cancel-aware warm-up: during the 5s window we hold a guard that
    // SIGKILLs the child if this future is dropped (tool cancellation).
    // After the window elapses normally the guard disarms, the tokio
    // Child handle is dropped, and the process runs detached.
    let pid = child.id();
    drop(child);
    let mut kill_guard = BackgroundKillGuard {
        pid,
        disarmed: false,
    };

    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    kill_guard.disarmed = true;

    let log_content = std::fs::read_to_string(&log_path).unwrap_or_default();
    let tail_50: String = {
        let lines: Vec<&str> = log_content.lines().collect();
        let start = lines.len().saturating_sub(50);
        lines[start..].join("\n")
    };

    let ref_id = ctx.output_buffer.store_background(log_path);

    let mut bg_result = serde_json::json!({
        "output_id": ref_id,
        "hint": format!(
            "Process running. Output captured in {} — use run_command(\"tail -50 {}\") or grep/cat as needed.",
            ref_id, ref_id
        )
    });
    if !tail_50.is_empty() {
        bg_result["stdout"] = json!(tail_50);
    }
    Ok(bg_result)
}

/// Guard that SIGKILLs a background child if dropped while armed. Used during
/// the `spawn_background_command` warm-up window so that a cancelled tool
/// future does not leave orphaned processes behind.
struct BackgroundKillGuard {
    pid: Option<u32>,
    disarmed: bool,
}

impl Drop for BackgroundKillGuard {
    fn drop(&mut self) {
        if self.disarmed {
            return;
        }
        if let Some(pid) = self.pid {
            #[cfg(unix)]
            // SAFETY: libc::kill with a PID obtained from a child we just spawned,
            // SIGKILL is safe to send. Worst case the PID was reaped and we
            // kill nothing (ESRCH), which is a no-op.
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGKILL);
            }
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("taskkill")
                    .args(["/F", "/PID", &pid.to_string()])
                    .status();
            }
        }
    }
}

fn inject_tee(
    resolved_command: &str,
    buffer_only: bool,
) -> anyhow::Result<(String, Option<TmpfileGuard>)> {
    use super::command_summary::detect_terminal_filter;
    if buffer_only {
        return Ok((resolved_command.to_string(), None));
    }
    if let Some(pipe_pos) = detect_terminal_filter(resolved_command) {
        // Use tempfile::NamedTempFile for unpredictable path (SF-3).
        // persist() converts it to a regular file we manage via TmpfileGuard.
        let named = tempfile::Builder::new()
            .prefix("codescout-unfiltered-")
            .tempfile()?;
        let tmppath = named.into_temp_path();
        let tmpfile = tmppath.to_string_lossy().to_string();
        // Keep the file on disk — TmpfileGuard handles cleanup.
        tmppath.keep()?;
        // Safety (SF-4): the path is generated by tempfile under $TMPDIR
        // and contains only alphanumeric chars, hyphens, and dots — no
        // shell metacharacters.
        if !tmpfile
            .chars()
            .all(|c| c.is_alphanumeric() || c == '/' || c == '-' || c == '_' || c == '.')
        {
            return Err(super::RecoverableError::new(format!(
                "temporary file path contains unexpected characters: {}",
                tmpfile,
            ))
            .into());
        }
        let cmd = format!(
            "{} | tee {} | {}",
            resolved_command[..pipe_pos].trim_end(),
            tmpfile,
            resolved_command[pipe_pos + 1..].trim_start()
        );
        Ok((cmd, Some(TmpfileGuard(tmpfile))))
    } else {
        Ok((resolved_command.to_string(), None))
    }
}

async fn handle_successful_output(
    original_command: &str,
    raw_stdout: String,
    raw_stderr: String,
    exit_code: i32,
    buffer_only: bool,
    unfiltered_tmpfile: Option<TmpfileGuard>,
    ctx: &ToolContext,
) -> anyhow::Result<Value> {
    use super::command_summary::{
        count_lines, detect_command_type, needs_summary, strip_ansi_codes, summarize_build_output,
        summarize_generic, summarize_test_output, truncate_lines, truncate_lines_and_bytes,
        CommandType, BUFFER_QUERY_INLINE_CAP,
    };

    // Buffer-only queries strip ANSI codes — they are opaque to LLMs and bloat byte counts.
    let raw_stdout = if buffer_only {
        strip_ansi_codes(&raw_stdout)
    } else {
        raw_stdout
    };
    let raw_stderr = if buffer_only {
        strip_ansi_codes(&raw_stderr)
    } else {
        raw_stderr
    };

    // --- Step 6.5: Read tee capture and store as unfiltered_output ref ---
    let unfiltered_ref: Option<(String, bool)> = if let Some(ref tmpfile) = unfiltered_tmpfile {
        let capture = std::fs::read_to_string(&tmpfile.0).ok();
        // tmpfile drops at function exit — TmpfileGuard::drop() removes the file.
        // Skip empty captures: when the terminal filter matched nothing, both
        // raw_stdout and the tee file are empty — surfacing a handle is misleading.
        capture.and_then(|content| {
            if content.is_empty() {
                return None;
            }
            let (stored, truncated) = if crate::tools::exceeds_inline_limit(&content) {
                let mut byte_budget = crate::tools::MAX_INLINE_TOKENS * 4;
                let capped: String = content
                    .lines()
                    .take_while(|line| {
                        if byte_budget == 0 {
                            return false;
                        }
                        byte_budget = byte_budget.saturating_sub(line.len() + 1);
                        true
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                (capped, true)
            } else {
                (content, false)
            };
            let ref_id = ctx.output_buffer.store(
                original_command.to_string(),
                stored,
                String::new(), // unfiltered capture is stdout-only
                exit_code,
            );
            Some((ref_id, truncated))
        })
    } else {
        None
    };

    // --- Step 6: Decide whether to buffer + summarize ---
    let mut result = if needs_summary(&raw_stdout, &raw_stderr) {
        if buffer_only {
            // Buffer-only: return inline, never create a new buffer ref (avoids infinite loop).
            const STDERR_BUDGET: usize = 20;
            let buffer_stderr: String = if raw_stderr.is_empty() {
                original_command
                    .find("@cmd_")
                    .or_else(|| original_command.find("@file_"))
                    .and_then(|pos| {
                        original_command[pos..]
                            .split_whitespace()
                            .next()
                            .and_then(|tok| ctx.output_buffer.get(tok))
                    })
                    .map(|e| e.stderr)
                    .unwrap_or_default()
            } else {
                raw_stderr.clone()
            };
            let stderr_budget = STDERR_BUDGET.min(count_lines(&buffer_stderr));
            let stdout_budget = BUFFER_QUERY_INLINE_CAP - stderr_budget;

            let (stderr_out, stderr_shown, stderr_total) =
                truncate_lines(&buffer_stderr, STDERR_BUDGET);

            // Byte budget: keep final JSON under TOOL_OUTPUT_BUFFER_THRESHOLD to avoid re-buffering loop.
            const JSON_OVERHEAD: usize = 300;
            let stdout_byte_budget = crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD
                .saturating_sub(JSON_OVERHEAD)
                .saturating_sub(stderr_out.len());

            let (stdout_out, stdout_shown, stdout_total) =
                truncate_lines_and_bytes(&raw_stdout, stdout_budget, stdout_byte_budget);

            let was_truncated = stdout_shown < stdout_total || stderr_shown < stderr_total;

            let mut result = json!({"exit_code": exit_code});
            if !stdout_out.is_empty() {
                result["stdout"] = json!(stdout_out);
            }
            if !stderr_out.is_empty() {
                result["stderr"] = json!(stderr_out);
            }
            if was_truncated {
                result["truncated"] = json!(true);
                result["stdout_shown"] = json!(stdout_shown);
                result["stdout_total"] = json!(stdout_total);
                if stderr_total > 0 {
                    result["stderr_shown"] = json!(stderr_shown);
                    result["stderr_total"] = json!(stderr_total);
                }
                let stderr_note = if stderr_total > 0 {
                    format!(", stderr {stderr_shown}/{stderr_total}")
                } else {
                    String::new()
                };
                let next_start = stdout_shown + 1;
                let next_end = stdout_shown + BUFFER_QUERY_INLINE_CAP;
                result["hint"] = json!(format!(
                    "Output capped at {BUFFER_QUERY_INLINE_CAP} lines \
                     (stdout {stdout_shown}/{stdout_total}{stderr_note}). \
                     Next page: sed -n '{next_start},{next_end}p' @ref. \
                     Or grep 'keyword' @ref for targeted search.",
                ));
            }
            // buffer_only => tee injection was skipped (unfiltered_tmpfile is None).
            return Ok(result);
        }

        let output_id = ctx.output_buffer.store(
            original_command.to_string(),
            raw_stdout.clone(),
            raw_stderr.clone(),
            exit_code,
        );

        let cmd_type = detect_command_type(original_command);
        let cmd_summary = match cmd_type {
            CommandType::Test => summarize_test_output(&raw_stdout, &raw_stderr, exit_code),
            CommandType::Build => summarize_build_output(&raw_stdout, &raw_stderr, exit_code),
            CommandType::Generic => summarize_generic(&raw_stdout, &raw_stderr, exit_code),
        };

        // Rebuild with correct field order so output_id appears before content fields.
        rebuild_buffered_summary(cmd_summary, &output_id)
    } else {
        // Short output — apply byte budget for buffer-only to prevent re-buffering loop.
        if buffer_only
            && raw_stdout.len() + raw_stderr.len()
                > crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD.saturating_sub(300)
        {
            const JSON_OVERHEAD: usize = 300;
            let byte_budget = crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD
                .saturating_sub(JSON_OVERHEAD)
                .saturating_sub(raw_stderr.len());
            let (stdout_out, stdout_shown, stdout_total) =
                truncate_lines_and_bytes(&raw_stdout, BUFFER_QUERY_INLINE_CAP, byte_budget);
            let mut r = json!({"exit_code": exit_code});
            if !stdout_out.is_empty() {
                r["stdout"] = json!(stdout_out);
            }
            if !raw_stderr.is_empty() {
                r["stderr"] = json!(raw_stderr);
            }
            if stdout_shown < stdout_total {
                r["truncated"] = json!(true);
                r["hint"] = json!(
                    "Match truncated: a single grep match inside a @tool_* ref \
                     contains compact JSON (one very long line). \
                     Use read_file(@tool_abc, json_path=\"$.field\") to extract \
                     a specific field, or read_file(@tool_abc, start_line=N, \
                     end_line=M) to browse sections of the pretty-printed result."
                );
            }
            r
        } else {
            let mut r = json!({"exit_code": exit_code});
            if !raw_stdout.is_empty() {
                r["stdout"] = json!(raw_stdout);
            }
            if !raw_stderr.is_empty() {
                r["stderr"] = json!(raw_stderr);
            }
            r
        }
    };

    // Attach unfiltered_output ref if we captured via tee.
    if let Some((ref ref_id, truncated)) = unfiltered_ref {
        result["unfiltered_output"] = json!(ref_id);
        if truncated {
            result["unfiltered_truncated"] = json!(true);
        }
    }

    Ok(result)
}

/// always happens in the caller regardless of early returns.
#[allow(clippy::too_many_arguments)]
async fn run_command_inner(
    original_command: &str,
    resolved_command: &str,
    timeout_secs: u64,
    acknowledge_risk: bool,
    cwd_param: Option<&str>,
    buffer_only: bool,
    run_in_background: bool,
    root: &Path,
    security: &crate::util::path_security::PathSecurityConfig,
    ctx: &ToolContext,
) -> anyhow::Result<Value> {
    use crate::util::path_security::is_dangerous_command;

    // --- Step 2: Dangerous command gate ---
    // Order: (a) acknowledge_risk bypass → (b) pending_ack two-round-trip fallback.
    if !buffer_only && !acknowledge_risk {
        // Use resolved_command (with @refs substituted) so buffer-only grep/awk
        // commands don't get flagged for patterns in the buffer content.
        if let Some(reason) = is_dangerous_command(resolved_command, security) {
            let handle = ctx.output_buffer.store_dangerous(
                resolved_command.to_string(),
                cwd_param.map(str::to_string),
                timeout_secs,
            );
            return Ok(serde_json::json!({
                "pending_ack": handle,
                "reason": reason,
                "hint": format!("run_command(\"{handle}\") to execute")
            }));
        }
    }

    // --- Step 2.5: Source file access block ---
    if !buffer_only && !acknowledge_risk {
        if let Some(hint) = crate::util::path_security::check_source_file_access(resolved_command) {
            return Err(super::RecoverableError::with_hint(
                "shell access to source files is blocked",
                &hint,
            )
            .into());
        }
    }

    // --- Step 3: Shell command mode check (skip for buffer-only queries) ---
    if !buffer_only {
        match security.shell_command_mode.as_str() {
            "disabled" => {
                return Err(super::RecoverableError::with_hint(
                    "shell commands are disabled",
                    "Set security.shell_command_mode = \"warn\" or \"unrestricted\" in .codescout/project.toml",
                ).into());
            }
            "unrestricted" | "warn" | "" => {} // allowed
            other => {
                return Err(super::RecoverableError::with_hint(
                    format!("unknown shell_command_mode: '{}'", other),
                    "Use \"warn\", \"unrestricted\", or \"disabled\".",
                )
                .into());
            }
        }
    }

    // --- Step 4: Resolve working directory ---
    let work_dir = resolve_work_dir(root, cwd_param)?;

    // --- Step 4.7: Background spawn with warm return ---
    if run_in_background {
        if buffer_only {
            return Err(super::RecoverableError::with_hint(
                "run_in_background cannot be used with buffer queries",
                "Remove run_in_background, or run the query as a plain command without @ref interpolation.",
            )
            .into());
        }
        return spawn_background_command(resolved_command, &work_dir, ctx).await;
    }

    // --- Step 4.5: Tee injection for terminal filter commands ---
    // When the last pipe stage is a known filter (grep, head, tail, sed, awk, etc.),
    // inject `tee /tmp/codescout-unfiltered-XXXX` before the filter so the caller
    // can surface the unfiltered stream as a buffer ref without re-running the command.
    let (effective_command, unfiltered_tmpfile) = inject_tee(resolved_command, buffer_only)?;

    // --- Step 5: Execute command ---
    // On Unix we spawn into a new process group (process_group(0) → PGID = child PID)
    // so killpg() can reap the entire tree on timeout.  Without this, dropping the tokio
    // future orphans curl/grep/tee/head and they keep running until the download finishes.
    //
    // `kill_on_drop(true)` is the cancellation lifeline: when the rmcp request is
    // cancelled (user pressed Escape), call_tool_inner drops the tool future, which
    // drops `child_output_fut`, which drops the `Child` — and tokio then SIGKILLs the
    // immediate child.  We *also* keep the timeout-path killpg() below for the case
    // where the future isn't dropped: SIGKILL on the lone shell wouldn't propagate to
    // the pipeline (curl, grep, tee, etc.), but killpg() reaps the whole group.
    //
    // We also reset SIGPIPE to SIG_DFL in pre_exec.  Claude Code's Node.js parent sets
    // SIGPIPE=SIG_IGN; every spawned process inherits it.  With SIG_IGN, a `| head -N`
    // pipeline never terminates via SIGPIPE: tee ignores the broken pipe from head and
    // keeps draining curl's output into the tmpfile until the download completes.
    #[cfg(unix)]
    let (child_output_fut, child_pgid) = {
        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c")
            .arg(&effective_command)
            .current_dir(&work_dir)
            .env("GIT_PAGER", "cat")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .process_group(0) // new process group; PGID = child PID
            .kill_on_drop(true); // SIGKILL on Drop — reaps shell on cancel
                                 // SAFETY: pre_exec runs in the child after fork(), before exec().
                                 // signal() is async-signal-safe (POSIX).  No locks are held at this point.
        unsafe {
            cmd.pre_exec(|| {
                libc::signal(libc::SIGPIPE, libc::SIG_DFL);
                Ok(())
            });
        }
        let child = cmd.spawn()?;
        let pgid: Option<i32> = child.id().map(|id| id as i32);
        // Drop guard: if the future is cancelled, we want the *entire pipeline*
        // killed — not just the shell. tokio's kill_on_drop only SIGKILLs the
        // immediate child; killpg() walks the whole process group. We attach
        // the guard to the future so its Drop runs on cancellation.
        let pgid_for_guard = pgid;
        let fut: std::pin::Pin<
            Box<dyn std::future::Future<Output = std::io::Result<std::process::Output>> + Send>,
        > = Box::pin(async move {
            struct PgidKillGuard(Option<i32>);
            impl Drop for PgidKillGuard {
                fn drop(&mut self) {
                    if let Some(pgid) = self.0 {
                        // SAFETY: pgid was created with process_group(0); SIGKILL is
                        // safe to send to our own group. No-op if already reaped.
                        unsafe { libc::killpg(pgid, libc::SIGKILL) };
                    }
                }
            }
            let mut guard = PgidKillGuard(pgid_for_guard);
            let result = child.wait_with_output().await;
            // Successful completion: disarm the guard by clearing the pgid so
            // the Drop impl sees None and skips the SIGKILL.
            guard.0 = None;
            result
        });
        (fut, pgid)
    };

    #[cfg(windows)]
    let (child_output_fut, child_pgid) = {
        let fut: std::pin::Pin<
            Box<dyn std::future::Future<Output = std::io::Result<std::process::Output>> + Send>,
        > = Box::pin(
            tokio::process::Command::new("cmd")
                .arg("/C")
                .arg(&effective_command)
                .current_dir(&work_dir)
                .env("GIT_PAGER", "cat")
                .kill_on_drop(true)
                .output(),
        );
        (fut, None::<i32>)
    };

    // Heartbeat: send elapsed-seconds progress every 3s while the command runs.
    // AbortOnDrop guarantees the task is cancelled even when early `return`s fire.
    let progress_clone = ctx.progress.clone();
    let _heartbeat = AbortOnDrop(tokio::spawn(async move {
        let start = std::time::Instant::now();
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            if let Some(p) = &progress_clone {
                let elapsed = start.elapsed().as_secs();
                p.report_text(&format!("{}s elapsed", elapsed)).await;
            }
        }
    }));

    match tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        child_output_fut,
    )
    .await
    {
        Ok(Ok(output)) => {
            handle_successful_output(
                original_command,
                String::from_utf8_lossy(&output.stdout).into_owned(),
                String::from_utf8_lossy(&output.stderr).into_owned(),
                output.status.code().unwrap_or(-1),
                buffer_only,
                unfiltered_tmpfile,
                ctx,
            )
            .await
        }
        Ok(Err(e)) => {
            Err(super::RecoverableError::new(format!("command execution error: {}", e)).into())
        }
        Err(_) => {
            // Kill the entire process group so orphaned children (curl, grep, tee, etc.)
            // are reaped immediately rather than running to completion in the background.
            #[cfg(unix)]
            if let Some(pgid) = child_pgid {
                // SAFETY: pgid is the process group we created with process_group(0) above.
                // killpg with SIGKILL is the only reliable way to stop the whole pipeline
                // tree (sh + curl + grep + tee + head) in one shot.
                unsafe { libc::killpg(pgid, libc::SIGKILL) };
            }
            Ok(json!({
                "timed_out": true,
                "stderr": format!("Command timed out after {} seconds", timeout_secs),
                "exit_code": null,
                "hint": "If the command launches background processes (e.g. with &), use run_in_background: true — shell & leaves background processes holding the stdout pipe open, so output() never gets EOF. run_in_background spawns via a log file instead and returns immediately."
            }))
        }
    }
}

#[allow(dead_code)] // Kept as safety net for byte-level shell_output_limit_bytes config.
fn truncate_output(output: &str, limit: usize) -> (String, bool) {
    if output.len() > limit {
        let safe_end = crate::tools::floor_char_boundary(output, limit);
        (
            format!(
                "{}\n... (truncated, showing first {} of {} bytes)",
                &output[..safe_end],
                safe_end,
                output.len()
            ),
            true,
        )
    } else {
        (output.to_string(), false)
    }
}

#[cfg(test)]
mod tests;
