//! The eight field-based lifecycle checks over a [`LifecycleTree`].
//!
//! Pure: `check` reads the tree, `today` and the policy, nothing else. No git, no clock, no
//! filesystem, so no check can read a commit date (anti-goal 2).
//!
//! Which entries each check looks at:
//!
//! - `EntryInvalid` (spec `entry_invalid`) gives one finding per problem `validate_entry`
//!   returns, which generalises the former `status_undeclared`. An entry that does not parse
//!   gives one finding with every parse error. It looks at every entry, archived or not (data
//!   integrity), and also reports a non-terminal archived entry (`NonTerminalArchived`:
//!   archiving is for finished work).
//! - `RevisitDue`, `CitedTargetClosed` and `TerminalInLiveDir` look at un-archived entries
//!   only: an archived entry is done and does not nag.
//! - `StreamAllTerminal` and `LedgerQuiet` look at un-archived collections only. `LedgerQuiet`
//!   counts every entry of such a ledger, archived ones too, as activity.
//! - `CollectionInvalid` (the eighth check) reports every collection-level problem: a readme whose
//!   schema fails, a frontmatter block that does not parse, a nested collection, a file that
//!   is not Markdown, a name that is no key. Archived or not, it is a data-integrity finding.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use chrono::{Days, NaiveDate};
use serde::Serialize;

use super::entry::{class_of, validate_entry, EntryProblem};
use super::layout::{Lifecycle, PlacementError};
use super::relations::{is_backward, Relation};
use super::schema::{SchemaError, StatusClass};
use super::table::key_as_str;
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

/// What `Finding::CollectionInvalid` found wrong. One variant per way a tracker file can fail
/// before any entry field is read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectionProblem {
    /// One problem in a collection readme's schema (a readme with several gives several).
    #[error("{0}")]
    Schema(SchemaError),
    /// The frontmatter block of a readme or an entry file does not parse; the detail is the
    /// parser's message, which already names the frontmatter (`malformed frontmatter YAML: ..`).
    #[error("{0}")]
    Frontmatter(String),
    /// The file sits at or below a subfolder that holds its own `readme.md`.
    #[error("sits below a subfolder that holds its own readme.md")]
    NestedCollection,
    /// A file below a lifecycle folder that is not Markdown.
    #[error("is not a Markdown file")]
    NotMarkdown,
    /// A collection folder name or an entry file stem is no key: the `Display` text of the
    /// `KeyError`, which has no serializer.
    #[error("name is not a valid key: {0}")]
    BadName(String),
}

/// The eight check names, in the variant order of [`Finding`]; `Finding::check_name` returns one.
pub const CHECK_NAMES: [&str; 8] = [
    "entry_invalid",
    "revisit_due",
    "cited_target_closed",
    "stream_all_terminal",
    "collection_invalid",
    "ledger_quiet",
    "terminal_in_live_dir",
    "orphan_entry",
];

/// One thing `doctor` reports. The variants are listed in the order `check` returns them.
///
/// Serialized with an internal `check` tag (the check name) beside the variant's fields; keys
/// serialize as strings. Plan L-B2 and the identity session read this JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "check", rename_all = "snake_case")]
pub enum Finding {
    /// An entry breaks its collection's schema: one finding per problem `validate_entry`
    /// returns (including `EntryProblem::NonTerminalArchived`, an archived entry whose status
    /// is not terminal), or one `EntryProblem::Unparsed` carrying every error of an entry whose
    /// frontmatter does not parse at all.
    EntryInvalid {
        #[serde(serialize_with = "key_as_str")]
        entry: Key,
        problem: EntryProblem,
    },
    /// A blocked or deferred entry's `revisit_by` has passed.
    RevisitDue {
        #[serde(serialize_with = "key_as_str")]
        entry: Key,
        revisit_by: NaiveDate,
    },
    /// A live entry cites a target closed after the entry's `status_since`. Only forward
    /// relation labels count: a backward label (`relations::BACKWARD_LABELS`) never fires.
    CitedTargetClosed {
        #[serde(serialize_with = "key_as_str")]
        entry: Key,
        #[serde(serialize_with = "key_as_str")]
        target: Key,
        closed: NaiveDate,
    },
    /// Every entry of a live stream or temp collection is terminal, and none failed to parse.
    StreamAllTerminal {
        #[serde(serialize_with = "key_as_str")]
        collection: Key,
    },
    /// Something wrong at collection level, not on one entry's fields. `subject` is the
    /// collection key for `CollectionProblem::Schema`, otherwise the repo-relative path of the
    /// file at fault.
    CollectionInvalid {
        subject: String,
        problem: CollectionProblem,
    },
    /// A ledger with `review_every` has had no entry created or closed in that period.
    LedgerQuiet {
        #[serde(serialize_with = "key_as_str")]
        collection: Key,
        last_activity: Option<NaiveDate>,
    },
    /// A terminal entry stayed outside `archive/` past the grace period.
    TerminalInLiveDir {
        #[serde(serialize_with = "key_as_str")]
        entry: Key,
        closed: NaiveDate,
    },
    /// An entry file with no collection readme above it: an entry in a folder with no readme,
    /// or a file directly in a lifecycle folder.
    OrphanEntry { rel_path: String },
}

