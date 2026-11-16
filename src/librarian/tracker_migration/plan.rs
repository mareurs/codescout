//! The deterministic migration planner (Plan L-E1 Task 8).
//!
//! Spec: `docs/superpowers/specs/2026-11-15-tracker-migration-design.md` § What apply writes,
//! § Re-runs and late records, § Dates, § Citations inside records, § Index rows,
//! § Preconditions.
//!
//! [`plan_migration`] is a pure function of its inputs. It decides every file `apply` writes and
//! deletes, and the owner of every source line. Every entry body comes from [`expected_body`] over
//! the source text and a line range, so `check` recomputes each body from the source at `HEAD`
//! with the same function and does not trust the texts of the plan. `problems` empty is the only
//! state in which `apply` may write.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;

use chrono::NaiveDate;
use regex::Regex;
use serde::Serialize;
use serde_json::Value;

use super::dates::{derive_dates, DateInputs};
use super::index_rows::{
    index_tables, normalise_status, record_facts, Facts, IndexRow, IndexTable,
};
use super::mapping::{
    missing_section_message, EntryKindRule, Mapping, Override, SectionSpec, SidecarSpec,
    StatusSource,
};
use super::records::{
    partition_problems, slice, split_source, ExtraRecord, Lines, PartitionProblem, SourceKind,
    Split,
};
use super::rewrite::rewrite_record_citations;
use crate::librarian::augmentation_sidecar::rel_path_for;
use crate::librarian::frontmatter;
use crate::librarian::lifecycle::collection::collection_readme;
use crate::librarian::lifecycle::layout::{place, Lifecycle, ARCHIVE_DIR, TRACKERS_DIR};
use crate::librarian::lifecycle::naming::{archive_rel_path, entry_stem};
use crate::librarian::lifecycle::schema::{parse_schema, CollectionSchema, FieldType, StatusClass};
use crate::librarian::lifecycle::write::{
    entry_template, render_entry, NewEntry, RenderRules, WriteProblem,
};

/// One source file at `HEAD`, as the CLI read it. A `files` source arrives as one `Whole` source
/// per markdown file, with `fixed_stem` set to its lowercase file stem (plan P-R2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceText {
    pub path: String,
    pub kind: SourceKind,
    pub text: String,
    /// `blame_dates` of the file: index `i` is line `i + 1`.
    pub blame: Vec<NaiveDate>,
    pub fixed_stem: Option<String>,
}

/// An entry of the existing collection (`target.existing = true`), read from the working tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExistingEntry {
    pub rel_path: String,
    pub text: String,
    /// The token of its first heading.
    pub token: Option<String>,
}

pub struct MigrationInput<'a> {
    pub mapping: &'a Mapping,
    pub sources: Vec<SourceText>,
    /// Every other markdown file (repo-relative path to text), for the key-change rewrite.
    pub other_markdown: &'a BTreeMap<String, String>,
    /// Each key in the repo to the paths that have it, untracked `.md` files under `docs/`
    /// included, so the entries of an earlier run are seen with their paths.
    pub key_paths: &'a BTreeMap<String, BTreeSet<String>>,
    /// `target.existing`: the schema, the entry template and the entries of the collection.
    pub existing: Option<(&'a CollectionSchema, Option<&'a str>, Vec<ExistingEntry>)>,
    pub now: NaiveDate,
}

/// Who owns a range of source lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Owner {
    /// A record carried into the entry at `entry` (repo-relative). `heading` is the synthesised
    /// heading of a row-only record (its range is empty); `row` is the index row, carried
    /// verbatim as the last body line.
    Record {
        entry: String,
        token: Option<String>,
        heading: Option<String>,
        row: Option<String>,
    },
    Readme,
    Drop,
    Frontmatter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OwnedRange {
    pub source: String,
    pub lines: Lines,
    pub owner: Owner,
}

