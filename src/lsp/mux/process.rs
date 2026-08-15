//! Mux process — multiplexes LSP messages between multiple clients and a single LSP server.
//!
//! Lifecycle:
//! 1. Acquire exclusive flock on a lock file
//! 2. Spawn the LSP server child process
//! 3. Perform the LSP initialize handshake
//! 4. Bind a Unix socket and signal "ready" to the parent
//! 5. Route messages between connected clients and the server
//! 6. Shut down on idle timeout or server death

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use fs4::fs_std::FileExt;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::process::Command;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::lsp::mux::protocol::{self, ClientTag, DocumentState};
use crate::lsp::transport::{read_message, write_message};

/// Writer handle shared between tasks. Both server stdin and client streams use this type.
type SharedWriter = Arc<Mutex<Box<dyn AsyncWrite + Unpin + Send>>>;

/// Internal state for the mux event loop.
struct MuxState {
    clients: HashMap<ClientTag, SharedWriter>,
    doc_state: DocumentState,
    cached_init_result: Value,
    cached_capabilities: Vec<Value>,
    edit_lock_owner: Option<ClientTag>,
    next_tag: u32,
    idle_since: Option<Instant>,
}

impl MuxState {
    fn new(init_result: Value) -> Self {
        Self {
            clients: HashMap::new(),
            doc_state: DocumentState::new(),
            cached_init_result: init_result,
            cached_capabilities: Vec::new(),
            edit_lock_owner: None,
            next_tag: 0,
            idle_since: Some(Instant::now()),
        }
    }

    fn next_tag(&mut self) -> ClientTag {
        let tag = char::from(b'a' + (self.next_tag % 26) as u8).to_string();
        self.next_tag += 1;
        tag
    }
}

/// Run the mux process. This is the entry point called by `codescout mux`.
///
/// Blocks until idle timeout or server death. The caller should `std::process::exit`
/// after this returns.
pub async fn run(
    socket_path: &Path,
    lock_path: &Path,
    workspace_root: &Path,
    idle_timeout_secs: u64,
    server_command: &str,
    server_args: &[String],
    server_env: &[(String, String)],
) -> Result<()> {
    // 1. Acquire exclusive flock
    let lock_file = std::fs::File::create(lock_path)
        .with_context(|| format!("failed to create lock file: {}", lock_path.display()))?;
    lock_file
        .try_lock_exclusive()
        .context("another mux instance holds the lock")?;
    // Write PID for diagnostics
    use std::io::Write;
    writeln!(&lock_file, "{}", std::process::id())?;

    // 2. Spawn LSP server
    // process_group(0) puts the LSP server in its OWN process group (PGID = child
    // PID), so a killpg() on shutdown reaps the JVM AND its grandchildren (kotlin-lsp
    // forks) — kill_on_drop only kills the direct child and only on graceful Drop.
    // tokio's Command exposes process_group() inherently (no CommandExt import needed;
    // see the matching idiom in src/tools/run_command/inner.rs).
    let mut child = Command::new(server_command)
        .args(server_args)
        .envs(server_env.iter().map(|(k, v)| (k, v)))
        .current_dir(workspace_root)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .process_group(0) // own group so killpg reaps grandchildren (JVM forks)
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("failed to spawn LSP server: {server_command}"))?;
    // Capture the PGID (== child PID) so the shutdown path can killpg the whole group.
    let child_pgid: Option<libc::pid_t> = child.id().map(|id| id as libc::pid_t);

    let server_stdin = child.stdin.take().context("no stdin on child")?;
    let server_stdout = child.stdout.take().context("no stdout on child")?;

    let server_writer: SharedWriter = Arc::new(Mutex::new(
        Box::new(server_stdin) as Box<dyn AsyncWrite + Unpin + Send>
    ));
    let mut server_reader = BufReader::new(server_stdout);

    // Spawn stderr logger — info level so JVM GC/OOM warnings reach the diagnostic log
    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => info!(target: "mux::server_stderr", "{}", line.trim_end()),
                }
            }
        });
    }

    // Spawn memory watcher — warns when RSS+swap exceeds expected bounds, and kills
    // past the ceiling. The marker path lets that kill outlive this process so the
    // manager can throttle a respawn loop; see `memkill_path_for_lock`.
    if let Some(pid) = child.id() {
        tokio::spawn(watch_memory(pid, super::memkill_path_for_lock(lock_path)));
    }

    // 3. LSP initialize handshake
    let init_request = json!({
        "jsonrpc": "2.0",
        "id": 0,
        "method": "initialize",
        "params": {
            "processId": null,
            "capabilities": {
                "textDocument": {
                    "synchronization": {
                        "dynamicRegistration": true,
                        "didSave": true
                    },
                    "definition": { "dynamicRegistration": true },
                    "references": { "dynamicRegistration": true },
                    "hover": { "dynamicRegistration": true },
                    "rename": { "dynamicRegistration": true },
                    "documentSymbol": {
                        "dynamicRegistration": true,
                        "hierarchicalDocumentSymbolSupport": true
                    },
                    "completion": { "dynamicRegistration": true }
                },
                "workspace": {
                    "workspaceFolders": true,
                    "applyEdit": true
                }
            },
            "rootUri": url::Url::from_file_path(workspace_root).map(|u| u.to_string()).unwrap_or_default()
        }
    });

    {
        let mut w = server_writer.lock().await;
        write_message(&mut *w, &init_request).await?;
    }

    // Read messages until we get the initialize response (id: 0).
    // LSP servers often send server-to-client requests during startup
    // (workspace/configuration, client/registerCapability, window/workDoneProgress/create)
    // before the actual initialize response. We auto-respond to those.
    let init_result = loop {
        let msg = read_message(&mut server_reader)
            .await
            .context("failed to read message during initialize handshake")?;

        // Check if this is the response to our initialize request (id: 0)
        if msg.get("id").and_then(|v| v.as_i64()) == Some(0) && msg.get("method").is_none() {
            // This is the initialize response
            break msg
                .get("result")
                .cloned()
                .context("initialize response missing 'result'")?;
        }

        // Server-to-client request — auto-respond with null
        if let Some(id) = msg.get("id") {
            if msg.get("method").is_some() {
                debug!(
                    "auto-responding to server request during init: {}",
                    msg.get("method").unwrap()
                );
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": null,
                });
                let mut w = server_writer.lock().await;
                write_message(&mut *w, &response).await?;
            }
        }
        // Notifications (no id) — just ignore during init
    };

    info!("LSP server initialized successfully");

    // Send initialized notification
    let initialized_notif = json!({
        "jsonrpc": "2.0",
        "method": "initialized",
        "params": {}
    });
    {
        let mut w = server_writer.lock().await;
        write_message(&mut *w, &initialized_notif).await?;
    }

    // 4. Bind Unix socket
    if socket_path.exists() {
        std::fs::remove_file(socket_path).ok();
    }
    let listener = UnixListener::bind(socket_path)
        .with_context(|| format!("failed to bind socket: {}", socket_path.display()))?;
    // Restrict socket to the current user. Defence-in-depth on top of the
    // per-user directory: anyone with `/tmp` read access could otherwise
    // attempt to connect on older systems where the socket was created
    // world-writable before this chmod.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600));
    }

    // 5. Signal ready to parent, then drop stdout
    {
        let mut stdout = tokio::io::stdout();
        stdout.write_all(b"ready\n").await?;
        stdout.flush().await?;
    }
    // stdout is dropped here (goes out of scope)

    // Run the event loop
    let state = Arc::new(Mutex::new(MuxState::new(init_result)));
    let result = event_loop(
        &listener,
        &mut server_reader,
        &server_writer,
        &state,
        idle_timeout_secs,
    )
    .await;

    // Shutdown: kill the LSP server before reclaiming its analyzer HOME so no
    // writer holds the dir, then remove the socket.
    let _ = child.kill().await;
    // Kill the whole LSP process group (JVM + forks). kill_on_drop / child.kill()
    // only reap the direct child; grandchildren would orphan and squat the RocksDB
    // index. Runs on EVERY event_loop exit path (idle timeout, server disconnect,
    // SIGTERM/SIGINT). SIGKILL of the mux itself is uncatchable and is covered by
    // the reap-before-spawn net on the next mux start.
    // killpg on an already-dead group returns ESRCH — harmless.
    if let Some(pgid) = child_pgid {
        kill_process_group(pgid).await;
    }
    std::fs::remove_file(socket_path).ok();
    // flock released when lock_file drops

    // Reclaim a codescout-provisioned kotlin-lsp analyzer HOME, if any. The
    // analyzer index escapes --system-path into <user.home>/.config/JetBrains/
    // analyzer and grows unbounded; codescout redirects user.home per-workspace
    // (JAVA_TOOL_OPTIONS=-Duser.home=<cache>/codescout/kotlin-lsp-home/<hash>)
    // and reclaims it here so the churning RocksDB store can't accumulate across
    // sessions. See docs/issues/2026-06-01-kotlin-lsp-analyzer-index-unbounded-disk.md.
    reclaim_kotlin_analyzer_home(server_env);

    info!("mux process shutting down");
    result
}

