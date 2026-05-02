//! Interactive mode for run_command — elicitation-driven stdin/stdout loop.

use std::path::Path;

use serde_json::{json, Value};

use super::super::{RecoverableError, ToolContext};

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
pub(crate) async fn run_command_interactive(
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
        return Err(RecoverableError::with_hint(
            "interactive mode requires elicitation support",
            "The MCP client does not support elicitation. Use run_command without interactive: true.",
        )
        .into());
    }

    // Dangerous command check — block in interactive mode to keep the spike focused.
    if let Some(reason) = crate::util::path_security::is_dangerous_command(command, security) {
        return Err(RecoverableError::with_hint(
            format!("interactive mode blocked dangerous command: {reason}"),
            "Remove the dangerous pattern or use the non-interactive path with acknowledge_risk: true.",
        )
        .into());
    }

    // Resolve working directory.
    let work_dir = if let Some(rel) = cwd_param {
        let candidate = root.join(rel);
        candidate.canonicalize().map_err(|e| {
            RecoverableError::with_hint(
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
