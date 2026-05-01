# Call Graph (item A) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the `call_graph` tool stub with a working implementation that returns transitive caller/callee graphs, backed by LSP `callHierarchy` with a tree-sitter classifier fallback and a sqlite edge cache.

**Architecture:** A bottom-up build — first add LSP `callHierarchy` methods to the `LspClientOps` trait + real client + mock, then a tree-sitter call-site classifier, then a shared one-hop edge resolver, then the sqlite edge cache wired to `did_change` invalidation, then a BFS traversal engine, finally wire the tool's output formatter. Live-LSP smoke tests, prompt surface updates, and an experimental docs page close the work.

**Tech Stack:** Rust, `lsp-types` (already a dep), `tree-sitter` per-language grammars (already deps), `rusqlite` (already a dep, used by `src/embed/index.rs`), `tokio` for async.

**Spec:** `docs/superpowers/specs/2026-05-01-call-graph-design.md` (commit `227ec87`).

**Pre-existing:** `src/tools/symbol/call_graph.rs` exists as a stub returning `RecoverableError`. It's already registered in `src/tools/symbol/mod.rs` and `src/server.rs::CodeScoutServer::from_parts`. Schema (`symbol`, `direction`, `max_depth`) is frozen.

---

## File Structure

**New files:**
- `src/lsp/call_hierarchy.rs` — local types or re-exports from `lsp-types`; capability gating helper.
- `src/tools/symbol/call_edges/mod.rs` — module entry; re-exports `resolve_one_hop`, `Edge`, `Direction`.
- `src/tools/symbol/call_edges/resolver.rs` — `resolve_one_hop(symbol, dir)`; LSP-first then ts-fallback.
- `src/tools/symbol/call_edges/ts_classifier.rs` — per-language call-expression node-type map.
- `src/tools/symbol/call_edges/cache.rs` — sqlite read/write/invalidate over `call_edges` table.
- `src/tools/symbol/call_graph/traversal.rs` — BFS engine with depth-coherent cap.
- `tests/fixtures/call_graph/` — small per-language fixture projects for live-LSP smoke tests.
- `docs/manual/src/experimental/call-graph.md` — experimental docs page.

**Modified files:**
- `src/lsp/ops.rs` — three new methods on `LspClientOps`.
- `src/lsp/client.rs` — real impl of the three methods, capability handshake check.
- `src/lsp/mock.rs` (or wherever `MockLspClient` lives) — mock impl.
- `src/agent.rs` or `src/lsp/mod.rs` — wire `notify_file_changed` to also invalidate the edge cache.
- `src/tools/symbol/call_graph.rs` — replace stub `call()` with real impl + new `format_compact()`.
- `src/tools/symbol/mod.rs` — declare `call_edges` and `call_graph::traversal` submodules if not already present.
- `src/embed/index.rs` (or wherever the project DB schema lives) — add `call_edges` table migration.
- `src/prompts/server_instructions.md`, `src/prompts/onboarding_prompt.md`, `src/prompts/builders.rs` — mention `call_graph`.
- `src/tools/onboarding.rs` — bump `ONBOARDING_VERSION`.
- `docs/manual/src/experimental/index.md` — link to the new page.

---

## Branch & Workspace

All work on the `experiments` branch (per `CLAUDE.md`). Each task ends in a commit. Cherry-pick to `master` only after the full feature lands and is manually verified via `cargo build --release` + `/mcp` restart.

---

## Task 1: Add `callHierarchy` methods to `LspClientOps` trait

**Files:**
- Modify: `src/lsp/ops.rs`

**Why first:** The trait shape gates every downstream impl. Once it compiles with stubs in `LspClient` and `MockLspClient`, downstream tasks have something to call against.

- [ ] **Step 1: Read the current trait**

Use `mcp__codescout__symbols` on `src/lsp/ops.rs` to confirm the existing method shapes you'll mirror.

- [ ] **Step 2: Write the failing test**