/// Reclaim the codescout-provisioned kotlin-lsp analyzer HOME named in
/// `server_env` (`JAVA_TOOL_OPTIONS=-Duser.home=<dir>`), if present and guarded.
/// kotlin-lsp's analyzer index ignores `--system-path` and grows unbounded in
/// `<user.home>/.config/JetBrains/analyzer`; codescout redirects user.home into
/// a per-workspace cache dir, swept here on mux exit. See
/// `docs/issues/2026-06-01-kotlin-lsp-analyzer-index-unbounded-disk.md`.
fn reclaim_kotlin_analyzer_home(server_env: &[(String, String)]) {
    let Some(home) = kotlin_home_from_env(server_env) else {
        return;
    };
    match std::fs::remove_dir_all(&home) {
        Ok(()) => info!("reclaimed kotlin-lsp analyzer home {}", home.display()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => warn!("failed to reclaim kotlin-lsp home {}: {e}", home.display()),
    }
}

/// Extract a codescout-owned kotlin-lsp HOME from `JAVA_TOOL_OPTIONS` in the
/// server env. Returns the path only if it passes the codescout-home guard
/// (so we never `remove_dir_all` a real home or arbitrary directory).
fn kotlin_home_from_env(server_env: &[(String, String)]) -> Option<std::path::PathBuf> {
    let jto = server_env
        .iter()
        .find(|(k, _)| k == "JAVA_TOOL_OPTIONS")
        .map(|(_, v)| v.as_str())?;
    // codescout appends its -Duser.home last; take the last occurrence.
    let dir = jto
        .rsplit_once("-Duser.home=")?
        .1
        .split_whitespace()
        .next()?;
    let path = std::path::PathBuf::from(dir);
    crate::lsp::servers::is_codescout_kotlin_home(&path).then_some(path)
}

/// Main event loop — accepts clients, reads from server, checks idle timeout.
async fn event_loop(
    listener: &UnixListener,
    server_reader: &mut BufReader<tokio::process::ChildStdout>,
    server_writer: &SharedWriter,
    state: &Arc<Mutex<MuxState>>,
    idle_timeout_secs: u64,
) -> Result<()> {
    let idle_timeout = std::time::Duration::from_secs(idle_timeout_secs);
    let watchdog_interval = tokio::time::Duration::from_secs(10);
    let mut watchdog_tick = tokio::time::interval(watchdog_interval);
    watchdog_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // Install signal handlers ONCE (not per loop iteration — re-registering each
    // select pass would leak handler registrations). Breaking the loop on
    // SIGTERM/SIGINT lets `run` killpg the LSP process group on graceful shutdown.
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .context("install SIGTERM handler")?;
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .context("install SIGINT handler")?;

    loop {
        tokio::select! {
            // Accept new client connections
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, _addr)) => {
                        let (read_half, write_half) = stream.into_split();
                        let writer: SharedWriter = Arc::new(Mutex::new(
                            Box::new(write_half) as Box<dyn AsyncWrite + Unpin + Send>,
                        ));

                        let mut st = state.lock().await;
                        let tag = st.next_tag();
                        st.clients.insert(tag.clone(), writer.clone());
                        st.idle_since = None;

                        // Send cached init info to the new client
                        let init_msg = json!({
                            "type": "init",
                            "result": st.cached_init_result,
                            "registered_capabilities": st.cached_capabilities,
                        });
                        drop(st);

                        let w = writer.clone();
                        let tag_clone = tag.clone();
                        tokio::spawn(async move {
                            let mut w = w.lock().await;
                            if let Err(e) = write_message(&mut *w, &init_msg).await {
                                warn!(tag = %tag_clone, "failed to send init to client: {e}");
                            }
                        });

                        // Spawn per-client reader
                        let reader = BufReader::new(read_half);
                        let sw = server_writer.clone();
                        let st_clone = state.clone();
                        tokio::spawn(client_reader_task(tag, reader, sw, st_clone));

                        info!("client connected");
                    }
                    Err(e) => {
                        warn!("failed to accept client connection: {e}");
                    }
                }
            }

            // Read messages from the LSP server
            server_msg = read_message(server_reader) => {
                match server_msg {
                    Ok(msg) => {
                        handle_server_message(msg, state, server_writer).await;
                    }
                    Err(e) => {
                        info!("LSP server disconnected: {e}");
                        break;
                    }
                }
            }

            // Idle watchdog
            _ = watchdog_tick.tick() => {
                let st = state.lock().await;
                if let Some(since) = st.idle_since {
                    if since.elapsed() >= idle_timeout {
                        info!("idle timeout reached ({idle_timeout_secs}s), shutting down");
                        break;
                    }
                }
            }

            // Signalled shutdown — break so `run` can killpg the LSP process group.
            _ = sigterm.recv() => {
                info!("mux received SIGTERM, exiting event loop");
                break;
            }
            _ = sigint.recv() => {
                info!("mux received SIGINT, exiting event loop");
                break;
            }
        }
    }

    Ok(())
}

