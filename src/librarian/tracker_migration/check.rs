//! The check of a migration (Plan L-E1 Task 10): the independent proof that `apply` lost nothing.
//!
//! Spec: `docs/superpowers/specs/2026-11-15-tracker-migration-design.md` § What check proves.
//!
//! [`check_migration`] is pure. Every text arrives as a parameter: the source texts at `HEAD`,
//! the files under the collection folder as they are on disk, the markdown files of the working
//! tree. It does not read a text the planner stored for a body (`MigrationPlan::writes`): it
//! recomputes each expected body from the source text and the line range with [`expected_body`],
//! and compares it with the entry file on disk. A planner bug that drops a line from both the
//! range and the write is found by the partition, which counts the source lines itself.

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use serde::Serialize;

use super::index_rows::table_cells;
use super::plan::{expected_body, MigrationPlan, OwnedRange, Owner, Review};
use super::records::{heading_token, partition_problems, Lines, PartitionProblem};
use crate::librarian::frontmatter;
use crate::librarian::identity::grammar::{extract, Reference};
use crate::librarian::identity::key::is_folder_index;
use crate::librarian::lifecycle::checks::{check, CheckPolicy, Finding};
use crate::librarian::lifecycle::collection::LoadedCollection;
use crate::librarian::lifecycle::entry::{class_of, parse_entry};
use crate::librarian::lifecycle::layout::place;
use crate::librarian::lifecycle::schema::StatusClass;

/// What a check reads. The CLI fills it from `HEAD`, the disk and the plan it re-plans from
/// `HEAD`; a test fills it by hand.
pub struct CheckInput<'a> {
    pub plan: &'a MigrationPlan,
    /// Each source path to its text at `HEAD`.
    pub sources: &'a BTreeMap<String, String>,
    /// Every file under `plan.target_dir`, from disk (repo-relative path to text; a file that is
    /// not markdown has an empty text).
    pub collection_files: &'a BTreeMap<String, String>,
    /// Every markdown file of the working tree. Read only when the key changes.
    pub repo_markdown: &'a BTreeMap<String, String>,
    /// The sources and, with `sidecar = retire`, their sidecars that are still on disk. The CLI
    /// derives these paths from the mapping, not from `plan.deletes`.
    pub still_present: &'a BTreeSet<String>,
    /// The files of `collection_files` whose disk text equals their text at `HEAD`. In
    /// `existing` mode (`plan.readme_path` is `None`) such a file is an entry the collection had
    /// before this run, and not an extra file.
    pub unchanged_at_head: &'a BTreeSet<String>,
    /// The collection loaded from `plan.target_dir`, or why it does not load.
    pub loaded: Result<&'a LoadedCollection, String>,
    /// `None` when the hot view does not apply (an archived collection, or no `hot_cap`).
    pub hot: Option<Result<(), String>>,
    pub today: NaiveDate,
}

/// What a check found. `blocking` empty is the only clean result.
#[derive(Debug, Clone, Serialize)]
pub struct CheckReport {
    pub blocking: Vec<CheckFinding>,
    /// Never blocking: the plan's review, path mentions, and the time-based findings.
    pub review: Review,
    /// The entries carried with a `shorten` body, which the body item does not compare.
    pub shortened: Vec<String>,
}

/// One blocking finding. [`CheckFinding::text`] is its one-line message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckFinding {
    /// The plan's ranges do not tile the source: item 1.
    Partition {
        source: String,
        problem: PartitionProblem,
    },
    /// A plan range names a source that is not at `HEAD`.
    SourceMissing(String),
    /// The plan from `HEAD` has a problem, so `apply` would refuse it.
    PlanProblem(String),
    /// Item 2: no entry has this record token as its first heading.
    TokenMissing(String),
    /// Item 2: an entry has a token that no source defines.
    TokenExtra { token: String, file: String },
    /// Item 2: more than one entry has this record token as its first heading.
    TokenTwice(String),
    /// A file in the collection folder that the plan does not write and that has no token
    /// heading (or one the plan already gives to another entry).
    ExtraFile(String),
    /// A token that is in `plan.defined` or in the plan's records, but not in both.
    DefinedDiffers(String),
    /// A planned entry path that is not on disk.
    EntryMissing(String),
    /// Item 3: the body on disk is not the body recomputed from the source.
    BodyDiffers { entry: String, first_line: usize },
    /// The index row or the synthesised heading of a record is not a line of its source.
    RowDiffers(String),
    /// Item 3 for the readme: a readme section is not in the readme, in source order.
    ReadmeMissing { source: String, start: u32 },
    /// Item 4: a terminal entry outside `archive/`, or a non-terminal entry in it.
    Placement { entry: String, terminal: bool },
    /// Item 4: a date out of order.
    DateOrder { entry: String, detail: String },
    /// Item 5: the collection does not load, or the lifecycle checks report a problem.
    Lifecycle(String),
    /// Item 6: a citation of the old key.
    OldKeyCitation { file: String, line: u32 },
    /// Item 6: a retired sidecar is still on disk.
    SidecarLeft(String),
    /// A source is still on disk.
    SourceLeft(String),
    /// Item 7: the hot view does not render.
    HotView(String),
}

