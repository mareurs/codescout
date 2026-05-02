# Librarian Tool Consolidation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce 22 librarian MCP tools to 16 by absorbing 6 single-purpose tools into their natural parent tools.

**Architecture:** Each removed tool maps cleanly to an existing tool via one new optional parameter. No behavior changes for existing callers — all new params are optional with defaults matching prior behavior. Delete the source file once tests pass.

**Tech Stack:** Rust, `serde_json`, `rusqlite`, `crates/librarian-mcp`

---

## Merge Map

| Remove | Into | Mechanism |
|---|---|---|
| `artifact_list_by_kind` | `artifact_find` | `kind`/`status` shortcut params |
| `artifact_observe` | `artifact_event_create` | `kind=note` already existed; add observation dual-write |
| `tracker_create` | `artifact_create` | `augment` + `status` optional params |
| `artifact_update_params` | `artifact_augment` | `merge=true` mode (RFC 7396) |
| `artifact_links` | `artifact_get` | `links_direction`/`links_rel` filter params |
| `artifact_refresh_commit` | `artifact_update` | `commit_refresh: true` param |

## File Map

| File | Action |
|---|---|
| `crates/librarian-mcp/src/tools/find.rs` | Modify: add `kind`, `status` params + `merge_kind_status` helper |
| `crates/librarian-mcp/src/tools/list_by_kind.rs` | Delete |
| `crates/librarian-mcp/src/tools/event_create.rs` | Modify: dual-write ObservationRow for `kind=note` |
| `crates/librarian-mcp/src/tools/observe.rs` | Delete |
| `crates/librarian-mcp/src/tools/create.rs` | Modify: add `status`, `augment` optional params |
| `crates/librarian-mcp/src/tools/tracker_create.rs` | Delete |
| `crates/librarian-mcp/src/tools/augment.rs` | Modify: `prompt` → `Option<String>`, add `merge: bool` mode |
| `crates/librarian-mcp/src/tools/update_params.rs` | Delete |
| `crates/librarian-mcp/src/tools/get.rs` | Modify: add `links_direction`, `links_rel` to `include_links` path |
| `crates/librarian-mcp/src/tools/links.rs` | Delete |
| `crates/librarian-mcp/src/tools/update.rs` | Modify: add `commit_refresh` param |
| `crates/librarian-mcp/src/tools/refresh_commit.rs` | Delete |
| `crates/librarian-mcp/src/tools/mod.rs` | Remove 6 `pub mod` lines + 6 entries in `all_tools()` |
| `crates/librarian-mcp/src/prompts/server_instructions.md` | Update tool-selection table; remove old names |

---

## Task 1: `artifact_find` absorbs `artifact_list_by_kind`

**Files:**
- Modify: `crates/librarian-mcp/src/tools/find.rs`
- Delete: `crates/librarian-mcp/src/tools/list_by_kind.rs` (after tests pass)

**Context:** `artifact_list_by_kind` is `artifact_find` with a required `kind` top-level param instead of a filter AST. `FilterNode::Leaf` is a `serde_json::Map<String, Value>`. Building one: `FilterNode::Leaf([("kind".to_string(), json!({"eq": k}))].into_iter().collect())`.

- [ ] **Step 1: Write failing tests in `find.rs`**

Add to `crates/librarian-mcp/src/tools/find.rs` inside the `tests` module:

```rust
#[tokio::test]
async fn kind_shortcut_filters_by_kind() {
    use crate::catalog::artifact::{upsert, ArtifactRow};
    let cat = Catalog::open_in_memory().unwrap();
    fn row(id: &str, kind: &str) -> ArtifactRow {
        ArtifactRow {
            id: id.into(), repo: "r".into(), rel_path: format!("{id}.md"),
            kind: kind.into(), status: "active".into(), title: Some(id.into()),
            owners: vec![], tags: vec![], topic: None, time_scope: None,
            source: None, created_at: 0, updated_at: 0, file_mtime: 0,
            file_sha256: "".into(), confidence: 1.0,
        }
    }
    upsert(&cat, &row("spec-1", "spec")).unwrap();
    upsert(&cat, &row("plan-1", "plan")).unwrap();
    let ctx = mk_ctx(cat);
    let result = ArtifactFind.call(&ctx, json!({"kind": "spec"})).await.unwrap();
    let items = result["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], "spec-1");
}

#[tokio::test]
async fn kind_and_filter_combine_with_and() {
    use crate::catalog::artifact::{upsert, ArtifactRow};
    let cat = Catalog::open_in_memory().unwrap();
    fn row(id: &str, kind: &str, status: &str) -> ArtifactRow {
        ArtifactRow {
            id: id.into(), repo: "r".into(), rel_path: format!("{id}.md"),
            kind: kind.into(), status: status.into(), title: Some(id.into()),
            owners: vec![], tags: vec![], topic: None, time_scope: None,
            source: None, created_at: 0, updated_at: 0, file_mtime: 0,
            file_sha256: "".into(), confidence: 1.0,
        }
    }
    upsert(&cat, &row("spec-active", "spec", "active")).unwrap();
    upsert(&cat, &row("spec-draft", "spec", "draft")).unwrap();
    upsert(&cat, &row("plan-active", "plan", "active")).unwrap();
    let ctx = mk_ctx(cat);
    // kind=spec + filter status=active should return only spec-active
    let result = ArtifactFind.call(&ctx, json!({
        "kind": "spec",
        "filter": {"status": {"eq": "active"}},
        "include_archived": true
    })).await.unwrap();
    let items = result["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], "spec-active");
}

#[tokio::test]
async fn status_shortcut_filters_by_status() {
    use crate::catalog::artifact::{upsert, ArtifactRow};
    let cat = Catalog::open_in_memory().unwrap();
    fn row(id: &str, status: &str) -> ArtifactRow {
        ArtifactRow {
            id: id.into(), repo: "r".into(), rel_path: format!("{id}.md"),
            kind: "spec".into(), status: status.into(), title: Some(id.into()),
            owners: vec![], tags: vec![], topic: None, time_scope: None,
            source: None, created_at: 0, updated_at: 0, file_mtime: 0,
            file_sha256: "".into(), confidence: 1.0,
        }
    }
    upsert(&cat, &row("a", "active")).unwrap();
    upsert(&cat, &row("d", "draft")).unwrap();
    let ctx = mk_ctx(cat);
    let result = ArtifactFind.call(&ctx, json!({"status": "active", "include_archived": true})).await.unwrap();
    let items = result["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], "a");
}
```

