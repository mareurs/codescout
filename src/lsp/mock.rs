//! Test-only mock implementations of LspClientOps and LspProvider.
//! Returned symbol lists are configured at construction time; all
//! other LSP methods return Ok(Default::default()) silently.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::lsp::ops::{LspClientOps, LspProvider};
use crate::lsp::SymbolInfo;

pub struct MockLspClient {
    symbols: HashMap<PathBuf, Vec<SymbolInfo>>,
    /// BUG-041 test infra: when set for a path, `document_symbols` returns the
    /// FRONT of the queue (without popping), and `did_change` pops the front
    /// (unless only one entry remains, which then sticks). Simulates an LSP
    /// that serves stale positions until `textDocument/didChange` has propagated.
    symbols_sequence:
        std::sync::Mutex<HashMap<PathBuf, std::collections::VecDeque<Vec<SymbolInfo>>>>,
    definitions: HashMap<(u32, u32), Vec<lsp_types::Location>>,
    workspace_results: Vec<SymbolInfo>,
    /// Test infra for mux-disconnect retry: each `hover` call pops the front
    /// of this queue and returns it. When empty, falls back to `Ok(None)`.
    /// Use `with_hover_responses` to populate.
    hover_responses: std::sync::Mutex<std::collections::VecDeque<anyhow::Result<Option<String>>>>,
    /// Same idea for `goto_definition`. When empty, falls back to the
    /// `definitions` map keyed by (line, col).
    goto_responses:
        std::sync::Mutex<std::collections::VecDeque<anyhow::Result<Vec<lsp_types::Location>>>>,
    /// Canned responses for `prepare_call_hierarchy`, keyed by (path, line, col).
    pub prepare_call_hierarchy_results: std::sync::Mutex<
        std::collections::HashMap<
            (std::path::PathBuf, u32, u32),
            Option<lsp_types::CallHierarchyItem>,
        >,
    >,
    /// Canned responses for `incoming_calls`, keyed by item name.
    pub incoming_calls_results: std::sync::Mutex<
        std::collections::HashMap<String, Vec<lsp_types::CallHierarchyIncomingCall>>,
    >,
    /// Canned responses for `outgoing_calls`, keyed by item name.
    pub outgoing_calls_results: std::sync::Mutex<
        std::collections::HashMap<String, Vec<lsp_types::CallHierarchyOutgoingCall>>,
    >,
    /// Canned responses for `references`, keyed by file path.
    pub references_results:
        std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, Vec<lsp_types::Location>>>,
}

impl MockLspClient {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            symbols_sequence: std::sync::Mutex::new(HashMap::new()),
            definitions: HashMap::new(),
            workspace_results: vec![],
            hover_responses: std::sync::Mutex::new(std::collections::VecDeque::new()),
            goto_responses: std::sync::Mutex::new(std::collections::VecDeque::new()),
            prepare_call_hierarchy_results: std::sync::Mutex::new(std::collections::HashMap::new()),
            incoming_calls_results: std::sync::Mutex::new(std::collections::HashMap::new()),
            outgoing_calls_results: std::sync::Mutex::new(std::collections::HashMap::new()),
            references_results: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Pre-load symbol results for a given file path.
    /// The path must match exactly what the tool passes to `document_symbols`.
    pub fn with_symbols(mut self, path: impl Into<PathBuf>, syms: Vec<SymbolInfo>) -> Self {
        self.symbols.insert(path.into(), syms);
        self
    }

    /// Pre-load a QUEUE of symbol results for a path. Useful for simulating
    /// LSP staleness (BUG-041): the first element is returned by `document_symbols`,
    /// and `did_change` advances to the next. The final element sticks once
    /// reached. An empty sequence behaves like no sequence at all and falls
    /// back to `with_symbols` (if set) or an empty Vec.
    pub fn with_symbols_sequence(
        self,
        path: impl Into<PathBuf>,
        sequence: Vec<Vec<SymbolInfo>>,
    ) -> Self {
        self.symbols_sequence
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(path.into(), sequence.into());
        self
    }

