//! Fold one artifact row's catalog history into another (worktree-merge safety).
//!
//! Re-points the FK-linked child tables (`events`, `artifact_observation`,
//! `artifact_link`, `event_edges.dst_artifact_id`) off `from_id` onto
//! `into_id`, then deletes `from_id`. DELETE IS LAST: those tables all
//! `REFERENCES artifact(id) ON DELETE CASCADE`, so deleting the source before
//! re-pointing would destroy the very history we migrate. Augmentation params
//! merge (`artifact_augmentation`) is added in a later task.

use crate::librarian::catalog::Catalog;
use crate::librarian::tools::RecoverableError;
use anyhow::Result;
use rusqlite::params;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Default, serde::Serialize)]
pub struct GraftReport {
    pub events_repointed: usize,
    pub observations_repointed: usize,
    pub links_repointed: usize,
    pub links_dropped: usize,
    pub event_edges_repointed: usize,
    pub event_edges_dropped: usize,
    pub entries_merged: usize,
    pub entries_renumbered: usize,
    pub remap: BTreeMap<String, String>,
    pub suspicious: Vec<Value>,
}

fn row_exists(conn: &rusqlite::Connection, id: &str) -> Result<bool> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM artifact WHERE id=?1", [id], |r| {
        r.get(0)
    })?;
    Ok(n > 0)
}