- [ ] **Step 2: Run to confirm fail**

```bash
cargo test -p librarian-mcp -- tools::find::tests::kind_shortcut 2>&1 | tail -5
```

Expected: `error[E0560]: struct ... has no field named 'kind'`

- [ ] **Step 3: Add `kind`/`status` to `Args` and implement `merge_kind_status` in `find.rs`**

In `find.rs`, add fields to `Args`:

```rust
pub struct Args {
    pub filter: Option<FilterNode>,
    /// Shortcut: equivalent to filter `{kind: {eq: value}}`.
    pub kind: Option<String>,
    /// Shortcut: equivalent to filter `{status: {eq: value}}`. Disables the
    /// archived-hide default (same as supplying a status filter explicitly).
    pub status: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
    pub semantic: Option<String>,
    pub scope: Option<Scope>,
    #[serde(default)]
    pub include_archived: bool,
    pub augmented: Option<bool>,
}
```

Add helper before `impl Tool for ArtifactFind`:

```rust
fn merge_kind_status(
    filter: Option<FilterNode>,
    kind: Option<&str>,
    status: Option<&str>,
) -> Option<FilterNode> {
    let mut parts: Vec<FilterNode> = Vec::new();
    if let Some(k) = kind {
        parts.push(FilterNode::Leaf(
            [("kind".to_string(), json!({"eq": k}))].into_iter().collect(),
        ));
    }
    if let Some(s) = status {
        parts.push(FilterNode::Leaf(
            [("status".to_string(), json!({"eq": s}))].into_iter().collect(),
        ));
    }
    if let Some(f) = filter {
        parts.push(f);
    }
    match parts.len() {
        0 => None,
        1 => parts.into_iter().next(),
        _ => Some(FilterNode::And { and: parts }),
    }
}
```

In `call`, replace the line `let user_filter: Option<FilterNode> = if let Some(want_augmented) = a.augmented {` with a pre-merge step. Insert before that block:

```rust
let status_shortcut_set = a.status.is_some();
let merged_filter = merge_kind_status(a.filter, a.kind.as_deref(), a.status.as_deref());
```

Then replace every use of `a.filter` in the augmented block with `merged_filter`. Also, to correctly detect `user_constrains_status` when `status` shortcut is used, the existing `filter_mentions_status` call must see the merged filter. Replace:

```rust
let user_constrains_status = user_filter
    .as_ref()
    .map(filter_mentions_status)
    .unwrap_or(false);
```

with:

```rust
let user_constrains_status = status_shortcut_set
    || user_filter.as_ref().map(filter_mentions_status).unwrap_or(false);
```

Add `kind` and `status` to `input_schema` in the properties object:

```rust
"kind": {
    "type": "string",
    "description": "Shortcut: filter to this kind only (equivalent to filter {kind: {eq: value}})"
},
"status": {
    "type": "string",
    "description": "Shortcut: filter to this status (equivalent to filter {status: {eq: value}}). Disables archived-hide default."
},
```

Update description:

```rust
fn description(&self) -> &'static str {
    "Search artifacts by filter AST (kind/status/tags/updated_at etc). \
     Shortcut params: `kind` and `status` expand to eq-filters and combine \
     with `filter` using AND. \
     Composition: and/or/not. Leaf ops: eq/ne/in/nin/gt/lt/gte/lte/contains/prefix. \
     Defaults: scope=project (current sub-project only), archived/superseded hidden \
     when the filter does not constrain status. Pass scope=repo|umbrella|all to widen, \
     include_archived=true to surface archived rows."
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p librarian-mcp -- tools::find::tests 2>&1 | tail -10
```

Expected: all pass.

- [ ] **Step 5: Remove `list_by_kind` from `mod.rs`**

In `crates/librarian-mcp/src/tools/mod.rs`:
- Remove `pub mod list_by_kind;`
- Remove `Arc::new(list_by_kind::ArtifactListByKind),` from `all_tools()`

- [ ] **Step 6: Delete the file**

```bash
rm crates/librarian-mcp/src/tools/list_by_kind.rs
```

- [ ] **Step 7: Verify full build**

```bash
cargo test -p librarian-mcp 2>&1 | tail -15
```

Expected: all tests pass, no compile errors.

- [ ] **Step 8: Commit**

```bash
git add crates/librarian-mcp/src/tools/find.rs crates/librarian-mcp/src/tools/mod.rs
git rm crates/librarian-mcp/src/tools/list_by_kind.rs
git commit -m "refactor(librarian): absorb artifact_list_by_kind into artifact_find (kind/status shortcuts)"
```

---

## Task 2: `artifact_event_create` absorbs `artifact_observe`

**Files:**
- Modify: `crates/librarian-mcp/src/tools/event_create.rs`
- Delete: `crates/librarian-mcp/src/tools/observe.rs`

