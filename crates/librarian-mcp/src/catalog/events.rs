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
    insert_with(&cat.conn, ev)
}

/// Insert into an existing connection or transaction. Use this when the
/// caller wants atomicity across multiple writes (event row + edges).
pub fn insert_with(conn: &rusqlite::Connection, ev: &EventRow) -> Result<()> {
    conn.execute(
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
    until: Option<i64>,
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
    if until.is_some() {
        sql.push_str(" AND created_at <= ?");
    }
    sql.push_str(" ORDER BY created_at DESC, id DESC LIMIT ?");
    let mut stmt = cat.conn.prepare(&sql)?;
    let mut params_dyn: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(artifact_id.to_string())];
    if let Some(ks) = kinds {
        for k in ks {
            params_dyn.push(Box::new(k.to_string()));
        }
    }
    if let Some(u) = until {
        params_dyn.push(Box::new(u));
    }
    params_dyn.push(Box::new(limit as i64));
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params_dyn.iter()), row_to_event)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Return all `intent` events that have not yet been resolved by a
/// `verdict` event (i.e. no `event_edges` row with `rel='resolves'`
/// pointing at the intent's id).
pub fn open_intents(cat: &Catalog) -> Result<Vec<EventRow>> {
    let mut stmt = cat.conn.prepare(
        "SELECT id, artifact_id, kind, payload, anchor_commit, head_commit, author, created_at
         FROM events
         WHERE kind='intent'
           AND id NOT IN (
             SELECT dst_event_id FROM event_edges
             WHERE rel='resolves' AND dst_event_id IS NOT NULL
           )
         ORDER BY created_at DESC, id DESC",
    )?;
    let rows = stmt
        .query_map([], row_to_event)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Return all `verdict` events that have no outgoing `resolves` edge.
/// Such rows are a data bug — every verdict must resolve an intent.
pub fn orphan_verdicts(cat: &Catalog) -> Result<Vec<EventRow>> {
    let mut stmt = cat.conn.prepare(
        "SELECT id, artifact_id, kind, payload, anchor_commit, head_commit, author, created_at
         FROM events
         WHERE kind='verdict'
           AND id NOT IN (
             SELECT src_event_id FROM event_edges WHERE rel='resolves'
           )
         ORDER BY created_at DESC, id DESC",
    )?;
    let rows = stmt
        .query_map([], row_to_event)?
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
        let only_notes = timeline_for_artifact(&cat, "a", Some(&["note"]), None, 10).unwrap();
        assert_eq!(only_notes.len(), 3);
        let capped = timeline_for_artifact(&cat, "a", None, None, 2).unwrap();
        assert_eq!(capped.len(), 2);
        assert_eq!(capped[0].id, "04"); // newest first
    }

    #[test]
    fn timeline_until_pushes_into_sql_not_post_limit() {
        // Seed 50 events at ts=1..=50 plus one ancient event at ts=0.
        let cat = Catalog::open_in_memory().unwrap();
        crate::catalog::artifact::upsert(&cat, &art("a")).unwrap();
        // Ancient event at ts=0
        insert(&cat, &ev("ancient", "a", "note", 0)).unwrap();
        // 50 events at ts=1..=50
        for i in 1i64..=50 {
            insert(&cat, &ev(&format!("e{i:02}"), "a", "note", i)).unwrap();
        }

        // limit=10 without until → returns the 10 newest (ts=41..=50)
        let newest = timeline_for_artifact(&cat, "a", None, None, 10).unwrap();
        assert_eq!(newest.len(), 10);
        assert!(newest.iter().all(|e| e.created_at >= 41));

        // limit=10 with until=5 → must return events ts=0..=5 (6 rows), NOT the 10 newest.
        // This proves the until filter is applied in SQL before the LIMIT clause.
        let bounded = timeline_for_artifact(&cat, "a", None, Some(5), 10).unwrap();
        assert_eq!(
            bounded.len(),
            6, // ts=0,1,2,3,4,5
            "until must be pushed into SQL before the LIMIT"
        );
        assert!(
            bounded.iter().all(|e| e.created_at <= 5),
            "all results must satisfy created_at <= 5"
        );
    }

    #[test]
    fn open_intents_excludes_resolved_and_includes_unresolved() {
        use crate::catalog::event_edges::{insert_many as edges_insert, EdgeRow};
        let cat = Catalog::open_in_memory().unwrap();
        art_insert(&cat, &art("a")).unwrap();
        // unresolved intent
        insert(&cat, &ev("i_open", "a", "intent", 100)).unwrap();
        // resolved intent (verdict + resolves edge)
        insert(&cat, &ev("i_done", "a", "intent", 200)).unwrap();
        insert(&cat, &ev("v1", "a", "verdict", 300)).unwrap();
        edges_insert(
            &cat,
            &[EdgeRow {
                src_event_id: "v1".into(),
                dst_event_id: Some("i_done".into()),
                dst_artifact_id: None,
                dst_source_id: None,
                rel: "resolves".into(),
            }],
        )
        .unwrap();

        let open = open_intents(&cat).unwrap();
        let ids: Vec<&str> = open.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["i_open"]);
    }

    #[test]
    fn verdict_without_intent_is_data_bug() {
        use crate::catalog::event_edges::{insert_many as edges_insert, EdgeRow};
        let cat = Catalog::open_in_memory().unwrap();
        art_insert(&cat, &art("a")).unwrap();
        // healthy verdict (has resolves edge)
        insert(&cat, &ev("intent_h", "a", "intent", 100)).unwrap();
        insert(&cat, &ev("verdict_h", "a", "verdict", 200)).unwrap();
        edges_insert(
            &cat,
            &[EdgeRow {
                src_event_id: "verdict_h".into(),
                dst_event_id: Some("intent_h".into()),
                dst_artifact_id: None,
                dst_source_id: None,
                rel: "resolves".into(),
            }],
        )
        .unwrap();
        // orphan verdict (no resolves edge)
        insert(&cat, &ev("verdict_orphan", "a", "verdict", 300)).unwrap();

        let orphans = orphan_verdicts(&cat).unwrap();
        let ids: Vec<&str> = orphans.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["verdict_orphan"]);
    }
}
