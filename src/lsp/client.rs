//! Async LSP client: spawns a language server subprocess and communicates
//! via JSON-RPC over stdio.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use tokio::io::AsyncWrite;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

/// Pending outbound request map: request ID → response channel.
type PendingRequests = Arc<StdMutex<HashMap<i64, oneshot::Sender<Result<Value>>>>>;

use super::transport;
use crate::tools::RecoverableError;

/// Maximum number of stderr lines retained in the shared buffer.
///
/// Only error/exception/fatal lines are buffered (others are debug-logged and
/// dropped). The buffer is checked during `initialize()` to detect fatal
/// conditions (e.g. kotlin-lsp "Multiple editing sessions"). Older lines are
/// evicted once the cap is reached to prevent unbounded growth for long-lived
/// or unusually noisy server processes.
const MAX_STDERR_LINES: usize = 200;

/// Convert an LSP `file://` URI back to a filesystem path.
///
/// Uses `url::Url` for correct handling of Windows drive letters,
/// UNC paths, and percent-encoding. Falls back to raw path extraction
/// if the URI cannot be parsed.
fn uri_to_path(uri: &lsp_types::Uri) -> PathBuf {
    // Parse the lsp_types URI string with url::Url which handles
    // Windows drive letters and percent-encoding correctly.
    url::Url::parse(uri.as_str())
        .ok()
        .and_then(|u| u.to_file_path().ok())
        .unwrap_or_else(|| PathBuf::from(uri.path().as_str()))
}

/// Convert a filesystem path to an LSP `file://` URI.
///
/// Uses `url::Url` for correct encoding of special characters and
/// proper `file:///` formatting on all platforms.
fn path_to_uri(path: &Path) -> Result<lsp_types::Uri> {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let u = url::Url::from_file_path(&abs)
        .map_err(|_| anyhow::anyhow!("cannot convert path to URI: {}", abs.display()))?;
    u.as_str()
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid URI: {}", e))
}

/// Return true if the given LSP method is safe to retry after a
/// `-32800 RequestCancelled` response.
///
/// Idempotent methods only return information — retrying them may do extra
/// work server-side but does not double-apply any mutation. Methods like
/// `textDocument/rename` or `workspace/applyEdit` MAY have partially mutated
/// state before the cancellation, so retrying can double-apply edits.
fn is_idempotent_lsp_method(method: &str) -> bool {
    matches!(
        method,
        "textDocument/documentSymbol"
            | "textDocument/references"
            | "textDocument/hover"
            | "textDocument/definition"
            | "textDocument/declaration"
            | "textDocument/typeDefinition"
            | "textDocument/implementation"
            | "textDocument/completion"
            | "textDocument/signatureHelp"
            | "textDocument/codeAction"
            | "textDocument/codeLens"
            | "textDocument/foldingRange"
            | "textDocument/selectionRange"
            | "textDocument/prepareRename"
            | "workspace/symbol"
            | "initialize"
    )
}

/// Return true if cold-start's extended retry budget makes sense for `method`.
///
/// Most LSP queries that return `-32800 RequestCancelled` during cold start
/// become answerable per-file as the server parses each file, so retrying
/// for ~45s catches them. But `workspace/symbol` stays unanswerable until
/// the *whole* project is indexed (minutes for large Rust/Kotlin projects),
/// and a 45s retry budget + 30s per-attempt timeout blows through the MCP
/// 60s tool timeout. For that method we keep the short warm budget and let
/// callers (e.g. `find_symbol`) fail over to tree-sitter quickly.
fn uses_cold_start_retry_budget(method: &str) -> bool {
    !matches!(method, "workspace/symbol")
}

/// Scan a stderr line buffer for patterns that make LSP-side retries pointless.
///
/// Pure helper so it can be exercised in unit tests without spinning up a real
/// server. Add new patterns here when we encounter additional permanent-failure
/// modes (as opposed to transient ones that legitimately benefit from retry).
fn detect_fatal_stderr(lines: &[String]) -> Option<RecoverableError> {
    for line in lines {
        if line.contains("Multiple editing sessions") {
            return Some(RecoverableError::with_hint(
                "kotlin-lsp rejected this workspace: \
                 \"Multiple editing sessions for one workspace are not supported yet\"",
                "Another codescout instance or editor is already serving this \
                 project with kotlin-lsp. Only one Kotlin LSP session per \
                 workspace is allowed in the current kotlin-lsp release. \
                 Stop the other session and retry.",
            ));
        }
    }
    None
}

/// Convert hierarchical `DocumentSymbol[]` into our `SymbolInfo` tree.
fn convert_document_symbols(
    symbols: &[lsp_types::DocumentSymbol],
    file: &PathBuf,
    parent_path: &str,
) -> Vec<super::SymbolInfo> {
    symbols
        .iter()
        .map(|ds| {
            let name_path = if parent_path.is_empty() {
                ds.name.clone()
            } else {
                format!("{}/{}", parent_path, ds.name)
            };
            let children = ds
                .children
                .as_ref()
                .map(|c| convert_document_symbols(c, file, &name_path))
                .unwrap_or_default();
            super::SymbolInfo {
                name: ds.name.clone(),
                name_path: name_path.clone(),
                kind: ds.kind.into(),
                file: file.clone(),
                start_line: ds.selection_range.start.line,
                end_line: ds.range.end.line,
                start_col: ds.selection_range.start.character,
                range_start_line: Some(ds.range.start.line),
                children,
                detail: ds.detail.clone().filter(|s| !s.is_empty()),
            }
        })
        .collect()
}

/// Configuration for launching a language server.
#[derive(Debug, Clone)]
pub struct LspServerConfig {
    pub command: String,
    #[allow(dead_code)]
    pub args: Vec<String>,
    pub workspace_root: std::path::PathBuf,
    /// Timeout for the LSP `initialize` handshake. JVM-based servers need longer.
    /// Defaults to 30s if not set.
    pub init_timeout: Option<std::time::Duration>,
    /// If true, this language uses the LSP multiplexer for shared instances.
    pub mux: bool,
    /// Additional environment variables for the LSP server process.
    pub env: Vec<(String, String)>,
    /// Seconds the mux process waits with no connected clients before
    /// exiting. Only used when `mux == true`. `None` falls back to the
    /// mux default of 300s. Ignored on the direct-process path.
    pub idle_timeout_secs: Option<u64>,
}

/// How this LspClient is connected to its language server.
#[derive(Debug)]
#[allow(dead_code)] // Socket variant used in Task 3 (LspClient::connect)
pub(crate) enum LspTransport {
    /// Direct child process (normal LSP servers).
    Process { child_pid: Option<u32> },
    /// Connected to a mux socket (shared LSP servers like kotlin-lsp).
    Socket { socket_path: std::path::PathBuf },
}

/// A running LSP client session connected to a language server process.
pub struct LspClient {
    writer: Arc<Mutex<Box<dyn AsyncWrite + Unpin + Send>>>,
    #[allow(dead_code)]
    next_id: AtomicI64,
    #[allow(dead_code)]
    pending: Arc<StdMutex<HashMap<i64, oneshot::Sender<Result<Value>>>>>,
    #[allow(dead_code)]
    alive: Arc<AtomicBool>,
    #[allow(dead_code)]
    reader_handle: StdMutex<Option<JoinHandle<()>>>,
    pub workspace_root: std::path::PathBuf,
    #[allow(dead_code)]
    pub(crate) capabilities: StdMutex<lsp_types::ServerCapabilities>,
    transport: LspTransport,
    /// Timeout for the LSP initialize handshake.
    init_timeout: std::time::Duration,
    /// Tracks files opened via textDocument/didOpen, mapped to their current document version.
    /// The LSP spec requires a monotonically increasing version on every didOpen/didChange.
    /// Keys are canonicalized paths to avoid symlink/relative-path aliases.
    /// The spec prohibits sending didOpen for an already-open file without an
    /// intervening didClose; some servers (e.g. kotlin-lsp) error on duplicates.
    open_files: StdMutex<HashMap<PathBuf, i32>>,
    /// Collects stderr lines from the server process. Checked during init retries
    /// to detect fatal errors (e.g. kotlin-lsp "Multiple editing sessions").
    stderr_lines: Arc<StdMutex<Vec<String>>>,
    /// Wall-clock instant when the LSP client was constructed. Used as a
    /// fallback anchor for the cold-start window if init hasn't completed yet.
    pub(crate) started_at: std::time::Instant,
    /// Set when the LSP `initialize` handshake completes successfully. The
    /// cold-start retry window is measured from this point, not from
    /// construction — otherwise a slow kotlin-lsp init (5+ min Gradle import)
    /// consumes the whole budget before the first user request.
    pub(crate) init_completed_at: std::sync::OnceLock<std::time::Instant>,
}