**Context:** `artifact_observe` writes to the `observations` table AND dual-writes a `note` event. `artifact_event_create` only writes events. After this task, `event_create(kind=note)` also inserts an ObservationRow so that `artifact_get(include_observations=true)` still works. `ObservationRow` and `observations::insert` are in `crates/librarian-mcp/src/catalog/observations.rs`.

- [ ] **Step 1: Write failing test in `event_create.rs`**

Add inside the `tests` module in `event_create.rs`:

```rust
#[tokio::test]
async fn note_event_also_writes_observation_row() {
    use crate::catalog::{observations, Catalog};
    let cat = Catalog::open_in_memory().unwrap();
    let artifact = art("obs-art");
    crate::catalog::artifact::upsert(&cat, &artifact).unwrap();
    let ctx = crate::tools::tests::mk_tool_ctx(cat.clone());

    ArtifactEventCreate
        .call(
            &ctx,
            json!({
                "artifact_id": "obs-art",
                "kind": "note",
                "payload": {"text": "hello observation"}
            }),
        )
        .await
        .unwrap();

    let obs = observations::list_for_artifact(&cat, "obs-art").unwrap();
    assert_eq!(obs.len(), 1);
    assert_eq!(obs[0].text, "hello observation");
}
```

Note: `mk_tool_ctx` helper may need to be added if not present. Check `tests` module for existing helpers. Use the same pattern as `art()` helper already in the file.

- [ ] **Step 2: Run to confirm fail**

```bash
cargo test -p librarian-mcp -- tools::event_create::tests::note_event_also_writes_observation 2>&1 | tail -5
```

Expected: test fails with `assertion failed: obs.len() == 1` (obs is empty).

- [ ] **Step 3: Add observation dual-write to `event_create.rs`**

In `event_create.rs::call`, find the block that handles `kind == "note"`. After inserting the event row, add the observation write. The note payload has shape `{"text": "..."}`. The `author` arg is the observation source.

Locate the section in `call` that processes the event (after `apply_payload_to_frontmatter` and event insert). Add:

```rust
// Dual-write to observations table for note events so artifact_get(include_observations)
// still surfaces them.
if a.kind == "note" {
    if let Some(text) = a.payload.get("text").and_then(|v| v.as_str()) {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let obs = crate::catalog::observations::ObservationRow {
            id: None,
            artifact_id: a.artifact_id.clone(),
            text: text.to_string(),
            source: a.author.clone(),
            created_at: now_ms,
        };
        // Best-effort: observation failure must not fail the event write.
        let _ = crate::catalog::observations::insert(&cat, &obs);
    }
}
```

Place this block after the event row has been committed to DB (after the transaction completes) but before the `Ok(...)` return. Keep inside the `_guard` scope so `cat` is still live.

- [ ] **Step 4: Run tests**

```bash
cargo test -p librarian-mcp -- tools::event_create::tests 2>&1 | tail -10
```

Expected: all pass including new test.

- [ ] **Step 5: Remove `observe` from `mod.rs`**

- Remove `pub mod observe;`
- Remove `Arc::new(observe::ArtifactObserve),` from `all_tools()`

- [ ] **Step 6: Delete the file**

```bash
rm crates/librarian-mcp/src/tools/observe.rs
```

- [ ] **Step 7: Verify full build**

```bash
cargo test -p librarian-mcp 2>&1 | tail -15
```

- [ ] **Step 8: Commit**

```bash
git add crates/librarian-mcp/src/tools/event_create.rs crates/librarian-mcp/src/tools/mod.rs
git rm crates/librarian-mcp/src/tools/observe.rs
git commit -m "refactor(librarian): absorb artifact_observe into artifact_event_create (note dual-write)"
```

---

## Task 3: `artifact_create` absorbs `tracker_create`

**Files:**
- Modify: `crates/librarian-mcp/src/tools/create.rs`
- Delete: `crates/librarian-mcp/src/tools/tracker_create.rs`

**Context:** `tracker_create` = `artifact_create` + `artifact_augment` called atomically. `artifact_create` currently hardcodes `status: "draft"`. `tracker_create` uses `status: "active"`. Both need to be supported, so this task also adds an optional `status` param to `create`.

- [ ] **Step 1: Write failing tests in `create.rs`**

Add inside the `tests` module:

```rust
#[tokio::test]
async fn create_with_augment_writes_augmentation_row() {
    use crate::catalog::{augmentation, Catalog};
    use tempfile::TempDir;
    let tmp = TempDir::new().unwrap();
    let ctx = mk_ctx(tmp.path().to_path_buf());

    let result = ArtifactCreate
        .call(
            &ctx,
            json!({
                "repo": "root",
                "rel_path": "trackers/my-tracker.md",
                "kind": "tracker",
                "title": "My Tracker",
                "body": "initial body",
                "status": "active",
                "augment": {
                    "prompt": "Keep this tracker up to date.",
                    "params": {"threshold": 5}
                }
            }),
        )
        .await
        .unwrap();

    let id = result["id"].as_str().unwrap().to_string();
    let cat = ctx.catalog.lock();
    let aug = augmentation::get(&cat, &id).unwrap();
    assert!(aug.is_some(), "augmentation row must be created");
    let aug = aug.unwrap();
    assert_eq!(aug.prompt, "Keep this tracker up to date.");
    let params: serde_json::Value = serde_json::from_str(&aug.params).unwrap();
    assert_eq!(params["threshold"], 5);
}

#[tokio::test]
async fn create_with_explicit_status_active() {
    use tempfile::TempDir;
    let tmp = TempDir::new().unwrap();
    let ctx = mk_ctx(tmp.path().to_path_buf());

    ArtifactCreate
        .call(
            &ctx,
            json!({
                "repo": "root",
                "rel_path": "trackers/active.md",
                "kind": "tracker",
                "title": "Active",
                "body": "",
                "status": "active"
            }),
        )
        .await
        .unwrap();

    let cat = ctx.catalog.lock();
    let row = crate::catalog::artifact::get(&cat, &crate::catalog::artifact_id("root", "trackers/active.md")).unwrap().unwrap();
    assert_eq!(row.status, "active");
}
```

