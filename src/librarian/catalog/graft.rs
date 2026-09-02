//! Fold one artifact row's catalog history into another (worktree-merge safety).
//!
//! Re-points the FK-linked child tables (`events`, `artifact_observation`,
//! `artifact_link`, `event_edges.dst_artifact_id`) off `from_id` onto
//! `into_id`, then deletes `from_id`. DELETE IS LAST: those tables all
//! `REFERENCES artifact(id) ON DELETE CASCADE`, so deleting the source before
//! re-pointing would destroy the very history we migrate. Augmentation params
//! (`artifact_augmentation`) are migrated wholesale when only one side is
//! augmented, or merged entry-by-entry (renumbering id collisions, flagging
//! near-dup content) when both sides share an `entry_collection`.

use crate::librarian::catalog::augmentation::next_index;
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
    /// `entry_reservation` rows folded onto the destination, taking the MAX of the
    /// two marks per prefix. Without this the graft's cascade-delete dropped them,
    /// so `doc(move)` reset a ledger's id counter.
    pub entry_reservations_folded: usize,
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

    // 1-4. Re-point events / observations / links / event_edges onto `into_id`.
    let mut report = GraftReport::default();
    repoint_history(&tx, from_id, into_id, &mut report)?;

    // 5. Augmentation params — migrate or merge (before the delete: params live
    //    on `artifact_augmentation`, which also cascade-deletes with the source).
    merge_augmentation(&tx, from_id, into_id, &mut report)?;

    // 6. Carry the slug forward, then delete source LAST — cascades any leftover
    //    dup links / edges / augmentation. Capture, THEN delete, THEN write: `slug`
    //    is UNIQUE, so writing `from_id`'s slug onto `into_id` while `from_id` still
    //    holds it would violate the index — both rows can't hold the same slug at
    //    once, even mid-transaction. `into_id` keeps whatever slug it already had
    //    (freshly minted or otherwise) when `from_id` had none.
    let from_slug: Option<String> =
        tx.query_row("SELECT slug FROM artifact WHERE id=?1", [from_id], |r| {
            r.get(0)
        })?;
    tx.execute("DELETE FROM artifact WHERE id=?1", [from_id])?;
    if let Some(slug) = from_slug {
        tx.execute(
            "UPDATE artifact SET slug=?1 WHERE id=?2",
            params![slug, into_id],
        )?;
    }

    tx.commit()?;

    Ok(report)
}

/// Re-point `events`, `artifact_observation`, `artifact_link`, and
/// `event_edges.dst_artifact_id` rows from `from_id` onto `into_id`, recording
/// counts on `report`. A link or edge re-point that would collide with an
/// existing unique key on `into_id` is dropped (not an error); the caller's
/// subsequent cascade-delete of `from_id` cleans up the leftover dup rows.
pub(crate) fn repoint_history(
    tx: &rusqlite::Transaction<'_>,
    from_id: &str,
    into_id: &str,
    report: &mut GraftReport,
) -> Result<()> {
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

    // 5. Entry reservations — PK (artifact_id, prefix), and the value is a HIGH-WATER
    //    MARK, so the merge rule is `max`, not last-writer-wins: folding a lower mark
    //    over a higher one would hand the next allocation an id that already exists.
    //    A plain `UPDATE ... SET artifact_id` cannot express that and would conflict
    //    outright whenever the destination already tracks the same prefix.
    //
    //    Before this existed, `graft_rows`' cascade-delete of the source silently
    //    dropped these rows, which is what let `doc(move)` — i.e. archiving —
    //    reset a ledger's counter
    //    (docs/issues/archive/2026-08-17-ledger-id-reissue-silently-repoints-citations.md).
    let entry_reservations_folded = tx.execute(
        "INSERT INTO entry_reservation (artifact_id, prefix, max_allocated, updated_at)
         SELECT ?1, prefix, max_allocated, updated_at
           FROM entry_reservation WHERE artifact_id = ?2
         ON CONFLICT(artifact_id, prefix) DO UPDATE SET
           max_allocated = MAX(entry_reservation.max_allocated, excluded.max_allocated),
           updated_at    = excluded.updated_at",
        params![into_id, from_id],
    )?;

    report.entry_reservations_folded = entry_reservations_folded;
    report.events_repointed = events_repointed;
    report.observations_repointed = observations_repointed;
    report.links_repointed = u1 + u2;
    report.links_dropped = links_left as usize; // conflicting dups, cascade-deleted above
    report.event_edges_repointed = (ee_before - ee_left) as usize;
    report.event_edges_dropped = ee_left as usize; // conflicting dups, cascade-deleted above

    Ok(())
}

