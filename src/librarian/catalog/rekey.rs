//! Move an entry-id namespace from one prefix to another, atomically.
//!
//! This is the catalog half of `doc(action="rekey_prefix")`. It exists because
//! `update_entry` refuses `id` (`augmentation.rs:340`) and is *right* to: entry ids key
//! `entry_cite` rows, so re-keying one through a field patch would strand its citations.
//! The refusal's predicate was always correct; what was missing is a surface that moves the
//! citations WITH the id. This is that surface.
//!
//! **Two tables, and they are treated differently on purpose.**
//!
//! - `entry_reservation` — the catalog's own per-`(artifact, prefix)` high-water mark. Moved.
//!   It is a *second* copy of the committed frontmatter `entry_high_water_<PREFIX>` key, and
//!   moving only the frontmatter half leaves the allocator issuing ids under the old prefix.
//! - `entry_cite` — split by `origin`, because the two kinds are not the same kind of fact:
//!   - `origin='write'` rows come from `append_entry(cites=…)`. They are a human's assertion,
//!     nothing re-derives them, and `origin` sits outside the PK precisely so a later scan
//!     cannot clobber them (`entry_cite.rs:16-26`). **Re-pointed.**
//!   - `origin='scan'` rows are derived from prose by `link_scan`, which prunes and re-derives
//!     them per source on every write-mode pass. **Dropped, not re-pointed** — see below.
//!
//! **Why derived rows are DROPPED rather than rewritten**, which reads backwards until you
//! follow it through: rewriting them would assert edges the files on disk do not support. A
//! rekey moves the ledger's own ids; the prose in every *citing* file still says the old
//! token until someone repoints it. A rewritten scan row would claim those citations already
//! point at the new id. It would also be pointless — the next `link_scan(write=true)`
//! prunes by source and re-derives from the prose, silently reverting every rewritten row.
//! Dropping them makes the intermediate state honest: the citations genuinely are unresolved
//! until the prose sweep lands, and `doctor` says so instead of a stale row hiding it.

use crate::librarian::catalog::entry_cite;
use crate::librarian::catalog::Catalog;
use crate::librarian::tools::LibrarianRecoverableError;
use anyhow::Result;
use rusqlite::{params, OptionalExtension};

/// What a rekey moved. Counted separately because they are different kinds of fact —
/// a dropped derived row is not a loss, a re-pointed durable one is a migration.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct RekeyReport {
    /// `origin='write'` rows where this ledger is the CITER, re-pointed to the new `src_local`.
    pub durable_outbound_repointed: usize,
    /// `origin='write'` rows where this ledger is the CITED, re-pointed to the new `dst_ref`.
    pub durable_inbound_repointed: usize,
    /// `origin='scan'` rows dropped for re-derivation by the next write-mode `link_scan`.
    pub derived_rows_dropped: usize,
    /// Durable rows whose re-point was refused because the destination key already held the
    /// same edge, and which were therefore dropped as duplicates rather than left stranded.
    /// Non-zero is unusual and worth reporting to the caller, not an error.
    pub duplicates_merged: usize,
    /// Whether an `entry_reservation` row existed and moved. `false` is normal, not a failure:
    /// a params-backed ledger never allocates through `allocate_entry_id` and so has no row.
    pub reservation_moved: bool,
}

/// Retarget one entry token, **preserving its numeric tail byte-for-byte**.
///
/// The verbatim tail is the whole point and is not incidental. `graft::split_id` parses the
/// suffix as a `u64`, which is right for allocating the next free id and wrong here: it maps
/// `T-001` to `1`, so re-formatting would silently emit `SRI-1` and change the id by more
/// than its prefix. This corpus has live zero-padded ids — `tool-usage-patterns` spells
/// `T-001`…`T-013` padded and `T-14`…`T-34` bare, in one ledger — and that padding is load
/// bearing: it is the only thing that kept the founding `T` collision disjoint.
///
/// Returns `None` unless `id` is exactly `<from_prefix>-<digits>`, so a token that merely
/// *starts with* the prefix (`TX-1` under `from="T"`) is left alone.
pub(crate) fn retarget_token(id: &str, from_prefix: &str, to_prefix: &str) -> Option<String> {
    let digits = id.strip_prefix(from_prefix)?.strip_prefix('-')?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(format!("{to_prefix}-{digits}"))
}