Note: `mk_ctx` in `create.rs` tests creates a workspace with a single repo named `"root"`. Use the existing pattern — check `create.rs` tests for the `mk_ctx` helper that sets up `WorkspaceConfig` with a single root named `"root"`.

- [ ] **Step 2: Run to confirm fail**

```bash
cargo test -p librarian-mcp -- tools::create::tests::create_with_augment 2>&1 | tail -5
```

Expected: compile error — `augment` field unknown.

- [ ] **Step 3: Add `status` and `augment` to `create.rs`**

Add a new struct before `Args`:

```rust
#[derive(Debug, Deserialize)]
pub struct AugmentSpec {
    pub prompt: String,
    pub params: Option<Value>,
}
```

Update `Args`:

```rust
pub struct Args {
    pub repo: String,
    pub rel_path: String,
    pub kind: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub owners: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Optional initial status. Defaults to "draft".
    pub status: Option<String>,
    /// If set, attach an augmentation row atomically after creating the artifact.
    pub augment: Option<AugmentSpec>,
}
```

In `call`, change the hardcoded `"draft"` to use `a.status`:

```rust
let initial_status = a.status.as_deref().unwrap_or("draft").to_string();
// replace "draft".into() with initial_status.clone() in both the Frontmatter and ArtifactRow
```

After `artifact::upsert(&ctx.catalog.lock(), &row)?;`, add the augmentation block:

```rust
if let Some(aug_spec) = &a.augment {
    let params_str = aug_spec
        .params
        .as_ref()
        .map(|p| serde_json::to_string(p))
        .transpose()?
        .unwrap_or_else(|| "{}".to_string());
    let now_ts = chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();
    let cat = ctx.catalog.lock();
    crate::catalog::augmentation::upsert(
        &cat,
        &crate::catalog::augmentation::AugmentationRow {
            artifact_id: id.clone(),
            prompt: aug_spec.prompt.clone(),
            params: params_str,
            last_refreshed_at: None,
            refresh_count: 0,
            created_at: now_ts.clone(),
            updated_at: now_ts,
            render_template: None,
            params_schema: None,
        },
    )?;
}
```

Update `input_schema` to add `status` and `augment` properties:

```rust
"status": {
    "type": "string",
    "description": "Initial status. Defaults to \"draft\"."
},
"augment": {
    "type": "object",
    "description": "Attach augmentation atomically. Pass prompt + optional params.",
    "properties": {
        "prompt": {"type": "string"},
        "params": {"type": "object"}
    },
    "required": ["prompt"]
},
```

Update description:

```rust
fn description(&self) -> &'static str {
    "Create a new artifact. Writes frontmatter + body to the file. Fails if path exists. \
     Optional `status` (default: draft) and `augment` (prompt + params) for atomic \
     tracker-style creation."
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p librarian-mcp -- tools::create::tests 2>&1 | tail -10
```

Expected: all pass.

- [ ] **Step 5: Remove `tracker_create` from `mod.rs`**

- Remove `pub mod tracker_create;`
- Remove `Arc::new(tracker_create::TrackerCreate),` from `all_tools()`

- [ ] **Step 6: Delete the file**

```bash
rm crates/librarian-mcp/src/tools/tracker_create.rs
```

- [ ] **Step 7: Verify full build**

```bash
cargo test -p librarian-mcp 2>&1 | tail -15
```

- [ ] **Step 8: Commit**

```bash
git add crates/librarian-mcp/src/tools/create.rs crates/librarian-mcp/src/tools/mod.rs
git rm crates/librarian-mcp/src/tools/tracker_create.rs
git commit -m "refactor(librarian): absorb tracker_create into artifact_create (status + augment params)"
```

---

## Task 4: `artifact_augment` absorbs `artifact_update_params`

**Files:**
- Modify: `crates/librarian-mcp/src/tools/augment.rs`
- Delete: `crates/librarian-mcp/src/tools/update_params.rs`

**Context:** `artifact_update_params` does RFC 7396 merge-patch on `params` only. Adding `merge: true` to `artifact_augment` triggers this path. When `merge=true`, `prompt` is not required. Make `prompt: Option<String>`. Validate against `params_schema` before writing (same as `update_params` does).

- [ ] **Step 1: Write failing tests in `augment.rs`**

Add inside `tests` module:

```rust
#[tokio::test]
async fn merge_true_patches_params_without_touching_prompt() {
    let ctx = mk_ctx();
    seed_artifact(&ctx, "aug-1");
    // First, augment with a prompt and initial params
    ArtifactAugment
        .call(
            &ctx,
            json!({"id": "aug-1", "prompt": "do stuff", "params": {"a": 1, "b": 2}}),
        )
        .await
        .unwrap();

    // Now merge-patch: add c, delete b
    ArtifactAugment
        .call(
            &ctx,
            json!({"id": "aug-1", "merge": true, "params": {"c": 3, "b": null}}),
        )
        .await
        .unwrap();

    let cat = ctx.catalog.lock();
    let aug = crate::catalog::augmentation::get(&cat, "aug-1").unwrap().unwrap();
    assert_eq!(aug.prompt, "do stuff", "prompt must be unchanged");
    let params: serde_json::Value = serde_json::from_str(&aug.params).unwrap();
    assert_eq!(params["a"], 1, "a must survive merge");
    assert_eq!(params["c"], 3, "c must be added");
    assert!(params.get("b").map(|v| v.is_null()).unwrap_or(true), "b must be deleted");
}

#[tokio::test]
async fn merge_true_without_existing_augmentation_errors() {
    let ctx = mk_ctx();
    seed_artifact(&ctx, "aug-2");
    let err = ArtifactAugment
        .call(&ctx, json!({"id": "aug-2", "merge": true, "params": {"x": 1}}))
        .await;
    assert!(err.is_err());
    let msg = err.unwrap_err().to_string();
    assert!(msg.contains("artifact_augment"), "error must mention artifact_augment");
}

#[tokio::test]
async fn non_merge_without_prompt_errors() {
    let ctx = mk_ctx();
    seed_artifact(&ctx, "aug-3");
    let err = ArtifactAugment
        .call(&ctx, json!({"id": "aug-3", "params": {"x": 1}}))
        .await;
    assert!(err.is_err());
    let msg = err.unwrap_err().to_string();
    assert!(msg.contains("prompt"), "error must mention prompt");
}
```

- [ ] **Step 2: Run to confirm fail**

```bash
cargo test -p librarian-mcp -- tools::augment::tests::merge_true 2>&1 | tail -5
```

Expected: compile error on unknown field `merge`.

- [ ] **Step 3: Implement in `augment.rs`**

Update `Args`:

```rust
pub struct Args {
    pub id: String,
    /// Required when merge=false (create/replace). Ignored when merge=true.
    pub prompt: Option<String>,
    pub params: Option<Value>,
    pub render_template: Option<String>,
    pub params_schema: Option<Value>,
    /// When true: RFC 7396 merge-patch on params only. Requires existing augmentation.
    #[serde(default)]
    pub merge: bool,
}
```

Replace `call` body:

```rust
async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args)?;
    let cat = ctx.catalog.lock();

    if a.merge {
        // RFC 7396 merge-patch path — mirrors artifact_update_params
        let patch = a.params.as_ref().cloned().unwrap_or(Value::Object(Default::default()));
        if let Some(existing) = augmentation::get(&cat, &a.id)? {
            if let Some(schema_text) = existing.params_schema.as_deref() {
                let mut current: Value = serde_json::from_str(&existing.params)
                    .unwrap_or(Value::Object(Default::default()));
                augmentation::apply_merge_patch(&mut current, &patch);
                crate::tools::schema_validate::validate_against_stored(schema_text, &current)
                    .map_err(|e| {
                        RecoverableError::new(format!("merged params violate params_schema: {e}"))
                    })?;
            }
        }
        let found = augmentation::merge_params(&cat, &a.id, &patch)?;
        if !found {
            return Err(RecoverableError::new(format!(
                "no augmentation for artifact '{}' — call artifact_augment first",
                a.id
            )));
        }
        return Ok(json!("ok"));
    }

    // Create/replace path
    let prompt = a.prompt.ok_or_else(|| {
        RecoverableError::new("prompt is required (set merge=true to patch params only)")
    })?;

    if artifact::get(&cat, &a.id)?.is_none() {
        return Err(RecoverableError::new(format!("artifact '{}' not found", a.id)));
    }

    let params_str = a
        .params
        .map(|p| serde_json::to_string(&p))
        .transpose()?
        .unwrap_or_else(|| "{}".to_string());

    let params_schema_str = a
        .params_schema
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;

    if let Some(schema) = &a.params_schema {
        let parsed_params: Value = serde_json::from_str(&params_str)?;
        crate::tools::schema_validate::validate(schema, &parsed_params).map_err(|e| {
            RecoverableError::new(format!("initial params violate params_schema: {e}"))
        })?;
    }

    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
    augmentation::upsert(
        &cat,
        &augmentation::AugmentationRow {
            artifact_id: a.id.clone(),
            prompt,
            params: params_str,
            last_refreshed_at: None,
            refresh_count: 0,
            created_at: now.clone(),
            updated_at: now,
            render_template: a.render_template,
            params_schema: params_schema_str,
        },
    )?;
    Ok(json!("ok"))
}
```

Update `input_schema` to reflect `prompt` as optional and add `merge`:

```rust
"prompt": {
    "type": "string",
    "description": "Refresh prompt. Required when merge=false."
},
"merge": {
    "type": "boolean",
    "default": false,
    "description": "When true: RFC 7396 merge-patch on params only. prompt is ignored. Requires existing augmentation."
},
```

Remove `"required": ["prompt"]` from the schema (or change to empty required array).

Update description:

```rust
fn description(&self) -> &'static str {
    "Attach or replace a persistent prompt + params on any artifact (merge=false, default). \
     Or RFC 7396 merge-patch params on an existing augmentation (merge=true — prompt ignored). \
     Idempotent in replace mode. merge=true errors if no augmentation exists yet."
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p librarian-mcp -- tools::augment::tests 2>&1 | tail -10
```

Expected: all pass. Note: `creates_augmentation_row` and `idempotent_update_replaces_prompt` may need minor updates since `prompt` is now `Option<String>` — pass `"prompt": "..."` explicitly, they should still work.

- [ ] **Step 5: Remove `update_params` from `mod.rs`**

- Remove `pub mod update_params;`
- Remove `Arc::new(update_params::ArtifactUpdateParams),` from `all_tools()`

- [ ] **Step 6: Delete the file**

```bash
rm crates/librarian-mcp/src/tools/update_params.rs
```

- [ ] **Step 7: Verify full build**

```bash
cargo test -p librarian-mcp 2>&1 | tail -15
```

- [ ] **Step 8: Commit**