/// Review material: listed for the user, never blocking.
#[derive(Debug, Default, Clone, Serialize)]
pub struct Review {
    pub derived_dates: Vec<String>,
    pub dropped: Vec<String>,
    pub skipped_rewrites: Vec<String>,
    pub row_only: Vec<String>,
    pub no_row: Vec<String>,
    pub disagreements: Vec<String>,
    pub path_mentions: Vec<String>,
    pub time_based: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MigrationPlan {
    /// The collection folder, repo-relative.
    pub target_dir: String,
    /// `None` in `existing` mode: the readme is not written.
    pub readme_path: Option<String>,
    pub writes: BTreeMap<String, String>,
    pub deletes: BTreeSet<String>,
    /// Every owned line range of every source, sorted by source and first line.
    pub ranges: Vec<OwnedRange>,
    /// The entries (paths) whose body is the `shorten` replacement.
    pub shortened: BTreeSet<String>,
    /// The tokens the sources define (row-only records included).
    pub defined: BTreeSet<String>,
    pub old_key: String,
    pub new_key: String,
    pub review: Review,
    pub problems: Vec<String>,
}

/// Plan a migration. See the module documentation; the steps are those of plan L-E1 Task 8.
pub fn plan_migration(input: &MigrationInput) -> MigrationPlan {
    let m = input.mapping;
    let sources = &input.sources;
    let mut problems: Vec<String> = Vec::new();
    let mut review = Review::default();
    let old_key = m
        .sources
        .first()
        .map(|s| path_stem(&s.path))
        .unwrap_or_default();
    let new_key = m.target.key.clone().unwrap_or_else(|| old_key.clone());
    let lifecycle = m.target.lifecycle.as_str();
    let target_dir = if m.target.archived {
        format!("{TRACKERS_DIR}/{lifecycle}/{ARCHIVE_DIR}/{new_key}")
    } else {
        format!("{TRACKERS_DIR}/{lifecycle}/{new_key}")
    };
    let readme_path = format!("{target_dir}/readme.md");
    let source_paths: BTreeSet<&str> = sources.iter().map(|s| s.path.as_str()).collect();

    // Step 1: split each source with the extras, and read the index tables.
    let splits: Vec<Split> = sources
        .iter()
        .map(|s| {
            let extra: Vec<ExtraRecord> = m
                .records_extra
                .iter()
                .filter(|e| e.source == s.path)
                .map(|e| ExtraRecord {
                    heading_line: e.line,
                    token: e.token.clone(),
                })
                .collect();
            split_source(&s.text, &s.kind, &extra)
        })
        .collect();
    let tables: Vec<Vec<IndexTable>> = sources
        .iter()
        .zip(&splits)
        .map(|(s, sp)| index_tables(&s.text, sp))
        .collect();
    let rows = choose_rows(m, sources, &tables, &mut problems);

    // Step 2: repeated tokens and unsettled possible records.
    let defined_at = definitions(sources, &splits, &mut problems);
    possible_problems(m, sources, &splits, &mut problems);

    let mut items: Vec<Item> = Vec::new();
    for (i, (s, sp)) in sources.iter().zip(&splits).enumerate() {
        for rec in &sp.records {
            let name = rec
                .token
                .clone()
                .or_else(|| s.fixed_stem.clone())
                .unwrap_or_else(|| rec.title.clone());
            items.push(Item {
                src: i,
                token: rec.token.clone(),
                name,
                title: rec.title.clone(),
                heading_line: rec.heading_line,
                lines: rec.lines.clone(),
                heading: None,
                row: rec
                    .token
                    .as_ref()
                    .and_then(|t| rows.get(t))
                    .map(|(_, r)| *r),
                facts: record_facts(&s.text, rec),
            });
        }
    }
    let mut row_only: Vec<(usize, &IndexRow)> = rows
        .iter()
        .filter(|(t, _)| !defined_at.contains_key(*t))
        .map(|(_, v)| *v)
        .collect();
    row_only.sort_by_key(|(i, r)| (*i, r.line));
    let title_column = m
        .row_title_column
        .as_deref()
        .map(|c| c.trim().to_lowercase());
    for (i, row) in row_only {
        review.row_only.push(row.token.clone());
        let cell = title_column
            .as_ref()
            .and_then(|c| row.cells.get(c))
            .filter(|c| !c.is_empty());
        let Some(cell) = cell else {
            problems.push(format!(
                "index row {} at line {} of {} has no record heading and no title cell. Set row_title_column.",
                row.token, row.line, sources[i].path
            ));
            continue;
        };
        items.push(Item {
            src: i,
            token: Some(row.token.clone()),
            name: row.token.clone(),
            title: cell.clone(),
            heading_line: row.line,
            // Empty: a row-only record owns no source line; its row stays in its section.
            lines: row.line..=row.line - 1,
            heading: Some(format!("## {} — {cell}", row.token)),
            row: Some(row),
            facts: no_facts(),
        });
    }
    let defined: BTreeSet<String> = items.iter().filter_map(|it| it.token.clone()).collect();

    // Section owners. A section with no `sections` entry gets no owner (decision 2).
    let existing_mode = input.existing.is_some();
    let mut ranges: Vec<(usize, OwnedRange)> = Vec::new();
    for (i, (s, sp)) in sources.iter().zip(&splits).enumerate() {
        let range = |lines: &Lines, owner: Owner| {
            (
                i,
                OwnedRange {
                    source: s.path.clone(),
                    lines: lines.clone(),
                    owner,
                },
            )
        };
        if let Some(fm) = &sp.frontmatter {
            ranges.push(range(fm, Owner::Frontmatter));
        }
        for x in &sp.sections {
            let Some(spec) = section_spec(m, &s.path, x.start) else {
                problems.push(missing_section_message(&s.path, x.start, &x.first_line));
                continue;
            };
            let owner = match spec.to.as_str() {
                "readme" if existing_mode => {
                    problems.push(format!(
                        "the section at line {} of {} goes to readme, but target.existing is true and the readme is not written. Set its sections entry to drop.",
                        x.start, s.path
                    ));
                    continue;
                }
                "readme" => Owner::Readme,
                "drop" => {
                    review.dropped.push(format!(
                        "{} {} (\"{}\")",
                        s.path,
                        lines_text(&x.lines),
                        x.first_line
                    ));
                    Owner::Drop
                }
                other => {
                    problems.push(format!(
                        "the sections entry at line {} of {} has to = \"{other}\". Use readme or drop.",
                        x.start, s.path
                    ));
                    continue;
                }
            };
            ranges.push(range(&x.lines, owner));
        }
    }
    for spec in &m.sections {
        let named = sources.iter().zip(&splits).any(|(s, sp)| {
            s.path == spec.source && sp.sections.iter().any(|x| x.start == spec.start)
        });
        if !named {
            problems.push(format!(
                "the sections entry at line {} of {} names no section",
                spec.start, spec.source
            ));
        }
    }

    // Step 6: the schema, and the readme of a new collection.
    let mut readme_text: Option<String> = None;
    let (schema, template): (Option<CollectionSchema>, Option<String>) = match &input.existing {
        Some((schema, template, _)) => (Some((*schema).clone()), template.map(str::to_string)),
        None => {
            let mut parts: Vec<String> = m
                .readme_body
                .iter()
                .map(|b| b.trim_matches('\n').to_string())
                .filter(|b| !b.trim().is_empty())
                .collect();
            let mut readme_ranges: Vec<&(usize, OwnedRange)> = ranges
                .iter()
                .filter(|(_, r)| r.owner == Owner::Readme)
                .collect();
            readme_ranges.sort_by_key(|(i, r)| (*i, *r.lines.start()));
            for (i, r) in readme_ranges {
                let text = &sources[*i].text;
                let name = format!("{} line {}", r.source, r.lines.start());
                note_skips(
                    &mut review,
                    &name,
                    text,
                    &r.lines,
                    &defined,
                    &old_key,
                    &new_key,
                );
                let part = expected_body(text, r, &defined, &old_key, &new_key);
                parts.push(part.trim_matches('\n').to_string());
            }
            let body = parts.join("\n\n");
            match new_readme(m, &new_key, &body) {
                Ok((text, schema)) => {
                    readme_text = Some(text);
                    (Some(schema), entry_template(&body))
                }
                Err(found) => {
                    problems.extend(found);
                    (None, None)
                }
            }
        }
    };
    if let Some(schema) = &schema {
        declared_problems(m, schema, &mut problems);
    }

    // Steps 3-5: status, dates, fields, title, key and place of every record.
    let mut writes: BTreeMap<String, String> = BTreeMap::new();
    let mut shortened: BTreeSet<String> = BTreeSet::new();
    let mut planned_keys: BTreeMap<String, String> = BTreeMap::new();
    let existing_entries: &[ExistingEntry] = input
        .existing
        .as_ref()
        .map_or(&[], |(_, _, entries)| entries.as_slice());
    for it in &items {
        let s = &sources[it.src];
        let n = it.name.as_str();
        let ov = m.overrides.get(n);
        let row = it.row;
        if let Some(t) = &it.token {
            if row.is_none() {
                review.no_row.push(t.clone());
            }
            let column = match &m.status_source {
                StatusSource::IndexColumn(h) => h.trim().to_lowercase(),
                _ => "status".to_string(),
            };
            let body_status = it
                .facts
                .raw_status_body
                .as_deref()
                .and_then(normalise_status);
            let index_status = row
                .and_then(|r| r.cells.get(&column))
                .and_then(|c| normalise_status(c));
            if let (Some(b), Some(x)) = (body_status, index_status) {
                if b != x {
                    review.disagreements.push(format!(
                        "{t}: the body says \"{b}\", the index says \"{x}\""
                    ));
                }
            }
        }
        let mut range = OwnedRange {
            source: s.path.clone(),
            lines: it.lines.clone(),
            owner: Owner::Record {
                entry: String::new(),
                token: it.token.clone(),
                heading: it.heading.clone(),
                row: row.map(|r| r.raw.clone()),
            },
        };
        note_skips(
            &mut review,
            n,
            &s.text,
            &it.lines,
            &defined,
            &old_key,
            &new_key,
        );
        let short = m.shorten.get(n);
        let body = match short {
            Some(replacement) => replacement.clone(),
            None => expected_body(&s.text, &range, &defined, &old_key, &new_key),
        };

        let entry: Option<String> = 'entry: {
            if let Some(t) = &it.token {
                let found = existing_entries
                    .iter()
                    .find(|e| e.token.as_deref() == Some(t.as_str()));
                if let Some(e) = found {
                    if same_body(&e.text, &body) {
                        break 'entry Some(e.rel_path.clone());
                    }
                    problems.push(format!(
                        "{t}: entry {} exists and its body is not the body of the record at line {} of {}. Make the two equal, or remove the entry.",
                        e.rel_path, it.heading_line, s.path
                    ));
                    break 'entry None;
                }
            }
            let status = match status_of(m, n, ov, &it.facts, row) {
                Ok(status) => status,
                Err(msg) => {
                    problems.push(msg);
                    break 'entry None;
                }
            };
            let Some(schema) = &schema else {
                break 'entry None;
            };
            // An undeclared status or kind is already a problem that names its mapping key.
            let Some(class) = schema.statuses.get(&status).copied() else {
                break 'entry None;
            };
            let Some(kind) = entry_kind_of(m, it, &mut problems) else {
                break 'entry None;
            };
            if !schema.entry_kinds.contains(&kind) {
                break 'entry None;
            }
            let fields = fields_of(m, schema, n, row, &mut problems);
            let terminal = class == StatusClass::Terminal;
            let inputs = DateInputs {
                facts: &it.facts,
                row_created: row_created(m, row),
                blame: &s.blame,
                lines: &it.lines,
                terminal,
                needs_revisit: class.needs_revisit(),
                ov,
                token: n,
            };
            let dates = match derive_dates(&inputs, input.now) {
                Ok(dates) => dates,
                Err(found) => {
                    problems.extend(found);
                    break 'entry None;
                }
            };
            if !dates.derived.is_empty() {
                review
                    .derived_dates
                    .push(format!("{n}: {}", dates.derived.join(", ")));
            }
            let fixed_stem = match ov.and_then(|o| o.slug.as_deref()) {
                Some(slug) => match entry_stem(schema.key_style, dates.dates.created, slug) {
                    Ok(key) => Some(key.as_str().to_string()),
                    Err(_) => {
                        problems.push(format!(
                            "{n}: overrides.{n}.slug \"{slug}\" gives no valid file name. Set overrides.{n}.slug to lowercase letters and digits joined by single hyphens."
                        ));
                        break 'entry None;
                    }
                },
                None => s.fixed_stem.clone(),
            };
            let new_entry = NewEntry {
                entry_kind: &kind,
                title: &it.title,
                status: Some(&status),
                tags: Vec::new(),
                valid: None,
                revisit_by: dates.revisit_by,
                fields,
                body: Some(&body),
            };
            let rules = RenderRules {
                refuse_past_revisit: false,
                fixed_stem,
            };
            let rendered = render_entry(
                schema,
                &target_dir,
                template.as_deref(),
                &new_entry,
                &dates.dates,
                input.now,
                &rules,
            );
            let file = match rendered {
                Ok(file) => file,
                Err(found) => {
                    problems.extend(found.iter().map(|p| write_problem_text(n, p)));
                    break 'entry None;
                }
            };
            let rel = if terminal {
                match place(&file.rel_path) {
                    Some(Ok(placement)) => archive_rel_path(&file.rel_path, &placement)
                        .unwrap_or_else(|| file.rel_path.clone()),
                    _ => {
                        problems.push(format!(
                            "{n}: the entry path {} is not in a collection. Check target.",
                            file.rel_path
                        ));
                        break 'entry None;
                    }
                }
            } else {
                file.rel_path.clone()
            };
            let key = file.key.as_str();
            if let Some(first) = planned_keys.insert(key.to_string(), n.to_string()) {
                problems.push(format!(
                    "{first} and {n} both get the key {key}. Set overrides.{n}.slug."
                ));
            }
            for held in input.key_paths.get(key).into_iter().flatten() {
                if *held != rel && !source_paths.contains(held.as_str()) {
                    problems.push(format!(
                        "{n}: the key {key} is already used by {held}. Set overrides.{n}.slug."
                    ));
                }
            }
            if short.is_some() {
                shortened.insert(rel.clone());
            }
            writes.insert(rel.clone(), file.text);
            Some(rel)
        };
        if let (Some(path), Owner::Record { entry, .. }) = (entry, &mut range.owner) {
            *entry = path;
        }
        ranges.push((it.src, range));
    }

