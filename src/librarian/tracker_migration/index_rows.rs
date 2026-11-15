//! Index tables, status normalisation and per-record facts.
//!
//! Spec: `docs/superpowers/specs/2026-11-15-tracker-migration-design.md`.
//!
//! An *index table* is a markdown table outside every record whose data rows start
//! with a record token (`F-1 | open | …`). [`index_tables`] reads them from the
//! non-record sections of a [`Split`], so a token table that sits inside a record
//! body is part of that record and is never an index. Fenced blocks are skipped with
//! the same [`FenceState`] the heading parser uses.
//!
//! [`record_facts`] reads the status and the dates a record states about itself, from
//! its heading line and its bold-labelled lines only; a date in prose is not a fact.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use chrono::NaiveDate;
use regex::Regex;
use serde::Serialize;

use super::records::{slice, Lines, Record, Split};
use crate::util::markdown_fence::FenceState;

/// One data row of an index table, keyed by its token cell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IndexRow {
    pub token: String,
    /// 1-indexed source line of the row.
    pub line: u32,
    /// The source line, untrimmed.
    pub raw: String,
    /// Header text (lowercased, trimmed) to cell text.
    pub cells: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IndexTable {
    /// First line of the section that holds the table.
    pub section_start: u32,
    /// Header line through the last row.
    pub lines: Lines,
    /// Header texts, lowercased and trimmed.
    pub header: Vec<String>,
    /// The data rows whose first cell is a token.
    pub rows: Vec<IndexRow>,
}

/// What a record states about itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Facts {
    /// The text after `**Status:**` on the first status line.
    pub raw_status_body: Option<String>,
    /// Every valid ISO date on the heading line and on labelled lines, in order.
    pub stated_dates: Vec<NaiveDate>,
    pub status_date: Option<NaiveDate>,
    pub closed_date: Option<NaiveDate>,
    pub revisit_by: Option<NaiveDate>,
}

fn token_cell_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Z]{1,3}-\d+[a-z]?$").unwrap())
}

fn separator_cell_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^:?-+:?$").unwrap())
}

fn status_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s*(?:[-*]\s+)?\*\*Status:?\*\*:?\s*(.*)$").unwrap())
}

fn label_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"^\s*(?:[-*]\s+)?\*\*(Date|Created|Opened|Valid|Status|Closed|Fixed|Revisit by|Revisit):?\*\*:?",
        )
        .unwrap()
    })
}

fn date_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b\d{4}-\d{2}-\d{2}\b").unwrap())
}

/// The cells of a table row: split on `|` outside code spans, `\|` is a literal pipe
/// (its backslash is dropped), cells trimmed, a leading and a trailing pipe ignored.
/// A code span keeps its backticks in the cell text.
pub fn table_cells(line: &str) -> Vec<String> {
    let line = line.trim();
    let body = line.strip_prefix('|').unwrap_or(line);
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut in_code = false;
    let mut ends_with_pipe = false;
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        ends_with_pipe = false;
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                chars.next();
                cur.push('|');
            }
            '`' => {
                in_code = !in_code;
                cur.push(c);
            }
            '|' if !in_code => {
                cells.push(cur.trim().to_string());
                cur.clear();
                ends_with_pipe = true;
            }
            _ => cur.push(c),
        }
    }
    if !(ends_with_pipe && cur.is_empty()) {
        cells.push(cur.trim().to_string());
    }
    cells
}

fn is_token(cell: &str) -> bool {
    token_cell_re().is_match(cell)
}

fn is_separator(line: &str) -> bool {
    let cells = table_cells(line);
    !cells.is_empty() && cells.iter().all(|c| separator_cell_re().is_match(c))
}