/// Per-client reader task — reads messages from a client and forwards to the server.
async fn client_reader_task(
    tag: ClientTag,
    mut reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    server_writer: SharedWriter,
    state: Arc<Mutex<MuxState>>,
) {
    while let Ok(mut msg) = read_message(&mut reader).await {
        if let Err(e) = handle_client_message(&tag, &mut msg, &server_writer, &state).await {
            warn!(tag = %tag, "error handling client message: {e}");
        }
    }

    // Client disconnected — clean up
    handle_client_disconnect(&tag, &server_writer, &state).await;
}

/// Process a message from a client before forwarding to the server.
async fn handle_client_message(
    tag: &str,
    msg: &mut Value,
    server_writer: &SharedWriter,
    state: &Arc<Mutex<MuxState>>,
) -> Result<()> {
    let method = msg.get("method").and_then(|m| m.as_str()).map(String::from);

    // Tag the id only on REQUESTS (has both id and method) so responses can
    // route back to this client via untag_response_id. Client-to-server
    // RESPONSES (has id, no method) — e.g. auto-responses to server-initiated
    // workspace/applyEdit — must forward the id UNCHANGED so the server can
    // match it to its pending request. Tagging those caused rust-analyzer to
    // panic with "received response for unknown request".
    if method.is_some() {
        if let Some(id) = msg.get("id") {
            let tagged = protocol::tag_request_id(id, tag);
            msg["id"] = tagged;
        }
    }

    // Handle document synchronization
    if let Some(ref method) = method {
        let mut st = state.lock().await;
        match method.as_str() {
            "textDocument/didOpen" => {
                if let Some(uri) = extract_text_document_uri(msg) {
                    let forward = st.doc_state.open(&uri, tag);
                    if !forward {
                        debug!(tag = %tag, uri = %uri, "didOpen suppressed (already open)");
                        return Ok(());
                    }
                }
            }
            "textDocument/didClose" => {
                if let Some(uri) = extract_text_document_uri(msg) {
                    let forward = st.doc_state.close(&uri, tag);
                    if !forward {
                        debug!(tag = %tag, uri = %uri, "didClose suppressed (other clients still have it open)");
                        return Ok(());
                    }
                }
            }
            "textDocument/didChange" => {
                if let Some(uri) = extract_text_document_uri(msg) {
                    let version = st.doc_state.next_version(&uri);
                    // Rewrite version in the message
                    if let Some(td) = msg
                        .get_mut("params")
                        .and_then(|p| p.get_mut("textDocument"))
                    {
                        td["version"] = json!(version);
                    }
                }
            }
            "textDocument/rename" => {
                st.edit_lock_owner = Some(tag.to_string());
            }
            _ => {}
        }
    }

    // Forward to server
    let mut w = server_writer.lock().await;
    write_message(&mut *w, msg).await?;
    Ok(())
}

