//! Manages per-language LSP client instances with lazy initialization.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::Weak;
use std::time::Duration;
use std::time::Instant;

use anyhow::Result;
use tokio::sync::Mutex;

/// Return the idle TTL for a given language.
///
/// Kotlin's LSP takes 8–10 s to restart, so it gets a much longer idle window
/// to avoid paying that cost after brief gaps in tool use. All other languages
/// fall back to the caller-supplied global default.
fn ttl_for_language(language: &str, global: Duration) -> Duration {
    match language {
        "kotlin" => Duration::from_secs(2 * 3600),
        _ => global,
    }
}

/// Restart-cost tier for LSP eviction selection. Lower numbers = cheaper to restart.
///
/// When the pool is at `max_clients`, the eviction selector prefers the
/// cheapest-to-restart victim instead of pure LRU. Rationale (from
/// docs/usage-reports/2026-05-27-usage-analysis.md LSP-events):
/// - Kotlin: avg 5.3s cold start, p100 62.5s, `lru_evicted` avg 24s
/// - Java:   avg 2.2s, max 6.3s
/// - Rust / TS / JS / Python / shell: ≤500ms (typically <100ms)
///
/// So evicting an idle Kotlin server to make room for a brief rust query is a
/// bad trade. This selector pushes Kotlin/Java to last-resort and only evicts
/// them when the entire pool is expensive.
fn restart_cost_tier(language: &str) -> u8 {
    match language {
        "kotlin" | "java" => 2,
        _ => 1,
    }
}

use super::client::{LspClient, LspServerConfig};
use super::servers;

/// Composite key for the LSP client pool: one client per (language, project_root).
#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct LspKey {
    pub language: String,
    pub project_root: PathBuf,
}

impl LspKey {
    pub fn new(language: &str, project_root: &Path) -> Self {
        Self {
            language: language.to_string(),
            project_root: project_root.to_path_buf(),
        }
    }
}

/// Manages LSP client instances, one per (language, project_root) pair.
///
/// Clients are lazily started on first use and cached. When the pool
/// reaches `max_clients`, the least-recently-used client is evicted.
/// Clients idle for longer than `idle_ttl` are also reaped by a background
/// task spawned in `new_arc_with_ttl`.
pub struct LspManager {
    clients: Mutex<HashMap<LspKey, Arc<LspClient>>>,
    /// Tracks last access time for LRU eviction.
    last_used: Mutex<HashMap<LspKey, Instant>>,
    /// Per-key startup barrier: concurrent callers for the same key
    /// wait on a `watch` channel. The first caller sends `true` on success or
    /// `false` on failure; late arrivals always see the final value.
    ///
    /// Uses `std::sync::Mutex` (not tokio) so it can be locked in `Drop`
    /// guards, which are synchronous. The lock is never held across `await`
    /// points — only for brief HashMap insert/remove operations.
    starting: StdMutex<HashMap<LspKey, tokio::sync::watch::Receiver<Option<bool>>>>,
    /// Maximum number of concurrent LSP clients before LRU eviction kicks in.
    max_clients: usize,
    /// How long a client may sit idle before the background task evicts it.
    idle_ttl: Duration,
    /// Maps LspKey → db rowid for the two-phase write.
    /// Populated by do_start; consumed (first-caller-wins) by record_first_response.
    pending_first_response: StdMutex<HashMap<LspKey, i64>>,
    /// Reason for the next cold start of a given key, set by eviction paths before
    /// removing the client. Consumed by do_start (defaults to "new_session" if absent).
    pub(crate) pending_reason: StdMutex<HashMap<LspKey, String>>,
    /// Project root for production usage.db writes. Set at construction time via new_arc_with_root.
    project_root: Option<std::path::PathBuf>,
    /// Circuit-breaker: tracks consecutive startup failures per key.
    /// After `CIRCUIT_BREAKER_MAX_FAILURES` failures within `CIRCUIT_BREAKER_WINDOW`,
    /// get_or_start returns an error immediately instead of spawning another process.
    /// Reset on successful start or after the window expires.
    startup_failures: StdMutex<HashMap<LspKey, (usize, Instant)>>,
    /// Cold-start grace period per key. Set by do_start on successful initialization.
    /// While Instant::now() < cold_start_until[key], startup failures are not counted
    /// toward the circuit-breaker — the server may still be indexing (e.g. Gradle import)
    /// and transient crashes during that window should not trip the breaker prematurely.
    cold_start_until: StdMutex<HashMap<LspKey, Instant>>,
    /// When a mux another process owns last stayed unreachable for a FULL peer-startup wait.
    /// Two callers loop `get_or_start` per file, so without this every file paid the whole
    /// wait; within `MUX_WEDGED_MEMORY` later attempts make one connect attempt and fail fast
    /// (a mux that has come up since still connects). Cleared on a successful connect.
    mux_wedged_at: StdMutex<HashMap<LspKey, tokio::time::Instant>>,
    /// Test seam: run synchronously when a wait on a peer's mux ends because its holder died,
    /// before the next round claims the lock. The window it opens (a third process taking the
    /// lock) has no yield point in the natural flow, so a test cannot land in it otherwise.
    #[cfg(test)]
    after_early_stop: StdMutex<Option<Arc<dyn Fn() + Send + Sync>>>,
    /// Project root for test-only DB writes. Set by new_for_test_with_root.
    #[cfg(test)]
    project_root_for_test: Option<std::path::PathBuf>,
}

impl Default for LspManager {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for LspKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.language, self.project_root.display())
    }
}

/// RAII guard that removes a language entry from the `starting` barrier map
/// when dropped, regardless of how the enclosing scope exits (success, error,
/// or async cancellation).
///
/// This prevents a stale closed-channel entry from accumulating in the map
/// when `do_start` is cancelled mid-flight by a tool timeout.
struct StartingCleanup<'a> {
    starting: &'a StdMutex<HashMap<LspKey, tokio::sync::watch::Receiver<Option<bool>>>>,
    key: LspKey,
}

impl Drop for StartingCleanup<'_> {
    fn drop(&mut self) {
        // best-effort: if another task won the race and re-inserted a live
        // entry while this guard was cancellation-dropped, we leave it alone.
        // In tokio's cooperative scheduling, the cancellation drop runs
        // synchronously inside the current poll — no other task can interleave
        // between the timeout firing and this Drop executing, so in practice
        // this always removes the stale entry and never removes a live one.
        if let Ok(mut map) = self.starting.lock() {
            map.remove(&self.key);
        }
    }
}

/// Max lines of the mux child's stderr retained for diagnostics on failure.
#[cfg(unix)]
const MUX_STDERR_TAIL_LINES: usize = 40;

/// True when a mux/LSP startup failure was caused by the workspace's index lock
/// already being held — RocksDB `LOCK` (`EAGAIN`/"Resource temporarily unavailable")
/// or the mux's own flock. In that case a direct-LSP fallback against the same
/// shared index is futile and would leave a squatter that deadlocks every future
/// mux, so the caller surfaces the error instead of falling back.
#[cfg(unix)]
pub(super) fn mux_failure_is_index_contention(detail: &str) -> bool {
    detail.contains("Resource temporarily unavailable")
        || detail.contains("RocksDBException")
        || detail.contains("another mux instance holds the lock")
}

/// Build a `(message, hint)` describing a mux startup failure from the (possibly
/// empty) stdout "ready" line and the tail of the mux child's captured stderr.
/// Pure so the classification is unit-tested; `get_or_start_via_mux` only formats.
#[cfg(unix)]
pub(super) fn mux_failure_report(stdout_line: &str, stderr_tail: &[String]) -> (String, String) {
    let detail = stderr_tail.join("\n");
    // Prefer the mux's own stderr (the real cause) over the empty stdout line.
    let summary = if !detail.trim().is_empty() {
        detail.trim().to_string()
    } else if !stdout_line.trim().is_empty() {
        stdout_line.trim().to_string()
    } else {
        "(no diagnostic output — mux exited silently)".to_string()
    };
    let message = format!("mux process failed to start: {summary}");
    let hint = if mux_failure_is_index_contention(&format!("{stdout_line}\n{detail}")) {
        "The workspace's LSP index is locked by another running server (RocksDB/mux lock). \
         A stale or concurrent LSP for this workspace holds it. Close other sessions on this \
         workspace, or locate the holder with `fuser <lsp-home>/.../rocks/*/LOCK` and stop it, \
         then retry."
            .to_string()
    } else {
        "Check that another codescout mux isn't already running for this workspace and that \
         the lock-file directory is writable; the detail above is the mux child's own stderr."
            .to_string()
    };
    (message, hint)
}
/// Non-blocking POSIX (`fcntl(F_SETLK, F_WRLCK)`) whole-file write-lock probe —
/// the SAME lock family RocksDB uses, so it detects a held RocksDB index `LOCK`
/// (which `fs4`'s `flock`-based API is blind to). Opens `path` and attempts a
/// write lock; if it would block (`EAGAIN`/`EACCES`) another process holds it →
/// returns `true`. On success it releases immediately (drops the fd) so the probe
/// never becomes a squatter. Returns `false` if the file can't be opened.
#[cfg(unix)]
fn posix_write_lock_is_held(path: &std::path::Path) -> bool {
    use std::os::unix::io::AsRawFd;
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
    {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut fl: libc::flock = unsafe { std::mem::zeroed() };
    fl.l_type = libc::F_WRLCK as libc::c_short;
    fl.l_whence = libc::SEEK_SET as libc::c_short;
    fl.l_start = 0;
    fl.l_len = 0; // 0 = lock to EOF → whole file (matches RocksDB)
    let rc = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETLK, &fl) };
    if rc == -1 {
        return matches!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::EAGAIN) | Some(libc::EACCES)
        );
    }
    // Acquired → nobody else holds it. Release before returning.
    let mut unlock: libc::flock = unsafe { std::mem::zeroed() };
    unlock.l_type = libc::F_UNLCK as libc::c_short;
    unlock.l_whence = libc::SEEK_SET as libc::c_short;
    unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETLK, &unlock) };
    false
}

/// True when the kotlin workspace's RocksDB analyzer index `LOCK` is held by
/// another process. Detection-by-state: robust to *where* a failing kotlin-lsp
/// logged its `RocksDBException` (stderr vs `intellij-server.log`), unlike the
/// stderr-signature match in [`mux_failure_is_index_contention`], which misses the
/// "initialize response missing 'result'" failure mode (verified by the live repro
/// in issues/2026-06-11-mux-failure-masks-rocksdb-lock-collision). Returns `false`
/// for non-kotlin languages and when no analyzer home / `LOCK` exists.
#[cfg(unix)]
fn kotlin_index_lock_held(language: &str, workspace_root: &std::path::Path) -> bool {
    if language != "kotlin" {
        return false;
    }
    let ws_hash = crate::lsp::mux::workspace_hash(workspace_root);
    let analyzer_dir =
        crate::lsp::servers::kotlin_analyzer_home(&ws_hash).join(".config/JetBrains/analyzer");
    if !analyzer_dir.exists() {
        return false;
    }
    walkdir::WalkDir::new(&analyzer_dir)
        .into_iter()
        .filter_map(Result::ok)
        .any(|e| e.file_name() == "LOCK" && posix_write_lock_is_held(e.path()))
}

/// PIDs (other than ours) holding an open fd on `lock_path`, via `/proc/<pid>/fd`.
/// Returns empty if `/proc` is unreadable (cannot reap → caller proceeds to spawn).
#[cfg(unix)]
fn pids_holding_fd_on(lock_path: &std::path::Path, canon: &std::path::Path) -> Vec<i32> {
    let mut pids = Vec::new();
    let Ok(proc_dir) = std::fs::read_dir("/proc") else {
        return pids;
    };
    for entry in proc_dir.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(|s| s.parse::<i32>().ok()) else {
            continue;
        };
        if pid == std::process::id() as i32 {
            continue;
        }
        let Ok(fds) = std::fs::read_dir(entry.path().join("fd")) else {
            continue;
        };
        for fd in fds.flatten() {
            if let Ok(target) = std::fs::read_link(fd.path()) {
                if target == *canon || target == *lock_path {
                    pids.push(pid);
                    break;
                }
            }
        }
    }
    pids
}

/// Terminate every process holding an open fd on `lock_path`: SIGTERM, a 2s
/// grace, then SIGKILL any process STILL holding it (re-scanned — a PID that
/// exited and was possibly reused during the grace is never killed). Returns
/// whether any holder was signalled; `Ok(false)` if the lock is unheld or no
/// holder fd can be identified.
///
/// SAFETY of killing: the analyzer home this lock lives under is codescout-private
/// (`<cache>/codescout/kotlin-lsp-home/<ws_hash>/…`), so any holder is a
/// codescout-spawned JVM, never a user's own IDE/standalone kotlin-lsp. Callers
/// reach this only after observing the mux ownership flock was free at probe time
/// (no mux was running); the flock is released before reaping so the mux child can
/// re-acquire it. Together: a holder here is a dead-mux orphan, not a live server.
#[cfg(unix)]
async fn reap_holders_of_lock(lock_path: &std::path::Path) -> anyhow::Result<bool> {
    if !posix_write_lock_is_held(lock_path) {
        return Ok(false);
    }
    let canon = std::fs::canonicalize(lock_path).unwrap_or_else(|_| lock_path.to_path_buf());
    let holders = pids_holding_fd_on(lock_path, &canon);
    if holders.is_empty() {
        // Held per the POSIX probe but no /proc fd match (perms, or /proc
        // unreadable) — cannot identify a holder to reap. Proceed to spawn.
        return Ok(false);
    }
    for &pid in &holders {
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }
        tracing::warn!("reaped orphan index-lock holder pid={pid} (SIGTERM)");
    }
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    // Re-scan and SIGKILL only PIDs STILL holding an fd on this lock. A PID that
    // exited during the grace (and may have been reused by an unrelated process)
    // is no longer in the holder set, so we never SIGKILL the wrong process.
    for pid in pids_holding_fd_on(lock_path, &canon) {
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
        tracing::warn!("orphan index-lock holder pid={pid} survived SIGTERM → SIGKILL");
    }
    Ok(true)
}

/// Reap an orphaned RocksDB index-lock holder for a kotlin workspace, if any.
/// No-op for non-kotlin or when the index is free. Returns whether a reap ran.
#[cfg(unix)]
async fn reap_orphan_index_holder(
    language: &str,
    workspace_root: &std::path::Path,
) -> anyhow::Result<bool> {
    if language != "kotlin" {
        return Ok(false);
    }
    let ws_hash = crate::lsp::mux::workspace_hash(workspace_root);
    let analyzer_dir =
        crate::lsp::servers::kotlin_analyzer_home(&ws_hash).join(".config/JetBrains/analyzer");
    if !analyzer_dir.exists() {
        return Ok(false);
    }
    let mut any = false;
    for e in walkdir::WalkDir::new(&analyzer_dir)
        .into_iter()
        .filter_map(Result::ok)
    {
        if e.file_name() == "LOCK" {
            any |= reap_holders_of_lock(e.path()).await?;
        }
    }
    Ok(any)
}

/// Build the CLI argv passed to a spawned `codescout mux` child. Factored out
/// for unit-testability; `get_or_start_via_mux` is the only caller.
#[cfg(unix)]
pub(super) fn build_mux_args(
    workspace_root: &std::path::Path,
    socket_path: &std::path::Path,
    lock_path: &std::path::Path,
    config: &crate::lsp::client::LspServerConfig,
) -> Vec<String> {
    let idle = config.idle_timeout_secs.unwrap_or(300);
    let mut args = vec![
        "mux".to_string(),
        "--socket".to_string(),
        socket_path.to_string_lossy().to_string(),
        "--lock".to_string(),
        lock_path.to_string_lossy().to_string(),
        "--cwd".to_string(),
        workspace_root.to_string_lossy().to_string(),
        "--idle-timeout".to_string(),
        idle.to_string(),
    ];
    for (k, v) in &config.env {
        args.push("--env".to_string());
        args.push(format!("{k}={v}"));
    }
    args.push("--".to_string());
    args.push(config.command.clone());
    args.extend(config.args.iter().cloned());
    args
}

/// Strip the Linux `/proc/self/exe` `"<path> (deleted)"` marker. Returns the
/// underlying path when the suffix is present, else `None`.
#[cfg(unix)]
fn strip_deleted_suffix(exe: &Path) -> Option<PathBuf> {
    exe.to_string_lossy()
        .strip_suffix(" (deleted)")
        .map(PathBuf::from)
}

/// A stable, on-disk codescout binary to spawn the mux from when `current_exe()`
/// is no longer usable. Checks `$CARGO_HOME/bin/codescout` then
/// `$HOME/.cargo/bin/codescout` (the documented install symlink).
#[cfg(unix)]
fn stable_codescout_binary() -> Option<PathBuf> {
    let candidates = [
        std::env::var_os("CARGO_HOME").map(|c| PathBuf::from(c).join("bin").join("codescout")),
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo/bin/codescout")),
    ];
    candidates.into_iter().flatten().find(|p| p.exists())
}

/// Resolve the codescout binary to spawn the mux with.
///
/// Prefers `current_exe()`. But after a mid-session `cargo build` rename-replaces
/// the running binary, `/proc/self/exe` resolves to a *deleted inode*: spawning
/// it fails with `ENOENT`, surfaced as the opaque "Failed to spawn mux process"
/// (the rust-lsp-mux-spawn-fail / 3fc22ad2 family). When `current_exe()` is gone
/// we (1) strip the `" (deleted)"` marker — the same path now holds the rebuilt
/// binary — then (2) fall back to a stable install path, and only then (3) error
/// with an actionable message.
#[cfg(unix)]
fn resolve_mux_binary() -> Result<PathBuf> {
    let exe = std::env::current_exe()
        .map_err(|e| anyhow::anyhow!("failed to determine codescout binary path: {e}"))?;
    if exe.exists() {
        return Ok(exe);
    }
    if let Some(live) = strip_deleted_suffix(&exe) {
        if live.exists() {
            tracing::warn!(
                "codescout binary was replaced since this server started; spawning mux from rebuilt {}",
                live.display()
            );
            return Ok(live);
        }
    }
    if let Some(fallback) = stable_codescout_binary() {
        tracing::warn!(
            "current_exe() ({}) is gone (binary rebuilt mid-session?); spawning mux from {}",
            exe.display(),
            fallback.display()
        );
        return Ok(fallback);
    }
    anyhow::bail!(
        "codescout binary at {} is gone (rebuilt mid-session?) and no stable fallback was found — reconnect the MCP server (/mcp) to load the rebuilt binary",
        exe.display()
    )
}

/// Resolve the effective `mux` flag. `override_` (from project config) wins; else fall back to `default`.
pub(super) fn resolve_mux_flag(default: bool, override_: Option<bool>) -> bool {
    override_.unwrap_or(default)
}

/// True when the current executable lives in cargo's test-binary directory
/// (`target/<profile>/deps/`) — i.e. a `cargo test` runner, where spawning a
/// `codescout mux` child (via `current_exe()`) would re-exec the test binary.
/// The direct-LSP fallback in `get_or_start` is retained ONLY for this case.
///
/// We classify by LOCATION, not basename: cargo names this crate's test binary
/// `codescout-<hash>` (the lib target is `codescout`), so a `starts_with(
/// "codescout")` check would misclassify the test runner as production. The
/// installed binary (`~/.cargo/bin/codescout`) and `cargo run` binary
/// (`target/<profile>/codescout`) are never under `deps/`.
// Live on unix and in tests; the sole non-test caller is unix-only, so the
// windows non-test lib build sees it as dead. Surfaced by the windows-gnu
// cross-compile (scripts/build-windows.sh), invisible to the Linux gate.
#[cfg_attr(windows, allow(dead_code))]
fn is_test_runner_exe(exe: &std::path::Path) -> bool {
    exe.parent()
        .and_then(|p| p.file_name())
        .map(|n| n == "deps")
        .unwrap_or(false)
}

/// Try to claim the mux ownership flock at `lock_path`.
///
/// `Ok(Some(file))` — the BSD `flock` was free (no live mux owns this workspace);
/// the caller drops the handle to release it so the mux child can re-acquire.
/// `Ok(None)` — the flock is held (a live process owns it). `Err` only when the
/// lock file itself cannot be opened.
///
/// This is the liveness *arbiter*: the flock says whether a live process claims
/// ownership, but NOT whether its socket is reachable. The caller pairs this with
/// an actual socket connect to tell a healthy mux from a wedged one
/// (issues/2026-06-11-mux-failure-masks-rocksdb-lock-collision, defect #1).
#[cfg(unix)]
fn claim_mux_lock(lock_path: &std::path::Path) -> std::io::Result<Option<std::fs::File>> {
    use fs4::fs_std::FileExt;
    use std::os::unix::fs::OpenOptionsExt;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .mode(0o600)
        .open(lock_path)?;
    match file.try_lock_exclusive() {
        Ok(()) => Ok(Some(file)),
        Err(_) => Ok(None),
    }
}

