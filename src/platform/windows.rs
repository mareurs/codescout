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

/// Resolve the Git Bash executable from an injected environment + existence probe.
///
/// Split out of [`git_bash_path`] so the not-installed branch is reachable in a
/// test. It is the branch that matters: it is what a machine without Git for
/// Windows hits, and the only way to observe it through the real resolver is to
/// uninstall git.
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
fn resolve_git_bash(
    env: impl Fn(&str) -> Option<std::ffi::OsString>,
    is_file: impl Fn(&std::path::Path) -> bool,
) -> Option<std::path::PathBuf> {
    use std::path::PathBuf;

    for var in ["CODESCOUT_BASH", "CLAUDE_CODE_GIT_BASH_PATH"] {
        if let Some(p) = env(var).map(PathBuf::from) {
            if is_file(&p) {
                return Some(p);
            }
        }
    }

    let roots = [
        env("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Programs")),
        env("ProgramFiles").map(PathBuf::from),
        env("ProgramFiles(x86)").map(PathBuf::from),
    ];
    for root in roots.into_iter().flatten() {
        let candidate = root.join("Git").join("bin").join("bash.exe");
        if is_file(&candidate) {
            return Some(candidate);
        }
    }

    // PATH last, and never the WSL launcher under System32.
    //
    // Compared with `windows_dir_eq`, never `==`: Windows setup writes the
    // system PATH entry as `C:\Windows\system32` while the join below produces
    // `C:\Windows\System32`, and `Path` equality is byte-exact on normal
    // components (only the drive-letter prefix folds case). A plain `==` here
    // therefore never matched on a real host, and this exclusion — the
    // load-bearing step of the whole resolution order — did nothing at all.
    let system32 = env("SystemRoot")
        .map(|r| PathBuf::from(r).join("System32"))
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows\System32"));
    if let Some(path) = env("PATH") {
        for dir in std::env::split_paths(&path) {
            if super::windows_dir_eq(&dir, &system32) {
                continue;
            }
            let candidate = dir.join("bash.exe");
            if is_file(&candidate) {
                return Some(candidate);
            }
        }
    }

    None
}

/// Resolve the Git Bash executable, cached for the process lifetime.
///
/// `None` means no Git Bash is installed — see [`resolve_git_bash`] for the
/// resolution order. Callers that must produce a `Command` anyway fall back to
/// the bare name `bash`, so the failure surfaces as a spawn error naming the
/// program rather than a panic at startup; [`shell_unavailable_hint`] is the
/// surface that turns it into an actionable message before we get there.
fn git_bash_path() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;
    use std::sync::OnceLock;

    static RESOLVED: OnceLock<Option<PathBuf>> = OnceLock::new();
    RESOLVED
        .get_or_init(|| resolve_git_bash(|var| std::env::var_os(var), |p| p.is_file()))
        .clone()
}