```bash
git add crates/librarian-mcp/src/tools/augment.rs crates/librarian-mcp/src/tools/mod.rs
git rm crates/librarian-mcp/src/tools/update_params.rs
git commit -m "refactor(librarian): absorb artifact_update_params into artifact_augment (merge=true mode)"
```

---

## Task 5: `artifact_get` absorbs `artifact_links`

**Files:**
- Modify: `crates/librarian-mcp/src/tools/get.rs`
- Delete: `crates/librarian-mcp/src/tools/links.rs`

**Context:** `artifact_get` already has `include_links: Option<bool>` which returns `{outgoing: [...], incoming: [...]}`. Add `links_direction: Option<String>` (default "both") and `links_rel: Option<String>` to filter the returned links. These params are no-ops when `include_links` is false.

- [ ] **Step 1: Write failing tests in `get.rs`**

Add inside `tests` module (after the existing `get_with_links_and_observations` test):

```rust
#[tokio::test]
async fn include_links_direction_out_hides_incoming() {
    use crate::catalog::{artifact, links as lcat, Catalog};
    use crate::catalog::artifact::ArtifactRow;
    let cat = Catalog::open_in_memory().unwrap();
    let base = mk_row("center");
    let src = mk_row("other");
    artifact::upsert(&cat, &base).unwrap();
    artifact::upsert(&cat, &src).unwrap();
    lcat::insert(&cat, &lcat::LinkRow {
        src_id: "center".into(), dst_id: "other".into(), rel: "implements".into(),
        created_at: 0,
    }).unwrap();
    lcat::insert(&cat, &lcat::LinkRow {
        src_id: "other".into(), dst_id: "center".into(), rel: "supersedes".into(),
        created_at: 0,
    }).unwrap();
    let ctx = mk_ctx(cat);
    let result = ArtifactGet
        .call(&ctx, json!({"id": "center", "include_links": true, "links_direction": "out"}))
        .await
        .unwrap();
    let outgoing = result["links"]["outgoing"].as_array().unwrap();
    let incoming = result["links"]["incoming"].as_array().unwrap();
    assert_eq!(outgoing.len(), 1);
    assert_eq!(incoming.len(), 0);
}

#[tokio::test]
async fn include_links_rel_filters_by_rel_type() {
    use crate::catalog::{artifact, links as lcat, Catalog};
    let cat = Catalog::open_in_memory().unwrap();
    artifact::upsert(&cat, &mk_row("a")).unwrap();
    artifact::upsert(&cat, &mk_row("b")).unwrap();
    artifact::upsert(&cat, &mk_row("c")).unwrap();
    lcat::insert(&cat, &lcat::LinkRow {
        src_id: "a".into(), dst_id: "b".into(), rel: "implements".into(), created_at: 0,
    }).unwrap();
    lcat::insert(&cat, &lcat::LinkRow {
        src_id: "a".into(), dst_id: "c".into(), rel: "supersedes".into(), created_at: 0,
    }).unwrap();
    let ctx = mk_ctx(cat);
    let result = ArtifactGet
        .call(&ctx, json!({"id": "a", "include_links": true, "links_rel": "implements"}))
        .await
        .unwrap();
    let outgoing = result["links"]["outgoing"].as_array().unwrap();
    assert_eq!(outgoing.len(), 1);
    assert_eq!(outgoing[0]["rel"], "implements");
}
```

Note: check the `lcat::LinkRow` struct fields — if `created_at` is not a field, remove it. Check `crates/librarian-mcp/src/catalog/links.rs` for the struct definition.

- [ ] **Step 2: Run to confirm fail**

```bash
cargo test -p librarian-mcp -- tools::get::tests::include_links_direction 2>&1 | tail -5
```

Expected: compile error — `links_direction` not in `Args`.

- [ ] **Step 3: Add `links_direction` and `links_rel` to `get.rs`**

Add to `Args`:

```rust
/// Filter links by direction: "out"|"in"|"both". Only applies when include_links=true. Default: "both".
pub links_direction: Option<String>,
/// Filter links by rel type. Only applies when include_links=true.
pub links_rel: Option<String>,
```

In `call`, find the `links_json` block:

```rust
let links_json = if want_links {
    let out_links = links::outgoing(&cat, &a.id)?;
    let in_links = links::incoming(&cat, &a.id)?;
    Some(json!({
        "outgoing": out_links.into_iter().map(...).collect::<Vec<_>>(),
        "incoming": in_links.into_iter().map(...).collect::<Vec<_>>(),
    }))
} else {
    None
};
```

Replace with:

```rust
let links_json = if want_links {
    let direction = a.links_direction.as_deref().unwrap_or("both");
    let rel_filter = a.links_rel.as_deref();

    let outgoing_items: Vec<Value> = if direction == "out" || direction == "both" {
        links::outgoing(&cat, &a.id)?
            .into_iter()
            .filter(|l| rel_filter.map_or(true, |r| l.rel == r))
            .map(|l| json!({"dst_id": l.dst_id, "rel": l.rel}))
            .collect()
    } else {
        vec![]
    };

    let incoming_items: Vec<Value> = if direction == "in" || direction == "both" {
        links::incoming(&cat, &a.id)?
            .into_iter()
            .filter(|l| rel_filter.map_or(true, |r| l.rel == r))
            .map(|l| json!({"src_id": l.src_id, "rel": l.rel}))
            .collect()
    } else {
        vec![]
    };

    Some(json!({
        "outgoing": outgoing_items,
        "incoming": incoming_items,
    }))
} else {
    None
};
```

Update `input_schema` to add the two new params:

```rust
"links_direction": {
    "type": "string",
    "enum": ["out", "in", "both"],
    "description": "Filter links by direction. Default: both. Only applies when include_links=true."
},
"links_rel": {
    "type": "string",
    "description": "Filter links to only this rel type. Only applies when include_links=true."
},
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p librarian-mcp -- tools::get::tests 2>&1 | tail -10
```