/// Delete a durable row whose re-point was refused because the destination key already
/// holds the same edge. Returns rows removed.
///
/// **This is the cleanup `graft` does not need and a rekey does.** `graft::repoint_history`
/// also re-points with `UPDATE OR IGNORE`, and it can leave the ignored row where it lies
/// because its caller's final `DELETE FROM artifact` cascades the leftover away
/// (`graft.rs:112-120`). A rekey deletes no artifact, so the same idiom would strand a row
/// keyed to a token the ledger no longer defines — which is exactly `doctor`'s
/// `entry_without_definition`, the state `rekey_prefix` exists to repair. Borrowing the
/// statement without borrowing what made it safe is how this feature would have inflicted
/// its own bug.
///
/// Dropping is lossless here and that is why it is a delete rather than a refusal: the
/// destination row asserts the same `(citer, cited, rel)` fact under the new token, so the
/// source row is a duplicate of a fact already recorded, not a second fact.
fn drop_superseded(
    tx: &rusqlite::Transaction<'_>,
    src_slug: &str,
    src_local: &str,
    dst_ref: &str,
    rel: &str,
) -> Result<usize> {
    Ok(tx.execute(
        "DELETE FROM entry_cite WHERE src_slug=?1 AND src_local=?2 AND dst_ref=?3 AND rel=?4",
        params![src_slug, src_local, dst_ref, rel],
    )?)
}