Add to `src/lsp/ops.rs` (or a new `tests/` module if one doesn't exist there). The test asserts the trait has the new methods by being generic over `LspClientOps` and calling them through a no-op mock:

```rust
#[cfg(test)]
mod call_hierarchy_trait_tests {
    use super::*;

    struct NoopLsp;
    #[async_trait::async_trait]
    impl LspClientOps for NoopLsp {
        // ... stub all existing methods to unimplemented!() ...

        async fn prepare_call_hierarchy(
            &self, _path: &Path, _line: u32, _col: u32, _language_id: &str,
        ) -> anyhow::Result<Option<lsp_types::CallHierarchyItem>> {
            Ok(None)
        }

        async fn incoming_calls(
            &self, _item: &lsp_types::CallHierarchyItem, _language_id: &str,
        ) -> anyhow::Result<Vec<lsp_types::CallHierarchyIncomingCall>> {
            Ok(vec![])
        }

        async fn outgoing_calls(
            &self, _item: &lsp_types::CallHierarchyItem, _language_id: &str,
        ) -> anyhow::Result<Vec<lsp_types::CallHierarchyOutgoingCall>> {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn call_hierarchy_methods_compile_and_default_empty() {
        let lsp = NoopLsp;
        assert!(lsp.prepare_call_hierarchy(
            Path::new("x.rs"), 0, 0, "rust").await.unwrap().is_none());
        assert!(lsp.incoming_calls(&dummy_item(), "rust").await.unwrap().is_empty());
        assert!(lsp.outgoing_calls(&dummy_item(), "rust").await.unwrap().is_empty());
    }

    fn dummy_item() -> lsp_types::CallHierarchyItem { /* construct with placeholder fields */ }
}
```

- [ ] **Step 3: Run the test — verify it fails**

Run: `cargo test -p codescout call_hierarchy_methods_compile_and_default_empty`
Expected: compile error — methods don't exist on the trait yet.

- [ ] **Step 4: Add the trait methods**

Edit `src/lsp/ops.rs`. Insert below `did_change`:

```rust
async fn prepare_call_hierarchy(
    &self,
    path: &Path,
    line: u32,
    col: u32,
    language_id: &str,
) -> anyhow::Result<Option<lsp_types::CallHierarchyItem>>;

async fn incoming_calls(
    &self,
    item: &lsp_types::CallHierarchyItem,
    language_id: &str,
) -> anyhow::Result<Vec<lsp_types::CallHierarchyIncomingCall>>;

async fn outgoing_calls(
    &self,
    item: &lsp_types::CallHierarchyItem,
    language_id: &str,
) -> anyhow::Result<Vec<lsp_types::CallHierarchyOutgoingCall>>;
```

- [ ] **Step 5: Add stub impls in real `LspClient` and `MockLspClient`**

Both must compile but return `Ok(None)` / `Ok(vec![])`. This makes the trait change non-breaking. Real impls land in Tasks 3–4.

`src/lsp/client.rs` — at the bottom of `impl crate::lsp::ops::LspClientOps for LspClient`:

```rust
async fn prepare_call_hierarchy(&self, _path: &Path, _line: u32, _col: u32, _language_id: &str)
    -> anyhow::Result<Option<lsp_types::CallHierarchyItem>>
{
    anyhow::bail!("prepare_call_hierarchy not yet implemented")
}
async fn incoming_calls(&self, _item: &lsp_types::CallHierarchyItem, _language_id: &str)
    -> anyhow::Result<Vec<lsp_types::CallHierarchyIncomingCall>>
{
    anyhow::bail!("incoming_calls not yet implemented")
}
async fn outgoing_calls(&self, _item: &lsp_types::CallHierarchyItem, _language_id: &str)
    -> anyhow::Result<Vec<lsp_types::CallHierarchyOutgoingCall>>
{
    anyhow::bail!("outgoing_calls not yet implemented")
}
```

In `MockLspClient` — return `Ok(None)` / `Ok(vec![])` so existing mock-using tests still pass.

- [ ] **Step 6: Run the test — verify it passes**

Run: `cargo test -p codescout call_hierarchy_methods_compile_and_default_empty`
Expected: PASS.

- [ ] **Step 7: Run the full suite to confirm no regressions**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass.

- [ ] **Step 8: Commit**

```
feat(lsp): add callHierarchy methods to LspClientOps trait
```

---

## Task 2: Capability gating helper

**Files:**
- Create: `src/lsp/call_hierarchy.rs`
- Modify: `src/lsp/mod.rs` (declare module)

**Why:** When an LSP server doesn't advertise `callHierarchyProvider`, we must skip the LSP path and fall back to tree-sitter. Centralize the capability check.

- [ ] **Step 1: Write the failing test**

In `src/lsp/call_hierarchy.rs`:

```rust
use lsp_types::ServerCapabilities;

pub fn supports_call_hierarchy(caps: &ServerCapabilities) -> bool {
    matches!(caps.call_hierarchy_provider, Some(_))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_when_provider_present() {
        let caps = ServerCapabilities {
            call_hierarchy_provider: Some(lsp_types::CallHierarchyServerCapability::Simple(true)),
            ..Default::default()
        };
        assert!(supports_call_hierarchy(&caps));
    }

    #[test]
    fn unsupported_when_none() {
        let caps = ServerCapabilities::default();
        assert!(!supports_call_hierarchy(&caps));
    }
}
```

- [ ] **Step 2: Declare the module**

Edit `src/lsp/mod.rs`:

```rust
pub mod call_hierarchy;
```

- [ ] **Step 3: Run the test — verify it passes**

Run: `cargo test -p codescout supports_when_provider_present unsupported_when_none`
Expected: PASS.

- [ ] **Step 4: Commit**

```
feat(lsp): add capability check for callHierarchy
```

---

## Task 3: Real LSP `callHierarchy` impl in `LspClient`

**Files:**
- Modify: `src/lsp/client.rs`

This task wires actual LSP requests. Use the existing request patterns in `LspClient` — find the methods that wrap `references` and `goto_definition` and mirror their structure (request id, response correlation, timeout, idempotency flag).

- [ ] **Step 1: Read existing references/goto_definition impls**

Use `mcp__codescout__symbols(name="references", path="src/lsp/client.rs", include_body=true)`. Note how it serializes params, sends the request, awaits the response. Mirror that.

- [ ] **Step 2: Write integration tests using real rust-analyzer fixture**

Create `tests/lsp_call_hierarchy.rs`:

```rust
// Behind a #[cfg(feature = "live-lsp")] flag — the standard pattern in this repo.
// Use the existing test fixture conventions; if there's a small fixture crate,
// extend it with three functions: a, b, c where a -> b -> c.
```

This test runs only with the `live-lsp` feature. Don't gate it behind `#[ignore]` directly; follow whatever pattern other live-LSP tests in this repo use (check `src/lsp/client.rs::tests` for examples).

- [ ] **Step 3: Implement `prepare_call_hierarchy`**

Replace the stub from Task 1.5. Send LSP method `textDocument/prepareCallHierarchy` with `CallHierarchyPrepareParams { text_document, position }`. Use `is_idempotent_lsp_method` if needed; this method IS idempotent. Return the first item in the response, or `None` if empty.

If the server's capabilities (cached at `initialize` time inside `LspClient`) indicate no `callHierarchyProvider`, return `Ok(None)` immediately without sending.

- [ ] **Step 4: Implement `incoming_calls` and `outgoing_calls`**

Send `callHierarchy/incomingCalls` / `callHierarchy/outgoingCalls` with the `CallHierarchyItem`. Return the deserialized list.

- [ ] **Step 5: Run integration test (if rust-analyzer is on PATH)**

Run: `cargo test --features live-lsp -p codescout call_hierarchy_rust`
Expected: PASS, with `a` calling `b` returning `b` as outgoing, `a` as incoming for `b`.

If `rust-analyzer` is not available, skip; covered by Task 11.

- [ ] **Step 6: Run unit suite**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass.

- [ ] **Step 7: Commit**

```
feat(lsp): implement callHierarchy in LspClient
```

---

## Task 4: `MockLspClient` callHierarchy impl

**Files:**
- Modify: wherever `MockLspClient` lives (likely `src/lsp/mock.rs` — confirm via `mcp__codescout__symbols(name="MockLspClient")`).

The mock should let downstream tests (resolver, traversal) seed expected callHierarchy responses without spinning up a real LSP.

- [ ] **Step 1: Add fields for canned responses**

```rust
pub struct MockLspClient {
    // ... existing fields ...
    pub prepare_call_hierarchy_results:
        std::sync::Mutex<std::collections::HashMap<(PathBuf, u32, u32), Option<lsp_types::CallHierarchyItem>>>,
    pub incoming_calls_results:
        std::sync::Mutex<std::collections::HashMap<String, Vec<lsp_types::CallHierarchyIncomingCall>>>,
    pub outgoing_calls_results:
        std::sync::Mutex<std::collections::HashMap<String, Vec<lsp_types::CallHierarchyOutgoingCall>>>,
}
```

Key for incoming/outgoing maps is `item.name` (sufficient for tests).

- [ ] **Step 2: Replace stubs with map lookups**

```rust
async fn prepare_call_hierarchy(&self, path: &Path, line: u32, col: u32, _language_id: &str)
    -> anyhow::Result<Option<lsp_types::CallHierarchyItem>>
{
    Ok(self.prepare_call_hierarchy_results.lock().unwrap()
        .get(&(path.to_path_buf(), line, col))
        .cloned()
        .flatten())
}
// ... similarly for incoming_calls, outgoing_calls keyed on item.name ...
```

- [ ] **Step 3: Write a test that drives the mock**

```rust
#[tokio::test]
async fn mock_call_hierarchy_returns_canned_responses() {
    let mock = MockLspClient::new(/* ... */);
    let item = lsp_types::CallHierarchyItem { name: "a".into(), /* ... */ };
    mock.prepare_call_hierarchy_results.lock().unwrap()
        .insert((PathBuf::from("a.rs"), 0, 0), Some(item.clone()));
    let got = mock.prepare_call_hierarchy(Path::new("a.rs"), 0, 0, "rust").await.unwrap();
    assert_eq!(got.unwrap().name, "a");
}
```

- [ ] **Step 4: Run test, verify pass**

Run: `cargo test -p codescout mock_call_hierarchy_returns_canned_responses`
Expected: PASS.

- [ ] **Step 5: `cargo fmt && cargo clippy -- -D warnings && cargo test`**

Expected: all pass.

- [ ] **Step 6: Commit**

```
feat(lsp): MockLspClient supports callHierarchy
```

---

## Task 5: Tree-sitter call-site classifier

**Files:**
- Create: `src/tools/symbol/call_edges/mod.rs` (initially just `pub mod ts_classifier;`)
- Create: `src/tools/symbol/call_edges/ts_classifier.rs`
- Modify: `src/tools/symbol/mod.rs` — add `pub mod call_edges;`

- [ ] **Step 1: Sketch the API and one failing test per supported language**

`src/tools/symbol/call_edges/ts_classifier.rs`:

```rust
use tree_sitter::{Node, Tree};

/// Returns true if the byte position lies inside a call-expression node
/// for the given language.
pub fn position_is_call(tree: &Tree, byte_offset: usize, language_id: &str) -> bool {
    let node = tree.root_node().descendant_for_byte_range(byte_offset, byte_offset);
    let Some(mut n) = node else { return false; };
    let call_kinds: &[&str] = match language_id {
        "rust"       => &["call_expression", "method_call_expression", "macro_invocation"],
        "python"     => &["call"],
        "typescript" | "javascript" | "tsx" | "jsx"
                     => &["call_expression", "new_expression"],
        "kotlin"     => &["call_expression"],
        "java"       => &["method_invocation", "object_creation_expression"],
        _            => return false,
    };
    loop {
        if call_kinds.contains(&n.kind()) { return true; }
        match n.parent() {
            Some(p) => n = p,
            None => return false,
        }
    }
}
```

- [ ] **Step 2: Tests — one per language**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str, lang: tree_sitter::Language) -> Tree {
        let mut p = tree_sitter::Parser::new();
        p.set_language(&lang).unwrap();
        p.parse(src, None).unwrap()
    }

    #[test]
    fn rust_call_expression_classifies() {
        let src = "fn main() { foo(1); }";
        let tree = parse(src, tree_sitter_rust::LANGUAGE.into());
        let byte = src.find("foo").unwrap();
        assert!(position_is_call(&tree, byte, "rust"));
    }

    #[test]
    fn rust_type_ref_does_not_classify_as_call() {
        let src = "fn main() { let x: Foo = bar(); }";
        let tree = parse(src, tree_sitter_rust::LANGUAGE.into());
        let byte = src.find("Foo").unwrap();
        assert!(!position_is_call(&tree, byte, "rust"));
    }

    #[test]
    fn python_call_classifies() {
        let src = "x = foo(1)\n";
        let tree = parse(src, tree_sitter_python::LANGUAGE.into());
        let byte = src.find("foo").unwrap();
        assert!(position_is_call(&tree, byte, "python"));
    }

    // Repeat for typescript (call_expression, new_expression),
    // kotlin (call_expression), java (method_invocation, object_creation_expression).
    // For each language: one positive (call site) and one negative (type ref / import).
}
```

The exact tree-sitter language crate names are already deps — see `Cargo.toml`. If a language crate isn't present, that language's tests are gated behind a feature flag matching the existing convention; consult `src/embed/ast_chunker.rs::LANGUAGE_REGISTRY` to confirm what's available.

- [ ] **Step 3: Run tests — verify they fail (file doesn't exist) then pass after creating it**

Run: `cargo test -p codescout ts_classifier`
Expected: PASS after creation.

- [ ] **Step 4: `cargo fmt && cargo clippy -- -D warnings && cargo test`**

Expected: all pass.

- [ ] **Step 5: Commit**

```
feat(call_edges): tree-sitter call-site classifier per language
```

---

## Task 6: Edge resolver — one-hop, LSP-first with ts-fallback

**Files:**
- Create: `src/tools/symbol/call_edges/resolver.rs`

Pure function — no caching yet (Task 7 layers it). Takes a symbol's location, returns one hop of edges in the requested direction.

- [ ] **Step 1: Define types**

```rust
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction { Callers, Callees }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeSource { Lsp, Ts }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub caller_sym: String,
    pub callee_sym: String,
    pub file: PathBuf,
    pub line: u32,
    pub col: u32,
    pub source: EdgeSource,
}
```

- [ ] **Step 2: Resolver signature**

```rust
pub async fn resolve_one_hop(
    client: &dyn crate::lsp::ops::LspClientOps,
    sym_name: &str,
    sym_path: &std::path::Path,
    sym_line: u32,
    sym_col: u32,
    language_id: &str,
    direction: Direction,
) -> anyhow::Result<Vec<Edge>>;
```

- [ ] **Step 3: Write the LSP-success test**

```rust
#[tokio::test]
async fn resolve_one_hop_uses_lsp_when_available() {
    let mock = MockLspClient::new(/* ... */);
    // Seed prepare + incoming_calls
    let item = lsp_types::CallHierarchyItem { name: "a".into(), /* ... */ };
    mock.prepare_call_hierarchy_results.lock().unwrap()
        .insert((PathBuf::from("a.rs"), 10, 5), Some(item.clone()));
    mock.incoming_calls_results.lock().unwrap()
        .insert("a".into(), vec![/* one CallHierarchyIncomingCall pointing to b at b.rs:3:0 */]);

    let edges = resolve_one_hop(&mock, "a", Path::new("a.rs"), 10, 5, "rust", Direction::Callers)
        .await.unwrap();

    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].source, EdgeSource::Lsp);
    assert_eq!(edges[0].caller_sym, "b");
    assert_eq!(edges[0].callee_sym, "a");
}
```

- [ ] **Step 4: Write the ts-fallback test**

```rust
#[tokio::test]
async fn resolve_one_hop_falls_back_to_ts_when_lsp_returns_none() {
    let mock = MockLspClient::new(/* ... */);
    // No prepare_call_hierarchy result seeded -> returns None.
    // Seed mock.references_results so the fallback has refs to classify.
    // Pre-populate fixture file on disk that the classifier can parse.
    let edges = resolve_one_hop(&mock, "a", &fixture_path, 0, 0, "rust", Direction::Callers)
        .await.unwrap();
    assert!(edges.iter().all(|e| matches!(e.source, EdgeSource::Ts)));
}
```

- [ ] **Step 5: Implement**

```rust
pub async fn resolve_one_hop(
    client: &dyn crate::lsp::ops::LspClientOps,
    sym_name: &str,
    sym_path: &std::path::Path,
    sym_line: u32,
    sym_col: u32,
    language_id: &str,
    direction: Direction,
) -> anyhow::Result<Vec<Edge>> {
    // LSP path
    if let Some(item) = client
        .prepare_call_hierarchy(sym_path, sym_line, sym_col, language_id).await?
    {
        let edges = match direction {
            Direction::Callers => {
                let calls = client.incoming_calls(&item, language_id).await?;
                calls.into_iter().flat_map(|c| {
                    let from = c.from.name;
                    c.from_ranges.into_iter().map(move |r| Edge {
                        caller_sym: from.clone(),
                        callee_sym: sym_name.to_string(),
                        file: lsp_uri_to_path(&c.from.uri).unwrap_or_default(),
                        line: r.start.line,
                        col: r.start.character,
                        source: EdgeSource::Lsp,
                    })
                }).collect()
            }
            Direction::Callees => {
                let calls = client.outgoing_calls(&item, language_id).await?;
                calls.into_iter().flat_map(|c| {
                    let to = c.to.name;
                    let to_uri = c.to.uri.clone();
                    c.from_ranges.into_iter().map(move |r| Edge {
                        caller_sym: sym_name.to_string(),
                        callee_sym: to.clone(),
                        file: lsp_uri_to_path(&to_uri).unwrap_or_default(),
                        line: r.start.line,
                        col: r.start.character,
                        source: EdgeSource::Lsp,
                    })
                }).collect()
            }
        };
        return Ok(edges);
    }

    // Tree-sitter fallback
    let refs = client.references(sym_path, sym_line, sym_col, language_id).await?;
    let mut out = Vec::new();
    for loc in refs {
        let path = match lsp_uri_to_path(&loc.uri) { Some(p) => p, None => continue };
        let src = std::fs::read_to_string(&path).unwrap_or_default();
        let tree = parse_tree(&src, language_id)?; // helper using the same lang map
        let byte = position_to_byte(&src, loc.range.start.line, loc.range.start.character);
        if !crate::tools::symbol::call_edges::ts_classifier::position_is_call(&tree, byte, language_id) {
            continue;
        }
        // Determine which symbol the ref site is INSIDE — its enclosing function.
        // For Direction::Callers: caller_sym = enclosing fn at the ref site; callee_sym = sym_name.
        // For Direction::Callees: caller_sym = sym_name; callee_sym = identifier at the call site.
        let enclosing = enclosing_function_name(&tree, byte, language_id).unwrap_or("?".into());
        let edge = match direction {
            Direction::Callers => Edge {
                caller_sym: enclosing,
                callee_sym: sym_name.to_string(),
                file: path,
                line: loc.range.start.line,
                col: loc.range.start.character,
                source: EdgeSource::Ts,
            },
            Direction::Callees => Edge {
                caller_sym: sym_name.to_string(),
                callee_sym: enclosing, // the identifier-at-call-site for callees needs the actual call target
                file: path,
                line: loc.range.start.line,
                col: loc.range.start.character,
                source: EdgeSource::Ts,
            },
        };
        out.push(edge);
    }
    Ok(out)
}

