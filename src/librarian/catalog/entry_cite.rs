use super::Catalog;
use anyhow::Result;
use rusqlite::params;

#[derive(Debug, Clone, PartialEq)]
pub struct EntryCiteRow {
    pub src_slug: String,
    pub src_local: String,
    pub dst_ref: String,
    pub rel: String,
    pub origin: String,
    pub created_at: i64,
}

/// Insert one entry-grain edge. `INSERT OR IGNORE` — the PK
/// (src_slug, src_local, dst_ref, rel) makes duplicates a no-op.
pub fn insert_with(conn: &rusqlite::Connection, row: &EntryCiteRow) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO entry_cite
           (src_slug, src_local, dst_ref, rel, origin, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            row.src_slug,
            row.src_local,
            row.dst_ref,
            row.rel,
            row.origin,
            row.created_at
        ],
    )?;
    Ok(())
}

pub fn outgoing(cat: &Catalog, src_slug: &str) -> Result<Vec<EntryCiteRow>> {
    collect(cat, "WHERE src_slug = ?1", params![src_slug])
}

pub fn incoming(cat: &Catalog, dst_ref: &str) -> Result<Vec<EntryCiteRow>> {
    collect(cat, "WHERE dst_ref = ?1", params![dst_ref])
}

/// Incoming edges whose dst_ref matches a SQL LIKE pattern (e.g. "<slug>:%"
/// to find everything citing any entry of a tracker). Exact-match `incoming`
/// stays the right call for artifact-id targets.
pub fn incoming_like(cat: &Catalog, pattern: &str) -> Result<Vec<EntryCiteRow>> {
    collect(cat, "WHERE dst_ref LIKE ?1", params![pattern])
}

fn collect(
    cat: &Catalog,
    where_clause: &str,
    p: impl rusqlite::Params,
) -> Result<Vec<EntryCiteRow>> {
    let sql = format!(
        "SELECT src_slug, src_local, dst_ref, rel, origin, created_at FROM entry_cite {where_clause}"
    );
    let mut stmt = cat.conn.prepare(&sql)?;
    let rows = stmt
        .query_map(p, |r| {
            Ok(EntryCiteRow {
                src_slug: r.get(0)?,
                src_local: r.get(1)?,
                dst_ref: r.get(2)?,
                rel: r.get(3)?,
                origin: r.get(4)?,
                created_at: r.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{self, TestArtifactRowBuilder};
    use crate::librarian::catalog::Catalog;

    fn seed_slugged(cat: &Catalog, id: &str, slug: &str) {
        artifact::upsert(cat, &TestArtifactRowBuilder::new(id).build()).unwrap();
        cat.conn
            .execute(
                "UPDATE artifact SET slug=?1 WHERE id=?2",
                rusqlite::params![slug, id],
            )
            .unwrap();
    }

    #[test]
    fn insert_and_read_roundtrip() {
        let cat = Catalog::open_in_memory().unwrap();
        seed_slugged(&cat, "art-a", "tracker-a");
        insert_with(
            &cat.conn,
            &EntryCiteRow {
                src_slug: "tracker-a".into(),
                src_local: "W-1".into(),
                dst_ref: "art-b-id".into(),
                rel: "cites".into(),
                origin: "write".into(),
                created_at: 1,
            },
        )
        .unwrap();
        assert_eq!(outgoing(&cat, "tracker-a").unwrap().len(), 1);
        assert_eq!(incoming(&cat, "art-b-id").unwrap().len(), 1);
    }

    #[test]
    fn cascade_delete_removes_entry_cite_when_artifact_deleted() {
        let cat = Catalog::open_in_memory().unwrap();
        seed_slugged(&cat, "art-a", "tracker-a");
        insert_with(
            &cat.conn,
            &EntryCiteRow {
                src_slug: "tracker-a".into(),
                src_local: "W-1".into(),
                dst_ref: "x".into(),
                rel: "cites".into(),
                origin: "write".into(),
                created_at: 1,
            },
        )
        .unwrap();
        cat.conn
            .execute("DELETE FROM artifact WHERE id='art-a'", [])
            .unwrap();
        assert_eq!(outgoing(&cat, "tracker-a").unwrap().len(), 0);
    }
}
