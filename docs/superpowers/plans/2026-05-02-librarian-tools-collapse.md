# Librarian Tools Collapse (16 → 5) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Collapse 16 librarian MCP tools into 5 action-dispatched tools to reduce token pressure and conceptual sprawl.

**Architecture:** Each new tool is a thin dispatcher with an `action` enum param. Existing handler files are stripped of their `impl Tool` blocks and expose a `pub(super) async fn call(ctx, args) -> Result<Value>` free function. The dispatcher just routes on `action`. Serde ignores the extra `action` field in each handler's `from_value(args)` call (no `deny_unknown_fields` on any `Args` struct).

**Tech Stack:** Rust, async_trait, serde_json, anyhow. Crate: `crates/librarian-mcp`.

---

## File Map

**New files (5 tools):**
- `crates/librarian-mcp/src/tools/artifact.rs` — 7-action dispatcher (find/get/create/update/link/graph/state_at)
- `crates/librarian-mcp/src/tools/artifact_event.rs` — 2-action dispatcher (create/list)
- `crates/librarian-mcp/src/tools/artifact_refresh.rs` — 2-action dispatcher (gather/list_stale)
- `crates/librarian-mcp/src/tools/librarian.rs` — 4-action dispatcher (context/reindex/tracker_design/workspace_state_at)

**Stripped (15 handler files — remove `impl Tool`, add `pub(super) async fn call`):**
- `find.rs`, `get.rs`, `create.rs`, `update.rs`, `link.rs`, `graph.rs`, `state_at.rs` → feed `artifact`
- `event_create.rs`, `timeline.rs` → feed `artifact_event`
- `refresh.rs`, `refresh_stale.rs` → feed `artifact_refresh`
- `context.rs`, `reindex.rs`, `tracker_design.rs`, `workspace_state_at.rs` → feed `librarian`

**Unchanged:** `augment.rs` (already named `artifact_augment`, keep as-is)

**Modified:**
- `crates/librarian-mcp/src/tools/mod.rs` — `all_tools()` returns 5, module visibility updated
- `crates/librarian-mcp/src/server.rs` — update `serde_error_gets_helpful_hint` test
- `crates/librarian-mcp/src/prompts/server_instructions.md` — update tool-name table
- `crates/librarian-mcp/src/prompts/companion_hint.md` — update tool-name table

---

## Task 1: Extract handlers feeding `artifact` (7 files)

**Files:**
- Modify: `crates/librarian-mcp/src/tools/find.rs`
- Modify: `crates/librarian-mcp/src/tools/get.rs`
- Modify: `crates/librarian-mcp/src/tools/create.rs`
- Modify: `crates/librarian-mcp/src/tools/update.rs`
- Modify: `crates/librarian-mcp/src/tools/link.rs`
- Modify: `crates/librarian-mcp/src/tools/graph.rs`
- Modify: `crates/librarian-mcp/src/tools/state_at.rs`

The mechanical pattern for each file is:
1. Remove `pub struct ArtifactXxx;`
2. Remove the entire `#[async_trait] impl Tool for ArtifactXxx { ... }` block
3. Add `pub(super) async fn call(ctx: &ToolContext, args: Value) -> Result<Value>` with the body that was inside `impl Tool ... / call`
4. In `#[cfg(test)] mod tests`, replace `ArtifactXxx.call(` with `call(` (remove the struct receiver)

- [ ] **Step 1: Transform find.rs**

Remove the struct and impl block. Replace with a free function. Before the `#[cfg(test)]` block, add:

```rust
pub(super) async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args)?;
    // ... (paste the existing call body verbatim) ...
}
```

In the tests module, change every occurrence of:
```rust
ArtifactFind
    .call(&ctx, json!({...}))
    .await
```
to:
```rust
call(&ctx, json!({...}))
    .await
```

- [ ] **Step 2: Transform get.rs** (same pattern as find.rs — `ArtifactGet` → `call`)

- [ ] **Step 3: Transform create.rs** (same pattern — `ArtifactCreate` → `call`)

- [ ] **Step 4: Transform update.rs** (same pattern — `ArtifactUpdate` → `call`)

