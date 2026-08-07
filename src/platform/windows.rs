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

/// Resolve the Git Bash executable, cached for the process lifetime.
///
/// Resolution order is deliberate, and a plain `PATH` scan is NOT first:
///
/// 1. `CODESCOUT_BASH` — explicit operator override, always wins.
/// 2. `CLAUDE_CODE_GIT_BASH_PATH` — already set by Claude Code hosts for the
///    same purpose; reuse it rather than making the user configure it twice.
/// 3. Known Git-for-Windows install roots.
/// 4. `PATH`, **skipping `%SystemRoot%\System32`**.
///
/// Step 4's exclusion is the load-bearing one. On a machine with WSL enabled,
/// `where bash` returns `C:\Windows\System32\bash.exe` FIRST — that is the WSL
/// launcher, not Git Bash. Selecting it would silently move every command into
/// a different filesystem namespace, where the project is reachable only via
/// `/mnt/c` (measured on this repo: 25-170x slower than native through the 9P
/// boundary) and `$HOME`, installed toolchains, and PATH all differ. It would
/// not fail loudly — it would just be wrong and slow.
///
/// Falls back to the bare name `bash` so an unresolvable install surfaces as a
/// spawn error naming the program, rather than a panic at startup.
fn git_bash_path() -> std::path::PathBuf {
    use std::path::PathBuf;
    use std::sync::OnceLock;

    static RESOLVED: OnceLock<PathBuf> = OnceLock::new();
    RESOLVED
        .get_or_init(|| {
            for var in ["CODESCOUT_BASH", "CLAUDE_CODE_GIT_BASH_PATH"] {
                if let Some(p) = std::env::var_os(var).map(PathBuf::from) {
                    if p.is_file() {
                        return p;
                    }
                }
            }

            let roots = [
                std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Programs")),
                std::env::var_os("ProgramFiles").map(PathBuf::from),
                std::env::var_os("ProgramFiles(x86)").map(PathBuf::from),
            ];
            for root in roots.into_iter().flatten() {
                let candidate = root.join("Git").join("bin").join("bash.exe");
                if candidate.is_file() {
                    return candidate;
                }
            }

            // PATH last, and never the WSL launcher under System32.
            let system32 = std::env::var_os("SystemRoot")
                .map(|r| PathBuf::from(r).join("System32"))
                .unwrap_or_else(|| PathBuf::from(r"C:\Windows\System32"));
            if let Some(path) = std::env::var_os("PATH") {
                for dir in std::env::split_paths(&path) {
                    if dir == system32 {
                        continue;
                    }
                    let candidate = dir.join("bash.exe");
                    if candidate.is_file() {
                        return candidate;
                    }
                }
            }

            PathBuf::from("bash")
        })
        .clone()
}

/// Spawn `<git-bash> -c "<cmd>"`.
///
/// Windows runs commands through Git Bash, not `cmd.exe`, so the shell is the
/// same POSIX shell on every platform. That is what makes the documented
/// buffer-query workflow (`grep PATTERN @cmd_abc`, `tail -20 @cmd_xyz`) and the
/// Iron Law 3 pipeline rules actually executable here — under `cmd.exe` none of
/// those binaries exist, so the guidance codescout ships was unrunnable on
/// Windows.
///
/// `MSYS_NO_PATHCONV` / `MSYS2_ARG_CONV_EXCL` disable MSYS's argument
/// mangling. Without them the runtime rewrites anything that looks like a Unix
/// path into a Windows one *inside* the `-c` script, which silently corrupts
/// commands such as `sed 's/a/b/'` or `find / -name x`.
///
/// Sets `GIT_PAGER=cat`; the caller sets cwd, stdio, and kill_on_drop. stdin
/// defaults to null (prevents inherited-pipe / REPL hangs on the MCP stdio
/// server); callers needing real stdin (interactive mode) override it.
pub fn shell_command_configured(cmd: &str) -> tokio::process::Command {
    let mut std_cmd = std::process::Command::new(git_bash_path());
    std_cmd
        .arg("-c")
        .arg(cmd)
        .env("GIT_PAGER", "cat")
        .env("MSYS_NO_PATHCONV", "1")
        .env("MSYS2_ARG_CONV_EXCL", "*")
        .stdin(std::process::Stdio::null());
    tokio::process::Command::from(std_cmd)
}

/// POSIX tokenization — Windows executes through Git Bash, so the security
/// layer must read commands with the same rules the shell will apply. The
/// previous cmd-style tokenizer (double quotes only, no escapes) no longer
/// matches the executing shell.
pub fn shell_tokenize(cmd: &str) -> Result<Vec<String>, String> {
    super::posix_tokenize(cmd)
}

/// RAII owner of a Windows Job Object created with
/// `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. Dropping it closes the last handle to
/// the job, which terminates **every** process still assigned to it.
///
/// This is the Windows answer to the Unix process-group kill: `kill_on_drop`
/// only terminates the direct child, so a shell's own children survive it. With
/// `bash -c "sleep 5 && touch x"`, cancelling killed `bash` while `sleep` ran on
/// to completion and `touch` still fired.
pub struct JobGuard(windows_sys::Win32::Foundation::HANDLE);

// SAFETY: a job handle is a process-wide kernel handle with no thread affinity.
// The handle is owned solely by this guard and closed exactly once, in `Drop`.
unsafe impl Send for JobGuard {}
// SAFETY: as above — the guard exposes no interior mutability, and `Drop` is the
// only operation that touches the handle.
unsafe impl Sync for JobGuard {}

impl Drop for JobGuard {
    fn drop(&mut self) {
        // SAFETY: `self.0` is a live job handle from `CreateJobObjectW` (checked
        // non-null at construction) and is closed exactly once here.
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

/// Put `child` in a kill-on-close job so the whole tree dies with the guard.
///
/// Returns `None` — leaving the previous kill-the-direct-child-only behaviour
/// in place — when the job cannot be created or assigned. Assignment can
/// legitimately fail when the process is already in a job that forbids nesting,
/// so this is a best-effort hardening, never a hard requirement for spawning.
pub fn kill_on_close_job(child: &tokio::process::Child) -> Option<JobGuard> {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    let raw = child.raw_handle()? as HANDLE;

    // SAFETY: every handle is null-checked; `info` is a fully-initialised
    // out-param of exactly the size passed to SetInformationJobObject; the job
    // handle is closed on every early-return path and otherwise transferred to
    // the returned JobGuard.
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return None;
        }

        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let set = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            std::ptr::addr_of!(info) as *const core::ffi::c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        if set == 0 {
            CloseHandle(job);
            return None;
        }

        if AssignProcessToJobObject(job, raw) == 0 {
            CloseHandle(job);
            return None;
        }

        Some(JobGuard(job))
    }
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
