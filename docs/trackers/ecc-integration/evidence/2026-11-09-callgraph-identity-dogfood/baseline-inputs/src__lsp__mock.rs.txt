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
    /// When set for a path, `document_symbols` fails with this message instead of
    /// answering: a server that gave no answer (e.g. a persistent null, which
    /// `LspClient::document_symbols` reports as an error). Use `with_symbols_error`.
    symbols_errors: HashMap<PathBuf, String>,
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
    /// The `(path, language_id)` of every `prepare_call_hierarchy` call, in order. The real
    /// client opens the file with that languageId (`did_open`), and typescript-language-server
    /// maps it to a script kind, so which id a node is opened under is observable behaviour.
    pub prepare_call_hierarchy_calls: std::sync::Mutex<Vec<(std::path::PathBuf, String)>>,
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
    /// Scripted `references` answers: each call pops the front; once empty, answers fall back to
    /// `references_results`. Simulates a server that answers `[]` while it warms and the full set
    /// after (`with_references_sequence`).
    references_sequence:
        std::sync::Mutex<std::collections::VecDeque<Result<Vec<lsp_types::Location>, String>>>,
    /// How many times `references` was called.
    pub references_calls: std::sync::atomic::AtomicUsize,
    /// `references` answers once the script runs out with this error instead of the canned map,
    /// as a dead client does (`with_references_failing_after_script`).
    references_failing_after_script: Option<String>,
    /// Per-call sleep before `references` answers: call N sleeps `references_delays[N]`, then no
    /// delay. A tokio sleep, so a paused-time test advances it without wall time.
    references_delays: std::sync::Mutex<std::collections::VecDeque<std::time::Duration>>,
    /// Sleep before `prepare_call_hierarchy` and `incoming_calls` answer (a server still cold).
    call_hierarchy_delay: std::time::Duration,
    /// How many times `incoming_calls` was called (`prepare_call_hierarchy_calls` counts the other).
    pub incoming_calls_count: std::sync::atomic::AtomicUsize,
    /// What `cold_start_window_open` answers. Starts `true`, a freshly started client, which is
    /// what makes a server that reports no progress at all (pyright) wait.
    cold_start_window: std::sync::atomic::AtomicBool,
    /// The per-client "references cold-start wait spent" flag. Starts `true` so a test that does not
    /// care about the wait never blocks; `with_references_warmup_pending` models a freshly
    /// started client, as `LspClient` is.
    references_warmup_done: std::sync::atomic::AtomicBool,
    /// The edit `rename` answers with. `None` answers an empty edit.
    rename_edit: Option<lsp_types::WorkspaceEdit>,
    /// How many times `document_symbols` was called, answered or failed.
    pub document_symbols_calls: std::sync::atomic::AtomicUsize,
}

impl MockLspClient {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            symbols_errors: HashMap::new(),
            symbols_sequence: std::sync::Mutex::new(HashMap::new()),
            definitions: HashMap::new(),
            workspace_results: vec![],
            hover_responses: std::sync::Mutex::new(std::collections::VecDeque::new()),
            goto_responses: std::sync::Mutex::new(std::collections::VecDeque::new()),
            prepare_call_hierarchy_results: std::sync::Mutex::new(std::collections::HashMap::new()),
            prepare_call_hierarchy_calls: std::sync::Mutex::new(Vec::new()),
            incoming_calls_results: std::sync::Mutex::new(std::collections::HashMap::new()),
            outgoing_calls_results: std::sync::Mutex::new(std::collections::HashMap::new()),
            references_results: std::sync::Mutex::new(std::collections::HashMap::new()),
            references_sequence: std::sync::Mutex::new(std::collections::VecDeque::new()),
            references_calls: std::sync::atomic::AtomicUsize::new(0),
            references_failing_after_script: None,
            references_delays: std::sync::Mutex::new(std::collections::VecDeque::new()),
            call_hierarchy_delay: std::time::Duration::ZERO,
            incoming_calls_count: std::sync::atomic::AtomicUsize::new(0),
            cold_start_window: std::sync::atomic::AtomicBool::new(true),
            references_warmup_done: std::sync::atomic::AtomicBool::new(true),
            rename_edit: None,
            document_symbols_calls: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Make `document_symbols` fail for `path` with `message`, as a server does
    /// when it never answers. The path must match exactly what the tool passes.
    pub fn with_symbols_error(mut self, path: impl Into<PathBuf>, message: &str) -> Self {
        self.symbols_errors.insert(path.into(), message.to_string());
        self
    }

    /// Make `rename` answer with `edit`, as a language server does with the sites it would change.
    pub fn with_rename_edit(mut self, edit: lsp_types::WorkspaceEdit) -> Self {
        self.rename_edit = Some(edit);
        self
    }
    /// Script the next `references` answers: call N answers `answers[N]`; after the script runs
    /// out, answers come from `references_results`.
    pub fn with_references_sequence(self, answers: Vec<Vec<lsp_types::Location>>) -> Self {
        self.with_references_script(answers.into_iter().map(Ok).collect())
    }