- [ ] **Step 5: Transform link.rs** (same pattern — `ArtifactLink` → `call`)

- [ ] **Step 6: Transform graph.rs** (same pattern — `ArtifactGraph` → `call`)

- [ ] **Step 7: Transform state_at.rs** (same pattern — `ArtifactStateAt` → `call`)

- [ ] **Step 8: Verify compilation**

```bash
cd crates/librarian-mcp && cargo check 2>&1 | head -40
```
Expected: errors about unused imports or missing module references — fine, will fix in Task 3. The handler files themselves should compile cleanly.

---

## Task 2: Extract handlers feeding `artifact_event` and `artifact_refresh`

**Files:**
- Modify: `crates/librarian-mcp/src/tools/event_create.rs`
- Modify: `crates/librarian-mcp/src/tools/timeline.rs`
- Modify: `crates/librarian-mcp/src/tools/refresh.rs`
- Modify: `crates/librarian-mcp/src/tools/refresh_stale.rs`

- [ ] **Step 1: Transform event_create.rs** — remove `ArtifactEventCreate` struct and `impl Tool`, add `pub(super) async fn call`. Update tests from `ArtifactEventCreate.call(` to `call(`.

- [ ] **Step 2: Transform timeline.rs** — remove `ArtifactTimeline` struct and `impl Tool`, add `pub(super) async fn call`. Update tests from `ArtifactTimeline.call(` to `call(`.

- [ ] **Step 3: Transform refresh.rs** — remove `ArtifactRefresh` struct and `impl Tool`, add `pub(super) async fn call`. Update tests from `ArtifactRefresh.call(` to `call(`.

- [ ] **Step 4: Transform refresh_stale.rs** — remove `ArtifactRefreshStale` struct and `impl Tool`, add `pub(super) async fn call`. Update tests from `ArtifactRefreshStale.call(` to `call(`.

---

## Task 3: Extract handlers feeding `librarian`

**Files:**
- Modify: `crates/librarian-mcp/src/tools/context.rs`
- Modify: `crates/librarian-mcp/src/tools/reindex.rs`
- Modify: `crates/librarian-mcp/src/tools/tracker_design.rs`
- Modify: `crates/librarian-mcp/src/tools/workspace_state_at.rs`

- [ ] **Step 1: Transform context.rs** — remove `LibrarianContext` struct and `impl Tool`, add `pub(super) async fn call`. Update tests.

- [ ] **Step 2: Transform reindex.rs** — remove `LibrarianReindex` struct and `impl Tool`, add `pub(super) async fn call`. Update tests.

- [ ] **Step 3: Transform tracker_design.rs** — remove `TrackerDesign` struct and `impl Tool`, add `pub(super) async fn call`. Update tests.

- [ ] **Step 4: Transform workspace_state_at.rs** — remove `WorkspaceStateAt` struct and `impl Tool`, add `pub(super) async fn call`. Update tests.

---

## Task 4: Create `artifact.rs`

**Files:**
- Create: `crates/librarian-mcp/src/tools/artifact.rs`

- [ ] **Step 1: Write the failing test**

```rust
// At bottom of artifact.rs, in #[cfg(test)] mod tests:
#[tokio::test]
async fn unknown_action_returns_recoverable_error() {
    use crate::catalog::Catalog;
    use crate::workspace::WorkspaceConfig;
    use std::sync::Arc;
    let ctx = ToolContext {
        catalog: Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap())),
        workspace: Arc::new(WorkspaceConfig { roots: vec![], ignore: vec![], rules: vec![], umbrellas: vec![] }),
        rules: Arc::new(vec![]),
        embedding: None,
        current_project: None,
    };
    let err = Artifact.call(&ctx, serde_json::json!({"action": "bogus"})).await.unwrap_err();
    let is_recoverable = err.downcast_ref::<super::RecoverableError>().is_some();
    assert!(is_recoverable, "expected RecoverableError, got: {err}");
}

#[tokio::test]
async fn missing_action_returns_recoverable_error() {
    use crate::catalog::Catalog;
    use crate::workspace::WorkspaceConfig;
    use std::sync::Arc;
    let ctx = ToolContext {
        catalog: Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap())),
        workspace: Arc::new(WorkspaceConfig { roots: vec![], ignore: vec![], rules: vec![], umbrellas: vec![] }),
        rules: Arc::new(vec![]),
        embedding: None,
        current_project: None,
    };
    let err = Artifact.call(&ctx, serde_json::json!({})).await.unwrap_err();
    let is_recoverable = err.downcast_ref::<super::RecoverableError>().is_some();
    assert!(is_recoverable, "expected RecoverableError, got: {err}");
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd crates/librarian-mcp && cargo test artifact::tests 2>&1 | tail -10
```
Expected: FAIL — `Artifact` not defined yet.