/// Move every `<from_prefix>-N` entry id owned by `artifact_id` to `<to_prefix>-N`, across the
/// catalog tables keyed on it.
///
/// Runs in a single `IMMEDIATE` transaction: either the whole rekey lands or none of it does,
/// so a mid-rekey failure can never leave half the citations pointing at a token the ledger
/// no longer defines.
pub fn rekey_prefix_rows(
    cat: &mut Catalog,
    artifact_id: &str,
    from_prefix: &str,
    to_prefix: &str,
) -> Result<RekeyReport> {
    if from_prefix == to_prefix {
        return Err(LibrarianRecoverableError::new(
            "rekey_prefix: `from` and `to` are the same prefix — nothing to move",
        ));
    }

    let tx = cat
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let mut report = RekeyReport::default();

    // A ledger with no slug has no entry-grain citations at all — `entry_cite` is keyed on
    // the slug, and `ensure_slug` mints one lazily on the first `append_entry(cites=…)`.
    // Its reservation row still has to move, so this is a skip and not an early return.
    let slug: Option<String> = tx
        .query_row(
            "SELECT slug FROM artifact WHERE id=?1",
            [artifact_id],
            |r| r.get(0),
        )
        .optional()?
        .flatten();

    if let Some(slug) = slug {
        // --- Outbound: this ledger is the CITER, and the token sits in `src_local`.
        let mut stmt =
            tx.prepare("SELECT src_local, dst_ref, rel, origin FROM entry_cite WHERE src_slug=?1")?;
        let outbound: Vec<(String, String, String, String)> = stmt
            .query_map([&slug], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })?
            .collect::<std::result::Result<_, _>>()?;
        drop(stmt);

        for (src_local, dst_ref, rel, origin) in outbound {
            // `None` means a different namespace in the same ledger (`W-4` under a rekey of
            // `F`). Leaving it alone is the whole reason this is a per-token check rather
            // than a `LIKE 'F%'` sweep.
            let Some(new_local) = retarget_token(&src_local, from_prefix, to_prefix) else {
                continue;
            };
            if origin == entry_cite::ORIGIN_SCAN {
                report.derived_rows_dropped += tx.execute(
                    "DELETE FROM entry_cite \
                     WHERE src_slug=?1 AND src_local=?2 AND dst_ref=?3 AND rel=?4",
                    params![slug, src_local, dst_ref, rel],
                )?;
                continue;
            }
            let moved = tx.execute(
                "UPDATE OR IGNORE entry_cite SET src_local=?1 \
                 WHERE src_slug=?2 AND src_local=?3 AND dst_ref=?4 AND rel=?5",
                params![new_local, slug, src_local, dst_ref, rel],
            )?;
            report.durable_outbound_repointed += moved;
            if moved == 0 {
                report.duplicates_merged +=
                    drop_superseded(&tx, &slug, &src_local, &dst_ref, &rel)?;
            }
        }

        // --- Inbound: someone else is the citer, and the token is PACKED into `dst_ref` as
        // `<slug>:<local>`. Same rekey, different column and different string surgery — which
        // is why this cannot be one query over both directions.
        let mut stmt = tx.prepare(
            "SELECT src_slug, src_local, dst_ref, rel, origin FROM entry_cite WHERE dst_ref LIKE ?1",
        )?;
        let pattern = format!("{slug}:{from_prefix}-%");
        let inbound: Vec<(String, String, String, String, String)> = stmt
            .query_map([&pattern], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })?
            .collect::<std::result::Result<_, _>>()?;
        drop(stmt);

        for (src_slug, src_local, dst_ref, rel, origin) in inbound {
            // Re-derive rather than trusting the LIKE: `_` is a single-character wildcard in
            // SQL LIKE, so the pattern above is a prefilter and never the predicate.
            let Some(local) = dst_ref.strip_prefix(&format!("{slug}:")) else {
                continue;
            };
            let Some(new_local) = retarget_token(local, from_prefix, to_prefix) else {
                continue;
            };
            if origin == entry_cite::ORIGIN_SCAN {
                report.derived_rows_dropped += tx.execute(
                    "DELETE FROM entry_cite \
                     WHERE src_slug=?1 AND src_local=?2 AND dst_ref=?3 AND rel=?4",
                    params![src_slug, src_local, dst_ref, rel],
                )?;
                continue;
            }
            let moved = tx.execute(
                "UPDATE OR IGNORE entry_cite SET dst_ref=?1 \
                 WHERE src_slug=?2 AND src_local=?3 AND dst_ref=?4 AND rel=?5",
                params![
                    format!("{slug}:{new_local}"),
                    src_slug,
                    src_local,
                    dst_ref,
                    rel
                ],
            )?;
            report.durable_inbound_repointed += moved;
            if moved == 0 {
                report.duplicates_merged +=
                    drop_superseded(&tx, &src_slug, &src_local, &dst_ref, &rel)?;
            }
        }
    }

    // --- The catalog's own high-water mark. Two independent things are going on here.
    //
    // `AND prefix=?3` is load-bearing: the PK is `(artifact_id, prefix)` and a ledger may own
    // two namespaces at once (`session-log-bug-fix-work-stream` holds `F` and `W`), so an
    // unqualified UPDATE drags the untouched one along.
    //
    // The destination check is what makes that predicate TESTABLE, and it is a real guard in
    // its own right. Written as `UPDATE OR IGNORE`, a rekey onto an already-occupied prefix
    // silently discards the destination's high-water mark — and, worse, the `OR IGNORE` then
    // absorbs the missing-predicate bug too: the first row moves, the second collides and is
    // dropped, and the end state is byte-identical to the correct one. Measured 2026-09-17 by
    // mutation: `AND prefix=?3` -> `AND ?3=?3` SURVIVED an 8-test suite under `OR IGNORE`, and
    // KILLS once the refusal below replaces it. `graft` meets the same collision and merges by
    // MAX (`graft.rs:179-189`), which is right for a merge and wrong here — a rekey that
    // silently folded two counters could re-issue a live id.
    let occupied: bool = tx
        .query_row(
            "SELECT 1 FROM entry_reservation WHERE artifact_id=?1 AND prefix=?2",
            params![artifact_id, to_prefix],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if occupied {
        return Err(LibrarianRecoverableError::with_hint(
            format!(
                "rekey_prefix: `{artifact_id}` already holds an entry reservation for `{to_prefix}`"
            ),
            format!(
                "Moving `{from_prefix}` onto it would discard one of the two high-water marks, \
                 and the allocator would then re-issue ids that are already live. Pick a free \
                 prefix, or rekey `{to_prefix}` out of the way first."
            ),
        ));
    }
    report.reservation_moved = tx.execute(
        "UPDATE entry_reservation SET prefix=?1 WHERE artifact_id=?2 AND prefix=?3",
        params![to_prefix, artifact_id, from_prefix],
    )? > 0;

    tx.commit()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::TestArtifactRowBuilder;
    use crate::librarian::catalog::entry_cite::{self, EntryCiteRow, ORIGIN_SCAN};
    use crate::librarian::catalog::Catalog;

    fn art(cat: &Catalog, id: &str, path: &str, slug: &str) {
        let row = TestArtifactRowBuilder::new(id)
            .with_abs_path(path)
            .with_kind("tracker")
            .build();
        crate::librarian::catalog::artifact::upsert(cat, &row).unwrap();
        cat.conn
            .execute("UPDATE artifact SET slug=?1 WHERE id=?2", params![slug, id])
            .unwrap();
    }

    fn cite(cat: &Catalog, src_slug: &str, src_local: &str, dst_ref: &str, origin: &str) {
        entry_cite::insert_with(
            &cat.conn,
            &EntryCiteRow {
                src_slug: src_slug.into(),
                src_local: src_local.into(),
                dst_ref: dst_ref.into(),
                rel: "cites".into(),
                origin: origin.into(),
                created_at: 1,
            },
        )
        .unwrap();
    }

    fn reserve(cat: &Catalog, artifact_id: &str, prefix: &str, max_allocated: i64) {
        cat.conn
            .execute(
                "INSERT INTO entry_reservation (artifact_id, prefix, max_allocated, updated_at)
                 VALUES (?1, ?2, ?3, '2026-01-01T00:00:00.000Z')",
                params![artifact_id, prefix, max_allocated],
            )
            .unwrap();
    }

    fn locals(cat: &Catalog, slug: &str) -> Vec<String> {
        let mut stmt = cat
            .conn
            .prepare("SELECT src_local FROM entry_cite WHERE src_slug=?1 ORDER BY src_local")
            .unwrap();
        let v = stmt
            .query_map([slug], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        v
    }

    fn dsts(cat: &Catalog) -> Vec<String> {
        let mut stmt = cat
            .conn
            .prepare("SELECT dst_ref FROM entry_cite ORDER BY dst_ref")
            .unwrap();
        let v = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        v
    }

    // ---- retarget_token ----

    /// The zero-padded fixture is load-bearing: `tool-usage-patterns` really does hold
    /// `T-001` and `T-14` in one ledger. Parse-then-reformat (what `graft::split_id` does,
    /// correctly, for allocation) would emit `SRI-1` here and change the id by more than its
    /// prefix. Replace `T-001` with a bare id and this test passes against that bug.
    #[test]
    fn the_numeric_tail_is_preserved_verbatim_so_zero_padding_survives() {
        assert_eq!(
            retarget_token("T-001", "T", "SRI").as_deref(),
            Some("SRI-001")
        );
        assert_eq!(
            retarget_token("T-14", "T", "SRI").as_deref(),
            Some("SRI-14")
        );
    }

    /// `TX-1` shares a leading `T` with the prefix being moved and is a different namespace.
    /// A `starts_with` check without the `-` would rename it to `SRI-1` and collide.
    #[test]
    fn a_token_that_merely_starts_with_the_prefix_is_left_alone() {
        assert_eq!(retarget_token("TX-1", "T", "SRI"), None);
        assert_eq!(retarget_token("T1", "T", "SRI"), None);
        assert_eq!(retarget_token("T-", "T", "SRI"), None);
        assert_eq!(retarget_token("T-1a", "T", "SRI"), None);
    }

    // ---- rekey_prefix_rows ----

    /// `tool-usage-patterns:T-19` is the real shape of this: a durable row written by
    /// `append_entry(cites=…)` that no scan will ever rebuild.
    #[test]
    fn a_durable_outbound_row_follows_the_entry_to_its_new_prefix() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "led", "/repo/ledger.md", "my-ledger");
        cite(&cat, "my-ledger", "T-19", "4059035cf39e6aab", "write");

        let report = rekey_prefix_rows(&mut cat, "led", "T", "SRI").unwrap();

        assert_eq!(report.durable_outbound_repointed, 1);
        assert_eq!(locals(&cat, "my-ledger"), vec!["SRI-19".to_string()]);
    }

    #[test]
    fn a_durable_inbound_row_is_repointed_at_the_new_token() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "led", "/repo/ledger.md", "my-ledger");
        art(&cat, "other", "/repo/other.md", "other-log");
        cite(&cat, "other-log", "F-3", "my-ledger:T-7", "write");

        let report = rekey_prefix_rows(&mut cat, "led", "T", "SRI").unwrap();

        assert_eq!(report.durable_inbound_repointed, 1);
        assert_eq!(dsts(&cat), vec!["my-ledger:SRI-7".to_string()]);
    }
    /// A durable citation that collides on re-point must be DROPPED, not left behind.
    ///
    /// `graft` re-points with the same `UPDATE OR IGNORE` and can safely leave the ignored
    /// row, because its caller's `DELETE FROM artifact` cascades it away. A rekey deletes
    /// nothing. Leaving it would strand `my-ledger:T-5` — a citation keyed to a token the
    /// ledger no longer defines — which is `doctor`'s `entry_without_definition`, the exact
    /// state this whole feature exists to repair.
    ///
    /// The fixture's load-bearing detail is that BOTH `T-5` and `SRI-5` cite the same target,
    /// so the re-point genuinely collides on the PK `(src_slug, src_local, dst_ref, rel)`.
    /// Point them at different targets and there is no collision, the `OR IGNORE` never
    /// fires, and this test passes while guarding nothing.
    #[test]
    fn a_durable_row_that_collides_on_repoint_is_dropped_not_stranded() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "led", "/repo/ledger.md", "my-ledger");
        cite(&cat, "my-ledger", "T-5", "abcdef0123456789", "write");
        cite(&cat, "my-ledger", "SRI-5", "abcdef0123456789", "write");

        let report = rekey_prefix_rows(&mut cat, "led", "T", "SRI").unwrap();

        assert_eq!(report.duplicates_merged, 1);
        assert_eq!(report.durable_outbound_repointed, 0, "the move was refused");
        assert_eq!(
            locals(&cat, "my-ledger"),
            vec!["SRI-5".to_string()],
            "no row may survive under the old token — that is entry_without_definition"
        );
    }

    /// Derived rows are DROPPED, never rewritten. A rewritten scan row would assert that the
    /// citing prose already says the new token — which it does not until the sweep lands —
    /// and the next `link_scan(write=true)` would revert it anyway.
    #[test]
    fn derived_rows_are_dropped_rather_than_rewritten() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "led", "/repo/ledger.md", "my-ledger");
        art(&cat, "other", "/repo/other.md", "other-log");
        cite(&cat, "my-ledger", "T-3", "abcdef0123456789", ORIGIN_SCAN);
        cite(&cat, "other-log", "F-1", "my-ledger:T-3", ORIGIN_SCAN);

        let report = rekey_prefix_rows(&mut cat, "led", "T", "SRI").unwrap();

        assert_eq!(report.derived_rows_dropped, 2);
        assert_eq!(report.durable_outbound_repointed, 0);
        assert_eq!(report.durable_inbound_repointed, 0);
        assert!(
            dsts(&cat).is_empty(),
            "derived rows must be gone, not renamed — link_scan re-derives them from prose"
        );
    }

    /// The real target of the motivating repair has no `entry_reservation` row at all: it is
    /// params-backed, so it allocates through `augmentation::append_entry`, which persists its
    /// high-water mark nowhere but the params array. Erroring here would block the one case
    /// this feature exists for.
    ///
    /// **The durable row is what makes this test non-vacuous, and it is the point of the
    /// fixture.** `assert!(!reservation_moved)` is an absence assertion, monotone under
    /// removal — a `rekey_prefix_rows` that did nothing at all satisfies it, and this test
    /// passed against exactly that stub before the implementation landed. The paired positive
    /// is what distinguishes "ran, found no reservation row" from "never ran". Delete the
    /// citation and its assertion and the test keeps passing while guarding nothing.
    #[test]
    fn a_ledger_with_no_reservation_row_is_not_an_error() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "led", "/repo/ledger.md", "my-ledger");
        cite(&cat, "my-ledger", "T-4", "abcdef0123456789", "write");

        let report = rekey_prefix_rows(&mut cat, "led", "T", "SRI").unwrap();

        assert_eq!(
            report.durable_outbound_repointed, 1,
            "the rekey must have run — without this the absence assertion below is vacuous"
        );
        assert!(!report.reservation_moved);
    }

    /// `session-log-bug-fix-work-stream` really holds `F:172` and `W:144` — the PK is
    /// `(artifact_id, prefix)`, so a reservation UPDATE that forgets `AND prefix = ?` would
    /// drag the untouched namespace along. This is the test that catches that.
    #[test]
    fn rekeying_one_prefix_leaves_the_ledgers_other_prefix_untouched() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "led", "/repo/session-log.md", "session-log");
        reserve(&cat, "led", "F", 172);
        reserve(&cat, "led", "W", 144);
        cite(&cat, "session-log", "F-9", "abcdef0123456789", "write");
        cite(&cat, "session-log", "W-4", "abcdef0123456789", "write");

        let report = rekey_prefix_rows(&mut cat, "led", "F", "FRIC").unwrap();

        assert!(report.reservation_moved);
        assert_eq!(report.durable_outbound_repointed, 1, "only the F row moves");
        assert_eq!(
            locals(&cat, "session-log"),
            vec!["FRIC-9".to_string(), "W-4".to_string()]
        );
        let w: i64 = cat
            .conn
            .query_row(
                "SELECT max_allocated FROM entry_reservation WHERE artifact_id='led' AND prefix='W'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(w, 144, "W's reservation must survive a rekey of F");
        let f: i64 = cat
            .conn
            .query_row(
                "SELECT max_allocated FROM entry_reservation WHERE artifact_id='led' AND prefix='FRIC'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            f, 172,
            "F's high-water mark follows the prefix it belongs to"
        );
    }

    #[test]
    fn rekeying_a_prefix_onto_itself_is_refused() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "led", "/repo/ledger.md", "my-ledger");

        let err = rekey_prefix_rows(&mut cat, "led", "T", "T").unwrap_err();

        assert!(err.to_string().contains("same prefix"), "got: {err}");
    }
    /// A rekey onto a prefix this ledger already reserves would discard one of the two
    /// high-water marks and let the allocator re-issue live ids. `graft` merges by MAX in the
    /// same situation, which is right for a merge and wrong for a rename.
    ///
    /// This refusal is also what makes `AND prefix=?3` above testable at all — see that site's
    /// comment for the measured mutation it un-masks.
    #[test]
    fn rekeying_onto_a_prefix_this_ledger_already_reserves_is_refused() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "led", "/repo/session-log.md", "session-log");
        reserve(&cat, "led", "F", 172);
        reserve(&cat, "led", "W", 144);

        let err = rekey_prefix_rows(&mut cat, "led", "F", "W").unwrap_err();

        assert!(
            err.to_string()
                .contains("already holds an entry reservation"),
            "got: {err}"
        );
        let f: i64 = cat
            .conn
            .query_row(
                "SELECT max_allocated FROM entry_reservation WHERE artifact_id='led' AND prefix='F'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(f, 172, "the refusal must leave both counters intact");
    }
}
