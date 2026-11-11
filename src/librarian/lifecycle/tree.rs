//! The lifecycle tree: every tracker document under `docs/trackers/<lifecycle>/` placed in
//! its collection, parsed against that collection's schema, with the problems found on the way.
//!
//! Pure: `build` reads only the documents handed to it. `DocInput` carries a path and a text,
//! no date, so no later check can read a git commit date (anti-goal 2).

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;

use super::entry::{parse_date, parse_entry, EntryError, EntryMeta};
use super::layout::{has_nested_readme, place, Lifecycle, Placement, PlacementError, Role};
use super::relations::{relations, Relation};
use super::schema::{parse_schema, CollectionSchema, SchemaError};
use crate::librarian::frontmatter::{self, Frontmatter};
use crate::librarian::identity::key::{is_folder_index, key_of, Key};

/// One document handed to [`LifecycleTree::build`]: its repo-relative path and its text.
/// The identity index's input type, shared so both models read one document set.
pub use crate::librarian::identity::index::DocInput;

/// One tracker entry that parsed.
#[derive(Clone, Debug, PartialEq)]
pub struct EntryNode {
    pub rel_path: String,
    pub key: Key,
    /// The entry, or its collection, sits under an `archive/` folder.
    pub archived: bool,
    pub meta: EntryMeta,
    pub relations: Vec<Relation>,
}

/// One collection whose readme gave a valid schema, with the entries found below it.
#[derive(Clone, Debug, PartialEq)]
pub struct CollectionNode {
    pub key: Key,
    pub lifecycle: Lifecycle,
    /// Repo-relative folder of the collection.
    pub dir: String,
    pub archived: bool,
    pub schema: CollectionSchema,
    pub entries: Vec<EntryNode>,
    /// Repo-relative paths of the entry files placed in this collection (archived part
    /// included) that did not become an `entries` node, in path order: frontmatter that fails
    /// `parse_entry`, a frontmatter block that does not parse, or a file name that is no key.
    /// A check that reasons about the whole stream must know they exist.
    pub unparsed: Vec<String>,
}

/// Something wrong with the tree that is not a lifecycle check on a field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeProblem {
    /// A file below a lifecycle folder has no valid place (or an entry file name is no key).
    Placement(PlacementError),
    /// A file sits at or below a subfolder that holds its own `readme.md`.
    NestedCollection(String),
    /// An entry with no collection readme above it.
    OrphanEntry(String),
    /// A collection readme whose schema fails; the collection and its entries are left out.
    Schema {
        collection: Key,
        errors: Vec<SchemaError>,
    },
    /// An entry whose frontmatter does not parse as an entry; it is left out of `entries`
    /// and its path is listed in its collection's `unparsed`.
    Entry {
        rel_path: String,
        key: Key,
        errors: Vec<EntryError>,
    },
    /// A tracker file whose frontmatter block does not parse; it is left out.
    Frontmatter { rel_path: String, detail: String },
}

/// The placed, parsed tracker tree.
#[derive(Clone, Debug, PartialEq)]
pub struct LifecycleTree {
    pub collections: Vec<CollectionNode>,
    pub problems: Vec<TreeProblem>,
    /// The frontmatter `closed:` date of every input document that has one, keyed by the
    /// document's key. When two documents share a key, the later date is kept.
    pub closed_dates: BTreeMap<Key, NaiveDate>,
}

/// The folder of `rel_path`, `/`-separated, read the way `layout::place` reads a path
/// (`\` is `/`; empty and `.` components are dropped).
fn parent_dir(rel_path: &str) -> String {
    let segs: Vec<&str> = rel_path
        .split(['/', '\\'])
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();
    segs[..segs.len().saturating_sub(1)].join("/")
}

/// The `closed:` date of a document, when it declares a real ISO date.
fn closed_of(fm: &Frontmatter) -> Option<NaiveDate> {
    fm.extra
        .get("closed")
        .and_then(|v| v.as_str())
        .and_then(parse_date)
}