- [ ] **Step 3: Write artifact.rs**

```rust
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use super::{RecoverableError, Tool, ToolContext};

pub struct Artifact;

#[async_trait]
impl Tool for Artifact {
    fn name(&self) -> &'static str {
        "artifact"
    }

    fn description(&self) -> &'static str {
        "Artifact CRUD and query. action: find | get | create | update | link | graph | state_at. \
         Defaults: scope=project (current sub-project only), archived/superseded hidden when \
         filter does not constrain status. Shortcut params kind/status expand to eq-filters \
         and combine with filter via AND."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["action"],
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["find", "get", "create", "update", "link", "graph", "state_at"],
                    "description": "Operation to perform"
                },
                "filter": {
                    "type": "object",
                    "description": "find: filter AST (and/or/not + eq/ne/in/nin/gt/lt/gte/lte/contains/prefix leaves)"
                },
                "kind": {
                    "type": "string",
                    "description": "find: shortcut eq-filter on kind. create: artifact kind (spec/plan/adr/tracker/…)"
                },
                "status": {
                    "type": "string",
                    "description": "find: shortcut eq-filter on status (disables archived-hide). create/update: set status."
                },
                "semantic": {
                    "type": "string",
                    "description": "find: natural-language query for semantic search (requires embedder)"
                },
                "scope": {
                    "type": "string",
                    "enum": ["project", "repo", "umbrella", "all"],
                    "default": "project",
                    "description": "find: scope for listing. Defaults to current sub-project."
                },
                "augmented": {
                    "type": "boolean",
                    "description": "find: filter to augmented (true) or non-augmented (false) artifacts"
                },
                "include_archived": { "type": "boolean", "default": false },
                "limit": { "type": "integer", "default": 50, "maximum": 500 },
                "offset": { "type": "integer", "default": 0, "maximum": 100000 },
                "id": {
                    "type": "string",
                    "description": "get/update/graph: artifact id"
                },
                "include_links": { "type": "boolean", "default": false, "description": "get: include link edges" },
                "links_direction": {
                    "type": "string",
                    "enum": ["out", "in", "both"],
                    "description": "get: filter links by direction (default: both)"
                },
                "links_rel": { "type": "string", "description": "get: filter links to this rel type" },
                "include_observations": { "type": "boolean", "default": false },
                "full": { "type": "boolean", "default": false, "description": "get: include full body" },
                "heading": { "type": "string", "description": "get: fetch one section by heading" },
                "headings": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "get: fetch multiple sections by heading"
                },
                "start_line": { "type": "integer", "description": "get: 1-indexed start of line slice" },
                "end_line": { "type": "integer", "description": "get: 1-indexed inclusive end of line slice" },
                "rel_path": { "type": "string", "description": "create: relative path for new file" },
                "repo": { "type": "string", "description": "create: workspace root name" },
                "title": { "type": "string", "description": "create: artifact title" },
                "body": { "type": "string", "description": "create: markdown body" },
                "owners": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "create/update: owner list"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "create/update: tag list"
                },
                "augment": {
                    "type": "object",
                    "description": "create: attach augmentation atomically. Pass prompt + optional params.",
                    "properties": {
                        "prompt": { "type": "string" },
                        "params": { "type": "object" }
                    },
                    "required": ["prompt"]
                },
                "patch": {
                    "type": "object",
                    "description": "update: fields to change (body, title, status, topic, owners, tags)"
                },
                "addBlocks": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "update: task IDs this artifact blocks"
                },
                "addBlockedBy": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "update: task IDs that block this artifact"
                },
                "owner": { "type": "string", "description": "update: set owner field" },
                "commit_refresh": {
                    "type": "boolean",
                    "description": "update: atomically record a completed refresh cycle"
                },
                "activeForm": { "type": "string", "description": "update: present-continuous label shown in spinner" },
                "src_id": { "type": "string", "description": "link: source artifact id" },
                "dst_id": { "type": "string", "description": "link: destination artifact id" },
                "rel": { "type": "string", "description": "link: relation type (supersedes, implements, …)" },
                "depth": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 3,
                    "description": "graph: BFS depth (1–3)"
                },
                "rels": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "graph: filter edges to these rel types"
                },
                "include_events": {
                    "type": "boolean",
                    "default": false,
                    "description": "graph: also walk event and source nodes via event_edges"
                },
                "artifact_id": { "type": "string", "description": "state_at: artifact id" },
                "commit": { "type": "string", "description": "state_at: git commit hash as time-travel cutoff" },
                "timestamp": {
                    "type": "integer",
                    "format": "int64",
                    "description": "state_at: unix epoch ms as time-travel cutoff"
                }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let action = args["action"].as_str().ok_or_else(|| {
            RecoverableError::new(
                "action required — one of: find, get, create, update, link, graph, state_at",
            )
        })?;
        match action {
            "find"     => super::find::call(ctx, args).await,
            "get"      => super::get::call(ctx, args).await,
            "create"   => super::create::call(ctx, args).await,
            "update"   => super::update::call(ctx, args).await,
            "link"     => super::link::call(ctx, args).await,
            "graph"    => super::graph::call(ctx, args).await,
            "state_at" => super::state_at::call(ctx, args).await,
            other => Err(RecoverableError::new(format!(
                "unknown action '{other}' — expected one of: find, get, create, update, link, graph, state_at"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;
    use crate::workspace::WorkspaceConfig;
    use std::sync::Arc;

    fn mk_ctx() -> ToolContext {
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap())),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: None,
        }
    }

    #[tokio::test]
    async fn unknown_action_returns_recoverable_error() {
        let err = Artifact.call(&mk_ctx(), serde_json::json!({"action": "bogus"})).await.unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some(), "expected RecoverableError, got: {err}");
    }

    #[tokio::test]
    async fn missing_action_returns_recoverable_error() {
        let err = Artifact.call(&mk_ctx(), serde_json::json!({})).await.unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some(), "expected RecoverableError, got: {err}");
    }

    #[tokio::test]
    async fn find_action_routes_correctly() {
        let v = Artifact.call(&mk_ctx(), serde_json::json!({"action": "find"})).await.unwrap();
        assert!(v["count"].is_number());
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cd crates/librarian-mcp && cargo test artifact::tests 2>&1 | tail -10
```
Expected: all 3 tests pass.

