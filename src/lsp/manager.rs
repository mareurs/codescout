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

/// Resolve the effective `mux` flag. `override_` (from project config) wins; else fall back to `default`.
pub(super) fn resolve_mux_flag(default: bool, override_: Option<bool>) -> bool {
    override_.unwrap_or(default)
}

impl LspManager {
    /// Maximum consecutive startup failures before the circuit-breaker trips.
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
            #[cfg(test)]
            project_root_for_test: None,
        }
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
                    self.last_used.lock().await.insert(key, Instant::now());
                    return Ok(client);
                }
                Err(e) => {
                    // Mux is an optimization — fall back to direct mode rather than
                    // failing the caller. Happens in test environments where
                    // current_exe() is the test runner (not the codescout binary).
                    tracing::warn!(
                        "Mux startup failed for {language}, falling back to direct LSP: {e}"
                    );
                    config.mux = false;
                }
            }
        }

        // LRU eviction: if at capacity, shut down the least-recently-used client.
        // Lock ordering: never nest clients → last_used.  Check capacity first,
        // find the oldest under last_used alone, then re-acquire clients to remove.
        let evict_info: Option<(LspKey, Option<Arc<LspClient>>)> = {
            let at_capacity = self.clients.lock().await.len() >= self.max_clients;
            if at_capacity {
                // Find the LRU key under last_used lock alone.
                let oldest_key = {
                    let last_used = self.last_used.lock().await;
                    last_used
                        .iter()
                        .min_by_key(|(_, t)| *t)
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

    /// Start or connect to a multiplexed LSP server.
    ///
    /// The mux process is a detached codescout child that owns the real LSP
    /// server and multiplexes connections over a Unix socket.  If no mux is
    /// running for this workspace we spawn one and wait for its "ready" line
    /// on stdout before connecting.
    #[cfg(unix)]
    async fn get_or_start_via_mux(
        &self,
        language: &str,
        workspace_root: &Path,
        config: LspServerConfig,
    ) -> Result<Arc<LspClient>> {
        use anyhow::Context;
        use fs4::fs_std::FileExt;

        let socket_path = crate::lsp::mux::socket_path_for_workspace(language, workspace_root);
        let lock_path = crate::lsp::mux::lock_path_for_workspace(language, workspace_root);

        let lock_file = {
            let mut opts = std::fs::OpenOptions::new();
            opts.create(true).write(true).truncate(false);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            opts.open(&lock_path)
                .context("Failed to open mux lock file")?
        };

        let need_spawn = match lock_file.try_lock_exclusive() {
            Ok(()) => {
                // Got the lock — no mux running. Drop releases it so the
                // mux child can acquire.
                drop(lock_file);
                true
            }
            Err(_) => {
                tracing::info!(
                    "mux already running for {}, connecting to {:?}",
                    language,
                    socket_path
                );
                false
            }
        };

        if need_spawn {
            let exe =
                std::env::current_exe().context("Failed to determine codescout binary path")?;

            let mux_args = build_mux_args(workspace_root, &socket_path, &lock_path, &config);

            // Spawn mux as a detached process — do NOT set kill_on_drop
            let mut child = tokio::process::Command::new(&exe)
                .args(&mux_args)
                .stdout(std::process::Stdio::piped())
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .context("Failed to spawn mux process")?;

            // Wait for "ready" signal on stdout
            let stdout = child.stdout.take().expect("stdout piped");
            let mut reader = tokio::io::BufReader::new(stdout);
            let mut line = String::new();
            match tokio::time::timeout(
                std::time::Duration::from_secs(120),
                tokio::io::AsyncBufReadExt::read_line(&mut reader, &mut line),
            )
            .await
            {
                Ok(Ok(_)) if line.trim().starts_with("ready") => {
                    tracing::info!("mux process ready for {} at {:?}", language, socket_path);
                }
                Ok(Ok(_)) => {
                    return Err(crate::tools::RecoverableError::with_hint(
                        format!("mux process failed to start: {}", line.trim()),
                        "Check that another codescout mux isn't already running for this \
                         workspace, and that the lock file directory is writable.",
                    )
                    .into());
                }
                Ok(Err(e)) => {
                    return Err(crate::tools::RecoverableError::new(format!(
                        "mux process stdout error: {e}"
                    ))
                    .into());
                }
                Err(_) => {
                    return Err(crate::tools::RecoverableError::with_hint(
                        "mux process timed out waiting for ready (120s)",
                        "The LSP server is slow to initialize (Gradle/Cargo index?). \
                         Retry in a moment; if the problem persists, check server logs.",
                    )
                    .into());
                }
            }
            // Detach child — mux runs independently
        }

        // Connect as client, with retries
        let mut last_err = None;
        for attempt in 0..5u32 {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
            match LspClient::connect(&socket_path, workspace_root.to_path_buf()).await {
                Ok(client) => return Ok(Arc::new(client)),
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.unwrap())
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
                    let reason_clone = reason.clone();
                    let rowid_result = tokio::task::spawn_blocking(move || {
                        let conn = crate::usage::db::open_db(&root)?;
                        crate::usage::db::write_lsp_event(&conn, &lang, &reason_clone, handshake_ms)
                    })
                    .await;
                    if let Ok(Ok(rowid)) = rowid_result {
                        self.pending_first_response
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .insert(key.clone(), rowid);
                    }
                }

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
        let mut clients = self.clients.lock().await;
        for (key, client) in clients.drain() {
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

#[async_trait::async_trait]
impl crate::lsp::ops::LspProvider for LspManager {
    async fn get_or_start(
        &self,
        language: &str,
        workspace_root: &std::path::Path,
        mux_override: Option<bool>,
    ) -> anyhow::Result<Arc<dyn crate::lsp::ops::LspClientOps>> {
        let client = LspManager::get_or_start(self, language, workspace_root, mux_override).await?;
        Ok(client as Arc<dyn crate::lsp::ops::LspClientOps>)
    }

    async fn notify_file_changed(&self, path: &std::path::Path) {
        LspManager::notify_file_changed(self, path).await
    }

    async fn shutdown_all(&self) {
        LspManager::shutdown_all(self).await
    }

    async fn is_ready(&self, language: &str, workspace_root: &std::path::Path) -> bool {
        LspManager::get(self, language, workspace_root)
            .await
            .is_some()
    }

    async fn record_first_response(
        &self,
        language: &str,
        workspace_root: &std::path::Path,
        elapsed_ms: i64,
    ) {
        // Call the inherent method by name to avoid infinite recursion
        // (self.record_first_response(...) would resolve back to this trait method)
        LspManager::record_first_response_inner(self, language, workspace_root, elapsed_ms).await;
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

        // Wait 4× the TTL so the background check interval fires at least once
        tokio::time::sleep(ttl * 4).await;

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
