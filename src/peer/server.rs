//! The `codescout peer-serve` process: serve one workspace's read tools over a
//! per-workspace Unix socket. Phase 1 = synchronous remote tools.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde_json::json;
use tokio::io::BufReader;
use tokio::net::{UnixListener, UnixStream};

use crate::agent::Agent;
use crate::lsp::transport::{read_message, write_message};
use crate::peer::protocol::{Capabilities, EnvelopeKind, ErrorCode, PeerEnvelope, PeerError};
use crate::server::CodeScoutServer;

/// Tools a peer may invoke in Phase 1 — deny-by-default. Phase 1 is read-only
/// delegation: only these explicit safe-read tools are exposed; every other tool
/// (writes, `run_command`, `workspace`, librarian mutations) is rejected by
/// construction, independent of the peer's `read_only` grant and robust to any
/// `Tool::is_write` coverage gap. Write delegation is a later phase with its own
/// explicit curated set.
const PEER_EXPOSED_TOOLS: &[&str] = &[
    "symbols",
    "symbol_at",
    "references",
    "call_graph",
    "tree",
    "grep",
    "semantic_search",
    "read_file",
    "read_markdown",
    "get_guide",
];

/// A bound peer-serve context: the server for the served workspace, the
/// read-only grant, and the audit-log path. The `read_only` grant is
/// **advisory** — it is advertised via `hello` (`Capabilities.read_only`) but
/// is NOT used for enforcement. Access is enforced by the `PEER_EXPOSED_TOOLS`
/// allow-list in `handle_tool_call_inner`: the served workspace is the Agent's
/// home and is therefore always read-write, so a peer-layer `read_only` gate
/// would be redundant (see spec D-Wall, revised 2026-06-01).
pub struct PeerServe {
    pub server: Arc<CodeScoutServer>,
    pub read_only: bool,
    pub audit_path: Option<std::path::PathBuf>,
}

/// Construct a `CodeScoutServer` for `root` and wrap it with the read-only grant.
///
/// When `read_only`, flip the served (default) workspace's `read_only` flag so the
/// Agent write-guard engages as a second layer behind the `PEER_EXPOSED_TOOLS`
/// allow-list. `Agent::new` makes `root` the home, and home is read-write by
/// default (`build_workspace`'s `is_home => rw` invariant); peer-serve is a pure
/// reader, so we override that here. Peer dispatch is unpinned and resolves to
/// the default workspace, so this covers every served call.
pub async fn build_server_for(root: &Path, read_only: bool) -> Result<PeerServe> {
    let agent = Agent::new(Some(root.to_path_buf()))
        .await
        .context("failed to construct agent for peer workspace")?;
    if read_only {
        agent
            .with_project_at_mut(None, |p| -> anyhow::Result<()> {
                p.read_only = true;
                Ok(())
            })
            .await
            .context("failed to mark peer workspace read-only")?;
    }
    let server = Arc::new(CodeScoutServer::new(agent).await);
    let audit_path = Some(root.join(".codescout").join("peer-audit.jsonl"));
    Ok(PeerServe {
        server,
        read_only,
        audit_path,
    })
}