/// `Some(hint)` when no POSIX shell is available to spawn commands through.
///
/// Windows runs every command through Git Bash. Without it, `CreateProcessW`
/// answers `program not found`, which names neither the requirement nor the fix
/// — a whole CI job's worth of failures said only that. The hint is consumed by
/// `run_command`, which turns it into a `RecoverableError` before any spawn.
pub fn shell_unavailable_hint() -> Option<String> {
    if git_bash_path().is_some() {
        return None;
    }
    Some(
        "Install Git for Windows (which provides Git Bash), or point codescout at an \
         existing bash.exe with CODESCOUT_BASH=<path-to-bash.exe>. codescout runs Windows \
         commands through Git Bash so that the documented buffer-query workflow \
         (grep/tail on @cmd_* refs) and one POSIX tokenizer for the security layer hold \
         on every platform."
            .to_string(),
    )
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
/// `MSYS_NO_PATHCONV` / `MSYS2_ARG_CONV_EXCL` are explicitly *removed*, not set.
/// MSYS argument conversion is what lets a **native** (non-MSYS) binary such as
/// `git.exe` accept an MSYS-form path: `git -C /c/work/repo` only works because
/// the runtime rewrites the argument to `C:/work/repo` before `git` sees it.
/// Disabling conversion breaks every native binary taking a path argument. It
/// does *not* protect `sed 's/a/b/'` or `find / -name x` — `sed` and `find` in
/// Git Bash are themselves MSYS binaries, and MSYS never converts arguments
/// passed between MSYS programs, so there is nothing there to protect against.
/// Removing the variables (rather than just declining to set them) keeps the
/// shell deterministic when the parent process happens to export them, for the
/// same reason `GIT_PAGER` is pinned.
/// See `docs/issues/archive/2026-08-07-msys-pathconv-optout-breaks-native-exe-paths.md`.
///
/// Sets `GIT_PAGER=cat`; the caller sets cwd, stdio, and kill_on_drop. stdin
/// defaults to null (prevents inherited-pipe / REPL hangs on the MCP stdio
/// server); callers needing real stdin (interactive mode) override it.
pub fn shell_command_configured(cmd: &str) -> tokio::process::Command {
    let bash = git_bash_path().unwrap_or_else(|| std::path::PathBuf::from("bash"));
    let mut std_cmd = std::process::Command::new(bash);
    std_cmd
        .arg("-c")
        .arg(cmd)
        .env("GIT_PAGER", "cat")
        .env_remove("MSYS_NO_PATHCONV")
        .env_remove("MSYS2_ARG_CONV_EXCL")
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
        let mut child = std::process::Command::new("cmd")
            .args(["/C", "ping -n 30 127.0.0.1 >nul"])
            .spawn()
            .unwrap();
        let pid = child.id();
        assert!(process_alive(pid), "sleeper should be alive");
        terminate_process(pid).unwrap();
        // Give the OS a moment to reap the terminated process.
        std::thread::sleep(std::time::Duration::from_millis(300));
        let alive_after_terminate = process_alive(pid);
        // Reap before asserting: an unwaited `Child` holds the process handle open
        // for the rest of the test binary's life. Liveness is sampled first so the
        // assertion still observes exactly what it did before this reap was added.
        let _ = child.wait();
        assert!(
            !alive_after_terminate,
            "sleeper should be dead after terminate"
        );
    }

    #[test]
    fn win32_liveness_false_for_dead_pid() {
        // A PID that almost certainly does not exist.
        assert!(!process_alive(0xFFFF_FFF0));
    }

    /// The not-installed branch: no override, no install root, nothing on PATH.
    ///
    /// This is the branch a Git-less Windows box hits, and the one the wine CI
    /// job hits — 22 lib tests there failed with a bare `program not found`,
    /// which named neither the requirement nor the fix. The real resolver
    /// cannot reach it on a developer machine without uninstalling git, so the
    /// probe is injected.
    #[test]
    fn resolve_git_bash_is_none_when_nothing_is_installed() {
        let resolved = resolve_git_bash(
            |var| match var {
                "PATH" => Some(std::ffi::OsString::from(r"C:\Windows\System32")),
                _ => None,
            },
            |_| false,
        );
        assert!(
            resolved.is_none(),
            "a host with no bash.exe anywhere must resolve to None, got {resolved:?}"
        );
    }

    /// The System32 exclusion, asserted in every spelling Windows produces.
    ///
    /// A `bash.exe` under System32 is the WSL launcher and must be rejected
    /// even though the existence probe says yes. The case-folding half of that
    /// is pinned cross-platform by `windows_dir_eq_folds_case_and_separators`
    /// in `platform::mod`; this pins the resolver actually using it.
    #[test]
    fn resolve_git_bash_never_selects_the_wsl_launcher() {
        // Every spelling a real host produces. The lowercase one is the whole
        // point: `%SystemRoot%`.join("System32") is capitalized while Windows
        // setup writes the PATH entry lowercase, and the original `==`
        // compared them byte-exactly. A capitalized-only fixture passes
        // against a guard that does nothing.
        for path_spelling in [
            r"C:\Windows\system32",
            r"C:\Windows\System32",
            r"C:\WINDOWS\SYSTEM32\",
            "C:/Windows/system32",
        ] {
            let resolved = resolve_git_bash(
                |var| match var {
                    "SystemRoot" => Some(std::ffi::OsString::from(r"C:\Windows")),
                    "PATH" => Some(std::ffi::OsString::from(path_spelling)),
                    _ => None,
                },
                // Every probe answers yes, so only the exclusion can reject.
                |_| true,
            );
            assert!(
                resolved.is_none(),
                "System32 bash.exe is the WSL launcher and must never be selected; \
                 PATH spelled {path_spelling:?} resolved to {resolved:?}"
            );
        }
    }

    /// Positive control for the exclusion: it must reject System32 and nothing
    /// else.
    ///
    /// Without this, `resolve_git_bash_never_selects_the_wsl_launcher` is also
    /// satisfied by a resolver that stopped returning anything from `PATH` at
    /// all — "never selects the wrong one" and "never selects one" are the same
    /// assertion until something pins the accepting case.
    #[test]
    fn resolve_git_bash_still_accepts_a_non_system32_path_entry() {
        let resolved = resolve_git_bash(
            |var| match var {
                "SystemRoot" => Some(std::ffi::OsString::from(r"C:\Windows")),
                "PATH" => Some(std::ffi::OsString::from(r"C:\tools\git\bin")),
                _ => None,
            },
            |_| true,
        );
        assert_eq!(
            resolved,
            Some(std::path::PathBuf::from(r"C:\tools\git\bin").join("bash.exe")),
            "a PATH entry that is not System32 must still resolve"
        );
    }

    /// The override wins, and it is the documented escape hatch named by
    /// [`shell_unavailable_hint`].
    #[test]
    fn resolve_git_bash_honours_the_codescout_bash_override() {
        let target = std::path::Path::new(r"D:\tools\bash.exe");
        let resolved = resolve_git_bash(
            |var| match var {
                "CODESCOUT_BASH" => Some(std::ffi::OsString::from(target)),
                _ => None,
            },
            |p| p == target,
        );
        assert_eq!(resolved.as_deref(), Some(target));
    }

    /// Regression: `MSYS_NO_PATHCONV=1` / `MSYS2_ARG_CONV_EXCL=*` must never
    /// reach the shell. They disable the argument rewriting that lets a native
    /// binary accept an MSYS-form path, so `git -C /c/...` dies with
    /// `cannot change to '/c/...': No such file or directory`.
    ///
    /// This asserts on the *native* side of the boundary on purpose. A test
    /// that only ran MSYS builtins (`ls /c/...`) would pass either way — MSYS
    /// programs resolve MSYS paths themselves and never see the conversion.
    /// See `docs/issues/archive/2026-08-07-msys-pathconv-optout-breaks-native-exe-paths.md`.
    #[tokio::test]
    async fn msys_form_path_resolves_for_native_binaries() {
        let dir = std::env::temp_dir();
        let slashed = dir.to_string_lossy().replace('\\', "/");
        let (drive, rest) = slashed
            .split_once(":/")
            .expect("temp dir should be drive-qualified");
        let msys = format!("/{}/{}", drive.to_ascii_lowercase(), rest);
        assert!(
            msys.starts_with('/'),
            "probe path must be MSYS-form, got {msys}"
        );

        let out = shell_command_configured(&format!("git -C '{msys}' rev-parse --git-dir"))
            .output()
            .await
            .expect("git should be spawnable — Git Bash implies a git install");
        let stderr = String::from_utf8_lossy(&out.stderr);

        // `git` may still fail with "not a git repository" (temp dir usually
        // isn't one). That is fine: it proves git resolved and entered the
        // directory. Only the chdir failure indicates conversion was disabled.
        assert!(
            !stderr.contains("cannot change to"),
            "git could not resolve MSYS-form path {msys}: MSYS argument \
             conversion is disabled. stderr: {stderr}"
        );
    }
}
