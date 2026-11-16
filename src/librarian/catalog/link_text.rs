//! A manual link as committed markdown: one relation line under the source document's
//! `## Relations` heading, such as `**Supersedes:** [[dst-key]]`.
//!
//! The catalog keeps a link as an `artifact_link` row. Plan C2 writes the same fact into the
//! document it starts from, so Plan D can export old rows into this format and Plan E can stop
//! reading the rows. This module builds the label, splices the line and does no I/O. A label is
//! read back from text by [`crate::librarian::lifecycle::relations::relations`]; the derived
//! index stores none.

use crate::librarian::catalog::event_text::{home_lines, splice_under_home};
use crate::librarian::frontmatter;
use crate::librarian::lifecycle::relations::relations;
use anyhow::{bail, Result};

/// The heading that is the home of every relation line of a document.
pub(crate) const RELATIONS_HEADING: &str = "## Relations";

/// The relation label for a link `rel`: its words (split on `_`, `-` and space) in lowercase,
/// joined with spaces, the first letter a capital. `depends_on` gives `Depends on`.
///
/// The error says why `rel` gives no label. A label is what `lifecycle::relations` reads: ASCII
/// letters and single spaces, at least two characters, the last one lowercase.
pub(crate) fn label_for_rel(rel: &str) -> Result<String, String> {
    let words: Vec<String> = rel
        .split(['_', '-', ' '])
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect();
    let mut label = words.join(" ");
    if let Some(first) = label.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    valid_label(&label).map(|()| label)
}

/// `doc` with the line `**<label>:** <form>` added at the end of its `## Relations` section, after
/// one blank line. The heading is created at the end of the body when absent. `Ok(None)` when the
/// identical line is already in that section (a repeated link adds nothing).
///
/// `form` is one grammar token such as `[[key]]` or `[[key:F-3]]`, without whitespace. It is an
/// error when `form` is not such a token, when the frontmatter does not parse, or when the new line
/// would not be read back as a relation of `label` (for example an unclosed code fence before the
/// insertion point swallows it): the check re-reads the result with `lifecycle::relations`.
pub(crate) fn splice_relation(doc: &str, label: &str, form: &str) -> Result<Option<String>> {
    if let Err(why) = valid_label(label) {
        bail!(
            "`{label}` is not a relation label: {why}",
            label = label.escape_debug()
        );
    }
    if !(form.starts_with("[[") && form.ends_with("]]") && !form.contains(char::is_whitespace)) {
        bail!(
            "relation target {:?} is not one [[…]] token without whitespace",
            form
        );
    }
    let (_, body) = frontmatter::parse(doc)?;
    let line = format!("**{label}:** {form}");
    if home_lines(body, RELATIONS_HEADING)
        .iter()
        .any(|l| l.trim_end_matches('\r') == line)
    {
        return Ok(None);
    }
    let out = splice_under_home(doc, body, RELATIONS_HEADING, &format!("{line}\n"));
    // An open code fence, or other text that swallows the new line, would make the relation
    // invisible to the reader, and a repeat of the call would then add it again.
    if readable_copies(&out, label, &line) != readable_copies(doc, label, &line) + 1 {
        bail!(
            "the relation line `{line}` would not be readable back from this document (an \
             unclosed code fence before the insertion point is the usual cause); nothing was added"
        );
    }
    Ok(Some(out))
}

/// Whether `label` is one the reader takes: ASCII letters and spaces, a capital first, a
/// lowercase letter last, so at least two characters. This restates the pattern of
/// `lifecycle::relations::label_of`; `splice_relation` re-reads its result, so a drift between
/// the two shows as a refusal and never as a line that reads back as nothing.
fn valid_label(label: &str) -> Result<(), String> {
    let first = label.chars().next();
    let last = label.chars().last();
    if !label.chars().all(|c| c.is_ascii_alphabetic() || c == ' ') {
        return Err("a rel may hold only letters, and `_`, `-` and space between words (a digit or another character gives no label)".to_string());
    }
    if label.len() < 2
        || !first.is_some_and(|c| c.is_ascii_uppercase())
        || !last.is_some_and(|c| c.is_ascii_lowercase())
    {
        return Err(
            "a label needs two letters or more, a capital first and a lowercase letter last"
                .to_string(),
        );
    }
    Ok(())
}

