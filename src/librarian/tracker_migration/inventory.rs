//! What `codescout trackers inventory` reports about a set of tracker sources.
//!
//! Spec: `docs/superpowers/specs/2026-11-15-tracker-migration-design.md`
//! § The per-tracker loop, step 2 (Inventory).
//!
//! These types are the `inventory --json` output. The `migrating-trackers` skill
//! (Plan L-E2) reads that JSON, and [`super::mapping::mapping_problems`] checks a
//! mapping against it, so the field names are an interface. Task 6 adds the builder.
//!
//! Lines are 1-indexed lines of `text.lines()`, as in [`super::records`]. Paths are
//! repo-relative.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::Serialize;

use super::records::{Possible, Repeat};

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
}