---

## Task 5: Create `artifact_event.rs`

**Files:**
- Create: `crates/librarian-mcp/src/tools/artifact_event.rs`

- [ ] **Step 1: Write artifact_event.rs**

```rust
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use super::{RecoverableError, Tool, ToolContext};

pub struct ArtifactEvent;

#[async_trait]
impl Tool for ArtifactEvent {
    fn name(&self) -> &'static str {
        "artifact_event"
    }

    fn description(&self) -> &'static str {
        "Artifact event log. action: create | list. \
         Events are immutable append-only records anchored to git commits — \
         distinct from field patches (use artifact(update) for those)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["action"],
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "list"],
                    "description": "Operation: create appends an event; list returns events newest-first."
                },
                "artifact_id": { "type": "string", "description": "create/list: artifact id" },
                "kind": {
                    "type": "string",
                    "description": "create: event kind (note, reviewed, status_change, field_patch, superseded_by, external_signal, intent, verdict)"
                },
                "payload": { "description": "create: event payload (any JSON)" },
                "anchor_commit": { "type": "string", "description": "create: git commit to anchor event to" },
                "head_commit": { "type": "string", "description": "create: HEAD commit at write time" },
                "parent_event_id": { "type": "string", "description": "create: parent event id for threading" },
                "author": { "type": "string", "description": "create: event author" },
                "also_mutates": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "create: additional artifact ids mutated by this event"
                },
                "resolves_intent_event_id": { "type": "string", "description": "create: intent event id this verdict resolves" },
                "source": {
                    "type": "object",
                    "description": "create: external signal source {uri, kind, payload?}",
                    "properties": {
                        "uri": { "type": "string" },
                        "kind": { "type": "string" },
                        "payload": {}
                    },
                    "required": ["uri", "kind"]
                },
                "kinds": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "list: filter to these event kinds"
                },
                "limit": { "type": "integer", "default": 50, "description": "list: max results" },
                "since": { "type": "integer", "format": "int64", "description": "list: return events after this ms epoch" },
                "until": { "type": "integer", "format": "int64", "description": "list: return events before this ms epoch" }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let action = args["action"].as_str().ok_or_else(|| {
            RecoverableError::new("action required — one of: create, list")
        })?;
        match action {
            "create" => super::event_create::call(ctx, args).await,
            "list"   => super::timeline::call(ctx, args).await,
            other => Err(RecoverableError::new(format!(
                "unknown action '{other}' — expected one of: create, list"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;
    use crate::workspace::WorkspaceConfig;
    use std::sync::Arc;

    fn mk_ctx() -> ToolContext {
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap())),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: None,
        }
    }

    #[tokio::test]
    async fn unknown_action_returns_recoverable_error() {
        let err = ArtifactEvent
            .call(&mk_ctx(), serde_json::json!({"action": "bogus", "artifact_id": "x"}))
            .await
            .unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }

    #[tokio::test]
    async fn list_action_routes_correctly() {
        let v = ArtifactEvent
            .call(&mk_ctx(), serde_json::json!({"action": "list", "artifact_id": "nonexistent"}))
            .await
            .unwrap();
        // timeline returns array even for unknown ids
        assert!(v.is_array() || v["events"].is_array());
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd crates/librarian-mcp && cargo test artifact_event::tests 2>&1 | tail -10
```
Expected: tests pass (or compile error — fix before continuing).

