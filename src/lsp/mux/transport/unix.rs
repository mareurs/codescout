//! Unix-domain-socket implementation of the mux transport.
//!
//! Endpoints are filesystem entries under the per-user mux dir
//! (see `crate::lsp::mux::per_user_mux_dir`). The endpoint file must be
//! removed before re-binding (UDS lifecycle) and is restricted to the current
//! user via mode `0o600` as defence-in-depth on top of the per-user directory.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub type ServerStream = tokio::net::UnixStream;
pub type ServerReadHalf = tokio::net::unix::OwnedReadHalf;
pub type ServerWriteHalf = tokio::net::unix::OwnedWriteHalf;

pub type ClientStream = tokio::net::UnixStream;
pub type ClientReadHalf = tokio::net::unix::OwnedReadHalf;
pub type ClientWriteHalf = tokio::net::unix::OwnedWriteHalf;

/// Server-side listener for incoming mux connections.
pub struct Listener(tokio::net::UnixListener);

impl Listener {
    pub async fn accept(&mut self) -> Result<ServerStream> {
        let (stream, _addr) = self.0.accept().await.context("failed to accept client")?;
        Ok(stream)
    }
}

/// Compute the endpoint path for a (language, workspace_hash) pair.
///
/// Unix endpoints live in `per_user_dir` as `.sock` files; the directory
/// is determined by `crate::lsp::mux::per_user_mux_dir`.
pub fn endpoint_path(per_user_dir: &Path, language: &str, workspace_hash: &str) -> PathBuf {
    per_user_dir.join(format!("codescout-{language}-mux-{workspace_hash}.sock"))
}

/// Bind a server endpoint at `path`.
///
/// Removes any stale endpoint, binds, then restricts the endpoint to the
/// current user (mode 0o600). Returns the bound listener.
pub async fn bind(path: &Path) -> Result<Listener> {
    cleanup(path);
    let inner = tokio::net::UnixListener::bind(path)
        .with_context(|| format!("failed to bind socket: {}", path.display()))?;
    restrict_to_user(path);
    Ok(Listener(inner))
}

/// Connect a client endpoint to the server at `path`.
pub async fn connect(path: &Path) -> Result<ClientStream> {
    ClientStream::connect(path)
        .await
        .with_context(|| format!("Failed to connect to mux socket: {:?}", path))
}

/// Best-effort cleanup of a bound endpoint. Idempotent; no-op if absent.
pub fn cleanup(path: &Path) {
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
}

/// Split a server-side stream into owned read and write halves.
pub fn split_server(stream: ServerStream) -> (ServerReadHalf, ServerWriteHalf) {
    stream.into_split()
}

/// Split a client-side stream into owned read and write halves.
pub fn split_client(stream: ClientStream) -> (ClientReadHalf, ClientWriteHalf) {
    stream.into_split()
}

fn restrict_to_user(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}