impl LspClient {
    /// Dispatch a single incoming LSP message to the appropriate pending sender,
    /// or auto-respond null to server-to-client requests.
    ///
    /// Shared by the reader tasks in [`LspClient::start`] (process transport) and
    /// [`LspClient::connect`] (socket/mux transport). Both loops are structurally
    /// identical in the `Ok` branch; they differ only in error handling (process
    /// exit diagnostics vs. mux disconnection).
    ///
    /// `request_label` and `notification_label` control the tracing output so log
    /// lines identify the transport in use.
    async fn dispatch_lsp_message(
        msg: Value,
        pending: &PendingRequests,
        writer: &Arc<Mutex<Box<dyn AsyncWrite + Unpin + Send>>>,
        request_label: &str,
        notification_label: &str,
    ) {
        if let Some(id) = msg.get("id").and_then(|v| v.as_i64()) {
            if msg.get("method").is_some() {
                // Server-to-client request. Auto-respond null so the server doesn't stall.
                tracing::debug!(
                    "{} (id={}): {} — auto-responding null",
                    request_label,
                    id,
                    msg["method"]
                );
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": null,
                });
                let mut w = writer.lock().await;
                let _ = transport::write_message(&mut *w, &response).await;
            } else {
                // Response to one of our outbound requests.
                if let Some(sender) = pending
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&id)
                {
                    if let Some(error) = msg.get("error") {
                        let err_msg = error["message"].as_str().unwrap_or("unknown LSP error");
                        let _ = sender.send(Err(anyhow::anyhow!(
                            "LSP error (code {}): {}",
                            error["code"],
                            err_msg
                        )));
                    } else {
                        let result = msg.get("result").cloned().unwrap_or(Value::Null);
                        let _ = sender.send(Ok(result));
                    }
                }
            }
        } else if let Some(method) = msg.get("method").and_then(|v| v.as_str()) {
            tracing::debug!("{}: {}", notification_label, method);
        }
    }

    /// Read messages from `reader` and dispatch them via `dispatch_lsp_message`
    /// until a transport error occurs. Returns the error so callers can run
    /// transport-specific cleanup before draining pending requests.
    async fn run_dispatch_loop<R>(
        mut reader: BufReader<R>,
        pending: PendingRequests,
        writer: Arc<Mutex<Box<dyn AsyncWrite + Unpin + Send>>>,
        request_label: &'static str,
        notif_label: &'static str,
    ) -> anyhow::Error
    where
        R: tokio::io::AsyncRead + Unpin + Send + 'static,
    {
        loop {
            match transport::read_message(&mut reader).await {
                Ok(msg) => {
                    Self::dispatch_lsp_message(msg, &pending, &writer, request_label, notif_label)
                        .await;
                }
                Err(e) => return e,
            }
        }
    }

    /// Drain all pending requests with a disconnect error message.
    fn drain_pending_disconnect(pending: &PendingRequests, msg: &'static str) {
        let mut map = pending.lock().unwrap_or_else(|e| e.into_inner());
        for (_, sender) in map.drain() {
            let _ = sender.send(Err(anyhow::anyhow!(msg)));
        }
    }

    /// Start a language server process and perform the LSP initialize handshake.
    /// Start a language server process and perform the LSP initialize handshake.
    pub async fn start(config: LspServerConfig) -> Result<Self> {
        tracing::info!("Starting LSP server: {} {:?}", config.command, config.args);

        let mut cmd = Command::new(&config.command);
        cmd.args(&config.args)
            .current_dir(&config.workspace_root)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        for (key, val) in &config.env {
            cmd.env(key, val);
        }
        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to start LSP server: {}", config.command))?;

        // These `.take().expect()` calls are infallible: stdin, stdout, and stderr are
        // configured as `Stdio::piped()` in the Command builder immediately above, so
        // tokio guarantees they are `Some` after a successful spawn.
        let stdin = child.stdin.take().expect("stdin must be piped");
        let stdout = child.stdout.take().expect("stdout must be piped");
        let stderr = child.stderr.take().expect("stderr must be piped");
        let child_pid = child.id();
        tracing::debug!(
            pid = ?child_pid,
            binary = %config.command,
            "LSP server spawned"
        );

        let pending: PendingRequests = Arc::new(StdMutex::new(HashMap::new()));
        let alive = Arc::new(AtomicBool::new(true));

        // Wrap writer in Arc so the reader task can share it for auto-responses.
        let writer = Arc::new(Mutex::new(
            Box::new(stdin) as Box<dyn AsyncWrite + Unpin + Send>
        ));

        // Shared stderr buffer — checked by initialize() to detect fatal errors.
        let stderr_lines: Arc<StdMutex<Vec<String>>> = Arc::new(StdMutex::new(Vec::new()));
        let stderr_lines_clone = stderr_lines.clone();

        // Spawn stderr reader (logs server stderr, populates shared buffer)
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            loop {
                line.clear();
                match tokio::io::AsyncBufReadExt::read_line(&mut reader, &mut line).await {
                    Ok(0) => break,
                    Ok(_) => {
                        let trimmed = line.trim_end();
                        let lower = trimmed.to_lowercase();
                        if lower.contains("error")
                            || lower.contains("exception")
                            || lower.contains("fatal")
                        {
                            tracing::warn!(target: "lsp_stderr", "{}", trimmed);
                            let mut buf =
                                stderr_lines_clone.lock().unwrap_or_else(|e| e.into_inner());
                            if buf.len() >= MAX_STDERR_LINES {
                                buf.remove(0);
                            }
                            buf.push(trimmed.to_string());
                        } else {
                            tracing::debug!(target: "lsp_stderr", "{}", trimmed);
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // Spawn stdout reader task — dispatches responses to pending senders
        let pending_clone = pending.clone();
        let alive_clone = alive.clone();
        let writer_clone = writer.clone();
        let reader_handle = tokio::spawn(async move {
            let read_err = Self::run_dispatch_loop(
                BufReader::new(stdout),
                pending_clone.clone(),
                writer_clone,
                "LSP server request",
                "LSP notification",
            )
            .await;
            // EOF or read error — server crashed or exited
            if alive_clone.load(Ordering::SeqCst) {
                tracing::warn!("LSP reader error: {}", read_err);
            }
            // Try to get the exit status for diagnostics.
            // try_wait() returns Ok(None) if the child is still running (rare at EOF),
            // Ok(Some(status)) if it has exited, or Err if the call itself failed.
            match child.try_wait() {
                Ok(Some(status)) => {
                    tracing::warn!(exit_status = ?status, "LSP server exited")
                }
                Ok(None) => tracing::warn!("LSP reader EOF but child still running"),
                Err(wait_err) => {
                    tracing::warn!("could not get LSP exit status: {wait_err}")
                }
            }
            alive_clone.store(false, Ordering::SeqCst);
            Self::drain_pending_disconnect(&pending_clone, "LSP server disconnected");
            // Wait for child to exit (kill_on_drop will handle cleanup)
            let _ = child.wait().await;
        });
        let init_timeout = config
            .init_timeout
            .unwrap_or(std::time::Duration::from_secs(30));

        let client = Self {
            writer,
            next_id: AtomicI64::new(1),
            pending,
            alive,
            reader_handle: StdMutex::new(Some(reader_handle)),
            workspace_root: config.workspace_root.clone(),
            capabilities: StdMutex::new(lsp_types::ServerCapabilities::default()),
            transport: LspTransport::Process { child_pid },
            init_timeout,
            open_files: StdMutex::new(HashMap::new()),
            stderr_lines,
            started_at: std::time::Instant::now(),
            init_completed_at: std::sync::OnceLock::new(),
        };

        // Perform the LSP initialize handshake
        client.initialize().await?;

        Ok(client)
    }

    /// Connect to an existing mux socket instead of spawning a process.
    ///
    /// The mux sends a JSON init message immediately on connect containing
    /// the cached `InitializeResult`. This client does NOT perform the LSP
    /// initialize handshake.
    pub async fn connect(
        socket_path: &std::path::Path,
        workspace_root: std::path::PathBuf,
    ) -> Result<Self> {
        use crate::lsp::mux::transport as mux_transport;

        let stream = mux_transport::connect(socket_path).await?;

        let (read_half, write_half) = mux_transport::split_client(stream);

        let pending: PendingRequests = Arc::new(StdMutex::new(HashMap::new()));
        let alive = Arc::new(AtomicBool::new(true));
        let writer: Arc<Mutex<Box<dyn AsyncWrite + Unpin + Send>>> =
            Arc::new(Mutex::new(Box::new(write_half)));

        // Read init message from the mux — contains the cached InitializeResult
        // so we skip the full LSP handshake.
        let mut buf_reader = BufReader::new(read_half);
        let init_msg = transport::read_message(&mut buf_reader)
            .await
            .context("Failed to read mux init message")?;

        let capabilities = if let Some(result) = init_msg.get("result") {
            let init_result: lsp_types::InitializeResult =
                serde_json::from_value(result.clone())
                    .context("Failed to parse InitializeResult from mux")?;
            init_result.capabilities
        } else {
            tracing::warn!("Mux init message missing 'result' field, using default capabilities");
            lsp_types::ServerCapabilities::default()
        };

        // Spawn reader task — dispatches responses to pending senders.
        let pending_clone = pending.clone();
        let alive_clone = alive.clone();
        let writer_clone = writer.clone();
        let reader_handle = tokio::spawn(async move {
            let _read_err = Self::run_dispatch_loop(
                buf_reader,
                pending_clone.clone(),
                writer_clone,
                "mux forwarded server request",
                "LSP notification from mux",
            )
            .await;
            alive_clone.store(false, Ordering::SeqCst);
            Self::drain_pending_disconnect(&pending_clone, "Mux connection lost");
        });

        Ok(Self {
            writer,
            next_id: AtomicI64::new(1),
            pending,
            alive,
            reader_handle: StdMutex::new(Some(reader_handle)),
            workspace_root,
            capabilities: StdMutex::new(capabilities),
            transport: LspTransport::Socket {
                socket_path: socket_path.to_path_buf(),
            },
            init_timeout: std::time::Duration::from_secs(30),
            open_files: StdMutex::new(HashMap::new()),
            stderr_lines: Arc::new(StdMutex::new(Vec::new())),
            started_at: std::time::Instant::now(),
            init_completed_at: std::sync::OnceLock::new(),
        })
    }

    /// Send a JSON-RPC request and await the response.
    pub async fn request(&self, method: &str, params: Value) -> Result<Value> {
        // During the cold-start indexing window (e.g. Gradle import for kotlin-lsp),
        // the server returns -32800 (RequestCancelled) for every query. We use a
        // patient retry window while fresh, and a short one once warm.
        //
        // Cold: 10 retries × 3 s linear backoff ≈ 45 s max wait.
        // Warm:  3 retries × 300 ms linear backoff ≈ 1.2 s max wait.
        const COLD_START_WINDOW: std::time::Duration = std::time::Duration::from_secs(5 * 60);
        const MAX_RETRIES_COLD: usize = 10;
        const RETRY_DELAY_COLD_MS: u64 = 3_000;
        const MAX_RETRIES_WARM: usize = 3;
        const RETRY_DELAY_WARM_MS: u64 = 300;

        // Anchor cold-start budget at init completion if we have it; otherwise
        // fall back to construction time so in-flight init requests still
        // benefit from the patient window.
        let anchor = self.init_completed_at.get().unwrap_or(&self.started_at);
        let in_cold_start = anchor.elapsed() < COLD_START_WINDOW;
        let (max_retries, retry_delay_ms) = if in_cold_start && uses_cold_start_retry_budget(method)
        {
            (MAX_RETRIES_COLD, RETRY_DELAY_COLD_MS)
        } else {
            (MAX_RETRIES_WARM, RETRY_DELAY_WARM_MS)
        };

        // Only retry idempotent methods on -32800 (RequestCancelled). Retrying
        // a non-idempotent method like textDocument/rename risks double-applying
        // an edit if the server cancelled AFTER performing the operation.
        let retry_on_cancel = is_idempotent_lsp_method(method);
        let effective_max_retries = if retry_on_cancel { max_retries } else { 0 };

        let mut last_err = None;
        for attempt in 0..=effective_max_retries {
            if attempt > 0 {
                let delay = std::time::Duration::from_millis(retry_delay_ms * attempt as u64);
                tokio::time::sleep(delay).await;
                tracing::debug!(
                    "LSP request cancelled, retrying {}/{}: {} (cold_start={})",
                    attempt,
                    effective_max_retries,
                    method,
                    in_cold_start,
                );
            }
            match self
                .request_with_timeout(method, params.clone(), std::time::Duration::from_secs(30))
                .await
            {
                Ok(result) => return Ok(result),
                Err(e) if e.to_string().contains("code -32800") => {
                    if !retry_on_cancel {
                        // Surface as RecoverableError so sibling tool calls
                        // survive and the caller can retry at a higher level
                        // where the semantics are known.
                        return Err(RecoverableError::with_hint(
                            format!("LSP cancelled non-idempotent request: {method}"),
                            "The server cancelled mid-operation. Retry is unsafe here because \
                             the edit may have partially applied. Re-issue the request manually.",
                        )
                        .into());
                    }
                    last_err = Some(e);
                }
                Err(e) => return Err(e),
            }
        }
        Err(last_err.unwrap())
    }

    #[tracing::instrument(skip(self, params, timeout), fields(lsp_method = %method))]
    pub async fn request_with_timeout(
        &self,
        method: &str,
        params: Value,
        timeout: std::time::Duration,
    ) -> Result<Value> {
        if !self.alive.load(Ordering::SeqCst) {
            return Err(RecoverableError::with_hint(
                "LSP server is not running",
                "The language server exited or failed to start. Try re-activating the \
                 project or check logs for server startup errors.",
            )
            .into());
        }

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();

        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, tx);

        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        {
            let mut writer = self.writer.lock().await;
            if let Err(e) = transport::write_message(&mut *writer, &msg).await {
                self.pending
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&id);
                return Err(e);
            }
        }

        // Await response with timeout
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(result)) => {
                match &result {
                    Ok(v) => {
                        tracing::debug!(response_bytes = v.to_string().len(), "lsp response");
                    }
                    Err(e) => {
                        tracing::debug!(error = %e, "lsp response error");
                    }
                }
                result
            }
            Ok(Err(_)) => bail!("LSP response channel closed"),
            Err(_) => {
                self.pending
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&id);
                // Tell the server to stop working on this id — otherwise it
                // keeps computing (real CPU waste for slow kotlin-lsp /
                // rust-analyzer queries during Gradle or Cargo load).
                let _ = self.notify("$/cancelRequest", json!({ "id": id })).await;
                Err(RecoverableError::with_hint(
                    format!(
                        "LSP request timed out after {}s: {}",
                        timeout.as_secs(),
                        method
                    ),
                    "The server did not respond in time. This is common during cold \
                     start or heavy indexing; retry in a moment.",
                )
                .into())
            }
        }
    }

    /// Send a JSON-RPC notification (no response expected).
    pub async fn notify(&self, method: &str, params: Value) -> Result<()> {
        if !self.alive.load(Ordering::SeqCst) {
            return Err(RecoverableError::with_hint(
                "LSP server is not running",
                "The language server exited or failed to start.",
            )
            .into());
        }

        let msg = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });

        let mut writer = self.writer.lock().await;
        transport::write_message(&mut *writer, &msg).await
    }

    /// Scan the buffered server stderr for patterns that make further
    /// retries pointless. Returns a `RecoverableError` with a user-facing
    /// hint if a known-fatal pattern is present.
    ///
    /// The caller is expected to surface the error to the tool layer; sibling
    /// tool calls should keep working. Add new patterns here as we encounter
    /// more permanent-failure modes.
    fn fatal_stderr_hint(&self) -> Option<RecoverableError> {
        let stderr = self.stderr_lines.lock().unwrap_or_else(|e| e.into_inner());
        detect_fatal_stderr(&stderr)
    }

    /// Perform the LSP initialize/initialized handshake.
    ///
    /// Retries on -32800 (RequestCancelled) because JVM-based servers like
    /// kotlin-lsp may return this during early JVM bootstrap before they're
    /// ready to handle the initialize request.
    async fn initialize(&self) -> Result<()> {
        let root_uri = path_to_uri(&self.workspace_root)?;

        let params = lsp_types::InitializeParams {
            process_id: Some(std::process::id()),
            capabilities: lsp_types::ClientCapabilities {
                text_document: Some(lsp_types::TextDocumentClientCapabilities {
                    document_symbol: Some(lsp_types::DocumentSymbolClientCapabilities {
                        hierarchical_document_symbol_support: Some(true),
                        ..Default::default()
                    }),
                    references: Some(lsp_types::DynamicRegistrationClientCapabilities {
                        dynamic_registration: Some(false),
                    }),
                    definition: Some(lsp_types::GotoCapability {
                        dynamic_registration: Some(false),
                        link_support: Some(false),
                    }),
                    rename: Some(lsp_types::RenameClientCapabilities {
                        prepare_support: Some(true),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
            workspace_folders: Some(vec![lsp_types::WorkspaceFolder {
                uri: root_uri,
                name: self
                    .workspace_root
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
            }]),
            ..Default::default()
        };

        // Retry on -32800 (RequestCancelled) during initialization.
        // JVM-based servers (kotlin-lsp) may cancel the init request while
        // still bootstrapping their platform subsystems.
        const MAX_INIT_RETRIES: usize = 5;
        const INIT_RETRY_DELAY_MS: u64 = 3000;

        let params_value = serde_json::to_value(params)?;
        let mut last_err = None;
        for attempt in 0..=MAX_INIT_RETRIES {
            if attempt > 0 {
                let delay = std::time::Duration::from_millis(INIT_RETRY_DELAY_MS * attempt as u64);
                tokio::time::sleep(delay).await;
                tracing::info!(
                    "LSP initialize cancelled, retrying {}/{}: {}",
                    attempt,
                    MAX_INIT_RETRIES,
                    self.workspace_root.display()
                );
            }
            // Pre-flight stderr check: a fatal error (e.g. kotlin-lsp
            // "Multiple editing sessions") may have been emitted between
            // attempts. Retrying would just spawn another doomed request.
            if let Some(fatal) = self.fatal_stderr_hint() {
                return Err(fatal.into());
            }
            match self
                .request_with_timeout("initialize", params_value.clone(), self.init_timeout)
                .await
            {
                Ok(result) => {
                    // Parse and store server capabilities
                    let init_result: lsp_types::InitializeResult = serde_json::from_value(result)?;
                    *self.capabilities.lock().unwrap_or_else(|e| e.into_inner()) =
                        init_result.capabilities;

                    // Send initialized notification
                    self.notify("initialized", json!({})).await?;

                    // Anchor the cold-start retry budget here, not at construction.
                    // A slow kotlin-lsp init (5+ min Gradle import) used to burn
                    // the whole window before the first user request.
                    let _ = self.init_completed_at.set(std::time::Instant::now());

                    tracing::info!("LSP server initialized successfully");
                    return Ok(());
                }
                Err(e) => {
                    // Any error path (-32800, timeout, disconnect, …): check
                    // stderr for fatal patterns before deciding whether to
                    // retry. The original code only checked on -32800, which
                    // missed the common case where kotlin-lsp crashes mid-init
                    // and the next attempt times out or hits a closed pipe
                    // rather than -32800.
                    if let Some(fatal) = self.fatal_stderr_hint() {
                        return Err(fatal.into());
                    }
                    if e.to_string().contains("code -32800") {
                        last_err = Some(e);
                    } else {
                        return Err(e);
                    }
                }
            }
        }
        Err(last_err.unwrap())
    }

    /// Check if the server process is still alive.
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// Gracefully shut down the LSP server.
    pub async fn shutdown(&self) -> Result<()> {
        if !self.alive.load(Ordering::SeqCst) {
            return Ok(());
        }

        // Send shutdown request
        let _ = self.request("shutdown", Value::Null).await;

        // Send exit notification
        let _ = self.notify("exit", Value::Null).await;

        // Mark as dead
        self.alive.store(false, Ordering::SeqCst);

        // Wait for reader task to finish (with timeout)
        // Extract handle before awaiting to avoid holding MutexGuard across await
        let handle = self
            .reader_handle
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(handle) = handle {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;
        }

        Ok(())
    }

    /// Request all symbols in the workspace matching a query string.
    ///
    /// Uses `workspace/symbol` — one round-trip for the whole project, vs
    /// `textDocument/documentSymbol` which requires one request per file.
    /// Returns a flat list (no hierarchy); `container_name` is preserved in
    /// `name_path` when available.
    pub async fn workspace_symbols(&self, query: &str) -> Result<Vec<super::SymbolInfo>> {
        let params = lsp_types::WorkspaceSymbolParams {
            query: query.to_string(),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let result = self
            .request("workspace/symbol", serde_json::to_value(params)?)
            .await?;

        if result.is_null() {
            return Ok(vec![]);
        }

        let infos: Vec<lsp_types::SymbolInformation> =
            serde_json::from_value(result).context("failed to parse workspace/symbol response")?;

        Ok(infos
            .into_iter()
            .map(|si| {
                let file = uri_to_path(&si.location.uri);
                let name_path = match &si.container_name {
                    Some(container) if !container.is_empty() => {
                        format!("{}/{}", container, si.name)
                    }
                    _ => si.name.clone(),
                };
                super::SymbolInfo {
                    name: si.name,
                    name_path,
                    kind: si.kind.into(),
                    file,
                    start_line: si.location.range.start.line,
                    end_line: si.location.range.end.line,
                    start_col: si.location.range.start.character,
                    range_start_line: None,
                    children: vec![],
                    detail: None,
                }
            })
            .collect())
    }

    /// Send textDocument/didOpen notification for a file.
    pub async fn did_open(&self, path: &Path, language_id: &str) -> Result<()> {
        // For socket transport, the mux handles document state dedup — skip
        // local open_files tracking so every didOpen is forwarded to the mux.
        let is_socket = matches!(self.transport, LspTransport::Socket { .. });
        if !is_socket {
            // Canonicalize before tracking to avoid treating symlinks or relative paths
            // as different files — the LSP spec prohibits duplicate didOpen notifications.
            let canonical = crate::platform::canonicalize(path)
                .with_context(|| format!("Failed to canonicalize path for didOpen: {:?}", path))?;
            {
                let mut open_files = self.open_files.lock().unwrap_or_else(|e| e.into_inner());
                if open_files.contains_key(&canonical) {
                    return Ok(());
                }
                // Version 1 is the conventional initial version per LSP spec.
                open_files.insert(canonical, 1);
            }
        }

        const MAX_DID_OPEN_SIZE: u64 = 10 * 1024 * 1024; // 10 MiB
        if let Ok(metadata) = std::fs::metadata(path) {
            if metadata.len() > MAX_DID_OPEN_SIZE {
                tracing::debug!(
                    "skipping didOpen for large file ({} bytes): {}",
                    metadata.len(),
                    path.display()
                );
                return Ok(());
            }
        }

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read file for didOpen: {:?}", path))?;
        let uri = path_to_uri(path)?;

        self.notify(
            "textDocument/didOpen",
            serde_json::to_value(lsp_types::DidOpenTextDocumentParams {
                text_document: lsp_types::TextDocumentItem {
                    uri,
                    language_id: language_id.to_string(),
                    version: 1,
                    text: content,
                },
            })?,
        )
        .await
    }

    /// Request document symbols for a file.
    ///
    /// Returns the hierarchical `DocumentSymbol[]` response parsed into our
    /// `SymbolInfo` tree. Sends `didOpen` first if the file hasn't been opened.
    pub async fn document_symbols(
        &self,
        path: &Path,
        language_id: &str,
    ) -> Result<Vec<super::SymbolInfo>> {
        // Ensure the file is open in the server
        self.did_open(path, language_id).await?;

        let uri = path_to_uri(path)?;
        let params = lsp_types::DocumentSymbolParams {
            text_document: lsp_types::TextDocumentIdentifier { uri },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let result = self
            .request("textDocument/documentSymbol", serde_json::to_value(params)?)
            .await?;

        // LSP returns either DocumentSymbol[] (hierarchical) or SymbolInformation[] (flat)
        // We prefer hierarchical and convert both to SymbolInfo
        if result.is_null() {
            return Ok(vec![]);
        }

        let file_path = path.to_path_buf();

        // Try hierarchical first
        if let Ok(symbols) =
            serde_json::from_value::<Vec<lsp_types::DocumentSymbol>>(result.clone())
        {
            return Ok(convert_document_symbols(&symbols, &file_path, ""));
        }

        // Fall back to flat SymbolInformation[]
        if let Ok(infos) = serde_json::from_value::<Vec<lsp_types::SymbolInformation>>(result) {
            return Ok(infos
                .iter()
                .map(|si| {
                    let name_path = match &si.container_name {
                        Some(container) if !container.is_empty() => {
                            format!("{}/{}", container, si.name)
                        }
                        _ => si.name.clone(),
                    };
                    super::SymbolInfo {
                        name: si.name.clone(),
                        name_path,
                        kind: si.kind.into(),
                        file: file_path.clone(),
                        start_line: si.location.range.start.line,
                        end_line: si.location.range.end.line,
                        start_col: si.location.range.start.character,
                        range_start_line: None,
                        children: vec![],
                        detail: None,
                    }
                })
                .collect());
        }

        Ok(vec![])
    }

    /// Request references for a symbol at a given position.
    pub async fn references(
        &self,
        path: &Path,
        line: u32,
        col: u32,
        language_id: &str,
    ) -> Result<Vec<lsp_types::Location>> {
        self.did_open(path, language_id).await?;
        let uri = path_to_uri(path)?;
        let params = lsp_types::ReferenceParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
                position: lsp_types::Position {
                    line,
                    character: col,
                },
            },
            context: lsp_types::ReferenceContext {
                include_declaration: true,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let result = self
            .request("textDocument/references", serde_json::to_value(params)?)
            .await?;

        if result.is_null() {
            return Ok(vec![]);
        }

        Ok(serde_json::from_value(result)?)
    }

    /// Request definition location for a symbol at a given position.
    pub async fn goto_definition(
        &self,
        path: &Path,
        line: u32,
        col: u32,
        language_id: &str,
    ) -> Result<Vec<lsp_types::Location>> {
        self.did_open(path, language_id).await?;
        let uri = path_to_uri(path)?;
        let params = lsp_types::GotoDefinitionParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
                position: lsp_types::Position {
                    line,
                    character: col,
                },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let result = self
            .request("textDocument/definition", serde_json::to_value(params)?)
            .await?;

        if result.is_null() {
            return Ok(vec![]);
        }

        // Can return Location | Location[] | LocationLink[]
        if let Ok(loc) = serde_json::from_value::<lsp_types::Location>(result.clone()) {
            return Ok(vec![loc]);
        }
        if let Ok(locs) = serde_json::from_value::<Vec<lsp_types::Location>>(result.clone()) {
            return Ok(locs);
        }
        if let Ok(links) = serde_json::from_value::<Vec<lsp_types::LocationLink>>(result) {
            return Ok(links
                .into_iter()
                .map(|l| lsp_types::Location {
                    uri: l.target_uri,
                    range: l.target_selection_range,
                })
                .collect());
        }

        Ok(vec![])
    }

    /// Send textDocument/hover and return the hover contents as a string.
    pub async fn hover(
        &self,
        path: &Path,
        line: u32,
        col: u32,
        language_id: &str,
    ) -> Result<Option<String>> {
        self.did_open(path, language_id).await?;
        let uri = path_to_uri(path)?;
        let params = lsp_types::HoverParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
                position: lsp_types::Position {
                    line,
                    character: col,
                },
            },
            work_done_progress_params: Default::default(),
        };

        let result = self
            .request("textDocument/hover", serde_json::to_value(params)?)
            .await?;

        if result.is_null() {
            return Ok(None);
        }

        let hover: lsp_types::Hover = serde_json::from_value(result)?;

        let text = match hover.contents {
            lsp_types::HoverContents::Scalar(ms) => match ms {
                lsp_types::MarkedString::String(s) => s,
                lsp_types::MarkedString::LanguageString(ls) => ls.value,
            },
            lsp_types::HoverContents::Array(arr) => arr
                .into_iter()
                .map(|ms| match ms {
                    lsp_types::MarkedString::String(s) => s,
                    lsp_types::MarkedString::LanguageString(ls) => ls.value,
                })
                .collect::<Vec<_>>()
                .join("\n\n"),
            lsp_types::HoverContents::Markup(mc) => mc.value,
        };

        Ok(Some(text))
    }

    /// Request a rename across the workspace.
    pub async fn rename(
        &self,
        path: &Path,
        line: u32,
        col: u32,
        new_name: &str,
        language_id: &str,
    ) -> Result<lsp_types::WorkspaceEdit> {
        self.did_open(path, language_id).await?;
        let uri = path_to_uri(path)?;
        let params = lsp_types::RenameParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
                position: lsp_types::Position {
                    line,
                    character: col,
                },
            },
            new_name: new_name.to_string(),
            work_done_progress_params: Default::default(),
        };

        let result = self
            .request("textDocument/rename", serde_json::to_value(params)?)
            .await?;

        Ok(serde_json::from_value(result)?)
    }

    /// Send textDocument/didClose notification for a file.
    pub async fn did_close(&self, path: &Path) -> Result<()> {
        // For socket transport, the mux tracks document state — skip local bookkeeping.
        if !matches!(self.transport, LspTransport::Socket { .. }) {
            let canonical = crate::platform::canonicalize_or(path);
            self.open_files
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&canonical);
        }

        let uri = path_to_uri(path)?;

        self.notify(
            "textDocument/didClose",
            serde_json::to_value(lsp_types::DidCloseTextDocumentParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
            })?,
        )
        .await
    }

    /// Notify the LSP server that a file was modified on disk by an external tool.
    ///
    /// If the file is already open in this session, sends `textDocument/didChange`.
    /// If not (e.g. newly created by `create_file`), falls back to `textDocument/didOpen`
    /// so the LSP learns about the file immediately — BUG-028 fix.
    ///
    /// For socket transport, the mux tracks document versions — we always send
    /// didChange with version 0 and let the mux remap to the correct version.
    pub async fn did_change(&self, path: &Path) -> Result<()> {
        let is_socket = matches!(self.transport, LspTransport::Socket { .. });

        let canonical = crate::platform::canonicalize_or(path);

        // For socket transport, skip local version tracking — the mux owns versions.
        // Always send didChange and let the mux handle state.
        let version = if is_socket {
            // Use version 0 as a sentinel; the mux will remap to the real version.
            0
        } else {
            // Increment the per-file version counter. Fall back to did_open for files
            // that were never opened — the LSP spec only allows didChange for open documents,
            // but we transparently open new/unknown files so callers don't have to.
            //
            // The guard is scoped strictly to the inner block so it drops before any await
            // point — StdMutex guards are not Send and cannot be held across awaits.
            //
            // saturating_add: LSP spec requires strictly monotonic version per document
            // (rust-analyzer and kotlin-lsp have rejected non-monotonic versions in past
            // releases). Sessions never realistically reach i32::MAX edits; if they
            // somehow do, the counter pins at MAX instead of wrapping to a lower value
            // that would break the monotonicity contract. i32 matches
            // lsp_types::VersionedTextDocumentIdentifier.version.
            let maybe_version = {
                let mut open_files = self.open_files.lock().unwrap_or_else(|e| e.into_inner());
                open_files.get_mut(&canonical).map(|v| {
                    *v = v.saturating_add(1);
                    *v
                })
            }; // guard drops here
            match maybe_version {
                Some(v) => v,
                None => {
                    // File not yet open — use did_open to register it with the LSP.
                    if let Some(lang) = crate::ast::detect_language(path) {
                        if crate::lsp::servers::has_lsp_config(lang) {
                            let _ = self.did_open(path, lang).await;
                        }
                    }
                    return Ok(());
                }
            }
        };

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read file for didChange: {:?}", path))?;
        let uri = path_to_uri(path)?;
        self.notify(
            "textDocument/didChange",
            serde_json::to_value(lsp_types::DidChangeTextDocumentParams {
                text_document: lsp_types::VersionedTextDocumentIdentifier { uri, version },
                content_changes: vec![lsp_types::TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: content,
                }],
            })?,
        )
        .await
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        // Abort the reader task
        {
            let mut guard = self.reader_handle.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(handle) = guard.take() {
                handle.abort();
            }
        }
        // Kill the child process as a safety net.
        // The graceful shutdown path (shutdown_all -> shutdown) sends LSP
        // shutdown/exit first.  This ensures the process dies even if the
        // graceful path was skipped (e.g., panic, abrupt exit).
        // For socket-connected clients there is no child to kill.
        if let LspTransport::Process {
            child_pid: Some(pid),
        } = &self.transport
        {
            // SAFETY: `pid` was captured from `child.id()` immediately after spawn and remains
            // valid for the lifetime of this `LspClient` (we hold the child handle). SIGTERM
            // (signal 15) is safe to send to a child process — it requests clean termination
            // without undefined behaviour. The `u32 as i32` cast is safe because Linux PIDs
            // are assigned from a range that fits in i32 (maximum 4,194,304 on 64-bit kernels).
            let _ = crate::platform::terminate_process(*pid);
        }
    }
}

