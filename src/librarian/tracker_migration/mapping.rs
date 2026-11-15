//! The mapping file: the TOML the agent drafts and the user approves, which tells
//! `apply` how to turn the sources into one lifecycle collection.
//!
//! Spec: `docs/superpowers/specs/2026-11-15-tracker-migration-design.md`
//! § The mapping file.
//!
//! [`parse_mapping`] reads the text (the CLI reads the file through the read door)
//! and refuses unknown keys on every table. [`mapping_problems`] checks that the
//! mapping settles everything the inventory reported; `apply` refuses while it
//! returns anything. Each message names the item and the key that settles it.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::{Deserialize, Deserializer};

use super::inventory::{Inventory, SourceInventory};
use crate::librarian::lifecycle::layout::Lifecycle;

/// The values of `sources[].kind`.
const SOURCE_KINDS: [&str; 4] = ["file", "sections", "whole", "files"];

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mapping {
    pub sources: Vec<SourceSpec>,
    pub target: Target,
    #[serde(default)]
    pub schema: Option<toml::Table>,
    #[serde(default)]
    pub hot_view_none: Option<String>,
    #[serde(default)]
    pub readme_body: Option<String>,
    pub entry_kind: EntryKindRule,
    pub status_source: StatusSource,
    #[serde(default)]
    pub status_map: BTreeMap<String, String>,
    #[serde(default)]
    pub status_none: Option<String>,
    #[serde(default)]
    pub columns: BTreeMap<String, String>,
    #[serde(default)]
    pub row_title_column: Option<String>,
    #[serde(default)]
    pub index_table: Option<u32>,
    #[serde(default)]
    pub records_extra: Vec<ExtraRecordSpec>,
    #[serde(default)]
    pub possibles_inside: Vec<PossibleSpec>,
    #[serde(default)]
    pub overrides: BTreeMap<String, Override>,
    #[serde(default)]
    pub sections: Vec<SectionSpec>,
    #[serde(default)]
    pub code_mentions: BTreeMap<String, MentionClass>,
    #[serde(default)]
    pub sidecar: Option<SidecarSpec>,
    #[serde(default)]
    pub shorten: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSpec {
    pub path: String,
    /// `file`, `sections`, `whole` or `files`.
    pub kind: String,
    /// The heading level of a `sections` source.
    #[serde(default)]
    pub level: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    /// The lifecycle folder: `ledgers`, `streams` or `temp`.
    pub lifecycle: String,
    /// The collection key; `None` is the stem of the first source.
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub existing: bool,
}

/// One kind for every entry (`entry_kind = "finding"`), or a kind per token prefix
/// (`[entry_kind]` with `F = "friction"`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum EntryKindRule {
    One(String),
    ByPrefix(BTreeMap<String, String>),
}

/// A possible record the user names a record.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtraRecordSpec {
    pub source: String,
    /// The heading line.
    pub line: u32,
    /// The token text of the heading (`R-56b`).
    pub token: String,
}

/// A possible record that stays inside its enclosing record.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PossibleSpec {
    pub source: String,
    pub line: u32,
}

/// Where a non-record section goes, keyed by its source and first line.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SectionSpec {
    pub source: String,
    pub start: u32,
    /// `readme` or `drop`.
    pub to: String,
    /// The user confirms that a possible record in this section is not a record.
    #[serde(default)]
    pub confirm: bool,
}

/// `status_source = "body_label"`, `{ index_column = "<header>" }` or
/// `{ constant = "<status>" }`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusSource {
    BodyLabel,
    IndexColumn(String),
    Constant(String),
}

/// Values for one record. A date is a TOML date (`2026-08-01`) or a quoted one.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Override {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default, deserialize_with = "opt_date")]
    pub created: Option<NaiveDate>,
    #[serde(default, deserialize_with = "opt_date")]
    pub closed: Option<NaiveDate>,
    #[serde(default, deserialize_with = "opt_date")]
    pub revisit_by: Option<NaiveDate>,
    #[serde(default)]
    pub slug: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MentionClass {
    Reader,
    Comment,
    Fixture,
}

/// `sidecar = "retire"` or `sidecar = { keep = "<reason>" }`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SidecarSpec {
    Retire,
    Keep(String),
}

/// A TOML date, or a string in `YYYY-MM-DD` form.
fn opt_date<'de, D: Deserializer<'de>>(d: D) -> Result<Option<NaiveDate>, D::Error> {
    use serde::de::Error;
    let text = match toml::Value::deserialize(d)? {
        toml::Value::String(s) => s,
        toml::Value::Datetime(dt) if dt.time.is_none() && dt.offset.is_none() => dt.to_string(),
        other => other.to_string(),
    };
    NaiveDate::parse_from_str(&text, "%Y-%m-%d")
        .map(Some)
        .map_err(|_| D::Error::custom(format!("\"{text}\" is not a date (YYYY-MM-DD)")))
}