Expected: all pass.

- [ ] **Step 5: Remove `links` from `mod.rs`**

- Remove `pub mod links;`
- Remove `Arc::new(links::ArtifactLinks),` from `all_tools()`

- [ ] **Step 6: Delete the file**

```bash
rm crates/librarian-mcp/src/tools/links.rs
```

- [ ] **Step 7: Verify full build**

```bash
cargo test -p librarian-mcp 2>&1 | tail -15
```

- [ ] **Step 8: Commit**

```bash
git add crates/librarian-mcp/src/tools/get.rs crates/librarian-mcp/src/tools/mod.rs
git rm crates/librarian-mcp/src/tools/links.rs
git commit -m "refactor(librarian): absorb artifact_links into artifact_get (links_direction/links_rel params)"
```

---

## Task 6: `artifact_update` absorbs `artifact_refresh_commit`

**Files:**
- Modify: `crates/librarian-mcp/src/tools/update.rs`
- Delete: `crates/librarian-mcp/src/tools/refresh_commit.rs`

**Context:** `artifact_refresh_commit` calls `augmentation::commit_refresh(&cat, &id)` which increments `refresh_count` and sets `last_refreshed_at`. Adding `commit_refresh: true` to `artifact_update` runs this atomically after the file + DB update, saving a round-trip in the refresh workflow.

- [ ] **Step 1: Write failing test in `update.rs`**

Add inside `tests` module:

```rust
#[tokio::test]
async fn update_with_commit_refresh_increments_refresh_count() {
    use crate::catalog::{augmentation, augmentation::AugmentationRow};
    use tempfile::TempDir;
    let tmp = TempDir::new().unwrap();
    let ctx = mk_ctx(tmp.path().to_path_buf());

    // Create artifact
    let artifact_id = {
        let content = "---\nkind: tracker\nstatus: active\ntitle: T\n---\n\nbody\n";
        let path = tmp.path().join("tracker.md");
        std::fs::write(&path, content).unwrap();
        let id = crate::catalog::artifact_id("root", "tracker.md");
        let now = chrono::Utc::now().timestamp_millis();
        let row = crate::catalog::artifact::ArtifactRow {
            id: id.clone(), repo: "root".into(), rel_path: "tracker.md".into(),
            kind: "tracker".into(), status: "active".into(), title: Some("T".into()),
            owners: vec![], tags: vec![], topic: None, time_scope: None,
            source: None, created_at: now, updated_at: now, file_mtime: now,
            file_sha256: "".into(), confidence: 1.0,
        };
        crate::catalog::artifact::upsert(&ctx.catalog.lock(), &row).unwrap();
        id
    };

    // Seed augmentation
    {
        let ts = "2026-01-01T00:00:00.000Z".to_string();
        let cat = ctx.catalog.lock();
        augmentation::upsert(&cat, &AugmentationRow {
            artifact_id: artifact_id.clone(), prompt: "p".into(), params: "{}".into(),
            last_refreshed_at: None, refresh_count: 0,
            created_at: ts.clone(), updated_at: ts,
            render_template: None, params_schema: None,
        }).unwrap();
    }

    // Update body + commit refresh in one call
    crate::tools::update::ArtifactUpdate
        .call(&ctx, json!({
            "id": artifact_id,
            "patch": {"body": "new body"},
            "commit_refresh": true
        }))
        .await
        .unwrap();

    let cat = ctx.catalog.lock();
    let aug = augmentation::get(&cat, &artifact_id).unwrap().unwrap();
    assert_eq!(aug.refresh_count, 1);
    assert!(aug.last_refreshed_at.is_some());
}
```

- [ ] **Step 2: Run to confirm fail**

```bash
cargo test -p librarian-mcp -- tools::update::tests::update_with_commit_refresh 2>&1 | tail -5
```

Expected: compile error — `commit_refresh` not in `Args`.

- [ ] **Step 3: Add `commit_refresh` to `update.rs`**

Update `Args`:

```rust
pub struct Args {
    pub id: String,
    pub patch: UpdatePatch,
    /// When true, also call augmentation::commit_refresh after the update.
    #[serde(default)]
    pub commit_refresh: bool,
}
```

In `call`, after `artifact::upsert(&cat, &updated_row)?;`, add:

```rust
let committed = if a.commit_refresh {
    Some(crate::catalog::augmentation::commit_refresh(&cat, &a.id)?)
} else {
    None
};
```

Change the return value from:

```rust
Ok(json!({"id": a.id, "updated": true}))
```

to:

```rust
let mut out = json!({"id": a.id, "updated": true});
if let Some(c) = committed {
    out["committed"] = json!(c);
}
Ok(out)
```

Add `commit_refresh` to `input_schema`:

```rust
"commit_refresh": {
    "type": "boolean",
    "default": false,
    "description": "When true, also record a completed refresh cycle (increments refresh_count, sets last_refreshed_at). Replaces a separate artifact_refresh_commit call."
},
```

Update description:

```rust
fn description(&self) -> &'static str {
    "Update an existing artifact's frontmatter fields and/or body. Only provided fields are changed. \
     Set commit_refresh=true to atomically record a completed refresh cycle (replaces artifact_refresh_commit)."
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p librarian-mcp -- tools::update::tests 2>&1 | tail -10
```

Expected: all pass.

- [ ] **Step 5: Remove `refresh_commit` from `mod.rs`**

- Remove `pub mod refresh_commit;`
- Remove `Arc::new(refresh_commit::ArtifactRefreshCommit),` from `all_tools()`

