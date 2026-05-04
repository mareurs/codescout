# `append_mode` + History Cap Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `append_mode` + `history_cap` to artifact augmentation so `artifact_update` prepends a new dated section on each refresh instead of replacing the body, with automatic trimming of old sections beyond the cap.

**Architecture:** Two new SQLite columns on `artifact_augmentation` (migration v5). `artifact_augment` exposes them in its input schema. `artifact_update`'s body-patch path detects `append_mode` on the augmentation row and prepends `## YYYY-MM-DD\n\n<delta>\n\n` before the existing body, trimming via `trim_history` when `history_cap` is set. `artifact_refresh` adds an `"append_mode": true` hint to its response package so the LLM knows to write a delta, not a full rewrite.

**Tech Stack:** Rust, rusqlite (SQLite), chrono, regex (all already in Cargo.toml), `#[async_trait]`, tokio

---

## File Map

| File | Change |
|---|---|
| `crates/librarian-mcp/src/catalog/augmentation.rs` | +2 fields on `AugmentationRow`; update `upsert`, `get`, `get_batch`, `row_from_sql`; update `aug()` test helper |
| `crates/librarian-mcp/src/catalog/mod.rs` | `run_migrations` v5: two `ALTER TABLE` + schema version insert |
| `crates/librarian-mcp/src/tools/augment.rs` | `Args` +2 fields; `input_schema`; `call` passes new fields to row literal |
| `crates/librarian-mcp/src/tools/update.rs` | `call` body-patch path checks augmentation for `append_mode`; add `trim_history` helper |
| `crates/librarian-mcp/src/tools/refresh.rs` | Add `"append_mode": true` to response when set; add test module |

---

### Task 1: Migration v5 + catalog layer

**Files:**
- Modify: `crates/librarian-mcp/src/catalog/augmentation.rs`
- Modify: `crates/librarian-mcp/src/catalog/mod.rs`

- [ ] **Step 1: Write the failing catalog roundtrip tests**

Add inside the `#[cfg(test)]` module of `augmentation.rs`, after `get_batch_returns_map`:

```rust
#[test]
fn append_mode_and_history_cap_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let cat = crate::catalog::open(dir.path().join("cat.db")).unwrap();
    let art = sample_art("a1");
    crate::catalog::artifact::upsert(&cat, &art).unwrap();
    let mut row = aug("a1");
    row.append_mode = true;
    row.history_cap = Some(5);
    upsert(&cat, &row).unwrap();
    let got = get(&cat, "a1").unwrap().unwrap();
    assert!(got.append_mode);
    assert_eq!(got.history_cap, Some(5));
}

#[test]
fn append_mode_defaults_to_false() {
    let dir = tempfile::tempdir().unwrap();
    let cat = crate::catalog::open(dir.path().join("cat.db")).unwrap();
    let art = sample_art("a2");
    crate::catalog::artifact::upsert(&cat, &art).unwrap();
    upsert(&cat, &aug("a2")).unwrap();
    let got = get(&cat, "a2").unwrap().unwrap();
    assert!(!got.append_mode);
    assert_eq!(got.history_cap, None);
}
```

- [ ] **Step 2: Run to verify compile failure**

```bash
cargo test -p librarian-mcp -- append_mode_and_history_cap_roundtrip 2>&1 | grep "error\[" | head -5
```

Expected: `error[E0560]: struct ... has no field named 'append_mode'`

- [ ] **Step 3: Add fields to `AugmentationRow`**

In `augmentation.rs`, replace the struct body (the `pub struct AugmentationRow { ... }` block):