/// Fold `from_id`'s catalog history onto `into_id`, then delete `from_id`.
///
/// Re-points `events`, `artifact_observation`, `artifact_link`, and
/// `event_edges.dst_artifact_id` rows. A link or edge re-point that would
/// collide with an existing unique key on `into_id` is dropped (not an error)
/// and cascade-deleted along with the source artifact row.
///
/// Runs in a single `IMMEDIATE` transaction: either the whole graft lands or
/// none of it does, so a mid-graft failure can never leave `from_id` partially
/// re-pointed and orphaned.
pub fn graft_rows(cat: &mut Catalog, from_id: &str, into_id: &str) -> Result<GraftReport> {
    if from_id == into_id {
        return Err(RecoverableError::new(
            "graft: from_id and into_id are the same row",
        ));
    }
    if !row_exists(&cat.conn, from_id)? {
        return Err(RecoverableError::new(format!(
            "graft: unknown from_id `{from_id}`"
        )));
    }
    if !row_exists(&cat.conn, into_id)? {
        return Err(RecoverableError::new(format!(
            "graft: unknown into_id `{into_id}`"
        )));
    }

    let tx = cat
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

    // 1. Events — plain re-point (event id is unique, no conflict). Re-pointing
    //    events.artifact_id does NOT touch event_edges' src_event_id/dst_event_id
    //    (event row ids are unchanged), so only dst_artifact_id needs handling (4).
    let events_repointed = tx.execute(
        "UPDATE events SET artifact_id=?1 WHERE artifact_id=?2",
        params![into_id, from_id],
    )?;

    // 2. Observations — same shape.
    let observations_repointed = tx.execute(
        "UPDATE artifact_observation SET artifact_id=?1 WHERE artifact_id=?2",
        params![into_id, from_id],
    )?;

    // 3. Links — directional, PK (src_id,dst_id,rel). Re-point both endpoints
    //    with OR IGNORE; a re-point that would duplicate an existing edge is
    //    skipped (row keeps from_id) and cascade-deleted with the source below.
    let u1 = tx.execute(
        "UPDATE OR IGNORE artifact_link SET src_id=?1 WHERE src_id=?2",
        params![into_id, from_id],
    )?;
    let u2 = tx.execute(
        "UPDATE OR IGNORE artifact_link SET dst_id=?1 WHERE dst_id=?2",
        params![into_id, from_id],
    )?;
    let links_left: i64 = tx.query_row(
        "SELECT COUNT(*) FROM artifact_link WHERE src_id=?1 OR dst_id=?1",
        [from_id],
        |r| r.get(0),
    )?;

    // 4. Event-graph edges pointing AT the source artifact. Same OR IGNORE dedup
    //    as links: event_edges has a UNIQUE index over
    //    (src_event_id, rel, COALESCE(dst_event_id,''), COALESCE(dst_artifact_id,''),
    //    COALESCE(dst_source_id,'')), so a re-point that would duplicate an edge
    //    is dropped (keeps from_id) and cascade-deleted with the source below.
    let ee_before: i64 = tx.query_row(
        "SELECT COUNT(*) FROM event_edges WHERE dst_artifact_id=?1",
        [from_id],
        |r| r.get(0),
    )?;
    tx.execute(
        "UPDATE OR IGNORE event_edges SET dst_artifact_id=?1 WHERE dst_artifact_id=?2",
        params![into_id, from_id],
    )?;
    let ee_left: i64 = tx.query_row(
        "SELECT COUNT(*) FROM event_edges WHERE dst_artifact_id=?1",
        [from_id],
        |r| r.get(0),
    )?;

    // 5. Delete source LAST — cascades any leftover dup links / edges.
    tx.execute("DELETE FROM artifact WHERE id=?1", [from_id])?;

    tx.commit()?;

    Ok(GraftReport {
        events_repointed,
        observations_repointed,
        links_repointed: u1 + u2,
        links_dropped: links_left as usize, // conflicting dups, cascade-deleted above
        event_edges_repointed: (ee_before - ee_left) as usize,
        event_edges_dropped: ee_left as usize, // conflicting dups, cascade-deleted above
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::TestArtifactRowBuilder;
    use crate::librarian::catalog::observations::{self, ObservationRow};
    use crate::librarian::catalog::Catalog;
    use crate::librarian::catalog::{event_edges, events, events::TestEventRowBuilder};

    fn art(cat: &Catalog, id: &str, path: &str) {
        let row = TestArtifactRowBuilder::new(id)
            .with_abs_path(path)
            .with_kind("tracker")
            .build();
        crate::librarian::catalog::artifact::upsert(cat, &row).unwrap();
    }

    #[test]
    fn graft_repoints_events_and_deletes_source_last() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "from", "/wt/x.md");
        art(&cat, "into", "/main/x.md");
        events::insert(
            &cat,
            &TestEventRowBuilder::new("from", "note")
                .with_id("e1")
                .build(),
        )
        .unwrap();
        events::insert(
            &cat,
            &TestEventRowBuilder::new("from", "note")
                .with_id("e2")
                .build(),
        )
        .unwrap();
        // An observation on the source too — it must survive the graft.
        observations::insert(
            &cat,
            &ObservationRow {
                id: None,
                artifact_id: "from".into(),
                text: "obs".into(),
                source: None,
                created_at: 1,
            },
        )
        .unwrap();

        let report = graft_rows(&mut cat, "from", "into").unwrap();

        assert_eq!(report.events_repointed, 2);
        assert_eq!(report.observations_repointed, 1);
        // History survived onto `into` (delete-last invariant held).
        let n: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM events WHERE artifact_id='into'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 2, "events re-pointed, not cascade-deleted");
        let obs_n: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM artifact_observation WHERE artifact_id='into'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(obs_n, 1, "observation re-pointed, not cascade-deleted");
        // Source row is gone.
        let src: i64 = cat
            .conn
            .query_row("SELECT COUNT(*) FROM artifact WHERE id='from'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(src, 0);
    }

    #[test]
    fn graft_dedups_conflicting_link_and_drops_it() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "from", "/wt/x.md");
        art(&cat, "into", "/main/x.md");
        art(&cat, "dst", "/main/y.md");
        let mk = |src: &str| crate::librarian::catalog::links::LinkRow {
            src_id: src.into(),
            dst_id: "dst".into(),
            rel: "cites".into(),
            created_at: 1,
        };
        // Both from->dst and into->dst exist with the same rel: re-pointing from->dst
        // onto into->dst is a PK conflict, so it must be DROPPED, not error.
        crate::librarian::catalog::links::insert(&cat, &mk("from")).unwrap();
        crate::librarian::catalog::links::insert(&cat, &mk("into")).unwrap();

        let report = graft_rows(&mut cat, "from", "into").unwrap();

        assert_eq!(report.links_dropped, 1);
        let n: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM artifact_link WHERE src_id='into' AND dst_id='dst'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "single surviving edge, no duplicate");
    }

    #[test]
    fn graft_repoints_event_edge_to_artifact() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "from", "/wt/x.md");
        art(&cat, "into", "/main/x.md");
        // A real source event for the edge (event id is stable across graft).
        events::insert(
            &cat,
            &TestEventRowBuilder::new("into", "note")
                .with_id("ev1")
                .build(),
        )
        .unwrap();
        // Narrative-graph edge pointing AT the source artifact. If graft ignores
        // event_edges, the final DELETE of `from` cascade-deletes this edge —
        // silent history loss.
        event_edges::insert_many(
            &cat,
            &[event_edges::EdgeRow {
                src_event_id: "ev1".into(),
                dst_event_id: None,
                dst_artifact_id: Some("from".into()),
                dst_source_id: None,
                rel: "mutates".into(),
            }],
        )
        .unwrap();

        let report = graft_rows(&mut cat, "from", "into").unwrap();

        assert_eq!(report.event_edges_repointed, 1);
        assert_eq!(report.event_edges_dropped, 0);
        // Edge survived onto `into` (not cascade-deleted with the source artifact).
        let dst: String = cat
            .conn
            .query_row(
                "SELECT dst_artifact_id FROM event_edges WHERE src_event_id='ev1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(dst, "into", "edge re-pointed, not cascade-deleted");
    }

    #[test]
    fn graft_errors_on_unknown_id() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "into", "/main/x.md");
        let err = graft_rows(&mut cat, "nope", "into").unwrap_err();
        assert!(err.to_string().contains("nope"));
    }
}
