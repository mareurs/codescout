//! Unix-domain-socket implementation of the mux transport.
//!
//! Endpoints are filesystem entries under the per-user mux dir
//! (see `crate::lsp::mux::per_user_mux_dir`). The endpoint file must be
//! removed before re-binding (UDS lifecycle) and is restricted to the current
//! user via mode `0o600` as defence-in-depth on top of the per-user directory.

use std::path::Path;

use anyhow::{Context, Result};

pub type Listener = tokio::net::UnixListener;
pub type Stream = tokio::net::UnixStream;
pub type OwnedReadHalf = tokio::net::unix::OwnedReadHalf;

/// Bind a server endpoint at `path`.
///
/// Removes any stale endpoint, binds, then restricts the endpoint to the
/// current user. Returns the bound listener.
pub async fn bind(path: &Path) -> Result<Listener> {
    cleanup(path);
    let listener = Listener::bind(path)
        .with_context(|| format!("failed to bind socket: {}", path.display()))?;
    restrict_to_user(path);
    Ok(listener)
}

/// Connect a client endpoint to the server at `path`.
pub async fn connect(path: &Path) -> Result<Stream> {
    Stream::connect(path)
        .await
        .with_context(|| format!("Failed to connect to mux socket: {:?}", path))
}

/// Best-effort cleanup of a bound endpoint. Idempotent; no-op if absent.
pub fn cleanup(path: &Path) {
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
}

fn restrict_to_user(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}
