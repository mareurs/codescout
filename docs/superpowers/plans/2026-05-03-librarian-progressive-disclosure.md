# Librarian Progressive Disclosure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Surface catalog orientation info on cold `artifact(find)` calls, add tracker guidance to tool description and create response, and document augmentation in server instructions.

**Architecture:** Four targeted changes across two crates — a new `catalog_summary` query helper, cold-call detection in the find tool, a tracker hint in the create tool, and two prose edits (tool description + server_instructions.md). No new files; all changes are additive.

**Tech Stack:** Rust, rusqlite (SQLite), serde_json, librarian-mcp crate

---

## File Map

| File | Change |
|------|--------|
| `crates/librarian-mcp/src/catalog/find.rs` | Add `CatalogSummary` struct + `catalog_summary()` fn |
| `crates/librarian-mcp/src/tools/find.rs` | Detect cold call; inject `catalog` field in response |
| `crates/librarian-mcp/src/tools/create.rs` | Add `tracker_hint` when `kind=tracker` and no `augment` |
| `crates/librarian-mcp/src/tools/artifact.rs` | Append one sentence to `description()` |
| `src/prompts/server_instructions.md` | Add augmentation + tracker 2-liner |

---

### Task 1: artifact tool description

**Files:**
- Modify: `crates/librarian-mcp/src/tools/artifact.rs:15-20`

- [ ] **Step 1: Edit the description method**

Replace the body of `impl Tool for Artifact / description` with:

```rust
    fn description(&self) -> &'static str {
        "Artifact CRUD and query. action: find | get | create | update | link | graph | state_at. \
         Defaults: scope=project (current sub-project only), archived/superseded hidden when \
         filter does not constrain status. Shortcut params kind/status expand to eq-filters \
         and combine with filter via AND. \
         Trackers are artifacts with kind=tracker — augmented documents that auto-refresh their \
         body via a persistent prompt; call librarian(tracker_design) before creating one."
    }
```

- [ ] **Step 2: Verify it compiles**

```bash
cd crates/librarian-mcp && cargo check 2>&1
```
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/librarian-mcp/src/tools/artifact.rs
git commit -m "feat(librarian): add tracker kind hint to artifact tool description"
```

---

### Task 2: `CatalogSummary` + `catalog_summary()` in catalog

**Files:**
- Modify: `crates/librarian-mcp/src/catalog/find.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `crates/librarian-mcp/src/catalog/find.rs`:

```rust
    #[test]
    fn catalog_summary_counts_by_kind_and_total() {
        use crate::catalog::artifact::{upsert, ArtifactRow};
        let cat = crate::catalog::Catalog::open_in_memory().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        for (id, kind) in [("a1", "tracker"), ("a2", "tracker"), ("a3", "plan")] {
            upsert(&cat, &ArtifactRow {
                id: id.into(), repo: "r".into(), rel_path: format!("{id}.md"),
                kind: kind.into(), status: "draft".into(),
                title: None, owners: vec![], tags: vec![], topic: None,
                time_scope: None, source: None,
                created_at: now, updated_at: now, file_mtime: now,
                file_sha256: "".into(), confidence: 1.0,
            }).unwrap();
        }
        let s = catalog_summary(&cat, None).unwrap();
        assert_eq!(s.total, 3);
        assert_eq!(s.by_kind["tracker"], 2);
        assert_eq!(s.by_kind["plan"], 1);
        assert_eq!(s.augmented, 0);
    }

    #[test]
    fn catalog_summary_counts_augmented() {
        use crate::catalog::artifact::{upsert, ArtifactRow};
        use crate::catalog::augmentation;
        let cat = crate::catalog::Catalog::open_in_memory().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        let now_ts = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        upsert(&cat, &ArtifactRow {
            id: "a1".into(), repo: "r".into(), rel_path: "a1.md".into(),
            kind: "tracker".into(), status: "draft".into(),
            title: None, owners: vec![], tags: vec![], topic: None,
            time_scope: None, source: None,
            created_at: now, updated_at: now, file_mtime: now,
            file_sha256: "".into(), confidence: 1.0,
        }).unwrap();
        upsert(&cat, &ArtifactRow {
            id: "a2".into(), repo: "r".into(), rel_path: "a2.md".into(),
            kind: "plan".into(), status: "draft".into(),
            title: None, owners: vec![], tags: vec![], topic: None,
            time_scope: None, source: None,
            created_at: now, updated_at: now, file_mtime: now,
            file_sha256: "".into(), confidence: 1.0,
        }).unwrap();
        augmentation::upsert(&cat, &crate::catalog::augmentation::AugmentationRow {
            artifact_id: "a1".into(),
            prompt: "track".into(),
            params: "{}".into(),
            last_refreshed_at: None,
            refresh_count: 0,
            created_at: now_ts.clone(),
            updated_at: now_ts,
            render_template: None,
            params_schema: None,
            append_mode: false,
            history_cap: None,
        }).unwrap();
        let s = catalog_summary(&cat, None).unwrap();
        assert_eq!(s.total, 2);
        assert_eq!(s.augmented, 1);
    }

    #[test]
    fn catalog_summary_respects_scoped_filter() {
        use crate::catalog::artifact::{upsert, ArtifactRow};
        use crate::filter::FilterNode;
        use serde_json::json;
        let cat = crate::catalog::Catalog::open_in_memory().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        for (id, repo) in [("a1", "repo-a"), ("a2", "repo-b")] {
            upsert(&cat, &ArtifactRow {
                id: id.into(), repo: repo.into(), rel_path: format!("{id}.md"),
                kind: "plan".into(), status: "draft".into(),
                title: None, owners: vec![], tags: vec![], topic: None,
                time_scope: None, source: None,
                created_at: now, updated_at: now, file_mtime: now,
                file_sha256: "".into(), confidence: 1.0,
            }).unwrap();
        }
        // Filter to repo-a only
        let f = FilterNode::Leaf(
            [("repo".to_string(), json!({"eq": "repo-a"}))].into_iter().collect()
        );
        let s = catalog_summary(&cat, Some(&f)).unwrap();
        assert_eq!(s.total, 1);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd crates/librarian-mcp && cargo test catalog_summary 2>&1
```
Expected: compile error — `catalog_summary` not defined yet.

