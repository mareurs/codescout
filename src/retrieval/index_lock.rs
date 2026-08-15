//! Per-project exclusive lock for the retrieval index pass.
//!
//! Without it, N concurrent `codescout index --project <same>` runs each execute
//! the full `stream_index` pipeline against the same Qdrant collection and
//! `project_id`, duplicating the entire embedding workload. Observed 2026-07-27
//! with four simultaneous runs (3h24m / 2h02m / 1h08m / 1h05m), all orphaned to
//! `systemd --user`. See docs/issues/archive/2026-07-25-concurrent-index-no-project-lock.md
//!
//! Deliberately NOT `.codescout/write.lock`: that lock is taken per write-tool
//! call by `crate::agent::write_guard::WriteGuard`. An index holding it for hours
//! would block every edit tool for the duration.
//!
//! Keyed on `project_id` rather than on the filesystem root, and stored outside
//! any repository: the contended resource is the `(collection, project_id)` pair
//! in Qdrant, and library syncs pass a third-party checkout as `root` that must
//! not gain a `.codescout/` directory.

use anyhow::{Context, Result};
use fs4::fs_std::FileExt;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::path::{Path, PathBuf};

/// RAII handle for the per-project index lock.
///
/// The `flock` is released on drop, and by the kernel if the process dies — so a
/// leftover lock file is inert and needs no recovery logic.
#[derive(Debug)]
pub struct IndexLock {
    file: File,
    path: PathBuf,
}

impl IndexLock {
    /// Filesystem path this lock occupies. For diagnostics and tests.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for IndexLock {
    fn drop(&mut self) {
        // Explicit unlock documents intent; closing the fd would also release it.
        let _ = FileExt::unlock(&self.file);
    }
}

/// Deterministic lock-file path for `project_id`, sited in `dir`.
///
/// Split out of [`lock_path`] purely as a test seam: the tests below must not
/// write into the real per-user runtime directory. Production has exactly one
/// caller — [`lock_path`] — which passes `per_user_runtime_dir()`.
pub fn lock_path_in(dir: &Path, project_id: &str) -> PathBuf {
    let mut h = Sha256::new();
    h.update(project_id.as_bytes());
    let digest = format!("{:x}", h.finalize());
    dir.join(format!("codescout-index-{}.lock", &digest[..16]))
}

/// Deterministic lock-file path for `project_id`.
///
/// Hashed so any `project_id` — including one with path separators or spaces —
/// maps to a safe, fixed-length filename.
///
/// Sited in the per-user runtime directory rather than bare `temp_dir()`: a
/// predictable path in world-writable `/tmp` lets a local user pre-create it as a
/// symlink (which `set_len(0)` below would then truncate) or simply hold the flock
/// to wedge every index run. `per_user_runtime_dir()` handles both platforms —
/// `0o700` dir on Unix, already-per-user `temp_dir()` on Windows.
pub fn lock_path(project_id: &str) -> PathBuf {
    lock_path_in(&crate::socket_discovery::per_user_runtime_dir(), project_id)
}

/// Acquire the exclusive index lock for `project_id` under `dir`, or fail
/// immediately.
///
/// Test seam, mirroring [`lock_path_in`]. Production goes through [`acquire`].
pub fn acquire_in(dir: &Path, project_id: &str) -> Result<IndexLock> {
    let path = lock_path_in(dir, project_id);

    // create(true) + truncate(false): `File::create` truncates on open, which
    // would erase the current holder's PID line before we even try to lock.
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .with_context(|| format!("failed to open index lock file: {}", path.display()))?;

    file.try_lock_exclusive().with_context(|| {
        format!(
            "another codescout index is already running for project '{project_id}' \
             (lock file: {} — its first line is the holder's PID). The holder may be \
             a CLI `codescout index` run OR an in-process background index (e.g. an \
             MCP server's auto-index task) — check the PID, don't assume `pgrep -af \
             'codescout index'` will show it.",
            path.display()
        )
    })?;

    // PID for diagnostics, mirroring src/lsp/mux/process.rs:81. Only after the
    // lock is held, so we never clobber another holder's record. Best-effort:
    // a failed write must not fail an otherwise-valid lock.
    use std::io::Write;
    let _ = file.set_len(0);
    let _ = writeln!(&file, "{}", std::process::id());

    Ok(IndexLock { file, path })
}

