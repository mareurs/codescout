//! Core command execution logic for run_command.

use std::path::Path;

use serde_json::{json, Value};

use super::super::{RecoverableError, ToolContext};
use super::output::{handle_successful_output_with, LateKeys};
use crate::tools::output_buffer::{BackgroundJob, JobState};
use crate::util::redact::redact_credentials;

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

/// RAII guard: SIGKILLs a whole process group when dropped, unless disarmed by setting it to
/// `None` once the child was waited on to completion. `kill_on_drop` kills only the shell; this is
/// what reaps its pipeline when a future holding the wait is dropped (cancellation, a timeout).
#[cfg(unix)]
struct PgidKillGuard(Option<i32>);

#[cfg(unix)]
impl Drop for PgidKillGuard {
    fn drop(&mut self) {
        if let Some(pgid) = self.0 {
            // SAFETY: pgid was created with process_group(0); SIGKILL is
            // safe to send to our own group. No-op if already reaped.
            unsafe { libc::killpg(pgid, libc::SIGKILL) };
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

/// Spawn a detached background command and return immediately.
///
/// **The `Child` is handed to a supervisor task rather than dropped, and that is
/// the whole point.** Dropping it (as this function did until 2026-09-15) gives
/// the process to tokio's orphan reaper and discards its exit status with it —
/// there is no other channel that status exists on, which is why "just record
/// the exit code" was never a smaller change than this one.
///
/// **No warm-up window.** This used to sleep 5s to scrape an early log tail,
/// which bought a preview at the cost of emitting `Process running.` *after* a
/// fast command had already exited — a claim about current state made at the one
/// moment it could be false. Returning immediately makes the running state true
/// by construction at emit time; the outcome arrives later, through the envelope
/// of any command that names the handle (`OutputBuffer::job_states_in`).
///
/// docs/issues/archive/2026-09-13-background-command-loses-terminal-status.md
/// docs/trackers/architecture-boundary-measurement.md — slice 1
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

    let mut cmd = crate::platform::shell_command_configured(resolved_command);
    crate::tools::invocation_context::apply_to_shell(&mut cmd)?;
    let mut child = cmd
        .current_dir(work_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::from(log_file))
        .stderr(std::process::Stdio::from(log_stderr))
        .spawn()?;

    let ref_id = ctx.output_buffer.store_background(BackgroundJob {
        log_path,
        command: resolved_command.to_string(),
        state: JobState::Running,
    });

    // The supervisor outlives this call deliberately: it is not tied to the tool
    // future, so cancelling the call does not discard the outcome. It holds an
    // `Arc<OutputBuffer>` clone, which is what makes the write-after-return legal.
    let buffer = ctx.output_buffer.clone();
    let supervised_id = ref_id.clone();
    tokio::spawn(async move {
        let state = match child.wait().await {
            Ok(status) => JobState::Exited {
                code: status.code(),
            },
            Err(e) => JobState::Failed {
                error: e.to_string(),
            },
        };
        buffer.set_job_state(&supervised_id, state);
    });

    // No `exit_code` key: the job has just started and no status exists to
    // report. `format_run_command`'s three-state match reads that absence as
    // "running", which is correct here precisely because this response is
    // emitted at spawn time rather than five seconds later.
    Ok(serde_json::json!({
        "output_id": ref_id,
        "hint": format!(
            "Started; outcome not yet observed. Read output with run_command(\"tail -50 {}\") \
             — that response carries this job's state.",
            ref_id
        )
    }))
}

/// SF-4: is `path` safe to interpolate into a shell command?
///
/// The tee target is always generated by `tempfile` under the system temp dir,
/// never user input, so this is a tripwire against that assumption breaking —
/// not the primary defence (the path is also single-quoted at the call site).
///
/// Permitted beyond alphanumerics: `/` `-` `_` `.` plus two Windows-specific
/// characters that a `%TEMP%` path routinely contains and whose absence made
/// this check reject *every* Windows path:
/// - `:` — the drive-letter prefix (`C:/...`).
/// - `~` — 8.3 short names (`C:/Users/MAILIN~1.002/...`), which Windows uses
///   whenever the long name is over 8 chars or contains a dot. Bash performs
///   tilde expansion only at the *start* of a word; this path always begins
///   with a drive letter, so an interior `~` is literal.
///
/// Pure + cross-platform so its tests run on every CI target.
pub(super) fn tee_path_is_safe(path: &str) -> bool {
    !path.is_empty()
        && path.chars().all(|c| {
            c.is_alphanumeric()
                || c == '/'
                || c == '-'
                || c == '_'
                || c == '.'
                || c == ':'
                || c == '~'
        })
}

/// What tee injection decided for one command.
pub(super) struct TeeInjection {
    /// The command to run: spliced with a `tee` capture, or the original untouched.
    pub(super) command: String,
    /// Deletes the capture file when dropped. `None` when nothing is being captured.
    pub(super) capture: Option<TmpfileGuard>,
    /// Why a capture that WOULD have been taken was skipped, for the caller to surface.
    /// `None` both when there was nothing to capture (no terminal filter) and when the
    /// capture is in place — only a capture that was wanted and could not be made sets it.
    pub(super) skipped: Option<String>,
    /// Everything after the spliced `|` (`B | C` in `A | tee cap | B | C`; just `B` for a single
    /// pipe), `None` when nothing is captured. `tee_splice_point` picks the pipe: the first one at
    /// nesting depth 0, so the capture is the producer's own stream, and only when the text after
    /// it is ONE pipeline whose last stage is a terminal filter. So this is the rest of that
    /// pipeline and nothing else: no `;` / `&&` tail and no heredoc body, and no fallback to
    /// another pipe when those fail (the command then runs untouched and this is `None`).
    /// Only the LAST stage is known to be a terminal filter, and the text before the pipe can set
    /// context the filter ran in, so being capturable does NOT make it safe to run again: the
    /// timeout path re-runs it over the partial capture only when
    /// `command_summary::filter_is_replayable(prefix, filter)` says so.
    pub(super) filter: Option<String>,
    /// The text before the spliced `|` (`A`), `Some` exactly when `filter` is: what
    /// `filter_is_replayable` judges the producer side by.
    pub(super) prefix: Option<String>,
}

#[cfg(test)]
thread_local! {
    /// Test-only override of the directory the tee capture is created in. Per-THREAD, not
    /// process env: `set_var` is process-global and races every other test in the binary
    /// (`docs/conventions/test-env-isolation.md`). Sound only because `#[tokio::test]`
    /// runs on one thread and `inject_tee` executes inline on it, never on a spawned task.
    pub(super) static TEE_DIR_OVERRIDE: std::cell::RefCell<Option<std::path::PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

fn tee_dir() -> std::path::PathBuf {
    #[cfg(test)]
    {
        if let Some(dir) = TEE_DIR_OVERRIDE.with(|d| d.borrow().clone()) {
            return dir;
        }
    }
    std::env::temp_dir()
}

fn inject_tee(resolved_command: &str, buffer_only: bool) -> anyhow::Result<TeeInjection> {
    inject_tee_in(resolved_command, buffer_only, &tee_dir())
}

/// `inject_tee` with the capture directory as an argument, so a test can hand it one that
/// cannot be written to without touching process env.
///
/// **The capture is a best-effort side channel, and a failure to create it must not become
/// a refused command.** It exists only to offer the unfiltered stream as a bonus
/// `@cmd_*` buffer; the command it instruments does not need it. Creating the file used to
/// propagate its error with `?`, so a full `/tmp` refused every filter-terminated command
/// — on a machine with native `Bash` denied, that is no working shell at the moment one is
/// needed to diagnose the full disk. Now the command runs un-teed and `skipped` says why.
/// The `tee_path_is_safe` tripwire below is a different kind of failure — a path the shell
/// must not be handed — and stays a refusal.
/// docs/issues/archive/2026-09-24-run-command-refuses-a-filtered-command-when-tmp-is-full.md
pub(super) fn inject_tee_in(
    resolved_command: &str,
    buffer_only: bool,
    tmp_dir: &std::path::Path,
) -> anyhow::Result<TeeInjection> {
    use super::super::command_summary::tee_splice_point;
    let untouched = |skipped: Option<String>| TeeInjection {
        command: resolved_command.to_string(),
        capture: None,
        skipped,
        filter: None,
        prefix: None,
    };
    if buffer_only {
        return Ok(untouched(None));
    }
    // `tee_splice_point` scans a heredoc-masked copy: a `|` inside a heredoc BODY is data
    // destined for a file, not pipeline syntax, and the splice used to land in written content. See
    // docs/issues/archive/2026-08-19-run-command-rewrites-pipes-inside-heredoc-content.md.
    let Some(pipe_pos) = tee_splice_point(resolved_command) else {
        return Ok(untouched(None));
    };
    let skip = |why: &dyn std::fmt::Display| {
        untouched(Some(format!(
            "the unfiltered-output capture was skipped: its temp file could not be created in \
             {} ({why}). The command ran normally; only the `unfiltered_output` buffer is missing.",
            tmp_dir.display(),
        )))
    };
    // Use tempfile::NamedTempFile for unpredictable path (SF-3).
    // persist() converts it to a regular file we manage via TmpfileGuard.
    let named = match tempfile::Builder::new()
        .prefix("codescout-unfiltered-")
        .tempfile_in(tmp_dir)
    {
        Ok(named) => named,
        Err(e) => return Ok(skip(&e)),
    };
    let tmppath = named.into_temp_path();
    // Forward-slash form — this string is interpolated into a shell command
    // run by Git Bash on Windows, where `\` is an escape character. Rust's
    // fs APIs accept the forward-slash form too, so TmpfileGuard cleanup is
    // unaffected.
    let tmpfile = crate::platform::shell_path_str(&tmppath);
    // Keep the file on disk — TmpfileGuard handles cleanup.
    if let Err(e) = tmppath.keep() {
        return Ok(skip(&e));
    }
    // Safety (SF-4): see tee_path_is_safe. The path is also single-quoted
    // below, so this is a tripwire rather than the only defence.
    if !tee_path_is_safe(&tmpfile) {
        return Err(RecoverableError::new(format!(
            "temporary file path contains unexpected characters: {}",
            tmpfile,
        ))
        .into());
    }
    // Single-quote the path: commands run under a POSIX shell on both
    // platforms now, and quoting neutralises anything the allowlist might
    // not anticipate (a future temp-dir naming scheme, a space in the path).
    // `'` itself is excluded by the allowlist, so the quoting cannot be
    // escaped from.
    // The tee goes at the splice point (`tee_splice_point`: the first pipe at nesting depth 0, only
    // when the rest is one pipeline ending in a terminal filter and the shell parses both the
    // command and the spliced text), so in a chain
    // (`cargo test | grep x | head -20`) the capture is the producer's own stream and the
    // whole chain runs inside the group. A command that does not qualify never reaches here:
    // it runs untouched, with no capture and no fallback to another pipe.
    // The filter side is wrapped in a group that, once the filter exits, drains up to the
    // capture store limit of what it left unread and then exits with the FILTER's status.
    // Without the drain an early-exit filter (`head -N`) closes the pipe, `tee` takes SIGPIPE,
    // and the capture is cut to what arrived before that — which the response then reported
    // as the whole stream. The drain is bounded because the handle keeps no more than that
    // limit anyway. A FAST endless producer (`yes | head -1`) still ends at once: the group
    // exits after the bound, the pipe closes and the producer stops. A SLOW one (fewer bytes
    // than the bound before `timeout_secs`) never reaches the bound, so it runs to the
    // timeout, where the result is flagged `capture_incomplete`: the filter run once over the
    // partial capture when `filter_is_replayable` allows it, else the capture's handle with
    // `filter_not_run`. Pinned by `a_slow_endless_producer_into_head_runs_to_the_timeout`.
    // POSIX only: `sh -c` may be dash. The shape (`splice_tee`) puts the newline before `s=$?` (not
    // `;`) so a trailing `# comment` in the filter cannot swallow the closing brace. The group
    // sits on the filter's side of the pipe, so the producer's stderr is not piped into it.
    let filter = resolved_command[pipe_pos + 1..].trim_start().to_string();
    let prefix = resolved_command[..pipe_pos].to_string();
    let command = super::super::command_summary::splice_tee(
        resolved_command,
        pipe_pos,
        &tmpfile,
        super::output::CAPTURE_STORE_LIMIT_BYTES,
    );
    Ok(TeeInjection {
        command,
        capture: Some(TmpfileGuard(tmpfile)),
        skipped: None,
        filter: Some(filter),
        prefix: Some(prefix),
    })
}

/// Seconds the filter of a timed-out command gets to run once over the partial capture.
pub(super) const TIMEOUT_FILTER_GRACE_SECS: u64 = 5;

/// Run `filter` once through the platform shell with the capture file at `capture_path` as its
/// stdin. The timeout path calls this after it killed the producer, and only for a filter
/// `command_summary::filter_is_replayable` passed, so a timed-out filtered command returns what its
/// filter makes of the partial capture.
///
/// Every outcome is distinct ([`FilterRun`]): the filter's stdout, stderr and exit code when it
/// ended within `grace`; `GraceExpired` when it did not; `NotRun` with the reason when it could not
/// be started. The filter's whole process group is killed whenever the wait does not complete:
/// on expiry, and when the future is dropped during the grace (the call was cancelled). That is
/// the foreground spawn's [`PgidKillGuard`] on Unix and its kill-on-close job on Windows;
/// `kill_on_drop` alone ends only the shell and leaves the filter's stages running.
pub(super) async fn run_filter_on_capture(
    filter: &str,
    capture_path: &str,
    work_dir: &Path,
    grace: std::time::Duration,
) -> FilterRun {
    let stdin = match std::fs::File::open(capture_path) {
        Ok(file) => file,
        Err(e) => {
            tracing::warn!(path = %capture_path, error = %e, "tee capture unreadable for the filter");
            return FilterRun::NotRun("the capture file could not be opened for the filter");
        }
    };
    let mut cmd = crate::platform::shell_command_configured(filter);
    if let Err(e) = crate::tools::invocation_context::apply_to_shell(&mut cmd) {
        tracing::warn!(error = %e, "the filter of a timed-out command could not be configured");
        return FilterRun::NotRun("the filter's shell could not be configured");
    }
    cmd.current_dir(work_dir)
        .stdin(std::process::Stdio::from(stdin))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            tracing::warn!(error = %e, "the filter of a timed-out command could not be started");
            return FilterRun::NotRun("the filter could not be started");
        }
    };
    // `process_group(0)` in the platform builder: the group id is the shell's pid. Armed until the
    // wait completes, so a drop of this future at any await below kills the group.
    #[cfg(unix)]
    let mut guard = PgidKillGuard(child.id().map(|id| id as i32));
    // Dropped with this future, which closes the job and ends the filter's tree.
    #[cfg(windows)]
    let _job_guard = crate::platform::kill_on_close_job(&child);
    match tokio::time::timeout(grace, child.wait_with_output()).await {
        Ok(Ok(output)) => {
            // Waited to completion: the group may be gone and its id reused, so do not kill it.
            #[cfg(unix)]
            {
                guard.0 = None;
            }
            FilterRun::Ran {
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                exit_code: output.status.code().unwrap_or(-1),
            }
        }
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "the filter of a timed-out command failed");
            FilterRun::NotRun("waiting on the filter failed")
        }
        // The guard, still armed, kills the group as this returns.
        Err(_) => FilterRun::GraceExpired,
    }
}

