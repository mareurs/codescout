//! Platform abstraction layer.
//!
//! Provides OS-specific implementations for filesystem paths, shell commands,
//! process management, and security defaults. All platform-specific code should
//! go through this module rather than using `#[cfg]` blocks elsewhere.

use std::path::PathBuf;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix as imp;
#[cfg(windows)]
use windows as imp;

/// Return the user's home directory.
pub fn home_dir() -> Option<PathBuf> {
    imp::home_dir()
}

/// Return the system temporary directory.
pub fn temp_dir() -> PathBuf {
    imp::temp_dir()
}

/// Return the platform-specific read deny-list prefixes (e.g. `~/.ssh`).
pub fn denied_read_prefixes() -> &'static [&'static str] {
    imp::denied_read_prefixes()
}

/// Tokenize a command string into arguments using platform-appropriate rules.
/// Unix: shell_words::split. Windows: custom tokenizer (no backslash escapes).
pub fn shell_tokenize(cmd: &str) -> Result<Vec<String>, String> {
    imp::shell_tokenize(cmd)
}

/// Send a termination signal to a process.
/// Unix: SIGTERM. Windows: TerminateProcess.
pub fn terminate_process(pid: u32) -> std::io::Result<()> {
    imp::terminate_process(pid)
}

/// Check if a process is alive.
pub fn process_alive(pid: u32) -> bool {
    imp::process_alive(pid)
}

/// Platform-aware rename that overwrites the destination.
/// On Unix this is a no-op wrapper around `std::fs::rename`.
/// On Windows this uses `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`.
pub fn rename_overwrite(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    imp::rename_overwrite(from, to)
}

/// Platform-aware LSP server binary name.
/// On Windows, appends `.cmd` or `.exe` as needed.
pub fn lsp_binary_name(base: &str) -> String {
    imp::lsp_binary_name(base)
}

/// Resolve the on-disk binary name for a dual-packaged LSP server, parameterized
/// over an existence probe so the extension-preference logic is pure and
/// unit-testable on any platform (the `PATH` side-effect lives in the Windows
/// `find_on_path`). Windows-only in effect — `windows::lsp_binary_name` is the only
/// caller — but kept here so its tests run on the Linux gate, not just on Windows.
///
/// Preference: native `.exe` first (spawns directly, no `cmd.exe` shim — the WIN-1
/// EDR grandchild hazard), then the npm `.cmd` shim, then `.bat`. Non-dual-packaged
/// servers are always `.exe`. Falls back to `.cmd` when nothing resolves, preserving
/// the historical npm default and the prior spawn-failure message.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn lsp_binary_name_with(base: &str, exists: impl Fn(&str) -> bool) -> String {
    let dual_packaged = matches!(
        base,
        "typescript-language-server"
            | "vscode-json-language-server"
            | "yaml-language-server"
            | "bash-language-server"
            | "pyright-langserver"
    );

    if !dual_packaged {
        return format!("{base}.exe");
    }

    // Preference order: native binary (`.exe`) first — it spawns directly,
    // avoiding the extra `cmd.exe` shim layer a `.cmd` batch wrapper forces
    // (the EDR grandchild-spawn hazard from WIN-1). Then the npm shim (`.cmd`),
    // then `.bat`.
    for ext in ["exe", "cmd", "bat"] {
        let candidate = format!("{base}.{ext}");
        if exists(&candidate) {
            return candidate;
        }
    }
    // Nothing on PATH — keep the historical default so npm installs and the
    // prior failure message are unchanged.
    format!("{base}.cmd")
}

/// `Some(hint)` when this platform has no shell to spawn commands through.
///
/// Unix always has `sh`, so it is always `None` there. Windows runs commands
/// through Git Bash and answers `Some` when none is installed — `run_command`
/// turns that into a `RecoverableError` naming the requirement, instead of
/// letting every spawn fail with a bare `program not found`.
pub fn shell_unavailable_hint() -> Option<String> {
    imp::shell_unavailable_hint()
}

/// Build a fully-configured shell `tokio::process::Command` for `cmd`.
/// Windows: `<git-bash> -c <cmd>` — a POSIX shell, same as Unix, since WIN-32.
/// Unix: `sh -c <cmd>` in a fresh process group with SIGPIPE reset.
/// Sets `GIT_PAGER=cat`. The caller sets cwd, stdio, and kill_on_drop.
/// stdin defaults to null on **both** platforms (prevents inherited-pipe / REPL
/// hangs on the MCP stdio server); callers that need real stdin (interactive
/// mode) override with `.stdin(...)`.
pub fn shell_command_configured(cmd: &str) -> tokio::process::Command {
    imp::shell_command_configured(cmd)
}