/// The rounds of one call ran out because the mux lock changed hands in every one of them: each
/// holder that was waited on exited, and the next process took the lock. Nothing is known to be
/// wedged and the lock may be free by now, so unlike [`mux_socket_unreachable_error`] this names
/// no holder and gives no `fuser` hint.
#[cfg(unix)]
fn mux_lock_changed_hands_error(language: &str, socket_path: &std::path::Path) -> anyhow::Error {
    crate::tools::RecoverableError::with_hint(
        format!(
            "the mux lock for {language} changed hands in each of {MUX_CONNECT_ROUNDS} rounds \
             (every holder waited on exited) and no mux is serving {}",
            socket_path.display(),
        ),
        "Muxes for this workspace are starting and exiting in turn. Retry in a moment; if it \
         repeats, look for a crash loop in the LSP server's own log.",
    )
    .into()
}

/// Actionable error for defect #1: a live process holds the mux flock but its
/// socket never accepted across the connect retries — a wedged or mid-restart
/// mux. A respawn is impossible (the held lock blocks it), so name the situation
/// and route the human to the holder instead of returning a bare connect error.
#[cfg(unix)]
fn mux_socket_unreachable_error(
    language: &str,
    socket_path: &std::path::Path,
    lock_path: &std::path::Path,
) -> anyhow::Error {
    crate::tools::RecoverableError::with_hint(
        format!(
            "mux for {language} holds its lock ({}) but its socket ({}) is \
             unreachable — the mux process is wedged or mid-restart",
            lock_path.display(),
            socket_path.display(),
        ),
        "A live process holds this workspace's mux lock yet isn't accepting \
         connections. Wait a few seconds and retry (it may be restarting); if it \
         persists, locate the holder with `fuser` on the lock file and stop it, \
         then retry.",
    )
    .into()
}
/// How long a client keeps retrying a mux socket it did NOT just start itself.
///
/// The mux takes its ownership flock FIRST and binds its socket only after the language server
/// has answered `initialize` (`src/lsp/mux/process.rs`, steps 1 and 4) — seconds for kotlin-lsp's
/// JVM. A second process arriving in that window sees a held lock and no socket, which is a mux
/// that is STARTING, not one that is wedged. Measured 2026-10-17 on kotlin-lsp: a `symbols` call
/// 1 s after the call that spawned the mux failed with "holds its lock … socket … is
/// unreachable" and the next call, a second later, succeeded.
// cap-class: NOT_A_CAP — bound on how long a client waits for another process's startup; it picks when the wedged-mux error is raised, never what an answer contains
#[cfg(unix)]
const MUX_PEER_STARTUP_WAIT: Duration = Duration::from_secs(30);

/// How long a full peer-startup wait that ended unreachable is remembered (see
/// `LspManager::mux_wedged_at`): within it, later attempts for the same workspace make one
/// connect attempt and fail fast.
///
/// Why the wait is 30 s and not the 120 s a SPAWNING client allows its own mux: the wait is paid
/// inside one tool call, and the server's tool timeout is 60 s (its message tells the caller to
/// raise `tool_timeout_secs`). One wait must leave room for the rest of the call, and the
/// error it ends in says to retry, which now connects at once if the mux has finished starting.
// cap-class: NOT_A_CAP — how long a failed wait is remembered; it picks whether a later call re-serves the wait, never what an answer contains
#[cfg(unix)]
const MUX_WEDGED_MEMORY: Duration = Duration::from_secs(60);

/// Retry window after WE spawned the mux and saw its `ready` line: the socket is already bound,
/// so this only absorbs the accept race.
#[cfg(unix)]
const MUX_SPAWNED_CONNECT_WINDOW: Duration = Duration::from_millis(800);

/// Pause between two connect attempts.
#[cfg(unix)]
const MUX_CONNECT_RETRY_INTERVAL: Duration = Duration::from_millis(200);

/// Most lock-arbitration rounds one `get_or_start_via_mux` call makes. Two are the normal ceiling
/// (wait on a peer, then respawn or report it wedged); the third exists for a lock that changed
/// hands in between.
// cap-class: NOT_A_CAP — round ceiling; exhaustion returns the wedged-or-mid-restart error, never a shorter answer
#[cfg(unix)]
const MUX_CONNECT_ROUNDS: u32 = 3;

/// The least time one attempt gives a mux to send its init message after accepting, however
/// little of the window is left: a window of zero (a remembered wedge) still lets a mux that is
/// up answer. A judgement, not a measurement against a real mux: the stand-ins in the tests
/// answer at once.
// cap-class: NOT_A_CAP — floor on a wait for a peer's first message; expiry surfaces as a failed connect attempt, never as a shorter answer
#[cfg(unix)]
const MUX_INIT_READ_MIN: Duration = Duration::from_millis(500);

/// How long one connect attempt waits for the mux's init message: what is left of the caller's
/// window, but at least `min_init_read`. The attempt may therefore end up to `min_init_read`
/// after the window does; with the loop's 200 ms pause a call can end up to
/// `MUX_CONNECT_RETRY_INTERVAL + min_init_read` after its window (derived from the code, not
/// measured).
#[cfg(unix)]
fn mux_init_read_bound(remaining: Duration, min_init_read: Duration) -> Duration {
    remaining.max(min_init_read)
}

/// How one round of `get_or_start_via_mux_waiting` connects: for how long, with what floor for
/// the init read, and whether the mux lock is checked between attempts.
#[cfg(unix)]
#[derive(Debug, PartialEq, Eq)]
struct MuxConnectPlan {
    window: Duration,
    min_init_read: Duration,
    check_holder: bool,
}

/// The plan for a round. `left` is what is left of the call's one wait budget.
///
/// - We spawned the mux and saw its `ready` line: the socket is bound, so the window only absorbs
///   the accept race, there is no other holder to check, and the init read gets the full default
///   bound (`MUX_INIT_READ_TIMEOUT`): this path had no bound on that read before 2026-10-18 and
///   a mux that is up has nothing to wait for but a busy machine.
/// - A remembered wedge: one attempt (a mux that has come up since still connects).
/// - Otherwise a peer's mux that may still be starting: what is left of the budget.
#[cfg(unix)]
fn plan_mux_connect(need_spawn: bool, remembered_wedge: bool, left: Duration) -> MuxConnectPlan {
    if need_spawn {
        MuxConnectPlan {
            window: MUX_SPAWNED_CONNECT_WINDOW,
            min_init_read: crate::lsp::client::MUX_INIT_READ_TIMEOUT,
            check_holder: false,
        }
    } else if remembered_wedge {
        MuxConnectPlan {
            window: Duration::ZERO,
            min_init_read: MUX_INIT_READ_MIN,
            check_holder: true,
        }
    } else {
        MuxConnectPlan {
            window: left,
            min_init_read: MUX_INIT_READ_MIN,
            check_holder: true,
        }
    }
}

/// Why [`connect_to_mux`] gave up: the last connect error, and whether it stopped because the
/// holder of the mux lock turned out to be gone (the lock was found free) rather than because the
/// whole window passed. Only the second is evidence of a wedged mux.
#[cfg(unix)]
#[derive(Debug)]
struct MuxConnectFailure {
    source: anyhow::Error,
    holder_gone: bool,
}

#[cfg(unix)]
impl std::fmt::Display for MuxConnectFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#}", self.source)
    }
}

/// Connect to a mux socket, retrying for up to `window`.
///
/// `holder_lock` is the mux's ownership lock when the caller is waiting on ANOTHER process's mux:
/// once that lock is free its holder has died, no socket will ever appear, and the wait ends at
/// once so the caller can respawn instead of sitting out the window. The lock is checked after
/// EVERY failed attempt, including the one that finds the deadline passed, so a holder that died
/// during the last interval is reported gone rather than wedged.
///
/// An attempt that connects but gets no init message (a mux wedged after `bind`) is bounded by
/// the window still left, but never by less than `min_init_read` (see [`mux_init_read_bound`]),
/// so it is a failed attempt like any other: the lock is checked, and the window ends it.
#[cfg(unix)]
async fn connect_to_mux(
    socket_path: &std::path::Path,
    workspace_root: &Path,
    window: Duration,
    holder_lock: Option<&std::path::Path>,
    min_init_read: Duration,
) -> std::result::Result<LspClient, MuxConnectFailure> {
    // tokio's clock, so a test can serve the production window on a paused clock.
    let deadline = tokio::time::Instant::now() + window;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let err = match LspClient::connect_with_init_timeout(
            socket_path,
            workspace_root.to_path_buf(),
            mux_init_read_bound(remaining, min_init_read),
        )
        .await
        {
            Ok(client) => return Ok(client),
            Err(e) => e,
        };
        // A free lock means the holder is gone: stop waiting, the caller re-arbitrates.
        if let Some(lock) = holder_lock {
            if matches!(claim_mux_lock(lock), Ok(Some(_))) {
                return Err(MuxConnectFailure {
                    source: err,
                    holder_gone: true,
                });
            }
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(MuxConnectFailure {
                source: err,
                holder_gone: false,
            });
        }
        tokio::time::sleep(MUX_CONNECT_RETRY_INTERVAL).await;
    }
}

impl LspManager {
    /// Maximum consecutive startup failures before the circuit-breaker trips.
    // cap-class: NOT_A_CAP — consecutive-startup-failure ceiling; tripping returns an error naming the breaker rather than a shorter answer
    const CIRCUIT_BREAKER_MAX_FAILURES: usize = 5;

    /// Time window for the circuit-breaker. Failures older than this are forgotten.
    const CIRCUIT_BREAKER_WINDOW: Duration = Duration::from_secs(60);

    /// Grace period after a successful LSP init during which startup failures are
    /// not counted toward the circuit-breaker. Covers the post-init indexing phase
    /// (e.g. kotlin-lsp Gradle import: 1–5 min). Matches the cold-start retry
    /// window in `LspClient::request()`.
    const COLD_START_GRACE: Duration = Duration::from_secs(5 * 60);

    /// Default idle TTL for LSP clients. Both `new()` and `new_arc()` use this
    /// value so tests and production see consistent behaviour.
    pub const DEFAULT_IDLE_TTL: Duration = Duration::from_secs(30 * 60);

    pub fn new() -> Self {
        Self {
            clients: Mutex::new(HashMap::new()),
            last_used: Mutex::new(HashMap::new()),
            starting: StdMutex::new(HashMap::new()),
            max_clients: 10,
            idle_ttl: Self::DEFAULT_IDLE_TTL,
            pending_first_response: StdMutex::new(HashMap::new()),
            pending_reason: StdMutex::new(HashMap::new()),
            project_root: None,
            startup_failures: StdMutex::new(HashMap::new()),
            cold_start_until: StdMutex::new(HashMap::new()),
            mux_wedged_at: StdMutex::new(HashMap::new()),
            #[cfg(test)]
            after_early_stop: StdMutex::new(None),
            #[cfg(test)]
            project_root_for_test: None,
        }
    }

    /// Fold a mux-side memory kill into the startup circuit breaker, then clear it.
    ///
    /// The mem-kill watchdog lives in the mux process and the respawn is driven from
    /// here, so the two never met: every respawn after a kill *succeeds*, which resets
    /// the breaker, and the kill → respawn → grow → kill cycle runs at whatever rate
    /// the LSP can reach the ceiling. The `-Xmx` cap and the kill ceiling bound how
    /// much memory one instance takes; nothing bounded how often.
    ///
    /// Counted as a startup failure rather than given its own counter so a mixed run
    /// (two mem-kills, three failed starts) trips at five, not at neither. They are
    /// the same thing from a caller's side: this language is not staying up.
    ///
    /// The marker is consumed even when it is too old to count — a stale file left by
    /// a kill an hour ago must not sit there and inflate the next unrelated burst.
    fn count_mem_kill_marker(
        &self,
        key: &LspKey,
        language: &str,
        workspace_root: &std::path::Path,
    ) {
        let lock_path = crate::lsp::mux::lock_path_for_workspace(language, workspace_root);
        let marker = crate::lsp::mux::memkill_path_for_lock(&lock_path);
        let Ok(meta) = std::fs::metadata(&marker) else {
            return;
        };
        let fresh = meta
            .modified()
            .ok()
            .and_then(|m| m.elapsed().ok())
            .is_some_and(|age| age < Self::CIRCUIT_BREAKER_WINDOW);
        let _ = std::fs::remove_file(&marker);
        if !fresh {
            return;
        }
        let mut failures = self
            .startup_failures
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let entry = failures
            .entry(key.clone())
            .or_insert((0, std::time::Instant::now()));
        entry.0 += 1;
        tracing::warn!(
            "LSP {} was killed by the memory watchdog; counting toward the circuit-breaker ({}/{})",
            language,
            entry.0,
            Self::CIRCUIT_BREAKER_MAX_FAILURES
        );
    }

    /// Get an existing client for the language, or start one.
    ///
    /// If the existing client has a different workspace root or has crashed,
    /// it is replaced with a new instance.
    ///
    /// The mutex is held only for the fast cache check, not during the slow
    /// LSP process startup.  This allows concurrent cold-starts for different
    /// languages to proceed in parallel.
    pub async fn get_or_start(
        &self,
        language: &str,
        workspace_root: &Path,
        mux_override: Option<bool>,
    ) -> Result<Arc<LspClient>> {
        let key = LspKey::new(language, workspace_root);

        // Fast path: cache hit.
        {
            let clients = self.clients.lock().await;
            if let Some(client) = clients.get(&key) {
                if client.is_alive() {
                    // Update last_used outside clients lock to avoid deadlock.
                    drop(clients);
                    self.last_used
                        .lock()
                        .await
                        .insert(key.clone(), Instant::now());
                    // Re-fetch since we dropped the lock (another task could have
                    // evicted it, but that's extremely unlikely and we'd just
                    // fall through to the slow path).
                    let clients = self.clients.lock().await;
                    if let Some(client) = clients.get(&key) {
                        return Ok(client.clone());
                    }
                }
            }
        }

        // A mem-kill by the mux's watchdog is a failure the breaker never saw. The
        // kill happens in another process and the respawn that follows SUCCEEDS, so
        // `do_start` resets the counter and kill -> respawn -> grow -> kill repeats
        // unthrottled: bounded in blast radius by the ceiling, but not in frequency.
        // Consume the marker here, before the check below, so the existing breaker
        // does the throttling rather than a second mechanism competing with it.
        self.count_mem_kill_marker(&key, language, workspace_root);

        // Circuit-breaker: if this language has failed too many times recently,
        // stop spawning processes and return a clear error.
        {
            let failures = self
                .startup_failures
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some((count, first_failure)) = failures.get(&key) {
                if first_failure.elapsed() < Self::CIRCUIT_BREAKER_WINDOW
                    && *count >= Self::CIRCUIT_BREAKER_MAX_FAILURES
                {
                    return Err(crate::tools::RecoverableError::with_hint(
                        format!(
                            "LSP server for {} failed to start {} times in {}s — circuit-breaker open",
                            language,
                            count,
                            first_failure.elapsed().as_secs(),
                        ),
                        format!(
                            "Another process may hold the workspace lock. Check for other \
                             codescout instances or editors targeting this project. The breaker \
                             resets after {}s of inactivity.",
                            Self::CIRCUIT_BREAKER_WINDOW.as_secs()
                        ),
                    )
                    .into());
                }
            }
        }

        // Resolve the server config early — fail fast for unknown languages
        // before touching the barrier map at all.
        let mut config = servers::default_config(language, workspace_root).ok_or_else(|| {
            anyhow::anyhow!("No LSP server configured for language: {}", language)
        })?;

        // Apply per-project mux override from project config (if any).
        config.mux = resolve_mux_flag(config.mux, mux_override);

        // Mux path: languages that use the multiplexer bypass the normal pool.
        // The fast-path cache check at the top of get_or_start() handles
        // subsequent calls within the same session.
        #[cfg(unix)]
        if config.mux {
            let mux_start_time = std::time::Instant::now();
            match self
                .get_or_start_via_mux(language, workspace_root, config.clone())
                .await
            {
                Ok(client) => {
                    // Cache the mux client so subsequent calls hit the fast path
                    let key = LspKey::new(language, workspace_root);
                    {
                        let mut clients = self.clients.lock().await;
                        clients.insert(key.clone(), client.clone());
                    }
                    self.last_used
                        .lock()
                        .await
                        .insert(key.clone(), Instant::now());
                    self.record_lsp_success(
                        &key,
                        mux_start_time.elapsed().as_millis() as i64,
                        "new_session",
                    )
                    .await;
                    return Ok(client);
                }
                Err(e) => {
                    let key = LspKey::new(language, workspace_root);
                    let handshake_ms = mux_start_time.elapsed().as_millis() as i64;
                    // If the mux failed because the workspace's LSP index lock is
                    // already held, a direct fallback would also fail to open the
                    // index AND leave a squatter that deadlocks every future mux.
                    // Surface the (now-actionable) error instead of poison-falling-back.
                    if mux_failure_is_index_contention(&e.to_string()) {
                        self.record_lsp_failure(&key, handshake_ms, "new_session", &e.to_string())
                            .await;
                        return Err(e);
                    }
                    // The stderr-signature check above is timing/log-routing fragile:
                    // when the kotlin LSP fails to open RocksDB *during initialize* the
                    // mux reports a generic "initialize response missing 'result'" and the
                    // RocksDBException lands in intellij-server.log, not the drained stderr
                    // (issues/2026-06-11-mux-failure-masks-rocksdb-lock-collision). Probe the
                    // index LOCK directly (detection-by-state) before falling back.
                    if kotlin_index_lock_held(language, workspace_root) {
                        self.record_lsp_failure(&key, handshake_ms, "new_session", &e.to_string())
                            .await;
                        return Err(crate::tools::RecoverableError::with_hint(
                            format!(
                                "kotlin LSP index is locked by another process — the mux \
                                 could not start: {e}"
                            ),
                            "Another kotlin-lsp holds this workspace's RocksDB index lock. \
                             Close other sessions on this workspace, or locate the holder \
                             with `fuser <kotlin-lsp-home>/.../rocks/*/LOCK` and stop it, \
                             then retry.",
                        )
                        .into());
                    }
                    // For mux languages, a silent direct fallback spawns a
                    // competing LSP on the shared index (S3) — refuse it in
                    // production. Retain the fallback ONLY when current_exe() is a
                    // test runner (spawning a `codescout mux` child would re-exec
                    // the test binary). See docs/adrs/2026-06-11-mux-single-owner-invariant.md (S3).
                    let exe_is_test = std::env::current_exe()
                        .map(|p| is_test_runner_exe(&p))
                        .unwrap_or(false); // fail closed: unresolved exe ⇒ treat as prod ⇒ refuse the fallback
                    if !exe_is_test {
                        self.record_lsp_failure(&key, handshake_ms, "new_session", &e.to_string())
                            .await;
                        return Err(crate::tools::RecoverableError::with_hint(
                            format!("mux startup failed for {language}: {e}"),
                            "codescout will not fall back to a direct LSP for a \
                             multiplexed language — that would open a second process \
                             on the shared index. Retry in a moment; if it persists, \
                             check for an orphaned LSP with \
                             `fuser <kotlin-lsp-home>/.../rocks/*/LOCK` and stop it.",
                        )
                        .into());
                    }
                    tracing::warn!(
                        "Mux startup failed for {language} in a test runner, \
                         falling back to direct LSP: {e}"
                    );
                    config.mux = false;
                }
            }
        }

        // LRU eviction: if at capacity, shut down the least-recently-used client.
        self.evict_lru_if_at_capacity().await;

        // Slow path: need to start (or wait for someone else starting).
        // Use a per-key watch channel: the first caller creates a sender,
        // concurrent callers clone the receiver and wait. Unlike Notify, watch
        // channels never lose signals — late subscribers always see the value.
        let mut rx_opt = None;
        let tx_opt;
        {
            let mut starting = self.starting.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(existing_rx) = starting.get(&key) {
                // Someone else is already starting this key — grab a receiver.
                rx_opt = Some(existing_rx.clone());
                tx_opt = None;
            } else {
                // We're the first — create the channel and register.
                let (tx, rx) = tokio::sync::watch::channel(None);
                starting.insert(key.clone(), rx);
                tx_opt = Some(tx);
            }
        }

        // If we're a waiter, wait for the starter to finish.
        if let Some(mut rx) = rx_opt {
            // Wait until the value changes from None to Some(bool).
            let _ = rx.wait_for(|v| v.is_some()).await;
            // Check the cache — starter should have inserted on success.
            // IMPORTANT: scope the lock so it drops before any call to do_start,
            // which also locks `self.clients`. Tokio Mutex is not reentrant —
            // holding it while calling do_start would deadlock.
            {
                let clients = self.clients.lock().await;
                if let Some(client) = clients.get(&key) {
                    if client.is_alive() {
                        return Ok(client.clone());
                    }
                }
            }
            // Starter failed or client doesn't match — fall through to try ourselves.
            // Clean up the old barrier and register as a new starter.
            let (tx, rx) = tokio::sync::watch::channel(None);
            {
                let mut starting = self.starting.lock().unwrap_or_else(|e| e.into_inner());
                starting.insert(key.clone(), rx);
            }
            return self.do_start(&key, config, tx).await;
        }

        // We're the starter.
        self.do_start(&key, config, tx_opt.expect("tx_opt is always Some when rx_opt is None — set in the same exclusive branch above"))
            .await
    }

