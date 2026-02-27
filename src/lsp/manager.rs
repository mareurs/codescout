//! Manages per-language LSP client instances with lazy initialization.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::Mutex;

use super::client::LspClient;
use super::servers;

/// Manages LSP client instances, one per language.
///
/// Clients are lazily started on first use and cached. If a client's
/// workspace root changes (e.g. project switch), the old client is
/// shut down and a new one started.
pub struct LspManager {
    clients: Mutex<HashMap<String, Arc<LspClient>>>,
    /// Per-language startup barrier: concurrent callers for the same language
    /// wait on the first caller's result instead of each spawning a JVM.
    starting: Mutex<HashMap<String, Arc<tokio::sync::Notify>>>,
}

impl Default for LspManager {
    fn default() -> Self {
        Self::new()
    }
}

impl LspManager {
    pub fn new() -> Self {
        Self {
            clients: Mutex::new(HashMap::new()),
            starting: Mutex::new(HashMap::new()),
        }
    }

    /// Get an existing client for the language, or start one.
    ///
    /// If the existing client has a different workspace root or has crashed,
    /// it is replaced with a new instance.
    ///
    /// The mutex is held only for the fast cache check, not during the slow
    /// LSP process startup.  This allows concurrent cold-starts for different
    /// languages to proceed in parallel.
    pub async fn get_or_start(
        &self,
        language: &str,
        workspace_root: &Path,
    ) -> Result<Arc<LspClient>> {
        // Fast path: cache hit.
        {
            let clients = self.clients.lock().await;
            if let Some(client) = clients.get(language) {
                if client.is_alive() && client.workspace_root == workspace_root {
                    return Ok(client.clone());
                }
            }
        }

        // Slow path: need to start (or wait for someone else starting).
        // Use a per-language Notify to prevent thundering herd: only the first
        // caller actually spawns the LSP process; concurrent callers wait.
        let notify = {
            let mut starting = self.starting.lock().await;
            if let Some(existing) = starting.get(language) {
                // Someone else is already starting this language — wait for them.
                let notify = existing.clone();
                drop(starting);
                notify.notified().await;
                // They're done — check the cache again.
                let clients = self.clients.lock().await;
                if let Some(client) = clients.get(language) {
                    if client.is_alive() && client.workspace_root == workspace_root {
                        return Ok(client.clone());
                    }
                }
                // Their startup failed — fall through to try ourselves.
                // Re-acquire starting lock and register ourselves.
                let mut starting = self.starting.lock().await;
                let notify = Arc::new(tokio::sync::Notify::new());
                starting.insert(language.to_string(), notify.clone());
                notify
            } else {
                // We're the first — register ourselves.
                let notify = Arc::new(tokio::sync::Notify::new());
                starting.insert(language.to_string(), notify.clone());
                notify
            }
        };

        // Evict dead/stale client if present.
        {
            let mut clients = self.clients.lock().await;
            if let Some(client) = clients.get(language) {
                if !client.is_alive() || client.workspace_root != workspace_root {
                    let old = clients.remove(language).unwrap();
                    let _ = old.shutdown().await;
                }
            }
        }

        let config = servers::default_config(language, workspace_root)
            .ok_or_else(|| anyhow::anyhow!("No LSP server configured for language: {}", language));

        let result = match config {
            Ok(config) => LspClient::start(config).await.map(Arc::new),
            Err(e) => Err(e),
        };

        // Clean up the starting barrier and notify waiters.
        {
            let mut starting = self.starting.lock().await;
            starting.remove(language);
        }
        notify.notify_waiters();

        let new_client = result?;
        let mut clients = self.clients.lock().await;
        clients.insert(language.to_string(), new_client.clone());
        Ok(new_client)
    }

    /// Get an existing alive client without starting one.
    pub async fn get(&self, language: &str) -> Option<Arc<LspClient>> {
        let clients = self.clients.lock().await;
        clients.get(language).filter(|c| c.is_alive()).cloned()
    }

    /// Shut down all active LSP servers.
    pub async fn shutdown_all(&self) {
        let mut clients = self.clients.lock().await;
        for (lang, client) in clients.drain() {
            tracing::info!("Shutting down LSP for: {}", lang);
            if let Err(e) = client.shutdown().await {
                tracing::warn!("Error shutting down LSP for {}: {}", lang, e);
            }
        }
    }

    /// List currently active languages.
    pub async fn active_languages(&self) -> Vec<String> {
        let clients = self.clients.lock().await;
        clients
            .iter()
            .filter(|(_, c)| c.is_alive())
            .map(|(lang, _)| lang.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn manager_starts_empty() {
        let mgr = LspManager::new();
        assert!(mgr.active_languages().await.is_empty());
        assert!(mgr.get("rust").await.is_none());
    }

    #[tokio::test]
    async fn manager_errors_for_unknown_language() {
        let mgr = LspManager::new();
        let dir = tempfile::tempdir().unwrap();
        let result = mgr.get_or_start("brainfuck", dir.path()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn manager_shutdown_all_empty() {
        let mgr = LspManager::new();
        mgr.shutdown_all().await; // Should not panic
    }

    #[tokio::test]
    async fn shutdown_all_stops_running_servers() {
        use std::process::Command as StdCommand;

        // Check if rust-analyzer is available
        if StdCommand::new("rust-analyzer")
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("Skipping: rust-analyzer not installed");
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        // Create minimal Cargo project
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"t\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "pub fn f() {}").unwrap();

        let mgr = LspManager::new();
        let client = mgr.get_or_start("rust", dir.path()).await.unwrap();
        assert!(client.is_alive());

        mgr.shutdown_all().await;

        // After shutdown, the client should be dead
        assert!(!client.is_alive());
        assert!(mgr.active_languages().await.is_empty());
    }
}