```rust
pub struct AugmentationRow {
    pub artifact_id: String,
    pub prompt: String,
    pub params: String, // raw JSON text
    pub last_refreshed_at: Option<String>,
    pub refresh_count: i64,
    pub created_at: String,
    pub updated_at: String,
    /// Optional MiniJinja template projecting `params` into a markdown snippet
    /// rendered into `librarian_context` output. Decouples live state (params)
    /// from prose (artifact body).
    pub render_template: Option<String>,
    /// Optional JSON Schema (draft-07+) validating `params` on every merge.
    pub params_schema: Option<String>,
    /// When true, artifact_update prepends a new dated section instead of replacing the body.
    pub append_mode: bool,
    /// Max number of dated `## YYYY-MM-DD` sections to retain. Oldest are dropped beyond cap.
    pub history_cap: Option<i64>,
}
```

- [ ] **Step 4: Update `upsert`**

Replace the `upsert` function body:

```rust
pub fn upsert(cat: &Catalog, row: &AugmentationRow) -> Result<()> {
    cat.conn.execute(
        "INSERT INTO artifact_augmentation
           (artifact_id, prompt, params, last_refreshed_at, refresh_count,
            created_at, updated_at, render_template, params_schema,
            append_mode, history_cap)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(artifact_id) DO UPDATE SET
           prompt = excluded.prompt,
           params = excluded.params,
           render_template = excluded.render_template,
           params_schema = excluded.params_schema,
           append_mode = excluded.append_mode,
           history_cap = excluded.history_cap,
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
        ],
    )?;
    Ok(())
}
```

- [ ] **Step 5: Update `get` and `get_batch` SELECT lists**

Replace the SQL string in `get`:

```rust
pub fn get(cat: &Catalog, artifact_id: &str) -> Result<Option<AugmentationRow>> {
    let mut stmt = cat.conn.prepare(
        "SELECT artifact_id, prompt, params, last_refreshed_at, refresh_count,
                created_at, updated_at, render_template, params_schema,
                append_mode, history_cap
         FROM artifact_augmentation WHERE artifact_id = ?1",
    )?;
    let mut rows = stmt.query_map([artifact_id], row_from_sql)?;
    Ok(rows.next().transpose()?)
}
```

Replace the SQL `format!` string inside `get_batch`:

```rust
    let sql = format!(
        "SELECT artifact_id, prompt, params, last_refreshed_at, refresh_count,
                created_at, updated_at, render_template, params_schema,
                append_mode, history_cap
         FROM artifact_augmentation WHERE artifact_id IN ({placeholders})"
    );
