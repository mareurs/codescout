# Filterable Trackers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add per-entry metadata filtering to a single tracker — `artifact(get, id, entry_filter={…})` returns the matching rows from the tracker's declared entry collection, reusing the existing `{field:{op:value}}` filter AST.

**Architecture:** A tracker declares an `entry_collection` (a params key) on its augmentation. A new in-memory `eval(&FilterNode, &Map)` evaluator (sibling to the SQL-only `compile()`) filters the objects in that collection. `tracker_design` teaches the convention; a retrofit guide converts prose trackers. Single-tracker only; cross-tracker search is a documented future graduation.

**Tech Stack:** Rust, rusqlite (SQLite), serde_json, async-trait, tokio (tests). MCP server.

**Spec:** `docs/superpowers/specs/2026-05-28-filterable-trackers-design.md`
**Session log:** `docs/trackers/metadata-filtering-session-log.md` (F-1, F-2)

---

## File Structure

| File | Responsibility | Change |
|---|---|---|
| `src/librarian/catalog/mod.rs` | migration runner (`run_migrations`) | add v7 guarded `ALTER TABLE … ADD COLUMN entry_collection` |
| `src/librarian/catalog/augmentation.rs` | `AugmentationRow` + storage (`upsert`/`row_from_sql`/`get`/`get_batch`) | add `entry_collection` field + column round-trip |
| `src/librarian/tools/augment.rs` | `artifact_augment` tool | accept + persist `entry_collection`; update description/schema |
| `src/librarian/filter.rs` | filter AST + engines | add in-memory `eval` + helpers + consistency test |
| `src/librarian/tools/get.rs` | `artifact(get)` handler | add `entry_filter` param + entry-filtering block |
| `src/librarian/tools/tracker_design.rs` | tracker teaching tool | SYSTEM_PROMPT step + `entry_collection` in archetype example |
| `docs/conventions/retrofitting-trackers-for-filtering.md` | retrofit guide | new doc |

**Note on test commands:** `cargo test --lib <name>` runs an in-crate unit/`tokio::test`. The repo memory `cargo-test-lib-skips-integration` notes plain `cargo test` skips the integration tier — `--lib` is correct for everything in this plan.

---

## Task 1: Storage — add the `entry_collection` column

These changes must land together (the struct, its SQL, and the migration compile as a unit). TDD: a round-trip test through `upsert` → `get`.

**Files:**
- Modify: `src/librarian/catalog/mod.rs` (`run_migrations`, 63-108)
- Modify: `src/librarian/catalog/augmentation.rs` (`AugmentationRow` 6-24; `upsert` 26-56; `row_from_sql` 69-83; `get` 58-67; `get_batch` 145-173)
- Test: `src/librarian/catalog/augmentation.rs` tests module (236-475)

- [ ] **Step 1: Write the failing round-trip test**

Add to the `tests` module in `src/librarian/catalog/augmentation.rs`. Mirror the fixture setup an existing test in this module uses (a `Catalog` + a seeded `artifact` row for the FK):

```rust
#[test]
fn entry_collection_round_trips() {
    let cat = test_catalog();            // existing module fixture
    seed_artifact(&cat, "ec-art");       // existing module helper (FK target)
    upsert(
        &cat,
        &AugmentationRow {
            artifact_id: "ec-art".into(),
            prompt: "p".into(),
            params: "{}".into(),
            last_refreshed_at: None,
            refresh_count: 0,
            created_at: "2026-05-28T00:00:00.000Z".into(),
            updated_at: "2026-05-28T00:00:00.000Z".into(),
            render_template: None,
            params_schema: None,
            append_mode: false,
            history_cap: None,
            entry_collection: Some("failures".into()),
        },
    )
    .unwrap();
    let got = get(&cat, "ec-art").unwrap().unwrap();
    assert_eq!(got.entry_collection.as_deref(), Some("failures"));
}
```