impl Finding {
    /// The check this finding belongs to: one of [`CHECK_NAMES`], and the `check` tag of its JSON.
    pub fn check_name(&self) -> &'static str {
        match self {
            Finding::EntryInvalid { .. } => CHECK_NAMES[0],
            Finding::RevisitDue { .. } => CHECK_NAMES[1],
            Finding::CitedTargetClosed { .. } => CHECK_NAMES[2],
            Finding::StreamAllTerminal { .. } => CHECK_NAMES[3],
            Finding::CollectionInvalid { .. } => CHECK_NAMES[4],
            Finding::LedgerQuiet { .. } => CHECK_NAMES[5],
            Finding::TerminalInLiveDir { .. } => CHECK_NAMES[6],
            Finding::OrphanEntry { .. } => CHECK_NAMES[7],
        }
    }

    /// The line of [`Display`](std::fmt::Display) before control characters are escaped.
    fn unescaped_line(&self) -> String {
        let detail = match self {
            Finding::EntryInvalid { entry, problem } => format!("{entry} — {problem}"),
            Finding::RevisitDue { entry, revisit_by } => {
                format!("{entry} — revisit_by {revisit_by} has passed")
            }
            Finding::CitedTargetClosed {
                entry,
                target,
                closed,
            } => format!(
                "{entry} — cites {target}, which closed {closed}, after this entry's status_since"
            ),
            Finding::StreamAllTerminal { collection } => {
                format!("{collection} — every entry is terminal")
            }
            Finding::CollectionInvalid { subject, problem } => format!("{subject} — {problem}"),
            Finding::LedgerQuiet {
                collection,
                last_activity: Some(last),
            } => format!("{collection} — no entry created or closed since {last}"),
            Finding::LedgerQuiet {
                collection,
                last_activity: None,
            } => format!("{collection} — no parsed entry has a created or closed date"),
            Finding::TerminalInLiveDir { entry, closed } => {
                format!("{entry} — terminal since {closed} but not archived")
            }
            Finding::OrphanEntry { rel_path } => {
                format!("{rel_path} — no collection readme above this file")
            }
        };
        format!("{}: {detail}", self.check_name())
    }

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
            Finding::CollectionInvalid { subject, .. } => (4, subject.as_str(), String::new()),
            Finding::LedgerQuiet { collection, .. } => (5, collection.as_str(), String::new()),
            Finding::TerminalInLiveDir { entry, closed } => (6, entry.as_str(), closed.to_string()),
            Finding::OrphanEntry { rel_path } => (7, rel_path.as_str(), String::new()),
        }
    }
}