```

- [ ] **Step 6: Update `row_from_sql`**

Replace `row_from_sql`:

```rust
fn row_from_sql(row: &rusqlite::Row<'_>) -> rusqlite::Result<AugmentationRow> {
    Ok(AugmentationRow {
        artifact_id: row.get(0)?,
        prompt: row.get(1)?,
        params: row.get(2)?,
        last_refreshed_at: row.get(3)?,
        refresh_count: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
        render_template: row.get(7)?,
        params_schema: row.get(8)?,
        append_mode: row.get::<_, i64>(9).map(|v| v != 0)?,
        history_cap: row.get(10)?,
    })
}
```

- [ ] **Step 7: Update the `aug()` test helper**

In the `tests` module, replace the `aug` function body:

```rust
fn aug(artifact_id: &str) -> AugmentationRow {
    AugmentationRow {
        artifact_id: artifact_id.to_string(),
        prompt: "test prompt".to_string(),
        params: "{}".to_string(),
        last_refreshed_at: None,
        refresh_count: 0,
        created_at: "2026-01-01T00:00:00.000Z".to_string(),
        updated_at: "2026-01-01T00:00:00.000Z".to_string(),
        render_template: None,
        params_schema: None,
        append_mode: false,
        history_cap: None,
    }
}
```

- [ ] **Step 8: Add migration v5 in `catalog/mod.rs`**

In `run_migrations`, insert after the v4 block (before the closing `Ok(())`):

```rust
    // v5: append_mode + history_cap columns on artifact_augmentation
    if !column_exists(conn, "artifact_augmentation", "append_mode")? {
        conn.execute(
            "ALTER TABLE artifact_augmentation ADD COLUMN append_mode INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    if !column_exists(conn, "artifact_augmentation", "history_cap")? {
        conn.execute(
            "ALTER TABLE artifact_augmentation ADD COLUMN history_cap INTEGER",
            [],
        )?;
    }
    conn.execute(
        "INSERT OR IGNORE INTO schema_version (version) VALUES (5)",
        [],
    )?;
```

- [ ] **Step 9: Run the two new catalog tests**

```bash
cargo test -p librarian-mcp -- append_mode_and_history_cap_roundtrip append_mode_defaults_to_false 2>&1 | grep -E "running|ok\.|FAILED"
```

Expected: both pass.

- [ ] **Step 10: Run full lib tests — no regressions**

```bash
cargo test -p librarian-mcp --lib 2>&1 | tail -3
```

Expected: all pass. Count increases by 2 from prior baseline.

---

### Task 2: `artifact_augment` — expose and persist new fields

**Files:**
- Modify: `crates/librarian-mcp/src/tools/augment.rs`

- [ ] **Step 1: Write failing tests**

Add after `non_merge_without_prompt_errors` in `augment.rs` tests:

```rust
#[tokio::test]
async fn persists_append_mode_and_history_cap() {
    let ctx = mk_ctx();
    seed_artifact(&ctx, "a99");
    ArtifactAugment
        .call(
            &ctx,
            serde_json::json!({
                "id": "a99",
                "prompt": "track me",
                "append_mode": true,
                "history_cap": 10,
            }),
        )
        .await
        .unwrap();
    let cat = ctx.catalog.lock();
    let row = augmentation::get(&cat, "a99").unwrap().unwrap();
    assert!(row.append_mode);
    assert_eq!(row.history_cap, Some(10));
}

#[tokio::test]
async fn append_mode_defaults_to_false_when_absent() {
    let ctx = mk_ctx();
    seed_artifact(&ctx, "a100");
    ArtifactAugment
        .call(&ctx, serde_json::json!({"id": "a100", "prompt": "no append"}))
        .await
        .unwrap();
    let cat = ctx.catalog.lock();
    let row = augmentation::get(&cat, "a100").unwrap().unwrap();
    assert!(!row.append_mode);
    assert_eq!(row.history_cap, None);
}
```

- [ ] **Step 2: Run to verify compile failure**

```bash
cargo test -p librarian-mcp -- persists_append_mode_and_history_cap 2>&1 | grep "error\[" | head -3
```

Expected: struct literal missing fields `append_mode` / `history_cap`.

- [ ] **Step 3: Add fields to `Args`**

Replace the `Args` struct in `augment.rs`:

```rust
struct Args {
    id: String,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    params: Option<Value>,
    #[serde(default)]
    render_template: Option<String>,
    #[serde(default)]
    params_schema: Option<Value>,
    #[serde(default)]
    merge: bool,
    #[serde(default)]
    append_mode: Option<bool>,
    #[serde(default)]
    history_cap: Option<usize>,
}
```

- [ ] **Step 4: Add fields to `input_schema`**

In `input_schema`, add after the `"merge"` property entry:

```rust
                "append_mode": {
                    "type": "boolean",
                    "default": false,
                    "description": "When true, artifact_update prepends a new dated section instead of replacing the body. Prompt should instruct the LLM to write only the new delta block."
                },
                "history_cap": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Max number of dated ## YYYY-MM-DD sections to retain. Oldest sections beyond cap are dropped on each append."
                },
```

- [ ] **Step 5: Pass new fields in `call`**

In the create/replace path of `call`, update the `augmentation::upsert(...)` call to include the new fields:

```rust
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
                append_mode: a.append_mode.unwrap_or(false),
                history_cap: a.history_cap.map(|v| v as i64),
            },
        )?;
```

- [ ] **Step 6: Run new tests**

```bash
cargo test -p librarian-mcp -- persists_append_mode_and_history_cap append_mode_defaults_to_false_when_absent 2>&1 | grep -E "running|ok\.|FAILED"
```

Expected: both pass.

- [ ] **Step 7: Run full lib tests**

```bash
cargo test -p librarian-mcp --lib 2>&1 | tail -3
```

Expected: all pass, count up by 2 from Task 1 baseline.

---

### Task 3: `trim_history` helper + `artifact_update` append logic

**Files:**
- Modify: `crates/librarian-mcp/src/tools/update.rs`

- [ ] **Step 1: Add `augmentation` import to the test module**

In `update.rs`, add to the imports block at the top of `mod tests`:

```rust
    use crate::catalog::augmentation;