/// Read a mapping from its TOML text. A TOML or shape error is one message; the
/// value checks (source kinds, levels, lifecycle, section targets) all come back
/// together.
pub fn parse_mapping(text: &str) -> Result<Mapping, Vec<String>> {
    let m: Mapping =
        toml::from_str(text).map_err(|e| vec![format!("the mapping file has an error: {e}")])?;
    let mut problems = Vec::new();
    if m.sources.is_empty() {
        problems.push("sources is empty".to_string());
    }
    for s in &m.sources {
        if !SOURCE_KINDS.contains(&s.kind.as_str()) {
            problems.push(format!(
                "source {} has kind \"{}\". Use file, sections, whole or files.",
                s.path, s.kind
            ));
        } else if s.kind == "sections" && s.level.is_none() {
            problems.push(format!(
                "source {} has kind \"sections\" but no level",
                s.path
            ));
        } else if s.kind != "sections" && s.level.is_some() {
            problems.push(format!(
                "source {} has a level, but only kind \"sections\" takes one",
                s.path
            ));
        }
    }
    if Lifecycle::from_dir(&m.target.lifecycle).is_none() {
        problems.push(format!(
            "target.lifecycle \"{}\" is not a lifecycle folder. Use ledgers, streams or temp.",
            m.target.lifecycle
        ));
    }
    for s in &m.sections {
        if s.to != "readme" && s.to != "drop" {
            problems.push(format!(
                "the sections entry at line {} of {} has to = \"{}\". Use readme or drop.",
                s.start, s.source, s.to
            ));
        }
    }
    if problems.is_empty() {
        Ok(m)
    } else {
        Err(problems)
    }
}

/// What the mapping leaves unsettled for this inventory. Empty means `apply` may plan.
///
/// The inventory is the split **without** `records_extra`, so a section that starts
/// at a `records_extra` heading needs no `sections` entry (it becomes a record).
/// "Inside a record" is decided by the possible's line against the records' line
/// ranges, because `Possible.enclosing` is `None` both in a section and in a record
/// with no token.
pub fn mapping_problems(m: &Mapping, inv: &Inventory) -> Vec<String> {
    let mut out = Vec::new();

    for src in &inv.sources {
        if kind_of(m, &src.path).is_none() {
            out.push(format!("inventory source {} is not in sources", src.path));
        }
    }

    match (&m.schema, m.target.existing) {
        (Some(_), true) => out.push("schema must be absent when target.existing is true".into()),
        (None, false) => out.push("schema must be present when target.existing is false".into()),
        _ => {}
    }
    if !m.target.existing && m.hot_view_none.is_none() {
        for key in ["hot_cap", "stale_after"] {
            if !m.schema.as_ref().is_some_and(|s| s.contains_key(key)) {
                out.push(format!("schema has no {key} and hot_view_none is not set"));
            }
        }
    }

    // A constant status source gives every record the same status: raw values are
    // not read, so they need no mapping.
    let reads_status = !matches!(m.status_source, StatusSource::Constant(_));
    if reads_status {
        let mut values: BTreeMap<&str, usize> = BTreeMap::new();
        for src in &inv.sources {
            for (value, n) in &src.status_values {
                *values.entry(value.as_str()).or_default() += n;
            }
        }
        for (value, n) in values {
            if !m.status_map.contains_key(value) {
                out.push(format!(
                    "status value \"{value}\" ({}) is not in status_map",
                    records(n)
                ));
            }
        }
    }

    if let EntryKindRule::ByPrefix(kinds) = &m.entry_kind {
        let mut prefixes: BTreeMap<&str, usize> = BTreeMap::new();
        let mut tokenless: Vec<(&str, usize)> = Vec::new();
        for src in &inv.sources {
            let extra = m
                .records_extra
                .iter()
                .filter(|e| e.source == src.path)
                .map(|e| e.token.as_str());
            let tokens = src
                .records
                .iter()
                .filter_map(|r| r.token.as_deref())
                .chain(src.row_only.iter().map(String::as_str))
                .chain(extra);
            for token in tokens {
                let prefix = token.split('-').next().unwrap_or(token);
                *prefixes.entry(prefix).or_default() += 1;
            }
            let none = src.records.iter().filter(|r| r.token.is_none()).count();
            if none > 0 {
                tokenless.push((src.path.as_str(), none));
            }
        }
        for (prefix, n) in prefixes {
            if !kinds.contains_key(prefix) {
                out.push(format!(
                    "token prefix \"{prefix}\" ({}) is not in entry_kind",
                    records(n)
                ));
            }
        }
        for (path, n) in tokenless {
            out.push(format!(
                "entry_kind by prefix cannot class the {} with no token in {path}",
                records(n)
            ));
        }
    }

    for src in &inv.sources {
        source_problems(m, src, reads_status, &mut out);
    }

    for e in &m.records_extra {
        if !inv.sources.iter().any(|s| s.path == e.source) {
            out.push(format!(
                "records_extra names {}, which is not an inventory source",
                e.source
            ));
        }
    }

    let mut code_files: Vec<&str> = inv.mentions.code.iter().map(|h| h.file.as_str()).collect();
    code_files.sort_unstable();
    code_files.dedup();
    for file in code_files {
        if !m.code_mentions.contains_key(file) {
            out.push(format!("code mention {file} has no class in code_mentions"));
        }
    }

    if let (Some(sidecar), None) = (&inv.sidecar, &m.sidecar) {
        out.push(format!("sidecar {} has no sidecar entry", sidecar.path));
    }
    out
}