/// Process a message from the LSP server and route to the correct client(s).
async fn handle_server_message(
    mut msg: Value,
    state: &Arc<Mutex<MuxState>>,
    server_writer: &SharedWriter,
) {
    let has_id = msg.get("id").is_some();
    let has_method = msg.get("method").and_then(|m| m.as_str()).is_some();

    if has_id && !has_method {
        // Response to a client request
        handle_server_response(&mut msg, state).await;
    } else if has_id && has_method {
        // Server-to-client request
        handle_server_request(&msg, state, server_writer).await;
    } else if has_method {
        // Server notification — broadcast to all clients
        broadcast_to_clients(&msg, state).await;
    } else {
        // Neither id nor method: not a valid JSON-RPC message. Silent drop
        // used to hide misbehaving servers — log so it shows up in telemetry.
        tracing::debug!(
            ?msg,
            "mux: dropping server message with no id and no method"
        );
    }
}

/// Route a server response back to the originating client.
async fn handle_server_response(msg: &mut Value, state: &Arc<Mutex<MuxState>>) {
    let id = match msg.get("id") {
        Some(id) => id.clone(),
        None => return,
    };

    let (tag, original_id) = match protocol::untag_response_id(&id) {
        Some(pair) => pair,
        None => {
            debug!("server response with untagged id: {id}");
            return;
        }
    };

    // Restore original ID
    msg["id"] = original_id;

    // Check if this completes a rename operation
    {
        let mut st = state.lock().await;
        if st.edit_lock_owner.as_deref() == Some(&tag) {
            // Clear edit lock on rename response
            st.edit_lock_owner = None;
        }
    }

    // Send to the tagged client
    let writer = {
        let st = state.lock().await;
        st.clients.get(&tag).cloned()
    };

    if let Some(writer) = writer {
        let mut w = writer.lock().await;
        if let Err(e) = write_message(&mut *w, msg).await {
            warn!(tag = %tag, "failed to send response to client: {e}");
        }
    } else {
        debug!(tag = %tag, "response for disconnected client, dropping");
    }
}

/// Extract the LSP registration ids carried by a `client/registerCapability` message.
fn registration_ids(msg: &Value) -> Vec<String> {
    msg.get("params")
        .and_then(|p| p.get("registrations"))
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|r| r.get("id").and_then(Value::as_str).map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// Insert a `client/registerCapability` message into the replay cache, superseding any
/// cached entry whose registration ids are all present in `msg`. LSP re-registration
/// replaces a prior registration of the same id, so the previous append-only cache grew
/// without bound on every re-registration
/// (docs/issues/2026-06-23-mux-cached-capabilities-unbounded.md).
fn cache_registration(cache: &mut Vec<Value>, msg: &Value) {
    let new_ids: std::collections::HashSet<String> = registration_ids(msg).into_iter().collect();
    if !new_ids.is_empty() {
        // Drop a cached message only when every registration it carries is superseded by
        // `msg`; keep it if it still holds at least one live (non-superseded) id.
        cache.retain(|cached| {
            registration_ids(cached)
                .iter()
                .any(|id| !new_ids.contains(id))
        });
    }
    cache.push(msg.clone());
}

/// Handle a server-to-client request (e.g. workspace/applyEdit, client/registerCapability).
async fn handle_server_request(
    msg: &Value,
    state: &Arc<Mutex<MuxState>>,
    server_writer: &SharedWriter,
) {
    let method = msg
        .get("method")
        .and_then(|m| m.as_str())
        .unwrap_or_default();
    let id = msg.get("id").cloned().unwrap_or(Value::Null);

    match method {
        "workspace/applyEdit" => {
            // Route to the edit lock owner
            let writer = {
                let st = state.lock().await;
                st.edit_lock_owner
                    .as_ref()
                    .and_then(|tag| st.clients.get(tag).cloned())
            };

            if let Some(writer) = writer {
                let mut w = writer.lock().await;
                if let Err(e) = write_message(&mut *w, msg).await {
                    warn!("failed to forward applyEdit to client: {e}");
                    // Auto-respond with failure
                    send_auto_response(&id, server_writer, false).await;
                }
            } else {
                // No edit lock owner — auto-respond with success
                send_auto_response(&id, server_writer, true).await;
            }
        }
        "client/registerCapability" => {
            // Cache the capability so new clients get it in their init message.
            // Do NOT broadcast: broadcast would make connected clients auto-respond
            // via dispatch_lsp_message, producing a duplicate response with the
            // server's original id, which crashes rust-analyzer with
            // "received response for unknown request".
            {
                let mut st = state.lock().await;
                cache_registration(&mut st.cached_capabilities, msg);
            }
            // Auto-respond with null — `client/registerCapability` spec response is void.
            let response = json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": null,
            });
            let mut w = server_writer.lock().await;
            if let Err(e) = write_message(&mut *w, &response).await {
                error!("failed to send auto-response to server: {e}");
            }
        }
        _ => {
            // Unknown server request — auto-respond with null
            let response = json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": null
            });
            let mut w = server_writer.lock().await;
            if let Err(e) = write_message(&mut *w, &response).await {
                error!("failed to send auto-response to server: {e}");
            }
        }
    }
}

/// Send a simple response back to the server for a server-initiated request.
async fn send_auto_response(id: &Value, server_writer: &SharedWriter, success: bool) {
    let result = if success {
        json!({ "applied": true })
    } else {
        json!({ "applied": false })
    };
    let response = json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    });
    let mut w = server_writer.lock().await;
    if let Err(e) = write_message(&mut *w, &response).await {
        error!("failed to send auto-response to server: {e}");
    }
}