```

- [ ] **Step 2: Write `trim_history` unit tests**

Add to the `tests` module:

```rust
    #[test]
    fn trim_history_keeps_all_when_under_cap() {
        let body = "## 2026-01-03\n\nnewest\n\n## 2026-01-02\n\nmiddle\n";
        assert_eq!(trim_history(body, 5), body);
    }

    #[test]
    fn trim_history_drops_oldest_entries() {
        let body =
            "## 2026-01-03\n\nnewest\n\n## 2026-01-02\n\nmiddle\n\n## 2026-01-01\n\noldest\n";
        let result = trim_history(body, 2);
        assert!(result.contains("newest"), "newest missing");
        assert!(result.contains("middle"), "middle missing");
        assert!(!result.contains("oldest"), "oldest should be dropped");
    }

    #[test]
    fn trim_history_preserves_intro_prose() {
        let body = "Intro paragraph.\n\n## 2026-01-02\n\nnew\n\n## 2026-01-01\n\nold\n";
        let result = trim_history(body, 1);
        assert!(result.contains("Intro paragraph"), "intro prose missing");
        assert!(result.contains("new"), "new section missing");
        assert!(!result.contains("old"), "old section should be dropped");
    }

    #[test]
    fn trim_history_no_dated_sections_unchanged() {
        let body = "Just prose, no dated headers.\n";
        assert_eq!(trim_history(body, 2), body);
    }
```

- [ ] **Step 3: Write append-mode integration tests**

Add after the `trim_history` unit tests, still in the `tests` module:

```rust
    async fn seed_with_augment(
        ctx: &ToolContext,
        rel_path: &str,
        append_mode: bool,
        history_cap: Option<i64>,
    ) -> String {
        let v = ArtifactCreate
            .call(
                ctx,
                serde_json::json!({
                    "repo": "r",
                    "rel_path": rel_path,
                    "kind": "spec",
                    "title": "test",
                    "body": "original body",
                }),
            )
            .await
            .unwrap();
        let id = v["id"].as_str().unwrap().to_string();
        let cat = ctx.catalog.lock();
        augmentation::upsert(
            &cat,
            &augmentation::AugmentationRow {
                artifact_id: id.clone(),
                prompt: "test".to_string(),
                params: "{}".to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                render_template: None,
                params_schema: None,
                append_mode,
                history_cap,
            },
        )
        .unwrap();
        id
    }

    #[tokio::test]
    async fn append_mode_prepends_dated_section() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let id = seed_with_augment(&ctx, "b1.md", true, None).await;

        ArtifactUpdate
            .call(&ctx, serde_json::json!({"id": id, "patch": {"body": "delta content"}}))
            .await
            .unwrap();

        let content = std::fs::read_to_string(tmp.path().join("b1.md")).unwrap();
        assert!(content.contains("\n## 20"), "dated header missing: {content}");
        assert!(content.contains("delta content"), "delta missing");
        assert!(content.contains("original body"), "original body missing");
    }

    #[tokio::test]
    async fn second_append_newest_first() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let id = seed_with_augment(&ctx, "b2.md", true, None).await;

        ArtifactUpdate
            .call(&ctx, serde_json::json!({"id": id, "patch": {"body": "first delta"}}))
            .await
            .unwrap();
        ArtifactUpdate
            .call(&ctx, serde_json::json!({"id": id, "patch": {"body": "second delta"}}))
            .await
            .unwrap();

        let content = std::fs::read_to_string(tmp.path().join("b2.md")).unwrap();
        let pos_second = content.find("second delta").unwrap();
        let pos_first = content.find("first delta").unwrap();
        assert!(
            pos_second < pos_first,
            "second delta should appear before first delta"
        );
    }

    #[tokio::test]
    async fn history_cap_drops_oldest_section() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let id = seed_with_augment(&ctx, "b3.md", true, Some(2)).await;

        for entry in &["entry 1", "entry 2", "entry 3"] {
            ArtifactUpdate
                .call(&ctx, serde_json::json!({"id": id, "patch": {"body": entry}}))
                .await
                .unwrap();
        }

        let content = std::fs::read_to_string(tmp.path().join("b3.md")).unwrap();
        assert!(content.contains("entry 3"), "newest missing");
        assert!(content.contains("entry 2"), "second missing");
        assert!(!content.contains("entry 1"), "oldest should be dropped");
    }

    #[tokio::test]
    async fn no_append_mode_replace_unchanged() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let id = seed_with_augment(&ctx, "b4.md", false, None).await;

        ArtifactUpdate
            .call(
                &ctx,
                serde_json::json!({"id": id, "patch": {"body": "replacement body"}}),
            )
            .await
            .unwrap();

        let content = std::fs::read_to_string(tmp.path().join("b4.md")).unwrap();
        assert!(content.contains("replacement body"), "body missing");
        assert!(
            !content.contains("## 20"),
            "dated header should not appear in replace mode"
        );
    }
