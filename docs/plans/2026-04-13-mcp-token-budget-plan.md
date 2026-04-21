# MCP Surface on a Token Budget — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Spec:** [`docs/plans/2026-04-13-mcp-token-budget-design.md`](./2026-04-13-mcp-token-budget-design.md)

**Goal:** Cut per-turn MCP token overhead by moving reference material into MCP resources, capping + conditionally hiding tool descriptions, and lighting up progress notifications for long-running operations.

**Architecture:** Three orthogonal features landing in one PR on `experiments`. New `src/mcp_resources/` module implements `resources/list` and `resources/read`. `Tool` trait gets an `availability()` method; tool filtering happens in `ServerHandler::list_tools`; `activate_project` emits `notifications/tools/list_changed` when the set changes. `ProgressReporter` (already wired to `peer.notify_progress`) gains 2 Hz throttling and new call sites in `index_project`, `semantic_search`, LSP cold-start, and long-running `run_command`.

**Tech Stack:** Rust 2021, rmcp 1.3, tokio, async-trait, existing tree-sitter + LSP + embeddings stack.

**Precondition checklist** (run once before Task 1):
- `cargo build --release` succeeds on `experiments` branch HEAD
- `cargo test` green (baseline 1142 tests)
- rmcp 1.3 confirmed in `Cargo.toml:21` — exposes `enable_resources()`, `ListResourcesResult`, `ReadResourceResult`, `ResourceContents`, `notify_tools_list_changed`

---

## File structure

**Create**
- `src/mcp_resources/mod.rs` — `ResourceProvider` trait, `ResourceRegistry`, errors
- `src/mcp_resources/doc.rs` — static file resource provider (`doc://*`)
- `src/mcp_resources/memory.rs` — per-memory-file provider (`memory://*`)
- `src/mcp_resources/project_summary.rs` — dynamic `project://summary` provider
- `src/mcp_resources/tool_guide.rs` — generated `doc://codescout-tool-guide` provider
- `docs/manual/src/experimental/mcp-resources.md` — user-facing experimental docs

**Modify**
- `src/lib.rs` — declare `pub mod mcp_resources;`
- `src/tools/mod.rs` — add `Availability` enum + `Tool::availability()` (default `Always`)
- `src/tools/progress.rs` — add 2 Hz throttle inside `ProgressReporter`
- `src/server.rs` — wire `ResourceRegistry`, implement `list_resources` + `read_resource`, filter `list_tools` by availability, enable resources capability
- `src/agent.rs` — on `activate_project`, refresh `memory://` resources + recompute availability set + emit `tools/list_changed` if changed
- `src/tools/symbol.rs`, `src/tools/lsp_*.rs`, etc. — override `availability()` for LSP-gated tools
- `src/tools/semantic.rs` — override `availability()` for embedding-gated tools; add progress calls
- `src/tools/library.rs` — override `availability()` for library tools
- `src/tools/index.rs` (or wherever `index_project` lives) — add progress calls
- `src/tools/command.rs` (run_command) — add progress calls for long-running ops
- `src/lsp/client.rs` (or similar) — add progress calls during cold start
- Every tool's `description()` — audit + cap at 300 chars
- `src/prompts/server_instructions.md` — shorten, point to `doc://codescout-tool-guide` for examples
- `src/prompts/onboarding_prompt.md` — same
- `src/tools/workflow.rs` — update `build_system_prompt_draft`, bump `ONBOARDING_VERSION` from 3 to 4
- `docs/manual/src/experimental/index.md` — link new page

---

## Task 1: Foundation — `ResourceProvider` trait + `ResourceRegistry` skeleton

