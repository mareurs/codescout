use std::path::PathBuf;

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
}

pub fn temp_dir() -> PathBuf {
    std::env::temp_dir()
}

pub fn denied_read_prefixes() -> &'static [&'static str] {
    &[
        // Cloud / provider credentials
        "~/.ssh",
        "~/.aws",
        "~/.gnupg",
        "~/.config/gcloud",
        "~/.config/gh",
        "~/.netrc",
        "~/.npmrc",
        "~/.pypirc",
        "~/.docker/config.json",
        "~/.kube/config",
        // Git credential stores
        "~/.git-credentials",
        "~/.config/git/credentials",
        // Package-registry credentials
        "~/.cargo/credentials.toml",
        "~/.cargo/credentials",
        // DB + SQL client credentials
        "~/.pgpass",
        "~/.my.cnf",
        // Password managers
        "~/.password-store",
        "~/.config/op",
        "~/.config/Bitwarden",
        // Shell/tool history
        "~/.bash_history",
        "~/.zsh_history",
        "~/.psql_history",
        "~/.python_history",
        "~/.config/atuin",
    ]
}

pub fn shell_command_configured(cmd: &str) -> tokio::process::Command {
    use std::os::windows::process::CommandExt;
    let mut std_cmd = std::process::Command::new("cmd");
    std_cmd
        .raw_arg(super::build_windows_cmdline(cmd))
        .env("GIT_PAGER", "cat")
        .stdin(std::process::Stdio::null());
    tokio::process::Command::from(std_cmd)
}

pub fn shell_tokenize(cmd: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in cmd.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ' ' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    if in_quotes {
        return Err("unclosed quote".to_string());
    }
    Ok(tokens)
}

pub fn terminate_process(pid: u32) -> std::io::Result<()> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
    // SAFETY: OpenProcess returns a null handle on failure (checked below); the
    // handle is closed on every path before returning. bInheritHandle = 0 (FALSE).
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if handle.is_null() {
            // Process already gone (or we lack rights) — treat "gone" as success,
            // matching the old taskkill semantics where a dead PID is not an error.
            return Ok(());
        }
        let ok = TerminateProcess(handle, 1);
        CloseHandle(handle);
        if ok == 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

pub fn process_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    // GetExitCodeProcess reports STILL_ACTIVE (259) for a running process.
    // Defined locally to avoid windows-sys version drift in its export path.
    const STILL_ACTIVE: u32 = 259;
    // SAFETY: handle is null-checked and closed before returning; exit_code is a
    // valid out-param for the duration of the call. bInheritHandle = 0 (FALSE).
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut exit_code: u32 = 0;
        let got = GetExitCodeProcess(handle, &mut exit_code);
        CloseHandle(handle);
        got != 0 && exit_code == STILL_ACTIVE
    }
}

pub fn rename_overwrite(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    if to.exists() {
        std::fs::remove_file(to)?;
    }
    std::fs::rename(from, to)
}

/// Resolve the on-disk binary name for an LSP server on Windows.
///
/// Several Node-based servers (typescript, json, yaml, bash) ship as npm
/// `.cmd` shims, but the same tools — pyright especially — are just as often
/// installed via pip/pipx or as a standalone `.exe`. Rather than assume one
/// packaging, probe `PATH` and return whichever variant actually exists.
/// Falls back to `.cmd` for those dual-packaged servers when nothing resolves
/// (preserving the historical default and spawn-failure message), and `.exe`
/// for everything else.
///
/// The pure resolution logic lives in `super::lsp_binary_name_with` (testable on
/// any platform); this wrapper supplies the Windows `PATH`-probe.
pub fn lsp_binary_name(base: &str) -> String {
    super::lsp_binary_name_with(base, |name| find_on_path(name).is_some())
}

/// Search `PATH` for a file with the exact given name (extension included).
/// Returns the first match. Used to detect which packaging of a
/// dual-packaged LSP server is present.
fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| {
        let candidate = dir.join(name);
        candidate.is_file().then_some(candidate)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn win32_terminate_and_liveness() {
        // Spawn a long sleeper, confirm alive, terminate, confirm dead.
        let child = std::process::Command::new("cmd")
            .args(["/C", "ping -n 30 127.0.0.1 >nul"])
            .spawn()
            .unwrap();
        let pid = child.id();
        assert!(process_alive(pid), "sleeper should be alive");
        terminate_process(pid).unwrap();
        // Give the OS a moment to reap the terminated process.
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(
            !process_alive(pid),
            "sleeper should be dead after terminate"
        );
    }

    #[test]
    fn win32_liveness_false_for_dead_pid() {
        // A PID that almost certainly does not exist.
        assert!(!process_alive(0xFFFF_FFF0));
    }
}