(If this module's fixture/seed helpers have different names, use the ones already present — every test in the module constructs a catalog the same way.)

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --lib entry_collection_round_trips`
Expected: FAIL — compile error, `AugmentationRow` has no field `entry_collection`.

- [ ] **Step 3: Add the field to `AugmentationRow`**

In `src/librarian/catalog/augmentation.rs`, append to the struct (after `history_cap`):

```rust
    /// Names the params array whose objects are the tracker's filterable
    /// entry rows (e.g. "failures", "children"). None = not entry-filterable.
    pub entry_collection: Option<String>,
```

- [ ] **Step 4: Add the migration**

In `src/librarian/catalog/mod.rs`, inside `run_migrations`, immediately after the v5 `history_cap` block and before the `migrate_v6::add_columns(conn)?;` call:

```rust
    // v7: entry_collection column on artifact_augmentation (filterable trackers)
    if !column_exists(conn, "artifact_augmentation", "entry_collection")? {
        conn.execute(
            "ALTER TABLE artifact_augmentation ADD COLUMN entry_collection TEXT",
            [],
        )?;
    }
```

- [ ] **Step 5: Thread the column through `upsert`**

Replace the SQL + params in `upsert` (`src/librarian/catalog/augmentation.rs`):

```rust
    cat.conn.execute(
        "INSERT INTO artifact_augmentation
           (artifact_id, prompt, params, last_refreshed_at, refresh_count,
            created_at, updated_at, render_template, params_schema,
            append_mode, history_cap, entry_collection)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(artifact_id) DO UPDATE SET
           prompt = excluded.prompt,
           params = excluded.params,
           render_template = excluded.render_template,
           params_schema = excluded.params_schema,
           append_mode = excluded.append_mode,
           history_cap = excluded.history_cap,
           entry_collection = excluded.entry_collection,
           updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        rusqlite::params![
            row.artifact_id,
            row.prompt,
            row.params,
            row.last_refreshed_at,
            row.refresh_count,
            row.created_at,
            row.updated_at,
            row.render_template,
            row.params_schema,
            row.append_mode as i64,
            row.history_cap,
            row.entry_collection,
        ],
    )?;
```

- [ ] **Step 6: Read the column back in `row_from_sql`**

In `row_from_sql`, add after `history_cap: row.get(10)?,`:

```rust
        entry_collection: row.get(11)?,
```

- [ ] **Step 7: Add the column to both SELECTs**

In `get` and in `get_batch`, change the column list so the SELECT order matches `row_from_sql` indices — append `, entry_collection` after `history_cap`:

```sql
SELECT artifact_id, prompt, params, last_refreshed_at, refresh_count,
       created_at, updated_at, render_template, params_schema,
       append_mode, history_cap, entry_collection
FROM artifact_augmentation WHERE …
```

(Both `get` (58-67) and `get_batch` (145-173) have this identical column list — change both. `list_stale` builds a different `StaleEntry` struct and is unaffected.)

- [ ] **Step 8: Fix any other `AugmentationRow { … }` literals the compiler flags**

Run `cargo build --lib` and add `entry_collection: None,` to every `AugmentationRow` struct literal the compiler reports as missing the field (notably `src/librarian/tools/augment.rs` construction site ~272-284 and any test fixtures). Leave the value `None` except where a test intends otherwise.

- [ ] **Step 9: Run the test to verify it passes**

Run: `cargo test --lib entry_collection_round_trips`
Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add src/librarian/catalog/mod.rs src/librarian/catalog/augmentation.rs src/librarian/tools/augment.rs
git commit -m "feat(librarian): add entry_collection column to artifact_augmentation"
```

---

## Task 2: Expose `entry_collection` on `artifact_augment`

**Files:**
- Modify: `src/librarian/tools/augment.rs` (`Args` 11-27; row construction ~272-284; `description`/`input_schema`)
- Test: `src/librarian/tools/augment.rs` tests module

- [ ] **Step 1: Write the failing test**

Mirror `persists_render_template_and_params_schema` (same module, ~392) which uses `mk_ctx()` + `seed_artifact(&ctx, …)`:

```rust
#[tokio::test]
async fn persists_entry_collection() {
    let ctx = mk_ctx();
    seed_artifact(&ctx, "ec-tool");
    ArtifactAugment
        .call_content(
            &ctx,
            json!({
                "id": "ec-tool",
                "prompt": "maintain the failures list",
                "params": { "failures": [] },
                "entry_collection": "failures"
            }),
        )
        .await
        .unwrap();
    let row = {
        let cat = ctx.catalog.lock();
        augmentation::get(&cat, "ec-tool").unwrap().unwrap()
    };
    assert_eq!(row.entry_collection.as_deref(), Some("failures"));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib persists_entry_collection`
Expected: FAIL — `Args` has no `entry_collection`; the value is never persisted.

- [ ] **Step 3: Add the field to `Args`**

In `src/librarian/tools/augment.rs`, append to `Args` (after `history_cap`):

```rust
    #[serde(default)]
    entry_collection: Option<String>,
```

- [ ] **Step 4: Set it on the constructed `AugmentationRow`**

In the `augmentation::upsert(&cat, &augmentation::AugmentationRow { … })` literal (~272-284), add:

```rust
                entry_collection: a.entry_collection,
```

- [ ] **Step 5: Document it in `input_schema` and `description`**

In `input_schema`, add a property alongside `history_cap`:

```rust
                "entry_collection": {
                    "type": "string",
                    "description": "Names the params array whose objects are this tracker's filterable entry rows (e.g. \"failures\"). Enables artifact(get, entry_filter=...). On merge=false this field is overwritten with the call's value (None if omitted) — pass the existing value back to preserve it."
                },
```

In `description`, change "ALL six caller-controlled fields — prompt, params, render_template, params_schema, append_mode, history_cap" to "ALL seven caller-controlled fields — prompt, params, render_template, params_schema, append_mode, history_cap, entry_collection" and "leaves the other five fields untouched" → "the other six fields untouched".

- [ ] **Step 6: Run to verify it passes**

Run: `cargo test --lib persists_entry_collection`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/librarian/tools/augment.rs
git commit -m "feat(librarian): accept entry_collection on artifact_augment"
```

---

## Task 3: The in-memory `eval` engine

A sibling to `compile()` that evaluates the AST against one entry object. No `ALLOWED_FIELDS` gate (no SQL-injection surface in-memory; entry fields are arbitrary). Op semantics mirror `compile_leaf`.

**Files:**
- Modify: `src/librarian/filter.rs` (add `eval`, `eval_leaf`, `json_eq`, `json_cmp` + `use std::cmp::Ordering;`)
- Test: `src/librarian/filter.rs` tests module (274-381)

- [ ] **Step 1: Write failing per-op unit tests**

Add to the `tests` module in `src/librarian/filter.rs` (it already has a `parse` helper that builds a `FilterNode` from `json!`):

```rust
fn entry(json: Value) -> serde_json::Map<String, Value> {
    json.as_object().unwrap().clone()
}

#[test]
fn eval_eq_and_missing_field() {
    let e = entry(json!({"status": "open", "priority": 2}));
    assert!(eval(&parse(json!({"status": {"eq": "open"}})), &e).unwrap());
    assert!(!eval(&parse(json!({"status": {"eq": "done"}})), &e).unwrap());
    // missing field never matches
    assert!(!eval(&parse(json!({"owner": {"eq": "x"}})), &e).unwrap());
}

#[test]
fn eval_in_gt_contains_prefix() {
    let e = entry(json!({"cat": "hardware", "priority": 2, "tags": ["gpu", "thermal"]}));
    assert!(eval(&parse(json!({"cat": {"in": ["hardware", "software"]}})), &e).unwrap());
    assert!(eval(&parse(json!({"priority": {"gt": 1}})), &e).unwrap());
    assert!(!eval(&parse(json!({"priority": {"lt": 1}})), &e).unwrap());
    assert!(eval(&parse(json!({"cat": {"contains": "hard"}})), &e).unwrap());      // string substring
    assert!(eval(&parse(json!({"tags": {"contains": "gpu"}})), &e).unwrap());      // array membership
    assert!(eval(&parse(json!({"cat": {"prefix": "hard"}})), &e).unwrap());
}

#[test]
fn eval_and_or_not() {
    let e = entry(json!({"cat": "hardware", "status": "open"}));
    assert!(eval(
        &parse(json!({"and": [{"cat": {"eq": "hardware"}}, {"status": {"eq": "open"}}]})),
        &e
    )
    .unwrap());
    assert!(eval(&parse(json!({"not": {"status": {"eq": "done"}}})), &e).unwrap());
    assert!(!eval(
        &parse(json!({"or": [{"cat": {"eq": "software"}}, {"status": {"eq": "done"}}]})),
        &e
    )
    .unwrap());
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --lib eval_`
Expected: FAIL — `eval` not found.

- [ ] **Step 3: Implement `eval` + helpers**

Add to `src/librarian/filter.rs` (top: `use std::cmp::Ordering;`). Place after `compile_leaf`:

```rust
/// Evaluate a filter AST against one entry object, in memory.
///
/// Sibling to `compile` (which emits SQL). No `ALLOWED_FIELDS` gate —
/// matching a JSON map has no injection surface and entry fields are
/// arbitrary by design. Op semantics mirror `compile_leaf`.
pub fn eval(node: &FilterNode, entry: &serde_json::Map<String, Value>) -> Result<bool> {
    match node {
        FilterNode::And { and } => {
            if and.is_empty() {
                return Err(empty_composition_err("and"));
            }
            for c in and {
                if !eval(c, entry)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        FilterNode::Or { or } => {
            if or.is_empty() {
                return Err(empty_composition_err("or"));
            }
            for c in or {
                if eval(c, entry)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        FilterNode::Not { not } => Ok(!eval(not, entry)?),
        FilterNode::Leaf(map) => eval_leaf(map, entry),
    }
}

fn empty_composition_err(op: &str) -> anyhow::Error {
    RecoverableError::with_hint(
        format!("empty composition `{op}`"),
        "`and` / `or` / `not` require at least one child filter.",
    )
    .into()
}

fn eval_leaf(
    map: &serde_json::Map<String, Value>,
    entry: &serde_json::Map<String, Value>,
) -> Result<bool> {
    if map.len() != 1 {
        return Err(RecoverableError::with_hint(
            format!("leaf must have exactly one field, got {}", map.len()),
            "Each leaf has shape `{field: {op: value}}`. Wrap multiple fields with `and`/`or`.",
        )
        .into());
    }
    let (field, ops) = map.iter().next().unwrap();
    let ops = ops.as_object().ok_or_else(|| {
        RecoverableError::with_hint(
            "ops must be an object",
            "Leaf op shape is `{field: {op: value}}`.",
        )
    })?;
    if ops.len() != 1 {
        return Err(RecoverableError::with_hint(
            format!("exactly one op per leaf, got {}", ops.len()),
            "Wrap multiple ops on the same field with `and`/`or`.",
        )
        .into());
    }
    let (op_name, value) = ops.iter().next().unwrap();
    let op = op_name.parse::<LeafOp>().map_err(|_| {
        RecoverableError::with_hint(
            format!("unknown op `{op_name}`"),
            "valid ops: eq, ne, in, nin, gt, lt, gte, lte, contains, prefix",
        )
    })?;

    // Missing field never matches (mirrors SQL NULL comparison semantics).
    let Some(actual) = entry.get(field) else {
        return Ok(false);
    };

    Ok(match op {
        LeafOp::Eq => json_eq(actual, value),
        LeafOp::Ne => !json_eq(actual, value),
        LeafOp::In | LeafOp::Nin => {
            let arr = value.as_array().ok_or_else(|| {
                RecoverableError::with_hint(
                    "`in`/`nin` expects an array",
                    "Provide a JSON array, e.g. `{\"in\": [\"a\", \"b\"]}`.",
                )
            })?;
            let hit = arr.iter().any(|v| json_eq(actual, v));
            if op == LeafOp::In {
                hit
            } else {
                !hit
            }
        }
        LeafOp::Gt => json_cmp(actual, value) == Some(Ordering::Greater),
        LeafOp::Lt => json_cmp(actual, value) == Some(Ordering::Less),
        LeafOp::Gte => matches!(json_cmp(actual, value), Some(Ordering::Greater | Ordering::Equal)),
        LeafOp::Lte => matches!(json_cmp(actual, value), Some(Ordering::Less | Ordering::Equal)),
        LeafOp::Contains => match actual {
            // scalar string → substring (mirrors SQL LIKE %v%)
            Value::String(s) => {
                let needle = value.as_str().ok_or_else(|| {
                    RecoverableError::with_hint(
                        "`contains` on a string field expects a string value",
                        "Provide a string, e.g. `{\"contains\": \"foo\"}`.",
                    )
                })?;
                s.contains(needle)
            }
            // array field → membership (mirrors SQL json_each on tags/owners)
            Value::Array(items) => items.iter().any(|v| json_eq(v, value)),
            _ => false,
        },
        LeafOp::Prefix => {
            let pfx = value.as_str().ok_or_else(|| {
                RecoverableError::with_hint(
                    "`prefix` expects a string",
                    "Provide a string value, e.g. `{\"prefix\": \"docs/\"}`.",
                )
            })?;
            actual.as_str().map(|s| s.starts_with(pfx)).unwrap_or(false)
        }
    })
}

/// JSON value equality with numeric coercion (`2` == `2.0`).
fn json_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            matches!((x.as_f64(), y.as_f64()), (Some(p), Some(q)) if p == q)
        }
        _ => a == b,
    }
}

/// Ordering for gt/lt/gte/lte: numbers numerically, strings lexically.
/// Mismatched or non-orderable types → None (treated as non-match).
fn json_cmp(a: &Value, b: &Value) -> Option<Ordering> {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64()?.partial_cmp(&y.as_f64()?),
        (Value::String(x), Value::String(y)) => Some(x.cmp(y)),
        _ => None,
    }
}
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test --lib eval_`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/librarian/filter.rs
git commit -m "feat(librarian): in-memory eval() for entry-grain filtering"
```

---

## Task 4: Dual-engine consistency test (the F-1 drift guard)

Proves `compile()`→SQL and `eval()` agree, so the two engines can't silently diverge.

**Files:**
- Test: `src/librarian/filter.rs` tests module

- [ ] **Step 1: Write the consistency test**

Add to the `tests` module. It loads the same rows into an in-memory SQLite table and as JSON maps, then asserts both engines select the same ids for a battery of filters:

```rust
#[test]
fn eval_matches_compile_on_fixture() {
    use rusqlite::Connection;

    // Fixture rows (entry objects). Columns mirror the JSON fields.
    let rows = [
        json!({"id": "a", "status": "open", "priority": 1}),
        json!({"id": "b", "status": "done", "priority": 3}),
        json!({"id": "c", "status": "open", "priority": 5}),
    ];

    // --- SQL side: a temp table with the same columns ---
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE e (id TEXT, status TEXT, priority INTEGER)",
        [],
    )
    .unwrap();
    for r in &rows {
        conn.execute(
            "INSERT INTO e (id, status, priority) VALUES (?1, ?2, ?3)",
            rusqlite::params![
                r["id"].as_str().unwrap(),
                r["status"].as_str().unwrap(),
                r["priority"].as_i64().unwrap(),
            ],
        )
        .unwrap();
    }

    // Filters expressible against allowlisted-shaped columns. (We reuse the
    // AST; `status`/`priority` stand in as columns for the SQL side.)
    // NOTE: compile() enforces ALLOWED_FIELDS, so this fixture deliberately
    // uses the column names the SQL side declares; the point is op-semantics
    // parity, not field-name parity.
    for filter_json in [
        json!({"status": {"eq": "open"}}),
        json!({"priority": {"gt": 2}}),
        json!({"priority": {"gte": 3}}),
        json!({"and": [{"status": {"eq": "open"}}, {"priority": {"lt": 5}}]}),
        json!({"not": {"status": {"eq": "done"}}}),
        json!({"status": {"in": ["open", "flaky"]}}),
    ] {
        let node = parse(filter_json.clone());

        // eval side
        let mut eval_ids: Vec<String> = rows
            .iter()
            .filter(|r| eval(&node, r.as_object().unwrap()).unwrap())
            .map(|r| r["id"].as_str().unwrap().to_string())
            .collect();
        eval_ids.sort();

        // compile side
        let frag = compile(&node).unwrap();
        let sql = format!("SELECT id FROM e WHERE {} ORDER BY id", frag.sql);
        let mut stmt = conn.prepare(&sql).unwrap();
        let sql_ids: Vec<String> = stmt
            .query_map(rusqlite::params_from_iter(frag.params.iter()), |row| {
                row.get::<_, String>(0)
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(eval_ids, sql_ids, "engine disagreement on filter {filter_json}");
    }
}
```

> If `status`/`priority` are not in `ALLOWED_FIELDS`, the SQL side of this test will error on `compile`. In that case, change the fixture's field names to allowlisted columns (`kind`, `status`, `confidence`) so `compile` accepts them — the test's job is op-semantics parity, not field-name coverage. (`status` is allowlisted per `filter.rs:75-89`; use `confidence` in place of `priority` for the numeric column.)

- [ ] **Step 2: Run to verify it passes**

Run: `cargo test --lib eval_matches_compile_on_fixture`
Expected: PASS. (If it fails on a specific op, the §5a semantics table in the spec is canonical — fix `eval`, not the test.)

- [ ] **Step 3: Commit**

```bash
git add src/librarian/filter.rs
git commit -m "test(librarian): dual-engine consistency between compile() and eval()"
```

---

## Task 5: `entry_filter` on `artifact(get)`

**Files:**
- Modify: `src/librarian/tools/get.rs` (`Args` 70-92; `call` — insert before `out["augmentation"]` at ~241)
- Test: `src/librarian/tools/get.rs` tests module

- [ ] **Step 1: Write failing integration tests**

Add to the `tests` module in `src/librarian/tools/get.rs` (use the module's existing ctx/seed helpers; augment via `ArtifactAugment` as Task 2's test does — add `use crate::librarian::tools::augment::ArtifactAugment;` and ensure `use crate::librarian::catalog::augmentation;` are in scope in the test module):

```rust
#[tokio::test]
async fn entry_filter_returns_matching_rows() {
    let ctx = mk_ctx();
    seed_artifact(&ctx, "roadmap");
    ArtifactAugment
        .call_content(&ctx, json!({
            "id": "roadmap",
            "prompt": "maintain items",
            "params": { "items": [
                {"id": "R-1", "category": "hardware", "status": "open"},
                {"id": "R-2", "category": "software", "status": "open"},
                {"id": "R-3", "category": "hardware", "status": "done"}
            ]},
            "entry_collection": "items"
        }))
        .await
        .unwrap();

    let out = super::call(&ctx, json!({
        "id": "roadmap",
        "entry_filter": {"and": [
            {"category": {"eq": "hardware"}},
            {"status": {"eq": "open"}}
        ]}
    }))
    .await
    .unwrap();

    let entries = out["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["id"], "R-1");
    assert_eq!(out["entry_total"], 3);
}

#[tokio::test]
async fn entry_filter_on_non_augmented_is_recoverable_error() {
    let ctx = mk_ctx();
    seed_artifact(&ctx, "plain");
    let err = super::call(&ctx, json!({
        "id": "plain",
        "entry_filter": {"category": {"eq": "hardware"}}
    }))
    .await
    .unwrap_err();
    assert!(err.to_string().contains("not augmented") || err.to_string().contains("entry_collection"));
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --lib entry_filter_`
Expected: FAIL — `Args` has no `entry_filter`; `out["entries"]` is absent.

- [ ] **Step 3: Add the param to `Args`**

In `src/librarian/tools/get.rs`, add the import and the field:

```rust
use crate::librarian::filter::{eval, FilterNode};
```

```rust
    #[serde(default)]
    entry_filter: Option<FilterNode>,
```

- [ ] **Step 4: Add the filtering block**

In `call`, after `let mut out = json!({ … });` and its `out["freshness"]` / `out["latest_event"]` assignments, but **before** `out["augmentation"] = match aug { … }` (which moves `aug`), insert:

```rust
    if let Some(ref filter) = a.entry_filter {
        let aug_row = aug.as_ref().ok_or_else(|| {
            RecoverableError::new(
                "entry_filter set but this artifact is not augmented — declare \
                 entry_collection on its augmentation, or retrofit it \
                 (docs/conventions/retrofitting-trackers-for-filtering.md)",
            )
        })?;
        let collection = aug_row.entry_collection.as_deref().ok_or_else(|| {
            RecoverableError::new(
                "entry_filter set but the augmentation has no entry_collection — \
                 declare which params array holds the filterable rows",
            )
        })?;
        let params: Value = serde_json::from_str(&aug_row.params)?;
        let arr = params
            .get(collection)
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                RecoverableError::new(format!(
                    "entry_collection points at `{collection}` but params has no array there"
                ))
            })?;
        let mut matched: Vec<Value> = Vec::new();
        for item in arr {
            if let Some(obj) = item.as_object() {
                if eval(filter, obj)? {
                    matched.push(item.clone());
                }
            }
        }
        out["entry_total"] = json!(arr.len());
        out["entries"] = json!(matched);
    }
```

(`RecoverableError` and `json` are already imported in this file; `Value` is in scope via `serde_json::Value` used elsewhere in `call`.)

- [ ] **Step 5: Run to verify they pass**

Run: `cargo test --lib entry_filter_`
Expected: PASS (2 tests).

- [ ] **Step 6: Commit**

```bash
git add src/librarian/tools/get.rs
git commit -m "feat(librarian): entry_filter param on artifact(get) returns matching entry rows"
```

---

## Task 6: `tracker_design` teaches the convention

**Files:**
- Modify: `src/librarian/tools/tracker_design.rs` (`SYSTEM_PROMPT` ~424-469; `archetype_failure_table` 79-115)

- [ ] **Step 1: Add `entry_collection` to the failure_table example**

In `archetype_failure_table`, add a field to the returned `json!({…})` (after `prompt_template` or alongside the example fields):

```rust
            "entry_collection": "failures",
```

- [ ] **Step 2: Add a teaching step to `SYSTEM_PROMPT`**

In the `SYSTEM_PROMPT` raw string, insert a step between the render_template step (Step 5) and the body-skeleton step (Step 6):

```text
## Step 5b — Make entries filterable (optional)

If a tracker's per-entry rows should be queryable (e.g. "show only the
open hardware items"), set `entry_collection` to the params key holding
the array of entry objects (e.g. `"failures"`). This enables
`artifact(get, id=..., entry_filter={field:{op:value}})`, which returns
the matching rows using the same filter syntax as `artifact(find)`.
Only the `failure_table`/`task_list`-style archetypes (entries in params)
support this; `reflective` trackers keep entries in prose — retrofit them
first (see docs/conventions/retrofitting-trackers-for-filtering.md).
```

- [ ] **Step 3: Verify build + existing tracker_design tests still pass**

Run: `cargo test --lib tracker_design`
Expected: PASS. (No `ONBOARDING_VERSION` bump — `tracker_design` output is live, not a cached prompt surface.)

- [ ] **Step 4: Commit**

```bash
git add src/librarian/tools/tracker_design.rs
git commit -m "docs(librarian): teach entry_collection in tracker_design"
```

---

## Task 7: The retrofit guide

**Files:**
- Create: `docs/conventions/retrofitting-trackers-for-filtering.md`

- [ ] **Step 1: Write the guide**

Create `docs/conventions/retrofitting-trackers-for-filtering.md`:

```markdown
# Retrofitting a Tracker for Filtering

Converts a prose tracker (entries as `## X-N` sections + `**Key:** value`
lines — the `reflective` shape) into a *filterable* tracker (entries as a
structured array in augmentation params — the `failure_table` shape), so
`artifact(get, id, entry_filter={…})` can query its rows.

## When to retrofit

- The tracker has repeating numbered entries (`F-N`, `U-N`, roadmap items).
- You want to query them by metadata (status, category, severity).
- It is currently prose-only (no `entry_collection` declared).

## Procedure

1. **Read the tracker** via `artifact(get, id, full=true)`. Identify the
   repeating `## X-N — …` sections and the `**Key:** value` lines under them.
2. **Derive the schema.** Each `**Key:**` is a field. Pin types: enums for
   `status`/`severity`, strings for dates/titles. Use `failure_table`'s
   `params_schema_example` (from `librarian(tracker_design)`) as the template.
3. **Build the array.** Populate `params.<collection> = [{id, …fields…}]`
   from the existing sections — one object per `## X-N`.
4. **Write the `render_template`** (MiniJinja) so the rendered body
   reproduces the existing `## X-N` prose. Humans should see no change.
5. **Declare the pointer** via
   `artifact_augment(id=…, merge=false, prompt=…, params=…, render_template=…, entry_collection="<collection>")`.
   (merge=false resets sibling fields — pass `prompt`/`render_template`/etc. back.)
6. **Verify.** `artifact(get, id, full=true)` — the rendered body must match
   the original section-for-section. Then test a filter:
   `artifact(get, id, entry_filter={"status":{"eq":"open"}})`.

## Notes

- Never delete the prose body content in the same step you add params —
  use `artifact(update, patch={body_edits:[…]})`, never a wholesale `body`
  overwrite (the 50% shrink guard exists for this).
- `id` is the natural per-entry key; keep it in every object so filtered
  rows are traceable back to their `## X-N` section.
```

- [ ] **Step 2: Commit**

```bash
git add docs/conventions/retrofitting-trackers-for-filtering.md
git commit -m "docs: add retrofit guide for filterable trackers"
```

---

## Task 8: Prompt surfaces + final verification

**Files:**
- Modify: `src/librarian/tools/artifact.rs` (`get` action description — mention `entry_filter`)
- Modify: `src/prompts/source.md` and/or `get_guide` content for librarian/tracker-conventions (note the entry-grain twin + link the retrofit guide)
- Verify: full build, clippy, tests, prompt-surface test, live MCP

- [ ] **Step 1: Document `entry_filter` in the `artifact` tool description**

In `src/librarian/tools/artifact.rs`, in the `get` action's description text, add a sentence: *"`entry_filter` (a filter AST, same shape as `find`'s `filter`) returns the matching rows from the tracker's declared `entry_collection` as structured entries."*

- [ ] **Step 2: Update the librarian guide surfaces**

Add to the filter section of the librarian guide a note that `eval`/`entry_filter` is the entry-grain twin of the artifact-grain `filter`, and link `docs/conventions/retrofitting-trackers-for-filtering.md` from the tracker-conventions guide. (Find the source via `grep "Filter Syntax" src/` — these guides are `include_str!`'d constants; per `CLAUDE.md` "Prompt Surface Consistency", editing the backing `.md` is not "just a doc change.")

- [ ] **Step 3: Run the prompt-surface guard test**

Run: `cargo test --lib prompt_surfaces_reference_only_real_tools`
Expected: PASS (no stale/unknown tool tokens introduced).

- [ ] **Step 4: Full verification**

Run, in order:
```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```
Expected: clippy clean; all tests pass.

- [ ] **Step 5: Live MCP smoke test**

```bash
cargo build --release
```
Then `/mcp` reconnect (the `~/.cargo/bin/codescout` symlink → `target/release/codescout` per `CLAUDE.md`). Manually: augment a scratch tracker with `entry_collection`, then call `artifact(get, id, entry_filter={…})` and confirm structured rows come back, and that a non-augmented artifact yields the RecoverableError (not a crash).

- [ ] **Step 6: Commit**

```bash
git add src/librarian/tools/artifact.rs src/prompts/source.md
git commit -m "docs(prompts): document entry_filter + link retrofit guide"
```

---

## Notes for the implementer

- **No `ONBOARDING_VERSION` bump.** Nothing here changes the cached `onboarding_prompt` surface or `build_system_prompt_draft()` — `tracker_design` and `server_instructions` are live per connect.
- **Error discipline:** all input-driven failures use `RecoverableError` (`isError:false`), never `anyhow::bail!`. The only `bail!`s are the pre-existing argument-shape guards in `get.rs`.
- **Branch:** work on `experiments`; cherry-pick to `master` only after the Standard Ship Sequence (tests + clippy + manual MCP). See `CLAUDE.md`.
- **Future graduation (out of scope):** cross-tracker `find(entry_filter)` would promote `eval` semantics into a catalog `entries` table populated at reindex (spec §9). Do not build it now.