    /// LRU eviction: if the pool is at capacity, shut down the least-recently-used
    /// client to make room. Extracted from `get_or_start` to keep that function
    /// under the symbol inline budget.
    async fn evict_lru_if_at_capacity(&self) {
        // LRU eviction: if at capacity, shut down the least-recently-used client.
        // Lock ordering: never nest clients → last_used.  Check capacity first,
        // find the oldest under last_used alone, then re-acquire clients to remove.
        let evict_info: Option<(LspKey, Option<Arc<LspClient>>)> = {
            let at_capacity = self.clients.lock().await.len() >= self.max_clients;
            if at_capacity {
                // Find the LRU key under last_used lock alone.
                // I-2: cost-aware selection — sort by (restart_cost_tier ASC, last_used ASC).
                // Cheap-restart languages get evicted before Kotlin/Java; only when
                // every pool entry is expensive do we evict an expensive one by LRU.
                let oldest_key = {
                    let last_used = self.last_used.lock().await;
                    last_used
                        .iter()
                        .min_by_key(|(k, t)| (restart_cost_tier(&k.language), *t))
                        .map(|(k, _)| k.clone())
                };
                if let Some(oldest_key) = oldest_key {
                    let mut clients = self.clients.lock().await;
                    // Re-check: another task may have evicted between locks.
                    if clients.len() >= self.max_clients {
                        self.pending_reason
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .insert(oldest_key.clone(), "lru_evicted".to_string());
                        self.pending_first_response
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .remove(&oldest_key);
                        let evict_client = clients.remove(&oldest_key);
                        Some((oldest_key, evict_client))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };
        if let Some((oldest_key, evict_client)) = evict_info {
            self.last_used.lock().await.remove(&oldest_key);
            if let Some(old) = evict_client {
                tracing::info!("LRU evicting LSP client: {}", oldest_key);
                let _ = old.shutdown().await;
            }
        }
    }

    /// Start or connect to a multiplexed LSP server.
    ///
    /// The mux process is a detached codescout child that owns the real LSP
    /// server and multiplexes connections over a Unix socket.  If no mux is
    /// running for this workspace we spawn one and wait for its "ready" line
    /// on stdout before connecting.  A mux that ANOTHER process owns and is
    /// still starting is waited for up to `MUX_PEER_STARTUP_WAIT`.
    #[cfg(unix)]
    async fn get_or_start_via_mux(
        &self,
        language: &str,
        workspace_root: &Path,
        config: LspServerConfig,
    ) -> Result<Arc<LspClient>> {
        self.get_or_start_via_mux_waiting(language, workspace_root, config, MUX_PEER_STARTUP_WAIT)
            .await
    }

    /// Run the [`Self::after_early_stop`] test seam, if one is installed.
    #[cfg(all(unix, test))]
    fn run_after_early_stop_hook(&self) {
        let hook = self
            .after_early_stop
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Some(hook) = hook {
            hook();
        }
    }

    /// [`Self::get_or_start_via_mux`] with the wait for another process's still-starting mux
    /// (`MUX_PEER_STARTUP_WAIT`) as a parameter, so a test can drive the wedged case in
    /// milliseconds instead of sitting out the production window.
    #[cfg(unix)]
    async fn get_or_start_via_mux_waiting(
        &self,
        language: &str,
        workspace_root: &Path,
        config: LspServerConfig,
        peer_startup_wait: Duration,
    ) -> Result<Arc<LspClient>> {
        use anyhow::Context;

        let socket_path = crate::lsp::mux::socket_path_for_workspace(language, workspace_root);
        let lock_path = crate::lsp::mux::lock_path_for_workspace(language, workspace_root);

        // Liveness is the flock AND socket reachability, not the flock alone
        // (defect #1, issues/2026-06-11-mux-failure-masks-rocksdb-lock-collision).
        // Round 0 may connect to an existing mux (flock held). If that mux's socket
        // is unreachable through the connect retries, round 1 re-arbitrates the
        // flock: a now-free lock means the holder died and left a stale `.sock` →
        // respawn; a still-held lock means a live but wedged mux → actionable error.
        // The exception is a holder that DIED while we waited and a lock that is held again
        // by the next round: a third process took it in the gap and its mux is starting, so
        // that one is waited for too (`MUX_CONNECT_ROUNDS` bounds how often this repeats).
        let mut last_err: Option<anyhow::Error> = None;
        // One budget for every wait on a peer's mux in this call, however many holders it
        // meets: the server's tool timeout is 60 s and one wait must leave room for the rest of
        // the call (see `MUX_WEDGED_MEMORY`).
        let wait_deadline = tokio::time::Instant::now() + peer_startup_wait;
        // Whether the last round's holder was found DEAD (its lock free) rather than wedged.
        let mut last_holder_gone = false;
        for round in 0..MUX_CONNECT_ROUNDS {
            let need_spawn =
                match claim_mux_lock(&lock_path).context("Failed to open mux lock file")? {
                    Some(guard) => {
                        // Got the lock — no live mux. Drop releases it so the mux child
                        // can acquire.
                        drop(guard);
                        true
                    }
                    None => {
                        if round > 0 && !last_holder_gone {
                            // A later round and the flock is STILL held while the socket stayed
                            // unreachable through the last round's retries → a wedged mux we
                            // cannot replace (the held lock blocks a respawn). Surface it.
                            // When the last round's holder DIED instead, a lock held now is a
                            // NEW process (a peer that spawned its own mux in the gap): its mux
                            // is starting, so it is waited for like any peer's, below.
                            return Err(mux_socket_unreachable_error(
                                language,
                                &socket_path,
                                &lock_path,
                            ));
                        }
                        tracing::info!(
                            "mux already running for {}, connecting to {:?}",
                            language,
                            socket_path
                        );
                        false
                    }
                };

            if need_spawn {
                let exe = resolve_mux_binary()?;

                // We observed the ownership flock was free (no live mux). The
                // analyzer home is codescout-private, so any process still holding
                // this workspace's RocksDB index LOCK is an orphaned dead-mux JVM.
                // Reap it before spawning, or the new mux's LSP child can't open the
                // index. (S1/S2 net.) Non-fatal: log and continue.
                match reap_orphan_index_holder(language, workspace_root).await {
                    Ok(true) => {
                        tracing::info!("reaped orphan index holder for {language} before mux spawn")
                    }
                    Ok(false) => {}
                    Err(e) => tracing::warn!("orphan reap probe failed (continuing): {e}"),
                }

                let mux_args = build_mux_args(workspace_root, &socket_path, &lock_path, &config);

                // Spawn mux as a detached process — do NOT set kill_on_drop.
                // Capture stderr: a startup failure's real cause (e.g. a held RocksDB
                // index lock) is written there, not to stdout. Without this the caller
                // only saw a blank "mux process failed to start:".
                let mut child = tokio::process::Command::new(&exe)
                    .args(&mux_args)
                    .stdout(std::process::Stdio::piped())
                    .stdin(std::process::Stdio::null())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .context("Failed to spawn mux process")?;

                // Drain the mux child's stderr into a bounded ring buffer. On success
                // the mux lives for its idle-timeout and keeps logging, so the drain
                // also prevents a full-pipe write stall; on failure it holds the cause.
                let stderr_tail = std::sync::Arc::new(std::sync::Mutex::new(
                    std::collections::VecDeque::<String>::with_capacity(MUX_STDERR_TAIL_LINES),
                ));
                let stderr_drain = child.stderr.take().map(|stderr| {
                    let tail = stderr_tail.clone();
                    tokio::spawn(async move {
                        use tokio::io::AsyncBufReadExt;
                        let mut reader = tokio::io::BufReader::new(stderr);
                        let mut line = String::new();
                        loop {
                            line.clear();
                            match reader.read_line(&mut line).await {
                                Ok(0) | Err(_) => break,
                                Ok(_) => {
                                    let mut tail = tail.lock().unwrap_or_else(|e| e.into_inner());
                                    if tail.len() == MUX_STDERR_TAIL_LINES {
                                        tail.pop_front();
                                    }
                                    tail.push_back(line.trim_end().to_string());
                                }
                            }
                        }
                    })
                });

                // Wait for the "ready" signal on stdout.
                let stdout = child.stdout.take().expect("stdout piped");
                let mut reader = tokio::io::BufReader::new(stdout);
                let mut line = String::new();
                let read_result = tokio::time::timeout(
                    std::time::Duration::from_secs(120),
                    tokio::io::AsyncBufReadExt::read_line(&mut reader, &mut line),
                )
                .await;

                let is_ready =
                    matches!(read_result, Ok(Ok(n)) if n > 0) && line.trim().starts_with("ready");
                if is_ready {
                    tracing::info!("mux process ready for {} at {:?}", language, socket_path);
                } else {
                    // Let the stderr drain settle so we capture the mux's own cause.
                    if let Some(handle) = stderr_drain {
                        let _ = tokio::time::timeout(std::time::Duration::from_millis(500), handle)
                            .await;
                    }
                    let tail: Vec<String> = stderr_tail
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .iter()
                        .cloned()
                        .collect();
                    return Err(match read_result {
                        Err(_) => {
                            let extra = if tail.is_empty() {
                                String::new()
                            } else {
                                format!(" — mux stderr: {}", tail.join(" | "))
                            };
                            crate::tools::RecoverableError::with_hint(
                                format!("mux process timed out waiting for ready (120s){extra}"),
                                "The LSP server is slow to initialize (Gradle/Cargo index?). \
                                 Retry in a moment; if the problem persists, check server logs.",
                            )
                            .into()
                        }
                        Ok(read) => {
                            let stdout_line = match read {
                                Ok(_) => line.trim().to_string(),
                                Err(e) => format!("(stdout read error: {e})"),
                            };
                            let (message, hint) = mux_failure_report(&stdout_line, &tail);
                            crate::tools::RecoverableError::with_hint(message, hint).into()
                        }
                    });
                }
                // Detach child — mux runs independently
            }

            // Connect as client, with retries. The socket — not the flock — is the
            // real liveness signal. A mux another process owns may still be starting
            // (flock held, socket not bound yet), so that case waits for it.
            let wedge_key = LspKey::new(language, workspace_root);
            let remembered_wedge = self
                .mux_wedged_at
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&wedge_key)
                .is_some_and(|at| at.elapsed() < MUX_WEDGED_MEMORY);
            let left = wait_deadline.saturating_duration_since(tokio::time::Instant::now());
            let plan = plan_mux_connect(need_spawn, remembered_wedge, left);
            let holder_lock = plan.check_holder.then_some(lock_path.as_path());
            // A round that follows a holder's death waits on a REPLACEMENT for what is left of
            // the one budget (possibly about nothing), so its outcome is not a verdict on a mux.
            let waited_on_replacement = last_holder_gone;
            match connect_to_mux(
                &socket_path,
                workspace_root,
                plan.window,
                holder_lock,
                plan.min_init_read,
            )
            .await
            {
                Ok(client) => {
                    self.mux_wedged_at
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .remove(&wedge_key);
                    return Ok(Arc::new(client));
                }
                Err(failure) => {
                    let MuxConnectFailure {
                        source: e,
                        holder_gone,
                    } = failure;
                    if need_spawn {
                        // We spawned the mux and saw its "ready", yet cannot connect
                        // — a genuine startup/connect failure, not a stale-lock case.
                        return Err(e);
                    }
                    if !remembered_wedge && !holder_gone && !waited_on_replacement {
                        // The WHOLE wait was served on the holder the call started with and it is
                        // still alive: the next callers need not serve it again. A wait that
                        // stopped early because the lock turned free found nothing wedged (its
                        // holder died), so nothing is recorded and the next round takes over; a
                        // round on a replacement holder had only what was left of the budget, so
                        // it records nothing either.
                        self.mux_wedged_at
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .insert(wedge_key, tokio::time::Instant::now());
                    }
                    // Flock held but socket unreachable. Loop to re-arbitrate (respawn if the
                    // holder is gone, wait for a new holder, else error).
                    last_holder_gone = holder_gone;
                    #[cfg(test)]
                    if holder_gone {
                        self.run_after_early_stop_hook();
                    }
                    last_err = Some(e);
                }
            }
        }

        // Reached only when the lock changed hands in every round (each holder died in turn):
        // there is no wedged holder to name, and the lock may be free now, so the error says
        // what happened and does not point at a holder.
        if let Some(e) = &last_err {
            tracing::warn!(
                "mux connect for {language} gave up after {MUX_CONNECT_ROUNDS} rounds: {e:#}"
            );
        }
        Err(mux_lock_changed_hands_error(language, &socket_path))
    }

    /// Record a successful LSP start to `lsp_events` — best-effort, never fails
    /// the caller. Shared by `do_start` (direct pool) and `get_or_start`'s mux
    /// dispatch, so both code paths that can obtain a working LSP client record
    /// the same way. `reason` labels why this start happened (see
    /// `write_lsp_event`); the mux path does not track eviction reasons the way
    /// the direct pool does, so it always passes `"new_session"` — including for
    /// a connect to an already-running mux, which simply reports a much lower
    /// `handshake_ms` than a fresh spawn.
    async fn record_lsp_success(&self, key: &LspKey, handshake_ms: i64, reason: &str) {
        tracing::info!(
            "LSP initialized in {}ms (language: {}, reason: {})",
            handshake_ms,
            key.language,
            reason
        );
        let project_root_opt = self.project_root.clone();
        #[cfg(test)]
        let project_root_opt = self.project_root_for_test.clone().or(project_root_opt);
        if let Some(root) = project_root_opt {
            let lang = key.language.clone();
            let reason = reason.to_string();
            let rowid_result = tokio::task::spawn_blocking(move || {
                let conn = crate::usage::db::open_db(&root)?;
                crate::usage::db::write_lsp_event(&conn, &lang, &reason, handshake_ms)
            })
            .await;
            if let Ok(Ok(rowid)) = rowid_result {
                self.pending_first_response
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(key.clone(), rowid);
            }
        }
    }

    /// Record a failed LSP start to `lsp_events` — best-effort, never masks the
    /// real error returned to the caller. See `record_lsp_success` for why this
    /// is shared between the direct pool (`do_start`) and the mux dispatch.
    async fn record_lsp_failure(&self, key: &LspKey, handshake_ms: i64, reason: &str, error: &str) {
        let project_root_opt = self.project_root.clone();
        #[cfg(test)]
        let project_root_opt = self.project_root_for_test.clone().or(project_root_opt);
        if let Some(root) = project_root_opt {
            let lang = key.language.clone();
            let reason = reason.to_string();
            let error = error.to_string();
            let _ = tokio::task::spawn_blocking(move || {
                let conn = crate::usage::db::open_db(&root)?;
                crate::usage::db::write_lsp_failure(&conn, &lang, &reason, handshake_ms, &error)
            })
            .await;
        }
    }

    /// Internal: actually start the LSP, update cache, and signal waiters.
    ///
    /// The `StartingCleanup` guard ensures the barrier entry is removed from
    /// `self.starting` on every exit path: success, error, **and async
    /// cancellation** (tool timeout dropping the future mid-flight).
    async fn do_start(
        &self,
        key: &LspKey,
        config: LspServerConfig,
        tx: tokio::sync::watch::Sender<Option<bool>>,
    ) -> Result<Arc<LspClient>> {
        // Register the cleanup guard first. It removes the `starting` entry
        // when this function returns or is cancelled.
        let _cleanup = StartingCleanup {
            starting: &self.starting,
            key: key.clone(),
        };

        // Evict dead client if present.
        // Remove from map first and release the lock, THEN shut down.
        // Calling shutdown().await while holding the clients lock would block
        // all other get_or_start callers for up to 35 seconds.
        let stale_client = {
            let mut clients = self.clients.lock().await;
            if let Some(client) = clients.get(key) {
                if !client.is_alive() {
                    clients.remove(key)
                } else {
                    None
                }
            } else {
                None
            }
        };
        if let Some(old) = stale_client {
            let _ = old.shutdown().await;
        }

        let start_time = std::time::Instant::now();
        let result = LspClient::start(config).await.map(Arc::new);

        match result {
            Ok(new_client) => {
                // Insert into cache BEFORE signalling waiters.
                {
                    let mut clients = self.clients.lock().await;
                    clients.insert(key.clone(), new_client.clone());
                }
                // Update last_used.
                self.last_used
                    .lock()
                    .await
                    .insert(key.clone(), Instant::now());

                // Record LSP startup event — best-effort, never fail the startup.
                let reason = self
                    .pending_reason
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(key)
                    .unwrap_or_else(|| "new_session".to_string());
                let handshake_ms = start_time.elapsed().as_millis() as i64;
                self.record_lsp_success(key, handshake_ms, &reason).await;

                // Circuit-breaker: reset on success.
                self.startup_failures
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(key);

                // Cold-start grace period: for COLD_START_GRACE after a successful
                // init, startup failures are not counted toward the circuit-breaker.
                // kotlin-lsp may still be running Gradle import (1-5 min) and could
                // crash transiently; the breaker should not trip during that window.
                self.cold_start_until
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(key.clone(), Instant::now() + Self::COLD_START_GRACE);

                // Signal success. The `starting` entry is removed by _cleanup
                // when this function returns.
                let _ = tx.send(Some(true));
                Ok(new_client)
            }
            Err(e) => {
                // Record the failed start as an `outcome='failed'` lsp_events row —
                // best-effort, never masks the real error. A server that dies during
                // `initialize` (e.g. an expired LSP build) otherwise leaves no trace in
                // lsp_events at all. Recorded independent of the cold-start grace /
                // circuit-breaker below, which gate the breaker — not observability.
                {
                    let handshake_ms = start_time.elapsed().as_millis() as i64;
                    // Peek (don't consume) pending_reason so the triggering reason
                    // still labels the eventual successful retry.
                    let reason = self
                        .pending_reason
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .get(key)
                        .cloned()
                        .unwrap_or_else(|| "new_session".to_string());
                    self.record_lsp_failure(key, handshake_ms, &reason, &e.to_string())
                        .await;
                }

                // Circuit-breaker: record failure, but skip if we're within the
                // cold-start grace period of the previous successful start — the
                // server may have crashed during Gradle import and the breaker
                // should not penalise what is effectively a transient indexing crash.
                let in_grace = self
                    .cold_start_until
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .get(key)
                    .is_some_and(|until| Instant::now() < *until);

                if in_grace {
                    tracing::info!(
                        "LSP startup failure for {} suppressed by cold-start grace period",
                        key
                    );
                } else {
                    let mut failures = self
                        .startup_failures
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    let entry = failures.entry(key.clone()).or_insert((0, Instant::now()));
                    if entry.1.elapsed() >= Self::CIRCUIT_BREAKER_WINDOW {
                        // Window expired — start a fresh count.
                        *entry = (1, Instant::now());
                    } else {
                        entry.0 += 1;
                    }
                    if entry.0 >= Self::CIRCUIT_BREAKER_MAX_FAILURES {
                        tracing::warn!(
                            "LSP circuit-breaker tripped for {} ({} failures in {}s)",
                            key,
                            entry.0,
                            entry.1.elapsed().as_secs()
                        );
                    }
                }

                // Signal failure. The `starting` entry is removed by _cleanup
                // when this function returns.
                let _ = tx.send(Some(false));
                Err(e)
            }
        }
    }

    pub async fn get(&self, language: &str, project_root: &Path) -> Option<Arc<LspClient>> {
        let key = LspKey::new(language, project_root);
        let clients = self.clients.lock().await;
        clients.get(&key).filter(|c| c.is_alive()).cloned()
    }

    /// Shut down all active LSP servers.
    pub async fn shutdown_all(&self) {
        // Drain the map under the lock, then RELEASE it before awaiting each
        // client's shutdown(). Holding `clients` across every shutdown().await
        // blocks all concurrent LSP navigation (and a post_compact flush) for
        // the full shutdown duration.
        // (docs/issues/archive/2026-07-10-lsp-shutdown-all-holds-clients-lock-across-await.md)
        let drained: Vec<(LspKey, Arc<LspClient>)> = {
            let mut clients = self.clients.lock().await;
            clients.drain().collect()
        };
        for (key, client) in drained {
            tracing::info!("Shutting down LSP for: {}", key);
            match client.shutdown().await {
                Ok(()) => tracing::debug!("LSP server shut down cleanly: {}", key),
                Err(e) => tracing::warn!("Error shutting down LSP for {}: {}", key, e),
            }
        }
        self.last_used.lock().await.clear();
    }

    /// List currently active languages (deduplicated).
    pub async fn active_languages(&self) -> Vec<String> {
        let clients = self.clients.lock().await;
        let mut langs: Vec<String> = clients
            .iter()
            .filter(|(_, c)| c.is_alive())
            .map(|(key, _)| key.language.clone())
            .collect();
        langs.sort();
        langs.dedup();
        langs
    }

    /// Notify LSP clients whose project_root is an ancestor of the changed file.
    /// Each client silently skips the file if it doesn't have it open.
    pub async fn notify_file_changed(&self, path: &std::path::Path) {
        let clients: Vec<_> = self
            .clients
            .lock()
            .await
            .iter()
            .filter(|(key, _)| path.starts_with(&key.project_root))
            .map(|(_, client)| client.clone())
            .collect();
        for client in clients {
            if client.is_alive() {
                let _ = client.did_change(path).await;
            }
        }
    }

    /// Inner implementation of first-response recording. Called by the LspProvider
    /// trait impl. Named `_inner` to avoid the infinite-recursion trap where
    /// `self.record_first_response(...)` inside a trait impl resolves back to the
    /// trait method rather than this inherent method.
    pub async fn record_first_response_inner(
        &self,
        language: &str,
        workspace_root: &std::path::Path,
        elapsed_ms: i64,
    ) {
        let key = LspKey::new(language, workspace_root);
        let pending = self
            .pending_first_response
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&key);

        let Some(rowid) = pending else { return };

        tracing::debug!(
            "LSP first response in {}ms (language: {})",
            elapsed_ms,
            language
        );

        let project_root_opt = self.project_root.clone();
        #[cfg(test)]
        let project_root_opt = self.project_root_for_test.clone().or(project_root_opt);

        let Some(root) = project_root_opt else { return };

        let _ = tokio::task::spawn_blocking(move || {
            if let Ok(conn) = crate::usage::db::open_db(&root) {
                let _ = crate::usage::db::update_lsp_first_response(&conn, rowid, elapsed_ms);
            }
        })
        .await;
    }

    /// Return the number of in-progress language starts. Should be 0 after any
    /// `get_or_start` call completes (success, failure, or cancellation).
    #[cfg(test)]
    pub fn starting_count_sync(&self) -> usize {
        self.starting.lock().unwrap().len()
    }

    /// Like `get_or_start` but accepts a custom `LspServerConfig`, bypassing
    /// `servers::default_config`. Used in tests to inject fake (e.g. `sleep`)
    /// servers so the startup can be cancelled or timed out on demand.
    #[cfg(test)]
    pub async fn get_or_start_for_test(
        &self,
        language: &str,
        config: LspServerConfig,
    ) -> Result<Arc<LspClient>> {
        let workspace_root = config.workspace_root.clone();
        let key = LspKey::new(language, &workspace_root);

        // Fast path
        {
            let clients = self.clients.lock().await;
            if let Some(client) = clients.get(&key) {
                if client.is_alive() {
                    return Ok(client.clone());
                }
            }
        }

        // Barrier
        let mut rx_opt = None;
        let tx_opt;
        {
            let mut starting = self.starting.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(existing_rx) = starting.get(&key) {
                rx_opt = Some(existing_rx.clone());
                tx_opt = None;
            } else {
                let (tx, rx) = tokio::sync::watch::channel(None);
                starting.insert(key.clone(), rx);
                tx_opt = Some(tx);
            }
        }

        if let Some(mut rx) = rx_opt {
            let _ = rx.wait_for(|v| v.is_some()).await;
            {
                let clients = self.clients.lock().await;
                if let Some(client) = clients.get(&key) {
                    if client.is_alive() {
                        return Ok(client.clone());
                    }
                }
            }
            let (tx, rx) = tokio::sync::watch::channel(None);
            {
                let mut starting = self.starting.lock().unwrap_or_else(|e| e.into_inner());
                starting.insert(key.clone(), rx);
            }
            return self.do_start(&key, config, tx).await;
        }

        self.do_start(&key, config, tx_opt.expect("tx_opt is always Some when rx_opt is None — set in the same exclusive branch above"))
            .await
    }

    #[cfg(test)]
    pub async fn new_for_test_with_root(project_root: &std::path::Path) -> Arc<Self> {
        let mut mgr = Self::new();
        mgr.project_root_for_test = Some(project_root.to_path_buf());
        Arc::new(mgr)
    }
}

impl LspManager {
    /// Shared construction: builds Arc<LspManager> with the given TTL and optional project root,
    /// spawning the idle eviction loop.
    fn new_arc_inner(ttl: Duration, project_root: Option<std::path::PathBuf>) -> Arc<Self> {
        let mut mgr = Self::new();
        mgr.idle_ttl = ttl;
        mgr.project_root = project_root;
        let arc = Arc::new(mgr);
        let weak = Arc::downgrade(&arc);
        tokio::spawn(async move {
            Self::idle_eviction_loop(weak, ttl).await;
        });
        arc
    }