// Helpers `parse_tree`, `position_to_byte`, `enclosing_function_name`,
// `lsp_uri_to_path` go alongside in this file. `enclosing_function_name`
// walks up the tree to the first `function_item`/`function_definition`/
// `function_declaration`/etc node and extracts its name child — same
// per-language map style as ts_classifier.
```

> **Note for the implementer:** the Direction::Callees ts-fallback is genuinely tricky because `LspClientOps::references(sym, …)` finds refs TO sym, not refs FROM sym. The fallback for Callees needs a different approach: parse `sym`'s definition body, find call expressions inside, resolve each via `goto_definition` to get the callee symbol's location. If this gets gnarly, gate Callees behind LSP only and surface a `RecoverableError` for ts-only languages — document it in the tool description and the experimental docs. Decide based on what works in practice; flag in `docs/TODO-tool-misbehaviors.md` if you defer.

- [ ] **Step 6: Run tests**

Run: `cargo test -p codescout resolve_one_hop`
Expected: PASS for both LSP-success and ts-fallback.

- [ ] **Step 7: `cargo fmt && cargo clippy -- -D warnings && cargo test`**

Expected: all pass.

- [ ] **Step 8: Commit**

```
feat(call_edges): one-hop edge resolver (LSP + ts fallback)
```

---

## Task 7: Sqlite edge cache

**Files:**
- Create: `src/tools/symbol/call_edges/cache.rs`
- Modify: `src/embed/index.rs` (or wherever the project-DB schema migrations live — confirm by searching for `CREATE TABLE` in `src/`)

- [ ] **Step 1: Add the schema migration**

Locate the existing migration block. Add:

```sql
CREATE TABLE IF NOT EXISTS call_edges (
    project_id   TEXT NOT NULL,
    caller_sym   TEXT NOT NULL,
    callee_sym   TEXT NOT NULL,
    file         TEXT NOT NULL,
    line         INTEGER NOT NULL,
    col          INTEGER NOT NULL,
    source       TEXT NOT NULL,
    computed_at  INTEGER NOT NULL,
    PRIMARY KEY (project_id, caller_sym, callee_sym, file, line, col)
);
CREATE INDEX IF NOT EXISTS call_edges_caller ON call_edges(project_id, caller_sym);
CREATE INDEX IF NOT EXISTS call_edges_callee ON call_edges(project_id, callee_sym);
CREATE INDEX IF NOT EXISTS call_edges_file   ON call_edges(project_id, file);
```

- [ ] **Step 2: Cache API**

`src/tools/symbol/call_edges/cache.rs`:

```rust
use rusqlite::{params, Connection};
use super::resolver::{Edge, EdgeSource};