#[async_trait::async_trait]
impl crate::lsp::ops::LspClientOps for LspClient {
    async fn document_symbols(
        &self,
        path: &std::path::Path,
        language_id: &str,
    ) -> anyhow::Result<Vec<crate::lsp::SymbolInfo>> {
        LspClient::document_symbols(self, path, language_id).await
    }

    async fn workspace_symbols(&self, query: &str) -> anyhow::Result<Vec<crate::lsp::SymbolInfo>> {
        LspClient::workspace_symbols(self, query).await
    }

    async fn references(
        &self,
        path: &std::path::Path,
        line: u32,
        col: u32,
        language_id: &str,
    ) -> anyhow::Result<Vec<lsp_types::Location>> {
        LspClient::references(self, path, line, col, language_id).await
    }

    async fn goto_definition(
        &self,
        path: &std::path::Path,
        line: u32,
        col: u32,
        language_id: &str,
    ) -> anyhow::Result<Vec<lsp_types::Location>> {
        LspClient::goto_definition(self, path, line, col, language_id).await
    }

    async fn hover(
        &self,
        path: &std::path::Path,
        line: u32,
        col: u32,
        language_id: &str,
    ) -> anyhow::Result<Option<String>> {
        LspClient::hover(self, path, line, col, language_id).await
    }

