# Artifact Augmentation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `artifact_augmentation` — a sidecar table that lets any artifact carry a persistent prompt + AI-editable params, enabling server-assisted context gathering and AI-driven refresh.

**Architecture:** New `artifact_augmentation` table (sidecar, cascade-delete). Five new tools (`artifact_augment`, `artifact_update_params`, `artifact_refresh`, `artifact_refresh_commit`, `tracker_create`). Three modified tools (`artifact_get`, `artifact_find`, `librarian_context`). Gather sources (git_log, artifacts, observations, file, grep) implemented in a shared `tools/gather.rs` module.

**Tech Stack:** Rust async (tokio), rusqlite, git2 (already in dep tree), walkdir + ignore (already in dep tree), regex (add if absent), serde_json merge-patch.

---

## File Map

**New files:**
- `crates/librarian-mcp/src/catalog/augmentation.rs` — `AugmentationRow` + CRUD
- `crates/librarian-mcp/src/tools/gather.rs` — `GatherSource` enum + `gather_all`
- `crates/librarian-mcp/src/tools/augment.rs` — `ArtifactAugment` tool
- `crates/librarian-mcp/src/tools/update_params.rs` — `ArtifactUpdateParams` tool
- `crates/librarian-mcp/src/tools/refresh.rs` — `ArtifactRefresh` tool
- `crates/librarian-mcp/src/tools/refresh_commit.rs` — `ArtifactRefreshCommit` tool
- `crates/librarian-mcp/src/tools/tracker_create.rs` — `TrackerCreate` tool

**Modified files:**
- `crates/librarian-mcp/src/catalog/schema.sql` — add v3 table
- `crates/librarian-mcp/src/catalog/mod.rs` — `pub mod augmentation`
- `crates/librarian-mcp/src/catalog/observations.rs` — add `list_recent`
- `crates/librarian-mcp/src/tools/mod.rs` — add modules + `all_tools()` entries
- `crates/librarian-mcp/src/tools/get.rs` — include augmentation in response
- `crates/librarian-mcp/src/tools/find.rs` — `augmented: Option<bool>` filter
- `crates/librarian-mcp/src/tools/context.rs` — `[LIVE]` rendering + tracker priority
- `crates/librarian-mcp/src/prompts/server_instructions.md` — document new tools

---

## Task 1: Schema v3 — `artifact_augmentation` table

**Files:**
- Modify: `crates/librarian-mcp/src/catalog/schema.sql`
- Modify: `crates/librarian-mcp/src/catalog/mod.rs` (test only)

- [ ] **Step 1: Add v3 block to schema.sql**

Append to the end of `crates/librarian-mcp/src/catalog/schema.sql`:

```sql
-- v3: artifact augmentation (prompt + params for AI-maintained artifacts)
CREATE TABLE IF NOT EXISTS artifact_augmentation (
  artifact_id       TEXT    NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,
  prompt            TEXT    NOT NULL,
  params            TEXT    NOT NULL DEFAULT '{}',
  last_refreshed_at TEXT,
  refresh_count     INTEGER NOT NULL DEFAULT 0,
  created_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (artifact_id)
);
CREATE INDEX IF NOT EXISTS idx_augmentation_artifact ON artifact_augmentation(artifact_id);

INSERT OR IGNORE INTO schema_version (version) VALUES (3);
```

Note: `params` is `TEXT NOT NULL` (not `JSON`) because SQLite's JSON type alias is syntactic sugar — it's TEXT underneath. Using TEXT avoids any driver confusion.

- [ ] **Step 2: Write failing test in `catalog/mod.rs`**

Add to the `tests` module in `crates/librarian-mcp/src/catalog/mod.rs`:

```rust
#[test]
fn schema_has_augmentation_table() {
    let cat = Catalog::open_in_memory().unwrap();
    let tables: Vec<String> = cat
        .conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(tables.iter().any(|t| t == "artifact_augmentation"),
        "expected artifact_augmentation table, got: {tables:?}");
}
```

- [ ] **Step 3: Run test to verify it fails**

```bash
cargo test -p librarian-mcp schema_has_augmentation_table
```

