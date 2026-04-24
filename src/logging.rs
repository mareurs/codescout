use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// Directory containing `.codescout/` for the current project.
/// Set once during `init()`, read by the panic hook.
static CODESCOUT_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Path to the current diagnostic log file (when `--diagnostic` is active).
static DIAGNOSTIC_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Rotate log files in `dir`: keep last 3 numbered backups.
/// debug.log.3 → deleted
/// debug.log.2 → debug.log.3
/// debug.log.1 → debug.log.2
/// debug.log   → debug.log.1
pub fn rotate_logs(dir: &Path) {
    const KEEP: u32 = 3;
    // Delete oldest
    let _ = std::fs::remove_file(dir.join(format!("debug.log.{}", KEEP)));
    // Shift numbered backups downward (highest first to avoid clobbering)
    for i in (1..KEEP).rev() {
        let from = dir.join(format!("debug.log.{}", i));
        let to = dir.join(format!("debug.log.{}", i + 1));
        let _ = std::fs::rename(from, to);
    }
    // Move current log to .1
    let _ = std::fs::rename(dir.join("debug.log"), dir.join("debug.log.1"));
}

/// Generate a 4-hex-char random instance ID for log file naming.
/// Uses std::hash::RandomState which is randomly seeded per process.
fn generate_instance_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_usize(std::process::id() as usize);
    format!("{:04x}", hasher.finish() as u16)
}

/// Rotate diagnostic log files: keep the 6 most recent by mtime.
/// Different from `rotate_logs` which uses numbered backups for a single file.
pub fn rotate_diagnostic_logs(dir: &Path) {
    const KEEP: usize = 6;

    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with("diagnostic-") && name.ends_with(".log")
        })
        .filter_map(|e| {
            let mtime = e.metadata().ok()?.modified().ok()?;
            Some((e.path(), mtime))
        })
        .collect();

    if entries.len() <= KEEP {
        return;
    }

    // Sort newest first
    entries.sort_by(|a, b| b.1.cmp(&a.1));

    // Remove everything beyond the 6th
    for (path, _) in &entries[KEEP..] {
        let _ = std::fs::remove_file(path);
    }
}

/// Hard cap per log file before size-based rotation fires.
///
/// Defense-in-depth against runtime log-flooding bugs (e.g. BUG-047 — a spinning
/// `Poll::Pending` emitted millions of WARN lines per second until two files
/// reached 268 GB each). Count-based `rotate_logs` runs only at startup; this
/// cap runs on every write.
const MAX_LOG_BYTES: u64 = 50 * 1024 * 1024; // 50 MiB

/// Write-only file wrapper that rotates to numbered backups when growth would
/// exceed `max_bytes`. Keeps 3 backups (`.1`, `.2`, `.3`).
///
/// Used as the underlying `Write` for `tracing_appender::non_blocking` so the
/// rotation runs on the dedicated log-writer thread, never on the hot path.
struct SizeRotatingFile {
    path: PathBuf,
    file: std::fs::File,
    max_bytes: u64,
    current_bytes: u64,
}

impl SizeRotatingFile {
    const KEEP: u32 = 3;

    fn open(path: PathBuf, max_bytes: u64) -> std::io::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)?;
        Ok(Self {
            path,
            file,
            max_bytes,
            current_bytes: 0,
        })
    }

    /// Shift `.N` → `.N+1` (descending), current → `.1`, open fresh file.
    /// Mirrors `rotate_logs` but runs mid-session on size, not at startup.
    fn rotate(&mut self) -> std::io::Result<()> {
        let _ = std::fs::remove_file(numbered(&self.path, Self::KEEP));
        for i in (1..Self::KEEP).rev() {
            let _ = std::fs::rename(numbered(&self.path, i), numbered(&self.path, i + 1));
        }
        let _ = std::fs::rename(&self.path, numbered(&self.path, 1));
        self.file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.path)?;
        self.current_bytes = 0;
        Ok(())
    }
}

/// Append `.{n}` to a log path: `debug.log` + `1` → `debug.log.1`.
fn numbered(path: &Path, n: u32) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(format!(".{n}"));
    PathBuf::from(s)
}