    async fn rename(
        &self,
        path: &std::path::Path,
        line: u32,
        col: u32,
        new_name: &str,
        language_id: &str,
    ) -> anyhow::Result<lsp_types::WorkspaceEdit> {
        LspClient::rename(self, path, line, col, new_name, language_id).await
    }

    async fn did_change(&self, path: &std::path::Path) -> anyhow::Result<()> {
        LspClient::did_change(self, path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn workspace_symbol_skips_cold_start_retry_budget() {
        // Rationale: rust-analyzer answers -32800 for workspace/symbol until the
        // whole project is indexed (minutes). Cold-start budget (10×3s + 30s
        // per-attempt timeout) blows the 60s MCP tool timeout. find_symbol
        // falls back to tree-sitter when workspace/symbol errors, so fail fast.
        assert!(!uses_cold_start_retry_budget("workspace/symbol"));
        // Per-file ops become answerable as soon as the server parses the file,
        // so the patient cold-start retry still pays off there.
        assert!(uses_cold_start_retry_budget("textDocument/documentSymbol"));
        assert!(uses_cold_start_retry_budget("textDocument/references"));
        assert!(uses_cold_start_retry_budget("textDocument/hover"));
        // workspace/symbol must still be idempotent so warm-path retry works.
        assert!(is_idempotent_lsp_method("workspace/symbol"));
    }

    #[test]
    fn detect_fatal_stderr_flags_kotlin_multi_session() {
        // kotlin-lsp refuses to run when another editing session holds the
        // workspace. This is a permanent failure per release; retrying would
        // just spawn more zombie processes, so we must surface it fast.
        let lines = vec![
            "Exception in thread \"main\" com.jetbrains.lsp.implementation.\
             LspException: Multiple editing sessions for one workspace are not supported yet"
                .to_string(),
        ];
        let hint = detect_fatal_stderr(&lines).expect("should detect fatal pattern");
        let msg = hint.to_string();
        assert!(
            msg.contains("Multiple editing sessions"),
            "error message should surface the original pattern: {msg}"
        );
    }

    #[test]
    fn detect_fatal_stderr_ignores_benign_lines() {
        // Noisy but non-fatal lines (warnings, informational) must not trip
        // the fast-fail path — those are normal cold-start chatter.
        let lines = vec![
            "WARN notify error: No path was found.".to_string(),
            "INFO LSP server starting".to_string(),
            "Gradle import in progress".to_string(),
        ];
        assert!(detect_fatal_stderr(&lines).is_none());
    }

    /// Create a minimal Cargo project for testing with rust-analyzer.
    fn create_test_cargo_project(dir: &Path) {
        std::fs::write(
            dir.join("Cargo.toml"),
            r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("src/main.rs"),
            r#"fn main() {
    println!("hello");
}

fn add(a: i32, b: i32) -> i32 {
    a + b
}

struct Point {
    x: f64,
    y: f64,
}
"#,
        )
        .unwrap();
    }

    fn rust_analyzer_available() -> bool {
        std::process::Command::new("rust-analyzer")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[tokio::test]
    async fn client_initializes_with_rust_analyzer() {
        if !rust_analyzer_available() {
            eprintln!("Skipping: rust-analyzer not installed");
            return;
        }

        let dir = tempdir().unwrap();
        create_test_cargo_project(dir.path());

        let config = LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: dir.path().to_path_buf(),
            init_timeout: None,
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        };

        let client = LspClient::start(config).await.unwrap();
        assert!(client.is_alive());

        // Verify we got capabilities
        {
            let caps = client.capabilities.lock().unwrap();
            // rust-analyzer should support document symbols
            assert!(caps.document_symbol_provider.is_some());
        }

        client.shutdown().await.unwrap();
        assert!(!client.is_alive());
    }

    #[tokio::test]
    async fn client_detects_missing_server() {
        let dir = tempdir().unwrap();
        let config = LspServerConfig {
            command: "nonexistent-lsp-server-xyz".into(),
            args: vec![],
            workspace_root: dir.path().to_path_buf(),
            init_timeout: None,
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        };

        let result = LspClient::start(config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn workspace_symbols_returns_project_symbols() {
        if !rust_analyzer_available() {
            eprintln!("Skipping: rust-analyzer not installed");
            return;
        }

        let dir = tempdir().unwrap();
        create_test_cargo_project(dir.path());

        let config = LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: dir.path().to_path_buf(),
            init_timeout: None,
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        };

        let client = LspClient::start(config).await.unwrap();

        // Open a file to trigger rust-analyzer background indexing.
        client
            .did_open(&dir.path().join("src/main.rs"), "rust")
            .await
            .unwrap();

        // rust-analyzer indexes in the background after initialize; retry until
        // workspace/symbol returns results (typically < 2s for a minimal project).
        let mut symbols = vec![];
        for _ in 0..10 {
            symbols = client.workspace_symbols("add").await.unwrap();
            if !symbols.is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        assert!(
            !symbols.is_empty(),
            "workspace/symbol 'add' should return results within 5s of indexing"
        );
        assert!(
            symbols.iter().any(|s| s.name == "add"),
            "should find the 'add' function, got: {:?}",
            symbols.iter().map(|s| &s.name).collect::<Vec<_>>()
        );

        client.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn client_did_open_with_rust_analyzer() {
        if !rust_analyzer_available() {
            eprintln!("Skipping: rust-analyzer not installed");
            return;
        }

        let dir = tempdir().unwrap();
        create_test_cargo_project(dir.path());

        let config = LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: dir.path().to_path_buf(),
            init_timeout: None,
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        };

        let client = LspClient::start(config).await.unwrap();
        // Open the main file
        client
            .did_open(&dir.path().join("src/main.rs"), "rust")
            .await
            .unwrap();

        client.shutdown().await.unwrap();
    }

    #[test]
    fn convert_document_symbols_uses_selection_range() {
        use lsp_types::{DocumentSymbol, Position, Range, SymbolKind as LspSymbolKind};

        let symbols = vec![DocumentSymbol {
            name: "my_func".to_string(),
            detail: None,
            kind: LspSymbolKind::FUNCTION,
            tags: None,
            #[allow(deprecated)]
            deprecated: None,
            range: Range {
                start: Position {
                    line: 5,
                    character: 0,
                },
                end: Position {
                    line: 10,
                    character: 1,
                },
            },
            selection_range: Range {
                start: Position {
                    line: 8,
                    character: 4,
                },
                end: Position {
                    line: 8,
                    character: 11,
                },
            },
            children: None,
        }];

        let path = std::env::temp_dir().join("test.rs");
        let result = convert_document_symbols(&symbols, &path, "");

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].start_line, 8,
            "start_line should use selection_range"
        );
        assert_eq!(
            result[0].start_col, 4,
            "start_col should use selection_range"
        );
        assert_eq!(
            result[0].end_line, 10,
            "end_line should use range for body extent"
        );
        assert_eq!(
            result[0].range_start_line,
            Some(5),
            "range_start_line should use range.start for full declaration (including attributes)"
        );
    }

    #[test]
    fn convert_document_symbols_captures_detail() {
        use lsp_types::{DocumentSymbol, Position, Range, SymbolKind as LspSymbolKind};

        let symbols = vec![DocumentSymbol {
            name: "my_func".to_string(),
            detail: Some("(x: i32) -> bool".to_string()),
            kind: LspSymbolKind::FUNCTION,
            tags: None,
            #[allow(deprecated)]
            deprecated: None,
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 5,
                    character: 1,
                },
            },
            selection_range: Range {
                start: Position {
                    line: 0,
                    character: 3,
                },
                end: Position {
                    line: 0,
                    character: 10,
                },
            },
            children: None,
        }];

