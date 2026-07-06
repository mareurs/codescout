# Atomic Index Allocation for Librarian Trackers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a new `artifact(action="append_entry")` MCP action that atomically assigns the next monotonic id (e.g. `F-13`) and appends an entry to a tracker's `entry_collection` params array, closing the index-collision bug documented in `docs/superpowers/specs/2026-07-06-librarian-atomic-index-allocation-design.md`.

**Architecture:** The read-max-write sequence runs inside a single SQLite `IMMEDIATE` transaction against `artifact_augmentation.params`, guaranteeing correctness under both intra-process concurrency (already covered by the existing `Arc<parking_lot::Mutex<Catalog>>`) and cross-process concurrency (separate codescout server processes sharing one on-disk catalog file — the actual failure mode documented in the spec). A new catalog-layer function does the allocation; a thin MCP tool module wires it into the existing `artifact` action dispatch.

**Tech Stack:** Rust, rusqlite 0.39 (bundled SQLite, WAL mode), serde_json, the existing librarian catalog (`src/librarian/catalog/`) and tool dispatch (`src/librarian/tools/`).

## Global Constraints

- `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` (not `--lib` — see memory `cargo-test-lib-skips-integration`) must all pass before any task is considered done.
- `librarian` is a default Cargo feature — no `--features` flag needed for build/test commands.
- New errors use `RecoverableError::new` / `RecoverableError::with_hint` (from `crate::librarian::tools::RecoverableError`), never `anyhow::bail!` — these are expected, input-driven failures the caller can self-correct from.
- Follow existing import style: `use rusqlite::{params, OptionalExtension};` where both are needed.

---

### Task 1: Set `busy_timeout` on disk-backed catalog connections

**Why first:** `Catalog::open`/`open_with_workspace` already set `PRAGMA journal_mode = WAL` but never set `busy_timeout`. Without it, a second writer that hits the write lock held by Task 2's `IMMEDIATE` transaction gets an immediate `SQLITE_BUSY` error instead of waiting — silently defeating the whole point of the transaction. This is a one-line fix and a prerequisite for Task 3's cross-process test to pass.

**Files:**
- Modify: `src/librarian/catalog/mod.rs:166` (inside `Catalog::open`) and `src/librarian/catalog/mod.rs:199` (inside `Catalog::open_with_workspace`)
- Test: `src/librarian/catalog/mod.rs` (existing `#[cfg(test)] mod tests` block, starts at line 213)

**Interfaces:** None — this only changes connection setup, no new public functions.

- [ ] **Step 1: Write the failing test**

Add to the `mod tests` block in `src/librarian/catalog/mod.rs` (alongside `migrations_are_idempotent`, which already shows the disk-backed-`tempfile` pattern this test reuses):

```rust
#[test]
fn open_sets_busy_timeout_for_cross_process_writers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cat.sqlite");
    let cat = Catalog::open(&path).unwrap();
    let ms: i64 = cat
        .conn
        .query_row("PRAGMA busy_timeout", [], |r| r.get(0))
        .unwrap();
    assert_eq!(ms, 5000);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test open_sets_busy_timeout_for_cross_process_writers`