```

- [ ] **Step 4: Run tests to verify compile failure (trim_history missing)**

```bash
cargo test -p librarian-mcp -- trim_history_keeps_all_when_under_cap 2>&1 | grep "error\[" | head -3
```

Expected: `error[E0425]: cannot find function 'trim_history'`

- [ ] **Step 5: Add `trim_history` helper**

Add this function at module level in `update.rs`, after `write_field_to_frontmatter`:

```rust
fn trim_history(body: &str, cap: usize) -> String {
    let re = regex::Regex::new(r"(?m)^## \d{4}-\d{2}-\d{2}").unwrap();
    let positions: Vec<usize> = re.find_iter(body).map(|m| m.start()).collect();
    if positions.len() <= cap {
        return body.to_string();
    }
    let cutoff = positions[cap];
    body[..cutoff].trim_end().to_string() + "\n"
}
```

- [ ] **Step 6: Run `trim_history` unit tests**

```bash
cargo test -p librarian-mcp -- trim_history 2>&1 | grep -E "running|ok\.|FAILED"
```

Expected: 4 passed.

- [ ] **Step 7: Modify the body-patch branch in `artifact_update/call`**

In `update.rs` `call`, replace the `if let Some(new_body) = &patch.body { ... }` branch. The current branch starts at `let new_content = if let Some(new_body) = &patch.body {` and ends just before the `} else {`. Replace that entire `if` arm (keep the `else` arm untouched):

```rust
        let new_content = if let Some(new_body) = &patch.body {
            // Re-parse frontmatter and rebuild with new body
            let (fm_opt, old_body) = crate::frontmatter::parse(&original)?;
            let mut fm = fm_opt.unwrap_or_default();
            if let Some(v) = &patch.status {
                fm.status = Some(v.clone());
            }
            if let Some(v) = &patch.title {
                fm.title = Some(v.clone());
            }
            if let Some(v) = &patch.owners {
                fm.owners = v.clone();
            }
            if let Some(v) = &patch.tags {
                fm.tags = v.clone();
            }
            if let Some(v) = &patch.topic {
                fm.topic = Some(v.clone());
            }
            let actual_body = match crate::catalog::augmentation::get(&cat, &a.id)? {
                Some(aug) if aug.append_mode => {
                    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
                    let mut appended =
                        format!("## {date}\n\n{new_body}\n\n{}", old_body.trim_start());
                    if let Some(cap) = aug.history_cap {
                        appended = trim_history(&appended, cap as usize);
                    }
                    appended
                }
                _ => new_body.clone(),
            };
            crate::frontmatter::write(&fm, &format!("\n{actual_body}\n"))
```

- [ ] **Step 8: Run all append-mode integration tests**

```bash
cargo test -p librarian-mcp -- append_mode_prepends second_append_newest history_cap_drops no_append_mode_replace 2>&1 | grep -E "running|ok\.|FAILED"
```

Expected: 4 passed.

- [ ] **Step 9: Run full lib tests**

```bash
cargo test -p librarian-mcp --lib 2>&1 | tail -3
```

Expected: all pass, count up by 8 from Task 2 baseline.

---

### Task 4: `artifact_refresh` — `append_mode` hint

**Files:**
- Modify: `crates/librarian-mcp/src/tools/refresh.rs`

`refresh.rs` has no test module yet. This task adds one.

- [ ] **Step 1: Write the failing test**

Append to the bottom of `refresh.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::augmentation;
    use crate::catalog::Catalog;
    use crate::tools::augment::ArtifactAugment;
    use crate::tools::create::ArtifactCreate;
    use crate::workspace::{Root, WorkspaceConfig};
    use std::sync::Arc;
    use tempfile::TempDir;

    fn mk_ctx(tmp_root: std::path::PathBuf) -> ToolContext {
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap())),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![Root {
                    name: "r".into(),
                    path: tmp_root,
                }],
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
    async fn refresh_includes_append_mode_hint_when_set() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        let v = ArtifactCreate
            .call(
                &ctx,
                serde_json::json!({
                    "repo": "r",
                    "rel_path": "hint_test.md",
                    "kind": "spec",
                    "title": "hint test",
                    "body": "body",
                }),
            )
            .await
            .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        ArtifactAugment
            .call(
                &ctx,
                serde_json::json!({
                    "id": id,
                    "prompt": "track",
                    "append_mode": true,
                }),
            )
            .await
            .unwrap();

        let result = ArtifactRefresh
            .call(&ctx, serde_json::json!({"id": id}))
            .await
            .unwrap();
        assert_eq!(result["append_mode"], serde_json::json!(true));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cargo test -p librarian-mcp -- refresh_includes_append_mode_hint_when_set 2>&1 | grep -E "running|FAILED|assertion"
```

Expected: assertion failure — `append_mode` key absent from response.

- [ ] **Step 3: Add the hint to `refresh.rs` `call`**

In `refresh.rs`, replace the final `Ok(json!({...}))` at the end of `call` with:

```rust
        let mut out = json!({
            "artifact_id": a.id,
            "prompt": aug.prompt,
            "params": params,
            "current_body": current_body,
            "context": context,
            "last_refreshed_at": aug.last_refreshed_at,
            "hints": hints,
        });
        if aug.append_mode {
            out["append_mode"] = json!(true);
        }
        Ok(out)
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p librarian-mcp -- refresh_includes_append_mode_hint_when_set 2>&1 | grep -E "running|ok\.|FAILED"
```

Expected: 1 passed.

---

### Task 5: Full suite, fmt, clippy, commit

- [ ] **Step 1: Run full librarian-mcp lib test suite**

```bash
cargo test -p librarian-mcp --lib 2>&1 | tail -3
```

Expected: all pass. Count should be 299 (pre-task baseline) + 13 new = 312.

- [ ] **Step 2: Run fmt**

```bash
cargo fmt -p librarian-mcp
```

Expected: no output.

- [ ] **Step 3: Run clippy**

```bash
cargo clippy -p librarian-mcp -- -D warnings 2>&1 | tail -10
```

Expected: no warnings.

- [ ] **Step 4: Commit**

```bash
git add crates/librarian-mcp/src/catalog/augmentation.rs \
        crates/librarian-mcp/src/catalog/mod.rs \
        crates/librarian-mcp/src/tools/augment.rs \
        crates/librarian-mcp/src/tools/update.rs \
        crates/librarian-mcp/src/tools/refresh.rs
git commit -m "feat(augmentation): append_mode + history_cap

artifact_update with append_mode=true prepends a new ## YYYY-MM-DD dated
section instead of replacing the body. history_cap trims oldest sections
beyond the cap. artifact_refresh hints append_mode: true so the LLM knows
to write a delta, not a full rewrite.

Migration v5: two new columns on artifact_augmentation (append_mode,
history_cap). trim_history scans ## YYYY-MM-DD headers top-to-bottom,
keeps first N, preserves intro prose above the first header.

13 new tests across catalog, augment, update (4 unit + 4 integration),
and refresh layers.

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```
