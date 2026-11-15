//! Split a tracker source into records and non-record sections that tile it.
//!
//! Spec: `docs/superpowers/specs/2026-11-15-tracker-migration-design.md`
//! § What a record is, § Source kinds.
//!
//! [`split_source`] is the part that proves every source line is owned exactly once:
//! the frontmatter, the records and the sections it returns tile `1..=line_count`
//! with no gap and no overlap ([`partition_problems`] is the check). A [`Record`]
//! carries no text; every body is [`slice`] of the source over `record.lines`, so
//! the planner and the check read the same source lines and cannot disagree about
//! what a record holds.
//!
//! Headings come from [`headings::parse`] (fence-aware) and definitions from
//! [`entry_sections`] (fence- and frontmatter-aware, built on the same parser), so
//! this module, the link scan and the previews agree about what a heading, a fence
//! and a definition are. Lines are numbered as `text.lines()` numbers them,
//! 1-indexed; `lines()` drops the `\r` of a CRLF ending, so a CRLF source splits
//! like LF.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;

use crate::librarian::preview::headings;
use crate::librarian::tools::link_scan::extract::entry_sections;

/// 1-indexed, inclusive lines of `text.lines()`.
pub type Lines = std::ops::RangeInclusive<u32>;

/// What a record is in one source (spec § Source kinds; `files` is expanded by the
/// CLI into one `Whole` source per file).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum SourceKind {
    /// A record per definition heading (`TOKEN — title`) and per `extra` heading.
    File,
    /// A record per heading of `level` (token `None`, title = heading text).
    Sections { level: usize },
    /// One record: every line after the frontmatter.
    Whole,
}

/// A heading the mapping names a record (`records_extra`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExtraRecord {
    pub heading_line: u32,
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Record {
    pub token: Option<String>,
    pub heading_line: u32,
    pub level: usize,
    /// The heading text without its `TOKEN — ` part.
    pub title: String,
    pub lines: Lines,
}

/// A run of lines outside the frontmatter and every record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Section {
    /// First line; equal to `*lines.start()`.
    pub start: u32,
    pub lines: Lines,
    /// The text of the first line.
    pub first_line: String,
}

/// A heading whose text starts with a token but that is not a record heading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Possible {
    pub line: u32,
    pub token: String,
    /// The token of the record that contains the heading; `None` in a section, and
    /// also inside a record that has no token (`Sections`, `Whole`).
    pub enclosing: Option<String>,
}

/// A token whose heading appears a second time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Repeat {
    pub token: String,
    pub first_line: u32,
    pub other_line: u32,
    /// The second heading is inside the record of the first and stays part of it.
    pub nested: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Split {
    pub frontmatter: Option<Lines>,
    pub records: Vec<Record>,
    pub sections: Vec<Section>,
    pub possibles: Vec<Possible>,
    pub repeats: Vec<Repeat>,
    pub line_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum PartitionProblem {
    Uncovered(Lines),
    Overlap { a: Lines, b: Lines },
}

/// A record heading's shape: `#…# TOKEN — title`, the token optionally bold and
/// optionally suffixed (`R-56b`).
fn heading_token_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^#{1,6}\s+(?:\*\*)?([A-Z]{1,3}-\d+[a-z]?)(?:\*\*)?\s+[—–-]\s+(.+)$").unwrap()
    })
}

/// `(token, title)` of a record heading line, the title without the `TOKEN — ` part.
pub fn heading_token(line: &str) -> Option<(String, String)> {
    let caps = heading_token_re().captures(line)?;
    Some((caps[1].to_string(), caps[2].trim().to_string()))
}

/// The lines of `text.lines()` in `lines`, joined with `"\n"`.
pub fn slice(text: &str, lines: &Lines) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let skip = lines.start().saturating_sub(1) as usize;
    let take = (lines.end() - lines.start()) as usize + 1;
    text.lines()
        .skip(skip)
        .take(take)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Gaps in `1..=line_count` and overlaps between `owned` ranges. Empty when the
