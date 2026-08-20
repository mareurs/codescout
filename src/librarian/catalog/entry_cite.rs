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

/// Insert one entry-grain edge. Returns **1 if a row was written, 0 if one already
/// existed** — `INSERT OR IGNORE`, and the PK `(src_slug, src_local, dst_ref, rel)`
/// makes a duplicate a no-op.
///
/// **The return value is load-bearing, not a courtesy.** `origin` is deliberately NOT
/// part of that PK, so an edge a caller wrote explicitly via `append_entry(cites=…)`
/// and that `link_scan` later derives from prose collides and is ignored — the row
/// keeps `origin='write'` forever, which is the intended precedence. The consequence is
/// that *edges derived* and *rows written* are different numbers, and without this
/// count they are indistinguishable: a materializer tallying its own insert calls would
/// report a figure its instrument never measured.
/// See `statement-validity-session-log:F-5`.
pub fn insert_with(conn: &rusqlite::Connection, row: &EntryCiteRow) -> Result<usize> {
    let n = conn.execute(
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
    Ok(n)
}

/// The scanner's own `origin` value. Rows carrying it are owned by `link_scan` and are
/// pruned and re-derived on every write-mode scan; anything else is a human's and is
/// never touched.
pub const ORIGIN_SCAN: &str = "scan";

/// What a materialize pass did, counted separately because they differ.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MaterializeReport {
    /// Edges the scan resolved to an entry-grain row.
    pub derived: usize,
    /// Rows actually inserted.
    pub written: usize,
    /// Derived edges an existing row already covered — almost always an `origin='write'`
    /// row the scan must not clobber. `derived == written + skipped_existing`.
    pub skipped_existing: usize,
}

/// Delete every scanner-owned row whose source is one of `src_slugs`. Returns rows removed.
///
/// **Scoped to the scanned sources, never a bare `WHERE origin='scan'`.** `link_scan`
/// runs under a scope (`project` by default) and a `limit`, so a global prune would
/// delete rows belonging to artifacts this pass never looked at and could not re-derive
/// — silently dropping edges a wider earlier scan had correctly materialized. This
/// mirrors the `prunable` set the artifact-grain path already builds from the rows it
/// actually extracted.
pub fn prune_scan_rows(
    conn: &rusqlite::Connection,
    src_slugs: &std::collections::BTreeSet<String>,
) -> Result<usize> {
    let mut removed = 0usize;
    let mut stmt = conn.prepare("DELETE FROM entry_cite WHERE origin = ?1 AND src_slug = ?2")?;
    for slug in src_slugs {
        removed += stmt.execute(params![ORIGIN_SCAN, slug])?;
    }
    Ok(removed)
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

    fn row(src_slug: &str, src_local: &str, dst_ref: &str, origin: &str) -> EntryCiteRow {
        EntryCiteRow {
            src_slug: src_slug.into(),
            src_local: src_local.into(),
            dst_ref: dst_ref.into(),
            rel: "cites".into(),
            origin: origin.into(),
            created_at: 1,
        }
    }

    #[test]
    fn insert_with_reports_zero_when_a_row_already_covers_the_edge() {
        // `origin` is NOT in the PK, so a scan-derived edge that duplicates a
        // hand-written one is silently ignored and the row keeps origin='write'. The
        // precedence is intended; what must not happen is a caller counting its own
        // insert calls and reporting them as rows written.
        // statement-validity-session-log:F-5
        let cat = Catalog::open_in_memory().unwrap();
        seed_slugged(&cat, "art-a", "tracker-a");

        assert_eq!(
            insert_with(&cat.conn, &row("tracker-a", "W-1", "other:F-2", "write")).unwrap(),
            1,
            "first insert writes a row"
        );
        assert_eq!(
            insert_with(
                &cat.conn,
                &row("tracker-a", "W-1", "other:F-2", ORIGIN_SCAN)
            )
            .unwrap(),
            0,
            "same PK, different origin — ignored, and the count must say so"
        );

        let rows = outgoing(&cat, "tracker-a").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].origin, "write",
            "the hand-written row wins; the scan must never clobber it"
        );
    }

    #[test]
    fn prune_scan_rows_removes_only_scan_rows_of_the_named_sources() {
        let cat = Catalog::open_in_memory().unwrap();
        seed_slugged(&cat, "art-a", "tracker-a");
        seed_slugged(&cat, "art-b", "tracker-b");

        insert_with(&cat.conn, &row("tracker-a", "W-1", "x:F-1", ORIGIN_SCAN)).unwrap();
        insert_with(&cat.conn, &row("tracker-a", "W-2", "x:F-2", "write")).unwrap();
        insert_with(&cat.conn, &row("tracker-b", "W-3", "x:F-3", ORIGIN_SCAN)).unwrap();

        // Only `tracker-a` was scanned this pass.
        let scanned: std::collections::BTreeSet<String> = ["tracker-a".to_string()].into();
        assert_eq!(prune_scan_rows(&cat.conn, &scanned).unwrap(), 1);

        let a = outgoing(&cat, "tracker-a").unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(
            a[0].origin, "write",
            "a hand-written row survives a prune of its own source"
        );
        assert_eq!(
            outgoing(&cat, "tracker-b").unwrap().len(),
            1,
            "an UNSCANNED source keeps its scan rows — a bare `WHERE origin='scan'` \
             would delete edges this pass never looked at and cannot re-derive"
        );
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
