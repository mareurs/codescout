use crate::librarian::catalog::Catalog;
use crate::librarian::tools::{schema_validate, RecoverableError};
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
    /// Names the params array whose objects are the tracker's filterable
    /// entry rows (e.g. "failures", "children"). None = not entry-filterable.
    pub entry_collection: Option<String>,
}

pub fn upsert(cat: &Catalog, row: &AugmentationRow) -> Result<()> {
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
    Ok(())
}

pub fn get(cat: &Catalog, artifact_id: &str) -> Result<Option<AugmentationRow>> {
    let mut stmt = cat.conn.prepare(
        "SELECT artifact_id, prompt, params, last_refreshed_at, refresh_count,
                created_at, updated_at, render_template, params_schema,
                append_mode, history_cap, entry_collection
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
        render_template: row.get(7)?,
        params_schema: row.get(8)?,
        append_mode: row.get::<_, i64>(9).map(|v| v != 0)?,
        history_cap: row.get(10)?,
        entry_collection: row.get(11)?,
    })
}

pub fn merge_params(cat: &Catalog, artifact_id: &str, patch: &Value) -> Result<bool> {
    let Some(existing) = get(cat, artifact_id)? else {
        return Ok(false);
    };
    let mut current: Value = serde_json::from_str(&existing.params).unwrap_or_else(|_| json!({}));
    apply_merge_patch(&mut current, patch);
    if let Some(schema_text) = existing.params_schema.as_deref() {
        schema_validate::validate_against_stored(schema_text, &current).map_err(|e| {
            RecoverableError::new(format!("merge_params: patch violates params_schema: {e}"))
        })?;
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

/// Shallow RFC 7396 merge-patch applied in place to `target`. `null` keys in the
/// patch delete; non-null values overwrite the corresponding target key entirely.
/// Nested objects are overwritten in full (not recursively merged). This is intentional —
/// artifact params are expected to be flat key-value objects. Non-object patches are
/// no-ops (the tool schema enforces object at the boundary).
pub fn apply_merge_patch(target: &mut Value, patch: &Value) {
    if let (Value::Object(t), Value::Object(p)) = (target, patch) {
        for (k, v) in p {
            if v.is_null() {
                t.remove(k);
            } else {
                t.insert(k.clone(), v.clone());
            }
        }
    }
}

pub fn commit_refresh(cat: &Catalog, artifact_id: &str) -> Result<bool> {
    let n = cat.conn.execute(
        "UPDATE artifact_augmentation
         SET refresh_count = refresh_count + 1,
             last_refreshed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE artifact_id = ?1",
        rusqlite::params![artifact_id],
    )?;
    Ok(n > 0)
}

pub fn list_all_ids(cat: &Catalog) -> Result<Vec<String>> {
    let mut stmt = cat
        .conn
        .prepare("SELECT artifact_id FROM artifact_augmentation ORDER BY artifact_id")?;
    let ids = stmt
        .query_map([], |row| row.get(0))?
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
    let placeholders = ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT artifact_id, prompt, params, last_refreshed_at, refresh_count,
                created_at, updated_at, render_template, params_schema,
                append_mode, history_cap, entry_collection
         FROM artifact_augmentation WHERE artifact_id IN ({placeholders})"
    );
    let mut stmt = cat.conn.prepare(&sql)?;
    let params: Vec<&dyn rusqlite::ToSql> = ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
    let rows = stmt
        .query_map(params.as_slice(), row_from_sql)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .map(|r| (r.artifact_id.clone(), r))
        .collect())
}

#[derive(Debug, Clone)]
pub struct StaleEntry {
    pub artifact_id: String,
    pub abs_path: std::path::PathBuf,
    pub kind: String,
    pub title: Option<String>,
    pub last_refreshed_at: Option<String>,
    pub refresh_count: i64,
}

