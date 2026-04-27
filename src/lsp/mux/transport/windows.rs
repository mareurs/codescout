//! Windows named-pipe implementation of the mux transport.
//!
//! Endpoints are pipe names in the form `\\.\pipe\codescout-<lang>-mux-<hash>`.
//! Pipes live in a kernel namespace (not the filesystem) and are scoped per
//! session; the OS handles cleanup automatically when the last handle closes.
//! Per-user isolation comes from the default ACL applied by `CreateNamedPipeW`,
//! which restricts the pipe to the creating user's token.
//!
//! The named-pipe lifecycle differs from Unix domain sockets in one important
//! way: each `NamedPipeServer` instance accepts exactly one client. To accept
//! multiple clients we hold a pre-created server instance, await `connect()`
//! on it, then immediately create the next instance. The connected instance
//! is returned as the `ServerStream`.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::io::{ReadHalf, WriteHalf};
use tokio::net::windows::named_pipe::{
    ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
};

pub type ServerStream = NamedPipeServer;
pub type ServerReadHalf = ReadHalf<NamedPipeServer>;
pub type ServerWriteHalf = WriteHalf<NamedPipeServer>;

pub type ClientStream = NamedPipeClient;
pub type ClientReadHalf = ReadHalf<NamedPipeClient>;
pub type ClientWriteHalf = WriteHalf<NamedPipeClient>;

/// Server-side listener for incoming mux connections.
///
/// Holds a pre-created pipe instance ready to accept the next client.
/// `accept` rotates: it awaits a connection on the held instance, creates a
/// fresh instance for the next client, and returns the connected one.
pub struct Listener {
    name: OsString,
    pending: NamedPipeServer,
}

impl Listener {
    pub async fn accept(&mut self) -> Result<ServerStream> {
        self.pending
            .connect()
            .await
            .context("failed to accept client on named pipe")?;
        let next = ServerOptions::new()
            .create(&self.name)
            .context("failed to create next named-pipe instance")?;
        let connected = std::mem::replace(&mut self.pending, next);
        Ok(connected)
    }
}

/// Compute the endpoint name for a (language, workspace_hash) pair.
///
/// Windows endpoints are kernel pipe names; the `per_user_dir` argument is
/// ignored — pipes live outside the filesystem.
pub fn endpoint_path(_per_user_dir: &Path, language: &str, workspace_hash: &str) -> PathBuf {
    PathBuf::from(format!(
        r"\\.\pipe\codescout-{language}-mux-{workspace_hash}"
    ))
}

/// Bind a server endpoint at `path` (interpreted as a named-pipe name).
///
/// Sets `first_pipe_instance(true)` on the initial instance so that any
/// stale server still bound to this name is rejected — a defence against
/// pipe-squatting if the previous mux process crashed without cleanup.
pub async fn bind(path: &Path) -> Result<Listener> {
    let name: OsString = path.as_os_str().to_owned();
    let pending = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&name)
        .with_context(|| format!("failed to bind named pipe: {}", path.display()))?;
    Ok(Listener { name, pending })
}

/// Connect a client endpoint to the server at `path`.
///
/// `ClientOptions::open` returns `ERROR_PIPE_BUSY` if all server instances
/// are currently servicing other clients. We retry with a short backoff
/// before giving up.
pub async fn connect(path: &Path) -> Result<ClientStream> {
    const MAX_ATTEMPTS: u32 = 20;
    const RETRY_DELAY: Duration = Duration::from_millis(100);

    let name: &OsStr = path.as_os_str();
    let mut last_err = None;
    for _ in 0..MAX_ATTEMPTS {
        match ClientOptions::new().open(name) {
            Ok(client) => return Ok(client),
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => {
                last_err = Some(e);
                tokio::time::sleep(RETRY_DELAY).await;
            }
            Err(e) => {
                return Err(anyhow::Error::from(e))
                    .with_context(|| format!("Failed to connect to mux pipe: {:?}", path));
            }
        }
    }
    Err(anyhow::Error::from(
        last_err.expect("loop ran at least once"),
    ))
    .with_context(|| format!("Failed to connect to mux pipe (busy): {:?}", path))
}

/// Best-effort cleanup of a bound endpoint. No-op on Windows: named pipes
/// live in a kernel namespace and are released automatically when their
/// last handle closes.
pub fn cleanup(_path: &Path) {}

/// Split a server-side stream into read and write halves.
pub fn split_server(stream: ServerStream) -> (ServerReadHalf, ServerWriteHalf) {
    tokio::io::split(stream)
}

/// Split a client-side stream into read and write halves.
pub fn split_client(stream: ClientStream) -> (ClientReadHalf, ClientWriteHalf) {
    tokio::io::split(stream)
}

// `ERROR_PIPE_BUSY` from winapi: returned by CreateFile/Open when all pipe
// instances are busy. Defined here as a constant to avoid pulling in a
// winapi dep just for one number.
const ERROR_PIPE_BUSY: u32 = 231;