/// Broadcast a message to all connected clients.
async fn broadcast_to_clients(msg: &Value, state: &Arc<Mutex<MuxState>>) {
    let writers: Vec<(ClientTag, SharedWriter)> = {
        let st = state.lock().await;
        st.clients
            .iter()
            .map(|(tag, w)| (tag.clone(), w.clone()))
            .collect()
    };

    for (tag, writer) in writers {
        let mut w = writer.lock().await;
        if let Err(e) = write_message(&mut *w, msg).await {
            debug!(tag = %tag, "failed to broadcast to client: {e}");
        }
    }
}

/// Clean up after a client disconnects.
async fn handle_client_disconnect(
    tag: &str,
    server_writer: &SharedWriter,
    state: &Arc<Mutex<MuxState>>,
) {
    info!(tag = %tag, "client disconnected");

    let uris_to_close = {
        let mut st = state.lock().await;
        st.clients.remove(tag);

        // Clear edit lock if this client held it
        if st.edit_lock_owner.as_deref() == Some(tag) {
            st.edit_lock_owner = None;
        }

        let uris = st.doc_state.disconnect(tag);

        // Set idle timer if no clients remain
        if st.clients.is_empty() {
            st.idle_since = Some(Instant::now());
            info!("no clients connected, starting idle timer");
        }

        uris
    };

    // Send didClose for orphaned documents
    for uri in uris_to_close {
        let close_msg = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didClose",
            "params": {
                "textDocument": { "uri": uri }
            }
        });
        let mut w = server_writer.lock().await;
        if let Err(e) = write_message(&mut *w, &close_msg).await {
            warn!("failed to send didClose to server for {uri}: {e}");
        }
    }
}

/// Extract the text document URI from a notification's params.
fn extract_text_document_uri(msg: &Value) -> Option<String> {
    msg.get("params")?
        .get("textDocument")?
        .get("uri")?
        .as_str()
        .map(String::from)
}

/// Periodically samples RSS+swap for the LSP server process and **kills the LSP
/// process group** when it balloons, to bound native (off-heap) memory growth the
/// JVM `-Xmx` heap cap does not constrain
/// (docs/issues/2026-06-19-kotlin-lsp-uncapped-jvm-heap.md, Fix 2). Logs warn at
/// 4 GiB and error at 8 GiB. Kills when rss+swap crosses an absolute ceiling
/// (`CODESCOUT_LSP_KILL_RSS_CEIL_MB`, default 24 GiB) or when host `MemAvailable`
/// falls below a floor (`CODESCOUT_LSP_KILL_AVAIL_FLOOR_MB`, default 15 GiB) while
/// this process is itself large (>= 8 GiB). Set `CODESCOUT_LSP_MEM_KILL_DISABLE=1`
/// to log only. Exits when the process dies.
async fn watch_memory(pid: u32, memkill_marker: std::path::PathBuf) {
    let thresholds = MemThresholds::from_env();

    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(60));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        let Some((rss_kb, swap_kb)) = read_proc_memory(pid) else {
            break;
        };
        let avail_kb = read_mem_available_kb();
        let total_kb = rss_kb + swap_kb;
        let rss_gib = rss_kb as f64 / (1024.0 * 1024.0);
        let swap_gib = swap_kb as f64 / (1024.0 * 1024.0);
        let total_gib = total_kb as f64 / (1024.0 * 1024.0);
        match classify_memory(rss_kb, swap_kb, avail_kb, &thresholds) {
            MemAction::Kill(reason) => {
                let avail_gib = avail_kb
                    .map(|a| a as f64 / (1024.0 * 1024.0))
                    .unwrap_or(f64::NAN);
                error!(
                    target: "mux::memory",
                    "LSP server memory watchdog KILLING process group (pid={} reason={}): {:.1} GiB total (rss={:.1} GiB swap={:.1} GiB avail={:.1} GiB)",
                    pid, reason, total_gib, rss_gib, swap_gib, avail_gib
                );
                // Leave a breadcrumb the manager can find AFTER this process is gone.
                // Written before the kill: killing the group can take this mux down
                // with it, and a marker that only lands on the happy path is a marker
                // that never lands on the path that matters.
                if let Err(e) = std::fs::write(&memkill_marker, reason) {
                    warn!(
                        target: "mux::memory",
                        "could not record mem-kill marker at {}: {e}", memkill_marker.display()
                    );
                }
                // PGID == PID (child spawned with process_group(0)).
                kill_process_group(pid as libc::pid_t).await;
                break;
            }
            MemAction::Error => {
                error!(
                    target: "mux::memory",
                    "LSP server memory CRITICAL (pid={}): {:.1} GiB total (rss={:.1} GiB swap={:.1} GiB)",
                    pid, total_gib, rss_gib, swap_gib
                );
            }
            MemAction::Warn => {
                warn!(
                    target: "mux::memory",
                    "LSP server memory high (pid={}): {:.1} GiB total (rss={:.1} GiB swap={:.1} GiB)",
                    pid, total_gib, rss_gib, swap_gib
                );
            }
            MemAction::Ok => {
                debug!(
                    target: "mux::memory",
                    "LSP server memory (pid={}): rss={:.1} GiB swap={:.1} GiB",
                    pid, rss_gib, swap_gib
                );
            }
        }
    }
}

/// SIGTERM the process group, wait 500ms, then SIGKILL. `killpg` on an
/// already-dead group returns ESRCH — harmless. The pgid was created with
/// `process_group(0)` (PGID == child PID), so signalling the group reaps the JVM
/// *and* its kotlin-lsp forks. Shared by `run`'s shutdown path and the memory
/// watchdog.
async fn kill_process_group(pgid: libc::pid_t) {
    // SAFETY: pgid was created with process_group(0); signalling our own group is safe.
    unsafe {
        libc::killpg(pgid, libc::SIGTERM);
    }
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    unsafe {
        libc::killpg(pgid, libc::SIGKILL);
    }
}