pub struct EdgeCache<'a> { conn: &'a Connection, project_id: &'a str }

impl<'a> EdgeCache<'a> {
    pub fn new(conn: &'a Connection, project_id: &'a str) -> Self { Self { conn, project_id } }

    pub fn lookup_callers(&self, callee_sym: &str) -> rusqlite::Result<Vec<Edge>> { /* … */ }
    pub fn lookup_callees(&self, caller_sym: &str) -> rusqlite::Result<Vec<Edge>> { /* … */ }
    pub fn upsert(&self, edges: &[Edge]) -> rusqlite::Result<()> { /* INSERT OR REPLACE */ }
    pub fn invalidate_file(&self, file: &std::path::Path) -> rusqlite::Result<usize> {
        self.conn.execute(
            "DELETE FROM call_edges WHERE project_id = ?1 AND file = ?2",
            params![self.project_id, file.to_string_lossy()],
        )
    }
}
```

- [ ] **Step 3: Tests — round-trip and invalidation**

```rust
#[test]
fn upsert_then_lookup_round_trip() {
    let conn = Connection::open_in_memory().unwrap();
    apply_call_edges_schema(&conn);
    let cache = EdgeCache::new(&conn, "test");
    let edge = Edge { caller_sym: "b".into(), callee_sym: "a".into(),
        file: "a.rs".into(), line: 3, col: 0, source: EdgeSource::Lsp };
    cache.upsert(&[edge.clone()]).unwrap();
    let got = cache.lookup_callers("a").unwrap();
    assert_eq!(got, vec![edge]);
}

