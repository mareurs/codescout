//! What `codescout trackers inventory` reports about a set of tracker sources.
//!
//! Spec: `docs/superpowers/specs/2026-11-15-tracker-migration-design.md`
//! § The per-tracker loop, step 2 (Inventory).
//!
//! These types are the `inventory --json` output. The `migrating-trackers` skill
//! (Plan L-E2) reads that JSON, and [`super::mapping::mapping_problems`] checks a
//! mapping against it, so the field names are an interface. [`build_inventory`] builds it
//! from texts the CLI read at `HEAD`; it reads nothing itself.
//!
//! Lines are 1-indexed lines of `text.lines()`, as in [`super::records`]. Paths are
//! repo-relative.

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use serde::Serialize;

use super::index_rows::{index_tables, normalise_status, record_facts, IndexRow};
use super::mapping::StatusSource;
use super::records::{split_source, Possible, Repeat, SourceKind};
use crate::librarian::cutover::code_refs::paths_named_in;
use crate::librarian::frontmatter;
use crate::librarian::identity::grammar::{extract_spanned, Reference};
use crate::librarian::identity::key::key_of;
use crate::util::markdown_fence::FenceState;

/// The whole report: one entry per source, the mentions of every source outside
/// the sources, and the sidecar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Inventory {
    pub sources: Vec<SourceInventory>,
    pub mentions: Mentions,
    pub sidecar: Option<SidecarInfo>,
}