impl std::io::Write for SizeRotatingFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.current_bytes.saturating_add(buf.len() as u64) > self.max_bytes {
            // Best-effort rotation — if it fails, keep writing to the current
            // file rather than lose the log line entirely.
            let _ = self.rotate();
        }
        let n = self.file.write(buf)?;
        self.current_bytes = self.current_bytes.saturating_add(n as u64);
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

/// Logging init result — holds worker guards and optional instance ID.
pub struct LoggingGuards {
    /// MUST be held for the lifetime of main. Dropping flushes and closes writers.
    pub guards: Vec<WorkerGuard>,
    /// 4-hex-char instance ID when diagnostic mode is active, for span injection.
    pub instance_id: Option<String>,
}

/// Install a panic hook that persists crash info to disk via synchronous I/O.
///
/// With `panic = "abort"` in release builds, `Drop` impls never run on panic —
/// the `non_blocking` tracing writer's buffer is silently lost.  This hook runs
/// *before* the abort, writing directly to `.codescout/crash.log` (and the
/// diagnostic log if active).  Synchronous writes bypass the non-blocking
/// pipeline entirely, so the crash message reaches disk even under abort.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let msg = format!("epoch={epoch}  PANIC  {info}\n");

        // Write to .codescout/crash.log (always attempted)
        if let Some(dir) = CODESCOUT_DIR.get() {
            sync_append(&dir.join("crash.log"), &msg);
        }
        // Also append to current diagnostic log if diagnostic mode is active
        if let Some(path) = DIAGNOSTIC_PATH.get() {
            sync_append(path, &msg);
        }
        // Default hook: prints to stderr
        default_hook(info);
    }));
}

/// Append `msg` to `path` synchronously.  Failures are silently ignored —
/// we're already in a panic handler; there's nothing useful to do with an
/// I/O error here.
fn sync_append(path: &Path, msg: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = f.write_all(msg.as_bytes());
        let _ = f.flush();
    }
}

