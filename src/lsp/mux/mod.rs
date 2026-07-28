#[cfg(unix)]
pub mod process;
pub mod protocol;

#[cfg(test)]
mod coherence_rust;
#[cfg(test)]
pub(crate) mod test_support;

use std::path::{Path, PathBuf};

use crate::socket_discovery::per_user_runtime_dir as per_user_mux_dir;
pub use crate::socket_discovery::workspace_hash;

/// Directory holding mux sockets and lock files.
///
/// In production this is `per_user_mux_dir()` directly. In test builds it is a
/// scratch subdirectory of it, one per test process, and dirs belonging to exited
/// test processes are swept on first use.
///
/// The seam is `cfg(test)` rather than a parameter because the callers that
/// create these files are *production* code: 17 lib tests reach `claim_mux_lock`
/// via `LspManager::get_or_start` -> `get_or_start_via_mux`, with a `TempDir` as
/// the workspace root. `workspace_hash` of a fresh temp dir is unique per run, so
/// every run minted new filenames and nothing was ever reused — measured
/// 2026-07-28 at +17 files per `cargo test --lib`, 468 accumulated. There is no
/// argument to thread through without pushing a test concern into the LSP
/// manager's public surface, and an env-var override is off the table (`set_var`
/// in a parallel test runner is the unsoundness this project has already fixed
/// twice). See docs/issues/2026-07-28-index-lock-tests-pollute-runtime-dir.md.
///
/// Nested INSIDE `per_user_mux_dir()` rather than in bare `temp_dir()` so the
/// `0o700` protection still applies — a predictable path in world-writable `/tmp`
/// lets a local user pre-create it and hold the flock.
///
/// Relocation alone only *contained* the leak: the shared directory stopped being
/// polluted (which was the harm — 203 files buried the one real lock during a live
/// diagnostic), but each run still left a dir holding 17 files. Hence the sweep,
/// which bounds the total to the number of concurrently running test processes.
///
/// Note this covers unit tests only. `tests/*.rs` link the lib built without
/// `cfg(test)` and so still use the real directory; measured at ~1 file per full
/// `cargo test`, versus 17 from the lib.
fn mux_dir() -> PathBuf {
    #[cfg(test)]
    {
        static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
        DIR.get_or_init(|| {
            let base = per_user_mux_dir();
            sweep_dead_test_mux_dirs(&base);
            let d = base.join(format!("{TEST_MUX_PREFIX}{}", std::process::id()));
            let _ = std::fs::create_dir_all(&d);
            d
        })
        .clone()
    }
    #[cfg(not(test))]
    {
        per_user_mux_dir()
    }
}

#[cfg(test)]
const TEST_MUX_PREFIX: &str = "codescout-test-mux-";

/// Remove scratch dirs whose owning test process has exited.
///
/// This is the one place that reads the runtime directory, and it is deliberately
/// NOT discovery: it resolves nothing and no caller depends on what it finds. A
/// future blast-radius scout greps for `read_dir` to check whether anything locates
/// mux files by scanning (see docs/trackers/reconnaissance-patterns.md R-45) — this
/// hit is cleanup, so relocating the directory again stays safe.
///
/// Only dirs matching `TEST_MUX_PREFIX` + a parseable PID that is neither ours nor
/// alive are removed, so a concurrent `cargo test` is never disturbed. PID reuse
/// could in principle target a live process's dir; the blast radius is one test
/// run's throwaway mux scratch files, and `pid_max` makes wraparound within a
/// session unlikely. Best-effort throughout — a failed sweep must never fail a test.
#[cfg(all(test, unix))]
fn sweep_dead_test_mux_dirs(base: &Path) {
    let me = std::process::id();
    let Ok(entries) = std::fs::read_dir(base) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(pid) = name
            .strip_prefix(TEST_MUX_PREFIX)
            .and_then(|rest| rest.parse::<u32>().ok())
        else {
            continue;
        };
        if pid == me || pid_is_alive(pid) {
            continue;
        }
        let _ = std::fs::remove_dir_all(entry.path());
    }
}

