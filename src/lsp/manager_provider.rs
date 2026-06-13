//! `LspProvider` trait implementation for `LspManager`.
//!
//! This forwarder lives in its own file on purpose. Co-locating it with the
//! inherent `impl LspManager` in `manager.rs` gave that file two symbols
//! sharing the `LspManager/get_or_start` name_path — the inherent method plus
//! this trait forwarder — and likewise for `notify_file_changed` and
//! `shutdown_all`. Those duplicate name_paths made `edit_code` ambiguous on
//! the functions and tripped the legibility `name_collision` detector. Keeping
//! the trait impl here leaves `manager.rs` with a single symbol per name_path.
//! Do not fold this back into `manager.rs`.

use std::sync::Arc;

use crate::lsp::LspManager;

#[async_trait::async_trait]
impl crate::lsp::ops::LspProvider for LspManager {
    async fn get_or_start(
        &self,
        language: &str,
        workspace_root: &std::path::Path,
        mux_override: Option<bool>,
    ) -> anyhow::Result<Arc<dyn crate::lsp::ops::LspClientOps>> {
        let client = LspManager::get_or_start(self, language, workspace_root, mux_override).await?;
        Ok(client as Arc<dyn crate::lsp::ops::LspClientOps>)
    }

    async fn notify_file_changed(&self, path: &std::path::Path) {
        LspManager::notify_file_changed(self, path).await
    }

    async fn shutdown_all(&self) {
        LspManager::shutdown_all(self).await
    }

    async fn is_ready(&self, language: &str, workspace_root: &std::path::Path) -> bool {
        LspManager::get(self, language, workspace_root)
            .await
            .is_some()
    }

    async fn record_first_response(
        &self,
        language: &str,
        workspace_root: &std::path::Path,
        elapsed_ms: i64,
    ) {
        // Call the inherent method by name to avoid infinite recursion
        // (self.record_first_response(...) would resolve back to this trait method)
        LspManager::record_first_response_inner(self, language, workspace_root, elapsed_ms).await;
    }
}
