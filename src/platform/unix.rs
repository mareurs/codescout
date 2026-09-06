use std::path::PathBuf;

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
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
        "~/.docker/config.json",
        "~/.netrc",
        "~/.npmrc",
        "~/.kube/config",
        // Git credential stores (both legacy and XDG locations)
        "~/.git-credentials",
        "~/.config/git/credentials",
        // Package-registry credentials
        "~/.pypirc",
        "~/.cargo/credentials.toml",
        "~/.cargo/credentials",
        // DB + SQL client credentials
        "~/.pgpass",
        "~/.my.cnf",
        // Password managers / keyrings
        "~/.password-store",
        "~/.config/op",
        "~/.config/Bitwarden",
        "~/.local/share/keyrings",
        // Shell/tool history — often captures secret argv
        "~/.bash_history",
        "~/.zsh_history",
        "~/.psql_history",
        "~/.python_history",
        "~/.config/atuin",
        // macOS: Keychain stores
        "~/Library/Keychains",
        // System secrets (Linux)
        "/etc/shadow",
        "/etc/gshadow",
        "/etc/sudoers",
        "/etc/sudoers.d",
        // macOS system secrets
        "/etc/master.passwd",
        "/private/etc/sudoers",
        "/private/etc/sudoers.d",
        "/private/etc/master.passwd",
        // Linux: /proc/self/environ and /proc/self/mem leak the current
        // process's env and memory. Only self is predictable without a pid
        // glob; deny-list format does not support glob so we block the most
        // directly reachable patterns.
        "/proc/self/environ",
        "/proc/self/mem",
    ]
}

/// Always `None` — POSIX guarantees `/bin/sh`. The Windows twin can answer
/// `Some(hint)` when no Git Bash is installed.
pub fn shell_unavailable_hint() -> Option<String> {
    None
}

pub fn shell_command_configured(cmd: &str) -> tokio::process::Command {
    let mut c = tokio::process::Command::new("sh");
    c.arg("-c")
        .arg(cmd)
        .env("GIT_PAGER", "cat")
        .stdin(std::process::Stdio::null())
        .process_group(0);
    // SAFETY: pre_exec runs post-fork, pre-exec; signal() is async-signal-safe.
    unsafe {
        c.pre_exec(|| {
            libc::signal(libc::SIGPIPE, libc::SIG_DFL);
            Ok(())
        });
    }
    c
}

/// POSIX tokenization, shared with the Windows implementation now that both
/// platforms execute through a POSIX shell.
pub fn shell_tokenize(cmd: &str) -> Result<Vec<String>, String> {
    super::posix_tokenize(cmd)
}

/// Narrow a `u32` to a pid that addresses **one process**, or `None`.
///
/// `libc::kill` takes a signed `pid_t`, and the sign is not a formality — it
/// selects a different addressing mode entirely:
///
/// | value | what `kill` addresses |
/// |---|---|
/// | `> 0` | that one process |
/// | `0` | every process in the **caller's own process group** |
/// | `-1` | every process the caller has permission to signal |
/// | `< -1` | every process in process group `-pid` |
///
/// So `pid as i32` is not a widening formality. Any `u32` at or above `2^31`
/// wraps to a negative value and silently changes what the call means:
/// [`process_alive`] returned **`true` for a process that does not exist**
/// (`kill(-1, 0)` succeeds), and [`terminate_process`] would have sent SIGTERM
/// to every process the user owns.
///
/// Kernel-assigned pids fit in `i32` — Linux caps `pid_max` at 4,194,304 — so a
/// pid taken straight from `Child::id()` can never reach these cases, which is
/// what the safety comment at `lsp/client.rs` correctly claims of *its* caller.
/// A pid **parsed** from a file, a JSON payload or a catalog row can, and this
/// crate now has both kinds: `librarian::reindex_progress` reads pids back out
/// of `catalog_meta`. Validating here rather than at each call site is the
/// difference between one check and N.
///
/// Measured 2026-09-05: `process_alive(u32::MAX)` returned `true` on Linux, and
/// `process_alive(0)` returns `true` for any caller in a live process group —
/// the latter known since 2026-08-18 and worked around at one call site
/// (`tools/rendezvous.rs`) rather than fixed here.
fn addressable_pid(pid: u32) -> Option<libc::pid_t> {
    match libc::pid_t::try_from(pid) {
        Ok(p) if p > 0 => Some(p),
        _ => None,
    }
}