- [ ] **Step 6: Delete the file**

```bash
rm crates/librarian-mcp/src/tools/refresh_commit.rs
```

- [ ] **Step 7: Verify full build**

```bash
cargo test -p librarian-mcp 2>&1 | tail -15
```

- [ ] **Step 8: Commit**

```bash
git add crates/librarian-mcp/src/tools/update.rs crates/librarian-mcp/src/tools/mod.rs
git rm crates/librarian-mcp/src/tools/refresh_commit.rs
git commit -m "refactor(librarian): absorb artifact_refresh_commit into artifact_update (commit_refresh param)"
```

---

## Task 7: Update INSTRUCTIONS prompt + final verification

**Files:**
- Modify: `crates/librarian-mcp/src/prompts/server_instructions.md`

**Context:** The INSTRUCTIONS prompt has a tool-selection table that still lists removed tools. Update it to reflect the merged API. Also verify codescout's `prompt_surfaces_reference_only_real_tools` test still passes.

- [ ] **Step 1: Update the tool-selection table in `server_instructions.md`**

Replace the Tool selection table with:

```markdown
## Tool selection

| Want                                             | Use                    |
|--------------------------------------------------|------------------------|
| List artifacts of one kind                       | `artifact_find` with `kind` param |
| Complex filter (multiple fields, and/or/not)     | `artifact_find`        |
| Read one artifact + its neighbourhood            | `artifact_get`         |
| Edges from a node (filtered by direction/rel)    | `artifact_get` with `include_links=true`, `links_direction`, `links_rel` |
| BFS explore around a node (depth 1–3)            | `artifact_graph`       |
| Topic or anchor → packed markdown context        | `librarian_context`    |
| Write new artifact                               | `artifact_create`      |
| Write tracker artifact with augmentation         | `artifact_create` with `kind=tracker`, `status=active`, `augment={prompt,params}` |
| Patch frontmatter or body                        | `artifact_update`      |
| Patch frontmatter + record refresh in one call   | `artifact_update` with `commit_refresh=true` |
| Add relation edge (supersedes, implements, …)    | `artifact_link`        |
| Append observation note                          | `artifact_event_create` with `kind=note` |
| Manual re-scan (project-scoped by default)       | `librarian_reindex`    |
| Attach/replace prompt+params on artifact         | `artifact_augment`     |
| Merge-patch params on existing augmentation      | `artifact_augment` with `merge=true` |
| Gather context for refresh (read-only)           | `artifact_refresh`     |
| Design a tracker (archetypes + teaching prompt)  | `tracker_design`       |
| List/find augmented artifacts                    | `artifact_find` with `augmented: true` |
| Discover stale augmented artifacts               | `artifact_refresh_stale` |
```

- [ ] **Step 2: Update the Refresh cycle section**

Find the refresh cycle block (4 steps) and update step 4:

```markdown
**Refresh cycle** (3 steps — commit is now inline):
1. `artifact_refresh(id)` — server gathers context per params, returns package
   `{ prompt, params, current_body, context, hints }`. Does NOT write.
2. Synthesize new body from `prompt + context + current_body`.
3. `artifact_update(id, { body: "<new content>" }, commit_refresh=true)` — write back and record refresh metadata.
```

- [ ] **Step 3: Update the Tracker kind section**

Replace:
```
**Tracker kind:** `tracker_create` creates a `kind: tracker` artifact...
```
with:
```
**Tracker kind:** `artifact_create` with `kind=tracker`, `status=active`, and `augment={prompt, params}` creates a tracker artifact and attaches augmentation atomically.
```

Remove the `tracker_design` block's reference to `tracker_create` — update to say `artifact_create`.

Remove the `artifact_update_params` reference in `render_template` + `params_schema` section — replace with `artifact_augment(merge=true)`.

- [ ] **Step 4: Update Default scope section**

Remove `artifact_list_by_kind` from this sentence:
```
Listing tools (`artifact_list_by_kind`, `artifact_find`, `librarian_context`)
```
→
```
Listing tools (`artifact_find`, `librarian_context`)
```

- [ ] **Step 5: Run the prompt surfaces test**

```bash
cargo test -p codescout -- prompt_surfaces_reference_only_real_tools 2>&1 | tail -10
```

If it fails with stale tool names still in the surfaces, look up which surface contains them and remove. If new tool names are missing from the allowlist, add them.

- [ ] **Step 6: Run full test suite**

```bash
cargo test 2>&1 | tail -20
```

Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add crates/librarian-mcp/src/prompts/server_instructions.md
git commit -m "docs(librarian): update INSTRUCTIONS for 22→16 tool consolidation"
```

---

## Final Verification

After all 7 tasks:

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
```

Tool count check:
```bash
cargo test -p librarian-mcp -- tools::mod 2>&1 | grep -c "ok"
```

The `all_tools()` function should return 16 tools:
`artifact_find`, `artifact_get`, `artifact_list_by_kind`❌, `artifact_graph`, `artifact_create`, `artifact_update`, `artifact_link`, `artifact_event_create`, `artifact_timeline`, `artifact_state_at`, `workspace_state_at`, `librarian_reindex`, `librarian_context`, `artifact_augment`, `artifact_refresh`, `artifact_refresh_stale`, `tracker_design`

Wait — that's 17. Recount from `all_tools()` after removals:
`find`, `get`, `list_by_kind`❌, `links`❌, `graph`, `create`, `update`, `link`, `observe`❌, `event_create`, `timeline`, `state_at`, `workspace_state_at`, `reindex`, `context`, `augment`, `update_params`❌, `refresh`, `refresh_commit`❌, `tracker_create`❌, `tracker_design`, `refresh_stale`

22 − 6 = **16 tools** ✓
