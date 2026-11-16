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
use super::inventory::{dropped_frontmatter, dropped_frontmatter_line};
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
use crate::librarian::identity::grammar::{extract, Reference};
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
            // The entry frontmatter replaces it: list each key that no entry keeps (review C1).
            for k in dropped_frontmatter(&s.text) {
                review.dropped.push(dropped_frontmatter_line(&s.path, &k));
            }
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
    let ctx = EntryCtx {
        m,
        schema: schema.as_ref(),
        template: template.as_deref(),
        target_dir: &target_dir,
        now: input.now,
        key_paths: input.key_paths,
        source_paths: &source_paths,
        existing: existing_entries,
    };
    for it in &items {
        let s = &sources[it.src];
        let n = it.name.as_str();
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

        let mut out = EntryOut {
            problems: &mut problems,
            review: &mut review,
            writes: &mut writes,
            shortened: &mut shortened,
            planned_keys: &mut planned_keys,
        };
        let entry = plan_entry(&ctx, &mut out, it, s, &body, short.is_some());
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
    folded_key_problems(
        m,
        sources,
        input.other_markdown,
        &ranges,
        &rows,
        &defined,
        &old_key,
        &new_key,
        &mut problems,
    );

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
/// What every entry of one plan reads.
struct EntryCtx<'a> {
    m: &'a Mapping,
    schema: Option<&'a CollectionSchema>,
    template: Option<&'a str>,
    target_dir: &'a str,
    now: NaiveDate,
    key_paths: &'a BTreeMap<String, BTreeSet<String>>,
    source_paths: &'a BTreeSet<&'a str>,
    existing: &'a [ExistingEntry],
}

/// What the entries of one plan add to it.
struct EntryOut<'a> {
    problems: &'a mut Vec<String>,
    review: &'a mut Review,
    writes: &'a mut BTreeMap<String, String>,
    shortened: &'a mut BTreeSet<String>,
    planned_keys: &'a mut BTreeMap<String, String>,
}

/// Steps 3-5 for one record with its `body`: the existing entry it equals, or its status, dates,
/// fields, key and place, rendered into `out.writes`. The path of its entry, or `None` when a
/// problem stops it.
fn plan_entry(
    ctx: &EntryCtx,
    out: &mut EntryOut,
    it: &Item,
    s: &SourceText,
    body: &str,
    short: bool,
) -> Option<String> {
    let m = ctx.m;
    let n = it.name.as_str();
    let ov = m.overrides.get(n);
    let row = it.row;
    let problems = &mut *out.problems;
    if let Some(t) = &it.token {
        let found = ctx
            .existing
            .iter()
            .find(|e| e.token.as_deref() == Some(t.as_str()));
        if let Some(e) = found {
            if same_body(&e.text, body) {
                return Some(e.rel_path.clone());
            }
            problems.push(format!(
                "{t}: entry {} exists and its body is not the body of the record at line {} of {}. Make the two equal, or remove the entry.",
                e.rel_path, it.heading_line, s.path
            ));
            return None;
        }
    }
    let status = match status_of(m, n, ov, &it.facts, row) {
        Ok(status) => status,
        Err(msg) => {
            problems.push(msg);
            return None;
        }
    };
    let schema = ctx.schema?;
    // An undeclared status or kind is already a problem that names its mapping key.
    let class = schema.statuses.get(&status).copied()?;
    let kind = entry_kind_of(m, it, problems)?;
    if !schema.entry_kinds.contains(&kind) {
        return None;
    }
    let fields = fields_of(m, schema, n, row, problems);
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
    let dates = match derive_dates(&inputs, ctx.now) {
        Ok(dates) => dates,
        Err(found) => {
            problems.extend(found);
            return None;
        }
    };
    if !dates.derived.is_empty() {
        out.review
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
                return None;
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
        body: Some(body),
    };
    let rules = RenderRules {
        refuse_past_revisit: false,
        fixed_stem,
    };
    let rendered = render_entry(
        schema,
        ctx.target_dir,
        ctx.template,
        &new_entry,
        &dates.dates,
        ctx.now,
        &rules,
    );
    let file = match rendered {
        Ok(file) => file,
        Err(found) => {
            problems.extend(found.iter().map(|p| write_problem_text(n, p)));
            return None;
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
                return None;
            }
        }
    } else {
        file.rel_path.clone()
    };
    let key = file.key.as_str();
    if let Some(first) = out.planned_keys.insert(key.to_string(), n.to_string()) {
        problems.push(format!(
            "{first} and {n} both get the key {key}. Set overrides.{n}.slug."
        ));
    }
    for held in ctx.key_paths.get(key).into_iter().flatten() {
        if *held != rel && !ctx.source_paths.contains(held.as_str()) {
            problems.push(format!(
                "{n}: the key {key} is already used by {held}. Set overrides.{n}.slug."
            ));
        }
    }
    if short {
        out.shortened.insert(rel.clone());
    }
    out.writes.insert(rel.clone(), file.text);
    Some(rel)
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

