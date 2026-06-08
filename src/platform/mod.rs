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

/// Build a shell command for executing a string.
/// Returns `(program, args)` — e.g. `("sh", ["-c", cmd])` on Unix.
pub fn shell_command(cmd: &str) -> (&'static str, Vec<String>) {
    imp::shell_command(cmd)
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
/// Build the verbatim command-line tail handed to `cmd /C` on Windows.
/// Wrapped in an outer quote pair so cmd's `/C` quote rule consumes exactly
/// that pair and runs the inner command — including its own quotes — verbatim.
/// Pure + cross-platform so it is testable on the Linux CI.
pub fn build_windows_cmdline(cmd: &str) -> String {
    format!("/C \"{cmd}\"")
}

/// Build a fully-configured shell `tokio::process::Command` for `cmd`.
/// Windows: `cmd /C "<cmd>"` via raw_arg (no MSVC-CRT quote mangling).
/// Unix: `sh -c <cmd>` in a fresh process group with SIGPIPE reset.
/// Sets `GIT_PAGER=cat`. The caller sets cwd, stdio, and kill_on_drop.
pub fn shell_command_configured(cmd: &str) -> tokio::process::Command {
    imp::shell_command_configured(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_cmdline_wraps_in_outer_quotes() {
        // cmd /C with a leading quote strips the first+last quote of the whole
        // line and runs the remainder verbatim, so the command — including its
        // own inner quotes — must be wrapped in exactly one outer pair.
        assert_eq!(
            build_windows_cmdline(r#"py -c "print(1)""#),
            r#"/C "py -c "print(1)"""#
        );
        assert_eq!(
            build_windows_cmdline("git --version"),
            r#"/C "git --version""#
        );
    }
}
