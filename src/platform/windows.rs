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

pub fn system_path_prefixes() -> &'static [&'static str] {
    // Standard install/user roots on a typical Windows system. Canonicalize
    // is applied at the comparison site, so missing entries (e.g. no
    // `Program Files (x86)` on ARM-only installs) simply never match.
    &[
        r"C:\",
        r"C:\Windows",
        r"C:\Program Files",
        r"C:\Program Files (x86)",
        r"C:\ProgramData",
        r"C:\Users",
    ]
}

pub fn shell_command(cmd: &str) -> (&'static str, Vec<String>) {
    ("cmd.exe", vec!["/C".to_string(), cmd.to_string()])
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

pub fn lsp_binary_name(base: &str) -> String {
    match base {
        "typescript-language-server"
        | "vscode-json-language-server"
        | "yaml-language-server"
        | "bash-language-server"
        | "pyright-langserver" => {
            format!("{}.cmd", base)
        }
        _ => format!("{}.exe", base),
    }
}

#[cfg(test)]
mod tests {
    use super::super::canonicalize;

    #[test]
    fn canonicalize_strips_verbatim_unc_prefix() {
        // dunce::canonicalize should drop the `\\?\` prefix for paths that do
        // not actually require it (i.e. ordinary drive-letter paths). This is
        // the whole reason we route through the platform helper rather than
        // calling std::fs::canonicalize directly.
        let dir = std::env::temp_dir();
        let canon = canonicalize(&dir).expect("temp_dir canonicalize");
        let s = canon.to_string_lossy();
        assert!(
            !s.starts_with(r"\\?\"),
            "canonicalized path should not retain verbatim UNC prefix: {s}"
        );
    }
}
