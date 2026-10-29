//! Interactive mode for run_command — elicitation-driven stdin/stdout loop.

use std::path::Path;

use serde_json::{json, Value};
use tokio::io::{AsyncRead, AsyncReadExt};

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
    use tokio::io::AsyncWriteExt;

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

    // Shell preflight — same contract as the non-interactive path: after the
    // policy gates, before the spawn. See WIN-36.
    if let Some(hint) = crate::platform::shell_unavailable_hint() {
        return Err(
            RecoverableError::with_hint("no POSIX shell available to run commands", hint).into(),
        );
    }

    // Spawn with piped stdin/stdout/stderr via the hardened shell builder.
    // Interactive overrides the builder's default stdin (NUL on Windows) with a
    // real pipe, since it drives the child's stdin.
    let mut cmd = crate::platform::shell_command_configured(command);
    crate::tools::invocation_context::apply_to_shell(&mut cmd)?;
    let mut child = cmd
        .current_dir(&work_dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().expect("stdin piped");
    let mut stdout_reader = tokio::io::BufReader::new(child.stdout.take().expect("stdout piped"));
    let mut stderr_reader = tokio::io::BufReader::new(child.stderr.take().expect("stderr piped"));

    let mut accumulated_output = String::new();

    #[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
    struct InteractiveInput {
        /// Text to send to the process stdin (leave empty to cancel and kill the process)
        input: String,
    }
    rmcp::elicit_safe!(InteractiveInput);

    // Interaction loop.
    let mut round = 0u32;
    // cap-class: NOT_A_CAP — runaway-loop guard on the interactive session; it kills the process and appends an explicit max-rounds note
    const MAX_ROUNDS: u32 = 50; // guard against runaway loops
    loop {
        if round >= MAX_ROUNDS {
            let _ = child.kill().await;
            accumulated_output.push_str("\n[interactive: max rounds reached, process killed]");
            break;
        }
        round += 1;

        // Read post-spawn / post-input output with 150 ms settle.
        let (chunk, both_eof) =
            drain_with_settle(&mut stdout_reader, &mut stderr_reader, 150).await;
        if !chunk.is_empty() {
            accumulated_output.push_str(&chunk);
        }

        // Check whether the process already exited (see `child_exit` for why one `try_wait`
        // is not enough once both pipes have closed).
        match child_exit(&mut child, both_eof, EXIT_WAIT_BOUND).await {
            Ok(Some(status)) => {
                let code = status.code().unwrap_or(-1);
                // Drain any remaining output after exit.
                let (tail, _) = drain_with_settle(&mut stdout_reader, &mut stderr_reader, 50).await;
                if !tail.is_empty() {
                    accumulated_output.push_str(&tail);
                }
                let mut response = interactive_response(code, &accumulated_output, round, None);
                if let Some(signal) = super::output::terminating_signal(&status) {
                    response["terminated_by_signal"] = json!(signal);
                }
                return Ok(response);
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
    let (tail, _) = drain_with_settle(&mut stdout_reader, &mut stderr_reader, 50).await;
    if !tail.is_empty() {
        accumulated_output.push_str(&tail);
    }

    Ok(interactive_response(
        -1,
        &accumulated_output,
        round,
        Some("process killed or loop exited before natural termination"),
    ))
}

/// Drain what stdout and stderr hold until both are quiet for `settle_ms`, or both are at EOF.
///
/// Returns the text and whether BOTH streams reached EOF. A stream at EOF is retired from the
/// race, not treated as the end of the drain: ending the drain there dropped the other stream's
/// unread bytes, because `tokio::select!` picks among ready arms at random (a short command's
/// pipes close together, so such a command could come back with `stdout: ""`).
///
/// Two independent buffers, one per reader, avoid the double-borrow of one `&mut buf`.
pub(super) async fn drain_with_settle<O, E>(
    stdout_reader: &mut tokio::io::BufReader<O>,
    stderr_reader: &mut tokio::io::BufReader<E>,
    settle_ms: u64,
) -> (String, bool)
where
    O: AsyncRead + Unpin,
    E: AsyncRead + Unpin,
{
    let settle = std::time::Duration::from_millis(settle_ms);
    let mut output = String::new();
    let mut out_buf = [0u8; 4096];
    let mut err_buf = [0u8; 4096];
    let mut out_eof = false;
    let mut err_eof = false;

    while !(out_eof && err_eof) {
        tokio::select! {
            result = tokio::time::timeout(settle, stdout_reader.read(&mut out_buf)), if !out_eof => {
                match result {
                    Ok(Ok(n)) if n > 0 => {
                        output.push_str(&String::from_utf8_lossy(&out_buf[..n]));
                    }
                    Ok(_) => out_eof = true, // EOF or read error
                    Err(_) => break,         // settle window elapsed with the pipe quiet
                }
            }
            result = tokio::time::timeout(settle, stderr_reader.read(&mut err_buf)), if !err_eof => {
                match result {
                    Ok(Ok(n)) if n > 0 => {
                        output.push_str(&String::from_utf8_lossy(&err_buf[..n]));
                    }
                    Ok(_) => err_eof = true, // EOF or read error
                    Err(_) => break,         // settle window elapsed with the pipe quiet
                }
            }
        }
    }
    (output, out_eof && err_eof)
}

/// How long `child_exit` waits for a child whose pipes have both closed.
// cap-class: NOT_A_CAP — a wait bound on a child's exit; on expiry the child is treated as still running, nothing is truncated
const EXIT_WAIT_BOUND: std::time::Duration = std::time::Duration::from_secs(2);

/// Whether the child has exited.
///
/// A process closes its file descriptors while it exits, BEFORE it can be waited on. So once
/// both pipes are at EOF (`both_eof`) a single `try_wait` can still answer "running" for a child
/// that is about to be reaped, and the caller then asks the user for input on a finished process.
/// In that case this waits for the child, up to `bound`; a child that closed both pipes but lives
/// on is reported as running once `bound` passes. Without `both_eof` it is one `try_wait`.
pub(super) async fn child_exit(
    child: &mut tokio::process::Child,
    both_eof: bool,
    bound: std::time::Duration,
) -> std::io::Result<Option<std::process::ExitStatus>> {
    if both_eof {
        match tokio::time::timeout(bound, child.wait()).await {
            Ok(waited) => waited.map(Some),
            Err(_) => Ok(None),
        }
    } else {
        child.try_wait()
    }
}

/// The response for an interactive run. **The accumulated output is scrubbed here, once, on the whole
/// text** and never per chunk: `drain_with_settle` decodes whatever each read returned, so a
/// credential that straddles two reads is in neither chunk whole. (Bug bc0cb248b224d1dd.) The
/// elicitation prompt in the loop shows the process output to the operator, a human, and is left as is.
pub(super) fn interactive_response(
    exit_code: i32,
    output: &str,
    rounds: u32,
    note: Option<&str>,
) -> serde_json::Value {
    let scrubbed = crate::util::redact::redact_credentials(output);
    let mut result = json!({
        "exit_code": exit_code,
        "stdout": scrubbed.text,
        "interactive_rounds": rounds,
    });
    if let Some(note) = note {
        result["note"] = json!(note);
    }
    crate::util::redact::note_in(&mut result, scrubbed.count);
    result
}
