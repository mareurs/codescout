//! Peer delegation: run another codescout instance's read tools over a
//! per-workspace Unix socket. Phase 1 = synchronous remote tools.

pub mod client;
pub mod protocol;
pub mod registry;
pub mod server;

pub use protocol::{
    Capabilities, EnvelopeKind, ErrorCode, PeerEnvelope, PeerError, PROTOCOL_VERSION,
};

/// Re-exported for convenience; the canonical home is `socket_discovery`.
pub use crate::socket_discovery::{peer_lock_path_for_workspace, peer_socket_path_for_workspace};