/// `kill(pid, 0)` probes existence without signalling. `EPERM` means the process
/// exists but is not ours, so it counts as alive — never sweep it.
#[cfg(all(test, unix))]
fn pid_is_alive(pid: u32) -> bool {
    // SAFETY: `kill` with signal 0 performs permission/existence checks only and
    // delivers nothing. Any pid value is a valid argument.
    let rc = unsafe { libc::kill(pid as libc::pid_t, 0) };
    rc == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// No portable `kill(pid, 0)` equivalent here, so skip the sweep rather than risk
/// removing a live process's scratch dir. The per-process dirs still keep the
/// shared directory clean.
#[cfg(all(test, not(unix)))]
fn sweep_dead_test_mux_dirs(_base: &Path) {}

pub fn socket_path_for_workspace(language: &str, workspace_root: &Path) -> PathBuf {
    mux_dir().join(format!(
        "codescout-{}-mux-{}.sock",
        language,
        workspace_hash(workspace_root)
    ))
}

pub fn lock_path_for_workspace(language: &str, workspace_root: &Path) -> PathBuf {
    mux_dir().join(format!(
        "codescout-{}-mux-{}.lock",
        language,
        workspace_hash(workspace_root)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_path_deterministic_for_same_workspace() {
        let p1 = socket_path_for_workspace("kotlin", Path::new("/home/user/project"));
        let p2 = socket_path_for_workspace("kotlin", Path::new("/home/user/project"));
        assert_eq!(p1, p2);

        let p3 = socket_path_for_workspace("kotlin", Path::new("/home/user/other"));
        assert_ne!(p1, p3);
    }

    #[test]
    fn different_languages_get_different_paths() {
        let p1 = socket_path_for_workspace("kotlin", Path::new("/project"));
        let p2 = socket_path_for_workspace("java", Path::new("/project"));
        assert_ne!(p1, p2);
    }

    /// Build-agnostic invariants for the `mux_dir` seam. Asserting "no files
    /// appeared in the real runtime dir" by counting would be flaky — a
    /// concurrent `cargo test` or a live MCP server can add one at any moment.
    ///
    /// The socket and lock for one workspace MUST share a parent: `claim_mux_lock`
    /// takes the lock and then the mux binds the socket, and
    /// `get_or_start_via_mux` treats "lock held but socket absent" as the wedged
    /// state. Split them across directories and that diagnosis breaks.
    #[test]
    fn socket_and_lock_share_a_parent_inside_the_per_user_dir() {
        let ws = Path::new("/some/workspace");
        let sock = socket_path_for_workspace("rust", ws);
        let lock = lock_path_for_workspace("rust", ws);
        assert_eq!(
            sock.parent(),
            lock.parent(),
            "socket and lock must live in the same directory"
        );

        // Holds in both builds: production returns the dir itself, test builds a
        // child of it. Fails if the seam is ever re-sited to bare temp_dir().
        let parent = sock.parent().expect("socket path has a parent");
        assert!(
            parent.starts_with(per_user_mux_dir()),
            "mux dir must stay inside the per-user runtime dir, got {}",
            parent.display()
        );
    }

    /// In test builds the mux dir must NOT be the shared per-user dir itself —
    /// that is the leak this seam closes. Compiled only under `cfg(test)`, which
    /// is exactly when the redirect is supposed to be active.
    #[test]
    fn test_builds_redirect_the_mux_dir_away_from_the_shared_one() {
        let parent = socket_path_for_workspace("rust", Path::new("/w"))
            .parent()
            .unwrap()
            .to_path_buf();
        assert_ne!(
            parent,
            per_user_mux_dir(),
            "test builds must not write mux files into the shared per-user dir"
        );
    }
}
