use crate::catalog::Catalog;
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
    cat.conn.execute(
        "INSERT INTO artifact_augmentation
           (artifact_id, prompt, params, last_refreshed_at, refresh_count, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(artifact_id) DO UPDATE SET
           prompt = excluded.prompt,
           params = excluded.params,
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
    let mut current: Value = serde_json::from_str(&existing.params).unwrap_or_else(|_| json!({}));
    // RFC 7396 merge-patch
    if let (Value::Object(target), Value::Object(patch_map)) = (&mut current, patch) {
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
                created_at, updated_at
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
