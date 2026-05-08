use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::Catalog;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationRow {
    pub id: Option<i64>,
    pub artifact_id: String,
    pub text: String,
    pub source: Option<String>,
    pub created_at: i64,
}

pub fn insert(cat: &Catalog, obs: &ObservationRow) -> Result<i64> {
    cat.conn.execute(
        "INSERT INTO artifact_observation (artifact_id, text, source, created_at) VALUES (?, ?, ?, ?)",
        params![obs.artifact_id, obs.text, obs.source, obs.created_at],
    )?;
    Ok(cat.conn.last_insert_rowid())
}

pub fn list_for_artifact(cat: &Catalog, artifact_id: &str) -> Result<Vec<ObservationRow>> {
    let mut stmt = cat.conn.prepare(
        "SELECT id, artifact_id, text, source, created_at FROM artifact_observation
         WHERE artifact_id = ?1 ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map(params![artifact_id], |r| {
        Ok(ObservationRow {
            id: Some(r.get(0)?),
            artifact_id: r.get(1)?,
            text: r.get(2)?,
            source: r.get(3)?,
            created_at: r.get(4)?,
        })
    })?;
    rows.collect::<Result<_, _>>().map_err(Into::into)
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::artifact;
    use crate::catalog::artifact::ArtifactRow;

    fn art(id: &str) -> ArtifactRow {
        ArtifactRow {
            id: id.into(),
            abs_path: std::path::PathBuf::from(format!("/test/r/{id}.md")),
            kind: "spec".into(),
            status: "active".into(),
            title: None,
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: 0,
            updated_at: 0,
            file_mtime: 0,
            file_sha256: "".into(),
            confidence: 1.0,
        }
    }

    #[test]
    fn insert_and_list() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a")).unwrap();
        insert(
            &cat,
            &ObservationRow {
                id: None,
                artifact_id: "a".into(),
                text: "first note".into(),
                source: Some("agent".into()),
                created_at: 1,
            },
        )
        .unwrap();
        insert(
            &cat,
            &ObservationRow {
                id: None,
                artifact_id: "a".into(),
                text: "second note".into(),
                source: None,
                created_at: 2,
            },
        )
        .unwrap();
        let rows = list_for_artifact(&cat, "a").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].text, "first note");
    }

    #[test]
    fn cascade_delete_removes_observations() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a")).unwrap();
        insert(
            &cat,
            &ObservationRow {
                id: None,
                artifact_id: "a".into(),
                text: "note".into(),
                source: None,
                created_at: 1,
            },
        )
        .unwrap();
        artifact::delete(&cat, "a").unwrap();
        assert!(list_for_artifact(&cat, "a").unwrap().is_empty());
    }

    #[test]
    fn list_recent_filters_by_since() {
        use crate::catalog::artifact::{upsert as art_upsert, ArtifactRow};
        let cat = Catalog::open_in_memory().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        let art = ArtifactRow {
            id: "a1".to_string(),
            abs_path: std::path::PathBuf::from("/test/r/a.md"),
            kind: "tracker".to_string(),
            status: "active".to_string(),
            title: None,
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
        };
        art_upsert(&cat, &art).unwrap();

        // insert two observations 5s apart
        let old_ts = now - 5000;
        let new_ts = now;
        insert(
            &cat,
            &ObservationRow {
                id: None,
                artifact_id: "a1".to_string(),
                text: "old".to_string(),
                source: None,
                created_at: old_ts,
            },
        )
        .unwrap();
        insert(
            &cat,
            &ObservationRow {
                id: None,
                artifact_id: "a1".to_string(),
                text: "new".to_string(),
                source: None,
                created_at: new_ts,
            },
        )
        .unwrap();

        let recent = list_recent(&cat, None, Some(old_ts + 1), 10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].text, "new");
    }
}