impl LifecycleTree {
    /// Place and parse `docs`. The result does not depend on the order of `docs`.
    pub fn build(docs: impl IntoIterator<Item = DocInput>) -> LifecycleTree {
        let mut docs: Vec<DocInput> = docs.into_iter().collect();
        docs.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));

        // `place` cannot see siblings: the folders that hold a readme are found here, over the
        // whole input, so a file below a nested readme can be refused (`has_nested_readme`).
        let readme_dirs: BTreeSet<String> = docs
            .iter()
            .filter(|d| place(&d.rel_path).is_some() && is_folder_index(&d.rel_path))
            .map(|d| parent_dir(&d.rel_path))
            .collect();

        let mut problems = Vec::new();
        let mut closed_dates: BTreeMap<Key, NaiveDate> = BTreeMap::new();
        let mut collections: BTreeMap<String, CollectionNode> = BTreeMap::new();
        // Collections whose readme failed: their entries are skipped, not orphaned.
        let mut failed: BTreeSet<String> = BTreeSet::new();
        // An entry file is queued with its frontmatter, or with `None` when the frontmatter
        // block did not parse: the second pass still has to count it against its collection.
        let mut pending: Vec<(&DocInput, Placement, Option<Frontmatter>)> = Vec::new();

        for doc in &docs {
            let parsed = frontmatter::parse(&doc.text);
            if let Ok((Some(fm), _)) = &parsed {
                if let (Some(closed), Ok(key)) = (closed_of(fm), key_of(&doc.rel_path)) {
                    let slot = closed_dates.entry(key).or_insert(closed);
                    *slot = (*slot).max(closed);
                }
            }
            let placement = match place(&doc.rel_path) {
                None => continue,
                Some(Err(e)) => {
                    problems.push(TreeProblem::Placement(e));
                    continue;
                }
                Some(Ok(p)) => p,
            };
            if placement.role == Role::Entry && has_nested_readme(&doc.rel_path, &readme_dirs) {
                problems.push(TreeProblem::NestedCollection(doc.rel_path.clone()));
                continue;
            }
            let fm = match parsed {
                Ok((fm, _)) => fm.unwrap_or_default(),
                Err(e) => {
                    problems.push(TreeProblem::Frontmatter {
                        rel_path: doc.rel_path.clone(),
                        detail: e.to_string(),
                    });
                    match placement.role {
                        Role::CollectionReadme => {
                            failed.insert(placement.collection_dir);
                        }
                        Role::Entry => pending.push((doc, placement, None)),
                    }
                    continue;
                }
            };
            match placement.role {
                Role::Entry => pending.push((doc, placement, Some(fm))),
                Role::CollectionReadme => match parse_schema(placement.lifecycle, &fm) {
                    Ok(schema) => {
                        collections
                            .entry(placement.collection_dir.clone())
                            .or_insert(CollectionNode {
                                key: placement.collection,
                                lifecycle: placement.lifecycle,
                                dir: placement.collection_dir,
                                archived: placement.archived,
                                schema,
                                entries: Vec::new(),
                                unparsed: Vec::new(),
                            });
                    }
                    Err(errors) => {
                        problems.push(TreeProblem::Schema {
                            collection: placement.collection,
                            errors,
                        });
                        failed.insert(placement.collection_dir);
                    }
                },
            }
        }

        for (doc, placement, fm) in pending {
            let Some(collection) = collections.get_mut(&placement.collection_dir) else {
                // A file with broken frontmatter already has its `Frontmatter` problem: no
                // orphan on top of it.
                if fm.is_some() && !failed.contains(&placement.collection_dir) {
                    problems.push(TreeProblem::OrphanEntry(doc.rel_path.clone()));
                }
                continue;
            };
            // Whatever keeps a file from becoming an `EntryNode` lists it in `unparsed`, so no
            // collection-level check judges the stream without it.
            let Some(fm) = fm else {
                collection.unparsed.push(doc.rel_path.clone());
                continue;
            };
            let key = match key_of(&doc.rel_path) {
                Ok(key) => key,
                Err(e) => {
                    collection.unparsed.push(doc.rel_path.clone());
                    problems.push(TreeProblem::Placement(PlacementError::BadKey {
                        rel_path: doc.rel_path.clone(),
                        error: e,
                    }));
                    continue;
                }
            };
            match parse_entry(&fm) {
                Ok(meta) => collection.entries.push(EntryNode {
                    rel_path: doc.rel_path.clone(),
                    key,
                    archived: placement.archived,
                    meta,
                    relations: relations(&doc.text),
                }),
                Err(errors) => {
                    collection.unparsed.push(doc.rel_path.clone());
                    problems.push(TreeProblem::Entry {
                        rel_path: doc.rel_path.clone(),
                        key,
                        errors,
                    });
                }
            }
        }

        LifecycleTree {
            collections: collections.into_values().collect(),
            problems,
            closed_dates,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::lifecycle::testkit::{d, doc, entry, k, LEDGER, STREAM, STREAM_NO_EXIT};

    const BUG: &str = "---\nkind: bug\nstatus: fixed\nclosed: 2026-10-06\n---\n";

    #[test]
    fn a_collection_gets_its_entries_archived_flags_relations_and_the_closed_dates() {
        let t = LifecycleTree::build([
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-a.md",
                &entry("open", "", "**Rests on:** [[2026-10-05-the-bug]]\n"),
            ),
            doc(
                "docs/trackers/ledgers/l/archive/2026-10-01-b.md",
                &entry("fixed", "closed: 2026-10-02\n", ""),
            ),
            doc(
                "docs/trackers/ledgers/l/reports/2026-10-01-c.md",
                &entry("open", "", ""),
            ),
            doc("docs/issues/2026-10-05-the-bug.md", BUG),
        ]);
        assert!(t.problems.is_empty(), "{:?}", t.problems);
        assert_eq!(t.collections.len(), 1);
        let c = &t.collections[0];
        assert_eq!(
            (c.key.as_str(), c.lifecycle, c.archived),
            ("l", Lifecycle::Ledger, false)
        );
        assert_eq!(c.dir, "docs/trackers/ledgers/l");
        let got: Vec<(&str, bool)> = c
            .entries
            .iter()
            .map(|e| (e.key.as_str(), e.archived))
            .collect();
        assert_eq!(
            got,
            [
                ("2026-10-01-a", false),
                ("2026-10-01-b", true),
                ("2026-10-01-c", false)
            ]
        );
        let a = &c.entries[0];
        assert_eq!(a.relations.len(), 1);
        assert_eq!(a.relations[0].label, "Rests on");
        // The closed dates of the archived entry and of the bug file, both keyed by `key_of`.
        assert_eq!(
            t.closed_dates.get(&k("2026-10-01-b")),
            Some(&d("2026-10-02"))
        );
        assert_eq!(
            t.closed_dates.get(&k("2026-10-05-the-bug")),
            Some(&d("2026-10-06"))
        );
        assert_eq!(t.closed_dates.len(), 2);
    }

    #[test]
    fn the_result_does_not_depend_on_input_order() {
        let docs = vec![
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-a.md",
                &entry("open", "", ""),
            ),
            doc(
                "docs/trackers/ledgers/l/2026-10-02-b.md",
                &entry("open", "", ""),
            ),
            doc("docs/trackers/streams/s/readme.md", STREAM),
            doc(
                "docs/trackers/streams/s/2026-10-01-c.md",
                &entry("open", "", ""),
            ),
            doc(
                "docs/trackers/streams/s/2026-10-02-d.md",
                &entry("done", "closed: 2026-10-03\n", ""),
            ),
            // Two problems (two orphans) and one closed date, so no part of the comparison is empty.
            doc(
                "docs/trackers/temp/lost-one/2026-10-01-e.md",
                &entry("open", "", ""),
            ),
            doc(
                "docs/trackers/temp/lost-two/2026-10-01-f.md",
                &entry("open", "", ""),
            ),
            doc("docs/issues/2026-10-05-the-bug.md", BUG),
        ];
        let mut rev = docs.clone();
        rev.reverse();
        let forward = LifecycleTree::build(docs);
        let backward = LifecycleTree::build(rev);
        assert_eq!(forward.collections.len(), 2);
        assert_eq!(forward.problems.len(), 2);
        assert_eq!(forward.closed_dates.len(), 2);
        // `LifecycleTree` is `PartialEq`: a whole-tree comparison, not a rendering of it.
        assert_eq!(forward, backward);
    }

    #[test]
    fn an_entry_with_no_readme_above_it_is_an_orphan_and_misplaced_files_are_placement_problems() {
        let t = LifecycleTree::build([
            doc(
                "docs/trackers/streams/no-readme/2026-10-01-lost.md",
                &entry("open", "", ""),
            ),
            doc("docs/trackers/ledgers/readme.md", LEDGER),
            doc("docs/trackers/ledgers/loose.md", &entry("open", "", "")),
            doc("docs/trackers/ledgers/Bad_Name/readme.md", LEDGER),
        ]);
        assert!(t.collections.is_empty());
        assert!(t.problems.contains(&TreeProblem::OrphanEntry(
            "docs/trackers/streams/no-readme/2026-10-01-lost.md".into()
        )));
        let placement: Vec<&PlacementError> = t
            .problems
            .iter()
            .filter_map(|p| match p {
                TreeProblem::Placement(e) => Some(e),
                _ => None,
            })
            .collect();
        assert_eq!(placement.len(), 3, "{:?}", t.problems);
        assert!(placement
            .iter()
            .any(|e| matches!(e, PlacementError::OutsideCollection(p) if p == "docs/trackers/ledgers/readme.md")));
        assert!(placement
            .iter()
            .any(|e| matches!(e, PlacementError::OutsideCollection(p) if p == "docs/trackers/ledgers/loose.md")));
        assert!(placement.iter().any(|e| matches!(
            e,
            PlacementError::BadKey { rel_path, .. }
                if rel_path == "docs/trackers/ledgers/Bad_Name/readme.md"
        )));
    }

    #[test]
    fn a_subfolder_with_its_own_readme_is_a_nested_collection_and_its_files_are_skipped() {
        let t = LifecycleTree::build([
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-a.md",
                &entry("open", "", ""),
            ),
            doc("docs/trackers/ledgers/l/reports/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/l/reports/2026-10-01-r.md",
                &entry("open", "", ""),
            ),
            doc(
                "docs/trackers/ledgers/l/reports/deep/2026-10-01-s.md",
                &entry("open", "", ""),
            ),
        ]);
        let mut nested: Vec<&str> = t
            .problems
            .iter()
            .filter_map(|p| match p {
                TreeProblem::NestedCollection(path) => Some(path.as_str()),
                _ => None,
            })
            .collect();
        nested.sort_unstable();
        assert_eq!(
            nested,
            [
                "docs/trackers/ledgers/l/reports/2026-10-01-r.md",
                "docs/trackers/ledgers/l/reports/deep/2026-10-01-s.md",
                "docs/trackers/ledgers/l/reports/readme.md",
            ]
        );
        assert_eq!(t.problems.len(), 3, "{:?}", t.problems);
        // The enclosing collection keeps only its own entry.
        assert_eq!(t.collections.len(), 1);
        assert_eq!(t.collections[0].entries.len(), 1);
        assert_eq!(t.collections[0].entries[0].key.as_str(), "2026-10-01-a");
    }

    #[test]
    fn a_failing_schema_is_reported_once_and_its_entries_are_skipped_silently() {
        let t = LifecycleTree::build([
            doc("docs/trackers/streams/s/readme.md", STREAM_NO_EXIT),
            doc(
                "docs/trackers/streams/s/2026-10-01-a.md",
                &entry("open", "", ""),
            ),
            doc(
                "docs/trackers/streams/s/2026-10-02-b.md",
                &entry("open", "", ""),
            ),
        ]);
        assert!(t.collections.is_empty());
        assert_eq!(
            t.problems,
            [TreeProblem::Schema {
                collection: k("s"),
                errors: vec![SchemaError::ExitRequired]
            }]
        );
    }

    #[test]
    fn a_bad_entry_is_reported_and_left_out() {
        let t = LifecycleTree::build([
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-a.md",
                "---\nentry_kind: f\n---\n",
            ),
        ]);
        assert!(t.collections[0].entries.is_empty());
        assert!(matches!(
            t.problems.as_slice(),
            [TreeProblem::Entry { rel_path, key, errors }]
                if rel_path == "docs/trackers/ledgers/l/2026-10-01-a.md"
                    && key.as_str() == "2026-10-01-a"
                    && !errors.is_empty()
        ));
    }

    #[test]
    fn a_bad_key_names_the_file_for_a_collection_folder_and_for_an_entry_stem() {
        let t = LifecycleTree::build([
            doc("docs/trackers/ledgers/Bad_Name/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/Bad_Name/2026-10-01-a.md",
                &entry("open", "", ""),
            ),
            doc("docs/trackers/ledgers/c/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/c/Bad_Stem.md",
                &entry("open", "", ""),
            ),
        ]);
        let mut got: Vec<&str> = t
            .problems
            .iter()
            .filter_map(|p| match p {
                TreeProblem::Placement(PlacementError::BadKey { rel_path, error }) => {
                    // The error carries the offending NAME; the problem carries the FILE.
                    assert!(
                        rel_path.ends_with(".md") && rel_path.starts_with("docs/trackers/"),
                        "{rel_path} {error:?}"
                    );
                    Some(rel_path.as_str())
                }
                _ => None,
            })
            .collect();
        got.sort_unstable();
        assert_eq!(
            got,
            [
                "docs/trackers/ledgers/Bad_Name/2026-10-01-a.md",
                "docs/trackers/ledgers/Bad_Name/readme.md",
                "docs/trackers/ledgers/c/Bad_Stem.md",
            ]
        );
    }

    #[test]
    fn an_unparsed_entry_is_listed_in_its_collection_archived_part_included() {
        let t = LifecycleTree::build([
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/l/archive/2026-10-01-z.md",
                "---\nentry_kind: f\n---\n",
            ),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-a.md",
                "---\nentry_kind: f\n---\n",
            ),
            doc(
                "docs/trackers/ledgers/l/2026-10-02-ok.md",
                &entry("open", "", ""),
            ),
        ]);
        assert_eq!(
            t.collections[0].unparsed,
            [
                "docs/trackers/ledgers/l/2026-10-01-a.md",
                "docs/trackers/ledgers/l/archive/2026-10-01-z.md"
            ]
        );
        assert_eq!(t.collections[0].entries.len(), 1);
    }

    #[test]
    fn a_tracker_file_with_broken_frontmatter_is_a_problem_and_a_file_outside_is_silent() {
        let t = LifecycleTree::build([
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-a.md",
                "---\nentry_kind: [unclosed\n---\n",
            ),
            doc(
                "docs/issues/2026-10-05-broken.md",
                "---\nkind: bug\nclosed: 2026-10-06\n",
            ),
        ]);
        assert!(t.collections[0].entries.is_empty());
        assert!(matches!(
            t.problems.as_slice(),
            [TreeProblem::Frontmatter { rel_path, .. }]
                if rel_path == "docs/trackers/ledgers/l/2026-10-01-a.md"
        ));
        assert!(t.closed_dates.is_empty());
    }

    #[test]
    fn an_entry_file_the_tree_cannot_read_is_listed_unparsed_and_keeps_its_problem_once() {
        let t = LifecycleTree::build([
            doc("docs/trackers/streams/s/readme.md", STREAM),
            doc(
                "docs/trackers/streams/s/2026-10-01-done.md",
                &entry("done", "closed: 2026-10-03\n", ""),
            ),
            doc(
                "docs/trackers/streams/s/Bad_Stem.md",
                &entry("open", "", ""),
            ),
            doc(
                "docs/trackers/streams/s/2026-10-02-broken.md",
                "---\nentry_kind: [unclosed\n---\n",
            ),
        ]);
        assert_eq!(
            t.collections[0].unparsed,
            [
                "docs/trackers/streams/s/2026-10-02-broken.md",
                "docs/trackers/streams/s/Bad_Stem.md"
            ]
        );
        assert_eq!(t.collections[0].entries.len(), 1);
        // Each file keeps its one problem and gains no second one (no orphan, no entry error).
        assert_eq!(t.problems.len(), 2, "{:?}", t.problems);
        assert!(t.problems.iter().any(|p| matches!(
            p,
            TreeProblem::Placement(PlacementError::BadKey { rel_path, .. })
                if rel_path == "docs/trackers/streams/s/Bad_Stem.md"
        )));
        assert!(t.problems.iter().any(|p| matches!(
            p,
            TreeProblem::Frontmatter { rel_path, .. }
                if rel_path == "docs/trackers/streams/s/2026-10-02-broken.md"
        )));
    }

    #[test]
    fn a_broken_entry_in_a_folder_with_no_readme_is_still_only_a_frontmatter_problem() {
        let t = LifecycleTree::build([doc(
            "docs/trackers/streams/no-readme/2026-10-02-broken.md",
            "---\nentry_kind: [unclosed\n---\n",
        )]);
        assert!(t.collections.is_empty());
        assert!(matches!(
            t.problems.as_slice(),
            [TreeProblem::Frontmatter { .. }]
        ));
    }

    #[test]
    fn a_broken_readme_frontmatter_is_one_problem_and_does_not_orphan_its_entries() {
        let t = LifecycleTree::build([
            doc(
                "docs/trackers/ledgers/l/readme.md",
                "---\ntitle: [oops\n---\n",
            ),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-a.md",
                &entry("open", "", ""),
            ),
        ]);
        assert!(t.collections.is_empty());
        assert!(matches!(
            t.problems.as_slice(),
            [TreeProblem::Frontmatter { .. }]
        ));
    }

    #[test]
    fn closed_dates_skip_unparseable_values_and_keys_and_keep_the_later_of_two() {
        let t = LifecycleTree::build([
            doc(
                "docs/a/2026-01-01-impossible.md",
                "---\nclosed: 2026-02-30\n---\n",
            ),
            doc("docs/a/2026-01-01-prose.md", "---\nclosed: soon\n---\n"),
            doc("docs/a/Bad_Key.md", "---\nclosed: 2026-01-01\n---\n"),
            doc(
                "docs/a/2026-01-01-quoted.md",
                "---\nclosed: \"2026-03-04\"\n---\n",
            ),
            doc("docs/a/x/dup.md", "---\nclosed: 2026-03-04\n---\n"),
            doc("docs/b/dup.md", "---\nclosed: 2026-05-06\n---\n"),
            doc("docs/b/no-frontmatter.md", "# just a body\n"),
        ]);
        let got: Vec<(&str, NaiveDate)> = t
            .closed_dates
            .iter()
            .map(|(key, date)| (key.as_str(), *date))
            .collect();
        assert_eq!(
            got,
            [
                ("2026-01-01-quoted", d("2026-03-04")),
                ("dup", d("2026-05-06"))
            ]
        );
    }
}