#[test]
fn invalidate_file_removes_only_that_files_edges() {
    let conn = Connection::open_in_memory().unwrap();
    apply_call_edges_schema(&conn);
    let cache = EdgeCache::new(&conn, "test");
    let e1 = /* file=a.rs */;
    let e2 = /* file=b.rs */;
    cache.upsert(&[e1.clone(), e2.clone()]).unwrap();
    let removed = cache.invalidate_file(Path::new("a.rs")).unwrap();
    assert_eq!(removed, 1);
    assert!(cache.lookup_callers("a").unwrap().contains(&e2));
    assert!(!cache.lookup_callers("a").unwrap().contains(&e1));
}
```

- [ ] **Step 4: Run, verify**

Run: `cargo test -p codescout edge_cache`
Expected: PASS.

- [ ] **Step 5: `cargo fmt && cargo clippy -- -D warnings && cargo test`**

Expected: all pass.

- [ ] **Step 6: Commit**

```
feat(call_edges): sqlite-backed edge cache with per-file invalidation
```

---

## Task 8: Wire `did_change` to invalidate the cache

**Files:**
- Modify: `src/lsp/mod.rs` or `src/agent.rs` — wherever `notify_file_changed` is dispatched.

- [ ] **Step 1: Find the existing `notify_file_changed` callsite**

Use `mcp__codescout__search_pattern("notify_file_changed", path="src")`. Identify the function that receives a changed-file event and broadcasts.

- [ ] **Step 2: Write the three-query sandwich test**

Per `CLAUDE.md § Testing Patterns`. In a new `tests/cache_invalidation.rs` or a module of `cache.rs`:

```rust
#[tokio::test]
async fn did_change_invalidates_call_edges_for_changed_file() {
    // 1. Seed: cache contains edges from a.rs (caller "b" -> callee "a").
    // 2. Mutate a.rs on disk WITHOUT calling did_change — change "b()" -> "x()".
    // 3. Query call_graph -> result is STALE (still shows caller "b").
    // 4. Trigger did_change for a.rs.
    // 5. Query call_graph -> result is FRESH (caller is now "x" or absent).
    // The stale-step is the regression assertion.
}
```

- [ ] **Step 3: Run — verify it fails (cache not invalidated yet)**

Expected: step 5 FAILS — cache is stale even after did_change.

- [ ] **Step 4: Add invalidation call**

Inside the `notify_file_changed` dispatcher, after the existing LSP notification:

```rust
if let Ok(conn) = ctx.agent.project_db_conn() { // or however the conn is reached
    if let Ok(project_id) = ctx.agent.project_id() {
        let cache = crate::tools::symbol::call_edges::cache::EdgeCache::new(&conn, &project_id);
        let _ = cache.invalidate_file(path);
    }
}
```

- [ ] **Step 5: Run — verify the test passes**

Expected: PASS — step 5's fresh-assertion succeeds.

- [ ] **Step 6: `cargo fmt && cargo clippy -- -D warnings && cargo test`**

Expected: all pass.

- [ ] **Step 7: Commit**

```
feat(call_edges): invalidate cache on did_change
```

---

## Task 9: BFS traversal engine

**Files:**
- Create: `src/tools/symbol/call_graph/traversal.rs`
- Modify: `src/tools/symbol/call_graph.rs` — declare `mod traversal;` (rename file → directory: `call_graph/mod.rs` housing the existing tool impl, plus `traversal.rs` next to it).

- [ ] **Step 1: Restructure `call_graph.rs` into a directory**

Move existing `src/tools/symbol/call_graph.rs` to `src/tools/symbol/call_graph/mod.rs`. Add `mod traversal;` at the top.

- [ ] **Step 2: Define types and BFS interface**

`src/tools/symbol/call_graph/traversal.rs`:

```rust
use crate::tools::symbol::call_edges::resolver::{Direction, Edge};
use std::collections::{HashSet, VecDeque};

pub struct TraversalConfig { pub max_depth: u32, pub max_edges: usize }

pub struct TraversalResult {
    pub edges: Vec<EdgeWithDepth>,
    pub truncated: bool,
    pub truncated_at_depth: Option<u32>,
    pub max_depth_reached: u32,
}

pub struct EdgeWithDepth { pub edge: Edge, pub depth: u32, pub paths: u32 }

pub trait OneHopResolver {
    /// Resolve one hop of edges for `symbol` in `direction`.
    /// Implementations: cache-checking wrapper around `resolve_one_hop`.
    async fn one_hop(&self, symbol: &str, direction: Direction)
        -> anyhow::Result<Vec<Edge>>;
}

