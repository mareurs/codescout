//! RAII guard held by the write-tool gate in `CodeScoutServer::call_tool_inner`.
//!
//! Two layers:
//! 1. Async `tokio::sync::Mutex<()>` — serializes writes inside a single
//!    codescout process. Acquired FIRST.
//! 2. `flock` (via `fs4`) on `.codescout/write.lock` — serializes writes
//!    across codescout processes on the same project. Acquired SECOND.
//!
//! Order matters: always inner mutex → outer flock. Releasing happens in
//! reverse order on drop (flock released first, then async mutex).

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use fs4::fs_std::FileExt;
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

use crate::tools::RecoverableError;

/// Held for the duration of a single write-tool call.
/// Drop order: `_async_guard` drops last (Rust drops struct fields in
/// declaration order), so we declare the file-lock handle first.
pub struct WriteGuard {
    file: Arc<File>,
    /// Sidecar carrying the holder record. NOT the lock file — see
    /// `holder_record_path` for why they must be different files.
    holder_path: PathBuf,
    _async_guard: OwnedMutexGuard<()>,
}

impl Drop for WriteGuard {
    fn drop(&mut self) {
        // Release the flock explicitly — documents intent. Closing the fd
        // would also release it, but we keep the File alive in an Arc across
        // calls, so an explicit unlock is required.
        // Clear the record BEFORE unlocking. The reverse order leaves a window in which
        // the next holder has already taken the flock and written its record,
        // and this clear erases it — reporting a live holder as anonymous.
        let _ = std::fs::write(&self.holder_path, "");
        let _ = FileExt::unlock(&*self.file);
    }
}

/// Milliseconds since the Unix epoch, or 0 if the clock is before it.
fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// The holder record: `<epoch_ms>\t<holder>`, written to `holder_record_path`.
///
/// `IC-17` layer 2 — a shared resource carries an owner. The model is
/// `.git/session-stage-log`, which answered in one command a question three
/// sessions had been answering from memory.
///
/// ORDERING IS THE CORRECTNESS ARGUMENT, not the format or the location: the
/// record is written AFTER the flock is taken and cleared BEFORE it is
/// released, so the only process that can have written it is the one holding
/// the lock. That is what makes it trustworthy without a second lock protecting
/// it, and moving the bytes out of the lock file does not weaken it.
fn write_holder_record(path: &Path, holder: &str) -> std::io::Result<()> {
    std::fs::write(path, format!("{}\t{}", now_ms(), holder))
}

/// Read the holder record, or `None` when it is absent, empty or unparseable.
///
/// Every `None` branch means "no usable owner information" and NEVER "no
/// holder" — the flock already established that someone holds it. The caller
/// must not turn a `None` into a claim that the lock is free.
fn read_holder_record(path: &Path) -> Option<(u128, String)> {
    let buf = std::fs::read_to_string(path).ok()?;
    let (ts, holder) = buf.trim_end().split_once('\t')?;
    let holder = holder.trim();
    if holder.is_empty() {
        return None;
    }
    Some((ts.parse().ok()?, holder.to_string()))
}

/// Path of the holder-record sidecar: `.codescout/write.lock.holder`.
///
/// **It is a SEPARATE FILE from `write.lock`, and that is the entire point.**
/// The record used to live inside the lock file, which is correct on Unix and
/// inert on Windows: `flock(2)` is *advisory*, so a contending process opens and
/// reads the locked file freely, while `LockFileEx` is *mandatory*, so the
/// contender's read fails with `Os { code: 33 }` — "another process has locked a
/// portion of the file". That `Err` becomes `None`, `None` selects the anonymous
/// branch, and every Windows refusal read "The holder recorded no identity" no
/// matter who held it. The feature the record exists for never worked on a third
/// of the matrix, and three tests asserting it had never passed there.
///
/// Keeping the record outside the locked bytes restores it on every platform and
/// costs nothing on Unix.
/// See `docs/issues/2026-09-08-the-write-guard-holder-tests-fail-on-every-windows-lane.md`.
pub fn holder_record_path(root: &Path) -> PathBuf {
    root.join(".codescout").join("write.lock.holder")
}

