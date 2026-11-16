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

use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;

use crate::librarian::identity::grammar::{extract, Reference};
use crate::util::markdown_fence::FenceState;

/// One `File` or `Entry` citation on a labelled line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Relation {
    /// The label text between `**` and `:**`, such as `Rests on`.
    pub label: String,
    pub reference: Reference,
    /// 1-indexed line in the whole document, the numbering `extract` reports.
    pub line: u32,
}

/// Labels that point back at what an entry came from, never forward at what it waits on. A
/// citation under one of these is provenance: the cited document closing later changes nothing
/// for this entry, so `cited_target_closed` ignores it. Any other label counts as forward.
// cap-class: NOT_A_CAP — a closed vocabulary of relation labels, not a limit on any result
pub const BACKWARD_LABELS: [&str; 2] = ["Promoted from", "Supersedes"];

/// Whether `label` is one of the `BACKWARD_LABELS`, ignoring ASCII case.
pub fn is_backward(label: &str) -> bool {
    BACKWARD_LABELS
        .iter()
        .any(|b| b.eq_ignore_ascii_case(label))
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

/// Whether `line` starts the next bold field (`**Valid:**`, `**Status:**`), which ends a paragraph.
fn is_bold_field_label(line: &str) -> bool {
    line.strip_prefix("**").is_some_and(|r| r.contains(":**"))
}

/// The label of every line that lies in the paragraph of a declaring line, keyed by 1-indexed line:
/// the paragraph rule in the module doc, with no regard to what the lines hold. [`relations`] keeps
/// the `File` and `Entry` citations on these lines.
pub fn relation_labels(doc: &str) -> BTreeMap<u32, String> {
    // `extract` numbers lines over the whole text, frontmatter included, counting `\n`; so does
    // `str::lines` (it also drops a `\r`).
    let mut labels: BTreeMap<u32, String> = BTreeMap::new();
    let mut current: Option<String> = None;
    // CommonMark fence rules, the same tracker `statements.rs` uses for the same field.
    let mut fence = FenceState::new();
    for (i, line) in doc.lines().enumerate() {
        let n = i as u32 + 1;
        let trimmed = line.trim_start();
        let is_delim = fence.feed(trimmed);
        if is_delim {
            current = None;
            continue;
        }
        if fence.in_fence() {
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
    labels
}

/// The canonical `rel` of a relation label: its words (split on whitespace) in ASCII lowercase,
/// joined with `_`. `Depends on` gives `depends_on`. The inverse direction of
/// `link_text::label_for_rel`, which reads every spelling of a rel (`depends-on`, `Depends On`)
/// and gives one label, so every spelling lands on one stored value.
pub fn rel_of_label(label: &str) -> String {
    label
        .split_whitespace()
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>()
        .join("_")
}

/// Every `File` or `Entry` citation in `doc` that lies in the paragraph of a declaring line,
/// labelled with that line's label. Bare tokens and malformed targets are not relations.
pub fn relations(doc: &str) -> Vec<Relation> {
    let labels = relation_labels(doc);
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

    // mutation: `rel_of_label` joins the words with a space instead of `_`.
    #[test]
    fn rel_of_label_is_snake_case_of_the_words() {
        assert_eq!(rel_of_label("Depends on"), "depends_on");
        assert_eq!(rel_of_label("Rests  on"), "rests_on");
        assert_eq!(rel_of_label("Supersedes"), "supersedes");
    }

    #[test]
    fn relation_labels_name_every_line_of_each_paragraph() {
        let doc = "**Rests on:** [[a]]\n  wrapped\n\nprose\n**Blocks:** [[b]]\n";
        let labels = relation_labels(doc);
        let got: Vec<(u32, &str)> = labels.iter().map(|(n, l)| (*n, l.as_str())).collect();
        assert_eq!(got, [(1, "Rests on"), (2, "Rests on"), (5, "Blocks")]);
    }

    #[test]
    fn backward_labels_are_exactly_promoted_from_and_supersedes() {
        assert!(is_backward("Promoted from"));
        assert!(is_backward("supersedes"));
        assert!(!is_backward("Rests on"));
        assert!(!is_backward("Promoted to"));
        assert!(!is_backward("Blocks"));
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

    #[test]
    fn a_non_relation_bold_field_ends_the_paragraph_and_is_not_one() {
        // `**Note 1:**` fails the label regex (a digit), so it declares nothing; the `[[b]]` below
        // it must not inherit the Rests on label.
        assert_eq!(
            lines_and_labels("**Rests on:** [[a]]\n**Note 1:** x\n[[b]]\n"),
            vec![l(1, "Rests on")]
        );
    }

    #[test]
    fn a_target_only_on_a_continuation_line_is_a_relation() {
        assert_eq!(
            lines_and_labels("**Rests on:** see\n  [[b]]\n"),
            vec![l(2, "Rests on")]
        );
    }

    #[test]
    fn a_shorter_run_inside_a_longer_fence_does_not_close_it() {
        // CommonMark: line 3 is content, line 4 closes the fence, line 5 is outside it.
        assert_eq!(
            lines_and_labels("````\n```\n````\n**Rests on:** [[a]]\n"),
            vec![l(4, "Rests on")]
        );
    }

    #[test]
    fn a_mismatched_fence_character_does_not_close_it() {
        assert_eq!(
            lines_and_labels("~~~\n```\n~~~\n**Rests on:** [[a]]\n"),
            vec![l(4, "Rests on")]
        );
    }

    #[test]
    fn a_backtick_info_string_is_not_a_fence_opener() {
        // An opener whose backtick info string contains a backtick is prose; the paragraph goes on.
        assert_eq!(
            lines_and_labels("**Rests on:** [[a]]\n```x`y [[b]]\n"),
            vec![l(1, "Rests on"), l(2, "Rests on")]
        );
    }
}