    let names: BTreeSet<&str> = items.iter().map(|it| it.name.as_str()).collect();
    for k in m.overrides.keys().filter(|k| !names.contains(k.as_str())) {
        problems.push(format!("overrides.{k} names no record"));
    }
    for k in m.shorten.keys().filter(|k| !names.contains(k.as_str())) {
        problems.push(format!("shorten.{k} names no record"));
    }
    for held in input.key_paths.get(&new_key).into_iter().flatten() {
        if !held.eq_ignore_ascii_case(&readme_path) && !source_paths.contains(held.as_str()) {
            problems.push(format!(
                "the collection key {new_key} is already used by {held}. Set target.key."
            ));
        }
    }
    let readme_path = readme_text.map(|text| {
        writes.insert(readme_path.clone(), text);
        readme_path
    });

    // Step 7: a key change rewrites `[[old-key…]]` in every other file, and nothing else there.
    if old_key != new_key {
        for (path, text) in input.other_markdown {
            if source_paths.contains(path.as_str()) || writes.contains_key(path) {
                continue;
            }
            let (out, _) = rewrite_record_citations(text, &BTreeSet::new(), &old_key, &new_key);
            if out != *text {
                writes.insert(path.clone(), out);
            }
        }
    }

    // Step 8: deletes, ranges and the partition of each source.
    let mut deletes: BTreeSet<String> = sources.iter().map(|s| s.path.clone()).collect();
    if matches!(m.sidecar, Some(SidecarSpec::Retire)) {
        for s in sources {
            deletes.insert(rel_path_for(Path::new(""), Path::new(&s.path)));
        }
    }
    ranges.sort_by(|(i, a), (j, b)| {
        (i, a.lines.start(), a.lines.end()).cmp(&(j, b.lines.start(), b.lines.end()))
    });
    for (i, sp) in splits.iter().enumerate() {
        let owned: Vec<Lines> = ranges
            .iter()
            .filter(|(j, _)| *j == i)
            .map(|(_, r)| r.lines.clone())
            .collect();
        for p in partition_problems(sp.line_count, &owned) {
            problems.push(partition_text(&sources[i].path, &p));
        }
    }

    MigrationPlan {
        target_dir,
        readme_path,
        writes,
        deletes,
        ranges: ranges.into_iter().map(|(_, r)| r).collect(),
        shortened,
        defined,
        old_key,
        new_key,
        review,
        problems,
    }
}

/// The body of the entry (or the readme part) that owns `r`: the source lines of `r` after the
/// citation rewrite. A `Record` with a synthesised `heading` (a row-only record, empty range)
/// starts with that heading. A `Record` with a `row` ends with a blank line and
/// `**Index row:** <row>`, the row verbatim and exempt from the rewrite.
///
/// This is the one definition of an entry body: the planner builds every body with it and
/// `check` recomputes every body with it from the source at `HEAD`.
pub fn expected_body(
    source: &str,
    r: &OwnedRange,
    defined: &BTreeSet<String>,
    old: &str,
    new: &str,
) -> String {
    let (mut body, _) = rewrite_record_citations(&slice(source, &r.lines), defined, old, new);
    if let Owner::Record { heading, row, .. } = &r.owner {
        if let Some(heading) = heading {
            body = if body.is_empty() {
                heading.clone()
            } else {
                format!("{heading}\n\n{body}")
            };
        }
        if let Some(row) = row {
            // Trailing blank lines of the range would double the blank line before the row.
            body.truncate(body.trim_end_matches('\n').len());
            if !body.is_empty() {
                body.push_str("\n\n");
            }
            body.push_str("**Index row:** ");
            body.push_str(row);
        }
    }
    body
}
/// A record to carry: a heading record of a source, or a row-only record of an index.
struct Item<'t> {
    src: usize,
    token: Option<String>,
    /// The token, else the fixed stem, else the title: the name in messages and the key of
    /// `overrides` and `shorten`.
    name: String,
    title: String,
    /// The heading line, or the row line of a row-only record.
    heading_line: u32,
    lines: Lines,
    heading: Option<String>,
    row: Option<&'t IndexRow>,
    facts: Facts,
}

fn no_facts() -> Facts {
    Facts {
        raw_status_body: None,
        stated_dates: Vec::new(),
        status_date: None,
        closed_date: None,
        revisit_by: None,
    }
}

/// The index row of each token. Two rows in one table are refused (`index_table` cannot pick
/// between them); rows in two tables need `index_table` (plan P-R8).
fn choose_rows<'t>(
    m: &Mapping,
    sources: &[SourceText],
    tables: &'t [Vec<IndexTable>],
    problems: &mut Vec<String>,
) -> BTreeMap<String, (usize, &'t IndexRow)> {
    let mut found: BTreeMap<&str, Vec<(usize, &IndexTable, &IndexRow)>> = BTreeMap::new();
    for (i, ts) in tables.iter().enumerate() {
        for t in ts {
            for r in &t.rows {
                found.entry(r.token.as_str()).or_default().push((i, t, r));
            }
        }
    }
    let mut out = BTreeMap::new();
    for (token, rows) in found {
        let mut per_table: BTreeMap<(usize, u32), usize> = BTreeMap::new();
        for (i, t, _) in &rows {
            *per_table.entry((*i, *t.lines.start())).or_default() += 1;
        }
        let twice: Vec<(usize, u32)> = per_table
            .iter()
            .filter(|(_, n)| **n > 1)
            .map(|(k, _)| *k)
            .collect();
        for (i, line) in &twice {
            problems.push(format!(
                "token {token} has two rows in the index table at line {line} of {}. Keep one row.",
                sources[*i].path
            ));
        }
        if !twice.is_empty() {
            continue;
        }
        let picked: Vec<(usize, &IndexTable, &IndexRow)> = if rows.len() == 1 {
            rows.clone()
        } else {
            rows.iter()
                .filter(|(_, t, _)| Some(t.section_start) == m.index_table)
                .copied()
                .collect()
        };
        match picked.as_slice() {
            [(i, _, r)] => {
                out.insert(token.to_string(), (*i, *r));
            }
            _ => {
                let at: Vec<String> = rows
                    .iter()
                    .map(|(i, t, _)| format!("line {} of {}", t.lines.start(), sources[*i].path))
                    .collect();
                problems.push(format!(
                    "token {token} has rows in the index tables at {}. Set index_table to the first line of the section that holds the table to use.",
                    at.join(" and ")
                ));
            }
        }
    }
    out
}

/// The first definition of each token. A token defined in two sources, or twice in one source
/// outside its own record, is a problem.
fn definitions(
    sources: &[SourceText],
    splits: &[Split],
    problems: &mut Vec<String>,
) -> BTreeMap<String, (usize, u32)> {
    let mut at: BTreeMap<String, (usize, u32)> = BTreeMap::new();
    for (i, (s, sp)) in sources.iter().zip(splits).enumerate() {
        for r in sp.repeats.iter().filter(|r| !r.nested) {
            problems.push(format!(
                "token {} has two record headings in {}, at line {} and at line {}. Keep one.",
                r.token, s.path, r.first_line, r.other_line
            ));
        }
        for rec in &sp.records {
            let Some(t) = &rec.token else { continue };
            match at.get(t) {
                None => {
                    at.insert(t.clone(), (i, rec.heading_line));
                }
                Some(&(j, line)) if j != i => problems.push(format!(
                    "token {t} is defined at line {line} of {} and at line {} of {}. Keep one definition.",
                    sources[j].path, rec.heading_line, s.path
                )),
                Some(_) => {}
            }
        }
    }
    at
}

/// Possible records that nothing settles, and `possibles_inside` entries that name nothing.
/// Inside a record only `possibles_inside` settles one (`records_extra` would have made it a
/// record); in a section only a `sections` entry with `confirm = true` does.
fn possible_problems(
    m: &Mapping,
    sources: &[SourceText],
    splits: &[Split],
    problems: &mut Vec<String>,
) {
    for (s, sp) in sources.iter().zip(splits) {
        let path = s.path.as_str();
        for p in &sp.possibles {
            let (line, token) = (p.line, p.token.as_str());
            let kept = m
                .possibles_inside
                .iter()
                .any(|k| k.source == path && k.line == line);
            match sp.records.iter().find(|r| r.lines.contains(&line)) {
                Some(_) if kept => {}
                Some(_) if s.kind == SourceKind::Whole => problems.push(format!(
                    "possible record {token} at line {line} of {path} is inside the whole record of {path}. Add it to possibles_inside."
                )),
                Some(r) => {
                    let name = match &r.token {
                        Some(t) => format!("record {t}"),
                        None => format!("the record at line {}", r.heading_line),
                    };
                    problems.push(format!(
                        "possible record {token} at line {line} of {path} is inside {name}. Add it to records_extra or possibles_inside."
                    ));
                }
                None => {
                    let confirmed = sp
                        .sections
                        .iter()
                        .find(|x| x.lines.contains(&line))
                        .and_then(|x| section_spec(m, path, x.start))
                        .is_some_and(|spec| spec.confirm);
                    if !confirmed {
                        problems.push(format!(
                            "possible record {token} at line {line} of {path} is in a section. Add it to records_extra, or to sections with confirm = true."
                        ));
                    }
                }
            }
        }
    }
    for k in &m.possibles_inside {
        let named = sources
            .iter()
            .zip(splits)
            .any(|(s, sp)| s.path == k.source && sp.possibles.iter().any(|p| p.line == k.line));
        if !named {
            problems.push(format!(
                "the possibles_inside entry at line {} of {} names no possible record",
                k.line, k.source
            ));
        }
    }
}