pub fn terminate_process(pid: u32) -> std::io::Result<()> {
    // Refused rather than cast: see `addressable_pid`. The cast would turn a
    // single-process SIGTERM into a group- or session-wide one.
    let Some(target) = addressable_pid(pid) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{pid} does not address a single process"),
        ));
    };
    let ret = unsafe { libc::kill(target, libc::SIGTERM) };
    if ret == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

pub fn process_alive(pid: u32) -> bool {
    // `false` rather than a cast: a pid that cannot address one process is not
    // a live process, and answering `true` — which `kill(-1, 0)` and
    // `kill(0, 0)` both do — makes a dead holder look alive to every liveness
    // check in the tree. See `addressable_pid`.
    let Some(target) = addressable_pid(pid) else {
        return false;
    };
    unsafe { libc::kill(target, 0) == 0 }
}

pub fn rename_overwrite(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::rename(from, to)
}

pub fn lsp_binary_name(base: &str) -> String {
    base.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_dir_returns_some() {
        assert!(home_dir().is_some());
    }

    /// A pid that is definitely not running — spawned, then reaped.
    ///
    /// Deliberately NOT a magic constant. `0` and any value ≥ `2^31` are both
    /// "dead" for reasons that have nothing to do with liveness (see
    /// `addressable_pid`), so a constant would let a `process_alive` that never
    /// calls `kill` at all pass this file's liveness tests.
    fn a_reaped_pid() -> u32 {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        pid
    }

    #[test]
    fn liveness_is_true_for_this_process_and_false_for_a_reaped_one() {
        assert!(process_alive(std::process::id()));
        assert!(!process_alive(a_reaped_pid()));
    }

    /// The values where `kill`'s addressing mode changes, which a `pid as i32`
    /// cast reaches silently.
    ///
    /// **Every row here was `true` before 2026-09-05**, which is the whole
    /// point: each one makes a nonexistent process report as alive, so any
    /// stale-holder cleanup keyed on liveness never fires. `u32::MAX` casts to
    /// `-1` (every signalable process); `0` addresses the caller's own process
    /// group, so it is true for any live caller; `0x8000_0000` is the first
    /// value that wraps negative.
    ///
    /// Pair this with the row above: without a genuinely reaped pid there, a
    /// `process_alive` hard-wired to `false` would satisfy this test.
    #[test]
    fn a_pid_that_does_not_address_one_process_is_not_alive() {
        for pid in [0u32, 0x8000_0000, u32::MAX] {
            assert!(
                !process_alive(pid),
                "process_alive({pid}) must be false — it does not name one process"
            );
        }
    }

    /// The same boundary on the signalling path, where the consequence is not a
    /// wrong answer but a wrong *action*: `kill(-1, SIGTERM)` terminates every
    /// process the user owns.
    #[test]
    fn terminating_a_pid_that_does_not_address_one_process_is_refused() {
        for pid in [0u32, 0x8000_0000, u32::MAX] {
            let err = terminate_process(pid).expect_err(
                "must refuse: casting this would signal a process GROUP, not one process",
            );
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput, "pid {pid}");
        }
    }

    #[test]
    fn temp_dir_exists() {
        assert!(temp_dir().exists());
    }

    #[test]
    fn shell_command_uses_sh() {
        let cmd = shell_command_configured("echo hello");
        let std_cmd = cmd.as_std();
        assert_eq!(std_cmd.get_program().to_str().unwrap(), "sh");
        let args: Vec<&str> = std_cmd.get_args().map(|a| a.to_str().unwrap()).collect();
        assert_eq!(args, vec!["-c", "echo hello"]);
    }

    #[test]
    fn shell_tokenize_splits_correctly() {
        let tokens = shell_tokenize("echo 'hello world'").unwrap();
        assert_eq!(tokens, vec!["echo", "hello world"]);
    }

    #[test]
    fn lsp_binary_name_unchanged() {
        assert_eq!(lsp_binary_name("rust-analyzer"), "rust-analyzer");
    }
}
