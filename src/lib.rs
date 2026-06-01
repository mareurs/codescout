//! codescout: high-performance coding agent MCP server.
//!
//! Provides IDE-grade code intelligence to LLMs via the Model Context Protocol.
#![recursion_limit = "256"]

/// Install rustls' ring crypto provider as the default. Idempotent — safe to
/// call from multiple entry points (binary `main`, integration tests, library
/// users). Required because reqwest uses `rustls-no-provider` feature: callers
/// must install a provider before the first TLS handshake.
pub fn install_default_crypto_provider() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

pub mod agent;
pub mod ast;
pub mod cli;
pub mod config;
#[cfg(feature = "dashboard")]
pub mod dashboard;
pub mod embed;
pub mod fs;

pub mod git;
pub mod hardware;
#[cfg(feature = "librarian")]
pub mod librarian;
pub mod library;
pub mod logging;
pub mod lsp;
pub mod mcp_resources;
pub mod memory;
pub mod migrate;
pub mod peer;
pub mod platform;
pub mod prompts;
pub mod retrieval;
pub mod server;
pub mod socket_discovery;
pub mod symbol;
pub mod tools;
pub mod usage;
pub mod util;
pub mod workspace;