pub async fn bfs<R: OneHopResolver>(
    resolver: &R,
    seed_symbol: &str,
    direction: Direction,
    cfg: TraversalConfig,
) -> anyhow::Result<TraversalResult>;
```

- [ ] **Step 3: Tests with a fake resolver**

```rust
struct FakeResolver { graph: HashMap<(String, Direction), Vec<Edge>> }
#[async_trait]
impl OneHopResolver for FakeResolver { /* lookup in graph */ }

#[tokio::test]
async fn bfs_reaches_max_depth_then_stops() { /* a -> b -> c -> d, max_depth=2 */ }

#[tokio::test]
async fn bfs_dedupes_repeated_pair_with_paths_count() {
    /* edges b->a (file1) and b->a (file2 reaching via different intermediate)
       → single EdgeWithDepth with paths=2 */
}

#[tokio::test]
async fn bfs_handles_cycle_without_infinite_loop() {
    /* a -> b -> a; depth-2 traversal terminates */
}

#[tokio::test]
async fn bfs_depth_coherent_cap_finishes_current_level() {
    /* graph that produces 50 depth-1 edges, 200 depth-2 edges; cap=100;
       result has all 50 depth-1 edges + truncated=true at_depth=2, NOT a partial depth-2 sliver */
}
```

- [ ] **Step 4: Implement `bfs`**

```rust
pub async fn bfs<R: OneHopResolver>(
    resolver: &R, seed_symbol: &str, direction: Direction, cfg: TraversalConfig,
) -> anyhow::Result<TraversalResult> {
    let mut visited = HashSet::new();
    let mut current_level: VecDeque<String> = VecDeque::new();
    current_level.push_back(seed_symbol.to_string());
    visited.insert(seed_symbol.to_string());

    let mut all_edges: Vec<EdgeWithDepth> = Vec::new();
    let mut max_depth_reached = 0;
    let mut truncated = false;
    let mut truncated_at_depth = None;

    for depth in 1..=cfg.max_depth {
        let mut next_level: VecDeque<String> = VecDeque::new();
        let mut level_edges: Vec<EdgeWithDepth> = Vec::new();

        while let Some(sym) = current_level.pop_front() {
            let hops = resolver.one_hop(&sym, direction.clone()).await?;
            for edge in hops {
                let other = match direction {
                    Direction::Callers => edge.caller_sym.clone(),
                    Direction::Callees => edge.callee_sym.clone(),
                };
                if visited.insert(other.clone()) { next_level.push_back(other); }
                level_edges.push(EdgeWithDepth { edge, depth, paths: 1 });
            }
        }

        // Dedupe (caller, callee, direction) within this level, summing paths
        level_edges = dedupe_with_paths(level_edges);

        // Depth-coherent cap: if adding this level would exceed cfg.max_edges,
        // accept it only if depth==1 (first level always returns); otherwise truncate before adding.
        if !all_edges.is_empty() && all_edges.len() + level_edges.len() > cfg.max_edges {
            truncated = true;
            truncated_at_depth = Some(depth);
            break;
        }
        all_edges.extend(level_edges);
        max_depth_reached = depth;
        current_level = next_level;
        if current_level.is_empty() { break; }
    }

    Ok(TraversalResult { edges: all_edges, truncated, truncated_at_depth, max_depth_reached })
}

fn dedupe_with_paths(edges: Vec<EdgeWithDepth>) -> Vec<EdgeWithDepth> { /* … */ }
```

- [ ] **Step 5: Run tests, verify all pass**

Run: `cargo test -p codescout traversal::`
Expected: PASS for all four scenarios.

- [ ] **Step 6: `cargo fmt && cargo clippy -- -D warnings && cargo test`**

Expected: all pass.

- [ ] **Step 7: Commit**

```
feat(call_graph): BFS traversal engine with depth-coherent cap
```

---

## Task 10: Wire the tool — output formatter + cache-checking resolver

**Files:**
- Modify: `src/tools/symbol/call_graph/mod.rs`

This is where the stub goes away.

- [ ] **Step 1: Build the cache-checking resolver**

Inside `call_graph/mod.rs` (or a small `cached_resolver.rs` next to it):

```rust
struct CachedResolver<'a> {
    cache: &'a EdgeCache<'a>,
    client: &'a dyn LspClientOps,
    seed_path: &'a Path,
    seed_pos_index: HashMap<String, (PathBuf, u32, u32, String)>, // sym -> location/lang
}