---

## Task 6: Create `artifact_refresh.rs`

**Files:**
- Create: `crates/librarian-mcp/src/tools/artifact_refresh.rs`

- [ ] **Step 1: Write artifact_refresh.rs**

```rust
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use super::{RecoverableError, Tool, ToolContext};

pub struct ArtifactRefreshTool;

#[async_trait]
impl Tool for ArtifactRefreshTool {
    fn name(&self) -> &'static str {
        "artifact_refresh"
    }

    fn description(&self) -> &'static str {
        "Augmentation lifecycle. action: gather | list_stale. \
         gather: collect context for an augmented artifact (does NOT write — synthesize then call \
         artifact(update, commit_refresh=true) to write back). \
         list_stale: list augmented artifacts whose last refresh is older than threshold_hours \
         (default 24h), oldest-first."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["action"],
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["gather", "list_stale"],
                    "description": "gather: collect context for one artifact. list_stale: list stale augmented artifacts."
                },
                "id": { "type": "string", "description": "gather: artifact id" },
                "threshold_hours": {
                    "type": "integer",
                    "default": 24,
                    "description": "list_stale: hours since last refresh to consider stale (default 24)"
                },
                "scope": {
                    "type": "string",
                    "enum": ["project", "repo", "umbrella", "all"],
                    "default": "project",
                    "description": "list_stale: scope (default project)"
                },
                "limit": {
                    "type": "integer",
                    "default": 10,
                    "maximum": 50,
                    "description": "list_stale: max results (default 10, max 50)"
                }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let action = args["action"].as_str().ok_or_else(|| {
            RecoverableError::new("action required — one of: gather, list_stale")
        })?;
        match action {
            "gather"     => super::refresh::call(ctx, args).await,
            "list_stale" => super::refresh_stale::call(ctx, args).await,
            other => Err(RecoverableError::new(format!(
                "unknown action '{other}' — expected one of: gather, list_stale"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;
    use crate::workspace::WorkspaceConfig;
    use std::sync::Arc;

    fn mk_ctx() -> ToolContext {
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap())),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: None,
        }
    }

    #[tokio::test]
    async fn unknown_action_returns_recoverable_error() {
        let err = ArtifactRefreshTool
            .call(&mk_ctx(), serde_json::json!({"action": "bogus"}))
            .await
            .unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }

    #[tokio::test]
    async fn list_stale_action_routes_correctly() {
        let v = ArtifactRefreshTool
            .call(&mk_ctx(), serde_json::json!({"action": "list_stale"}))
            .await
            .unwrap();
        assert!(v.is_array() || v["items"].is_array());
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd crates/librarian-mcp && cargo test artifact_refresh::tests 2>&1 | tail -10
```

