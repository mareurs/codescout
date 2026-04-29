use crate::catalog::Catalog;
use anyhow::Result;
use rusqlite::{params, OptionalExtension};
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

pub fn latest_for_artifact(cat: &Catalog, artifact_id: &str) -> Result<Option<EventRow>> {
    let mut stmt = cat.conn.prepare(
        "SELECT id, artifact_id, kind, payload, anchor_commit, head_commit, author, created_at
         FROM events WHERE artifact_id=?1 ORDER BY created_at DESC, id DESC LIMIT 1",
    )?;
    let row = stmt
        .query_row(params![artifact_id], row_to_event)
        .optional()?;
    Ok(row)
}

pub fn timeline_for_artifact(
    cat: &Catalog,
    artifact_id: &str,
    kinds: Option<&[&str]>,
    limit: usize,
) -> Result<Vec<EventRow>> {
    let mut sql = String::from(
        "SELECT id, artifact_id, kind, payload, anchor_commit, head_commit, author, created_at
         FROM events WHERE artifact_id=?1",
    );
    if let Some(ks) = kinds {
        if !ks.is_empty() {
            let placeholders = ks.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            sql.push_str(&format!(" AND kind IN ({placeholders})"));
        }
    }
    sql.push_str(" ORDER BY created_at DESC, id DESC LIMIT ?");
    let mut stmt = cat.conn.prepare(&sql)?;
    let mut params_dyn: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(artifact_id.to_string())];
    if let Some(ks) = kinds {
        for k in ks {
            params_dyn.push(Box::new(k.to_string()));
        }
    }
    params_dyn.push(Box::new(limit as i64));
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params_dyn.iter()), row_to_event)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn row_to_event(r: &rusqlite::Row) -> rusqlite::Result<EventRow> {
    Ok(EventRow {
        id: r.get(0)?,
        artifact_id: r.get(1)?,
        kind: r.get(2)?,
        payload: r.get(3)?,
        anchor_commit: r.get(4)?,
        head_commit: r.get(5)?,
        author: r.get(6)?,
        created_at: r.get(7)?,
    })
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

    fn ev(id: &str, art: &str, kind: &str, ts: i64) -> EventRow {
        EventRow {
            id: id.into(),
            artifact_id: art.into(),
            kind: kind.into(),
            payload: "{}".into(),
            anchor_commit: None,
            head_commit: None,
            author: None,
            created_at: ts,
        }
    }

    #[test]
    fn latest_for_artifact_returns_newest() {
        let cat = Catalog::open_in_memory().unwrap();
        crate::catalog::artifact::upsert(&cat, &art("a")).unwrap();
        insert(&cat, &ev("01", "a", "note", 1)).unwrap();
        insert(&cat, &ev("02", "a", "reviewed", 5)).unwrap();
        insert(&cat, &ev("03", "a", "note", 3)).unwrap();
        let latest = latest_for_artifact(&cat, "a").unwrap().unwrap();
        assert_eq!(latest.id, "02");
    }

    #[test]
    fn timeline_filters_by_kind_and_limit() {
        let cat = Catalog::open_in_memory().unwrap();
        crate::catalog::artifact::upsert(&cat, &art("a")).unwrap();
        for i in 0..5 {
            let kind = if i % 2 == 0 { "note" } else { "reviewed" };
            insert(&cat, &ev(&format!("0{i}"), "a", kind, i as i64)).unwrap();
        }
        let only_notes = timeline_for_artifact(&cat, "a", Some(&["note"]), 10).unwrap();
        assert_eq!(only_notes.len(), 3);
        let capped = timeline_for_artifact(&cat, "a", None, 2).unwrap();
        assert_eq!(capped.len(), 2);
        assert_eq!(capped[0].id, "04"); // newest first
    }
}
