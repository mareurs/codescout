//! The eight field-based lifecycle checks over a [`LifecycleTree`].
//!
//! Pure: `check` reads the tree, `today` and the policy, nothing else. No git, no clock, no
//! filesystem, so no check can read a commit date (anti-goal 2).
//!
//! Which entries each check looks at: `EntryInvalid` (spec `entry_invalid`, one finding per
//! problem `validate_entry` returns, which generalises the former `status_undeclared`; an entry
//! that does not parse gives one finding with every parse error) looks
//! at every entry, archived or not (data integrity). `RevisitDue`, `CitedTargetClosed` and
//! `TerminalInLiveDir` look at
//! un-archived entries only: an archived entry is done and does not nag. `StreamAllTerminal`
//! and `LedgerQuiet` look at un-archived collections only; `LedgerQuiet` counts every entry of
//! such a ledger, archived ones too, as activity.

use std::collections::BTreeSet;

use chrono::{Days, NaiveDate};

use super::entry::{class_of, validate_entry, EntryProblem};
use super::layout::{Lifecycle, PlacementError};
use super::relations::{is_backward, Relation};
use super::schema::{SchemaError, StatusClass};
use super::tree::{CollectionNode, EntryNode, LifecycleTree, TreeProblem};
use crate::librarian::identity::grammar::Reference;
use crate::librarian::identity::key::Key;

/// Thresholds the checks cannot read from a document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckPolicy {
    /// Days a terminal entry may stay outside `archive/` after its `closed` date.
    pub terminal_grace_days: u32,
}

impl Default for CheckPolicy {
    fn default() -> Self {
        CheckPolicy {
            terminal_grace_days: 14,
        }
    }
}

/// One thing `doctor` reports. The variants are listed in the order `check` returns them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// An entry breaks its collection's schema: one finding per problem `validate_entry`
    /// returns, or one `EntryProblem::Unparsed` carrying every error of an entry whose
    /// frontmatter does not parse at all.
    EntryInvalid { entry: Key, problem: EntryProblem },
    /// A blocked or deferred entry's `revisit_by` has passed.
    RevisitDue { entry: Key, revisit_by: NaiveDate },
    /// A live entry cites a target closed after the entry's `status_since`.
    CitedTargetClosed {
        entry: Key,
        target: Key,
        closed: NaiveDate,
    },
    /// Every entry of a live stream or temp collection is terminal, and none failed to parse.
    StreamAllTerminal { collection: Key },
    /// A stream or temp collection declares no `exit_when`.
    StreamWithoutExit { collection: Key },
    /// A ledger with `review_every` has had no entry created or closed in that period.
    LedgerQuiet {
        collection: Key,
        last_activity: Option<NaiveDate>,
    },
    /// A terminal entry stayed outside `archive/` past the grace period.
    TerminalInLiveDir { entry: Key, closed: NaiveDate },
    /// An entry file with no collection readme above it: an entry in a folder with no readme,
    /// or a file directly in a lifecycle folder.
    OrphanEntry { rel_path: String },
}

impl Finding {
    /// Variant position, then the identifying keys: the sort order of `check`'s output.
    fn sort_key(&self) -> (u8, &str, String) {
        match self {
            // The third element is empty: one entry's problems keep the order `validate_entry`
            // returned them in (a fixed order), because `check` sorts stably.
            Finding::EntryInvalid { entry, .. } => (0, entry.as_str(), String::new()),
            Finding::RevisitDue { entry, revisit_by } => {
                (1, entry.as_str(), revisit_by.to_string())
            }
            Finding::CitedTargetClosed { entry, target, .. } => {
                (2, entry.as_str(), target.as_str().to_string())
            }
            Finding::StreamAllTerminal { collection } => (3, collection.as_str(), String::new()),
            Finding::StreamWithoutExit { collection } => (4, collection.as_str(), String::new()),
            Finding::LedgerQuiet { collection, .. } => (5, collection.as_str(), String::new()),
            Finding::TerminalInLiveDir { entry, closed } => (6, entry.as_str(), closed.to_string()),
            Finding::OrphanEntry { rel_path } => (7, rel_path.as_str(), String::new()),
        }
    }
}

/// The key a relation cites in this repo: a `File` or `Entry` citation with no repo qualifier.
/// A `repo/key` citation names a document of another repo, which this tree cannot date.
fn local_target(relation: &Relation) -> Option<&Key> {
    match &relation.reference {
        Reference::File { repo: None, key }
        | Reference::Entry {
            repo: None, key, ..
        } => Some(key),
        _ => None,
    }
}