/// Bind the per-user peer socket, restricted to mode 0600.
pub fn bind_peer_socket(socket_path: &Path) -> Result<UnixListener> {
    if socket_path.exists() {
        std::fs::remove_file(socket_path).ok();
    }
    let listener = UnixListener::bind(socket_path)
        .with_context(|| format!("failed to bind peer socket: {}", socket_path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(listener)
}

/// Accept a single connection and serve it to completion (used by tests and the
/// Phase-1 sequential accept loop).
pub async fn accept_one(listener: &UnixListener, ctx: &PeerServe) -> Result<()> {
    let (stream, _addr) = listener.accept().await?;
    serve_connection(stream, ctx).await
}
/// Run the peer-serve process for `workspace`: acquire the single-instance
/// flock, bind the socket, signal `ready`, and serve connections until the
/// idle timeout elapses with no connection. Mirrors `lsp::mux::process::run`.
pub async fn run(
    socket_path: &Path,
    workspace: &Path,
    read_only: bool,
    idle_timeout_secs: u64,
) -> Result<()> {
    let lock_path = crate::socket_discovery::peer_lock_path_for_workspace(workspace);
    run_with_lock(
        socket_path,
        &lock_path,
        workspace,
        read_only,
        idle_timeout_secs,
    )
    .await
}

/// Inner form taking an explicit lock path, for tests that control the lock.
async fn run_with_lock(
    socket_path: &Path,
    lock_path: &Path,
    workspace: &Path,
    read_only: bool,
    idle_timeout_secs: u64,
) -> Result<()> {
    use fs4::fs_std::FileExt;

    let lock_file = {
        let mut opts = std::fs::OpenOptions::new();
        opts.create(true).write(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        opts.open(lock_path)
            .with_context(|| format!("failed to open peer lock file: {}", lock_path.display()))?
    };
    if lock_file.try_lock_exclusive().is_err() {
        tracing::info!(
            "peer-serve already running for {}, exiting",
            workspace.display()
        );
        return Ok(());
    }

    let ctx = build_server_for(workspace, read_only).await?;
    let listener = bind_peer_socket(socket_path)?;

    {
        use tokio::io::AsyncWriteExt;
        let mut stdout = tokio::io::stdout();
        stdout.write_all(b"ready\n").await.ok();
        stdout.flush().await.ok();
    }

    let idle = std::time::Duration::from_secs(idle_timeout_secs);
    loop {
        match tokio::time::timeout(idle, listener.accept()).await {
            Ok(Ok((stream, _addr))) => {
                if serve_connection(stream, &ctx).await.is_err() {
                    break;
                }
            }
            Ok(Err(e)) => {
                tracing::warn!("peer-serve accept error: {e}");
                break;
            }
            Err(_elapsed) => {
                tracing::info!(
                    "peer-serve idle timeout reached ({idle_timeout_secs}s), shutting down"
                );
                break;
            }
        }
    }
    std::fs::remove_file(socket_path).ok();
    Ok(())
}

/// Serve one client connection: read envelopes, dispatch, write replies, until EOF.
async fn serve_connection(stream: UnixStream, ctx: &PeerServe) -> Result<()> {
    let (rd, mut wr) = stream.into_split();
    let mut rd = BufReader::new(rd);
    loop {
        let msg = match read_message(&mut rd).await {
            Ok(m) => m,
            Err(_) => return Ok(()), // EOF / client gone
        };
        let env: PeerEnvelope = match serde_json::from_value(msg) {
            Ok(e) => e,
            Err(e) => {
                let err = PeerEnvelope::error(
                    "0",
                    PeerError {
                        code: ErrorCode::BadParams,
                        message: e.to_string(),
                        data: None,
                    },
                );
                write_message(&mut wr, &serde_json::to_value(&err)?).await?;
                continue;
            }
        };
        let reply = dispatch_envelope(&env, ctx).await;
        write_message(&mut wr, &serde_json::to_value(&reply)?).await?;
    }
}

/// Route one request envelope to its handler.
async fn dispatch_envelope(env: &PeerEnvelope, ctx: &PeerServe) -> PeerEnvelope {
    if env.kind != EnvelopeKind::Request {
        return PeerEnvelope::error(
            &env.id,
            PeerError {
                code: ErrorCode::BadParams,
                message: "expected a request".into(),
                data: None,
            },
        );
    }
    match env.method.as_deref() {
        Some("hello") => handle_hello(&env.id, ctx).await,
        Some("tool.call") => handle_tool_call(&env.id, env.params.clone(), ctx).await,
        Some("buffer.read") => handle_buffer_read(&env.id, env.params.clone(), ctx).await,
        Some("buffer.grep") => handle_buffer_grep(&env.id, env.params.clone(), ctx).await,
        Some(other) => PeerEnvelope::error(
            &env.id,
            PeerError {
                code: ErrorCode::UnknownMethod,
                message: format!("unknown method: {other}"),
                data: None,
            },
        ),
        None => PeerEnvelope::error(
            &env.id,
            PeerError {
                code: ErrorCode::BadParams,
                message: "missing method".into(),
                data: None,
            },
        ),
    }
}

async fn handle_hello(id: &str, ctx: &PeerServe) -> PeerEnvelope {
    let caps = Capabilities {
        project: ctx.server.project_name().await,
        root: ctx.server.project_root_string().await,
        read_only: ctx.read_only,
        tools: ctx
            .server
            .tool_names()
            .into_iter()
            .filter(|t| PEER_EXPOSED_TOOLS.contains(&t.as_str()))
            .collect(),
        executor_available: false, // Phase 2
    };
    PeerEnvelope::response(id, serde_json::to_value(caps).unwrap_or(json!({})))
}

async fn handle_tool_call_inner(
    id: &str,
    params: Option<serde_json::Value>,
    ctx: &PeerServe,
) -> PeerEnvelope {
    let params = match params {
        Some(p) => p,
        None => return bad_params(id, "tool.call requires params"),
    };
    let tool = match params.get("tool").and_then(|t| t.as_str()) {
        Some(t) => t.to_string(),
        None => return bad_params(id, "tool.call requires a 'tool' name"),
    };
    let args = params
        .get("args")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));

    // Deny-by-default: only the explicit safe-read allow-list is exposed over the
    // peer protocol. Everything else (writes, run_command, workspace, librarian
    // mutations) is rejected here, before dispatch — independent of read_only and
    // robust to is_write coverage gaps. (Phase 1 = read-only delegation.)
    if !PEER_EXPOSED_TOOLS.contains(&tool.as_str()) {
        return PeerEnvelope::error(
            id,
            PeerError {
                code: ErrorCode::AccessDenied,
                message: format!(
                    "tool '{tool}' is not exposed over the peer protocol (read-only delegation)"
                ),
                data: None,
            },
        );
    }

    match ctx.server.call_tool_by_name(&tool, args).await {
        Ok(result) => {
            let body = serde_json::to_value(&result).unwrap_or(serde_json::Value::Null);
            if result.is_error.unwrap_or(false) {
                PeerEnvelope::error(
                    id,
                    PeerError {
                        code: ErrorCode::ToolError,
                        message: "tool returned an error".into(),
                        data: Some(body),
                    },
                )
            } else {
                PeerEnvelope::response(id, body)
            }
        }
        Err(e) => PeerEnvelope::error(
            id,
            PeerError {
                code: ErrorCode::UnknownTool,
                message: e.to_string(),
                data: None,
            },
        ),
    }
}
async fn handle_tool_call(
    id: &str,
    params: Option<serde_json::Value>,
    ctx: &PeerServe,
) -> PeerEnvelope {
    let tool_name = params
        .as_ref()
        .and_then(|p| p.get("tool"))
        .and_then(|t| t.as_str())
        .unwrap_or("?")
        .to_string();
    let reply = handle_tool_call_inner(id, params, ctx).await;
    if let Some(path) = &ctx.audit_path {
        let record =
            serde_json::json!({ "id": id, "tool": tool_name, "ok": reply.error.is_none() });
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            use std::io::Write as _;
            let _ = writeln!(f, "{record}");
        }
    }
    reply
}

