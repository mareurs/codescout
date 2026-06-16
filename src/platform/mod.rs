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

/// Build the verbatim command-line tail handed to `cmd /C` on Windows.
/// Wrapped in an outer quote pair so cmd's `/C` quote rule consumes exactly
/// that pair and runs the inner command — including its own quotes — verbatim.
/// Pure + cross-platform so it is testable on the Linux CI.
pub fn build_windows_cmdline(cmd: &str) -> String {
    format!("/C \"{cmd}\"")
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

/// Build a fully-configured shell `tokio::process::Command` for `cmd`.
/// Windows: `cmd /C "<cmd>"` via raw_arg (no MSVC-CRT quote mangling).
/// Unix: `sh -c <cmd>` in a fresh process group with SIGPIPE reset.
/// Sets `GIT_PAGER=cat`. The caller sets cwd, stdio, and kill_on_drop.
/// stdin defaults to null on **both** platforms (prevents inherited-pipe / REPL
/// hangs on the MCP stdio server); callers that need real stdin (interactive
/// mode) override with `.stdin(...)`.
pub fn shell_command_configured(cmd: &str) -> tokio::process::Command {
    imp::shell_command_configured(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_cmdline_wraps_in_outer_quotes() {
        // cmd /C with a leading quote strips the outer-pair quotes of the whole
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
