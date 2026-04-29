use crate::catalog::Catalog;
use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRow {
    pub id: String,
    pub artifact_id: String,
    pub kind: String,
    pub payload: String,
    pub anchor_commit: Option<String>,
    pub head_commit: Option<String>,
    pub author: Option<String>,
    pub created_at: i64,
}

pub fn insert(cat: &Catalog, ev: &EventRow) -> Result<()> {
    cat.conn.execute(
        "INSERT INTO events (id, artifact_id, kind, payload, anchor_commit, head_commit, author, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![ev.id, ev.artifact_id, ev.kind, ev.payload, ev.anchor_commit, ev.head_commit, ev.author, ev.created_at],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::artifact::{upsert as art_insert, ArtifactRow};

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
    fn insert_event_round_trip() {
        let cat = Catalog::open_in_memory().unwrap();
        art_insert(&cat, &art("a")).unwrap();
        let ev = EventRow {
            id: "01H".into(),
            artifact_id: "a".into(),
            kind: "note".into(),
            payload: r#"{"text":"hi"}"#.into(),
            anchor_commit: Some("abc".into()),
            head_commit: Some("def".into()),
            author: Some("user".into()),
            created_at: 100,
        };
        insert(&cat, &ev).unwrap();
        let count: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM events WHERE id=?1",
                params!["01H"],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
}