fn section_spec<'m>(m: &'m Mapping, path: &str, start: u32) -> Option<&'m SectionSpec> {
    m.sections
        .iter()
        .find(|s| s.source == path && s.start == start)
}

/// The readme of a new collection from the mapping's `schema` (its `title`, else the key) and
/// `body`, and the schema read back from it.
fn new_readme(
    m: &Mapping,
    key: &str,
    body: &str,
) -> Result<(String, CollectionSchema), Vec<String>> {
    let Some(lifecycle) = Lifecycle::from_dir(&m.target.lifecycle) else {
        return Err(vec![format!(
            "target.lifecycle \"{}\" is not a lifecycle folder. Use ledgers, streams or temp.",
            m.target.lifecycle
        )]);
    };
    let mut extra: BTreeMap<String, Value> = match m.schema.as_ref().map(serde_json::to_value) {
        None => BTreeMap::new(),
        Some(Ok(Value::Object(map))) => map.into_iter().collect(),
        Some(_) => return Err(vec!["schema in the mapping is not a table".to_string()]),
    };
    let title = match extra.remove("title") {
        None => key.to_string(),
        Some(Value::String(title)) => title,
        Some(_) => return Err(vec!["schema.title is not a string".to_string()]),
    };
    let invalid =
        |e: &dyn std::fmt::Display| format!("the schema in the mapping is not valid: {e}");
    let text = collection_readme(lifecycle, &title, &extra, body)
        .map_err(|errs| errs.iter().map(|e| invalid(e)).collect::<Vec<_>>())?;
    let Ok((Some(fm), _)) = frontmatter::parse(&text) else {
        return Err(vec![
            "the rendered readme does not read back. Check schema in the mapping.".to_string(),
        ]);
    };
    let schema = parse_schema(lifecycle, &fm)
        .map_err(|errs| errs.iter().map(|e| invalid(e)).collect::<Vec<_>>())?;
    Ok((text, schema))
}

/// Every mapped status and entry kind must be declared by the collection, and every mapped
/// column must name a declared field. A `constant` status is a declared status itself.
fn declared_problems(m: &Mapping, schema: &CollectionSchema, out: &mut Vec<String>) {
    let undeclared = |s: &str| !schema.statuses.contains_key(s);
    for (raw, s) in &m.status_map {
        if undeclared(s) {
            out.push(format!(
                "status_map \"{raw}\" = \"{s}\": \"{s}\" is not a declared status of the collection"
            ));
        }
    }
    if let Some(s) = m.status_none.as_deref().filter(|s| undeclared(s)) {
        out.push(format!(
            "status_none \"{s}\" is not a declared status of the collection"
        ));
    }
    if let StatusSource::Constant(s) = &m.status_source {
        if undeclared(s) {
            out.push(format!(
                "status_source constant \"{s}\" is not a declared status of the collection"
            ));
        }
    }
    for (name, o) in &m.overrides {
        if let Some(s) = o.status.as_deref().filter(|s| undeclared(s)) {
            out.push(format!(
                "overrides.{name}.status \"{s}\" is not a declared status of the collection"
            ));
        }
    }
    let kind_undeclared = |k: &String| !schema.entry_kinds.contains(k);
    match &m.entry_kind {
        EntryKindRule::One(k) => {
            if kind_undeclared(k) {
                out.push(format!(
                    "entry_kind \"{k}\" is not a declared entry kind of the collection"
                ));
            }
        }
        EntryKindRule::ByPrefix(kinds) => {
            for (prefix, k) in kinds.iter().filter(|(_, k)| kind_undeclared(k)) {
                out.push(format!(
                    "entry_kind.{prefix} \"{k}\" is not a declared entry kind of the collection"
                ));
            }
        }
    }
    for (header, target) in &m.columns {
        if !is_date_or_alias(target) && !schema.fields.contains_key(target) {
            out.push(format!(
                "columns.{header} names field \"{target}\", which the collection does not declare"
            ));
        }
    }
}

/// A `columns` target that is no field: `created` feeds the dates; `title_alias` keeps the cell
/// only in the `**Index row:**` line.
fn is_date_or_alias(target: &str) -> bool {
    target == "created" || target == "title_alias"
}

/// The mapped status of one record: `overrides.<name>.status`, else the constant, else the raw
/// value through `status_map`, else `status_none`.
fn status_of(
    m: &Mapping,
    name: &str,
    ov: Option<&Override>,
    facts: &Facts,
    row: Option<&IndexRow>,
) -> Result<String, String> {
    if let Some(status) = ov.and_then(|o| o.status.clone()) {
        return Ok(status);
    }
    let raw = match &m.status_source {
        StatusSource::Constant(status) => return Ok(status.clone()),
        StatusSource::BodyLabel => facts.raw_status_body.as_deref().and_then(normalise_status),
        StatusSource::IndexColumn(header) => row
            .and_then(|r| r.cells.get(&header.trim().to_lowercase()))
            .and_then(|c| normalise_status(c)),
    };
    match raw {
        Some(raw) => m.status_map.get(&raw).cloned().ok_or_else(|| {
            format!("{name}: status value \"{raw}\" is not in status_map. Add it to status_map, or set overrides.{name}.status.")
        }),
        None => m.status_none.clone().ok_or_else(|| {
            format!("{name}: the record has no status. Set status_none, or overrides.{name}.status.")
        }),
    }
}

fn entry_kind_of(m: &Mapping, it: &Item, problems: &mut Vec<String>) -> Option<String> {
    let kinds = match &m.entry_kind {
        EntryKindRule::One(kind) => return Some(kind.clone()),
        EntryKindRule::ByPrefix(kinds) => kinds,
    };
    let Some(token) = &it.token else {
        problems.push(format!(
            "{}: entry_kind by prefix cannot class a record with no token. Use one entry_kind.",
            it.name
        ));
        return None;
    };
    let prefix = token.split('-').next().unwrap_or(token);
    let kind = kinds.get(prefix).cloned();
    if kind.is_none() {
        problems.push(format!(
            "{}: token prefix \"{prefix}\" is not in entry_kind. Add it to entry_kind.",
            it.name
        ));
    }
    kind
}

/// The declared fields that `columns` maps from the index row, each a string.
fn fields_of(
    m: &Mapping,
    schema: &CollectionSchema,
    name: &str,
    row: Option<&IndexRow>,
    problems: &mut Vec<String>,
) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    let Some(row) = row else { return out };
    for (header, field) in m.columns.iter().filter(|(_, f)| !is_date_or_alias(f)) {
        let cell = row.cells.get(&header.trim().to_lowercase());
        let Some(cell) = cell.filter(|c| !c.is_empty()) else {
            continue;
        };
        match schema.fields.get(field) {
            // Already a problem from `declared_problems`, once per column.
            None => {}
            Some(FieldType::Scalar) => {
                out.insert(field.clone(), Value::String(cell.clone()));
            }
            Some(FieldType::Enum(values)) if values.contains(cell) => {
                out.insert(field.clone(), Value::String(cell.clone()));
            }
            Some(FieldType::Enum(_)) => problems.push(format!(
                "{name}: the cell \"{cell}\" of column \"{header}\" is not a value of field \"{field}\". Change the cell, or remove columns.{header}."
            )),
            Some(FieldType::List) => problems.push(format!(
                "{name}: column \"{header}\" maps to the list field \"{field}\", and a cell is one value. Remove columns.{header}."
            )),
        }
    }
    out
}

/// The earliest date in the cells of the columns mapped to `created`.
fn row_created(m: &Mapping, row: Option<&IndexRow>) -> Option<NaiveDate> {
    let row = row?;
    m.columns
        .iter()
        .filter(|(_, target)| target.as_str() == "created")
        .filter_map(|(header, _)| row.cells.get(&header.trim().to_lowercase()))
        .filter_map(|cell| {
            date_re()
                .find_iter(cell)
                .find_map(|d| NaiveDate::parse_from_str(d.as_str(), "%Y-%m-%d").ok())
        })
        .min()
}

fn date_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b\d{4}-\d{2}-\d{2}\b").unwrap())
}

/// One review line for a range whose rewrite skipped a token: every count of the rewrite.
fn note_skips(
    review: &mut Review,
    name: &str,
    text: &str,
    lines: &Lines,
    defined: &BTreeSet<String>,
    old: &str,
    new: &str,
) {
    let (_, c) = rewrite_record_citations(&slice(text, lines), defined, old, new);
    if c.skipped_link_text + c.skipped_url + c.skipped_adjacent > 0 {
        review.skipped_rewrites.push(format!(
            "{name}: {} rewritten, skipped {} in link text, {} in URLs, {} next to ( or !",
            c.rewritten, c.skipped_link_text, c.skipped_url, c.skipped_adjacent
        ));
    }
}