/// One source file. A `files` source is one entry per markdown file of its folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceInventory {
    pub path: String,
    pub line_count: u32,
    pub records: Vec<RecordRow>,
    /// The non-record sections (the frontmatter is not a section).
    pub sections: Vec<SectionRow>,
    /// Headings that start with a token and are not record headings.
    pub possibles: Vec<Possible>,
    /// Tokens defined on more than one heading.
    pub repeats: Vec<Repeat>,
    /// The index tables of the non-record sections.
    pub tables: Vec<TableRow>,
    /// Normalised raw status value to the number of records that carry it.
    pub status_values: BTreeMap<String, usize>,
    /// The token of each record with no raw status (its title when it has no token).
    pub no_status: Vec<String>,
    /// Tokens of index rows that have no record heading.
    pub row_only: Vec<String>,
    /// Tokens of records that have no index row.
    pub no_row: Vec<String>,
    /// Tokens that have a row in more than one index table.
    pub duplicate_rows: Vec<String>,
    /// Records whose body status and index status differ.
    pub disagreements: Vec<Disagreement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecordRow {
    pub token: Option<String>,
    pub heading_line: u32,
    pub level: usize,
    /// The heading text without its `TOKEN — ` part.
    pub title: String,
    /// First line of the record.
    pub first: u32,
    /// Last line of the record.
    pub last: u32,
    /// The status as the source states it, normalised; `None` when it states none.
    pub raw_status: Option<String>,
    /// Where `raw_status` came from: `"body"` (a `**Status:**` line) or
    /// `"index:<header>"` (an index column); `None` when there is no status.
    pub status_origin: Option<String>,
    /// Every date the record states, in source order.
    pub stated_dates: Vec<NaiveDate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SectionRow {
    /// First line; the `sections` entries of a mapping are keyed by it.
    pub start: u32,
    pub last: u32,
    pub first_line: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TableRow {
    /// First line of the section that holds the table; `index_table` names it.
    pub section_start: u32,
    /// Header texts, lowercased and trimmed.
    pub header: Vec<String>,
    /// The number of rows whose first cell is a token.
    pub rows: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Disagreement {
    pub token: String,
    /// The status the body states.
    pub body: String,
    /// The status the index row states.
    pub index: String,
}

/// Every mention of a source outside the sources.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mentions {
    /// `[[key…]]` citations.
    pub citations: Vec<Hit>,
    /// Path mentions of a source file.
    pub paths: Vec<PathHit>,
    /// A source's frontmatter `id:` written in prose.
    pub ids: Vec<Hit>,
    /// Non-markdown files that name a source.
    pub code: Vec<CodeHit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hit {
    pub file: String,
    pub line: u32,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PathHit {
    pub file: String,
    pub line: u32,
    /// Inline code holds the mention.
    pub in_code: bool,
    /// The file is frozen (`CLAUDE.md`, a memory); nothing edits it.
    pub frozen: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodeHit {
    pub file: String,
    pub line: u32,
    /// `"code_refs"` or `"grep"`: the search that found it.
    pub origin: String,
}

/// The sidecar `docs/augmentations/docs-trackers-<stem>.yaml`, when it exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SidecarInfo {
    pub path: String,
    pub prompt: Option<String>,
}

/// The inventory of `sources` (`(repo-relative path, text at HEAD, kind)`), split without
/// `records_extra` (the mapping does not exist yet).
///
/// - **Status.** `BodyLabel` (and `Constant`, whose values the mapping never reads) takes
///   each record's `**Status:**` line, origin `"body"`. `IndexColumn(h)` takes the cell of
///   column `h` (trimmed, lowercased, as [`TableRow::header`] holds it) in the record's
///   **first** index row, origin `"index:<h>"`; a record with no row, or an empty cell, has
///   no status. Every value is [`normalise_status`]d, so `status_values` holds the words a
///   `status_map` must cover.
/// - **Index rows.** A record's row is its first row in source order. `duplicate_rows`
///   lists every token with more than one row: in two tables, and also twice in one table
///   (the planner must refuse the second case, which `index_table` cannot settle).
///   `no_row` lists every record with a token and no row, also in a source with no index
///   table. `row_only` lists the row tokens no record heading defines, in source order.
/// - **Disagreements** compare the body status with the first row's cell of the status
///   column (`IndexColumn(h)`, else a column headed `status`), both normalised.
/// - **Mentions** are searched in `other_markdown`, skipping every source path: see
///   [`Mentions`]. `frozen` answers for a file holding a path mention. `code_hits` (already
///   merged, [`merge_code_hits`]) are sorted by file and line.
pub fn build_inventory(
    sources: &[(String, String, SourceKind)],
    status: &StatusSource,
    other_markdown: &BTreeMap<String, String>,
    frozen: &dyn Fn(&str) -> bool,
    code_hits: Vec<CodeHit>,
    sidecar: Option<SidecarInfo>,
) -> Inventory {
    let mut code = code_hits;
    code.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    let mut mentions = mentions_of(sources, other_markdown, frozen);
    mentions.code = code;
    Inventory {
        sources: sources
            .iter()
            .map(|(path, text, kind)| source_inventory(path, text, kind, status))
            .collect(),
        mentions,
        sidecar,
    }
}

/// One [`SourceInventory`]; see [`build_inventory`] for the rules.
fn source_inventory(
    path: &str,
    text: &str,
    kind: &SourceKind,
    status: &StatusSource,
) -> SourceInventory {
    let split = split_source(text, kind, &[]);
    let tables = index_tables(text, &split);
    let mut rows_of: BTreeMap<&str, Vec<&IndexRow>> = BTreeMap::new();
    for table in &tables {
        for row in &table.rows {
            rows_of.entry(row.token.as_str()).or_default().push(row);
        }
    }
    let index_column = match status {
        StatusSource::IndexColumn(header) => Some(header.trim().to_lowercase()),
        StatusSource::BodyLabel | StatusSource::Constant(_) => None,
    };
    let compare_column = index_column.clone().unwrap_or_else(|| "status".to_string());

    let mut records = Vec::new();
    let mut status_values: BTreeMap<String, usize> = BTreeMap::new();
    let mut no_status = Vec::new();
    let mut no_row = Vec::new();
    let mut disagreements = Vec::new();
    for rec in &split.records {
        let facts = record_facts(text, rec);
        let body = facts.raw_status_body.as_deref().and_then(normalise_status);
        let row = rec
            .token
            .as_deref()
            .and_then(|t| rows_of.get(t))
            .and_then(|rows| rows.first().copied());
        let cell = |column: &str| {
            row.and_then(|r| r.cells.get(column))
                .and_then(|c| normalise_status(c))
        };
        let (raw_status, status_origin) = match &index_column {
            Some(column) => {
                let value = cell(column);
                let origin = value.as_ref().map(|_| format!("index:{column}"));
                (value, origin)
            }
            None => (body.clone(), body.as_ref().map(|_| "body".to_string())),
        };
        match &raw_status {
            Some(value) => *status_values.entry(value.clone()).or_default() += 1,
            None => no_status.push(rec.token.clone().unwrap_or_else(|| rec.title.clone())),
        }
        if let Some(token) = &rec.token {
            if row.is_none() {
                no_row.push(token.clone());
            }
            if let (Some(b), Some(i)) = (&body, cell(&compare_column)) {
                if *b != i {
                    disagreements.push(Disagreement {
                        token: token.clone(),
                        body: b.clone(),
                        index: i,
                    });
                }
            }
        }
        records.push(RecordRow {
            token: rec.token.clone(),
            heading_line: rec.heading_line,
            level: rec.level,
            title: rec.title.clone(),
            first: *rec.lines.start(),
            last: *rec.lines.end(),
            raw_status,
            status_origin,
            stated_dates: facts.stated_dates,
        });
    }

    let defined: BTreeSet<&str> = split
        .records
        .iter()
        .filter_map(|r| r.token.as_deref())
        .collect();
    let mut row_only: Vec<String> = Vec::new();
    for row in tables.iter().flat_map(|t| &t.rows) {
        if !defined.contains(row.token.as_str()) && !row_only.contains(&row.token) {
            row_only.push(row.token.clone());
        }
    }
    let duplicate_rows = rows_of
        .iter()
        .filter(|(_, rows)| rows.len() > 1)
        .map(|(token, _)| token.to_string())
        .collect();

    SourceInventory {
        path: path.to_string(),
        line_count: split.line_count,
        records,
        sections: split
            .sections
            .iter()
            .map(|s| SectionRow {
                start: s.start,
                last: *s.lines.end(),
                first_line: s.first_line.clone(),
            })
            .collect(),
        possibles: split.possibles.clone(),
        repeats: split.repeats.clone(),
        tables: tables
            .iter()
            .map(|t| TableRow {
                section_start: t.section_start,
                header: t.header.clone(),
                rows: t.rows.len(),
            })
            .collect(),
        status_values,
        no_status,
        row_only,
        no_row,
        duplicate_rows,
        disagreements,
    }
}

/// The citations, path mentions and id mentions of the sources in `other_markdown`; `code`
/// is left empty for the caller.
///
/// - A citation is a `[[key]]` or `[[key:TOKEN]]` with no repo part whose key is a source's
///   key; its `text` is the citation as written.
/// - A path mention is a line that names a source path, as written from the root or relative
///   to the file's folder (a link). One [`PathHit`] per line: `in_code` is true when every
///   mention on the line is in code (a fenced block, or an inline code span).
/// - An id mention is a line that holds a source's frontmatter `id:`; its `text` is the id.
fn mentions_of(
    sources: &[(String, String, SourceKind)],
    other_markdown: &BTreeMap<String, String>,
    frozen: &dyn Fn(&str) -> bool,
) -> Mentions {
    let paths: BTreeSet<&str> = sources.iter().map(|(p, _, _)| p.as_str()).collect();
    let keys: BTreeSet<String> = sources
        .iter()
        .filter_map(|(p, _, _)| key_of(p).ok().map(|k| k.as_str().to_string()))
        .collect();
    let ids: BTreeSet<String> = sources
        .iter()
        .filter_map(|(_, text, _)| frontmatter::parse(text).ok()?.0?.id)
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();

    let mut out = Mentions {
        citations: Vec::new(),
        paths: Vec::new(),
        ids: Vec::new(),
        code: Vec::new(),
    };
    for (file, text) in other_markdown {
        if paths.contains(file.as_str()) {
            continue;
        }
        for s in extract_spanned(text) {
            let key = match &s.citation.reference {
                Reference::File { repo: None, key }
                | Reference::Entry {
                    repo: None, key, ..
                } => key,
                _ => continue,
            };
            if keys.contains(key.as_str()) {
                out.citations.push(Hit {
                    file: file.clone(),
                    line: s.citation.line,
                    text: text.get(s.range.clone()).unwrap_or_default().to_string(),
                });
            }
        }

        let named = paths_named_in(file, text);
        let lines: BTreeSet<u32> = paths
            .iter()
            .filter_map(|p| named.get(*p))
            .flatten()
            .map(|(_, line)| *line)
            .collect();
        if !lines.is_empty() {
            let fenced = fenced_lines(text);
            let all: Vec<&str> = text.lines().collect();
            let is_frozen = frozen(file);
            for line in lines {
                let line_text = all.get(line as usize - 1).copied().unwrap_or_default();
                let in_code =
                    fenced.contains(&line) || !names_outside_code(file, line_text, &paths);
                out.paths.push(PathHit {
                    file: file.clone(),
                    line,
                    in_code,
                    frozen: is_frozen,
                });
            }
        }

        for (i, line) in text.lines().enumerate() {
            for id in ids.iter().filter(|id| line.contains(id.as_str())) {
                out.ids.push(Hit {
                    file: file.clone(),
                    line: i as u32 + 1,
                    text: id.clone(),
                });
            }
        }
    }
    out
}

/// The 1-indexed lines of `text` inside a fenced block, its fence lines included.
fn fenced_lines(text: &str) -> BTreeSet<u32> {
    let mut fence = FenceState::new();
    let mut out = BTreeSet::new();
    for (i, line) in text.lines().enumerate() {
        if fence.feed(line.trim_start()) || fence.in_fence() {
            out.insert(i as u32 + 1);
        }
    }
    out
}

/// Whether `line` of `file` names one of `paths` outside its inline code spans.
fn names_outside_code(file: &str, line: &str, paths: &BTreeSet<&str>) -> bool {
    let named = paths_named_in(file, &without_code_spans(line));
    paths.iter().any(|p| named.contains_key(*p))
}

/// `line` with each inline code span (a backtick run, text, and a closing run of the same
/// length) replaced by one space. A run with no closing run stays as text.
fn without_code_spans(line: &str) -> String {
    let bytes = line.as_bytes();
    let run_at = |i: usize| bytes[i..].iter().take_while(|&&b| b == b'`').count();
    let mut out = String::new();
    let mut i = 0;
    let mut kept = 0;
    while i < bytes.len() {
        if bytes[i] != b'`' {
            i += 1;
            continue;
        }
        let open = run_at(i);
        let mut j = i + open;
        let mut close = None;
        while j < bytes.len() {
            if bytes[j] == b'`' {
                let run = run_at(j);
                if run == open {
                    close = Some(j + run);
                    break;
                }
                j += run;
            } else {
                j += 1;
            }
        }
        match close {
            Some(end) => {
                out.push_str(&line[kept..i]);
                out.push(' ');
                kept = end;
                i = end;
            }
            None => i += open,
        }
    }
    out.push_str(&line[kept..]);
    out
}

/// One [`CodeHit`] per `(file, line)` that either search found, sorted by file and line. A
/// hit `CodeRefs` found has origin `"code_refs"`, whether or not `git grep` found it too; a
/// hit only `git grep` found has origin `"grep"`, so the grep-only hits are the readers
/// `CodeRefs` misses (data files such as JSON, a stem written without its path).
pub fn merge_code_hits(code_refs: &[(String, u32)], grep: &[(String, u32)]) -> Vec<CodeHit> {
    let mut merged: BTreeMap<(&str, u32), &str> = BTreeMap::new();
    for (file, line) in grep {
        merged.insert((file.as_str(), *line), "grep");
    }
    for (file, line) in code_refs {
        merged.insert((file.as_str(), *line), "code_refs");
    }
    merged
        .into_iter()
        .map(|((file, line), origin)| CodeHit {
            file: file.to_string(),
            line,
            origin: origin.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    /// One value of every type, so each field name and each JSON shape is pinned.
    /// A renamed field, a `skip`, a `rename_all` or a changed `Option` shape breaks it.
    #[test]
    fn inventory_types_round_trip_json() {
        let inv = Inventory {
            sources: vec![SourceInventory {
                path: "docs/trackers/x.md".into(),
                line_count: 60,
                records: vec![RecordRow {
                    token: Some("F-1".into()),
                    heading_line: 5,
                    level: 2,
                    title: "First".into(),
                    first: 5,
                    last: 14,
                    raw_status: Some("open".into()),
                    status_origin: Some("body".into()),
                    stated_dates: vec![d("2026-08-01")],
                }],
                sections: vec![SectionRow {
                    start: 3,
                    last: 4,
                    first_line: "# X".into(),
                }],
                possibles: vec![Possible {
                    line: 9,
                    token: "F-1a".into(),
                    enclosing: Some("F-1".into()),
                }],
                repeats: vec![Repeat {
                    token: "F-1".into(),
                    first_line: 5,
                    other_line: 11,
                    nested: true,
                }],
                tables: vec![TableRow {
                    section_start: 3,
                    header: vec!["id".into(), "status".into()],
                    rows: 1,
                }],
                status_values: BTreeMap::from([("open".to_string(), 1usize)]),
                no_status: vec![],
                row_only: vec!["F-7".into()],
                no_row: vec![],
                duplicate_rows: vec![],
                disagreements: vec![Disagreement {
                    token: "F-1".into(),
                    body: "open".into(),
                    index: "fixed".into(),
                }],
            }],
            mentions: Mentions {
                citations: vec![Hit {
                    file: "a.md".into(),
                    line: 2,
                    text: "[[x:F-1]]".into(),
                }],
                paths: vec![PathHit {
                    file: "CLAUDE.md".into(),
                    line: 4,
                    in_code: true,
                    frozen: true,
                }],
                ids: vec![],
                code: vec![CodeHit {
                    file: "scripts/x.json".into(),
                    line: 3,
                    origin: "grep".into(),
                }],
            },
            sidecar: Some(SidecarInfo {
                path: "docs/augmentations/docs-trackers-x.yaml".into(),
                prompt: None,
            }),
        };
        let text = serde_json::to_string(&inv).unwrap();
        let back: serde_json::Value = serde_json::from_str(&text).unwrap();
        let expected = json!({
            "sources": [{
                "path": "docs/trackers/x.md",
                "line_count": 60,
                "records": [{
                    "token": "F-1", "heading_line": 5, "level": 2, "title": "First",
                    "first": 5, "last": 14, "raw_status": "open", "status_origin": "body",
                    "stated_dates": ["2026-08-01"]
                }],
                "sections": [{"start": 3, "last": 4, "first_line": "# X"}],
                "possibles": [{"line": 9, "token": "F-1a", "enclosing": "F-1"}],
                "repeats": [{"token": "F-1", "first_line": 5, "other_line": 11, "nested": true}],
                "tables": [{"section_start": 3, "header": ["id", "status"], "rows": 1}],
                "status_values": {"open": 1},
                "no_status": [],
                "row_only": ["F-7"],
                "no_row": [],
                "duplicate_rows": [],
                "disagreements": [{"token": "F-1", "body": "open", "index": "fixed"}]
            }],
            "mentions": {
                "citations": [{"file": "a.md", "line": 2, "text": "[[x:F-1]]"}],
                "paths": [{"file": "CLAUDE.md", "line": 4, "in_code": true, "frozen": true}],
                "ids": [],
                "code": [{"file": "scripts/x.json", "line": 3, "origin": "grep"}]
            },
            "sidecar": {"path": "docs/augmentations/docs-trackers-x.yaml", "prompt": null}
        });
        assert_eq!(back, expected);
    }

    fn row(
        token: &str,
        heading: u32,
        title: &str,
        last: u32,
        status: Option<(&str, &str)>,
        dates: &[&str],
    ) -> RecordRow {
        RecordRow {
            token: Some(token.into()),
            heading_line: heading,
            level: 2,
            title: title.into(),
            first: heading,
            last,
            raw_status: status.map(|(s, _)| s.to_string()),
            status_origin: status.map(|(_, o)| o.to_string()),
            stated_dates: dates.iter().map(|s| d(s)).collect(),
        }
    }

    fn section(start: u32, last: u32, first_line: &str) -> SectionRow {
        SectionRow {
            start,
            last,
            first_line: first_line.into(),
        }
    }

    // 1 ---                              18 **Status:** open
    // 2 title: Log                       19
    // 3 ---                              20 ## F-3 — third
    // 4 # Log                            21 body with no status, seen 2026-09-09
    // 5                                  22
    // 6 | ID | Status | Title |          23 ## F-4 — fourth
    // 7 |----|--------|-------|          24 - **Status:** Fixed (by hand)
    // 8 | F-1 | fixed | first |          25
    // 9 | F-2 | open | second |         26 ## Old index
    // 10 | F-9 | open | row only |       27
    // 11 | F-2 | open | second again |   28 | ID | Status |
    // 12                                 29 |----|--------|
    // 13 ## F-1 — first (2026-08-01)     30 | F-3 | open |
    // 14 **Status:** Open — still seen   31 | F-1 | fixed |
    // 15 **Valid:** dated 2026-08-03     32
    // 16                                 33 ## R-56b — a hit
    // 17 ## F-2 — second                 34 text
    const LOG: &str = "---\ntitle: Log\n---\n# Log\n\n\
    | ID | Status | Title |\n|----|--------|-------|\n\
    | F-1 | fixed | first |\n| F-2 | open | second |\n\
    | F-9 | open | row only |\n| F-2 | open | second again |\n\n\
    ## F-1 — first (2026-08-01)\n**Status:** Open — still seen\n\
    **Valid:** dated 2026-08-03\n\n\
    ## F-2 — second\n**Status:** open\n\n\
    ## F-3 — third\nbody with no status, seen 2026-09-09\n\n\
    ## F-4 — fourth\n- **Status:** Fixed (by hand)\n\n\
    ## Old index\n\n| ID | Status |\n|----|--------|\n\
    | F-3 | open |\n| F-1 | fixed |\n\n\
    ## R-56b — a hit\ntext\n";

    fn inventory_of(text: &str, kind: SourceKind, status: StatusSource) -> Inventory {
        build_inventory(
            &[("docs/trackers/log.md".into(), text.into(), kind)],
            &status,
            &BTreeMap::new(),
            &|_| false,
            Vec::new(),
            None,
        )
    }

    /// Every field of one source, derived by hand from `LOG`. Breaks if: a status is not
    /// normalised (`Open — still seen` must count as `open`); a record with no status is
    /// counted; `no_status`, `row_only` or `no_row` miss or add a token; a same-table
    /// duplicate (`F-2`) or a cross-table one (`F-1`) is not in `duplicate_rows`; the
    /// disagreement compares with any row but the first (`F-1`'s first row says fixed); the
    /// stated dates drop the heading date; sections, tables or possibles are not copied
    /// from the split.
    #[test]
    fn inventory_counts_status_values_and_lists_gaps() {
        let inv = inventory_of(LOG, SourceKind::File, StatusSource::BodyLabel);
        let body = |s| Some((s, "body"));
        let expected = SourceInventory {
            path: "docs/trackers/log.md".into(),
            line_count: 34,
            records: vec![
                row(
                    "F-1",
                    13,
                    "first (2026-08-01)",
                    16,
                    body("open"),
                    &["2026-08-01", "2026-08-03"],
                ),
                row("F-2", 17, "second", 19, body("open"), &[]),
                row("F-3", 20, "third", 22, None, &[]),
                row("F-4", 23, "fourth", 25, body("fixed"), &[]),
            ],
            sections: vec![
                section(4, 12, "# Log"),
                section(26, 32, "## Old index"),
                section(33, 34, "## R-56b — a hit"),
            ],
            possibles: vec![Possible {
                line: 33,
                token: "R-56b".into(),
                enclosing: None,
            }],
            repeats: vec![],
            tables: vec![
                TableRow {
                    section_start: 4,
                    header: vec!["id".into(), "status".into(), "title".into()],
                    rows: 4,
                },
                TableRow {
                    section_start: 26,
                    header: vec!["id".into(), "status".into()],
                    rows: 2,
                },
            ],
            status_values: BTreeMap::from([("fixed".to_string(), 1), ("open".to_string(), 2)]),
            no_status: vec!["F-3".into()],
            row_only: vec!["F-9".into()],
            no_row: vec!["F-4".into()],
            duplicate_rows: vec!["F-1".into(), "F-2".into()],
            disagreements: vec![Disagreement {
                token: "F-1".into(),
                body: "open".into(),
                index: "fixed".into(),
            }],
        };
        assert_eq!(inv.sources, vec![expected]);
    }

    /// With an index column as the status source, the status comes from the record's first
    /// row and the origin names the column. Breaks if the body is read instead (`F-1` would
    /// be `open`, `F-4` would be `fixed`), if the origin is not `index:<header>`, or if a
    /// record with no row is not counted as having no status.
    #[test]
    fn an_index_column_status_comes_from_the_first_row() {
        let inv = inventory_of(
            LOG,
            SourceKind::File,
            StatusSource::IndexColumn("Status".into()),
        );
        assert_eq!(inv.sources.len(), 1);
        let src = &inv.sources[0];
        let statuses: Vec<(Option<&str>, Option<&str>)> = src
            .records
            .iter()
            .map(|r| (r.raw_status.as_deref(), r.status_origin.as_deref()))
            .collect();
        let index = Some("index:status");
        assert_eq!(
            statuses,
            vec![
                (Some("fixed"), index),
                (Some("open"), index),
                (Some("open"), index),
                (None, None),
            ]
        );
        assert_eq!(
            src.status_values,
            BTreeMap::from([("fixed".to_string(), 1), ("open".to_string(), 2)])
        );
        assert_eq!(src.no_status, vec!["F-4".to_string()]);
        assert_eq!(src.disagreements.len(), 1, "{:?}", src.disagreements);
    }

    /// A record with no token is named by its title in `no_status`, and has no index row to
    /// miss. Breaks if `no_status` holds an empty string or `no_row` lists a tokenless record.
    #[test]
    fn a_tokenless_record_is_named_by_its_title() {
        // 1 # Notes / 2 / 3 ## Alpha / 4 **Status:** open / 5 / 6 ## Beta / 7 text
        let text = "# Notes\n\n## Alpha\n**Status:** open\n\n## Beta\ntext\n";
        let inv = inventory_of(
            text,
            SourceKind::Sections { level: 2 },
            StatusSource::BodyLabel,
        );
        assert_eq!(inv.sources.len(), 1);
        let src = &inv.sources[0];
        assert_eq!(src.records.len(), 2);
        assert_eq!(src.no_status, vec!["Beta".to_string()]);
        assert_eq!(src.status_values, BTreeMap::from([("open".to_string(), 1)]));
        assert!(src.no_row.is_empty());
        assert_eq!(src.sections, vec![section(1, 2, "# Notes")]);
    }

    // `docs/notes/a.md`, numbered:
    // 1 # A
    // 2 See `docs/trackers/x.md` for the log.
    // 3 Also [the log](../trackers/x.md) and [[x:F-1]].
    // 4 ```
    // 5 cat docs/trackers/x.md
    // 6 ```
    // 7 Old id 0123456789abcdef here, and [[x]] too.
    // 8 Not ours: [[y:F-1]] and docs/trackers/xx.md
    const NOTE: &str = "# A\nSee `docs/trackers/x.md` for the log.\n\
    Also [the log](../trackers/x.md) and [[x:F-1]].\n```\ncat docs/trackers/x.md\n```\n\
    Old id 0123456789abcdef here, and [[x]] too.\n\
    Not ours: [[y:F-1]] and docs/trackers/xx.md\n";

    /// Mentions of `docs/trackers/x.md` in other markdown, hand-derived from `NOTE` and a
    /// memory. Breaks if: inline code (line 2) or a fence (line 5) is reported as prose, or a
    /// relative link (line 3) or a plain path (memory line 1) as code; a relative link is
    /// not resolved against its file's folder; `frozen` is not taken from the callback;
    /// `[[y:F-1]]` or `xx.md` counts; the frontmatter id is not found in prose; the source's
    /// own text is scanned; the code hits are not sorted by file and line.
    #[test]
    fn mentions_separate_inline_code_paths_from_prose_paths() {
        let source = "---\nid: 0123456789abcdef\n---\n# X\n## F-1 — a\nsee docs/trackers/x.md\n";
        let other = BTreeMap::from([
            ("docs/notes/a.md".to_string(), NOTE.to_string()),
            (
                ".codescout/memories/m.md".to_string(),
                "Read docs/trackers/x.md first.\n".to_string(),
            ),
            // The source itself: its own mentions are not outside it.
            ("docs/trackers/x.md".to_string(), source.to_string()),
        ]);
        let hit = |file: &str, line, origin: &str| CodeHit {
            file: file.into(),
            line,
            origin: origin.into(),
        };
        let inv = build_inventory(
            &[("docs/trackers/x.md".into(), source.into(), SourceKind::File)],
            &StatusSource::BodyLabel,
            &other,
            &|f| f.starts_with(".codescout/"),
            vec![
                hit("scripts/x.json", 2, "grep"),
                hit("scripts/a.py", 1, "code_refs"),
            ],
            None,
        );
        let path = |file: &str, line, in_code, frozen| PathHit {
            file: file.into(),
            line,
            in_code,
            frozen,
        };
        let text_hit = |line, text: &str| Hit {
            file: "docs/notes/a.md".into(),
            line,
            text: text.into(),
        };
        assert_eq!(
            inv.mentions,
            Mentions {
                citations: vec![text_hit(3, "[[x:F-1]]"), text_hit(7, "[[x]]")],
                paths: vec![
                    path(".codescout/memories/m.md", 1, false, true),
                    path("docs/notes/a.md", 2, true, false),
                    path("docs/notes/a.md", 3, false, false),
                    path("docs/notes/a.md", 5, true, false),
                ],
                ids: vec![text_hit(7, "0123456789abcdef")],
                code: vec![
                    hit("scripts/a.py", 1, "code_refs"),
                    hit("scripts/x.json", 2, "grep"),
                ],
            }
        );
    }

    /// A hit both searches find is one hit with origin `code_refs`; a hit only `git grep`
    /// finds keeps origin `grep`, so the difference is listed. Breaks if the merge prefers
    /// `grep`, keeps both copies of `scripts/a.py:3`, or drops a hit only one search found.
    #[test]
    fn a_hit_both_searches_find_is_one_code_refs_hit() {
        let code_refs = vec![
            ("scripts/a.py".to_string(), 3),
            ("tools/b.sh".to_string(), 7),
        ];
        let grep = vec![
            ("scripts/x.json".to_string(), 2),
            ("scripts/a.py".to_string(), 3),
            ("scripts/a.py".to_string(), 9),
        ];
        let hit = |file: &str, line, origin: &str| CodeHit {
            file: file.into(),
            line,
            origin: origin.into(),
        };
        assert_eq!(
            merge_code_hits(&code_refs, &grep),
            vec![
                hit("scripts/a.py", 3, "code_refs"),
                hit("scripts/a.py", 9, "grep"),
                hit("scripts/x.json", 2, "grep"),
                hit("tools/b.sh", 7, "code_refs"),
            ]
        );
    }
}