- [ ] **Step 3: Implement `CatalogSummary` and `catalog_summary`**

Add to `crates/librarian-mcp/src/catalog/find.rs` after the `count_matching` function (around line 60):

```rust
pub struct CatalogSummary {
    pub total: usize,
    pub by_kind: std::collections::BTreeMap<String, usize>,
    pub augmented: usize,
}

/// Catalog-level summary for the given scoped filter: total non-archived
/// artifact count, count by kind, and count of augmented artifacts.
/// Caller is responsible for passing a filter that already excludes
/// archived/superseded rows if desired.
pub fn catalog_summary(
    cat: &Catalog,
    scoped_filter: Option<&FilterNode>,
) -> Result<CatalogSummary> {
    let (where_sql, params) = match scoped_filter {
        Some(f) => {
            let frag = compile(f)?;
            (format!(" WHERE {}", frag.sql), frag.params)
        }
        None => (String::new(), Vec::new()),
    };

    let mut by_kind = std::collections::BTreeMap::new();
    let mut total = 0usize;
    {
        let sql = format!(
            "SELECT kind, COUNT(*) FROM artifact{} GROUP BY kind",
            where_sql
        );
        let mut stmt = cat.conn.prepare(&sql)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(params.iter()),
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
        )?;
        for row in rows {
            let (kind, count) = row?;
            let c = count.max(0) as usize;
            total += c;
            by_kind.insert(kind, c);
        }
    }

    let augmented = {
        let aug_sql = format!(
            "SELECT COUNT(*) FROM augmentation \
             WHERE artifact_id IN (SELECT id FROM artifact{})",
            where_sql
        );
        let mut stmt = cat.conn.prepare(&aug_sql)?;
        let n: i64 = stmt.query_row(
            rusqlite::params_from_iter(params.iter()),
            |r| r.get(0),
        )?;
        n.max(0) as usize
    };

    Ok(CatalogSummary { total, by_kind, augmented })
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd crates/librarian-mcp && cargo test catalog_summary 2>&1
```
Expected: 3 tests pass.

- [ ] **Step 5: Run full test suite**

```bash
cd crates/librarian-mcp && cargo test 2>&1
```
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/librarian-mcp/src/catalog/find.rs
git commit -m "feat(librarian): add catalog_summary helper for cold-call orientation"
```

---

### Task 3: Cold-call catalog injection in `artifact(find)`

**Files:**
- Modify: `crates/librarian-mcp/src/tools/find.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `crates/librarian-mcp/src/tools/find.rs`:

```rust
    #[tokio::test]
    async fn cold_call_returns_catalog_field() {
        use crate::catalog::artifact::{upsert, ArtifactRow};
        let cat = crate::catalog::Catalog::open_in_memory().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        upsert(&cat, &ArtifactRow {
            id: "a1".into(), repo: "claude".into(), rel_path: "a1.md".into(),
            kind: "tracker".into(), status: "draft".into(),
            title: None, owners: vec![], tags: vec![], topic: None,
            time_scope: None, source: None,
            created_at: now, updated_at: now, file_mtime: now,
            file_sha256: "".into(), confidence: 1.0,
        }).unwrap();
        let ctx = mk_ctx(cat);
        let result = call(&ctx, serde_json::json!({})).await.unwrap();
        assert!(result["catalog"].is_object(), "cold call must include catalog field");
        assert_eq!(result["catalog"]["total"], 1);
        assert_eq!(result["catalog"]["by_kind"]["tracker"], 1);
        assert_eq!(result["catalog"]["augmented"], 0);
    }

    #[tokio::test]
    async fn find_with_kind_filter_omits_catalog_field() {
        use crate::catalog::artifact::{upsert, ArtifactRow};
        let cat = crate::catalog::Catalog::open_in_memory().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        upsert(&cat, &ArtifactRow {
            id: "a1".into(), repo: "claude".into(), rel_path: "a1.md".into(),
            kind: "tracker".into(), status: "draft".into(),
            title: None, owners: vec![], tags: vec![], topic: None,
            time_scope: None, source: None,
            created_at: now, updated_at: now, file_mtime: now,
            file_sha256: "".into(), confidence: 1.0,
        }).unwrap();
        let ctx = mk_ctx(cat);
        let result = call(&ctx, serde_json::json!({"kind": "tracker"})).await.unwrap();
        assert!(
            result.get("catalog").is_none() || result["catalog"].is_null(),
            "filtered find must not include catalog field"
        );
    }
```

Note: `mk_ctx` in `tools/find.rs` sets `roots[0].name = "claude"`. Use that repo name in test rows.

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd crates/librarian-mcp && cargo test cold_call_returns_catalog_field find_with_kind_filter_omits 2>&1
```
Expected: tests compile but fail — no `catalog` field returned yet.

- [ ] **Step 3: Add `is_cold_call` detection and catalog injection**

In `crates/librarian-mcp/src/tools/find.rs`, modify the `call` function:

**3a.** Add the `catalog_summary` import at the top of the file alongside the existing find imports:

```rust
use crate::catalog::find::{catalog_summary, count_matching, find, FindOpts};
```

**3b.** Right after `let a: Args = serde_json::from_value(args)?;`, add:

```rust
    let is_cold_call = a.filter.is_none()
        && a.semantic.is_none()
        && a.kind.is_none()
        && a.status.is_none()
        && a.augmented.is_none();
```

**3c.** After `let cat = ctx.catalog.lock();` and after computing `scoped_filter` (the line `let (scoped_filter, applied) = apply_scope(...)?;`), and BEFORE moving `scoped_filter` into `FindOpts`, add:

```rust
    let catalog_value: Option<serde_json::Value> = if is_cold_call {
        let summary = catalog_summary(&cat, scoped_filter.as_ref())?;
        Some(serde_json::json!({
            "total": summary.total,
            "by_kind": summary.by_kind,
            "augmented": summary.augmented,
        }))
    } else {
        None
    };
```

**3d.** Replace the final `Ok(json!({...}))` return with:

```rust
    let mut response = serde_json::json!({
        "count": items.len(),
        "items": items,
        "scope": applied.to_json(),
        "hints": hints,
    });
    if let Some(cat_val) = catalog_value {
        response["catalog"] = cat_val;
    }
    Ok(response)
```

- [ ] **Step 4: Run new tests**

```bash
cd crates/librarian-mcp && cargo test cold_call_returns_catalog_field find_with_kind_filter_omits 2>&1
```
Expected: both pass.

- [ ] **Step 5: Run full test suite**

```bash
cd crates/librarian-mcp && cargo test 2>&1
```
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/librarian-mcp/src/tools/find.rs
git commit -m "feat(librarian): inject catalog summary on cold artifact(find) calls"
```

---

### Task 4: `tracker_hint` in `artifact(create)`