/// The index tables of the non-record sections of `text`.
///
/// A table is a header line, a separator line and the rows that follow while a line
/// starts with `|`. It is an index when at least one row's first cell is a token; the
/// rows that are not tokens are left out of `rows`.
pub fn index_tables(text: &str, split: &Split) -> Vec<IndexTable> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    for section in &split.sections {
        let (start, end) = (*section.lines.start(), *section.lines.end());
        let mut fence = FenceState::new();
        let mut n = start;
        while n <= end.min(lines.len() as u32) {
            let line = lines[n as usize - 1];
            let trimmed = line.trim_start();
            if fence.feed(trimmed) || fence.in_fence() {
                n += 1;
                continue;
            }
            let has_separator = n < end && is_separator(lines[n as usize]);
            if !(trimmed.starts_with('|') && has_separator) {
                n += 1;
                continue;
            }
            let header: Vec<String> = table_cells(line)
                .into_iter()
                .map(|h| h.to_lowercase())
                .collect();
            let mut last = n + 1;
            let mut rows = Vec::new();
            while last < end {
                let row = lines[last as usize];
                if !row.trim_start().starts_with('|') {
                    break;
                }
                last += 1;
                let cells = table_cells(row);
                if cells.first().is_some_and(|c| is_token(c)) {
                    rows.push(IndexRow {
                        token: cells[0].clone(),
                        line: last,
                        raw: row.to_string(),
                        cells: header.iter().cloned().zip(cells.iter().cloned()).collect(),
                    });
                }
            }
            if !rows.is_empty() {
                out.push(IndexTable {
                    section_start: section.start,
                    lines: n..=last,
                    header,
                    rows,
                });
            }
            n = last + 1;
        }
    }
    out
}

/// A status as a comparable word: `**` and backticks removed, lowercased, cut at the
/// first ` — `, ` – `, ` (`, `,`, `;` or `:`. Empty is `None`.
pub fn normalise_status(raw: &str) -> Option<String> {
    let cleaned = raw.replace("**", "").replace('`', "");
    let cleaned = cleaned.trim().to_lowercase();
    let cut = [" — ", " – ", " (", ",", ";", ":"]
        .iter()
        .filter_map(|d| cleaned.find(d))
        .min()
        .unwrap_or(cleaned.len());
    let status = cleaned[..cut].trim();
    (!status.is_empty()).then(|| status.to_string())
}

fn dates_in(line: &str) -> Vec<NaiveDate> {
    date_re()
        .find_iter(line)
        .filter_map(|m| NaiveDate::parse_from_str(m.as_str(), "%Y-%m-%d").ok())
        .collect()
}