/// ranges tile the lines exactly once. An overlap names the earlier range that
/// reaches furthest and the range that starts inside it.
pub fn partition_problems(line_count: u32, owned: &[Lines]) -> Vec<PartitionProblem> {
    let mut sorted: Vec<&Lines> = owned.iter().filter(|r| !r.is_empty()).collect();
    sorted.sort_by_key(|r| (*r.start(), *r.end()));
    let mut out = Vec::new();
    let mut covered_to = 0u32;
    let mut reach: Option<&Lines> = None;
    for range in sorted {
        let (start, end) = (*range.start(), *range.end());
        if start > covered_to + 1 {
            let gap_end = (start - 1).min(line_count);
            if covered_to < gap_end {
                out.push(PartitionProblem::Uncovered(covered_to + 1..=gap_end));
            }
        } else if let Some(prev) = reach.filter(|_| start <= covered_to) {
            out.push(PartitionProblem::Overlap {
                a: prev.clone(),
                b: range.clone(),
            });
        }
        if end > covered_to {
            covered_to = end;
            reach = Some(range);
        }
    }
    if covered_to < line_count {
        out.push(PartitionProblem::Uncovered(covered_to + 1..=line_count));
    }
    out
}

/// One ATX heading outside the frontmatter and outside fences.
struct Head {
    line: u32,
    level: usize,
    text: String,
}

/// A record heading before its extent is known.
struct Candidate {
    line: u32,
    level: usize,
    token: Option<String>,
    title: String,
}

/// Last line of the frontmatter, `0` when there is none. Uses the same reader and
/// the same `Err(_) => 0` fallback as [`entry_sections`], so a block it refuses is
/// body here too and the two agree about which headings count.
fn frontmatter_last_line(text: &str) -> u32 {
    match crate::librarian::frontmatter::parse(text) {
        Ok((_, body)) => text[..text.len() - body.len()].lines().count() as u32,
        Err(_) => 0,
    }
}

