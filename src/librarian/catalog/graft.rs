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
use crate::librarian::tools::LibrarianRecoverableError;
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
    /// Outgoing `entry_cite` rows carried across with the slug. Reported rather than
    /// merely preserved: `doc(move)` already returns a four-field inbound-citation
    /// block and said nothing about the outgoing edges it was destroying, so a caller
    /// had no number that would have moved when they went missing.
    pub entry_citations_carried: usize,
    pub remap: BTreeMap<String, String>,
    pub suspicious: Vec<Value>,
}

fn row_exists(conn: &rusqlite::Connection, id: &str) -> Result<bool> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM artifact WHERE id=?1", [id], |r| {
        r.get(0)
    })?;
    Ok(n > 0)
}

/// Step 0 of [`graft_rows`]: see the call site for why. Reads only; writes nothing.
///
/// Refuses exactly the case that fails today — both rows hold a slug, the slugs differ,
/// and the destination's slug has `entry_cite` children. An established destination slug
/// with no children is overwritten without error and still is: the refusal is no wider
/// than its cause.
fn refuse_if_handover_would_strand_citations(
    tx: &rusqlite::Transaction<'_>,
    from_id: &str,
    into_id: &str,
) -> Result<()> {
    let slug_of = |id: &str| -> Result<Option<String>> {
        Ok(tx.query_row("SELECT slug FROM artifact WHERE id=?1", [id], |r| r.get(0))?)
    };
    let (Some(from_slug), Some(into_slug)) = (slug_of(from_id)?, slug_of(into_id)?) else {
        return Ok(());
    };
    if from_slug == into_slug {
        return Ok(());
    }
    let cites = |slug: &str| -> Result<i64> {
        Ok(tx.query_row(
            "SELECT COUNT(*) FROM entry_cite WHERE src_slug=?1",
            [slug],
            |r| r.get(0),
        )?)
    };
    let stranded = cites(&into_slug)?;
    if stranded == 0 {
        return Ok(());
    }
    let from_cited = cites(&from_slug)?;
    // Name only remedies that exist. No code updates `entry_cite.src_slug` (`rekey` moves
    // `src_local` and `dst_ref` only), so "move the citations first" is not an option; and the
    // reverse direction is refused by this same function when the OTHER row's slug is cited.
    let remedy = if from_cited == 0 {
        format!(
            "The reverse graft works, because `{from_slug}` owns no citations: \
             doc(action=\"graft\", from_id=`{into_id}`, into_id=`{from_id}`). Note what it \
             does — `{into_id}` is the row that is deleted (its id and abs_path stop \
             resolving) and `{from_id}` survives carrying `{into_slug}`'s history and slug. If \
             that is not the row you want to keep, nothing else can be done today."
        )
    } else {
        format!(
            "`{from_slug}` ALSO owns {from_cited} citation(s), so neither direction can graft \
             these two rows: the handover overwrites a slug in both. No tool re-keys \
             `entry_cite.src_slug`, so the citations cannot be moved either. Leave the rows \
             separate."
        )
    };
    Err(LibrarianRecoverableError::new(format!(
        "graft: refused — into_id `{into_id}` already owns slug `{into_slug}` with {stranded} \
         entry-grain citation(s) keyed by it, and from_id `{from_id}` carries slug \
         `{from_slug}`, which the graft would write over it. Those citations would be \
         stranded (entry_cite.src_slug has no ON UPDATE). Nothing was changed. {remedy}"
    )))
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
///
/// **Refusal:** if both rows hold a slug, the slugs differ, and `into_id`'s slug has
/// `entry_cite` children, the graft refuses before writing anything
/// ([`refuse_if_handover_would_strand_citations`]). The handover would write `from_id`'s
/// slug over it, and `entry_cite.src_slug` has no `ON UPDATE` clause, so those rows would
/// dangle and sqlite would fail at COMMIT with a bare `FOREIGN KEY constraint failed`.
/// Reachable through `doc(action="graft")`, which takes two arbitrary ids; the in-tree
/// `mv.rs` and `reseat_one` seed the destination slug-less and never hit it. Re-pointing
/// the displaced slug's rows instead of refusing is a policy choice (which slug should
/// survive) and is deliberately not made here.
///
/// **Refusal (renumber):** a fold that renumbers an id the SOURCE document defines by a
/// `## F-n` heading, or whose source cannot be read, refuses before writing anything
/// ([`merge_augmentation`]): the renumber changes the params only, and no tool renumbers one
/// entry in the params and the markdown together.
pub fn graft_rows(cat: &mut Catalog, from_id: &str, into_id: &str) -> Result<GraftReport> {
    if from_id == into_id {
        return Err(LibrarianRecoverableError::new(
            "graft: from_id and into_id are the same row",
        ));
    }
    if !row_exists(&cat.conn, from_id)? {
        return Err(LibrarianRecoverableError::new(format!(
            "graft: unknown from_id `{from_id}`"
        )));
    }
    if !row_exists(&cat.conn, into_id)? {
        return Err(LibrarianRecoverableError::new(format!(
            "graft: unknown into_id `{into_id}`"
        )));
    }

    // Taken before the transaction borrows the connection: `merge_augmentation` reads the
    // source document through the read door with the roots this call armed.
    let roots = cat.write_roots();
    let tx = cat
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

    // 0. Refuse, before any write, the one handover the schema cannot perform.
    //    `into_id` already owns a slug that durable `entry_cite` rows hang off, and
    //    `from_id` has a slug that step 6 would write over it. `entry_cite.src_slug`
    //    declares no `ON UPDATE`, so the displaced slug's rows would dangle and sqlite
    //    fails at COMMIT with a bare `FOREIGN KEY constraint failed` naming nothing.
    //    `doc(action="graft")` takes two arbitrary ids, so this is reachable — not only
    //    from the in-tree `mv`/`reseat` callers, which seed the destination slug-less.
    //    Checked inside the IMMEDIATE transaction so no writer can add a citation
    //    between the check and the handover; returning drops `tx` and rolls back
    //    (nothing has been written yet).
    refuse_if_handover_would_strand_citations(&tx, from_id, into_id)?;

    // 1-4. Re-point events / observations / links / event_edges onto `into_id`.
    let mut report = GraftReport::default();
    repoint_history(&tx, from_id, into_id, &mut report)?;

    // 5. Augmentation params — migrate or merge (before the delete: params live
    //    on `artifact_augmentation`, which also cascade-deletes with the source).
    merge_augmentation(&tx, from_id, into_id, &roots, &mut report)?;

    // 6. Hand the slug over, then delete source LAST — cascades any leftover
    //    dup links / edges / augmentation. `into_id` keeps whatever slug it already
    //    had (freshly minted or otherwise) when `from_id` had none.
    //
    //    **The handover is a three-step under deferred foreign keys, and the order is
    //    load-bearing.** `slug` is UNIQUE, so both rows cannot hold it at once — but
    //    the obvious resolution (delete the source first, then write its slug onto the
    //    destination) silently destroys data: `entry_cite.src_slug` is
    //    `REFERENCES artifact(slug) ON DELETE CASCADE` (mod.rs:201), so deleting the
    //    row that holds the slug takes every outgoing entry-grain citation with it, and
    //    the slug then lands on a destination that cites nothing. `origin='scan'` rows
    //    are rebuilt by the next write-mode `link_scan`; `origin='write'` rows are
    //    written only by `append_entry(cites=…)` and nothing recreates them.
    //
    //    Deferring the FK check is what makes the NULL step legal: between statement 1
    //    and 2 the child rows reference a slug no row holds, which an immediate check
    //    rejects and a deferred one tolerates. **It is NOT what saves the rows** —
    //    measured 2026-09-17: `defer_foreign_keys` with the delete-first order still
    //    leaves 0 rows, because `ON DELETE CASCADE` is an ACTION that fires at statement
    //    time and the pragma governs only when the CHECK runs. What saves them is that
    //    step 3 deletes a row that no longer owns a slug, so there is nothing to cascade.
    //    At commit the slug is held by `into_id` and every child resolves again.
    let from_slug: Option<String> =
        tx.query_row("SELECT slug FROM artifact WHERE id=?1", [from_id], |r| {
            r.get(0)
        })?;
    if let Some(slug) = from_slug.as_deref() {
        tx.execute_batch("PRAGMA defer_foreign_keys = ON")?;
        tx.execute("UPDATE artifact SET slug=NULL WHERE id=?1", [from_id])?;
        tx.execute(
            "UPDATE artifact SET slug=?1 WHERE id=?2",
            params![slug, into_id],
        )?;
    }
    tx.execute("DELETE FROM artifact WHERE id=?1", [from_id])?;
    if let Some(slug) = from_slug.as_deref() {
        // Counted AFTER the delete, deliberately. The same query before it would report
        // rows FOUND, and "found, then destroyed by the next statement" is exactly the
        // defect being fixed — a pre-delete count is green in both worlds and so cannot
        // be the number that moves when the handover breaks.
        report.entry_citations_carried = tx.query_row(
            "SELECT COUNT(*) FROM entry_cite WHERE src_slug=?1",
            [slug],
            |r| r.get::<_, i64>(0),
        )? as usize;
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
///
/// When that fold renumbers anything, the SOURCE document is read through the read door (with
/// `roots`, the managed roots the caller took before the transaction opened) and the fold is
/// refused if a heading there defines a renumbered id, or if the source cannot be read. A fold
/// that renumbers nothing reads no file.
fn merge_augmentation(
    tx: &rusqlite::Transaction<'_>,
    from_id: &str,
    into_id: &str,
    roots: &[std::path::PathBuf],
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

    // A renumber changes the id in the params and nowhere else: a source whose markdown
    // defines that id by heading would keep the old token in the file while its row moved
    // to the new one, and no tool renumbers one entry in both stores. Only when the fold
    // renumbered something, so a graft with nothing to renumber (`doc(move)`, whose
    // destination is never augmented) reads no file here.
    if !report.remap.is_empty() {
        refuse_renumber_of_markdown_defined(tx, from_id, into_id, roots, &report.remap)?;
    }

    // Guard: `unwrap_or_else` above only catches PARSE errors; valid-but-non-object
    // JSON (e.g. a bare array) would panic on index-assign. Fail recoverably instead.
    match into_json.as_object_mut() {
        Some(obj) => {
            obj.insert(coll.clone(), Value::Array(merged));
        }
        None => {
            return Err(LibrarianRecoverableError::new(format!(
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

/// Old ids in `remap` that a heading defines in `source_body` (a `## F-1 — title` line the
/// entry scanner reads, not a token cited in prose). The SOURCE body only: the survivor's
/// own headings are not a reason to refuse, because the survivor's entries keep their ids.
/// Pure; the result follows `remap`'s key order.
pub(crate) fn renumbered_ids_defined_in(
    remap: &BTreeMap<String, String>,
    source_body: &str,
) -> Vec<String> {
    let defined: std::collections::HashSet<String> =
        crate::librarian::tools::link_scan::extract::entry_sections(source_body)
            .into_iter()
            .map(|s| s.id)
            .collect();
    remap
        .keys()
        .filter(|old| defined.contains(*old))
        .cloned()
        .collect()
}

/// Refuses, before anything is written, a fold whose renumber would split an entry between
/// the params and the markdown of `from_id`'s document. Fails closed: a source that cannot
/// be read through the read door is a refusal, not a skip.
fn refuse_renumber_of_markdown_defined(
    tx: &rusqlite::Transaction<'_>,
    from_id: &str,
    into_id: &str,
    roots: &[std::path::PathBuf],
    remap: &BTreeMap<String, String>,
) -> Result<()> {
    let abs_path_of = |id: &str| -> Result<std::path::PathBuf> {
        Ok(std::path::PathBuf::from(tx.query_row(
            "SELECT abs_path FROM artifact WHERE id=?1",
            [id],
            |r| r.get::<_, String>(0),
        )?))
    };
    let from_path = abs_path_of(from_id)?;
    let from_label = crate::librarian::catalog::dual_write::document_label(&from_path);
    let into_label = crate::librarian::catalog::dual_write::document_label(&abs_path_of(into_id)?);
    // Two rows can wear one label (one file name in two folders): tell them apart by role.
    let (from_name, into_name) = if from_label == into_label {
        (
            format!("the folded row's {from_label}"),
            format!("the survivor's {into_label}"),
        )
    } else {
        (from_label, into_label)
    };
    let door = crate::librarian::read_door::ReadDoor::new(roots.to_vec());
    let body = door.text(&from_path).map_err(|e| {
        // A file that is not there is put back; one the door refuses is made readable.
        let way_forward = if matches!(&e, crate::librarian::read_door::ReadError::Io(io)
            if io.kind() == std::io::ErrorKind::NotFound)
        {
            "it is not on disk. Put the file back at the folded row's path \
             (doc(action=\"find\") shows the live row if it moved) and retry, or leave the two \
             rows separate."
        } else {
            "the read door refuses it. Make it a readable regular file inside its repository \
             and retry, or leave the two rows separate."
        };
        LibrarianRecoverableError::new(format!(
            "graft: refused — {} would be renumbered to fit {into_name}, and {from_name} \
             could not be read to check whether it defines them by heading: {way_forward} \
             Nothing was changed.",
            remap.keys().cloned().collect::<Vec<_>>().join(", ")
        ))
    })?;
    let hits = renumbered_ids_defined_in(remap, &body);
    if hits.is_empty() {
        return Ok(());
    }
    Err(LibrarianRecoverableError::new(format!(
        "graft: refused — {} collide with entries of {into_name} and would be renumbered, but \
         {from_name} defines them by heading, and no tool renumbers an entry in the markdown \
         and the params together. Nothing was changed. Leave the two rows separate.",
        hits.join(", ")
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::TestArtifactRowBuilder;
    use crate::librarian::catalog::augmentation::{self, AugmentationRow};
    use crate::librarian::catalog::entry_cite::{self, EntryCiteRow};
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

    /// A real source document under a temp git root, with no entry heading: for a graft that
    /// renumbers, which reads the source before it writes. Returns the guard and the path.
    fn headingless_source() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        let path = dir.path().join("x.md");
        std::fs::write(&path, "# x\n").unwrap();
        let path = path.to_str().unwrap().to_string();
        (dir, path)
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

    /// The sibling test above asserts the slug ARRIVES on the destination. This one
    /// asserts the rows KEYED BY that slug arrive with it. `entry_cite.src_slug` is
    /// `REFERENCES artifact(slug) ON DELETE CASCADE` (catalog/mod.rs:201), so deleting
    /// the source artifact destroys every outgoing entry-grain citation before the slug
    /// is written onto the destination — the slug lands on a row that now cites nothing,
    /// and the sibling test is green throughout because it only asks where the slug went.
    #[test]
    fn graft_carries_durable_entry_citations_forward_not_only_the_slug() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "from", "/wt/x.md");
        art(&cat, "into", "/main/x.md");
        art(&cat, "third", "/main/y.md");
        cat.conn
            .execute("UPDATE artifact SET slug='my-tracker' WHERE id='from'", [])
            .unwrap();
        cat.conn
            .execute("UPDATE artifact SET slug='elsewhere' WHERE id='third'", [])
            .unwrap();

        // `origin: "write"` is load-bearing. A scan-origin row is derived state that the
        // next write-mode `link_scan` rebuilds from prose, so losing one proves nothing;
        // only a durable row makes this a test about data nothing can reconstruct. Change
        // it to ORIGIN_SCAN and this test still passes its own assertion while ceasing to
        // be about the defect.
        entry_cite::insert_with(
            &cat.conn,
            &EntryCiteRow {
                src_slug: "my-tracker".into(),
                src_local: "T-1".into(),
                dst_ref: "elsewhere:F-2".into(),
                rel: "cites".into(),
                origin: "write".into(),
                created_at: 1,
            },
        )
        .unwrap();

        // Positive control, and the reason this test can name its own cause: an INBOUND
        // row is keyed on `dst_ref`, which carries no foreign key, so the cascade cannot
        // reach it. If the outgoing assertion reds while this one stays green, the FK
        // cascade is what did it. If both red, the test is measuring something else.
        entry_cite::insert_with(
            &cat.conn,
            &EntryCiteRow {
                src_slug: "elsewhere".into(),
                src_local: "F-9".into(),
                dst_ref: "my-tracker:T-1".into(),
                rel: "cites".into(),
                origin: "write".into(),
                created_at: 1,
            },
        )
        .unwrap();

        let report = graft_rows(&mut cat, "from", "into").unwrap();

        let inbound = entry_cite::incoming(&cat, "my-tracker:T-1").unwrap();
        assert_eq!(
            inbound.len(),
            1,
            "control: `dst_ref` has no foreign key, so an inbound citation is untouched \
             by the cascade — if this one reds, the test is not measuring the cascade"
        );

        let outbound = entry_cite::outgoing(&cat, "my-tracker").unwrap();
        assert_eq!(
            outbound.len(),
            1,
            "the durable outgoing citation must follow the slug across the graft; \
             ON DELETE CASCADE on entry_cite.src_slug destroys it when the source row goes, \
             and no rescan rebuilds an origin='write' row"
        );
        assert_eq!(outbound[0].src_local, "T-1");
        assert_eq!(
            outbound[0].origin, "write",
            "and it must arrive still durable — re-pointing it as a scan row would make \
             the next link_scan prune it"
        );
        assert_eq!(
            report.entry_citations_carried, 1,
            "and the report must SAY it carried one. `doc(move)` returns a four-field \
             inbound-citation block; the outgoing edges had no field at all, so their \
             loss moved no number a caller could read"
        );
    }
    /// `doc(action="graft")` takes two arbitrary ids, so `into_id` can be an established
    /// artifact that already owns a slug with durable outgoing citations. The handover
    /// would overwrite that slug, and `entry_cite.src_slug` has no `ON UPDATE` clause, so
    /// the old behaviour was a bare `FOREIGN KEY constraint failed` at COMMIT naming no
    /// table, column or row. The graft must now refuse BEFORE touching anything, naming the
    /// slug and the count, and leave both rows and every citation exactly as they were.
    #[test]
    fn graft_onto_an_established_slug_with_citations_refuses_actionably_and_changes_nothing() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "from", "/wt/x.md");
        art(&cat, "into", "/main/x.md");
        cat.conn
            .execute("UPDATE artifact SET slug='from-ledger' WHERE id='from'", [])
            .unwrap();
        cat.conn
            .execute("UPDATE artifact SET slug='into-ledger' WHERE id='into'", [])
            .unwrap();
        for (slug, local) in [("into-ledger", "F-1"), ("into-ledger", "F-2")] {
            entry_cite::insert_with(
                &cat.conn,
                &EntryCiteRow {
                    src_slug: slug.into(),
                    src_local: local.into(),
                    dst_ref: "xyz".into(),
                    rel: "cites".into(),
                    origin: "write".into(),
                    created_at: 1,
                },
            )
            .unwrap();
        }
        events::insert(
            &cat,
            &TestEventRowBuilder::new("from", "note")
                .with_id("e1")
                .build(),
        )
        .unwrap();

        let err = graft_rows(&mut cat, "from", "into").unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            !msg.contains("FOREIGN KEY constraint failed"),
            "the opaque sqlite error must not be what the caller reads: {msg}"
        );
        assert!(
            msg.contains("into-ledger") && msg.contains("2"),
            "the refusal must name the displaced slug and how many durable citations it \
             would strand: {msg}"
        );
        assert!(
            err.downcast_ref::<LibrarianRecoverableError>().is_some(),
            "an expected refusal is a recoverable error, not a bug: {msg}"
        );

        // Nothing moved: refusal precedes the transaction.
        let rows: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM artifact WHERE id IN ('from','into')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rows, 2, "neither row may be deleted by a refused graft");
        let ev: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM events WHERE artifact_id='from'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ev, 1, "history stays on the source of a refused graft");
        assert_eq!(entry_cite::outgoing(&cat, "into-ledger").unwrap().len(), 2);
    }

    /// Control for the test above: the refusal is scoped to the case that actually fails.
    /// An established destination slug with NO durable children is overwritten without
    /// error today, and must stay that way — otherwise the refusal is wider than its cause.
    #[test]
    fn graft_onto_an_established_slug_without_citations_still_succeeds() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "from", "/wt/x.md");
        art(&cat, "into", "/main/x.md");
        cat.conn
            .execute("UPDATE artifact SET slug='from-ledger' WHERE id='from'", [])
            .unwrap();
        cat.conn
            .execute("UPDATE artifact SET slug='into-ledger' WHERE id='into'", [])
            .unwrap();
        graft_rows(&mut cat, "from", "into").unwrap();
        let slug: Option<String> = cat
            .conn
            .query_row("SELECT slug FROM artifact WHERE id='into'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(slug.as_deref(), Some("from-ledger"));
    }
    /// The refusal names only remedies that exist. "Move the citations first" named a tool
    /// nothing provides (no code updates `entry_cite.src_slug`; `rekey` moves `src_local` and
    /// `dst_ref` only), and "graft the other way" fails identically when the OTHER row's slug
    /// also owns citations. So the text must follow the facts: the reverse direction when it is
    /// actually available, and an honest "none exists" when both sides are cited.
    #[test]
    fn graft_refusal_names_only_remedies_that_exist() {
        let cite = |cat: &Catalog, slug: &str, local: &str| {
            entry_cite::insert_with(
                &cat.conn,
                &EntryCiteRow {
                    src_slug: slug.into(),
                    src_local: local.into(),
                    dst_ref: "xyz".into(),
                    rel: "cites".into(),
                    origin: "write".into(),
                    created_at: 1,
                },
            )
            .unwrap();
        };
        let fresh = || {
            let cat = Catalog::open_in_memory().unwrap();
            art(&cat, "from", "/wt/x.md");
            art(&cat, "into", "/main/x.md");
            cat.conn
                .execute("UPDATE artifact SET slug='from-ledger' WHERE id='from'", [])
                .unwrap();
            cat.conn
                .execute("UPDATE artifact SET slug='into-ledger' WHERE id='into'", [])
                .unwrap();
            cat
        };

        // Only the destination is cited: the reverse direction works, and the text says so —
        // and verifiably does: run it.
        let mut cat = fresh();
        cite(&cat, "into-ledger", "F-1");
        let msg = format!("{:#}", graft_rows(&mut cat, "from", "into").unwrap_err());
        assert!(
            !msg.contains("citations first"),
            "no such tool exists: {msg}"
        );
        assert!(
            msg.contains("from_id=`into`") && msg.contains("into_id=`from`"),
            "must spell out the direction that works: {msg}"
        );
        graft_rows(&mut cat, "into", "from")
            .expect("the direction the refusal recommends must actually succeed");

        // Both sides cited: neither direction can work, and the text must not pretend one can.
        let mut cat = fresh();
        cite(&cat, "into-ledger", "F-1");
        cite(&cat, "from-ledger", "T-1");
        let msg = format!("{:#}", graft_rows(&mut cat, "from", "into").unwrap_err());
        assert!(
            msg.contains("neither direction") && msg.contains("from-ledger"),
            "both-cited must say no graft direction is possible: {msg}"
        );
        assert!(!msg.contains("citations first"), "{msg}");
        let rev = format!("{:#}", graft_rows(&mut cat, "into", "from").unwrap_err());
        assert!(
            rev.contains("neither direction"),
            "the reverse call is refused too: {rev}"
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
        // A colliding id is renumbered only after the source's file is read, so it has one.
        let (_dir, src) = headingless_source();
        art(&cat, "from", &src);
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
        let (_dir, src) = headingless_source();
        art(&cat, "from", &src);
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

    /// Two catalogued ledgers of one collection, each with a real file under a temp git
    /// root (the fold reads the SOURCE document through the read door). `from_text` and
    /// `into_text` are the files' bytes; `None` leaves the source file absent. Both rows
    /// carry `F-1` in params, so folding `from` onto `into` renumbers it to `F-2`.
    fn colliding_ledgers(from_text: Option<&str>, into_text: &str) -> (tempfile::TempDir, Catalog) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        std::fs::create_dir_all(dir.path().join("wt")).unwrap();
        std::fs::create_dir_all(dir.path().join("main")).unwrap();
        let from_path = dir.path().join("wt/ledger.md");
        let into_path = dir.path().join("main/ledger.md");
        if let Some(text) = from_text {
            std::fs::write(&from_path, text).unwrap();
        }
        std::fs::write(&into_path, into_text).unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        art(&cat, "from", from_path.to_str().unwrap());
        art(&cat, "into", into_path.to_str().unwrap());
        augmentation::upsert(
            &cat,
            &aug("from", "items", r#"{"items":[{"id":"F-1","t":"wt"}]}"#),
        )
        .unwrap();
        augmentation::upsert(
            &cat,
            &aug("into", "items", r#"{"items":[{"id":"F-1","t":"main"}]}"#),
        )
        .unwrap();
        (dir, cat)
    }

    /// Review Focus 7. The source's markdown defines `F-1` by heading, so a renumber to
    /// `F-2` would change the params and leave the heading: refused, nothing written.
    /// Mutation: skip the `refuse_renumber_of_markdown_defined` call in `merge_augmentation`
    /// and the graft succeeds and deletes `from`.
    #[test]
    fn a_graft_that_would_renumber_a_markdown_defined_entry_is_refused() {
        let (dir, mut cat) = colliding_ledgers(Some("## F-1 — x\n"), "## F-1 — y\n");
        let msg = format!("{:#}", graft_rows(&mut cat, "from", "into").unwrap_err());
        assert!(msg.contains("F-1"), "{msg}");
        assert!(msg.contains("Nothing was changed"), "{msg}");
        assert!(
            msg.contains("the folded row's") && msg.contains("the survivor's"),
            "two rows with one label are told apart: {msg}"
        );
        assert!(
            !msg.contains(dir.path().to_str().unwrap()),
            "a refusal names a document by label, never by path: {msg}"
        );
        assert!(row_exists(&cat.conn, "from").unwrap());
        let into_params = augmentation::get(&cat, "into").unwrap().unwrap().params;
        assert!(
            into_params.contains("\"main\"") && !into_params.contains("\"wt\""),
            "the refused graft leaves the survivor's params alone: {into_params}"
        );
    }

    /// The survivor's own heading is not a reason to refuse: the renumbered entry is the
    /// SOURCE's, and its markdown defines nothing. Mutation: also check the `into` body.
    #[test]
    fn a_survivor_heading_alone_does_not_block_a_renumber() {
        let (_dir, mut cat) = colliding_ledgers(Some("# Ledger\n"), "## F-1 — y\n");
        let report = graft_rows(&mut cat, "from", "into").unwrap();
        assert_eq!(
            report.remap,
            BTreeMap::from([("F-1".to_string(), "F-2".to_string())])
        );
        assert!(!row_exists(&cat.conn, "from").unwrap());
    }

    /// Fail closed: a source the door cannot read is a refusal, not "no heading". Mutation:
    /// map the read error to an empty body.
    #[test]
    fn a_graft_that_renumbers_refuses_when_the_source_cannot_be_read() {
        let (_dir, mut cat) = colliding_ledgers(None, "## F-1 — y\n");
        let msg = format!("{:#}", graft_rows(&mut cat, "from", "into").unwrap_err());
        assert!(
            msg.contains("F-1") && msg.contains("could not be read"),
            "{msg}"
        );
        assert!(
            msg.contains("Put the file back at the folded row's path")
                && msg.contains("leave the two rows separate"),
            "a missing source says what to do next: {msg}"
        );
        assert!(
            msg.contains("not on disk") && !msg.contains("readable regular file"),
            "a missing source is not reported as a refused one: {msg}"
        );
        assert!(row_exists(&cat.conn, "from").unwrap());
    }

    /// A source the read door REFUSES (a symlink to a file outside the repository) is not a
    /// missing one: putting a file back is the wrong action, so the refusal says to make it a
    /// readable regular file. Mutation: report every unreadable source as missing.
    #[cfg(unix)]
    #[test]
    fn a_graft_whose_source_the_door_refuses_says_to_make_it_readable() {
        let (dir, mut cat) = colliding_ledgers(Some("# x\n"), "## F-1 — y\n");
        let from_file = dir.path().join("wt/ledger.md");
        let elsewhere = tempfile::tempdir().unwrap();
        let outside = elsewhere.path().join("outside.md");
        std::fs::write(&outside, "# x\n").unwrap();
        std::fs::remove_file(&from_file).unwrap();
        std::os::unix::fs::symlink(&outside, &from_file).unwrap();
        let msg = format!("{:#}", graft_rows(&mut cat, "from", "into").unwrap_err());
        assert!(
            msg.contains("the read door refuses it")
                && msg.contains("readable regular file")
                && !msg.contains("Put the file back"),
            "{msg}"
        );
        assert!(row_exists(&cat.conn, "from").unwrap());
    }

    /// Headings define; prose citations do not. Mutation: match the tokens anywhere in the
    /// body.
    #[test]
    fn renumbered_ids_defined_in_reads_headings_not_prose() {
        let remap = BTreeMap::from([
            ("F-1".to_string(), "F-3".to_string()),
            ("F-2".to_string(), "F-4".to_string()),
        ]);
        assert_eq!(
            renumbered_ids_defined_in(&remap, "## F-1 — x\n\nsee F-2\n"),
            vec!["F-1".to_string()]
        );
    }
}