/// The status and dates `record` states about itself in `source`.
pub fn record_facts(source: &str, record: &Record) -> Facts {
    let body = slice(source, &record.lines);
    let mut facts = Facts {
        raw_status_body: None,
        stated_dates: Vec::new(),
        status_date: None,
        closed_date: None,
        revisit_by: None,
    };
    for (i, line) in body.lines().enumerate() {
        let number = record.lines.start() + i as u32;
        let label = label_re().captures(line).map(|c| c[1].to_string());
        if number != record.heading_line && label.is_none() {
            continue;
        }
        let dates = dates_in(line);
        facts.stated_dates.extend(dates.iter().copied());
        let Some(label) = label else { continue };
        let first = dates.first().copied();
        match label.as_str() {
            "Status" => {
                if facts.raw_status_body.is_none() {
                    if let Some(caps) = status_re().captures(line) {
                        facts.raw_status_body = Some(caps[1].trim().to_string());
                        facts.status_date = first;
                    }
                }
            }
            "Closed" | "Fixed" => facts.closed_date = facts.closed_date.or(first),
            "Revisit by" | "Revisit" => facts.revisit_by = facts.revisit_by.or(first),
            _ => {}
        }
    }
    facts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::tracker_migration::records::{split_source, SourceKind};

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn split(text: &str) -> Split {
        split_source(text, &SourceKind::File, &[])
    }

    /// Breaks if the splitter ignores `\|` or splits inside backticks.
    #[test]
    fn cells_respect_escaped_pipes_and_code_spans() {
        assert_eq!(
            table_cells("| F-1 | a \\| b | `x|y` | open |"),
            vec!["F-1", "a | b", "`x|y`", "open"]
        );
    }

    #[test]
    fn cells_keep_an_empty_cell_and_ignore_only_the_outer_pipes() {
        assert_eq!(table_cells("| a | | c |"), vec!["a", "", "c"]);
        assert_eq!(table_cells("a | b"), vec!["a", "b"]);
    }

    /// Breaks if rows are keyed by position instead of header text, or the header
    /// is not lowercased.
    #[test]
    fn an_index_table_maps_rows_by_token() {
        // 1 # Log
        // 2
        // 3 | ID | Status | Title |
        // 4 |----|:------:|-------|
        // 5 | F-1 | open | first |
        // 6 | R-22b | fixed | a \| b |
        // 7
        // 8 ## F-1 — first
        // 9 body
        let text = "# Log\n\n| ID | Status | Title |\n|----|:------:|-------|\n\
                    | F-1 | open | first |\n| R-22b | fixed | a \\| b |\n\n\
                    ## F-1 — first\nbody\n";
        let tables = index_tables(text, &split(text));
        assert_eq!(tables.len(), 1);
        let t = &tables[0];
        assert_eq!(t.section_start, 1);
        assert_eq!(t.lines, 3..=6);
        assert_eq!(t.header, vec!["id", "status", "title"]);
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[0].token, "F-1");
        assert_eq!(t.rows[0].line, 5);
        assert_eq!(t.rows[0].raw, "| F-1 | open | first |");
        assert_eq!(t.rows[0].cells["id"], "F-1");
        assert_eq!(t.rows[0].cells["status"], "open");
        assert_eq!(t.rows[0].cells["title"], "first");
        assert_eq!(t.rows[1].token, "R-22b");
        assert_eq!(t.rows[1].line, 6);
        assert_eq!(t.rows[1].cells["title"], "a | b");
    }

    /// Breaks if a table needs no token cell to be an index.
    #[test]
    fn a_table_without_token_rows_is_not_an_index() {
        let text = "# Notes\n\n| Name | Status |\n|---|---|\n| alpha | open |\n";
        assert!(index_tables(text, &split(text)).is_empty());
    }

    /// Breaks if the scan reads record lines as well as section lines.
    #[test]
    fn a_token_table_inside_a_record_is_not_an_index() {
        // 1 # Log
        // 2 ## F-1 — first
        // 3 | ID | Status |
        // 4 |----|--------|
        // 5 | F-2 | open |
        let text = "# Log\n## F-1 — first\n| ID | Status |\n|----|--------|\n| F-2 | open |\n";
        let s = split(text);
        assert_eq!(s.records.len(), 1);
        assert_eq!(s.records[0].lines, 2..=5);
        assert!(index_tables(text, &s).is_empty());
    }

    /// Breaks if fence delimiters are not tracked while scanning a section.
    #[test]
    fn a_fenced_example_row_is_not_an_index() {
        // 1 # Notes
        // 2 ```
        // 3 | ID | Status |
        // 4 |----|--------|
        // 5 | F-1 | open |
        // 6 ```
        let text = "# Notes\n```\n| ID | Status |\n|----|--------|\n| F-1 | open |\n```\n";
        assert!(split(text).records.is_empty());
        assert!(index_tables(text, &split(text)).is_empty());
    }

    /// The real table after a fence is still found (the fence closes).
    #[test]
    fn a_table_after_a_closed_fence_is_found() {
        // 1 ```
        // 2 x
        // 3 ```
        // 4 | ID |
        // 5 |----|
        // 6 | F-9 |
        let text = "```\nx\n```\n| ID |\n|----|\n| F-9 |\n";
        let tables = index_tables(text, &split(text));
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].lines, 4..=6);
        assert_eq!(tables[0].rows[0].line, 6);
    }

    #[test]
    fn status_is_normalised() {
        assert_eq!(
            normalise_status("**Fixed-verified** (2026-10-02)").as_deref(),
            Some("fixed-verified")
        );
        assert_eq!(
            normalise_status("wrong instrument — three claims…").as_deref(),
            Some("wrong instrument")
        );
        assert_eq!(normalise_status("`Open`, still").as_deref(), Some("open"));
        assert_eq!(normalise_status("Done; shipped").as_deref(), Some("done"));
        assert_eq!(normalise_status("Parked: later").as_deref(), Some("parked"));
        assert_eq!(normalise_status("a – b").as_deref(), Some("a"));
        assert_eq!(normalise_status("  **  ** "), None);
    }

    // 1  # Log
    // 2
    // 3  ## F-7 — Broke on 2026-01-05
    // 4
    // 5  - **Status:** Fixed (2026-10-02) — see 2026-10-03 notes
    // 6  **Opened:** 2026-09-30
    // 7  Prose mentions 2025-01-01 here.
    // 8  **Closed:** 2026-10-04, then 2026-10-05
    // 9  **Revisit by:** 2026-12-01
    // 10 **Note:** 2026-02-02 is not a labelled date
    // 11 **Date:** 2026-13-45 is invalid
    const FACTS_SRC: &str = "# Log\n\n## F-7 — Broke on 2026-01-05\n\n\
        - **Status:** Fixed (2026-10-02) — see 2026-10-03 notes\n\
        **Opened:** 2026-09-30\n\
        Prose mentions 2025-01-01 here.\n\
        **Closed:** 2026-10-04, then 2026-10-05\n\
        **Revisit by:** 2026-12-01\n\
        **Note:** 2026-02-02 is not a labelled date\n\
        **Date:** 2026-13-45 is invalid\n";

    /// Breaks if prose or unlisted labels are scanned, or invalid dates kept.
    #[test]
    fn facts_take_dates_from_heading_and_labelled_lines_only() {
        let s = split(FACTS_SRC);
        assert_eq!(s.records.len(), 1);
        let f = record_facts(FACTS_SRC, &s.records[0]);
        assert_eq!(
            f.stated_dates,
            vec![
                d(2026, 1, 5),
                d(2026, 10, 2),
                d(2026, 10, 3),
                d(2026, 9, 30),
                d(2026, 10, 4),
                d(2026, 10, 5),
                d(2026, 12, 1),
            ]
        );
    }

    /// Breaks if a date is taken from the wrong labelled line or not the first one.
    #[test]
    fn facts_read_status_closed_and_revisit_dates() {
        let s = split(FACTS_SRC);
        let f = record_facts(FACTS_SRC, &s.records[0]);
        assert_eq!(
            f.raw_status_body.as_deref(),
            Some("Fixed (2026-10-02) — see 2026-10-03 notes")
        );
        assert_eq!(f.status_date, Some(d(2026, 10, 2)));
        assert_eq!(f.closed_date, Some(d(2026, 10, 4)));
        assert_eq!(f.revisit_by, Some(d(2026, 12, 1)));
    }

    #[test]
    fn facts_accept_label_variants_and_report_absence() {
        // 1 # Log
        // 2 ## R-1 — one
        // 3 **Status**: open
        // 4 * **Fixed:** 2026-03-04
        // 5 **Revisit:** 2026-05-06
        // 6 ## R-2 — two
        // 7 nothing here
        let text = "# Log\n## R-1 — one\n**Status**: open\n* **Fixed:** 2026-03-04\n\
                    **Revisit:** 2026-05-06\n## R-2 — two\nnothing here\n";
        let s = split(text);
        assert_eq!(s.records.len(), 2);
        let one = record_facts(text, &s.records[0]);
        assert_eq!(one.raw_status_body.as_deref(), Some("open"));
        assert_eq!(one.status_date, None);
        assert_eq!(one.closed_date, Some(d(2026, 3, 4)));
        assert_eq!(one.revisit_by, Some(d(2026, 5, 6)));
        assert_eq!(one.stated_dates, vec![d(2026, 3, 4), d(2026, 5, 6)]);
        let two = record_facts(text, &s.records[1]);
        assert_eq!(two.raw_status_body, None);
        assert!(two.stated_dates.is_empty());
        assert_eq!(two.closed_date, None);
    }
}
