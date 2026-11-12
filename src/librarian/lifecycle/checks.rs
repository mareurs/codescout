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
//!   only: an archived entry is done and does not nag. `CitedTargetClosed` covers every
//!   non-terminal entry, blocked and deferred ones included. `TerminalInLiveDir` looks at
//!   entries of ledgers only (default grace 30 days): a stream or temp collection is archived
//!   whole, and `StreamAllTerminal` judges it.
//! - `StreamAllTerminal` and `LedgerQuiet` look at un-archived collections only. `LedgerQuiet`
//!   counts every entry of such a ledger, archived ones too, as activity.
//! - `CollectionInvalid` (the eighth check) reports every collection-level problem: a readme whose
//!   schema fails, a frontmatter block that does not parse, a nested collection, a second readme
//!   in one collection folder (only the first in path order is the readme), a file that is not
//!   Markdown, a name that is no key. Archived or not, it is a data-integrity finding.

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
    /// Days a terminal ledger entry may stay outside `archive/` after its `closed` date. The
    /// default is 30; the measurement behind it is entry W-3 of
    /// `docs/trackers/tracker-organization-session-log.md`.
    pub terminal_grace_days: u32,
}

impl Default for CheckPolicy {
    fn default() -> Self {
        CheckPolicy {
            terminal_grace_days: 30,
        }
    }
}

/// What `Finding::CollectionInvalid` found wrong. One variant per way a tracker file can fail
/// before any entry field is read: a readme whose schema fails, a frontmatter block that does
/// not parse, a file in a place the layout does not allow (below a nested readme, a second readme
/// in its folder, not Markdown), a name that is no key.
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
    /// The file sits at or below a subfolder that holds its own readme (`README.md`, `readme.md`:
    /// the name is matched without regard to case).
    #[error("sits below a subfolder that holds its own readme.md")]
    NestedCollection,
    /// The file is a second readme in its collection folder (a name `is_folder_index` reads as a
    /// readme: `README.md`, `Readme.md`, `readme.md`); the first one in path order is the readme.
    #[error(
        "is a second readme in its collection folder; only the first one in path order is read"
    )]
    DuplicateReadme,
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
    /// A live non-terminal entry (open, blocked or deferred) cites a target closed after the
    /// entry's `status_since`. Only forward relation labels count: a backward label
    /// (`relations::BACKWARD_LABELS`) never fires.
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
    /// Something wrong at collection level, not on one entry's fields. `subject` is always the
    /// repo-relative path of the file at fault: the collection's readme for a
    /// `CollectionProblem::Schema`, the skipped second readme (never the kept one) for a
    /// `CollectionProblem::DuplicateReadme`.
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
    /// A terminal entry of a ledger stayed outside `archive/` past the grace period. Streams
    /// and temp collections are archived whole, so this check does not look at their entries;
    /// `StreamAllTerminal` judges them.
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
/// key or the path (`CollectionInvalid` and `OrphanEntry` name a file, the others a key).
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
    } else {
        match collection.lifecycle {
            Lifecycle::Ledger => {
                if let Some(closed) = meta.closed {
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
            // A stream or temp collection is archived whole and judged by `StreamAllTerminal`,
            // so a terminal entry of one is never late by itself.
            Lifecycle::Stream | Lifecycle::Temp => {}
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
            TreeProblem::DuplicateReadme(rel_path) => out.push(collection_invalid(
                rel_path,
                CollectionProblem::DuplicateReadme,
            )),
            TreeProblem::Frontmatter { rel_path, detail } => out.push(collection_invalid(
                rel_path,
                CollectionProblem::Frontmatter(detail.clone()),
            )),
            TreeProblem::Schema { rel_path, errors } => {
                for error in errors {
                    out.push(collection_invalid(
                        rel_path,
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
mod tests;
