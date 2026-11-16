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
use crate::librarian::lifecycle::relations::{rel_of_label, relation_labels, relations};
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
/// Whether a link of `rel` is a `supersedes` link: any spelling the label reader takes as
/// `Supersedes` (`supersedes`, `SUPERSEDES`, `Supersedes`) writes the destination's `status:
/// superseded` and the row's canonical rel. One predicate for the write (`tools::link::call`) and
/// for the path the write names (`artifact::write_target_id`), so the two never disagree on which
/// file changed.
pub(crate) fn is_superseding(rel: &str) -> bool {
    label_for_rel(rel).is_ok_and(|label| rel_of_label(&label) == "supersedes")
}

/// `doc` with the line `**<label>:** <form>` added to its `## Relations` section, in text order:
/// before the first declaring line (a column-0 `**Label:**` line outside a code fence) whose text is
/// greater than the new line's, with one blank line after it; with no greater line, at the end of
/// the section after one blank line. The heading is created at the end of the body when absent.
/// `Ok(None)` when the identical line is already in that section (a repeated link adds nothing).
///
/// The text order makes a line that two branches add to the same document merge as one line, and
/// keeps a document's relation lines in one order whatever order the links were made in. Text is
/// compared with a trailing `\r` removed; a line put before a CRLF line ends in CRLF.
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
    let out = match first_greater_line(body, &line) {
        Some((at, eol)) => {
            let new_body = format!("{}{line}{eol}{eol}{}", &body[..at], &body[at..]);
            frontmatter::replace_body(doc, &new_body).unwrap_or(new_body)
        }
        None => splice_under_home(doc, body, RELATIONS_HEADING, &format!("{line}\n")),
    };
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

