//! Relation lines: body lines of the form `**<Label>:** … [[…]] …`.
//!
//! The grammar (`extract`) decides what is a citation: code, headings and frontmatter never are.
//! This module decides which citations belong to a relation, with the paragraph rule that the
//! existing reader of the same field, `statements.rs` (`rests_re`, `first_declaration_paragraph`),
//! applies to `**Rests on:**`:
//!
//! - A declaring line starts at column 0 with `**<Label>:**`. An indented or listed one is prose
//!   under a list item, not a declaration.
//! - The relation's paragraph is the declaring line plus the lines after it, until a blank line, a
//!   heading, a fence delimiter, or the next line that starts a bold field (`**…:**`; that line may
//!   itself declare a new relation). Hard-wrapped continuation lines carry targets, so a
//!   line-only rule would drop them.
//! - Every `File` or `Entry` citation in that paragraph is a `Relation` with the paragraph's
//!   label and its own line. `Bare` and `Malformed` citations are dropped.

use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;

use crate::librarian::identity::grammar::{extract, Reference};

/// One `File` or `Entry` citation on a labelled line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Relation {
    /// The label text between `**` and `:**`, such as `Rests on`.
    pub label: String,
    pub reference: Reference,
    /// 1-indexed line in the whole document, the numbering `extract` reports.
    pub line: u32,
}

/// The label of a declaring line: one that starts at column 0 with `**<Label>:**`. An indented or
/// listed `**Rests on:**` is prose under a list item, not a declaration (as in `statements.rs`).
fn label_of(line: &str) -> Option<String> {
    static LABEL: OnceLock<Regex> = OnceLock::new();
    let re = LABEL.get_or_init(|| {
        Regex::new(r"^\*\*([A-Z][A-Za-z ]*[a-z]):\*\*").expect("label regex is valid")
    });
    re.captures(line).map(|c| c[1].trim().to_string())
}