**Files:**
- Create: `src/mcp_resources/mod.rs`
- Modify: `src/lib.rs` (add `pub mod mcp_resources;`)
- Test: `src/mcp_resources/mod.rs` (inline `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing test**

Add to `src/mcp_resources/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_rejects_duplicate_uri() {
        let mut reg = ResourceRegistry::new();
        reg.register(Box::new(StubProvider::new("doc://a")));
        let err = reg
            .try_register(Box::new(StubProvider::new("doc://a")))
            .expect_err("duplicate URI must error");
        assert!(err.to_string().contains("doc://a"));
    }

    #[test]
    fn registry_read_unknown_returns_not_found() {
        let reg = ResourceRegistry::new();
        let err = futures::executor::block_on(reg.read("doc://missing"))
            .expect_err("unknown URI must error");
        assert!(matches!(err, ResourceError::NotFound(_)));
    }

    struct StubProvider { uri: String }
    impl StubProvider { fn new(u: &str) -> Self { Self { uri: u.into() } } }
    #[async_trait::async_trait]
    impl ResourceProvider for StubProvider {
        fn descriptors(&self) -> Vec<ResourceDescriptor> {
            vec![ResourceDescriptor {
                uri: self.uri.clone(),
                name: "stub".into(),
                description: None,
                mime_type: "text/plain".into(),
            }]
        }
        async fn read(&self, _uri: &str) -> Result<ResourceBytes, ResourceError> {
            Ok(ResourceBytes::Text("stub".into()))
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p codescout mcp_resources::tests`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Write the trait + registry**

Create `src/mcp_resources/mod.rs`:

```rust
//! MCP resource providers — `resources/list` + `resources/read` handlers.
//!
//! Keep this module's naming distinct from `config::ResourcesSection` (which
//! governs LSP resource limits, not MCP resources).

use std::collections::HashMap;

pub mod doc;
pub mod memory;
pub mod project_summary;
pub mod tool_guide;

#[derive(Debug, Clone)]
pub struct ResourceDescriptor {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: String,
}

pub enum ResourceBytes {
    Text(String),
    Blob(Vec<u8>),
}

#[derive(Debug, thiserror::Error)]
pub enum ResourceError {
    #[error("resource not found: {0}")]
    NotFound(String),
    #[error("source unavailable for {0}: {1}")]
    SourceUnavailable(String, String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[async_trait::async_trait]
pub trait ResourceProvider: Send + Sync {
    fn descriptors(&self) -> Vec<ResourceDescriptor>;
    async fn read(&self, uri: &str) -> Result<ResourceBytes, ResourceError>;
}

#[derive(Default)]
pub struct ResourceRegistry {
    providers: Vec<Box<dyn ResourceProvider>>,
    index: HashMap<String, usize>,
}

impl ResourceRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn try_register(&mut self, p: Box<dyn ResourceProvider>) -> anyhow::Result<()> {
        for d in p.descriptors() {
            if self.index.contains_key(&d.uri) {
                anyhow::bail!("duplicate resource URI: {}", d.uri);
            }
        }
        let idx = self.providers.len();
        for d in p.descriptors() {
            self.index.insert(d.uri, idx);
        }
        self.providers.push(p);
        Ok(())
    }

    /// Convenience: panic on duplicate (use for static registration).
    pub fn register(&mut self, p: Box<dyn ResourceProvider>) {
        self.try_register(p).expect("resource URI collision");
    }

    pub fn list(&self) -> Vec<ResourceDescriptor> {
        self.providers.iter().flat_map(|p| p.descriptors()).collect()
    }

    pub async fn read(&self, uri: &str) -> Result<ResourceBytes, ResourceError> {
        let idx = self.index.get(uri).ok_or_else(|| ResourceError::NotFound(uri.into()))?;
        self.providers[*idx].read(uri).await
    }
}
```

Add to `src/lib.rs` alongside other `pub mod` lines: `pub mod mcp_resources;`

Stub-out the four submodule files as empty (`//! TBD`) so the module tree compiles. They get real content in later tasks.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p codescout mcp_resources::tests`
Expected: PASS (2 tests)

- [ ] **Step 5: Commit**

```bash
git add src/mcp_resources/ src/lib.rs
git commit -m "feat(mcp): resource registry skeleton"
```

---

## Task 2: `doc://` + `memory://` providers

**Files:**
- Modify: `src/mcp_resources/doc.rs`, `src/mcp_resources/memory.rs`
- Test: alongside each module

- [ ] **Step 1: Write the failing tests**

In `doc.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn doc_provider_reads_existing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("guide.md");
        std::fs::write(&path, "# hello").unwrap();
        let p = DocProvider::new(vec![DocSource {
            uri: "doc://guide".into(),
            name: "guide".into(),
            description: None,
            path,
        }]);
        let bytes = p.read("doc://guide").await.unwrap();
        match bytes {
            ResourceBytes::Text(s) => assert_eq!(s, "# hello"),
            _ => panic!("expected text"),
        }
    }

    #[tokio::test]
    async fn doc_provider_reports_missing_source() {
        let p = DocProvider::new(vec![DocSource {
            uri: "doc://missing".into(),
            name: "missing".into(),
            description: None,
            path: PathBuf::from("/nonexistent/path"),
        }]);
        let err = p.read("doc://missing").await.unwrap_err();
        assert!(matches!(err, ResourceError::SourceUnavailable(_, _)));
    }
}
```

In `memory.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn memory_provider_enumerates_md_files_in_dir() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("arch.md"), "arch body").unwrap();
        std::fs::write(tmp.path().join("NOT_MEMORY.txt"), "ignore").unwrap();
        let p = MemoryProvider::new(tmp.path().to_path_buf());
        let uris: Vec<_> = p.descriptors().into_iter().map(|d| d.uri).collect();
        assert!(uris.contains(&"memory://arch".to_string()));
        assert_eq!(uris.len(), 1);
    }
}
```

- [ ] **Step 2: Run tests — expect FAIL**

Run: `cargo test -p codescout mcp_resources`
Expected: FAIL — `DocProvider` / `MemoryProvider` do not exist.

- [ ] **Step 3: Implement both providers**

`src/mcp_resources/doc.rs`:

```rust
use std::path::PathBuf;
use super::{ResourceBytes, ResourceDescriptor, ResourceError, ResourceProvider};

#[derive(Debug, Clone)]
pub struct DocSource {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub path: PathBuf,
}

pub struct DocProvider { sources: Vec<DocSource> }

impl DocProvider {
    pub fn new(sources: Vec<DocSource>) -> Self { Self { sources } }
}

#[async_trait::async_trait]
impl ResourceProvider for DocProvider {
    fn descriptors(&self) -> Vec<ResourceDescriptor> {
        self.sources.iter().map(|s| ResourceDescriptor {
            uri: s.uri.clone(),
            name: s.name.clone(),
            description: s.description.clone(),
            mime_type: "text/markdown".into(),
        }).collect()
    }

    async fn read(&self, uri: &str) -> Result<ResourceBytes, ResourceError> {
        let src = self.sources.iter().find(|s| s.uri == uri)
            .ok_or_else(|| ResourceError::NotFound(uri.into()))?;
        let body = tokio::fs::read_to_string(&src.path).await
            .map_err(|e| ResourceError::SourceUnavailable(uri.into(), e.to_string()))?;
        Ok(ResourceBytes::Text(body))
    }
}
```

`src/mcp_resources/memory.rs`:

```rust
use std::path::{Path, PathBuf};
use super::{ResourceBytes, ResourceDescriptor, ResourceError, ResourceProvider};

/// One resource per `*.md` file in the active project's memory directory.
///
/// URIs: `memory://<stem>` where `<stem>` is the filename without extension.
pub struct MemoryProvider { dir: PathBuf }

impl MemoryProvider {
    pub fn new(dir: PathBuf) -> Self { Self { dir } }

    fn entries(&self) -> Vec<(String, PathBuf)> {
        let mut out = Vec::new();
        let Ok(rd) = std::fs::read_dir(&self.dir) else { return out };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) == Some("md") {
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    out.push((stem.to_string(), p));
                }
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    fn lookup(&self, uri: &str) -> Option<PathBuf> {
        let stem = uri.strip_prefix("memory://")?;
        self.entries().into_iter().find(|(s, _)| s == stem).map(|(_, p)| p)
    }
}

#[async_trait::async_trait]
impl ResourceProvider for MemoryProvider {
    fn descriptors(&self) -> Vec<ResourceDescriptor> {
        self.entries().into_iter().map(|(stem, _)| ResourceDescriptor {
            uri: format!("memory://{}", stem),
            name: stem.clone(),
            description: Some(format!("Project memory: {}", stem)),
            mime_type: "text/markdown".into(),
        }).collect()
    }

    async fn read(&self, uri: &str) -> Result<ResourceBytes, ResourceError> {
        let path = self.lookup(uri).ok_or_else(|| ResourceError::NotFound(uri.into()))?;
        let body = tokio::fs::read_to_string(&path).await
            .map_err(|e| ResourceError::SourceUnavailable(uri.into(), e.to_string()))?;
        Ok(ResourceBytes::Text(body))
    }
}
```

- [ ] **Step 4: Run tests — expect PASS**

Run: `cargo test -p codescout mcp_resources`
Expected: 4 new tests pass + earlier 2 still pass.

- [ ] **Step 5: Commit**

```bash
git add src/mcp_resources/doc.rs src/mcp_resources/memory.rs
git commit -m "feat(mcp): doc:// and memory:// resource providers"
```

---

## Task 3: `project://summary` dynamic provider

**Files:**
- Modify: `src/mcp_resources/project_summary.rs`

- [ ] **Step 1: Failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn summary_returns_json_with_required_keys() {
        let p = ProjectSummaryProvider::new(StubSource);
        let bytes = p.read("project://summary").await.unwrap();
        let json: serde_json::Value = match bytes {
            ResourceBytes::Text(s) => serde_json::from_str(&s).unwrap(),
            _ => panic!("expected text"),
        };
        for k in ["active_project", "index_status", "language", "lsp_ready"] {
            assert!(json.get(k).is_some(), "missing {}", k);
        }
    }

    struct StubSource;
    #[async_trait::async_trait]
    impl SummarySource for StubSource {
        async fn snapshot(&self) -> SummarySnapshot {
            SummarySnapshot {
                active_project: Some("/tmp/proj".into()),
                index_status: "fresh".into(),
                language: Some("rust".into()),
                lsp_ready: true,
            }
        }
    }
}
```

- [ ] **Step 2: Run — expect FAIL**

`cargo test -p codescout mcp_resources::project_summary` → module has no content yet.

- [ ] **Step 3: Implement**

```rust
use async_trait::async_trait;
use super::{ResourceBytes, ResourceDescriptor, ResourceError, ResourceProvider};

#[derive(Debug, Clone, serde::Serialize)]
pub struct SummarySnapshot {
    pub active_project: Option<String>,
    pub index_status: String,
    pub language: Option<String>,
    pub lsp_ready: bool,
}

#[async_trait]
pub trait SummarySource: Send + Sync {
    async fn snapshot(&self) -> SummarySnapshot;
}

pub struct ProjectSummaryProvider<S: SummarySource> { source: S }

impl<S: SummarySource> ProjectSummaryProvider<S> {
    pub fn new(source: S) -> Self { Self { source } }
}

const URI: &str = "project://summary";

#[async_trait]
impl<S: SummarySource + 'static> ResourceProvider for ProjectSummaryProvider<S> {
    fn descriptors(&self) -> Vec<ResourceDescriptor> {
        vec![ResourceDescriptor {
            uri: URI.into(),
            name: "project-summary".into(),
            description: Some("Active project, index freshness, language, LSP readiness.".into()),
            mime_type: "application/json".into(),
        }]
    }

    async fn read(&self, uri: &str) -> Result<ResourceBytes, ResourceError> {
        if uri != URI { return Err(ResourceError::NotFound(uri.into())); }
        let snap = self.source.snapshot().await;
        let text = serde_json::to_string_pretty(&snap)
            .map_err(|e| ResourceError::Other(e.into()))?;
        Ok(ResourceBytes::Text(text))
    }
}
```

The real `SummarySource` implementation is a thin adapter over `Agent`/`ActiveProject` — wire it in Task 7 when we register providers in `server.rs::from_parts`. The trait/stub pair lets this task be pure and testable.

- [ ] **Step 4: Run — expect PASS**

`cargo test -p codescout mcp_resources::project_summary`

- [ ] **Step 5: Commit**

```bash
git add src/mcp_resources/project_summary.rs
git commit -m "feat(mcp): project://summary resource provider"
```

---

## Task 4: `Availability` enum + `Tool::availability()` default

**Files:**
- Modify: `src/tools/mod.rs`
- Test: `src/tools/mod.rs`

- [ ] **Step 1: Failing test**

Add to the existing tests module (or create one) in `src/tools/mod.rs`:

```rust
#[cfg(test)]
mod availability_tests {
    use super::*;

    struct AlwaysTool;
    #[async_trait::async_trait]
    impl Tool for AlwaysTool {
        fn name(&self) -> &str { "always" }
        fn description(&self) -> &str { "" }
        fn input_schema(&self) -> serde_json::Value { serde_json::json!({}) }
        async fn call(&self, _i: serde_json::Value, _c: &ToolContext) -> anyhow::Result<serde_json::Value> {
            Ok(serde_json::json!({}))
        }
    }

    #[test]
    fn default_availability_is_always() {
        let t = AlwaysTool;
        let caps = ToolCapabilities { has_lsp: false, has_embeddings: false, has_git_remote: false, has_libraries: false };
        assert!(matches!(t.availability(&caps), Availability::Always));
    }
}
```

- [ ] **Step 2: Run — expect FAIL**

`cargo test -p codescout tools::availability_tests`

- [ ] **Step 3: Implement**

Add to `src/tools/mod.rs` (near the `Tool` trait):

```rust
/// Snapshot of capabilities the Tool can inspect to decide visibility.
///
/// Built by `server.rs` from the active project state at each
/// `list_tools` call. Cheap to construct; do not hold references.
#[derive(Debug, Clone, Copy)]
pub struct ToolCapabilities {
    pub has_lsp: bool,
    pub has_embeddings: bool,
    pub has_git_remote: bool,
    pub has_libraries: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum Availability {
    Always,
    RequiresLsp,
    RequiresEmbeddings,
    RequiresGitRemote,
    RequiresLibraries,
}

impl Availability {
    pub fn is_available(self, c: &ToolCapabilities) -> bool {
        match self {
            Availability::Always            => true,
            Availability::RequiresLsp       => c.has_lsp,
            Availability::RequiresEmbeddings=> c.has_embeddings,
            Availability::RequiresGitRemote => c.has_git_remote,
            Availability::RequiresLibraries => c.has_libraries,
        }
    }
}
```

Extend the `Tool` trait:

```rust
    fn availability(&self, _caps: &ToolCapabilities) -> Availability {
        Availability::Always
    }
```

- [ ] **Step 4: Run — expect PASS**

`cargo test -p codescout tools::availability_tests` — and `cargo check` to confirm no other tool implementation broke.

- [ ] **Step 5: Commit**

```bash
git add src/tools/mod.rs
git commit -m "feat(tools): Availability enum + Tool::availability() default"
```

---

## Task 5: Mark LSP / embeddings / library tools

**Files:**
- Modify: `src/tools/symbol.rs` (hover, goto_definition, find_references, rename_symbol)
- Modify: `src/tools/semantic.rs` (semantic_search, index_project, index_status)
- Modify: `src/tools/library.rs` (register_library, list_libraries)

For each of the following tools, add an `availability()` override. Locate the tool struct via `cargo check` / `find_symbol` — the following pairs are mandatory:

| Tool struct                          | Override returns             |
|--------------------------------------|------------------------------|
| `Hover`                              | `Availability::RequiresLsp`  |
| `GotoDefinition`                     | `Availability::RequiresLsp`  |
| `FindReferences`                     | `Availability::RequiresLsp`  |
| `RenameSymbol`                       | `Availability::RequiresLsp`  |
| `SemanticSearch`                     | `Availability::RequiresEmbeddings` |
| `IndexProject`                       | `Availability::RequiresEmbeddings` |
| `IndexStatus`                        | `Availability::RequiresEmbeddings` |
| `RegisterLibrary`                    | `Availability::RequiresLibraries`  |
| `ListLibraries`                      | `Availability::RequiresLibraries`  |

- [ ] **Step 1: Write one representative failing test** (the mechanical rest won't need tests — compilation + the golden test in Task 6 is enough)

In `src/tools/symbol.rs`:

```rust
#[cfg(test)]
mod availability_tests {
    use super::*;
    use crate::tools::{Availability, ToolCapabilities};

    #[test]
    fn hover_requires_lsp() {
        let t = Hover;
        let off = ToolCapabilities { has_lsp: false, has_embeddings: false, has_git_remote: false, has_libraries: false };
        let on  = ToolCapabilities { has_lsp: true,  ..off };
        assert!(!t.availability(&off).is_available(&off));
        assert!( t.availability(&on).is_available(&on));
    }
}
```

- [ ] **Step 2: Run — expect FAIL**

`cargo test -p codescout tools::symbol::availability_tests`

- [ ] **Step 3: Add the overrides to all 9 tools**

Example body for each:

```rust
fn availability(&self, _c: &crate::tools::ToolCapabilities) -> crate::tools::Availability {
    crate::tools::Availability::RequiresLsp
}
```

- [ ] **Step 4: Run — expect PASS**

`cargo test -p codescout tools::symbol::availability_tests` plus `cargo test` whole suite — no regressions.

- [ ] **Step 5: Commit**

```bash
git add src/tools/symbol.rs src/tools/semantic.rs src/tools/library.rs
git commit -m "feat(tools): mark LSP/embeddings/library tools' availability"
```

---

## Task 6: Filter `list_tools` by availability + emit `tools/list_changed`

**Files:**
- Modify: `src/server.rs` (ServerHandler impl)
- Modify: `src/agent.rs` (activate_project)
- Test: `tests/` integration test or `src/server.rs` inline

- [ ] **Step 1: Failing test**

In `src/server.rs` (or a new integration test if project pattern prefers):

```rust
#[cfg(test)]
mod availability_filter_tests {
    use super::*;
    // ... uses existing test helpers `test_server_with_caps(caps)` — see src/server.rs::tests

    #[tokio::test]
    async fn list_tools_hides_lsp_tools_when_no_lsp() {
        let server = test_server_with_caps(ToolCapabilities {
            has_lsp: false, has_embeddings: true, has_git_remote: true, has_libraries: false,
        });
        let result = server.list_tools(None, fake_ctx()).await.unwrap();
        let names: Vec<_> = result.tools.iter().map(|t| t.name.clone()).collect();
        assert!(!names.iter().any(|n| n == "hover"));
        assert!( names.iter().any(|n| n == "semantic_search"));
    }
}
```

If `test_server_with_caps` / `fake_ctx` helpers do not exist, add them at the top of the tests module — keep them minimal, following existing `src/server.rs::tests` patterns.

- [ ] **Step 2: Run — expect FAIL**

`cargo test -p codescout availability_filter_tests`

- [ ] **Step 3: Implement the filter + broadcast**

In `src/server.rs::ServerHandler::list_tools`, build a `ToolCapabilities` from the server's current `Agent` snapshot and filter:

```rust
async fn list_tools(&self, _req: Option<PaginatedRequestParams>, _ctx: RequestContext<RoleServer>)
    -> std::result::Result<ListToolsResult, McpError>
{
    let caps = self.current_capabilities().await;  // new helper on CodeScoutServer
    let tools = self.tools.iter()
        .filter(|t| t.availability(&caps).is_available(&caps))
        .map(|t| {
            let schema = t.input_schema();
            let schema_obj = schema.as_object().cloned().unwrap_or_default();
            McpTool::new(t.name().to_owned(), t.description().to_owned(), schema_obj)
        })
        .collect();
    Ok(ListToolsResult::with_all_items(tools))
}
```

Add `CodeScoutServer::current_capabilities()` that reads:
- `has_lsp` — any registered LSP provider usable for the active project's language
- `has_embeddings` — `agent.config().embeddings.enabled` (or equivalent)
- `has_git_remote` — `agent.active_project.has_git_remote()`
- `has_libraries` — `agent.library_registry.has_any()` for a supported language

In `src/agent.rs::Agent::activate_project`, at the end:

```rust
let new_caps = self.capabilities_snapshot().await;
if new_caps != self.last_broadcast_caps.swap(new_caps) {
    if let Some(peer) = &self.server_peer {
        let _ = peer.notify_tools_list_changed().await;
    }
}
```

Make `ToolCapabilities` `PartialEq + Eq` to enable the comparison (add to the derive in Task 4 — go back and add it).

Wire `server_peer: ArcSwap<Option<Peer<RoleServer>>>` on `Agent` (or use an `OnceCell`) and populate it in `CodeScoutServer::set_peer()` called from the transport bootstrap.

- [ ] **Step 4: Run — expect PASS + full suite green**

```
cargo test -p codescout availability_filter_tests
cargo test -p codescout
```

- [ ] **Step 5: Commit**

```bash
git add src/server.rs src/agent.rs src/tools/mod.rs
git commit -m "feat(mcp): filter list_tools by availability, broadcast tools/list_changed"
```

---

## Task 7: Wire `ResourceRegistry` into the server

**Files:**
- Modify: `src/server.rs` — register providers, implement `list_resources` / `read_resource`, `enable_resources()` in capabilities
- Modify: `src/agent.rs` — refresh `memory://` on `activate_project`

- [ ] **Step 1: Failing test**

In `src/server.rs::tests`:

```rust
#[tokio::test]
async fn list_resources_includes_doc_and_memory() {
    let server = test_server_with_memory_dir(tempfile::tempdir().unwrap());
    let listed = server.list_resources(None, fake_ctx()).await.unwrap();
    let uris: Vec<_> = listed.resources.iter().map(|r| r.uri.clone()).collect();
    assert!(uris.iter().any(|u| u.starts_with("doc://")));
    assert!(uris.iter().any(|u| u == "project://summary"));
}

#[tokio::test]
async fn read_resource_roundtrips_a_doc() {
    let server = test_server_with_memory_dir(tempfile::tempdir().unwrap());
    let res = server.read_resource(
        ReadResourceRequestParam { uri: "doc://progressive-disclosure".into() },
        fake_ctx()).await.unwrap();
    assert!(!res.contents.is_empty());
}
```

- [ ] **Step 2: Run — expect FAIL**

`cargo test -p codescout server::tests -- list_resources read_resource`

- [ ] **Step 3: Implement handlers + registration**

In `CodeScoutServer::from_parts` (or wherever assembly happens), build the registry:

```rust
let mut rr = ResourceRegistry::new();
rr.register(Box::new(DocProvider::new(vec![
    DocSource { uri: "doc://progressive-disclosure".into(), name: "progressive-disclosure".into(),
                description: Some("Output sizing, overflow hints, agent guidance".into()),
                path: project_root.join("docs/PROGRESSIVE_DISCOVERABILITY.md") },
    DocSource { uri: "doc://tool-misbehaviors".into(), name: "tool-misbehaviors".into(),
                description: Some("Living log of observed tool bugs".into()),
                path: project_root.join("docs/TODO-tool-misbehaviors.md") },
])));
rr.register(Box::new(MemoryProvider::new(agent.memory_dir().await)));
rr.register(Box::new(ProjectSummaryProvider::new(AgentSummarySource(agent.clone()))));
// Tool-guide provider added in Task 8.
```

Update `get_info`:

```rust
fn get_info(&self) -> ServerInfo {
    ServerInfo::new(
        ServerCapabilities::builder()
            .enable_tools()
            .enable_resources()      // new
            .build()
    ).with_instructions(self.instructions.clone())
}
```

Implement handlers:

```rust
async fn list_resources(
    &self,
    _req: Option<PaginatedRequestParams>,
    _ctx: RequestContext<RoleServer>,
) -> std::result::Result<ListResourcesResult, McpError> {
    let resources = self.resources.list().into_iter().map(|d| Resource {
        uri: d.uri,
        name: d.name,
        description: d.description,
        mime_type: Some(d.mime_type),
        ..Default::default()
    }).collect();
    Ok(ListResourcesResult { resources, next_cursor: None })
}

async fn read_resource(
    &self,
    req: ReadResourceRequestParam,
    _ctx: RequestContext<RoleServer>,
) -> std::result::Result<ReadResourceResult, McpError> {
    match self.resources.read(&req.uri).await {
        Ok(ResourceBytes::Text(t)) => Ok(ReadResourceResult {
            contents: vec![ResourceContents::text(t, &req.uri)],
        }),
        Ok(ResourceBytes::Blob(b)) => Ok(ReadResourceResult {
            contents: vec![ResourceContents::blob(b, &req.uri)],
        }),
        Err(ResourceError::NotFound(u)) =>
            Err(McpError::resource_not_found(format!("resource not found: {u}"), None)),
        Err(e) => Err(McpError::internal_error(e.to_string(), None)),
    }
}
```

Implement `AgentSummarySource` as a thin adapter reading from `Agent` state and mapping to `SummarySnapshot`.

Hook `activate_project` in `src/agent.rs` to rebuild the `MemoryProvider` portion of the registry — simplest approach: `ResourceRegistry` is replaced wholesale on project switch, held behind `ArcSwap` on `CodeScoutServer`. Document the choice inline.

- [ ] **Step 4: Run — expect PASS**

`cargo test -p codescout server::tests` + full suite.

- [ ] **Step 5: Commit**

```bash
git add src/server.rs src/agent.rs
git commit -m "feat(mcp): wire resource registry, list/read handlers, enable_resources capability"
```

---

## Task 8: Tool-description diet + generated `doc://codescout-tool-guide`

**Files:**
- Modify: every tool file's `description()` (audit pass)
- Modify: `src/mcp_resources/tool_guide.rs`
- Modify: `src/server.rs` (register `ToolGuideProvider` built from the tool registry)

- [ ] **Step 1: Baseline measurement test**

Add a failing guard test in `src/server.rs::tests`:

```rust
#[test]
fn tool_descriptions_stay_under_budget() {
    let server = test_server_default();
    for t in &server.tools {
        let d = t.description();
        assert!(
            d.len() <= 300,
            "tool `{}` description is {} chars (cap 300)",
            t.name(), d.len()
        );
    }
}
```

- [ ] **Step 2: Run — expect FAIL (at least some tools over 300)**

`cargo test -p codescout tool_descriptions_stay_under_budget`

- [ ] **Step 3: Shorten descriptions + implement ToolGuideProvider**

For each offending tool, trim `description()` to one line of purpose + the 1–2 params that change behavior. Move examples + long rationale to the generated tool guide.

Create `src/mcp_resources/tool_guide.rs`:

```rust
use std::sync::Arc;
use async_trait::async_trait;
use super::{ResourceBytes, ResourceDescriptor, ResourceError, ResourceProvider};
use crate::tools::Tool;

const URI: &str = "doc://codescout-tool-guide";

pub struct ToolGuideProvider {
    tools: Vec<Arc<dyn Tool>>,
}

impl ToolGuideProvider {
    pub fn new(tools: Vec<Arc<dyn Tool>>) -> Self { Self { tools } }

    fn render(&self) -> String {
        let mut s = String::from("# Codescout tool guide\n\n");
        s.push_str("Long-form usage notes. Short descriptions live in the MCP tool list; this \
                    resource holds examples and 'when to use this vs. that' prose.\n\n");
        for t in &self.tools {
            s.push_str(&format!("## {}\n\n{}\n\n", t.name(), t.long_docs().unwrap_or_default()));
        }
        s
    }
}

#[async_trait]
impl ResourceProvider for ToolGuideProvider {
    fn descriptors(&self) -> Vec<ResourceDescriptor> {
        vec![ResourceDescriptor {
            uri: URI.into(),
            name: "codescout-tool-guide".into(),
            description: Some("Extended usage notes for every codescout tool.".into()),
            mime_type: "text/markdown".into(),
        }]
    }
    async fn read(&self, uri: &str) -> Result<ResourceBytes, ResourceError> {
        if uri != URI { return Err(ResourceError::NotFound(uri.into())); }
        Ok(ResourceBytes::Text(self.render()))
    }
}
```

Add `fn long_docs(&self) -> Option<&str> { None }` to the `Tool` trait (default). Override on tools that need extended docs — populate with the paragraphs you just deleted from `description()`.

Register the provider in `CodeScoutServer::from_parts` after the other `register` calls.

Update `tool_descriptions_stay_under_budget` is the primary guard; no additional tests needed for the guide itself beyond existing resource round-trip test (extend Task 7's test to assert the guide URI is listed).

- [ ] **Step 4: Run — expect PASS**

`cargo test -p codescout` (full suite must stay green).

- [ ] **Step 5: Commit**

```bash
git add src/tools/ src/mcp_resources/tool_guide.rs src/server.rs
git commit -m "feat(mcp): cap tool descriptions, generate doc://codescout-tool-guide"
```

---

## Task 9: Update prompt surfaces + bump `ONBOARDING_VERSION`

**Files:**
- Modify: `src/prompts/server_instructions.md`
- Modify: `src/prompts/onboarding_prompt.md`
- Modify: `src/tools/workflow.rs` (`build_system_prompt_draft` and `ONBOARDING_VERSION`)

- [ ] **Step 1: Failing test**

Update the existing `build_system_prompt_draft` tests (in `src/tools/workflow.rs` around L3338) — add:

```rust
#[test]
fn system_prompt_points_to_tool_guide_resource() {
    let prompt = build_system_prompt_draft(&test_config());
    assert!(prompt.contains("doc://codescout-tool-guide"),
            "system prompt must point LLMs to the generated tool guide resource");
    assert_eq!(ONBOARDING_VERSION, 4);
}
```

- [ ] **Step 2: Run — expect FAIL**

`cargo test -p codescout workflow::tests::system_prompt_points_to_tool_guide_resource`

- [ ] **Step 3: Edit the three prompt surfaces**

In `server_instructions.md`: replace example-laden sections with one-liners that name `doc://codescout-tool-guide`, `doc://progressive-disclosure`, `memory://<name>`. Keep Iron Laws 1–5 intact and at the top.

In `onboarding_prompt.md`: replace tool-by-tool prose with a pointer to the resource.

In `build_system_prompt_draft`: same treatment; add a sentence like "Extended tool usage notes: read the resource `doc://codescout-tool-guide`."

Bump `const ONBOARDING_VERSION: u32 = 3;` → `4` at `src/tools/workflow.rs:15`.

- [ ] **Step 4: Run — expect PASS**

`cargo test -p codescout workflow::tests` — all prompt-drafting tests must stay green.

- [ ] **Step 5: Commit**

```bash
git add src/prompts/ src/tools/workflow.rs
git commit -m "feat(prompts): shorten surfaces, point to doc://codescout-tool-guide, bump ONBOARDING_VERSION=4"
```

---

## Task 10: 2 Hz throttle in `ProgressReporter`

**Files:**
- Modify: `src/tools/progress.rs`

- [ ] **Step 1: Failing test**

```rust
#[cfg(test)]
mod throttle_tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    struct CountingPeer { count: Arc<AtomicU32> }
    // Implement just enough of the peer surface used by ProgressReporter, OR
    // refactor ProgressReporter so the emission side can be mocked via a trait.

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn at_most_two_emissions_per_second() {
        // Construct a reporter over a counting stub, fire 100 report() calls in 900ms,
        // assert counter <= 2. Requires virtual time via tokio::test start_paused.
    }
}
```

Because the current `ProgressReporter::report` touches a live `Peer<RoleServer>`, this task must also refactor the emission path behind a small `ProgressSink` trait to make it mockable. Keep the public API (`report`, `report_text`) identical.

- [ ] **Step 2: Run — expect FAIL / not-implemented**

- [ ] **Step 3: Implement throttle**

Inside `ProgressReporter`:

```rust
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant};

struct Throttle { last: Mutex<Option<Instant>>, min_gap: Duration }

impl Throttle {
    fn new() -> Self { Self { last: Mutex::new(None), min_gap: Duration::from_millis(500) } }
    async fn allow(&self) -> bool {
        let now = Instant::now();
        let mut g = self.last.lock().await;
        match *g {
            Some(t) if now.duration_since(t) < self.min_gap => false,
            _ => { *g = Some(now); true }
        }
    }
}
```

Gate every `notify_progress` call behind `throttle.allow().await`. Dropped progress events are coalesced (the next permitted call carries the most recent state; no internal queue).

- [ ] **Step 4: Run — expect PASS**

`cargo test -p codescout tools::progress`

- [ ] **Step 5: Commit**

```bash
git add src/tools/progress.rs
git commit -m "feat(mcp): 2 Hz throttle on ProgressReporter emissions"
```

---

## Task 11: Progress call sites

**Files:**
- Modify: `src/tools/semantic.rs` (`IndexProject`, `SemanticSearch`)
- Modify: LSP cold-start path (find via `grep "get_or_start"` under `src/lsp`)
- Modify: `src/tools/command.rs` (`run_command` long-running branch)

- [ ] **Step 1: Failing tests**

For `IndexProject`, write a test that constructs the tool with a mock progress sink and asserts `report()` was called N times for an N-file fixture. Mirror for `SemanticSearch` cold-start and for the LSP cold-start shared helper.

Test shape (pseudocode — exact types depend on the `ProgressSink` refactor from Task 10):

```rust
#[tokio::test]
async fn index_project_reports_progress_per_file() {
    let sink = Arc::new(CountingSink::default());
    let ctx = ToolContext { progress: Some(ProgressReporter::with_sink(sink.clone())), ..test_ctx() };
    IndexProject.call(input_for(&["a.rs","b.rs","c.rs"]), &ctx).await.unwrap();
    assert!(sink.steps.load(Ordering::Relaxed) >= 3);
}
```

- [ ] **Step 2: Run — expect FAIL**

- [ ] **Step 3: Add emission sites**

- In `IndexProject`, call `ctx.progress.as_ref()?.report(i as u32, Some(total)).await` once per file, plus `report_text` on start and finish.
- In `SemanticSearch`, on cold path emit `report_text("loading embedding model")` and `report_text("searching")`.
- In LSP cold-start, emit `report_text("starting <lang>")` / `report_text("indexing workspace")`.
- In `run_command`, when elapsed wall time ≥ 2s, emit `report_text(format!("{}s, {} lines so far", elapsed, lines))` per tick. Throttle enforces the 2 Hz cap.

- [ ] **Step 4: Run — expect PASS**

`cargo test -p codescout` — every new test green, no regressions.

- [ ] **Step 5: Commit**

```bash
git add src/tools/semantic.rs src/tools/command.rs src/lsp/
git commit -m "feat(mcp): emit progress notifications from index_project, semantic_search, LSP cold-start, run_command"
```

---

## Task 12: Experimental docs page + index link

**Files:**
- Create: `docs/manual/src/experimental/mcp-resources.md`
- Modify: `docs/manual/src/experimental/index.md`

- [ ] **Step 1: Write the page**

Content (fill the sections; keep it short — users will skim):

```markdown
# MCP resources, tool diet, and progress notifications

> ⚠ Experimental — may change without notice.

Codescout now exposes three mechanisms to reduce per-turn token overhead and
surface activity from long-running operations.

## Resources
`resources/list` + `resources/read` publish:
- `doc://progressive-disclosure`, `doc://tool-misbehaviors`, `doc://codescout-tool-guide`
- One `memory://<name>` per file in the project's memory directory
- `project://summary` for active-project / index / LSP status

Claude Code surfaces resources via `@mention` autocompletion.

## Tool-description diet
Descriptions are capped at 300 characters. Extended usage lives in
`doc://codescout-tool-guide`, fetched on demand.

## Conditional exposure
Tools are hidden when their capability is missing:
- LSP tools when no LSP provider is wired
- Embedding tools when embeddings are disabled
- Library tools when no supported language is detected

`activate_project` emits `notifications/tools/list_changed` when the set shifts.

## Progress notifications
`index_project`, `semantic_search`, LSP cold-start, and long-running
`run_command` emit `notifications/progress` (throttled to 2 Hz).
```

- [ ] **Step 2: Link from the index**

In `docs/manual/src/experimental/index.md`, add:

```markdown
- [MCP resources, tool diet, progress notifications](mcp-resources.md)
```

- [ ] **Step 3: Commit**

```bash
git add docs/manual/src/experimental/
git commit -m "docs(experimental): mcp resources, tool diet, progress notifications"
```

---

## Task 13: Final verification

- [ ] **Step 1: Full local verification**

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Expected: all three green. If clippy fires on new code, fix in-place.

- [ ] **Step 2: Build release binary + smoke test via `/mcp` reload**

```bash
cargo build --release
```

Then in a Claude Code session with codescout active, run `/mcp` → reconnect. Verify:
- `@mcp__codescout__doc://progressive-disclosure` autocompletes in the composer
- `list_tools` output for a plain-text directory hides LSP tools (run something that triggers `list_tools` after `activate_project` on a non-Rust dir)
- Progress bar appears during an `index_project` call on a medium repo

- [ ] **Step 3: Open questions follow-up**

Add to `docs/trackers/mcp-integration-ideas-2026-04.md` under a new section "After bundle (a) ships":
- measurement: capture `tools/list` response size before/after (for #7 usage-driven pruning)
- decision: keep or split `doc://codescout-tool-guide` once it has 20+ tools

- [ ] **Step 4: Ready-for-graduation marker**

No commit here — verification is a gate. If everything passes, the PR is ready for review and, after one week of live use, graduation to `master` via the cherry-pick-with-doc-move dance from `CLAUDE.md`.

---

## Self-review notes (addressed inline)

- **Spec coverage:** all three features (resources, tool-diet + conditional, progress) have dedicated tasks (1–3 + 7; 4–6 + 8–9; 10–11). Experimental docs page required by `CLAUDE.md` lands in Task 12.
- **Placeholder scan:** one intentional non-code placeholder in Task 10 (counting peer mock) — acceptable because the refactor shape is sketched and the exact mock type depends on what the `ProgressSink` abstraction looks like once extracted. Everything else has concrete code.
- **Type consistency:** `ToolCapabilities` is introduced in Task 4, consumed in Tasks 5 and 6, unchanged after. `Availability` enum variants are stable. `ResourceBytes` / `ResourceError` / `ResourceProvider` — identical shape in Tasks 1, 2, 3, 7, 8.
- **Scope check:** single PR on `experiments`, 12 working tasks + 1 verification task. Spec calls this a single feature bundle — not over-scoped.
