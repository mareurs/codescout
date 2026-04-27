//! Platform-specific IPC transport for the mux ↔ client connection.
//!
//! Unix uses Unix domain sockets. Windows will use named pipes (Phase B).
//! Callers operate on the `Listener` / `Stream` type aliases and the free
//! functions exported here — they should not reach into the underlying tokio
//! types directly. Adding a third transport (e.g. TCP loopback) means adding
//! one more sibling module, not editing call sites.

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use self::unix::*;