/// The problems of one source: status gaps, index rows, `records_extra` entries,
/// possible records and sections.
fn source_problems(m: &Mapping, src: &SourceInventory, reads_status: bool, out: &mut Vec<String>) {
    let path = src.path.as_str();
    if reads_status && !src.no_status.is_empty() && m.status_none.is_none() {
        out.push(format!(
            "records with no status in {path} need status_none: {}",
            src.no_status.join(", ")
        ));
    }

    if !src.row_only.is_empty() {
        match &m.row_title_column {
            None => out.push(format!(
                "index rows with no record heading in {path} need row_title_column: {}",
                src.row_only.join(", ")
            )),
            Some(column) => {
                let wanted = column.trim().to_lowercase();
                if !src.tables.iter().any(|t| t.header.contains(&wanted)) {
                    out.push(format!(
                        "row_title_column \"{column}\" is not a header of an index table in {path}"
                    ));
                }
            }
        }
    }

    if !src.duplicate_rows.is_empty() {
        match m.index_table {
            None => out.push(format!(
                "tokens with rows in two index tables of {path} need index_table: {}",
                src.duplicate_rows.join(", ")
            )),
            Some(start) => {
                if !src.tables.iter().any(|t| t.section_start == start) {
                    out.push(format!(
                        "index_table {start} is not the first line of a section with an index table in {path}"
                    ));
                }
            }
        }
    }

    // The splitter ignores an `extra` it cannot use, so this is the only guard.
    let whole = kind_of(m, path) == Some("whole");
    let extras: Vec<&ExtraRecordSpec> = m
        .records_extra
        .iter()
        .filter(|e| e.source == path)
        .collect();
    for e in &extras {
        let line = e.line;
        if whole {
            out.push(format!(
                "records_extra line {line} of {path} has no effect: a whole source has one record"
            ));
        } else if src.records.iter().any(|r| r.heading_line == line) {
            out.push(format!(
                "records_extra line {line} of {path} is already a record heading"
            ));
        } else if src.repeats.iter().any(|r| r.nested && r.other_line == line) {
            out.push(format!(
                "records_extra line {line} of {path} repeats the token of the record that holds it"
            ));
        } else {
            match src.possibles.iter().find(|p| p.line == line) {
                None => out.push(format!(
                    "records_extra line {line} of {path} is not a heading that starts with a token"
                )),
                Some(p) if p.token != e.token => out.push(format!(
                    "records_extra token \"{}\" differs from the heading token \"{}\" at line {line} of {path}",
                    e.token, p.token
                )),
                Some(_) => {}
            }
        }
    }
    let in_extra = |line: u32| extras.iter().any(|e| e.line == line);
    let section_spec = |start: u32| {
        m.sections
            .iter()
            .find(|s| s.source == path && s.start == start)
    };

    for p in &src.possibles {
        let (line, token) = (p.line, p.token.as_str());
        let kept_inside = m
            .possibles_inside
            .iter()
            .any(|s| s.source == path && s.line == line);
        if in_extra(line) && kept_inside {
            out.push(format!(
                "possible record {token} at line {line} of {path} is in records_extra and in possibles_inside. Keep one."
            ));
            continue;
        }
        let enclosing = src
            .records
            .iter()
            .find(|r| (r.first..=r.last).contains(&line));
        match enclosing {
            Some(r) => {
                if !in_extra(line) && !kept_inside {
                    let name = match &r.token {
                        Some(t) => format!("record {t}"),
                        None => format!("the record at line {}", r.heading_line),
                    };
                    out.push(format!(
                        "possible record {token} at line {line} of {path} is inside {name}. Add it to records_extra or possibles_inside."
                    ));
                }
            }
            None => {
                let confirmed = src
                    .sections
                    .iter()
                    .find(|s| (s.start..=s.last).contains(&line))
                    .and_then(|s| section_spec(s.start))
                    .is_some_and(|spec| spec.confirm);
                if !in_extra(line) && !confirmed {
                    out.push(format!(
                        "possible record {token} at line {line} of {path} is in a section. Add it to records_extra, or to sections with confirm = true."
                    ));
                }
            }
        }
    }

    for s in &src.sections {
        if !in_extra(s.start) && section_spec(s.start).is_none() {
            out.push(format!(
                "section at line {} of {path} (\"{}\") has no sections entry",
                s.start, s.first_line
            ));
        }
    }
}

/// The kind of an inventory source: its own `sources` entry, or `whole` for a file
/// directly inside a `files` folder. `None` when no entry names it.
fn kind_of<'a>(m: &'a Mapping, path: &str) -> Option<&'a str> {
    m.sources.iter().find_map(|s| {
        if s.path == path {
            return Some(s.kind.as_str());
        }
        let folder = s.path.trim_end_matches('/');
        let in_folder = path
            .strip_prefix(folder)
            .and_then(|rest| rest.strip_prefix('/'))
            .is_some_and(|name| !name.is_empty() && !name.contains('/'));
        (s.kind == "files" && in_folder).then_some("whole")
    })
}