/// One line: `<check_name>: <subject> — <detail>`, the subject being the entry key, the collection
/// key or the path.
///
/// The line is one line whatever the finding holds: text that comes from outside the crate (a
/// YAML status word, a file name git lists) may carry a control character, and each one is
/// escaped here, in the one place every `Finding` is rendered (`\n` prints as `\n`).
impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let line = self.unescaped_line();
        // `escape_default` is `\n`, `\r`, `\t`, or `\u{..}` for any other control character.
        // U+2028 and U+2029 are not control characters but end a line in some viewers.
        for c in line.chars() {
            if c.is_control() || matches!(c, '\u{2028}' | '\u{2029}') {
                write!(f, "{}", c.escape_default())?;
            } else {
                f.write_char(c)?;
            }
        }
        Ok(())
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
        // An undeclared status already gave `StatusUndeclared`; only a declared one can be
        // non-terminal.
        if class_of(&collection.schema, meta).is_some_and(|c| c != StatusClass::Terminal) {
            out.push(Finding::EntryInvalid {
                entry: entry.key.clone(),
                problem: EntryProblem::NonTerminalArchived {
                    status: meta.status.clone(),
                },
            });
        }
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

fn collection_invalid(subject: &str, problem: CollectionProblem) -> Finding {
    Finding::CollectionInvalid {
        subject: subject.to_string(),
        problem,
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
        // No wildcard arm: a new `TreeProblem` or `PlacementError` variant must be mapped here.
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
            TreeProblem::Placement(PlacementError::NotMarkdown(rel_path)) => {
                out.push(collection_invalid(rel_path, CollectionProblem::NotMarkdown))
            }
            // A bad collection folder name arrives here too, once per file in that folder.
            TreeProblem::Placement(PlacementError::BadKey { rel_path, error }) => out.push(
                collection_invalid(rel_path, CollectionProblem::BadName(error.to_string())),
            ),
            TreeProblem::NestedCollection(rel_path) => out.push(collection_invalid(
                rel_path,
                CollectionProblem::NestedCollection,
            )),
            TreeProblem::Frontmatter { rel_path, detail } => out.push(collection_invalid(
                rel_path,
                CollectionProblem::Frontmatter(detail.clone()),
            )),
            TreeProblem::Schema { collection, errors } => {
                for error in errors {
                    out.push(collection_invalid(
                        collection.as_str(),
                        CollectionProblem::Schema(error.clone()),
                    ));
                }
            }
            TreeProblem::Entry { key, errors, .. } => out.push(Finding::EntryInvalid {
                entry: key.clone(),
                problem: EntryProblem::Unparsed(errors.clone()),
            }),
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
    use crate::librarian::lifecycle::testkit::{d, doc, entry, k, LEDGER, STREAM, STREAM_NO_EXIT};
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
        // The archived `waiting` entry is entry_invalid: archived and not terminal.
        assert!(f.contains(&Finding::EntryInvalid {
            entry: k("2026-10-01-old"),
            problem: EntryProblem::NonTerminalArchived {
                status: "waiting".into()
            }
        }));
    }

    #[test]
    fn an_archived_deferred_entry_is_entry_invalid_too() {
        let f = run(vec![
            doc("docs/trackers/ledgers/l/readme.md", LEDGER_DEFERRED),
            doc(
                "docs/trackers/ledgers/l/archive/2026-10-01-parked.md",
                &entry("later", "revisit_by: 2026-12-01\n", ""),
            ),
        ]);
        // The archived `later` entry is entry_invalid: archived and not terminal.
        assert!(f.contains(&Finding::EntryInvalid {
            entry: k("2026-10-01-parked"),
            problem: EntryProblem::NonTerminalArchived {
                status: "later".into()
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
    fn a_stream_readme_without_exit_when_is_collection_invalid() {
        let f = run(vec![doc(
            "docs/trackers/streams/s/readme.md",
            STREAM_NO_EXIT,
        )]);
        assert_eq!(
            f,
            [Finding::CollectionInvalid {
                subject: "s".into(),
                problem: CollectionProblem::Schema(SchemaError::ExitRequired)
            }]
        );
    }

    #[test]
    fn a_readme_with_two_schema_errors_gives_one_finding_per_error_in_parse_order() {
        // `parse_schema` pushes `MissingKey("initial_status")` before it checks `exit_when`.
        let two_errors = STREAM_NO_EXIT.replace("initial_status: open\n", "");
        let f = run(vec![doc("docs/trackers/streams/s/readme.md", &two_errors)]);
        assert_eq!(
            f,
            [
                Finding::CollectionInvalid {
                    subject: "s".into(),
                    problem: CollectionProblem::Schema(SchemaError::MissingKey("initial_status"))
                },
                Finding::CollectionInvalid {
                    subject: "s".into(),
                    problem: CollectionProblem::Schema(SchemaError::ExitRequired)
                }
            ]
        );
    }

    /// One finding of each variant, in variant order.
    fn one_of_each() -> [Finding; 8] {
        [
            Finding::EntryInvalid {
                entry: k("2026-10-01-bad"),
                problem: EntryProblem::StatusUndeclared("x".into()),
            },
            Finding::RevisitDue {
                entry: k("2026-10-01-waits"),
                revisit_by: d("2026-11-01"),
            },
            Finding::CitedTargetClosed {
                entry: k("2026-10-01-cites"),
                target: k("2026-10-05-the-bug"),
                closed: d("2026-10-06"),
            },
            Finding::StreamAllTerminal { collection: k("s") },
            Finding::CollectionInvalid {
                subject: "s".into(),
                problem: CollectionProblem::Schema(SchemaError::ExitRequired),
            },
            Finding::LedgerQuiet {
                collection: k("l"),
                last_activity: Some(d("2026-10-02")),
            },
            Finding::TerminalInLiveDir {
                entry: k("2026-10-01-old"),
                closed: d("2026-10-02"),
            },
            Finding::OrphanEntry {
                rel_path: "docs/trackers/streams/no-readme/2026-10-01-lost.md".into(),
            },
        ]
    }

    #[test]
    fn check_names_are_the_eight_of_the_spec_and_each_variant_reports_one() {
        let distinct: BTreeSet<&str> = CHECK_NAMES.iter().copied().collect();
        assert_eq!(distinct.len(), 8);
        let hit: BTreeSet<&str> = one_of_each().iter().map(Finding::check_name).collect();
        assert_eq!(hit, distinct);
    }

    #[test]
    fn display_is_one_line_naming_the_subject() {
        let subjects = [
            "2026-10-01-bad",
            "2026-10-01-waits",
            "2026-10-01-cites",
            "s",
            "s",
            "l",
            "2026-10-01-old",
            "docs/trackers/streams/no-readme/2026-10-01-lost.md",
        ];
        for (finding, subject) in one_of_each().iter().zip(subjects) {
            let text = finding.to_string();
            assert!(!text.contains('\n'), "{text}");
            let prefix = format!("{}: {subject} — ", finding.check_name());
            assert!(text.starts_with(&prefix), "{text}");
        }
        let [invalid, due, ..] = one_of_each();
        assert_eq!(
            due.to_string(),
            "revisit_due: 2026-10-01-waits — revisit_by 2026-11-01 has passed"
        );
        assert_eq!(
            invalid.to_string(),
            "entry_invalid: 2026-10-01-bad — status \"x\" is not declared in the collection readme"
        );
    }

    #[test]
    fn display_escapes_control_characters_from_outside_text() {
        // A quoted YAML key and class with a real newline reach `UnknownClass` as-is.
        let readme = "---\ntitle: S\nentry_kinds: [f]\nstatuses: {open: open, done: terminal, \"a\\nb\": \"c\\nd\"}\ninitial_status: open\nexit_when: it ships\n---\n";
        let f = run(vec![doc("docs/trackers/streams/s/readme.md", readme)]);
        assert_eq!(f.len(), 1, "{f:?}");
        let Finding::CollectionInvalid {
            problem: CollectionProblem::Schema(SchemaError::UnknownClass { word, class }),
            ..
        } = &f[0]
        else {
            panic!("{:?}", f[0]);
        };
        // The finding keeps the raw text (JSON is unaffected) ...
        assert_eq!((word.as_str(), class.as_str()), ("a\nb", "c\nd"));
        // ... and the one-line rendering escapes it.
        let line = f[0].to_string();
        assert!(!line.contains('\n') && !line.contains('\r'), "{line:?}");
        assert!(line.contains("a\\nb") && line.contains("c\\nd"), "{line}");

        let path = Finding::OrphanEntry {
            rel_path: "docs/trackers/streams/x/a\nb\t\r\u{1b}.md".into(),
        };
        let line = path.to_string();
        assert!(!line.chars().any(char::is_control), "{line:?}");
        assert!(line.contains("a\\nb\\t\\r\\u{1b}.md"), "{line}");
    }

    #[test]
    fn display_leaves_ordinary_text_and_non_ascii_unescaped() {
        let finding = Finding::OrphanEntry {
            rel_path: "docs/trackers/streams/é/x.md".into(),
        };
        assert_eq!(
            finding.to_string(),
            "orphan_entry: docs/trackers/streams/é/x.md — no collection readme above this file"
        );
    }

    #[test]
    fn every_problem_message_names_the_offending_word_field_or_date() {
        let cases: Vec<(String, &str)> = vec![
            (SchemaError::MissingTitle.to_string(), "title"),
            (
                SchemaError::MissingKey("entry_kinds").to_string(),
                "entry_kinds",
            ),
            (
                SchemaError::Malformed {
                    key: "fields",
                    detail: "not a map".into(),
                }
                .to_string(),
                "fields",
            ),
            (
                SchemaError::UnknownClass {
                    word: "open-ish".into(),
                    class: "half".into(),
                }
                .to_string(),
                "half",
            ),
            (SchemaError::NoTerminalStatus.to_string(), "terminal"),
            (
                SchemaError::InitialNotOpen("done".into()).to_string(),
                "done",
            ),
            (
                SchemaError::InitialUndeclared("nope".into()).to_string(),
                "nope",
            ),
            (SchemaError::ExitRequired.to_string(), "exit_when"),
            (SchemaError::ExitRefused.to_string(), "exit_when"),
            (EntryError::Missing("created").to_string(), "created"),
            (
                EntryError::BadDate {
                    field: "closed",
                    value: "2026-8-1".into(),
                }
                .to_string(),
                "2026-8-1",
            ),
            (EntryError::BadValidity("soon".into()).to_string(), "soon"),
            (EntryProblem::KindUndeclared("zz".into()).to_string(), "zz"),
            (
                EntryProblem::RevisitMissing {
                    status: "waiting".into(),
                }
                .to_string(),
                "waiting",
            ),
            (
                EntryProblem::ClosedMissing {
                    status: "fixed".into(),
                }
                .to_string(),
                "fixed",
            ),
            (
                EntryProblem::ClosedOnNonTerminal {
                    status: "open".into(),
                }
                .to_string(),
                "open",
            ),
            (
                EntryProblem::NonTerminalArchived {
                    status: "later".into(),
                }
                .to_string(),
                "later",
            ),
            (
                EntryProblem::FieldUndeclared("sev".into()).to_string(),
                "sev",
            ),
            (
                EntryProblem::FieldType {
                    field: "sev".into(),
                    expected: FieldType::Enum(vec!["low".into(), "high".into()]),
                }
                .to_string(),
                "high",
            ),
            (
                EntryProblem::Unparsed(vec![
                    EntryError::Missing("created"),
                    EntryError::BadValidity("soon".into()),
                ])
                .to_string(),
                "soon",
            ),
            (
                CollectionProblem::Frontmatter("bad yaml".into()).to_string(),
                "bad yaml",
            ),
            (
                CollectionProblem::BadName("Bad_Stem".into()).to_string(),
                "Bad_Stem",
            ),
        ];
        for (text, needle) in cases {
            assert!(text.contains(needle), "{text:?} lacks {needle:?}");
            assert!(!text.contains('\n'), "{text:?}");
        }
    }

    #[test]
    fn serialized_findings_have_this_exact_json() {
        let [invalid, due, ..] = one_of_each();
        assert_eq!(
            serde_json::to_string(&due).unwrap(),
            r#"{"check":"revisit_due","entry":"2026-10-01-waits","revisit_by":"2026-11-01"}"#
        );
        assert_eq!(
            serde_json::to_string(&invalid).unwrap(),
            r#"{"check":"entry_invalid","entry":"2026-10-01-bad","problem":{"status_undeclared":"x"}}"#
        );
        let collection_invalid = Finding::CollectionInvalid {
            subject: "s".into(),
            problem: CollectionProblem::Schema(SchemaError::ExitRequired),
        };
        assert_eq!(
            serde_json::to_string(&collection_invalid).unwrap(),
            r#"{"check":"collection_invalid","subject":"s","problem":{"schema":"exit_required"}}"#
        );
        for finding in one_of_each() {
            let value = serde_json::to_value(&finding).unwrap();
            assert_eq!(value["check"], finding.check_name());
        }
    }

    #[test]
    fn a_ledger_quiet_with_no_parsed_entry_serializes_null_and_says_why() {
        let quiet = Finding::LedgerQuiet {
            collection: k("l"),
            last_activity: None,
        };
        assert_eq!(
            serde_json::to_string(&quiet).unwrap(),
            r#"{"check":"ledger_quiet","collection":"l","last_activity":null}"#
        );
        // Entries may exist and all fail to parse, so the line must not say there are none.
        assert_eq!(
            quiet.to_string(),
            "ledger_quiet: l — no parsed entry has a created or closed date"
        );
    }

    #[test]
    fn nested_problems_serialize_in_snake_case_with_default_tagging() {
        let problem = EntryProblem::FieldType {
            field: "sev".into(),
            expected: FieldType::Enum(vec!["low".into()]),
        };
        assert_eq!(
            serde_json::to_string(&problem).unwrap(),
            r#"{"field_type":{"field":"sev","expected":{"enum":["low"]}}}"#
        );
        let unparsed = EntryProblem::Unparsed(vec![
            EntryError::Missing("created"),
            EntryError::BadDate {
                field: "closed",
                value: "x".into(),
            },
        ]);
        assert_eq!(
            serde_json::to_string(&unparsed).unwrap(),
            r#"{"unparsed":[{"missing":"created"},{"bad_date":{"field":"closed","value":"x"}}]}"#
        );
        assert_eq!(
            serde_json::to_string(&CollectionProblem::NestedCollection).unwrap(),
            r#""nested_collection""#
        );
    }

    #[test]
    fn a_ledger_readme_with_exit_when_is_collection_invalid() {
        let with_exit = LEDGER.replace("review_every: 30d\n", "exit_when: it ships\n");
        let f = run(vec![doc("docs/trackers/ledgers/l/readme.md", &with_exit)]);
        assert_eq!(
            f,
            [Finding::CollectionInvalid {
                subject: "l".into(),
                problem: CollectionProblem::Schema(SchemaError::ExitRefused)
            }]
        );
    }

    #[test]
    fn a_readme_with_broken_yaml_is_reported_by_path_and_its_entries_are_not_orphans() {
        let f = run(vec![
            doc(
                "docs/trackers/ledgers/l/readme.md",
                "---\ntitle: [unclosed\n---\n",
            ),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-a.md",
                &entry("open", "", ""),
            ),
        ]);
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(matches!(
            &f[0],
            Finding::CollectionInvalid {
                subject,
                problem: CollectionProblem::Frontmatter(_)
            } if subject == "docs/trackers/ledgers/l/readme.md"
        ));
        assert!(!f.iter().any(|x| matches!(x, Finding::OrphanEntry { .. })));
    }

    #[test]
    fn every_tree_problem_kind_is_reported() {
        let f = run(vec![
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            // Schema: one error (ExitRequired).
            doc("docs/trackers/streams/noexit/readme.md", STREAM_NO_EXIT),
            // Frontmatter: a block that is not YAML.
            doc(
                "docs/trackers/ledgers/l/2026-10-01-broken.md",
                "---\nentry_kind: [unclosed\n---\n",
            ),
            // Entry: YAML, but not an entry.
            doc(
                "docs/trackers/ledgers/l/2026-10-01-not-an-entry.md",
                "---\nentry_kind: f\n---\n",
            ),
            // NestedCollection: a readme and a file below it.
            doc("docs/trackers/ledgers/l/reports/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/l/reports/2026-10-01-r.md",
                &entry("open", "", ""),
            ),
            // Placement(NotMarkdown).
            doc("docs/trackers/ledgers/l/notes.txt", "plain"),
            // Placement(BadKey): an entry file name, and a collection folder name.
            doc(
                "docs/trackers/ledgers/l/Bad_Stem.md",
                &entry("open", "", ""),
            ),
            doc("docs/trackers/ledgers/Bad_Name/readme.md", LEDGER),
            // OrphanEntry: no readme above it, and a file directly in a lifecycle folder.
            doc(
                "docs/trackers/streams/no-readme/2026-10-01-lost.md",
                &entry("open", "", ""),
            ),
            doc("docs/trackers/temp/loose.md", &entry("open", "", "")),
        ]);
        let subjects_of = |pick: fn(&CollectionProblem) -> bool| -> Vec<&str> {
            let mut subjects: Vec<&str> = f
                .iter()
                .filter_map(|x| match x {
                    Finding::CollectionInvalid { subject, problem } if pick(problem) => {
                        Some(subject.as_str())
                    }
                    _ => None,
                })
                .collect();
            subjects.sort_unstable();
            subjects
        };
        assert_eq!(
            subjects_of(|p| matches!(p, CollectionProblem::Schema(_))),
            ["noexit"]
        );
        assert_eq!(
            subjects_of(|p| matches!(p, CollectionProblem::Frontmatter(_))),
            ["docs/trackers/ledgers/l/2026-10-01-broken.md"]
        );
        assert_eq!(
            subjects_of(|p| matches!(p, CollectionProblem::NestedCollection)),
            [
                "docs/trackers/ledgers/l/reports/2026-10-01-r.md",
                "docs/trackers/ledgers/l/reports/readme.md"
            ]
        );
        assert_eq!(
            subjects_of(|p| matches!(p, CollectionProblem::NotMarkdown)),
            ["docs/trackers/ledgers/l/notes.txt"]
        );
        assert_eq!(
            subjects_of(|p| matches!(p, CollectionProblem::BadName(_))),
            [
                "docs/trackers/ledgers/Bad_Name/readme.md",
                "docs/trackers/ledgers/l/Bad_Stem.md"
            ]
        );
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
                "docs/trackers/streams/no-readme/2026-10-01-lost.md",
                "docs/trackers/temp/loose.md"
            ]
        );
        let unparsed: Vec<&Key> = f
            .iter()
            .filter_map(|x| match x {
                Finding::EntryInvalid {
                    entry,
                    problem: EntryProblem::Unparsed(_),
                } => Some(entry),
                _ => None,
            })
            .collect();
        assert_eq!(unparsed, [&k("2026-10-01-not-an-entry")]);
        // Nothing else is a collection-level finding.
        let total = f
            .iter()
            .filter(|x| matches!(x, Finding::CollectionInvalid { .. }))
            .count();
        assert_eq!(total, 1 + 1 + 2 + 1 + 2, "{f:?}");
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

    #[test]
    fn a_non_terminal_entry_in_archive_is_entry_invalid() {
        let path = "docs/trackers/ledgers/l/archive/2026-10-01-x.md";
        let f = run(vec![
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc(path, &entry("open", "", "")),
        ]);
        let got: Vec<&Finding> = f.iter().filter(|x| is_entry_invalid(x)).collect();
        assert_eq!(
            got,
            [&invalid(
                "2026-10-01-x",
                EntryProblem::NonTerminalArchived {
                    status: "open".into()
                }
            )]
        );
        // The same file, terminal and closed, is fine.
        let f = run(vec![
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc(path, &entry("fixed", "closed: 2026-10-02\n", "")),
        ]);
        assert!(!f.iter().any(is_entry_invalid), "{f:?}");
    }

    #[test]
    fn an_open_entry_of_a_stream_moved_to_streams_archive_is_reported() {
        let f = run(vec![
            doc("docs/trackers/streams/archive/s/readme.md", STREAM),
            doc(
                "docs/trackers/streams/archive/s/2026-10-01-x.md",
                &entry("open", "", ""),
            ),
        ]);
        let got: Vec<&Finding> = f.iter().filter(|x| is_entry_invalid(x)).collect();
        assert_eq!(
            got,
            [&invalid(
                "2026-10-01-x",
                EntryProblem::NonTerminalArchived {
                    status: "open".into()
                }
            )]
        );
    }

    #[test]
    fn an_archived_entry_with_an_undeclared_status_gives_only_status_undeclared() {
        let f = run(vec![
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/l/archive/2026-10-01-x.md",
                &entry("mystery", "", ""),
            ),
        ]);
        let got: Vec<&Finding> = f.iter().filter(|x| is_entry_invalid(x)).collect();
        assert_eq!(
            got,
            [&invalid(
                "2026-10-01-x",
                EntryProblem::StatusUndeclared("mystery".into())
            )]
        );
    }
}