impl CheckFinding {
    /// The finding on one line, with repo-relative paths only. Text from outside (a lifecycle
    /// finding, a load error) is joined onto one line.
    pub fn text(&self) -> String {
        let line = match self {
            CheckFinding::Partition { source, problem } => match problem {
                PartitionProblem::Uncovered(r) => format!(
                    "{source}: {} of the source at HEAD have no owner in the plan.",
                    lines_text(r)
                ),
                PartitionProblem::Overlap { a, b } => format!(
                    "{source}: {} and {} of the source at HEAD have two owners in the plan.",
                    lines_text(a),
                    lines_text(b)
                ),
            },
            CheckFinding::SourceMissing(s) => {
                format!("The plan has line ranges of {s}, and {s} is not at HEAD.")
            }
            CheckFinding::PlanProblem(p) => {
                format!("The plan from HEAD has a problem, so apply refuses it: {p}")
            }
            CheckFinding::TokenMissing(t) => {
                format!("No entry has the record token {t} in its first heading.")
            }
            CheckFinding::TokenExtra { token, file } => format!(
                "{file} has the token {token} in its first heading, and no source record has it."
            ),
            CheckFinding::TokenTwice(t) => {
                format!("More than one entry has the record token {t} in its first heading.")
            }
            CheckFinding::ExtraFile(f) => {
                format!("{f} is in the collection folder, and the plan writes no entry there.")
            }
            CheckFinding::DefinedDiffers(t) => format!(
                "The token {t} is in the plan's defined tokens or in its records, not in both."
            ),
            CheckFinding::EntryMissing(e) => {
                format!("{e} is not on disk. The plan writes an entry there.")
            }
            CheckFinding::BodyDiffers { entry, first_line } => format!(
                "{entry}: the body is not the body of its source lines at HEAD. The first \
                 difference is at body line {first_line}."
            ),
            CheckFinding::RowDiffers(e) => format!(
                "{e}: the index row or the synthesised heading of the plan is not in the source \
                 at HEAD."
            ),
            CheckFinding::ReadmeMissing { source, start } => format!(
                "The readme does not hold the section at line {start} of {source} in source order."
            ),
            CheckFinding::Placement {
                entry,
                terminal: true,
            } => format!("{entry} has a terminal status, and it is not in archive/."),
            CheckFinding::Placement {
                entry,
                terminal: false,
            } => format!("{entry} is in archive/, and its status is not terminal."),
            CheckFinding::DateOrder { entry, detail } => format!("{entry}: {detail}."),
            CheckFinding::Lifecycle(s) => format!("Lifecycle check: {s}"),
            CheckFinding::OldKeyCitation { file, line } => {
                format!("{file} line {line} cites the old collection key.")
            }
            CheckFinding::SidecarLeft(p) => {
                format!("{p} is still on disk. The mapping retires this sidecar.")
            }
            CheckFinding::SourceLeft(p) => {
                format!("{p} is still on disk. Apply deletes every source.")
            }
            CheckFinding::HotView(e) => format!("The hot view does not render: {e}"),
        };
        line.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Run the check. See the module documentation; the items are those of spec § What check proves.
pub fn check_migration(i: &CheckInput) -> CheckReport {
    let plan = i.plan;
    let mut blocking: Vec<CheckFinding> = plan
        .problems
        .iter()
        .map(|p| CheckFinding::PlanProblem(p.clone()))
        .collect();
    let mut review = plan.review.clone();

    partition_item(i, &mut blocking);
    let records = records_of(plan, &mut blocking);
    let tokens: BTreeSet<String> = records
        .iter()
        .filter_map(|r| r.token.map(str::to_string))
        .collect();
    for t in tokens.symmetric_difference(&plan.defined) {
        blocking.push(CheckFinding::DefinedDiffers(t.clone()));
    }
    token_item(i, &records, &tokens, &mut blocking);
    let shortened = body_item(i, &records, &mut blocking);
    readme_item(i, &mut blocking);
    if let Ok(loaded) = &i.loaded {
        placement_item(i, loaded, &mut blocking);
    }
    lifecycle_item(i, &mut blocking, &mut review);
    old_key_item(i, &mut blocking);
    for p in i.still_present {
        blocking.push(if i.sources.contains_key(p) {
            CheckFinding::SourceLeft(p.clone())
        } else {
            CheckFinding::SidecarLeft(p.clone())
        });
    }
    if let Some(Err(e)) = &i.hot {
        blocking.push(CheckFinding::HotView(e.clone()));
    }
    CheckReport {
        blocking,
        review,
        shortened,
    }
}

/// One `Owner::Record` range of the plan, with its parts borrowed.
struct Rec<'p> {
    range: &'p OwnedRange,
    entry: &'p str,
    token: Option<&'p str>,
    heading: Option<&'p str>,
    row: Option<&'p str>,
}

/// The record ranges of the plan. A record with no entry path is a plan problem (the planner
/// gives it none only when a problem stopped it).
fn records_of<'p>(plan: &'p MigrationPlan, blocking: &mut Vec<CheckFinding>) -> Vec<Rec<'p>> {
    let mut out = Vec::new();
    for r in &plan.ranges {
        let Owner::Record {
            entry,
            token,
            heading,
            row,
        } = &r.owner
        else {
            continue;
        };
        if entry.is_empty() {
            blocking.push(CheckFinding::PlanProblem(format!(
                "the record at {} of {} has no entry path",
                lines_text(&r.lines),
                r.source
            )));
            continue;
        }
        out.push(Rec {
            range: r,
            entry,
            token: token.as_deref(),
            heading: heading.as_deref(),
            row: row.as_deref(),
        });
    }
    out
}

/// Item 1: the plan's ranges of each source tile `1..=line_count`, the count taken from the
/// source text itself. A range of a source that is not given is a finding too.
fn partition_item(i: &CheckInput, blocking: &mut Vec<CheckFinding>) {
    let named: BTreeSet<&str> = i.plan.ranges.iter().map(|r| r.source.as_str()).collect();
    for source in named.iter().filter(|s| !i.sources.contains_key(**s)) {
        blocking.push(CheckFinding::SourceMissing(source.to_string()));
    }
    for (source, text) in i.sources {
        let line_count = u32::try_from(text.lines().count()).unwrap_or(u32::MAX);
        let owned: Vec<Lines> = i
            .plan
            .ranges
            .iter()
            .filter(|r| r.source == *source)
            .map(|r| r.lines.clone())
            .collect();
        for problem in partition_problems(line_count, &owned) {
            blocking.push(CheckFinding::Partition {
                source: source.clone(),
                problem,
            });
        }
    }
}

/// The readme of the collection folder: a readme name directly in `target_dir`.
fn is_readme(target_dir: &str, path: &str) -> bool {
    is_folder_index(path) && path.rsplit_once('/').map(|(dir, _)| dir) == Some(target_dir)
}

fn is_markdown(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".md")
}

/// The token of the first heading of a file's body (after its frontmatter).
fn first_heading_token(text: &str) -> Option<String> {
    let body = frontmatter::parse(text).map_or(text, |(_, body)| body);
    body.lines()
        .map(|l| l.trim_end_matches('\r'))
        .find(|l| l.starts_with('#'))
        .and_then(heading_token)
        .map(|(token, _)| token)
}