/// Reads `MemAvailable` (KiB) from `/proc/meminfo`. Returns `None` if it cannot be
/// read/parsed. Mirrors `hardware::probe_ram`'s line-scan idiom.
#[cfg(target_os = "linux")]
fn read_mem_available_kb() -> Option<u64> {
    let content = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("MemAvailable:") {
            return rest.split_whitespace().next().and_then(|v| v.parse().ok());
        }
    }
    None
}

#[cfg(not(target_os = "linux"))]
fn read_mem_available_kb() -> Option<u64> {
    None
}

/// Watchdog thresholds, resolved once at watcher start. Log thresholds are fixed;
/// kill thresholds are env-tunable (in MB), mirroring the `CODESCOUT_INDEX_FLUSH_BATCH`
/// override precedent — no CLI plumbing through main.rs/manager.rs.
#[derive(Clone, Copy, Debug)]
struct MemThresholds {
    warn_kb: u64,
    error_kb: u64,
    /// Absolute per-process (rss+swap) ceiling — kill regardless of host state.
    kill_rss_ceil_kb: u64,
    /// Host `MemAvailable` floor — kill a large process when the host drops below this.
    kill_avail_floor_kb: u64,
    /// When false, the watchdog only logs and never kills.
    kill_enabled: bool,
}

