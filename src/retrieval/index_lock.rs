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
/// leftover lock file is inert and needs no recovery logic. The same is true of a
/// leftover [`pid_path`] sidecar: it is only ever read when a lock is genuinely held,
/// and every acquirer replaces it wholesale.
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
        // Remove the PID record BEFORE releasing, so any window in which a contender
        // can read it is a window in which the lock is genuinely still held. The
        // reverse order would let the next holder acquire and write its own record
        // between our unlock and our cleanup, and we would then delete *its* record.
        let _ = std::fs::remove_file(pid_path(&self.path));
        // Explicit unlock documents intent; closing the fd would also release it.
        let _ = FileExt::unlock(&self.file);
    }
}

/// The specific, downcastable cause behind an [`acquire_in`] failure: the lock is
/// currently held by someone else. `acquire_in` wraps this in `anyhow::Error` like any
/// other failure, so a caller that only wants the diagnostic text sees no difference —
/// but a caller that needs to tell "someone else is already indexing" apart from a
/// genuine I/O error (permissions, disk full, ...) can `downcast_ref::<LockHeldError>()`
/// instead of matching on message text.
#[derive(Debug)]
pub struct LockHeldError {
    pub project_id: String,
    pub path: PathBuf,
    /// Best-effort — the PID is written to the [`pid_path`] sidecar only after the holder
    /// acquires the lock, so it can be absent, and it is read without holding the lock so
    /// it can be stale by the time the caller looks. `None` means "couldn't determine",
    /// never "no holder".
    pub holder_pid: Option<u32>,
}

impl std::fmt::Display for LockHeldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Print the PID rather than telling the reader to go open a file: on Windows the
        // lock file cannot be read at all while the lock is held (see `pid_path`), so the
        // old "its first line is the holder's PID" advice was impossible to follow there.
        let holder = match self.holder_pid {
            Some(pid) => format!("PID {pid}"),
            None => format!(
                "an undetermined PID — see {}",
                pid_path(&self.path).display()
            ),
        };
        write!(
            f,
            "another codescout index is already running for project '{}' \
             (held by {holder}; lock file: {}). The holder may be \
             a CLI `codescout index` run OR an in-process background index (e.g. an \
             MCP server's auto-index task) — check the PID, don't assume `pgrep -af \
             'codescout index'` will show it.",
            self.project_id,
            self.path.display()
        )
    }
}

impl std::error::Error for LockHeldError {}

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

/// Path of the PID sidecar beside a lock file: `codescout-index-<hash>.pid`.
///
/// The holder's PID cannot live *inside* the lock file. `fs4` takes the lock over the
/// whole byte range (`LockFileEx(.., 0, !0, !0)`), and Windows byte-range locks are
/// **mandatory** rather than advisory: while the lock is held, a read through any other
/// handle fails with `ERROR_LOCK_VIOLATION` — which is exactly the moment a waiter wants
/// to know who holds it. Unix never showed this, because `flock` is advisory and the read
/// simply succeeds. A separate, never-locked file reads the same on both.
///
/// Measured on CI run `32961510592` (`windows-latest`): `holder_pid` came back `None`,
/// which `index(action="status")` and `index(action="build")` both surface to the caller
/// as `holder_pid: null`, and the error text advised opening a file Windows would refuse.
fn pid_path(lock_path: &Path) -> PathBuf {
    lock_path.with_extension("pid")
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

    // The lock file is a pure lock token and carries no content — the holder's PID
    // lives in the [`pid_path`] sidecar. `truncate(false)` because there is nothing to
    // truncate and no reason to write to a file another process is holding a lock on;
    // `File::create` would truncate on open.
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .with_context(|| format!("failed to open index lock file: {}", path.display()))?;

    if file.try_lock_exclusive().is_err() {
        // Read the holder's PID from the sidecar, never from `path`: the lock covers
        // every byte of that file, and on Windows that makes it unreadable through any
        // other handle for as long as it is held. See [`pid_path`].
        let holder_pid = std::fs::read_to_string(pid_path(&path))
            .ok()
            .and_then(|s| s.lines().next().and_then(|l| l.trim().parse::<u32>().ok()));
        return Err(LockHeldError {
            project_id: project_id.to_string(),
            path,
            holder_pid,
        }
        .into());
    }

    // PID for diagnostics, mirroring src/lsp/mux/process.rs:81. Only after the
    // lock is held, so we never clobber another holder's record. Best-effort:
    // a failed write must not fail an otherwise-valid lock. `fs::write` truncates,
    // so a longer record left by a dead process cannot leave a tail behind.
    let _ = std::fs::write(pid_path(&path), format!("{}\n", std::process::id()));

    Ok(IndexLock { file, path })
}