/// Item 2, over every file of the collection folder except the readme. Each record token of the
/// plan is the first heading of exactly one file. The allowed tokens are the plan's record
/// tokens and the heading token of each planned Whole entry (its record has no token, so its
/// `## IC-10 — …` heading is not extra). A file the plan does not write is extra whatever its
/// heading; in `existing` mode a file unchanged since `HEAD` is an older entry and is not.
fn token_item(
    i: &CheckInput,
    records: &[Rec],
    tokens: &BTreeSet<String>,
    blocking: &mut Vec<CheckFinding>,
) {
    let plan = i.plan;
    let existing_mode = plan.readme_path.is_none();
    let planned: BTreeMap<&str, Option<&str>> =
        records.iter().map(|r| (r.entry, r.token)).collect();
    let heads: Vec<(&str, Option<String>)> = i
        .collection_files
        .iter()
        .filter(|(p, _)| !is_readme(&plan.target_dir, p))
        .map(|(p, text)| {
            let token = if is_markdown(p) {
                first_heading_token(text)
            } else {
                None
            };
            (p.as_str(), token)
        })
        .collect();
    let mut allowed = tokens.clone();
    for (path, token) in &heads {
        if let (Some(None), Some(t)) = (planned.get(path), token) {
            allowed.insert(t.clone());
        }
    }
    let mut count: BTreeMap<&str, usize> = BTreeMap::new();
    for (path, token) in &heads {
        if let Some(t) = token {
            *count.entry(t.as_str()).or_default() += 1;
        }
        let older = existing_mode && i.unchanged_at_head.contains(*path);
        if older {
            continue;
        }
        match token {
            Some(t) if !allowed.contains(t) => blocking.push(CheckFinding::TokenExtra {
                token: t.clone(),
                file: path.to_string(),
            }),
            _ if !planned.contains_key(path) => {
                blocking.push(CheckFinding::ExtraFile(path.to_string()))
            }
            _ => {}
        }
    }
    for t in tokens {
        match count.get(t.as_str()).copied().unwrap_or(0) {
            0 => blocking.push(CheckFinding::TokenMissing(t.clone())),
            1 => {}
            _ => blocking.push(CheckFinding::TokenTwice(t.clone())),
        }
    }
}

/// The lines of a body as the check compares them: `\r` stripped, leading and trailing blank
/// lines left out (as `frontmatter::render_body` writes a body).
fn body_lines(text: &str) -> Vec<&str> {
    let lines: Vec<&str> = text.lines().map(|l| l.trim_end_matches('\r')).collect();
    let start = lines
        .iter()
        .position(|l| !l.trim().is_empty())
        .unwrap_or(lines.len());
    let end = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map_or(start, |e| e + 1);
    lines[start..end].to_vec()
}

/// The 1-indexed line of the first difference, `None` when the two are equal.
fn first_difference(want: &[&str], have: &[&str]) -> Option<usize> {
    if want == have {
        return None;
    }
    let same = want.iter().zip(have).take_while(|(a, b)| a == b).count();
    Some(same + 1)
}

/// Item 3 and the row check. Each planned entry is on disk; its body after the frontmatter
/// equals `expected_body` of its source range at `HEAD`, except a shortened entry, which is
/// listed instead. Returns the shortened entries.
fn body_item(i: &CheckInput, records: &[Rec], blocking: &mut Vec<CheckFinding>) -> Vec<String> {
    let plan = i.plan;
    let mut shortened = Vec::new();
    for r in records {
        let Some(text) = i.collection_files.get(r.entry) else {
            blocking.push(CheckFinding::EntryMissing(r.entry.to_string()));
            continue;
        };
        let Some(source) = i.sources.get(&r.range.source) else {
            continue; // `SourceMissing` already names it.
        };
        if !row_matches_source(i, r, source) {
            blocking.push(CheckFinding::RowDiffers(r.entry.to_string()));
        }
        if plan.shortened.contains(r.entry) {
            shortened.push(r.entry.to_string());
            continue;
        }
        let want = expected_body(source, r.range, &plan.defined, &plan.old_key, &plan.new_key);
        let on_disk = frontmatter::parse(text).map_or(text.as_str(), |(_, body)| body);
        if let Some(first_line) = first_difference(&body_lines(&want), &body_lines(on_disk)) {
            blocking.push(CheckFinding::BodyDiffers {
                entry: r.entry.to_string(),
                first_line,
            });
        }
    }
    shortened
}

/// The plan's index row and synthesised heading of a record against the source at `HEAD`. The
/// row is a line of a non-record section (`Readme` or `Drop`) of its source, whose first cell is
/// the record's token; for a row-only record (an empty range) it is the line at the range's
/// start. A heading is synthesised only for a row-only record: `## TOKEN — <a cell of the row>`.
fn row_matches_source(i: &CheckInput, r: &Rec, source: &str) -> bool {
    let row_only = r.range.lines.is_empty();
    let Some(row) = r.row else {
        return r.heading.is_none();
    };
    let Some(token) = r.token else {
        return false;
    };
    let cells = table_cells(row);
    if cells.first().map(String::as_str) != Some(token) {
        return false;
    }
    let lines: Vec<&str> = source.lines().map(|l| l.trim_end_matches('\r')).collect();
    let in_section = |n: u32| {
        i.plan.ranges.iter().any(|s| {
            s.source == r.range.source
                && matches!(s.owner, Owner::Readme | Owner::Drop)
                && s.lines.contains(&n)
        })
    };
    let is_row_line = |n: u32| {
        n >= 1
            && lines.get(n as usize - 1).copied() == Some(row.trim_end_matches('\r'))
            && in_section(n)
    };
    if row_only {
        let heading_ok = r.heading.is_some_and(|h| {
            cells[1..]
                .iter()
                .any(|c| !c.is_empty() && h == format!("## {token} — {c}"))
        });
        heading_ok && is_row_line(*r.range.lines.start())
    } else {
        r.heading.is_none() && (1..=lines.len() as u32).any(is_row_line)
    }
}

/// Item 3 for the readme: each `Readme` range's `expected_body`, trimmed as the planner trims a
/// readme part, is a run of whole lines of the readme body on disk, each after the one before,
/// in the plan's source order.
fn readme_item(i: &CheckInput, blocking: &mut Vec<CheckFinding>) {
    let plan = i.plan;
    let parts: Vec<&OwnedRange> = plan
        .ranges
        .iter()
        .filter(|r| r.owner == Owner::Readme)
        .collect();
    if parts.is_empty() {
        return;
    }
    let readme = i
        .collection_files
        .iter()
        .find(|(p, _)| is_readme(&plan.target_dir, p))
        .map(|(_, text)| frontmatter::parse(text).map_or(text.as_str(), |(_, body)| body))
        .unwrap_or("");
    let have: Vec<&str> = readme.lines().map(|l| l.trim_end_matches('\r')).collect();
    let mut from = 0usize;
    for r in parts {
        let Some(source) = i.sources.get(&r.source) else {
            continue;
        };
        let part = expected_body(source, r, &plan.defined, &plan.old_key, &plan.new_key);
        let want = body_lines(part.trim_matches('\n'));
        let found = (from..=have.len().saturating_sub(want.len()))
            .find(|&k| have.get(k..k + want.len()) == Some(&want[..]));
        match found {
            Some(k) => from = k + want.len(),
            None => blocking.push(CheckFinding::ReadmeMissing {
                source: r.source.clone(),
                start: *r.lines.start(),
            }),
        }
    }
}

