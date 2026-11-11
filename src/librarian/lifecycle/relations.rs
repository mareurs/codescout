//! Relation lines: body lines of the form `**<Label>:** … [[…]] …`.
//!
//! The grammar decides what is a citation (code, headings and frontmatter never are); this module
//! only decides which lines carry a label. A citation is a relation when its line does.

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

/// The label of a line whose first non-whitespace text is `**<Label>:**`.
fn label_of(line: &str) -> Option<String> {
    static LABEL: OnceLock<Regex> = OnceLock::new();
    let re = LABEL.get_or_init(|| {
        Regex::new(r"^\s*\*\*([A-Z][A-Za-z ]*[a-z]):\*\*").expect("label regex is valid")
    });
    re.captures(line).map(|c| c[1].trim().to_string())
}

/// Every `File` or `Entry` citation in `doc` whose source line starts with a label.
/// Bare tokens and malformed targets are not relations.
pub fn relations(doc: &str) -> Vec<Relation> {
    // `extract` numbers lines over the whole text, frontmatter included, counting `\n`; so does
    // `str::lines`.
    let labels: HashMap<u32, String> = doc
        .lines()
        .enumerate()
        .filter_map(|(i, l)| label_of(l).map(|label| (i as u32 + 1, label)))
        .collect();
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
    #[test]
    fn the_inline_code_line_is_a_label_line_the_grammar_empties() {
        assert_eq!(
            label_of("**Rests on:** `[[x-doc]]`"),
            Some("Rests on".to_string())
        );
        assert_eq!(
            label_of("  **Rests on:** [[x-doc]]"),
            Some("Rests on".to_string())
        );
        assert_eq!(label_of("Note: **Rests on:** [[x-doc]]"), None);
        assert_eq!(label_of("**rests on:** [[x-doc]]"), None);
    }
}