---

## Task 7: Create `librarian.rs`

**Files:**
- Create: `crates/librarian-mcp/src/tools/librarian.rs`

- [ ] **Step 1: Write librarian.rs**

```rust
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use super::{RecoverableError, Tool, ToolContext};

pub struct Librarian;

#[async_trait]
impl Tool for Librarian {
    fn name(&self) -> &'static str {
        "librarian"
    }

    fn description(&self) -> &'static str {
        "Workspace-level librarian operations. \
         action: context | reindex | tracker_design | workspace_state_at. \
         context: pack topic/anchor neighbourhood into a markdown bundle. \
         reindex: re-scan and classify markdown artifacts. \
         tracker_design: return teaching prompt + archetype library (call BEFORE artifact(create) for trackers). \
         workspace_state_at: time-travel snapshot of all artifacts at a commit/timestamp."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["action"],
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["context", "reindex", "tracker_design", "workspace_state_at"],
                    "description": "Operation to perform"
                },
                "topic": { "type": "string", "description": "context: subject for semantic/LIKE search across titles and topics" },
                "anchor_id": { "type": "string", "description": "context: artifact id to anchor the bundle (uses link graph)" },
                "max_tokens": { "type": "integer", "default": 4000, "description": "context: approximate token budget" },
                "include_archived": { "type": "boolean", "default": false },
                "scope": {
                    "type": "string",
                    "enum": ["project", "repo", "umbrella", "all"],
                    "default": "project",
                    "description": "context/reindex/workspace_state_at: scope. Defaults to current sub-project."
                },
                "repo": { "type": "string", "description": "reindex: restrict to a specific workspace root" },
                "force": { "type": "boolean", "description": "reindex: wipe rows for targeted scope before re-walking" },
                "intent": { "type": "string", "description": "tracker_design: free-form intent (optional)" },
                "commit": { "type": "string", "description": "workspace_state_at: git commit hash as time-travel cutoff. Exactly one of commit or timestamp required." },
                "timestamp": { "type": "integer", "format": "int64", "description": "workspace_state_at: unix epoch ms as cutoff. Exactly one of commit or timestamp required." },
                "kinds": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "workspace_state_at: filter by artifact kinds"
                },
                "freshness_filter": {
                    "type": "array",
                    "items": { "type": "string", "enum": ["fresh", "stale", "unknown", "superseded"] },
                    "description": "workspace_state_at: only return artifacts matching these freshness values"
                }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let action = args["action"].as_str().ok_or_else(|| {
            RecoverableError::new(
                "action required — one of: context, reindex, tracker_design, workspace_state_at",
            )
        })?;
        match action {
            "context"           => super::context::call(ctx, args).await,
            "reindex"           => super::reindex::call(ctx, args).await,
            "tracker_design"    => super::tracker_design::call(ctx, args).await,
            "workspace_state_at" => super::workspace_state_at::call(ctx, args).await,
            other => Err(RecoverableError::new(format!(
                "unknown action '{other}' — expected one of: context, reindex, tracker_design, workspace_state_at"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;
    use crate::workspace::WorkspaceConfig;
    use std::sync::Arc;

    fn mk_ctx() -> ToolContext {
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap())),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: None,
        }
    }

    #[tokio::test]
    async fn unknown_action_returns_recoverable_error() {
        let err = Librarian
            .call(&mk_ctx(), serde_json::json!({"action": "bogus"}))
            .await
            .unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }

    #[tokio::test]
    async fn tracker_design_routes_correctly() {
        let v = Librarian
            .call(&mk_ctx(), serde_json::json!({"action": "tracker_design"}))
            .await
            .unwrap();
        assert!(v["archetypes"].is_array());
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd crates/librarian-mcp && cargo test librarian::tests 2>&1 | tail -10
```