impl MemThresholds {
    fn from_env() -> Self {
        const WARN_KB: u64 = 4 * 1024 * 1024; // 4 GiB
        const ERROR_KB: u64 = 8 * 1024 * 1024; // 8 GiB
        const DEFAULT_KILL_RSS_CEIL_MB: u64 = 24 * 1024; // 24 GiB
        const DEFAULT_KILL_AVAIL_FLOOR_MB: u64 = 15 * 1024; // 15 GiB

        let mb_to_kb = |key: &str, default_mb: u64| -> u64 {
            std::env::var(key)
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .filter(|&n| n > 0)
                .unwrap_or(default_mb)
                .saturating_mul(1024)
        };
        let kill_enabled = !matches!(
            std::env::var("CODESCOUT_LSP_MEM_KILL_DISABLE")
                .ok()
                .as_deref(),
            Some("1") | Some("true")
        );
        Self {
            warn_kb: WARN_KB,
            error_kb: ERROR_KB,
            kill_rss_ceil_kb: mb_to_kb("CODESCOUT_LSP_KILL_RSS_CEIL_MB", DEFAULT_KILL_RSS_CEIL_MB),
            kill_avail_floor_kb: mb_to_kb(
                "CODESCOUT_LSP_KILL_AVAIL_FLOOR_MB",
                DEFAULT_KILL_AVAIL_FLOOR_MB,
            ),
            kill_enabled,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MemAction {
    Ok,
    Warn,
    Error,
    /// Kill the LSP process group; the reason is for logs/metrics.
    Kill(&'static str),
}

/// Pure decision: given a process's rss+swap and (optionally) host `MemAvailable`,
/// decide whether to log or kill. No I/O — unit-testable in isolation.
///
/// Two kill arms:
/// - **rss_ceiling**: rss+swap crosses an absolute ceiling — a single LSP this large
///   is pathological regardless of host RAM (the "35 GB for a fixture" case, which on
///   a big box never depresses host `MemAvailable`).
/// - **host_pressure**: host `MemAvailable` is below the floor *and* this process is
///   itself large (>= error threshold) — protects the host from a global OOM while
///   sparing an innocent small LSP when the pressure came from elsewhere.
fn classify_memory(
    rss_kb: u64,
    swap_kb: u64,
    avail_kb: Option<u64>,
    th: &MemThresholds,
) -> MemAction {
    let total_kb = rss_kb.saturating_add(swap_kb);
    if th.kill_enabled {
        if total_kb >= th.kill_rss_ceil_kb {
            return MemAction::Kill("rss_ceiling");
        }
        if let Some(avail) = avail_kb {
            if avail < th.kill_avail_floor_kb && total_kb >= th.error_kb {
                return MemAction::Kill("host_pressure");
            }
        }
    }
    if total_kb >= th.error_kb {
        MemAction::Error
    } else if total_kb >= th.warn_kb {
        MemAction::Warn
    } else {
        MemAction::Ok
    }
}

/// Reads VmRSS and VmSwap from `/proc/{pid}/status`. Returns `None` if the process is gone.
#[cfg(target_os = "linux")]
fn read_proc_memory(pid: u32) -> Option<(u64, u64)> {
    let content = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let mut rss_kb = None;
    let mut swap_kb = None;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            rss_kb = rest.split_whitespace().next().and_then(|v| v.parse().ok());
        } else if let Some(rest) = line.strip_prefix("VmSwap:") {
            swap_kb = rest.split_whitespace().next().and_then(|v| v.parse().ok());
        }
        if rss_kb.is_some() && swap_kb.is_some() {
            break;
        }
    }
    Some((rss_kb?, swap_kb.unwrap_or(0)))
}

#[cfg(not(target_os = "linux"))]
fn read_proc_memory(_pid: u32) -> Option<(u64, u64)> {
    None
}

#[cfg(test)]
mod kotlin_home_tests {
    use super::*;

    #[test]
    fn kotlin_home_from_env_extracts_guarded_home() {
        let home = crate::lsp::servers::kotlin_analyzer_home("abc123");
        let env = vec![
            ("GRADLE_USER_HOME".to_string(), "/tmp/g".to_string()),
            (
                "JAVA_TOOL_OPTIONS".to_string(),
                format!("-Duser.home={}", home.display()),
            ),
        ];
        assert_eq!(kotlin_home_from_env(&env), Some(home));
    }

    #[test]
    fn kotlin_home_from_env_takes_last_user_home() {
        // codescout appends its -Duser.home last; an earlier (foreign) one is
        // ignored because rsplit takes the final occurrence.
        let home = crate::lsp::servers::kotlin_analyzer_home("ws9");
        let env = vec![(
            "JAVA_TOOL_OPTIONS".to_string(),
            format!(
                "-Xmx2g -Duser.home=/home/real -Duser.home={}",
                home.display()
            ),
        )];
        assert_eq!(kotlin_home_from_env(&env), Some(home));
    }

    #[test]
    fn kotlin_home_from_env_rejects_foreign_and_absent() {
        // A foreign user.home (a real home) must be rejected by the guard.
        let env = vec![(
            "JAVA_TOOL_OPTIONS".to_string(),
            "-Duser.home=/home/victim".to_string(),
        )];
        assert_eq!(kotlin_home_from_env(&env), None);
        // No JAVA_TOOL_OPTIONS at all.
        let env2 = vec![("GRADLE_USER_HOME".to_string(), "/tmp/g".to_string())];
        assert_eq!(kotlin_home_from_env(&env2), None);
    }
}

/// Tests for the process-group reaping mechanism that `run`'s shutdown relies on (leak S2).
///
/// HONESTY NOTE: these exercise the *OS primitive* the fix is built on —
/// `process_group(0)` (own group, PGID == child PID) plus `killpg`, which is exactly
/// how `run` spawns the LSP child and tears down the group on signalled exit. They do
/// NOT spin up a real mux + LSP handshake + Unix socket: faking an LSP that completes
/// the framed `initialize` handshake AND forks a trackable grandchild was judged too
/// heavy to write honestly (see Task 3 report). The end-to-end JVM-orphan path is
/// verified manually via `/mcp` (Step 5). A direct SIGKILL-of-the-mux test is
/// impossible (SIGKILL is uncatchable) and is covered by Task 2's reap-before-spawn.
#[cfg(all(test, unix))]
mod process_group_reaping_tests {
    use tokio::process::Command;

    /// `kill(pid, 0)` probes existence without sending a signal: 0 = alive,
    /// ESRCH = gone. (EPERM would also mean "exists"; not reachable here since we
    /// only probe our own descendants.)
    fn pid_alive(pid: libc::pid_t) -> bool {
        unsafe { libc::kill(pid, 0) == 0 }
    }

    /// The fix's core claim: a child spawned with `process_group(0)` that forks a
    /// grandchild puts the grandchild in the SAME group, so a single `killpg` on the
    /// group PGID reaps BOTH — which is why `run`'s shutdown kills the JVM *and* its
    /// kotlin-lsp forks, not just the direct child.
    #[tokio::test]
    async fn killpg_reaps_grandchild_in_child_process_group() {
        // Child shell forks a grandchild that sleeps 60s, prints the grandchild PID,
        // then waits — so both child and grandchild are alive and in the new group.
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("sleep 60 & echo $! ; wait")
            .stdout(std::process::Stdio::piped())
            .process_group(0) // same wiring as run()'s LSP child
            .kill_on_drop(true)
            .spawn()
            .expect("spawn group leader");

        let pgid = child.id().expect("child has a pid") as libc::pid_t;

        // Read the grandchild PID the shell printed (one bounded line).
        use tokio::io::AsyncReadExt as _;
        let mut stdout = child.stdout.take().expect("child stdout");
        let grandchild_pid = {
            let mut buf = Vec::new();
            // Bound the wait so a hung shell can't hang the test.
            let read = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                let mut byte = [0u8; 1];
                loop {
                    let n = stdout.read(&mut byte).await.expect("read stdout");
                    if n == 0 || byte[0] == b'\n' {
                        break;
                    }
                    buf.push(byte[0]);
                }
            })
            .await;
            read.expect("timed out reading grandchild pid");
            String::from_utf8(buf)
                .expect("utf8 pid")
                .trim()
                .parse::<libc::pid_t>()
                .expect("parse grandchild pid")
        };

        assert!(
            pid_alive(grandchild_pid),
            "grandchild should be alive before killpg"
        );

        // The exact teardown run() performs on signalled exit.
        unsafe {
            libc::killpg(pgid, libc::SIGTERM);
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        unsafe {
            libc::killpg(pgid, libc::SIGKILL);
        }
        // Reap the direct child so it is not left a zombie.
        let _ = child.wait().await;

        // Give the kernel a moment to tear the grandchild down.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert!(
            !pid_alive(grandchild_pid),
            "grandchild (pid {grandchild_pid}) should be reaped by killpg on the group; \
             it orphaned instead — process_group(0) wiring is broken"
        );
    }

    /// `killpg` on an already-dead group is harmless (returns ESRCH), so the shutdown
    /// path is safe to run on every event_loop exit even if the child died on its own.
    #[tokio::test]
    async fn killpg_on_dead_group_is_harmless() {
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("exit 0")
            .process_group(0)
            .kill_on_drop(true)
            .spawn()
            .expect("spawn short-lived child");
        let pgid = child.id().expect("child has a pid") as libc::pid_t;
        let _ = child.wait().await; // child exits immediately

        // Signalling the now-empty group must not panic / must be tolerated.
        unsafe {
            libc::killpg(pgid, libc::SIGTERM);
            libc::killpg(pgid, libc::SIGKILL);
        }
        // No assertion on errno — the contract is "does not blow up". ESRCH is expected.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reg(id: &str, method: &str) -> Value {
        json!({
            "jsonrpc": "2.0",
            "method": "client/registerCapability",
            "params": { "registrations": [ { "id": id, "method": method } ] }
        })
    }