    /// Pre-load definition results for a specific (line, col) position (0-indexed).
    /// `goto_definition` returns these locations only when called with an exact match.
    pub fn with_definitions(
        mut self,
        line: u32,
        col: u32,
        locations: Vec<lsp_types::Location>,
    ) -> Self {
        self.definitions.insert((line, col), locations);
        self
    }

    /// Pre-load workspace/symbol results returned for any query.
    pub fn with_workspace_symbols(mut self, syms: Vec<SymbolInfo>) -> Self {
        self.workspace_results = syms;
        self
    }

    /// Pre-load a sequence of `hover` responses. Each call pops the front;
    /// when the queue is empty, falls back to `Ok(None)`.
    /// Used to simulate transient mux disconnects in retry tests.
    pub fn with_hover_responses(self, responses: Vec<anyhow::Result<Option<String>>>) -> Self {
        let mut q = self
            .hover_responses
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        q.extend(responses);
        drop(q);
        self
    }

    /// Pre-load a sequence of `goto_definition` responses. Each call pops the
    /// front; when the queue is empty, falls back to the (line, col) map.
    pub fn with_goto_responses(
        self,
        responses: Vec<anyhow::Result<Vec<lsp_types::Location>>>,
    ) -> Self {
        let mut q = self
            .goto_responses
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        q.extend(responses);
        drop(q);
        self
    }
}

impl Default for MockLspClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl LspClientOps for MockLspClient {
    async fn document_symbols(
        &self,
        path: &Path,
        _language_id: &str,
    ) -> anyhow::Result<Vec<SymbolInfo>> {
        if let Some(front) = self
            .symbols_sequence
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(path)
            .and_then(|q| q.front().cloned())
        {
            return Ok(front);
        }
        Ok(self.symbols.get(path).cloned().unwrap_or_default())
    }

    async fn workspace_symbols(&self, _query: &str) -> anyhow::Result<Vec<SymbolInfo>> {
        Ok(self.workspace_results.clone())
    }

    async fn references(
        &self,
        path: &Path,
        _line: u32,
        _col: u32,
        _language_id: &str,
    ) -> anyhow::Result<Vec<lsp_types::Location>> {
        let map = self
            .references_results
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        Ok(map.get(path).cloned().unwrap_or_default())
    }

    async fn goto_definition(
        &self,
        _path: &Path,
        line: u32,
        col: u32,
        _language_id: &str,
    ) -> anyhow::Result<Vec<lsp_types::Location>> {
        if let Some(next) = self
            .goto_responses
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pop_front()
        {
            return next;
        }
        Ok(self
            .definitions
            .get(&(line, col))
            .cloned()
            .unwrap_or_default())
    }

    async fn hover(
        &self,
        _path: &Path,
        _line: u32,
        _col: u32,
        _language_id: &str,
    ) -> anyhow::Result<Option<String>> {
        if let Some(next) = self
            .hover_responses
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pop_front()
        {
            return next;
        }
        Ok(None)
    }

    async fn rename(
        &self,
        _path: &Path,
        _line: u32,
        _col: u32,
        _new_name: &str,
        _language_id: &str,
    ) -> anyhow::Result<lsp_types::WorkspaceEdit> {
        Ok(lsp_types::WorkspaceEdit::default())
    }

    async fn did_change(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(q) = self
            .symbols_sequence
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_mut(path)
        {
            if q.len() > 1 {
                q.pop_front();
            }
        }
        Ok(())
    }

    async fn prepare_call_hierarchy(
        &self,
        path: &Path,
        line: u32,
        col: u32,
        _language_id: &str,
    ) -> anyhow::Result<Option<lsp_types::CallHierarchyItem>> {
        Ok(self
            .prepare_call_hierarchy_results
            .lock()
            .unwrap()
            .get(&(path.to_path_buf(), line, col))
            .cloned()
            .flatten())
    }

