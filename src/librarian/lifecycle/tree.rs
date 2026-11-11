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
#[derive(Clone, Debug)]
pub struct DocInput {
    pub rel_path: String,
    pub text: String,
}

/// One tracker entry that parsed.
#[derive(Clone, Debug)]
pub struct EntryNode {
    pub rel_path: String,
    pub key: Key,
    /// The entry, or its collection, sits under an `archive/` folder.
    pub archived: bool,
    pub meta: EntryMeta,
    pub relations: Vec<Relation>,
}

/// One collection whose readme gave a valid schema, with the entries found below it.
#[derive(Clone, Debug)]
pub struct CollectionNode {
    pub key: Key,
    pub lifecycle: Lifecycle,
    /// Repo-relative folder of the collection.
    pub dir: String,
    pub archived: bool,
    pub schema: CollectionSchema,
    pub entries: Vec<EntryNode>,
}

/// Something wrong with the tree that is not a lifecycle check on a field.
#[derive(Debug, PartialEq, Eq)]
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
    /// An entry whose frontmatter does not parse as an entry; it is left out.
    Entry {
        rel_path: String,
        errors: Vec<EntryError>,
    },
    /// A tracker file whose frontmatter block does not parse; it is left out.
    Frontmatter { rel_path: String, detail: String },
}

/// The placed, parsed tracker tree.
#[derive(Debug)]
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
        let mut pending: Vec<(&DocInput, Placement, Frontmatter)> = Vec::new();

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
                    if placement.role == Role::CollectionReadme {
                        failed.insert(placement.collection_dir);
                    }
                    continue;
                }
            };
            match placement.role {
                Role::Entry => pending.push((doc, placement, fm)),
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
                if !failed.contains(&placement.collection_dir) {
                    problems.push(TreeProblem::OrphanEntry(doc.rel_path.clone()));
                }
                continue;
            };
            let key = match key_of(&doc.rel_path) {
                Ok(key) => key,
                Err(e) => {
                    problems.push(TreeProblem::Placement(PlacementError::BadKey(e)));
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
                Err(errors) => problems.push(TreeProblem::Entry {
                    rel_path: doc.rel_path.clone(),
                    errors,
                }),
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
    use crate::librarian::lifecycle::testkit::{d, doc, entry, k, LEDGER};

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
        ];
        let mut rev = docs.clone();
        rev.reverse();
        let keys = |t: &LifecycleTree| -> Vec<String> {
            t.collections[0]
                .entries
                .iter()
                .map(|e| e.rel_path.clone())
                .collect()
        };
        assert_eq!(
            keys(&LifecycleTree::build(docs)),
            keys(&LifecycleTree::build(rev))
        );
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
        assert!(placement
            .iter()
            .any(|e| matches!(e, PlacementError::BadKey(_))));
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
        let no_exit = "---\ntitle: S\nentry_kinds: [f]\nstatuses: {open: open, done: terminal}\ninitial_status: open\n---\n";
        let t = LifecycleTree::build([
            doc("docs/trackers/streams/s/readme.md", no_exit),
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
            [TreeProblem::Entry { rel_path, errors }]
                if rel_path == "docs/trackers/ledgers/l/2026-10-01-a.md" && !errors.is_empty()
        ));
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
