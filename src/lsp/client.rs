//! Async LSP client that manages a language server subprocess.
//!
//! TODO: Implement using `tower-lsp` or direct jsonrpc over stdio pipes.

use anyhow::Result;
use std::path::{Path, PathBuf};

use super::symbols::SymbolInfo;

/// Configuration for launching a language server.
#[derive(Debug, Clone)]
pub struct LspServerConfig {
    /// Executable to launch (e.g. "rust-analyzer", "pyright-langserver")
    pub command: String,
    /// Arguments passed to the executable
    pub args: Vec<String>,
    /// Working directory (usually the project root)
    pub workspace_root: PathBuf,
}

/// A running LSP client session (stub — not yet implemented).
pub struct LspClient {
    config: LspServerConfig,
}

impl LspClient {
    pub async fn start(config: LspServerConfig) -> Result<Self> {
        tracing::info!("Starting LSP server: {}", config.command);
        // TODO: spawn process, establish jsonrpc connection, send initialize
        Ok(Self { config })
    }

    pub async fn document_symbols(&self, _path: &Path) -> Result<Vec<SymbolInfo>> {
        // TODO: send textDocument/documentSymbol request
        todo!("LSP document_symbols")
    }

    pub async fn workspace_symbols(&self, _query: &str) -> Result<Vec<SymbolInfo>> {
        // TODO: send workspace/symbol request
        todo!("LSP workspace_symbols")
    }

    pub async fn shutdown(&self) -> Result<()> {
        // TODO: send shutdown + exit
        Ok(())
    }
}
