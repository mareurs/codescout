//! LSP client layer: manages language server processes and exposes a
//! unified async interface for symbol operations.

pub mod client;
pub mod manager;
pub mod mock;
pub mod servers;
pub mod symbols;
pub use mock::{MockLspClient, MockLspProvider};
pub mod call_hierarchy;
pub mod ops;

pub use ops::{LspClientOps, LspProvider};
pub mod mux;
pub mod transport;

pub use client::{LspClient, LspServerConfig};
pub use manager::LspManager;
pub use symbols::{SymbolInfo, SymbolKind};

/// Languages whose LSP servers are pre-warmed on project activation.
/// Hardcoded to JVM languages — Kotlin in particular takes 30–60 s to start.
const PREWARM_LANGUAGES: &[&str] = &["java", "kotlin"];

/// Spawn background `get_or_start` tasks for any JVM languages found in
/// `project_languages`. Safe to call concurrently: `LspManager`'s starting-map
/// serialises parallel starters via a watch channel — no double-start risk.
pub fn prewarm_lsp_background(
    lsp: std::sync::Arc<dyn LspProvider>,
    root: std::path::PathBuf,
    project_languages: &[String],
) {
    for lang in project_languages {
        if PREWARM_LANGUAGES.contains(&lang.as_str()) {
            let lsp = lsp.clone();
            let root = root.clone();
            let lang = lang.clone();
            tokio::spawn(async move {
                if let Err(e) = lsp.get_or_start(&lang, &root, None).await {
                    tracing::debug!("LSP pre-warm for {lang} skipped: {e}");
                }
            });
        }
    }
}
