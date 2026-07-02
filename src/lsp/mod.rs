//! LSP client layer: manages language server processes and exposes a
//! unified async interface for symbol operations.

pub mod client;
pub mod client_ops;
pub mod manager;
pub mod manager_provider;
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

/// Budget for a tool call that would otherwise block on an LSP cold start.
/// Callers fall back to tree-sitter output (marked `"lsp": "warming"`) when
/// the budget elapses; the start continues in a DETACHED task so the next
/// call hits the warm fast path.
pub const LSP_FIRST_CALL_BUDGET: std::time::Duration = std::time::Duration::from_secs(2);

/// Bounded LSP acquisition: immediate when a live client exists; otherwise
/// start it on a detached task and wait at most `budget`. `None` means "not
/// ready yet — serve the AST fallback"; it is never an error.
pub async fn client_within_budget(
    lsp: std::sync::Arc<dyn LspProvider>,
    language: &str,
    root: &std::path::Path,
    mux_override: Option<bool>,
    budget: std::time::Duration,
) -> Option<std::sync::Arc<dyn LspClientOps>> {
    if lsp.is_ready(language, root).await {
        return lsp.get_or_start(language, root, mux_override).await.ok();
    }
    let lang = language.to_string();
    let root_buf = root.to_path_buf();
    let handle =
        tokio::spawn(async move { lsp.get_or_start(&lang, &root_buf, mux_override).await });
    match tokio::time::timeout(budget, handle).await {
        Ok(Ok(Ok(client))) => Some(client),
        _ => None, // start error, join error, or budget elapsed (task keeps warming)
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    /// LspProvider whose cold start takes `delay`; `is_ready` flips true once
    /// a start has completed (mirrors LspManager's cache-hit fast path).
    struct SlowStart {
        client: Arc<MockLspClient>,
        delay: Duration,
        ready: AtomicBool,
    }

    #[async_trait::async_trait]
    impl LspProvider for SlowStart {
        async fn get_or_start(
            &self,
            _language: &str,
            _workspace_root: &Path,
            _mux_override: Option<bool>,
        ) -> anyhow::Result<Arc<dyn LspClientOps>> {
            if !self.ready.load(Ordering::SeqCst) {
                tokio::time::sleep(self.delay).await;
                self.ready.store(true, Ordering::SeqCst);
            }
            Ok(self.client.clone())
        }
        async fn notify_file_changed(&self, _path: &Path) {}
        async fn shutdown_all(&self) {}
        async fn is_ready(&self, _language: &str, _workspace_root: &Path) -> bool {
            self.ready.load(Ordering::SeqCst)
        }
    }

    #[tokio::test]
    async fn cold_start_over_budget_returns_none_but_keeps_warming() {
        let lsp: Arc<dyn LspProvider> = Arc::new(SlowStart {
            client: Arc::new(MockLspClient::new()),
            delay: Duration::from_millis(200),
            ready: AtomicBool::new(false),
        });
        // Budget (50ms) < cold start (200ms): must yield None, not block 200ms.
        let t0 = std::time::Instant::now();
        let got = client_within_budget(
            lsp.clone(),
            "rust",
            Path::new("/tmp"),
            None,
            Duration::from_millis(50),
        )
        .await;
        assert!(got.is_none());
        assert!(
            t0.elapsed() < Duration::from_millis(150),
            "must not wait out the cold start"
        );

        // The DETACHED warm-up must finish on its own: after the delay elapses,
        // the provider is ready and the next call succeeds immediately.
        tokio::time::sleep(Duration::from_millis(250)).await;
        let got = client_within_budget(
            lsp,
            "rust",
            Path::new("/tmp"),
            None,
            Duration::from_millis(50),
        )
        .await;
        assert!(
            got.is_some(),
            "second call after warm-up must hit the ready fast path"
        );
    }
}