fn check_entry(
    entry: &EntryNode,
    collection: &CollectionNode,
    tree: &LifecycleTree,
    today: NaiveDate,
    policy: &CheckPolicy,
    out: &mut Vec<Finding>,
) {
    let meta = &entry.meta;
    for problem in validate_entry(&collection.schema, meta) {
        out.push(Finding::EntryInvalid {
            entry: entry.key.clone(),
            problem,
        });
    }
    if entry.archived {
        return;
    }
    let Some(class) = class_of(&collection.schema, meta) else {
        return;
    };
    if class.needs_revisit() {
        if let Some(revisit_by) = meta.revisit_by.filter(|r| *r < today) {
            out.push(Finding::RevisitDue {
                entry: entry.key.clone(),
                revisit_by,
            });
        }
    }
    if class != StatusClass::Terminal {
        let mut seen: BTreeSet<&Key> = BTreeSet::new();
        let forward = entry.relations.iter().filter(|r| !is_backward(&r.label));
        for target in forward.filter_map(local_target) {
            if let Some(closed) = tree.closed_dates.get(target).copied() {
                if closed > meta.status_since && seen.insert(target) {
                    out.push(Finding::CitedTargetClosed {
                        entry: entry.key.clone(),
                        target: target.clone(),
                        closed,
                    });
                }
            }
        }
    } else if let Some(closed) = meta.closed {
        let grace = Days::new(u64::from(policy.terminal_grace_days));
        if closed
            .checked_add_days(grace)
            .is_some_and(|due| due < today)
        {
            out.push(Finding::TerminalInLiveDir {
                entry: entry.key.clone(),
                closed,
            });
        }
    }
}

fn check_collection(collection: &CollectionNode, today: NaiveDate, out: &mut Vec<Finding>) {
    if collection.archived {
        return;
    }
    match collection.lifecycle {
        Lifecycle::Stream | Lifecycle::Temp => {
            let all_terminal = !collection.entries.is_empty()
                && collection.unparsed.is_empty()
                && collection
                    .entries
                    .iter()
                    .all(|e| class_of(&collection.schema, &e.meta) == Some(StatusClass::Terminal));
            if all_terminal {
                out.push(Finding::StreamAllTerminal {
                    collection: collection.key.clone(),
                });
            }
        }
        Lifecycle::Ledger => {
            let Some(days) = collection.schema.review_every_days else {
                return;
            };
            let last_activity = collection
                .entries
                .iter()
                .flat_map(|e| [Some(e.meta.created), e.meta.closed])
                .flatten()
                .max();
            let cutoff = today.checked_sub_days(Days::new(u64::from(days)));
            let quiet = match (last_activity, cutoff) {
                (None, _) => true,
                (Some(last), Some(cutoff)) => last < cutoff,
                (Some(_), None) => false,
            };
            if quiet {
                out.push(Finding::LedgerQuiet {
                    collection: collection.key.clone(),
                    last_activity,
                });
            }
        }
    }
}

