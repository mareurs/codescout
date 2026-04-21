use std::path::Path;
use std::sync::Arc;

use crate::lsp::SymbolInfo;

/// Abstract interface over LSP operations used by tools.
/// `LspClient` implements this; `MockLspClient` implements it for tests.
#[async_trait::async_trait]
pub trait LspClientOps: Send + Sync {
    async fn document_symbols(
        &self,
        path: &Path,
        language_id: &str,
    ) -> anyhow::Result<Vec<SymbolInfo>>;

    async fn workspace_symbols(&self, query: &str) -> anyhow::Result<Vec<SymbolInfo>>;

    async fn references(
        &self,
        path: &Path,
        line: u32,
        col: u32,
        language_id: &str,
    ) -> anyhow::Result<Vec<lsp_types::Location>>;

    async fn goto_definition(
        &self,
        path: &Path,
        line: u32,
        col: u32,
        language_id: &str,
    ) -> anyhow::Result<Vec<lsp_types::Location>>;

    async fn hover(
        &self,
        path: &Path,
        line: u32,
        col: u32,
        language_id: &str,
    ) -> anyhow::Result<Option<String>>;

    async fn rename(
        &self,
        path: &Path,
        line: u32,
        col: u32,
        new_name: &str,
        language_id: &str,
    ) -> anyhow::Result<lsp_types::WorkspaceEdit>;

    async fn did_change(&self, path: &Path) -> anyhow::Result<()>;
}

/// Abstract factory that starts or retrieves an LSP client for a given language.
/// `LspManager` implements this; `MockLspProvider` implements it for tests.
#[async_trait::async_trait]
pub trait LspProvider: Send + Sync {
    async fn get_or_start(
        &self,
        language: &str,
        workspace_root: &Path,
        mux_override: Option<bool>,
    ) -> anyhow::Result<Arc<dyn LspClientOps>>;

    async fn notify_file_changed(&self, path: &Path);

    async fn shutdown_all(&self);

    /// Returns `true` if there is already a live LSP client for the given language
    /// in the given workspace root. Must NOT start a new client — this is a
    /// non-blocking readiness probe used by diagnostic/summary tools.
    async fn is_ready(&self, _language: &str, _workspace_root: &Path) -> bool {
        false
    }

    /// Record the first real LSP response time for a cold-started client.
    /// Default implementation is a no-op — only `LspManager` does real work.
    /// Best-effort: implementations must never propagate errors.
    async fn record_first_response(
        &self,
        _language: &str,
        _workspace_root: &std::path::Path,
        _elapsed_ms: i64,
    ) {
    }
}
