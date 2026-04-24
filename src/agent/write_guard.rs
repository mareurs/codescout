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
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use fs4::fs_std::FileExt;
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

use crate::tools::RecoverableError;

/// Held for the duration of a single write-tool call.
/// Drop order: `_async_guard` drops last (Rust drops struct fields in
/// declaration order), so we declare the file-lock handle first.
pub struct WriteGuard {
    file: Arc<File>,
    _async_guard: OwnedMutexGuard<()>,
}

impl Drop for WriteGuard {
    fn drop(&mut self) {
        // Release the flock explicitly — documents intent. Closing the fd
        // would also release it, but we keep the File alive in an Arc across
        // calls, so an explicit unlock is required.
        let _ = FileExt::unlock(&*self.file);
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
pub async fn acquire(
    async_mutex: Arc<AsyncMutex<()>>,
    file: Arc<File>,
    timeout: Duration,
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
        return Err(RecoverableError::with_hint(
            "another codescout instance is writing to this project",
            "Retry in a moment — the holder should release shortly.",
        ));
    }

    Ok(WriteGuard {
        file,
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
        let g = acquire(m, fd, Duration::from_secs(1)).await.unwrap();
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

        let _held = acquire(m_a, fd_a, Duration::from_secs(1)).await.unwrap();

        let r = acquire(m_b, fd_b, Duration::from_millis(200)).await;
        assert!(r.is_err(), "second process should time out");
    }

    #[tokio::test]
    async fn guard_drop_releases_lock() {
        let dir = tempdir().unwrap();
        let fd_a = open_lock_file(dir.path()).unwrap();
        let fd_b = open_lock_file(dir.path()).unwrap();

        {
            let _g = acquire(Arc::new(AsyncMutex::new(())), fd_a, Duration::from_secs(1))
                .await
                .unwrap();
        } // guard drops here → flock released

        let r = acquire(
            Arc::new(AsyncMutex::new(())),
            fd_b,
            Duration::from_millis(500),
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
}
