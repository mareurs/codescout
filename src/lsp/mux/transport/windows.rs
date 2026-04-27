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
use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY};

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
///
/// **Race window note.** Between `connect()` returning and the next
/// `ServerOptions::create()` call, no instance is bound to the pipe name.
/// A client connecting in that microsecond window sees `ERROR_FILE_NOT_FOUND`
/// rather than `ERROR_PIPE_BUSY`. The client-side `connect` function retries
/// both error codes for exactly that reason — see `connect` below.
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
/// existing server bound to this name is rejected — a defence against
/// pipe-squatting (a different process registering the same name first).
///
/// **Recovery story for crashes.** When the prior mux process exits, the
/// kernel releases its pipe handles; the next mux can bind cleanly. The
/// only path that defeats `first_pipe_instance(true)` is a same-user
/// process that is *still alive* and holds an instance — there is no
/// equivalent of `cleanup()` for named pipes (the kernel namespace is
/// not file-backed). In practice the lock-file flock in `manager.rs`
/// guards against two muxes for the same workspace running concurrently,
/// so this should not arise outside developer mistakes.
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
/// Two retryable error modes:
///
/// * `ERROR_PIPE_BUSY` — all server instances are currently servicing
///   other clients. The mux normally pre-creates a fresh instance after
///   each `accept`, so this is rare in practice.
/// * `ERROR_FILE_NOT_FOUND` — no instance is currently bound. This
///   covers (a) the gap between `Listener::accept` returning and the
///   next `ServerOptions::create` call, and (b) the cold-start window
///   before the first `bind` runs (though `manager.rs` waits for the
///   mux's "ready" stdout line before calling this function, so case
///   (b) is normally already covered).
///
/// 30 attempts × 100ms = 3s budget. Errors other than the two retryable
/// codes return immediately.
pub async fn connect(path: &Path) -> Result<ClientStream> {
    const MAX_ATTEMPTS: u32 = 30;
    const RETRY_DELAY: Duration = Duration::from_millis(100);

    let name: &OsStr = path.as_os_str();
    let mut last_err = None;
    for _ in 0..MAX_ATTEMPTS {
        match ClientOptions::new().open(name) {
            Ok(client) => return Ok(client),
            Err(e) if is_retryable(&e) => {
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
    .with_context(|| {
        format!(
            "Failed to connect to mux pipe (no instance available): {:?}",
            path
        )
    })
}

fn is_retryable(e: &std::io::Error) -> bool {
    matches!(
        e.raw_os_error(),
        Some(code) if code == ERROR_PIPE_BUSY as i32 || code == ERROR_FILE_NOT_FOUND as i32
    )
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