Expected: FAIL — `assertion 'left == right' failed` with `left: 0, right: 5000` (SQLite's default `busy_timeout` is 0).

- [ ] **Step 3: Set the pragma in both constructors**

In `src/librarian/catalog/mod.rs`, change this line (it appears twice, once in `Catalog::open` at line 166 and once in `Catalog::open_with_workspace` at line 199):

```rust
conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
```

to:

```rust
// Cross-process writers (separate codescout server instances sharing one
// catalog file) block and retry instead of failing immediately.
conn.execute_batch(
    "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;",
)?;
```

Apply this same change at both occurrences.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test open_sets_busy_timeout_for_cross_process_writers`
Expected: PASS

- [ ] **Step 5: Run the full existing catalog test suite to confirm no regression**

Run: `cargo test --lib librarian::catalog`
Expected: PASS (all existing catalog tests, including `migrations_are_idempotent`, still pass)

- [ ] **Step 6: Commit**

```bash
git add src/librarian/catalog/mod.rs
git commit -m "fix(librarian): set busy_timeout on disk-backed catalog connections"
```

---

### Task 2: `catalog::augmentation::append_entry` — atomic allocation + write

**Files:**
- Modify: `src/librarian/catalog/augmentation.rs` — add `use rusqlite::OptionalExtension;` to the top imports (currently `use crate::librarian::catalog::Catalog;` / `use crate::librarian::tools::{schema_validate, RecoverableError};` / `use anyhow::Result;` / `use serde_json::{json, Value};`), then add `append_entry` and `next_index` near `merge_params` (after line 139)
- Test: same file, `#[cfg(test)] mod tests` block (starts at line 271; reuses the existing `sample_art` and `aug` helpers already defined there)

**Interfaces:**
- Produces: `pub fn append_entry(cat: &mut Catalog, artifact_id: &str, entry_collection: &str, id_prefix: &str, entry: Value) -> Result<String>` — returns the assigned id (e.g. `"F-13"`) on success. Callers pass `entry` as a JSON object *without* an `id` field; the function sets `id` itself, overwriting any caller-supplied value.
- Consumes: `RecoverableError::new`/`with_hint`, `schema_validate::validate_against_stored(schema_text: &str, params: &Value) -> Result<()>` (both already imported in this file), `augmentation::get`/`upsert` (existing, used only by tests here).

- [ ] **Step 1: Write the failing tests**

Add to the `mod tests` block in `src/librarian/catalog/augmentation.rs` (after the existing `merge_params_accepts_valid` test):

```rust
#[test]
fn append_entry_assigns_first_id_to_empty_collection() {
    let mut cat = Catalog::open_in_memory().unwrap();
    art_upsert(&cat, &sample_art("art1")).unwrap();
    let mut a = aug("art1");
    a.entry_collection = Some("failures".to_string());
    a.params = r#"{"failures":[]}"#.to_string();
    upsert(&cat, &a).unwrap();

    let id = append_entry(&mut cat, "art1", "failures", "F", json!({"status": "fail"})).unwrap();

    assert_eq!(id, "F-1");
    let row = get(&cat, "art1").unwrap().unwrap();
    let params: Value = serde_json::from_str(&row.params).unwrap();
    assert_eq!(params["failures"][0]["id"], "F-1");
    assert_eq!(params["failures"][0]["status"], "fail");
}

#[test]
fn append_entry_computes_max_plus_one_across_non_contiguous_ids() {
    let mut cat = Catalog::open_in_memory().unwrap();
    art_upsert(&cat, &sample_art("art1")).unwrap();
    let mut a = aug("art1");
    a.entry_collection = Some("failures".to_string());
    a.params = r#"{"failures":[{"id":"F-1"},{"id":"F-3"},{"id":"F-9"}]}"#.to_string();
    upsert(&cat, &a).unwrap();

    let id = append_entry(&mut cat, "art1", "failures", "F", json!({})).unwrap();
    assert_eq!(id, "F-10");
}

#[test]
fn append_entry_rejects_unknown_entry_collection() {
    let mut cat = Catalog::open_in_memory().unwrap();
    art_upsert(&cat, &sample_art("art1")).unwrap();
    let mut a = aug("art1");
    a.entry_collection = Some("failures".to_string());
    a.params = r#"{"failures":[]}"#.to_string();
    upsert(&cat, &a).unwrap();

    let err = append_entry(&mut cat, "art1", "bugs", "B", json!({})).unwrap_err();
    assert!(err.to_string().contains("failures"));
}

#[test]
fn append_entry_rejects_missing_augmentation() {
    let mut cat = Catalog::open_in_memory().unwrap();
    art_upsert(&cat, &sample_art("art1")).unwrap();

    let err = append_entry(&mut cat, "art1", "failures", "F", json!({})).unwrap_err();
    assert!(err.to_string().contains("no augmentation"));
}

#[test]
fn append_entry_rejects_schema_violation_without_writing() {
    let mut cat = Catalog::open_in_memory().unwrap();
    art_upsert(&cat, &sample_art("art1")).unwrap();
    let mut a = aug("art1");
    a.entry_collection = Some("failures".to_string());
    a.params = r#"{"failures":[{"id":"F-1","status":"fail"}]}"#.to_string();
    a.params_schema = Some(
        json!({
            "type": "object",
            "properties": {
                "failures": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["id", "status"],
                        "properties": {
                            "status": {"type": "string", "enum": ["fail", "pass"]}
                        }
                    }
                }
            }
        })
        .to_string(),
    );
    upsert(&cat, &a).unwrap();

    let err = append_entry(&mut cat, "art1", "failures", "F", json!({"status": "bogus"})).unwrap_err();
    assert!(err.to_string().contains("params_schema"));

    // Rolled back: still exactly the one original entry.
    let row = get(&cat, "art1").unwrap().unwrap();
    let params: Value = serde_json::from_str(&row.params).unwrap();
    assert_eq!(params["failures"].as_array().unwrap().len(), 1);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test append_entry`
Expected: FAIL to compile — `append_entry` is not yet defined.

- [ ] **Step 3: Implement `append_entry` and its `next_index` helper**

Add `use rusqlite::OptionalExtension;` to the imports at the top of `src/librarian/catalog/augmentation.rs`. Then add, after `merge_params` (line 139):

```rust
/// Atomically assigns the next `<id_prefix>-N` id and appends `entry` to
/// `params.<entry_collection>`. Runs inside a single `IMMEDIATE` transaction
/// so the read-max-write is safe under both intra-process and cross-process
/// concurrency (paired with `busy_timeout` set on the connection).
pub fn append_entry(
    cat: &mut Catalog,
    artifact_id: &str,
    entry_collection: &str,
    id_prefix: &str,
    mut entry: Value,
) -> Result<String> {
    let tx = cat
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

    let row: Option<(String, Option<String>, Option<String>)> = tx
        .query_row(
            "SELECT params, params_schema, entry_collection
             FROM artifact_augmentation WHERE artifact_id = ?1",
            [artifact_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;

    let Some((params_text, params_schema, declared_collection)) = row else {
        return Err(RecoverableError::new(format!(
            "append_entry: artifact `{artifact_id}` has no augmentation — augment it with an entry_collection first"
        )));
    };

    if declared_collection.as_deref() != Some(entry_collection) {
        return Err(RecoverableError::with_hint(
            format!("append_entry: `{entry_collection}` is not this artifact's entry_collection"),
            match declared_collection {
                Some(c) => format!("This artifact's entry_collection is `{c}` — pass that instead."),
                None => "This artifact has no entry_collection declared — set one via artifact_augment first.".to_string(),
            },
        ));
    }

    let mut params: Value = serde_json::from_str(&params_text).unwrap_or_else(|_| json!({}));
    let existing_ids: Vec<String> = params
        .get(entry_collection)
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|e| e.get("id").and_then(|v| v.as_str()).map(String::from))
        .collect();
    let new_id = format!("{id_prefix}-{}", next_index(&existing_ids, id_prefix));

    if let Some(obj) = entry.as_object_mut() {
        obj.insert("id".to_string(), json!(new_id));
    }

    match params.get_mut(entry_collection) {
        Some(Value::Array(arr)) => arr.push(entry),
        _ => {
            params[entry_collection] = json!([entry]);
        }
    }

    if let Some(schema_text) = params_schema.as_deref() {
        schema_validate::validate_against_stored(schema_text, &params).map_err(|e| {
            RecoverableError::new(format!("append_entry: new entry violates params_schema: {e}"))
        })?;
    }

    let new_params_text = serde_json::to_string(&params)?;
    tx.execute(
        "UPDATE artifact_augmentation SET params = ?1,
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE artifact_id = ?2",
        rusqlite::params![new_params_text, artifact_id],
    )?;
    tx.commit()?;
    Ok(new_id)
}

fn next_index(existing_ids: &[String], id_prefix: &str) -> u64 {
    let re = regex::Regex::new(&format!(r"^{}-(\d+)$", regex::escape(id_prefix))).unwrap();
    existing_ids
        .iter()
        .filter_map(|id| re.captures(id))
        .filter_map(|c| c[1].parse::<u64>().ok())
        .max()
        .map(|m| m + 1)
        .unwrap_or(1)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test append_entry`
Expected: PASS (all five tests from Step 1)

- [ ] **Step 5: Commit**

```bash
git add src/librarian/catalog/augmentation.rs
git commit -m "feat(librarian): add atomic append_entry allocation primitive"
```

---

### Task 3: Cross-process concurrency proof

**Why a separate task:** Task 2's tests all use one `Catalog` instance — they prove the logic is *correct*, not that it's *safe under concurrency*. The actual bug this spec fixes (backend-kotlin's SI-N collisions) came from **separate OS processes** racing, which the in-process `Arc<Mutex<Catalog>>` used by the MCP server can't protect against on its own. This test opens two independent `Connection`s to the same on-disk file — bypassing that mutex entirely — so it only passes because of Task 1's `busy_timeout` + Task 2's `IMMEDIATE` transaction.

**Files:**
- Test: `src/librarian/catalog/augmentation.rs` (same `mod tests` block)

**Interfaces:** None new — exercises `append_entry` and `Catalog::open` from Task 1/2.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn append_entry_serializes_across_independent_connections_to_same_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cat.sqlite");

    {
        let cat = Catalog::open(&path).unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        let mut a = aug("art1");
        a.entry_collection = Some("failures".to_string());
        a.params = r#"{"failures":[]}"#.to_string();
        upsert(&cat, &a).unwrap();
    }

    let path1 = path.clone();
    let path2 = path.clone();
    let h1 = std::thread::spawn(move || {
        let mut cat = Catalog::open(&path1).unwrap();
        append_entry(&mut cat, "art1", "failures", "F", json!({"who": "one"})).unwrap()
    });
    let h2 = std::thread::spawn(move || {
        let mut cat = Catalog::open(&path2).unwrap();
        append_entry(&mut cat, "art1", "failures", "F", json!({"who": "two"})).unwrap()
    });

    let id1 = h1.join().unwrap();
    let id2 = h2.join().unwrap();

    assert_ne!(id1, id2, "concurrent appends from independent connections must not collide");
    let mut ids = vec![id1, id2];
    ids.sort();
    assert_eq!(ids, vec!["F-1".to_string(), "F-2".to_string()]);

    let cat = Catalog::open(&path).unwrap();
    let row = get(&cat, "art1").unwrap().unwrap();
    let params: Value = serde_json::from_str(&row.params).unwrap();
    assert_eq!(params["failures"].as_array().unwrap().len(), 2);
}
```

- [ ] **Step 2: Run test to verify it currently passes or fails**

Run: `cargo test append_entry_serializes_across_independent_connections_to_same_file`
Expected: PASS already, since Tasks 1 and 2 are both in place. If it instead fails or hangs, that means `busy_timeout` (Task 1) or the `IMMEDIATE` transaction behavior (Task 2) isn't taking effect — stop and re-check those before proceeding; do not weaken this test to make it pass.

- [ ] **Step 3: Commit**

```bash
git add src/librarian/catalog/augmentation.rs
git commit -m "test(librarian): prove append_entry serializes across independent connections"
```

---

### Task 4: Wire `append_entry` into the `artifact` MCP tool

**Files:**
- Create: `src/librarian/tools/append_entry.rs`
- Modify: `src/librarian/tools/mod.rs` — add `pub mod append_entry;` alongside the other action modules (e.g. after line 165's `pub mod mv;`)
- Modify: `src/librarian/tools/artifact.rs` — `input_schema()` (enum + new properties) and `call()` (dispatch arm + both error messages)
- Test: `src/librarian/tools/append_entry.rs` (new `#[cfg(test)] mod tests`, following the local `mk_ctx()` pattern used in `artifact_event.rs`/`artifact_refresh.rs`)

**Interfaces:**
- Consumes: `catalog::augmentation::append_entry` (Task 2), `ToolContext.catalog: Arc<parking_lot::Mutex<Catalog>>`, `RecoverableError`.
- Produces: `pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value>` returning `{"id": "<prefix>-<n>"}` — mirrors `update::call`'s signature so `artifact.rs` dispatches to it the same way as every other action.

- [ ] **Step 1: Write the failing tests**

Create `src/librarian/tools/append_entry.rs`:

```rust
use super::{RecoverableError, ToolContext};
use crate::librarian::catalog::augmentation;
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct Args {
    id: String,
    entry_collection: String,
    id_prefix: String,
    #[serde(default = "default_entry")]
    entry: Value,
}

fn default_entry() -> Value {
    json!({})
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args)?;
    if !a.entry.is_object() {
        return Err(RecoverableError::new(
            "append_entry: `entry` must be a JSON object",
        ));
    }
    let mut cat = ctx.catalog.lock();
    let id = augmentation::append_entry(&mut cat, &a.id, &a.entry_collection, &a.id_prefix, a.entry)?;
    Ok(json!({"id": id}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{upsert as art_upsert, ArtifactRow};
    use crate::librarian::catalog::augmentation::{upsert as aug_upsert, AugmentationRow};
    use crate::librarian::catalog::Catalog;
    use crate::librarian::workspace::WorkspaceConfig;
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
            artifact_store: None,
            current_project: None,
        }
    }

    fn seed(ctx: &ToolContext, id: &str) {
        let now = chrono::Utc::now().timestamp_millis();
        let cat = ctx.catalog.lock();
        art_upsert(
            &cat,
            &ArtifactRow {
                id: id.to_string(),
                abs_path: std::path::PathBuf::from(format!("/test/{id}.md")),
                kind: "tracker".to_string(),
                status: "active".to_string(),
                title: Some("T".to_string()),
                owners: vec![],
                tags: vec![],
                topic: None,
                time_scope: None,
                source: None,
                created_at: now,
                updated_at: now,
                file_mtime: now,
                file_sha256: "x".to_string(),
                confidence: 1.0,
            },
        )
        .unwrap();
        aug_upsert(
            &cat,
            &AugmentationRow {
                artifact_id: id.to_string(),
                prompt: "test".to_string(),
                params: r#"{"failures":[]}"#.to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: Some("failures".to_string()),
                refreshed_at_commit: None,
            },
        )
        .unwrap();
    }

    #[tokio::test]
    async fn call_assigns_and_returns_next_id() {
        let ctx = mk_ctx();
        seed(&ctx, "art1");

        let result = call(
            &ctx,
            json!({
                "id": "art1",
                "entry_collection": "failures",
                "id_prefix": "F",
                "entry": {"status": "fail"}
            }),
        )
        .await
        .unwrap();

        assert_eq!(result["id"], "F-1");
    }

    #[tokio::test]
    async fn call_rejects_non_object_entry() {
        let ctx = mk_ctx();
        seed(&ctx, "art1");

        let err = call(
            &ctx,
            json!({
                "id": "art1",
                "entry_collection": "failures",
                "id_prefix": "F",
                "entry": "not an object"
            }),
        )
        .await
        .unwrap_err();

        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }

    #[tokio::test]
    async fn call_missing_artifact_returns_recoverable_error() {
        let ctx = mk_ctx();

        let err = call(
            &ctx,
            json!({
                "id": "nope",
                "entry_collection": "failures",
                "id_prefix": "F",
                "entry": {}
            }),
        )
        .await
        .unwrap_err();

        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test append_entry::tests`
Expected: FAIL to compile — `append_entry` module doesn't exist yet / isn't wired into `mod.rs`.

- [ ] **Step 3: Register the module**

In `src/librarian/tools/mod.rs`, add after line 165 (`pub mod mv;`):

```rust
pub mod append_entry;
```

- [ ] **Step 4: Wire dispatch and schema into `artifact.rs`**

In `src/librarian/tools/artifact.rs`, change the `action` enum (in `input_schema()`):

```rust
"enum": ["find", "get", "create", "update", "move", "delete", "link", "graph", "state_at"],
```
to:
```rust
"enum": ["find", "get", "create", "update", "move", "delete", "link", "graph", "state_at", "append_entry"],
```

Add three new properties to `input_schema()`'s `"properties"` object, right after the existing `"timestamp"` property (the last one, immediately before the object's closing braces):

