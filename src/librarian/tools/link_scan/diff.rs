// src/librarian/tools/link_scan/diff.rs
//! Diff desired (prose-derived) `cites` edges against the catalog, and apply
//! the delta in one transaction. Prune safety rules:
//! - only `rel="cites"` edges are ever touched (scanner-owned by declaration;
//!   manual rels and the side-effectful `supersedes` are invisible here);
//! - only edges whose `src_id` is in `prunable_src` — the artifacts actually
//!   scanned this run whose files were readable — may be pruned. A scoped run
//!   must not delete edges owned by unscanned artifacts, and an unreadable
//!   file must not read as "no citations".

use std::collections::{BTreeSet, HashSet};

use anyhow::Result;

use crate::librarian::catalog::{links, links::LinkRow, Catalog};

/// The scanner-owned rel. Never hand-write it; never scan-write anything else.
pub const CITES_REL: &str = "cites";

/// The `**Rests on:**` relation, entry grain only.
///
/// Coexists with a `cites` edge between the same pair rather than replacing it:
/// `entry_cite`'s primary key is `(src_slug, src_local, dst_ref, rel)`, so `rel`
/// discriminates. That property is what the whole Layer 3c design rests on and it
/// is asserted directly by
/// `a_rests_on_edge_coexists_with_a_cites_edge_between_the_same_pair`.
///
/// **File grain deliberately does not get this rel.** `**Rests on:**` is declared by
/// an entry and is a claim about that entry's proof; projecting it up to the artifact
/// would assert that the whole file rests on the target.
pub const RESTS_ON_REL: &str = "rests-on";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct LinkDiff {
    pub to_add: Vec<(String, String)>,
    pub stale: Vec<(String, String)>,
    pub unchanged: usize,
}

pub fn diff(
    existing_cites: &[LinkRow],
    desired: &BTreeSet<(String, String)>,
    prunable_src: &HashSet<String>,
    resolvable_dst: &BTreeSet<String>,
) -> LinkDiff {
    let existing: BTreeSet<(String, String)> = existing_cites
        .iter()
        .map(|l| (l.src_id.clone(), l.dst_id.clone()))
        .collect();
    let mut d = LinkDiff::default();
    for pair in desired {
        if existing.contains(pair) {
            d.unchanged += 1;
        } else {
            d.to_add.push(pair.clone());
        }
    }
    // BOTH ends bound the prune, because both ends bound `desired`. The source bound keeps a
    // scoped run off edges owned by artifacts it never read. The destination bound is its twin,
    // missing until 2026-09-24: the resolver's corpus is only the scanned rows, so a citation
    // whose destination fell outside a `limit`ed or narrow scan resolves to nothing and is
    // absent from `desired` because it was never LOOKED AT — not because it went away.
    // Measured: `link_scan(limit=10)` reported 53 edges stale against 2 for the full scan.
    // Failing this way leaves a genuinely dead edge to an unscanned destination in place until
    // a scan wide enough to see both ends — an extra edge rather than a lost one.
    // docs/issues/archive/2026-09-21-a-narrowed-link-scan-prunes-edges-whose-destination-it-never-looked-at.md
    for pair in &existing {
        if !desired.contains(pair)
            && prunable_src.contains(pair.0.as_str())
            && resolvable_dst.contains(pair.1.as_str())
        {
            d.stale.push(pair.clone());
        }
    }
    d
}

/// Apply the delta atomically: insert `to_add` (INSERT OR IGNORE — idempotent
/// on the (src, dst, rel) PK) and delete `stale`, all as `rel="cites"`.
pub fn apply(cat: &Catalog, d: &LinkDiff) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    let tx = cat.conn.unchecked_transaction()?;
    for (src, dst) in &d.to_add {
        links::insert_with(
            &tx,
            &LinkRow {
                src_id: src.clone(),
                dst_id: dst.clone(),
                rel: CITES_REL.into(),
                created_at: now,
            },
        )?;
    }
    for (src, dst) in &d.stale {
        links::delete_with(&tx, src, dst, CITES_REL)?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(src: &str, dst: &str) -> LinkRow {
        LinkRow {
            src_id: src.into(),
            dst_id: dst.into(),
            rel: CITES_REL.into(),
            created_at: 0,
        }
    }

    #[test]
    fn diff_partitions_add_stale_unchanged() {
        let existing = vec![row("a", "x"), row("a", "y"), row("b", "z")];
        let desired: BTreeSet<_> = [
            ("a".to_string(), "x".to_string()), // unchanged
            ("c".to_string(), "w".to_string()), // to_add
        ]
        .into();
        let prunable: HashSet<String> = ["a".to_string()].into();
        // Every destination resolvable, so this case isolates the SOURCE bound.
        let resolvable: BTreeSet<String> = ["x", "y", "z", "w"].map(String::from).into();
        let d = diff(&existing, &desired, &prunable, &resolvable);
        assert_eq!(d.unchanged, 1);
        assert_eq!(d.to_add, vec![("c".into(), "w".into())]);
        // a→y stale (src scanned); b→z NOT stale (src outside scanned set).
        assert_eq!(d.stale, vec![("a".into(), "y".into())]);
    }

    #[test]
    fn unreadable_src_is_never_pruned() {
        let existing = vec![row("unreadable-doc", "x")];
        let desired = BTreeSet::new();
        let prunable = HashSet::new(); // unreadable → excluded from prunable
        let resolvable: BTreeSet<String> = ["x".to_string()].into();
        let d = diff(&existing, &desired, &prunable, &resolvable);
        assert!(d.stale.is_empty());
    }

    /// The DESTINATION bound. `desired` can only hold a pair whose destination the resolver
    /// could resolve to — its corpus, which a `limit`ed or narrowly-scoped scan truncates — so
    /// an existing edge to an unscanned destination is absent from `desired` because it was
    /// never LOOKED AT, not because its citation went away. Measured 2026-09-24:
    /// `link_scan(limit=10)` reported 53 such edges stale against 2 for the full scan, and
    /// `write=true` would have deleted them.
    /// docs/issues/archive/2026-09-21-a-narrowed-link-scan-prunes-edges-whose-destination-it-never-looked-at.md
    ///
    /// Load-bearing: `a→x` has BOTH ends scanned and must still be pruned — without it this is
    /// monotone under "prune nothing", which would pass by disabling the prune entirely.
    #[test]
    fn an_edge_to_an_unscanned_destination_is_never_pruned() {
        let existing = vec![row("a", "x"), row("a", "outside")];
        let desired = BTreeSet::new();
        let prunable: HashSet<String> = ["a".to_string()].into();
        let resolvable: BTreeSet<String> = ["a", "x"].map(String::from).into();
        let d = diff(&existing, &desired, &prunable, &resolvable);
        assert_eq!(
            d.stale,
            vec![("a".into(), "x".into())],
            "a→x (both ends scanned) is genuinely stale; a→outside was never resolvable here"
        );
    }
}
