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
// cap-class: NOT_A_CAP — ring-buffer capacity for server stderr used to detect fatal init conditions; diagnostic state, never part of a tool result
const MAX_STDERR_LINES: usize = 200;

#[cfg(unix)]
/// Longest [`LspClient::connect`] waits for a mux to send its init message after accepting the
/// connection. A mux writes it as soon as it accepts, so a socket still silent after this long is
/// accepting but not serving; the caller of [`LspClient::connect_with_init_timeout`] may pick a
/// shorter bound.
// cap-class: NOT_A_CAP — bound on a wait for a peer's first message; expiry surfaces as a connect error, never as a shorter answer
pub(crate) const MUX_INIT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
#[cfg(unix)]
/// The error of a mux that accepted a connection and sent no init message within `after`. A type
/// of its own so a caller (and a test) can read the bound that was applied instead of parsing it
/// out of the message.
#[derive(Debug)]
pub(crate) struct MuxInitTimeout {
    pub(crate) socket_path: std::path::PathBuf,
    pub(crate) after: std::time::Duration,
}

#[cfg(unix)]
impl std::fmt::Display for MuxInitTimeout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "mux accepted the connection at {:?} but sent no init message within {:?}",
            self.socket_path, self.after
        )
    }
}

#[cfg(unix)]
impl std::error::Error for MuxInitTimeout {}

use super::call_hierarchy::supports_call_hierarchy;
use super::progress::{keep_waiting, ProgressTracker};

/// Convert an LSP URI to a filesystem path.
///
/// Delegates to [`crate::util::file_address::FileAddress::from_lsp_uri`] for the
/// canonical conversion. Falls back to `PathBuf::from(uri.path())` when neither
/// the URI parse nor the raw-path component yields a value (empty URI), to
/// preserve the original infallible signature.
fn uri_to_path(uri: &lsp_types::Uri) -> PathBuf {
    crate::util::file_address::FileAddress::from_lsp_uri(uri)
        .map(crate::util::file_address::FileAddress::into_path)
        .unwrap_or_else(|| PathBuf::from(uri.path().as_str()))
}

/// Decode the answer to `textDocument/rename`. The protocol type is `Option<WorkspaceEdit>`,
/// so `null` is a legal answer: the server has no edit to give yet. It becomes a
/// `RecoverableError`, so sibling tool calls survive and the caller knows to ask again.
fn decode_rename_answer(answer: Value) -> Result<lsp_types::WorkspaceEdit> {
    if answer.is_null() {
        return Err(RecoverableError::with_hint(
            "the language server answered textDocument/rename with null: it returned no edit",
            "The server may still be working through a recent change. Ask again in a moment. \
             If it keeps answering null, check that the position is on the symbol to rename.",
        )
        .into());
    }
    Ok(serde_json::from_value(answer)?)
}

/// Convert a filesystem path to an LSP `file://` URI.
///
/// Delegates to [`crate::util::file_address::FileAddress::as_lsp_uri`].
fn path_to_uri(path: &Path) -> Result<lsp_types::Uri> {
    crate::util::file_address::FileAddress::from_path(path).as_lsp_uri()
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
            | "textDocument/prepareCallHierarchy"
            | "callHierarchy/incomingCalls"
            | "callHierarchy/outgoingCalls"
            | "workspace/symbol"
            | "initialize"
    )
}

/// Transient LSP error codes that warrant an automatic retry.
///
/// - `-32800` `RequestCancelled` — server cancelled mid-request (typically
///   workspace lock contention or cold-start indexing).
/// - `-32801` `ContentModified` — server's analysis snapshot advanced
///   between request issue and response (typical during indexer warmup
///   right after `/mcp` reconnect; rust-analyzer publishes new diagnostics
///   and cancels any in-flight requests against the stale snapshot).
///
/// Both are explicitly "retry on the new snapshot" signals per the LSP
/// spec. We treat them identically — same backoff, same idempotency guard.
pub(crate) fn is_retryable_lsp_error(err: &anyhow::Error) -> bool {
    let s = err.to_string();
    s.contains("code -32800") || s.contains("code -32801")
}

/// Return true if cold-start's extended retry budget makes sense for `method`.
///
/// Most LSP queries that return `-32800 RequestCancelled` during cold start
/// become answerable per-file as the server parses each file, so the cold
/// retries (delays of 3 s x N, 165 s in all on Linux, `request`) catch them. But `workspace/symbol` stays unanswerable until
/// the *whole* project is indexed (minutes for large Rust/Kotlin projects),
/// and that retry budget, with a 30 s timeout per attempt, blows through the MCP
/// 60s tool timeout. For that method we keep the short warm budget and let
/// callers (e.g. `symbols`) fail over to tree-sitter quickly.
fn uses_cold_start_retry_budget(method: &str) -> bool {
    !matches!(method, "workspace/symbol")
}

/// Cold-start retry count for LSP requests, scaled by host OS.
///
/// rust-analyzer and the Kotlin LSP cold-start are documented in the codescout
/// `gotchas` memory as 2–3× slower on Windows than Linux; macOS sits in between
/// under rosetta/ARM heterogeneity. The platform variance is largest at the
/// tail (slow startups), not the median, so scaling the retry **count** keeps
/// first-attempt latency identical on every platform — we only add more
/// attempts before giving up.
///
/// The retry delay (`RETRY_DELAY_COLD_MS`) and cold-start window
/// (`COLD_START_WINDOW`) deliberately stay platform-uniform; only the retry
/// count varies. `cfg!` is const-stable, so the body resolves at compile time
/// and the caller still treats the result as a plain `const usize`.
const fn cold_start_max_retries() -> usize {
    if cfg!(target_os = "windows") {
        20
    } else if cfg!(target_os = "macos") {
        15
    } else {
        10
    }
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
        if line.contains("Unknown binary 'rust-analyzer'") {
            return Some(RecoverableError::with_hint(
                "rust-analyzer is unreachable. The rustup shim is on PATH but the \
                 component is not installed: \
                 \"error: Unknown binary 'rust-analyzer' in official toolchain\"",
                "Run `rustup component add rust-analyzer` to install it for the active \
                 toolchain, or install rust-analyzer outside rustup \
                 (https://rust-analyzer.github.io). Rust LSP tools (edit_code, symbols, \
                 references, call_graph on .rs files) will keep returning this error \
                 until rust-analyzer launches successfully.",
            ));
        }
    }
    None
}

/// `(len, mtime)` — the cheap pair used to notice a file changing behind our back.
pub(crate) type DiskSignature = (u64, Option<std::time::SystemTime>);

/// Read a file's [`DiskSignature`], or `None` if it cannot be stat'd.
///
/// Both fields, deliberately. `len` alone misses a same-size change — exactly what
/// reverting one character with `git checkout` produces. `mtime` alone has
/// filesystem-dependent granularity.
///
/// **The pair narrows the blind spot; it does not close it.** A same-size change
/// landing inside one mtime tick still reads as unchanged. That residual is
/// accepted: it requires a filesystem with coarse mtime AND a byte-identical-length
/// edit AND sub-tick timing, and its cost is one stale answer — which is the
/// behaviour without any of this. A content hash would close it and would mean
/// hashing every file on every navigation call.
///
/// So this is a staleness *hint*: a false negative costs one stale answer, a false
/// positive costs one redundant `didChange`. The asymmetry is why the cheap check
/// is the right one.
pub(crate) fn disk_signature(path: &Path) -> Option<DiskSignature> {
    let md = std::fs::metadata(path).ok()?;
    Some((md.len(), md.modified().ok()))
}

/// What was last delivered for one file: the disk signature of those bytes, and the path
/// the server knows the file by.
#[derive(Debug, Clone)]
struct Delivered {
    signature: DiskSignature,
    /// The path as it was SENT, not canonicalised. The server addresses a document by the URI
    /// of that path, so a resync must send the same one. Through a symlink the canonical path
    /// names a document the server never opened (on macOS `tempdir()` lives under `/var`, a
    /// link to `/private/var`).
    sent_as: PathBuf,
}

/// Per-file record of the on-disk signature last **delivered** to the server.
///
/// Its own type rather than a bare map because the `overwrite` rule is the whole
/// correctness argument and belongs next to the data it governs — see [`Self::record`].
#[derive(Debug, Default)]
pub(crate) struct SyncedSignatures(StdMutex<HashMap<PathBuf, Delivered>>);

impl SyncedSignatures {
    /// Note that `canonical`'s current bytes were handed to the server.
    ///
    /// `overwrite: false` is for `didOpen`, which does **not** reliably deliver for a
    /// file the server already has: on stdio it early-returns, and on socket the mux
    /// dedups it. Recording there would replace a true "last delivered" with a merely
    /// *current* disk state, and [`Self::has_drifted`] would then compare the file
    /// against itself and never fire.
    ///
    /// That is not hypothetical. It shipped in `eedb308c` and made the external-write
    /// fix **inert on the mux path** — the one this project uses for Rust — while its
    /// stdio unit test passed, because the early return hides the difference. Live
    /// verification caught it.
    ///
    /// `overwrite: true` is for `didChange`, which always sends.
    fn record(&self, canonical: &Path, sent_as: &Path, overwrite: bool) {
        let Some(sig) = disk_signature(canonical) else {
            return;
        };
        let delivered = Delivered {
            signature: sig,
            sent_as: sent_as.to_path_buf(),
        };
        let mut map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        match map.entry(canonical.to_path_buf()) {
            std::collections::hash_map::Entry::Occupied(mut e) => {
                if overwrite {
                    e.insert(delivered);
                }
            }
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(delivered);
            }
        }
    }

    /// Whether `canonical` differs on disk from what was last delivered.
    ///
    /// `false` for a file never delivered — that is `didOpen`'s business, not drift.
    fn has_drifted(&self, canonical: &Path) -> bool {
        let Some(current) = disk_signature(canonical) else {
            return false;
        };
        let map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        map.get(canonical)
            .is_some_and(|last| last.signature != current)
    }

    /// Every recorded file that differs on disk from what was last delivered, as the path it
    /// was SENT under (see [`Delivered::sent_as`]).
    ///
    /// One `stat` per recorded file. The record is taken under the lock and the `stat`s run
    /// after it is released, so a slow filesystem never holds up `record`.
    fn drifted(&self) -> Vec<PathBuf> {
        let recorded: Vec<(PathBuf, PathBuf)> = {
            let map = self.0.lock().unwrap_or_else(|e| e.into_inner());
            map.iter()
                .map(|(canonical, d)| (canonical.clone(), d.sent_as.clone()))
                .collect()
        };
        recorded
            .into_iter()
            .filter(|(canonical, _)| self.has_drifted(canonical))
            .map(|(_, sent_as)| sent_as)
            .collect()
    }

    #[cfg(test)]
    fn get(&self, canonical: &Path) -> Option<DiskSignature> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(canonical)
            .map(|d| d.signature)
    }
}

/// Convert hierarchical `DocumentSymbol[]` into our `SymbolInfo` tree.
pub(crate) fn convert_document_symbols(
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

/// pyright reports an assignment's range as its name alone, so a multi-line Python
/// constant arrives one line long — and `symbols` bodies and `edit_code` ranges are
/// both derived from it. Widen those ranges to the whole statement, here where the
/// symbols enter codescout, so every consumer agrees on the correct span.
/// BUG docs/issues/archive/2026-09-24-python-multiline-constant-range-is-its-first-line.md
fn with_python_ranges(
    mut symbols: Vec<super::SymbolInfo>,
    language_id: &str,
    path: &Path,
) -> Vec<super::SymbolInfo> {
    if language_id == "python" {
        if let Ok(source) = std::fs::read_to_string(path) {
            crate::ast::python_ranges::extend_python_assignment_ranges(&mut symbols, &source);
        }
    }
    symbols
}

/// [`with_python_ranges`] for a flat `workspace/symbol` result spanning many files:
/// each Python file holding a variable-like symbol is read and parsed once, and only
/// that file's symbols are matched against it. Order is preserved — it is the ranking.
fn with_python_ranges_by_file(mut symbols: Vec<super::SymbolInfo>) -> Vec<super::SymbolInfo> {
    use super::symbols::SymbolKind;
    let mut files: Vec<PathBuf> = symbols
        .iter()
        .filter(|s| matches!(s.kind, SymbolKind::Constant | SymbolKind::Variable))
        .filter(|s| crate::ast::detect_language(&s.file) == Some("python"))
        .map(|s| s.file.clone())
        .collect();
    files.sort();
    files.dedup();
    for file in files {
        let Ok(source) = std::fs::read_to_string(&file) else {
            continue;
        };
        crate::ast::python_ranges::extend_python_assignment_ranges(
            symbols.iter_mut().filter(|s| s.file == file),
            &source,
        );
    }
    symbols
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

/// Owns one in-flight request's bookkeeping for as long as its future is alive. If the future is
/// dropped before a response arrives (an outer `tokio::time::timeout` such as the per-language
/// budget in `search_project_symbols`, or a cancelled tool call), the pending entry is removed
/// and, when the request had reached the wire, the server is sent `$/cancelRequest` so it stops
/// computing an answer nobody awaits. The inner timeout path disarms it and sends its own cancel.
struct InFlightRequest {
    id: i64,
    pending: PendingRequests,
    writer: Arc<Mutex<Box<dyn AsyncWrite + Unpin + Send>>>,
    /// The request message was fully written, so the server may be working on it.
    sent: bool,
    armed: bool,
}

impl Drop for InFlightRequest {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.id);
        if !self.sent {
            return;
        }
        // `Drop` cannot await: hand the write to the runtime. No runtime (shutdown) means
        // nobody is left to cancel for.
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let writer = self.writer.clone();
        let id = self.id;
        handle.spawn(async move {
            let msg = json!({
                "jsonrpc": "2.0",
                "method": "$/cancelRequest",
                "params": { "id": id },
            });
            let mut writer = writer.lock().await;
            let _ = transport::write_message(&mut *writer, &msg).await;
        });
    }
}

/// How many `documentSymbol` answers in a row were `null` for each file on this client.
///
/// A `null` is a server that has not answered yet, and the hint says to retry. The same advice on
/// the tenth identical `null` for one file is not advice any more (FR-12: an agent retried one
/// `edit_code` insert ten times in half an hour, with `sleep 60`/`90`/`120` between tries). The
/// count lets the hint change once waiting has plainly not been enough. Cleared by any real
/// answer for the file, so it never grows past the files that are stuck right now.
#[derive(Default)]
pub(crate) struct NullRuns(StdMutex<HashMap<PathBuf, u32>>);

impl NullRuns {
    /// From this many nulls in a row, the hint names a different route.
    pub(crate) const REPEATED_AT: u32 = 3;

    /// Count one more `null` for `path` and return the run length including it.
    pub(crate) fn record_null(&self, path: &Path) -> u32 {
        let mut runs = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let n = runs.entry(path.to_path_buf()).or_insert(0);
        *n += 1;
        *n
    }

    /// The file answered: its run ends.
    pub(crate) fn clear(&self, path: &Path) {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(path);
    }