/// What running a timed-out command's filter over its partial capture produced.
#[derive(Debug, PartialEq)]
pub(super) enum FilterRun {
    /// The filter ended within its grace. Raw text: the caller scrubs it.
    Ran {
        stdout: String,
        stderr: String,
        exit_code: i32,
    },
    /// The filter was still running when its grace expired, and was killed.
    GraceExpired,
    /// Nothing ran, or nothing usable came back, for this reason (`filter_not_run`).
    NotRun(&'static str),
}

/// Classify a command into a known-slow bucket. Returns a short label used
/// to tailor the timeout-error hint. Conservative — only matches strong
/// signals (test runners, build systems, ETL/training pipelines) so the
/// generic fallback hint still fires for ad-hoc commands.
///
/// Source for the patterns: docs/usage-reports/2026-05-27-usage-analysis.md
/// query F (slow run_commands) — the top-15 timeout offenders.
pub(crate) fn classify_slow_command(cmd: &str) -> Option<&'static str> {
    let lower = cmd.to_lowercase();
    if lower.contains("pytest")
        || lower.contains("cargo test")
        || lower.contains("npm test")
        || lower.contains("pnpm test")
        || lower.contains("yarn test")
        || lower.contains("tox ")
    {
        return Some("test suite");
    }
    if lower.contains("cargo build")
        || lower.contains("npm run build")
        || lower.contains("pnpm build")
        || lower.contains("yarn build")
        || lower.contains("make ")
        || lower.contains("gradle ")
        || lower.contains("./gradlew")
        || lower.contains("mvn ")
        || lower.contains("./scripts/build")
        || lower.contains("docker build")
    {
        return Some("build");
    }
    if lower.contains(" ingest")
        || lower.contains(" eval")
        || lower.contains("benchmark")
        || lower.contains(" training")
        || lower.contains(" train ")
    {
        return Some("ETL/eval/training");
    }
    if lower.contains("uv run python")
        || lower.contains("python -m ")
        || lower.contains("python scripts/")
    {
        return Some("python script");
    }
    None
}

