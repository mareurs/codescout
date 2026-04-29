//! Platform abstraction layer.
//!
//! Provides OS-specific implementations for filesystem paths, shell commands,
//! process management, and security defaults. All platform-specific code should
//! go through this module rather than using `#[cfg]` blocks elsewhere.

use std::path::{Path, PathBuf};

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

/// Canonicalize a path, normalising platform-specific quirks.
///
/// On Unix this is a thin wrapper around `std::fs::canonicalize`.
///
/// On Windows this uses `dunce::canonicalize`, which strips Windows verbatim
/// UNC prefixes (`\\?\C:\foo` → `C:\foo`) when the underlying path does not
/// actually need them. This matters because:
///   * `std::fs::canonicalize` on Windows always returns a `\\?\…` prefix.
///   * Many downstream consumers (LSP servers, `git`, prefix comparisons,
///     human-readable error messages) do not handle the verbatim form.
///     Falling back to `dunce` keeps paths comparable across the codebase.
pub fn canonicalize(path: &Path) -> std::io::Result<PathBuf> {
    #[cfg(windows)]
    {
        dunce::canonicalize(path)
    }
    #[cfg(not(windows))]
    {
        std::fs::canonicalize(path)
    }
}

/// Best-effort canonicalize: returns the canonical form when the path exists,
/// otherwise the input unchanged. Use at validation/comparison boundaries
/// where a missing path is not itself an error.
pub fn canonicalize_or(path: &Path) -> PathBuf {
    canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
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
