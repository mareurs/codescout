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
use crate::librarian::catalog::{augmentation, Catalog};
use crate::librarian::tools::LibrarianRecoverableError;
use anyhow::Result;
use regex::Regex;
use rusqlite::{params, OptionalExtension};
use serde_json::Value;

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
    /// Entry ids rewritten inside the augmentation's `entry_collection` array.
    pub params_ids_repointed: usize,
    /// `pattern` strings in `params_schema` retargeted from the old prefix to the new one.
    /// **Zero alongside a non-zero `params_ids_repointed` is not automatically wrong** — a
    /// ledger may pin no id pattern at all — but it is the shape that would ship a schema
    /// rejecting every id, which is why the validation below is not optional.
    pub schema_patterns_repointed: usize,
    /// Whether the augmentation's `prompt` text mentioned the old prefix and was rewritten.
    pub prompt_rewritten: bool,
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

/// Retarget `pattern` strings that pin the id namespace, recursively, returning how many
/// moved.
///
/// **Deliberately exact-match on the canonical form `^<prefix>-\d+$`, not a substring
/// replace.** A `pattern` may constrain something that merely contains the prefix's letters,
/// and rewriting it would corrupt a schema this rekey has no business touching. Recognising
/// only the one form the corpus actually uses, and letting the caller's validation catch
/// anything else, trades a silent wrong edit for a loud refusal — which is the trade this
/// whole module is about.
fn retarget_patterns(v: &mut Value, from_prefix: &str, to_prefix: &str) -> usize {
    let canonical = format!("^{from_prefix}-\\d+$");
    let replacement = format!("^{to_prefix}-\\d+$");
    let mut moved = 0;
    match v {
        Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                if key == "pattern" && val.as_str() == Some(canonical.as_str()) {
                    *val = Value::String(replacement.clone());
                    moved += 1;
                    continue;
                }
                moved += retarget_patterns(val, from_prefix, to_prefix);
            }
        }
        Value::Array(arr) => {
            for val in arr.iter_mut() {
                moved += retarget_patterns(val, from_prefix, to_prefix);
            }
        }
        _ => {}
    }
    moved
}

/// Retarget namespace mentions in prose: concrete ids (`T-9`) **and the template form
/// `T-N`**, where `N` is the literal letter.
///
/// The template form is not a nicety — it is how augmentation prompts teach the entry shape.
/// The live `system-retrospective-improvements` prompt reads *"every task also keeps its own
/// `## T-N — <title>` body section (that heading is what makes T-N citable)"*, so a rewrite
/// that handled only digits would leave the ledger instructing its own maintainer to write
/// headings under the prefix it just abandoned.
///
/// `\b` on both sides is what keeps `TX-2` and `FT-1` out of a rekey of `T`: the first has no
/// `-` where one is required, and the second has no word boundary before its `T`.
fn retarget_prose(text: &str, from_prefix: &str, to_prefix: &str) -> Result<String> {
    let re = Regex::new(&format!(r"\b{}-(\d+|N)\b", regex::escape(from_prefix)))?;
    Ok(re
        .replace_all(text, format!("{to_prefix}-$1").as_str())
        .into_owned())
}

