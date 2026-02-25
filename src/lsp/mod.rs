//! LSP client layer: manages language server processes and exposes a
//! unified async interface for symbol operations.

pub mod client;
pub mod servers;
pub mod symbols;

use anyhow::Result;
use std::path::Path;

/// Unified interface over any language server.
#[async_trait::async_trait]
pub trait LanguageServer: Send + Sync {
    /// List all symbols in a file.
    async fn document_symbols(&self, path: &Path) -> Result<Vec<SymbolInfo>>;

    /// Search workspace-wide symbols by name query.
    async fn workspace_symbols(&self, query: &str) -> Result<Vec<SymbolInfo>>;

    /// Find the definition location of the symbol at `(line, col)` (0-indexed).
    async fn goto_definition(
        &self,
        path: &Path,
        line: u32,
        col: u32,
    ) -> Result<Vec<lsp_types::Location>>;

    /// Find all references to the symbol at `(line, col)`.
    async fn references(
        &self,
        path: &Path,
        line: u32,
        col: u32,
    ) -> Result<Vec<lsp_types::Location>>;

    /// Rename a symbol at `(line, col)` to `new_name`.
    async fn rename(
        &self,
        path: &Path,
        line: u32,
        col: u32,
        new_name: &str,
    ) -> Result<lsp_types::WorkspaceEdit>;

    /// Shutdown and clean up the language server process.
    async fn shutdown(&self) -> Result<()>;
}

pub use symbols::{SymbolInfo, SymbolKind};
