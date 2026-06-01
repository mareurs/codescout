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

/// A bound peer-serve context: the server for the served workspace plus the
/// read-only grant. Read-only is enforced at THIS layer (Task 6), because the
/// served workspace is the Agent's home and is therefore always read-write.
pub struct PeerServe {
    pub server: Arc<CodeScoutServer>,
    pub read_only: bool,
}

/// Construct a `CodeScoutServer` for `root` and wrap it with the read-only grant.
pub async fn build_server_for(root: &Path, read_only: bool) -> Result<PeerServe> {
    let agent = Agent::new(Some(root.to_path_buf()))
        .await
        .context("failed to construct agent for peer workspace")?;
    let server = Arc::new(CodeScoutServer::new(agent).await);
    Ok(PeerServe { server, read_only })
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

/// Route one request envelope to its handler. Tasks 6-7 add more arms.
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
        tools: ctx.server.tool_names(),
        executor_available: false, // Phase 2
    };
    PeerEnvelope::response(id, serde_json::to_value(caps).unwrap_or(json!({})))
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
}