**Files:**
- Modify: `crates/librarian-mcp/src/tools/create.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `crates/librarian-mcp/src/tools/create.rs`:

```rust
    #[tokio::test]
    async fn tracker_without_augment_returns_hint() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let result = call(&ctx, serde_json::json!({
            "repo": "r",
            "rel_path": "docs/trackers/my-tracker.md",
            "kind": "tracker",
            "title": "My Tracker",
            "body": ""
        })).await.unwrap();
        assert!(
            result["tracker_hint"].is_string(),
            "tracker without augment must include tracker_hint"
        );
    }

    #[tokio::test]
    async fn tracker_with_augment_no_hint() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let result = call(&ctx, serde_json::json!({
            "repo": "r",
            "rel_path": "docs/trackers/augmented-tracker.md",
            "kind": "tracker",
            "title": "Augmented Tracker",
            "body": "",
            "augment": {"prompt": "track the state of X"}
        })).await.unwrap();
        assert!(
            result.get("tracker_hint").is_none(),
            "tracker with augment must not include tracker_hint"
        );
    }

    #[tokio::test]
    async fn non_tracker_kind_no_hint() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let result = call(&ctx, serde_json::json!({
            "repo": "r",
            "rel_path": "docs/plans/my-plan.md",
            "kind": "plan",
            "title": "My Plan",
            "body": ""
        })).await.unwrap();
        assert!(
            result.get("tracker_hint").is_none(),
            "non-tracker kind must not include tracker_hint"
        );
    }
```

Note: `mk_ctx` in `create.rs` sets `name: "r"` — use `"repo": "r"` in all test JSON.

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd crates/librarian-mcp && cargo test tracker_without_augment tracker_with_augment_no_hint non_tracker_kind_no_hint 2>&1
```
Expected: `tracker_without_augment_returns_hint` fails (no `tracker_hint`); others pass.

- [ ] **Step 3: Add tracker_hint to create response**

In `crates/librarian-mcp/src/tools/create.rs`, replace the final return of `call`:

Current:
```rust
    Ok(json!({"id": id, "repo": row.repo, "rel_path": row.rel_path}))
```

Replace with:
```rust
    let mut result = json!({"id": id, "repo": row.repo, "rel_path": row.rel_path});
    if a.kind == "tracker" && a.augment.is_none() {
        result["tracker_hint"] = json!(
            "Tracker created without augmentation. \
             Call librarian(tracker_design) to pick an archetype \
             and attach a refresh prompt via artifact_augment."
        );
    }
    Ok(result)
```

- [ ] **Step 4: Run new tests**

```bash
cd crates/librarian-mcp && cargo test tracker_without_augment tracker_with_augment_no_hint non_tracker_kind_no_hint 2>&1
```
Expected: all 3 pass.

- [ ] **Step 5: Run full test suite**

```bash
cd crates/librarian-mcp && cargo test 2>&1
```
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/librarian-mcp/src/tools/create.rs
git commit -m "feat(librarian): warn when tracker created without augmentation"
```

---

### Task 5: server_instructions.md — augmentation + tracker 2-liner

**Files:**
- Modify: `src/prompts/server_instructions.md`

- [ ] **Step 1: Edit the Artifact & Tracker Routing section**

In `src/prompts/server_instructions.md`, in the `### Artifact & Tracker Routing` section, after the **Entry point** line and before **Create workflow:**, insert:

```markdown
**Artifact model:** Artifacts can carry **augmentation** — a persistent prompt that auto-refreshes their body as the codebase evolves. **Trackers** (`kind=tracker`) are the canonical augmented artifact: living documents for issue lists, ADR logs, experiment records, and similar multi-entry state.
```

The section should read (showing context):
```markdown
**Entry point:** `librarian_context(topic)` — packs a semantic bundle of relevant artifacts and context. Call this first before any artifact task to orient and avoid duplicates.

**Artifact model:** Artifacts can carry **augmentation** — a persistent prompt that auto-refreshes their body as the codebase evolves. **Trackers** (`kind=tracker`) are the canonical augmented artifact: living documents for issue lists, ADR logs, experiment records, and similar multi-entry state.

**Create workflow:**
```

- [ ] **Step 2: Verify prompt surface test passes**

```bash
cargo test prompt_surfaces_reference_only_real_tools 2>&1
```
Expected: passes (no new tool names introduced — only descriptive prose).

- [ ] **Step 3: Run full test suite**

```bash
cargo test 2>&1
```
Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add src/prompts/server_instructions.md
git commit -m "docs(prompts): add augmentation + tracker model description to artifact routing"
```

---

## Verification

After all tasks:

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
```

All must pass before marking this plan complete.