#[async_trait]
impl<'a> OneHopResolver for CachedResolver<'a> {
    async fn one_hop(&self, symbol: &str, direction: Direction) -> anyhow::Result<Vec<Edge>> {
        // 1. Cache hit?
        let hit = match direction {
            Direction::Callers => self.cache.lookup_callers(symbol)?,
            Direction::Callees => self.cache.lookup_callees(symbol)?,
        };
        if !hit.is_empty() { return Ok(hit); }

        // 2. Miss: resolve via LSP/ts, upsert, return.
        let (path, line, col, lang) = self.lookup_pos(symbol)?;
        let edges = resolve_one_hop(self.client, symbol, &path, line, col, &lang, direction).await?;
        self.cache.upsert(&edges)?;
        Ok(edges)
    }
}
```

`seed_pos_index` is built lazily — when a symbol's position isn't known, fall back to `workspace_symbols(symbol)` to discover it.

- [ ] **Step 2: Replace `call()` body**

Pseudocode:

```rust
async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
    let symbol = require_str_param(&input, "symbol")?;
    let direction_str = input["direction"].as_str().unwrap_or("callers");
    let max_depth = input["max_depth"].as_u64().unwrap_or(3) as u32;
    let detail_full = OutputGuard::from_input(&input).should_include_body(); // proxy for full mode

    // Resolve seed symbol -> (path, line, col, lang)
    let (path, line, col, lang) = resolve_seed(ctx, symbol).await?;
    let (client, _) = get_lsp_client(ctx, &path).await?;

    let conn = ctx.agent.project_db_conn().await?;
    let project_id = ctx.agent.project_id().await?;
    let cache = EdgeCache::new(&conn, &project_id);

    let cap = if detail_full { 500 } else { 200 };
    let cfg = TraversalConfig { max_depth, max_edges: cap };

    let directions = match direction_str {
        "callers" => vec![Direction::Callers],
        "callees" => vec![Direction::Callees],
        "both"    => vec![Direction::Callers, Direction::Callees],
        other     => return Err(RecoverableError::new(format!("invalid direction '{}'", other)).into()),
    };

    let mut by_dir: HashMap<&str, TraversalResult> = HashMap::new();
    for d in &directions {
        let resolver = CachedResolver { cache: &cache, client: client.as_ref(),
            seed_path: &path, seed_pos_index: HashMap::new() };
        let res = bfs(&resolver, symbol, d.clone(), cfg.clone()).await?;
        by_dir.insert(if matches!(d, Direction::Callers) { "callers" } else { "callees" }, res);
    }

    let total_edges: usize = by_dir.values().map(|r| r.edges.len()).sum();
    let auto_promote = total_edges <= 30;
    let render_full = detail_full || auto_promote;

    Ok(format_output(symbol, &by_dir, render_full, auto_promote))
}
```

- [ ] **Step 3: `format_output`**

```rust
fn format_output(symbol: &str, by_dir: &HashMap<&str, TraversalResult>,
                 render_full: bool, auto_promote: bool) -> Value {
    let mut out = json!({ "symbol": symbol });

    for (key, res) in by_dir {
        if render_full {
            let edges_json: Vec<_> = res.edges.iter().map(|e| json!({
                "caller": e.edge.caller_sym, "callee": e.edge.callee_sym,
                "file":   e.edge.file.to_string_lossy(),
                "line":   e.edge.line + 1, "depth": e.depth,
                "source": match e.edge.source { EdgeSource::Lsp => "lsp", EdgeSource::Ts => "ts" },
                "paths":  e.paths,
            })).collect();
            out[key] = json!(edges_json);
        } else {
            let mut by_file: BTreeMap<String, usize> = BTreeMap::new();
            let mut by_depth: BTreeMap<u32, usize> = BTreeMap::new();
            for e in &res.edges {
                *by_file.entry(e.edge.file.to_string_lossy().into()).or_default() += 1;
                *by_depth.entry(e.depth).or_default() += 1;
            }
            out[key] = json!({
                "count": res.edges.len(), "by_file": by_file, "by_depth": by_depth,
            });
        }
        if res.truncated {
            out[format!("{}_truncated_at_depth", key)] = json!(res.truncated_at_depth);
        }
    }
    if auto_promote { out["auto_promoted"] = json!(true); }
    let max_d = by_dir.values().map(|r| r.max_depth_reached).max().unwrap_or(0);
    out["max_depth_reached"] = json!(max_d);
    out
}
```

- [ ] **Step 4: `format_compact`**

```rust
fn format_compact(&self, result: &Value) -> Option<String> {
    let sym = result.get("symbol")?.as_str()?;
    let mut parts = vec![format!("call_graph for `{}`", sym)];
    for key in &["callers", "callees"] {
        if let Some(v) = result.get(key) {
            if let Some(count) = v.get("count").and_then(|c| c.as_u64()) {
                let n_files = v.get("by_file").and_then(|f| f.as_object()).map(|m| m.len()).unwrap_or(0);
                parts.push(format!("{}: {} across {} files", key, count, n_files));
            } else if let Some(arr) = v.as_array() {
                parts.push(format!("{}: {}", key, arr.len()));
            }
        }
    }
    Some(parts.join("; "))
}
```

- [ ] **Step 5: Drop the stub test, write end-to-end tool tests**

Replace `call_graph_stub_returns_recoverable_error` with:

```rust
#[tokio::test]
async fn call_graph_callers_uses_seeded_lsp_data() {
    let ctx = minimal_ctx();
    /* seed mock with prepare + incoming_calls for symbol "a" */
    let result = CallGraph.call(json!({ "symbol": "a", "direction": "callers" }), &ctx).await.unwrap();
    assert!(result.get("callers").is_some());
    assert_eq!(result["max_depth_reached"], json!(1));
}

#[tokio::test]
async fn call_graph_auto_promotes_small_results() {
    /* seed exactly 5 callers; assert response has full edge list and auto_promoted=true */
}

#[tokio::test]
async fn call_graph_compact_summary_for_large_results() {
    /* seed 250 edges; default detail returns count + by_file + by_depth, no edge list */
}

#[tokio::test]
async fn call_graph_both_direction_returns_both_keys() { /* ... */ }
```

- [ ] **Step 6: Run all the new tests, then the full suite**

Run: `cargo test -p codescout call_graph`
Then: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass.

- [ ] **Step 7: Update tool description**

In `call_graph::description()`, drop the "NOT YET IMPLEMENTED" sentence. Replace with a working description that mentions `source: "lsp"` vs `"ts"` and the `direction` semantics.

```rust
fn description(&self) -> &str {
    "Transitive call graph for a symbol. `direction=callers` for blast radius, \
     `callees` for outbound flow, `both` for both. `max_depth` (default 3) bounds traversal. \
     Each edge is tagged `source: \"lsp\"` (semantic, authoritative) or \"ts\" \
     (tree-sitter classifier — best-effort, may include false positives from \
     macros or shadowing). Use `references` for ALL refs (not call-filtered)."
}
```

- [ ] **Step 8: Commit**

```
feat(call_graph): wire traversal + cache + output formatter
```

---

## Task 11: Live-LSP smoke tests per language

**Files:**
- Create: `tests/fixtures/call_graph/{rust,python,typescript,kotlin,java}/` — one tiny project per language with a known a→b→c call chain.
- Create: `tests/call_graph_live.rs`

These tests run only with the `live-lsp` feature flag (or whatever the existing convention is — confirm by inspecting `tests/`). They start a real LSP, exercise `call_graph(symbol="a")`, and assert the expected callers/callees.

- [ ] **Step 1: One fixture per language**

Each fixture is the smallest project the language's LSP will accept. Three functions: `a` calls `b`, `b` calls `c`. Configure as needed (e.g., `Cargo.toml` for Rust, `package.json` for TS, `pom.xml` or `build.gradle` for Java/Kotlin).

- [ ] **Step 2: One test per language**

```rust
#[cfg(feature = "live-lsp")]
#[tokio::test]
async fn call_graph_rust_real_lsp() {
    let ctx = ctx_for_fixture("tests/fixtures/call_graph/rust");
    let result = CallGraph.call(json!({"symbol": "a", "direction": "callees", "max_depth": 2}), &ctx).await.unwrap();
    let edges = result["callees"].as_array().unwrap();
    let names: Vec<_> = edges.iter().map(|e| e["callee"].as_str().unwrap()).collect();
    assert!(names.contains(&"b"));
    assert!(names.contains(&"c"));
    assert!(edges.iter().all(|e| e["source"] == "lsp")); // rust-analyzer supports callHierarchy
}
```

Repeat for python (pyright), typescript (ts-server), java (jdtls). For kotlin, **expect ts-fallback** — assert `source == "ts"` for at least some edges, and document the finding in `docs/issues/` if behavior differs.

- [ ] **Step 3: Run with the feature**

Run: `cargo test --features live-lsp -p codescout call_graph_live`
Expected: all five PASS (or kotlin shows ts-source as expected).

- [ ] **Step 4: Commit**

```
test(call_graph): live-LSP smoke tests for 5 languages
```

---

## Task 12: Prompt surface updates + ONBOARDING_VERSION bump

**Files:**
- Modify: `src/prompts/server_instructions.md`
- Modify: `src/prompts/onboarding_prompt.md`
- Modify: `src/prompts/builders.rs` (function `build_system_prompt_draft`)
- Modify: `src/tools/onboarding.rs` — bump `ONBOARDING_VERSION`

Per `CLAUDE.md § Prompt Surface Consistency` and `src/prompts/README.md`. The test `prompt_surfaces_reference_only_real_tools` will catch any stale references at build time.

- [ ] **Step 1: server_instructions.md — add `call_graph` row**

Find the navigation tools section. Add a row:

> `call_graph(symbol, direction, max_depth)` — transitive callers (blast radius) or callees (flow). Use `direction="callers"` to size the impact of a change. Until `references(kind="call")` ships, `call_graph(depth=1, direction="callers")` is the way to filter refs to call sites only.

Respect the writing rules in `src/prompts/README.md` (rule caps, repetition budget). Read that file before editing.

- [ ] **Step 2: onboarding_prompt.md — add to navigation-by-knowledge-level section**

Where it lists "Know the name → LSP/AST tools (`symbols`, `symbol_at`, `references`)", append `call_graph`.

- [ ] **Step 3: builders.rs — same**

Mirror the change in `build_system_prompt_draft()`.

- [ ] **Step 4: Bump `ONBOARDING_VERSION`**

In `src/tools/onboarding.rs`, increment by one. Per CLAUDE.md guidance, this counts as "tool name added" — qualifying for a bump.

- [ ] **Step 5: Run prompt-surface test**

Run: `cargo test -p codescout prompt_surfaces_reference_only_real_tools`
Expected: PASS — `call_graph` is a real tool, no other surfaces drift.

- [ ] **Step 6: Run full suite**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all pass.

- [ ] **Step 7: Commit**

```
docs(prompts): announce call_graph + bump onboarding version
```

---

## Task 13: Experimental docs page

**Files:**
- Create: `docs/manual/src/experimental/call-graph.md`
- Modify: `docs/manual/src/experimental/index.md`

Per `CLAUDE.md § Documenting Features on experiments`.

- [ ] **Step 1: Write the docs page**

```markdown
# `call_graph`

