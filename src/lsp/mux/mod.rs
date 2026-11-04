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
/// twice). See docs/issues/archive/2026-07-28-index-lock-tests-pollute-runtime-dir.md.
///
/// Nested INSIDE `per_user_mux_dir()` rather than in bare `temp_dir()` so the
/// `0o700` protection still applies — a predictable path in world-writable `/tmp`
/// lets a local user pre-create it and hold the flock.
///
/// When that per-user dir is too long for a socket path to bind below it (`TMPDIR` is long and
/// `XDG_RUNTIME_DIR` is unset), the scratch dir nests in the same private dir rooted at `/tmp`
/// instead: see `test_mux_base` and bug 98379dd228578791.
///
/// Relocation alone only *contained* the leak: the shared directory stopped being
/// polluted (which was the harm — 203 files buried the one real lock during a live
/// diagnostic), but each run still left a dir holding 17 files. Hence the sweep,
/// which bounds the total to the number of concurrently running test processes.
///
/// Note this covers unit tests only. `tests/*.rs` link the lib built without
/// `cfg(test)` and so still use the real directory; measured at ~1 file per full
/// `cargo test`, versus 17 from the lib.
fn mux_dir() -> Result<PathBuf, crate::socket_discovery::RuntimeDirError> {
    #[cfg(test)]
    {
        static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
        if let Some(dir) = DIR.get() {
            return Ok(dir.clone());
        }
        let base = test_mux_base()?;
        Ok(DIR
            .get_or_init(|| {
                sweep_dead_test_mux_dirs(&base);
                let d = base.join(format!("{TEST_MUX_PREFIX}{}", std::process::id()));
                let _ = std::fs::create_dir_all(&d);
                d
            })
            .clone())
    }
    #[cfg(not(test))]
    {
        per_user_mux_dir()
    }
}

#[cfg(test)]
// Short on purpose: a macOS `sockaddr_un` holds 103 bytes, and the CI runner's temp directory plus
// `codescout-501/`, this directory and `codescout-rust-mux-<16 hex>.sock` made 104 (run
// 41344511357: six `lsp::manager` tests failed with "path must be shorter than SUN_LEN").
const TEST_MUX_PREFIX: &str = "cs-tmux-";

/// Bytes a socket path may hold. macOS `sockaddr_un` is the smaller platform limit (103 bytes
/// plus the NUL), so it is the one a test must respect.
// cap-class: NOT_A_CAP — a byte limit on a test fixture's socket path; production code never reads it and no tool result depends on it
#[cfg(test)]
const TEST_SOCKET_BYTES_MAX: usize = 103;

/// The longest per-user dir under which every socket path a test builds still binds. The rest
/// of the path is `/` + [`TEST_MUX_PREFIX`] + a pid of up to 10 digits + `/` +
/// `codescout-<language>-mux-<16 hex>.sock`, with `typescript` as the longest language.
///
/// Measured 2026-11-04 (bug 98379dd228578791): with `XDG_RUNTIME_DIR` unset and a 41-byte
/// `TMPDIR`, the fallback `<TMPDIR>/codescout-<uid>` made seven `lsp::manager` tests fail with
/// "path must be shorter than SUN_LEN". The same failure hit a macOS runner (CI run
/// 41344511357); the prefix was shortened then, which moved the threshold and did not remove it.
// cap-class: NOT_A_CAP — a byte budget for a test fixture's directory; it picks which directory a test builds in and never shapes a tool result
#[cfg(test)]
const TEST_MUX_BASE_MAX: usize = TEST_SOCKET_BYTES_MAX
    - (1 + TEST_MUX_PREFIX.len()
        + 10
        + 1
        + "codescout-".len()
        + 10
        + "-mux-".len()
        + 16
        + ".sock".len());

/// Whether `base` leaves room for the longest test socket path.
#[cfg(test)]
fn test_mux_base_fits(base: &Path) -> bool {
    base.as_os_str().len() <= TEST_MUX_BASE_MAX
}

/// The directory a test build nests its per-process scratch dir in: the per-user runtime dir
/// when a socket path below it fits, else the same private dir rooted at `/tmp`.
#[cfg(test)]
fn test_mux_base() -> Result<PathBuf, crate::socket_discovery::RuntimeDirError> {
    let per_user = per_user_mux_dir()?;
    #[cfg(unix)]
    if !test_mux_base_fits(&per_user) {
        return crate::socket_discovery::short_per_user_runtime_dir();
    }
    Ok(per_user)
}

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

