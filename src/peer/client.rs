//! Requester side of peer delegation. Holds one connection to a peer's socket.
//! Phase 1 is synchronous: one in-flight request per connection, so the envelope
//! `id` is sufficient for correlation (no tag multiplexing needed).

use std::path::Path;

use anyhow::{anyhow, Result};
use serde_json::Value;
use tokio::io::BufReader;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::UnixStream;

use crate::lsp::transport::{read_message, write_message};
use crate::peer::protocol::{Capabilities, EnvelopeKind, PeerEnvelope};
use crate::tools::RecoverableError;

pub struct PeerClient {
    rd: BufReader<OwnedReadHalf>,
    wr: OwnedWriteHalf,
    next_id: u64,
}

impl PeerClient {
    pub async fn connect(socket_path: &Path) -> Result<Self> {
        let stream = UnixStream::connect(socket_path).await.map_err(|e| {
            anyhow!(
                "failed to connect to peer socket {}: {e}",
                socket_path.display()
            )
        })?;
        let (rd, wr) = stream.into_split();
        Ok(Self {
            rd: BufReader::new(rd),
            wr,
            next_id: 0,
        })
    }

    fn next_id(&mut self) -> String {
        self.next_id += 1;
        format!("c:{}", self.next_id)
    }

    /// Send a request and await the single correlated reply. A peer *error* envelope
    /// becomes a `RecoverableError` (input-driven; must not abort sibling calls);
    /// transport faults return a plain `anyhow` error.
    async fn round_trip(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id();
        let req = PeerEnvelope::request(&id, method, params);
        write_message(&mut self.wr, &serde_json::to_value(&req)?).await?;
        let resp: PeerEnvelope = serde_json::from_value(read_message(&mut self.rd).await?)?;

        match resp.kind {
            EnvelopeKind::Response => resp
                .result
                .ok_or_else(|| anyhow!("peer response missing result")),
            EnvelopeKind::Error => {
                let err = resp
                    .error
                    .ok_or_else(|| anyhow!("peer error envelope missing error field"))?;
                Err(RecoverableError {
                    message: format!("peer error [{:?}]: {}", err.code, err.message),
                    guidance: None,
                    extra: Box::new(serde_json::Map::new()),
                }
                .into())
            }
            other => Err(anyhow!("unexpected envelope kind from peer: {other:?}")),
        }
    }

    pub async fn hello(&mut self) -> Result<Capabilities> {
        let v = self.round_trip("hello", serde_json::json!({})).await?;
        Ok(serde_json::from_value(v)?)
    }

    pub async fn call_tool(&mut self, tool: &str, args: Value) -> Result<Value> {
        self.round_trip(
            "tool.call",
            serde_json::json!({ "tool": tool, "args": args }),
        )
        .await
    }

    pub async fn read_buffer(&mut self, handle: &str) -> Result<Value> {
        self.round_trip("buffer.read", serde_json::json!({ "handle": handle }))
            .await
    }

    pub async fn grep_buffer(&mut self, handle: &str, pattern: &str) -> Result<Value> {
        self.round_trip(
            "buffer.grep",
            serde_json::json!({ "handle": handle, "pattern": pattern }),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer::server::{accept_one, bind_peer_socket, build_server_for};

    #[tokio::test]
    async fn client_hello_then_tool_call() {
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

        // connect with retry — socket may not be bound yet
        let mut client = None;
        for _ in 0..50 {
            if let Ok(c) = PeerClient::connect(&sock).await {
                client = Some(c);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        let mut client = client.expect("peer socket never came up");

        let caps = client.hello().await.unwrap();
        assert!(caps.read_only, "peer was built read_only=true");
        assert!(
            caps.tools.iter().any(|t| t == "symbols"),
            "expected 'symbols' in exposed tools, got: {:?}",
            caps.tools
        );

        let result = client
            .call_tool("tree", serde_json::json!({ "path": "." }))
            .await
            .unwrap();
        assert!(
            result.is_object()
                || result.is_array()
                || result.get("content").is_some()
                || !result.is_null(),
            "expected non-null result from tree, got: {result}"
        );

        // a non-exposed write tool returns a recoverable peer error, not a transport panic
        let denied = client
            .call_tool(
                "create_file",
                serde_json::json!({ "path": "x", "content": "y" }),
            )
            .await;
        assert!(
            denied.is_err(),
            "create_file is not exposed → expected peer error, got Ok"
        );

        handle.abort();
    }
}
