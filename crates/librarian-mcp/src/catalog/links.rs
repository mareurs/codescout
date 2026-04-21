use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::Catalog;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkRow {
    pub src_id: String,
    pub dst_id: String,
    pub rel: String,
    pub created_at: i64,
}

pub fn insert(cat: &Catalog, link: &LinkRow) -> Result<()> {
    cat.conn.execute(
        "INSERT OR IGNORE INTO artifact_link (src_id, dst_id, rel, created_at) VALUES (?, ?, ?, ?)",
        params![link.src_id, link.dst_id, link.rel, link.created_at],
    )?;
    Ok(())
}

pub fn outgoing(cat: &Catalog, src_id: &str) -> Result<Vec<LinkRow>> {
    collect(cat, "WHERE src_id = ?1", params![src_id])
}

pub fn incoming(cat: &Catalog, dst_id: &str) -> Result<Vec<LinkRow>> {
    collect(cat, "WHERE dst_id = ?1", params![dst_id])
}

fn collect(cat: &Catalog, where_clause: &str, p: impl rusqlite::Params) -> Result<Vec<LinkRow>> {
    let sql = format!("SELECT src_id, dst_id, rel, created_at FROM artifact_link {where_clause}");
    let mut stmt = cat.conn.prepare(&sql)?;
    let rows = stmt.query_map(p, |r| {
        Ok(LinkRow {
            src_id: r.get(0)?,
            dst_id: r.get(1)?,
            rel: r.get(2)?,
            created_at: r.get(3)?,
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
    fn insert_and_query_links() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a")).unwrap();
        artifact::upsert(&cat, &art("b")).unwrap();
        insert(
            &cat,
            &LinkRow {
                src_id: "a".into(),
                dst_id: "b".into(),
                rel: "supersedes".into(),
                created_at: 1,
            },
        )
        .unwrap();
        let out = outgoing(&cat, "a").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].dst_id, "b");
        let inc = incoming(&cat, "b").unwrap();
        assert_eq!(inc.len(), 1);
    }

    #[test]
    fn cascade_delete_removes_links() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a")).unwrap();
        artifact::upsert(&cat, &art("b")).unwrap();
        insert(
            &cat,
            &LinkRow {
                src_id: "a".into(),
                dst_id: "b".into(),
                rel: "implements".into(),
                created_at: 1,
            },
        )
        .unwrap();
        artifact::delete(&cat, "a").unwrap();
        assert!(outgoing(&cat, "a").unwrap().is_empty());
        assert!(incoming(&cat, "b").unwrap().is_empty());
    }
}
