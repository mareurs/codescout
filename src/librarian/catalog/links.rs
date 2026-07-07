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
    insert_with(&cat.conn, link)
}

/// Insert into an existing connection or transaction. Use this when the
/// caller wants atomicity across multiple writes.
pub fn insert_with(conn: &rusqlite::Connection, link: &LinkRow) -> Result<()> {
    conn.execute(
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

/// All links with the given rel, across the whole catalog. Used by the
/// link_scan differ to load the scanner-owned edge set in one query.
pub fn by_rel(cat: &Catalog, rel: &str) -> Result<Vec<LinkRow>> {
    collect(cat, "WHERE rel = ?1", params![rel])
}

/// Delete one edge. Returns the number of rows removed (0 or 1 — the
/// composite PK makes the (src, dst, rel) triple unique).
pub fn delete(cat: &Catalog, src_id: &str, dst_id: &str, rel: &str) -> Result<usize> {
    delete_with(&cat.conn, src_id, dst_id, rel)
}

/// Transaction-friendly twin of [`delete`], mirroring [`insert_with`].
pub fn delete_with(
    conn: &rusqlite::Connection,
    src_id: &str,
    dst_id: &str,
    rel: &str,
) -> Result<usize> {
    let n = conn.execute(
        "DELETE FROM artifact_link WHERE src_id = ?1 AND dst_id = ?2 AND rel = ?3",
        params![src_id, dst_id, rel],
    )?;
    Ok(n)
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
    use crate::librarian::catalog::artifact::ArtifactRow;
    use crate::librarian::catalog::artifact::{self, TestArtifactRowBuilder};

    fn art(id: &str) -> ArtifactRow {
        TestArtifactRowBuilder::new(id).build()
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

    #[test]
    fn by_rel_filters_across_endpoints() {
        let cat = Catalog::open_in_memory().unwrap();
        for id in ["a", "b", "c"] {
            artifact::upsert(&cat, &art(id)).unwrap();
        }
        insert(
            &cat,
            &LinkRow {
                src_id: "a".into(),
                dst_id: "b".into(),
                rel: "cites".into(),
                created_at: 1,
            },
        )
        .unwrap();
        insert(
            &cat,
            &LinkRow {
                src_id: "b".into(),
                dst_id: "c".into(),
                rel: "cites".into(),
                created_at: 2,
            },
        )
        .unwrap();
        insert(
            &cat,
            &LinkRow {
                src_id: "a".into(),
                dst_id: "c".into(),
                rel: "supersedes".into(),
                created_at: 3,
            },
        )
        .unwrap();
        let cites = by_rel(&cat, "cites").unwrap();
        assert_eq!(cites.len(), 2);
        assert!(cites.iter().all(|l| l.rel == "cites"));
        let sup = by_rel(&cat, "supersedes").unwrap();
        assert_eq!(sup.len(), 1);
        assert!(by_rel(&cat, "implements").unwrap().is_empty());
    }

    #[test]
    fn delete_removes_exactly_the_named_triple() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a")).unwrap();
        artifact::upsert(&cat, &art("b")).unwrap();
        // Same endpoints, two rels — delete must be rel-scoped.
        insert(
            &cat,
            &LinkRow {
                src_id: "a".into(),
                dst_id: "b".into(),
                rel: "cites".into(),
                created_at: 1,
            },
        )
        .unwrap();
        insert(
            &cat,
            &LinkRow {
                src_id: "a".into(),
                dst_id: "b".into(),
                rel: "evidence-for".into(),
                created_at: 2,
            },
        )
        .unwrap();
        assert_eq!(delete(&cat, "a", "b", "cites").unwrap(), 1);
        // Second delete of the same triple is a no-op.
        assert_eq!(delete(&cat, "a", "b", "cites").unwrap(), 0);
        // The other rel between the same endpoints survives.
        let out = outgoing(&cat, "a").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].rel, "evidence-for");
    }
}
