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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::artifact;
    use crate::catalog::artifact::ArtifactRow;

    fn art(id: &str) -> ArtifactRow {
        ArtifactRow {
            id: id.into(),
            repo: "r".into(),
            rel_path: format!("{id}.md"),
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
}