/// Tokenize a command string using POSIX shell rules: single quotes are
/// literal, double quotes group without consuming backslash escapes, and an
/// unescaped backslash escapes the next character outside single quotes.
///
/// Shared by both platforms because both now execute through a POSIX shell
/// (`sh -c` on Unix, Git Bash `bash -c` on Windows).
///
/// **This is on the security path.** Two callers in
/// `src/util/path_security.rs` consume it, and they consume it differently:
///
/// * `shell_normalized` — rejoins the tokens and runs the dangerous-pattern
///   regexes over that form *in addition to* the raw string. A union, so it can
///   only add catches.
/// * `shell_tokens` — the token source for `stage_trims`, `grep_is_counting`,
///   `is_unbounded_lhs`, `has_recursive_flag`, `extract_grep_pattern` and
///   `check_source_file_access`. A replacement, not a union: those helpers read
///   head tokens and flags, so quote-awareness changes their answers. It swallows
///   the `Err` below and falls back to `split_whitespace` — an unclosed quote must
///   never let a check be skipped entirely.
///
/// What still does NOT agree with the shell: `is_dangerous_command`'s raw-string
/// pass (deliberate — it is the other half of the union) and `OutputBuffer`'s
/// path-likeness heuristic in `src/tools/output_buffer.rs`.
///
/// `il3_offending_lead` used to belong on that list — it split a pipeline on a bare
/// `|`. Fixed 2026-08-14 along with `pipeline_segments`, which had the same
/// quote-blindness on `;` / `&&` / `||` and was the more serious of the two: it let a
/// quoted `;` hide a genuine pipe from the enforcer entirely.
///
/// History, because this comment has been wrong twice in opposite directions.
/// It first claimed to feed the security checks when nothing called it. That was
/// corrected on 2026-08-08 to say it had no production callers — and the very
/// next commit gave it one without updating this text, so the correction was
/// false within the hour. Before changing this paragraph, run
/// `references(symbol="posix_tokenize")` and describe what is actually there.
/// See `docs/issues/archive/2026-08-08-security-layer-tokenizes-unlike-the-shell.md`.
///
/// Note `shell_tokenize` — the per-platform wrapper below — still has no
/// production callers; both consumers reach for this function directly.
///
/// Pure + cross-platform so its tests run on every CI target, not just Windows.
pub fn posix_tokenize(cmd: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut escape_next = false;

    for ch in cmd.chars() {
        if escape_next {
            current.push(ch);
            escape_next = false;
            continue;
        }
        match ch {
            '\\' if !in_single => escape_next = true,
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            c if c.is_whitespace() && !in_single && !in_double => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    if in_single || in_double {
        return Err("unclosed quote".to_string());
    }
    Ok(tokens)
}

/// Render a path for interpolation into a shell command string.
///
/// Unix: unchanged. Windows: backslashes become forward slashes, because the
/// command is executed by Git Bash, where `\` is an escape character — a raw
/// `C:\Users\x\tmp` reaches the shell as `C:Usersxtmp` and names nothing.
/// Git Bash accepts the `C:/Users/x/tmp` form natively.
///
/// Deliberately does NOT quote. `OutputBuffer::resolve_refs` matches substituted
/// words against the same strings it pushes into `temp_path_strings` to decide
/// `is_buffer_only`, which in turn gates the dangerous-command check. Adding
/// quotes here would break that match and silently reclassify buffer-only
/// commands. Whitespace in the path is therefore still the caller's problem —
/// unchanged from the previous behaviour, not a new gap.
pub fn shell_path_str(path: &std::path::Path) -> String {
    let s = path.to_string_lossy();
    if cfg!(windows) {
        s.replace('\\', "/")
    } else {
        s.into_owned()
    }
}

/// Whether two Windows directory paths name the same directory.
///
/// `Path` equality compares components byte-exactly — only the drive-letter
/// prefix folds case — so `C:\Windows\System32` and `C:\Windows\system32` are
/// NOT equal. That is load-bearing in `windows::resolve_git_bash`: Windows
/// setup writes the system `PATH` in the lowercase spelling, while
/// `%SystemRoot%` joined with `"System32"` produces the capitalized one, so the
/// `==` meant to keep the WSL launcher out of the running never matched on a
/// real host and let it be selected.
///
/// Folds ASCII case, accepts either separator, and ignores one trailing
/// separator. Deliberately NOT a prefix test — it answers "same directory", so
/// `C:\Windows\System` never matches `C:\Windows\System32`.
///
/// Lives here rather than in `windows.rs` because that module is
/// `#[cfg(windows)]`: a fix tested only there is verified on one CI leg, and
/// this is the half of the logic that was wrong.
///
/// `pub`, not `pub(crate)`, for the same reason as its neighbours: the only
/// caller is inside `#[cfg(windows)]`, so on a Linux build `-D dead-code`
/// rejects the crate-private form.
pub fn windows_dir_eq(a: &std::path::Path, b: &std::path::Path) -> bool {
    fn key(p: &std::path::Path) -> String {
        let slashed = p.to_string_lossy().replace('\\', "/");
        slashed.trim_end_matches('/').to_ascii_lowercase()
    }
    key(a) == key(b)
}

/// Windows only: assign `child` to a kill-on-close Job Object so dropping the
/// returned guard reaps the entire process tree, not just the direct child.
///
/// The Unix side already reaps the tree via `process_group(0)` + `killpg`.
/// Windows has no process-group analogue, so a Job Object is how cancel/timeout
/// stops a shell's own children — without it `kill_on_drop` kills `bash` and
/// leaves whatever it launched still running.
#[cfg(windows)]
pub fn kill_on_close_job(child: &tokio::process::Child) -> Option<windows::JobGuard> {
    imp::kill_on_close_job(child)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posix_tokenize_handles_quotes_and_escapes() {
        // Single quotes are literal; double quotes group; backslash escapes
        // outside single quotes. These are the rules the executing shell
        // (`sh -c` / Git Bash `bash -c`) applies, so the security layer must
        // read commands the same way it will run them.
        assert_eq!(
            posix_tokenize("echo 'hello world'").unwrap(),
            vec!["echo", "hello world"]
        );
        assert_eq!(
            posix_tokenize(r#"py -c "print(1)""#).unwrap(),
            vec!["py", "-c", "print(1)"]
        );
        assert_eq!(
            posix_tokenize(r"echo a\ b").unwrap(),
            vec!["echo", "a b"],
            "backslash escapes a separator outside quotes"
        );
        assert_eq!(
            posix_tokenize(r#"echo '\n'"#).unwrap(),
            vec!["echo", r"\n"],
            "backslash stays literal inside single quotes"
        );
    }

    #[test]
    fn posix_tokenize_rejects_unclosed_quote() {
        // Must be an error, not a silent truncation: the security layer treats
        // the token list as authoritative when deciding what a command does.
        assert!(posix_tokenize(r#"echo "unterminated"#).is_err());
        assert!(posix_tokenize("echo 'unterminated").is_err());
    }

    /// The exact pair that made the WSL exclusion inert:
    /// `%SystemRoot%`.join("System32") yields the capitalized spelling, while
    /// Windows setup writes the system `PATH` with the lowercase one, and a
    /// byte-exact `Path` compare calls those different directories.
    ///
    /// Runs on every CI leg — `platform::windows` is `#[cfg(windows)]`, so
    /// asserting this only there would verify the fix on one leg of four.
    #[test]
    fn windows_dir_eq_folds_case_and_separators() {
        use std::path::Path;

        assert!(windows_dir_eq(
            Path::new(r"C:\Windows\system32"),
            Path::new(r"C:\Windows\System32"),
        ));
        assert!(windows_dir_eq(
            Path::new(r"C:\WINDOWS\SYSTEM32\"),
            Path::new("C:/Windows/System32"),
        ));

        // "Same directory", never a prefix test: a sibling whose name merely
        // starts with the excluded one must still be searched, and a child of
        // the excluded directory is not the excluded directory.
        assert!(!windows_dir_eq(
            Path::new(r"C:\Windows\System"),
            Path::new(r"C:\Windows\System32"),
        ));
        assert!(!windows_dir_eq(
            Path::new(r"C:\Windows\System32\downlevel"),
            Path::new(r"C:\Windows\System32"),
        ));
        assert!(!windows_dir_eq(
            Path::new(r"C:\tools\git\bin"),
            Path::new(r"C:\Windows\System32"),
        ));
    }

    #[test]
    fn pyright_prefers_exe_when_only_exe_present() {
        // Regression: a pip/pipx/standalone pyright install ships
        // `pyright-langserver.exe`, not the npm `.cmd` shim. The old
        // hardcoded `.cmd` named a file that did not exist, so the LSP
        // spawn failed with "Failed to start LSP server: pyright-langserver.cmd".
        let only_exe = |name: &str| name == "pyright-langserver.exe";
        assert_eq!(
            lsp_binary_name_with("pyright-langserver", only_exe),
            "pyright-langserver.exe"
        );
    }

    #[test]
    fn pyright_prefers_cmd_when_npm_shim_present() {
        let only_cmd = |name: &str| name == "pyright-langserver.cmd";
        assert_eq!(
            lsp_binary_name_with("pyright-langserver", only_cmd),
            "pyright-langserver.cmd"
        );
    }

    #[test]
    fn pyright_prefers_exe_when_both_present() {
        // Both packagings on PATH: prefer the native `.exe`, which spawns
        // directly instead of through a `cmd.exe` shim (the WIN-1 EDR hazard).
        let both = |_: &str| true;
        assert_eq!(
            lsp_binary_name_with("pyright-langserver", both),
            "pyright-langserver.exe"
        );
    }

    #[test]
    fn dual_packaged_falls_back_to_cmd_when_absent() {
        // Nothing resolves — preserve the prior default + error message.
        let none = |_: &str| false;
        assert_eq!(
            lsp_binary_name_with("pyright-langserver", none),
            "pyright-langserver.cmd"
        );
    }

    #[test]
    fn non_dual_packaged_server_uses_exe() {
        let none = |_: &str| false;
        assert_eq!(
            lsp_binary_name_with("rust-analyzer", none),
            "rust-analyzer.exe"
        );
    }
}