/// Initialise tracing.
///
/// - `debug`: enables both DEBUG-level file logging to `.codescout/debug.log`
///   and INFO-level diagnostic logging to `.codescout/diagnostic-<hash>.log`.
///
/// Returns guards that MUST be held for the lifetime of `main`, plus the
/// diagnostic instance ID (if active) for root span injection.
pub fn init(debug: bool) -> LoggingGuards {
    let mut guards = Vec::new();

    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")));

    let log_dir = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".codescout");

    // Store for the panic hook — even when debug is off, crash.log is useful.
    CODESCOUT_DIR.set(log_dir.clone()).ok();

    if debug {
        if let Err(e) = std::fs::create_dir_all(&log_dir) {
            eprintln!("codescout: could not create log directory: {e}");
        }
    }

    // --- Debug file layer (DEBUG level) ---
    let debug_layer = if debug {
        rotate_logs(&log_dir);
        match SizeRotatingFile::open(log_dir.join("debug.log"), MAX_LOG_BYTES) {
            Ok(file) => {
                let (non_blocking, guard) = tracing_appender::non_blocking(file);
                guards.push(guard);
                Some(
                    tracing_subscriber::fmt::layer()
                        .with_writer(non_blocking)
                        .with_ansi(false)
                        .with_filter(EnvFilter::new("debug")),
                )
            }
            Err(e) => {
                eprintln!("codescout: could not open debug log: {e}");
                None
            }
        }
    } else {
        None
    };

    // --- Diagnostic file layer (INFO level) ---
    let mut instance_id = None;
    let diagnostic_layer = if debug {
        rotate_diagnostic_logs(&log_dir);
        let id = generate_instance_id();
        let filename = format!("diagnostic-{id}.log");
        match SizeRotatingFile::open(log_dir.join(&filename), MAX_LOG_BYTES) {
            Ok(file) => {
                let (non_blocking, guard) = tracing_appender::non_blocking(file);
                guards.push(guard);
                DIAGNOSTIC_PATH.set(log_dir.join(&filename)).ok();
                instance_id = Some(id);
                Some(
                    tracing_subscriber::fmt::layer()
                        .with_writer(non_blocking)
                        .with_ansi(false)
                        .with_filter(EnvFilter::new("info")),
                )
            }
            Err(e) => {
                eprintln!("codescout: could not open diagnostic log {filename}: {e}");
                None
            }
        }
    } else {
        None
    };

    // try_init returns Err when a global subscriber is already set (common in tests
    // where multiple test threads each call init). Surface any failure to stderr so
    // production startups don't silently lose all tracing.
    if let Err(e) = tracing_subscriber::registry()
        .with(stderr_layer)
        .with(debug_layer)
        .with(diagnostic_layer)
        .try_init()
    {
        eprintln!("codescout: failed to initialize tracing: {e}");
    }

    install_panic_hook();

    LoggingGuards {
        guards,
        instance_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotate_diagnostic_keeps_last_6() {
        let dir = tempfile::tempdir().unwrap();
        // Create 8 diagnostic files with staggered mtimes
        for i in 0..8 {
            let path = dir.path().join(format!("diagnostic-{:04x}.log", i));
            std::fs::write(&path, format!("log {i}")).unwrap();
            let mtime = filetime::FileTime::from_unix_time(1000 + i as i64, 0);
            filetime::set_file_mtime(&path, mtime).unwrap();
        }

        super::rotate_diagnostic_logs(dir.path());

        let mut remaining: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        remaining.sort();
        assert_eq!(
            remaining.len(),
            6,
            "should keep exactly 6 files: {remaining:?}"
        );
        // The two oldest (0000 and 0001) should be deleted
        assert!(!remaining.contains(&"diagnostic-0000.log".to_string()));
        assert!(!remaining.contains(&"diagnostic-0001.log".to_string()));
    }

    #[test]
    fn rotate_diagnostic_ignores_non_diagnostic_files() {
        let dir = tempfile::tempdir().unwrap();
        // Create 8 diagnostic files + 3 non-diagnostic files
        for i in 0..8 {
            let path = dir.path().join(format!("diagnostic-{:04x}.log", i));
            std::fs::write(&path, format!("log {i}")).unwrap();
            let mtime = filetime::FileTime::from_unix_time(1000 + i as i64, 0);
            filetime::set_file_mtime(&path, mtime).unwrap();
        }
        std::fs::write(dir.path().join("debug.log"), "debug").unwrap();
        std::fs::write(dir.path().join("debug.log.1"), "debug old").unwrap();
        std::fs::write(dir.path().join("random.txt"), "other").unwrap();

        super::rotate_diagnostic_logs(dir.path());

        let all: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        // 6 diagnostic + 3 non-diagnostic = 9
        assert_eq!(
            all.len(),
            9,
            "non-diagnostic files must be untouched: {all:?}"
        );
    }

    #[test]
    fn generate_instance_id_is_4_hex_chars() {
        let id = super::generate_instance_id();
        assert_eq!(id.len(), 4, "instance ID must be 4 chars: got '{id}'");
        assert!(
            id.chars().all(|c| c.is_ascii_hexdigit()),
            "instance ID must be hex: got '{id}'"
        );
    }

    #[test]
    fn generate_instance_id_varies_across_calls() {
        // RandomState is randomly seeded, so two calls should differ.
        // There's a 1/65536 chance of collision — acceptable for a test.
        let a = super::generate_instance_id();
        let b = super::generate_instance_id();
        assert_ne!(a, b, "instance IDs should vary across calls");
    }

    #[test]
    fn rotate_keeps_last_3() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();

        // Populate 4 log files with their own name as content (for verification)
        for name in &["debug.log", "debug.log.1", "debug.log.2", "debug.log.3"] {
            std::fs::write(p.join(name), name.as_bytes()).unwrap();
        }

        rotate_logs(p);

        // Original debug.log.3 is deleted — no debug.log.4 should exist
        assert!(!p.join("debug.log.4").exists());
        // debug.log.3 now contains original debug.log.2 content
        assert_eq!(
            std::fs::read_to_string(p.join("debug.log.3")).unwrap(),
            "debug.log.2"
        );
        // debug.log.2 now contains original debug.log.1 content
        assert_eq!(
            std::fs::read_to_string(p.join("debug.log.2")).unwrap(),
            "debug.log.1"
        );
        // debug.log.1 now contains original debug.log content
        assert_eq!(
            std::fs::read_to_string(p.join("debug.log.1")).unwrap(),
            "debug.log"
        );
        // debug.log itself is gone (renamed to .1)
        assert!(!p.join("debug.log").exists());
    }

    #[test]
    fn rotate_works_when_no_files_exist() {
        let dir = tempfile::tempdir().unwrap();
        rotate_logs(dir.path()); // Must not panic
    }

    #[test]
    fn rotate_works_with_only_current_log() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        std::fs::write(p.join("debug.log"), b"hello").unwrap();
        rotate_logs(p);
        assert!(!p.join("debug.log").exists());
        assert_eq!(
            std::fs::read_to_string(p.join("debug.log.1")).unwrap(),
            "hello"
        );
    }

    #[test]
    fn sync_append_creates_and_appends() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("crash.log");

        // First write creates the file
        sync_append(&path, "line 1\n");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "line 1\n");

        // Second write appends
        sync_append(&path, "line 2\n");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "line 1\nline 2\n");
    }

    #[test]
    fn sync_append_silently_ignores_bad_path() {
        // Should not panic on a non-existent directory
        sync_append(Path::new("/nonexistent/dir/crash.log"), "test\n");
    }

    #[test]
    fn size_rotating_file_rotates_on_cap_exceeded() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("debug.log");

        // 64-byte cap — small enough to trigger rotation quickly.
        let mut f = SizeRotatingFile::open(path.clone(), 64).unwrap();

        // First write fits: 32 bytes of 'a'.
        f.write_all(&[b'a'; 32]).unwrap();
        f.flush().unwrap();
        assert!(!path.with_extension("log.1").exists());

        // Second write: another 32 bytes. current_bytes (32) + 32 = 64, NOT > 64.
        f.write_all(&[b'b'; 32]).unwrap();
        f.flush().unwrap();
        assert!(!path.with_extension("log.1").exists());

        // Third write: 1 byte. 64 + 1 > 64 → triggers rotation.
        f.write_all(b"c").unwrap();
        f.flush().unwrap();

        // debug.log.1 now holds the pre-rotation content (64 bytes: a's and b's).
        let rotated = std::fs::read(path.with_extension("log.1")).unwrap();
        assert_eq!(rotated.len(), 64);
        assert!(rotated.iter().all(|&b| b == b'a' || b == b'b'));

        // debug.log holds the new content (single 'c').
        let current = std::fs::read(&path).unwrap();
        assert_eq!(current, b"c");
    }

    #[test]
    fn size_rotating_file_caps_total_growth_at_keep_plus_one() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("debug.log");

        let mut f = SizeRotatingFile::open(path.clone(), 16).unwrap();

        // Write 20 × 16-byte chunks — forces many rotations.
        for _ in 0..20 {
            f.write_all(&[b'x'; 16]).unwrap();
            f.write_all(&[b'y'; 1]).unwrap(); // triggers rotate
            f.flush().unwrap();
        }

        // Only debug.log, .1, .2, .3 should exist. Nothing beyond KEEP.
        assert!(path.exists());
        assert!(path.with_extension("log.1").exists());
        assert!(path.with_extension("log.2").exists());
        assert!(path.with_extension("log.3").exists());
        assert!(
            !path.with_extension("log.4").exists(),
            "rotation must not leave files beyond KEEP"
        );
    }

    #[test]
    fn size_rotating_file_single_write_under_cap_does_not_rotate() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("debug.log");

        let mut f = SizeRotatingFile::open(path.clone(), 1024).unwrap();
        f.write_all(b"small").unwrap();
        f.flush().unwrap();

        assert!(!path.with_extension("log.1").exists());
        assert_eq!(std::fs::read(&path).unwrap(), b"small");
    }

    #[test]
    fn numbered_appends_suffix() {
        assert_eq!(
            numbered(Path::new("/tmp/debug.log"), 1),
            PathBuf::from("/tmp/debug.log.1")
        );
        assert_eq!(
            numbered(Path::new("diagnostic-abcd.log"), 3),
            PathBuf::from("diagnostic-abcd.log.3")
        );
    }
}