Expected: FAIL — table not found (because we haven't added the SQL yet, or it was just added — re-run after adding SQL).

- [ ] **Step 4: Run test to verify it passes after SQL addition**

```bash
cargo test -p librarian-mcp schema_has_augmentation_table
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/librarian-mcp/src/catalog/schema.sql \
        crates/librarian-mcp/src/catalog/mod.rs
git commit -m "feat(librarian): add artifact_augmentation schema (v3)"
```

---

## Task 2: `catalog/augmentation.rs` — CRUD

**Files:**
- Create: `crates/librarian-mcp/src/catalog/augmentation.rs`
- Modify: `crates/librarian-mcp/src/catalog/mod.rs` — add `pub mod augmentation`

- [ ] **Step 1: Write failing tests**

Create `crates/librarian-mcp/src/catalog/augmentation.rs` with tests only:

```rust
use crate::catalog::{artifact, Catalog};
use anyhow::Result;
use serde_json::{json, Value};

pub struct AugmentationRow {
    pub artifact_id: String,
    pub prompt: String,
    pub params: String, // raw JSON text
    pub last_refreshed_at: Option<String>,
    pub refresh_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

pub fn upsert(cat: &Catalog, row: &AugmentationRow) -> Result<()> {
    todo!()
}

pub fn get(cat: &Catalog, artifact_id: &str) -> Result<Option<AugmentationRow>> {
    todo!()
}

pub fn merge_params(cat: &Catalog, artifact_id: &str, patch: &Value) -> Result<bool> {
    todo!()
}

pub fn commit_refresh(cat: &Catalog, artifact_id: &str) -> Result<bool> {
    todo!()
}

pub fn list_all_ids(cat: &Catalog) -> Result<Vec<String>> {
    todo!()
}

pub fn get_batch(
    cat: &Catalog,
    ids: &[String],
) -> Result<std::collections::HashMap<String, AugmentationRow>> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::artifact::{upsert as art_upsert, ArtifactRow};
    use chrono::Utc;

    fn sample_art(id: &str) -> ArtifactRow {
        let now = Utc::now().timestamp_millis();
        ArtifactRow {
            id: id.to_string(),
            repo: "repo".to_string(),
            rel_path: format!("{id}.md"),
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
            file_sha256: "abc".to_string(),
            confidence: 1.0,
        }
    }

    fn aug(artifact_id: &str) -> AugmentationRow {
        AugmentationRow {
            artifact_id: artifact_id.to_string(),
            prompt: "Keep it updated".to_string(),
            params: "{}".to_string(),
            last_refreshed_at: None,
            refresh_count: 0,
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
        }
    }

    #[test]
    fn upsert_and_get_roundtrip() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        upsert(&cat, &aug("art1")).unwrap();
        let row = get(&cat, "art1").unwrap().expect("row should exist");
        assert_eq!(row.artifact_id, "art1");
        assert_eq!(row.prompt, "Keep it updated");
        assert_eq!(row.refresh_count, 0);
    }

    #[test]
    fn upsert_replaces_on_conflict() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        upsert(&cat, &aug("art1")).unwrap();
        let mut updated = aug("art1");
        updated.prompt = "New prompt".to_string();
        upsert(&cat, &updated).unwrap();
        let row = get(&cat, "art1").unwrap().unwrap();
        assert_eq!(row.prompt, "New prompt");
    }

    #[test]
    fn merge_params_adds_key() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        upsert(&cat, &aug("art1")).unwrap();
        let patch = json!({"format": "table"});
        let found = merge_params(&cat, "art1", &patch).unwrap();
        assert!(found);
        let row = get(&cat, "art1").unwrap().unwrap();
        let params: Value = serde_json::from_str(&row.params).unwrap();
        assert_eq!(params["format"], "table");
    }

    #[test]
    fn merge_params_null_deletes_key() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        let mut a = aug("art1");
        a.params = r#"{"format":"table"}"#.to_string();
        upsert(&cat, &a).unwrap();
        let patch = json!({"format": null});
        merge_params(&cat, "art1", &patch).unwrap();
        let row = get(&cat, "art1").unwrap().unwrap();
        let params: Value = serde_json::from_str(&row.params).unwrap();
        assert!(params.get("format").is_none());
    }

    #[test]
    fn merge_params_missing_artifact_returns_false() {
        let cat = Catalog::open_in_memory().unwrap();
        let found = merge_params(&cat, "nope", &json!({"x": 1})).unwrap();
        assert!(!found);
    }

    #[test]
    fn commit_refresh_increments_count() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        upsert(&cat, &aug("art1")).unwrap();
        let found = commit_refresh(&cat, "art1").unwrap();
        assert!(found);
        let row = get(&cat, "art1").unwrap().unwrap();
        assert_eq!(row.refresh_count, 1);
        assert!(row.last_refreshed_at.is_some());
    }

    #[test]
    fn commit_refresh_missing_returns_false() {
        let cat = Catalog::open_in_memory().unwrap();
        let found = commit_refresh(&cat, "nope").unwrap();
        assert!(!found);
    }

    #[test]
    fn cascade_delete_removes_augmentation() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        upsert(&cat, &aug("art1")).unwrap();
        crate::catalog::artifact::delete(&cat, "art1").unwrap();
        assert!(get(&cat, "art1").unwrap().is_none());
    }

    #[test]
    fn list_all_ids_returns_augmented() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        art_upsert(&cat, &sample_art("art2")).unwrap();
        upsert(&cat, &aug("art1")).unwrap();
        let ids = list_all_ids(&cat).unwrap();
        assert_eq!(ids, vec!["art1"]);
    }

    #[test]
    fn get_batch_returns_map() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        art_upsert(&cat, &sample_art("art2")).unwrap();
        upsert(&cat, &aug("art1")).unwrap();
        let map = get_batch(&cat, &["art1".to_string(), "art2".to_string()]).unwrap();
        assert!(map.contains_key("art1"));
        assert!(!map.contains_key("art2"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p librarian-mcp augmentation
```

Expected: compile error (todo! panics or functions missing).

- [ ] **Step 3: Implement all functions**

Replace the `todo!()` stubs with real implementations:

```rust
pub fn upsert(cat: &Catalog, row: &AugmentationRow) -> Result<()> {
    cat.conn.execute(
        "INSERT INTO artifact_augmentation
           (artifact_id, prompt, params, last_refreshed_at, refresh_count, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(artifact_id) DO UPDATE SET
           prompt = excluded.prompt,
           params = excluded.params,
           last_refreshed_at = excluded.last_refreshed_at,
           refresh_count = excluded.refresh_count,
           updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        rusqlite::params![
            row.artifact_id,
            row.prompt,
            row.params,
            row.last_refreshed_at,
            row.refresh_count,
            row.created_at,
            row.updated_at,
        ],
    )?;
    Ok(())
}

pub fn get(cat: &Catalog, artifact_id: &str) -> Result<Option<AugmentationRow>> {
    let mut stmt = cat.conn.prepare(
        "SELECT artifact_id, prompt, params, last_refreshed_at, refresh_count,
                created_at, updated_at
         FROM artifact_augmentation WHERE artifact_id = ?1",
    )?;
    let mut rows = stmt.query_map([artifact_id], row_from_sql)?;
    Ok(rows.next().transpose()?)
}

fn row_from_sql(row: &rusqlite::Row<'_>) -> rusqlite::Result<AugmentationRow> {
    Ok(AugmentationRow {
        artifact_id: row.get(0)?,
        prompt: row.get(1)?,
        params: row.get(2)?,
        last_refreshed_at: row.get(3)?,
        refresh_count: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

pub fn merge_params(cat: &Catalog, artifact_id: &str, patch: &Value) -> Result<bool> {
    let Some(existing) = get(cat, artifact_id)? else {
        return Ok(false);
    };
    let mut current: Value = serde_json::from_str(&existing.params)
        .unwrap_or_else(|_| json!({}));
    // RFC 7396 merge-patch
    if let (Value::Object(target), Value::Object(patch_map)) =
        (&mut current, patch)
    {
        for (k, v) in patch_map {
            if v.is_null() {
                target.remove(k);
            } else {
                target.insert(k.clone(), v.clone());
            }
        }
    }
    let new_params = serde_json::to_string(&current)?;
    cat.conn.execute(
        "UPDATE artifact_augmentation SET params = ?1,
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE artifact_id = ?2",
        rusqlite::params![new_params, artifact_id],
    )?;
    Ok(true)
}

pub fn commit_refresh(cat: &Catalog, artifact_id: &str) -> Result<bool> {
    let n = cat.conn.execute(
        "UPDATE artifact_augmentation
         SET last_refreshed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
             refresh_count = refresh_count + 1,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE artifact_id = ?1",
        [artifact_id],
    )?;
    Ok(n > 0)
}

pub fn list_all_ids(cat: &Catalog) -> Result<Vec<String>> {
    let mut stmt = cat
        .conn
        .prepare("SELECT artifact_id FROM artifact_augmentation ORDER BY artifact_id")?;
    let ids = stmt
        .query_map([], |r| r.get(0))?
        .collect::<Result<Vec<String>, _>>()?;
    Ok(ids)
}

pub fn get_batch(
    cat: &Catalog,
    ids: &[String],
) -> Result<std::collections::HashMap<String, AugmentationRow>> {
    if ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let placeholders = ids.iter().enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT artifact_id, prompt, params, last_refreshed_at, refresh_count,
                created_at, updated_at
         FROM artifact_augmentation WHERE artifact_id IN ({placeholders})"
    );
    let mut stmt = cat.conn.prepare(&sql)?;
    let params = rusqlite::params_from_iter(ids.iter());
    let rows = stmt
        .query_map(params, row_from_sql)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().map(|r| (r.artifact_id.clone(), r)).collect())
}
```

Also add `pub mod augmentation;` to `crates/librarian-mcp/src/catalog/mod.rs` alongside the other `pub mod` declarations.

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -p librarian-mcp augmentation
```

Expected: all 9 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/librarian-mcp/src/catalog/augmentation.rs \
        crates/librarian-mcp/src/catalog/mod.rs
git commit -m "feat(librarian): catalog/augmentation CRUD"
```

---

## Task 3: `catalog/observations.rs` — add `list_recent`

**Files:**
- Modify: `crates/librarian-mcp/src/catalog/observations.rs`

- [ ] **Step 1: Write failing test**

Add to the `tests` module in `observations.rs`:

```rust
#[test]
fn list_recent_filters_by_since() {
    use crate::catalog::artifact::{upsert as art_upsert, ArtifactRow};
    let cat = Catalog::open_in_memory().unwrap();
    let now = chrono::Utc::now().timestamp_millis();
    let art = ArtifactRow {
        id: "a1".to_string(), repo: "r".to_string(), rel_path: "a.md".to_string(),
        kind: "tracker".to_string(), status: "active".to_string(), title: None,
        owners: vec![], tags: vec![], topic: None, time_scope: None, source: None,
        created_at: now, updated_at: now, file_mtime: now,
        file_sha256: "x".to_string(), confidence: 1.0,
    };
    art_upsert(&cat, &art).unwrap();

    // insert two observations 100ms apart
    let old_ts = now - 5000;
    let new_ts = now;
    insert(&cat, &ObservationRow {
        id: None, artifact_id: "a1".to_string(),
        text: "old".to_string(), source: None, created_at: old_ts,
    }).unwrap();
    insert(&cat, &ObservationRow {
        id: None, artifact_id: "a1".to_string(),
        text: "new".to_string(), source: None, created_at: new_ts,
    }).unwrap();

    let recent = list_recent(&cat, None, Some(old_ts + 1), 10).unwrap();
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].text, "new");
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p librarian-mcp list_recent_filters_by_since
```

Expected: FAIL — `list_recent` not defined.

- [ ] **Step 3: Implement `list_recent`**

Add to `crates/librarian-mcp/src/catalog/observations.rs` after `list_for_artifact`:

```rust
/// Fetch recent observations, optionally scoped to one artifact and/or a
/// `since` cutoff (ms-epoch). Returns newest first, capped at `limit`.
pub fn list_recent(
    cat: &Catalog,
    artifact_id: Option<&str>,
    since_ms: Option<i64>,
    limit: usize,
) -> Result<Vec<ObservationRow>> {
    let mut parts: Vec<String> = Vec::new();
    let mut param_vals: Vec<rusqlite::types::Value> = Vec::new();

    if let Some(id) = artifact_id {
        parts.push(format!("artifact_id = ?{}", param_vals.len() + 1));
        param_vals.push(rusqlite::types::Value::Text(id.to_string()));
    }
    if let Some(since) = since_ms {
        parts.push(format!("created_at > ?{}", param_vals.len() + 1));
        param_vals.push(rusqlite::types::Value::Integer(since));
    }

    let where_clause = if parts.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", parts.join(" AND "))
    };

    let sql = format!(
        "SELECT id, artifact_id, text, source, created_at
         FROM artifact_observation {where_clause}
         ORDER BY created_at DESC LIMIT ?{}",
        param_vals.len() + 1
    );
    param_vals.push(rusqlite::types::Value::Integer(limit as i64));

    let mut stmt = cat.conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(param_vals.iter()), |row| {
            Ok(ObservationRow {
                id: row.get(0)?,
                artifact_id: row.get(1)?,
                text: row.get(2)?,
                source: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p librarian-mcp observations
```

Expected: all tests PASS including new `list_recent_filters_by_since`.

- [ ] **Step 5: Commit**

```bash
git add crates/librarian-mcp/src/catalog/observations.rs
git commit -m "feat(librarian): catalog/observations — add list_recent"
```

---

## Task 4: `tools/gather.rs` — gather sources

**Files:**
- Create: `crates/librarian-mcp/src/tools/gather.rs`

Check `Cargo.toml` for `librarian-mcp` — if `regex` is not present, add it:

```toml
regex = "1"
```

- [ ] **Step 1: Write failing tests**

Create `crates/librarian-mcp/src/tools/gather.rs`:

```rust
use crate::catalog::{augmentation, find::{find, FindOpts}, observations};
use crate::filter::FilterNode;
use crate::tools::ToolContext;
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum GatherSource {
    GitLog {
        limit: Option<usize>,
        /// "last_refresh" or ISO-8601 timestamp
        since: Option<String>,
        branch: Option<String>,
        grep: Option<String>,
    },
    Artifacts {
        filter: Option<Value>,
        limit: Option<usize>,
    },
    Observations {
        artifact_id: Option<String>,
        limit: Option<usize>,
        /// "last_refresh" or ISO-8601 timestamp
        since: Option<String>,
    },
    File {
        path: String,
    },
    Grep {
        pattern: String,
        path: Option<String>,
        limit: Option<usize>,
    },
    // Forward compat: any unknown source deserialized via catch-all
    #[serde(other)]
    Unknown,
}

pub struct GatherResult {
    pub source_key: String,
    pub data: Value,
}

/// Resolve "last_refresh" or ISO string to ms-epoch i64.
fn resolve_since(since: &str, last_refreshed_at: Option<&str>) -> Option<i64> {
    if since == "last_refresh" {
        last_refreshed_at.and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| dt.timestamp_millis())
        })
    } else {
        chrono::DateTime::parse_from_rfc3339(since)
            .ok()
            .map(|dt| dt.timestamp_millis())
    }
}

/// Gather context from all configured sources. Unknown sources produce
/// a warning. Returns (results, warnings).
pub async fn gather_all(
    sources: &[GatherSource],
    ctx: &ToolContext,
    last_refreshed_at: Option<&str>,
) -> Result<(Vec<GatherResult>, Vec<String>)> {
    let mut results: Vec<GatherResult> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    for source in sources {
        match source {
            GatherSource::Unknown => {
                warnings.push("unknown gather source skipped".to_string());
            }
            GatherSource::GitLog { limit, since, branch, grep } => {
                match gather_git_log(ctx, *limit, since.as_deref(), branch.as_deref(), grep.as_deref(), last_refreshed_at) {
                    Ok(data) => results.push(GatherResult { source_key: "git_log".to_string(), data }),
                    Err(e) => warnings.push(format!("git_log gather failed: {e}")),
                }
            }
            GatherSource::Artifacts { filter, limit } => {
                match gather_artifacts(ctx, filter.as_ref(), *limit) {
                    Ok(data) => results.push(GatherResult { source_key: "artifacts".to_string(), data }),
                    Err(e) => warnings.push(format!("artifacts gather failed: {e}")),
                }
            }
            GatherSource::Observations { artifact_id, limit, since } => {
                let since_ms = since.as_deref().and_then(|s| resolve_since(s, last_refreshed_at));
                match gather_observations(ctx, artifact_id.as_deref(), since_ms, limit.unwrap_or(20)) {
                    Ok(data) => results.push(GatherResult { source_key: "observations".to_string(), data }),
                    Err(e) => warnings.push(format!("observations gather failed: {e}")),
                }
            }
            GatherSource::File { path } => {
                match gather_file(ctx, path) {
                    Ok(data) => results.push(GatherResult { source_key: "file".to_string(), data }),
                    Err(e) => warnings.push(format!("file gather failed for '{path}': {e}")),
                }
            }
            GatherSource::Grep { pattern, path, limit } => {
                match gather_grep(ctx, pattern, path.as_deref(), limit.unwrap_or(50)) {
                    Ok(data) => results.push(GatherResult { source_key: "grep".to_string(), data }),
                    Err(e) => warnings.push(format!("grep gather failed: {e}")),
                }
            }
        }
    }

    Ok((results, warnings))
}

fn project_root(ctx: &ToolContext) -> Option<std::path::PathBuf> {
    ctx.current_project
        .as_ref()
        .and_then(|cp| {
            ctx.workspace.roots.iter()
                .find(|r| r.name == cp.repo)
                .map(|r| r.path.join(&cp.subdir))
        })
        .or_else(|| ctx.workspace.roots.first().map(|r| r.path.clone()))
}

fn gather_git_log(
    ctx: &ToolContext,
    limit: Option<usize>,
    since: Option<&str>,
    branch: Option<&str>,
    grep: Option<&str>,
    last_refreshed_at: Option<&str>,
) -> Result<Value> {
    let root = project_root(ctx)
        .ok_or_else(|| anyhow::anyhow!("no project root"))?;
    let repo = git2::Repository::discover(&root)
        .map_err(|e| anyhow::anyhow!("git repo not found: {e}"))?;

    let since_secs: Option<i64> = since.and_then(|s| {
        resolve_since(s, last_refreshed_at).map(|ms| ms / 1000)
    });

    let mut revwalk = repo.revwalk()?;
    if let Some(branch_name) = branch {
        let branch_ref = repo.find_branch(branch_name, git2::BranchType::Local)
            .or_else(|_| repo.find_branch(branch_name, git2::BranchType::Remote))
            .map_err(|_| anyhow::anyhow!("branch '{branch_name}' not found"))?;
        revwalk.push(branch_ref.get().peel_to_commit()?.id())?;
    } else {
        revwalk.push_head()?;
    }
    revwalk.set_sorting(git2::Sort::TIME)?;

    let limit = limit.unwrap_or(20);
    let grep_re = grep.map(|g| regex::Regex::new(g)).transpose()?;

    let commits: Vec<Value> = revwalk
        .filter_map(|oid| oid.ok())
        .filter_map(|oid| repo.find_commit(oid).ok())
        .filter(|c| since_secs.map_or(true, |ts| c.time().seconds() > ts))
        .filter(|c| {
            grep_re.as_ref().map_or(true, |re| {
                c.summary().map_or(false, |s| re.is_match(s))
            })
        })
        .take(limit)
        .map(|c| json!({
            "hash": &c.id().to_string()[..8],
            "time": c.time().seconds(),
            "subject": c.summary().unwrap_or(""),
            "author": c.author().name().unwrap_or(""),
        }))
        .collect();

    Ok(json!(commits))
}

fn gather_artifacts(ctx: &ToolContext, filter: Option<&Value>, limit: Option<usize>) -> Result<Value> {
    let filter_node: Option<FilterNode> = filter
        .map(|f| serde_json::from_value(f.clone()))
        .transpose()?;
    let cat = ctx.catalog.lock();
    let rows = find(&cat, &FindOpts {
        filter: filter_node,
        limit: limit.unwrap_or(20),
        offset: 0,
        semantic: None,
    })?;
    let items: Vec<Value> = rows.iter().map(|r| json!({
        "id": r.id,
        "kind": r.kind,
        "status": r.status,
        "title": r.title,
        "topic": r.topic,
        "rel_path": r.rel_path,
    })).collect();
    Ok(json!(items))
}

fn gather_observations(
    ctx: &ToolContext,
    artifact_id: Option<&str>,
    since_ms: Option<i64>,
    limit: usize,
) -> Result<Value> {
    let cat = ctx.catalog.lock();
    let obs = observations::list_recent(&cat, artifact_id, since_ms, limit)?;
    let items: Vec<Value> = obs.iter().map(|o| json!({
        "artifact_id": o.artifact_id,
        "text": o.text,
        "source": o.source,
        "created_at": o.created_at,
    })).collect();
    Ok(json!(items))
}

fn gather_file(ctx: &ToolContext, path: &str) -> Result<Value> {
    let base = project_root(ctx).unwrap_or_else(|| std::path::PathBuf::from("."));
    let full = base.join(path);
    let content = std::fs::read_to_string(&full)
        .map_err(|e| anyhow::anyhow!("cannot read '{}': {e}", full.display()))?;
    Ok(json!(content))
}

fn gather_grep(ctx: &ToolContext, pattern: &str, path: Option<&str>, limit: usize) -> Result<Value> {
    use walkdir::WalkDir;
    let base = project_root(ctx).unwrap_or_else(|| std::path::PathBuf::from("."));
    let search_root = path.map(|p| base.join(p)).unwrap_or(base);
    let re = regex::Regex::new(pattern)?;

    let mut matches: Vec<Value> = Vec::new();
    'outer: for entry in WalkDir::new(&search_root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path_str = entry.path().to_string_lossy().to_string();
        if let Ok(content) = std::fs::read_to_string(entry.path()) {
            for (lineno, line) in content.lines().enumerate() {
                if re.is_match(line) {
                    matches.push(json!({
                        "file": path_str,
                        "line": lineno + 1,
                        "text": line.trim(),
                    }));
                    if matches.len() >= limit {
                        break 'outer;
                    }
                }
            }
        }
    }
    Ok(json!(matches))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{artifact, Catalog};
    use crate::workspace::WorkspaceConfig;
    use std::sync::Arc;
    use parking_lot::Mutex;

    fn mk_ctx(cat: Catalog) -> ToolContext {
        ToolContext {
            catalog: Arc::new(Mutex::new(cat)),
            workspace: Arc::new(WorkspaceConfig::default()),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: None,
        }
    }

    #[tokio::test]
    async fn gather_artifacts_returns_rows() {
        let cat = Catalog::open_in_memory().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        artifact::upsert(&cat, &artifact::ArtifactRow {
            id: "a1".to_string(), repo: "r".to_string(), rel_path: "a.md".to_string(),
            kind: "spec".to_string(), status: "active".to_string(), title: Some("A".to_string()),
            owners: vec![], tags: vec![], topic: None, time_scope: None, source: None,
            created_at: now, updated_at: now, file_mtime: now,
            file_sha256: "x".to_string(), confidence: 1.0,
        }).unwrap();
        let ctx = mk_ctx(cat);
        let sources = vec![GatherSource::Artifacts { filter: None, limit: Some(10) }];
        let (results, warnings) = gather_all(&sources, &ctx, None).await.unwrap();
        assert!(warnings.is_empty());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_key, "artifacts");
        let arr = results[0].data.as_array().unwrap();
        assert_eq!(arr[0]["id"], "a1");
    }

    #[tokio::test]
    async fn unknown_source_produces_warning() {
        let cat = Catalog::open_in_memory().unwrap();
        let ctx = mk_ctx(cat);
        // Deserialize a raw JSON to GatherSource::Unknown
        let raw = serde_json::from_value::<GatherSource>(
            serde_json::json!({"source": "nonexistent_source"})
        ).unwrap();
        let sources = vec![raw];
        let (results, warnings) = gather_all(&sources, &ctx, None).await.unwrap();
        assert!(results.is_empty());
        assert_eq!(warnings.len(), 1);
    }

    #[tokio::test]
    async fn gather_file_returns_content() {
        use std::io::Write;
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("notes.md");
        let mut f = std::fs::File::create(&file_path).unwrap();
        writeln!(f, "hello world").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        let mut ws = WorkspaceConfig::default();
        ws.roots.push(crate::workspace::Root {
            name: "repo".to_string(),
            path: tmp.path().to_path_buf(),
        });
        let ctx = ToolContext {
            catalog: Arc::new(Mutex::new(cat)),
            workspace: Arc::new(ws),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: None,
        };
        let sources = vec![GatherSource::File { path: "notes.md".to_string() }];
        let (results, warnings) = gather_all(&sources, &ctx, None).await.unwrap();
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert!(results[0].data.as_str().unwrap().contains("hello world"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p librarian-mcp gather
```

Expected: compile errors (module not declared yet).

- [ ] **Step 3: Declare module in `tools/mod.rs`**

Add `pub mod gather;` to `crates/librarian-mcp/src/tools/mod.rs` with the other `mod` declarations.

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -p librarian-mcp gather
```

Expected: all gather tests PASS (git_log tests may be skipped if no git repo present in test env — that's fine).

- [ ] **Step 5: Commit**

```bash
git add crates/librarian-mcp/src/tools/gather.rs \
        crates/librarian-mcp/src/tools/mod.rs \
        crates/librarian-mcp/Cargo.toml
git commit -m "feat(librarian): gather sources module (git_log, artifacts, observations, file, grep)"
```

---

## Task 5: `tools/augment.rs` — `ArtifactAugment` tool

**Files:**
- Create: `crates/librarian-mcp/src/tools/augment.rs`
- Modify: `crates/librarian-mcp/src/tools/mod.rs`

- [ ] **Step 1: Write failing test**

Create `crates/librarian-mcp/src/tools/augment.rs` with stub + test:

```rust
use crate::catalog::{artifact, augmentation};
use crate::tools::{RecoverableError, Tool, ToolContext};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

pub struct ArtifactAugment;

#[derive(Deserialize)]
struct Args {
    id: String,
    prompt: String,
    params: Option<Value>,
}

impl Tool for ArtifactAugment {
    fn name(&self) -> &'static str { "artifact_augment" }

    fn description(&self) -> &'static str {
        "Attach or replace a persistent prompt + params on any artifact, enabling \
         server-assisted refresh. Idempotent — safe to call on already-augmented artifacts."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["id", "prompt"],
            "properties": {
                "id": { "type": "string", "description": "Artifact id" },
                "prompt": {
                    "type": "string",
                    "description": "Persistent instruction: what to maintain and how to format it"
                },
                "params": {
                    "type": "object",
                    "description": "Optional gather config (gather_from, format, max_tokens). Defaults to {}."
                }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args)?;
        let cat = ctx.catalog.lock();

        // Verify artifact exists
        if artifact::get(&cat, &a.id)?.is_none() {
            return Err(RecoverableError::new(format!(
                "artifact '{}' not found", a.id
            )).into());
        }

        let params_str = a.params
            .map(|p| serde_json::to_string(&p))
            .transpose()?
            .unwrap_or_else(|| "{}".to_string());

        let now = chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();

        augmentation::upsert(&cat, &augmentation::AugmentationRow {
            artifact_id: a.id.clone(),
            prompt: a.prompt,
            params: params_str,
            last_refreshed_at: None,
            refresh_count: 0,
            created_at: now.clone(),
            updated_at: now,
        })?;

        Ok(json!("ok"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;
    use crate::workspace::WorkspaceConfig;
    use parking_lot::Mutex;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn mk_ctx(tmp: &TempDir) -> ToolContext {
        let cat = Catalog::open_in_memory().unwrap();
        let mut ws = WorkspaceConfig::default();
        ws.roots.push(crate::workspace::Root {
            name: "repo".to_string(),
            path: tmp.path().to_path_buf(),
        });
        ToolContext {
            catalog: Arc::new(Mutex::new(cat)),
            workspace: Arc::new(ws),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: None,
        }
    }

    fn seed_artifact(ctx: &ToolContext, id: &str) {
        let now = chrono::Utc::now().timestamp_millis();
        let cat = ctx.catalog.lock();
        artifact::upsert(&cat, &artifact::ArtifactRow {
            id: id.to_string(), repo: "repo".to_string(),
            rel_path: format!("{id}.md"), kind: "tracker".to_string(),
            status: "active".to_string(), title: Some("T".to_string()),
            owners: vec![], tags: vec![], topic: None, time_scope: None, source: None,
            created_at: now, updated_at: now, file_mtime: now,
            file_sha256: "x".to_string(), confidence: 1.0,
        }).unwrap();
    }

    #[tokio::test]
    async fn creates_augmentation_row() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(&tmp);
        seed_artifact(&ctx, "art1");
        let result = ArtifactAugment.call(&ctx, json!({
            "id": "art1",
            "prompt": "Keep me updated",
            "params": {"format": "table"}
        })).await.unwrap();
        assert_eq!(result, json!("ok"));
        let cat = ctx.catalog.lock();
        let row = augmentation::get(&cat, "art1").unwrap().unwrap();
        assert_eq!(row.prompt, "Keep me updated");
        let params: Value = serde_json::from_str(&row.params).unwrap();
        assert_eq!(params["format"], "table");
    }

    #[tokio::test]
    async fn idempotent_update_replaces_prompt() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(&tmp);
        seed_artifact(&ctx, "art1");
        ArtifactAugment.call(&ctx, json!({"id": "art1", "prompt": "Old"})).await.unwrap();
        ArtifactAugment.call(&ctx, json!({"id": "art1", "prompt": "New"})).await.unwrap();
        let cat = ctx.catalog.lock();
        let row = augmentation::get(&cat, "art1").unwrap().unwrap();
        assert_eq!(row.prompt, "New");
    }

    #[tokio::test]
    async fn missing_artifact_returns_recoverable_error() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(&tmp);
        let err = ArtifactAugment.call(&ctx, json!({
            "id": "nope",
            "prompt": "Test"
        })).await.unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p librarian-mcp augment
```

Expected: FAIL (stub returns todo! or compile error).

- [ ] **Step 3: Implement (the code above already has the full implementation)**

The implementation is in the `call` method above — remove any `todo!()` stubs if present.

- [ ] **Step 4: Add module declaration to `tools/mod.rs`**

Add `pub mod augment;` to `crates/librarian-mcp/src/tools/mod.rs`.

- [ ] **Step 5: Run tests**

```bash
cargo test -p librarian-mcp augment
```

Expected: all 3 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/librarian-mcp/src/tools/augment.rs \
        crates/librarian-mcp/src/tools/mod.rs
git commit -m "feat(librarian): artifact_augment tool"
```

---

## Task 6: `tools/update_params.rs` — `ArtifactUpdateParams` tool

**Files:**
- Create: `crates/librarian-mcp/src/tools/update_params.rs`
- Modify: `crates/librarian-mcp/src/tools/mod.rs`

- [ ] **Step 1: Create with full implementation + tests**

```rust
use crate::catalog::augmentation;
use crate::tools::{RecoverableError, Tool, ToolContext};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

pub struct ArtifactUpdateParams;

#[derive(Deserialize)]
struct Args {
    id: String,
    params: Value,
}

impl Tool for ArtifactUpdateParams {
    fn name(&self) -> &'static str { "artifact_update_params" }

    fn description(&self) -> &'static str {
        "Merge-patch the params JSON of an augmented artifact (RFC 7396). \
         Keys set to null are deleted; present keys are merged. \
         Call this mid-session to tune gather sources without touching the prompt or body."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["id", "params"],
            "properties": {
                "id": { "type": "string" },
                "params": {
                    "type": "object",
                    "description": "Partial params to merge. Set a key to null to delete it."
                }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args)?;
        let cat = ctx.catalog.lock();
        let found = augmentation::merge_params(&cat, &a.id, &a.params)?;
        if !found {
            return Err(RecoverableError::new(format!(
                "no augmentation for artifact '{}' — call artifact_augment first", a.id
            )).into());
        }
        Ok(json!("ok"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{artifact, augmentation, Catalog};
    use crate::workspace::WorkspaceConfig;
    use parking_lot::Mutex;
    use std::sync::Arc;

    fn mk_ctx(cat: Catalog) -> ToolContext {
        ToolContext {
            catalog: Arc::new(Mutex::new(cat)),
            workspace: Arc::new(WorkspaceConfig::default()),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: None,
        }
    }

    fn seed(cat: &Catalog, id: &str, params: &str) {
        let now = chrono::Utc::now().timestamp_millis();
        artifact::upsert(cat, &artifact::ArtifactRow {
            id: id.to_string(), repo: "r".to_string(), rel_path: format!("{id}.md"),
            kind: "tracker".to_string(), status: "active".to_string(), title: None,
            owners: vec![], tags: vec![], topic: None, time_scope: None, source: None,
            created_at: now, updated_at: now, file_mtime: now,
            file_sha256: "x".to_string(), confidence: 1.0,
        }).unwrap();
        augmentation::upsert(cat, &augmentation::AugmentationRow {
            artifact_id: id.to_string(), prompt: "p".to_string(),
            params: params.to_string(), last_refreshed_at: None,
            refresh_count: 0,
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
        }).unwrap();
    }

    #[tokio::test]
    async fn merge_adds_key() {
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1", r#"{"format":"bullets"}"#);
        let ctx = mk_ctx(cat);
        ArtifactUpdateParams.call(&ctx, json!({"id": "a1", "params": {"max_tokens": 2000}}))
            .await.unwrap();
        let cat = ctx.catalog.lock();
        let row = augmentation::get(&cat, "a1").unwrap().unwrap();
        let p: Value = serde_json::from_str(&row.params).unwrap();
        assert_eq!(p["format"], "bullets");
        assert_eq!(p["max_tokens"], 2000);
    }

    #[tokio::test]
    async fn null_deletes_key() {
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1", r#"{"format":"table","max_tokens":3000}"#);
        let ctx = mk_ctx(cat);
        ArtifactUpdateParams.call(&ctx, json!({"id": "a1", "params": {"format": null}}))
            .await.unwrap();
        let cat = ctx.catalog.lock();
        let row = augmentation::get(&cat, "a1").unwrap().unwrap();
        let p: Value = serde_json::from_str(&row.params).unwrap();
        assert!(p.get("format").is_none());
        assert_eq!(p["max_tokens"], 3000);
    }

    #[tokio::test]
    async fn missing_augmentation_returns_recoverable() {
        let cat = Catalog::open_in_memory().unwrap();
        let ctx = mk_ctx(cat);
        let err = ArtifactUpdateParams.call(&ctx, json!({"id": "nope", "params": {}}))
            .await.unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }
}
```

- [ ] **Step 2: Add module, run tests**

Add `pub mod update_params;` to `tools/mod.rs`.

```bash
cargo test -p librarian-mcp update_params
```

Expected: all 3 tests PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/librarian-mcp/src/tools/update_params.rs \
        crates/librarian-mcp/src/tools/mod.rs
git commit -m "feat(librarian): artifact_update_params tool"
```

---

## Task 7: `tools/refresh.rs` — `ArtifactRefresh` tool

**Files:**
- Create: `crates/librarian-mcp/src/tools/refresh.rs`
- Modify: `crates/librarian-mcp/src/tools/mod.rs`

- [ ] **Step 1: Create with full implementation + tests**

```rust
use crate::catalog::augmentation;
use crate::tools::gather::{gather_all, GatherSource};
use crate::tools::{RecoverableError, Tool, ToolContext};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct ArtifactRefresh;

#[derive(Deserialize)]
struct Args {
    id: String,
}

impl Tool for ArtifactRefresh {
    fn name(&self) -> &'static str { "artifact_refresh" }

    fn description(&self) -> &'static str {
        "Gather context for an augmented artifact and return a refresh package \
         (prompt + current_body + gathered context). Does NOT write anything — \
         synthesize new content from the package then call artifact_update to write back, \
         then artifact_refresh_commit to record the refresh."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": { "type": "string", "description": "Artifact id" }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args)?;

        let aug_row = {
            let cat = ctx.catalog.lock();
            augmentation::get(&cat, &a.id)?
        };

        let aug = aug_row.ok_or_else(|| {
            RecoverableError::new(format!(
                "no augmentation for artifact '{}' — call artifact_augment first", a.id
            ))
        })?;

        // Parse gather sources from params
        let params: Value = serde_json::from_str(&aug.params).unwrap_or_else(|_| json!({}));
        let sources: Vec<GatherSource> = params
            .get("gather_from")
            .and_then(|g| serde_json::from_value(g.clone()).ok())
            .unwrap_or_default();

        // Gather context
        let (results, mut warnings) = gather_all(&sources, ctx, aug.last_refreshed_at.as_deref()).await?;

        // Build context map
        let mut context: HashMap<String, Value> = HashMap::new();
        for r in results {
            context.entry(r.source_key.clone())
                .and_modify(|existing| {
                    // If same source appears twice, merge arrays
                    if let (Value::Array(a), Value::Array(b)) =
                        (existing, &r.data)
                    {
                        a.extend(b.clone());
                    }
                })
                .or_insert(r.data);
        }
        if !warnings.is_empty() {
            context.insert("warnings".to_string(), json!(warnings));
        }

        // Read current body from disk
        let current_body = read_body(ctx, &a.id)?;

        // Build hints
        let mut hints: Vec<String> = Vec::new();
        for (key, val) in &context {
            if key == "warnings" { continue; }
            if let Some(arr) = val.as_array() {
                hints.push(format!("{} items gathered from {key}", arr.len()));
            }
        }

        Ok(json!({
            "artifact_id": a.id,
            "prompt": aug.prompt,
            "params": params,
            "current_body": current_body,
            "context": context,
            "last_refreshed_at": aug.last_refreshed_at,
            "hints": hints,
        }))
    }
}

fn read_body(ctx: &ToolContext, artifact_id: &str) -> Result<Option<String>> {
    let cat = ctx.catalog.lock();
    let row = match crate::catalog::artifact::get(&cat, artifact_id)? {
        Some(r) => r,
        None => return Ok(None),
    };
    let root_map: HashMap<String, std::path::PathBuf> = ctx.workspace.roots.iter()
        .map(|r| (r.name.clone(), r.path.clone()))
        .collect();
    let Some(root) = root_map.get(&row.repo) else { return Ok(None); };
    let full_path = root.join(&row.rel_path);
    match std::fs::read_to_string(&full_path) {
        Ok(content) => {
            let body = match crate::frontmatter::parse(&content) {
                Ok((_, b)) => b.to_string(),
                Err(_) => content,
            };
            Ok(Some(body))
        }
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{artifact, Catalog};
    use crate::workspace::WorkspaceConfig;
    use parking_lot::Mutex;
    use std::sync::Arc;

    fn mk_ctx(cat: Catalog) -> ToolContext {
        ToolContext {
            catalog: Arc::new(Mutex::new(cat)),
            workspace: Arc::new(WorkspaceConfig::default()),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: None,
        }
    }

    fn seed_art(cat: &Catalog, id: &str) {
        let now = chrono::Utc::now().timestamp_millis();
        artifact::upsert(cat, &artifact::ArtifactRow {
            id: id.to_string(), repo: "r".to_string(), rel_path: format!("{id}.md"),
            kind: "tracker".to_string(), status: "active".to_string(), title: None,
            owners: vec![], tags: vec![], topic: None, time_scope: None, source: None,
            created_at: now, updated_at: now, file_mtime: now,
            file_sha256: "x".to_string(), confidence: 1.0,
        }).unwrap();
    }

    fn seed_aug(cat: &Catalog, id: &str, params: &str) {
        augmentation::upsert(cat, &augmentation::AugmentationRow {
            artifact_id: id.to_string(), prompt: "Maintain state".to_string(),
            params: params.to_string(), last_refreshed_at: None, refresh_count: 0,
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
        }).unwrap();
    }

    #[tokio::test]
    async fn returns_package_fields() {
        let cat = Catalog::open_in_memory().unwrap();
        seed_art(&cat, "a1");
        seed_aug(&cat, "a1", "{}");
        let ctx = mk_ctx(cat);
        let result = ArtifactRefresh.call(&ctx, json!({"id": "a1"})).await.unwrap();
        assert_eq!(result["artifact_id"], "a1");
        assert_eq!(result["prompt"], "Maintain state");
        assert!(result.get("context").is_some());
        assert!(result.get("hints").is_some());
    }

    #[tokio::test]
    async fn unknown_source_in_params_produces_warning() {
        let cat = Catalog::open_in_memory().unwrap();
        seed_art(&cat, "a1");
        seed_aug(&cat, "a1", r#"{"gather_from": [{"source": "future_source"}]}"#);
        let ctx = mk_ctx(cat);
        let result = ArtifactRefresh.call(&ctx, json!({"id": "a1"})).await.unwrap();
        let warnings = &result["context"]["warnings"];
        assert!(warnings.as_array().map_or(false, |a| !a.is_empty()));
    }

    #[tokio::test]
    async fn missing_augmentation_returns_recoverable() {
        let cat = Catalog::open_in_memory().unwrap();
        seed_art(&cat, "a1");
        let ctx = mk_ctx(cat);
        let err = ArtifactRefresh.call(&ctx, json!({"id": "a1"})).await.unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }
}
```

- [ ] **Step 2: Add `pub mod refresh;` to `tools/mod.rs`, run tests**

```bash
cargo test -p librarian-mcp refresh
```

Expected: all 3 tests PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/librarian-mcp/src/tools/refresh.rs \
        crates/librarian-mcp/src/tools/mod.rs
git commit -m "feat(librarian): artifact_refresh tool"
```

---

## Task 8: `tools/refresh_commit.rs` — `ArtifactRefreshCommit` tool

**Files:**
- Create: `crates/librarian-mcp/src/tools/refresh_commit.rs`
- Modify: `crates/librarian-mcp/src/tools/mod.rs`

- [ ] **Step 1: Create with full implementation + tests**

```rust
use crate::catalog::augmentation;
use crate::tools::{Tool, ToolContext};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

pub struct ArtifactRefreshCommit;

#[derive(Deserialize)]
struct Args {
    id: String,
}

impl Tool for ArtifactRefreshCommit {
    fn name(&self) -> &'static str { "artifact_refresh_commit" }

    fn description(&self) -> &'static str {
        "Signal that a refresh cycle is complete. Increments refresh_count and sets \
         last_refreshed_at. Call this after artifact_update in every refresh cycle. \
         No-ops gracefully if the augmentation row has been deleted."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": { "type": "string", "description": "Artifact id" }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args)?;
        let cat = ctx.catalog.lock();
        let found = augmentation::commit_refresh(&cat, &a.id)?;
        Ok(json!({ "committed": found }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{artifact, augmentation, Catalog};
    use crate::workspace::WorkspaceConfig;
    use parking_lot::Mutex;
    use std::sync::Arc;

    fn mk_ctx(cat: Catalog) -> ToolContext {
        ToolContext {
            catalog: Arc::new(Mutex::new(cat)),
            workspace: Arc::new(WorkspaceConfig::default()),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: None,
        }
    }

    fn seed(cat: &Catalog, id: &str) {
        let now = chrono::Utc::now().timestamp_millis();
        artifact::upsert(cat, &artifact::ArtifactRow {
            id: id.to_string(), repo: "r".to_string(), rel_path: format!("{id}.md"),
            kind: "tracker".to_string(), status: "active".to_string(), title: None,
            owners: vec![], tags: vec![], topic: None, time_scope: None, source: None,
            created_at: now, updated_at: now, file_mtime: now,
            file_sha256: "x".to_string(), confidence: 1.0,
        }).unwrap();
        augmentation::upsert(cat, &augmentation::AugmentationRow {
            artifact_id: id.to_string(), prompt: "p".to_string(), params: "{}".to_string(),
            last_refreshed_at: None, refresh_count: 0,
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
        }).unwrap();
    }

    #[tokio::test]
    async fn increments_count_and_sets_timestamp() {
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        let ctx = mk_ctx(cat);
        let r = ArtifactRefreshCommit.call(&ctx, json!({"id": "a1"})).await.unwrap();
        assert_eq!(r["committed"], true);
        let cat = ctx.catalog.lock();
        let row = augmentation::get(&cat, "a1").unwrap().unwrap();
        assert_eq!(row.refresh_count, 1);
        assert!(row.last_refreshed_at.is_some());
    }

    #[tokio::test]
    async fn missing_row_returns_committed_false() {
        let cat = Catalog::open_in_memory().unwrap();
        let ctx = mk_ctx(cat);
        let r = ArtifactRefreshCommit.call(&ctx, json!({"id": "nope"})).await.unwrap();
        assert_eq!(r["committed"], false);
    }
}
```

- [ ] **Step 2: Add `pub mod refresh_commit;` to `tools/mod.rs`, run tests**

```bash
cargo test -p librarian-mcp refresh_commit
```

Expected: both tests PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/librarian-mcp/src/tools/refresh_commit.rs \
        crates/librarian-mcp/src/tools/mod.rs
git commit -m "feat(librarian): artifact_refresh_commit tool"
```

---

## Task 9: `tools/tracker_create.rs` — `TrackerCreate` tool

**Files:**
- Create: `crates/librarian-mcp/src/tools/tracker_create.rs`
- Modify: `crates/librarian-mcp/src/tools/mod.rs`

- [ ] **Step 1: Create with full implementation + tests**

```rust
use crate::catalog::{artifact, augmentation};
use crate::ids::artifact_id;
use crate::tools::{RecoverableError, Tool, ToolContext};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

pub struct TrackerCreate;

#[derive(Deserialize)]
struct Args {
    repo: String,
    rel_path: String,
    title: String,
    prompt: String,
    params: Option<Value>,
}

impl Tool for TrackerCreate {
    fn name(&self) -> &'static str { "tracker_create" }

    fn description(&self) -> &'static str {
        "Atomically create a tracker artifact (kind=tracker) and attach augmentation \
         in one call. Shorthand for artifact_create + artifact_augment. \
         Returns the new artifact id."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["repo", "rel_path", "title", "prompt"],
            "properties": {
                "repo": { "type": "string", "description": "Repository name (as configured in workspace.toml)" },
                "rel_path": { "type": "string", "description": "Relative path for the new file (e.g. 'trackers/features.md')" },
                "title": { "type": "string" },
                "prompt": { "type": "string", "description": "Persistent refresh instruction" },
                "params": { "type": "object", "description": "Optional gather config" }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args)?;

        // Validate rel_path
        if a.rel_path.contains("..") || std::path::Path::new(&a.rel_path).is_absolute() {
            return Err(RecoverableError::new(
                "rel_path must be relative and must not contain '..'"
            ).into());
        }

        // Resolve repo root
        let repo_root = ctx.workspace.roots.iter()
            .find(|r| r.name == a.repo)
            .map(|r| r.path.clone())
            .ok_or_else(|| RecoverableError::new(
                format!("repo '{}' not found in workspace", a.repo)
            ))?;

        let full_path = repo_root.join(&a.rel_path);

        // Refuse to overwrite
        if full_path.exists() {
            return Err(RecoverableError::new(
                format!("file already exists: {}", full_path.display())
            ).into());
        }

        // Write file
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let frontmatter = format!(
            "---\nkind: tracker\nstatus: active\ntitle: \"{}\"\n---\n\n",
            a.title.replace('"', "\\\"")
        );
        std::fs::write(&full_path, &frontmatter)?;

        let id = artifact_id(&a.repo, &a.rel_path);
        let now = chrono::Utc::now().timestamp_millis();

        let params_str = a.params
            .map(|p| serde_json::to_string(&p))
            .transpose()?
            .unwrap_or_else(|| "{}".to_string());

        // Atomic: both writes in one catalog lock
        {
            let cat = ctx.catalog.lock();

            artifact::upsert(&cat, &artifact::ArtifactRow {
                id: id.clone(),
                repo: a.repo,
                rel_path: a.rel_path,
                kind: "tracker".to_string(),
                status: "active".to_string(),
                title: Some(a.title),
                owners: vec![],
                tags: vec![],
                topic: None,
                time_scope: None,
                source: None,
                created_at: now,
                updated_at: now,
                file_mtime: now,
                file_sha256: "".to_string(), // will be recomputed on next reindex
                confidence: 1.0,
            })?;

            let ts = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
            augmentation::upsert(&cat, &augmentation::AugmentationRow {
                artifact_id: id.clone(),
                prompt: a.prompt,
                params: params_str,
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: ts.clone(),
                updated_at: ts,
            })?;
        }

        Ok(json!({ "id": id }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{augmentation, Catalog};
    use crate::workspace::WorkspaceConfig;
    use parking_lot::Mutex;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn mk_ctx(tmp: &TempDir) -> ToolContext {
        let cat = Catalog::open_in_memory().unwrap();
        let mut ws = WorkspaceConfig::default();
        ws.roots.push(crate::workspace::Root {
            name: "repo".to_string(),
            path: tmp.path().to_path_buf(),
        });
        ToolContext {
            catalog: Arc::new(Mutex::new(cat)),
            workspace: Arc::new(ws),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: None,
        }
    }

    #[tokio::test]
    async fn creates_file_artifact_and_augmentation() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(&tmp);
        let result = TrackerCreate.call(&ctx, json!({
            "repo": "repo",
            "rel_path": "trackers/features.md",
            "title": "Feature State",
            "prompt": "Keep features updated",
            "params": {"format": "table"}
        })).await.unwrap();

        let id = result["id"].as_str().unwrap();
        assert!(!id.is_empty());

        // File exists
        assert!(tmp.path().join("trackers/features.md").exists());

        // Augmentation row exists
        let cat = ctx.catalog.lock();
        let aug = augmentation::get(&cat, id).unwrap().unwrap();
        assert_eq!(aug.prompt, "Keep features updated");

        // Artifact kind = tracker
        let art = crate::catalog::artifact::get(&cat, id).unwrap().unwrap();
        assert_eq!(art.kind, "tracker");
    }

    #[tokio::test]
    async fn refuses_if_file_exists() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("exists.md"), "x").unwrap();
        let ctx = mk_ctx(&tmp);
        let err = TrackerCreate.call(&ctx, json!({
            "repo": "repo", "rel_path": "exists.md",
            "title": "T", "prompt": "p"
        })).await.unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }

    #[tokio::test]
    async fn rejects_dotdot_path() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(&tmp);
        let err = TrackerCreate.call(&ctx, json!({
            "repo": "repo", "rel_path": "../escape.md",
            "title": "T", "prompt": "p"
        })).await.unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }
}
```

- [ ] **Step 2: Add `pub mod tracker_create;` to `tools/mod.rs`, run tests**

```bash
cargo test -p librarian-mcp tracker_create
```

Expected: all 3 tests PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/librarian-mcp/src/tools/tracker_create.rs \
        crates/librarian-mcp/src/tools/mod.rs
git commit -m "feat(librarian): tracker_create atomic tool"
```

---

## Task 10: Modify `tools/get.rs` — include augmentation in response

**Files:**
- Modify: `crates/librarian-mcp/src/tools/get.rs`

- [ ] **Step 1: Write failing test**

Add to the `tests` module in `tools/get.rs`:

```rust
#[test]
fn get_includes_augmentation_when_present() {
    use crate::catalog::augmentation;
    let cat = Catalog::open_in_memory().unwrap();
    let row = mk_row("aug-art");
    let (ctx, _tmp) = mk_ctx_with_root(cat);
    {
        let cat = ctx.catalog.lock();
        crate::catalog::artifact::upsert(&cat, &row).unwrap();
        augmentation::upsert(&cat, &augmentation::AugmentationRow {
            artifact_id: "aug-art".to_string(),
            prompt: "Keep updated".to_string(),
            params: r#"{"format":"table"}"#.to_string(),
            last_refreshed_at: Some("2026-05-01T00:00:00.000Z".to_string()),
            refresh_count: 5,
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
        }).unwrap();
    }
    let result = tokio::runtime::Runtime::new().unwrap().block_on(
        ArtifactGet.call(&ctx, serde_json::json!({"id": "aug-art"}))
    ).unwrap();
    let aug = &result["augmentation"];
    assert_eq!(aug["prompt"], "Keep updated");
    assert_eq!(aug["refresh_count"], 5);
    assert_eq!(aug["last_refreshed_at"], "2026-05-01T00:00:00.000Z");
}

#[test]
fn get_omits_augmentation_when_absent() {
    let cat = Catalog::open_in_memory().unwrap();
    let row = mk_row("plain-art");
    let (ctx, _tmp) = mk_ctx_with_root(cat);
    {
        let cat = ctx.catalog.lock();
        crate::catalog::artifact::upsert(&cat, &row).unwrap();
    }
    let result = tokio::runtime::Runtime::new().unwrap().block_on(
        ArtifactGet.call(&ctx, serde_json::json!({"id": "plain-art"}))
    ).unwrap();
    assert!(result.get("augmentation").is_none() || result["augmentation"].is_null());
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p librarian-mcp get_includes_augmentation get_omits_augmentation
```

Expected: FAIL (augmentation field missing from response).

- [ ] **Step 3: Modify `ArtifactGet::call` to include augmentation**

In `crates/librarian-mcp/src/tools/get.rs`, locate the `call` method return statement (near line 370). Before the `Ok(json!({...}))`, add:

```rust
let augmentation_val: Value = {
    let cat = ctx.catalog.lock();
    match crate::catalog::augmentation::get(&cat, &a.id)? {
        Some(aug) => {
            let params: Value = serde_json::from_str(&aug.params).unwrap_or(json!({}));
            json!({
                "prompt": aug.prompt,
                "params": params,
                "last_refreshed_at": aug.last_refreshed_at,
                "refresh_count": aug.refresh_count,
            })
        }
        None => Value::Null,
    }
};
```

Then add `"augmentation": augmentation_val` to the returned JSON object (only when not null — or always include it; including null is fine for schema consistency).

- [ ] **Step 4: Run tests**

```bash
cargo test -p librarian-mcp get_
```

Expected: new tests PASS, existing get tests still PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/librarian-mcp/src/tools/get.rs
git commit -m "feat(librarian): artifact_get includes augmentation field when present"
```

---

## Task 11: Modify `tools/find.rs` — `augmented` filter

**Files:**
- Modify: `crates/librarian-mcp/src/tools/find.rs`

- [ ] **Step 1: Write failing test**

Add to the `tests` module in `tools/find.rs`:

```rust
#[test]
fn augmented_true_returns_only_augmented() {
    use crate::catalog::augmentation;
    let cat = Catalog::open_in_memory().unwrap();
    let plain = sample_row("plain", "Plain");
    let augmented = sample_row("aug", "Augmented");
    {
        let ctx = mk_ctx(cat);
        {
            let cat = ctx.catalog.lock();
            crate::catalog::artifact::upsert(&cat, &plain).unwrap();
            crate::catalog::artifact::upsert(&cat, &augmented).unwrap();
            augmentation::upsert(&cat, &augmentation::AugmentationRow {
                artifact_id: "aug".to_string(),
                prompt: "p".to_string(), params: "{}".to_string(),
                last_refreshed_at: None, refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
            }).unwrap();
        }
        let result = tokio::runtime::Runtime::new().unwrap().block_on(
            ArtifactFind.call(&ctx, serde_json::json!({"augmented": true}))
        ).unwrap();
        let rows = result["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], "aug");
    }
}
```

(Note: `sample_row` in find tests has signature `fn(id: &str, title: &str) -> ArtifactRow` — use that.)

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p librarian-mcp augmented_true_returns_only_augmented
```

Expected: FAIL — `augmented` field not recognized.

- [ ] **Step 3: Add `augmented` field to `Args` and implement**

In `tools/find.rs`, add `augmented: Option<bool>` to the `Args` struct:

```rust
#[derive(Deserialize)]
struct Args {
    filter: Option<FilterNode>,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
    semantic: Option<String>,
    scope: Option<Scope>,
    #[serde(default)]
    include_archived: bool,
    augmented: Option<bool>,  // ← add this
}
```

In the `input_schema` method, add to the `properties` object:

```rust
"augmented": {
    "type": "boolean",
    "description": "Filter to augmented (true) or non-augmented (false) artifacts. Omit to return all."
}
```

At the top of `call()`, after deserializing `Args`, add the augmented filter injection:

```rust
let augmented_filter: Option<FilterNode> = if let Some(want_augmented) = a.augmented {
    let ids = {
        let cat = ctx.catalog.lock();
        crate::catalog::augmentation::list_all_ids(&cat)?
    };
    if want_augmented {
        if ids.is_empty() {
            return Ok(json!({"rows": [], "count": 0, "hints": {}}));
        }
        let id_values: Vec<Value> = ids.into_iter().map(|id| json!(id)).collect();
        Some(FilterNode::Leaf(
            [("id".to_string(), json!({"in": id_values}))].into_iter().collect()
        ))
    } else {
        if ids.is_empty() {
            None
        } else {
            let id_values: Vec<Value> = ids.into_iter().map(|id| json!(id)).collect();
            Some(FilterNode::Leaf(
                [("id".to_string(), json!({"nin": id_values}))].into_iter().collect()
            ))
        }
    }
} else {
    None
};

let user_filter = match (a.filter, augmented_filter) {
    (Some(f), Some(af)) => Some(FilterNode::And { and: vec![f, af] }),
    (Some(f), None) => Some(f),
    (None, Some(af)) => Some(af),
    (None, None) => None,
};
```

Then replace all subsequent uses of `a.filter` with `user_filter`.

- [ ] **Step 4: Run all find tests**

```bash
cargo test -p librarian-mcp find
```

Expected: all tests PASS including new augmented test.

- [ ] **Step 5: Commit**

```bash
git add crates/librarian-mcp/src/tools/find.rs
git commit -m "feat(librarian): artifact_find augmented filter"
```

---

## Task 12: Modify `tools/context.rs` — `[LIVE]` rendering + tracker priority

**Files:**
- Modify: `crates/librarian-mcp/src/tools/context.rs`

- [ ] **Step 1: Write failing test**

Add to the `tests` module in `tools/context.rs`:

```rust
#[test]
fn live_header_present_for_augmented_artifact() {
    use crate::catalog::augmentation;
    use std::io::Write;

    let tmp = tempfile::tempdir().unwrap();
    let cat = Catalog::open_in_memory().unwrap();
    let ctx = mk_ctx(tmp.path().to_path_buf(), cat);

    // Seed an augmented tracker artifact
    let row = sample_row("aug-id", "repo", "tracker.md", "My Tracker", Some("test-topic"));
    {
        let cat = ctx.catalog.lock();
        crate::catalog::artifact::upsert(&cat, &row).unwrap();
        augmentation::upsert(&cat, &augmentation::AugmentationRow {
            artifact_id: "aug-id".to_string(),
            prompt: "Maintain state".to_string(),
            params: "{}".to_string(),
            last_refreshed_at: Some("2026-05-01T00:00:00.000Z".to_string()),
            refresh_count: 3,
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
        }).unwrap();
    }

    // Write the file
    let repo_root = ctx.workspace.roots[0].path.clone();
    std::fs::create_dir_all(&repo_root).unwrap();
    let mut f = std::fs::File::create(repo_root.join("tracker.md")).unwrap();
    writeln!(f, "# My Tracker\n\nsome content").unwrap();

    let result = tokio::runtime::Runtime::new().unwrap().block_on(
        LibrarianContext.call(&ctx, serde_json::json!({"topic": "test-topic"}))
    ).unwrap();

    let md = result["markdown"].as_str().unwrap();
    assert!(md.contains("[LIVE]"), "expected [LIVE] in:\n{md}");
    assert!(md.contains("Maintain state"), "expected prompt in:\n{md}");
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p librarian-mcp live_header_present
```

Expected: FAIL — no `[LIVE]` in output.

- [ ] **Step 3: Modify `LibrarianContext::call` to add [LIVE] rendering**

In `crates/librarian-mcp/src/tools/context.rs`, in the `call` method, after fetching `candidate_ids` and `rows_map`, add:

```rust
// Fetch augmentation rows for all candidates
let aug_map: std::collections::HashMap<String, crate::catalog::augmentation::AugmentationRow> = {
    let cat = ctx.catalog.lock();
    crate::catalog::augmentation::get_batch(
        &cat,
        &candidate_ids.iter().cloned().collect::<Vec<_>>(),
    )?
};

// Sort: trackers (augmented) first, then other augmented, then plain
let mut sorted_ids = candidate_ids.clone();
sorted_ids.sort_by_key(|id| {
    let row = rows_map.get(id);
    let is_tracker = row.map_or(false, |r| r.kind == "tracker");
    let is_augmented = aug_map.contains_key(id.as_str());
    match (is_tracker, is_augmented) {
        (true, _) => 0u8,
        (false, true) => 1,
        _ => 2,
    }
});
```

Then replace the iteration over `&candidate_ids` with `&sorted_ids`, and change the section rendering to:

```rust
let section = if let Some(aug) = aug_map.get(id.as_str()) {
    let refreshed = aug.last_refreshed_at.as_deref().unwrap_or("never");
    format!(
        "<!-- [LIVE]: {} | last refreshed: {} | refresh #{} -->\n\
         > Prompt: {}\n\n\
         ## {}  — {}/{}  ({}/{})\n{}\n\n",
        title, refreshed, aug.refresh_count,
        aug.prompt,
        title, row.kind, row.status, row.repo, row.rel_path, first_30
    )
} else {
    format!(
        "## {}  — {}/{}  ({}/{})\n{}\n\n",
        title, row.kind, row.status, row.repo, row.rel_path, first_30
    )
};
```

- [ ] **Step 4: Run all context tests**

```bash
cargo test -p librarian-mcp context
```

Expected: all tests PASS including new `live_header_present_for_augmented_artifact`.

- [ ] **Step 5: Commit**

```bash
git add crates/librarian-mcp/src/tools/context.rs
git commit -m "feat(librarian): librarian_context [LIVE] rendering + tracker priority"
```

---

## Task 13: Register all new tools + update prompt surfaces

**Files:**
- Modify: `crates/librarian-mcp/src/tools/mod.rs` — `all_tools()`
- Modify: `crates/librarian-mcp/src/prompts/server_instructions.md`

- [ ] **Step 1: Register tools in `all_tools()`**

In `crates/librarian-mcp/src/tools/mod.rs`, update `all_tools()`:

```rust
pub fn all_tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(find::ArtifactFind),
        Arc::new(get::ArtifactGet),
        Arc::new(list_by_kind::ArtifactListByKind),
        Arc::new(links::ArtifactLinks),
        Arc::new(graph::ArtifactGraph),
        Arc::new(create::ArtifactCreate),
        Arc::new(update::ArtifactUpdate),
        Arc::new(link::ArtifactLink),
        Arc::new(observe::ArtifactObserve),
        Arc::new(event_create::ArtifactEventCreate),
        Arc::new(timeline::ArtifactTimeline),
        Arc::new(state_at::ArtifactStateAt),
        Arc::new(workspace_state_at::WorkspaceStateAt),
        Arc::new(reindex::LibrarianReindex),
        Arc::new(context::LibrarianContext),
        // Augmentation
        Arc::new(augment::ArtifactAugment),
        Arc::new(update_params::ArtifactUpdateParams),
        Arc::new(refresh::ArtifactRefresh),
        Arc::new(refresh_commit::ArtifactRefreshCommit),
        Arc::new(tracker_create::TrackerCreate),
    ]
}
```

Also add all five module declarations at the top of `mod.rs`:

```rust
pub mod augment;
pub mod update_params;
pub mod refresh;
pub mod refresh_commit;
pub mod tracker_create;
pub mod gather;
```

- [ ] **Step 2: Run full test suite**

```bash
cargo test -p librarian-mcp
```

Expected: all tests PASS.

- [ ] **Step 3: Run clippy**

```bash
cargo clippy -p librarian-mcp -- -D warnings
```

Fix any warnings before proceeding.

- [ ] **Step 4: Update `server_instructions.md`**

In `crates/librarian-mcp/src/prompts/server_instructions.md`, add to the Tool selection table:

```markdown
| Attach/update prompt+params on artifact      | `artifact_augment`     |
| Tune gather params mid-session               | `artifact_update_params` |
| Gather context for refresh (read-only)       | `artifact_refresh`     |
| Commit completed refresh cycle              | `artifact_refresh_commit` |
| Create tracker artifact + augment atomically | `tracker_create`       |
| List/find augmented artifacts                | `artifact_find` with `augmented: true` |
```

Also add a new section after "## Writes round-trip":

```markdown
## Artifact augmentation and refresh

Any artifact can carry a persistent **prompt** + AI-editable **params** via
`artifact_augment`. This enables server-assisted context gathering.

**Refresh cycle** (4 steps):
1. `artifact_refresh(id)` — server gathers context per params, returns package
   `{ prompt, params, current_body, context, hints }`. Does NOT write.
2. Synthesize new body from `prompt + context + current_body`.
3. `artifact_update(id, { body: "<new content>" })` — write back.
4. `artifact_refresh_commit(id)` — record refresh metadata.

**Tracker kind:** `tracker_create` creates a `kind: tracker` artifact (body = live state)
and attaches augmentation atomically. Trackers are ranked first in `librarian_context`.

**`[LIVE]` in context:** Augmented artifacts appear with a `<!-- [LIVE] -->` header
and their prompt as a blockquote directive — read it as a standing instruction.

**Params gather sources:** `git_log`, `artifacts`, `observations`, `file`, `grep`.
Unknown sources are skipped with a warning (forward compat).
```

- [ ] **Step 5: Run full test suite + fmt**

```bash
cargo fmt -p librarian-mcp
cargo test -p librarian-mcp
cargo clippy -p librarian-mcp -- -D warnings
```

Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/librarian-mcp/src/tools/mod.rs \
        crates/librarian-mcp/src/prompts/server_instructions.md
git commit -m "feat(librarian): register augmentation tools + update server instructions"
```

---

## Task 14: Build release binary and verify

- [ ] **Step 1: Build release**

```bash
cargo build --release
```

Expected: compiles without error.

- [ ] **Step 2: Final check**

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

Expected: all green.

- [ ] **Step 3: Commit if any fmt fixes applied**

```bash
git add -u && git commit -m "chore: fmt + clippy fixes"
```

---

## Self-Review Checklist (completed inline)

**Spec coverage:**
- ✅ `artifact_augmentation` table — Task 1
- ✅ `AugmentationRow` CRUD — Task 2
- ✅ `artifact_augment` tool — Task 5
- ✅ `artifact_update_params` (RFC 7396 merge-patch) — Task 6
- ✅ `artifact_refresh` (gather + context package) — Task 7
- ✅ `artifact_refresh_commit` (metadata update, graceful no-op) — Task 8
- ✅ `tracker_create` (atomic) — Task 9
- ✅ `artifact_get` augmentation field — Task 10
- ✅ `artifact_find` augmented filter — Task 11
- ✅ `librarian_context` [LIVE] + tracker priority — Task 12
- ✅ `server_instructions.md` updated — Task 13
- ✅ `gather_from` sources: git_log, artifacts, observations, file, grep — Task 4
- ✅ Unknown sources → warning, not error — Task 4 + Task 7

**Placeholder scan:** None found.

**Type consistency:** `AugmentationRow` defined in Task 2 and used consistently in Tasks 5–12. `GatherSource` defined in Task 4, used in Task 7. `list_all_ids` defined in Task 2, used in Task 11. `get_batch` defined in Task 2, used in Task 12.

**Note:** `symbols` gather source intentionally omitted — requires tree-sitter/LSP not available in librarian-mcp. Marked as `Unknown` variant (forward compat). Spec's implementation notes acknowledged this.