/// The body of `entry_text` equals `body`, compared as `check` compares: by lines, leading and
/// trailing blank lines ignored.
fn same_body(entry_text: &str, body: &str) -> bool {
    match frontmatter::parse(entry_text) {
        Ok((_, on_disk)) => body_lines(on_disk) == body_lines(body),
        Err(_) => false,
    }
}

fn body_lines(text: &str) -> Vec<&str> {
    let lines: Vec<&str> = text.lines().collect();
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

/// The lowercase stem of the last component of `path` (`docs/trackers/x.md` gives `x`, the folder
/// `docs/trackers/ic` gives `ic`).
fn path_stem(path: &str) -> String {
    let name = path
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("");
    let stem = if name.to_ascii_lowercase().ends_with(".md") {
        &name[..name.len() - 3]
    } else {
        name
    };
    stem.to_ascii_lowercase()
}

fn lines_text(r: &Lines) -> String {
    if r.start() == r.end() {
        format!("line {}", r.start())
    } else {
        format!("lines {}-{}", r.start(), r.end())
    }
}

fn partition_text(path: &str, p: &PartitionProblem) -> String {
    match p {
        PartitionProblem::Uncovered(r) if r.start() == r.end() => {
            format!("line {} of {path} has no owner", r.start())
        }
        PartitionProblem::Uncovered(r) => format!("{} of {path} have no owner", lines_text(r)),
        PartitionProblem::Overlap { a, b } => format!(
            "{} and {} of {path} have two owners",
            lines_text(a),
            lines_text(b)
        ),
    }
}

/// A refusal of `render_entry` as a message that names the record and the mapping key that
/// settles it. No arm forwards the text of the `WriteProblem`, and there is no wildcard arm, so a
/// new variant must get its own message.
fn write_problem_text(n: &str, p: &WriteProblem) -> String {
    match p {
        WriteProblem::KindUndeclared(kind) => format!(
            "{n}: entry kind \"{kind}\" is not declared by the collection. Change entry_kind in the mapping."
        ),
        WriteProblem::StatusUndeclared(status) => format!(
            "{n}: status \"{status}\" is not declared by the collection. Set overrides.{n}.status."
        ),
        WriteProblem::Title(title) if title.trim().is_empty() || title.contains(['\n', '\r']) => {
            format!("{n}: the record has no one-line title. Give its first heading a title in the source.")
        }
        WriteProblem::Title(title) => format!(
            "{n}: the title \"{title}\" gives no file name. Set overrides.{n}.slug."
        ),
        WriteProblem::FixedStem(stem) => format!(
            "{n}: the file name \"{stem}\" is not a valid entry key. Set overrides.{n}.slug."
        ),
        WriteProblem::Stem(_) => format!(
            "{n}: the title gives no valid entry key. Set overrides.{n}.slug."
        ),
        WriteProblem::FieldUndeclared(field) => format!(
            "{n}: field \"{field}\" is not declared by the collection. Remove it from columns."
        ),
        WriteProblem::RevisitNotNeeded(status) => format!(
            "{n}: status \"{status}\" takes no revisit_by. Remove overrides.{n}.revisit_by."
        ),
        WriteProblem::RevisitPast(date) => format!(
            "{n}: revisit_by {date} is in the past. Set overrides.{n}.revisit_by."
        ),
        WriteProblem::DateOrder {
            field: "closed",
            date,
            created,
        } => format!(
            "{n}: closed {date} is before created {created}. Set overrides.{n}.closed or overrides.{n}.created."
        ),
        WriteProblem::DateOrder {
            field,
            date,
            created,
        } => format!(
            "{n}: {field} {date} is before created {created}. Set overrides.{n}.created."
        ),
        WriteProblem::DateInFuture {
            field: field @ ("created" | "closed"),
            date,
        } => format!("{n}: {field} {date} is after today. Set overrides.{n}.{field}."),
        WriteProblem::DateInFuture { field, date } => format!(
            "{n}: {field} {date} is after today. No override sets it. Correct the date that the record states for its status."
        ),
        WriteProblem::ClosedMismatch { terminal: true } => format!(
            "{n}: the status is terminal and no closed date is known. Set overrides.{n}.closed."
        ),
        WriteProblem::ClosedMismatch { terminal: false } => format!(
            "{n}: the status is not terminal and a closed date is set. Set overrides.{n}.status."
        ),
        WriteProblem::BadYaml(_) | WriteProblem::NoFrontmatter | WriteProblem::Invalid(_) => {
            format!("{n}: the rendered entry is not a valid entry. Check columns and overrides.{n}.")
        }
        WriteProblem::EmptyUpdate | WriteProblem::KindFixed | WriteProblem::KindNotRepairable(_) => {
            format!("{n}: the entry cannot be rendered. Check entry_kind and overrides.{n}.")
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;
    use crate::librarian::frontmatter::{self, Frontmatter};
    use crate::librarian::lifecycle::collection::collection_readme;
    use crate::librarian::lifecycle::layout::Lifecycle;
    use crate::librarian::lifecycle::schema::parse_schema;
    use crate::librarian::tracker_migration::mapping::{
        parse_mapping, ExtraRecordSpec, Override, PossibleSpec, SectionSpec, SourceSpec,
        StatusSource,
    };
    use crate::librarian::tracker_migration::records::partition_problems;

    const X: &str = "docs/trackers/x.md";
    const F1_PATH: &str = "docs/trackers/ledgers/x/2026-08-01-first-thing.md";
    const F2_PATH: &str = "docs/trackers/ledgers/x/archive/2026-08-02-second-thing.md";
    const README: &str = "docs/trackers/ledgers/x/readme.md";
    const F1_ROW: &str = "| F-1 | open | First, see F-2 | 2026-08-01 |";
    const F2_ROW: &str = "| F-2 | fixed | Second | 2026-08-02 |";

    /// Lines 1-3 frontmatter; 4-12 preamble and index (header 8, rows F-1 at 10 and F-2 at 11);
    /// F-1 13-17; F-2 18-22.
    const TRACKER: &str = "\
---
title: X log
---
# X log

Preamble cites F-2.

| ID | Status | Title | Opened |
|---|---|---|---|
| F-1 | open | First, see F-2 | 2026-08-01 |
| F-2 | fixed | Second | 2026-08-02 |

## F-1 — First thing

**Status:** open
See F-2 and [F-2](x.md).

## F-2 — Second thing

**Status:** fixed
**Closed:** 2026-09-01
Body two.
";

    /// Lines 13-17 rewritten (`F-2` defined; the link text is left), blank line 17 dropped
    /// before the row line, and the row verbatim.
    const F1_BODY: &str = "## F-1 — First thing\n\n**Status:** open\nSee [[x:F-2]] and [F-2](x.md).\n\n**Index row:** | F-1 | open | First, see F-2 | 2026-08-01 |";
    const F2_BODY: &str = "## F-2 — Second thing\n\n**Status:** fixed\n**Closed:** 2026-09-01\nBody two.\n\n**Index row:** | F-2 | fixed | Second | 2026-08-02 |";
    /// `readme_body`, then lines 4-12 rewritten: prose and table cells alike.
    const README_BODY: &str = "Findings of x.\n\n# X log\n\nPreamble cites [[x:F-2]].\n\n| ID | Status | Title | Opened |\n|---|---|---|---|\n| [[x:F-1]] | open | First, see [[x:F-2]] | 2026-08-01 |\n| [[x:F-2]] | fixed | Second | 2026-08-02 |";

    const BASE: &str = r#"
readme_body = "Findings of x."
entry_kind = "finding"
status_source = "body_label"
sidecar = "retire"

[[sources]]
path = "docs/trackers/x.md"
kind = "file"

[target]
lifecycle = "ledgers"

[schema]
entry_kinds = ["finding"]
initial_status = "open"
hot_cap = 10
stale_after = "30d"

[schema.statuses]
open = "open"
parked = "deferred"
fixed = "terminal"

[status_map]
open = "open"
fixed = "fixed"

[columns]
opened = "created"

[[sections]]
source = "docs/trackers/x.md"
start = 4
to = "readme"
"#;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn mapping() -> Mapping {
        parse_mapping(BASE).expect("BASE parses")
    }

    /// A `file` source whose every line was last changed on 2026-08-10.
    fn source(path: &str, text: &str) -> SourceText {
        SourceText {
            path: path.into(),
            kind: SourceKind::File,
            text: text.into(),
            blame: vec![d("2026-08-10"); text.lines().count()],
            fixed_stem: None,
        }
    }

    fn plan_with(
        m: &Mapping,
        sources: Vec<SourceText>,
        other: &BTreeMap<String, String>,
        keys: &BTreeMap<String, BTreeSet<String>>,
        existing: Option<(&CollectionSchema, Option<&str>, Vec<ExistingEntry>)>,
    ) -> MigrationPlan {
        plan_migration(&MigrationInput {
            mapping: m,
            sources,
            other_markdown: other,
            key_paths: keys,
            existing,
            now: d("2026-11-16"),
        })
    }

    fn plan(m: &Mapping, text: &str) -> MigrationPlan {
        plan_with(
            m,
            vec![source(X, text)],
            &BTreeMap::new(),
            &BTreeMap::new(),
            None,
        )
    }

    fn body(text: &str) -> String {
        frontmatter::parse(text)
            .unwrap()
            .1
            .trim_matches('\n')
            .to_string()
    }

    fn fm(text: &str) -> Frontmatter {
        frontmatter::parse(text).unwrap().0.unwrap()
    }

    fn assert_problem(p: &MigrationPlan, msg: &str) {
        assert!(
            p.problems.iter().any(|x| x == msg),
            "missing {msg:?} in {:#?}",
            p.problems
        );
    }

    fn no_problems(p: &MigrationPlan) {
        assert_eq!(p.problems, Vec::<String>::new());
    }

    fn keys(p: &MigrationPlan) -> Vec<&str> {
        p.writes.keys().map(String::as_str).collect()
    }

    fn range_of<'a>(p: &'a MigrationPlan, token: &str) -> &'a OwnedRange {
        p.ranges
            .iter()
            .find(|r| matches!(&r.owner, Owner::Record { token: Some(t), .. } if t == token))
            .unwrap_or_else(|| panic!("no range for {token}: {:#?}", p.ranges))
    }

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn slug_override(slug: &str) -> Override {
        Override {
            slug: Some(slug.into()),
            ..no_override()
        }
    }

    fn no_override() -> Override {
        Override {
            status: None,
            created: None,
            closed: None,
            revisit_by: None,
            slug: None,
        }
    }

    /// Breaks if the readme or an entry is not written, a body loses the rewrite or the row line,
    /// the source or the sidecar is not deleted, or the review lists change.
    #[test]
    fn a_two_record_tracker_plans_two_entries_and_a_readme() {
        let p = plan(&mapping(), TRACKER);
        no_problems(&p);
        assert_eq!(p.target_dir, "docs/trackers/ledgers/x");
        assert_eq!(p.readme_path.as_deref(), Some(README));
        assert_eq!(keys(&p), vec![F1_PATH, F2_PATH, README]);
        assert_eq!(body(&p.writes[F1_PATH]), F1_BODY);
        assert_eq!(body(&p.writes[F2_PATH]), F2_BODY);
        assert_eq!(body(&p.writes[README]), README_BODY);
        let f1 = fm(&p.writes[F1_PATH]);
        assert_eq!(f1.status.as_deref(), Some("open"));
        assert_eq!(f1.title.as_deref(), Some("First thing"));
        assert_eq!(f1.extra["created"], json!("2026-08-01"));
        assert_eq!(f1.extra["entry_kind"], json!("finding"));
        assert_eq!(
            p.deletes,
            set(&[X, "docs/augmentations/docs-trackers-x.yaml"])
        );
        assert_eq!(p.defined, set(&["F-1", "F-2"]));
        assert_eq!((p.old_key.as_str(), p.new_key.as_str()), ("x", "x"));
        assert_eq!(
            p.review.derived_dates,
            vec!["F-1: status_since", "F-2: status_since"]
        );
        assert_eq!(
            p.review.skipped_rewrites,
            vec!["F-1: 1 rewritten, skipped 1 in link text, 0 in URLs, 0 next to ( or !"]
        );
        assert!(p.review.no_row.is_empty() && p.review.row_only.is_empty());
    }

    /// Breaks if a terminal entry stays live, or loses its closed date.
    #[test]
    fn a_terminal_record_goes_to_archive() {
        let p = plan(&mapping(), TRACKER);
        no_problems(&p);
        let f2 = fm(&p.writes[F2_PATH]);
        assert_eq!(f2.status.as_deref(), Some("fixed"));
        assert_eq!(f2.extra["closed"], json!("2026-09-01"));
        assert_eq!(f2.extra["created"], json!("2026-08-02"));
        assert!(!p
            .writes
            .contains_key("docs/trackers/ledgers/x/2026-08-02-second-thing.md"));
        assert!(fm(&p.writes[F1_PATH]).extra.get("closed").is_none());
    }

    /// Breaks if `archived` is ignored, or a terminal entry gets a second `archive/` level.
    #[test]
    fn an_archived_target_puts_everything_under_lifecycle_archive() {
        let mut m = mapping();
        m.target.archived = true;
        let p = plan(&m, TRACKER);
        no_problems(&p);
        assert_eq!(p.target_dir, "docs/trackers/ledgers/archive/x");
        assert_eq!(
            keys(&p),
            vec![
                "docs/trackers/ledgers/archive/x/2026-08-01-first-thing.md",
                "docs/trackers/ledgers/archive/x/2026-08-02-second-thing.md",
                "docs/trackers/ledgers/archive/x/readme.md",
            ]
        );
    }

    /// Breaks if a raw status that `status_map` lacks gets a default status.
    #[test]
    fn an_unmapped_status_is_a_problem() {
        let mut m = mapping();
        m.status_map.remove("fixed");
        let p = plan(&m, TRACKER);
        assert_problem(
            &p,
            "F-2: status value \"fixed\" is not in status_map. Add it to status_map, or set overrides.F-2.status.",
        );
        assert!(!p.writes.contains_key(F2_PATH));
    }

    /// `### F-1a — Sub` (line 17) is token-shaped and inside F-1. Breaks if the planner plans a
    /// possible record that nothing settled, or refuses one that `possibles_inside` settled.
    #[test]
    fn an_unsettled_possible_record_is_a_problem() {
        let text = TRACKER.replace(
            "See F-2 and [F-2](x.md).\n",
            "See F-2 and [F-2](x.md).\n### F-1a — Sub\n",
        );
        let mut m = mapping();
        let p = plan(&m, &text);
        assert_problem(
            &p,
            "possible record F-1a at line 17 of docs/trackers/x.md is inside record F-1. Add it to records_extra or possibles_inside.",
        );
        m.possibles_inside.push(PossibleSpec {
            source: X.into(),
            line: 17,
        });
        let p = plan(&m, &text);
        no_problems(&p);
        assert_eq!(range_of(&p, "F-1").lines, 13..=18);
        assert!(body(&p.writes[F1_PATH]).contains("\n### F-1a — Sub\n"));
    }

    /// Row F-3 (line 12) has no heading. Breaks if the row-only record is lost, gets a range that
    /// owns source lines, or its body is not the synthesised heading plus the row line.
    #[test]
    fn a_row_only_record_gets_a_synthesised_heading() {
        let row3 = "| F-3 | open | Third | 2026-08-03 |";
        let text = TRACKER.replace(&format!("{F2_ROW}\n"), &format!("{F2_ROW}\n{row3}\n"));
        let mut m = mapping();
        m.row_title_column = Some("Title".into());
        m.status_none = Some("open".into());
        let p = plan(&m, &text);
        no_problems(&p);
        let want = format!("## F-3 — Third\n\n**Index row:** {row3}");
        let path = "docs/trackers/ledgers/x/2026-08-03-third.md";
        assert_eq!(body(&p.writes[path]), want);
        assert_eq!(fm(&p.writes[path]).title.as_deref(), Some("Third"));
        let r = range_of(&p, "F-3");
        assert_eq!(r.source, X);
        assert!(r.lines.is_empty(), "{:?}", r.lines);
        assert_eq!(expected_body(&text, r, &p.defined, "x", "x"), want);
        assert_eq!(p.review.row_only, vec!["F-3"]);
        assert!(p.defined.contains("F-3"));
        assert_eq!(range_of(&p, "F-1").lines, 14..=18);
    }

    /// The row of F-1 holds a bare `F-2`. Breaks if the row line is rewritten, moved, or appended
    /// before the rewrite of the range.
    #[test]
    fn the_index_row_line_is_verbatim_and_not_rewritten() {
        let p = plan(&mapping(), TRACKER);
        let b = body(&p.writes[F1_PATH]);
        assert!(b.ends_with(&format!("\n\n**Index row:** {F1_ROW}")), "{b}");
        assert!(b.contains("See [[x:F-2]] and"), "{b}");
        let r = range_of(&p, "F-1");
        assert_eq!(expected_body(TRACKER, r, &p.defined, "x", "x"), F1_BODY);
        let mut no_row = r.clone();
        no_row.owner = Owner::Record {
            entry: F1_PATH.into(),
            token: Some("F-1".into()),
            heading: None,
            row: None,
        };
        assert_eq!(
            expected_body(TRACKER, &no_row, &p.defined, "x", "x"),
            "## F-1 — First thing\n\n**Status:** open\nSee [[x:F-2]] and [F-2](x.md).\n"
        );
    }

    /// Breaks if a mapped column is not written, a value its enum refuses is accepted, or a
    /// column that names an undeclared field passes.
    #[test]
    fn columns_become_declared_fields() {
        let text = BASE.replace(
            "[schema.statuses]",
            "[schema.fields]\nlabel = { type = \"scalar\" }\narea = { type = \"enum\", values = [\"core\"] }\n\n[schema.statuses]",
        );
        let mut m = parse_mapping(&text).unwrap();
        m.columns.insert("Title".into(), "label".into());
        let p = plan(&m, TRACKER);
        no_problems(&p);
        assert_eq!(
            fm(&p.writes[F1_PATH]).extra["label"],
            json!("First, see F-2")
        );
        assert_eq!(fm(&p.writes[F2_PATH]).extra["label"], json!("Second"));

        m.columns.insert("status".into(), "area".into());
        m.columns.insert("id".into(), "nope".into());
        let p = plan(&m, TRACKER);
        assert_problem(
            &p,
            "F-1: the cell \"open\" of column \"status\" is not a value of field \"area\". Change the cell, or remove columns.status.",
        );
        assert_problem(
            &p,
            "F-2: the cell \"fixed\" of column \"status\" is not a value of field \"area\". Change the cell, or remove columns.status.",
        );
        assert_problem(
            &p,
            "columns.id names field \"nope\", which the collection does not declare",
        );
    }

    /// Breaks if a `shorten` record keeps its source body, or is not listed in `shortened`.
    #[test]
    fn a_shortened_record_is_listed() {
        let mut m = mapping();
        m.shorten.insert("F-1".into(), "Short.".into());
        let p = plan(&m, TRACKER);
        no_problems(&p);
        assert_eq!(body(&p.writes[F1_PATH]), "Short.");
        assert_eq!(p.shortened, set(&[F1_PATH]));
        assert_eq!(body(&p.writes[F2_PATH]), F2_BODY);
    }

    /// Breaks if a key that another file holds is not refused, if the message does not name both
    /// and the `slug` override, or if the override does not settle it.
    #[test]
    fn a_key_collision_names_the_slug_override() {
        let mut m = mapping();
        let held = BTreeMap::from([(
            "2026-08-01-first-thing".to_string(),
            set(&["docs/issues/2026-08-01-first-thing.md"]),
        )]);
        let p = plan_with(&m, vec![source(X, TRACKER)], &BTreeMap::new(), &held, None);
        assert_problem(
            &p,
            "F-1: the key 2026-08-01-first-thing is already used by docs/issues/2026-08-01-first-thing.md. Set overrides.F-1.slug.",
        );
        m.overrides.insert("F-1".into(), slug_override("first-alt"));
        let p = plan_with(&m, vec![source(X, TRACKER)], &BTreeMap::new(), &held, None);
        no_problems(&p);
        assert!(p
            .writes
            .contains_key("docs/trackers/ledgers/x/2026-08-01-first-alt.md"));
    }

    /// Breaks if a title with no ASCII letter or digit panics, passes, or names no override.
    #[test]
    fn an_unsluggable_title_is_refused_not_a_panic() {
        let text = "## R-1 — 日本語\n\n**Status:** open\n";
        let mut m = mapping();
        m.sections.clear();
        let p = plan(&m, text);
        assert_eq!(
            p.problems,
            vec!["R-1: the title \"日本語\" gives no file name. Set overrides.R-1.slug."]
        );
        m.overrides.insert("R-1".into(), slug_override("r-one"));
        let p = plan(&m, text);
        no_problems(&p);
        let path = "docs/trackers/ledgers/x/2026-08-10-r-one.md";
        assert_eq!(fm(&p.writes[path]).title.as_deref(), Some("日本語"));
    }

    /// The key space of a re-run holds the entries and the readme of the first run. Breaks if a
    /// path that the plan writes, or a source it deletes, counts as a collision.
    #[test]
    fn a_rerun_does_not_collide_with_its_own_untracked_entries() {
        let m = mapping();
        let mut held = BTreeMap::from([
            ("2026-08-01-first-thing".to_string(), set(&[F1_PATH])),
            ("2026-08-02-second-thing".to_string(), set(&[F2_PATH])),
            ("x".to_string(), set(&[X, README])),
        ]);
        let p = plan_with(&m, vec![source(X, TRACKER)], &BTreeMap::new(), &held, None);
        no_problems(&p);
        held.insert("x".into(), set(&[X, "docs/trackers/streams/x/readme.md"]));
        let p = plan_with(&m, vec![source(X, TRACKER)], &BTreeMap::new(), &held, None);
        assert_eq!(
            p.problems,
            vec!["the collection key x is already used by docs/trackers/streams/x/readme.md. Set target.key."]
        );
    }

    /// Breaks if another file's bare token is rewritten (a non-empty token set), if its old-key
    /// citation is left, if an unchanged file is written, or if a kept key rewrites anything.
    #[test]
    fn a_key_change_rewrites_old_key_citations_in_other_files_only() {
        let other = BTreeMap::from([
            (
                "docs/notes.md".to_string(),
                "See F-1 and [[x:F-1]].\n".to_string(),
            ),
            ("docs/plain.md".to_string(), "See F-2.\n".to_string()),
        ]);
        let mut m = mapping();
        let p = plan_with(&m, vec![source(X, TRACKER)], &other, &BTreeMap::new(), None);
        no_problems(&p);
        assert!(!p.writes.contains_key("docs/notes.md"));

        m.target.key = Some("y".into());
        let p = plan_with(&m, vec![source(X, TRACKER)], &other, &BTreeMap::new(), None);
        no_problems(&p);
        assert_eq!((p.old_key.as_str(), p.new_key.as_str()), ("x", "y"));
        assert_eq!(p.writes["docs/notes.md"], "See F-1 and [[y:F-1]].\n");
        assert!(!p.writes.contains_key("docs/plain.md"));
        let f1 = &p.writes["docs/trackers/ledgers/y/2026-08-01-first-thing.md"];
        assert!(body(f1).contains("See [[y:F-2]] and"));
    }

    fn existing_schema() -> CollectionSchema {
        let extra: BTreeMap<String, Value> = serde_json::from_value(json!({
            "entry_kinds": ["finding"],
            "statuses": {"open": "open", "parked": "deferred", "fixed": "terminal"},
            "initial_status": "open"
        }))
        .unwrap();
        let text = collection_readme(Lifecycle::Ledger, "X", &extra, "").unwrap();
        parse_schema(Lifecycle::Ledger, &fm(&text)).unwrap()
    }

    fn existing_mapping() -> Mapping {
        let mut m = mapping();
        m.target.existing = true;
        m.schema = None;
        m.sections[0].to = "drop".into();
        m
    }

    /// Breaks if an equal entry is written again, a missing one is not added, a changed one is
    /// overwritten or skipped without a problem, or the readme is written in existing mode.
    #[test]
    fn existing_mode_adds_only_missing_tokens_and_refuses_a_changed_one() {
        let m = existing_mapping();
        let schema = existing_schema();
        let entry = |body: &str| ExistingEntry {
            rel_path: F1_PATH.into(),
            text: format!("---\nkind: entry\ntitle: First thing\n---\n\n{body}\n"),
            token: Some("F-1".into()),
        };
        let none = BTreeMap::new();
        let p = plan_with(
            &m,
            vec![source(X, TRACKER)],
            &none,
            &BTreeMap::new(),
            Some((&schema, None, vec![entry(F1_BODY)])),
        );
        no_problems(&p);
        assert_eq!(keys(&p), vec![F2_PATH]);
        assert_eq!(p.readme_path, None);
        assert!(
            matches!(&range_of(&p, "F-1").owner, Owner::Record { entry, .. } if entry == F1_PATH)
        );
        assert_eq!(
            p.review.dropped,
            vec!["docs/trackers/x.md lines 4-12 (\"# X log\")"]
        );

        let p = plan_with(
            &m,
            vec![source(X, TRACKER)],
            &none,
            &BTreeMap::new(),
            Some((
                &schema,
                None,
                vec![entry("## F-1 — First thing\n\nChanged.")],
            )),
        );
        assert_eq!(
            p.problems,
            vec!["F-1: entry docs/trackers/ledgers/x/2026-08-01-first-thing.md exists and its body is not the body of the record at line 13 of docs/trackers/x.md. Make the two equal, or remove the entry."]
        );
        assert!(!p.writes.contains_key(F1_PATH));

        // A readme section has nowhere to go when the readme is not written.
        let mut m = existing_mapping();
        m.sections[0].to = "readme".into();
        let p = plan_with(
            &m,
            vec![source(X, TRACKER)],
            &none,
            &BTreeMap::new(),
            Some((&schema, None, vec![])),
        );
        assert_problem(
            &p,
            "the section at line 4 of docs/trackers/x.md goes to readme, but target.existing is true and the readme is not written. Set its sections entry to drop.",
        );
    }

    /// Breaks if a `files` entry gets a dated stem, or its title or body change.
    #[test]
    fn a_files_source_keeps_its_lowercase_stem() {
        let mut m = mapping();
        m.sources = vec![SourceSpec {
            path: "docs/trackers/ic".into(),
            kind: "files".into(),
            level: None,
        }];
        m.sections.clear();
        m.columns.clear();
        let text = "## IC-10 — Cluster ten\n\n**Status:** open\nBody.\n";
        let mut s = source("docs/trackers/ic/IC-10.md", text);
        s.kind = SourceKind::Whole;
        s.fixed_stem = Some("ic-10".into());
        let p = plan_with(&m, vec![s], &BTreeMap::new(), &BTreeMap::new(), None);
        no_problems(&p);
        let path = "docs/trackers/ledgers/ic/ic-10.md";
        assert_eq!(keys(&p), vec![path, "docs/trackers/ledgers/ic/readme.md"]);
        assert_eq!(
            body(&p.writes[path]),
            "## IC-10 — Cluster ten\n\n**Status:** open\nBody."
        );
        assert_eq!(
            fm(&p.writes[path]).title.as_deref(),
            Some("IC-10 — Cluster ten")
        );
        assert_eq!(
            p.ranges,
            vec![OwnedRange {
                source: "docs/trackers/ic/IC-10.md".into(),
                lines: 1..=4,
                owner: Owner::Record {
                    entry: path.into(),
                    token: None,
                    heading: None,
                    row: None,
                },
            }]
        );
        assert_eq!(
            p.deletes,
            set(&[
                "docs/trackers/ic/IC-10.md",
                "docs/augmentations/docs-trackers-ic-IC-10.yaml"
            ])
        );
    }

    /// Breaks if a range is lost, doubled or given to the wrong owner, or if a section with no
    /// `sections` entry gets a default owner.
    #[test]
    fn the_plan_ranges_tile_every_source() {
        let p = plan(&mapping(), TRACKER);
        let record = |entry: &str, token: &str, row: &str| Owner::Record {
            entry: entry.into(),
            token: Some(token.into()),
            heading: None,
            row: Some(row.into()),
        };
        let r = |lines: Lines, owner: Owner| OwnedRange {
            source: X.into(),
            lines,
            owner,
        };
        assert_eq!(
            p.ranges,
            vec![
                r(1..=3, Owner::Frontmatter),
                r(4..=12, Owner::Readme),
                r(13..=17, record(F1_PATH, "F-1", F1_ROW)),
                r(18..=22, record(F2_PATH, "F-2", F2_ROW)),
            ]
        );
        let lines: Vec<Lines> = p.ranges.iter().map(|r| r.lines.clone()).collect();
        assert!(partition_problems(22, &lines).is_empty());

        let mut m = mapping();
        m.sections.clear();
        let p = plan(&m, TRACKER);
        assert_problem(
            &p,
            "section at line 4 of docs/trackers/x.md (\"# X log\") has no sections entry",
        );
        assert_problem(&p, "lines 4-12 of docs/trackers/x.md have no owner");
        assert!(p.ranges.iter().all(|r| !r.lines.contains(&4)));
    }

    /// `### R-9b` (line 17) in `records_extra` cuts F-1, so `### Notes` (19-21) becomes a section
    /// that the inventory (split without extras) never listed. Breaks if that section gets an
    /// owner without a `sections` entry, or if the entry does not settle it.
    #[test]
    fn a_section_the_inventory_never_listed_needs_a_sections_entry() {
        let text = TRACKER.replace(
            "See F-2 and [F-2](x.md).\n",
            "See F-2 and [F-2](x.md).\n### R-9b — Extra\nText.\n### Notes\nMore.\n",
        );
        let mut m = mapping();
        m.records_extra.push(ExtraRecordSpec {
            source: X.into(),
            line: 17,
            token: "R-9b".into(),
        });
        m.status_none = Some("open".into());
        let p = plan(&m, &text);
        assert_problem(
            &p,
            "section at line 19 of docs/trackers/x.md (\"### Notes\") has no sections entry",
        );
        assert_problem(&p, "lines 19-21 of docs/trackers/x.md have no owner");
        assert!(p.ranges.iter().all(|r| !r.lines.contains(&19)));

        m.sections.push(SectionSpec {
            source: X.into(),
            start: 19,
            to: "drop".into(),
            confirm: false,
        });
        let p = plan(&m, &text);
        no_problems(&p);
        assert_eq!(range_of(&p, "F-1").lines, 13..=16);
        assert_eq!(range_of(&p, "R-9b").lines, 17..=18);
        assert_eq!(
            p.review.dropped,
            vec!["docs/trackers/x.md lines 19-21 (\"### Notes\")"]
        );
        assert_eq!(p.review.no_row, vec!["R-9b"]);
    }

    /// Breaks if a `sections` or `possibles_inside` entry that names nothing passes.
    #[test]
    fn stale_sections_and_possibles_inside_entries_are_problems() {
        let mut m = mapping();
        m.sections.push(SectionSpec {
            source: X.into(),
            start: 99,
            to: "drop".into(),
            confirm: false,
        });
        m.possibles_inside.push(PossibleSpec {
            source: X.into(),
            line: 5,
        });
        let p = plan(&m, TRACKER);
        assert_eq!(
            p.problems,
            vec![
                "the possibles_inside entry at line 5 of docs/trackers/x.md names no possible record",
                "the sections entry at line 99 of docs/trackers/x.md names no section",
            ]
        );
    }

    /// Breaks if a mapped status or entry kind that the collection does not declare passes, in a
    /// new collection or an existing one, or if a constant status goes through `status_map`.
    #[test]
    fn mapped_values_must_be_declared_by_the_collection() {
        let mut m = mapping();
        m.status_map.insert("fixed".into(), "done".into());
        m.status_none = Some("gone".into());
        m.overrides.insert(
            "F-1".into(),
            Override {
                status: Some("nope".into()),
                ..no_override()
            },
        );
        m.entry_kind =
            crate::librarian::tracker_migration::mapping::EntryKindRule::One("bug".into());
        let p = plan(&m, TRACKER);
        assert_eq!(
            p.problems,
            vec![
                "status_map \"fixed\" = \"done\": \"done\" is not a declared status of the collection",
                "status_none \"gone\" is not a declared status of the collection",
                "overrides.F-1.status \"nope\" is not a declared status of the collection",
                "entry_kind \"bug\" is not a declared entry kind of the collection",
            ]
        );

        let mut m = mapping();
        m.status_source = StatusSource::Constant("x".into());
        let p = plan(&m, TRACKER);
        assert_eq!(
            p.problems,
            vec!["status_source constant \"x\" is not a declared status of the collection"]
        );
        m.status_source = StatusSource::Constant("parked".into());
        m.status_map.clear();
        m.overrides.insert(
            "F-1".into(),
            Override {
                revisit_by: Some(d("2026-12-01")),
                ..no_override()
            },
        );
        m.overrides.insert(
            "F-2".into(),
            Override {
                revisit_by: Some(d("2026-12-01")),
                ..no_override()
            },
        );
        let p = plan(&m, TRACKER);
        no_problems(&p);
        assert_eq!(
            fm(&p.writes["docs/trackers/ledgers/x/2026-08-02-second-thing.md"])
                .status
                .as_deref(),
            Some("parked")
        );

        let mut m = existing_mapping();
        m.status_map.insert("fixed".into(), "done".into());
        let schema = existing_schema();
        let p = plan_with(
            &m,
            vec![source(X, TRACKER)],
            &BTreeMap::new(),
            &BTreeMap::new(),
            Some((&schema, None, vec![])),
        );
        assert_eq!(
            p.problems,
            vec!["status_map \"fixed\" = \"done\": \"done\" is not a declared status of the collection"]
        );
    }

    /// Breaks if a second row for one token in one index table is silently dropped or chosen.
    #[test]
    fn the_same_token_twice_in_one_index_table_is_refused() {
        let text = TRACKER.replace(
            &format!("{F2_ROW}\n"),
            &format!("{F2_ROW}\n| F-1 | open | Again | 2026-08-05 |\n"),
        );
        let p = plan(&mapping(), &text);
        assert_problem(
            &p,
            "token F-1 has two rows in the index table at line 8 of docs/trackers/x.md. Keep one row.",
        );
    }

    /// Breaks if a `WriteProblem` text reaches `problems`, or the message does not name the token
    /// and the override that settles it.
    #[test]
    fn a_date_problem_names_the_override_not_the_raw_write_problem() {
        let text = TRACKER.replace("**Closed:** 2026-09-01", "**Closed:** 2027-01-01");
        let p = plan(&mapping(), &text);
        assert_eq!(
            p.problems,
            vec!["F-2: closed 2027-01-01 is after today. Set overrides.F-2.closed."]
        );
    }

    /// Breaks if a token defined twice (in two sources, or twice in one) is planned.
    #[test]
    fn a_token_defined_twice_is_refused() {
        let mut m = mapping();
        m.sources.push(SourceSpec {
            path: "docs/trackers/x-archive.md".into(),
            kind: "file".into(),
            level: None,
        });
        let archive = source(
            "docs/trackers/x-archive.md",
            "## F-1 — Again\n\n**Status:** open\n",
        );
        let p = plan_with(
            &m,
            vec![source(X, TRACKER), archive],
            &BTreeMap::new(),
            &BTreeMap::new(),
            None,
        );
        assert_problem(
            &p,
            "token F-1 is defined at line 13 of docs/trackers/x.md and at line 1 of docs/trackers/x-archive.md. Keep one definition.",
        );

        let text = format!("{TRACKER}\n## F-1 — Again\n\n**Status:** open\n");
        let p = plan(&mapping(), &text);
        assert_problem(
            &p,
            "token F-1 has two record headings in docs/trackers/x.md, at line 13 and at line 24. Keep one.",
        );
    }
}