/// Item 4, from each entry file's own frontmatter and path: a terminal entry is archived (under
/// `archive/`, or in an archived collection) and any other entry is live; dates are in order and
/// none is after `today`. An entry that does not parse, or has an undeclared status, is left to
/// the lifecycle item.
fn placement_item(i: &CheckInput, loaded: &LoadedCollection, blocking: &mut Vec<CheckFinding>) {
    let schema = loaded.schema();
    let today = i.today;
    for (path, text) in i.collection_files {
        if !is_markdown(path) || is_readme(&i.plan.target_dir, path) {
            continue;
        }
        let Ok((Some(fm), _)) = frontmatter::parse(text) else {
            continue;
        };
        let Ok(meta) = parse_entry(&fm) else {
            continue;
        };
        let Some(class) = class_of(schema, &meta) else {
            continue;
        };
        let terminal = class == StatusClass::Terminal;
        let archived = matches!(place(path), Some(Ok(p)) if p.archived);
        if terminal != archived {
            blocking.push(CheckFinding::Placement {
                entry: path.clone(),
                terminal,
            });
        }
        let mut dates = |detail: String| {
            blocking.push(CheckFinding::DateOrder {
                entry: path.clone(),
                detail,
            })
        };
        if meta.status_since < meta.created {
            dates(format!(
                "status_since {} is before created {}",
                meta.status_since, meta.created
            ));
        }
        if let Some(closed) = meta.closed {
            if closed < meta.created {
                dates(format!(
                    "closed {closed} is before created {}",
                    meta.created
                ));
            }
            if !terminal {
                dates(format!(
                    "closed {closed} is set, and the status is not terminal"
                ));
            }
        }
        let named = [
            ("created", Some(meta.created)),
            ("status_since", Some(meta.status_since)),
            ("closed", meta.closed),
        ];
        for (field, date) in named {
            if let Some(date) = date.filter(|d| *d > today) {
                dates(format!("{field} {date} is after today {today}"));
            }
        }
    }
}

/// Item 5 and the time-based half of item 8. The collection loads from `target_dir`, and the
/// lifecycle checks report no `entry_invalid`, `collection_invalid` or `orphan_entry`. The
/// time-based findings, and each open entry past `stale_after`, go to `review.time_based`.
fn lifecycle_item(i: &CheckInput, blocking: &mut Vec<CheckFinding>, review: &mut Review) {
    let loaded = match &i.loaded {
        Ok(loaded) => *loaded,
        Err(e) => {
            blocking.push(CheckFinding::Lifecycle(format!(
                "the collection does not load: {e}"
            )));
            return;
        }
    };
    if loaded.dir != i.plan.target_dir {
        blocking.push(CheckFinding::Lifecycle(format!(
            "the collection loaded from {}, not from {}",
            loaded.dir, i.plan.target_dir
        )));
    }
    for finding in check(&loaded.tree, i.today, &CheckPolicy::default()) {
        // No wildcard arm: a new lifecycle check must be put in one of the two groups.
        match finding {
            Finding::EntryInvalid { .. }
            | Finding::CollectionInvalid { .. }
            | Finding::OrphanEntry { .. } => {
                blocking.push(CheckFinding::Lifecycle(finding.to_string()))
            }
            Finding::RevisitDue { .. }
            | Finding::CitedTargetClosed { .. }
            | Finding::StreamAllTerminal { .. }
            | Finding::LedgerQuiet { .. }
            | Finding::TerminalInLiveDir { .. } => review.time_based.push(finding.to_string()),
        }
    }
    let node = loaded.node();
    let Some(days) = node.schema.stale_after_days.filter(|_| !node.archived) else {
        return;
    };
    for e in node.entries.iter().filter(|e| !e.archived) {
        if class_of(&node.schema, &e.meta) != Some(StatusClass::Open) {
            continue;
        }
        let idle = (i.today - e.meta.status_since).num_days();
        if idle > i64::from(days) {
            review.time_based.push(format!(
                "{}: open, and its status_since {} is {idle} days ago, more than stale_after \
                 ({days} days)",
                e.key, e.meta.status_since
            ));
        }
    }
}

/// Item 6, first half: when the key changes, no markdown file of the working tree holds a
/// `[[old-key]]` or `[[old-key:TOKEN]]` citation of this repository.
fn old_key_item(i: &CheckInput, blocking: &mut Vec<CheckFinding>) {
    let plan = i.plan;
    if plan.old_key == plan.new_key {
        return;
    }
    for (file, text) in i.repo_markdown {
        for c in extract(text) {
            let key = match &c.reference {
                Reference::File { repo: None, key }
                | Reference::Entry {
                    repo: None, key, ..
                } => key,
                _ => continue,
            };
            if key.as_str() == plan.old_key {
                blocking.push(CheckFinding::OldKeyCitation {
                    file: file.clone(),
                    line: c.line,
                });
            }
        }
    }
}