/// Move the augmentation's half of the namespace: the `entry_collection` row ids, the
/// `params_schema` id pattern, and the `prompt` text — in the caller's transaction.
///
/// **Why this cannot be a separate call from the row work, and why the schema cannot be a
/// separate call from the ids.** `params_schema` is validated against the MERGED params
/// (`augment.rs:36-52` `validate_merged_against_schema`), so a ledger pinning
/// `pattern: ^T-\d+$` refuses `SRI-1` on the first row written. Ids and schema are therefore
/// not two steps that happen to be adjacent; either both move or neither can. This is not a
/// deduction — it is the recorded experience of `d3282868`, the one time this was done by
/// hand: *"Schema is validated against the merged params, so schema and ids had to move in
/// one atomic call."*
///
/// **The validation at the end is a safety net over this function's own pattern rewriting,
/// not a formality.** Recognising which schema strings constrain an id is a heuristic — the
/// canonical `^<prefix>-\d+$` is recognised, and a hand-rolled equivalent
/// (`^(T)-[0-9]+$`, say) is not. Left there, an unrecognised pattern would ship a schema
/// that rejects every id in the ledger, and nothing would notice until the next
/// `append_entry` failed for reasons pointing nowhere near this rekey. Validating the new
/// params against the new schema converts that silent trap into a refusal at the moment of
/// the rekey, which is the only moment anyone has the context to act on it.
fn rekey_augmentation(
    tx: &rusqlite::Transaction<'_>,
    artifact_id: &str,
    from_prefix: &str,
    to_prefix: &str,
    report: &mut RekeyReport,
) -> Result<()> {
    let Some(row) = augmentation::get_by_conn(tx, artifact_id)? else {
        return Ok(());
    };

    // --- 1. Entry ids in the declared collection.
    let mut params: Value = serde_json::from_str(&row.params).map_err(|e| {
        LibrarianRecoverableError::new(format!(
            "rekey_prefix: `{artifact_id}`'s stored params are not valid JSON: {e}"
        ))
    })?;
    if let Some(coll) = row.entry_collection.as_deref() {
        if let Some(arr) = params.get_mut(coll).and_then(|v| v.as_array_mut()) {
            for entry in arr.iter_mut() {
                let Some(id) = entry.get("id").and_then(|v| v.as_str()) else {
                    continue;
                };
                let Some(new_id) = retarget_token(id, from_prefix, to_prefix) else {
                    continue;
                };
                entry["id"] = Value::String(new_id);
                report.params_ids_repointed += 1;
            }
        }
    }

    // --- 2. The id pattern in `params_schema`.
    let mut schema: Option<Value> = match row.params_schema.as_deref() {
        Some(text) => Some(serde_json::from_str(text).map_err(|e| {
            LibrarianRecoverableError::new(format!(
                "rekey_prefix: `{artifact_id}`'s stored params_schema is not valid JSON: {e}"
            ))
        })?),
        None => None,
    };
    if let Some(s) = schema.as_mut() {
        report.schema_patterns_repointed += retarget_patterns(s, from_prefix, to_prefix);
    }

    // --- 3. The augmentation prompt, which teaches the entry shape by quoting it.
    let new_prompt = retarget_prose(&row.prompt, from_prefix, to_prefix)?;
    report.prompt_rewritten = new_prompt != row.prompt;

    // --- 4. The safety net. See this function's doc comment: pattern recognition is a
    // heuristic, and the cost of a miss is a ledger that refuses its own ids at some later
    // `append_entry`, far from anything naming this rekey.
    if let Some(s) = schema.as_ref() {
        crate::librarian::tools::schema_validate::validate(s, &params).map_err(|e| {
            LibrarianRecoverableError::with_hint(
                format!(
                    "rekey_prefix: the rewritten ids violate `{artifact_id}`'s params_schema: {e}"
                ),
                format!(
                    "The schema pins the old namespace in a form this rekey does not \
                     recognise — only the canonical `^{from_prefix}-\\d+$` is retargeted \
                     automatically. Nothing has been written. Update the pattern to accept \
                     `{to_prefix}` and re-run, so the ids and the schema still move in one \
                     transaction."
                ),
            )
        })?;
    }

    tx.execute(
        "UPDATE artifact_augmentation
            SET params=?1, params_schema=?2, prompt=?3,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          WHERE artifact_id=?4",
        params![
            params.to_string(),
            schema.as_ref().map(|s| s.to_string()),
            new_prompt,
            artifact_id
        ],
    )?;
    Ok(())
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

    rekey_augmentation(&tx, artifact_id, from_prefix, to_prefix, &mut report)?;

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
    // ---- augmentation: params ids, schema pattern, prompt ----

    /// Minimal augmentation row. `entry_collection` names the params array holding the rows
    /// whose `id` field is the citable token.
    fn aug(
        cat: &Catalog,
        artifact_id: &str,
        coll: Option<&str>,
        params: serde_json::Value,
        schema: Option<serde_json::Value>,
        prompt: &str,
    ) {
        let row = crate::librarian::catalog::augmentation::AugmentationRow {
            artifact_id: artifact_id.into(),
            prompt: prompt.into(),
            params: params.to_string(),
            last_refreshed_at: None,
            refresh_count: 0,
            created_at: "2026-01-01T00:00:00.000Z".into(),
            updated_at: "2026-01-01T00:00:00.000Z".into(),
            render_template: None,
            params_schema: schema.map(|s| s.to_string()),
            append_mode: false,
            history_cap: None,
            entry_collection: coll.map(|c| c.into()),
            refreshed_at_commit: None,
        };
        crate::librarian::catalog::augmentation::upsert(cat, &row).unwrap();
    }

    fn params_of(cat: &Catalog, artifact_id: &str) -> serde_json::Value {
        let row = crate::librarian::catalog::augmentation::get(cat, artifact_id)
            .unwrap()
            .unwrap();
        serde_json::from_str(&row.params).unwrap()
    }

    fn schema_of(cat: &Catalog, artifact_id: &str) -> serde_json::Value {
        let row = crate::librarian::catalog::augmentation::get(cat, artifact_id)
            .unwrap()
            .unwrap();
        serde_json::from_str(&row.params_schema.unwrap()).unwrap()
    }

    /// The canonical id pattern this corpus uses, in the exact stored form: read off the live
    /// `system-retrospective-improvements` augmentation, where it is `"^T-\\d+$"` in JSON.
    fn id_schema(prefix: &str) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "tasks": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": { "id": { "type": "string", "pattern": format!("^{prefix}-\\d+$") } }
                    }
                }
            }
        })
    }

    #[test]
    fn params_entry_ids_move_with_the_prefix() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "led", "/repo/ledger.md", "my-ledger");
        aug(
            &cat,
            "led",
            Some("tasks"),
            serde_json::json!({"tasks": [{"id": "T-1", "task": "a"}, {"id": "T-2", "task": "b"}]}),
            Some(id_schema("T")),
            "keep the table",
        );

        let report = rekey_prefix_rows(&mut cat, "led", "T", "SRI").unwrap();

        assert_eq!(report.params_ids_repointed, 2);
        let p = params_of(&cat, "led");
        assert_eq!(p["tasks"][0]["id"], "SRI-1");
        assert_eq!(p["tasks"][1]["id"], "SRI-2");
        assert_eq!(p["tasks"][0]["task"], "a", "sibling fields are untouched");
    }

    #[test]
    fn the_id_pattern_in_params_schema_moves_in_the_same_write() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "led", "/repo/ledger.md", "my-ledger");
        aug(
            &cat,
            "led",
            Some("tasks"),
            serde_json::json!({"tasks": [{"id": "T-1"}]}),
            Some(id_schema("T")),
            "p",
        );

        let report = rekey_prefix_rows(&mut cat, "led", "T", "SRI").unwrap();

        assert_eq!(report.schema_patterns_repointed, 1);
        let s = schema_of(&cat, "led");
        assert_eq!(
            s["properties"]["tasks"]["items"]["properties"]["id"]["pattern"],
            "^SRI-\\d+$"
        );
    }

    /// **This is the atomicity requirement made executable**, and it is a claim about the
    /// domain rather than about this function's internals: the ids this rekey just wrote are
    /// rejected by the schema it replaced. So ids and schema are not two adjacent steps that
    /// could be split into two calls — either both move or the ledger is left refusing its
    /// own rows. `d3282868` learned this by hand; this pins it.
    #[test]
    fn the_new_ids_would_violate_the_old_schema_which_is_why_both_must_move_together() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "led", "/repo/ledger.md", "my-ledger");
        aug(
            &cat,
            "led",
            Some("tasks"),
            serde_json::json!({"tasks": [{"id": "T-1"}]}),
            Some(id_schema("T")),
            "p",
        );

        rekey_prefix_rows(&mut cat, "led", "T", "SRI").unwrap();

        let new_params = params_of(&cat, "led");
        let err = crate::librarian::tools::schema_validate::validate(&id_schema("T"), &new_params)
            .unwrap_err();
        assert!(
            err.to_string().contains("pattern") || err.to_string().contains("SRI-1"),
            "the OLD schema must reject the NEW ids — that is the whole reason for one write; got: {err}"
        );
        crate::librarian::tools::schema_validate::validate(&id_schema("SRI"), &new_params)
            .expect("and the NEW schema must accept them");
    }

    /// The safety net over this function's own pattern-recognition heuristic. `^(T)-[0-9]+$`
    /// constrains the id exactly as the canonical form does and is not recognised by it. Left
    /// unrewritten, the ledger would ship a schema rejecting every one of its ids, and the
    /// failure would surface at some later `append_entry` pointing nowhere near this rekey.
    ///
    /// The fixture's load-bearing detail is that the pattern is *equivalent to* the canonical
    /// one but not *equal to* it. Replace it with `^T-\d+$` and the rewrite succeeds, the
    /// validation passes, and this test silently stops testing anything.
    #[test]
    fn a_schema_pattern_the_rewrite_cannot_reach_is_refused_rather_than_shipped_broken() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "led", "/repo/ledger.md", "my-ledger");
        let hand_rolled = serde_json::json!({
            "type": "object",
            "properties": {
                "tasks": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": { "id": { "type": "string", "pattern": "^(T)-[0-9]+$" } }
                    }
                }
            }
        });
        aug(
            &cat,
            "led",
            Some("tasks"),
            serde_json::json!({"tasks": [{"id": "T-1"}]}),
            Some(hand_rolled),
            "p",
        );

        let err = rekey_prefix_rows(&mut cat, "led", "T", "SRI").unwrap_err();

        assert!(
            err.to_string().contains("params_schema"),
            "the refusal must name the schema as the thing that needs a human; got: {err}"
        );
        assert_eq!(
            params_of(&cat, "led")["tasks"][0]["id"],
            "T-1",
            "and the transaction must have rolled back — a refused rekey leaves nothing moved"
        );
    }

    #[test]
    fn the_prompt_text_follows_the_prefix() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "led", "/repo/ledger.md", "my-ledger");
        aug(
            &cat,
            "led",
            Some("tasks"),
            serde_json::json!({"tasks": [{"id": "T-1"}]}),
            Some(id_schema("T")),
            "every task keeps its own `## T-N — <title>` body section; see T-3 for the shape",
        );

        let report = rekey_prefix_rows(&mut cat, "led", "T", "SRI").unwrap();

        assert!(report.prompt_rewritten);
        let row = crate::librarian::catalog::augmentation::get(&cat, "led")
            .unwrap()
            .unwrap();
        assert!(
            row.prompt.contains("## SRI-N —") && row.prompt.contains("see SRI-3"),
            "both the literal-N template form and a concrete id must move; got: {}",
            row.prompt
        );
        assert!(!row.prompt.contains("T-N"), "no old token may survive");
    }

    /// A ledger owning two namespaces must keep the one not being rekeyed. Same law as the
    /// `entry_reservation` case, one layer up.
    #[test]
    fn a_params_id_under_a_different_prefix_is_left_alone() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "led", "/repo/ledger.md", "my-ledger");
        aug(
            &cat,
            "led",
            Some("items"),
            serde_json::json!({"items": [{"id": "T-1"}, {"id": "W-4"}, {"id": "TX-2"}]}),
            None,
            "p",
        );

        let report = rekey_prefix_rows(&mut cat, "led", "T", "SRI").unwrap();

        assert_eq!(report.params_ids_repointed, 1);
        let p = params_of(&cat, "led");
        assert_eq!(p["items"][0]["id"], "SRI-1");
        assert_eq!(p["items"][1]["id"], "W-4", "a different namespace");
        assert_eq!(p["items"][2]["id"], "TX-2", "shares a leading letter only");
    }

    #[test]
    fn a_ledger_with_no_augmentation_is_not_an_error() {
        let mut cat = Catalog::open_in_memory().unwrap();
        art(&cat, "led", "/repo/ledger.md", "my-ledger");
        cite(&cat, "my-ledger", "T-4", "abcdef0123456789", "write");

        let report = rekey_prefix_rows(&mut cat, "led", "T", "SRI").unwrap();

        assert_eq!(
            report.durable_outbound_repointed, 1,
            "the rekey must have run — without this the absences below are vacuous"
        );
        assert_eq!(report.params_ids_repointed, 0);
        assert!(!report.prompt_rewritten);
    }
}