/// Split `"F-12"` into `("F", 12)`. Returns `None` for ids without a
/// trailing `-<int>` (e.g. no dash, or non-numeric suffix).
fn split_id(id: &str) -> Option<(&str, u64)> {
    let (prefix, num) = id.rsplit_once('-')?;
    num.parse::<u64>().ok().map(|n| (prefix, n))
}

/// Deep-clone of `entry` with the `id` field removed, for near-dup comparison.
fn strip_id(entry: &Value) -> Value {
    let mut e = entry.clone();
    if let Some(o) = e.as_object_mut() {
        o.remove("id");
    }
    e
}

/// Fold `incoming` entries onto `into_arr`, applying the reserved-universe
/// collision-renumber and near-dup detection shared by `merge_augmentation`.
/// Operates on arbitrary arrays (not necessarily a whole seeded collection) so
/// callers can fold a partial delta (e.g. a worktree's new entries) through the
/// same machinery.
///
/// - Incoming entries whose id doesn't collide with `into_arr` are appended
///   verbatim.
/// - Incoming entries whose id collides with a surviving `into_arr` id are
///   renumbered to the next free `<prefix>-N`, allocated over the whole
///   reserved universe (survivor ids + all free incoming ids + ids already
///   allocated this fold) so the result never contains a duplicate id
///   (`report.remap` records old->new).
/// - Incoming entries whose content (minus `id`) already matches a surviving
///   entry are flagged in `report.suspicious`.
pub(crate) fn fold_entries(
    into_arr: &[Value],
    incoming: &[Value],
    report: &mut GraftReport,
) -> Vec<Value> {
    let into_ids: std::collections::HashSet<String> = into_arr
        .iter()
        .filter_map(|e| e.get("id").and_then(Value::as_str).map(String::from))
        .collect();

    let id_of = |e: &Value| {
        e.get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let mut merged = into_arr.to_vec();

    // Near-dup detection: incoming object (minus id) deep-equals a surviving original.
    for entry in incoming {
        if into_arr.iter().any(|e| strip_id(e) == strip_id(entry)) {
            report.suspicious.push(entry.clone());
        }
    }

    // Reserved id universe = survivor ids + ALL free (non-colliding) incoming ids.
    // Free incoming ids survive un-renumbered, so a renumbered collision must never
    // land on one of them — seed the whole universe BEFORE allocating any new id.
    let mut reserved: std::collections::HashSet<String> = into_ids.clone();
    for entry in incoming {
        let id = id_of(entry);
        if !into_ids.contains(&id) {
            reserved.insert(id);
        }
    }

    // Append FREE incoming entries first (ids preserved verbatim).
    for entry in incoming {
        let id = id_of(entry);
        if !into_ids.contains(&id) {
            merged.push(entry.clone());
            report.entries_merged += 1;
        }
    }

    // Renumber COLLIDING incoming entries over the full reserved universe
    // (monotonic max+1 per prefix; each freshly allocated id is added to
    // `reserved` so two collisions on the same prefix get distinct numbers).
    for entry in incoming {
        let old = id_of(entry);
        if !into_ids.contains(&old) {
            continue;
        }
        let mut e = entry.clone();
        if let Some((prefix, _)) = split_id(&old) {
            let ids_vec: Vec<String> = reserved.iter().cloned().collect();
            let next = next_index(&ids_vec, prefix);
            let new_id = format!("{prefix}-{next}");
            reserved.insert(new_id.clone());
            if let Some(o) = e.as_object_mut() {
                o.insert("id".into(), serde_json::json!(new_id));
            }
            report.remap.insert(old, new_id);
            report.entries_renumbered += 1;
        }
        // else: id has no numeric suffix (never produced by append_entry). Cannot
        // prefix-renumber; leave it as-is (accepted, documented limitation). It will
        // NOT collide with a renumbered id because renumbering only targets numeric
        // prefixes.
        merged.push(e);
        report.entries_merged += 1;
    }

    merged
}

/// Fold `from_id`'s `artifact_augmentation` row onto `into_id`, before the
/// caller's final `DELETE FROM artifact` (that delete cascade-deletes
/// `artifact_augmentation`, so this must run first, same transaction).
///
/// - Neither side augmented, or only `from_id` is: re-point (migrate) the
///   whole row wholesale — nothing to merge.
/// - Both sides augmented but their `entry_collection`s differ (or either is
///   unset): leave `into_id`'s params untouched; `from_id`'s row is
///   cascade-deleted with the source.
/// - Both sides augmented with the same `entry_collection`: fold incoming
///   entries onto the survivor's array. Free (non-colliding) incoming ids
///   are preserved; any incoming id that collides with a surviving id is
///   renumbered to the next free `<prefix>-N`, allocated over the whole
///   reserved universe (survivor ids + all free incoming ids + ids already
///   allocated this graft) so the result never contains a duplicate id
///   (`report.remap` records old->new). Incoming entries whose content
///   (minus `id`) already matches a surviving entry are flagged in
///   `report.suspicious`.
fn merge_augmentation(
    tx: &rusqlite::Transaction<'_>,
    from_id: &str,
    into_id: &str,
    report: &mut GraftReport,
) -> Result<()> {
    let fetch = |id: &str| -> Result<Option<(String, Option<String>)>> {
        tx.query_row(
            "SELECT params, entry_collection FROM artifact_augmentation WHERE artifact_id=?1",
            [id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other.into()),
        })
    };

    let Some((from_params, from_coll)) = fetch(from_id)? else {
        return Ok(()); // from_id has no augmentation — nothing to merge or migrate.
    };
    let into_aug = fetch(into_id)?;

    // into has no augmentation -> migrate from's row wholesale (re-point PK).
    if into_aug.is_none() {
        tx.execute(
            "UPDATE artifact_augmentation SET artifact_id=?1 WHERE artifact_id=?2",
            params![into_id, from_id],
        )?;
        return Ok(());
    }
    let (into_params, into_coll) = into_aug.unwrap();

    // Both augmented but no shared entry_collection -> leave into's params as-is;
    // from's augmentation row cascade-deletes with the source below.
    let coll = match (&from_coll, &into_coll) {
        (Some(a), Some(b)) if a == b => a.clone(),
        _ => return Ok(()),
    };

    let mut into_json: Value =
        serde_json::from_str(&into_params).unwrap_or_else(|_| serde_json::json!({}));
    let from_json: Value =
        serde_json::from_str(&from_params).unwrap_or_else(|_| serde_json::json!({}));
    let into_arr = into_json
        .get(&coll)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let from_arr = from_json
        .get(&coll)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let merged = fold_entries(&into_arr, &from_arr, report);

    // Guard: `unwrap_or_else` above only catches PARSE errors; valid-but-non-object
    // JSON (e.g. a bare array) would panic on index-assign. Fail recoverably instead.
    match into_json.as_object_mut() {
        Some(obj) => {
            obj.insert(coll.clone(), Value::Array(merged));
        }
        None => {
            return Err(RecoverableError::new(format!(
                "graft: into_id `{into_id}` augmentation params is not a JSON object"
            )))
        }
    }
    tx.execute(
        "UPDATE artifact_augmentation SET params=?1 WHERE artifact_id=?2",
        params![serde_json::to_string(&into_json)?, into_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::TestArtifactRowBuilder;
    use crate::librarian::catalog::augmentation::{self, AugmentationRow};
    use crate::librarian::catalog::observations::{self, ObservationRow};
    use crate::librarian::catalog::Catalog;
    use crate::librarian::catalog::{event_edges, events, events::TestEventRowBuilder};
    use rusqlite::OptionalExtension;

    fn art(cat: &Catalog, id: &str, path: &str) {
        let row = TestArtifactRowBuilder::new(id)
            .with_abs_path(path)
            .with_kind("tracker")
            .build();
        crate::librarian::catalog::artifact::upsert(cat, &row).unwrap();
    }

    /// Minimal augmentation row builder for graft tests: fixed prompt/timestamps,
    /// caller supplies id, entry_collection name, and raw params JSON.
    fn aug(id: &str, coll: &str, params: &str) -> AugmentationRow {
        AugmentationRow {
            artifact_id: id.into(),
            prompt: "t".into(),
            params: params.into(),
            last_refreshed_at: None,
            refresh_count: 0,
            created_at: "2026-01-01T00:00:00.000Z".into(),
            updated_at: "2026-01-01T00:00:00.000Z".into(),
            render_template: None,
            params_schema: None,
            append_mode: false,
            history_cap: None,
            entry_collection: Some(coll.into()),
            refreshed_at_commit: None,
        }
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
    fn graft_carries_the_slug_forward_so_a_move_does_not_orphan_it() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "from", "/wt/x.md");
        art(&cat, "into", "/main/x.md");
        cat.conn
            .execute("UPDATE artifact SET slug='my-tracker' WHERE id='from'", [])
            .unwrap();

        graft_rows(&mut cat, "from", "into").unwrap();

        let slug: Option<String> = cat
            .conn
            .query_row("SELECT slug FROM artifact WHERE id='into'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            slug.as_deref(),
            Some("my-tracker"),
            "the slug must follow the artifact across the graft, not stay orphaned on the row this deletes"
        );
    }

    fn reserve(cat: &Catalog, artifact_id: &str, prefix: &str, max_allocated: i64) {
        cat.conn
            .execute(
                "INSERT INTO entry_reservation (artifact_id, prefix, max_allocated, updated_at)
                 VALUES (?1, ?2, ?3, '2026-01-01T00:00:00.000Z')",
                rusqlite::params![artifact_id, prefix, max_allocated],
            )
            .unwrap();
    }

    fn reserved(cat: &Catalog, artifact_id: &str, prefix: &str) -> Option<i64> {
        cat.conn
            .query_row(
                "SELECT max_allocated FROM entry_reservation WHERE artifact_id=?1 AND prefix=?2",
                rusqlite::params![artifact_id, prefix],
                |r| r.get(0),
            )
            .optional()
            .unwrap()
    }

    /// A reservation is a HIGH-WATER MARK, so the fold rule is `max`, not
    /// last-writer-wins. Folding the source's LOWER mark over the destination's higher
    /// one would hand the next allocation an id that already exists — reintroducing
    /// `docs/issues/archive/2026-08-17-ledger-id-reissue-silently-repoints-citations.md` through
    /// the move path, which is the path that caused it in the first place.
    ///
    /// Both directions are asserted, because only one of them can distinguish `max`
    /// from a plain overwrite: with the source ahead, overwrite and max agree.
    #[test]
    fn graft_folds_entry_reservations_taking_the_higher_mark() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "src", "/r/a.md");
        art(&cat, "dst", "/r/b.md");

        // Source BEHIND destination — the discriminating direction.
        reserve(&cat, "src", "HY", 5);
        reserve(&cat, "dst", "HY", 12);
        // A prefix only the source knows: must carry across, not be dropped.
        reserve(&cat, "src", "R", 77);

        let report = graft_rows(&mut cat, "src", "dst").unwrap();

        assert_eq!(
            reserved(&cat, "dst", "HY"),
            Some(12),
            "the higher mark must win; a plain overwrite would regress this to 5"
        );
        assert_eq!(
            reserved(&cat, "dst", "R"),
            Some(77),
            "a prefix the destination did not track must still fold across"
        );
        assert_eq!(report.entry_reservations_folded, 2);
        assert_eq!(
            reserved(&cat, "src", "HY"),
            None,
            "the source row goes with the cascade-deleted artifact"
        );
    }

    /// The other direction, so the pair pins `max` rather than either extreme.
    #[test]
    fn graft_folds_entry_reservations_when_the_source_is_ahead() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "src", "/r/a.md");
        art(&cat, "dst", "/r/b.md");
        reserve(&cat, "src", "HY", 20);
        reserve(&cat, "dst", "HY", 12);

        graft_rows(&mut cat, "src", "dst").unwrap();

        assert_eq!(reserved(&cat, "dst", "HY"), Some(20));
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

    #[test]
    fn graft_migrates_augmentation_when_into_has_none() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "from", "/wt/x.md");
        art(&cat, "into", "/main/x.md");
        augmentation::upsert(
            &cat,
            &aug("from", "failures", r#"{"failures":[{"id":"F-1","t":"a"}]}"#),
        )
        .unwrap();

        graft_rows(&mut cat, "from", "into").unwrap();

        let moved = augmentation::get(&cat, "into").unwrap().unwrap();
        let p: Value = serde_json::from_str(&moved.params).unwrap();
        assert_eq!(
            p["failures"][0]["id"], "F-1",
            "augmentation migrated wholesale"
        );
    }

    #[test]
    fn graft_renumbers_colliding_incoming_ids_and_reports_remap() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "from", "/wt/x.md");
        art(&cat, "into", "/main/x.md");
        augmentation::upsert(
            &cat,
            &aug(
                "into",
                "failures",
                r#"{"failures":[{"id":"F-1","t":"keep1"},{"id":"F-2","t":"keep2"}]}"#,
            ),
        )
        .unwrap();
        // Incoming F-2 collides (distinct content); F-9 is free -> preserved.
        // Renumber allocates over the full reserved universe {F-1,F-2,F-9},
        // so F-2 -> F-10 (max{1,2,9}+1), NEVER onto the free F-9.
        augmentation::upsert(
            &cat,
            &aug(
                "from",
                "failures",
                r#"{"failures":[{"id":"F-2","t":"incoming"},{"id":"F-9","t":"free"}]}"#,
            ),
        )
        .unwrap();

        let report = graft_rows(&mut cat, "from", "into").unwrap();

        assert_eq!(report.entries_renumbered, 1);
        assert_eq!(report.remap.get("F-2").map(String::as_str), Some("F-10"));
        let p: Value =
            serde_json::from_str(&augmentation::get(&cat, "into").unwrap().unwrap().params)
                .unwrap();
        let ids: Vec<&str> = p["failures"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, vec!["F-1", "F-2", "F-9", "F-10"]);
    }

    #[test]
    fn graft_flags_near_dup_as_suspicious() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "from", "/wt/x.md");
        art(&cat, "into", "/main/x.md");
        augmentation::upsert(
            &cat,
            &aug(
                "into",
                "failures",
                r#"{"failures":[{"id":"F-5","t":"same bug"}]}"#,
            ),
        )
        .unwrap();
        // Same content, different id string: same finding discovered twice.
        augmentation::upsert(
            &cat,
            &aug(
                "from",
                "failures",
                r#"{"failures":[{"id":"F-1","t":"same bug"}]}"#,
            ),
        )
        .unwrap();

        let report = graft_rows(&mut cat, "from", "into").unwrap();

        assert_eq!(report.suspicious.len(), 1);
        assert_eq!(report.suspicious[0]["t"], "same bug");
    }

    #[test]
    fn graft_renumber_avoids_free_incoming_id_no_duplicate() {
        // Regression: the common worktree merge — both sides added one unique
        // entry on a shared base. A naive allocator that seeds only from
        // survivor + already-appended ids renumbers F-1 onto F-4 (max{1,2,3}+1)
        // and then ALSO preserves the free incoming F-4 -> two F-4 (silent, since
        // params is a TEXT blob with no DB uniqueness). The reserved-universe
        // allocator must avoid the free incoming id entirely.
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "from", "/wt/x.md");
        art(&cat, "into", "/main/x.md");
        augmentation::upsert(
            &cat,
            &aug(
                "into",
                "failures",
                r#"{"failures":[{"id":"F-1","t":"i1"},{"id":"F-2","t":"i2"},{"id":"F-3","t":"i3"}]}"#,
            ),
        )
        .unwrap();
        augmentation::upsert(
            &cat,
            &aug(
                "from",
                "failures",
                r#"{"failures":[{"id":"F-1","t":"w1"},{"id":"F-2","t":"w2"},{"id":"F-4","t":"free"}]}"#,
            ),
        )
        .unwrap();

        let report = graft_rows(&mut cat, "from", "into").unwrap();

        // F-4 free -> preserved; F-1 -> F-5, F-2 -> F-6 (allocated above the
        // reserved universe {F-1,F-2,F-3,F-4}, then monotonically).
        assert_eq!(report.entries_renumbered, 2);
        assert_eq!(report.remap.get("F-1").map(String::as_str), Some("F-5"));
        assert_eq!(report.remap.get("F-2").map(String::as_str), Some("F-6"));
        let p: Value =
            serde_json::from_str(&augmentation::get(&cat, "into").unwrap().unwrap().params)
                .unwrap();
        let ids: Vec<String> = p["failures"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(ids, vec!["F-1", "F-2", "F-3", "F-4", "F-5", "F-6"]);
        // The whole point: NO duplicate id survives the merge.
        let distinct: std::collections::HashSet<&String> = ids.iter().collect();
        assert_eq!(distinct.len(), 6, "all merged ids must be distinct");
    }

    #[test]
    fn graft_no_op_when_entry_collections_differ() {
        // Both augmented, but different entry_collection names -> into's params
        // are left untouched (the return Ok(()) no-op branch); from's row is
        // gone (cascade-deleted with the source artifact).
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "from", "/wt/x.md");
        art(&cat, "into", "/main/x.md");
        augmentation::upsert(
            &cat,
            &aug(
                "into",
                "failures",
                r#"{"failures":[{"id":"F-1","t":"keep"}]}"#,
            ),
        )
        .unwrap();
        augmentation::upsert(
            &cat,
            &aug("from", "wins", r#"{"wins":[{"id":"W-1","t":"other"}]}"#),
        )
        .unwrap();

        graft_rows(&mut cat, "from", "into").unwrap();

        // into's params unchanged.
        let into_aug = augmentation::get(&cat, "into").unwrap().unwrap();
        assert_eq!(
            into_aug.params, r#"{"failures":[{"id":"F-1","t":"keep"}]}"#,
            "into's params must be untouched when collections differ"
        );
        // from's augmentation row is gone.
        assert!(
            augmentation::get(&cat, "from").unwrap().is_none(),
            "from's augmentation cascade-deleted with the source"
        );
    }

    // `fold_entries` must work directly on an arbitrary slice (e.g. a worktree
    // delta), not only a whole seeded collection — this is the shape Task 8's
    // merge_worktree needs. Exercise it with a 1-entry `incoming` delta against
    // a 2-entry survivor array, asserting the collision renumbers to F-3 (not
    // F-1 or F-2) and the remap is recorded.
    #[test]
    fn fold_entries_on_delta_slice_renumbers_collision() {
        let into_arr = vec![
            serde_json::json!({"id": "F-1", "t": "keep1"}),
            serde_json::json!({"id": "F-2", "t": "keep2"}),
        ];
        let incoming = vec![serde_json::json!({"id": "F-2", "t": "incoming-different"})];

        let mut report = GraftReport::default();
        let merged = fold_entries(&into_arr, &incoming, &mut report);

        assert_eq!(report.entries_renumbered, 1);
        assert_eq!(report.remap.get("F-2").map(String::as_str), Some("F-3"));
        let ids: Vec<&str> = merged.iter().map(|e| e["id"].as_str().unwrap()).collect();
        assert_eq!(ids, vec!["F-1", "F-2", "F-3"]);
    }
}