```rust
"entry_collection": {
    "type": "string",
    "description": "append_entry: the augmentation's entry_collection array to append into (must match the artifact's declared entry_collection)"
},
"id_prefix": {
    "type": "string",
    "description": "append_entry: id prefix — the assigned id is `<id_prefix>-<next integer>`, computed from the live max across existing entries"
},
"entry": {
    "type": "object",
    "description": "append_entry: the new entry's fields, excluding `id` — the server assigns and overwrites `id`"
}
```

Also extend the existing `"id"` property's description (it currently reads `"get/update/graph: artifact id"`) to:
```rust
"id": {
    "type": "string",
    "description": "get/update/graph/append_entry: artifact id"
},
```

Change the "action required" error:
```rust
"action required — one of: find, get, create, update, move, link, graph, state_at",
```
to:
```rust
"action required — one of: find, get, create, update, move, link, graph, state_at, append_entry",
```

Change the dispatch `match` to add a new arm (alongside the others, e.g. after `"state_at" => super::state_at::call(ctx, args).await,`):
```rust
"append_entry" => super::append_entry::call(ctx, args).await,
```

Change the "unknown action" error:
```rust
other => Err(RecoverableError::new(format!(
    "unknown action '{other}' — expected one of: find, get, create, update, move, delete, link, graph, state_at"
))),
```
to:
```rust
other => Err(RecoverableError::new(format!(
    "unknown action '{other}' — expected one of: find, get, create, update, move, delete, link, graph, state_at, append_entry"
))),
```