/// `12m10s` rather than `730s` — the measured hold in the bug this closes was
/// 12m10s, and a bare second count reads as an error at that magnitude.
fn human_elapsed(since_ms: u128) -> String {
    let secs = now_ms().saturating_sub(since_ms) / 1000;
    if secs < 60 {
        format!("{secs}s")
    } else {
        format!("{}m{:02}s", secs / 60, secs % 60)
    }
}

/// Acquire both locks. Returns `RecoverableError` on timeout so the caller
/// can surface it as `isError: false`.
///
/// `timeout` is a **total** budget covering both the in-process async-mutex
/// wait and the cross-process flock poll. Without wrapping the whole thing in
/// `tokio::time::timeout`, a queue of N tools waiting on the async mutex could
/// each consume up to `timeout` on the flock poll individually, giving an
/// effective ceiling of `timeout × queue_depth` — no overall deadline.
///
/// `holder_path` is the sidecar from `holder_record_path`, deliberately NOT the
/// lock file: on Windows the lock is mandatory, so a contender cannot read bytes
/// inside it. See that function for the measurement.
pub async fn acquire(
    async_mutex: Arc<AsyncMutex<()>>,
    file: Arc<File>,
    holder_path: PathBuf,
    timeout: Duration,
    holder: &str,
) -> Result<WriteGuard, RecoverableError> {
    let start = Instant::now();

    // Phase 1: in-process async mutex, bounded by the total budget.
    let async_guard = match tokio::time::timeout(timeout, async_mutex.lock_owned()).await {
        Ok(g) => g,
        Err(_) => {
            return Err(RecoverableError::with_hint(
                "timed out waiting for in-process write lock",
                "Another tool call is holding the project's write lock. Retry shortly.",
            ));
        }
    };

    // Phase 2: cross-process flock, bounded by whatever remains of the budget.
    let remaining = timeout.saturating_sub(start.elapsed());
    if remaining.is_zero() {
        return Err(RecoverableError::with_hint(
            "timed out before checking cross-process write lock",
            "In-process queue exhausted the write-lock budget. Retry shortly.",
        ));
    }

    let file_clone = file.clone();
    let acquired = tokio::task::spawn_blocking(move || {
        let start = Instant::now();
        loop {
            match file_clone.try_lock_exclusive() {
                Ok(()) => return true,
                Err(e) if e.raw_os_error() == fs4::lock_contended_error().raw_os_error() => {
                    if start.elapsed() >= remaining {
                        return false;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(_) => return false,
            }
        }
    })
    .await
    .unwrap_or(false);

    if !acquired {
        return Err(match read_holder_record(&holder_path) {
            Some((since_ms, held_by)) => RecoverableError::with_hint(
                format!("write lock held by {held_by}"),
                format!(
                    "Held for {}. Message the holder rather than retrying — a \
                     `librarian(reindex, reembed=true)` legitimately holds this for \
                     minutes, so waiting is not different from asking. If no such \
                     call is running, the holder has exited and the lock clears with \
                     its process.",
                    human_elapsed(since_ms)
                ),
            ),
            None => RecoverableError::with_hint(
                "another codescout instance is writing to this project",
                "The holder recorded no identity — an older codescout, or an exit \
                 between taking the lock and writing the record. Check for a running \
                 reindex before retrying.",
            ),
        });
    }

    // Best effort, and deliberately not fatal: the lock IS held, and failing a
    // granted acquisition because its metadata did not land would trade a real
    // capability for a diagnostic. A failed write degrades the NEXT refusal to
    // the `None` branch above, which says so rather than inventing a holder.
    let _ = write_holder_record(&holder_path, holder);

    Ok(WriteGuard {
        file,
        holder_path,
        _async_guard: async_guard,
    })
}

/// Open (or create) the lock file at `.codescout/write.lock` under `root`.
/// Idempotent; safe to call on an existing file. Returns an `Arc<File>` so
/// the descriptor can be shared by every tool call without re-opening.
pub fn open_lock_file(root: &Path) -> std::io::Result<Arc<File>> {
    let dir = root.join(".codescout");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("write.lock");
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)?;
    Ok(Arc::new(file))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn acquire_returns_guard_when_uncontended() {
        let dir = tempdir().unwrap();
        let fd = open_lock_file(dir.path()).unwrap();
        let m = Arc::new(AsyncMutex::new(()));
        let g = acquire(
            m,
            fd,
            holder_record_path(dir.path()),
            Duration::from_secs(1),
            "codescout:t x",
        )
        .await
        .unwrap();
        drop(g); // released
    }

    #[tokio::test]
    async fn acquire_times_out_on_cross_process_contention() {
        // Emulate a second process by opening a SEPARATE File handle on the
        // same path (flock is per-open-file-description, not per-fd).
        let dir = tempdir().unwrap();
        let fd_a = open_lock_file(dir.path()).unwrap();
        let fd_b = open_lock_file(dir.path()).unwrap();
        // Sanity: they must be different File handles.
        assert!(!Arc::ptr_eq(&fd_a, &fd_b));

        let m_a = Arc::new(AsyncMutex::new(()));
        let m_b = Arc::new(AsyncMutex::new(()));

        let _held = acquire(
            m_a,
            fd_a,
            holder_record_path(dir.path()),
            Duration::from_secs(1),
            "codescout:t x",
        )
        .await
        .unwrap();

        let r = acquire(
            m_b,
            fd_b,
            holder_record_path(dir.path()),
            Duration::from_millis(200),
            "codescout:t y",
        )
        .await;
        assert!(r.is_err(), "second process should time out");
    }

    #[tokio::test]
    async fn guard_drop_releases_lock() {
        let dir = tempdir().unwrap();
        let fd_a = open_lock_file(dir.path()).unwrap();
        let fd_b = open_lock_file(dir.path()).unwrap();

        {
            let _g = acquire(
                Arc::new(AsyncMutex::new(())),
                fd_a,
                holder_record_path(dir.path()),
                Duration::from_secs(1),
                "codescout:t x",
            )
            .await
            .unwrap();
        } // guard drops here → flock released

        let r = acquire(
            Arc::new(AsyncMutex::new(())),
            fd_b,
            holder_record_path(dir.path()),
            Duration::from_millis(500),
            "codescout:t y",
        )
        .await;
        assert!(r.is_ok(), "second acquire should succeed after first drops");
    }

    #[tokio::test]
    async fn open_lock_file_creates_codescout_dir() {
        let dir = tempdir().unwrap();
        let _ = open_lock_file(dir.path()).unwrap();
        assert!(dir.path().join(".codescout/write.lock").exists());
    }

    /// A refused party must be able to ACT, and the only action available is to
    /// message the holder — so the refusal has to name them.
    ///
    /// LOAD-BEARING: `fd_a` and `fd_b` are SEPARATE `open_lock_file` calls. flock
    /// is per-open-file-description, so cloning one `Arc` would not contend, the
    /// second acquire would succeed, and this test would assert nothing.
    #[tokio::test]
    async fn a_contended_acquire_names_the_holder_not_merely_that_someone_holds_it() {
        let dir = tempdir().unwrap();
        let fd_a = open_lock_file(dir.path()).unwrap();
        let fd_b = open_lock_file(dir.path()).unwrap();
        let m_a = Arc::new(AsyncMutex::new(()));
        let m_b = Arc::new(AsyncMutex::new(()));

        let _held = acquire(
            m_a,
            fd_a,
            holder_record_path(dir.path()),
            Duration::from_secs(1),
            "codescout:sid-alpha reindex",
        )
        .await
        .unwrap();

        let err = acquire(
            m_b,
            fd_b,
            holder_record_path(dir.path()),
            Duration::from_millis(200),
            "codescout:sid-beta edit_file",
        )
        .await;
        let err = match err {
            Ok(_) => panic!("a second open-file-description must contend"),
            Err(e) => e,
        };

        let rendered = format!("{} {:?}", err.message, err.guidance);
        assert!(
            rendered.contains("sid-alpha"),
            "the refusal must name the HOLDER — a refused party cannot message an \
             unnamed one: {rendered}"
        );
        // Discriminator: kills an implementation that echoes the CALLER's own
        // holder string back, which would satisfy the assertion above.
        assert!(
            !rendered.contains("sid-beta"),
            "must name the holder, not the refused caller: {rendered}"
        );
    }

    /// Releasing must clear the record. Otherwise the NEXT holder — one whose own
    /// record write failed — is reported under the PREVIOUS holder's name, which
    /// is strictly worse than anonymous: it sends a refused party to message
    /// someone who has already exited.
    ///
    /// LOAD-BEARING: the pre-drop assertion is the control. Without it, a build
    /// in which the record is never written at all would satisfy the post-drop
    /// assertion and this test would be monotone under the feature's removal.
    #[tokio::test]
    async fn releasing_the_lock_clears_the_holder_record() {
        let dir = tempdir().unwrap();
        let record = holder_record_path(dir.path());
        let fd = open_lock_file(dir.path()).unwrap();
        let m = Arc::new(AsyncMutex::new(()));

        let g = acquire(
            m,
            fd,
            holder_record_path(dir.path()),
            Duration::from_secs(1),
            "codescout:sid-gamma reindex",
        )
        .await
        .unwrap();
        assert!(
            std::fs::read_to_string(&record)
                .unwrap()
                .contains("sid-gamma"),
            "control: the record must exist WHILE held, or the assertion below \
             passes against a build that never writes one"
        );

        drop(g);
        assert_eq!(
            std::fs::read_to_string(&record).unwrap(),
            "",
            "the record must be cleared on release"
        );
    }

    /// The hint must report the elapsed hold and must not promise a short one.
    ///
    /// The superseded text said "the holder should release shortly" against a
    /// measured 12m10s reindex — a claim, and a false one. Both assertions are
    /// needed: the absence check alone is monotone under deleting the whole
    /// message, so the positive one carries the discrimination.
    #[tokio::test]
    async fn the_refusal_reports_elapsed_hold_and_promises_no_deadline() {
        let dir = tempdir().unwrap();
        let fd_a = open_lock_file(dir.path()).unwrap();
        let fd_b = open_lock_file(dir.path()).unwrap();
        let m_a = Arc::new(AsyncMutex::new(()));
        let m_b = Arc::new(AsyncMutex::new(()));

        let _held = acquire(
            m_a,
            fd_a,
            holder_record_path(dir.path()),
            Duration::from_secs(1),
            "codescout:sid-delta reindex",
        )
        .await
        .unwrap();
        let r = acquire(
            m_b,
            fd_b,
            holder_record_path(dir.path()),
            Duration::from_millis(200),
            "codescout:sid-eps edit_file",
        )
        .await;
        let err = match r {
            Ok(_) => panic!("a second open-file-description must contend"),
            Err(e) => e,
        };

        let rendered = format!("{} {:?}", err.message, err.guidance);
        assert!(
            rendered.contains("Held for"),
            "the hint must report elapsed hold: {rendered}"
        );
        assert!(
            !rendered.contains("shortly"),
            "must not promise a deadline it cannot know — the superseded text was \
             wrong by two orders of magnitude against a 12m10s reindex: {rendered}"
        );
    }
}