    /// Create an `Arc<LspManager>` with the default 30-minute idle TTL
    /// and spawn a background eviction task.
    pub fn new_arc() -> Arc<Self> {
        Self::new_arc_inner(Self::DEFAULT_IDLE_TTL, None)
    }

    /// Create an `Arc<LspManager>` with a custom idle TTL and spawn a
    /// background eviction task.  The task holds a `Weak` reference so it
    /// exits automatically when the last `Arc` is dropped.
    pub fn new_arc_with_ttl(ttl: Duration) -> Arc<Self> {
        Self::new_arc_inner(ttl, None)
    }

    /// Production constructor: writes LSP startup timing to usage.db under `project_root`.
    pub fn new_arc_with_root(project_root: std::path::PathBuf) -> Arc<Self> {
        Self::new_arc_inner(Duration::from_secs(30 * 60), Some(project_root))
    }

    /// Evict all clients that have not been accessed for longer than `ttl`.
    /// Called periodically by the background task; also `pub(crate)` for
    /// direct testing without the background task.
    pub(crate) async fn evict_idle(&self, ttl: Duration) {
        let now = Instant::now();
        let idle_keys: Vec<LspKey> = {
            let last_used = self.last_used.lock().await;
            last_used
                .iter()
                .filter(|(k, t)| now.duration_since(**t) > ttl_for_language(&k.language, ttl))
                .map(|(k, _)| k.clone())
                .collect()
        };
        for key in idle_keys {
            let client = {
                let mut clients = self.clients.lock().await;
                self.pending_reason
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(key.clone(), "idle_evicted".to_string());
                // Discard any pending first-response entry — this key's window is over.
                self.pending_first_response
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&key);
                clients.remove(&key)
            };
            self.last_used.lock().await.remove(&key);
            if let Some(c) = client {
                tracing::info!("Idle TTL evicting LSP client: {}", key);
                let _ = c.shutdown().await;
            }
        }
    }

    /// Background loop: wakes every `ttl / 4` and calls `evict_idle`.
    /// Exits when the `Weak` can no longer be upgraded (manager dropped).
    async fn idle_eviction_loop(weak: Weak<Self>, ttl: Duration) {
        let interval = ttl / 4;
        loop {
            tokio::time::sleep(interval).await;
            match weak.upgrade() {
                Some(mgr) => mgr.evict_idle(ttl).await,
                None => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal Cargo project under `dir` and return an
    /// `LspServerConfig` for rust-analyzer, or `None` if rust-analyzer is not
    /// installed. Tests that call this must skip when `None` is returned.
    fn ra_config_or_skip(dir: &std::path::Path) -> Option<LspServerConfig> {
        use std::process::Command as StdCommand;
        if StdCommand::new("rust-analyzer")
            .arg("--version")
            .output()
            .is_err()
        {
            return None;
        }
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/lib.rs"), "pub fn f() {}").unwrap();
        Some(LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: dir.to_path_buf(),
            init_timeout: Some(std::time::Duration::from_secs(30)),
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        })
    }

    #[tokio::test]
    async fn manager_starts_empty() {
        let mgr = LspManager::new();
        assert!(mgr.active_languages().await.is_empty());
        assert!(mgr.get("rust", Path::new("/tmp")).await.is_none());
    }

    #[tokio::test]
    async fn manager_errors_for_unknown_language() {
        let mgr = LspManager::new();
        let dir = tempfile::tempdir().unwrap();
        let result = mgr.get_or_start("brainfuck", dir.path(), None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn manager_shutdown_all_empty() {
        let mgr = LspManager::new();
        mgr.shutdown_all().await; // Should not panic
    }

    /// After a failed start (unknown language), the barrier map must be empty.
    /// This is the three-query sandwich for the StartingCleanup guard:
    ///   1. starting_count == 0 (baseline)
    ///   2. get_or_start for unknown language fails quickly
    ///   3. starting_count == 0 (guard cleaned up on normal failure exit)
    #[tokio::test]
    async fn failed_start_cleans_up_starting_map() {
        let mgr = LspManager::new();
        let dir = tempfile::tempdir().unwrap();

        // Step 1 — baseline
        assert_eq!(mgr.starting_count_sync(), 0, "map should start empty");

        // Step 2 — unknown language fails immediately (no config exists)
        let result = mgr.get_or_start("brainfuck", dir.path(), None).await;
        assert!(result.is_err());

        // Step 3 — cleanup guard fired on failure exit
        assert_eq!(
            mgr.starting_count_sync(),
            0,
            "map should be clean after failed start"
        );
    }

    /// After a cancelled start (tool timeout mid-initialize), the barrier map
    /// must be empty. Without the StartingCleanup guard the stale closed-channel
    /// entry would remain in `starting` until the next caller overwrote it.
    ///
    /// Uses `sleep 99999` as a fake LSP: it starts immediately but never writes
    /// to stdout, so `initialize()` blocks until the external timeout fires.
    #[cfg_attr(
        windows,
        ignore = "`sleep` is not available in Windows cmd.exe; fake-LSP needs a platform-native infinite-blocker"
    )]
    #[tokio::test]
    async fn cancelled_get_or_start_cleans_up_starting_map() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = LspManager::new();

        // Step 1 — baseline
        assert_eq!(mgr.starting_count_sync(), 0, "map should start empty");

        let config = LspServerConfig {
            command: "sleep".into(),
            args: vec!["99999".into()],
            workspace_root: dir.path().to_path_buf(),
            // Short init timeout so the LSP-level request also fails fast,
            // but the outer tokio::time::timeout fires first.
            init_timeout: Some(std::time::Duration::from_secs(30)),
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        };

        // Step 2 — cancel the future after 100 ms (before initialize responds)
        let cancelled = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            mgr.get_or_start_for_test("fake-slow-lsp", config),
        )
        .await;
        assert!(cancelled.is_err(), "expected outer timeout");

        // Step 3 — cleanup guard must have fired during the cancellation drop
        assert_eq!(
            mgr.starting_count_sync(),
            0,
            "stale starting entry leaked after cancellation"
        );
    }

    #[tokio::test]
    async fn shutdown_all_stops_running_servers() {
        use std::process::Command as StdCommand;

        // Check if rust-analyzer is available
        if StdCommand::new("rust-analyzer")
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("Skipping: rust-analyzer not installed");
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        // Create minimal Cargo project
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "pub fn f() {}").unwrap();

        let mgr = LspManager::new();
        // Pass Some(false) so the test uses rust-analyzer directly, without
        // needing the codescout-mux binary on PATH (mux: true is now the default).
        let client = mgr
            .get_or_start("rust", dir.path(), Some(false))
            .await
            .unwrap();
        assert!(client.is_alive());

        mgr.shutdown_all().await;