    /// The sentence to add to the hint for a run of `run` nulls, or `None` while a retry is
    /// still the right advice. Offered as a thing to check, not as a cause (ADR 2026-08-27,
    /// clause 3): the count says waiting has not helped, not why.
    pub(crate) fn repeat_advice(run: u32) -> Option<String> {
        (run >= Self::REPEATED_AT).then(|| {
            format!(
                " This is null answer {run} in a row for this file, so waiting has not been enough. \
                 To check whether the server is ready, run `symbols(path=...)` on a different \
                 file. For a plain text insert or replacement, `edit_file` does not need the server."
            )
        })
    }
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
    /// On-disk signature of the content last SENT to the server, per file.
    ///
    /// Separate from `open_files` (which tracks LSP document versions) because it
    /// answers a different question: not "does the server know this file" but "does
    /// the server have its current bytes". `did_open` returns early for an
    /// already-open file, so without this an external `git checkout` leaves the
    /// server answering from pre-checkout content indefinitely.
    synced_sigs: SyncedSignatures,
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
    /// The server's announced start-up work (`$/progress`), read off its notifications. It
    /// decides how long a `null` answer is re-asked in the cold-start window; see
    /// [`ProgressTracker`].
    progress: Arc<ProgressTracker>,
    /// Set once the `references` tool has finished its one cold-start wait on this client
    /// (settled or deadline hit), so later calls never wait again. Lives on the client because a
    /// client is one language server for one root: a restart builds a new client and a new wait.
    pub(crate) references_warmup_done: AtomicBool,
    /// Consecutive `null` `documentSymbol` answers per file; see [`NullRuns`].
    null_runs: NullRuns,
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
        progress: &ProgressTracker,
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
            progress.observe(&msg, std::time::Instant::now());
        }
    }

    /// Read messages from `reader` and dispatch them via `dispatch_lsp_message`
    /// until a transport error occurs. Returns the error so callers can run
    /// transport-specific cleanup before draining pending requests.
    async fn run_dispatch_loop<R>(
        mut reader: BufReader<R>,
        pending: PendingRequests,
        writer: Arc<Mutex<Box<dyn AsyncWrite + Unpin + Send>>>,
        progress: Arc<ProgressTracker>,
        request_label: &'static str,
        notif_label: &'static str,
    ) -> anyhow::Error
    where
        R: tokio::io::AsyncRead + Unpin + Send + 'static,
    {
        loop {
            match transport::read_message(&mut reader).await {
                Ok(msg) => {
                    Self::dispatch_lsp_message(
                        msg,
                        &pending,
                        &writer,
                        &progress,
                        request_label,
                        notif_label,
                    )
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
    pub async fn start(config: LspServerConfig) -> Result<Self> {
        const SPAWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

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
        // WIN-5: the spawn syscall is synchronous (CreateProcessW on Windows) and
        // can hang under EDR. Run it on a blocking thread, bounded. On timeout the
        // detached task keeps running; when the syscall eventually returns, the
        // Child is dropped and kill_on_drop reaps it.
        let command_name = config.command.clone();
        let mut child = match tokio::time::timeout(
            SPAWN_TIMEOUT,
            tokio::task::spawn_blocking(move || cmd.spawn()),
        )
        .await
        {
            Err(_elapsed) => anyhow::bail!(
                "LSP spawn for '{}' exceeded {}s — OS-level process creation hung \
                 (known cause: EDR interference on Windows; see WIN-5)",
                command_name,
                SPAWN_TIMEOUT.as_secs()
            ),
            Ok(Err(join_err)) => {
                return Err(anyhow::anyhow!("LSP spawn task panicked: {join_err}"))
            }
            Ok(Ok(spawn_res)) => spawn_res
                .with_context(|| format!("Failed to start LSP server: {}", command_name))?,
        };

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
        let progress = Arc::new(ProgressTracker::default());
        let progress_clone = progress.clone();
        let reader_handle = tokio::spawn(async move {
            let read_err = Self::run_dispatch_loop(
                BufReader::new(stdout),
                pending_clone.clone(),
                writer_clone,
                progress_clone,
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
            synced_sigs: SyncedSignatures::default(),
            stderr_lines,
            started_at: std::time::Instant::now(),
            init_completed_at: std::sync::OnceLock::new(),
            progress,
            references_warmup_done: AtomicBool::new(false),
            null_runs: NullRuns::default(),
        };

        // Perform the LSP initialize handshake
        client
            .initialize(
                crate::lsp::init_options::for_workspace(&config.command, &config.workspace_root),
                crate::lsp::init_options::wants_progress(&config.command),
                crate::lsp::init_options::wants_call_hierarchy(&config.command),
            )
            .await?;

        Ok(client)
    }

    /// Connect to an existing mux socket instead of spawning a process.
    ///
    /// The mux sends a JSON init message immediately on connect containing
    /// the cached `InitializeResult`. This client does NOT perform the LSP
    /// initialize handshake. The wait for that message is bounded by
    /// [`MUX_INIT_READ_TIMEOUT`]; use [`Self::connect_with_init_timeout`] to bound it by a
    /// caller's remaining window instead.
    #[cfg(unix)]
    pub async fn connect(
        socket_path: &std::path::Path,
        workspace_root: std::path::PathBuf,
    ) -> Result<Self> {
        Self::connect_with_init_timeout(socket_path, workspace_root, MUX_INIT_READ_TIMEOUT).await
    }

    /// [`Self::connect`] with the wait for the mux's init message bounded by `init_timeout`.
    ///
    /// A socket that accepts the connection and then says nothing (a mux wedged after `bind`)
    /// makes this return an error once `init_timeout` has passed. The error path returns before
    /// the reader task is spawned, so nothing outlives the call: the stream's halves are dropped
    /// and the connection closes.
    #[cfg(unix)]
    pub async fn connect_with_init_timeout(
        socket_path: &std::path::Path,
        workspace_root: std::path::PathBuf,
        init_timeout: std::time::Duration,
    ) -> Result<Self> {
        use tokio::net::UnixStream;

        let stream = UnixStream::connect(socket_path)
            .await
            .with_context(|| format!("Failed to connect to mux socket: {:?}", socket_path))?;

        let (read_half, write_half) = stream.into_split();

        let pending: PendingRequests = Arc::new(StdMutex::new(HashMap::new()));
        let alive = Arc::new(AtomicBool::new(true));
        let writer: Arc<Mutex<Box<dyn AsyncWrite + Unpin + Send>>> =
            Arc::new(Mutex::new(Box::new(write_half)));

        // Read init message from the mux — contains the cached InitializeResult
        // so we skip the full LSP handshake.
        let mut buf_reader = BufReader::new(read_half);
        let init_msg = tokio::time::timeout(init_timeout, transport::read_message(&mut buf_reader))
            .await
            .map_err(|_| {
                anyhow::Error::new(MuxInitTimeout {
                    socket_path: socket_path.to_path_buf(),
                    after: init_timeout,
                })
            })?
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
        let progress = Arc::new(ProgressTracker::default());
        let progress_clone = progress.clone();
        let reader_handle = tokio::spawn(async move {
            let _read_err = Self::run_dispatch_loop(
                buf_reader,
                pending_clone.clone(),
                writer_clone,
                progress_clone,
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
            synced_sigs: SyncedSignatures::default(),
            stderr_lines: Arc::new(StdMutex::new(Vec::new())),
            started_at: std::time::Instant::now(),
            init_completed_at: std::sync::OnceLock::new(),
            progress,
            references_warmup_done: AtomicBool::new(false),
            null_runs: NullRuns::default(),
        })
    }

    /// Whether this client is inside its cold-start window, where a server is still
    /// loading and transient non-answers are expected. Shared by `request`'s error
    /// retry and `document_symbols`' null-answer retry, so both read one clock.
    ///
    /// Anchored at init completion if we have it; otherwise at construction time,
    /// so in-flight init requests (and socket clients, which never run `initialize`)
    /// still get the patient window.
    pub(crate) fn in_cold_start_window(&self) -> bool {
        const COLD_START_WINDOW: std::time::Duration = std::time::Duration::from_secs(5 * 60);
        let anchor = self.init_completed_at.get().unwrap_or(&self.started_at);
        anchor.elapsed() < COLD_START_WINDOW
    }

    /// Send a JSON-RPC request and await the response.
    pub async fn request(&self, method: &str, params: Value) -> Result<Value> {
        // During the cold-start indexing window (e.g. Gradle import for kotlin-lsp),
        // the server returns -32800 (RequestCancelled) for every query. We use a
        // patient retry window while fresh, and a short one once warm.
        //
        // Cold: cold_start_max_retries() retries, the delay before retry N being 3 s x N
        //       (linear backoff), so the delays alone total 3 s x (1 + ... + retries):
        //       Linux 10 retries = 165 s, macOS 15 = 360 s, Windows 20 = 630 s. Each of the
        //       retries + 1 requests can also wait its own 30 s timeout, so the worst case is
        //       165 + 11 x 30 = 495 s on Linux, 360 + 16 x 30 = 840 s on macOS and
        //       630 + 21 x 30 = 1260 s on Windows. (`cold_start_max_retries`.)
        // Warm:  3 retries, delay 300 ms x N: 0.3 + 0.6 + 0.9 = 1.8 s of delays, plus up to
        //       4 x 30 s of request timeouts in the worst case.
        // cap-class: NOT_A_CAP — retry ceiling; exhaustion surfaces as an error, never as a shortened result
        const MAX_RETRIES_COLD: usize = cold_start_max_retries();
        const RETRY_DELAY_COLD_MS: u64 = 3_000;
        // cap-class: NOT_A_CAP — retry ceiling; exhaustion surfaces as an error, never as a shortened result
        const MAX_RETRIES_WARM: usize = 3;
        const RETRY_DELAY_WARM_MS: u64 = 300;

        // Bring the server's copy of every file up to date before it is asked anything.
        // This is the one place every request passes through (all but `initialize`, which
        // runs before any file is open), so a new LSP operation cannot forget it. See
        // `resync_all_drifted` for why it covers every file and not only the one named.
        if method != "initialize" && method != "shutdown" {
            self.resync_all_drifted().await;
        }

        let in_cold_start = self.in_cold_start_window();
        let (max_retries, retry_delay_ms) = if in_cold_start && uses_cold_start_retry_budget(method)
        {
            (MAX_RETRIES_COLD, RETRY_DELAY_COLD_MS)
        } else {
            (MAX_RETRIES_WARM, RETRY_DELAY_WARM_MS)
        };

        // Only retry idempotent methods on transient LSP errors (-32800
        // RequestCancelled, -32801 ContentModified). Retrying a non-idempotent
        // method like textDocument/rename risks double-applying an edit if
        // the server cancelled AFTER performing the operation.
        let retry_on_cancel = is_idempotent_lsp_method(method);
        let effective_max_retries = if retry_on_cancel { max_retries } else { 0 };

        let mut last_err = None;
        for attempt in 0..=effective_max_retries {
            if attempt > 0 {
                let delay = std::time::Duration::from_millis(retry_delay_ms * attempt as u64);
                tokio::time::sleep(delay).await;
                tracing::debug!(
                    "LSP transient error, retrying {}/{}: {} (cold_start={})",
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
                Err(e) if is_retryable_lsp_error(&e) => {
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
        // From here the future can be dropped by an outer deadline; the guard cleans up.
        let mut in_flight = InFlightRequest {
            id,
            pending: self.pending.clone(),
            writer: self.writer.clone(),
            sent: false,
            armed: true,
        };

        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        {
            let mut writer = self.writer.lock().await;
            if let Err(e) = transport::write_message(&mut *writer, &msg).await {
                in_flight.armed = false;
                self.pending
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&id);
                return Err(e);
            }
        }
        in_flight.sent = true;

        // Await response with timeout
        let outcome = tokio::time::timeout(timeout, rx).await;
        // Reached an outcome: the branches below own the cleanup from here.
        in_flight.armed = false;
        match outcome {
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
    async fn initialize(
        &self,
        initialization_options: Option<Value>,
        advertise_progress: bool,
        advertise_call_hierarchy: bool,
    ) -> Result<()> {
        let root_uri = path_to_uri(&self.workspace_root)?;

        let params = lsp_types::InitializeParams {
            process_id: Some(std::process::id()),
            capabilities: lsp_types::ClientCapabilities {
                window: advertise_progress.then(|| lsp_types::WindowClientCapabilities {
                    work_done_progress: Some(true),
                    ..Default::default()
                }),
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
                    call_hierarchy: advertise_call_hierarchy.then_some(
                        lsp_types::CallHierarchyClientCapabilities {
                            dynamic_registration: Some(false),
                        },
                    ),
                    ..Default::default()
                }),
                ..Default::default()
            },
            initialization_options,
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
        // cap-class: NOT_A_CAP — initialize retry ceiling; exhaustion surfaces as the last error, never as a shortened result
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
    ///
    /// A DIRECT client asks its own child to stop (`shutdown`, `exit`) and waits for the reader
    /// task, which ends when the child closes its stdout. A SOCKET client does neither: its
    /// server is shared through a mux with other clients, so it only disconnects. The mux
    /// answers a `shutdown` itself and drops `exit` (and a mux started by an older codescout
    /// would forward both and stop the server under every other client), and it never hangs up
    /// on a client, so waiting for the reader would burn the whole timeout and leave the
    /// connection — and with it the mux's idle timer — held open by a detached task. Closing
    /// both halves is what ends the client's session on the mux (`handle_client_disconnect`).
    pub async fn shutdown(&self) -> Result<()> {
        if !self.alive.load(Ordering::SeqCst) {
            return Ok(());
        }

        let is_socket = matches!(self.transport, LspTransport::Socket { .. });
        if !is_socket {
            // Send shutdown request
            let _ = self.request("shutdown", Value::Null).await;

            // Send exit notification
            let _ = self.notify("exit", Value::Null).await;
        }

        // Mark as dead
        self.alive.store(false, Ordering::SeqCst);

        // Extract handle before awaiting to avoid holding MutexGuard across await
        let handle = self
            .reader_handle
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(mut handle) = handle {
            if is_socket {
                // Dropping the task drops its read half and its clone of the writer.
                handle.abort();
                let _ = handle.await;
                // The aborted reader never reaches its own end-of-task drain.
                Self::drain_pending_disconnect(&self.pending, "LSP client shut down");
                // Then end our write half: the mux reads EOF and releases this client.
                let mut w = self.writer.lock().await;
                let _ = tokio::io::AsyncWriteExt::shutdown(&mut *w).await;
            } else if tokio::time::timeout(std::time::Duration::from_secs(5), &mut handle)
                .await
                .is_err()
            {
                // A dropped JoinHandle DETACHES its task; abort it so the reader (and the
                // child it owns) does not outlive a shutdown that gave up waiting.
                handle.abort();
                let _ = handle.await;
                // Same as the socket path: the aborted reader's own drain never runs.
                Self::drain_pending_disconnect(&self.pending, "LSP client shut down");
            }
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

        Ok(with_python_ranges_by_file(
            infos
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
                .collect(),
        ))
    }

    /// Note that `path`'s current bytes were handed to the server.
    ///
    /// See [`SyncedSignatures::record`] for what `overwrite` means and why it is not
    /// always `true`.
    fn record_synced(&self, path: &Path, overwrite: bool) {
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        self.synced_sigs.record(&canonical, path, overwrite);
    }

    /// Re-send every file the server already has that changed on disk behind our back.
    ///
    /// codescout's own writes notify the server. An external `git checkout`, `git
    /// stash`, a rebase, `cargo fmt`, or a plain editor save does not — and `did_open`
    /// returns early for an already-open file (stdio) or is deduped by the mux (socket), so
    /// nothing re-syncs it. The server then keeps answering from the old content,
    /// which is the expensive failure: not an error, a confident wrong answer, with
    /// symbol bodies and line numbers describing a version of the file that no longer
    /// exists.
    ///
    /// EVERY recorded file, not only the one a request names. A `references` or a rename
    /// answers about other files too, from the server's copy of each, and a rename edit is
    /// written to disk at the ranges the server computed. Repairing only the named file
    /// let a stale range overwrite the wrong bytes of another one
    /// (`docs/issues/2026-11-11-edit-code-rename-writes-a-stale-lsp-range-into-the-wrong-bytes-and-reports-ok.md`).
    ///
    /// Only fires for files with a recorded signature. A file we have never sent is
    /// `did_open`'s business, not this function's.
    async fn resync_all_drifted(&self) {
        for sent_as in self.synced_sigs.drifted() {
            tracing::debug!(path = %sent_as.display(), "resyncing file changed outside codescout");
            // did_change re-reads from disk and records the new signature, so a failure
            // here simply leaves the drift recorded for the next attempt.
            let _ = self.did_change(&sent_as).await;
        }
    }

    /// Send textDocument/didOpen notification for a file.
    pub async fn did_open(&self, path: &Path, language_id: &str) -> Result<()> {
        // For socket transport, the mux handles document state dedup — skip
        // local open_files tracking so every didOpen is forwarded to the mux.
        let is_socket = matches!(self.transport, LspTransport::Socket { .. });
        if !is_socket {
            // Canonicalize before tracking to avoid treating symlinks or relative paths
            // as different files — the LSP spec prohibits duplicate didOpen notifications.
            let canonical = std::fs::canonicalize(path)
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

        // cap-class: RESULT_CAP lsp.did_open_size — probed
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
        // Vacant-only: a repeat didOpen delivers nothing on either transport.
        self.record_synced(path, false);
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

    /// How long a SUCCESSFUL-but-empty answer is re-asked before it is taken as final:
    /// 5 s inside the cold-start window, 1 s once warm. See `request_until_answered`.
    fn not_answered_budget(&self) -> std::time::Duration {
        // cap-class: NOT_A_CAP — wait ceiling; each caller decides what exhaustion means
        if self.in_cold_start_window() {
            std::time::Duration::from_secs(5)
        } else {
            std::time::Duration::from_secs(1)
        }
    }

    /// Send `method` and re-ask while the answer is `not_answered` — a SUCCESS that
    /// carries nothing — every 200 ms within `not_answered_budget`, and past it for a `null`
    /// answer while the server reports start-up progress (see `progress::keep_waiting`).
    /// Returns `Ok(answer)`, or `Err(waited)` if every attempt was not-answered, with how long
    /// that took, so the caller's error states the time it really waited.
    ///
    /// rust-analyzer answers this way while it loads its workspace, and `request`'s
    /// retry cannot see it because it retries only -32800 / -32801 ERRORS. Measured
    /// 2026-09-24 against a cold 1.97.1 server on this repo: `documentSymbol` answers
    /// `null` for every open file at each crate-graph swap (0.2–0.7 s, twice during
    /// start-up); `references` answers `[]` and `null` in the same window, then -32801
    /// until cache priming ends (~10 s)
    /// (docs/issues/archive/2026-08-27-references-symbol-not-found-while-lsp-warms.md).
    async fn request_until_answered(
        &self,
        method: &str,
        params: Value,
        not_answered: fn(&Value) -> bool,
    ) -> Result<std::result::Result<Value, std::time::Duration>> {
        const RETRY_INTERVAL: std::time::Duration = std::time::Duration::from_millis(200);
        let budget = self.not_answered_budget();
        let first_asked = std::time::Instant::now();
        loop {
            let result = self.request(method, params.clone()).await?;
            if !not_answered(&result) {
                return Ok(Ok(result));
            }
            // Past the budget a null answer is still worth asking while the server reports
            // work (see `progress`): only in the cold-start window, and never for `[]`.
            let busy = self.in_cold_start_window() && self.progress.busy(std::time::Instant::now());
            let waited = first_asked.elapsed();
            if !keep_waiting(waited, budget, result.is_null(), busy) {
                return Ok(Err(waited));
            }
            tokio::time::sleep(RETRY_INTERVAL).await;
        }
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
        // Check for drift BEFORE did_open, not after — the ordering is the whole fix.
        //
        // On SOCKET transport `did_open` skips its already-open early return (the mux
        // owns document dedup), so it re-reads the file and re-records the signature on
        // every call. Running the drift check after that compares the current disk state
        // against itself and can never fire, while the mux quietly dedups the didOpen so
        // the server keeps its stale copy. Checking first compares against the last
        // content actually delivered.
        //
        // Ordering is also correct for stdio and for a first-time open: a file with no
        // recorded signature is `did_open`'s business, and `resync_all_drifted` returns
        // without doing anything.
        self.resync_all_drifted().await;
        // Ensure the file is open in the server.
        self.did_open(path, language_id).await?;

        let uri = path_to_uri(path)?;
        let params = lsp_types::DocumentSymbolParams {
            text_document: lsp_types::TextDocumentIdentifier { uri },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        // A null answer is re-asked, never read as "no symbols" — see
        // `request_until_answered`. It used to become `Ok(vec![])` here: `references`
        // said "symbol not found" and path-scoped `symbols` said "0 matches" for symbols
        // that exist. An empty ARRAY is an answer and is returned as one.
        let result = match self
            .request_until_answered(
                "textDocument/documentSymbol",
                serde_json::to_value(params)?,
                Value::is_null,
            )
            .await?
        {
            Ok(result) => {
                self.null_runs.clear(path);
                result
            }
            Err(waited) => {
                let run = self.null_runs.record_null(path);
                let mut hint = crate::lsp::watch_fanout::hint_with_fanout(
                    "null means the server has not answered, not that the file has no symbols \
                         — rust-analyzer sends it while it reloads its workspace. A repo with many \
                         nested linked worktrees (for example under `.claude/worktrees`) can keep \
                         it answering null for about a minute after start-up. Retry shortly.",
                    crate::lsp::watch_fanout::probe(&self.workspace_root)
                        .await
                        .as_ref(),
                );
                if let Some(advice) = NullRuns::repeat_advice(run) {
                    hint.push_str(&advice);
                }
                return Err(RecoverableError::with_hint(
                    format!(
                        "the language server answered textDocument/documentSymbol with null \
                         for {} on every attempt for {} ms",
                        path.display(),
                        waited.as_millis()
                    ),
                    hint,
                )
                .into());
            }
        };

        // LSP returns either DocumentSymbol[] (hierarchical) or SymbolInformation[] (flat)
        // We prefer hierarchical and convert both to SymbolInfo
        let file_path = path.to_path_buf();

        // Try hierarchical first
        if let Ok(symbols) =
            serde_json::from_value::<Vec<lsp_types::DocumentSymbol>>(result.clone())
        {
            return Ok(with_python_ranges(
                convert_document_symbols(&symbols, &file_path, ""),
                language_id,
                path,
            ));
        }

        // Fall back to flat SymbolInformation[]
        if let Ok(infos) = serde_json::from_value::<Vec<lsp_types::SymbolInformation>>(result) {
            let flat = infos
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
                .collect();
            return Ok(with_python_ranges(flat, language_id, path));
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

        // `null` and `[]` are both "not answered yet" here: with `includeDeclaration:
        // true`, a server that has resolved the symbol returns at least its declaration.
        // Re-asked within the budget (see `request_until_answered`). On exhaustion the
        // empty answer is KEPT, not turned into an error: a server that ignores
        // `includeDeclaration` answers `[]` for a genuinely unused symbol, and
        // `References::call` warns on a zero with no declaration instead.
        let result = self
            .request_until_answered(
                "textDocument/references",
                serde_json::to_value(params)?,
                |v| v.is_null() || v.as_array().is_some_and(|a| a.is_empty()),
            )
            .await?
            .unwrap_or(Value::Null);

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

        decode_rename_answer(result)
    }

    /// Send textDocument/didClose notification for a file.
    pub async fn did_close(&self, path: &Path) -> Result<()> {
        // For socket transport, the mux tracks document state — skip local bookkeeping.
        if !matches!(self.transport, LspTransport::Socket { .. }) {
            let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
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

        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

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
        // didChange always sends, so this is a true "last delivered".
        self.record_synced(path, true);
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

    /// Prepare a call hierarchy item at the given position.
    ///
    /// Returns `Ok(None)` if the server does not support call hierarchy or if no
    /// item is found at the cursor position.
    pub async fn prepare_call_hierarchy(
        &self,
        path: &Path,
        line: u32,
        col: u32,
        language_id: &str,
    ) -> Result<Option<lsp_types::CallHierarchyItem>> {
        {
            let caps = self.capabilities.lock().unwrap_or_else(|e| e.into_inner());
            if !supports_call_hierarchy(&caps) {
                return Ok(None);
            }
        }
        self.did_open(path, language_id).await?;
        let uri = path_to_uri(path)?;
        let params = lsp_types::CallHierarchyPrepareParams {
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
            .request(
                "textDocument/prepareCallHierarchy",
                serde_json::to_value(params)?,
            )
            .await?;

        if result.is_null() {
            return Ok(None);
        }

        let items: Vec<lsp_types::CallHierarchyItem> = serde_json::from_value(result)?;
        Ok(items.into_iter().next())
    }

    /// Shared capability-check / request / deserialize tail for the two
    /// call-hierarchy directions (`incoming_calls`/`outgoing_calls`), which
    /// differ only in the LSP method name, the params struct, and the
    /// response element type.
    async fn send_call_hierarchy_request<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: impl serde::Serialize,
    ) -> Result<Vec<T>> {
        {
            let caps = self.capabilities.lock().unwrap_or_else(|e| e.into_inner());
            if !supports_call_hierarchy(&caps) {
                return Ok(vec![]);
            }
        }
        let result = self.request(method, serde_json::to_value(params)?).await?;

        if result.is_null() {
            return Ok(vec![]);
        }

        let calls: Vec<T> = serde_json::from_value(result)?;
        Ok(calls)
    }

    /// Fetch incoming calls for a call hierarchy item.
    ///
    /// Returns an empty vec if the server does not support call hierarchy.
    pub async fn incoming_calls(
        &self,
        item: &lsp_types::CallHierarchyItem,
        _language_id: &str,
    ) -> Result<Vec<lsp_types::CallHierarchyIncomingCall>> {
        let params = lsp_types::CallHierarchyIncomingCallsParams {
            item: item.clone(),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        self.send_call_hierarchy_request("callHierarchy/incomingCalls", params)
            .await
    }

    /// Fetch outgoing calls for a call hierarchy item.
    ///
    /// Returns an empty vec if the server does not support call hierarchy.
    pub async fn outgoing_calls(
        &self,
        item: &lsp_types::CallHierarchyItem,
        _language_id: &str,
    ) -> Result<Vec<lsp_types::CallHierarchyOutgoingCall>> {
        let params = lsp_types::CallHierarchyOutgoingCallsParams {
            item: item.clone(),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        self.send_call_hierarchy_request("callHierarchy/outgoingCalls", params)
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
        // Kill the child process. This is the PRIMARY killer, not a safety net:
        // the graceful path (shutdown_all -> shutdown) runs only on an orderly
        // shutdown, so in the ordinary drop this SIGTERM is what reaps the child.
        // Measured 2026-09-08 by strace — `kill(<pid>, SIGTERM)` from this line
        // lands FIRST, and the `kill_on_drop(true)` SIGKILL that follows arrives
        // against a process already dead.
        // For socket-connected clients there is no child to kill.
        if let LspTransport::Process {
            child_pid: Some(pid),
        } = &self.transport
        {
            // No SAFETY block is owed here: `terminate_process` is safe, and it owns
            // the `unsafe` and the pid-addressability argument (`platform::unix`'s
            // `addressable_pid` REFUSES a pid that would signal a group, rather than
            // casting). What is worth saying is what this site alone controls: the
            // `Child` lives in the reader task, which the block above just aborted,
            // so nothing here holds the pid open. Recycling it inside that window
            // would take ~4M intervening spawns, so this is sound in practice and
            // not by construction — see
            // docs/issues/archive/2026-09-09-a-safety-comment-outlived-both-its-unsafe-block-and-its-own-rationale.md
            let _ = crate::platform::terminate_process(*pid);
        }
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
        // per-attempt timeout) blows the 60s MCP tool timeout. symbols
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
    fn cold_start_max_retries_matches_host_platform() {
        // The helper is `const fn` with cfg! branching, so the result for the
        // currently-compiling host should be deterministic. This test asserts
        // that no future refactor accidentally inverts the platform ranking
        // (Windows must get the most headroom, Linux the least).
        let actual = cold_start_max_retries();
        let expected = if cfg!(target_os = "windows") {
            20
        } else if cfg!(target_os = "macos") {
            15
        } else {
            10
        };
        assert_eq!(actual, expected);

        // Sanity: cold-start budget must always exceed the warm-path retry
        // count (3, see `request`). A future tweak that drops cold below warm
        // means cold-start would retry *fewer* times than steady-state — almost
        // certainly a regression.
        assert!(actual >= 3, "cold-start retries must exceed warm budget");
    }

    #[test]
    fn is_retryable_lsp_error_matches_both_transient_codes() {
        // Both -32800 (RequestCancelled) and -32801 (ContentModified) are
        // explicit LSP "retry on new snapshot" signals. They share a retry
        // path here; this test pins both codes against the matcher.
        let cancelled = anyhow::anyhow!("LSP error (code -32800): cancelled");
        let modified = anyhow::anyhow!("LSP error (code -32801): content modified");
        let internal = anyhow::anyhow!("LSP error (code -32603): internal error");
        let timeout = anyhow::anyhow!("LSP request timed out after 30s");

        assert!(is_retryable_lsp_error(&cancelled), "-32800 must retry");
        assert!(is_retryable_lsp_error(&modified), "-32801 must retry");
        assert!(
            !is_retryable_lsp_error(&internal),
            "-32603 is a real fault — must NOT retry"
        );
        assert!(
            !is_retryable_lsp_error(&timeout),
            "timeouts are surfaced as RecoverableError separately — must NOT match"
        );
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
    fn detect_fatal_stderr_flags_rustup_missing_component() {
        // rustup's shim for rust-analyzer prints this to stderr and exits 1
        // immediately if the component isn't installed for the active toolchain.
        // Pre-fix the LSP launch path swallowed it and reported the opaque
        // "LSP server disconnected" instead of pointing at the rustup fix.
        // See docs/issues/archive/2026-05-20-lsp-launch-opaque-disconnected-error.md.
        let lines = vec![
            "error: Unknown binary 'rust-analyzer' in official toolchain \
                 'stable-x86_64-unknown-linux-gnu'."
                .to_string(),
        ];
        let hint = detect_fatal_stderr(&lines).expect("should detect rustup pattern");
        let msg = hint.to_string();
        assert!(
            msg.contains("rust-analyzer"),
            "error message should name rust-analyzer: {msg}"
        );
        assert!(
            msg.contains("rustup component add rust-analyzer"),
            "error message should include the rustup fix command: {msg}"
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
    /// The `initialize` params a fake server received when `LspClient::start` ran it as
    /// `command_name` in a repository with one linked worktree nested inside it (`.worktrees/wt`).
    #[cfg(unix)]
    async fn initialize_params_seen_by_a_fake_server(command_name: &str) -> serde_json::Value {
        let server = crate::lsp::init_options::test_support::fake_server(command_name);
        let client = LspClient::start(LspServerConfig {
            command: server.command.to_string_lossy().into_owned(),
            args: server.args.clone(),
            workspace_root: server.workspace.clone(),
            init_timeout: Some(std::time::Duration::from_secs(10)),
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        })
        .await
        .expect("the fake server should start");
        client.shutdown().await.unwrap();
        server
            .recorded_params()
            .expect("the fake server recorded the initialize params")
    }

    /// rust-analyzer scans every directory under the root, so a linked worktree nested in the
    /// project multiplies the scan and keeps `documentSymbol` answering null for tens of seconds.
    /// `initialize` must tell it to leave those directories alone.
    #[cfg(unix)]
    #[tokio::test]
    async fn initialize_excludes_nested_worktrees_for_rust_analyzer() {
        let params = initialize_params_seen_by_a_fake_server("rust-analyzer").await;
        assert_eq!(
            params["initializationOptions"],
            serde_json::json!({"files": {"excludeDirs": [".worktrees"]}}),
            "initialize params were: {params}"
        );
        // rust-analyzer sends `$/progress` only to a client that advertises it; the cold-start
        // wait for a null answer reads that progress.
        assert_eq!(
            params["capabilities"]["window"]["workDoneProgress"],
            serde_json::json!(true),
            "initialize params were: {params}"
        );
    }

    /// `files.excludeDirs` is a rust-analyzer setting. Any other server must get no
    /// `initializationOptions` at all (not `null`), as before.
    #[cfg(unix)]
    #[tokio::test]
    async fn initialize_sends_no_options_to_another_server() {
        let params = initialize_params_seen_by_a_fake_server("some-other-server").await;
        assert!(
            params.get("initializationOptions").is_none(),
            "initialize params were: {params}"
        );
        // Another server (kotlin-lsp above all) keeps the capabilities it has always seen: an
        // advertised progress capability would make it send a stream it never sent before.
        assert!(
            params["capabilities"].get("window").is_none(),
            "initialize params were: {params}"
        );
    }
    /// `typescript-language-server` leaves `callHierarchyProvider` out of its capabilities unless the
    /// client declares `textDocument.callHierarchy`, and without it `call_graph` never asks the
    /// server. Another server keeps the capabilities it has always seen.
    #[cfg(unix)]
    #[tokio::test]
    async fn initialize_declares_call_hierarchy_to_typescript_language_server_only() {
        let ts = initialize_params_seen_by_a_fake_server("typescript-language-server").await;
        assert_eq!(
            ts["capabilities"]["textDocument"]["callHierarchy"]["dynamicRegistration"],
            serde_json::json!(false),
            "initialize params were: {ts}"
        );
        let other = initialize_params_seen_by_a_fake_server("some-other-server").await;
        assert!(
            other["capabilities"]["textDocument"]
                .get("callHierarchy")
                .is_none(),
            "initialize params were: {other}"
        );
    }

    /// Start the progress fixture (null `documentSymbol` answers for `null_seconds` after
    /// `initialize`, announced as a progress token unless `announce` is false) and ask for the
    /// symbols of one file. Real clock: a paused clock jumps over real socket I/O.
    #[cfg(unix)]
    async fn document_symbols_from_a_slow_starting_server(
        null_seconds: u32,
        announce: bool,
    ) -> (Result<Vec<crate::lsp::SymbolInfo>>, std::time::Duration) {
        document_symbols_in(tempdir().unwrap(), null_seconds, announce).await
    }

    /// [`document_symbols_from_a_slow_starting_server`] in a workspace the caller has prepared.
    #[cfg(unix)]
    async fn document_symbols_in(
        dir: tempfile::TempDir,
        null_seconds: u32,
        announce: bool,
    ) -> (Result<Vec<crate::lsp::SymbolInfo>>, std::time::Duration) {
        let file = dir.path().join("lib.rs");
        std::fs::write(&file, "fn ready_symbol() {}\n").unwrap();
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/fake_lsp_progress.py");
        let mut args = vec![
            fixture.to_string_lossy().into_owned(),
            null_seconds.to_string(),
        ];
        if !announce {
            args.push("noprogress".into());
        }
        let client = LspClient::start(LspServerConfig {
            command: "python3".into(),
            args,
            workspace_root: dir.path().to_path_buf(),
            init_timeout: Some(std::time::Duration::from_secs(10)),
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        })
        .await
        .expect("the fake server should start");
        // Time the call only: `shutdown` is not what these tests are about.
        let started = std::time::Instant::now();
        let symbols = client.document_symbols(&file, "rust").await;
        let elapsed = started.elapsed();
        client.shutdown().await.unwrap();
        (symbols, elapsed)
    }

    /// rust-analyzer answers `documentSymbol` with null for much longer than the 5 s cold budget on
    /// a big repo, and announces that work as progress. A null answer must be re-asked while a
    /// progress token is open: here the server is null for 7 s and the budget is 5 s.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_null_answer_is_re_asked_past_the_budget_while_the_server_reports_progress() {
        let (symbols, _) = document_symbols_from_a_slow_starting_server(7, true).await;
        let symbols = symbols.expect("the server reported progress for the whole null window");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "ready_symbol");
    }

    /// The control: the same 7 s of null with NO progress announced still ends at the 5 s budget,
    /// with the same error as before. Nothing is waited for that the server did not announce.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_null_answer_still_ends_at_the_budget_when_the_server_reports_no_progress() {
        let (result, elapsed) = document_symbols_from_a_slow_starting_server(7, false).await;
        let err = result.expect_err("no progress was announced, so the 5 s budget applies");
        assert!(
            err.to_string()
                .contains("answered textDocument/documentSymbol with null"),
            "got: {err}"
        );
        // The error states the time actually waited: a little over the 5 s budget.
        assert!(
            err.to_string().contains("on every attempt for 5"),
            "got: {err}"
        );
        assert!(
            elapsed < std::time::Duration::from_secs(7),
            "gave up at the 5 s budget, not after the 7 s null window; took {elapsed:?}"
        );
    }

    /// The hint of a `RecoverableError`, which is where the failure path puts its advice. It is read
    /// through `Debug` because `Guidance` is not exported outside `tools::core`.
    #[cfg(unix)]
    fn hint_of(err: &anyhow::Error) -> String {
        let recoverable = err
            .downcast_ref::<crate::tools::RecoverableError>()
            .unwrap_or_else(|| panic!("not a RecoverableError: {err:?}"));
        format!("{:?}", recoverable.guidance)
    }

    /// A null that outlasts the budget in a workspace holding many directory symlinks says so in
    /// the hint, and keeps the advice it already gave. This is the bug's shape: 109 links under a
    /// gitignored directory, and nothing named them.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_null_answer_in_a_workspace_full_of_directory_symlinks_names_them() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("target");
        std::fs::create_dir_all(&target).unwrap();
        for i in 0..25 {
            std::os::unix::fs::symlink(&target, dir.path().join(format!("l{i}"))).unwrap();
        }
        let (result, _) = document_symbols_in(dir, 7, false).await;
        let err = result.expect_err("no progress was announced, so the 5 s budget applies");
        let hint = hint_of(&err);
        assert!(hint.contains("25 directory symlinks"), "got: {hint}");
        assert!(
            hint.contains("25 directory symlinks, with no single directory holding most of them")
                && hint.contains("should live outside the workspace root"),
            "the hint says where the links are and what to do: {hint}"
        );
        assert!(
            hint.contains("Retry shortly"),
            "the old advice stays: {hint}"
        );
    }

    /// The control: the same null in a workspace with no directory symlink gets the old hint, with
    /// no sentence about links. A hint that always mentioned them would pass the test above.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_null_answer_in_a_workspace_without_directory_symlinks_gets_the_old_hint() {
        let (result, _) = document_symbols_from_a_slow_starting_server(7, false).await;
        let err = result.expect_err("no progress was announced, so the 5 s budget applies");
        assert!(
            !hint_of(&err).contains("directory symlinks"),
            "got: {err:?}"
        );
    }

    /// LIVE and manual: codescout's own client against a real rust-analyzer on THIS repository,
    /// asking for the symbols of one file just after the server has started, and printing how long
    /// the answer took. It passes when the call answers, so it is a smoke test of the whole chain
    /// (init options, progress capability, progress wait) and NOT a discriminator: the null window
    /// varies run to run (4.0 s and 6.2 s back to back with the progress wait on and off, because
    /// the file cache warms), so a short window passes with or without the wait. The deterministic
    /// evidence for the wait is the fixture test with a fixed 7 s window.
    ///
    /// Run: `cargo test --lib live_cold_document_symbols -- --ignored --nocapture`
    #[tokio::test]
    #[ignore] // needs rust-analyzer and a cold start of this repo; run manually
    async fn live_cold_document_symbols_smoke_test_on_this_repo() {
        if !rust_analyzer_available() {
            eprintln!("Skipping: rust-analyzer not installed");
            return;
        }
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let client = LspClient::start(LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: root.clone(),
            init_timeout: Some(std::time::Duration::from_secs(60)),
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        })
        .await
        .expect("rust-analyzer should start");
        // A cold rust-analyzer answers symbols for its first ~0.7 s, BEFORE its first workspace
        // reload, and then answers null until the scan ends (measured: symbols at 0.07 s, null
        // from 0.7 s). The failing call in the field came 1.4 s after start. Asking at once would
        // test nothing, so wait until the null window has begun.
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        let started = std::time::Instant::now();
        let symbols = client
            .document_symbols(&root.join("src/lsp/init_options.rs"), "rust")
            .await;
        eprintln!(
            "document_symbols answered after {:?}: {}",
            started.elapsed(),
            match &symbols {
                Ok(s) => format!("{} symbols", s.len()),
                Err(e) => format!("error: {e}"),
            }
        );
        client.shutdown().await.unwrap();
        assert!(
            symbols.is_ok_and(|s| !s.is_empty()),
            "the call must answer, not report null"
        );
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

    /// What `poll_until_nonempty` learned: the answer, and how many requests hit the ceiling.
    #[derive(Debug)]
    struct Polled<T> {
        found: Vec<T>,
        timeouts: u32,
    }

    /// Calls `query` until it answers with something, or `attempts` calls have run.
    ///
    /// A server that is still indexing answers empty, and under CPU load one request can pass
    /// the 30 s ceiling instead (bug 20927ccfe293c34e). Both mean "not ready yet". The first
    /// `max_timeouts` timeouts are retried and counted in `Polled::timeouts`; the next one is
    /// returned as an error, so a server that never answers is not hidden. Any other error is
    /// returned at once. Kept apart from the rust-analyzer test so these three rules run
    /// without a language server.
    async fn poll_until_nonempty<T, F, Fut>(
        attempts: u32,
        pause: std::time::Duration,
        max_timeouts: u32,
        mut query: F,
    ) -> Result<Polled<T>>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<Vec<T>>>,
    {
        let mut found = Vec::new();
        let mut timeouts = 0;
        for _ in 0..attempts {
            match query().await {
                Ok(answer) => {
                    found = answer;
                    if !found.is_empty() {
                        break;
                    }
                }
                Err(e) if e.to_string().contains("timed out") && timeouts < max_timeouts => {
                    timeouts += 1;
                }
                Err(e) => return Err(e),
            }
            tokio::time::sleep(pause).await;
        }
        Ok(Polled { found, timeouts })
    }

    /// A scripted answer for `poll_until_nonempty`: the server's reply to each successive call.
    fn scripted(
        replies: Vec<Result<Vec<u32>>>,
    ) -> (
        std::sync::Arc<std::sync::atomic::AtomicU32>,
        impl FnMut() -> std::future::Ready<Result<Vec<u32>>>,
    ) {
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let seen = calls.clone();
        let mut replies = replies.into_iter();
        let query = move || {
            seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            std::future::ready(replies.next().expect("the script ran out of replies"))
        };
        (calls, query)
    }

    /// The error `LspClient::request` returns when the 30 s ceiling passes, built the same way.
    fn request_timeout() -> anyhow::Error {
        RecoverableError::with_hint(
            "LSP request timed out after 30s: workspace/symbol",
            "The server did not respond in time.",
        )
        .into()
    }

    #[tokio::test]
    async fn a_timeout_is_retried_and_counted() {
        let (calls, query) = scripted(vec![Err(request_timeout()), Ok(vec![]), Ok(vec![7])]);
        let polled = poll_until_nonempty(5, std::time::Duration::ZERO, 2, query)
            .await
            .expect("one timeout is within the allowance");
        assert_eq!(polled.found, vec![7]);
        assert_eq!(
            polled.timeouts, 1,
            "the timeout must be counted, not silent"
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn a_third_timeout_fails_so_a_silent_server_is_not_hidden() {
        let (calls, query) = scripted(vec![
            Err(request_timeout()),
            Err(request_timeout()),
            Err(request_timeout()),
        ]);
        let err = poll_until_nonempty(5, std::time::Duration::ZERO, 2, query)
            .await
            .expect_err("a third timeout must fail the poll");
        assert!(err.to_string().contains("timed out"), "got: {err:#}");
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn an_error_that_is_not_a_timeout_fails_at_once() {
        let (calls, query) = scripted(vec![Err(anyhow::anyhow!(
            "LSP error (code -32603): internal error"
        ))]);
        let err = poll_until_nonempty(5, std::time::Duration::ZERO, 2, query)
            .await
            .expect_err("an internal error is not 'not ready yet'");
        assert!(err.to_string().contains("internal error"), "got: {err:#}");
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn an_answer_that_stays_empty_uses_every_attempt_and_reports_empty() {
        let (calls, query) = scripted(vec![Ok(vec![]), Ok(vec![]), Ok(vec![])]);
        let polled = poll_until_nonempty(3, std::time::Duration::ZERO, 2, query)
            .await
            .expect("empty answers are not errors");
        assert!(polled.found.is_empty());
        assert_eq!(polled.timeouts, 0);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 3);
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
        // workspace/symbol returns results. Local Linux: typically < 2s, but GHA
        // runners (especially macOS + Windows) can be I/O-slower under load. The
        // loop exits early on success, so a generous ceiling costs nothing when
        // indexing is fast.
        //
        // Windows gets 4× the budget (60s vs 15s): the 15s ceiling was still not
        // enough there — "workspace/symbol 'add' should return results within the
        // retry budget", run 31094160241 — while the same commit passed on ubuntu
        // and macOS. Cold `cargo metadata` with no warm target cache on that
        // runner is the difference, not a logic fault, so widen the budget rather
        // than skip the test and lose Windows coverage of workspace/symbol.
        let attempts = if cfg!(windows) { 120 } else { 30 };
        let polled =
            poll_until_nonempty(attempts, std::time::Duration::from_millis(500), 2, || {
                client.workspace_symbols("add")
            })
            .await
            .unwrap_or_else(|e| panic!("workspace/symbol 'add' failed: {e:#}"));
        // Visible when the test fails or under `--nocapture`: a pass that needed a retry
        // otherwise looks the same as a pass that did not.
        if polled.timeouts > 0 {
            eprintln!(
                "workspace/symbol 'add': {} request(s) passed the ceiling before it answered",
                polled.timeouts
            );
        }
        let symbols = polled.found;
        assert!(
            !symbols.is_empty(),
            "workspace/symbol 'add' should return results within the retry budget"
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
    /// An `LspClient` built by hand around a reader task the test controls.
    fn bare_client(transport: LspTransport, reader: Option<JoinHandle<()>>) -> LspClient {
        LspClient {
            writer: Arc::new(Mutex::new(
                Box::new(tokio::io::sink()) as Box<dyn AsyncWrite + Unpin + Send>
            )),
            next_id: AtomicI64::new(1),
            pending: Arc::new(StdMutex::new(HashMap::new())),
            alive: Arc::new(AtomicBool::new(true)),
            reader_handle: StdMutex::new(reader),
            workspace_root: std::path::PathBuf::from("/"),
            capabilities: StdMutex::new(lsp_types::ServerCapabilities::default()),
            transport,
            init_timeout: std::time::Duration::from_secs(30),
            open_files: StdMutex::new(HashMap::new()),
            synced_sigs: SyncedSignatures::default(),
            stderr_lines: Arc::new(StdMutex::new(Vec::new())),
            started_at: std::time::Instant::now(),
            init_completed_at: std::sync::OnceLock::new(),
            progress: Arc::new(ProgressTracker::default()),
            references_warmup_done: AtomicBool::new(false),
            null_runs: NullRuns::default(),
        }
    }

    /// Registers a request that is waiting for an answer, as `request()` does.
    fn park_request(client: &LspClient) -> oneshot::Receiver<Result<Value>> {
        let (tx, rx) = oneshot::channel();
        client.pending.lock().unwrap().insert(99, tx);
        rx
    }

    /// A reader stand-in that holds what the real reader holds (its clones of `pending` and the
    /// writer) and never ends by itself. Dropped, those clones are released; merely aborted, they
    /// stay until the runtime next polls the task.
    fn spawn_holding_reader(client: &LspClient) -> JoinHandle<()> {
        let pending = client.pending.clone();
        let writer = client.writer.clone();
        tokio::spawn(async move {
            let _held = (pending, writer);
            std::future::pending::<()>().await;
        })
    }
    /// An outer deadline (the per-language budget in `search_project_symbols`) drops the in-flight
    /// `request_with_timeout` future. The server must still be told to stop working on that id, and
    /// the pending entry must not outlive the caller.
    #[tokio::test(start_paused = true)]
    async fn dropping_an_in_flight_request_sends_cancel_request_and_clears_pending() {
        use tokio::io::AsyncReadExt;
        let mut client = bare_client(
            LspTransport::Socket {
                socket_path: "/unused.sock".into(),
            },
            None,
        );
        let (server_end, mut capture) = tokio::io::duplex(64 * 1024);
        client.writer = Arc::new(Mutex::new(Box::new(server_end)));

        let outer = tokio::time::timeout(
            std::time::Duration::from_secs(8),
            client.request_with_timeout(
                "workspace/symbol",
                json!({"query": "x"}),
                std::time::Duration::from_secs(30),
            ),
        )
        .await;
        assert!(outer.is_err(), "premise: the outer budget fires first");

        // Let a spawned cancel task (if any) run.
        for _ in 0..5 {
            tokio::task::yield_now().await;
        }
        assert!(
            client.pending.lock().unwrap().is_empty(),
            "the dropped request's pending entry must be removed"
        );

        drop(client);
        let mut wire = Vec::new();
        capture.read_to_end(&mut wire).await.unwrap();
        let wire = String::from_utf8_lossy(&wire);
        assert!(
            wire.contains("\"method\":\"workspace/symbol\""),
            "premise: the request itself was written: {wire}"
        );
        assert!(
            wire.contains("$/cancelRequest") && wire.contains("\"id\":1"),
            "a dropped request must send $/cancelRequest for its id: {wire}"
        );
    }

    /// `shutdown` must not return while the reader is still alive: `handle.abort()` only REQUESTS
    /// cancellation, and the task (with the read half and writer clone it owns) is dropped when the
    /// runtime next polls it. Awaiting the aborted handle is what makes "connection closed" true
    /// at the moment `shutdown` returns. Observed with no await between `shutdown()` and the
    /// check; on the current-thread runtime nothing else can run in between.
    #[tokio::test(start_paused = true)]
    async fn socket_shutdown_has_dropped_the_reader_when_it_returns() {
        let mut client = bare_client(
            LspTransport::Socket {
                socket_path: "/unused.sock".into(),
            },
            None,
        );
        let reader = spawn_holding_reader(&client);
        // Let the reader run once so it is parked holding its clones.
        tokio::task::yield_now().await;
        *client.reader_handle.get_mut().unwrap() = Some(reader);
        assert_eq!(
            Arc::strong_count(&client.pending),
            2,
            "premise: the reader holds a clone"
        );

        client.shutdown().await.unwrap();

        assert_eq!(
            Arc::strong_count(&client.pending),
            1,
            "the reader's clone must be released by the time shutdown returns"
        );
        assert_eq!(Arc::strong_count(&client.writer), 1);
    }

    /// The same for the direct client's timed-out wait.
    #[tokio::test(start_paused = true)]
    async fn direct_shutdown_has_dropped_the_reader_when_it_returns() {
        let mut client = bare_client(LspTransport::Process { child_pid: None }, None);
        let reader = spawn_holding_reader(&client);
        tokio::task::yield_now().await;
        *client.reader_handle.get_mut().unwrap() = Some(reader);
        assert_eq!(
            Arc::strong_count(&client.pending),
            2,
            "premise: the reader holds a clone"
        );

        client.shutdown().await.unwrap();

        assert_eq!(
            Arc::strong_count(&client.pending),
            1,
            "the reader's clone must be released by the time shutdown returns"
        );
        assert_eq!(Arc::strong_count(&client.writer), 1);
    }

    /// `shutdown` aborts the reader task, which is the one place a disconnect resolves the
    /// requests still waiting (`drain_pending_disconnect` runs at the END of the reader task, and
    /// an aborted task never gets there). Another tool call holding the same `Arc<LspClient>`
    /// mid-request — an LRU or idle eviction, `shutdown_all` — then waited out its whole 30 s
    /// timeout and failed, which `symbols` turns into `0 matches`.
    #[tokio::test(start_paused = true)]
    async fn socket_shutdown_resolves_a_request_still_in_flight() {
        let reader = tokio::spawn(std::future::pending::<()>());
        let client = bare_client(
            LspTransport::Socket {
                socket_path: "/unused.sock".into(),
            },
            Some(reader),
        );
        let in_flight = park_request(&client);

        client.shutdown().await.unwrap();

        let answer = tokio::time::timeout(std::time::Duration::from_secs(1), in_flight)
            .await
            .expect("a request in flight must be resolved by shutdown, not left to its timeout")
            .expect("answered, not dropped");
        assert!(
            answer.is_err(),
            "the in-flight request fails as disconnected"
        );
    }

    /// The `references` tool's one-wait-per-client flag: a new client has not spent it, marking
    /// spends it, and it stays spent. `NoopLsp`-style fakes answer `true`; only the real client's
    /// `false` start is what makes the first `references` call wait.
    #[test]
    fn a_fresh_client_has_not_spent_its_references_warmup() {
        use crate::lsp::ops::LspClientOps;
        let client = bare_client(LspTransport::Process { child_pid: None }, None);

        assert!(
            !client.references_warmup_done(),
            "a client that has served no `references` call has not waited yet"
        );
        client.mark_references_warmup_done();
        assert!(client.references_warmup_done());
        client.mark_references_warmup_done();
        assert!(client.references_warmup_done(), "marking is idempotent");
    }

    /// The direct client's timed-out wait (a child that ignores `exit` and keeps stdout open)
    /// aborts the reader too, with the same consequence for requests still waiting.
    #[tokio::test(start_paused = true)]
    async fn direct_shutdown_that_times_out_resolves_a_request_still_in_flight() {
        let reader = tokio::spawn(std::future::pending::<()>());
        let client = bare_client(LspTransport::Process { child_pid: None }, Some(reader));
        let in_flight = park_request(&client);

        client.shutdown().await.unwrap();

        let answer = tokio::time::timeout(std::time::Duration::from_secs(1), in_flight)
            .await
            .expect("a request in flight must be resolved by shutdown, not left to its timeout")
            .expect("answered, not dropped");
        assert!(
            answer.is_err(),
            "the in-flight request fails as disconnected"
        );
    }

    /// A mux never hangs up on a client, so a socket client that waited for its reader task
    /// (which ends only on EOF) burned the whole 5 s, dropped the handle — which DETACHES the
    /// task — and kept the connection open: the mux's idle timer never started while the
    /// process lived. `workspace(post_compact)`, idle eviction and exit all pay this.
    /// This stands in for a mux: it accepts, sends the init message, and reads until EOF.
    /// docs/issues/2026-10-17-symbols-name-lookup-on-kotlin-lsp-says-0-matches-for-a-symbol-that-exists.md
    #[cfg(unix)]
    #[tokio::test]
    async fn socket_client_shutdown_closes_its_connection_promptly_and_sends_no_shutdown() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("mux.sock");
        let listener = tokio::net::UnixListener::bind(&sock).unwrap();
        let (tx, mut seen) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let tx = tx.clone();
                tokio::spawn(async move {
                    let init = r#"{"jsonrpc":"2.0","id":0,"result":{"capabilities":{}}}"#;
                    let framed = format!("Content-Length: {}\r\n\r\n{init}", init.len());
                    stream.write_all(framed.as_bytes()).await.unwrap();
                    // Like a mux: never hang up first; report what arrived once the client does.
                    let mut got = Vec::new();
                    let _ = stream.read_to_end(&mut got).await;
                    let _ = tx.send(got);
                    // Keep our end OPEN past the client's hang-up: a reader task that was merely
                    // detached would stay blocked on this socket instead of ending by itself.
                    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                });
            }
        });

        // Three connect/flush cycles: a leaked connection would show as a missing EOF.
        for cycle in 0..3 {
            let client = LspClient::connect(&sock, dir.path().to_path_buf())
                .await
                .unwrap();
            let started = std::time::Instant::now();
            client.shutdown().await.unwrap();
            assert!(
                started.elapsed() < std::time::Duration::from_secs(2),
                "cycle {cycle}: shutdown must not wait out the reader, took {:?}",
                started.elapsed()
            );
            let got = tokio::time::timeout(std::time::Duration::from_secs(2), seen.recv())
                .await
                .unwrap_or_else(|_| panic!("cycle {cycle}: the mux never saw the client hang up"))
                .unwrap();
            // The client is still alive here: shutdown itself closed the connection.
            let text = String::from_utf8_lossy(&got);
            assert!(
                !text.contains("shutdown") && !text.contains("\"exit\""),
                "a shared server must not be asked to stop, got: {text}"
            );
            // The reader task (and the writer clone it holds) is gone, not detached, as of a
            // moment up to 2 s after `shutdown` returned (the peer's EOF was awaited first). That
            // is the weaker claim: a task merely aborted would also be dropped by then. That it is
            // gone AT return is pinned by `socket_shutdown_has_dropped_the_reader_when_it_returns`.
            assert_eq!(
                Arc::strong_count(&client.pending),
                1,
                "cycle {cycle}: the reader task must not survive shutdown"
            );
            assert_eq!(
                Arc::strong_count(&client.writer),
                1,
                "cycle {cycle}: the reader task's writer clone must not survive shutdown"
            );
        }
    }

    /// A variable-like symbol as pyright reports it: name-only range at `(line, 0)`.
    fn name_only(
        name: &str,
        kind: super::super::symbols::SymbolKind,
        file: &Path,
    ) -> super::super::SymbolInfo {
        super::super::SymbolInfo {
            name: name.into(),
            name_path: name.into(),
            kind,
            file: file.to_path_buf(),
            start_line: 0,
            end_line: 0,
            start_col: 0,
            range_start_line: Some(0),
            children: vec![],
            detail: None,
        }
    }

    #[test]
    fn with_python_ranges_widens_a_multi_line_constant_for_python_only() {
        use super::super::symbols::SymbolKind;
        let dir = tempdir().unwrap();
        let py = dir.path().join("m.py");
        std::fs::write(&py, "MARKERS = (\n    \"a\",\n)\n").unwrap();

        let out = with_python_ranges(
            vec![name_only("MARKERS", SymbolKind::Constant, &py)],
            "python",
            &py,
        );
        assert_eq!(
            out[0].end_line, 2,
            "document_symbols must hand out the whole statement"
        );

        // Same bytes, another language id: the pass is gated on the language, not the text.
        let out = with_python_ranges(
            vec![name_only("MARKERS", SymbolKind::Constant, &py)],
            "rust",
            &py,
        );
        assert_eq!(out[0].end_line, 0);
    }

    #[test]
    fn with_python_ranges_by_file_scopes_each_parse_to_its_own_file_and_keeps_order() {
        use super::super::symbols::SymbolKind;
        let dir = tempdir().unwrap();
        let multi = dir.path().join("a.py");
        let single = dir.path().join("b.py");
        let rust = dir.path().join("c.rs");
        std::fs::write(&multi, "A = (\n    1,\n)\n").unwrap();
        // Load-bearing: `B` sits at the SAME (0, 0) as `A`, so matching it against a.py's
        // parse would widen it. Only the per-file filter keeps it one line.
        std::fs::write(&single, "B = 1\n").unwrap();
        // Load-bearing: these bytes are VALID multi-line Python. A Rust-looking fixture fails to
        // parse as Python, so it stayed one line even with the language filter removed — the
        // test passed either way (review finding). Only the `.rs` extension may exclude it.
        std::fs::write(&rust, "C = (\n    1,\n)\n").unwrap();

        let out = with_python_ranges_by_file(vec![
            name_only("B", SymbolKind::Constant, &single),
            name_only("C", SymbolKind::Constant, &rust),
            name_only("A", SymbolKind::Constant, &multi),
        ]);
        let names: Vec<&str> = out.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            ["B", "C", "A"],
            "workspace/symbol order is the ranking"
        );
        assert_eq!(out[0].end_line, 0, "b.py's `B` is a one-line statement");
        assert_eq!(out[1].end_line, 0, "a .rs file is never parsed as Python");
        assert_eq!(out[2].end_line, 2, "a.py's `A` closes on line 2");
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

    /// `textDocument/rename` may answer `null` (`Option<WorkspaceEdit>` in `lsp-types`). That is
    /// a condition the caller can act on, not a decode failure.
    #[test]
    fn a_null_rename_answer_is_a_recoverable_error_that_names_the_method() {
        let err = decode_rename_answer(Value::Null).expect_err("null is no edit");
        assert!(
            err.downcast_ref::<RecoverableError>().is_some(),
            "must be a RecoverableError so sibling calls survive: {err:#}"
        );
        assert!(
            err.to_string()
                .contains("answered textDocument/rename with null"),
            "got: {err}"
        );
        assert!(
            !err.to_string().contains("invalid type"),
            "must not be the serde message: {err}"
        );
    }

    #[test]
    fn a_real_rename_answer_still_decodes_to_the_edit() {
        let answer = json!({
            "changes": {
                "file:///a.rs": [{
                    "range": {
                        "start": {"line": 0, "character": 7},
                        "end": {"line": 0, "character": 15}
                    },
                    "newText": "renamed_fn"
                }]
            }
        });
        let edit = decode_rename_answer(answer).expect("a WorkspaceEdit decodes");
        assert_eq!(edit.changes.expect("changes").len(), 1);
    }

    #[tokio::test]
    async fn drop_kills_child_process() {
        if !rust_analyzer_available() {
            eprintln!("Skipping: rust-analyzer not installed");
            return;
        }
        // INERT for BOTH deliberate kill paths — do not credit this test with
        // covering either. Measured 2026-09-08 by mutation + strace, and
        // RE-CONFIRMED 2026-09-12 alongside the fix
        // (docs/issues/archive/2026-09-08-drop-kills-child-process-passes-with-both-kill-paths-removed.md):
        // removing `terminate_process` from `Drop`, removing `.kill_on_drop(true)`,
        // and removing BOTH TOGETHER each leave this test green in <0.1s, because a
        // third path nobody wrote reaches the child first — dropping the client
        // closes its stdio pipes and rust-analyzer builds with
        // `unix_sigpipe = "sig_dfl"`, so it dies of SIGPIPE (observed:
        // `+++ killed by SIGPIPE +++` on four threads). What this test actually
        // asserts is that *something* reaps the child, which is true of any LSP
        // server that dies when its client goes away. Adding an assertion here does
        // not fix that; the child has to be one that survives SIGPIPE.
        //
        // KEPT rather than deleted, because "the real server does get reaped" is a
        // claim worth holding and this is the only test that makes it against a
        // real rust-analyzer. The discrimination it cannot do now lives in
        // `drop_sends_sigterm_and_nothing_else_can_reach_the_child` directly below,
        // which reds under the `terminate_process` mutation this one survives.
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

        // Drop sends SIGTERM and never reaps, so the pid stays addressable as a
        // zombie until tokio's orphan reaper collects it and `process_alive` keeps
        // reading true for a window after the kill. Poll that window instead of
        // sleeping a fixed 500ms: a fixed sleep asserts "enough time has passed"
        // and reds when the parallel suite has not scheduled the reaper yet
        // (docs/issues/archive/2026-09-01-peer-idle-timeout-test-is-the-third-load-sensitive-step.md).
        // The loop exits early on death, so the 10s ceiling costs nothing when the
        // reaper is prompt. Deleting the loop restores the load-sensitivity;
        // deleting the assertion below makes the test vacuous.
        for _ in 0..500 {
            if !crate::platform::process_alive(pid) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }

        // Verify child is dead
        assert!(
            !crate::platform::process_alive(pid),
            "child should be dead after drop"
        );
    }

    /// The discriminating twin of `drop_kills_child_process` above, which is inert
    /// for both deliberate kill paths.
    ///
    /// **The child is the fix, not the assertion.** `rust-analyzer` dies of SIGPIPE
    /// the moment its stdio closes, so with it as the child *no* assertion about
    /// liveness can attribute the death to `Drop`. This test removes the other two
    /// mechanisms **by construction** rather than defending against them:
    ///
    /// - **SIGPIPE (path 3) is impossible**: the child is spawned on `Stdio::null()`,
    ///   so there is no pipe whose closure could signal it. Not "survives SIGPIPE" —
    ///   cannot receive it.
    /// - **`kill_on_drop`'s SIGKILL (path 2) is impossible**: the child is spawned
    ///   with `std::process::Command`, so no tokio `Child` exists to carry
    ///   `kill_on_drop`, and no reader task owns it.
    /// - **`Drop`'s `terminate_process` (path 1) is therefore the only mechanism
    ///   left**, which is what makes `WTERMSIG == SIGTERM` attributable rather than
    ///   merely true.
    ///
    /// Built by direct struct construction instead of `LspClient::start`: `start`
    /// performs a real `initialize` handshake, and no child that is immune to the
    /// other two paths also speaks LSP. `reader_handle` is `None` so `Drop`'s abort
    /// is a no-op, leaving exactly one statement under test.
    ///
    /// MUTATIONS, both measured 2026-09-12 against this test AND its inert sibling,
    /// because a kill is only attributable if the two disagree:
    ///
    /// | mutation | this test | `drop_kills_child_process` |
    /// |---|---|---|
    /// | `terminate_process` removed from `Drop` | **RED** (10s timeout) | green, 0.05s |
    /// | `.kill_on_drop(true)` -> `(false)` | green | green, 0.05s |
    ///
    /// Row 1 is the coverage this bug was filed for. **Row 2 is the honest half:
    /// neither test covers `kill_on_drop`, and this one cannot by construction** —
    /// it spawns through `std::process::Command`, so there is no tokio `Child` for
    /// that flag to live on. That site's only non-redundant role is the WIN-5
    /// spawn-timeout path (`LspClient::start`), where no `LspClient` is ever built
    /// and `Drop` therefore never runs; it still has no test, and this file does
    /// not pretend otherwise.
    /// docs/issues/archive/2026-09-08-drop-kills-child-process-passes-with-both-kill-paths-removed.md
    #[cfg(unix)]
    #[tokio::test]
    async fn drop_sends_sigterm_and_nothing_else_can_reach_the_child() {
        use std::os::unix::process::ExitStatusExt;

        // `sleep` never writes to stdout, and `Stdio::null()` means it has no pipe
        // to write to in any case — the two independent reasons path 3 cannot fire.
        let mut child = std::process::Command::new("sleep")
            .arg("300")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn `sleep` as a stand-in child");
        let pid = child.id();

        let client = LspClient {
            writer: Arc::new(Mutex::new(
                Box::new(tokio::io::sink()) as Box<dyn AsyncWrite + Unpin + Send>
            )),
            next_id: AtomicI64::new(1),
            pending: Arc::new(StdMutex::new(HashMap::new())),
            alive: Arc::new(AtomicBool::new(true)),
            reader_handle: StdMutex::new(None),
            workspace_root: std::path::PathBuf::from("/"),
            capabilities: StdMutex::new(lsp_types::ServerCapabilities::default()),
            transport: LspTransport::Process {
                child_pid: Some(pid),
            },
            init_timeout: std::time::Duration::from_secs(30),
            open_files: StdMutex::new(HashMap::new()),
            synced_sigs: SyncedSignatures::default(),
            stderr_lines: Arc::new(StdMutex::new(Vec::new())),
            started_at: std::time::Instant::now(),
            init_completed_at: std::sync::OnceLock::new(),
            progress: Arc::new(ProgressTracker::default()),
            references_warmup_done: AtomicBool::new(false),
            null_runs: NullRuns::default(),
        };

        assert!(
            crate::platform::process_alive(pid),
            "child should be alive before drop"
        );

        drop(client);

        // Poll rather than sleep a fixed span, for the reason the sibling test
        // gives: a fixed sleep asserts "enough time has passed" and reds under
        // parallel-suite load. Nothing here is reaped by tokio, so `try_wait` is
        // ours alone and the status cannot be stolen by the runtime's orphan
        // reaper — which is exactly why this test can read a signal and
        // `drop_kills_child_process` cannot.
        let mut status = None;
        for _ in 0..500 {
            match child.try_wait() {
                Ok(Some(s)) => {
                    status = Some(s);
                    break;
                }
                Ok(None) => {
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
                Err(e) => panic!("try_wait on the stand-in child failed: {e}"),
            }
        }
        let status = status.unwrap_or_else(|| {
            // Leave no stray `sleep` behind when the assertion is about to fail.
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "child survived 10s after drop — `Drop for LspClient` did not signal \
                     it. With no pipes and no tokio `Child`, `terminate_process` is the \
                     only mechanism that could have, so this is that call going missing."
            )
        });

        assert_eq!(
            status.signal(),
            Some(libc::SIGTERM),
            "the child must die of SIGTERM, sent by `Drop`'s `terminate_process`. \
                 A normal exit or a different signal means the death came from \
                 somewhere this test deliberately made unreachable. status: {status:?}"
        );
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
    /// `disk_signature` must notice the change `git checkout` actually makes.
    ///
    /// The interesting case is a **same-length** edit — reverting one character
    /// leaves `len` identical, so if the signature were length-only the drift would
    /// be invisible and `symbols()` would keep serving pre-checkout content. mtime
    /// is forced explicitly rather than relying on wall-clock, or the test would be
    /// flaky on exactly the coarse-granularity filesystems the pair exists for.
    #[test]
    fn disk_signature_detects_a_same_length_change() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f.rs");

        std::fs::write(&file, "fn a() {}\n").unwrap();
        let t0 = filetime::FileTime::from_unix_time(1_600_000_000, 0);
        filetime::set_file_mtime(&file, t0).unwrap();
        let before = disk_signature(&file).unwrap();

        // Same byte count, different content — the `git checkout` shape.
        std::fs::write(&file, "fn b() {}\n").unwrap();
        filetime::set_file_mtime(&file, filetime::FileTime::from_unix_time(1_600_000_060, 0))
            .unwrap();
        let after = disk_signature(&file).unwrap();

        assert_eq!(
            before.0, after.0,
            "fixture must be same-length to be the case"
        );
        assert_ne!(before, after, "a same-length change must still register");

        // Stable when nothing changes: a signature that drifted on its own would
        // fire a redundant didChange on every single navigation call.
        assert_eq!(disk_signature(&file).unwrap(), after);

        // Restoring both content and mtime reproduces the signature exactly. This is
        // the documented residual, asserted rather than left implicit: a same-length
        // edit inside one mtime tick is invisible to this check by design.
        std::fs::write(&file, "fn a() {}\n").unwrap();
        filetime::set_file_mtime(&file, t0).unwrap();
        assert_eq!(
            disk_signature(&file).unwrap(),
            before,
            "same length + same mtime is the accepted blind spot"
        );
    }

    #[test]
    fn disk_signature_is_none_for_a_missing_file() {
        // Drives `resync_all_drifted`'s early return — a path that cannot be stat'd
        // must not be treated as "changed" and trigger an endless resync.
        let dir = tempfile::tempdir().unwrap();
        assert!(disk_signature(&dir.path().join("nope.rs")).is_none());
    }
    /// A repeat `didOpen` must not overwrite the recorded signature.
    ///
    /// This is the test the first attempt at the external-write fix did not have, and
    /// the reason that attempt shipped **inert**. `did_open` recorded unconditionally;
    /// on SOCKET transport it never takes its already-open early return (the mux owns
    /// dedup), so every `document_symbols` re-recorded the current disk state before
    /// the drift check ran — comparing the file against itself. The stdio unit test
    /// passed throughout, because the early return hides the difference. Live
    /// verification on the mux path is what caught it.
    ///
    /// Asserting on `SyncedSignatures` rather than through a transport is deliberate:
    /// it makes the invariant testable without standing up a mux, and the invariant is
    /// transport-independent — a notification that may be deduped must not claim
    /// delivery.
    #[test]
    fn a_repeat_did_open_must_not_overwrite_the_recorded_signature() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f.rs");
        std::fs::write(&file, "fn a() {}\n").unwrap();
        filetime::set_file_mtime(&file, filetime::FileTime::from_unix_time(1_600_000_000, 0))
            .unwrap();

        let sigs = SyncedSignatures::default();
        sigs.record(&file, &file, false); // the didOpen that actually delivered
        let first = sigs.get(&file).expect("didOpen must record on first sight");
        assert!(!sigs.has_drifted(&file), "nothing changed yet");

        // The file changes behind our back, then a repeat didOpen fires — exactly what
        // socket transport does on every document_symbols.
        std::fs::write(&file, "fn a() {}\nfn b() {}\n").unwrap();
        filetime::set_file_mtime(&file, filetime::FileTime::from_unix_time(1_600_000_060, 0))
            .unwrap();
        sigs.record(&file, &file, false);

        assert_eq!(
            sigs.get(&file),
            Some(first),
            "a repeat didOpen delivers nothing, so it must not claim the new content — \
             overwriting here is what made the drift check compare a file against itself"
        );
        assert!(
            sigs.has_drifted(&file),
            "and the drift must still be visible AFTER the repeat didOpen — this is the \
             assertion that fails against the shipped-inert version"
        );

        // didChange DOES always send, so it is allowed to move the mark forward.
        sigs.record(&file, &file, true);
        assert!(
            !sigs.has_drifted(&file),
            "didChange always sends, so it must record what it sent"
        );
    }

    /// A file never delivered is not "drifted" — that is `did_open`'s job.
    ///
    /// Without this, `has_drifted` returning `true` for an unknown path would make the
    /// very first navigation on every file fire a redundant `didChange`.
    #[test]
    fn an_unrecorded_file_has_not_drifted() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("never_sent.rs");
        std::fs::write(&file, "fn a() {}\n").unwrap();

        let sigs = SyncedSignatures::default();
        assert!(!sigs.has_drifted(&file));
        assert!(!sigs.has_drifted(&dir.path().join("does_not_exist.rs")));
    }

    /// `drifted` names every changed file by the path it was SENT under.
    ///
    /// The map is keyed by the canonical path, but the server knows a document by the URI of
    /// the path the client sent. Resyncing a file reached through a symlink with its canonical
    /// path would address a document the server never opened (on macOS `tempdir()` itself is
    /// under a link). Only the changed file is listed, so a resync costs a `didChange` per
    /// changed file and nothing for the rest.
    #[cfg(unix)]
    #[test]
    fn drifted_names_a_changed_file_by_the_path_it_was_sent_under() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let via_link = link.join("f.rs");
        std::fs::write(&via_link, "fn a() {}\n").unwrap();
        filetime::set_file_mtime(
            &via_link,
            filetime::FileTime::from_unix_time(1_600_000_000, 0),
        )
        .unwrap();
        let canonical = std::fs::canonicalize(&via_link).unwrap();
        assert_ne!(canonical, via_link, "the fixture must go through a link");

        let unchanged = real.join("g.rs");
        std::fs::write(&unchanged, "fn g() {}\n").unwrap();
        let unchanged_canonical = std::fs::canonicalize(&unchanged).unwrap();

        let sigs = SyncedSignatures::default();
        sigs.record(&canonical, &via_link, false);
        sigs.record(&unchanged_canonical, &unchanged, false);
        assert!(sigs.drifted().is_empty(), "nothing changed yet");

        std::fs::write(&via_link, "fn a() {}\nfn b() {}\n").unwrap();
        assert_eq!(
            sigs.drifted(),
            vec![via_link],
            "only the changed file, under the path it was sent as"
        );
    }

    /// `document_symbols` must reflect a write made outside codescout's edit tools.
    ///
    /// This replaces `did_change_refreshes_stale_symbol_positions`, which asserted
    /// the OPPOSITE at its step 3 — that a query after an external write returns
    /// stale positions — as a demonstration of why callers had to flush themselves.
    /// That demonstration WAS the bug: `symbols()` kept serving pre-checkout content
    /// after a `git checkout`, and a caller only escaped it by already knowing to
    /// call `post_compact`. `resync_all_drifted` now closes it for every request, so
    /// the old assertion documented a defect rather than a
    /// contract, and was rewritten with it rather than worked around.
    ///
    /// Step 4 keeps the explicit `did_change` path covered: it is still the repair
    /// primitive, and the auto-resync is only its first caller.
    #[tokio::test]
    async fn document_symbols_reflects_a_write_made_outside_codescout() {
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

        // Step 1: fn add sits at some line in the original file.
        let syms = client.document_symbols(&main_rs, "rust").await.unwrap();
        let original_line = syms
            .iter()
            .find(|s| s.name == "add")
            .expect("fn add not found")
            .start_line;

        // Step 2: prepend 3 blank lines on disk, the way `git checkout` or an editor
        // save would — with no notification to the server.
        let original = std::fs::read_to_string(&main_rs).unwrap();
        std::fs::write(&main_rs, format!("\n\n\n{}", original)).unwrap();

        // Step 3: query again with NO explicit flush. The drift must be noticed and
        // repaired before the request goes out.
        let line_after = client
            .document_symbols(&main_rs, "rust")
            .await
            .unwrap()
            .iter()
            .find(|s| s.name == "add")
            .expect("fn add not found")
            .start_line;
        assert_eq!(
            line_after,
            original_line + 3,
            "an external write must be picked up without the caller flushing"
        );

        // Step 4: the explicit primitive still works, and re-sending an already-synced
        // file must not shift anything.
        client.did_change(&main_rs).await.unwrap();
        let line_fresh = client
            .document_symbols(&main_rs, "rust")
            .await
            .unwrap()
            .iter()
            .find(|s| s.name == "add")
            .expect("fn add not found")
            .start_line;
        assert_eq!(
            line_fresh,
            original_line + 3,
            "explicit did_change on an already-synced file must be a no-op"
        );

        client.shutdown().await.unwrap();
    }

    // ── Answers about a file the call does not name, after it changed outside codescout ──
    //
    // Before the fix the drift repair ran only inside `document_symbols`, so it repaired only
    // the file a `documentSymbol` request names. The tests below pin the contract the by-name tools rely
    // on: after the named file is resynced the way those tools do it, no location and no
    // rename edit that the client returns may point into stale bytes of ANOTHER file.
    // Bugs `a73c49456056fb92` (read side) and `58a45860b802a477` (rename writes the stale
    // range). Each `*_is_resynced` twin is the control: it differs from its red test only by
    // one `document_symbols` call on the changed file, so a failure of the red test is the
    // stale view and not the fixture.
    //
    // Scope: the tests in this block use the stdio transport. The socket (mux) transport has its
    // own tests further down: `hover_follows_a_write_made_outside_codescout_through_the_mux`,
    // `two_clients_on_one_mux_follow_a_write_made_outside_codescout` and, ignored and run by
    // hand, `kotlin_hover_follows_a_write_made_outside_codescout_through_the_mux`.

    /// `lib.rs` defines `target_fn` (line 0, name at column 7) and `helper_fn`; `other.rs`
    /// calls `target_fn` on line 1. `target_fn` and `helper_fn` have the same length, so the
    /// swap case below keeps `other.rs` the same size.
    fn create_two_file_project(dir: &Path) {
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"two-files\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("src/lib.rs"),
            "pub fn target_fn(x: u32) -> u32 {\n    x + 1\n}\n\npub fn helper_fn(x: u32) -> u32 {\n    x\n}\n\npub mod other;\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("src/other.rs"),
            "pub fn use_it() -> u32 {\n    crate::target_fn(5)\n}\n",
        )
        .unwrap();
    }

    /// Start a stdio client and sync both files, so the server holds both of them open.
    async fn synced_two_file_client(
        root: &Path,
    ) -> (LspClient, std::path::PathBuf, std::path::PathBuf) {
        let config = LspServerConfig {
            command: "rust-analyzer".into(),
            args: vec![],
            workspace_root: root.to_path_buf(),
            init_timeout: None,
            mux: false,
            env: vec![],
            idle_timeout_secs: None,
        };
        let client = LspClient::start(config).await.unwrap();
        let lib_rs = root.join("src/lib.rs");
        let other_rs = root.join("src/other.rs");
        client.document_symbols(&lib_rs, "rust").await.unwrap();
        client.document_symbols(&other_rs, "rust").await.unwrap();
        (client, lib_rs, other_rs)
    }

    /// The text that `range` covers in the file as it is on disk now. The fixtures are ASCII,
    /// so a UTF-16 column is a character column.
    fn disk_text_at(path: &Path, range: &lsp_types::Range) -> String {
        let content = std::fs::read_to_string(path).unwrap();
        let line = content.lines().nth(range.start.line as usize).unwrap_or("");
        line.chars()
            .skip(range.start.character as usize)
            .take(range.end.character.saturating_sub(range.start.character) as usize)
            .collect()
    }

    /// Ranges of the rename edit that fall in the file whose URI ends with `suffix`.
    fn edit_ranges_in(edit: &lsp_types::WorkspaceEdit, suffix: &str) -> Vec<lsp_types::Range> {
        let mut out = Vec::new();
        if let Some(changes) = &edit.changes {
            for (uri, edits) in changes {
                if uri.as_str().ends_with(suffix) {
                    out.extend(edits.iter().map(|e| e.range));
                }
            }
        }
        if let Some(document_changes) = &edit.document_changes {
            let docs: Vec<&lsp_types::TextDocumentEdit> = match document_changes {
                lsp_types::DocumentChanges::Edits(edits) => edits.iter().collect(),
                lsp_types::DocumentChanges::Operations(ops) => ops
                    .iter()
                    .filter_map(|op| match op {
                        lsp_types::DocumentChangeOperation::Edit(e) => Some(e),
                        _ => None,
                    })
                    .collect(),
            };
            for doc in docs {
                if doc.text_document.uri.as_str().ends_with(suffix) {
                    for e in &doc.edits {
                        out.push(match e {
                            lsp_types::OneOf::Left(te) => te.range,
                            lsp_types::OneOf::Right(ate) => ate.text_edit.range,
                        });
                    }
                }
            }
        }
        out
    }

    /// References of `target_fn`, re-asked for up to 20 s until one lies in `other.rs`. The
    /// server may answer with the declaration alone while it is still loading the crate.
    async fn references_until_other_rs_appears(
        client: &LspClient,
        lib_rs: &Path,
    ) -> Vec<lsp_types::Location> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            let refs = client.references(lib_rs, 0, 7, "rust").await.unwrap();
            let found = refs
                .iter()
                .any(|l| l.uri.as_str().ends_with("/src/other.rs"));
            if found || std::time::Instant::now() >= deadline {
                return refs;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }

    /// Prepend three lines to `other.rs` outside codescout, then ask for the references of
    /// `target_fn` the way the by-name `references` tool does: resync the NAMED file
    /// (`document_symbols`), then ask by position. Every location in `other.rs` must cover
    /// `target_fn` in the file as it is on disk.
    async fn references_after_an_external_prepend(resync_other: bool) {
        if !rust_analyzer_available() {
            eprintln!("Skipping: rust-analyzer not installed");
            return;
        }
        let dir = tempdir().unwrap();
        create_two_file_project(dir.path());
        let (client, lib_rs, other_rs) = synced_two_file_client(dir.path()).await;

        // Baseline: the call is found on line 1 of `other.rs`, so a failure below is the
        // stale step and not the fixture.
        let before = references_until_other_rs_appears(&client, &lib_rs).await;
        let in_other: Vec<_> = before
            .iter()
            .filter(|l| l.uri.as_str().ends_with("/src/other.rs"))
            .collect();
        assert_eq!(
            in_other.len(),
            1,
            "baseline: one call in other.rs, got {before:?}"
        );
        assert_eq!(
            in_other[0].range.start.line, 1,
            "baseline: the call is on line 1"
        );

        let original = std::fs::read_to_string(&other_rs).unwrap();
        std::fs::write(&other_rs, format!("// a\n// b\n// c\n{original}")).unwrap();

        client.document_symbols(&lib_rs, "rust").await.unwrap();
        if resync_other {
            client.document_symbols(&other_rs, "rust").await.unwrap();
        }
        let after = references_until_other_rs_appears(&client, &lib_rs).await;
        let ranges: Vec<_> = after
            .iter()
            .filter(|l| l.uri.as_str().ends_with("/src/other.rs"))
            .map(|l| l.range)
            .collect();
        assert!(
            !ranges.is_empty(),
            "the call in other.rs must still be reported: {after:?}"
        );
        for range in &ranges {
            assert_eq!(
                disk_text_at(&other_rs, range),
                "target_fn",
                "a reference into other.rs must cover `target_fn` on disk, range {range:?}"
            );
        }
        client.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn references_into_another_file_reflect_a_write_made_outside_codescout() {
        references_after_an_external_prepend(false).await;
    }

    #[tokio::test]
    async fn references_into_another_file_are_current_once_that_file_is_resynced() {
        references_after_an_external_prepend(true).await;
    }

    /// How `other.rs` is changed outside codescout in `rename_after_an_external_write`.
    #[derive(Clone, Copy)]
    enum ExternalWrite {
        /// Three comment lines in front: the call moves from line 1 to line 4.
        PrependThreeLines,
        /// The call of `target_fn` becomes a call of `helper_fn`, a name of the same length,
        /// so the file keeps its size and the stale range lands on another call.
        SwapForASameLengthCallee,
    }

    /// `rename` is deliberately not retried by the client (`is_idempotent_lsp_method`), so a
    /// server that cancels it while it is busy surfaces as a `RecoverableError`. A server that is
    /// still working through a file change can also answer `null`, which `LspClient::rename`
    /// cannot decode (`invalid type: null, expected struct WorkspaceEdit`): measured 2026-11-12,
    /// 2 of 15 full-suite runs of `rename_edit_does_not_land_on_another_callee_…`, each after
    /// 0.6 s. These tests check the CONTENT of the edit, so they ask again, for up to 20 s, on
    /// exactly those two answers and on no other error. The retry does not hide a stale view: a
    /// stale server answers with an edit whose ranges are wrong, and the caller's assertions
    /// still fail on it.
    async fn rename_retrying_while_the_server_is_busy(
        client: &LspClient,
        lib_rs: &Path,
        new_name: &str,
    ) -> lsp_types::WorkspaceEdit {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            match client.rename(lib_rs, 0, 7, new_name, "rust").await {
                Ok(edit) => return edit,
                Err(e)
                    if e.to_string().contains("cancelled non-idempotent request")
                        || e.to_string()
                            .contains("answered textDocument/rename with null") =>
                {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "the server gave no rename edit within 20 s: {e:#}"
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
                Err(e) => panic!("rename failed: {e:#}"),
            }
        }
    }

    /// Change `other.rs` outside codescout, resync the NAMED file, ask for a rename of
    /// `target_fn`. Every edit range in `other.rs` must cover `target_fn` on disk.
    /// `edit_code rename` writes the ranges the server returns without comparing them with
    /// the old name (`apply_text_edits`, `symbol/edit.rs`).
    async fn rename_after_an_external_write(write: ExternalWrite, resync_other: bool) {
        if !rust_analyzer_available() {
            eprintln!("Skipping: rust-analyzer not installed");
            return;
        }
        let dir = tempdir().unwrap();
        create_two_file_project(dir.path());
        let (client, lib_rs, other_rs) = synced_two_file_client(dir.path()).await;
        // Same wait as the references tests: the server must know the call before the write.
        let before = references_until_other_rs_appears(&client, &lib_rs).await;
        assert!(
            before
                .iter()
                .any(|l| l.uri.as_str().ends_with("/src/other.rs")),
            "baseline: the call in other.rs must be known to the server"
        );

        let original = std::fs::read_to_string(&other_rs).unwrap();
        let rewritten = match write {
            ExternalWrite::PrependThreeLines => format!("// a\n// b\n// c\n{original}"),
            ExternalWrite::SwapForASameLengthCallee => original.replace("target_fn", "helper_fn"),
        };
        if let ExternalWrite::SwapForASameLengthCallee = write {
            assert_eq!(
                rewritten.len(),
                original.len(),
                "the swap must keep the size"
            );
        }
        std::fs::write(&other_rs, rewritten).unwrap();

        client.document_symbols(&lib_rs, "rust").await.unwrap();
        if resync_other {
            client.document_symbols(&other_rs, "rust").await.unwrap();
        }
        let edit = rename_retrying_while_the_server_is_busy(&client, &lib_rs, "renamed_fn").await;

        assert!(
            !edit_ranges_in(&edit, "/src/lib.rs").is_empty(),
            "the rename must edit lib.rs, or the check below proves nothing: {edit:?}"
        );
        if let ExternalWrite::PrependThreeLines = write {
            assert!(
                !edit_ranges_in(&edit, "/src/other.rs").is_empty(),
                "other.rs still calls target_fn, so the rename must edit it: {edit:?}"
            );
        }
        for range in edit_ranges_in(&edit, "/src/other.rs") {
            assert_eq!(
                disk_text_at(&other_rs, &range),
                "target_fn",
                "a rename edit for other.rs must cover `target_fn` on disk, range {range:?}"
            );
        }
        client.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn rename_edit_for_another_file_reflects_a_prepend_made_outside_codescout() {
        rename_after_an_external_write(ExternalWrite::PrependThreeLines, false).await;
    }

    #[tokio::test]
    async fn rename_edit_for_another_file_is_current_after_a_prepend_once_it_is_resynced() {
        rename_after_an_external_write(ExternalWrite::PrependThreeLines, true).await;
    }

    #[tokio::test]
    async fn rename_edit_does_not_land_on_another_callee_after_a_same_size_write_outside_codescout()
    {
        rename_after_an_external_write(ExternalWrite::SwapForASameLengthCallee, false).await;
    }

    #[tokio::test]
    async fn rename_edit_does_not_land_on_another_callee_after_a_same_size_write_once_resynced() {
        rename_after_an_external_write(ExternalWrite::SwapForASameLengthCallee, true).await;
    }
    /// Ask until the call answers without an error. Used for `connect`, where the first tries can
    /// fail while the mux finishes its handshake.
    async fn retrying_on_error<T, F, Fut>(mut ask: F) -> T
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut last = None;
        for _ in 0..10 {
            match ask().await {
                Ok(value) => return value,
                Err(e) => {
                    last = Some(e);
                    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                }
            }
        }
        panic!("the server kept answering with an error: {last:?}");
    }

    /// Ask until the server answers with something, for up to 20 s: an error is retried, and so is
    /// an answer `is_empty` calls empty. Right after a write outside codescout the server may
    /// answer a request with `ContentModified`, or with nothing while it analyzes the new text.
    /// Measured 2026-11-12 under one busy process per core: the first ask after the write came
    /// back empty in 5 test runs spread over 4 of 20 stress iterations of the stdio tests (hover,
    /// goto_definition and prepare_call_hierarchy, run together), and in none of 20 iterations
    /// with the CPU idle. The mux test passed all 20 loaded iterations.
    ///
    /// This does not let the defect under test through. With the resync in `request` disabled the
    /// answer does not turn correct after a while. Measured, it stays `Invalid offset LineCol {
    /// line: 4, col: 11 } (line index length: 51)` on rust-analyzer, `column is beyond end of file`
    /// on gopls, and empty on pyright and typescript-language-server, for the whole window,
    /// because the server keeps the content of an open file from the last `didOpen`/`didChange`
    /// and ignores the disk. The tests were re-run under that mutation after this helper was
    /// written, to check it.
    ///
    /// On the deadline it returns the last answer, so the caller's assertion shows an empty
    /// result; if every try was an error it panics with the last error.
    async fn answered_after_a_write<T, F, Fut>(mut ask: F, is_empty: impl Fn(&T) -> bool) -> T
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let mut last_answer = None;
        let mut last_error = None;
        loop {
            match ask().await {
                Ok(value) if !is_empty(&value) => return value,
                Ok(value) => last_answer = Some(value),
                Err(e) => last_error = Some(e),
            }
            if std::time::Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }
        match last_answer {
            Some(value) => value,
            None => panic!("the server kept answering with an error: {last_error:?}"),
        }
    }

    /// `goto_definition` on the call in `other.rs`, re-asked for up to 20 s until the server answers
    /// with a location. Used only for the baseline, while the server may still be loading the crate.
    async fn definition_until_found(
        client: &LspClient,
        other_rs: &Path,
        line: u32,
    ) -> Vec<lsp_types::Location> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            let found = client
                .goto_definition(other_rs, line, 11, "rust")
                .await
                .unwrap();
            if !found.is_empty() || std::time::Instant::now() >= deadline {
                return found;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }

    /// After three lines are prepended to `other.rs` outside codescout, `goto_definition` on the call
    /// at its NEW position must reach `target_fn` in `lib.rs`. The call is asked once and not polled:
    /// rust-analyzer watches files itself and would, given time, mask a stale answer. Before the
    /// resync in `request`, the server still held the three-line file and answered every try with
    /// `Invalid offset LineCol { line: 4, col: 11 } (line index length: 51)` (bug
    /// `a73c49456056fb92`).
    #[tokio::test]
    async fn goto_definition_follows_a_write_made_outside_codescout() {
        if !rust_analyzer_available() {
            eprintln!("Skipping: rust-analyzer not installed");
            return;
        }
        let dir = tempdir().unwrap();
        create_two_file_project(dir.path());
        let (client, lib_rs, other_rs) = synced_two_file_client(dir.path()).await;

        // Baseline: the call is on line 1 and resolves to `target_fn` in `lib.rs`, so a failure
        // below is the stale step and not the fixture.
        let before = definition_until_found(&client, &other_rs, 1).await;
        assert_eq!(before.len(), 1, "baseline: one definition, got {before:?}");
        assert_eq!(disk_text_at(&lib_rs, &before[0].range), "target_fn");

        let original = std::fs::read_to_string(&other_rs).unwrap();
        std::fs::write(&other_rs, format!("// a\n// b\n// c\n{original}")).unwrap();

        let after = answered_after_a_write(
            || client.goto_definition(&other_rs, 4, 11, "rust"),
            |locations| locations.is_empty(),
        )
        .await;
        assert_eq!(
            after.len(),
            1,
            "the call is on line 4 now and must still resolve: {after:?}"
        );
        assert!(
            after[0].uri.as_str().ends_with("/src/lib.rs"),
            "the definition is in lib.rs: {after:?}"
        );
        assert_eq!(
            disk_text_at(&lib_rs, &after[0].range),
            "target_fn",
            "the definition must cover `target_fn` on disk, range {:?}",
            after[0].range
        );
        client.shutdown().await.unwrap();
    }

    /// The same write, asked with `hover`: at the call's new position the answer must describe
    /// `target_fn`. Before the resync in `request`, the position lay past the end of the file the
    /// server still held, and `hover` failed with the same `Invalid offset` error.
    #[tokio::test]
    async fn hover_follows_a_write_made_outside_codescout() {
        if !rust_analyzer_available() {
            eprintln!("Skipping: rust-analyzer not installed");
            return;
        }
        let dir = tempdir().unwrap();
        create_two_file_project(dir.path());
        let (client, _lib_rs, other_rs) = synced_two_file_client(dir.path()).await;

        // Baseline, polled while the server may still be loading the crate.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let before = loop {
            let text = client.hover(&other_rs, 1, 11, "rust").await.unwrap();
            if text.is_some() || std::time::Instant::now() >= deadline {
                break text;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        };
        assert!(
            before.as_deref().is_some_and(|t| t.contains("target_fn")),
            "baseline: hover on the call describes target_fn, got {before:?}"
        );

        let original = std::fs::read_to_string(&other_rs).unwrap();
        std::fs::write(&other_rs, format!("// a\n// b\n// c\n{original}")).unwrap();

        let after = answered_after_a_write(
            || client.hover(&other_rs, 4, 11, "rust"),
            |text| text.is_none(),
        )
        .await;
        assert!(
            after.as_deref().is_some_and(|t| t.contains("target_fn")),
            "hover at the call's new position must describe target_fn, got {after:?}"
        );
        client.shutdown().await.unwrap();
    }
    /// The same write, asked with `prepare_call_hierarchy` at the raw position of the call. This is
    /// the entry point of `call_graph` and `references`. After the write the call sits three lines
    /// lower, so the position must resolve to the item for `target_fn`, not to nothing and not to an
    /// error from a server that still holds the old file.
    #[tokio::test]
    async fn prepare_call_hierarchy_follows_a_write_made_outside_codescout() {
        if !rust_analyzer_available() {
            eprintln!("Skipping: rust-analyzer not installed");
            return;
        }
        let dir = tempdir().unwrap();
        create_two_file_project(dir.path());
        let (client, _lib_rs, other_rs) = synced_two_file_client(dir.path()).await;

        // Baseline, polled while the server may still be loading the crate.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let before = loop {
            let item = client
                .prepare_call_hierarchy(&other_rs, 1, 11, "rust")
                .await
                .unwrap();
            if item.is_some() || std::time::Instant::now() >= deadline {
                break item;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        };
        assert_eq!(
            before.as_ref().map(|i| i.name.as_str()),
            Some("target_fn"),
            "baseline: the call resolves to target_fn, got {before:?}"
        );

        let original = std::fs::read_to_string(&other_rs).unwrap();
        std::fs::write(&other_rs, format!("// a\n// b\n// c\n{original}")).unwrap();

        let after = answered_after_a_write(
            || client.prepare_call_hierarchy(&other_rs, 4, 11, "rust"),
            |item| item.is_none(),
        )
        .await;
        assert_eq!(
            after.as_ref().map(|i| i.name.as_str()),
            Some("target_fn"),
            "the call's new position must resolve to target_fn, got {after:?}"
        );
        client.shutdown().await.unwrap();
    }

    /// The same write, with the client on the SOCKET transport: `mux::process::run` runs in this
    /// process with a real rust-analyzer behind it, and the client connects to its socket the way
    /// the codescout server does by default. A test cannot go through `LspManager::get_or_start`
    /// for this: in a test runner it cannot spawn the mux (the test binary has no `mux`
    /// subcommand) and falls back to a direct server, which is a stdio client again.
    #[cfg(unix)]
    #[tokio::test]
    async fn hover_follows_a_write_made_outside_codescout_through_the_mux() {
        if !rust_analyzer_available() {
            eprintln!("Skipping: rust-analyzer not installed");
            return;
        }
        let dir = tempdir().unwrap();
        create_two_file_project(dir.path());
        let sock_dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let (mux, socket) = in_process_mux(&root, sock_dir.path()).await;
        let client = retrying_on_error(|| LspClient::connect(&socket, root.clone())).await;
        let lib_rs = root.join("src/lib.rs");
        let other_rs = root.join("src/other.rs");
        client.document_symbols(&lib_rs, "rust").await.unwrap();
        client.document_symbols(&other_rs, "rust").await.unwrap();

        // Baseline, polled while the server may still be loading the crate.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let before = loop {
            let text = client.hover(&other_rs, 1, 11, "rust").await.unwrap();
            if text.is_some() || std::time::Instant::now() >= deadline {
                break text;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        };
        assert!(
            before.as_deref().is_some_and(|t| t.contains("target_fn")),
            "baseline: hover on the call describes target_fn, got {before:?}"
        );

        let original = std::fs::read_to_string(&other_rs).unwrap();
        std::fs::write(&other_rs, format!("// a\n// b\n// c\n{original}")).unwrap();

        let after = answered_after_a_write(
            || client.hover(&other_rs, 4, 11, "rust"),
            |text| text.is_none(),
        )
        .await;
        assert!(
            after.as_deref().is_some_and(|t| t.contains("target_fn")),
            "hover through the mux at the call's new position must describe target_fn, got {after:?}"
        );
        client.shutdown().await.unwrap();
        // Dropping the mux's future drops its server child (`kill_on_drop`).
        mux.abort();
    }
    /// Kotlin is the language that goes through the mux in production, and `kotlin-lsp` is a JVM
    /// server: it keeps an open file's text from the last `didOpen`/`didChange` like the others, so a
    /// write made outside codescout must be resent before the next request. The test runs the mux in
    /// this process with the production config (`default_config("kotlin")`: the 2 GiB heap cap,
    /// `--system-path`, the codescout-owned `user.home`), connects the socket client, writes
    /// `other.kt` outside the client and asks for hover at the call's new position.
    ///
    /// It is `#[ignore]`d because `kotlin-lsp` has used 35 GB of native memory on a small project
    /// (`docs/issues/archive/2026-06-19-kotlin-lsp-uncapped-jvm-heap.md`), and no gate lane caps
    /// memory. Run it by hand, under a cgroup limit so the kernel kills it instead of the host:
    ///
    /// ```text
    /// cargo test --lib --no-run   # note the test binary path
    /// systemd-run --user --scope -p MemoryMax=6G -p MemorySwapMax=0 \
    ///     <test binary> --ignored kotlin_hover_follows_a_write_made_outside_codescout_through_the_mux
    /// ```
    #[cfg(unix)]
    #[tokio::test]
    #[ignore = "starts a real kotlin-lsp (a JVM): run it under a memory cap, see the doc comment"]
    async fn kotlin_hover_follows_a_write_made_outside_codescout_through_the_mux() {
        if !on_path("kotlin-lsp") {
            eprintln!("Skipping: kotlin-lsp not installed");
            return;
        }
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let src = root.join("src/main/kotlin");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            src.join("Lib.kt"),
            "package demo\n\nfun targetFn(): Int = 1\n",
        )
        .unwrap();
        // `kotlin-lsp` answers nothing for a bare folder (measured: no hover for 240 s), so the project
        // is a Gradle one, with the plugin and distribution versions that `~/.gradle` already holds.
        std::fs::write(
            root.join("settings.gradle.kts"),
            "rootProject.name = \"demo\"\n",
        )
        .unwrap();
        std::fs::write(
        root.join("build.gradle.kts"),
        "plugins {\n    kotlin(\"jvm\") version \"2.1.20\"\n}\n\nrepositories {\n    mavenCentral()\n}\n",
    )
    .unwrap();
        std::fs::create_dir_all(root.join("gradle/wrapper")).unwrap();
        std::fs::write(
            root.join("gradle/wrapper/gradle-wrapper.properties"),
            "distributionBase=GRADLE_USER_HOME\ndistributionPath=wrapper/dists\n\
         distributionUrl=https\\://services.gradle.org/distributions/gradle-8.14.3-bin.zip\n\
         zipStoreBase=GRADLE_USER_HOME\nzipStorePath=wrapper/dists\n",
        )
        .unwrap();
        let other_kt = src.join("Other.kt");
        std::fs::write(
            &other_kt,
            "package demo\n\nfun caller(): Int {\n    return targetFn()\n}\n",
        )
        .unwrap();

        let cfg = crate::lsp::servers::default_config("kotlin", &root).unwrap();
        let args: Vec<String> = cfg.args.clone();
        // The production config gives each workspace a fresh `GRADLE_USER_HOME` under the temp dir, which
        // would download the distribution and the plugin. This manual test reuses `~/.gradle` instead.
        let gradle_home =
            std::path::PathBuf::from(std::env::var_os("HOME").unwrap()).join(".gradle");
        let env: Vec<(String, String)> = cfg
            .env
            .iter()
            .map(|(k, v)| match k.as_str() {
                "GRADLE_USER_HOME" => (k.clone(), gradle_home.to_string_lossy().to_string()),
                _ => (k.clone(), v.clone()),
            })
            .collect();
        let sock_dir = tempdir().unwrap();
        let (mux, socket) =
            in_process_mux_for(&root, sock_dir.path(), &cfg.command, &args, &env, 300).await;
        let client = retrying_on_error(|| LspClient::connect(&socket, root.clone())).await;
        client
            .document_symbols(&src.join("Lib.kt"), "kotlin")
            .await
            .unwrap();
        client.document_symbols(&other_kt, "kotlin").await.unwrap();

        // Baseline, polled while the server imports the Gradle project.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(420);
        let before = loop {
            let text = client.hover(&other_kt, 3, 11, "kotlin").await.unwrap();
            if text.is_some() || std::time::Instant::now() >= deadline {
                break text;
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        };

        let outcome = if before.as_deref().is_some_and(|t| t.contains("targetFn")) {
            let original = std::fs::read_to_string(&other_kt).unwrap();
            std::fs::write(&other_kt, format!("// a\n// b\n// c\n{original}")).unwrap();
            Some(
                answered_after_a_write(
                    || client.hover(&other_kt, 6, 11, "kotlin"),
                    |text| text.is_none(),
                )
                .await,
            )
        } else {
            None
        };

        // Cleanup before asserting, so a failed run does not leave a JVM and its index behind.
        let _ = client.shutdown().await;
        mux.abort();
        let ws_hash = crate::lsp::mux::workspace_hash(&root);
        let _ = std::fs::remove_dir_all(crate::lsp::servers::kotlin_analyzer_home(&ws_hash));
        let tmp = std::env::temp_dir();
        let _ = std::fs::remove_dir_all(tmp.join(format!("codescout-mux-kotlin-lsp-{ws_hash}")));

        assert!(
            before.as_deref().is_some_and(|t| t.contains("targetFn")),
            "baseline: hover on the call describes targetFn, got {before:?}"
        );
        let after = outcome.unwrap();
        assert!(
            after.as_deref().is_some_and(|t| t.contains("targetFn")),
            "hover through the mux at the call's new position must describe targetFn, got {after:?}"
        );
    }

    /// Start `mux::process::run` in this process, with a real rust-analyzer behind it, and wait until
    /// it has bound its socket. Aborting the returned task drops the server child (`kill_on_drop`).
    #[cfg(unix)]
    async fn in_process_mux(
        root: &Path,
        sock_dir: &Path,
    ) -> (tokio::task::JoinHandle<Result<()>>, std::path::PathBuf) {
        in_process_mux_for(root, sock_dir, "rust-analyzer", &[], &[], 30).await
    }

    /// `in_process_mux` for any server: `bind_secs` is how long the mux may take to start the server
    /// and bind its socket (a JVM server needs minutes where rust-analyzer needs seconds).
    #[cfg(unix)]
    async fn in_process_mux_for(
        root: &Path,
        sock_dir: &Path,
        command: &str,
        args: &[String],
        env: &[(String, String)],
        bind_secs: u64,
    ) -> (tokio::task::JoinHandle<Result<()>>, std::path::PathBuf) {
        let socket = sock_dir.join("mux.sock");
        let lock = sock_dir.join("mux.lock");
        let handle = {
            let (socket, lock, root) = (socket.clone(), lock.clone(), root.to_path_buf());
            let (command, args, env) = (command.to_string(), args.to_vec(), env.to_vec());
            tokio::spawn(async move {
                crate::lsp::mux::process::run(&socket, &lock, &root, 120, &command, &args, &env)
                    .await
            })
        };
        // The mux binds its socket after it has started the server.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(bind_secs);
        while !socket.exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "the mux did not bind {} within {bind_secs} s",
                socket.display()
            );
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        (handle, socket)
    }

    /// Two clients on ONE mux, the way two sessions share a workspace. A write outside codescout
    /// drifts both clients' view of `other.rs`. B asks first, so B's resync reaches the server
    /// before A has asked at all. Then A asks: its own resync re-sends content the mux already
    /// forwarded for B. Last, B disconnects and a second write is made, and A must still follow it.
    ///
    /// What it detects: the client no longer resyncing before a request (measured: red, `Invalid
    /// offset … (line index length: 51)`). What it does NOT detect, both measured by mutating the
    /// mux in a scratch copy and running this test: `DocumentState::disconnect` closing every file
    /// B had open, and `handle_client_message` no longer rewriting the `didChange` version. Both
    /// leave it green, the first probably because rust-analyzer reads a closed file from disk
    /// (not measured). The first is pinned by the unit tests in `mux/protocol.rs`; the second by
    /// nothing found.
    #[cfg(unix)]
    #[tokio::test]
    async fn two_clients_on_one_mux_follow_a_write_made_outside_codescout() {
        if !rust_analyzer_available() {
            eprintln!("Skipping: rust-analyzer not installed");
            return;
        }
        let dir = tempdir().unwrap();
        create_two_file_project(dir.path());
        let sock_dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let (mux, socket) = in_process_mux(&root, sock_dir.path()).await;
        let a = retrying_on_error(|| LspClient::connect(&socket, root.clone())).await;
        let b = retrying_on_error(|| LspClient::connect(&socket, root.clone())).await;
        let lib_rs = root.join("src/lib.rs");
        let other_rs = root.join("src/other.rs");
        for client in [&a, &b] {
            client.document_symbols(&lib_rs, "rust").await.unwrap();
            client.document_symbols(&other_rs, "rust").await.unwrap();
        }

        // Baseline through A, polled while the server may still be loading the crate.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let before = loop {
            let text = a.hover(&other_rs, 1, 11, "rust").await.unwrap();
            if text.is_some() || std::time::Instant::now() >= deadline {
                break text;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        };
        assert!(
            before.as_deref().is_some_and(|t| t.contains("target_fn")),
            "baseline: hover on the call describes target_fn, got {before:?}"
        );

        let original = std::fs::read_to_string(&other_rs).unwrap();
        std::fs::write(&other_rs, format!("// a\n// b\n// c\n{original}")).unwrap();

        let by_b =
            answered_after_a_write(|| b.hover(&other_rs, 4, 11, "rust"), |text| text.is_none())
                .await;
        assert!(
            by_b.as_deref().is_some_and(|t| t.contains("target_fn")),
            "B, asking first after the write, must get target_fn, got {by_b:?}"
        );
        let by_a =
            answered_after_a_write(|| a.hover(&other_rs, 4, 11, "rust"), |text| text.is_none())
                .await;
        assert!(
            by_a.as_deref().is_some_and(|t| t.contains("target_fn")),
            "A, asking second after the write, must get target_fn, got {by_a:?}"
        );

        b.shutdown().await.unwrap();
        let once_written = std::fs::read_to_string(&other_rs).unwrap();
        std::fs::write(&other_rs, format!("// d\n// e\n// f\n{once_written}")).unwrap();
        let after_b_left =
            answered_after_a_write(|| a.hover(&other_rs, 7, 11, "rust"), |text| text.is_none())
                .await;
        assert!(
            after_b_left
                .as_deref()
                .is_some_and(|t| t.contains("target_fn")),
            "A, after B left and a second write, must get target_fn, got {after_b_left:?}"
        );
        a.shutdown().await.unwrap();
        mux.abort();
    }

    /// True when an executable called `name` is in a `PATH` directory.
    fn on_path(name: &str) -> bool {
        std::env::var_os("PATH")
            .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(name).is_file()))
    }

    /// One language's fixture for the "write made outside codescout" hover test on stdio. `caller`
    /// calls the function `needle` names, at `(call_line, call_col)`; `comment` starts a one-line
    /// comment in that language.
    struct OutsideWriteCase {
        language: &'static str,
        server_binary: &'static str,
        files: &'static [(&'static str, &'static str)],
        caller: &'static str,
        call_line: u32,
        call_col: u32,
        comment: &'static str,
        needle: &'static str,
    }

    /// Start the language's default server, ask `hover` on the call, prepend three comment lines to
    /// the caller from outside codescout, and ask once more at the call's new position (three lines
    /// lower). The hover must still describe `needle`. The first hover waits for the server to load
    /// the project (up to 60 s); the second goes through `answered_after_a_write`.
    async fn hover_follows_a_write_made_outside_codescout_for(case: OutsideWriteCase) {
        if !on_path(case.server_binary) {
            eprintln!("Skipping: {} not installed", case.server_binary);
            return;
        }
        let dir = tempdir().unwrap();
        for (rel, text) in case.files {
            std::fs::write(dir.path().join(rel), text).unwrap();
        }
        let mut config = crate::lsp::servers::default_config(case.language, dir.path()).unwrap();
        config.mux = false;
        let client = LspClient::start(config).await.unwrap();
        let caller = dir.path().join(case.caller);
        for (rel, _) in case.files {
            let path = dir.path().join(rel);
            if path.extension() == caller.extension() {
                client.document_symbols(&path, case.language).await.unwrap();
            }
        }

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        let before = loop {
            let text = client
                .hover(&caller, case.call_line, case.call_col, case.language)
                .await
                .unwrap();
            if text.is_some() || std::time::Instant::now() >= deadline {
                break text;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        };
        assert!(
            before.as_deref().is_some_and(|t| t.contains(case.needle)),
            "baseline ({}): hover on the call describes {}, got {before:?}",
            case.language,
            case.needle
        );

        let original = std::fs::read_to_string(&caller).unwrap();
        let c = case.comment;
        std::fs::write(&caller, format!("{c}a\n{c}b\n{c}c\n{original}")).unwrap();

        let after = answered_after_a_write(
            || client.hover(&caller, case.call_line + 3, case.call_col, case.language),
            |text| text.is_none(),
        )
        .await;
        assert!(
            after.as_deref().is_some_and(|t| t.contains(case.needle)),
            "({}) hover at the call's new position must describe {}, got {after:?}",
            case.language,
            case.needle
        );
        client.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn python_hover_follows_a_write_made_outside_codescout() {
        hover_follows_a_write_made_outside_codescout_for(OutsideWriteCase {
            language: "python",
            server_binary: "pyright-langserver",
            files: &[
                ("lib.py", "def target_fn(x: int) -> int:\n    return x + 1\n"),
                (
                    "other.py",
                    "from lib import target_fn\n\n\ndef use_it() -> int:\n    return target_fn(5)\n",
                ),
            ],
            caller: "other.py",
            call_line: 4,
            call_col: 11,
            comment: "# ",
            needle: "target_fn",
        })
        .await;
    }

    #[tokio::test]
    async fn typescript_hover_follows_a_write_made_outside_codescout() {
        hover_follows_a_write_made_outside_codescout_for(OutsideWriteCase {
            language: "typescript",
            server_binary: "typescript-language-server",
            files: &[
                (
                    "tsconfig.json",
                    "{\"compilerOptions\":{\"strict\":true,\"module\":\"esnext\",\"moduleResolution\":\"node\",\"target\":\"es2020\"},\"include\":[\"*.ts\"]}\n",
                ),
                (
                    "lib.ts",
                    "export function target_fn(x: number): number {\n  return x + 1;\n}\n",
                ),
                (
                    "other.ts",
                    "import { target_fn } from \"./lib\";\n\nexport function useIt(): number {\n  return target_fn(5);\n}\n",
                ),
            ],
            caller: "other.ts",
            call_line: 3,
            call_col: 9,
            comment: "// ",
            needle: "target_fn",
        })
        .await;
    }

    #[tokio::test]
    async fn go_hover_follows_a_write_made_outside_codescout() {
        hover_follows_a_write_made_outside_codescout_for(OutsideWriteCase {
            language: "go",
            server_binary: "gopls",
            files: &[
                ("go.mod", "module example.com/twofiles\n\ngo 1.21\n"),
                (
                    "lib.go",
                    "package twofiles\n\nfunc TargetFn(x int) int {\n\treturn x + 1\n}\n",
                ),
                (
                    "other.go",
                    "package twofiles\n\nfunc UseIt() int {\n\treturn TargetFn(5)\n}\n",
                ),
            ],
            caller: "other.go",
            call_line: 3,
            call_col: 8,
            comment: "// ",
            needle: "TargetFn",
        })
        .await;
    }

    /// Verify that prepare_call_hierarchy returns a CallHierarchyItem for a known function.
    ///
    /// This test requires a live rust-analyzer. It is skipped automatically when
    /// rust-analyzer is not installed so the normal `cargo test` suite stays green.
    ///
    /// Note: `prepareCallHierarchy` requires semantic analysis (not just parsing), so the
    /// server may return `None` if indexing hasn't completed. We treat that as a soft skip
    /// rather than a hard failure to avoid a flaky test. The assertion fires only when we
    /// do get an item back, ensuring the name and kind are correct.
    #[tokio::test]
    async fn call_hierarchy_prepare_returns_item() {
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

        // Warm up rust-analyzer by fetching document symbols first.
        // This ensures the file is parsed before we send the call hierarchy request.
        let syms = client.document_symbols(&main_rs, "rust").await.unwrap();
        let add_sym = syms.iter().find(|s| s.name == "add");
        let Some(add_sym) = add_sym else {
            eprintln!("Skipping: fn add not found in symbols (indexing lag)");
            client.shutdown().await.unwrap();
            return;
        };

        // start_line and start_col from selectionRange point directly at the name.
        // prepareCallHierarchy needs semantic analysis; it may return None if not
        // yet indexed — treated as a soft skip, not a test failure.
        let item = client
            .prepare_call_hierarchy(&main_rs, add_sym.start_line, add_sym.start_col, "rust")
            .await
            .expect("prepare_call_hierarchy should not error");

        match item {
            None => {
                eprintln!(
                    "Skipping: prepare_call_hierarchy returned None for fn add \
                     (line={}, col={}) — indexing not yet complete",
                    add_sym.start_line, add_sym.start_col
                );
            }
            Some(item) => {
                assert_eq!(
                    item.name, "add",
                    "expected item name 'add', got '{}'",
                    item.name
                );
            }
        }

        client.shutdown().await.unwrap();
    }

    /// Verify that outgoing_calls returns calls made from a function.
    #[tokio::test]
    async fn call_hierarchy_outgoing_returns_calls() {
        if !rust_analyzer_available() {
            eprintln!("Skipping: rust-analyzer not installed");
            return;
        }

        // Extend the test project with a helper that calls `add`.
        let dir = tempdir().unwrap();
        create_test_cargo_project(dir.path());
        let main_rs = dir.path().join("src/main.rs");

        // Append a wrapper that calls `add` so outgoing_calls has something to find.
        let original = std::fs::read_to_string(&main_rs).unwrap();
        std::fs::write(
            &main_rs,
            format!("{original}\nfn add_twice(x: i32) -> i32 {{ add(x, x) }}\n"),
        )
        .unwrap();

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

        // Warm up: fetch document symbols to ensure the file is parsed.
        let syms = client.document_symbols(&main_rs, "rust").await.unwrap();
        let add_twice = syms.iter().find(|s| s.name == "add_twice");
        let Some(sym) = add_twice else {
            eprintln!("Skipping: add_twice not found in symbols (indexing lag)");
            client.shutdown().await.unwrap();
            return;
        };
        // Use selectionRange coords directly — they point at the function name.
        let item = client
            .prepare_call_hierarchy(&main_rs, sym.start_line, sym.start_col, "rust")
            .await
            .expect("prepare_call_hierarchy should not error");

        if let Some(item) = item {
            let calls = client
                .outgoing_calls(&item, "rust")
                .await
                .expect("outgoing_calls should not error");
            // add_twice calls add — expect at least one outgoing call.
            assert!(
                !calls.is_empty(),
                "expected outgoing calls from add_twice, got none"
            );
        } else {
            eprintln!("Skipping: prepare_call_hierarchy returned None (indexing lag)");
        }

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

    /// A scripted LSP peer on a Unix socket, served by `LspClient::connect`: it answers
    /// the first `nulls` `textDocument/documentSymbol` requests with a successful `null`
    /// and every later one with a single `alpha` function. Returns the socket path, the
    /// documentSymbol request counter, and the tempdir that owns both.
    ///
    /// This is rust-analyzer's real behaviour, measured 2026-09-24 against a cold 1.97.1
    /// server on this repo: it answers `null` — a SUCCESS, not an error, so the -32800 /
    /// -32801 retry in `request` never sees it — for every open file while it swaps its
    /// crate graph (`Building CrateGraph` → `Roots Scanned`), 0.2–0.7 s per swap, twice
    /// during start-up (`docs/issues/archive/2026-08-27-references-symbol-not-found-while-lsp-warms.md`).
    #[cfg(unix)]
    async fn scripted_null_document_symbol_peer(
        nulls: usize,
    ) -> (
        tempfile::TempDir,
        std::path::PathBuf,
        Arc<std::sync::atomic::AtomicUsize>,
    ) {
        let at = json!({"start": {"line": 0, "character": 3}, "end": {"line": 0, "character": 8}});
        scripted_not_ready_peer(
            "textDocument/documentSymbol",
            Value::Null,
            nulls,
            json!([{"name": "alpha", "kind": 12, "range": at, "selectionRange": at}]),
        )
        .await
    }

    /// The general form: the first `not_ready_count` requests for `method` get
    /// `not_ready` (a SUCCESS carrying nothing), every later one gets `answer`, and any
    /// other request gets `null`. Returns the tempdir, the socket path, and the counter
    /// of `method` requests served.
    #[cfg(unix)]
    async fn scripted_not_ready_peer(
        method: &'static str,
        not_ready: Value,
        not_ready_count: usize,
        answer: Value,
    ) -> (
        tempfile::TempDir,
        std::path::PathBuf,
        Arc<std::sync::atomic::AtomicUsize>,
    ) {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("peer.sock");
        let listener = tokio::net::UnixListener::bind(&sock).unwrap();
        let served = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let served_by_peer = served.clone();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (read_half, mut write_half) = stream.into_split();
            let mut reader = BufReader::new(read_half);
            // `connect` reads one init message carrying the cached InitializeResult.
            transport::write_message(
                &mut write_half,
                &json!({"jsonrpc": "2.0", "id": 0, "result": {"capabilities": {}}}),
            )
            .await
            .unwrap();
            while let Ok(msg) = transport::read_message(&mut reader).await {
                let Some(id) = msg.get("id").cloned() else {
                    continue; // didOpen and other notifications need no answer
                };
                let result = if msg["method"] == method {
                    let n = served_by_peer.fetch_add(1, Ordering::SeqCst) + 1;
                    if n <= not_ready_count {
                        not_ready.clone()
                    } else {
                        answer.clone()
                    }
                } else {
                    Value::Null
                };
                let reply = json!({"jsonrpc": "2.0", "id": id, "result": result});
                if transport::write_message(&mut write_half, &reply)
                    .await
                    .is_err()
                {
                    return;
                }
            }
        });
        (dir, sock, served)
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn document_symbols_waits_out_a_null_answer_instead_of_reporting_no_symbols() {
        let (dir, sock, served) = scripted_null_document_symbol_peer(2).await;
        let file = dir.path().join("lib.rs");
        std::fs::write(&file, "fn alpha() {}\n").unwrap();
        let client = LspClient::connect(&sock, dir.path().to_path_buf())
            .await
            .unwrap();

        let symbols = client.document_symbols(&file, "rust").await.unwrap();

        assert_eq!(
            symbols.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            vec!["alpha"],
            "a null answer means the server has not answered yet — an empty list here is the \
             false 'symbol not found' / '0 matches' of 7bdeb054a5ab2f46 and bf02ad346f61bf00 Bug B"
        );
        assert_eq!(
            served.load(Ordering::SeqCst),
            3,
            "two null answers must each be re-asked, and the real answer taken on the third"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn document_symbols_reports_a_persistent_null_as_no_answer_not_as_no_symbols() {
        let (dir, sock, served) = scripted_null_document_symbol_peer(usize::MAX).await;
        let file = dir.path().join("lib.rs");
        std::fs::write(&file, "fn alpha() {}\n").unwrap();
        let mut client = LspClient::connect(&sock, dir.path().to_path_buf())
            .await
            .unwrap();
        // Warm the client so the test waits out the short budget, not the cold one.
        client.started_at = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(600))
            .unwrap();

        let err = client.document_symbols(&file, "rust").await.expect_err(
            "a server that never answers must not be reported as a file with no symbols",
        );

        let msg = format!("{err:#}");
        assert!(
            msg.contains("null"),
            "the error must name what the server sent: {msg}"
        );
        assert!(
            served.load(Ordering::SeqCst) > 1,
            "the null answer must have been re-asked before giving up"
        );
    }
    /// FR-12: the third identical null for one file stops saying only "Retry shortly". The first two
    /// calls keep the old hint alone; from the third the hint also names a check and a route that do not
    /// need the server. A second file's count is its own.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_repeated_null_for_one_file_changes_the_advice_from_the_third_call() {
        let (dir, sock, _served) = scripted_null_document_symbol_peer(usize::MAX).await;
        let file = dir.path().join("lib.rs");
        std::fs::write(&file, "fn alpha() {}\n").unwrap();
        let mut client = LspClient::connect(&sock, dir.path().to_path_buf())
            .await
            .unwrap();
        client.started_at = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(600))
            .unwrap();

        let first = hint_of(&client.document_symbols(&file, "rust").await.unwrap_err());
        let second = hint_of(&client.document_symbols(&file, "rust").await.unwrap_err());
        let third = hint_of(&client.document_symbols(&file, "rust").await.unwrap_err());

        for early in [&first, &second] {
            assert!(
                early.contains("Retry shortly"),
                "the old advice stays: {early}"
            );
            assert!(
                !early.contains("in a row"),
                "two nulls are still a server that is starting: {early}"
            );
        }
        assert!(
            third.contains("null answer 3 in a row")
                && third.contains("symbols(path=...)")
                && third.contains("edit_file"),
            "the third null must name the check and the route that skip the server: {third}"
        );
        assert!(
            third.contains("Retry shortly"),
            "the old advice is kept, not replaced: {third}"
        );

        // The control that matters: a different file starts its own count, so the advice is
        // not a property of the whole client after three failures anywhere.
        let other = dir.path().join("other.rs");
        std::fs::write(&other, "fn beta() {}\n").unwrap();
        let other_first = hint_of(&client.document_symbols(&other, "rust").await.unwrap_err());
        assert!(
            !other_first.contains("in a row"),
            "a different file's first null carries no repeat advice: {other_first}"
        );
    }

    /// The counter alone: counts per file, ends a run on an answer, and the advice starts at
    /// [`NullRuns::REPEATED_AT`] and not before.
    #[test]
    fn null_runs_count_per_file_and_an_answer_ends_the_run() {
        let runs = NullRuns::default();
        let a = Path::new("/w/a.rs");
        let b = Path::new("/w/b.rs");
        assert_eq!(runs.record_null(a), 1);
        assert_eq!(runs.record_null(a), 2);
        assert_eq!(runs.record_null(b), 1, "a second file counts on its own");
        runs.clear(a);
        assert_eq!(runs.record_null(a), 1, "an answer ends the run");

        assert!(NullRuns::repeat_advice(NullRuns::REPEATED_AT - 1).is_none());
        assert!(NullRuns::repeat_advice(NullRuns::REPEATED_AT).is_some());
    }

    /// A real reference answer: one location. The not-ready replies it follows in the
    /// tests below are rust-analyzer's cold answers to `textDocument/references` with
    /// `includeDeclaration: true`, measured 2026-09-24: `[]` at 0.02 s and 1.16 s,
    /// `null` at each crate-graph swap, then correct (38 locations, declaration present)
    /// from 10.48 s. Both not-ready shapes carry no locations, not even the declaration.
    #[cfg(unix)]
    fn one_reference_location() -> Value {
        json!([{"uri": "file:///ws/lib.rs",
                "range": {"start": {"line": 0, "character": 3}, "end": {"line": 0, "character": 8}}}])
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn references_waits_out_an_empty_answer_during_warm_up() {
        let (dir, sock, served) = scripted_not_ready_peer(
            "textDocument/references",
            json!([]),
            2,
            one_reference_location(),
        )
        .await;
        let file = dir.path().join("lib.rs");
        std::fs::write(&file, "fn alpha() {}\n").unwrap();
        let client = LspClient::connect(&sock, dir.path().to_path_buf())
            .await
            .unwrap();

        let refs = client.references(&file, 0, 3, "rust").await.unwrap();

        assert_eq!(
            refs.len(),
            1,
            "the real answer after two empty ones must come back"
        );
        assert_eq!(
            served.load(Ordering::SeqCst),
            3,
            "each empty answer must be re-asked"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn references_waits_out_a_null_answer_during_warm_up() {
        let (dir, sock, served) = scripted_not_ready_peer(
            "textDocument/references",
            Value::Null,
            2,
            one_reference_location(),
        )
        .await;
        let file = dir.path().join("lib.rs");
        std::fs::write(&file, "fn alpha() {}\n").unwrap();
        let client = LspClient::connect(&sock, dir.path().to_path_buf())
            .await
            .unwrap();

        let refs = client.references(&file, 0, 3, "rust").await.unwrap();

        assert_eq!(
            refs.len(),
            1,
            "the real answer after two null ones must come back"
        );
        assert_eq!(
            served.load(Ordering::SeqCst),
            3,
            "each null answer must be re-asked"
        );
    }

    /// Exhaustion keeps the empty answer rather than failing: a server that ignores
    /// `includeDeclaration` answers `[]` for a genuinely unused symbol, and an error
    /// there would break the commonest `references` question. `References::call`
    /// warns on such a zero instead.
    #[cfg(unix)]
    #[tokio::test]
    async fn references_returns_a_persistent_empty_answer_as_empty_not_as_an_error() {
        let (dir, sock, served) = scripted_not_ready_peer(
            "textDocument/references",
            json!([]),
            usize::MAX,
            Value::Null,
        )
        .await;
        let file = dir.path().join("lib.rs");
        std::fs::write(&file, "fn alpha() {}\n").unwrap();
        let mut client = LspClient::connect(&sock, dir.path().to_path_buf())
            .await
            .unwrap();
        // Warm the client so the test waits out the short budget, not the cold one.
        client.started_at = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(600))
            .unwrap();

        let refs = client.references(&file, 0, 3, "rust").await.unwrap();

        assert!(refs.is_empty());
        assert!(
            served.load(Ordering::SeqCst) > 1,
            "the empty answer must have been re-asked before it was accepted"
        );
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

    /// `LspClient::connect` used to read the mux's init message with no bound, so a socket that
    /// accepted and said nothing held it forever. Its own bound is `MUX_INIT_READ_TIMEOUT`
    /// (5 s). Real clock (a paused clock jumps over real socket I/O): the call takes at least the
    /// bound and under 8 s; the outer 20 s bound only makes the pre-fix hang fail the test.
    #[cfg(unix)]
    #[tokio::test]
    async fn connect_gives_up_on_a_silent_socket_after_the_default_init_bound() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("mux.sock");
        let listener = tokio::net::UnixListener::bind(&sock).unwrap();
        tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.unwrap();
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        });

        let started = std::time::Instant::now();
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(20),
            LspClient::connect(&sock, dir.path().to_path_buf()),
        )
        .await
        .expect("a silent socket must not hold connect forever");
        let elapsed = started.elapsed();
        let err = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("a socket that sends no init message is not a mux"),
        };
        assert!(err.contains("no init message"), "got: {err}");
        assert!(
            elapsed >= super::MUX_INIT_READ_TIMEOUT
                && elapsed < super::MUX_INIT_READ_TIMEOUT + std::time::Duration::from_secs(3),
            "the bound is the constant, took {elapsed:?}"
        );
    }

    /// The caller's bound wins over the default: a 300 ms bound ends a silent socket's connect in
    /// well under the 5 s default (under 3 s asserted).
    #[cfg(unix)]
    #[tokio::test]
    async fn connect_with_init_timeout_uses_the_callers_bound() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("mux.sock");
        let listener = tokio::net::UnixListener::bind(&sock).unwrap();
        tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.unwrap();
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        });

        let bound = std::time::Duration::from_millis(300);
        let started = std::time::Instant::now();
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(20),
            LspClient::connect_with_init_timeout(&sock, dir.path().to_path_buf(), bound),
        )
        .await
        .expect("a silent socket must not hold connect forever");
        let elapsed = started.elapsed();
        let applied = result
            .err()
            .expect("a silent socket is not a mux")
            .downcast_ref::<MuxInitTimeout>()
            .expect("the failure is the init timeout")
            .after;
        assert_eq!(
            applied, bound,
            "the error must report the bound that was applied"
        );
        assert!(
            elapsed >= bound && elapsed < std::time::Duration::from_secs(3),
            "the caller's {bound:?} bound must apply, took {elapsed:?}"
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