async fn handle_buffer_read(
    id: &str,
    params: Option<serde_json::Value>,
    ctx: &PeerServe,
) -> PeerEnvelope {
    let handle = match params
        .as_ref()
        .and_then(|p| p.get("handle"))
        .and_then(|h| h.as_str())
    {
        Some(h) => h.to_string(),
        None => return bad_params(id, "buffer.read requires a 'handle'"),
    };
    match ctx.server.output_buffer_ref().get(&handle) {
        Some(entry) => {
            let content = if entry.stderr.is_empty() {
                entry.stdout
            } else {
                format!("{}\n{}", entry.stdout, entry.stderr)
            };
            PeerEnvelope::response(id, serde_json::json!({ "content": content }))
        }
        None => PeerEnvelope::error(
            id,
            PeerError {
                code: ErrorCode::UnknownHandle,
                message: format!("no such handle: {handle}"),
                data: None,
            },
        ),
    }
}

async fn handle_buffer_grep(
    id: &str,
    params: Option<serde_json::Value>,
    ctx: &PeerServe,
) -> PeerEnvelope {
    let p = params.unwrap_or_else(|| serde_json::json!({}));
    let handle = match p.get("handle").and_then(|h| h.as_str()) {
        Some(h) => h.to_string(),
        None => return bad_params(id, "buffer.grep requires a 'handle'"),
    };
    let pattern = match p.get("pattern").and_then(|h| h.as_str()) {
        Some(s) => s.to_string(),
        None => return bad_params(id, "buffer.grep requires a 'pattern'"),
    };
    let re = match regex::Regex::new(&pattern) {
        Ok(r) => r,
        Err(e) => return bad_params(id, &format!("invalid regex: {e}")),
    };
    match ctx.server.output_buffer_ref().get(&handle) {
        Some(entry) => {
            let matched: Vec<&str> = entry.stdout.lines().filter(|l| re.is_match(l)).collect();
            PeerEnvelope::response(
                id,
                serde_json::json!({ "matches": matched, "count": matched.len() }),
            )
        }
        None => PeerEnvelope::error(
            id,
            PeerError {
                code: ErrorCode::UnknownHandle,
                message: format!("no such handle: {handle}"),
                data: None,
            },
        ),
    }
}