/// Where in `body` the relation line `line` goes by text order: the byte offset of the first
/// declaring line of the `## Relations` section, outside a code fence, whose text (trailing `\r`
/// removed) is greater than `line`, and the line ending to use there (`\r\n` when that line ends in
/// `\r`). `None` when no declaring line is greater: the line goes at the end of the section.
///
/// A declaring line is one the relation reader labels: `relation_labels` of the line alone is not
/// empty. The line ends the paragraph before it (a bold field does), so the paragraph above keeps
/// its labels whether it ends in a blank line or in a wrapped line.
fn first_greater_line(body: &str, line: &str) -> Option<(usize, &'static str)> {
    home_lines(body, RELATIONS_HEADING)
        .into_iter()
        .find(|l| {
            let text = l.trim_end_matches('\r');
            text > line && !relation_labels(text).is_empty()
        })
        .and_then(|l| {
            // `home_lines` yields slices of `body`, so a line's offset is its distance from `body`.
            // The check is defensive: a slice that did not lie in `body` (a change to `home_lines`)
            // gives no place, and the line goes at the end of the section as it always did.
            let at = (l.as_ptr() as usize).checked_sub(body.as_ptr() as usize)?;
            body.get(at..)?.starts_with(l).then_some(())?;
            Some((at, if l.ends_with('\r') { "\r\n" } else { "\n" }))
        })
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

    // mutation: always append.
    #[test]
    fn a_line_takes_its_sorted_place_and_leaves_the_next_heading_alone() {
        let doc = "# T\n\n## Relations\n\n**Rests on:** [[a]]\n\n## Later\n\ntext\n";
        let out = splice_relation(doc, "Implements", "[[b]]")
            .unwrap()
            .unwrap();
        assert_eq!(
            out,
            "# T\n\n## Relations\n\n**Implements:** [[b]]\n\n**Rests on:** [[a]]\n\n## Later\n\ntext\n"
        );
    }
    // This one is green with always-append, and stays as a guard on the other side.
    // mutation: insert before the first declaring line whatever its text.
    #[test]
    fn a_line_that_sorts_last_still_appends() {
        let doc = "# T\n\n## Relations\n\n**Rests on:** [[a]]\n\n## Later\n\ntext\n";
        let out = splice_relation(doc, "Supersedes", "[[b]]")
            .unwrap()
            .unwrap();
        assert_eq!(
            out,
            "# T\n\n## Relations\n\n**Rests on:** [[a]]\n\n**Supersedes:** [[b]]\n\n## Later\n\ntext\n"
        );
    }

    // mutation: the inserted line and its blank line end in `\n` whatever the greater line ends in.
    #[test]
    fn a_crlf_document_takes_the_line_before_the_greater_line_and_keeps_crlf() {
        let doc = "# T\r\n\r\n## Relations\r\n\r\n**Rests on:** [[a]]\r\n";
        let out = splice_relation(doc, "Implements", "[[b]]")
            .unwrap()
            .unwrap();
        assert_eq!(
            out,
            "# T\r\n\r\n## Relations\r\n\r\n**Implements:** [[b]]\r\n\r\n**Rests on:** [[a]]\r\n"
        );
        assert_eq!(
            splice_relation(&out, "Implements", "[[b]]").unwrap(),
            None,
            "the CRLF copy is the same line"
        );
    }

    // mutation: the scan for the first greater line does not skip a fence's lines.
    #[test]
    fn a_greater_line_inside_a_fence_is_not_an_insertion_point() {
        let doc = "# T\n\n## Relations\n\n**Alpha link:** [[a]]\n\n```\n**Zzz:** [[z]]\n```\n\n**Rests on:** [[r]]\n";
        let out = splice_relation(doc, "Implements", "[[b]]")
            .unwrap()
            .unwrap();
        assert_eq!(
            out,
            "# T\n\n## Relations\n\n**Alpha link:** [[a]]\n\n```\n**Zzz:** [[z]]\n```\n\n**Implements:** [[b]]\n\n**Rests on:** [[r]]\n",
            "the fenced `**Zzz:**` line is text, not a place"
        );
    }

    // The line before the insertion point is the last line of the previous paragraph, not a blank
    // line. The new line is a bold field, so the reader ends that paragraph at it: it gets its own
    // label and the previous paragraph keeps its own on every line, with no extra blank line.
    // mutation: insert before the continuation line, inside the previous paragraph.
    #[test]
    fn a_line_after_a_continuation_line_gets_its_own_label() {
        use crate::librarian::lifecycle::relations::relation_labels;
        let doc =
            "# T\n\n## Relations\n\n**Alpha link:** [[a]]\nwrapped [[w]]\n**Rests on:** [[r]]\n";
        let out = splice_relation(doc, "Implements", "[[b]]")
            .unwrap()
            .unwrap();
        let labels = relation_labels(&out);
        let got: Vec<(u32, &str)> = labels.iter().map(|(n, l)| (*n, l.as_str())).collect();
        assert_eq!(
            got,
            [
                (5, "Alpha link"),
                (6, "Alpha link"),
                (7, "Implements"),
                (9, "Rests on")
            ],
            "{out}"
        );
        let read = relations(&out);
        let implements: Vec<_> = read.iter().filter(|r| r.label == "Implements").collect();
        assert_eq!(implements.len(), 1, "{read:?}");
        assert_eq!(implements[0].reference, parse_target("b"));
        assert!(
            read.iter()
                .any(|r| r.label == "Alpha link" && r.reference == parse_target("w")),
            "the continuation line's citation stays the previous paragraph's: {read:?}"
        );
    }

    // mutation: append, which conflicts where the two branches both added the line.
    #[test]
    fn the_same_link_added_on_two_branches_merges_as_one_line() {
        use crate::librarian::derived::testkit::git;
        let base = "# T\n\n## Relations\n\n**Depends on:** [[a]]\n\n**Supersedes:** [[z]]\n";
        let ours = splice_relation(base, "Implements", "[[k]]")
            .unwrap()
            .unwrap();
        let ours = splice_relation(&ours, "Supersedes", "[[zz]]")
            .unwrap()
            .unwrap();
        let theirs = splice_relation(base, "Implements", "[[k]]")
            .unwrap()
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        for (name, text) in [
            ("ours", ours.as_str()),
            ("base", base),
            ("theirs", theirs.as_str()),
        ] {
            std::fs::write(dir.path().join(name), text).unwrap();
        }
        let merged = git(dir.path(), &["merge-file", "-p", "ours", "base", "theirs"]);
        assert_eq!(merged, ours, "the shared line merges as one line");
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
