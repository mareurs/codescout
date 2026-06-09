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
    let status = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output()?;
    if status.status.success() {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "taskkill failed: {}",
                String::from_utf8_lossy(&status.stderr)
            ),
        ))
    }
}

pub fn process_alive(pid: u32) -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/NH"])
        .output()
        .map(|o| {
            let stdout = String::from_utf8_lossy(&o.stdout);
            stdout.contains(&pid.to_string())
        })
        .unwrap_or(false)
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
pub fn lsp_binary_name(base: &str) -> String {
    lsp_binary_name_with(base, |name| find_on_path(name).is_some())
}

/// Core resolution logic, parameterized over an existence probe so the
/// extension-preference behavior is unit-testable without touching `PATH`.
fn lsp_binary_name_with(base: &str, exists: impl Fn(&str) -> bool) -> String {
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

    // Preference order: npm shim (`.cmd`), native binary (`.exe`), then `.bat`.
    for ext in ["cmd", "exe", "bat"] {
        let candidate = format!("{base}.{ext}");
        if exists(&candidate) {
            return candidate;
        }
    }
    // Nothing on PATH — keep the historical default so npm installs and the
    // prior failure message are unchanged.
    format!("{base}.cmd")
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
    fn pyright_prefers_cmd_when_both_present() {
        // Both packagings on PATH: keep the historical npm choice.
        let both = |_: &str| true;
        assert_eq!(
            lsp_binary_name_with("pyright-langserver", both),
            "pyright-langserver.cmd"
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