    async fn incoming_calls(
        &self,
        item: &lsp_types::CallHierarchyItem,
        _language_id: &str,
    ) -> anyhow::Result<Vec<lsp_types::CallHierarchyIncomingCall>> {
        Ok(self
            .incoming_calls_results
            .lock()
            .unwrap()
            .get(&item.name)
            .cloned()
            .unwrap_or_default())
    }

    async fn outgoing_calls(
        &self,
        item: &lsp_types::CallHierarchyItem,
        _language_id: &str,
    ) -> anyhow::Result<Vec<lsp_types::CallHierarchyOutgoingCall>> {
        Ok(self
            .outgoing_calls_results
            .lock()
            .unwrap()
            .get(&item.name)
            .cloned()
            .unwrap_or_default())
    }
}

pub struct MockLspProvider {
    client: Arc<MockLspClient>,
}

impl MockLspProvider {
    pub fn with_client(client: MockLspClient) -> Arc<dyn LspProvider> {
        Arc::new(Self {
            client: Arc::new(client),
        })
    }
}

#[async_trait::async_trait]
impl LspProvider for MockLspProvider {
    async fn get_or_start(
        &self,
        _language: &str,
        _workspace_root: &Path,
        _mux_override: Option<bool>,
    ) -> anyhow::Result<Arc<dyn LspClientOps>> {
        Ok(Arc::clone(&self.client) as Arc<dyn LspClientOps>)
    }

    async fn notify_file_changed(&self, _path: &Path) {}

    async fn shutdown_all(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp::ops::LspClientOps;

    #[tokio::test]
    async fn mock_call_hierarchy_returns_canned_responses() {
        let mock = MockLspClient::new();
        let uri: lsp_types::Uri = "file:///a.rs".parse().unwrap();
        let item = lsp_types::CallHierarchyItem {
            name: "a".into(),
            kind: lsp_types::SymbolKind::FUNCTION,
            tags: None,
            detail: None,
            uri,
            range: lsp_types::Range::default(),
            selection_range: lsp_types::Range::default(),
            data: None,
        };

        // Seed prepare_call_hierarchy
        mock.prepare_call_hierarchy_results
            .lock()
            .unwrap()
            .insert((std::path::PathBuf::from("a.rs"), 0, 0), Some(item.clone()));

        let got = mock
            .prepare_call_hierarchy(std::path::Path::new("a.rs"), 0, 0, "rust")
            .await
            .unwrap();
        assert_eq!(got.unwrap().name, "a");

        // Miss on a different position returns None
        let miss = mock
            .prepare_call_hierarchy(std::path::Path::new("a.rs"), 1, 1, "rust")
            .await
            .unwrap();
        assert!(miss.is_none());

        // incoming_calls empty by default
        assert!(mock.incoming_calls(&item, "rust").await.unwrap().is_empty());

        // outgoing_calls empty by default
        assert!(mock.outgoing_calls(&item, "rust").await.unwrap().is_empty());

        // Seed incoming_calls and verify retrieval
        let caller_uri: lsp_types::Uri = "file:///b.rs".parse().unwrap();
        let caller = lsp_types::CallHierarchyItem {
            name: "b".into(),
            kind: lsp_types::SymbolKind::FUNCTION,
            tags: None,
            detail: None,
            uri: caller_uri,
            range: lsp_types::Range::default(),
            selection_range: lsp_types::Range::default(),
            data: None,
        };
        let incoming = lsp_types::CallHierarchyIncomingCall {
            from: caller,
            from_ranges: vec![],
        };
        mock.incoming_calls_results
            .lock()
            .unwrap()
            .insert("a".into(), vec![incoming]);

        let calls = mock.incoming_calls(&item, "rust").await.unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from.name, "b");
    }
}