> ⚠ Experimental — may change without notice.

Transitive call graph for a symbol. Two directions:

- `callers` (default): who calls this symbol, transitively. Use for blast-radius
  before refactoring.
- `callees`: what does this symbol call, transitively. Use to trace flow.
- `both`: both, returned in separate keys.

## Schema

```json
{ "symbol": "Agent::new", "direction": "callers", "max_depth": 3 }
```

## Output

By default returns a compact summary: counts + `by_file` + `by_depth`. When the
total result has ≤ 30 edges, auto-promotes to full edge list. Use
`detail_level: "full"` to force full output on large graphs (paginate with
`offset` / `limit`).

Each edge is tagged `source: "lsp"` (from `callHierarchy`, semantically
authoritative) or `"ts"` (from the tree-sitter classifier fallback —
best-effort, may include false positives on macros, shadowed names, or dynamic
dispatch).

## Caching

Edges are cached in the project sqlite DB (`call_edges` table). Caches are
invalidated per file on `did_change` notifications, so the cache stays correct
across multi-session edits.

## Known limitations

- Kotlin LSP coverage of `callHierarchy` is partial; expect `source: "ts"` edges
  for most Kotlin queries.
- Cross-project edges are not supported in v1.
- `Direction::Callees` via tree-sitter fallback is best-effort — described in
  the implementation; check `docs/TODO-tool-misbehaviors.md` for current status.
```

- [ ] **Step 2: Add to index**

Edit `docs/manual/src/experimental/index.md` — add a link to the new page.

- [ ] **Step 3: Commit**

```
docs(experimental): call_graph page
```

---

## Final Verification

- [ ] **Run the full pipeline**

```
cargo fmt
cargo clippy -- -D warnings
cargo test
cargo build --release
```

Expected: all pass; release binary produced.

- [ ] **Manual smoke via live MCP**

Restart the MCP server (`/mcp` reconnect after `cargo build --release`). Then in a session:

1. `call_graph(symbol="some_real_function_in_codescout", direction="callers")` — confirm summary shape.
2. Same with `detail_level="full"` — confirm full edge list.
3. `call_graph(symbol="...", direction="both", max_depth=2)` — confirm both keys.
4. `call_graph(symbol="nonexistent_symbol")` — confirm `RecoverableError` with helpful hint.

- [ ] **Update `docs/TODO-tool-misbehaviors.md`**

If anything surprised during implementation (callHierarchy on Kotlin returns garbage; pyright doesn't support it; macro expansion drops edges), log a one-liner per `CLAUDE.md`'s mandate.

- [ ] **Decide on shipping to master**

Per `CLAUDE.md § Standard Ship Sequence`. Cherry-pick the feature commits to master with a graduation commit that moves the doc out of `experimental/`. OR keep on `experiments` longer to bake.

---

## Self-Review Notes

- Spec coverage check: every section of the design doc maps to at least one task. §3.1 components → Tasks 1–10. §3.2 resolver → Task 6. §3.3 classifier → Task 5. §3.4 cache → Task 7. §3.5 traversal → Task 9. §3.6 output → Task 10. §3.7 LSP additions → Tasks 1–4. §6 testing → Tasks 1–11 (each task has tests). §7 prompt surfaces → Task 12. §9 sequencing → Tasks 1–13.
- Type names checked: `Edge`, `EdgeSource`, `Direction`, `EdgeWithDepth`, `TraversalConfig`, `TraversalResult`, `OneHopResolver`, `EdgeCache`, `CachedResolver` are consistent across tasks.
- Placeholder check: where the implementer must make a judgment (e.g. `seed_pos_index` lookup logic, `enclosing_function_name` per-language tree-walking), the plan says so explicitly and points to existing files for the pattern. No silent TBDs.
- One known gap noted inline: the ts-fallback for `Direction::Callees` is genuinely harder than for Callers and the plan flags this in Task 6 with explicit guidance to either implement `goto_definition` chasing or surface a `RecoverableError` for ts-only languages — and to log the choice in `docs/TODO-tool-misbehaviors.md`. This is a real design loose end the implementer must resolve, surfaced explicitly rather than hidden.