/// "1 record" or "N records".
fn records(n: usize) -> String {
    if n == 1 {
        "1 record".to_string()
    } else {
        format!("{n} records")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::tracker_migration::inventory::{
        CodeHit, Mentions, RecordRow, SectionRow, SidecarInfo, SourceInventory, TableRow,
    };
    use crate::librarian::tracker_migration::records::{Possible, Repeat};

    const X: &str = "docs/trackers/x.md";

    /// A mapping that settles everything [`base_inventory`] reports.
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
hot_cap = 10
stale_after = 30

[status_map]
open = "open"
mitigated = "fixed"

[[records_extra]]
source = "docs/trackers/x.md"
line = 41
token = "R-56b"

[[possibles_inside]]
source = "docs/trackers/x.md"
line = 20

[[sections]]
source = "docs/trackers/x.md"
start = 3
to = "readme"

[[sections]]
source = "docs/trackers/x.md"
start = 31
to = "drop"

[[sections]]
source = "docs/trackers/x.md"
start = 51
to = "drop"
confirm = true

[code_mentions]
"scripts/x.json" = "fixture"
"#;

    fn record(token: Option<&str>, heading: u32, last: u32, status: Option<&str>) -> RecordRow {
        RecordRow {
            token: token.map(String::from),
            heading_line: heading,
            level: 2,
            title: "T".into(),
            first: heading,
            last,
            raw_status: status.map(String::from),
            status_origin: status.map(|_| "body".to_string()),
            stated_dates: vec![],
        }
    }

    fn section(start: u32, last: u32, first_line: &str) -> SectionRow {
        SectionRow {
            start,
            last,
            first_line: first_line.into(),
        }
    }

    fn possible(line: u32, token: &str, enclosing: Option<&str>) -> Possible {
        Possible {
            line,
            token: token.into(),
            enclosing: enclosing.map(String::from),
        }
    }

    /// Records F-1 (5..14) and F-2 (15..30); sections at 3, 31, 41, 51; possibles
    /// F-2a inside F-2 (line 20), R-56b (41) and T-9 (51) each starting a section.
    fn source_x() -> SourceInventory {
        SourceInventory {
            path: X.into(),
            line_count: 60,
            records: vec![
                record(Some("F-1"), 5, 14, Some("open")),
                record(Some("F-2"), 15, 30, Some("mitigated")),
            ],
            sections: vec![
                section(3, 4, "# X"),
                section(31, 40, "## Template"),
                section(41, 50, "## R-56b — Hit: x"),
                section(51, 60, "## T-9 — not a record"),
            ],
            possibles: vec![
                possible(20, "F-2a", Some("F-2")),
                possible(41, "R-56b", None),
                possible(51, "T-9", None),
            ],
            repeats: vec![],
            tables: vec![TableRow {
                section_start: 3,
                header: vec!["id".into(), "status".into(), "title".into()],
                rows: 2,
            }],
            status_values: BTreeMap::from([("mitigated".to_string(), 32), ("open".to_string(), 3)]),
            no_status: vec![],
            row_only: vec![],
            no_row: vec![],
            duplicate_rows: vec![],
            disagreements: vec![],
        }
    }

    fn base_inventory() -> Inventory {
        Inventory {
            sources: vec![source_x()],
            mentions: Mentions {
                citations: vec![],
                paths: vec![],
                ids: vec![],
                code: vec![CodeHit {
                    file: "scripts/x.json".into(),
                    line: 3,
                    origin: "grep".into(),
                }],
            },
            sidecar: Some(SidecarInfo {
                path: "docs/augmentations/docs-trackers-x.yaml".into(),
                prompt: Some("Track x.".into()),
            }),
        }
    }

    fn base_mapping() -> Mapping {
        parse_mapping(BASE).expect("BASE parses")
    }

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn parse_err(text: &str) -> Vec<String> {
        parse_mapping(text).expect_err("the mapping is refused")
    }

    // ── parse ──────────────────────────────────────────────────────────────

    /// Every key once. Breaks on a renamed field, a lost `default`, the enum forms
    /// (`rename_all`, untagged `entry_kind`) and either date form.
    #[test]
    fn a_full_mapping_parses() {
        let text = r#"
hot_view_none = "a finished stream"
readme_body = "Body."
status_none = "open"
row_title_column = "Title"
index_table = 7
status_source = { index_column = "Status" }
sidecar = { keep = "a hook reads it" }

[[sources]]
path = "docs/trackers/x.md"
kind = "file"

[[sources]]
path = "docs/trackers/x-archive.md"
kind = "sections"
level = 2

[[sources]]
path = "docs/trackers/ic"
kind = "files"

[target]
lifecycle = "streams"
key = "x"
archived = true
existing = true

[entry_kind]
F = "friction"
W = "win"

[status_map]
open = "open"

[columns]
title = "title_alias"
opened = "created"

[[records_extra]]
source = "docs/trackers/x.md"
line = 41
token = "R-56b"

[[possibles_inside]]
source = "docs/trackers/x.md"
line = 20

[overrides.F-1]
status = "fixed"
created = 2026-08-01
closed = "2026-09-01"
revisit_by = 2026-12-01
slug = "first"

[[sections]]
source = "docs/trackers/x.md"
start = 31
to = "drop"
confirm = true

[code_mentions]
"scripts/a.json" = "reader"
"scripts/b.rs" = "comment"
"tests/c.yaml" = "fixture"

[shorten]
F-2 = "A short body."
"#;
        let expected = Mapping {
            sources: vec![
                SourceSpec {
                    path: X.into(),
                    kind: "file".into(),
                    level: None,
                },
                SourceSpec {
                    path: "docs/trackers/x-archive.md".into(),
                    kind: "sections".into(),
                    level: Some(2),
                },
                SourceSpec {
                    path: "docs/trackers/ic".into(),
                    kind: "files".into(),
                    level: None,
                },
            ],
            target: Target {
                lifecycle: "streams".into(),
                key: Some("x".into()),
                archived: true,
                existing: true,
            },
            schema: None,
            hot_view_none: Some("a finished stream".into()),
            readme_body: Some("Body.".into()),
            entry_kind: EntryKindRule::ByPrefix(BTreeMap::from([
                ("F".to_string(), "friction".to_string()),
                ("W".to_string(), "win".to_string()),
            ])),
            status_source: StatusSource::IndexColumn("Status".into()),
            status_map: BTreeMap::from([("open".to_string(), "open".to_string())]),
            status_none: Some("open".into()),
            columns: BTreeMap::from([
                ("opened".to_string(), "created".to_string()),
                ("title".to_string(), "title_alias".to_string()),
            ]),
            row_title_column: Some("Title".into()),
            index_table: Some(7),
            records_extra: vec![ExtraRecordSpec {
                source: X.into(),
                line: 41,
                token: "R-56b".into(),
            }],
            possibles_inside: vec![PossibleSpec {
                source: X.into(),
                line: 20,
            }],
            overrides: BTreeMap::from([(
                "F-1".to_string(),
                Override {
                    status: Some("fixed".into()),
                    created: Some(d("2026-08-01")),
                    closed: Some(d("2026-09-01")),
                    revisit_by: Some(d("2026-12-01")),
                    slug: Some("first".into()),
                },
            )]),
            sections: vec![SectionSpec {
                source: X.into(),
                start: 31,
                to: "drop".into(),
                confirm: true,
            }],
            code_mentions: BTreeMap::from([
                ("scripts/a.json".to_string(), MentionClass::Reader),
                ("scripts/b.rs".to_string(), MentionClass::Comment),
                ("tests/c.yaml".to_string(), MentionClass::Fixture),
            ]),
            sidecar: Some(SidecarSpec::Keep("a hook reads it".into())),
            shorten: BTreeMap::from([("F-2".to_string(), "A short body.".to_string())]),
        };
        assert_eq!(parse_mapping(text), Ok(expected));
    }

    /// The minimal forms: a string `entry_kind`, unit `status_source` and `sidecar`,
    /// and every optional key absent. Breaks when a `#[serde(default)]` is dropped.
    #[test]
    fn the_base_mapping_parses_with_defaults() {
        let m = base_mapping();
        assert_eq!(m.entry_kind, EntryKindRule::One("finding".into()));
        assert_eq!(m.status_source, StatusSource::BodyLabel);
        assert_eq!(m.sidecar, Some(SidecarSpec::Retire));
        assert_eq!(
            m.target,
            Target {
                lifecycle: "ledgers".into(),
                key: None,
                archived: false,
                existing: false,
            }
        );
        assert!(m.overrides.is_empty() && m.shorten.is_empty() && m.columns.is_empty());
        assert_eq!(m.index_table, None);
        assert!(!m.sections[0].confirm);
        let constant = BASE.replace(
            r#"status_source = "body_label""#,
            r#"status_source = { constant = "open" }"#,
        );
        assert_eq!(
            parse_mapping(&constant).unwrap().status_source,
            StatusSource::Constant("open".into())
        );
    }

    /// Breaks when `deny_unknown_fields` is removed from `Mapping`, `Target`,
    /// `SectionSpec` or `Override` (each case is one of them).
    #[test]
    fn an_unknown_key_is_refused() {
        let cases = [
            (
                BASE.replace("readme_body =", "colour = \"red\"\nreadme_body ="),
                "colour",
            ),
            (
                BASE.replace(
                    r#"lifecycle = "ledgers""#,
                    "lifecycle = \"ledgers\"\narchive = true",
                ),
                "archive",
            ),
            (BASE.replace("confirm = true", "confrim = true"), "confrim"),
            (
                format!("{BASE}\n[overrides.F-1]\nstauts = \"fixed\"\n"),
                "stauts",
            ),
            (
                BASE.replace("line = 20", "line = 20\nnote = \"stays\""),
                "note",
            ),
        ];
        for (text, key) in cases {
            let err = parse_err(&text);
            assert_eq!(err.len(), 1, "{err:?}");
            assert!(
                err[0].contains(&format!("unknown field `{key}`")),
                "{key}: {err:?}"
            );
        }
    }

    /// Breaks when `opt_date` accepts any string or drops the time check.
    #[test]
    fn a_bad_date_is_refused() {
        let text = format!("{BASE}\n[overrides.F-1]\ncreated = \"Aug 1\"\n");
        let err = parse_err(&text);
        assert!(
            err[0].contains(r#""Aug 1" is not a date (YYYY-MM-DD)"#),
            "{err:?}"
        );
        let text = format!("{BASE}\n[overrides.F-1]\ncreated = 2026-08-01T10:00:00\n");
        let err = parse_err(&text);
        assert!(err[0].contains("is not a date (YYYY-MM-DD)"), "{err:?}");
    }

    /// Breaks when one of the value checks in `parse_mapping` is removed; all
    /// problems come back together.
    #[test]
    fn bad_values_are_refused_together() {
        let text = BASE
            .replace(r#"kind = "file""#, "kind = \"dir\"\n\n[[sources]]\npath = \"docs/trackers/y.md\"\nkind = \"sections\"\n\n[[sources]]\npath = \"docs/trackers/z.md\"\nkind = \"file\"\nlevel = 2")
            .replace(r#"lifecycle = "ledgers""#, r#"lifecycle = "ledger""#)
            .replace(r#"to = "readme""#, r#"to = "keep""#);
        assert_eq!(
            parse_err(&text),
            vec![
                r#"source docs/trackers/x.md has kind "dir". Use file, sections, whole or files."#
                    .to_string(),
                r#"source docs/trackers/y.md has kind "sections" but no level"#.to_string(),
                r#"source docs/trackers/z.md has a level, but only kind "sections" takes one"#
                    .to_string(),
                r#"target.lifecycle "ledger" is not a lifecycle folder. Use ledgers, streams or temp."#
                    .to_string(),
                r#"the sections entry at line 3 of docs/trackers/x.md has to = "keep". Use readme or drop."#
                    .to_string(),
            ]
        );
        let none = BASE.replace(
            "[[sources]]\npath = \"docs/trackers/x.md\"\nkind = \"file\"\n",
            "",
        );
        let none = format!("sources = []\n{none}");
        assert_eq!(parse_err(&none), vec!["sources is empty".to_string()]);
    }

    // ── mapping_problems ──────────────────────────────────────────────────

    /// The baseline every refusal test changes in one place. Breaks when any check
    /// refuses a settled item (the exempt section at 41, the confirmed possible at
    /// 51, the possible at 20 kept inside F-2).
    #[test]
    fn a_complete_mapping_has_no_problems() {
        assert_eq!(
            mapping_problems(&base_mapping(), &base_inventory()),
            Vec::<String>::new()
        );
    }

    /// Breaks when the status_map check is removed or stops summing counts over
    /// sources (32 here = 30 in x.md + 2 in y.md).
    #[test]
    fn a_status_value_missing_from_status_map_is_refused() {
        let mut m = base_mapping();
        m.status_map.remove("mitigated");
        m.sources.push(SourceSpec {
            path: "docs/trackers/y.md".into(),
            kind: "file".into(),
            level: None,
        });
        let mut inv = base_inventory();
        inv.sources[0].status_values.insert("mitigated".into(), 30);
        let mut y = source_x();
        y.path = "docs/trackers/y.md".into();
        y.records.clear();
        y.sections.clear();
        y.possibles.clear();
        y.tables.clear();
        y.status_values = BTreeMap::from([("mitigated".to_string(), 2)]);
        inv.sources.push(y);
        assert_eq!(
            mapping_problems(&m, &inv),
            vec![r#"status value "mitigated" (32 records) is not in status_map"#.to_string()]
        );
        // One record reads "1 record".
        inv.sources.pop();
        inv.sources[0].status_values.insert("mitigated".into(), 1);
        m.sources.pop();
        assert_eq!(
            mapping_problems(&m, &inv),
            vec![r#"status value "mitigated" (1 record) is not in status_map"#.to_string()]
        );
    }

    /// Breaks when a constant status source still demands status_map or status_none.
    #[test]
    fn a_constant_status_source_needs_no_status_map() {
        let mut m = base_mapping();
        m.status_map.clear();
        m.status_source = StatusSource::Constant("open".into());
        let mut inv = base_inventory();
        inv.sources[0].no_status = vec!["F-3".into()];
        assert_eq!(mapping_problems(&m, &inv), Vec::<String>::new());
    }

    /// Breaks when the no_status check is removed or ignores status_none.
    #[test]
    fn records_with_no_status_need_status_none() {
        let m = base_mapping();
        let mut inv = base_inventory();
        inv.sources[0].no_status = vec!["F-3".into(), "F-4".into()];
        assert_eq!(
            mapping_problems(&m, &inv),
            vec![
                "records with no status in docs/trackers/x.md need status_none: F-3, F-4"
                    .to_string()
            ]
        );
        let mut m = m;
        m.status_none = Some("open".into());
        assert_eq!(mapping_problems(&m, &inv), Vec::<String>::new());
    }

    /// Breaks when the row_only check is removed, or when row_title_column is not
    /// matched against the table headers (lowercased).
    #[test]
    fn row_only_records_need_row_title_column() {
        let mut m = base_mapping();
        let mut inv = base_inventory();
        inv.sources[0].row_only = vec!["F-7".into()];
        assert_eq!(
            mapping_problems(&m, &inv),
            vec![
                "index rows with no record heading in docs/trackers/x.md need row_title_column: F-7"
                    .to_string()
            ]
        );
        m.row_title_column = Some("Name".into());
        assert_eq!(
            mapping_problems(&m, &inv),
            vec![
                r#"row_title_column "Name" is not a header of an index table in docs/trackers/x.md"#
                    .to_string()
            ]
        );
        m.row_title_column = Some("Title".into());
        assert_eq!(mapping_problems(&m, &inv), Vec::<String>::new());
    }

    /// Breaks when the duplicate_rows check is removed, or index_table is accepted
    /// without naming a table's section.
    #[test]
    fn a_token_in_two_index_tables_needs_index_table() {
        let mut m = base_mapping();
        let mut inv = base_inventory();
        inv.sources[0].duplicate_rows = vec!["F-1".into()];
        inv.sources[0].tables.push(TableRow {
            section_start: 31,
            header: vec!["id".into()],
            rows: 1,
        });
        assert_eq!(
            mapping_problems(&m, &inv),
            vec![
                "tokens with rows in two index tables of docs/trackers/x.md need index_table: F-1"
                    .to_string()
            ]
        );
        m.index_table = Some(41);
        assert_eq!(
            mapping_problems(&m, &inv),
            vec![
                "index_table 41 is not the first line of a section with an index table in docs/trackers/x.md"
                    .to_string()
            ]
        );
        m.index_table = Some(31);
        assert_eq!(mapping_problems(&m, &inv), Vec::<String>::new());
    }

    /// Breaks when a possible in a section is settled by a sections entry without
    /// `confirm = true`, or when the check is removed.
    #[test]
    fn a_possible_record_in_a_section_needs_records_extra_or_a_confirmed_section() {
        let mut m = base_mapping();
        m.sections[2].confirm = false;
        let inv = base_inventory();
        let want = vec![
            "possible record T-9 at line 51 of docs/trackers/x.md is in a section. Add it to records_extra, or to sections with confirm = true."
                .to_string(),
        ];
        assert_eq!(mapping_problems(&m, &inv), want);
        // Without any sections entry the section is unsettled too.
        m.sections.remove(2);
        assert_eq!(
            mapping_problems(&m, &inv),
            vec![
                want[0].clone(),
                r###"section at line 51 of docs/trackers/x.md ("## T-9 — not a record") has no sections entry"###
                    .to_string(),
            ]
        );
        // records_extra settles both the possible and its section.
        m.records_extra.push(ExtraRecordSpec {
            source: X.into(),
            line: 51,
            token: "T-9".into(),
        });
        assert_eq!(mapping_problems(&m, &inv), Vec::<String>::new());
    }

    /// Breaks when the possibles_inside check is removed, or when "inside a record"
    /// is read from `Possible.enclosing` instead of the record's lines.
    #[test]
    fn a_possible_record_inside_a_record_needs_records_extra_or_possibles_inside() {
        let mut m = base_mapping();
        m.possibles_inside.clear();
        let inv = base_inventory();
        assert_eq!(
            mapping_problems(&m, &inv),
            vec![
                "possible record F-2a at line 20 of docs/trackers/x.md is inside record F-2. Add it to records_extra or possibles_inside."
                    .to_string()
            ]
        );
        // A sections source: the record has no token and `enclosing` is None.
        let mut m = base_mapping();
        m.possibles_inside.clear();
        m.sources[0].kind = "sections".into();
        m.sources[0].level = Some(2);
        let mut inv = base_inventory();
        inv.sources[0].records[1].token = None;
        inv.sources[0].possibles[0].enclosing = None;
        assert_eq!(
            mapping_problems(&m, &inv),
            vec![
                "possible record F-2a at line 20 of docs/trackers/x.md is inside the record at line 15. Add it to records_extra or possibles_inside."
                    .to_string()
            ]
        );
    }

    /// Breaks when one possible may sit in both lists.
    #[test]
    fn a_possible_in_records_extra_and_possibles_inside_is_refused() {
        let mut m = base_mapping();
        m.records_extra.push(ExtraRecordSpec {
            source: X.into(),
            line: 20,
            token: "F-2a".into(),
        });
        assert_eq!(
            mapping_problems(&m, &base_inventory()),
            vec![
                "possible record F-2a at line 20 of docs/trackers/x.md is in records_extra and in possibles_inside. Keep one."
                    .to_string()
            ]
        );
    }

    /// Breaks when the sections check is removed or keyed by anything but source
    /// and start.
    #[test]
    fn a_non_record_section_needs_a_sections_entry() {
        let mut m = base_mapping();
        m.sections.remove(1);
        let inv = base_inventory();
        assert_eq!(
            mapping_problems(&m, &inv),
            vec![
                r###"section at line 31 of docs/trackers/x.md ("## Template") has no sections entry"###
                    .to_string()
            ]
        );
        // An entry for another source does not settle it.
        m.sections.push(SectionSpec {
            source: "docs/trackers/y.md".into(),
            start: 31,
            to: "drop".into(),
            confirm: false,
        });
        assert_eq!(mapping_problems(&m, &inv).len(), 1);
    }

    /// Breaks when the records_extra checks are removed: the splitter ignores a bad
    /// `extra` silently, so these refusals are the only guard.
    #[test]
    fn records_extra_must_name_a_possible_heading_with_its_token() {
        let inv = base_inventory();
        let extra = |source: &str, line: u32, token: &str| {
            let mut m = base_mapping();
            m.records_extra.push(ExtraRecordSpec {
                source: source.into(),
                line,
                token: token.into(),
            });
            mapping_problems(&m, &inv)
        };
        assert_eq!(
            extra(X, 8, "F-9"),
            vec!["records_extra line 8 of docs/trackers/x.md is not a heading that starts with a token".to_string()]
        );
        assert_eq!(
            extra(X, 15, "F-2"),
            vec![
                "records_extra line 15 of docs/trackers/x.md is already a record heading"
                    .to_string()
            ]
        );
        let mut m = base_mapping();
        m.records_extra[0].token = "R-56".into();
        assert_eq!(
            mapping_problems(&m, &inv),
            vec![r#"records_extra token "R-56" differs from the heading token "R-56b" at line 41 of docs/trackers/x.md"#.to_string()]
        );
        assert_eq!(
            extra("docs/trackers/q.md", 3, "Q-1"),
            vec![
                "records_extra names docs/trackers/q.md, which is not an inventory source"
                    .to_string()
            ]
        );
        // A nested repeat of the enclosing record's token.
        let mut inv2 = base_inventory();
        inv2.sources[0].repeats.push(Repeat {
            token: "F-1".into(),
            first_line: 5,
            other_line: 9,
            nested: true,
        });
        let mut m = base_mapping();
        m.records_extra.push(ExtraRecordSpec {
            source: X.into(),
            line: 9,
            token: "F-1".into(),
        });
        assert_eq!(
            mapping_problems(&m, &inv2),
            vec!["records_extra line 9 of docs/trackers/x.md repeats the token of the record that holds it".to_string()]
        );
    }

    /// A `files` folder makes each file a whole source. Breaks when the folder
    /// prefix is not matched (the source is "not in sources") or when records_extra
    /// on a whole source is accepted (the splitter ignores it).
    #[test]
    fn a_files_source_is_whole_and_takes_no_records_extra() {
        let file = "docs/trackers/ic/ic-1.md";
        let mut m = base_mapping();
        m.sources[0] = SourceSpec {
            path: "docs/trackers/ic".into(),
            kind: "files".into(),
            level: None,
        };
        for s in &mut m.sections {
            s.source = file.into();
        }
        m.records_extra.clear();
        m.possibles_inside[0].source = file.into();
        m.sections.push(SectionSpec {
            source: file.into(),
            start: 41,
            to: "drop".into(),
            confirm: true,
        });
        let mut inv = base_inventory();
        inv.sources[0].path = file.into();
        assert_eq!(mapping_problems(&m, &inv), Vec::<String>::new());
        m.records_extra.push(ExtraRecordSpec {
            source: file.into(),
            line: 41,
            token: "R-56b".into(),
        });
        assert_eq!(
            mapping_problems(&m, &inv),
            vec!["records_extra line 41 of docs/trackers/ic/ic-1.md has no effect: a whole source has one record".to_string()]
        );
    }

    /// Breaks when the inventory is checked against a mapping whose sources do not
    /// include it.
    #[test]
    fn an_inventory_source_missing_from_sources_is_refused() {
        let mut m = base_mapping();
        m.sources[0].path = "docs/trackers/other.md".into();
        let problems = mapping_problems(&m, &base_inventory());
        assert_eq!(
            problems[0],
            "inventory source docs/trackers/x.md is not in sources"
        );
    }

    /// Breaks when the code_mentions check is removed; one message per file.
    #[test]
    fn a_code_mention_needs_a_class() {
        let mut m = base_mapping();
        m.code_mentions.clear();
        let mut inv = base_inventory();
        inv.mentions.code.push(CodeHit {
            file: "scripts/x.json".into(),
            line: 9,
            origin: "code_refs".into(),
        });
        assert_eq!(
            mapping_problems(&m, &inv),
            vec!["code mention scripts/x.json has no class in code_mentions".to_string()]
        );
    }

    /// Breaks when the sidecar check is removed.
    #[test]
    fn a_sidecar_needs_a_sidecar_entry() {
        let mut m = base_mapping();
        m.sidecar = None;
        assert_eq!(
            mapping_problems(&m, &base_inventory()),
            vec![
                "sidecar docs/augmentations/docs-trackers-x.yaml has no sidecar entry".to_string()
            ]
        );
        let mut inv = base_inventory();
        inv.sidecar = None;
        assert_eq!(mapping_problems(&m, &inv), Vec::<String>::new());
    }

    /// Breaks when either direction of the schema/existing rule is removed.
    #[test]
    fn schema_follows_target_existing() {
        let mut m = base_mapping();
        m.target.existing = true;
        assert_eq!(
            mapping_problems(&m, &base_inventory()),
            vec!["schema must be absent when target.existing is true".to_string()]
        );
        m.schema = None;
        assert_eq!(
            mapping_problems(&m, &base_inventory()),
            Vec::<String>::new()
        );
        m.target.existing = false;
        m.hot_view_none = Some("a closed stream".into());
        assert_eq!(
            mapping_problems(&m, &base_inventory()),
            vec!["schema must be present when target.existing is false".to_string()]
        );
    }

    /// M5. Breaks when the check accepts one of the two keys alone, or ignores
    /// hot_view_none.
    #[test]
    fn a_new_collection_needs_hot_cap_and_stale_after_or_hot_view_none() {
        let mut m = base_mapping();
        m.schema.as_mut().unwrap().remove("stale_after");
        assert_eq!(
            mapping_problems(&m, &base_inventory()),
            vec!["schema has no stale_after and hot_view_none is not set".to_string()]
        );
        m.schema.as_mut().unwrap().remove("hot_cap");
        assert_eq!(
            mapping_problems(&m, &base_inventory()),
            vec![
                "schema has no hot_cap and hot_view_none is not set".to_string(),
                "schema has no stale_after and hot_view_none is not set".to_string(),
            ]
        );
        m.hot_view_none = Some("a closed stream".into());
        assert_eq!(
            mapping_problems(&m, &base_inventory()),
            Vec::<String>::new()
        );
    }

    /// Breaks when a by-prefix entry_kind is not checked against the record tokens
    /// (records, records_extra and row-only tokens), or token-less records pass.
    #[test]
    fn entry_kind_by_prefix_must_cover_every_token() {
        let mut m = base_mapping();
        m.entry_kind =
            EntryKindRule::ByPrefix(BTreeMap::from([("F".to_string(), "finding".to_string())]));
        m.status_none = Some("open".into());
        m.row_title_column = Some("title".into());
        let mut inv = base_inventory();
        inv.sources[0].row_only = vec!["W-1".into()];
        assert_eq!(
            mapping_problems(&m, &inv),
            vec![
                r#"token prefix "R" (1 record) is not in entry_kind"#.to_string(),
                r#"token prefix "W" (1 record) is not in entry_kind"#.to_string(),
            ]
        );
        inv.sources[0].records[0].token = None;
        inv.sources[0].row_only.clear();
        m.records_extra.clear();
        m.sections.push(SectionSpec {
            source: X.into(),
            start: 41,
            to: "drop".into(),
            confirm: true,
        });
        assert_eq!(
            mapping_problems(&m, &inv),
            vec![
                "entry_kind by prefix cannot class the 1 record with no token in docs/trackers/x.md"
                    .to_string()
            ]
        );
    }
}
