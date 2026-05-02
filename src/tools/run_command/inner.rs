//! Core command execution logic for run_command.

use std::path::Path;

use serde_json::{json, Value};

use super::super::{RecoverableError, ToolContext};
use super::output::handle_successful_output;

/// RAII guard: deletes a named temp file when dropped.
pub(crate) struct TmpfileGuard(pub(crate) String);

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

fn resolve_work_dir(root: &Path, cwd_param: Option<&str>) -> anyhow::Result<std::path::PathBuf> {
    if let Some(rel) = cwd_param {
        let candidate = root.join(rel);
        let canonical = candidate.canonicalize().map_err(|e| {
            RecoverableError::with_hint(
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
            return Err(RecoverableError::with_hint(
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

fn inject_tee(
    resolved_command: &str,
    buffer_only: bool,
) -> anyhow::Result<(String, Option<TmpfileGuard>)> {
    use super::super::command_summary::detect_terminal_filter;
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
            return Err(RecoverableError::new(format!(
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

#[allow(dead_code)] // Kept as safety net for byte-level shell_output_limit_bytes config.
pub(crate) fn truncate_output(output: &str, limit: usize) -> (String, bool) {
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

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_command_inner(
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
            return Err(RecoverableError::with_hint(
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
                return Err(RecoverableError::with_hint(
                    "shell commands are disabled",
                    "Set security.shell_command_mode = \"warn\" or \"unrestricted\" in .codescout/project.toml",
                ).into());
            }
            "unrestricted" | "warn" | "" => {} // allowed
            other => {
                return Err(RecoverableError::with_hint(
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
            return Err(RecoverableError::with_hint(
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
        Ok(Err(e)) => Err(RecoverableError::new(format!("command execution error: {}", e)).into()),
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

/// Returns true when `command` is a bare `@ack_<8hex>` handle.
pub(crate) fn looks_like_ack_handle(command: &str) -> bool {
    let s = command.trim();
    if !s.starts_with("@ack_") {
        return false;
    }
    let suffix = &s[5..]; // after "@ack_"
    suffix.len() == 8 && suffix.chars().all(|c| c.is_ascii_hexdigit())
}
