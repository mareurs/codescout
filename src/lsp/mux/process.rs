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
use fs2::FileExt;
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
    let mut child = Command::new(server_command)
        .args(server_args)
        .envs(server_env.iter().map(|(k, v)| (k, v)))
        .current_dir(workspace_root)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("failed to spawn LSP server: {server_command}"))?;

    let server_stdin = child.stdin.take().context("no stdin on child")?;
    let server_stdout = child.stdout.take().context("no stdout on child")?;

    let server_writer: SharedWriter = Arc::new(Mutex::new(
        Box::new(server_stdin) as Box<dyn AsyncWrite + Unpin + Send>
    ));
    let mut server_reader = BufReader::new(server_stdout);

    // Spawn stderr logger
    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => debug!(target: "mux::server_stderr", "{}", line.trim_end()),
                }
            }
        });
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
                    "documentSymbol": { "dynamicRegistration": true },
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

    // Shutdown: remove socket file
    std::fs::remove_file(socket_path).ok();
    // flock released when lock_file drops

    info!("mux process shutting down");
    result
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
                st.cached_capabilities.push(msg.clone());
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