/// A line that opens a code fence (backtick or tilde), after leading whitespace.
fn is_fence_delimiter(trimmed: &str) -> bool {
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

/// Whether `line` starts the next bold field (`**Valid:**`, `**Status:**`), which ends a paragraph.
fn is_bold_field_label(line: &str) -> bool {
    line.strip_prefix("**").is_some_and(|r| r.contains(":**"))
}

/// Every `File` or `Entry` citation in `doc` that lies in the paragraph of a declaring line,
/// labelled with that line's label. Bare tokens and malformed targets are not relations.
pub fn relations(doc: &str) -> Vec<Relation> {
    // Line -> label of the paragraph that line belongs to. `extract` numbers lines over the whole
    // text, frontmatter included, counting `\n`; so does `str::lines` (it also drops a `\r`).
    let mut labels: HashMap<u32, String> = HashMap::new();
    let mut current: Option<String> = None;
    let mut in_fence = false;
    for (i, line) in doc.lines().enumerate() {
        let n = i as u32 + 1;
        let trimmed = line.trim_start();
        if is_fence_delimiter(trimmed) {
            in_fence = !in_fence;
            current = None;
            continue;
        }
        if in_fence {
            continue;
        }
        if current.is_some() {
            if trimmed.is_empty() || trimmed.starts_with('#') {
                current = None;
                continue;
            }
            if !is_bold_field_label(line) {
                labels.insert(n, current.clone().expect("checked is_some"));
                continue;
            }
            // The next bold field ends this paragraph and may declare its own.
            current = None;
        }
        if let Some(label) = label_of(line) {
            labels.insert(n, label.clone());
            current = Some(label);
        }
    }
    extract(doc)
        .into_iter()
        .filter(|c| {
            matches!(
                c.reference,
                Reference::File { .. } | Reference::Entry { .. }
            )
        })
        .filter_map(|c| {
            labels.get(&c.line).map(|label| Relation {
                label: label.clone(),
                reference: c.reference,
                line: c.line,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labelled_lines_give_relations_and_other_lines_do_not() {
        let doc = "---\ntitle: x\n---\n**Rests on:** [[2026-08-27-negative-results]] and [[adr-b]]\n\nSee [[not-a-relation]].\n**Promoted to:** [[declared-not-wired]]\n";
        let rs = relations(doc);
        let got: Vec<(&str, String)> = rs
            .iter()
            .map(|r| (r.label.as_str(), format!("{:?}", r.reference)))
            .collect();
        assert_eq!(rs.len(), 3, "{got:?}");
        assert_eq!(rs[0].label, "Rests on");
        assert_eq!(rs[2].label, "Promoted to");
        assert_eq!(rs[2].line, 7);
    }

    #[test]
    fn code_and_prose_labels_are_not_relations() {
        assert!(relations("**Rests on:** `[[x-doc]]`\n").is_empty());
        assert!(relations("```\n**Rests on:** [[x-doc]]\n```\n").is_empty());
        assert!(relations("Note: **rests on:** [[x-doc]]\n").is_empty());
    }

    // The code-span case above is empty only because the grammar drops the link, not because the
    // label is missed: pin that the label line itself is recognised, so a label-regex bug cannot
    // hide behind the grammar and a regex-over-lines implementation could not pass.
    // The code-span case above is empty only because the grammar drops the link, not because the
    // label is missed: pin that the label line itself is recognised, so a label-regex bug cannot
    // hide behind the grammar and a regex-over-lines implementation could not pass.
    #[test]
    fn the_inline_code_line_is_a_label_line_the_grammar_empties() {
        assert_eq!(
            label_of("**Rests on:** `[[x-doc]]`"),
            Some("Rests on".to_string())
        );
        assert_eq!(label_of("Note: **Rests on:** [[x-doc]]"), None);
        assert_eq!(label_of("**rests on:** [[x-doc]]"), None);
    }

    fn lines_and_labels(doc: &str) -> Vec<(u32, String)> {
        relations(doc)
            .into_iter()
            .map(|r| (r.line, r.label))
            .collect()
    }

    fn l(line: u32, label: &str) -> (u32, String) {
        (line, label.to_string())
    }

    #[test]
    fn a_wrapped_paragraph_keeps_its_label_and_each_citation_line() {
        let rs = relations("**Rests on:** [[a]] and\n  [[b]]\n");
        assert_eq!(rs.len(), 2, "{rs:?}");
        assert_eq!((rs[0].line, rs[0].label.as_str()), (1, "Rests on"));
        assert_eq!((rs[1].line, rs[1].label.as_str()), (2, "Rests on"));
    }

    #[test]
    fn crlf_wrapped_paragraph_numbers_lines_like_extract() {
        let rs = relations("**Rests on:** [[a]]\r\n  [[b]]\r\n");
        let got: Vec<(u32, &str)> = rs.iter().map(|r| (r.line, r.label.as_str())).collect();
        assert_eq!(got, vec![(1, "Rests on"), (2, "Rests on")]);
    }

    #[test]
    fn an_indented_or_listed_label_is_prose_not_a_declaration() {
        assert!(relations("  **Rests on:** [[x-doc]]\n").is_empty());
        assert!(relations("- **Rests on:** [[x-doc]]\n").is_empty());
        assert!(relations("> **Rests on:** [[x-doc]]\n").is_empty());
    }

    #[test]
    fn a_blank_line_ends_the_paragraph() {
        assert_eq!(
            lines_and_labels("**Rests on:** [[a]]\n\n[[b]]\n"),
            vec![l(1, "Rests on")]
        );
    }

    #[test]
    fn a_heading_ends_the_paragraph() {
        assert_eq!(
            lines_and_labels("**Rests on:** [[a]]\n## H\n[[b]]\n"),
            vec![l(1, "Rests on")]
        );
    }

    #[test]
    fn a_fence_ends_the_paragraph() {
        assert_eq!(
            lines_and_labels("**Rests on:** [[a]]\n```\n[[c]]\n```\n[[b]]\n"),
            vec![l(1, "Rests on")]
        );
        assert_eq!(
            lines_and_labels("**Rests on:** [[a]]\n~~~\n[[c]]\n~~~\n[[b]]\n"),
            vec![l(1, "Rests on")]
        );
    }

    #[test]
    fn the_next_bold_label_ends_the_paragraph_and_may_start_its_own() {
        assert_eq!(
            lines_and_labels("**Rests on:** [[a]]\n**Promote when:** [[z]]\n"),
            vec![l(1, "Rests on"), l(2, "Promote when")]
        );
        // A bold field that is not a relation label still ends the paragraph.
        assert_eq!(
            lines_and_labels("**Rests on:** [[a]]\n**valid:** [[z]]\n"),
            vec![l(1, "Rests on")]
        );
    }

    #[test]
    fn a_label_line_inside_a_fence_declares_nothing() {
        assert!(relations("```\n**Rests on:** [[a]]\n  [[b]]\n```\n").is_empty());
    }

    #[test]
    fn bare_and_malformed_citations_are_dropped_and_entry_is_kept() {
        assert!(relations("**Rests on:** R-91 [[Not A Key]]\n").is_empty());
        let rs = relations("**Rests on:** [[some-key:F-3]]\n");
        assert_eq!(rs.len(), 1, "{rs:?}");
        assert!(
            matches!(rs[0].reference, Reference::Entry { .. }),
            "{:?}",
            rs[0].reference
        );
    }

    #[test]
    fn inline_code_in_a_wrapped_paragraph_is_still_no_relation() {
        assert!(relations("**Rests on:** `[[x-doc]]`\n  `[[y-doc]]`\n").is_empty());
    }
}
