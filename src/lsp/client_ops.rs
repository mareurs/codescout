//! `LspClientOps` trait implementation for `LspClient`.
//!
//! This forwarder lives in its own file on purpose. Co-locating it with the
//! inherent `impl LspClient` in `client.rs` gave that file ten symbols each
//! sharing a `LspClient/<method>` name_path (the inherent method plus this
//! trait forwarder) — every one ambiguous to `edit_code` ("matches 2 symbols")
//! and flagged by the legibility `name_collision` detector. Keeping the trait
//! impl here leaves `client.rs` with a single symbol per name_path. Do not fold
//! this back into `client.rs`. (Same template as `manager_provider.rs`.)

use crate::lsp::LspClient;

#[async_trait::async_trait]
impl crate::lsp::ops::LspClientOps for LspClient {
    async fn document_symbols(
        &self,
        path: &std::path::Path,
        language_id: &str,
    ) -> anyhow::Result<Vec<crate::lsp::SymbolInfo>> {
        LspClient::document_symbols(self, path, language_id).await
    }

    async fn workspace_symbols(&self, query: &str) -> anyhow::Result<Vec<crate::lsp::SymbolInfo>> {
        LspClient::workspace_symbols(self, query).await
    }

    async fn references(
        &self,
        path: &std::path::Path,
        line: u32,
        col: u32,
        language_id: &str,
    ) -> anyhow::Result<Vec<lsp_types::Location>> {
        LspClient::references(self, path, line, col, language_id).await
    }

    async fn goto_definition(
        &self,
        path: &std::path::Path,
        line: u32,
        col: u32,
        language_id: &str,
    ) -> anyhow::Result<Vec<lsp_types::Location>> {
        LspClient::goto_definition(self, path, line, col, language_id).await
    }

    async fn hover(
        &self,
        path: &std::path::Path,
        line: u32,
        col: u32,
        language_id: &str,
    ) -> anyhow::Result<Option<String>> {
        LspClient::hover(self, path, line, col, language_id).await
    }

    async fn rename(
        &self,
        path: &std::path::Path,
        line: u32,
        col: u32,
        new_name: &str,
        language_id: &str,
    ) -> anyhow::Result<lsp_types::WorkspaceEdit> {
        LspClient::rename(self, path, line, col, new_name, language_id).await
    }

    async fn did_change(&self, path: &std::path::Path) -> anyhow::Result<()> {
        LspClient::did_change(self, path).await
    }

    async fn prepare_call_hierarchy(
        &self,
        path: &std::path::Path,
        line: u32,
        col: u32,
        language_id: &str,
    ) -> anyhow::Result<Option<lsp_types::CallHierarchyItem>> {
        LspClient::prepare_call_hierarchy(self, path, line, col, language_id).await
    }

    async fn incoming_calls(
        &self,
        item: &lsp_types::CallHierarchyItem,
        language_id: &str,
    ) -> anyhow::Result<Vec<lsp_types::CallHierarchyIncomingCall>> {
        LspClient::incoming_calls(self, item, language_id).await
    }

    async fn outgoing_calls(
        &self,
        item: &lsp_types::CallHierarchyItem,
        language_id: &str,
    ) -> anyhow::Result<Vec<lsp_types::CallHierarchyOutgoingCall>> {
        LspClient::outgoing_calls(self, item, language_id).await
    }
}
