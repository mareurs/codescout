//! Platform-specific IPC transport for the mux ↔ client connection.
//!
//! Unix uses Unix domain sockets; Windows uses named pipes. Callers operate
//! on the type aliases (`Listener`, `ServerStream`, `ClientStream`,
//! `ServerReadHalf`, etc.) and the free functions exported here — they
//! should not reach into the underlying tokio types directly. Adding a
//! third transport (e.g. TCP loopback) means adding one more sibling
//! module, not editing call sites.

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use self::unix::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;
