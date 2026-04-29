use crate::catalog::Catalog;
use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeRow {
    pub src_event_id: String,
    pub dst_event_id: Option<String>,
    pub dst_artifact_id: Option<String>,
    pub dst_source_id: Option<String>,
    pub rel: String, // 'parent'|'mutates'|'triggered_by'|'merges_with'|'resolves'
}

pub fn insert_many(cat: &Catalog, edges: &[EdgeRow]) -> Result<()> {
    const VALID_RELS: &[&str] = &[
        "parent",
        "mutates",
        "triggered_by",
        "merges_with",
        "resolves",
    ];
    for e in edges {
        if !VALID_RELS.contains(&e.rel.as_str()) {
            anyhow::bail!("invalid rel {:?}; must be one of {:?}", e.rel, VALID_RELS);
        }
    }
    let tx = cat.conn.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT OR IGNORE INTO event_edges
             (src_event_id, dst_event_id, dst_artifact_id, dst_source_id, rel)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for e in edges {
            stmt.execute(params![
                e.src_event_id,
                e.dst_event_id,
                e.dst_artifact_id,
                e.dst_source_id,
                e.rel
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn outgoing(cat: &Catalog, src_event_id: &str) -> Result<Vec<EdgeRow>> {
    let mut stmt = cat.conn.prepare(
        "SELECT src_event_id, dst_event_id, dst_artifact_id, dst_source_id, rel
         FROM event_edges WHERE src_event_id=?1",
    )?;
    let rows = stmt
        .query_map(params![src_event_id], row_to_edge)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn incoming_by_rel(cat: &Catalog, dst_event_id: &str, rel: &str) -> Result<Vec<EdgeRow>> {
    let mut stmt = cat.conn.prepare(
        "SELECT src_event_id, dst_event_id, dst_artifact_id, dst_source_id, rel
         FROM event_edges WHERE dst_event_id=?1 AND rel=?2",
    )?;
    let rows = stmt
        .query_map(params![dst_event_id, rel], row_to_edge)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn row_to_edge(r: &rusqlite::Row) -> rusqlite::Result<EdgeRow> {
    Ok(EdgeRow {
        src_event_id: r.get(0)?,
        dst_event_id: r.get(1)?,
        dst_artifact_id: r.get(2)?,
        dst_source_id: r.get(3)?,
        rel: r.get(4)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::artifact::{upsert as art_upsert, ArtifactRow};
    use crate::catalog::events::{insert as ev_insert, EventRow};

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
            created_at: 1,
            updated_at: 1,
            file_mtime: 1,
            file_sha256: "s".into(),
            confidence: 1.0,
        }
    }

    fn ev(id: &str, art: &str, kind: &str) -> EventRow {
        EventRow {
            id: id.into(),
            artifact_id: art.into(),
            kind: kind.into(),
            payload: "{}".into(),
            anchor_commit: None,
            head_commit: None,
            author: None,
            created_at: 1,
        }
    }

    #[test]
    fn insert_and_traverse_resolves_edge() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &art("a")).unwrap();
        ev_insert(&cat, &ev("intent01", "a", "intent")).unwrap();
        ev_insert(&cat, &ev("verdict01", "a", "verdict")).unwrap();
        insert_many(
            &cat,
            &[EdgeRow {
                src_event_id: "verdict01".into(),
                dst_event_id: Some("intent01".into()),
                dst_artifact_id: None,
                dst_source_id: None,
                rel: "resolves".into(),
            }],
        )
        .unwrap();
        let out = outgoing(&cat, "verdict01").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].rel, "resolves");
        let inc = incoming_by_rel(&cat, "intent01", "resolves").unwrap();
        assert_eq!(inc[0].src_event_id, "verdict01");
    }

    #[test]
    fn rejects_unknown_rel() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &art("a")).unwrap();
        ev_insert(&cat, &ev("e1", "a", "note")).unwrap();
        let bad = EdgeRow {
            src_event_id: "e1".into(),
            dst_event_id: None,
            dst_artifact_id: None,
            dst_source_id: None,
            rel: "bogus".into(),
        };
        assert!(insert_many(&cat, &[bad]).is_err());
    }
}