---

## Task 8: Update `mod.rs` and `server.rs`

**Files:**
- Modify: `crates/librarian-mcp/src/tools/mod.rs`
- Modify: `crates/librarian-mcp/src/server.rs`

- [ ] **Step 1: Add new module declarations to mod.rs**

After the last existing `pub mod tracker_design;` line, insert the four new module declarations:

```rust
pub mod artifact;
pub mod artifact_event;
pub mod artifact_refresh;
pub mod librarian;
```

- [ ] **Step 2: Replace `all_tools()` in mod.rs**

Replace:
```rust
pub fn all_tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(find::ArtifactFind),
        Arc::new(get::ArtifactGet),
        Arc::new(graph::ArtifactGraph),
        Arc::new(create::ArtifactCreate),
        Arc::new(update::ArtifactUpdate),
        Arc::new(link::ArtifactLink),
        Arc::new(event_create::ArtifactEventCreate),
        Arc::new(timeline::ArtifactTimeline),
        Arc::new(state_at::ArtifactStateAt),
        Arc::new(workspace_state_at::WorkspaceStateAt),
        Arc::new(reindex::LibrarianReindex),
        Arc::new(context::LibrarianContext),
        Arc::new(augment::ArtifactAugment),
        Arc::new(refresh::ArtifactRefresh),
        Arc::new(tracker_design::TrackerDesign),
        Arc::new(refresh_stale::ArtifactRefreshStale),
    ]
}
```

With:
```rust
pub fn all_tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(artifact::Artifact),
        Arc::new(artifact_event::ArtifactEvent),
        Arc::new(augment::ArtifactAugment),
        Arc::new(artifact_refresh::ArtifactRefreshTool),
        Arc::new(librarian::Librarian),
    ]
}
```

- [ ] **Step 3: Update serde_error test in server.rs**

The test `serde_error_gets_helpful_hint` uses `ArtifactGet` directly. Update it:

```rust
#[tokio::test]
async fn serde_error_gets_helpful_hint() {
    use crate::tools::artifact::Artifact;
    use crate::tools::Tool;
    let ctx = mk_ctx();
    // Pass a string where a bool is expected — serde will reject it.
    let err = Artifact
        .call(
            &ctx,
            serde_json::json!({
                "action": "get",
                "id": "x",
                "include_links": "true"   // should be bool
            }),
        )
        .await
        .unwrap_err();
    let s = err.to_string();
    assert!(
        s.to_lowercase().contains("bool") || s.to_lowercase().contains("string"),
        "expected type-hint in error, got: {s}"
    );
}
```

- [ ] **Step 4: Run all librarian-mcp tests**

```bash
cd crates/librarian-mcp && cargo test 2>&1 | tail -20
```
Expected: all tests pass. If any fail due to missing `pub(super)` visibility on handler `call` functions, fix the visibility in the relevant handler file.

- [ ] **Step 5: Run clippy**

```bash
cd crates/librarian-mcp && cargo clippy -- -D warnings 2>&1 | head -40
```
Fix any warnings. Common issues: unused imports from removed `impl Tool` blocks in handler files.

---
## Task 9: Update prompt surfaces

**Files:**
- Modify: `crates/librarian-mcp/src/prompts/server_instructions.md`
- Modify: `crates/librarian-mcp/src/prompts/companion_hint.md`

The goal is to replace every old tool name in the "Tool selection" tables with the new names.

- [ ] **Step 1: Update server_instructions.md tool selection table**

Replace the `## Tool selection` section table with:

```markdown
## Tool selection

| Want                                             | Use                    |
|--------------------------------------------------|------------------------|
| List artifacts of one kind                       | `artifact` with `action=find`, `kind` param |
| Complex filter (multiple fields, and/or/not)     | `artifact` with `action=find`  |
| Read one artifact + its neighbourhood            | `artifact` with `action=get`   |
| Edges from a node (filtered by direction/rel)    | `artifact` with `action=get`, `include_links=true`, `links_direction`, `links_rel` |
| BFS explore around a node (depth 1–3)            | `artifact` with `action=graph` |
| Topic or anchor → packed markdown context        | `librarian` with `action=context` |
| Write new artifact                               | `artifact` with `action=create` |
| Write tracker artifact with augmentation         | `artifact` with `action=create`, `kind=tracker`, `status=active`, `augment={prompt,params}` |
| Patch frontmatter or body                        | `artifact` with `action=update` |
| Patch frontmatter + record refresh in one call   | `artifact` with `action=update`, `commit_refresh=true` |
| Add relation edge (supersedes, implements, …)    | `artifact` with `action=link`  |
| Append observation note                          | `artifact_event` with `action=create`, `kind=note` |
| Manual re-scan (project-scoped by default)       | `librarian` with `action=reindex` |
| Attach/replace prompt+params on artifact         | `artifact_augment`     |
| Merge-patch params on existing augmentation      | `artifact_augment` with `merge=true` |
| Gather context for refresh (read-only)           | `artifact_refresh` with `action=gather` |
| Design a tracker (archetypes + teaching prompt)  | `librarian` with `action=tracker_design` |
| List/find augmented artifacts                    | `artifact` with `action=find`, `augmented: true` |
| Discover stale augmented artifacts               | `artifact_refresh` with `action=list_stale` |
| Time-travel: single artifact at commit           | `artifact` with `action=state_at` |
| Time-travel: all artifacts at commit             | `librarian` with `action=workspace_state_at` |
```

Also update every other reference to old tool names in the file body:
- `artifact_find` → `artifact(find)`
- `artifact_create`/`_update` → `artifact(create)`/`artifact(update)`
- `librarian_reindex` → `librarian(reindex)`
- `artifact_refresh_stale` → `artifact_refresh(list_stale)`
- `tracker_design` → `librarian(tracker_design)`
- `artifact_event_create` → `artifact_event(create)`
- `artifact_link` → `artifact(link)`
- `artifact_get` → `artifact(get)`
- `artifact_graph` → `artifact(graph)`
- `librarian_context` → `librarian(context)`

- [ ] **Step 2: Update companion_hint.md** (same table and body replacements as above)

- [ ] **Step 3: Run tests to confirm prompt surface test passes**

```bash
cargo test prompt_surfaces_reference_only_real_tools 2>&1 | tail -10
```
Expected: PASS. If it fails, grep output for the stale tool names it found and fix them.

---

## Task 10: Full verification

- [ ] **Step 1: cargo fmt**

```bash
cd crates/librarian-mcp && cargo fmt
```

- [ ] **Step 2: cargo clippy clean**

```bash
cd crates/librarian-mcp && cargo clippy -- -D warnings 2>&1 | tail -20
```
Expected: no warnings.

- [ ] **Step 3: Full test suite**

```bash
cargo test 2>&1 | tail -30
```
Expected: all tests pass. Note total test count will be similar (handler tests unchanged, 5 new dispatcher routing tests added).

- [ ] **Step 4: Release build**

```bash
cargo build --release 2>&1 | tail -10
```
Expected: builds clean.

- [ ] **Step 5: Commit**

```bash
git add crates/librarian-mcp/src/tools/ crates/librarian-mcp/src/server.rs crates/librarian-mcp/src/prompts/
git commit -m "refactor(librarian): collapse 16 tools → 5 action-dispatched tools"
```

---

## Self-review notes

- `ArtifactRefreshTool` struct name differs from old `ArtifactRefresh` — avoids name collision with the old file during the transition. Could rename after the old struct is removed.
- `augment.rs` module stays `pub` in mod.rs because `create.rs` imports `AugmentSpec` from it via `use super::augment::...`. Verify this import path still resolves after the refactor — if `create.rs` uses a relative import into the augment module, it should still work since the module still exists.
- The `gather` module is a utility (not a tool) — it stays as `mod gather` unchanged.
- `render` and `schema_validate` are utility modules used by handler files — keep them as private mods in mod.rs.