fn bad_params(id: &str, msg: &str) -> PeerEnvelope {
    PeerEnvelope::error(
        id,
        PeerError {
            code: ErrorCode::BadParams,
            message: msg.into(),
            data: None,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp::transport::{read_message, write_message};
    use crate::peer::protocol::{PeerEnvelope, PROTOCOL_VERSION};
    use tokio::io::BufReader;
    use tokio::net::UnixStream;

    async fn connect_with_retry(sock: &std::path::Path) -> UnixStream {
        for _ in 0..50 {
            if let Ok(s) = UnixStream::connect(sock).await {
                return s;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        panic!("peer socket never came up");
    }

    #[tokio::test]
    async fn hello_returns_capabilities_for_read_only_peer() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        let sock = root.join("peer.sock");

        let (sr, ss) = (root.clone(), sock.clone());
        let handle = tokio::spawn(async move {
            let ctx = build_server_for(&sr, true).await.unwrap();
            let listener = bind_peer_socket(&ss).unwrap();
            accept_one(&listener, &ctx).await.unwrap();
        });

        let stream = connect_with_retry(&sock).await;
        let (rd, mut wr) = stream.into_split();
        let mut rd = BufReader::new(rd);

        let hello = PeerEnvelope::request("a:1", "hello", serde_json::json!({}));
        write_message(&mut wr, &serde_json::to_value(&hello).unwrap())
            .await
            .unwrap();

        let resp: PeerEnvelope =
            serde_json::from_value(read_message(&mut rd).await.unwrap()).unwrap();
        assert_eq!(resp.v, PROTOCOL_VERSION);
        let caps = resp.result.unwrap();
        assert_eq!(caps["read_only"], true);
        assert!(caps["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t == "symbols"));

        handle.abort();
    }

    #[tokio::test]
    async fn build_server_for_read_only_disables_writes_on_default_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join(".codescout")).unwrap();

        let ctx = build_server_for(&root, true).await.unwrap();
        let sec = ctx.server.agent_security_config().await;
        assert!(
            !sec.file_write_enabled,
            "read-only peer-serve must disable file writes on the served workspace"
        );
    }

    #[tokio::test]
    async fn tool_call_runs_an_exposed_read_tool() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        std::fs::write(root.join("a.txt"), "hello").unwrap();
        let sock = root.join("peer.sock");

        let (sr, ss) = (root.clone(), sock.clone());
        let handle = tokio::spawn(async move {
            let ctx = build_server_for(&sr, true).await.unwrap();
            let listener = bind_peer_socket(&ss).unwrap();
            accept_one(&listener, &ctx).await.unwrap();
        });

        let stream = connect_with_retry(&sock).await;
        let (rd, mut wr) = stream.into_split();
        let mut rd = BufReader::new(rd);
        let req = PeerEnvelope::request(
            "a:1",
            "tool.call",
            serde_json::json!({ "tool": "tree", "args": { "path": "." } }),
        );
        write_message(&mut wr, &serde_json::to_value(&req).unwrap())
            .await
            .unwrap();
        let resp: PeerEnvelope =
            serde_json::from_value(read_message(&mut rd).await.unwrap()).unwrap();
        assert!(
            resp.error.is_none(),
            "tree (exposed read tool) should not error: {:?}",
            resp.error
        );
        assert!(resp.result.is_some());
        handle.abort();
    }

    #[tokio::test]
    async fn peer_refuses_unexposed_tools_even_when_read_write() {
        use crate::peer::protocol::ErrorCode;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        // read_only = FALSE on purpose: prove the allow-list (not read_only) is the wall.
        let ctx = build_server_for(&root, false).await.unwrap();

        let cases: Vec<(&str, serde_json::Value, Option<&str>)> = vec![
            (
                "create_file",
                serde_json::json!({"path": "new.txt", "content": "x"}),
                Some("new.txt"),
            ),
            (
                "run_command",
                serde_json::json!({"command": "echo pwned > pwned.txt"}),
                Some("pwned.txt"),
            ),
            (
                "artifact",
                serde_json::json!({"action": "create", "kind": "tracker", "title": "x"}),
                None,
            ),
            (
                "workspace",
                serde_json::json!({"action": "activate", "path": "/tmp"}),
                None,
            ),
            (
                "edit_file",
                serde_json::json!({"path": "a.txt", "old": "x", "new": "y"}),
                None,
            ),
        ];
        for (tool, args, sentinel) in cases {
            let reply = handle_tool_call_inner(
                "a:1",
                Some(serde_json::json!({"tool": tool, "args": args})),
                &ctx,
            )
            .await;
            let err = reply
                .error
                .unwrap_or_else(|| panic!("tool {tool} must be refused (not exposed)"));
            assert_eq!(
                err.code,
                ErrorCode::AccessDenied,
                "tool {tool} should be AccessDenied"
            );
            if let Some(f) = sentinel {
                assert!(
                    !root.join(f).exists(),
                    "tool {tool} must not have created {f}"
                );
            }
        }
    }

    #[tokio::test]
    async fn buffer_read_and_grep_proxy_stored_content() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        let ctx = build_server_for(&root, true).await.unwrap();
        let handle = ctx
            .server
            .output_buffer_ref()
            .store_tool("probe", "LINE_ONE\nLINE_TWO".into());

        let read =
            handle_buffer_read("a:1", Some(serde_json::json!({ "handle": handle })), &ctx).await;
        let body = read.result.expect("buffer.read should return a result");
        let content = body["content"].as_str().unwrap();
        assert!(content.contains("LINE_ONE") && content.contains("LINE_TWO"));

        let handle2 = ctx
            .server
            .output_buffer_ref()
            .store_tool("probe", "LINE_ONE\nLINE_TWO".into());
        let grep = handle_buffer_grep(
            "a:2",
            Some(serde_json::json!({ "handle": handle2, "pattern": "LINE_ONE" })),
            &ctx,
        )
        .await;
        let gbody = grep.result.expect("buffer.grep should return a result");
        assert_eq!(gbody["count"], 1);

        let missing =
            handle_buffer_read("a:3", Some(serde_json::json!({ "handle": "@nope" })), &ctx).await;
        assert_eq!(
            missing.error.unwrap().code,
            crate::peer::protocol::ErrorCode::UnknownHandle
        );
    }

    #[tokio::test]
    async fn served_tool_calls_are_audited() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        let ctx = build_server_for(&root, true).await.unwrap();

        let _ = handle_tool_call(
            "a:1",
            Some(serde_json::json!({ "tool": "tree", "args": { "path": "." } })),
            &ctx,
        )
        .await;
        // a denied call is audited too
        let _ = handle_tool_call(
            "a:2",
            Some(serde_json::json!({ "tool": "create_file", "args": {} })),
            &ctx,
        )
        .await;

        let logged =
            std::fs::read_to_string(root.join(".codescout").join("peer-audit.jsonl")).unwrap();
        assert!(
            logged.contains("\"tool\":\"tree\""),
            "must record tree: {logged}"
        );
        assert!(
            logged.contains("\"tool\":\"create_file\""),
            "must record the denied call: {logged}"
        );
        assert!(logged.contains("\"id\":\"a:1\""));
    }

    #[tokio::test]
    async fn run_exits_after_idle_timeout_with_no_connections() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        let sock = root.join("peer.sock");
        let lock = root.join("peer.lock");

        let res = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            run_with_lock(&sock, &lock, &root, true, 1),
        )
        .await;
        assert!(
            res.is_ok(),
            "run() did not exit within 10s of a 1s idle timeout"
        );
        assert!(res.unwrap().is_ok(), "run() returned an error");
        assert!(!sock.exists(), "socket file should be cleaned up on exit");
    }

    #[tokio::test]
    async fn run_exits_quietly_when_lock_is_held() {
        use fs4::fs_std::FileExt;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        let sock = root.join("peer.sock");
        let lock = root.join("peer.lock");

        let held = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&lock)
            .unwrap();
        held.try_lock_exclusive().unwrap();

        let res = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            run_with_lock(&sock, &lock, &root, true, 30),
        )
        .await;
        assert!(res.is_ok(), "run() blocked despite the lock being held");
        assert!(res.unwrap().is_ok());
        assert!(
            !sock.exists(),
            "run() must not bind the socket when the lock is held"
        );
    }
    #[tokio::test]
    async fn end_to_end_served_read_tool_and_write_denied() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        std::fs::write(root.join("a.txt"), "hello").unwrap();
        let sock = root.join("peer.sock");
        let lock = root.join("peer.lock");

        let (sr, sk, lk) = (root.clone(), sock.clone(), lock.clone());
        let serve = tokio::spawn(async move {
            // long idle so the serve stays up for the test duration
            let _ = run_with_lock(&sk, &lk, &sr, true, 30).await;
        });

        // Wait for the socket to accept connections.
        let mut client = {
            let mut c = None;
            for _ in 0..50 {
                if let Ok(client) = crate::peer::client::PeerClient::connect(&sock).await {
                    c = Some(client);
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            c.expect("could not connect to served socket")
        };

        let _caps = client.hello().await.unwrap();

        // Exposed read tool succeeds.
        let ok = client
            .call_tool("tree", serde_json::json!({ "path": "." }))
            .await;
        assert!(ok.is_ok(), "exposed read tool should succeed: {ok:?}");

        // Non-exposed write tool is denied by the allow-list; no side effect.
        let denied = client
            .call_tool(
                "create_file",
                serde_json::json!({ "path": "x.txt", "content": "no" }),
            )
            .await;
        assert!(denied.is_err(), "create_file must be rejected");
        assert!(
            !root.join("x.txt").exists(),
            "denied write must have no side effect"
        );

        serve.abort();
    }
}