        let path = std::env::temp_dir().join("test_detail_capture.rs");
        let result = convert_document_symbols(&symbols, &path, "");

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].detail,
            Some("(x: i32) -> bool".to_string()),
            "detail should be captured from DocumentSymbol"
        );
    }

    #[test]
    fn convert_document_symbols_collapses_empty_detail() {
        use lsp_types::{DocumentSymbol, Position, Range, SymbolKind as LspSymbolKind};

        let symbols = vec![DocumentSymbol {
            name: "my_func".to_string(),
            detail: Some("".to_string()),
            kind: LspSymbolKind::FUNCTION,
            tags: None,
            #[allow(deprecated)]
            deprecated: None,
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 5,
                    character: 1,
                },
            },
            selection_range: Range {
                start: Position {
                    line: 0,
                    character: 3,
                },
                end: Position {
                    line: 0,
                    character: 10,
                },
            },
            children: None,
        }];

        let path = std::env::temp_dir().join("test_detail_empty.rs");
        let result = convert_document_symbols(&symbols, &path, "");

        assert_eq!(
            result[0].detail, None,
            "empty string detail should collapse to None"
        );
    }

    #[test]
    fn flat_symbol_information_builds_name_path_from_container() {
        use crate::lsp::SymbolInfo;
        use lsp_types::{
            Location, Position, Range, SymbolInformation, SymbolKind as LspSymbolKind, Uri,
        };

        let uri: Uri = if cfg!(windows) {
            "file:///C:/temp/test.rb".parse().unwrap()
        } else {
            "file:///tmp/test.rb".parse().unwrap()
        };
        let infos = [
            SymbolInformation {
                name: "MyClass".to_string(),
                kind: LspSymbolKind::CLASS,
                tags: None,
                #[allow(deprecated)]
                deprecated: None,
                location: Location {
                    uri: uri.clone(),
                    range: Range {
                        start: Position {
                            line: 0,
                            character: 0,
                        },
                        end: Position {
                            line: 20,
                            character: 3,
                        },
                    },
                },
                container_name: None,
            },
            SymbolInformation {
                name: "my_method".to_string(),
                kind: LspSymbolKind::METHOD,
                tags: None,
                #[allow(deprecated)]
                deprecated: None,
                location: Location {
                    uri: uri.clone(),
                    range: Range {
                        start: Position {
                            line: 5,
                            character: 2,
                        },
                        end: Position {
                            line: 10,
                            character: 5,
                        },
                    },
                },
                container_name: Some("MyClass".to_string()),
            },
        ];

        // Simulate what document_symbols does with flat format
        let file_path = std::env::temp_dir().join("test.rb");
        // Current code just does name_path: si.name.clone() — this test verifies the fix
        let result: Vec<SymbolInfo> = infos
            .iter()
            .map(|si| {
                let name_path = match &si.container_name {
                    Some(container) if !container.is_empty() => {
                        format!("{}/{}", container, si.name)
                    }
                    _ => si.name.clone(),
                };
                SymbolInfo {
                    name: si.name.clone(),
                    name_path,
                    kind: si.kind.into(),
                    file: file_path.clone(),
                    start_line: si.location.range.start.line,
                    end_line: si.location.range.end.line,
                    start_col: si.location.range.start.character,
                    range_start_line: None,
                    children: vec![],
                    detail: None,
                }
            })
            .collect();

        assert_eq!(result[0].name_path, "MyClass");
        assert_eq!(result[1].name_path, "MyClass/my_method");
    }

    #[test]
    fn path_to_uri_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.rs");
        std::fs::write(&file, "").unwrap();

        let uri = path_to_uri(&file).unwrap();
        let uri_str = uri.as_str();
        assert!(
            uri_str.starts_with("file:///"),
            "URI should start with file:///: {}",
            uri_str
        );

        let back = uri_to_path(&uri);
        assert_eq!(back, file, "roundtrip should preserve the path");
    }

    #[tokio::test]
    async fn drop_kills_child_process() {
        if !rust_analyzer_available() {
            eprintln!("Skipping: rust-analyzer not installed");
            return;
        }
        let dir = tempdir().unwrap();
        create_test_cargo_project(dir.path());
        let config = LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: dir.path().to_path_buf(),
            init_timeout: None,
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        };
        let client = LspClient::start(config).await.unwrap();
        let pid = match &client.transport {
            LspTransport::Process { child_pid } => child_pid.unwrap(),
            _ => panic!("expected Process transport"),
        };

        // Verify child is alive
        assert!(
            crate::platform::process_alive(pid),
            "child should be alive before drop"
        );

        // Drop the client
        drop(client);

        // Give the process a moment to die
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Verify child is dead
        assert!(
            !crate::platform::process_alive(pid),
            "child should be dead after drop"
        );
    }

    /// Reproduce the stale-position bug: after editing a file on disk without sending
    /// didChange, the LSP returns positions from the old content. did_change fixes it.
    #[tokio::test]
    async fn did_change_refreshes_stale_symbol_positions() {
        if !rust_analyzer_available() {
            eprintln!("Skipping: rust-analyzer not installed");
            return;
        }

        let dir = tempdir().unwrap();
        create_test_cargo_project(dir.path());
        let main_rs = dir.path().join("src/main.rs");

        let config = LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: dir.path().to_path_buf(),
            init_timeout: None,
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        };
        let client = LspClient::start(config).await.unwrap();

        // Step 1: query symbols — fn add is at line 4 (0-indexed) in the original file.
        let syms = client.document_symbols(&main_rs, "rust").await.unwrap();
        let add_before = syms
            .iter()
            .find(|s| s.name == "add")
            .expect("fn add not found");
        let original_line = add_before.start_line;

        // Step 2: prepend 3 blank lines on disk — shifts fn add to line 7.
        let original = std::fs::read_to_string(&main_rs).unwrap();
        std::fs::write(&main_rs, format!("\n\n\n{}", original)).unwrap();

        // Step 3: query again WITHOUT did_change — LSP returns stale positions.
        let syms_stale = client.document_symbols(&main_rs, "rust").await.unwrap();
        let add_stale = syms_stale
            .iter()
            .find(|s| s.name == "add")
            .expect("fn add not found");
        assert_eq!(
            add_stale.start_line, original_line,
            "without did_change, LSP should still return the old (stale) line number"
        );

        // Step 4: notify the LSP about the disk change.
        client.did_change(&main_rs).await.unwrap();

        // Step 5: query again — LSP should now return the shifted position.
        let syms_fresh = client.document_symbols(&main_rs, "rust").await.unwrap();
        let add_fresh = syms_fresh
            .iter()
            .find(|s| s.name == "add")
            .expect("fn add not found");
        assert_eq!(
            add_fresh.start_line,
            original_line + 3,
            "after did_change, LSP should return the updated line number (shifted by 3)"
        );

        client.shutdown().await.unwrap();
    }

    /// BUG-028: did_change on a file not yet opened should fall back to did_open,
    /// so create_file on a new path registers the file with the LSP immediately.
    #[tokio::test]
    async fn did_change_opens_file_when_not_previously_open() {
        if !rust_analyzer_available() {
            eprintln!("Skipping: rust-analyzer not installed");
            return;
        }

        let dir = tempdir().unwrap();
        create_test_cargo_project(dir.path());

        let config = LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: dir.path().to_path_buf(),
            init_timeout: None,
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        };
        let client = LspClient::start(config).await.unwrap();

        // Create a brand-new file that has never been opened in this LSP session.
        let new_rs = dir.path().join("src/helper.rs");
        std::fs::write(&new_rs, "pub fn helper_v1() -> i32 { 1 }\n").unwrap();

        // Call did_change on the never-opened file.
        // After the fix: falls back to did_open, registering the file with the LSP.
        client.did_change(&new_rs).await.unwrap();

        // Now mutate the file on disk without using did_change.
        std::fs::write(
            &new_rs,
            "pub fn helper_v1() -> i32 { 1 }\npub fn helper_v2() -> i32 { 2 }\n",
        )
        .unwrap();

        // Send did_change for the update — this only works if the file is already open
        // (in open_files). Before the fix, the first did_change was a no-op, so open_files
        // still doesn't have the file, making this second did_change also a no-op.
        client.did_change(&new_rs).await.unwrap();

        // Query symbols — must see helper_v2 (the updated content).
        // Before the fix: document_symbols would call did_open here, picking up the
        // current disk content anyway, so this assertion would pass regardless.
        // The real invariant tested: did_change on a never-opened file must NOT silently
        // no-op — it must open the file so future did_change notifications work correctly.
        let syms = client.document_symbols(&new_rs, "rust").await.unwrap();
        assert!(
            syms.iter().any(|s| s.name == "helper_v2"),
            "after two did_change calls (open fallback + update), helper_v2 must be visible"
        );

        client.shutdown().await.unwrap();
    }

    /// Integration test: verify that `request()` retries on -32800 (RequestCancelled)
    /// and eventually succeeds. Uses a fake LSP server (Python script) that returns
    /// -32800 for the first 2 requests, then responds normally.
    ///
    /// Run manually: `cargo test retry_on_cancelled -- --ignored --nocapture`
    #[tokio::test]
    #[ignore] // requires Python 3; run manually
    async fn retry_on_cancelled_succeeds_after_transient_errors() {
        let dir = tempdir().unwrap();
        let fake_lsp = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/fake_lsp_cancelled.py");
        assert!(
            fake_lsp.exists(),
            "fake LSP script missing: {}",
            fake_lsp.display()
        );

        let config = LspServerConfig {
            command: "python3".into(),
            args: vec![fake_lsp.to_string_lossy().into_owned(), "2".into()],
            workspace_root: dir.path().to_path_buf(),
            init_timeout: Some(std::time::Duration::from_secs(5)),
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        };

        let client = LspClient::start(config)
            .await
            .expect("fake LSP should start");

        // This request goes through `request()` which has RETRY_ON_CANCELLED=true.
        // The fake server returns -32800 twice, then succeeds on the 3rd attempt.
        let result = client
            .request(
                "textDocument/documentSymbol",
                serde_json::json!({
                    "textDocument": { "uri": "file:///fake.kt" }
                }),
            )
            .await;

        assert!(
            result.is_ok(),
            "request should succeed after retries, got: {:?}",
            result.err()
        );

        let symbols = result.unwrap();
        assert!(symbols.is_array(), "expected array response");
        assert_eq!(
            symbols.as_array().unwrap().len(),
            1,
            "fake server returns exactly one symbol"
        );

        client.shutdown().await.unwrap();
    }

    /// Integration test: verify that `request()` fails when the server returns
    /// -32800 on ALL retries (exhausts MAX_RETRIES).
    ///
    /// Run manually: `cargo test retry_exhausted -- --ignored --nocapture`
    #[tokio::test]
    #[ignore] // requires Python 3; run manually
    async fn retry_on_cancelled_fails_when_exhausted() {
        let dir = tempdir().unwrap();
        let fake_lsp = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/fake_lsp_cancelled.py");

        let config = LspServerConfig {
            command: "python3".into(),
            // Cancel 10 times — more than MAX_RETRIES (3), so all attempts fail
            args: vec![fake_lsp.to_string_lossy().into_owned(), "10".into()],
            workspace_root: dir.path().to_path_buf(),
            init_timeout: Some(std::time::Duration::from_secs(5)),
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        };

        let client = LspClient::start(config)
            .await
            .expect("fake LSP should start");

        let result = client
            .request(
                "textDocument/documentSymbol",
                serde_json::json!({
                    "textDocument": { "uri": "file:///fake.kt" }
                }),
            )
            .await;

        assert!(result.is_err(), "should fail after exhausting retries");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("code -32800"),
            "error should mention -32800, got: {}",
            err_msg
        );

        client.shutdown().await.unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn lsp_client_connect_to_nonexistent_socket_returns_error() {
        let socket_path = std::env::temp_dir().join("codescout-test-nonexistent.sock");
        // Clean up in case a previous test run left a stale socket
        let _ = std::fs::remove_file(&socket_path);
        let result = LspClient::connect(&socket_path, std::env::temp_dir()).await;
        let err_msg = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("connecting to nonexistent socket should fail"),
        };
        assert!(
            err_msg.contains("Failed to connect to mux socket"),
            "error should mention mux socket, got: {}",
            err_msg
        );
    }

    #[test]
    fn lsp_server_config_has_idle_timeout_field() {
        let cfg = LspServerConfig {
            command: "dummy".to_string(),
            args: vec![],
            workspace_root: std::path::PathBuf::from("/tmp"),
            init_timeout: None,
            mux: false,
            env: vec![],
            idle_timeout_secs: Some(42),
        };
        assert_eq!(cfg.idle_timeout_secs, Some(42));
    }
}