/// Citations of the key of a folded source other than the first. The run deletes that source and
/// rewrites only `old_key`, so such a citation would stop resolving: refuse it (operator ruling;
/// no rewrite of several old keys). A key equal to `old_key` or `new_key` still resolves. One
/// problem per file, at its first such citation: a source whose carried lines (a record or a
/// readme section) hold one, or another markdown file.
///
/// The index row of every carried record is scanned too, one problem per row: `expected_body`
/// appends it verbatim, and its source line can lie in a dropped section, which the scan of
/// carried lines does not read. Because the row is never rewritten, a row citation of `old_key`
/// is also refused when the key changes.
#[allow(clippy::too_many_arguments)]
fn folded_key_problems(
    m: &Mapping,
    sources: &[SourceText],
    other_markdown: &BTreeMap<String, String>,
    ranges: &[(usize, OwnedRange)],
    rows: &BTreeMap<String, (usize, &IndexRow)>,
    defined: &BTreeSet<String>,
    old_key: &str,
    new_key: &str,
    problems: &mut Vec<String>,
) {
    let folded: BTreeSet<String> = m
        .sources
        .iter()
        .skip(1)
        .map(|s| path_stem(&s.path))
        .filter(|k| k != old_key && k != new_key)
        .collect();
    let in_carried_lines = |i: usize, line: u32| {
        ranges.iter().any(|(j, r)| {
            *j == i
                && matches!(r.owner, Owner::Record { .. } | Owner::Readme)
                && r.lines.contains(&line)
        })
    };
    for (i, row) in rows
        .iter()
        .filter(|(token, _)| defined.contains(*token))
        .map(|(_, v)| *v)
    {
        let (path, line) = (&sources[i].path, row.line);
        // A folded key on a carried line is already reported by the scan of that source.
        let carried = in_carried_lines(i, line);
        let found = extract(&row.raw).into_iter().find_map(|c| {
            let key = match &c.reference {
                Reference::Entry {
                    repo: None, key, ..
                }
                | Reference::File { repo: None, key } => key.as_str(),
                _ => return None,
            };
            if folded.contains(key) && !carried {
                Some(format!(
                    "{path} line {line} cites {key}, which this run deletes; that citation would stop resolving. Rewrite it first, or fold the sources one at a time."
                ))
            } else if old_key != new_key && key == old_key {
                Some(format!(
                    "{path} line {line} has an index row that cites {old_key}. The row is carried verbatim and the key changes to {new_key}, so that citation would stop resolving. Rewrite the citation first, or keep the key."
                ))
            } else {
                None
            }
        });
        problems.extend(found);
    }
    if folded.is_empty() {
        return;
    }
    let first_hit = |text: &str, carried: &dyn Fn(u32) -> bool| -> Option<(u32, String)> {
        extract(text).into_iter().find_map(|c| {
            let key = match &c.reference {
                Reference::Entry {
                    repo: None, key, ..
                }
                | Reference::File { repo: None, key } => key.as_str(),
                _ => return None,
            };
            (folded.contains(key) && carried(c.line)).then(|| (c.line, key.to_string()))
        })
    };
    let mut push = |file: &str, hit: Option<(u32, String)>| {
        if let Some((line, stem)) = hit {
            problems.push(format!(
                "{file} line {line} cites {stem}, which this run deletes; that citation would stop resolving. Rewrite it first, or fold the sources one at a time."
            ));
        }
    };
    for (i, s) in sources.iter().enumerate() {
        let carried = |line: u32| {
            ranges.iter().any(|(j, r)| {
                *j == i
                    && matches!(r.owner, Owner::Record { .. } | Owner::Readme)
                    && r.lines.contains(&line)
            })
        };
        push(&s.path, first_hit(&s.text, &carried));
    }
    let source_paths: BTreeSet<&str> = sources.iter().map(|s| s.path.as_str()).collect();
    for (path, text) in other_markdown {
        if !source_paths.contains(path.as_str()) {
            push(path, first_hit(text, &|_| true));
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
mod tests;