    #[test]
    fn cache_registration_dedups_repeated_identical_registrations() {
        let msg = reg("reg-1", "textDocument/didChange");
        let mut cache = Vec::new();
        for _ in 0..100 {
            cache_registration(&mut cache, &msg);
        }
        assert_eq!(
            cache.len(),
            1,
            "identical re-registration must not grow the replay cache"
        );
    }

    // ---- memory watchdog (classify_memory / read_mem_available_kb) ----

    const GIB_KB: u64 = 1024 * 1024; // 1 GiB expressed in KiB

    fn th(rss_ceil_mb: u64, avail_floor_mb: u64, kill_enabled: bool) -> MemThresholds {
        MemThresholds {
            warn_kb: 4 * 1024 * 1024,
            error_kb: 8 * 1024 * 1024,
            kill_rss_ceil_kb: rss_ceil_mb * 1024,
            kill_avail_floor_kb: avail_floor_mb * 1024,
            kill_enabled,
        }
    }

    #[test]
    fn classify_memory_logs_below_thresholds() {
        let t = th(24 * 1024, 15 * 1024, true);
        assert_eq!(
            classify_memory(GIB_KB, 0, Some(60 * GIB_KB), &t),
            MemAction::Ok
        );
        assert_eq!(
            classify_memory(5 * GIB_KB, 0, Some(60 * GIB_KB), &t),
            MemAction::Warn
        );
        assert_eq!(
            classify_memory(9 * GIB_KB, 0, Some(60 * GIB_KB), &t),
            MemAction::Error
        );
    }

    #[test]
    fn classify_memory_kills_on_absolute_rss_ceiling() {
        let t = th(24 * 1024, 15 * 1024, true);
        // 25 GiB rss with plenty of host RAM free — absolute ceiling still fires.
        assert_eq!(
            classify_memory(25 * GIB_KB, 0, Some(90 * GIB_KB), &t),
            MemAction::Kill("rss_ceiling")
        );
        // swap counts toward the total.
        assert_eq!(
            classify_memory(23 * GIB_KB, 2 * GIB_KB, Some(90 * GIB_KB), &t),
            MemAction::Kill("rss_ceiling")
        );
    }

    #[test]
    fn classify_memory_kills_on_host_pressure_only_when_culpable() {
        let t = th(24 * 1024, 15 * 1024, true);
        // Host low (10 GiB avail) AND this LSP is large (10 GiB >= 8 GiB error) → kill.
        assert_eq!(
            classify_memory(10 * GIB_KB, 0, Some(10 * GIB_KB), &t),
            MemAction::Kill("host_pressure")
        );
        // Host low but this LSP is small (5 GiB < 8 GiB) → spare it, just warn.
        assert_eq!(
            classify_memory(5 * GIB_KB, 0, Some(10 * GIB_KB), &t),
            MemAction::Warn
        );
    }

    #[test]
    fn classify_memory_avail_unknown_disables_pressure_arm() {
        let t = th(24 * 1024, 15 * 1024, true);
        // No MemAvailable reading → host-pressure arm cannot fire; a large-but-under-ceiling
        // process only reaches Error.
        assert_eq!(classify_memory(10 * GIB_KB, 0, None, &t), MemAction::Error);
        // Absolute ceiling still fires without an avail reading.
        assert_eq!(
            classify_memory(30 * GIB_KB, 0, None, &t),
            MemAction::Kill("rss_ceiling")
        );
    }

    #[test]
    fn classify_memory_disabled_never_kills() {
        let t = th(24 * 1024, 15 * 1024, false);
        // Both arms would fire if enabled; disabled → log only (never Kill).
        assert_eq!(
            classify_memory(30 * GIB_KB, 0, Some(2 * GIB_KB), &t),
            MemAction::Error
        );
        assert_eq!(
            classify_memory(10 * GIB_KB, 0, Some(5 * GIB_KB), &t),
            MemAction::Error
        );
    }

    #[test]
    fn read_mem_available_kb_smoke() {
        // On Linux this reads /proc/meminfo; elsewhere it returns None. Must not panic,
        // and when Some, must be a plausible positive value.
        if let Some(kb) = read_mem_available_kb() {
            assert!(kb > 0, "MemAvailable should be positive KiB when readable");
        }
    }

    #[test]
    fn cache_registration_keeps_distinct_and_replaces_superseded() {
        let mut cache = Vec::new();
        cache_registration(&mut cache, &reg("a", "m-a"));
        cache_registration(&mut cache, &reg("b", "m-b"));
        assert_eq!(
            cache.len(),
            2,
            "distinct registration ids must both be retained"
        );

        // Re-register `a` — supersedes the prior `a` entry; cache stays bounded at 2.
        cache_registration(&mut cache, &reg("a", "m-a-v2"));
        assert_eq!(
            cache.len(),
            2,
            "re-registering an existing id must replace, not append"
        );

        let a_entry = cache
            .iter()
            .find(|m| registration_ids(m) == vec!["a".to_string()])
            .expect("registration a must still be cached");
        assert_eq!(
            a_entry["params"]["registrations"][0]["method"], "m-a-v2",
            "the retained entry for id `a` must be the newest registration"
        );
    }

    #[test]
    fn cache_registration_supersedes_prior_entry_when_batch_covers_its_ids() {
        let mut cache = Vec::new();
        cache_registration(&mut cache, &reg("x", "m-x"));
        // A batch covering `x` plus a new id `y` supersedes the standalone `x` entry.
        cache_registration(
            &mut cache,
            &json!({
                "params": { "registrations": [
                    { "id": "x", "method": "m-x-v2" },
                    { "id": "y", "method": "m-y" }
                ] }
            }),
        );
        assert_eq!(
            cache.len(),
            1,
            "a batch covering all ids of a prior entry supersedes it"
        );
    }
}
