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
    for pair in &existing {
        if !desired.contains(pair) && prunable_src.contains(pair.0.as_str()) {
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
        let d = diff(&existing, &desired, &prunable);
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
        let d = diff(&existing, &desired, &prunable);
        assert!(d.stale.is_empty());
    }
}