/// Non-blocking check: is `project_id`'s index lock currently held by someone else?
///
/// `Some(holder_pid)` if held (the inner `Option` is the same best-effort PID read
/// [`acquire_in`]'s error carries); `None` if free, **or** if the check itself failed
/// for an unrelated reason (e.g. the lock dir is unwritable). Callers that treat `None`
/// as "proceed as if free" are correct either way: a genuine acquire attempt right
/// after is the actual source of truth, and this is only an optimization to avoid
/// spawning work that is doomed to immediately lose that race.
///
/// Test seam, mirroring [`acquire_in`]. Production goes through [`peek`].
pub fn peek_in(dir: &Path, project_id: &str) -> Option<Option<u32>> {
    match acquire_in(dir, project_id) {
        Ok(_lock) => None, // acquired and immediately dropped below -> was free
        Err(e) => e.downcast_ref::<LockHeldError>().map(|le| le.holder_pid),
    }
}

/// Non-blocking check: is `project_id`'s index lock currently held by someone else?
/// See [`peek_in`] for the exact semantics.
pub fn peek(project_id: &str) -> Option<Option<u32>> {
    peek_in(&crate::socket_discovery::per_user_runtime_dir(), project_id)
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
    fn second_acquire_fails_with_a_downcastable_lock_held_error_naming_the_holder_pid() {
        let dir = scratch();
        let _first = acquire_in(dir.path(), "contend-typed").expect("first acquire must succeed");
        let second = acquire_in(dir.path(), "contend-typed");
        let err = second.expect_err(
            "a second acquire for the same project_id must fail while the first is held",
        );
        let lock_err = err
            .downcast_ref::<LockHeldError>()
            .expect("error must downcast to LockHeldError, not just a string-formatted context");
        assert_eq!(lock_err.holder_pid, Some(std::process::id()));
    }

    #[test]
    fn peek_in_returns_none_when_free() {
        let dir = scratch();
        assert_eq!(peek_in(dir.path(), "peek-free"), None);
    }

    #[test]
    fn peek_in_returns_holder_pid_when_locked() {
        let dir = scratch();
        let _held = acquire_in(dir.path(), "peek-held").expect("acquire must succeed");
        assert_eq!(
            peek_in(dir.path(), "peek-held"),
            Some(Some(std::process::id()))
        );
    }

    #[test]
    fn peek_in_does_not_leave_the_lock_held() {
        let dir = scratch();
        assert_eq!(peek_in(dir.path(), "peek-then-acquire"), None);
        // peek must release immediately — a real acquire right after must still succeed.
        let acquired = acquire_in(dir.path(), "peek-then-acquire");
        assert!(acquired.is_ok(), "peek must not leave the lock held");
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

    /// The PID record is cleaned up on release, so a later reader cannot mistake a
    /// leftover file for a live holder.
    ///
    /// Pins the ORDER too, indirectly: the record is removed before the unlock, so the
    /// re-acquire below — which only succeeds once the lock is released — proves the
    /// removal already happened by then. Reversing the two would let a fresh holder's
    /// record be deleted by the outgoing guard.
    #[test]
    fn drop_removes_the_pid_record() {
        let dir = scratch();
        let path = lock_path_in(dir.path(), "pid-cleanup");
        {
            let _held = acquire_in(dir.path(), "pid-cleanup").expect("acquire");
            assert!(
                pid_path(&path).exists(),
                "the holder must record its pid while it holds the lock"
            );
        }
        assert!(
            !pid_path(&path).exists(),
            "the pid record must not outlive the lock it describes"
        );

        let again = acquire_in(dir.path(), "pid-cleanup").expect("re-acquire after release");
        assert!(
            pid_path(&path).exists(),
            "a fresh holder must have its own record, not the previous holder's deletion"
        );
        drop(again);
    }

    /// A leftover lock *file* must never block a new run, and a leftover PID record
    /// must be replaced wholesale rather than believed or appended to.
    ///
    /// The planted PID is our OWN live pid, which kills a specific mutation: a
    /// PID-liveness check ("is the holder still alive? then refuse") would refuse and
    /// fail this test. Planting a dead pid like 999999 would NOT pin that — 999999 is
    /// above `pid_max` on most Linux configs and reads as dead anyway. The planted
    /// record is also LONGER than what `acquire` writes, so an append rather than a
    /// replace leaves a visible tail.
    #[test]
    fn preexisting_lock_file_does_not_block() {
        let dir = scratch();
        let path = lock_path_in(dir.path(), "stale");
        std::fs::write(&path, "leftover bytes from an older codescout\n")
            .expect("simulate a lock file left by a dead process");
        std::fs::write(
            pid_path(&path),
            format!("{}\nstale-tail-that-must-be-replaced\n", std::process::id()),
        )
        .expect("simulate a PID record left by a dead process");

        let lock =
            acquire_in(dir.path(), "stale").expect("a stale lock file must not block acquisition");

        // Read before dropping: the guard removes the record on release.
        let recorded = std::fs::read_to_string(pid_path(&path)).expect("read pid sidecar");
        assert_eq!(
            recorded.trim(),
            std::process::id().to_string(),
            "the PID record must be replaced wholesale, with no stale tail"
        );
        drop(lock);
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