/// Acquire the exclusive index lock for `project_id`, or fail immediately.
///
/// Fail-fast rather than queue. A queued second run would be nearly free — every
/// `chunk_id` would already be present, so nothing re-embeds — but it would hide
/// the duplication instead of surfacing it, which is how this bug went unnoticed
/// for hours.
///
/// The lock file is deliberately never unlinked. Beyond the fd race that
/// unlink-on-drop would open (a second `acquire` can hold an fd on an inode a
/// third has already replaced, so both would believe they hold the lock), the
/// leftover file is a durable forensic record: it names the project via
/// `sha256(project_id)`, and its mtime dates the run. That is how a 28-minute
/// index of an unrelated project was identified on 2026-07-28 after the process
/// had already exited. See
/// docs/issues/archive/2026-07-28-index-lock-tests-pollute-runtime-dir.md.
pub fn acquire(project_id: &str) -> Result<IndexLock> {
    acquire_in(&crate::socket_discovery::per_user_runtime_dir(), project_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every test that actually creates a lock file goes through [`acquire_in`]
    /// with a fresh scratch directory.
    ///
    /// The previous approach — production `acquire` plus a `project_id` carrying
    /// the PID and thread id — isolated tests from each other but wrote into the
    /// real per-user runtime directory, and a per-run-unique id meant a
    /// per-run-unique filename, so nothing was ever reused or removed. Measured
    /// 2026-07-28: 7 leaked files per `cargo test`, 203 accumulated. A scratch
    /// dir gives strictly better isolation (a different directory, not merely a
    /// different name), so the ids below can be plain literals.
    /// See docs/issues/archive/2026-07-28-index-lock-tests-pollute-runtime-dir.md.
    fn scratch() -> tempfile::TempDir {
        tempfile::tempdir().expect("scratch dir for lock files")
    }

    #[test]
    fn acquire_succeeds_for_fresh_project() {
        let dir = scratch();
        let lock = acquire_in(dir.path(), "fresh").expect("first acquire must succeed");
        assert!(lock.path().exists(), "lock file should exist on disk");
    }

    #[test]
    fn second_acquire_fails_while_first_is_held() {
        let dir = scratch();
        let _first = acquire_in(dir.path(), "contend").expect("first acquire must succeed");
        let second = acquire_in(dir.path(), "contend");
        assert!(
            second.is_err(),
            "a second acquire for the same project_id must fail while the first is held"
        );
        let msg = format!("{:#}", second.unwrap_err());
        assert!(
            msg.contains("already running"),
            "error must tell the operator what is happening, got: {msg}"
        );
    }

    #[test]
    fn different_projects_do_not_contend() {
        // Same directory on purpose: the isolation under test is the hashed
        // filename, not the parent dir.
        let dir = scratch();
        let _lock_a = acquire_in(dir.path(), "proj-a").expect("project a");
        let _lock_b = acquire_in(dir.path(), "proj-b").expect("project b must not contend with a");
    }

    #[test]
    fn lock_is_released_on_drop() {
        let dir = scratch();
        {
            let _held = acquire_in(dir.path(), "release").expect("first acquire");
        } // drop releases
        acquire_in(dir.path(), "release").expect("must be re-acquirable after the guard drops");
    }

    /// A leftover lock *file* must never block a new run, and the PID write must
    /// TRUNCATE rather than overwrite in place.
    ///
    /// The planted content is shaped to kill two distinct mutations at once, which
    /// requires both properties in one value:
    ///   - it starts with our OWN live pid, so a PID-liveness check ("is the holder
    ///     still alive? then refuse") would refuse and fail this test. Planting a
    ///     dead pid like 999999 would NOT pin that — 999999 is above `pid_max` on
    ///     most Linux configs and reads as dead anyway.
    ///   - it is LONGER than what `acquire` writes, so deleting `set_len(0)` leaves a
    ///     visible tail. Planting only our pid would NOT pin that either: `acquire`
    ///     writes identical bytes at offset 0, making the truncate unobservable.
    #[test]
    fn preexisting_lock_file_does_not_block() {
        let dir = scratch();
        let path = lock_path_in(dir.path(), "stale");
        std::fs::write(
            &path,
            format!(
                "{}\nstale-tail-that-must-be-truncated\n",
                std::process::id()
            ),
        )
        .expect("simulate a lock file left by a dead process");

        let lock =
            acquire_in(dir.path(), "stale").expect("a stale lock file must not block acquisition");
        drop(lock);

        let contents = std::fs::read_to_string(&path).expect("read lock file");
        assert_eq!(
            contents.trim(),
            std::process::id().to_string(),
            "lock file must contain exactly the holder's pid, with no stale tail"
        );
    }

    /// The seam itself is the regression guard. Asserting "the real runtime dir
    /// gained no files" by counting would be flaky — a concurrent `cargo test`
    /// process, or a genuine index run, can add one at any moment. Asserting that
    /// THIS project's own path was never created is precise and stable.
    ///
    /// Fails if `acquire_in` is ever reverted to resolving the directory itself.
    #[test]
    fn acquire_in_does_not_touch_the_real_runtime_dir() {
        let dir = scratch();
        // Distinctive enough that no real project or other test collides with it.
        let project_id = "index-lock-seam-guard-must-never-reach-the-runtime-dir";
        let real = lock_path(project_id);
        assert!(
            !real.exists(),
            "precondition: {} must not pre-exist",
            real.display()
        );

        let lock = acquire_in(dir.path(), project_id).expect("acquire in scratch dir");
        assert_eq!(
            lock.path().parent(),
            Some(dir.path()),
            "the lock must be sited in the injected dir"
        );
        assert!(
            !real.exists(),
            "acquire_in must not create {} in the real runtime dir",
            real.display()
        );
    }

    #[test]
    fn lock_path_is_deterministic_and_filename_safe() {
        let a = lock_path("some/project:with weird*chars");
        let b = lock_path("some/project:with weird*chars");
        assert_eq!(a, b, "lock_path must be deterministic");

        let name = a.file_name().unwrap().to_str().unwrap();
        assert!(
            name.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.'),
            "filename must be safe regardless of project_id, got: {name}"
        );
        assert_ne!(
            lock_path("project-one"),
            lock_path("project-two"),
            "distinct project ids must map to distinct lock files"
        );
    }

    /// The lock must not sit directly in a WORLD-WRITABLE temp dir: a
    /// predictable path there lets a local user pre-create it as a symlink,
    /// which `set_len(0)` in `acquire` would then truncate (or simply hold the
    /// flock to wedge every index run). Holds for both `per_user_runtime_dir()`
    /// arms on Unix — the `XDG_RUNTIME_DIR` path and the
    /// `temp_dir()/codescout-{uid}` fallback — since both are namespaced away
    /// from bare `temp_dir()`. Fails on a revert of `lock_path` to
    /// `std::env::temp_dir()` directly.
    ///
    /// **Unix-only, by construction rather than by omission.** The threat is
    /// `/tmp` being world-writable. On Windows `temp_dir()` is
    /// `%LOCALAPPDATA%\Temp` — already inside the user's own profile — so
    /// `per_user_runtime_dir()`'s `#[cfg(not(unix))]` arm returns it unchanged
    /// on purpose, and `lock_path`'s doc comment states that intent
    /// ("already-per-user `temp_dir()` on Windows"). Asserting the Unix
    /// invariant there fails against a *correct* implementation, which is what
    /// it did: see
    /// `docs/issues/archive/2026-08-06-windows-doctor-rehome-and-index-lock-tests-fail.md`.
    /// The platform-independent half of the invariant is
    /// `lock_path_is_sited_in_the_per_user_runtime_dir` below.
    #[cfg(unix)]
    #[test]
    fn lock_path_is_not_sited_in_bare_temp_dir() {
        let p = lock_path("some-project-for-siting-check");
        assert_ne!(
            p.parent(),
            Some(std::env::temp_dir().as_path()),
            "lock file must not sit directly in the bare temp dir, got parent {:?}",
            p.parent()
        );
    }

    /// The half of the siting invariant that holds on every platform: the lock
    /// lives in the directory `per_user_runtime_dir()` designates, which is what
    /// makes it per-user at all. Keeps Windows covered after the test above was
    /// scoped to Unix — a revert that sites the lock anywhere else still fails
    /// here, on every platform.
    #[test]
    fn lock_path_is_sited_in_the_per_user_runtime_dir() {
        let p = lock_path("some-project-for-siting-check");
        assert_eq!(
            p.parent(),
            Some(crate::socket_discovery::per_user_runtime_dir().as_path()),
            "lock must sit in per_user_runtime_dir(), got parent {:?}",
            p.parent()
        );
    }
}