Also update the `description()` string (currently `"Artifact CRUD and query. action: find | get | create | update | move | delete | link | graph | state_at. ..."`) to add `| append_entry` after `state_at`, and one clause noting it exists, e.g. append `" append_entry atomically assigns the next id and appends to a tracker's entry_collection — use it instead of a manual read-then-write for any monotonic-ID tracker (F-N, W-N, T-N, ...)."`

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test append_entry`
Expected: PASS (catalog-layer tests from Tasks 2–3, plus this task's three tool-layer tests)

- [ ] **Step 6: Run the full verification gate**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all three pass with no warnings or failures.

- [ ] **Step 7: Commit**

```bash
git add src/librarian/tools/append_entry.rs src/librarian/tools/mod.rs src/librarian/tools/artifact.rs
git commit -m "feat(librarian): expose append_entry as a new artifact action"
```

---

## Self-Review Notes

- **Spec coverage:** the spec's `artifact(action="append_entry")` primitive (Task 2/4), its two named error cases — unknown `entry_collection` and schema-violation-before-write (Task 2) — and its three named tests — concurrent-append race (Task 3), prefix/schema-mismatch-before-write (Task 2), non-contiguous-id-max (Task 2) — are each covered by a task above. The spec's response shape example (`{id, entries}`) is intentionally narrowed to `{id}` only in Task 4, per the project's "write tools return minimal new info, never echo content back" convention (memory `conventions`) — the caller can already fetch the full array via `artifact(get, entry_filter=...)` if needed.
- **Non-goals respected:** no changes to prose-only trackers or `body_edits` in this plan, matching the spec's explicit deferral.
