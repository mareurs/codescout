//! Manual links as relation lines under `## Relations` of the source document (D-R19).
//!
//! A pure text step like [`events`](super::events): the line is written with the code of a live
//! `doc(link)` call (`link_text::splice_relation`), so both write one format.

use crate::librarian::catalog::link_text::{label_for_rel, splice_relation};
use crate::librarian::cutover::entries::item;
use crate::librarian::cutover::plan::{Class, Item, Status};

const REASON_NO_LABEL: &str = "the rel gives no relation label";
const REASON_NO_FORM: &str = "the target has no name in the key space";
const REASON_HELD: &str = "the text already holds it";
const REASON_WRITTEN: &str = "relation line written";

/// `text` with one relation line per manual link of the document, and one item per link (D-R19).
///
/// A link is `(rel, form)`: the old row's `rel` and the grammar form of its destination, `None` when
/// the grammar cannot name the destination. Lines go in `(rel, form)` order, after the lines the
/// text already holds, whatever the order of the input. A relation goes into any document (`doc(link)`
/// writes into any source), so no flag is read. A link is:
/// - `already_exported` when the identical line is already under `## Relations`;
/// - `unrecoverable` when its `rel` gives no label (a digit or another character), when its
///   destination has no form, or when `splice_relation` refuses (an unclosed code fence, malformed
///   frontmatter); the error text carries no path, and the text stays as it was;
/// - `moved` otherwise.
pub fn export_links(
    rel_path: &str,
    text: &str,
    links: &[(String, Option<String>)],
) -> (String, Vec<Item>) {
    let mut text = text.to_owned();
    let mut items = Vec::new();
    let mut sorted: Vec<&(String, Option<String>)> = links.iter().collect();
    sorted.sort();
    for (rel, form) in sorted {
        let report = |status, reason: &str| {
            item(
                rel_path,
                Class::ManualLink,
                status,
                reason,
                Some(rel.clone()),
            )
        };
        let label = match label_for_rel(rel) {
            Ok(label) => label,
            Err(why) => {
                let reason = format!("{REASON_NO_LABEL}: {why}");
                items.push(report(Status::Unrecoverable, &reason));
                continue;
            }
        };
        let Some(form) = form else {
            items.push(report(Status::Unrecoverable, REASON_NO_FORM));
            continue;
        };
        match splice_relation(&text, &label, form) {
            Ok(Some(next)) => {
                text = next;
                items.push(report(Status::Moved, REASON_WRITTEN));
            }
            Ok(None) => items.push(report(Status::AlreadyExported, REASON_HELD)),
            Err(err) => {
                let reason = format!("the line cannot be added to the text: {err:#}");
                items.push(report(Status::Unrecoverable, &reason));
            }
        }
    }
    (text, items)
}

/// One `unrecoverable` item per manual link of a document the plan cannot edit (`reason` is why:
/// not in `HEAD`, outside the key space, frozen).
pub fn refuse_links(rel_path: &str, links: &[(String, Option<String>)], reason: &str) -> Vec<Item> {
    let mut sorted: Vec<&(String, Option<String>)> = links.iter().collect();
    sorted.sort();
    sorted
        .into_iter()
        .map(|(rel, _)| {
            item(
                rel_path,
                Class::ManualLink,
                Status::Unrecoverable,
                reason,
                Some(rel.clone()),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::cutover::plan::{Class, Status};

    const REL: &str = "docs/doc.md";

    fn link(rel: &str, form: Option<&str>) -> (String, Option<String>) {
        (rel.to_owned(), form.map(str::to_owned))
    }

    fn only(items: &[Item]) -> &Item {
        assert_eq!(items.len(), 1, "{items:?}");
        &items[0]
    }

    #[test]
    fn a_manual_link_becomes_a_relation_line() {
        let (out, items) = export_links(REL, "# Doc\n", &[link("depends_on", Some("[[target]]"))]);
        assert_eq!(out, "# Doc\n\n## Relations\n\n**Depends on:** [[target]]\n");
        let item = only(&items);
        assert_eq!(item.class, Class::ManualLink);
        assert_eq!(item.status, Status::Moved, "{item:?}");
        assert_eq!(item.path.as_deref(), Some(REL));
    }

    #[test]
    fn a_link_whose_line_the_text_holds_is_already_exported() {
        let links = [link("supersedes", Some("[[old]]"))];
        let (once, _) = export_links(REL, "# Doc\n", &links);
        let (twice, items) = export_links(REL, &once, &links);
        assert_eq!(twice, once);
        let item = only(&items);
        assert_eq!(item.status, Status::AlreadyExported, "{item:?}");
        assert_eq!(item.class, Class::ManualLink);
    }

    #[test]
    fn a_rel_with_no_label_is_unrecoverable() {
        let (out, items) = export_links(REL, "# Doc\n", &[link("v2_depends", Some("[[t]]"))]);
        assert_eq!(out, "# Doc\n");
        let item = only(&items);
        assert_eq!(item.status, Status::Unrecoverable);
        assert_eq!(item.class, Class::ManualLink);
        assert!(
            item.reason.starts_with("the rel gives no relation label: "),
            "{}",
            item.reason
        );
    }

    #[test]
    fn a_target_with_no_form_is_unrecoverable() {
        let (out, items) = export_links(REL, "# Doc\n", &[link("depends_on", None)]);
        assert_eq!(out, "# Doc\n");
        let item = only(&items);
        assert_eq!(item.status, Status::Unrecoverable);
        assert_eq!(item.reason, "the target has no name in the key space");
    }

    #[test]
    fn a_link_on_an_unflagged_document_is_exported() {
        // A relation goes into any document (`doc(link)` writes into any source): no flag is read.
        for text in ["# Doc\n", "---\ntitle: x\n---\n\n# Doc\n"] {
            let (out, items) = export_links(REL, text, &[link("depends_on", Some("[[t]]"))]);
            assert!(out.contains("**Depends on:** [[t]]"), "{out}");
            assert_eq!(only(&items).status, Status::Moved);
        }
    }

    #[test]
    fn a_link_the_text_cannot_hold_is_unrecoverable_and_leaves_the_text() {
        let text = "# Doc\n\n```\nnever closed\n";
        let (out, items) = export_links(REL, text, &[link("depends_on", Some("[[t]]"))]);
        assert_eq!(out, text);
        let item = only(&items);
        assert_eq!(item.status, Status::Unrecoverable);
        assert!(!item.reason.contains('/'), "no path: {}", item.reason);
    }

    #[test]
    fn lines_go_in_a_fixed_order_whatever_the_input_order() {
        let a = link("depends_on", Some("[[b]]"));
        let b = link("depends_on", Some("[[a]]"));
        let c = link("supersedes", Some("[[a]]"));
        let (forward, _) = export_links(REL, "# Doc\n", &[a.clone(), b.clone(), c.clone()]);
        let (backward, _) = export_links(REL, "# Doc\n", &[c, b, a]);
        assert_eq!(forward, backward);
        assert_eq!(
            forward,
            "# Doc\n\n## Relations\n\n**Depends on:** [[a]]\n\n**Depends on:** [[b]]\n\n**Supersedes:** [[a]]\n"
        );
    }
}