/// Run the eight lifecycle checks. The result is sorted by variant order, then key, so it is
/// the same for the same tree.
pub fn check(tree: &LifecycleTree, today: NaiveDate, policy: &CheckPolicy) -> Vec<Finding> {
    let mut out = Vec::new();
    for collection in &tree.collections {
        for entry in &collection.entries {
            check_entry(entry, collection, tree, today, policy, &mut out);
        }
        check_collection(collection, today, &mut out);
    }
    let mut orphans: BTreeSet<&str> = BTreeSet::new();
    for problem in &tree.problems {
        match problem {
            // A file under a lifecycle folder with no collection readme above it arrives in
            // one of two ways: an entry whose collection folder has no readme, or a file that
            // sits directly in the lifecycle folder (`place` refuses it). One path, one finding.
            TreeProblem::OrphanEntry(rel_path)
            | TreeProblem::Placement(PlacementError::OutsideCollection(rel_path)) => {
                if orphans.insert(rel_path.as_str()) {
                    out.push(Finding::OrphanEntry {
                        rel_path: rel_path.clone(),
                    });
                }
            }
            TreeProblem::Entry { key, errors, .. } => out.push(Finding::EntryInvalid {
                entry: key.clone(),
                problem: EntryProblem::Unparsed(errors.clone()),
            }),
            TreeProblem::Schema { collection, errors }
                if errors.contains(&SchemaError::ExitRequired) =>
            {
                out.push(Finding::StreamWithoutExit {
                    collection: collection.clone(),
                })
            }
            _ => {}
        }
    }
    out.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::lifecycle::entry::EntryError;
    use crate::librarian::lifecycle::schema::FieldType;
    use crate::librarian::lifecycle::testkit::{d, doc, entry, k, LEDGER, STREAM};
    use crate::librarian::lifecycle::tree::{DocInput, LifecycleTree};

    const BUG: &str = "---\nkind: bug\nstatus: fixed\nclosed: 2026-10-06\n---\n";

    fn tree() -> LifecycleTree {
        LifecycleTree::build([
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-cites-a-fixed-bug.md",
                &entry("open", "", "**Rests on:** [[2026-10-05-the-bug]]\n"),
            ),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-waits.md",
                &entry("waiting", "revisit_by: 2026-11-01\n", ""),
            ),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-fixed-long-ago.md",
                &entry("fixed", "closed: 2026-10-02\n", ""),
            ),
            doc(
                "docs/trackers/ledgers/l/archive/2026-10-01-archived-fixed.md",
                &entry("fixed", "closed: 2026-10-02\n", ""),
            ),
            doc("docs/issues/2026-10-05-the-bug.md", BUG),
            doc("docs/trackers/streams/s/readme.md", STREAM),
            doc(
                "docs/trackers/streams/s/2026-10-01-done.md",
                &entry("done", "closed: 2026-11-10\n", ""),
            ),
            doc(
                "docs/trackers/streams/no-readme/2026-10-01-lost.md",
                &entry("open", "", ""),
            ),
        ])
    }

    fn today() -> chrono::NaiveDate {
        d("2026-11-11")
    }

    fn run(docs: Vec<DocInput>) -> Vec<Finding> {
        check(
            &LifecycleTree::build(docs),
            today(),
            &CheckPolicy::default(),
        )
    }

    #[test]
    fn each_check_fires_on_its_case() {
        let f = check(&tree(), d("2026-11-11"), &CheckPolicy::default());
        assert!(f.contains(&Finding::CitedTargetClosed {
            entry: k("2026-10-01-cites-a-fixed-bug"),
            target: k("2026-10-05-the-bug"),
            closed: d("2026-10-06")
        }));
        assert!(f.contains(&Finding::RevisitDue {
            entry: k("2026-10-01-waits"),
            revisit_by: d("2026-11-01")
        }));
        assert!(f.contains(&Finding::TerminalInLiveDir {
            entry: k("2026-10-01-fixed-long-ago"),
            closed: d("2026-10-02")
        }));
        assert!(!f.iter().any(|x| matches!(x, Finding::TerminalInLiveDir { entry, .. } if entry.as_str() == "2026-10-01-archived-fixed")));
        assert!(f.contains(&Finding::StreamAllTerminal { collection: k("s") }));
        assert!(f.contains(&Finding::OrphanEntry {
            rel_path: "docs/trackers/streams/no-readme/2026-10-01-lost.md".into()
        }));
        assert!(f.contains(&Finding::LedgerQuiet {
            collection: k("l"),
            last_activity: Some(d("2026-10-02"))
        }));
    }

    #[test]
    fn reconfirming_an_open_entry_silences_cited_target_closed() {
        let mut docs: Vec<DocInput> = vec![
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-e.md",
                &entry("open", "", "**Rests on:** [[2026-10-05-the-bug]]\n")
                    .replace("status_since: 2026-10-01", "status_since: 2026-10-07"),
            ),
            doc("docs/issues/2026-10-05-the-bug.md", BUG),
        ];
        let f = check(
            &LifecycleTree::build(docs.drain(..)),
            d("2026-11-11"),
            &CheckPolicy::default(),
        );
        assert!(!f
            .iter()
            .any(|x| matches!(x, Finding::CitedTargetClosed { .. })));
    }

    #[test]
    fn findings_come_in_variant_order() {
        let f = check(&tree(), today(), &CheckPolicy::default());
        let names: Vec<String> = f
            .iter()
            .map(|x| {
                format!("{x:?}")
                    .split([' ', '('])
                    .next()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(
            names,
            [
                "RevisitDue",
                "CitedTargetClosed",
                "StreamAllTerminal",
                "LedgerQuiet",
                "TerminalInLiveDir",
                "OrphanEntry"
            ]
        );
    }

    #[test]
    fn an_archived_entry_does_not_nag_but_an_invalid_entry_is_found_wherever_it_sits() {
        let f = run(vec![
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            // Archived and blocked with a passed revisit_by: no RevisitDue.
            doc(
                "docs/trackers/ledgers/l/archive/2026-10-01-old.md",
                &entry(
                    "waiting",
                    "revisit_by: 2026-11-01\n",
                    "**Rests on:** [[2026-10-05-the-bug]]\n",
                ),
            ),
            // Archived with a word the schema never declared: still found.
            doc(
                "docs/trackers/ledgers/l/archive/2026-10-01-odd.md",
                &entry("mystery", "", ""),
            ),
            // Archived and terminal with no `closed`: still found.
            doc(
                "docs/trackers/ledgers/l/archive/2026-10-01-open-ended.md",
                &entry("fixed", "", ""),
            ),
            doc("docs/issues/2026-10-05-the-bug.md", BUG),
        ]);
        assert!(!f.iter().any(|x| matches!(
            x,
            Finding::RevisitDue { .. } | Finding::CitedTargetClosed { .. }
        )));
        assert!(f.contains(&Finding::EntryInvalid {
            entry: k("2026-10-01-odd"),
            problem: EntryProblem::StatusUndeclared("mystery".into())
        }));
        assert!(f.contains(&Finding::EntryInvalid {
            entry: k("2026-10-01-open-ended"),
            problem: EntryProblem::ClosedMissing {
                status: "fixed".into()
            }
        }));
    }

    #[test]
    fn an_unparsed_open_entry_keeps_its_stream_from_reading_as_all_terminal() {
        let f = run(vec![
            doc("docs/trackers/streams/s/readme.md", STREAM),
            doc(
                "docs/trackers/streams/s/2026-10-01-done.md",
                &entry("done", "closed: 2026-10-03\n", ""),
            ),
            // Open, but its `created` is malformed, so it is in no `entries`.
            doc(
                "docs/trackers/streams/s/2026-10-02-open.md",
                &entry("open", "", "").replace("created: 2026-10-01", "created: 2026-8-1"),
            ),
        ]);
        assert!(
            !f.iter()
                .any(|x| matches!(x, Finding::StreamAllTerminal { .. })),
            "{f:?}"
        );
        assert!(f.contains(&Finding::EntryInvalid {
            entry: k("2026-10-02-open"),
            problem: EntryProblem::Unparsed(vec![EntryError::BadDate {
                field: "created",
                value: "2026-8-1".into()
            }])
        }));
    }

    #[test]
    fn an_unparsed_entry_in_the_archive_also_keeps_its_stream_from_reading_as_all_terminal() {
        let f = run(vec![
            doc("docs/trackers/streams/s/readme.md", STREAM),
            doc(
                "docs/trackers/streams/s/2026-10-01-done.md",
                &entry("done", "closed: 2026-10-03\n", ""),
            ),
            doc(
                "docs/trackers/streams/s/archive/2026-10-02-odd.md",
                "---\nentry_kind: f\n---\n",
            ),
        ]);
        assert!(!f
            .iter()
            .any(|x| matches!(x, Finding::StreamAllTerminal { .. })));
    }

    #[test]
    fn a_loose_file_or_a_readme_in_a_lifecycle_folder_is_an_orphan_entry_once() {
        let f = run(vec![
            doc("docs/trackers/temp/loose.md", &entry("open", "", "")),
            doc("docs/trackers/ledgers/readme.md", LEDGER),
            doc(
                "docs/trackers/streams/no-readme/2026-10-01-lost.md",
                &entry("open", "", ""),
            ),
        ]);
        let mut orphans: Vec<&str> = f
            .iter()
            .filter_map(|x| match x {
                Finding::OrphanEntry { rel_path } => Some(rel_path.as_str()),
                _ => None,
            })
            .collect();
        orphans.sort_unstable();
        assert_eq!(
            orphans,
            [
                "docs/trackers/ledgers/readme.md",
                "docs/trackers/streams/no-readme/2026-10-01-lost.md",
                "docs/trackers/temp/loose.md",
            ]
        );
    }

    #[test]
    fn an_entry_with_no_status_since_is_an_invalid_entry_finding() {
        let f = run(vec![
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-undated.md",
                "---\nentry_kind: f\ntitle: t\nstatus: open\ncreated: 2026-10-01\n---\n",
            ),
        ]);
        assert!(f.contains(&Finding::EntryInvalid {
            entry: k("2026-10-01-undated"),
            problem: EntryProblem::Unparsed(vec![EntryError::Missing("status_since")])
        }));
    }

    #[test]
    fn a_stream_with_an_unreadable_entry_file_is_never_all_terminal() {
        let done = doc(
            "docs/trackers/streams/s/2026-10-01-done.md",
            &entry("done", "closed: 2026-10-03\n", ""),
        );
        let all_terminal = |docs: Vec<DocInput>| {
            run(docs)
                .iter()
                .any(|x| matches!(x, Finding::StreamAllTerminal { .. }))
        };
        // The guard is not always on: only valid terminal entries still read as all terminal.
        assert!(all_terminal(vec![
            doc("docs/trackers/streams/s/readme.md", STREAM),
            done.clone(),
        ]));
        // (i) an open entry file whose name is no key.
        assert!(!all_terminal(vec![
            doc("docs/trackers/streams/s/readme.md", STREAM),
            done.clone(),
            doc(
                "docs/trackers/streams/s/Bad_Stem.md",
                &entry("open", "", "")
            ),
        ]));
        // (ii) an open entry file whose frontmatter does not parse.
        assert!(!all_terminal(vec![
            doc("docs/trackers/streams/s/readme.md", STREAM),
            done,
            doc(
                "docs/trackers/streams/s/2026-10-02-broken.md",
                "---\nentry_kind: [unclosed\n---\n",
            ),
        ]));
    }

    #[test]
    fn a_stream_without_exit_is_a_finding_from_its_schema_problem() {
        let no_exit = "---\ntitle: S\nentry_kinds: [f]\nstatuses: {open: open, done: terminal}\ninitial_status: open\n---\n";
        let f = run(vec![doc("docs/trackers/streams/s/readme.md", no_exit)]);
        assert_eq!(f, [Finding::StreamWithoutExit { collection: k("s") }]);
    }

    #[test]
    fn a_ledger_with_no_entries_is_quiet_and_only_when_it_asks_for_review() {
        let no_review = "---\ntitle: L\nentry_kinds: [f]\nstatuses: {open: open, fixed: terminal}\ninitial_status: open\n---\n";
        let f = run(vec![
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc("docs/trackers/ledgers/plain/readme.md", no_review),
            doc("docs/trackers/ledgers/archive/gone/readme.md", LEDGER),
        ]);
        assert_eq!(
            f,
            [Finding::LedgerQuiet {
                collection: k("l"),
                last_activity: None
            }]
        );
    }

    #[test]
    fn a_ledger_with_recent_activity_is_not_quiet() {
        // review_every 30d; today 2026-11-11; created 2026-10-20 is 22 days ago.
        let f = run(vec![
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/l/2026-10-20-a.md",
                &entry("open", "", "").replace("created: 2026-10-01", "created: 2026-10-20"),
            ),
        ]);
        assert!(f.is_empty(), "{f:?}");
    }

    #[test]
    fn the_terminal_grace_is_a_policy_and_the_boundary_day_is_not_yet_late() {
        let docs = |closed: &str| {
            vec![
                doc("docs/trackers/ledgers/l/readme.md", LEDGER),
                doc(
                    "docs/trackers/ledgers/l/2026-10-20-a.md",
                    &entry("fixed", &format!("closed: {closed}\n"), "")
                        .replace("created: 2026-10-01", "created: 2026-10-20"),
                ),
            ]
        };
        let late = |closed: &str, grace: u32| {
            check(
                &LifecycleTree::build(docs(closed)),
                today(),
                &CheckPolicy {
                    terminal_grace_days: grace,
                },
            )
            .iter()
            .any(|x| matches!(x, Finding::TerminalInLiveDir { .. }))
        };
        // closed 2026-10-28 + 14 days = 2026-11-11: not before today.
        assert!(!late("2026-10-28", 14));
        assert!(late("2026-10-27", 14));
        assert!(late("2026-10-28", 13));
        assert_eq!(CheckPolicy::default().terminal_grace_days, 14);
    }

    #[test]
    fn a_citation_in_entry_form_fires_once_and_a_cross_repo_or_closed_source_one_does_not() {
        let f = run(vec![
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-twice.md",
                &entry(
                    "open",
                    "",
                    "**Rests on:** [[2026-10-05-the-bug:F-1]]\n**Promote when:** [[2026-10-05-the-bug]]\n",
                ),
            ),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-elsewhere.md",
                &entry("open", "", "**Rests on:** [[other-repo/2026-10-05-the-bug]]\n"),
            ),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-done.md",
                &entry("fixed", "closed: 2026-10-30\n", "**Rests on:** [[2026-10-05-the-bug]]\n"),
            ),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-blocked.md",
                &entry("waiting", "revisit_by: 2026-12-01\n", "**Rests on:** [[2026-10-05-the-bug]]\n"),
            ),
            doc("docs/issues/2026-10-05-the-bug.md", BUG),
        ]);
        let cited: Vec<&str> = f
            .iter()
            .filter_map(|x| match x {
                Finding::CitedTargetClosed { entry, .. } => Some(entry.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(cited, ["2026-10-01-blocked", "2026-10-01-twice"]);
    }

    // ---- defining conditions of each check ----

    /// A ledger readme with a `deferred`-class word, reviewed every 30 days.
    const LEDGER_DEFERRED: &str = "---\ntitle: L\nentry_kinds: [f]\nstatuses: {open: open, waiting: blocked, later: deferred, fixed: terminal}\ninitial_status: open\nreview_every: 30d\n---\n";

    fn of_kind(f: &[Finding], pick: fn(&Finding) -> bool) -> Vec<&Finding> {
        f.iter().filter(|x| pick(x)).collect()
    }

    fn is_revisit_due(f: &Finding) -> bool {
        matches!(f, Finding::RevisitDue { .. })
    }

    /// The findings for one ledger entry with `status` and a `revisit_by` date.
    fn revisit_findings(readme: &str, status: &str, revisit_by: &str) -> Vec<Finding> {
        run(vec![
            doc("docs/trackers/ledgers/l/readme.md", readme),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-e.md",
                &entry(status, &format!("revisit_by: {revisit_by}\n"), ""),
            ),
        ])
    }

    #[test]
    fn stream_all_terminal_needs_every_entry_terminal() {
        let f = run(vec![
            doc("docs/trackers/streams/s/readme.md", STREAM),
            doc(
                "docs/trackers/streams/s/2026-10-01-done.md",
                &entry("done", "closed: 2026-11-10\n", ""),
            ),
            doc(
                "docs/trackers/streams/s/2026-10-02-open.md",
                &entry("open", "", ""),
            ),
        ]);
        assert!(f.is_empty(), "{f:?}");
    }

    #[test]
    fn stream_all_terminal_needs_at_least_one_entry() {
        let f = run(vec![doc("docs/trackers/streams/s/readme.md", STREAM)]);
        assert!(f.is_empty(), "{f:?}");
    }

    #[test]
    fn revisit_due_is_strictly_before_today() {
        let on_the_day = revisit_findings(LEDGER, "waiting", "2026-11-11");
        assert!(
            of_kind(&on_the_day, is_revisit_due).is_empty(),
            "{on_the_day:?}"
        );
        let day_before = revisit_findings(LEDGER, "waiting", "2026-11-10");
        assert_eq!(
            of_kind(&day_before, is_revisit_due),
            [&Finding::RevisitDue {
                entry: k("2026-10-01-e"),
                revisit_by: d("2026-11-10")
            }]
        );
    }

    #[test]
    fn revisit_due_is_for_blocked_and_deferred_classes_only() {
        let f = revisit_findings(LEDGER, "open", "2026-11-01");
        assert!(of_kind(&f, is_revisit_due).is_empty(), "{f:?}");
    }

    #[test]
    fn a_deferred_entry_with_a_passed_revisit_by_is_due() {
        let f = revisit_findings(LEDGER_DEFERRED, "later", "2026-11-01");
        assert_eq!(
            of_kind(&f, is_revisit_due),
            [&Finding::RevisitDue {
                entry: k("2026-10-01-e"),
                revisit_by: d("2026-11-01")
            }]
        );
    }

    /// An open ledger entry whose `status_since` is `since`, citing `body`, and the bug file
    /// (closed 2026-10-06); the `CitedTargetClosed` findings of that tree.
    fn cited_findings(since: &str, body: &str) -> Vec<Finding> {
        let f = run(vec![
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-e.md",
                &entry("open", "", body).replace(
                    "status_since: 2026-10-01",
                    &format!("status_since: {since}"),
                ),
            ),
            doc("docs/issues/2026-10-05-the-bug.md", BUG),
        ]);
        f.into_iter()
            .filter(|x| matches!(x, Finding::CitedTargetClosed { .. }))
            .collect()
    }

    #[test]
    fn cited_target_closed_needs_the_target_closed_strictly_after_status_since() {
        let cite = "**Rests on:** [[2026-10-05-the-bug]]\n";
        assert!(cited_findings("2026-10-06", cite).is_empty());
        assert_eq!(cited_findings("2026-10-05", cite).len(), 1);
    }

    #[test]
    fn a_solo_entry_form_citation_is_a_relation_to_its_key() {
        let f = cited_findings("2026-10-01", "**Rests on:** [[2026-10-05-the-bug:F-3]]\n");
        assert_eq!(
            f,
            [Finding::CitedTargetClosed {
                entry: k("2026-10-01-e"),
                target: k("2026-10-05-the-bug"),
                closed: d("2026-10-06")
            }]
        );
    }

    #[test]
    fn a_backward_relation_to_a_closed_target_never_fires() {
        let cited = |label: &str| {
            cited_findings(
                "2026-10-01",
                &format!("**{label}:** [[2026-10-05-the-bug]]\n"),
            )
        };
        assert!(cited("Promoted from").is_empty());
        assert!(cited("Supersedes").is_empty());
        assert_eq!(cited("Rests on").len(), 1);
        assert_eq!(cited("Blocks").len(), 1);
    }

    /// One live ledger entry created on `created`, plus extra documents; the `LedgerQuiet`
    /// findings (today is 2026-11-11, `review_every` 30d, so the cutoff is 2026-10-12).
    fn quiet_findings(extra: Vec<DocInput>) -> Vec<Finding> {
        let mut docs = vec![doc("docs/trackers/ledgers/l/readme.md", LEDGER)];
        docs.extend(extra);
        run(docs)
            .into_iter()
            .filter(|x| matches!(x, Finding::LedgerQuiet { .. }))
            .collect()
    }

    fn created_on(created: &str) -> String {
        entry("open", "", "").replace("created: 2026-10-01", &format!("created: {created}"))
    }

    #[test]
    fn ledger_quiet_is_strictly_older_than_the_review_window() {
        // 2026-11-11 minus 30 days is 2026-10-12: activity on that day is not yet quiet.
        let on_the_cutoff = quiet_findings(vec![doc(
            "docs/trackers/ledgers/l/2026-10-12-a.md",
            &created_on("2026-10-12"),
        )]);
        assert!(on_the_cutoff.is_empty(), "{on_the_cutoff:?}");
        let day_before = quiet_findings(vec![doc(
            "docs/trackers/ledgers/l/2026-10-11-a.md",
            &created_on("2026-10-11"),
        )]);
        assert_eq!(
            day_before,
            [Finding::LedgerQuiet {
                collection: k("l"),
                last_activity: Some(d("2026-10-11"))
            }]
        );
    }

    #[test]
    fn an_archived_entry_counts_toward_ledger_activity() {
        // The live entry is older than the cutoff; the archived one was closed inside the window.
        let recent_archived = quiet_findings(vec![
            doc(
                "docs/trackers/ledgers/l/2026-10-01-live.md",
                &created_on("2026-10-01"),
            ),
            doc(
                "docs/trackers/ledgers/l/archive/2026-10-01-old.md",
                &entry("fixed", "closed: 2026-11-05\n", ""),
            ),
        ]);
        assert!(recent_archived.is_empty(), "{recent_archived:?}");
        // The archived entry is the latest activity, though still outside the window.
        let latest_archived = quiet_findings(vec![
            doc(
                "docs/trackers/ledgers/l/2026-09-01-live.md",
                &created_on("2026-09-01"),
            ),
            doc(
                "docs/trackers/ledgers/l/archive/2026-10-01-old.md",
                &entry("fixed", "closed: 2026-10-05\n", ""),
            ),
        ]);
        assert_eq!(
            latest_archived,
            [Finding::LedgerQuiet {
                collection: k("l"),
                last_activity: Some(d("2026-10-05"))
            }]
        );
    }

    // ---- entry_invalid: one finding per `validate_entry` problem, for every entry ----

    /// A ledger readme that also declares an enum field `severity`.
    const LEDGER_TYPED: &str = "---\ntitle: L\nentry_kinds: [f]\nstatuses: {open: open, waiting: blocked, fixed: terminal}\ninitial_status: open\nfields:\n  severity: {type: enum, values: [low, high]}\n---\n";

    fn is_entry_invalid(f: &Finding) -> bool {
        matches!(f, Finding::EntryInvalid { .. })
    }

    /// The `EntryInvalid` findings for one entry at `path` under the given readme.
    fn invalid_findings(readme: &str, path: &str, text: &str) -> Vec<Finding> {
        let f = run(vec![
            doc("docs/trackers/ledgers/l/readme.md", readme),
            doc(path, text),
        ]);
        f.into_iter().filter(is_entry_invalid).collect()
    }

    fn invalid(entry_key: &str, problem: EntryProblem) -> Finding {
        Finding::EntryInvalid {
            entry: k(entry_key),
            problem,
        }
    }

    const E: &str = "docs/trackers/ledgers/l/2026-10-01-e.md";

    #[test]
    fn entry_invalid_reports_an_undeclared_kind() {
        let text = entry("open", "", "").replace("entry_kind: f", "entry_kind: g");
        assert_eq!(
            invalid_findings(LEDGER, E, &text),
            [invalid(
                "2026-10-01-e",
                EntryProblem::KindUndeclared("g".into())
            )]
        );
    }

    #[test]
    fn entry_invalid_reports_an_undeclared_status() {
        assert_eq!(
            invalid_findings(LEDGER, E, &entry("mystery", "", "")),
            [invalid(
                "2026-10-01-e",
                EntryProblem::StatusUndeclared("mystery".into())
            )]
        );
    }

    #[test]
    fn entry_invalid_reports_a_blocked_entry_with_no_revisit_by() {
        assert_eq!(
            invalid_findings(LEDGER, E, &entry("waiting", "", "")),
            [invalid(
                "2026-10-01-e",
                EntryProblem::RevisitMissing {
                    status: "waiting".into()
                }
            )]
        );
    }

    #[test]
    fn entry_invalid_reports_a_terminal_entry_with_no_closed() {
        assert_eq!(
            invalid_findings(LEDGER, E, &entry("fixed", "", "")),
            [invalid(
                "2026-10-01-e",
                EntryProblem::ClosedMissing {
                    status: "fixed".into()
                }
            )]
        );
    }

    #[test]
    fn entry_invalid_reports_a_hand_reopened_entry_that_still_carries_closed() {
        assert_eq!(
            invalid_findings(LEDGER, E, &entry("open", "closed: 2026-10-02\n", "")),
            [invalid(
                "2026-10-01-e",
                EntryProblem::ClosedOnNonTerminal {
                    status: "open".into()
                }
            )]
        );
    }

    #[test]
    fn entry_invalid_reports_an_undeclared_field() {
        assert_eq!(
            invalid_findings(LEDGER, E, &entry("open", "bogus: 1\n", "")),
            [invalid(
                "2026-10-01-e",
                EntryProblem::FieldUndeclared("bogus".into())
            )]
        );
    }

    #[test]
    fn entry_invalid_reports_a_field_of_the_wrong_type() {
        assert_eq!(
            invalid_findings(LEDGER_TYPED, E, &entry("open", "severity: medium\n", "")),
            [invalid(
                "2026-10-01-e",
                EntryProblem::FieldType {
                    field: "severity".into(),
                    expected: FieldType::Enum(vec!["low".into(), "high".into()])
                }
            )]
        );
        // A value the field accepts is not a problem.
        assert!(
            invalid_findings(LEDGER_TYPED, E, &entry("open", "severity: low\n", "")).is_empty()
        );
    }

    #[test]
    fn a_clean_entry_has_no_entry_invalid_finding() {
        let f = check(&tree(), today(), &CheckPolicy::default());
        assert!(!f.iter().any(is_entry_invalid), "{f:?}");
    }

    #[test]
    fn every_problem_of_one_entry_is_a_finding_in_the_order_validate_entry_gives() {
        // Wrong kind, then a terminal status with no `closed`: two problems, one entry.
        let text = entry("fixed", "", "").replace("entry_kind: f", "entry_kind: g");
        assert_eq!(
            invalid_findings(LEDGER, E, &text),
            [
                invalid("2026-10-01-e", EntryProblem::KindUndeclared("g".into())),
                invalid(
                    "2026-10-01-e",
                    EntryProblem::ClosedMissing {
                        status: "fixed".into()
                    }
                ),
            ]
        );
    }

    #[test]
    fn each_entry_is_validated_against_its_own_collections_schema() {
        // `waiting` is declared by the ledger but not by the stream.
        let f = run(vec![
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-in-ledger.md",
                &entry("waiting", "revisit_by: 2026-12-01\n", ""),
            ),
            doc("docs/trackers/streams/s/readme.md", STREAM),
            doc(
                "docs/trackers/streams/s/2026-10-01-in-stream.md",
                &entry("waiting", "revisit_by: 2026-12-01\n", ""),
            ),
        ]);
        let got: Vec<&Finding> = f.iter().filter(|x| is_entry_invalid(x)).collect();
        assert_eq!(
            got,
            [&invalid(
                "2026-10-01-in-stream",
                EntryProblem::StatusUndeclared("waiting".into())
            )]
        );
    }
}