pub fn socket_path_for_workspace(
    language: &str,
    workspace_root: &Path,
) -> Result<PathBuf, crate::socket_discovery::RuntimeDirError> {
    Ok(mux_dir()?.join(format!(
        "codescout-{}-mux-{}.sock",
        language,
        workspace_hash(workspace_root)
    )))
}

pub fn lock_path_for_workspace(
    language: &str,
    workspace_root: &Path,
) -> Result<PathBuf, crate::socket_discovery::RuntimeDirError> {
    Ok(mux_dir()?.join(format!(
        "codescout-{}-mux-{}.lock",
        language,
        workspace_hash(workspace_root)
    )))
}
/// Where the mux records that it killed an LSP for memory, for the manager to find.
///
/// The kill happens in the **mux process**; the respawn is driven by `LspManager` in
/// the codescout process. Without a marker between them the manager only ever sees a
/// *successful* start — which resets the circuit breaker — so kill → respawn → grow →
/// kill repeats unthrottled. A file is the signal because the two sides share no
/// channel that survives the mux exiting, which is exactly what a mem-kill causes.
///
/// Derived from the lock path rather than recomputed from `(language, workspace)`, so
/// both sides cannot drift apart: the mux has only the lock path, and a second
/// `*_for_workspace` helper would be a second place to get the hash wrong.
pub fn memkill_path_for_lock(lock_path: &Path) -> PathBuf {
    lock_path.with_extension("memkill")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_path_deterministic_for_same_workspace() {
        let p1 = socket_path_for_workspace("kotlin", Path::new("/home/user/project")).unwrap();
        let p2 = socket_path_for_workspace("kotlin", Path::new("/home/user/project")).unwrap();
        assert_eq!(p1, p2);

        let p3 = socket_path_for_workspace("kotlin", Path::new("/home/user/other")).unwrap();
        assert_ne!(p1, p3);
    }

    #[test]
    fn different_languages_get_different_paths() {
        let p1 = socket_path_for_workspace("kotlin", Path::new("/project")).unwrap();
        let p2 = socket_path_for_workspace("java", Path::new("/project")).unwrap();
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
        let sock = socket_path_for_workspace("rust", ws).unwrap();
        let lock = lock_path_for_workspace("rust", ws).unwrap();
        assert_eq!(
            sock.parent(),
            lock.parent(),
            "socket and lock must live in the same directory"
        );

        // Holds in both builds: production returns the dir itself, test builds a
        // child of it. Fails if the seam is ever re-sited to bare temp_dir(). A test
        // build whose per-user dir is too long for a socket path uses the same private
        // dir rooted at /tmp, so the base is `test_mux_base()`, not `per_user_mux_dir()`.
        let parent = sock.parent().expect("socket path has a parent");
        assert!(
            parent.starts_with(test_mux_base().unwrap()),
            "mux dir must stay inside a private per-user runtime dir, got {}",
            parent.display()
        );
    }

    /// In test builds the mux dir must NOT be the shared per-user dir itself —
    /// that is the leak this seam closes. Compiled only under `cfg(test)`, which
    /// is exactly when the redirect is supposed to be active.
    #[test]
    fn test_builds_redirect_the_mux_dir_away_from_the_shared_one() {
        let parent = socket_path_for_workspace("rust", Path::new("/w"))
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        assert_ne!(
            parent,
            per_user_mux_dir().unwrap(),
            "test builds must not write mux files into the shared per-user dir"
        );
    }

    /// A per-user dir that leaves room for the longest test socket path is used as it is.
    #[test]
    fn a_per_user_dir_with_room_for_the_longest_test_socket_path_is_kept() {
        assert!(test_mux_base_fits(Path::new("/run/user/1000")));
        let at_the_limit = PathBuf::from(format!("/{}", "a".repeat(TEST_MUX_BASE_MAX - 1)));
        assert_eq!(at_the_limit.as_os_str().len(), TEST_MUX_BASE_MAX);
        assert!(test_mux_base_fits(&at_the_limit));
    }

    /// One byte over the budget and the test scratch dir must move to a short root, because
    /// the bind would fail with "path must be shorter than SUN_LEN" (bug 98379dd228578791).
    #[test]
    fn a_per_user_dir_one_byte_over_the_budget_is_not_used() {
        let over = PathBuf::from(format!("/{}", "a".repeat(TEST_MUX_BASE_MAX)));
        assert_eq!(over.as_os_str().len(), TEST_MUX_BASE_MAX + 1);
        assert!(!test_mux_base_fits(&over));
    }

    /// The budget is derived from the longest path a test builds, so a path at the budget
    /// still binds. A pid has at most 10 digits and `typescript` is the longest language.
    #[test]
    fn the_budget_keeps_the_longest_test_socket_path_within_the_bind_limit() {
        let longest = TEST_MUX_BASE_MAX
            + "/".len()
            + TEST_MUX_PREFIX.len()
            + u32::MAX.to_string().len()
            + "/".len()
            + "codescout-typescript-mux-".len()
            + workspace_hash(Path::new("/w")).len()
            + ".sock".len();
        assert_eq!(longest, TEST_SOCKET_BYTES_MAX);
    }

    /// Longest `TMPDIR`, in bytes, under which a PRODUCTION socket still binds when
    /// `XDG_RUNTIME_DIR` is unset: `<TMPDIR>/codescout-<uid>/<socket file name>`. `TMPDIR` is
    /// counted without a trailing slash. The file name comes from the real
    /// `socket_path_for_workspace`, so a change to its format changes the answer.
    fn longest_tmpdir_that_binds(language: &str, uid_digits: usize, limit: usize) -> usize {
        let file = socket_path_for_workspace(language, Path::new("/w"))
            .unwrap()
            .file_name()
            .unwrap()
            .len();
        let dir = "codescout-".len() + uid_digits;
        // TMPDIR + "/" + dir + "/" + file must not exceed the limit.
        limit - (1 + dir + 1 + file)
    }

    /// Characterization, not a requirement: production has no length check, so every language
    /// has a `TMPDIR` length above which its mux cannot bind (bug 98379dd228578791).
    ///
    /// - Linux, uid 1000, 107 bytes. Measured 2026-11-04 with the real `codescout mux` and
    ///   `XDG_RUNTIME_DIR` unset: for `rust` a 51-byte `TMPDIR` bound and a 52-byte one failed
    ///   with `failed to bind socket`, which is the first row of this table.
    /// - macOS, uid 501, 103 bytes. Computed, not measured: no Mac was available. The per-user
    ///   temp dir there is typically `/var/folders/xx/<30 characters>/T`, about 48 bytes, which
    ///   would leave `python`, `kotlin`, `csharp`, `typescript` and `javascript` over the limit.
    ///   That shape is recalled, not read from a Mac.
    ///
    /// If this fails after a change to the socket path layout, re-derive the table and decide
    /// whether the new thresholds are acceptable; do not just edit the numbers.
    #[test]
    fn the_longest_tmpdir_that_binds_a_production_socket_is_pinned_per_language() {
        const LANGUAGES: [&str; 18] = [
            "rust",
            "python",
            "typescript",
            "javascript",
            "tsx",
            "jsx",
            "go",
            "java",
            "kotlin",
            "c",
            "cpp",
            "csharp",
            "ruby",
            "html",
            "css",
            "scss",
            "less",
            "bash",
        ];
        let table: Vec<(&str, usize, usize)> = LANGUAGES
            .iter()
            .map(|l| {
                (
                    *l,
                    longest_tmpdir_that_binds(l, 3, 103),
                    longest_tmpdir_that_binds(l, 4, 107),
                )
            })
            .collect();
        assert_eq!(
            table,
            vec![
                // (language, macOS uid 501 / 103 bytes, Linux uid 1000 / 107 bytes)
                ("rust", 48, 51),
                ("python", 46, 49),
                ("typescript", 42, 45),
                ("javascript", 42, 45),
                ("tsx", 49, 52),
                ("jsx", 49, 52),
                ("go", 50, 53),
                ("java", 48, 51),
                ("kotlin", 46, 49),
                ("c", 51, 54),
                ("cpp", 49, 52),
                ("csharp", 46, 49),
                ("ruby", 48, 51),
                ("html", 48, 51),
                ("css", 49, 52),
                ("scss", 48, 51),
                ("less", 48, 51),
                ("bash", 48, 51),
            ]
        );
    }
}