/// What `RunCommand::call` knows about one call that the response must carry. Handed to
/// `run_command_inner` so the inline-or-summary gate measures it with the response it joins.
#[derive(Default)]
pub(crate) struct CallNotes {
    /// Said when `call` corrected the timeout parameter (`timeout_hint`).
    pub timeout_hint: Option<String>,
    /// `↻ <handle> refreshed from disk` lines for the `@file_*` handles `resolve_refs` re-read,
    /// prepended to `stdout` where the response has one.
    pub stdout_prefix: String,
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
    // What `call` knows about this call that the response must carry. Measured by the
    // inline-or-summary gate below (`LateKeys`), not added after it.
    notes: &CallNotes,
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
                run_in_background,
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
        if let Some(hint) =
            crate::util::path_security::check_source_file_access(resolved_command, root)
        {
            return Err(RecoverableError::with_hint(
                "shell access to source files is blocked",
                &hint,
            )
            .into());
        }
    }

    // --- Step 2.6: Commit-message backtick gate ---
    // Sits WITH the other policy gates rather than ahead of them: a command blocked for
    // any other reason is blocked whatever its message contains, and answering "backtick"
    // first would change what every other refusal reads as. Same ordering rule the shell
    // preflight at step 4.3 had to learn across two CI rounds.
    if !buffer_only && !acknowledge_risk {
        if let Some(inner) =
            crate::util::path_security::commit_message_backtick_hazard(resolved_command)
        {
            return Err(RecoverableError::with_hint(
                format!(
                    "the commit message contains a backtick the shell evaluates before git \
                     runs: `{inner}` executes as a command and its output replaces that text"
                ),
                "Write the message with a quoted heredoc and `git commit -F` — what 97% of \
                 this repo's backtick-bearing commits already do:\n  \
                 cat > /tmp/msg <<'EOF'\n  <message>\n  EOF\n  git commit -F /tmp/msg\n\
                 Otherwise escape each backtick as \\`, or drop the backticks. Pass \
                 acknowledge_risk=true if the substitution is genuinely intended.",
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
                    format!(
                        "Set security.shell_command_mode = \"warn\" or \"unrestricted\" in \
                         .codescout/project.toml. {}",
                        crate::config::project::CONFIG_IS_CACHED_REMEDY
                    ),
                )
                .into());
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

    // --- Step 4.2: Incompatible-parameter gate ---
    // Hoisted above the preflight with the other policy checks: this rejection is
    // about the caller's arguments and is true on any host, shell or no shell.
    if run_in_background && buffer_only {
        return Err(RecoverableError::with_hint(
            "run_in_background cannot be used with buffer queries",
            "Remove run_in_background, or run the query as a plain command without @ref interpolation.",
        )
        .into());
    }

    // --- Step 4.3: Shell preflight ---
    // LAST check before anything is spawned, and that ordering is the contract:
    // every gate above rejects on policy or arguments and is true whether or not
    // a shell exists, so answering "no shell" first would replace the real reason
    // for the refusal with an environmental one. On Windows every spawn goes
    // through Git Bash; without one the OS answers `program not found`, which
    // names neither the requirement nor the fix.
    // See docs/issues/archive/2026-08-08-run-command-unusable-without-git-bash.md (WIN-36).
    if let Some(hint) = crate::platform::shell_unavailable_hint() {
        return Err(
            RecoverableError::with_hint("no POSIX shell available to run commands", hint).into(),
        );
    }

    // --- Step 4.7: Background spawn, returning at spawn time ---
    if run_in_background {
        return spawn_background_command(resolved_command, &work_dir, ctx).await;
    }

    // --- Step 4.5: Tee injection for terminal filter commands ---
    // When the last pipe stage is a known filter (grep, head, tail, sed, awk, etc.),
    // inject `tee /tmp/codescout-unfiltered-XXXX` before the filter so the caller
    // can surface the unfiltered stream as a buffer ref without re-running the command.
    let TeeInjection {
        command: effective_command,
        capture: unfiltered_tmpfile,
        skipped: tee_skipped,
        filter: tee_filter,
        prefix: tee_prefix,
    } = inject_tee(resolved_command, buffer_only)?;

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
    // SIGPIPE=SIG_IGN; every spawned process inherits it, so with SIG_IGN a pipeline whose
    // reader exits early (`yes | head -1`) would never end: the writer gets EPIPE it ignores
    // and keeps going.  The reset keeps that ending for pipelines we do not splice.  For a
    // spliced `| tee cap | { filter; drain; }` the bounded drain, not SIGPIPE, decides how far
    // the producer runs: the group reads up to the capture store limit after the filter exits,
    // so a finite stream within it is captured whole. A fast endless one ends when that bound
    // is read and the pipe closes; a slow one runs to the timeout (see `inject_tee_in`).
    #[cfg(unix)]
    let (child_output_fut, child_pgid) = {
        // Base config (sh -c, GIT_PAGER, process_group(0), SIGPIPE reset,
        // stdin=null) comes from the shared platform builder; foreground only
        // adds piped stdio + kill_on_drop.
        let mut cmd = crate::platform::shell_command_configured(&effective_command);
        crate::tools::invocation_context::apply_to_shell(&mut cmd)?;
        cmd.current_dir(&work_dir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true); // SIGKILL on Drop — reaps shell on cancel
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
            let mut guard = PgidKillGuard(pgid_for_guard);
            let result = child.wait_with_output().await;
            // Successful completion: disarm the guard by clearing the pgid so
            // the Drop impl sees None and skips the SIGKILL.
            guard.0 = None;
            result
        });
        (fut, pgid)
    };

    // `_child_pgid` is unused on Windows — the process-group kill in the timeout
    // arm is `#[cfg(unix)]` only. The Unix branch above binds it as `child_pgid`.
    #[cfg(windows)]
    let (child_output_fut, _child_pgid) = {
        // Capture stdout/stderr via temp files instead of pipes, and wait on the
        // *process* rather than on pipe EOF.
        //
        // On Windows a grandchild — git's helper exe, a Python subprocess, or an
        // EDR/AV DLL injected into the child — inherits the write end of a
        // captured pipe and can hold it open after our direct child has exited.
        // `.output()` waits for pipe EOF, so it never returns even though the
        // command finished. (The timeout hint below — "output() never gets EOF" —
        // is exactly this failure.) Redirecting to files and waiting on the
        // process means a lingering grandchild keeping a file handle open does
        // not block our read.
        //
        // The Git Bash `-c` invocation, MSYS arg-conversion opt-outs, GIT_PAGER,
        // and stdin=NUL all come from the shared platform builder
        // (`platform::shell_command_configured`). Foreground only adds the
        // file-capture stdio + kill_on_drop.
        let out_tmp = tempfile::Builder::new()
            .prefix("codescout-cmd-out-")
            .tempfile()?;
        let err_tmp = tempfile::Builder::new()
            .prefix("codescout-cmd-err-")
            .tempfile()?;
        let (out_file, out_path) = out_tmp.keep()?;
        let (err_file, err_path) = err_tmp.keep()?;
        // Establish cleanup guards BEFORE spawn so an early `?` (spawn failure)
        // still deletes the just-`keep()`d temp files. They are moved into the
        // future below, where they also drop on normal completion and on
        // future-drop (timeout / cancellation).
        let out_guard = TmpfileGuard(out_path.to_string_lossy().into_owned());
        let err_guard = TmpfileGuard(err_path.to_string_lossy().into_owned());

        let mut cmd = crate::platform::shell_command_configured(&effective_command);
        crate::tools::invocation_context::apply_to_shell(&mut cmd)?;
        cmd.current_dir(&work_dir)
            .stdout(std::process::Stdio::from(out_file))
            .stderr(std::process::Stdio::from(err_file))
            .kill_on_drop(true); // kills the shell itself on cancel/timeout (future drop)
        let mut child = cmd.spawn()?;

        // `kill_on_drop` terminates only the shell. Its own children survive —
        // with `bash -c "sleep 5 && touch x"` cancelling killed bash while sleep
        // ran to completion and touch still fired. A kill-on-close Job Object is
        // the Windows analogue of the Unix process-group kill used above: the
        // whole tree dies when this guard drops. Best-effort — `None` when the
        // job cannot be created, which just restores the old behaviour.
        let job_guard = crate::platform::kill_on_close_job(&child);

        let fut: std::pin::Pin<
            Box<dyn std::future::Future<Output = std::io::Result<std::process::Output>> + Send>,
        > = Box::pin(async move {
            // Guards (created before spawn) move in here; they drop on normal
            // completion *and* on future-drop (timeout / cancellation).
            let _out_guard = out_guard;
            let _err_guard = err_guard;
            let _job_guard = job_guard;
            let status = child.wait().await?;
            let stdout = std::fs::read(&out_path).unwrap_or_default();
            let stderr = std::fs::read(&err_path).unwrap_or_default();
            Ok(std::process::Output {
                status,
                stdout,
                stderr,
            })
        });
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
            // Bug bc0cb248b224d1dd: scrub at the ONE point the raw streams become text, so the inline
            // response, the @cmd buffer, test compaction and a background job's `cat @bg_*` all see only
            // the scrubbed text. The tee capture is read from a file elsewhere and is scrubbed there.
            let stdout_text = String::from_utf8_lossy(&output.stdout);
            let stderr_text = String::from_utf8_lossy(&output.stderr);
            let stdout = redact_credentials(&stdout_text);
            let stderr = redact_credentials(&stderr_text);
            let redacted = stdout.count + stderr.count;
            let result = handle_successful_output_with(
                original_command,
                stdout.text.into_owned(),
                stderr.text.into_owned(),
                output.status.code().unwrap_or(-1),
                buffer_only,
                unfiltered_tmpfile,
                &work_dir,
                ctx,
                // Passed IN, not added afterwards: the summary-or-inline gate measures the response
                // including them. `unfiltered_output_skipped` says a wanted capture could not be
                // made; absent, the response would be indistinguishable from a command that simply
                // had no filter to capture.
                LateKeys {
                    redacted,
                    tee_skipped,
                    // Built here, after the child exited, so `jobs` reports what the read saw.
                    envelope: super::envelope_keys(
                        ctx,
                        original_command,
                        notes.timeout_hint.as_deref(),
                    ),
                    stdout_prefix: notes.stdout_prefix.clone(),
                },
            )
            .await?;
            Ok(result)
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
            let next_timeout = timeout_secs.saturating_mul(3).max(300);
            let hint = match classify_slow_command(original_command) {
                Some(label) => format!(
                    "Looks like a {label} command (long-running by nature). \
                     Two options: (1) re-run with run_in_background: true — returns \
                     immediately, output streams to a log file you can tail/grep; \
                     (2) re-run with a higher timeout_secs (current: {timeout_secs}s; \
                     try {next_timeout}s). If the command launches background processes \
                     (with &), prefer run_in_background — shell & leaves them holding \
                     the stdout pipe open."
                ),
                None => format!(
                    "Command exceeded {timeout_secs}s. If it launches background \
                     processes (e.g. with &), use run_in_background: true — shell & \
                     leaves background processes holding the stdout pipe open, so \
                     output() never gets EOF. run_in_background spawns via a log file \
                     instead and returns immediately. For genuinely slow commands, \
                     pass a higher timeout_secs (try {next_timeout}s)."
                ),
            };
            let mut response = json!({
                "timed_out": true,
                "stderr": format!("Command timed out after {} seconds", timeout_secs),
                "exit_code": null,
                "hint": hint,
            });
            // A filtered command's producer was killed with its capture on disk: run the filter
            // once over that partial capture, within a grace of its own, and return what it makes
            // of it, flagged `capture_incomplete` — but only a filter whose replay is provably
            // pure (`filter_is_replayable`); any other gets `filter_not_run` and the handle. No
            // filter, an unreadable capture or an empty one: the bare shape above, as before.
            if let (Some(filter), Some(prefix), Some(capture)) = (
                tee_filter.as_deref(),
                tee_prefix.as_deref(),
                &unfiltered_tmpfile,
            ) {
                if let Some((content, capture_redacted)) = super::output::read_capture(capture) {
                    if !content.is_empty() {
                        let (run, filter_redacted) =
                            match super::super::command_summary::filter_is_replayable(
                                prefix, filter,
                            ) {
                                // The filter reads the file as it is on disk; its streams are scrubbed
                                // here, the stored capture by `read_capture` above.
                                Ok(()) => match run_filter_on_capture(
                                    filter,
                                    &capture.0,
                                    &work_dir,
                                    std::time::Duration::from_secs(TIMEOUT_FILTER_GRACE_SECS),
                                )
                                .await
                                {
                                    FilterRun::Ran {
                                        stdout,
                                        stderr,
                                        exit_code,
                                    } => {
                                        let stdout = redact_credentials(&stdout);
                                        let stderr = redact_credentials(&stderr);
                                        let count = stdout.count + stderr.count;
                                        (
                                            FilterRun::Ran {
                                                stdout: stdout.text.into_owned(),
                                                stderr: stderr.text.into_owned(),
                                                exit_code,
                                            },
                                            count,
                                        )
                                    }
                                    other => (other, 0),
                                },
                                Err(reason) => (FilterRun::NotRun(reason), 0),
                            };
                        let unfiltered =
                            super::output::store_capture(content, original_command, -1, ctx);
                        // The envelope keys go on BEFORE the stdout room is measured, as the
                        // normal path's gate measures them; `call` then leaves them as they are.
                        if unfiltered.is_some() {
                            if let Some(obj) = response.as_object_mut() {
                                obj.extend(super::envelope_keys(
                                    ctx,
                                    original_command,
                                    notes.timeout_hint.as_deref(),
                                ));
                            }
                        }
                        super::output::attach_partial_capture(
                            &mut response,
                            run,
                            unfiltered,
                            capture_redacted + filter_redacted,
                            original_command,
                        );
                    }
                }
            }
            Ok(response)
        }
    }
}
