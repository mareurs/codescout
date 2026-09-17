//! codescout: high-performance coding agent MCP server.
//!
//! Provides IDE-grade code intelligence to LLMs via the Model Context Protocol.
//
// Forced by ONE expression: the `json!` literal that builds `doc`'s input schema
// (`librarian/tools/artifact.rs`). `json_internal!` recurses once per token of the
// literal, so the limit tracks that schema's size and nothing else — it was raised to
// 256 when the librarian crate was dissolved in (`d48bf992`), and to 512 when
// `rekey_prefix` added its 18th action and two parameters. The failure is a hard
// `error: recursion limit reached while expanding $crate::json_internal!` pointing at
// the literal, so it is loud; what is not obvious from that message is that the cause
// is cumulative schema growth rather than whatever was just added. If this needs
// raising a third time, that is the signal to split the schema out of one macro
// invocation rather than to keep doubling.
#![recursion_limit = "512"]

/// Install rustls' ring crypto provider as the default. Idempotent — safe to
/// call from multiple entry points (binary `main`, integration tests, library
/// users). Required because reqwest uses `rustls-no-provider` feature: callers
/// must install a provider before the first TLS handshake.
pub fn install_default_crypto_provider() {
    // Gated, not deleted: all four call sites stay unconditional and this
    // degrades to a no-op. `server-stack` implies `remote-embed`, so the single
    // feature covers both TLS consumers (the HTTP embedder and the reranker).
    #[cfg(feature = "remote-embed")]
    {
        use std::sync::Once;
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
        });
    }
}

pub mod agent;
pub mod ast;
pub mod cli;
pub mod config;
#[cfg(feature = "dashboard")]
pub mod dashboard;
pub mod embed;
pub mod engines;
pub mod fs;

pub mod git;
pub mod hardware;
pub mod heartbeat;
pub mod legibility;

#[cfg(feature = "librarian")]
pub mod librarian;
pub mod library;
pub mod logging;
pub mod lsp;
pub mod mcp_resources;
pub mod memory;
pub mod migrate;
pub mod operator_rules;
#[cfg(unix)]
pub mod peer;
pub mod perf;
pub mod platform;
pub mod prompts;
pub mod retrieval;
pub mod server;
pub mod socket_discovery;
pub mod sqlite_vec_ext;
pub mod symbol;
pub mod tools;
pub mod usage;
pub mod util;
pub mod workspace;