        // After shutdown, the client should be dead
        assert!(!client.is_alive());
        assert!(mgr.active_languages().await.is_empty());
    }

    #[tokio::test]
    async fn same_language_different_roots_get_separate_clients() {
        let key1 = LspKey::new("rust", Path::new("/project-a"));
        let key2 = LspKey::new("rust", Path::new("/project-b"));
        assert_ne!(key1, key2);

        // HashMap correctly distinguishes them
        let mut map: HashMap<LspKey, &str> = HashMap::new();
        map.insert(key1.clone(), "client-a");
        map.insert(key2.clone(), "client-b");
        assert_eq!(map.get(&key1), Some(&"client-a"));
        assert_eq!(map.get(&key2), Some(&"client-b"));
    }

    #[test]
    fn lsp_key_same_language_same_root_is_equal() {
        let k1 = LspKey::new("typescript", Path::new("/workspace/mcp-server"));
        let k2 = LspKey::new("typescript", Path::new("/workspace/mcp-server"));
        assert_eq!(k1, k2);
    }

    #[test]
    fn lsp_key_display() {
        let key = LspKey::new("rust", Path::new("/my/project"));
        assert_eq!(format!("{}", key), "rust@/my/project");
    }

    // --- Per-language TTL tests ---

    #[test]
    fn kotlin_gets_2h_ttl_regardless_of_global() {
        let global = Duration::from_secs(30 * 60);
        assert_eq!(
            ttl_for_language("kotlin", global),
            Duration::from_secs(2 * 3600)
        );
    }

    #[test]
    fn non_kotlin_languages_use_global_ttl() {
        let global = Duration::from_secs(30 * 60);
        for lang in &["rust", "typescript", "java", "python", "go"] {
            assert_eq!(
                ttl_for_language(lang, global),
                global,
                "expected global TTL for language: {lang}"
            );
        }
    }

    // --- Idle TTL eviction tests ---

    /// evict_idle must remove last_used entries whose age exceeds the TTL,
    /// even when no corresponding client exists in the pool (e.g. already
    /// LRU-evicted but last_used not yet cleaned up).
    ///
    /// Three-query sandwich:
    ///   1. Insert stale entry → baseline count = 1
    ///   2. evict_idle with 1 ms TTL → should remove it
    ///   3. Count = 0 → entry cleaned up
    #[tokio::test]
    async fn evict_idle_clears_stale_last_used_entries() {
        let mgr = LspManager::new();
        let key = LspKey::new("rust", Path::new("/stale-project"));

        // Step 1 — baseline: insert a stale entry (1 hour in the past)
        // 100ms — comfortably older than the 1ms TTL used in step 2, while small
        // enough to not underflow on Windows where Instant::now() is near process start.
        let stale = Instant::now()
            .checked_sub(std::time::Duration::from_millis(100))
            .expect("process has been running > 100ms");
        mgr.last_used.lock().await.insert(key.clone(), stale);
        assert_eq!(mgr.last_used.lock().await.len(), 1);

        // Step 2 — evict with a 1 ms TTL; the 1-hour-old entry qualifies
        mgr.evict_idle(std::time::Duration::from_millis(1)).await;

        // Step 3 — stale entry removed
        assert_eq!(mgr.last_used.lock().await.len(), 0);
    }

    #[test]
    fn restart_cost_tier_orders_cheap_languages_first() {
        // I-2: pure-logic check on the eviction-selection tier mapping.
        // (kotlin, java) → 2 (expensive); everything else → 1 (cheap).
        // Pool [(rust, t=0), (kotlin, t=10)] sorted by (tier, t): rust comes first
        // even though it's older — kotlin is protected from eviction.
        assert_eq!(restart_cost_tier("rust"), 1);
        assert_eq!(restart_cost_tier("typescript"), 1);
        assert_eq!(restart_cost_tier("python"), 1);
        assert_eq!(restart_cost_tier("javascript"), 1);
        assert_eq!(restart_cost_tier("bash"), 1);
        assert_eq!(restart_cost_tier("html"), 1);
        assert_eq!(restart_cost_tier("kotlin"), 2);
        assert_eq!(restart_cost_tier("java"), 2);
    }

    #[tokio::test]
    async fn lru_eviction_prefers_cheap_languages_over_kotlin() {
        // I-2: pool at max_clients with [kotlin (oldest), rust (newer)]. Pure
        // LRU would evict kotlin; cost-aware LRU evicts rust instead. We
        // simulate the pool by inserting last_used entries directly and
        // observing which key the selector picks.
        let mgr = LspManager::new();
        let kotlin_key = LspKey::new("kotlin", Path::new("/proj-a"));
        let rust_key = LspKey::new("rust", Path::new("/proj-b"));

        // Kotlin is OLDER than rust (would lose under pure-LRU).
        let kotlin_time = Instant::now()
            .checked_sub(std::time::Duration::from_secs(60))
            .expect("process has been running > 60s");
        let rust_time = Instant::now();
        {
            let mut lu = mgr.last_used.lock().await;
            lu.insert(kotlin_key.clone(), kotlin_time);
            lu.insert(rust_key.clone(), rust_time);
        }

        // Mirror the selector logic from get_or_start verbatim.
        let oldest_key = {
            let last_used = mgr.last_used.lock().await;
            last_used
                .iter()
                .min_by_key(|(k, t)| (restart_cost_tier(&k.language), *t))
                .map(|(k, _)| k.clone())
        };

        assert_eq!(
            oldest_key,
            Some(rust_key),
            "cost-aware LRU must pick rust over kotlin even though kotlin is older"
        );
    }

    #[tokio::test]
    async fn lru_eviction_evicts_kotlin_only_when_pool_is_all_expensive() {
        // I-2: pool of [kotlin (oldest), java (newer)] — both expensive — should
        // fall back to pure LRU within the expensive tier and pick kotlin.
        let mgr = LspManager::new();
        let kotlin_key = LspKey::new("kotlin", Path::new("/proj-a"));
        let java_key = LspKey::new("java", Path::new("/proj-b"));

        let kotlin_time = Instant::now()
            .checked_sub(std::time::Duration::from_secs(60))
            .expect("process has been running > 60s");
        let java_time = Instant::now();
        {
            let mut lu = mgr.last_used.lock().await;
            lu.insert(kotlin_key.clone(), kotlin_time);
            lu.insert(java_key.clone(), java_time);
        }

        let oldest_key = {
            let last_used = mgr.last_used.lock().await;
            last_used
                .iter()
                .min_by_key(|(k, t)| (restart_cost_tier(&k.language), *t))
                .map(|(k, _)| k.clone())
        };

        assert_eq!(
            oldest_key,
            Some(kotlin_key),
            "with no cheap victims available, fall back to pure LRU and evict kotlin"
        );
    }

    /// evict_idle must leave entries whose age is below the TTL untouched.
    #[tokio::test]
    async fn evict_idle_preserves_recent_entries() {
        let mgr = LspManager::new();
        let key = LspKey::new("typescript", Path::new("/fresh-project"));

        // Insert a just-accessed entry
        mgr.last_used
            .lock()
            .await
            .insert(key.clone(), Instant::now());

        // Evict with a 1-hour TTL — the fresh entry should survive
        mgr.evict_idle(std::time::Duration::from_secs(3600)).await;

        assert_eq!(mgr.last_used.lock().await.len(), 1);
    }

    /// Option C: startup failures within COLD_START_GRACE after a successful
    /// do_start must not increment the circuit-breaker counter.
    ///
    /// We simulate this by directly seeding cold_start_until with a future
    /// deadline, then manually running the failure-recording logic and
    /// asserting startup_failures stays empty.
    #[tokio::test]
    async fn cold_start_grace_suppresses_circuit_breaker_increment() {
        let mgr = LspManager::new();
        let key = LspKey::new("kotlin", std::path::Path::new("/proj"));

        // Seed grace period: still valid for the next 5 minutes.
        mgr.cold_start_until
            .lock()
            .unwrap()
            .insert(key.clone(), Instant::now() + Duration::from_secs(300));

        // Simulate what do_start's error path does.
        let in_grace = mgr
            .cold_start_until
            .lock()
            .unwrap()
            .get(&key)
            .is_some_and(|until| Instant::now() < *until);

        if !in_grace {
            let mut failures = mgr.startup_failures.lock().unwrap();
            let entry = failures.entry(key.clone()).or_insert((0, Instant::now()));
            entry.0 += 1;
        }

        // Grace was active → counter must remain absent (never incremented).
        assert_eq!(
            mgr.startup_failures.lock().unwrap().get(&key).map(|e| e.0),
            None,
            "circuit-breaker must not be incremented during cold-start grace"
        );
    }
    /// A mux-side mem-kill must reach the breaker, and must not linger.
    ///
    /// The kill happens in another process and the respawn after it SUCCEEDS, so
    /// before the marker there was nothing for the breaker to count and
    /// kill -> respawn -> grow -> kill ran unthrottled. Three arms, because each
    /// failure mode is silent on its own:
    ///
    /// * fresh marker counts AND is consumed — a marker that counted but stayed
    ///   would re-count on every subsequent call and trip the breaker instantly;
    /// * stale marker is consumed WITHOUT counting — an hour-old kill must not
    ///   inflate an unrelated burst later;
    /// * absent marker is a no-op, or every ordinary start would look like a kill.
    #[tokio::test]
    async fn mem_kill_marker_counts_toward_the_breaker_then_is_consumed() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        let mgr = LspManager::new();
        let key = LspKey::new("kotlin", ws);
        let marker = crate::lsp::mux::memkill_path_for_lock(
            &crate::lsp::mux::lock_path_for_workspace("kotlin", ws),
        );
        let count = || mgr.startup_failures.lock().unwrap().get(&key).map(|e| e.0);

        // Absent marker: nothing happens. Establishes the baseline, so the next
        // arm's increment is attributable to the marker and not to the call itself.
        let _ = std::fs::remove_file(&marker);
        mgr.count_mem_kill_marker(&key, "kotlin", ws);
        assert_eq!(count(), None, "no marker must not touch the breaker");

        // Fresh marker: counts once, and the file is gone afterwards.
        std::fs::write(&marker, "rss_ceiling").unwrap();
        mgr.count_mem_kill_marker(&key, "kotlin", ws);
        assert_eq!(count(), Some(1), "a fresh mem-kill must count");
        assert!(
            !marker.exists(),
            "the marker must be consumed, not re-counted"
        );

        // ...and a second call with no new marker does not keep counting.
        mgr.count_mem_kill_marker(&key, "kotlin", ws);
        assert_eq!(count(), Some(1), "consumed marker must not count twice");

        // Stale marker: consumed, not counted. Backdated well past the window.
        std::fs::write(&marker, "rss_ceiling").unwrap();
        let stale = filetime::FileTime::from_system_time(
            std::time::SystemTime::now()
                - LspManager::CIRCUIT_BREAKER_WINDOW
                - std::time::Duration::from_secs(60),
        );
        filetime::set_file_mtime(&marker, stale).unwrap();
        mgr.count_mem_kill_marker(&key, "kotlin", ws);
        assert_eq!(count(), Some(1), "a stale mem-kill must not count");
        assert!(!marker.exists(), "a stale marker must still be cleared");
    }

    /// Option C: once the grace period expires, failures ARE counted.
    #[tokio::test]
    async fn cold_start_grace_expired_counts_failure() {
        let mgr = LspManager::new();
        let key = LspKey::new("kotlin", std::path::Path::new("/proj2"));

        // Seed an already-expired grace period.
        mgr.cold_start_until
            .lock()
            .unwrap()
            .insert(key.clone(), Instant::now() - Duration::from_secs(1));

        let in_grace = mgr
            .cold_start_until
            .lock()
            .unwrap()
            .get(&key)
            .is_some_and(|until| Instant::now() < *until);

        if !in_grace {
            let mut failures = mgr.startup_failures.lock().unwrap();
            let entry = failures.entry(key.clone()).or_insert((0, Instant::now()));
            entry.0 += 1;
        }

        assert_eq!(
            mgr.startup_failures.lock().unwrap().get(&key).map(|e| e.0),
            Some(1),
            "circuit-breaker must be incremented after grace period expires"
        );
    }

    /// The background task spawned by new_arc_with_ttl must automatically
    /// evict a client that has not been accessed since longer than the TTL.
    ///
    /// Uses rust-analyzer as the real LSP; skipped if not installed.
    #[tokio::test]
    async fn idle_background_task_evicts_after_ttl() {
        use std::process::Command as StdCommand;
        if StdCommand::new("rust-analyzer")
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("Skipping: rust-analyzer not installed");
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "pub fn f() {}").unwrap();

        let ttl = std::time::Duration::from_millis(300);
        let mgr = LspManager::new_arc_with_ttl(ttl);

        // Start a real LSP client; pass Some(false) so the test uses rust-analyzer
        // directly, without needing the codescout-mux binary on PATH.
        mgr.get_or_start("rust", dir.path(), Some(false))
            .await
            .unwrap();
        assert!(
            !mgr.active_languages().await.is_empty(),
            "client should be alive"
        );

        // The background loop wakes every ttl/4, so eviction lands shortly after one
        // TTL elapses. Poll for it instead of sleeping a fixed 4×TTL: a fixed sleep
        // asserts "enough time has passed" and reds when the parallel suite has not
        // scheduled the eviction task yet
        // (docs/issues/archive/2026-09-01-peer-idle-timeout-test-is-the-third-load-sensitive-step.md).
        // `active_languages` reads `clients` and never touches `last_used`, so polling
        // cannot refresh the idle timer it waits on — checked at the source, not
        // assumed. The loop exits early, so the 10s ceiling costs nothing.
        for _ in 0..500 {
            if mgr.active_languages().await.is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }

        // Client must have been evicted
        assert!(
            mgr.active_languages().await.is_empty(),
            "idle client should have been evicted after TTL"
        );
    }

    #[tokio::test]
    async fn do_start_records_lsp_event_to_db() {
        // Use a real temp dir so open_db works
        let dir = tempfile::TempDir::new().unwrap();
        let Some(config) = ra_config_or_skip(dir.path()) else {
            eprintln!("Skipping: rust-analyzer not installed");
            return;
        };
        let mgr = LspManager::new_for_test_with_root(dir.path()).await;

        mgr.get_or_start_for_test("rust", config).await.unwrap();

        // Verify an lsp_events row was written
        let conn = crate::usage::db::open_db(dir.path()).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM lsp_events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let (lang, reason): (String, String) = conn
            .query_row("SELECT language, reason FROM lsp_events LIMIT 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(lang, "rust");
        assert_eq!(reason, "new_session");
    }

    #[tokio::test]
    async fn do_start_records_failure_event_when_start_fails() {
        // A bogus command makes LspClient::start fail deterministically, so this
        // test needs no language server installed and runs everywhere — unlike the
        // success tests above, which skip when rust-analyzer is absent.
        let dir = tempfile::TempDir::new().unwrap();
        let mgr = LspManager::new_for_test_with_root(dir.path()).await;
        let config = LspServerConfig {
            command: "codescout-nonexistent-lsp-binary-xyz".into(),
            args: vec![],
            workspace_root: dir.path().to_path_buf(),
            init_timeout: Some(std::time::Duration::from_secs(5)),
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        };

        let result = mgr.get_or_start_for_test("kotlin", config).await;
        assert!(result.is_err(), "start with a bogus binary must fail");

        // The failed start must leave an `outcome='failed'` lsp_events row, not a gap.
        let conn = crate::usage::db::open_db(dir.path()).unwrap();
        let (outcome, error): (String, Option<String>) = conn
            .query_row("SELECT outcome, error FROM lsp_events LIMIT 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(outcome, "failed");
        assert!(error.is_some());
    }

    #[tokio::test]
    async fn do_start_reason_evicted_consumes_pending_reason() {
        let dir = tempfile::TempDir::new().unwrap();
        let Some(config) = ra_config_or_skip(dir.path()) else {
            eprintln!("Skipping: rust-analyzer not installed");
            return;
        };
        let mgr = LspManager::new_for_test_with_root(dir.path()).await;
        let key = LspKey::new("rust", dir.path());

        // Pre-populate pending_reason as if eviction happened
        mgr.pending_reason
            .lock()
            .unwrap()
            .insert(key, "idle_evicted".to_string());

        mgr.get_or_start_for_test("rust", config).await.unwrap();

        // pending_reason should be consumed
        assert!(mgr.pending_reason.lock().unwrap().is_empty());

        // DB row should have reason = idle_evicted
        let conn = crate::usage::db::open_db(dir.path()).unwrap();
        let reason: String = conn
            .query_row("SELECT reason FROM lsp_events LIMIT 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(reason, "idle_evicted");
    }

    #[tokio::test]
    async fn record_first_response_consumes_pending_and_updates_db() {
        let dir = tempfile::TempDir::new().unwrap();
        let Some(config) = ra_config_or_skip(dir.path()) else {
            eprintln!("Skipping: rust-analyzer not installed");
            return;
        };
        let mgr = LspManager::new_for_test_with_root(dir.path()).await;

        // Start the LSP to create the pending entry
        mgr.get_or_start_for_test("rust", config).await.unwrap();

        // First call should consume the pending entry and write to DB
        mgr.record_first_response_inner("rust", dir.path(), 9100)
            .await;

        // pending_first_response should now be empty
        assert!(mgr.pending_first_response.lock().unwrap().is_empty());

        // DB row should be updated
        let conn = crate::usage::db::open_db(dir.path()).unwrap();
        let val: Option<i64> = conn
            .query_row(
                "SELECT first_response_ms FROM lsp_events LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(val, Some(9100));
    }

    #[tokio::test]
    async fn record_first_response_noop_when_no_pending() {
        let dir = tempfile::TempDir::new().unwrap();
        let mgr = LspManager::new_for_test_with_root(dir.path()).await;
        // No prior get_or_start — calling record_first_response_inner should not panic or error
        mgr.record_first_response_inner("rust", dir.path(), 5000)
            .await;
    }

    #[tokio::test]
    async fn record_first_response_second_call_is_noop() {
        let dir = tempfile::TempDir::new().unwrap();
        let Some(config) = ra_config_or_skip(dir.path()) else {
            eprintln!("Skipping: rust-analyzer not installed");
            return;
        };
        let mgr = LspManager::new_for_test_with_root(dir.path()).await;

        mgr.get_or_start_for_test("rust", config).await.unwrap();

        mgr.record_first_response_inner("rust", dir.path(), 9100)
            .await;
        // Second call — pending is already consumed, should be a silent no-op
        mgr.record_first_response_inner("rust", dir.path(), 1234)
            .await;

        let conn = crate::usage::db::open_db(dir.path()).unwrap();
        let val: Option<i64> = conn
            .query_row(
                "SELECT first_response_ms FROM lsp_events LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        // Should still be 9100 — second call didn't overwrite
        assert_eq!(val, Some(9100));
    }

    #[cfg(unix)]
    #[test]
    fn mux_failure_is_index_contention_detects_lock_signatures() {
        use super::mux_failure_is_index_contention as is_contention;
        // RocksDB index LOCK held (EAGAIN) — the live backend-kotlin failure.
        assert!(is_contention(
            "org.rocksdb.RocksDBException: While lock file: …/rocks/v492/LOCK: \
             Resource temporarily unavailable"
        ));
        assert!(is_contention(
            "Resource temporarily unavailable (os error 11)"
        ));
        // The mux's own flock, surfaced by `process::run`.
        assert!(is_contention("Error: another mux instance holds the lock"));
        // A genuine spawn failure is NOT contention — direct fallback is still correct.
        assert!(!is_contention("failed to spawn LSP server: kotlin-lsp"));
        assert!(!is_contention(""));
    }

    #[cfg(unix)]
    #[test]
    fn mux_failure_report_surfaces_stderr_cause_with_index_hint() {
        use super::mux_failure_report;
        // Empty stdout "ready" line + the real cause on stderr (the bug: this used
        // to render a blank "mux process failed to start:").
        let tail = vec![
            "Error: another mux instance holds the lock".to_string(),
            "Caused by:".to_string(),
            "    Resource temporarily unavailable (os error 11)".to_string(),
        ];
        let (message, hint) = mux_failure_report("", &tail);
        assert!(message.starts_with("mux process failed to start:"));
        assert!(
            message.contains("Resource temporarily unavailable"),
            "real cause must be surfaced, got: {message}"
        );
        assert!(
            hint.contains("index is locked"),
            "index-contention hint expected, got: {hint}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn mux_failure_report_handles_silent_exit_with_generic_hint() {
        use super::mux_failure_report;
        let (message, hint) = mux_failure_report("", &[]);
        assert!(message.contains("no diagnostic output"), "got: {message}");
        assert!(
            hint.contains("another codescout mux isn't already running"),
            "generic hint expected when no contention signature, got: {hint}"
        );
    }
    #[cfg(unix)]
    #[test]
    fn mux_contention_report_round_trips_through_get_or_start_guard() {
        // Fix 4 wiring (see `get_or_start`): the no-fallback guard re-classifies the
        // ERROR STRING that `get_or_start_via_mux` returns, via
        // `mux_failure_is_index_contention(&e.to_string())`. That error is a
        // `RecoverableError` built from `mux_failure_report`. This proves the
        // round-trip survives `RecoverableError`'s Display: a held-RocksDB-lock stderr
        // → report → RecoverableError → to_string() must STILL trip the contention
        // guard, so `get_or_start` returns Err instead of poison-falling-back to a
        // direct-LSP squatter on the locked index. A Display-format or hint-wording
        // drift that broke this would silently restore the squatter bug.
        use super::{mux_failure_is_index_contention, mux_failure_report};
        let tail = vec!["org.rocksdb.RocksDBException: While lock file: \
             …/analyzer/workspaces/<h>/rocks/v492/LOCK: \
             Resource temporarily unavailable"
            .to_string()];
        let (message, hint) = mux_failure_report("", &tail);
        let err = crate::tools::RecoverableError::with_hint(message, hint);
        assert!(
            mux_failure_is_index_contention(&err.to_string()),
            "the error get_or_start_via_mux returns for a held RocksDB lock must itself \
             trip get_or_start's contention guard (→ return Err, no direct fallback); got: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn non_contention_mux_report_does_not_trip_get_or_start_guard() {
        // Complement: a generic mux startup failure (no RocksDB/EAGAIN/flock signature)
        // must NOT be classified as contention, so `get_or_start` correctly FALLS BACK
        // to direct mode rather than failing the caller (the test-env path where
        // current_exe() is the test runner, not the codescout binary).
        use super::{mux_failure_is_index_contention, mux_failure_report};
        let (message, hint) =
            mux_failure_report("", &["failed to spawn LSP server: kotlin-lsp".to_string()]);
        let err = crate::tools::RecoverableError::with_hint(message, hint);
        assert!(
            !mux_failure_is_index_contention(&err.to_string()),
            "a generic mux failure must NOT trip the contention guard (get_or_start should \
             fall back to direct mode); got: {err}"
        );
    }
    #[cfg(unix)]
    #[test]
    fn posix_write_lock_is_held_false_on_unlocked_file() {
        // No holder → not-held, so a genuine mux-infra-unavailable failure still
        // falls back to direct (a false positive here would break that fallback).
        let dir = tempfile::tempdir().unwrap();
        let lock = dir.path().join("LOCK");
        std::fs::write(&lock, b"").unwrap();
        assert!(!super::posix_write_lock_is_held(&lock));
        // Missing file → not-held (don't block the fallback).
        assert!(!super::posix_write_lock_is_held(&dir.path().join("nope")));
    }

    /// Defect #1 liveness arbiter: a free flock is claimable (→ spawn), a held one
    /// reads as `None` (→ assume a live mux, connect). flock(2) conflicts across
    /// separate open file descriptions even within one process, so a held lock is
    /// observable without a subprocess — unlike the fcntl locks the
    /// `posix_write_lock_is_held` tests probe (which need an external holder).
    #[cfg(unix)]
    #[test]
    fn claim_mux_lock_some_when_free_none_when_held() {
        use fs4::fs_std::FileExt;
        let dir = tempfile::tempdir().unwrap();
        let lock = dir.path().join("mux.lock");

        // Free → claimable.
        let guard = super::claim_mux_lock(&lock).unwrap();
        assert!(guard.is_some(), "a free flock should be claimable");
        drop(guard);

        // Hold the flock on an independent fd; claim_mux_lock opens its own fd and
        // must see it as held. Acquiring right after drop(guard) races the same
        // flock-release-visibility latency as the reclaim step below — retry
        // rather than asserting instant acquisition (recurrence 2026-07-05 in
        // docs/issues/archive/2026-07-03-parallel-test-suite-peer-and-mux-lock-flakiness.md).
        let holder = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&lock)
            .unwrap();
        let holder_locked = (0..50).any(|_| {
            if holder.try_lock_exclusive().is_ok() {
                true
            } else {
                std::thread::sleep(std::time::Duration::from_millis(20));
                false
            }
        });
        assert!(
            holder_locked,
            "holder could not acquire the released flock within budget"
        );
        assert!(
            super::claim_mux_lock(&lock).unwrap().is_none(),
            "a held flock must read as None — no live-mux assumption"
        );

        // Released → claimable again. Under heavy parallel test load the OS can take
        // a moment to make the released flock visible to a fresh fd, so retry briefly
        // rather than asserting instant reclaim (fixes the parallel-suite flake,
        // docs/issues/archive/2026-07-03-parallel-test-suite-peer-and-mux-lock-flakiness.md).
        drop(holder);
        let reclaimed = (0..50).find_map(|_| match super::claim_mux_lock(&lock).unwrap() {
            Some(guard) => Some(guard),
            None => {
                std::thread::sleep(std::time::Duration::from_millis(20));
                None
            }
        });
        assert!(
            reclaimed.is_some(),
            "lock should be reclaimable after the holder releases"
        );
    }

    /// Defect #1 actionable error: when a live process holds the flock but the
    /// socket is unreachable, the error names the wedged-mux situation and routes
    /// the human to the holder — not a bare `ECONNREFUSED`.
    #[cfg(unix)]
    #[test]
    fn mux_socket_unreachable_error_names_situation_and_routes_to_holder() {
        let rendered = super::mux_socket_unreachable_error(
            "kotlin",
            std::path::Path::new("/run/user/1000/codescout-kotlin-mux-abc.sock"),
            std::path::Path::new("/run/user/1000/codescout-kotlin-mux-abc.lock"),
        )
        .to_string();
        assert!(
            rendered.contains("wedged or mid-restart"),
            "should name the wedged-mux situation: {rendered}"
        );
        assert!(
            rendered.contains(".lock") && rendered.contains(".sock"),
            "should cite both the lock and the socket: {rendered}"
        );
        assert!(
            rendered.contains("fuser"),
            "should route the human to the holder: {rendered}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn kotlin_index_lock_held_false_for_non_kotlin_and_missing_home() {
        // Non-kotlin: no IntelliJ index → never contention.
        assert!(!super::kotlin_index_lock_held(
            "rust",
            std::path::Path::new("/tmp/cs-nonexistent-ws")
        ));
        // Kotlin but a fresh workspace whose analyzer home doesn't exist → false.
        let dir = tempfile::tempdir().unwrap();
        assert!(!super::kotlin_index_lock_held("kotlin", dir.path()));
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "spawns a python3 fcntl holder; gated like the rust-analyzer mux test"]
    fn posix_write_lock_is_held_true_when_another_process_holds_it() {
        // fcntl(F_SETLK) locks are per-process, so a same-process holder wouldn't
        // conflict with the probe. Spawn a SEPARATE python3 process that takes the
        // POSIX write lock (the mechanism RocksDB uses), then probe → must be held.
        // Guards against a silently-broken probe that always reports not-held.
        let dir = tempfile::tempdir().unwrap();
        let lock = dir.path().join("LOCK");
        std::fs::write(&lock, b"").unwrap();
        let mut holder = std::process::Command::new("python3")
            .arg("-c")
            .arg(format!(
                "import fcntl,time; f=open(r'{}', 'r+'); \
                 fcntl.lockf(f, fcntl.LOCK_EX | fcntl.LOCK_NB); \
                 print('held', flush=True); time.sleep(10)",
                lock.display()
            ))
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("spawn python3 holder");
        {
            use std::io::Read;
            let mut buf = [0u8; 4];
            let _ = holder.stdout.as_mut().unwrap().read(&mut buf); // barrier: wait for "held"
        }
        let held = super::posix_write_lock_is_held(&lock);
        let _ = holder.kill();
        let _ = holder.wait();
        assert!(
            held,
            "probe must detect a POSIX write-lock held by another process"
        );
    }

    /// Defect #1 end-to-end: a live process holds the mux ownership flock but no
    /// socket is reachable → `get_or_start_via_mux` must surface the actionable
    /// wedged-mux error (not a bare connect error) AND must not respawn (the held
    /// flock blocks it). Mirrors the live e2e verified by hand on 2026-06-14
    /// (see issues/2026-06-11-mux-failure-masks-rocksdb-lock-collision § Verified
    /// live). Holds a BSD `flock` from a separate python3 — the SAME lock family
    /// `fs4::try_lock_exclusive` uses (distinct from the fcntl locks the
    /// `posix_write_lock` tests hold); blocking `LOCK_EX` guarantees acquisition
    /// before "held" is printed, so the barrier read can never race the lock.
    #[cfg(unix)]
    #[tokio::test]
    #[ignore = "spawns a python3 flock holder + drives the mux connect path; gated like the posix_write_lock tests"]
    async fn get_or_start_via_mux_surfaces_wedged_error_when_flock_held_socket_absent() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        let lock_path = crate::lsp::mux::lock_path_for_workspace("rust", ws);
        let sock_path = crate::lsp::mux::socket_path_for_workspace("rust", ws);
        // The wedged state: lock file present + held, NO reachable socket.
        std::fs::write(&lock_path, b"").unwrap();
        let _ = std::fs::remove_file(&sock_path);

        // Hold the BSD flock from a separate process (blocking acquire → prints
        // "held" only once it owns the lock; then sleeps to keep holding it).
        let mut holder = std::process::Command::new("python3")
            .arg("-c")
            .arg(format!(
                "import fcntl,time; f=open(r'{}', 'r+'); \
                 fcntl.flock(f, fcntl.LOCK_EX); \
                 print('held', flush=True); time.sleep(10)",
                lock_path.display()
            ))
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("spawn python3 flock holder");
        {
            use std::io::Read;
            let mut buf = [0u8; 4];
            let _ = holder.stdout.as_mut().unwrap().read(&mut buf); // barrier: wait for "held"
        }

        let mgr = LspManager::new();
        let config = LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: ws.to_path_buf(),
            init_timeout: None,
            mux: true,
            env: vec![],
            idle_timeout_secs: None,
        };
        let result = mgr
            .get_or_start_via_mux_waiting("rust", ws, config, Duration::from_millis(300))
            .await;

        let _ = holder.kill();
        let _ = holder.wait();

        let msg = match result {
            Ok(_) => panic!("held flock + absent socket must error, not connect or spawn"),
            Err(e) => e.to_string(),
        };
        assert!(
            msg.contains("wedged or mid-restart"),
            "must surface the wedged-mux situation, got: {msg}"
        );
        assert!(
            msg.contains("fuser"),
            "must route the human to the lock holder, got: {msg}"
        );
    }
    /// A mux another process owns takes its flock first and binds its socket only after the
    /// language server answered `initialize` — seconds, for kotlin-lsp. A client that connects in
    /// that window must wait for the socket, not call the mux wedged. This stands in for the
    /// mux: it holds the lock, then binds the socket late and sends the init message
    /// `LspClient::connect` reads.
    /// docs/issues/2026-10-17-symbols-name-lookup-on-kotlin-lsp-says-0-matches-for-a-symbol-that-exists.md
    #[cfg(unix)]
    #[tokio::test]
    async fn connect_to_mux_waits_for_a_peer_mux_that_binds_its_socket_late() {
        use tokio::io::AsyncWriteExt;
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join("mux.lock");
        let sock_path = dir.path().join("mux.sock");
        let _owner = super::claim_mux_lock(&lock_path)
            .unwrap()
            .expect("a free lock is claimed");

        let bind_at = sock_path.clone();
        let peer = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(1200)).await;
            let listener = tokio::net::UnixListener::bind(&bind_at).unwrap();
            let (mut stream, _) = listener.accept().await.unwrap();
            let init = r#"{"jsonrpc":"2.0","id":0,"result":{"capabilities":{}}}"#;
            stream
                .write_all(format!("Content-Length: {}\r\n\r\n{init}", init.len()).as_bytes())
                .await
                .unwrap();
            // Hold the connection open until the test is done with the client.
            tokio::time::sleep(Duration::from_secs(5)).await;
        });

        let connected = super::connect_to_mux(
            &sock_path,
            dir.path(),
            Duration::from_secs(10),
            Some(&lock_path),
            MUX_INIT_READ_MIN,
        )
        .await;
        peer.abort();
        assert!(
            connected.is_ok(),
            "a mux that binds its socket 1.2 s after taking its lock is starting, not wedged: {:?}",
            connected.err().map(|e| format!("{e:#}"))
        );
    }

    /// A mux that has bound its socket but never sends its init message used to hang
    /// `LspClient::connect` (it read the init message with no timeout), so the window and
    /// deadline of `connect_to_mux` were never reached.
    /// docs/issues/2026-10-17-connect-to-a-mux-that-never-sends-its-init-message-hangs-forever.md
    #[cfg(unix)]
    #[tokio::test]
    async fn connect_to_mux_gives_up_on_a_socket_that_never_sends_its_init_message() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("mux.sock");
        let listener = tokio::net::UnixListener::bind(&sock_path).unwrap();
        tokio::spawn(async move {
            // Accept and hold, saying nothing.
            let (_stream, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(30)).await;
        });

        let started = Instant::now();
        let returned = tokio::time::timeout(
            Duration::from_secs(5),
            super::connect_to_mux(
                &sock_path,
                dir.path(),
                Duration::from_secs(1),
                None,
                MUX_INIT_READ_MIN,
            ),
        )
        .await
        .is_ok();
        assert!(
            returned,
            "a 1 s window must end by itself; still waiting after {:?}",
            started.elapsed()
        );
    }
    /// Stand-in for a mux wedged after `bind`: accepts every connection, keeps it open and says
    /// nothing. Each accepted connection is reported on the returned channel when the client
    /// closes its end (end of file), so a test can see that no connection was left open.
    #[cfg(unix)]
    fn silent_mux(
        sock_path: &Path,
    ) -> (
        Arc<std::sync::atomic::AtomicUsize>,
        tokio::sync::mpsc::UnboundedReceiver<()>,
    ) {
        use tokio::io::AsyncReadExt;
        let listener = tokio::net::UnixListener::bind(sock_path).unwrap();
        let accepted = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (closed_tx, closed_rx) = tokio::sync::mpsc::unbounded_channel();
        let count = accepted.clone();
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let closed_tx = closed_tx.clone();
                tokio::spawn(async move {
                    let mut sink = Vec::new();
                    let _ = stream.read_to_end(&mut sink).await;
                    let _ = closed_tx.send(());
                });
            }
        });
        (accepted, closed_rx)
    }

    /// The bound is the caller's window: a socket that accepts and stays silent ends the connect
    /// when the window ends, not at the default init bound (5 s), and not an attempt-floor later.
    /// Real clock (a paused clock jumps over real socket I/O, see the issue file), so the proof
    /// is the number of attempts: one attempt that waits the whole window and ends it, where a
    /// bound shorter than the window retries and a longer one overruns. The elapsed-time bound
    /// is generous (3 s of slack). The failure is the connect's, not a lock verdict.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_silent_socket_ends_the_connect_when_the_window_ends() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("mux.sock");
        let (accepted, _closed) = silent_mux(&sock_path);

        let window = Duration::from_secs(1);
        let started = Instant::now();
        let failure = tokio::time::timeout(
            Duration::from_secs(20),
            super::connect_to_mux(&sock_path, dir.path(), window, None, MUX_INIT_READ_MIN),
        )
        .await
        .expect("the window must end the connect by itself")
        .err()
        .expect("a socket that never sends its init message is not a mux");
        let elapsed = started.elapsed();

        assert!(
            elapsed >= window && elapsed < window + Duration::from_secs(3),
            "the connect must end with its {window:?} window, took {elapsed:?}"
        );
        assert_eq!(
            accepted.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "one attempt must wait out the whole window"
        );
        assert!(
            !failure.holder_gone,
            "no lock was given, so the failure cannot say the holder is gone"
        );
        assert!(
            failure.to_string().contains("no init message"),
            "the error must say what happened, got: {failure}"
        );
    }

    /// The bound is what is LEFT of the window when the attempt starts, not the whole window: the
    /// socket here appears 1.5 s into a 3 s window, so the attempt that finds it has about 1.5 s
    /// left and the connect ends with the window (about 3.0 s), where a bound of the whole window
    /// would end it at about 4.5 s. Real clock; asserted between 3 s and 3.9 s (0.7 s of slack
    /// either side).
    #[cfg(unix)]
    #[tokio::test]
    async fn a_silent_socket_that_appears_late_still_ends_the_connect_with_the_window() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("mux.sock");
        let late = sock_path.clone();
        let peer = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            let _mux = silent_mux(&late);
            tokio::time::sleep(Duration::from_secs(60)).await;
        });

        let window = Duration::from_secs(3);
        let started = Instant::now();
        let failure = tokio::time::timeout(
            Duration::from_secs(20),
            super::connect_to_mux(&sock_path, dir.path(), window, None, MUX_INIT_READ_MIN),
        )
        .await
        .expect("the window must end the connect by itself");
        let elapsed = started.elapsed();
        peer.abort();

        let failure = failure
            .err()
            .expect("a socket that never sends init is not a mux");
        // The last attempt could not start before the socket appeared at 1.5 s, so what was left
        // of the 3 s window was at most 1.5 s: its bound is at most that (the floor, 0.5 s, is
        // lower). The whole window (3 s) or anything added to the bound would exceed 1.7 s. This
        // holds however loaded the machine is, which a wall-clock bound does not.
        let applied = failure
            .source
            .downcast_ref::<crate::lsp::client::MuxInitTimeout>()
            .expect("the failure is the init timeout")
            .after;
        assert!(
            applied <= Duration::from_millis(1700),
            "the last attempt's bound is what is left of the window, got {applied:?}"
        );
        assert!(
            elapsed >= window && elapsed < Duration::from_millis(3900),
            "the connect must end with its {window:?} window, took {elapsed:?}"
        );
    }

    /// A window of zero is a remembered wedge's one attempt: it still gives a mux that is up the
    /// floor (`MUX_INIT_READ_MIN`) to send its init message, so a mux that answers 100 ms after
    /// accepting is connected to, not written off with a 0 s bound. Real clock; 400 ms of slack
    /// against the 500 ms floor.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_window_of_zero_still_lets_a_live_mux_send_its_init_message() {
        use tokio::io::AsyncWriteExt;
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("mux.sock");
        let listener = tokio::net::UnixListener::bind(&sock_path).unwrap();
        let peer = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_millis(100)).await;
            let init = r#"{"jsonrpc":"2.0","id":0,"result":{"capabilities":{}}}"#;
            stream
                .write_all(format!("Content-Length: {}\r\n\r\n{init}", init.len()).as_bytes())
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_secs(5)).await;
        });

        let connected = super::connect_to_mux(
            &sock_path,
            dir.path(),
            Duration::ZERO,
            None,
            MUX_INIT_READ_MIN,
        )
        .await;
        peer.abort();
        assert!(
            connected.is_ok(),
            "a mux that sends its init message 100 ms after accepting is alive: {:?}",
            connected.err().map(|e| e.to_string())
        );
    }

    /// F5 of the review of dd961fe5f. The bound given to an attempt is read from the failure
    /// itself (`MuxInitTimeout::after`), so it is checked exactly and not through the wall clock
    /// (a +400 ms mutant at the call site stayed inside every wall-clock slack on a machine at
    /// load 13): with a 1 s window it never exceeds the window, and with a window of zero it is
    /// exactly the floor.
    #[cfg(unix)]
    #[tokio::test]
    async fn an_attempts_init_bound_is_what_is_left_of_the_window_and_never_below_the_floor() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("mux.sock");
        let (_accepted, _closed) = silent_mux(&sock_path);
        let applied = |failure: MuxConnectFailure| {
            failure
                .source
                .downcast_ref::<crate::lsp::client::MuxInitTimeout>()
                .expect("the failure is the init timeout")
                .after
        };

        let window = Duration::from_secs(1);
        let failure =
            super::connect_to_mux(&sock_path, dir.path(), window, None, MUX_INIT_READ_MIN)
                .await
                .err()
                .expect("silent");
        let first = applied(failure);
        assert!(
            first <= window && first >= Duration::from_millis(500),
            "one attempt waits what is left of a 1 s window: {first:?}"
        );

        let failure = super::connect_to_mux(
            &sock_path,
            dir.path(),
            Duration::ZERO,
            None,
            MUX_INIT_READ_MIN,
        )
        .await
        .err()
        .expect("silent");
        assert_eq!(
            applied(failure),
            Duration::from_millis(500),
            "a window of zero gives the floor and not a nanosecond more"
        );
    }

    /// The constants the docs and the issue files quote, pinned by literals: the tests above
    /// measure against the constants themselves, so changing one would otherwise move the
    /// behaviour and the expectation together (review F4: 5 s to 2 s, 500 ms to 150 ms survived).
    #[test]
    fn the_mux_connect_constants_have_the_values_the_docs_quote() {
        assert_eq!(
            crate::lsp::client::MUX_INIT_READ_TIMEOUT,
            Duration::from_secs(5)
        );
        assert_eq!(MUX_INIT_READ_MIN, Duration::from_millis(500));
        assert_eq!(MUX_SPAWNED_CONNECT_WINDOW, Duration::from_millis(800));
        assert_eq!(MUX_CONNECT_RETRY_INTERVAL, Duration::from_millis(200));
        assert_eq!(MUX_CONNECT_ROUNDS, 3);
    }

    /// What each kind of round connects with, by literals. The spawned path (F3) is the one that
    /// had no init bound before 2026-10-18: it gets the 5 s default and checks no lock.
    #[cfg(unix)]
    #[test]
    fn each_kind_of_round_connects_with_the_documented_window_and_init_floor() {
        let left = Duration::from_secs(7);
        assert_eq!(
            plan_mux_connect(true, false, left),
            MuxConnectPlan {
                window: Duration::from_millis(800),
                min_init_read: Duration::from_secs(5),
                check_holder: false,
            }
        );
        assert_eq!(
            plan_mux_connect(true, true, left),
            plan_mux_connect(true, false, left),
            "a spawned mux ignores the wedge memory"
        );
        assert_eq!(
            plan_mux_connect(false, true, left),
            MuxConnectPlan {
                window: Duration::ZERO,
                min_init_read: Duration::from_millis(500),
                check_holder: true,
            }
        );
        assert_eq!(
            plan_mux_connect(false, false, left),
            MuxConnectPlan {
                window: left,
                min_init_read: Duration::from_millis(500),
                check_holder: true,
            }
        );
        let half = Duration::from_millis(500);
        assert_eq!(
            mux_init_read_bound(Duration::from_secs(3), half),
            Duration::from_secs(3)
        );
        assert_eq!(mux_init_read_bound(Duration::from_millis(200), half), half);
        assert_eq!(mux_init_read_bound(Duration::ZERO, half), half);
    }

    /// F3 of the review of dd961fe5f. The path that SPAWNED the mux and saw its `ready` line
    /// (`MUX_SPAWNED_CONNECT_WINDOW`, 800 ms, no lock to check) had NO bound on the init read
    /// before the fix and got one of about 0.8 s from it, ending in a hard error. The mux is up
    /// there, so the init read is given the full default bound (5 s): a mux that answers 2 s
    /// after accepting (a busy machine) must be connected to.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_mux_we_just_spawned_gets_the_full_init_bound_not_the_connect_window() {
        use tokio::io::AsyncWriteExt;
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("mux.sock");
        let listener = tokio::net::UnixListener::bind(&sock_path).unwrap();
        let peer = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(2)).await;
            let init = r#"{"jsonrpc":"2.0","id":0,"result":{"capabilities":{}}}"#;
            stream
                .write_all(format!("Content-Length: {}\r\n\r\n{init}", init.len()).as_bytes())
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_secs(5)).await;
        });

        // The production plan for a mux this call just spawned.
        let plan = plan_mux_connect(true, false, Duration::ZERO);
        let connected = tokio::time::timeout(
            Duration::from_secs(30),
            super::connect_to_mux(
                &sock_path,
                dir.path(),
                plan.window,
                None,
                plan.min_init_read,
            ),
        )
        .await
        .expect("bounded");
        peer.abort();
        assert!(
            connected.is_ok(),
            "a mux that answers 2 s after accepting is alive: {:?}",
            connected.err().map(|e| e.to_string())
        );
    }

    /// An attempt that timed out leaves nothing behind: its connection is closed (the peer sees
    /// end of file) and no task outlives it. Before the bound the attempt never returned at all.
    /// Real clock.
    #[cfg(unix)]
    #[tokio::test]
    async fn an_attempt_that_gets_no_init_message_closes_its_connection_and_leaves_no_task() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("mux.sock");
        let (accepted, mut closed) = silent_mux(&sock_path);
        // Let the peer's accept loop start, then count the tasks alive with only it running.
        tokio::task::yield_now().await;
        let metrics = tokio::runtime::Handle::current().metrics();
        let before = metrics.num_alive_tasks();

        let window = Duration::from_millis(600);
        let failure = tokio::time::timeout(
            Duration::from_secs(20),
            super::connect_to_mux(&sock_path, dir.path(), window, None, MUX_INIT_READ_MIN),
        )
        .await
        .expect("the window must end the connect by itself")
        .err()
        .expect("no init message, no client");
        assert!(failure.to_string().contains("no init message"), "{failure}");
        let attempts = accepted.load(std::sync::atomic::Ordering::SeqCst);
        assert!(attempts >= 1, "the peer must have been connected to");

        // One end-of-file per connection the peer accepted.
        for n in 0..attempts {
            tokio::time::timeout(Duration::from_secs(5), closed.recv())
                .await
                .unwrap_or_else(|_| panic!("connection {n} of {attempts} was left open"))
                .expect("peer alive");
        }
        for _ in 0..4 {
            tokio::task::yield_now().await;
        }
        assert_eq!(
            metrics.num_alive_tasks(),
            before,
            "a failed attempt must not leave a task running"
        );
    }

    /// The whole path: a peer mux that holds its lock, accepts and never sends its init message is
    /// a wedged mux. The caller waits its window, gets the wedged-mux error (no respawn: the lock
    /// is held), and the next call does not serve the wait again. Real clock (a paused clock jumps
    /// over real socket I/O); the outer bound is only there so the pre-fix hang fails the test
    /// instead of hanging it. Bounds: the first call takes at least the 2 s wait and under 5 s;
    /// the second takes under the wait (it costs one attempt, the 0.5 s floor).
    #[cfg(unix)]
    #[tokio::test]
    async fn a_peer_mux_that_accepts_but_never_sends_init_is_wedged_and_remembered() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        let lock_path = crate::lsp::mux::lock_path_for_workspace("rust", ws);
        let sock_path = crate::lsp::mux::socket_path_for_workspace("rust", ws);
        let _ = std::fs::remove_file(&sock_path);
        let _owner = super::claim_mux_lock(&lock_path)
            .unwrap()
            .expect("a free lock is claimed");
        let (accepted, _closed) = silent_mux(&sock_path);
        let config = || LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: ws.to_path_buf(),
            init_timeout: None,
            mux: true,
            env: vec![],
            idle_timeout_secs: None,
        };

        let mgr = LspManager::new();
        let wait = Duration::from_secs(2);
        let started = Instant::now();
        let first = tokio::time::timeout(
            Duration::from_secs(30),
            mgr.get_or_start_via_mux_waiting("rust", ws, config(), wait),
        )
        .await
        .expect("a socket that never sends its init message must end the wait by itself");
        let first_took = started.elapsed();
        let msg = match first {
            Ok(_) => panic!("a silent socket must not connect or spawn"),
            Err(e) => e.to_string(),
        };
        assert!(msg.contains("wedged or mid-restart"), "{msg}");
        assert!(
            first_took >= wait && first_took < wait + Duration::from_secs(3),
            "the wait must be served, and no more than it, took {first_took:?}"
        );
        assert_eq!(
            mgr.mux_wedged_at.lock().unwrap().len(),
            1,
            "a live holder that accepted and stayed silent for the whole wait is a wedged mux"
        );
        assert_eq!(
            accepted.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "one attempt waits the whole window, and the wedged error needs no second round of waiting"
        );

        // Within the memory the next call makes one attempt: it costs the init floor, not the wait.
        let started = Instant::now();
        let second = tokio::time::timeout(
            Duration::from_secs(30),
            mgr.get_or_start_via_mux_waiting("rust", ws, config(), wait),
        )
        .await
        .expect("the remembered wedge's one attempt must end by itself");
        let second_took = started.elapsed();
        let _ = std::fs::remove_file(&lock_path);
        assert!(second.is_err());
        assert!(
            second_took < wait,
            "inside the memory the wait is not served again, took {second_took:?}"
        );
    }

    /// The other side of the same rule: a silent socket whose lock holder died is not a wedged
    /// mux. The init timeout ends the attempt, the lock check finds the holder gone, and nothing is
    /// remembered. Real clock: the lock is released by closing a file descriptor.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_silent_socket_whose_holder_died_is_not_remembered_as_a_wedged_mux() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        let lock_path = crate::lsp::mux::lock_path_for_workspace("rust", ws);
        let sock_path = crate::lsp::mux::socket_path_for_workspace("rust", ws);
        let _ = std::fs::remove_file(&sock_path);
        let owner = super::claim_mux_lock(&lock_path)
            .unwrap()
            .expect("a free lock is claimed");
        let (_accepted, _closed) = silent_mux(&sock_path);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            drop(owner); // the holder died, the stand-in socket outlives it
        });
        let config = LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: ws.to_path_buf(),
            init_timeout: None,
            mux: true,
            env: vec![],
            idle_timeout_secs: None,
        };

        let mgr = LspManager::new();
        // Round 1 finds the lock free and tries to spawn a mux, which fails in a test binary;
        // what matters is what was recorded about the wait that ended.
        let _ = tokio::time::timeout(
            Duration::from_secs(20),
            mgr.get_or_start_via_mux_waiting("rust", ws, config, Duration::from_secs(1)),
        )
        .await
        .expect("a silent socket must end the wait by itself");
        let _ = std::fs::remove_file(&lock_path);

        assert!(
            mgr.mux_wedged_at.lock().unwrap().is_empty(),
            "a holder that died is not a wedged mux, whatever its socket did"
        );
    }

    /// A waiting client probes the mux lock by TAKING it for an instant (`claim_mux_lock`, once
    /// per 200 ms in `connect_to_mux`), and a mux's own attempt (step 1 of `mux::process::run`)
    /// used to be a single try: landing inside a probe, it exited "another mux instance holds the
    /// lock". This puts both on real threads: a thread probes in a loop (a compressed stand-in for
    /// many waiting clients) while the mux's attempt is made at the same moment, over many trials,
    /// each on a fresh lock file. The assertion is one-sided on purpose: with nothing holding the
    /// lock but probes, no attempt may be refused, so a green run is exact and only a red run
    /// depends on timing (it is red when any trial collides). The collision RATE here is
    /// exaggerated by the loop and says nothing about how often it happens in production.
    /// docs/issues/2026-10-17-mux-connect-path-lock-races.md
    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_waiting_clients_lock_probe_never_makes_a_starting_mux_give_up() {
        use std::sync::atomic::{AtomicBool, Ordering};
        const TRIALS: usize = 300;
        let dir = tempfile::tempdir().unwrap();
        let mut refused = 0usize;
        for trial in 0..TRIALS {
            let lock = dir.path().join(format!("mux-{trial}.lock"));
            let go = Arc::new(AtomicBool::new(false));
            let probe = {
                let (lock, go) = (lock.clone(), go.clone());
                std::thread::spawn(move || {
                    while !go.load(Ordering::SeqCst) {
                        std::hint::spin_loop();
                    }
                    for _ in 0..50 {
                        let _ = super::claim_mux_lock(&lock);
                    }
                })
            };
            go.store(true, Ordering::SeqCst);
            if crate::lsp::mux::process::acquire_mux_lock(&lock)
                .await
                .is_err()
            {
                refused += 1;
            }
            probe.join().unwrap();
        }
        assert_eq!(
            refused, 0,
            "{refused} of {TRIALS} mux starts were refused while only a client's probe held the lock"
        );
    }

    /// Race (1) of docs/issues/2026-10-17-mux-connect-path-lock-races.md, tried the natural way: two
    /// clients (two managers, as two processes would be) wait on one lock holder in the same
    /// workspace, the holder dies, and both go on to the next round at nearly the same moment, so
    /// each one's lock claim can land inside the other's. No mux exists in this test and none is
    /// started (a test binary cannot be one), so a client that reports "holds its lock ... wedged
    /// or mid-restart" within 2 s was told something false: the only holder was the other client's
    /// claim of an instant. Measured 2026-10-18, 300 trials of 2 clients each: with the fix's one
    /// condition (`round > 0 && !last_holder_gone`) reverted to `round > 0`, 8 of 600 clients got
    /// that error; with the fix, 0 of 600. The assertion is one-sided: green is "none were told
    /// it", and a red run depends on timing.
    /// `scripts/with-slot.sh cargo test --lib -- --ignored two_clients_waiting`
    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore = "stress, about 150 s for 300 trials: the natural-interleaving attempt at race (1)"]
    async fn two_clients_waiting_on_one_dead_holder_are_never_told_a_live_mux_wedged_it() {
        const TRIALS: usize = 300;
        let mut wrongly_told = 0usize;
        for _ in 0..TRIALS {
            let dir = tempfile::tempdir().unwrap();
            let ws = dir.path().to_path_buf();
            let lock_path = crate::lsp::mux::lock_path_for_workspace("rust", &ws);
            let sock_path = crate::lsp::mux::socket_path_for_workspace("rust", &ws);
            let _ = std::fs::remove_file(&sock_path);
            let owner = super::claim_mux_lock(&lock_path).unwrap().unwrap();
            let clients: Vec<_> = (0..2)
                .map(|_| {
                    let ws = ws.clone();
                    tokio::spawn(async move {
                        let config = LspServerConfig {
                            command: "rust-analyzer".into(),
                            args: vec![],
                            workspace_root: ws.clone(),
                            init_timeout: None,
                            mux: true,
                            env: vec![],
                            idle_timeout_secs: None,
                        };
                        let started = Instant::now();
                        let result = LspManager::new()
                            .get_or_start_via_mux_waiting(
                                "rust",
                                &ws,
                                config,
                                Duration::from_secs(3),
                            )
                            .await;
                        (started.elapsed(), result.err().map(|e| e.to_string()))
                    })
                })
                .collect();
            // Both clients are in their wait; the holder dies between two of their probes.
            tokio::time::sleep(Duration::from_millis(250)).await;
            drop(owner);
            for client in clients {
                let (took, message) = client.await.unwrap();
                if took < Duration::from_secs(2)
                    && message.is_some_and(|m| m.contains("wedged or mid-restart"))
                {
                    wrongly_told += 1;
                }
            }
            let _ = std::fs::remove_file(&lock_path);
        }
        assert_eq!(
            wrongly_told,
            0,
            "{wrongly_told} of {} clients were told a live mux wedged the lock, with none running",
            TRIALS * 2
        );
    }

    /// What the process that takes the lock in the gap does next.
    #[cfg(unix)]
    enum ThirdHolder {
        /// Its mux binds its socket and sends the init message this long after it took the lock.
        BindsAfter(Duration),
        /// Its mux never binds: a wedged holder.
        NeverBinds,
        /// It dies this long after taking the lock, binding nothing.
        DiesAfter(Duration),
    }

    /// The manager under test, the lock currently held by a third process, and how many times the
    /// gap opened (the `after_early_stop` seam ran).
    #[cfg(unix)]
    struct LockHandOffs {
        mgr: LspManager,
        held: Arc<StdMutex<Option<std::fs::File>>>,
        gaps: Arc<std::sync::atomic::AtomicUsize>,
    }

    /// Stand-in for the lock-holding side of race (1): the first holder dies while a client waits
    /// on it, and in the instant after the client's wait ended on that (the `after_early_stop`
    /// seam, which runs where the natural flow has no yield point) a THIRD process takes the lock
    /// and behaves as `third` says. The gap is used for the first `max_hand_offs` times it opens.
    #[cfg(unix)]
    fn manager_whose_lock_changes_hands_after_the_first_holder_dies(
        ws: &Path,
        first_holder_dies_after: Duration,
        third: ThirdHolder,
        max_hand_offs: usize,
    ) -> LockHandOffs {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::io::AsyncWriteExt;
        let lock_path = crate::lsp::mux::lock_path_for_workspace("rust", ws);
        let sock_path = crate::lsp::mux::socket_path_for_workspace("rust", ws);
        let _ = std::fs::remove_file(&sock_path);
        let first = super::claim_mux_lock(&lock_path)
            .unwrap()
            .expect("a free lock is claimed");
        tokio::spawn(async move {
            tokio::time::sleep(first_holder_dies_after).await;
            drop(first); // the first holder died
        });

        let held: Arc<StdMutex<Option<std::fs::File>>> = Arc::default();
        let gaps = Arc::new(AtomicUsize::new(0));
        let mgr = LspManager::new();
        let (hook_held, hook_gaps) = (held.clone(), gaps.clone());
        *mgr.after_early_stop.lock().unwrap() = Some(Arc::new(move || {
            if hook_gaps.fetch_add(1, Ordering::SeqCst) >= max_hand_offs {
                return;
            }
            // The lock was found free a moment ago, yet one run in 60 of this test group failed
            // at the single claim that stood here (`expect`, the panic message was not kept).
            // Candidate cause, not shown: a child forked by a concurrent test holds a copy of
            // the probe's descriptor until it execs, which keeps the flock held for that long.
            // Wait that out; it is not what the test is about.
            let third_holder = (0..2000)
                .find_map(|_| {
                    super::claim_mux_lock(&lock_path).unwrap().or_else(|| {
                        std::thread::sleep(Duration::from_millis(1));
                        None
                    })
                })
                .expect("the lock was found free and stayed claimable for 2 s");
            *hook_held.lock().unwrap() = Some(third_holder);
            match &third {
                ThirdHolder::NeverBinds => {}
                ThirdHolder::DiesAfter(after) => {
                    let (after, held) = (*after, hook_held.clone());
                    tokio::spawn(async move {
                        tokio::time::sleep(after).await;
                        drop(held.lock().unwrap().take()); // the third holder died
                    });
                }
                ThirdHolder::BindsAfter(after) => {
                    let (after, sock_path) = (*after, sock_path.clone());
                    tokio::spawn(async move {
                        tokio::time::sleep(after).await;
                        let listener = tokio::net::UnixListener::bind(&sock_path).unwrap();
                        let (mut stream, _) = listener.accept().await.unwrap();
                        let init = r#"{"jsonrpc":"2.0","id":0,"result":{"capabilities":{}}}"#;
                        stream
                            .write_all(
                                format!("Content-Length: {}\r\n\r\n{init}", init.len()).as_bytes(),
                            )
                            .await
                            .unwrap();
                        tokio::time::sleep(Duration::from_secs(10)).await;
                    });
                }
            }
        }));
        LockHandOffs { mgr, held, gaps }
    }

    /// Race (1) of docs/issues/2026-10-17-mux-connect-path-lock-races.md: a client waits on a
    /// peer's mux, its holder dies, and before the client's next round a third process takes the
    /// lock and starts a mux (two clients waiting on one dead holder do this to each other). The
    /// lock is held again and no socket is bound yet: that is a mux that is STARTING, and the
    /// client must wait for it, not report "wedged or mid-restart" at once. The third mux binds
    /// 600 ms after taking the lock, inside a 5 s wait, so only a client that does not wait fails.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_mux_started_by_a_third_process_in_the_gap_after_the_holder_died_is_waited_for() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        let LockHandOffs { mgr, held, .. } =
            manager_whose_lock_changes_hands_after_the_first_holder_dies(
                ws,
                Duration::from_millis(300),
                ThirdHolder::BindsAfter(Duration::from_millis(600)),
                1,
            );
        let config = LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: ws.to_path_buf(),
            init_timeout: None,
            mux: true,
            env: vec![],
            idle_timeout_secs: None,
        };

        let result = tokio::time::timeout(
            Duration::from_secs(30),
            mgr.get_or_start_via_mux_waiting("rust", ws, config, Duration::from_secs(5)),
        )
        .await
        .expect("the wait is bounded by its window");
        drop(held);
        assert!(
            result.is_ok(),
            "a mux that took the lock after the first holder died is starting, not wedged: {:?}",
            result.err().map(|e| e.to_string())
        );
    }

    /// The other half of race (1): a replacement holder that really never binds is still wedged,
    /// and it is reported within the SAME wait, not after a second one. The first holder dies
    /// about 1 s into a 2 s wait, so the replacement gets what is left (about 1 s) and the error
    /// comes at about 2 s; a second full wait would make it about 3 s. Real clock; asserted
    /// between 2 s and 2.6 s.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_replacement_holder_that_never_binds_is_wedged_within_the_same_wait() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        let LockHandOffs { mgr, held, .. } =
            manager_whose_lock_changes_hands_after_the_first_holder_dies(
                ws,
                Duration::from_millis(1000),
                ThirdHolder::NeverBinds,
                1,
            );
        let config = LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: ws.to_path_buf(),
            init_timeout: None,
            mux: true,
            env: vec![],
            idle_timeout_secs: None,
        };

        let wait = Duration::from_secs(2);
        let started = Instant::now();
        let result = tokio::time::timeout(
            Duration::from_secs(30),
            mgr.get_or_start_via_mux_waiting("rust", ws, config, wait),
        )
        .await
        .expect("the wait is bounded by its window");
        let took = started.elapsed();
        drop(held);
        let msg = match result {
            Ok(_) => panic!("a holder that never binds must not connect"),
            Err(e) => e.to_string(),
        };
        assert!(msg.contains("wedged or mid-restart"), "{msg}");
        assert!(
            took >= wait && took < Duration::from_millis(2600),
            "the replacement is waited on for what is left of the {wait:?} wait, took {took:?}"
        );
    }

    /// A replacement holder that dies too leaves a free lock, and that is a stale socket's cue to
    /// respawn: the third round claims the lock and spawns a mux (which cannot work in a test
    /// binary, so the call fails with the SPAWN's error), where a client without that round would
    /// stop at the wedged-mux error for a lock nobody holds.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_replacement_holder_that_dies_too_hands_the_lock_to_a_respawn() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        let LockHandOffs { mgr, held, gaps } =
            manager_whose_lock_changes_hands_after_the_first_holder_dies(
                ws,
                Duration::from_millis(300),
                ThirdHolder::DiesAfter(Duration::from_millis(300)),
                1,
            );
        let config = LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: ws.to_path_buf(),
            init_timeout: None,
            mux: true,
            env: vec![],
            idle_timeout_secs: None,
        };

        let result = tokio::time::timeout(
            Duration::from_secs(30),
            mgr.get_or_start_via_mux_waiting("rust", ws, config, Duration::from_secs(5)),
        )
        .await
        .expect("the wait is bounded by its window");
        drop(held);
        let msg = match result {
            Ok(_) => panic!("a test binary cannot be the mux"),
            Err(e) => e.to_string(),
        };
        assert!(
            !msg.contains("holds its lock"),
            "nobody holds the lock any more, so the wedged-mux error is wrong: {msg}"
        );
        assert_eq!(
            gaps.load(std::sync::atomic::Ordering::SeqCst),
            2,
            "two holders died while the client waited"
        );
    }

    /// F1 of the review of dd961fe5f. The wedge memory is for a mux that was waited on for a WHOLE
    /// wait and stayed unreachable. A round that waited on a REPLACEMENT holder got only what was
    /// left of the budget (here about nothing: the first holder dies at 950 ms of a 1 s wait), so
    /// its failure says nothing about that mux, which here binds 1.5 s after taking the lock.
    /// Recorded anyway, the next call, whose 5 s wait would have covered the bind, failed at once.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_replacement_holder_that_is_still_starting_is_not_remembered_as_a_wedged_mux() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        let LockHandOffs { mgr, held, .. } =
            manager_whose_lock_changes_hands_after_the_first_holder_dies(
                ws,
                Duration::from_millis(950),
                ThirdHolder::BindsAfter(Duration::from_millis(1500)),
                1,
            );
        let config = || LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: ws.to_path_buf(),
            init_timeout: None,
            mux: true,
            env: vec![],
            idle_timeout_secs: None,
        };

        let first = tokio::time::timeout(
            Duration::from_secs(30),
            mgr.get_or_start_via_mux_waiting("rust", ws, config(), Duration::from_secs(1)),
        )
        .await
        .expect("the wait is bounded by its window");
        assert!(
            first.is_err(),
            "the replacement has not bound within the 1 s wait"
        );
        assert!(
            mgr.mux_wedged_at.lock().unwrap().is_empty(),
            "a replacement that got a short remainder of the wait is not a wedged mux"
        );

        let second = tokio::time::timeout(
            Duration::from_secs(30),
            mgr.get_or_start_via_mux_waiting("rust", ws, config(), Duration::from_secs(5)),
        )
        .await
        .expect("the wait is bounded by its window");
        drop(held);
        assert!(
            second.is_ok(),
            "the next call's 5 s wait covers the bind: {:?}",
            second.err().map(|e| e.to_string())
        );
    }

    /// A lock that changes hands in every round ends the call: after the last round the client
    /// reports the unreachable-mux error instead of waiting on a lock that never settles. Each
    /// replacement dies 300 ms after taking the lock, so every round ends early; the gap opens
    /// once per round and the call ends well inside its 10 s wait (under 5 s asserted).
    #[cfg(unix)]
    #[tokio::test]
    async fn a_lock_that_changes_hands_in_every_round_ends_in_the_unreachable_error() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        let LockHandOffs { mgr, held, gaps } =
            manager_whose_lock_changes_hands_after_the_first_holder_dies(
                ws,
                Duration::from_millis(300),
                ThirdHolder::DiesAfter(Duration::from_millis(300)),
                usize::MAX,
            );
        let config = LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: ws.to_path_buf(),
            init_timeout: None,
            mux: true,
            env: vec![],
            idle_timeout_secs: None,
        };

        let started = Instant::now();
        let result = tokio::time::timeout(
            Duration::from_secs(30),
            mgr.get_or_start_via_mux_waiting("rust", ws, config, Duration::from_secs(10)),
        )
        .await
        .expect("the wait is bounded by its window");
        let took = started.elapsed();
        drop(held);
        let msg = match result {
            Ok(_) => panic!("no mux ever bound"),
            Err(e) => e.to_string(),
        };
        assert!(
            msg.contains("changed hands")
                && !msg.contains("holds its lock")
                && !msg.contains("fuser"),
            "no holder is wedged, so none may be named or `fuser`red: {msg}"
        );
        assert_eq!(
            gaps.load(std::sync::atomic::Ordering::SeqCst),
            3,
            "one gap per round, three rounds"
        );
        assert!(took < Duration::from_secs(5), "took {took:?}");
    }

    /// F2 of the review of dd961fe5f, the reviewer's scenario: two hand-offs and a third holder
    /// that dies, so the rounds run out with the lock FREE. The error must not blame a holder
    /// (no "holds its lock", no `fuser` hint): nobody holds anything.
    #[cfg(unix)]
    #[tokio::test]
    async fn rounds_that_run_out_with_the_lock_free_do_not_blame_a_holder() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        let lock_path = crate::lsp::mux::lock_path_for_workspace("rust", ws);
        let LockHandOffs { mgr, held, gaps } =
            manager_whose_lock_changes_hands_after_the_first_holder_dies(
                ws,
                Duration::from_millis(300),
                ThirdHolder::DiesAfter(Duration::from_millis(300)),
                2,
            );
        let config = LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: ws.to_path_buf(),
            init_timeout: None,
            mux: true,
            env: vec![],
            idle_timeout_secs: None,
        };

        let result = tokio::time::timeout(
            Duration::from_secs(30),
            mgr.get_or_start_via_mux_waiting("rust", ws, config, Duration::from_secs(10)),
        )
        .await
        .expect("the wait is bounded by its window");
        let msg = match result {
            Ok(_) => panic!("no mux ever bound"),
            Err(e) => e.to_string(),
        };
        let lock_free_now = super::claim_mux_lock(&lock_path).unwrap().is_some();
        drop(held);
        assert_eq!(gaps.load(std::sync::atomic::Ordering::SeqCst), 3);
        assert!(lock_free_now, "the last holder died, so the lock is free");
        assert!(
            !msg.contains("holds its lock") && !msg.contains("fuser"),
            "nobody holds the lock, yet the error says someone does: {msg}"
        );
    }

    /// The wait's last act is a lock check. A holder that dies after the loop's previous check but
    /// before the deadline used to be reported as "the whole window was served with the holder
    /// alive", which the caller records as a wedged mux for 60 s. Dropped at 0.9 s of a 1 s wait
    /// (checks fall at 0.8 s and 1.0 s) the failure must say the holder is gone.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_holder_that_dies_just_before_the_deadline_is_reported_gone_not_wedged() {
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join("mux.lock");
        let sock_path = dir.path().join("mux.sock");
        let owner = super::claim_mux_lock(&lock_path)
            .unwrap()
            .expect("a free lock is claimed");
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(900)).await;
            drop(owner);
        });
        let failure = super::connect_to_mux(
            &sock_path,
            dir.path(),
            Duration::from_secs(1),
            Some(&lock_path),
            MUX_INIT_READ_MIN,
        )
        .await
        .err()
        .expect("no socket ever appeared");
        assert!(
            failure.holder_gone,
            "the holder died before the wait ended; a wedged mux must not be recorded"
        );
    }

    /// The wait is for a LIVE holder. When the lock turns free the holder died and no socket will
    /// ever appear, so the wait ends at once and the caller respawns, instead of sitting out the
    /// whole window.
    #[cfg(unix)]
    #[tokio::test]
    async fn connect_to_mux_stops_waiting_when_the_lock_holder_dies() {
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join("mux.lock");
        let sock_path = dir.path().join("mux.sock");
        let owner = super::claim_mux_lock(&lock_path)
            .unwrap()
            .expect("a free lock is claimed");
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(400)).await;
            drop(owner); // the holder died: its flock is released
        });

        // Real clock on purpose: the lock is released by closing a file descriptor, which a child
        // forked by a concurrent test can briefly keep open; a paused clock would race ahead of
        // that and misreport. The 5 s bound has 15 s of slack against the 20 s window.
        let started = Instant::now();
        let connected = super::connect_to_mux(
            &sock_path,
            dir.path(),
            Duration::from_secs(20),
            Some(&lock_path),
            MUX_INIT_READ_MIN,
        )
        .await;
        assert!(connected.is_err(), "no socket ever appeared");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "a dead holder must end the wait early, took {:?}",
            started.elapsed()
        );
    }

    /// A holder that never binds is still reported as wedged — after the wait, and without a
    /// respawn (the held flock blocks it). The same contract as
    /// `get_or_start_via_mux_surfaces_wedged_error_when_flock_held_socket_absent`, held in
    /// process so it runs everywhere.
    #[cfg(unix)]
    #[tokio::test(start_paused = true)]
    async fn a_peer_mux_that_never_binds_is_reported_wedged_after_the_wait() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        let lock_path = crate::lsp::mux::lock_path_for_workspace("rust", ws);
        let sock_path = crate::lsp::mux::socket_path_for_workspace("rust", ws);
        let _ = std::fs::remove_file(&sock_path);
        let _owner = super::claim_mux_lock(&lock_path)
            .unwrap()
            .expect("a free lock is claimed");

        let config = LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: ws.to_path_buf(),
            init_timeout: None,
            mux: true,
            env: vec![],
            idle_timeout_secs: None,
        };
        let started = tokio::time::Instant::now();
        let result = LspManager::new()
            .get_or_start_via_mux_waiting("rust", ws, config, Duration::from_millis(1500))
            .await;
        let elapsed = started.elapsed();
        let _ = std::fs::remove_file(&lock_path);

        let msg = match result {
            Ok(_) => panic!("held lock + absent socket must error, not connect or spawn"),
            Err(e) => e.to_string(),
        };
        assert!(msg.contains("wedged or mid-restart"), "{msg}");
        // 1500 ms is longer than the spawned-client window (800 ms), so a call site that passed
        // that window instead of the caller's wait would end early.
        assert!(
            elapsed >= Duration::from_millis(1500) && elapsed < Duration::from_millis(3000),
            "the whole wait must be served, and no more than it, took {elapsed:?}"
        );
    }
    /// Two loops call `get_or_start` once PER FILE (`search_files_restricted`, the directory
    /// overview). With a peer mux that holds its lock and never binds, every file paid the whole
    /// wait, so a name search over a few Kotlin files outran the 60 s tool timeout. The wait is
    /// paid once; the files after it fail fast with the same wedged error.
    #[cfg(unix)]
    #[tokio::test(start_paused = true)]
    async fn a_peer_mux_that_never_binds_costs_one_wait_not_one_per_file() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        let lock_path = crate::lsp::mux::lock_path_for_workspace("rust", ws);
        let sock_path = crate::lsp::mux::socket_path_for_workspace("rust", ws);
        let _ = std::fs::remove_file(&sock_path);
        let _owner = super::claim_mux_lock(&lock_path)
            .unwrap()
            .expect("a free lock is claimed");
        let config = || LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: ws.to_path_buf(),
            init_timeout: None,
            mux: true,
            env: vec![],
            idle_timeout_secs: None,
        };

        let mgr = LspManager::new();
        let wait = Duration::from_millis(600);
        let started = tokio::time::Instant::now();
        for file in 0..6 {
            let err = mgr
                .get_or_start_via_mux_waiting("rust", ws, config(), wait)
                .await
                .err()
                .unwrap_or_else(|| panic!("file {file}: a mux that never binds must error"));
            assert!(
                err.to_string().contains("wedged or mid-restart"),
                "file {file}: {err}"
            );
        }
        let elapsed = started.elapsed();
        let _ = std::fs::remove_file(&lock_path);
        assert!(
            elapsed < Duration::from_millis(1500),
            "6 files must cost about one {wait:?} wait, took {elapsed:?}"
        );
    }
    /// The wait `get_or_start` really serves for another process's mux is 30 s: the constant and
    /// its wiring through `get_or_start_via_mux`, on a paused clock so it costs no real time.
    #[cfg(unix)]
    #[tokio::test(start_paused = true)]
    async fn the_production_peer_wait_is_thirty_seconds() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        let lock_path = crate::lsp::mux::lock_path_for_workspace("rust", ws);
        let sock_path = crate::lsp::mux::socket_path_for_workspace("rust", ws);
        let _ = std::fs::remove_file(&sock_path);
        let _owner = super::claim_mux_lock(&lock_path)
            .unwrap()
            .expect("a free lock is claimed");
        let config = LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: ws.to_path_buf(),
            init_timeout: None,
            mux: true,
            env: vec![],
            idle_timeout_secs: None,
        };

        let started = tokio::time::Instant::now();
        let result = LspManager::new()
            .get_or_start_via_mux("rust", ws, config)
            .await;
        let waited = started.elapsed();
        let _ = std::fs::remove_file(&lock_path);

        assert!(result.is_err(), "a mux that never binds must error");
        assert!(
            waited >= Duration::from_secs(30) && waited < Duration::from_secs(31),
            "the production wait for another process's mux must be 30 s, was {waited:?}"
        );
    }
    /// The memory is for a mux that was waited on for the WHOLE window and still never bound. A
    /// wait that stopped early because the lock turned free means its holder DIED: nothing was
    /// found wedged, round 1 takes over, and a recorded "wedged" would deny the next 60 s of calls
    /// the wait against whatever mux starts next (docs/issues/2026-10-17-mux-connect-path-lock-races.md).
    #[cfg(unix)]
    #[tokio::test]
    async fn an_early_stop_because_the_holder_died_is_not_remembered_as_a_wedged_mux() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        let lock_path = crate::lsp::mux::lock_path_for_workspace("rust", ws);
        let sock_path = crate::lsp::mux::socket_path_for_workspace("rust", ws);
        let _ = std::fs::remove_file(&sock_path);
        let owner = super::claim_mux_lock(&lock_path)
            .unwrap()
            .expect("a free lock is claimed");
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            drop(owner); // the holder died
        });
        let config = LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: ws.to_path_buf(),
            init_timeout: None,
            mux: true,
            env: vec![],
            idle_timeout_secs: None,
        };

        let mgr = LspManager::new();
        // Round 1 finds the lock free and tries to spawn a mux, which fails in a test binary;
        // what matters is what was recorded about the wait that ended early.
        let _ = mgr
            .get_or_start_via_mux_waiting("rust", ws, config, Duration::from_secs(10))
            .await;
        let _ = std::fs::remove_file(&lock_path);

        assert!(
            mgr.mux_wedged_at.lock().unwrap().is_empty(),
            "a holder that died is not a wedged mux and must not be remembered as one"
        );
    }

    /// The memory has an edge at `MUX_WEDGED_MEMORY` (60 s): inside it a call makes one attempt,
    /// past it the call serves the full wait again. Paused clock, so both sides cost no real time.
    #[cfg(unix)]
    #[tokio::test(start_paused = true)]
    async fn the_wedge_memory_lasts_sixty_seconds_and_not_longer() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        let lock_path = crate::lsp::mux::lock_path_for_workspace("rust", ws);
        let sock_path = crate::lsp::mux::socket_path_for_workspace("rust", ws);
        let _ = std::fs::remove_file(&sock_path);
        let _owner = super::claim_mux_lock(&lock_path)
            .unwrap()
            .expect("a free lock is claimed");
        let config = || LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: ws.to_path_buf(),
            init_timeout: None,
            mux: true,
            env: vec![],
            idle_timeout_secs: None,
        };
        let mgr = LspManager::new();
        let wait = Duration::from_secs(2);
        macro_rules! timed {
            () => {{
                let started = tokio::time::Instant::now();
                assert!(mgr
                    .get_or_start_via_mux_waiting("rust", ws, config(), wait)
                    .await
                    .is_err());
                started.elapsed()
            }};
        }

        assert!(timed!() >= wait, "the first call serves the wait");

        tokio::time::advance(Duration::from_secs(59)).await;
        let inside = timed!();
        assert!(
            inside < Duration::from_secs(1),
            "59 s after the failure the memory still holds: one attempt, took {inside:?}"
        );

        tokio::time::advance(Duration::from_secs(2)).await;
        let past = timed!();
        let _ = std::fs::remove_file(&lock_path);
        assert!(
            past >= wait,
            "61 s after the failure the memory has expired and the wait is served again, took {past:?}"
        );
    }

    /// The remembered failure never blocks a mux that has come up since: the next call still
    /// makes its one connect attempt, and a success clears the memory.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_remembered_wedged_mux_is_still_tried_once_and_connects_when_it_has_come_up() {
        use tokio::io::AsyncWriteExt;
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        let lock_path = crate::lsp::mux::lock_path_for_workspace("rust", ws);
        let sock_path = crate::lsp::mux::socket_path_for_workspace("rust", ws);
        let _ = std::fs::remove_file(&sock_path);
        let _owner = super::claim_mux_lock(&lock_path)
            .unwrap()
            .expect("a free lock is claimed");
        let config = || LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: ws.to_path_buf(),
            init_timeout: None,
            mux: true,
            env: vec![],
            idle_timeout_secs: None,
        };

        let mgr = LspManager::new();
        let wait = Duration::from_millis(300);
        assert!(mgr
            .get_or_start_via_mux_waiting("rust", ws, config(), wait)
            .await
            .is_err());

        // The mux finishes starting after the failure was recorded.
        let listener = tokio::net::UnixListener::bind(&sock_path).unwrap();
        let peer = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let init = r#"{"jsonrpc":"2.0","id":0,"result":{"capabilities":{}}}"#;
            stream
                .write_all(format!("Content-Length: {}\r\n\r\n{init}", init.len()).as_bytes())
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_secs(3)).await;
        });
        let connected = mgr
            .get_or_start_via_mux_waiting("rust", ws, config(), wait)
            .await;
        peer.abort();
        let _ = std::fs::remove_file(&lock_path);
        let _ = std::fs::remove_file(&sock_path);
        assert!(
            connected.is_ok(),
            "a mux that is up must be connected to even after a recorded failure: {:?}",
            connected.err().map(|e| e.to_string())
        );
        assert!(
            mgr.mux_wedged_at.lock().unwrap().is_empty(),
            "a successful connect must clear the remembered failure"
        );
    }

    /// Regression for `docs/issues/archive/2026-08-21-mux-lsp-cold-starts-not-recorded.md`:
    /// `get_or_start`'s mux dispatch must record a real `lsp_events` success row.
    /// Before this fix, `get_or_start_via_mux` never called `do_start` — the only
    /// place `write_lsp_event`/`write_lsp_failure` lived — so every mux cold
    /// start, success or failure, was invisible to `lsp_events` even though the
    /// mux itself started fine.
    ///
    /// `get_or_start_via_mux` resolves the mux binary via `current_exe()`, which
    /// under `cargo test` is the test binary itself (confirmed: it cannot speak
    /// the mux CLI protocol, so a normal `get_or_start(..., Some(true))` call here
    /// silently takes the test-runner direct-LSP fallback instead — exercising
    /// `do_start`, already covered by `do_start_records_lsp_event_to_db`, not the
    /// mux `Ok` arm this bug is about). To reach the actual mux success path,
    /// this test pre-starts a REAL mux using the compiled `codescout` binary
    /// (not the test binary) and waits for its "ready" line — mirroring
    /// `get_or_start_via_mux`'s own protocol. `get_or_start` then finds the mux
    /// ownership lock already held, skips spawning entirely (`need_spawn =
    /// false`), and connects straight to the real socket — the same code path a
    /// second codescout process joining an existing mux takes in production.
    #[cfg(unix)]
    #[tokio::test]
    #[ignore = "spawns a real kotlin-lsp mux process; requires kotlin-lsp installed"]
    async fn get_or_start_mux_success_records_lsp_event() {
        if std::process::Command::new("kotlin-lsp")
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("Skipping: kotlin-lsp not installed");
            return;
        }
        // Honour CARGO_TARGET_DIR. `scripts/gate.sh` points it at a leased pool slot, and a
        // hardcoded `target/` here would make this test skip SILENTLY under it — a green that
        // never ran, which is the exact defect
        // `docs/issues/archive/2026-08-27-cross-process-write-lock-test-passes-when-it-does-not-run.md`
        // already cost this repo once. `CARGO_BIN_EXE_*` is not an option here: Cargo sets it
        // for integration tests, and this is a unit test inside the lib.
        let codescout_bin = match std::env::var_os("CARGO_TARGET_DIR") {
            Some(dir) => std::path::PathBuf::from(dir).join("debug/codescout"),
            None => std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/codescout"),
        };
        if !codescout_bin.exists() {
            eprintln!("Skipping: {codescout_bin:?} not built (run `cargo build --bin codescout`)");
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        let socket_path = crate::lsp::mux::socket_path_for_workspace("kotlin", ws);
        let lock_path = crate::lsp::mux::lock_path_for_workspace("kotlin", ws);

        let mut mux_child = std::process::Command::new(&codescout_bin)
            .arg("mux")
            .arg("--socket")
            .arg(&socket_path)
            .arg("--lock")
            .arg(&lock_path)
            .arg("--cwd")
            .arg(ws)
            .arg("--idle-timeout")
            .arg("60")
            .arg("--")
            .arg("kotlin-lsp")
            .arg("--stdio")
            .stdout(std::process::Stdio::piped())
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn real codescout mux");

        // Wait for the mux's "ready" line on stdout — same protocol
        // `get_or_start_via_mux` itself waits on.
        {
            use std::io::{BufRead, BufReader};
            let stdout = mux_child.stdout.take().expect("stdout piped");
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
            loop {
                line.clear();
                if reader.read_line(&mut line).unwrap_or(0) > 0 && line.trim().starts_with("ready")
                {
                    break;
                }
                if std::time::Instant::now() > deadline {
                    let _ = mux_child.kill();
                    panic!("real mux never printed 'ready' within 120s");
                }
            }
        }

        let mgr = LspManager::new_for_test_with_root(ws).await;
        let result = mgr.get_or_start("kotlin", ws, Some(true)).await;

        let _ = mux_child.kill();
        let _ = mux_child.wait();

        assert!(
            result.is_ok(),
            "expected get_or_start to connect to the pre-started real mux: {:?}",
            result.err()
        );

        let conn = crate::usage::db::open_db(ws).unwrap();
        let (lang, outcome, reason): (String, String, String) = conn
            .query_row(
                "SELECT language, outcome, reason FROM lsp_events LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(lang, "kotlin");
        assert_eq!(outcome, "success");
        assert_eq!(reason, "new_session");
    }

    #[cfg(unix)]
    #[tokio::test]
    #[ignore = "spawns a python3 fcntl holder; gated like posix_write_lock tests"]
    async fn reap_holders_of_lock_kills_an_orphan_holder() {
        use std::io::Read;
        let dir = tempfile::tempdir().unwrap();
        let lock = dir.path().join("LOCK");
        std::fs::write(&lock, b"").unwrap();
        let mut holder = std::process::Command::new("python3")
            .arg("-c")
            .arg(format!(
                "import fcntl,time; f=open(r'{}', 'r+'); \
                 fcntl.lockf(f, fcntl.LOCK_EX | fcntl.LOCK_NB); \
                 print('held', flush=True); time.sleep(30)",
                lock.display()
            ))
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("spawn holder");
        {
            let mut b = [0u8; 4];
            let _ = holder.stdout.as_mut().unwrap().read(&mut b);
        }
        // Query 1 — baseline: held.
        assert!(super::posix_write_lock_is_held(&lock), "precondition: held");
        // Query 2 — reap.
        let reaped = super::reap_holders_of_lock(&lock).await.expect("reap ok");
        assert!(reaped, "should report a reap happened");
        // Query 3 — fresh: released.
        assert!(
            !super::posix_write_lock_is_held(&lock),
            "lock freed after reap"
        );
        let _ = holder.wait();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn reap_holders_of_lock_noop_when_unheld() {
        let dir = tempfile::tempdir().unwrap();
        let lock = dir.path().join("LOCK");
        std::fs::write(&lock, b"").unwrap();
        assert!(
            !super::reap_holders_of_lock(&lock).await.unwrap(),
            "no holder → no reap"
        );
    }

    #[cfg(unix)]
    #[test]
    fn build_mux_args_includes_env_forwarding() {
        use std::path::PathBuf;
        let cfg = crate::lsp::client::LspServerConfig {
            command: "fakelsp".into(),
            args: vec!["--stdio".into()],
            workspace_root: PathBuf::from("/tmp/ws"),
            init_timeout: None,
            mux: true,
            env: vec![
                ("GRADLE_USER_HOME".into(), "/tmp/g".into()),
                ("FOO".into(), "bar".into()),
            ],
            idle_timeout_secs: Some(123),
        };
        let args = crate::lsp::manager::build_mux_args(
            &PathBuf::from("/tmp/ws"),
            &PathBuf::from("/tmp/sock"),
            &PathBuf::from("/tmp/lock"),
            &cfg,
        );
        // idle timeout honoured
        let idle_idx = args.iter().position(|a| a == "--idle-timeout").unwrap();
        assert_eq!(args[idle_idx + 1], "123");
        // env flags appear before `--`
        let dash_idx = args.iter().position(|a| a == "--").unwrap();
        let env_args: Vec<_> = args[..dash_idx]
            .iter()
            .zip(args[1..dash_idx].iter())
            .filter(|(a, _)| *a == "--env")
            .map(|(_, b)| b.clone())
            .collect();
        assert!(env_args.contains(&"GRADLE_USER_HOME=/tmp/g".to_string()));
        assert!(env_args.contains(&"FOO=bar".to_string()));
        // server command is last
        assert_eq!(args[dash_idx + 1], "fakelsp");
        assert_eq!(args[dash_idx + 2], "--stdio");
    }

    #[cfg(unix)]
    #[test]
    fn build_mux_args_defaults_idle_timeout_to_300_when_none() {
        use std::path::PathBuf;
        let cfg = crate::lsp::client::LspServerConfig {
            command: "x".into(),
            args: vec![],
            workspace_root: PathBuf::from("/tmp/ws"),
            init_timeout: None,
            mux: true,
            env: vec![],
            idle_timeout_secs: None,
        };
        let args = crate::lsp::manager::build_mux_args(
            &PathBuf::from("/tmp/ws"),
            &PathBuf::from("/tmp/sock"),
            &PathBuf::from("/tmp/lock"),
            &cfg,
        );
        let idle_idx = args.iter().position(|a| a == "--idle-timeout").unwrap();
        assert_eq!(args[idle_idx + 1], "300");
    }

    #[cfg(unix)]
    #[test]
    fn strip_deleted_suffix_recovers_rebuilt_path() {
        // /proc/self/exe after a mid-session `cargo build` rename-replace.
        assert_eq!(
            strip_deleted_suffix(std::path::Path::new(
                "/abs/target/release/codescout (deleted)"
            )),
            Some(std::path::PathBuf::from("/abs/target/release/codescout"))
        );
        // A live binary path has no marker.
        assert_eq!(
            strip_deleted_suffix(std::path::Path::new("/abs/target/release/codescout")),
            None
        );
    }

    #[test]
    fn resolve_mux_flag_override_wins() {
        assert!(!crate::lsp::manager::resolve_mux_flag(true, Some(false)));
        assert!(crate::lsp::manager::resolve_mux_flag(false, Some(true)));
    }

    #[test]
    fn resolve_mux_flag_none_uses_default() {
        assert!(crate::lsp::manager::resolve_mux_flag(true, None));
        assert!(!crate::lsp::manager::resolve_mux_flag(false, None));
    }

    #[test]
    fn is_test_runner_exe_classifies_by_deps_location() {
        use std::path::Path;
        // cargo test binaries live in target/<profile>/deps/ — including THIS crate's
        // own `codescout-<hash>` runner, which a basename check would misclassify.
        assert!(super::is_test_runner_exe(Path::new(
            "/repo/target/debug/deps/codescout-3ba224a8427ce46d"
        )));
        assert!(super::is_test_runner_exe(Path::new(
            "/repo/target/release/deps/some_integration_test-9f2a1b3c"
        )));
        // Not test runners: installed binary, `cargo run` dev binary, arbitrary path.
        assert!(!super::is_test_runner_exe(Path::new(
            "/home/u/.cargo/bin/codescout"
        )));
        assert!(!super::is_test_runner_exe(Path::new(
            "/repo/target/debug/codescout"
        )));
        assert!(!super::is_test_runner_exe(Path::new("/usr/bin/cargo")));
    }

    #[tokio::test]
    async fn project_override_forces_direct_path_for_rust() {
        let mgr = LspManager::new();
        let dir = tempfile::tempdir().unwrap();

        let default_mux = servers::default_config("rust", dir.path())
            .map(|c| c.mux)
            .unwrap_or(false);
        let effective = resolve_mux_flag(default_mux, Some(false));
        assert!(!effective, "project opt-out must force direct-process path");

        let effective_default = resolve_mux_flag(default_mux, None);
        assert_eq!(effective_default, default_mux);

        drop(mgr);
    }
}