    /// Like [`Self::with_references_sequence`], where an entry may be an error: `Err(message)`
    /// makes that `references` call fail with `message` (for a transient-error test, include
    /// `code -32800`).
    pub fn with_references_script(
        self,
        answers: Vec<Result<Vec<lsp_types::Location>, String>>,
    ) -> Self {
        self.references_sequence
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend(answers);
        self
    }

    /// Model a freshly started client: the `references` cold-start wait has not been spent yet.
    pub fn with_references_warmup_pending(self) -> Self {
        self.references_warmup_done
            .store(false, std::sync::atomic::Ordering::SeqCst);
        self
    }
    /// Once the `references` script runs out, fail every call with `message` rather than falling
    /// back to the canned map: a client whose server is gone.
    pub fn with_references_failing_after_script(mut self, message: &str) -> Self {
        self.references_failing_after_script = Some(message.to_string());
        self
    }

    /// Sleep `delays[N]` before the Nth `references` call answers (then no delay).
    pub fn with_references_delays(self, delays: Vec<std::time::Duration>) -> Self {
        self.references_delays
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend(delays);
        self
    }

    /// Sleep `delay` before `prepare_call_hierarchy` and `incoming_calls` answer.
    pub fn with_call_hierarchy_delay(mut self, delay: std::time::Duration) -> Self {
        self.call_hierarchy_delay = delay;
        self
    }

    /// Make `cold_start_window_open` answer `open`: `false` models a client long past start-up.
    pub fn with_cold_start_window(self, open: bool) -> Self {
        self.cold_start_window
            .store(open, std::sync::atomic::Ordering::SeqCst);
        self
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
        self.document_symbols_calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if let Some(message) = self.symbols_errors.get(path) {
            return Err(anyhow::anyhow!("{message}"));
        }
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
        self.references_calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let delay = self
            .references_delays
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pop_front();
        if let Some(delay) = delay {
            tokio::time::sleep(delay).await;
        }
        if let Some(next) = self
            .references_sequence
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pop_front()
        {
            return next.map_err(|message| anyhow::anyhow!("{message}"));
        }
        if let Some(message) = &self.references_failing_after_script {
            return Err(anyhow::anyhow!("{message}"));
        }
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
        Ok(self.rename_edit.clone().unwrap_or_default())
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
        language_id: &str,
    ) -> anyhow::Result<Option<lsp_types::CallHierarchyItem>> {
        self.prepare_call_hierarchy_calls
            .lock()
            .unwrap()
            .push((path.to_path_buf(), language_id.to_string()));
        if !self.call_hierarchy_delay.is_zero() {
            tokio::time::sleep(self.call_hierarchy_delay).await;
        }
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
        self.incoming_calls_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if !self.call_hierarchy_delay.is_zero() {
            tokio::time::sleep(self.call_hierarchy_delay).await;
        }
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
    fn references_warmup_done(&self) -> bool {
        self.references_warmup_done
            .load(std::sync::atomic::Ordering::SeqCst)
    }
    fn cold_start_window_open(&self) -> bool {
        self.cold_start_window
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    fn mark_references_warmup_done(&self) {
        self.references_warmup_done
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

pub struct MockLspProvider {
    /// The clients `get_or_start` hands out, in order; the last one repeats. One entry is the
    /// usual case; two model a mux disconnect that the next `get_or_start` heals.
    clients: Vec<Arc<MockLspClient>>,
    next: std::sync::atomic::AtomicUsize,
}

impl MockLspProvider {
    pub fn with_client(client: MockLspClient) -> Arc<dyn LspProvider> {
        Self::with_shared_client(Arc::new(client))
    }

    /// Like [`Self::with_client`], for a test that keeps its own handle on the client to read
    /// what the code under test asked of it (`document_symbols_calls`).
    pub fn with_shared_client(client: Arc<MockLspClient>) -> Arc<dyn LspProvider> {
        Self::with_shared_clients(vec![client])
    }

    /// `get_or_start` answers `clients[0]`, then `clients[1]`, and so on; the last repeats. The
    /// tool's first `get_lsp_client` takes the first, a retry after a disconnect takes the next.
    pub fn with_shared_clients(clients: Vec<Arc<MockLspClient>>) -> Arc<dyn LspProvider> {
        assert!(!clients.is_empty(), "a provider needs at least one client");
        Arc::new(Self {
            clients,
            next: std::sync::atomic::AtomicUsize::new(0),
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
        let n = self.next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let client = &self.clients[n.min(self.clients.len() - 1)];
        Ok(Arc::clone(client) as Arc<dyn LspClientOps>)
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