/// How many lines of `doc` are exactly `line` and are read as a relation of `label`.
fn readable_copies(doc: &str, label: &str, line: &str) -> usize {
    let read: std::collections::HashSet<u32> = relations(doc)
        .into_iter()
        .filter(|r| r.label == label)
        .map(|r| r.line)
        .collect();
    doc.lines()
        .enumerate()
        .filter(|(ix, l)| *l == line && read.contains(&(*ix as u32 + 1)))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::identity::grammar::parse_target;

    const DOC: &str = "---\ntitle: T\n---\n\n# T\n\nbody\n";

    fn count_of(doc: &str, needle: &str) -> usize {
        doc.matches(needle).count()
    }

    #[test]
    fn rel_words_become_a_label() {
        assert_eq!(label_for_rel("depends_on").unwrap(), "Depends on");
        assert_eq!(label_for_rel("see-also").unwrap(), "See also");
        assert_eq!(label_for_rel("implements").unwrap(), "Implements");
        assert_eq!(label_for_rel("supersedes").unwrap(), "Supersedes");
        assert_eq!(label_for_rel("Rests ON").unwrap(), "Rests on");
        assert_eq!(
            label_for_rel("__rests--on  ").unwrap(),
            "Rests on",
            "empty words are dropped"
        );
    }

    // mutation: `rel_of_label` keeps the case of the label's letters.
    #[test]
    fn every_spelling_of_one_label_has_one_canonical_rel() {
        use crate::librarian::lifecycle::relations::rel_of_label;
        for rel in ["depends_on", "depends-on", "Depends On", "__depends--on "] {
            let label = label_for_rel(rel).unwrap();
            assert_eq!(rel_of_label(&label), "depends_on", "{rel:?}");
        }
    }

    #[test]
    fn a_rel_with_a_digit_is_not_a_label() {
        let why = label_for_rel("depends2").unwrap_err();
        assert!(why.contains("letter"), "{why}");
    }

    #[test]
    fn a_rel_that_gives_no_valid_label_says_why() {
        for rel in ["", "_-_", "a", "dép", "see/also", "x\ny", "a:b"] {
            let why = label_for_rel(rel).expect_err(rel);
            assert!(!why.is_empty(), "{rel:?}");
            assert!(
                !why.contains('\n'),
                "the reason never echoes the rel's own line ends: {why:?}"
            );
        }
    }

    #[test]
    fn a_new_line_goes_under_one_relations_heading() {
        let once = splice_relation(DOC, "Implements", "[[dst]]")
            .unwrap()
            .unwrap();
        assert!(once.starts_with("---\ntitle: T\n---\n"), "{once}");
        assert!(
            once.ends_with("body\n\n## Relations\n\n**Implements:** [[dst]]\n"),
            "{once}"
        );
        let twice = splice_relation(&once, "Rests on", "[[other]]")
            .unwrap()
            .unwrap();
        assert_eq!(count_of(&twice, "## Relations"), 1, "{twice}");
        assert!(
            twice.ends_with("**Implements:** [[dst]]\n\n**Rests on:** [[other]]\n"),
            "{twice}"
        );
    }

    #[test]
    fn a_line_joins_the_end_of_an_existing_section_and_leaves_the_next_heading_alone() {
        let doc = "# T\n\n## Relations\n\n**Rests on:** [[a]]\n\n## Later\n\ntext\n";
        let out = splice_relation(doc, "Implements", "[[b]]")
            .unwrap()
            .unwrap();
        assert_eq!(
            out,
            "# T\n\n## Relations\n\n**Rests on:** [[a]]\n\n**Implements:** [[b]]\n\n## Later\n\ntext\n"
        );
    }

    #[test]
    fn the_same_line_twice_is_a_no_op() {
        let once = splice_relation(DOC, "Implements", "[[dst]]")
            .unwrap()
            .unwrap();
        assert_eq!(
            splice_relation(&once, "Implements", "[[dst]]").unwrap(),
            None
        );
        // The same target under another label is a new fact, and so is another target.
        assert!(splice_relation(&once, "Rests on", "[[dst]]")
            .unwrap()
            .is_some());
        assert!(splice_relation(&once, "Implements", "[[dst2]]")
            .unwrap()
            .is_some());
    }

    #[test]
    fn an_identical_line_outside_the_relations_section_does_not_count() {
        let doc =
            "# T\n\n**Implements:** [[dst]]\n\n```\n## Relations\n**Implements:** [[dst]]\n```\n";
        let out = splice_relation(doc, "Implements", "[[dst]]")
            .unwrap()
            .expect("a line elsewhere is not the idempotent copy");
        assert!(
            out.ends_with("```\n\n## Relations\n\n**Implements:** [[dst]]\n"),
            "{out}"
        );
    }

    #[test]
    fn a_crlf_copy_of_the_line_is_still_the_same_line() {
        let doc = "# T\r\n\r\n## Relations\r\n\r\n**Implements:** [[dst]]\r\n";
        assert_eq!(splice_relation(doc, "Implements", "[[dst]]").unwrap(), None);
    }

    #[test]
    fn lifecycle_relations_reads_the_written_line_back() {
        let out = splice_relation(DOC, "Depends on", "[[dst-key]]")
            .unwrap()
            .unwrap();
        let read = relations(&out);
        assert_eq!(read.len(), 1, "{read:?}");
        assert_eq!(read[0].label, "Depends on");
        assert_eq!(read[0].reference, parse_target("dst-key"));
        let entry = splice_relation(&out, "Supersedes", "[[dst-key:F-3]]")
            .unwrap()
            .unwrap();
        let read = relations(&entry);
        assert_eq!(read.len(), 2, "{read:?}");
        assert_eq!(read[1].label, "Supersedes");
        assert_eq!(read[1].reference, parse_target("dst-key:F-3"));
    }

    /// One document holding both sections, written in turn: event, relation, event, relation. The
    /// second relation has the label `Mutates`, the same label as each event's own `**Mutates:**`
    /// line, and the second event mutates the same target, so the identical line stands once in
    /// each section. Each writer must keep its own heading single, put its text in its own section,
    /// and not take the other's line for its own: an idempotence check that looked outside
    /// `## Relations` would return `None` for the last splice, and an event splice that ended its
    /// section at the wrong heading would lose or misorder the second event.
    #[test]
    fn one_document_holds_events_and_relation_lines_in_turn() {
        use crate::librarian::catalog::event_text::{
            parse_event_sections, splice_event, EventText,
        };
        let event = |id: &str, mutates: &str| EventText {
            id: id.to_owned(),
            kind: "note".to_owned(),
            created_at_ms: 1_794_637_800_000,
            author: Some("alice".to_owned()),
            anchor_commit: None,
            head_commit: None,
            parent: None,
            resolves: None,
            source: None,
            mutates: vec![mutates.to_owned()],
            payload: serde_json::json!({ "text": format!("text of {id}") }),
        };
        let mutates = label_for_rel("mutates").unwrap();
        assert_eq!(mutates, "Mutates");

        let doc = splice_event(DOC, &event("ev-1", "[[a]]")).unwrap();
        let doc = splice_relation(&doc, "Implements", "[[x]]")
            .unwrap()
            .unwrap();
        let doc = splice_event(&doc, &event("ev-2", "[[y]]")).unwrap();
        let doc = splice_relation(&doc, &mutates, "[[y]]")
            .unwrap()
            .expect("the event's identical line is not the idempotent copy");

        assert_eq!(count_of(&doc, "\n## Events\n"), 1, "{doc}");
        assert_eq!(count_of(&doc, "\n## Relations\n"), 1, "{doc}");
        let events = parse_event_sections(&doc);
        assert_eq!(
            events.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            ["ev-1", "ev-2"],
            "{doc}"
        );
        assert_eq!(events[0].mutates, ["[[a]]"]);
        assert_eq!(events[1].mutates, ["[[y]]"]);

        // The reader takes every column-0 label line, so each event's `**Mutates:**` line is a
        // relation too, in document order before the `## Relations` lines.
        let read: Vec<(String, &str)> = relations(&doc)
            .into_iter()
            .map(|r| {
                (
                    r.label,
                    doc.lines().nth(r.line as usize - 1).unwrap_or_default(),
                )
            })
            .collect();
        assert_eq!(
            read,
            [
                ("Mutates".to_owned(), "**Mutates:** [[a]]"),
                ("Mutates".to_owned(), "**Mutates:** [[y]]"),
                ("Implements".to_owned(), "**Implements:** [[x]]"),
                ("Mutates".to_owned(), "**Mutates:** [[y]]"),
            ],
            "{doc}"
        );
        let relations_at = doc.find("\n## Relations\n").unwrap();
        assert!(
            doc.find("\n## Events\n").unwrap() < relations_at,
            "the events section was written first and stays first: {doc}"
        );
        assert!(
            doc[relations_at..].ends_with("**Implements:** [[x]]\n\n**Mutates:** [[y]]\n"),
            "both relation lines sit in the relations section, in call order: {doc}"
        );
        // A repeat of the last link changes nothing, although the event holds the same line.
        assert_eq!(splice_relation(&doc, &mutates, "[[y]]").unwrap(), None);
    }

    #[test]
    fn the_frontmatter_block_stays_byte_for_byte() {
        let doc = "---\r\ntitle: T\r\ntags: [a]\r\n---\nbody\n";
        let out = splice_relation(doc, "Implements", "[[dst]]")
            .unwrap()
            .unwrap();
        assert!(
            out.starts_with("---\r\ntitle: T\r\ntags: [a]\r\n---\n"),
            "{out}"
        );
    }

    #[test]
    fn a_document_with_no_body_gets_the_heading_and_the_line() {
        let out = splice_relation("---\ntitle: T\n---\n", "Implements", "[[dst]]")
            .unwrap()
            .unwrap();
        assert_eq!(relations(&out).len(), 1, "{out}");
        let bare = splice_relation("", "Implements", "[[dst]]")
            .unwrap()
            .unwrap();
        assert_eq!(bare, "## Relations\n\n**Implements:** [[dst]]\n");
    }

    #[test]
    fn a_document_with_an_unclosed_fence_refuses_a_line_that_would_not_read_back() {
        let doc = "# T\n\n```\nunclosed\n";
        let err = splice_relation(doc, "Implements", "[[dst]]").unwrap_err();
        let text = format!("{err:#}");
        assert!(text.contains("would not be readable back"), "{text}");
        assert!(text.contains("nothing was added"), "{text}");
    }

    #[test]
    fn a_form_that_could_forge_a_line_is_refused() {
        for form in ["[[a]]\n**Rests on:** [[b]]", "[[a]] [[b]]", "a", "[[a]", ""] {
            assert!(
                splice_relation(DOC, "Implements", form).is_err(),
                "{form:?} must be refused"
            );
        }
    }

    #[test]
    fn a_label_the_reader_would_not_take_is_refused() {
        // `lifecycle::relations` takes `^\*\*([A-Z][A-Za-z ]*[a-z]):\*\*`; a label that fails it
        // would leave a line that reads back as nothing.
        for label in ["x", "Depends2", "", "A\nB"] {
            assert!(
                splice_relation(DOC, label, "[[dst]]").is_err(),
                "{label:?} must be refused"
            );
        }
    }

    #[test]
    fn malformed_frontmatter_is_an_error_and_not_a_guess() {
        assert!(
            splice_relation("---\ntitle: [unclosed\n---\nbody\n", "Implements", "[[d]]").is_err()
        );
    }
}