fn lines_text(r: &Lines) -> String {
    if r.start() == r.end() {
        format!("line {}", r.start())
    } else {
        format!("lines {}-{}", r.start(), r.end())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::lifecycle::collection::load_collection_dir;
    use crate::librarian::tracker_migration::plan::{OwnedRange, Owner};
    use crate::librarian::tracker_migration::records::Lines;

    const X: &str = "docs/trackers/x.md";
    const H: &str = "docs/trackers/handoff.md";
    const SIDECAR: &str = "docs/augmentations/docs-trackers-x.yaml";
    const DIR: &str = "docs/trackers/ledgers/x";
    const README: &str = "docs/trackers/ledgers/x/readme.md";
    const F1: &str = "docs/trackers/ledgers/x/2026-08-01-first-thing.md";
    const F2: &str = "docs/trackers/ledgers/x/archive/2026-08-02-second-thing.md";
    const R56: &str = "docs/trackers/ledgers/x/2026-08-03-hit-late-split.md";
    const F9: &str = "docs/trackers/ledgers/x/2026-08-04-ninth.md";
    const IC: &str = "docs/trackers/ledgers/x/2026-08-05-cluster-ten.md";

    /// Lines 1-3 frontmatter; 4-12 preamble and index (row F-1 at 10, row-only F-9 at 11);
    /// F-1 13-17; F-2 18-22; R-56b (a `records_extra` heading) 23-26.
    const SOURCE: &str = "\
---
title: X log
---
# X log

Preamble cites F-2.

| ID | Status | Title |
|---|---|---|
| F-1 | open | First |
| F-9 | open | Ninth |

## F-1 — First thing

**Status:** open
See F-2.

## F-2 — Second thing

**Status:** fixed
Body two.

## R-56b — Hit: late split

**Status:** open
Body three.
";

    /// A `whole` source: one record, lines 1-3, whose heading is token-shaped.
    const HANDOFF: &str = "## IC-10 — Cluster ten\n\nBody ic, see F-1.\n";

    // The bodies apply writes, written out by hand: a check that agrees with them agrees with
    // the bytes, not with a second call of the planner's own function.
    const F1_BODY: &str = "## F-1 — First thing\n\n**Status:** open\nSee [[x:F-2]].\n\n**Index row:** | F-1 | open | First |";
    const F2_BODY: &str = "## F-2 — Second thing\n\n**Status:** fixed\nBody two.";
    const R56_BODY: &str = "## R-56b — Hit: late split\n\n**Status:** open\nBody three.";
    const F9_BODY: &str = "## F-9 — Ninth\n\n**Index row:** | F-9 | open | Ninth |";
    const IC_BODY: &str = "## IC-10 — Cluster ten\n\nBody ic, see [[x:F-1]].";
    const README_BODY: &str = "Findings of x.\n\n# X log\n\nPreamble cites [[x:F-2]].\n\n| ID | Status | Title |\n|---|---|---|\n| [[x:F-1]] | open | First |\n| [[x:F-9]] | open | Ninth |";

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 16).unwrap()
    }

    fn entry(title: &str, status: &str, created: &str, closed: Option<&str>, body: &str) -> String {
        let closed = closed.map_or(String::new(), |c| format!("closed: {c}\n"));
        format!(
            "---\nentry_kind: finding\ntitle: '{title}'\nstatus: {status}\nstatus_since: {created}\ncreated: {created}\n{closed}---\n\n{body}\n"
        )
    }

    fn readme(extra: &str, body: &str) -> String {
        format!(
            "---\nkind: collection\ntitle: X\nentry_kinds: [finding]\nstatuses: {{open: open, fixed: terminal}}\ninitial_status: open\n{extra}---\n\n{body}\n"
        )
    }

    fn record(
        source: &str,
        lines: Lines,
        entry: &str,
        token: Option<&str>,
        heading: Option<&str>,
        row: Option<&str>,
    ) -> OwnedRange {
        OwnedRange {
            source: source.into(),
            lines,
            owner: Owner::Record {
                entry: entry.into(),
                token: token.map(Into::into),
                heading: heading.map(Into::into),
                row: row.map(Into::into),
            },
        }
    }

    fn section(source: &str, lines: Lines, owner: Owner) -> OwnedRange {
        OwnedRange {
            source: source.into(),
            lines,
            owner,
        }
    }

    /// The plan a correct planner makes for [`SOURCE`] and [`HANDOFF`], built by hand.
    fn plan() -> MigrationPlan {
        MigrationPlan {
            target_dir: DIR.into(),
            readme_path: Some(README.into()),
            writes: BTreeMap::new(),
            deletes: [X, H, SIDECAR].iter().map(|s| s.to_string()).collect(),
            ranges: vec![
                section(X, 1..=3, Owner::Frontmatter),
                section(X, 4..=12, Owner::Readme),
                record(
                    X,
                    // Empty, starting at the row line: a row-only record owns no line.
                    Lines::new(11, 10),
                    F9,
                    Some("F-9"),
                    Some("## F-9 — Ninth"),
                    Some("| F-9 | open | Ninth |"),
                ),
                record(
                    X,
                    13..=17,
                    F1,
                    Some("F-1"),
                    None,
                    Some("| F-1 | open | First |"),
                ),
                record(X, 18..=22, F2, Some("F-2"), None, None),
                record(X, 23..=26, R56, Some("R-56b"), None, None),
                record(H, 1..=3, IC, None, None, None),
            ],
            shortened: BTreeSet::new(),
            defined: ["F-1", "F-2", "F-9", "R-56b"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            old_key: "x".into(),
            new_key: "x".into(),
            review: Review::default(),
            problems: Vec::new(),
        }
    }

    /// Everything a check reads, for a collection that `apply` wrote correctly.
    struct Fx {
        plan: MigrationPlan,
        sources: BTreeMap<String, String>,
        files: BTreeMap<String, String>,
        repo_md: BTreeMap<String, String>,
        present: BTreeSet<String>,
        unchanged: BTreeSet<String>,
        hot: Option<Result<(), String>>,
    }

    fn fx() -> Fx {
        let files = [
            (README, readme("", README_BODY)),
            (
                F1,
                entry("First thing", "open", "2026-08-01", None, F1_BODY),
            ),
            (
                F2,
                entry(
                    "Second thing",
                    "fixed",
                    "2026-08-02",
                    Some("2026-09-01"),
                    F2_BODY,
                ),
            ),
            (
                R56,
                entry("Hit: late split", "open", "2026-08-03", None, R56_BODY),
            ),
            (F9, entry("Ninth", "open", "2026-08-04", None, F9_BODY)),
            (
                IC,
                entry("Cluster ten", "open", "2026-08-05", None, IC_BODY),
            ),
        ];
        Fx {
            plan: plan(),
            sources: [(X, SOURCE), (H, HANDOFF)]
                .iter()
                .map(|(p, t)| (p.to_string(), t.to_string()))
                .collect(),
            files: files.into_iter().map(|(p, t)| (p.to_string(), t)).collect(),
            repo_md: BTreeMap::new(),
            present: BTreeSet::new(),
            unchanged: BTreeSet::new(),
            hot: None,
        }
    }

    /// Write the collection files to a temporary root, load the collection from its folder as
    /// the CLI does, and run the check.
    fn run(f: &Fx) -> CheckReport {
        let t = tempfile::tempdir().unwrap();
        for (rel, text) in &f.files {
            let path = t.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let loaded = load_collection_dir(t.path(), &f.plan.target_dir).map_err(|e| e.to_string());
        let input = CheckInput {
            plan: &f.plan,
            sources: &f.sources,
            collection_files: &f.files,
            repo_markdown: &f.repo_md,
            still_present: &f.present,
            unchanged_at_head: &f.unchanged,
            loaded: loaded.as_ref().map_err(Clone::clone),
            hot: f.hot.clone(),
            today: today(),
        };
        check_migration(&input)
    }

    fn set_body(f: &mut Fx, path: &str, from: &str, to: &str) {
        let text = f.files[path].replace(from, to);
        assert_ne!(text, f.files[path], "the fixture holds {from:?}");
        f.files.insert(path.into(), text);
    }

    fn range_of<'a>(p: &'a mut MigrationPlan, entry_path: &str) -> &'a mut OwnedRange {
        p.ranges
            .iter_mut()
            .find(|r| matches!(&r.owner, Owner::Record { entry, .. } if entry == entry_path))
            .unwrap()
    }

    /// The baseline: a faithful collection is clean, with nothing shortened and nothing
    /// time-based. Every faulty-plan test below changes one thing of it. Breaks if any item
    /// misreads a correct collection (a token regex without the `R-56b` suffix, a Whole entry's
    /// `IC-10` heading taken as extra, a CRLF or blank-line difference taken as a body change).
    #[test]
    fn a_faithful_collection_passes() {
        let r = run(&fx());
        assert_eq!(r.blocking, vec![], "{:#?}", r.blocking);
        assert!(r.shortened.is_empty());
        assert!(r.review.time_based.is_empty(), "{:?}", r.review.time_based);
    }

    /// (a) The planner wrote F-1 from a body that lacks line 16. The plan's `writes` hold that
    /// faulty text and the disk holds it too: only a body recomputed from the source finds it.
    /// Breaks if the body item compares with `plan.writes`, or is skipped.
    #[test]
    fn a_plan_whose_write_lacks_a_line_fails_check() {
        let mut f = fx();
        set_body(&mut f, F1, "See [[x:F-2]].\n", "");
        f.plan.writes.insert(F1.into(), f.files[F1].clone());
        let r = run(&f);
        assert_eq!(
            r.blocking,
            vec![CheckFinding::BodyDiffers {
                entry: F1.into(),
                first_line: 4
            }]
        );
    }

    /// (b) The plan's F-1 range stops at line 15 and the entry was written from that range, so
    /// the body item agrees with the disk. Only the partition, which counts the source lines
    /// itself, sees lines 16-17 with no owner. Breaks if the partition item uses the plan's
    /// own idea of the line count, or is skipped.
    #[test]
    fn a_plan_with_a_gap_in_its_ranges_fails_the_partition() {
        let mut f = fx();
        range_of(&mut f.plan, F1).lines = 13..=15;
        set_body(&mut f, F1, "See [[x:F-2]].\n", "");
        let r = run(&f);
        assert_eq!(
            r.blocking,
            vec![CheckFinding::Partition {
                source: X.into(),
                problem: PartitionProblem::Uncovered(16..=17),
            }]
        );
    }

    /// (c) A disk mutant: a line deleted from an entry apply wrote correctly. Breaks if the
    /// body item does not re-read each entry from disk.
    #[test]
    fn a_line_deleted_from_a_written_entry_fails_check() {
        let mut f = fx();
        set_body(&mut f, F2, "**Status:** fixed\n", "");
        let r = run(&f);
        assert_eq!(
            r.blocking,
            vec![CheckFinding::BodyDiffers {
                entry: F2.into(),
                first_line: 3
            }]
        );
    }

    /// (d) An entry file the plan does not write, with a token no source defines. Breaks if the
    /// token item reads only the planned entries instead of every collection file.
    #[test]
    fn an_extra_entry_file_fails_tokens() {
        let mut f = fx();
        let extra = format!("{DIR}/2026-08-06-third.md");
        f.files.insert(
            extra.clone(),
            entry(
                "Third",
                "open",
                "2026-08-06",
                None,
                "## F-3 — Third\n\nBody.",
            ),
        );
        let r = run(&f);
        assert_eq!(
            r.blocking,
            vec![CheckFinding::TokenExtra {
                token: "F-3".into(),
                file: extra
            }]
        );
    }

    /// An extra file with no token heading, and a second copy of the Whole entry (a token the
    /// plan allows, at a path it does not write): both are extra files. Breaks if a file the
    /// plan does not write passes because its heading is allowed or absent.
    #[test]
    fn a_file_the_plan_does_not_write_is_blocking_whatever_its_heading() {
        let mut f = fx();
        let plain = format!("{DIR}/2026-08-07-notes.md");
        let copy = format!("{DIR}/2026-08-08-cluster-copy.md");
        f.files.insert(
            plain.clone(),
            entry("Notes", "open", "2026-08-07", None, "Some notes."),
        );
        f.files.insert(copy.clone(), f.files[IC].clone());
        let r = run(&f);
        assert_eq!(
            r.blocking,
            vec![
                CheckFinding::ExtraFile(plain),
                CheckFinding::ExtraFile(copy)
            ]
        );
    }

    /// (e) `R-56b` is found with its suffix: the faithful collection passes (baseline), and an
    /// entry whose heading lost the suffix leaves `R-56b` missing and `R-56` extra. Breaks if
    /// the token item reads headings with a pattern that drops the letter suffix.
    #[test]
    fn a_records_extra_token_with_a_suffix_is_found() {
        let mut f = fx();
        set_body(&mut f, R56, "## R-56b — Hit", "## R-56 — Hit");
        let r = run(&f);
        assert!(
            r.blocking
                .contains(&CheckFinding::TokenMissing("R-56b".into())),
            "{:#?}",
            r.blocking
        );
        assert!(r.blocking.contains(&CheckFinding::TokenExtra {
            token: "R-56".into(),
            file: R56.into()
        }));
    }

    /// (f) The readme lacks the preamble line of the section at line 4. Breaks if the readme
    /// item is skipped or tests a part with `contains` of a fragment.
    #[test]
    fn a_readme_section_missing_from_the_readme_fails() {
        let mut f = fx();
        set_body(&mut f, README, "Preamble cites [[x:F-2]].\n\n", "");
        let r = run(&f);
        assert_eq!(
            r.blocking,
            vec![CheckFinding::ReadmeMissing {
                source: X.into(),
                start: 4
            }]
        );
    }

    /// (g) The planner placed the terminal F-2 in the live folder, and apply wrote it there.
    /// The body and token items agree; only placement fails. The lifecycle check's
    /// `terminal_in_live_dir` is time-based and goes to review. Breaks if placement is
    /// skipped, or read from the plan instead of the entry's own frontmatter and path.
    #[test]
    fn a_terminal_entry_moved_live_fails_placement() {
        let mut f = fx();
        let live = format!("{DIR}/2026-08-02-second-thing.md");
        let text = f.files.remove(F2).unwrap();
        f.files.insert(live.clone(), text);
        if let Owner::Record { entry, .. } = &mut range_of(&mut f.plan, F2).owner {
            *entry = live.clone();
        }
        let r = run(&f);
        assert_eq!(
            r.blocking,
            vec![CheckFinding::Placement {
                entry: live,
                terminal: true
            }]
        );
        assert!(
            r.review
                .time_based
                .iter()
                .any(|l| l.contains("terminal_in_live_dir")),
            "{:?}",
            r.review.time_based
        );
    }

    /// A non-terminal entry in `archive/` is a placement finding too.
    #[test]
    fn an_open_entry_in_archive_fails_placement() {
        let mut f = fx();
        let archived = format!("{DIR}/archive/2026-08-01-first-thing.md");
        let text = f.files.remove(F1).unwrap();
        f.files.insert(archived.clone(), text);
        if let Owner::Record { entry, .. } = &mut range_of(&mut f.plan, F1).owner {
            *entry = archived.clone();
        }
        let r = run(&f);
        assert!(
            r.blocking.contains(&CheckFinding::Placement {
                entry: archived,
                terminal: false
            }),
            "{:#?}",
            r.blocking
        );
    }

    /// (h) The key changed from `old-x` to `x`, and a note still cites `[[old-x:F-1]]` and
    /// `[[old-x]]` on line 3. A citation of the new key, one qualified by another repo, and one
    /// in a code span are not old-key citations. Breaks if the old-key item is skipped, reads
    /// only the collection, or counts repo-qualified citations.
    #[test]
    fn an_old_key_citation_left_in_another_file_fails() {
        let mut f = fx();
        f.plan.old_key = "old-x".into();
        f.repo_md.insert(
            "docs/notes.md".into(),
            "# Notes\n\nSee [[old-x:F-1]] and [[old-x]].\nAlso [[x:F-1]], [[other/old-x:F-1]] and `[[old-x:F-2]]`.\n".into(),
        );
        let r = run(&f);
        let at = |line| CheckFinding::OldKeyCitation {
            file: "docs/notes.md".into(),
            line,
        };
        assert_eq!(r.blocking, vec![at(3), at(3)]);
    }

    /// When the key does not change, the working tree is not searched for old-key citations
    /// (the old key is the new key, and every citation of it resolves).
    #[test]
    fn an_unchanged_key_cites_nothing_old() {
        let mut f = fx();
        f.repo_md
            .insert("docs/notes.md".into(), "See [[x:F-1]].\n".into());
        assert_eq!(run(&f).blocking, vec![]);
    }

    /// (i, unit half) A hot view that does not render is blocking. The CLI half, with the same
    /// key under `ledgers/` and `streams/`, is in `cli/trackers/tests.rs`. Breaks if the hot
    /// item ignores `hot`.
    #[test]
    fn a_hot_view_that_does_not_render_is_blocking() {
        let mut f = fx();
        f.hot = Some(Err("the key x names 2 collections".into()));
        assert_eq!(
            run(&f).blocking,
            vec![CheckFinding::HotView(
                "the key x names 2 collections".into()
            )]
        );
        f.hot = Some(Ok(()));
        assert_eq!(run(&f).blocking, vec![]);
    }

    /// (j) A CRLF source at HEAD checks like the LF one against the LF entries apply wrote.
    /// Breaks if a `\r` reaches the line count, a body line or a row compare.
    #[test]
    fn a_crlf_source_passes() {
        let mut f = fx();
        for text in f.sources.values_mut() {
            *text = text.replace('\n', "\r\n");
        }
        let r = run(&f);
        assert_eq!(r.blocking, vec![], "{:#?}", r.blocking);
    }

    /// (k) With `stale_after: 30d` every open entry is stale on 2026-11-16, and with
    /// `review_every: 7d` the ledger is quiet: that is review, and the check stays clean.
    /// Breaks if a time-based finding is put in `blocking`.
    #[test]
    fn time_based_findings_are_review_not_blocking() {
        let mut f = fx();
        f.files.insert(
            README.into(),
            readme("stale_after: 30d\nreview_every: 7d\n", README_BODY),
        );
        let r = run(&f);
        assert_eq!(r.blocking, vec![], "{:#?}", r.blocking);
        let lines = &r.review.time_based;
        assert!(lines.iter().any(|l| l.contains("stale_after")), "{lines:?}");
        assert!(
            lines.iter().any(|l| l.contains("ledger_quiet")),
            "{lines:?}"
        );
    }

    /// (l) The Whole entry's heading `## IC-10 — …` is not an extra token, but that does not
    /// allow another token-shaped heading in a file the plan does not write. Breaks if the
    /// allowed tokens leave out the heading token of a planned Whole entry (the baseline
    /// fails), or allow every heading.
    #[test]
    fn a_whole_entry_with_a_token_shaped_heading_passes() {
        let r = run(&fx());
        assert!(
            !r.blocking
                .iter()
                .any(|b| matches!(b, CheckFinding::TokenExtra { .. })),
            "{:#?}",
            r.blocking
        );
        let mut g = fx();
        let other = format!("{DIR}/2026-08-09-other.md");
        g.files.insert(
            other.clone(),
            entry("Other", "open", "2026-08-09", None, "## IC-11 — Other"),
        );
        assert_eq!(
            run(&g).blocking,
            vec![CheckFinding::TokenExtra {
                token: "IC-11".into(),
                file: other
            }]
        );
    }

    /// Date order is read from each entry's frontmatter on disk. Breaks if the date item is
    /// skipped.
    #[test]
    fn a_status_since_before_created_fails_date_order() {
        let mut f = fx();
        set_body(
            &mut f,
            F1,
            "status_since: 2026-08-01",
            "status_since: 2026-07-01",
        );
        let r = run(&f);
        assert_eq!(
            r.blocking,
            vec![CheckFinding::DateOrder {
                entry: F1.into(),
                detail: "status_since 2026-07-01 is before created 2026-08-01".into()
            }]
        );
    }

    /// A date after today is a date-order finding.
    #[test]
    fn a_created_date_after_today_fails_date_order() {
        let mut f = fx();
        set_body(&mut f, R56, "2026-08-03", "2026-12-03");
        let r = run(&f);
        assert!(
            r.blocking.contains(&CheckFinding::DateOrder {
                entry: R56.into(),
                detail: "created 2026-12-03 is after today 2026-11-16".into()
            }),
            "{:#?}",
            r.blocking
        );
    }

    /// An entry with an undeclared status gives an `entry_invalid` lifecycle finding. Breaks if
    /// the lifecycle item is skipped or puts `entry_invalid` in review.
    #[test]
    fn an_entry_with_an_undeclared_status_fails_the_lifecycle_item() {
        let mut f = fx();
        set_body(&mut f, R56, "status: open", "status: weird");
        let r = run(&f);
        assert!(!r.blocking.is_empty());
        assert!(
            r.blocking
                .iter()
                .all(|b| matches!(b, CheckFinding::Lifecycle(l) if l.contains("entry_invalid"))),
            "{:#?}",
            r.blocking
        );
    }

    /// A collection that does not load is a lifecycle finding.
    #[test]
    fn a_collection_that_does_not_load_fails_the_lifecycle_item() {
        let mut f = fx();
        f.files.insert(README.into(), "no frontmatter\n".into());
        let r = run(&f);
        assert!(
            r.blocking
                .iter()
                .any(|b| matches!(b, CheckFinding::Lifecycle(_))),
            "{:#?}",
            r.blocking
        );
    }

    /// A retired sidecar and a source still on disk. Breaks if the item is skipped, or tells
    /// a source from a sidecar wrongly.
    #[test]
    fn a_retired_sidecar_or_a_source_left_on_disk_fails() {
        let mut f = fx();
        f.present = [SIDECAR, X].iter().map(|s| s.to_string()).collect();
        assert_eq!(
            run(&f).blocking,
            vec![
                CheckFinding::SidecarLeft(SIDECAR.into()),
                CheckFinding::SourceLeft(X.into())
            ]
        );
    }

    /// The plan's F-1 row text is not the source line (a typo), and the entry was written with
    /// it, so the body agrees; and the row-only F-9 got a heading whose title is no cell of its
    /// row. Breaks if the row text and the synthesised heading are not checked against the
    /// source at HEAD.
    #[test]
    fn a_plan_row_or_heading_that_is_not_the_source_fails() {
        let mut f = fx();
        if let Owner::Record { row, .. } = &mut range_of(&mut f.plan, F1).owner {
            *row = Some("| F-1 | open | Frist |".into());
        }
        set_body(&mut f, F1, "| First |", "| Frist |");
        if let Owner::Record { heading, .. } = &mut range_of(&mut f.plan, F9).owner {
            *heading = Some("## F-9 — Nine".into());
        }
        set_body(&mut f, F9, "## F-9 — Ninth", "## F-9 — Nine");
        assert_eq!(
            run(&f).blocking,
            vec![
                CheckFinding::RowDiffers(F9.into()),
                CheckFinding::RowDiffers(F1.into())
            ]
        );
    }

    /// A shortened entry is not compared and is listed. Breaks if `plan.shortened` is ignored
    /// (a body finding) or not reported.
    #[test]
    fn a_shortened_entry_is_listed_not_compared() {
        let mut f = fx();
        f.plan.shortened.insert(F1.into());
        set_body(&mut f, F1, "**Status:** open\nSee [[x:F-2]].\n", "Short.\n");
        let r = run(&f);
        assert_eq!(r.blocking, vec![]);
        assert_eq!(r.shortened, vec![F1.to_string()]);
    }

    /// A planned entry that is not on disk. Breaks if a missing entry is skipped by the body
    /// item.
    #[test]
    fn a_missing_entry_fails() {
        let mut f = fx();
        f.files.remove(F1);
        let r = run(&f);
        assert_eq!(
            r.blocking,
            vec![
                CheckFinding::TokenMissing("F-1".into()),
                CheckFinding::EntryMissing(F1.into())
            ]
        );
    }

    /// In `existing` mode an entry the collection had before the run, unchanged since HEAD, is
    /// not extra; the same file changed on disk is. Breaks if existing mode flags every older
    /// entry, or lets a changed one pass.
    #[test]
    fn an_older_entry_of_an_existing_collection_is_not_extra() {
        let mut f = fx();
        f.plan.readme_path = None;
        let older = format!("{DIR}/2026-07-01-older.md");
        f.files.insert(
            older.clone(),
            entry("Older", "open", "2026-07-01", None, "## F-7 — Older"),
        );
        f.unchanged.insert(older.clone());
        assert_eq!(run(&f).blocking, vec![]);
        f.unchanged.clear();
        assert_eq!(
            run(&f).blocking,
            vec![CheckFinding::TokenExtra {
                token: "F-7".into(),
                file: older
            }]
        );
    }

    /// `plan.defined` lacks F-2, a token of its records. Breaks if the defined set is trusted.
    #[test]
    fn a_defined_set_that_lacks_a_record_token_fails() {
        let mut f = fx();
        f.plan.defined.remove("F-2");
        let r = run(&f);
        assert!(
            r.blocking
                .contains(&CheckFinding::DefinedDiffers("F-2".into())),
            "{:#?}",
            r.blocking
        );
    }

    /// A plan with a problem is blocking.
    #[test]
    fn a_plan_problem_is_blocking() {
        let mut f = fx();
        f.plan.problems.push("token F-1 is defined twice".into());
        assert_eq!(
            run(&f).blocking,
            vec![CheckFinding::PlanProblem(
                "token F-1 is defined twice".into()
            )]
        );
    }

    /// Every finding prints on one line.
    #[test]
    fn every_finding_text_is_one_line() {
        let all = vec![
            CheckFinding::Partition {
                source: X.into(),
                problem: PartitionProblem::Overlap { a: 1..=4, b: 3..=5 },
            },
            CheckFinding::BodyDiffers {
                entry: F1.into(),
                first_line: 2,
            },
            CheckFinding::Lifecycle("a\nb".into()),
            CheckFinding::HotView("x\ny".into()),
        ];
        for f in all {
            let t = f.text();
            assert!(!t.contains('\n'), "{t}");
        }
    }
}