/// Split `text` into frontmatter, records and sections (spec § What a record is).
///
/// - A record heading is a definition heading from [`entry_sections`] (`File`), an
///   `extra` heading (`File`, `Sections`), or any heading of `level` (`Sections`).
///   An `extra` that names no heading line is ignored. `Whole` is one record over
///   every line after the frontmatter.
/// - A record ends before the earlier of the next heading of the same or higher
///   level and the next record heading whose token differs.
/// - A record heading whose token equals the enclosing record's token stays inside
///   it (`Repeat { nested: true }`); any other repeated token is
///   `Repeat { nested: false }`, and both headings are records.
/// - Every other line after the frontmatter is in exactly one section; a section
///   starts at the first uncovered line of a run or at a heading.
/// - A possible record is a heading (in a section or in a record) that
///   [`heading_token`] reads and that is not a record heading; one whose token
///   equals its enclosing record's token is a nested repeat instead.
pub fn split_source(text: &str, kind: &SourceKind, extra: &[ExtraRecord]) -> Split {
    let lines: Vec<&str> = text.lines().collect();
    let line_count = lines.len() as u32;
    let fm_last = frontmatter_last_line(text).min(line_count);
    let frontmatter = (fm_last > 0).then_some(1..=fm_last);
    let heads: Vec<Head> = headings::parse(text)
        .into_iter()
        .filter(|h| h.line as u32 > fm_last)
        .map(|h| Head {
            line: h.line as u32,
            level: h.level as usize,
            text: h.text,
        })
        .collect();
    // `headings::parse` accepts an indented `#`; `heading_token` is anchored at `#`.
    let raw = |line: u32| lines[line as usize - 1].trim_start();
    let title_of = |head: &Head| {
        heading_token(raw(head.line))
            .map(|(_, title)| title)
            .unwrap_or_else(|| head.text.clone())
    };

    let mut repeats = Vec::new();
    let records: Vec<Record> = match kind {
        SourceKind::Whole => {
            if fm_last < line_count {
                let first = heads.first();
                vec![Record {
                    token: None,
                    heading_line: first.map_or(fm_last + 1, |h| h.line),
                    level: first.map_or(0, |h| h.level),
                    title: first.map(|h| h.text.clone()).unwrap_or_default(),
                    lines: fm_last + 1..=line_count,
                }]
            } else {
                Vec::new()
            }
        }
        SourceKind::File | SourceKind::Sections { .. } => {
            let head_at = |line: u32| heads.iter().find(|h| h.line == line);
            let mut candidates: Vec<Candidate> = match kind {
                SourceKind::Sections { level } => heads
                    .iter()
                    .filter(|h| h.level == *level)
                    .map(|h| Candidate {
                        line: h.line,
                        level: h.level,
                        token: None,
                        title: h.text.clone(),
                    })
                    .collect(),
                _ => entry_sections(text)
                    .into_iter()
                    .filter_map(|s| {
                        let head = head_at(s.heading_line)?;
                        Some(Candidate {
                            line: head.line,
                            level: head.level,
                            token: Some(s.id),
                            title: title_of(head),
                        })
                    })
                    .collect(),
            };
            for e in extra {
                if candidates.iter().any(|c| c.line == e.heading_line) {
                    continue;
                }
                if let Some(head) = head_at(e.heading_line) {
                    candidates.push(Candidate {
                        line: head.line,
                        level: head.level,
                        token: Some(e.token.clone()),
                        title: title_of(head),
                    });
                }
            }
            candidates.sort_by_key(|c| c.line);

            // Records never overlap: any record heading with a different token ends
            // the open record, so the only record that can enclose a candidate is
            // the last accepted one, and only while no heading of its level or
            // higher has closed it.
            let mut accepted: Vec<Candidate> = Vec::new();
            for c in candidates {
                if let Some(open) = accepted.last() {
                    let still_open = !heads
                        .iter()
                        .any(|h| h.line > open.line && h.line < c.line && h.level <= open.level);
                    if open.token.is_some()
                        && open.token == c.token
                        && c.level > open.level
                        && still_open
                    {
                        repeats.push(Repeat {
                            token: c.token.clone().unwrap_or_default(),
                            first_line: open.line,
                            other_line: c.line,
                            nested: true,
                        });
                        continue;
                    }
                }
                if let Some(token) = &c.token {
                    if let Some(first) = accepted.iter().find(|r| r.token.as_ref() == Some(token)) {
                        repeats.push(Repeat {
                            token: token.clone(),
                            first_line: first.line,
                            other_line: c.line,
                            nested: false,
                        });
                    }
                }
                accepted.push(c);
            }

            // The next accepted record bounds the extent. When its token equals
            // this one's it was not nested, so a heading of this level or higher
            // comes no later and the level bound already holds: the minimum is the
            // spec's "next record heading whose token differs".
            let next_lines: Vec<Option<u32>> = accepted
                .iter()
                .skip(1)
                .map(|n| Some(n.line))
                .chain(std::iter::once(None))
                .collect();
            accepted
                .into_iter()
                .zip(next_lines)
                .map(|(r, next)| {
                    let by_level = heads
                        .iter()
                        .find(|h| h.line > r.line && h.level <= r.level)
                        .map(|h| h.line - 1);
                    let by_record = next.map(|n| n - 1);
                    let end = by_level
                        .into_iter()
                        .chain(by_record)
                        .min()
                        .unwrap_or(line_count);
                    Record {
                        token: r.token,
                        heading_line: r.line,
                        level: r.level,
                        title: r.title,
                        lines: r.line..=end,
                    }
                })
                .collect()
        }
    };

    // A `Whole` record's `heading_line` is only its first heading, not a record
    // heading: that heading can still be a possible record.
    let record_heads: BTreeSet<u32> = match kind {
        SourceKind::Whole => BTreeSet::new(),
        _ => records.iter().map(|r| r.heading_line).collect(),
    };
    let mut possibles = Vec::new();
    for head in &heads {
        if record_heads.contains(&head.line)
            || repeats
                .iter()
                .any(|r| r.nested && r.other_line == head.line)
        {
            continue;
        }
        let Some((token, _)) = heading_token(raw(head.line)) else {
            continue;
        };
        let enclosing = records.iter().find(|r| r.lines.contains(&head.line));
        match enclosing {
            Some(rec) if rec.token.as_deref() == Some(token.as_str()) => {
                repeats.push(Repeat {
                    token,
                    first_line: rec.heading_line,
                    other_line: head.line,
                    nested: true,
                });
            }
            _ => possibles.push(Possible {
                line: head.line,
                token,
                enclosing: enclosing.and_then(|r| r.token.clone()),
            }),
        }
    }
    repeats.sort_by_key(|r| (r.other_line, r.first_line));

    let mut covered = vec![false; line_count as usize + 1];
    for r in &records {
        for n in r.lines.clone() {
            covered[n as usize] = true;
        }
    }
    let head_lines: BTreeSet<u32> = heads.iter().map(|h| h.line).collect();
    let section = |start: u32, end: u32| Section {
        start,
        lines: start..=end,
        first_line: lines[start as usize - 1].to_string(),
    };
    let mut sections = Vec::new();
    let mut open: Option<u32> = None;
    for n in fm_last + 1..=line_count {
        let covered_here = covered[n as usize];
        if covered_here || open.is_none() || head_lines.contains(&n) {
            if let Some(start) = open.take() {
                sections.push(section(start, n - 1));
            }
        }
        if !covered_here && open.is_none() {
            open = Some(n);
        }
    }
    if let Some(start) = open {
        sections.push(section(start, line_count));
    }

    Split {
        frontmatter,
        records,
        sections,
        possibles,
        repeats,
        line_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every line range the split owns: frontmatter, records, sections.
    fn owned(split: &Split) -> Vec<Lines> {
        let mut out: Vec<Lines> = split.frontmatter.iter().cloned().collect();
        out.extend(split.records.iter().map(|r| r.lines.clone()));
        out.extend(split.sections.iter().map(|s| s.lines.clone()));
        out
    }

    fn assert_tiles(split: &Split) {
        assert_eq!(
            partition_problems(split.line_count, &owned(split)),
            vec![],
            "split does not tile the source: {split:#?}"
        );
    }

    /// (token, heading_line, lines) of every record.
    fn record_shape(split: &Split) -> Vec<(Option<&str>, u32, Lines)> {
        split
            .records
            .iter()
            .map(|r| (r.token.as_deref(), r.heading_line, r.lines.clone()))
            .collect()
    }

    /// (lines, first_line) of every section.
    fn section_shape(split: &Split) -> Vec<(Lines, &str)> {
        split
            .sections
            .iter()
            .map(|s| (s.lines.clone(), s.first_line.as_str()))
            .collect()
    }

    const TILED: &str = "---\n\
title: t\n\
---\n\
Preamble text.\n\
\n\
## Index\n\
| a |\n\
\n\
## R-1 — a\n\
body one\n\
### notes\n\
note text\n\
## R-2 — b\n\
body two\n\
\n\
## Template\n\
template body\n";

    #[test]
    fn records_and_sections_tile_the_source() {
        let split = split_source(TILED, &SourceKind::File, &[]);
        assert_eq!(split.line_count, 17);
        assert_eq!(split.frontmatter, Some(1..=3));
        assert_eq!(
            split.records,
            vec![
                Record {
                    token: Some("R-1".into()),
                    heading_line: 9,
                    level: 2,
                    title: "a".into(),
                    lines: 9..=12,
                },
                Record {
                    token: Some("R-2".into()),
                    heading_line: 13,
                    level: 2,
                    title: "b".into(),
                    lines: 13..=15,
                },
            ]
        );
        assert_eq!(
            section_shape(&split),
            vec![
                (4..=5, "Preamble text."),
                (6..=8, "## Index"),
                (16..=17, "## Template"),
            ]
        );
        assert_eq!(
            split.sections.iter().map(|s| s.start).collect::<Vec<_>>(),
            vec![4, 6, 16]
        );
        assert!(split.possibles.is_empty());
        assert!(split.repeats.is_empty());
        assert_tiles(&split);
        assert_eq!(
            slice(TILED, &split.records[0].lines),
            "## R-1 — a\nbody one\n### notes\nnote text"
        );
    }

    #[test]
    fn a_nested_child_ends_its_parent() {
        let text = "## SRI-1 — parent\n\
parent body\n\
### T-2 — child\n\
child body\n\
### more\n\
more body\n\
## SRI-3 — next\n";
        let split = split_source(text, &SourceKind::File, &[]);
        assert_eq!(
            record_shape(&split),
            vec![
                (Some("SRI-1"), 1, 1..=2),
                (Some("T-2"), 3, 3..=4),
                (Some("SRI-3"), 7, 7..=7),
            ]
        );
        assert_eq!(section_shape(&split), vec![(5..=6, "### more")]);
        assert_tiles(&split);
    }

    #[test]
    fn a_repeated_token_under_its_own_record_nests() {
        let text = "## A-38 — x\n\
body\n\
### A-38 — sub\n\
sub body\n\
## A-39 — y\n";
        let split = split_source(text, &SourceKind::File, &[]);
        assert_eq!(
            record_shape(&split),
            vec![(Some("A-38"), 1, 1..=4), (Some("A-39"), 5, 5..=5)]
        );
        assert_eq!(
            split.repeats,
            vec![Repeat {
                token: "A-38".into(),
                first_line: 1,
                other_line: 3,
                nested: true,
            }]
        );
        assert!(split.possibles.is_empty());
        assert!(split.sections.is_empty());
        assert_tiles(&split);
    }

    #[test]
    fn a_bold_repeat_under_its_own_record_nests_and_is_not_possible() {
        // Not a definition (`def_re` takes no `**`), but `heading_token` reads it:
        // a possible record whose token equals the enclosing one is a nested repeat.
        let text = "## A-38 — x\n\
body\n\
### **A-38** — sub\n\
sub body\n";
        let split = split_source(text, &SourceKind::File, &[]);
        assert_eq!(record_shape(&split), vec![(Some("A-38"), 1, 1..=4)]);
        assert_eq!(
            split.repeats,
            vec![Repeat {
                token: "A-38".into(),
                first_line: 1,
                other_line: 3,
                nested: true,
            }]
        );
        assert!(split.possibles.is_empty());
        assert_tiles(&split);
    }

    #[test]
    fn a_repeated_token_elsewhere_is_reported() {
        let text = "## F-1 — first\n\
a\n\
## F-1 — second\n\
b\n";
        let split = split_source(text, &SourceKind::File, &[]);
        assert_eq!(
            record_shape(&split),
            vec![(Some("F-1"), 1, 1..=2), (Some("F-1"), 3, 3..=4)]
        );
        assert_eq!(
            split.repeats,
            vec![Repeat {
                token: "F-1".into(),
                first_line: 1,
                other_line: 3,
                nested: false,
            }]
        );
        assert_tiles(&split);
    }

    #[test]
    fn a_same_token_child_after_the_parent_closed_is_not_nested() {
        // F-1 ends at `## Notes`; the later `### F-1 — x` is outside it.
        let text = "## F-1 — first\n\
a\n\
## Notes\n\
### F-1 — x\n\
b\n";
        let split = split_source(text, &SourceKind::File, &[]);
        assert_eq!(
            record_shape(&split),
            vec![(Some("F-1"), 1, 1..=2), (Some("F-1"), 4, 4..=5)]
        );
        assert_eq!(section_shape(&split), vec![(3..=3, "## Notes")]);
        assert_eq!(
            split.repeats,
            vec![Repeat {
                token: "F-1".into(),
                first_line: 1,
                other_line: 4,
                nested: false,
            }]
        );
        assert_tiles(&split);
    }

    #[test]
    fn a_suffixed_heading_in_a_section_is_a_possible_record() {
        let text = "# Log\n\
\n\
## R-56b — Hit: one\n\
body\n";
        let split = split_source(text, &SourceKind::File, &[]);
        assert!(split.records.is_empty());
        assert_eq!(
            split.possibles,
            vec![Possible {
                line: 3,
                token: "R-56b".into(),
                enclosing: None,
            }]
        );
        assert_eq!(
            section_shape(&split),
            vec![(1..=2, "# Log"), (3..=4, "## R-56b — Hit: one")]
        );
        assert_tiles(&split);

        let extra = [ExtraRecord {
            heading_line: 3,
            token: "R-56b".into(),
        }];
        let split = split_source(text, &SourceKind::File, &extra);
        assert_eq!(
            split.records,
            vec![Record {
                token: Some("R-56b".into()),
                heading_line: 3,
                level: 2,
                title: "Hit: one".into(),
                lines: 3..=4,
            }]
        );
        assert!(split.possibles.is_empty());
        assert_eq!(section_shape(&split), vec![(1..=2, "# Log")]);
        assert_tiles(&split);
    }

    #[test]
    fn an_extra_that_names_no_heading_is_ignored() {
        let text = "## F-1 — a\nbody\n";
        let extra = [ExtraRecord {
            heading_line: 2,
            token: "F-9".into(),
        }];
        let split = split_source(text, &SourceKind::File, &extra);
        assert_eq!(record_shape(&split), vec![(Some("F-1"), 1, 1..=2)]);
        assert_tiles(&split);
    }

    #[test]
    fn a_possible_record_inside_a_record_is_reported() {
        let text = "## SRI-15 — parent\n\
a\n\
### T-15a — sub\n\
b\n\
## SRI-16 — next\n";
        let split = split_source(text, &SourceKind::File, &[]);
        assert_eq!(
            record_shape(&split),
            vec![(Some("SRI-15"), 1, 1..=4), (Some("SRI-16"), 5, 5..=5)]
        );
        assert_eq!(
            split.possibles,
            vec![Possible {
                line: 3,
                token: "T-15a".into(),
                enclosing: Some("SRI-15".into()),
            }]
        );
        assert_tiles(&split);

        // Named a record, it ends its parent like any other record heading.
        let extra = [ExtraRecord {
            heading_line: 3,
            token: "T-15a".into(),
        }];
        let split = split_source(text, &SourceKind::File, &extra);
        assert_eq!(
            record_shape(&split),
            vec![
                (Some("SRI-15"), 1, 1..=2),
                (Some("T-15a"), 3, 3..=4),
                (Some("SRI-16"), 5, 5..=5),
            ]
        );
        assert!(split.possibles.is_empty());
        assert_tiles(&split);
    }

    #[test]
    fn a_heading_with_utf_8_in_the_middle_is_not_possible() {
        let text = "## Notes on UTF-8 handling\n\
a\n\
## UTF-8 notes\n\
b\n";
        let split = split_source(text, &SourceKind::File, &[]);
        assert!(split.possibles.is_empty());
        assert!(split.records.is_empty());
        assert_eq!(
            section_shape(&split),
            vec![
                (1..=2, "## Notes on UTF-8 handling"),
                (3..=4, "## UTF-8 notes")
            ]
        );
        assert_tiles(&split);
    }

    #[test]
    fn a_fenced_definition_is_not_a_record() {
        let text = "## F-1 — real\n\
```\n\
## F-2 — example\n\
```\n\
after\n";
        let split = split_source(text, &SourceKind::File, &[]);
        assert_eq!(record_shape(&split), vec![(Some("F-1"), 1, 1..=5)]);
        assert!(split.possibles.is_empty());
        assert!(split.repeats.is_empty());
        assert!(split.sections.is_empty());
        assert_tiles(&split);
    }

    #[test]
    fn a_heading_in_frontmatter_is_not_a_record() {
        // YAML comments shaped like headings: `headings::parse` is not
        // frontmatter-aware, so the split must drop them itself.
        let text = "---\n\
title: t\n\
## F-9 — in yaml\n\
# note\n\
---\n\
## F-1 — a\n\
body\n";
        let split = split_source(text, &SourceKind::File, &[]);
        assert_eq!(split.frontmatter, Some(1..=5));
        assert_eq!(record_shape(&split), vec![(Some("F-1"), 6, 6..=7)]);
        assert!(split.possibles.is_empty(), "{:?}", split.possibles);
        assert!(split.repeats.is_empty());
        assert!(split.sections.is_empty());
        assert_tiles(&split);
    }

    #[test]
    fn a_trailing_template_is_a_section() {
        let text = "## F-1 — a\n\
body\n\
\n\
## Template\n\
### F-NNN — title\n\
fields\n";
        let split = split_source(text, &SourceKind::File, &[]);
        assert_eq!(record_shape(&split), vec![(Some("F-1"), 1, 1..=3)]);
        // Every heading outside a record starts a section of its own.
        assert_eq!(
            section_shape(&split),
            vec![(4..=4, "## Template"), (5..=6, "### F-NNN — title")]
        );
        assert!(split.possibles.is_empty());
        assert_tiles(&split);
    }

    #[test]
    fn a_crlf_source_splits_like_lf() {
        let crlf = TILED.replace('\n', "\r\n");
        let lf_split = split_source(TILED, &SourceKind::File, &[]);
        let crlf_split = split_source(&crlf, &SourceKind::File, &[]);
        assert_eq!(crlf_split, lf_split);
        assert_eq!(crlf_split.frontmatter, Some(1..=3));
        assert_eq!(crlf_split.records.len(), 2);
        for r in &crlf_split.records {
            assert_eq!(slice(&crlf, &r.lines), slice(TILED, &r.lines));
        }
        assert!(!slice(&crlf, &crlf_split.records[0].lines).contains('\r'));
    }

    #[test]
    fn sections_kind_splits_on_the_level() {
        let text = "# Rulings\n\
intro\n\
## First ruling\n\
a\n\
### detail\n\
b\n\
## Second ruling\n\
c\n";
        let split = split_source(text, &SourceKind::Sections { level: 2 }, &[]);
        assert_eq!(
            split.records,
            vec![
                Record {
                    token: None,
                    heading_line: 3,
                    level: 2,
                    title: "First ruling".into(),
                    lines: 3..=6,
                },
                Record {
                    token: None,
                    heading_line: 7,
                    level: 2,
                    title: "Second ruling".into(),
                    lines: 7..=8,
                },
            ]
        );
        assert_eq!(section_shape(&split), vec![(1..=2, "# Rulings")]);
        assert_tiles(&split);
    }

    #[test]
    fn whole_kind_is_one_record() {
        // The frontmatter holds heading-shaped YAML comments; the record's heading
        // comes from the first heading AFTER the frontmatter.
        let text = "---\n\
title: h\n\
# Draft\n\
## F-9 — in yaml\n\
---\n\
# Handoff\n\
text\n\
## F-1 — x\n";
        let split = split_source(text, &SourceKind::Whole, &[]);
        assert_eq!(split.frontmatter, Some(1..=5));
        assert_eq!(
            split.records,
            vec![Record {
                token: None,
                heading_line: 6,
                level: 1,
                title: "Handoff".into(),
                lines: 6..=8,
            }]
        );
        assert!(split.sections.is_empty());
        assert!(split.repeats.is_empty());
        assert_eq!(
            split.possibles,
            vec![Possible {
                line: 8,
                token: "F-1".into(),
                enclosing: None,
            }]
        );
        assert_tiles(&split);
    }

    #[test]
    fn heading_token_strips_the_token_from_the_title() {
        assert_eq!(
            heading_token("## R-1 — 日本語"),
            Some(("R-1".into(), "日本語".into()))
        );
        assert_eq!(
            heading_token("### **F-12a** – t"),
            Some(("F-12a".into(), "t".into()))
        );
        assert_eq!(
            heading_token("#### A-1 - x"),
            Some(("A-1".into(), "x".into()))
        );
        assert_eq!(heading_token("## F-1 title"), None);
        assert_eq!(heading_token("## Notes"), None);
        assert_eq!(heading_token("####### A-1 — x"), None);
        assert_eq!(heading_token("## ABCD-1 — x"), None);
        assert_eq!(heading_token("F-1 — not a heading"), None);
    }

    #[test]
    fn slice_joins_the_range_with_newlines() {
        let text = "a\r\nb\nc\n";
        assert_eq!(slice(text, &(1..=2)), "a\nb");
        assert_eq!(slice(text, &(3..=3)), "c");
        #[allow(clippy::reversed_empty_ranges)]
        let empty = 3..=2;
        assert_eq!(slice(text, &empty), "");
    }

    #[test]
    fn partition_problems_reports_a_gap_and_an_overlap() {
        assert_eq!(
            partition_problems(10, &[3..=5, 1..=3, 8..=9]),
            vec![
                PartitionProblem::Overlap { a: 1..=3, b: 3..=5 },
                PartitionProblem::Uncovered(6..=7),
                PartitionProblem::Uncovered(10..=10),
            ]
        );
        assert_eq!(
            partition_problems(3, &[]),
            vec![PartitionProblem::Uncovered(1..=3)]
        );
        assert_eq!(partition_problems(0, &[]), vec![]);
        assert_eq!(partition_problems(4, &[1..=2, 3..=4]), vec![]);
        // A range wholly inside another overlaps it.
        assert_eq!(
            partition_problems(5, &[1..=5, 2..=3]),
            vec![PartitionProblem::Overlap { a: 1..=5, b: 2..=3 }]
        );
    }
}