/// Return augmented artifacts whose `last_refreshed_at` is older than
/// `threshold_iso` (ISO-8601), or has never been refreshed (NULL).
/// Results are ordered oldest-first (NULLs first — SQLite sorts NULLs as
/// less than any value in ASC order).
pub fn list_stale(
    cat: &Catalog,
    threshold_iso: &str,
    limit: usize,
    abs_path_prefix: Option<&std::path::Path>,
) -> Result<Vec<StaleEntry>> {
    let mut sql = String::from(
        "SELECT a.id, a.abs_path, a.kind, a.title, \
         au.last_refreshed_at, au.refresh_count \
         FROM artifact_augmentation au \
         JOIN artifact a ON a.id = au.artifact_id \
         WHERE (au.last_refreshed_at IS NULL OR au.last_refreshed_at < ?1)",
    );
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(threshold_iso.to_string())];
    let mut idx = 2usize;

    if let Some(prefix) = abs_path_prefix {
        let prefix_s = crate::util::fs::RepoPath::from(prefix);
        if !prefix_s.as_str().is_empty() {
            sql.push_str(&format!(" AND a.abs_path LIKE ?{idx}"));
            params.push(Box::new(format!("{prefix_s}/%")));
            idx += 1;
        }
    }

    sql.push_str(&format!(" ORDER BY au.last_refreshed_at ASC LIMIT ?{idx}"));
    params.push(Box::new(limit as i64));

    let refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = cat.conn.prepare(&sql)?;
    let rows = stmt
        .query_map(refs.as_slice(), |row| {
            let abs_path_s: String = row.get(1)?;
            Ok(StaleEntry {
                artifact_id: row.get(0)?,
                abs_path: std::path::PathBuf::from(abs_path_s),
                kind: row.get(2)?,
                title: row.get(3)?,
                last_refreshed_at: row.get(4)?,
                refresh_count: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{upsert as art_upsert, ArtifactRow};
    use chrono::Utc;

    fn sample_art(id: &str) -> ArtifactRow {
        let now = Utc::now().timestamp_millis();
        ArtifactRow {
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
            file_sha256: "abc".to_string(),
            confidence: 1.0,
        }
    }

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
            entry_collection: None,
        }
    }

    #[test]
    fn upsert_and_get_roundtrip() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        upsert(&cat, &aug("art1")).unwrap();
        let row = get(&cat, "art1").unwrap().expect("row should exist");
        assert_eq!(row.artifact_id, "art1");
        assert_eq!(row.prompt, "test prompt");
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
        assert_eq!(row.refresh_count, 0);
    }

    #[test]
    fn upsert_preserves_refresh_count_on_update() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        upsert(&cat, &aug("art1")).unwrap();
        // Simulate a refresh having happened
        commit_refresh(&cat, "art1").unwrap();
        // Re-augment with new prompt
        let mut updated = aug("art1");
        updated.prompt = "Updated prompt".to_string();
        upsert(&cat, &updated).unwrap();
        // refresh_count must NOT be reset
        let row = get(&cat, "art1").unwrap().unwrap();
        assert_eq!(
            row.refresh_count, 1,
            "refresh_count must survive re-augment"
        );
        assert!(
            row.last_refreshed_at.is_some(),
            "last_refreshed_at must survive re-augment"
        );
        assert_eq!(row.prompt, "Updated prompt");
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
    fn merge_params_rejects_violation() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        let schema = json!({
            "type": "object",
            "properties": {"count": {"type": "integer"}},
            "additionalProperties": false
        });
        let mut a = aug("art1");
        a.params_schema = Some(serde_json::to_string(&schema).unwrap());
        upsert(&cat, &a).unwrap();
        let patch = json!({"count": "not-a-number"});
        let err = merge_params(&cat, "art1", &patch).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("merge_params: patch violates params_schema"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn merge_params_accepts_valid() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("art1")).unwrap();
        let schema = json!({
            "type": "object",
            "properties": {"count": {"type": "integer"}},
            "additionalProperties": false
        });
        let mut a = aug("art1");
        a.params_schema = Some(serde_json::to_string(&schema).unwrap());
        upsert(&cat, &a).unwrap();
        let patch = json!({"count": 42});
        let found = merge_params(&cat, "art1", &patch).unwrap();
        assert!(found);
        let row = get(&cat, "art1").unwrap().unwrap();
        let params: Value = serde_json::from_str(&row.params).unwrap();
        assert_eq!(params["count"], 42);
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
        crate::librarian::catalog::artifact::delete(&cat, "art1").unwrap();
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

    #[test]
    fn append_mode_and_history_cap_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::open(dir.path().join("cat.db").as_path()).unwrap();
        let art = sample_art("a1");
        crate::librarian::catalog::artifact::upsert(&cat, &art).unwrap();
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
        let cat = Catalog::open(dir.path().join("cat.db").as_path()).unwrap();
        let art = sample_art("a2");
        crate::librarian::catalog::artifact::upsert(&cat, &art).unwrap();
        upsert(&cat, &aug("a2")).unwrap();
        let got = get(&cat, "a2").unwrap().unwrap();
        assert!(!got.append_mode);
        assert_eq!(got.history_cap, None);
    }
    #[test]
    fn entry_collection_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::open(dir.path().join("cat.db").as_path()).unwrap();
        crate::librarian::catalog::artifact::upsert(&cat, &sample_art("ec-art")).unwrap();
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
}
