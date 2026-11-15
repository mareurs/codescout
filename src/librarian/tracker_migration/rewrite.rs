//! The citation rewrite of a tracker migration: one pure function over text.
//!
//! Spec: `docs/superpowers/specs/2026-11-15-tracker-migration-design.md` (the citation rewrite;
//! path mentions are lint, not rewritten).

use std::collections::BTreeSet;
use std::ops::Range;

use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd};
use serde::Serialize;

use crate::librarian::identity::grammar::{
    extract_spanned, in_url_word, Reference, SpannedCitation,
};

/// What one call changed and what it deliberately left.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RewriteCounts {
    pub rewritten: usize,
    pub skipped_link_text: usize,
    pub skipped_url: usize,
}

/// Byte ranges of the TEXT (not the destination) of every ordinary markdown link and image.
/// Wiki links are citations themselves and autolinks have no prose text, so both are left out.
fn link_text_ranges(text: &str) -> Vec<Range<usize>> {
    let opts = Options::ENABLE_WIKILINKS | Options::ENABLE_TABLES;
    // One open link per entry: the union of the ranges of its child events (None until a child).
    let mut open: Vec<Option<Range<usize>>> = Vec::new();
    let mut out = Vec::new();
    for (event, range) in Parser::new_ext(text, opts).into_offset_iter() {
        match event {
            Event::Start(Tag::Link { link_type, .. })
                if !matches!(
                    link_type,
                    LinkType::WikiLink { .. } | LinkType::Autolink | LinkType::Email
                ) =>
            {
                open.push(None);
            }
            Event::End(TagEnd::Link) => {
                if let Some(Some(r)) = open.pop() {
                    out.push(r);
                }
            }
            _ => {
                // A child event extends every open link's text range (nested links cannot occur).
                for slot in open.iter_mut() {
                    *slot = Some(match slot.take() {
                        Some(r) => r.start.min(range.start)..r.end.max(range.end),
                        None => range.clone(),
                    });
                }
            }
        }
    }
    out
}

/// Rewrite the citations of one tracker record (or, with an empty `defined`, of another file).
///
/// - A `Bare` token in `defined` becomes `[[new_key:TOKEN]]`, unless it lies in the text of an
///   ordinary link (`skipped_link_text`) or in a URL word (`skipped_url`).
/// - When `old_key != new_key`, an `Entry`/`File` citation with no repo and key `old_key` has
///   only its key bytes replaced; a label or anything else inside the brackets stays.
///
/// Code spans and fences need no rule: `extract_spanned` yields no citation inside them (plan
/// ruling P-R1). All edits are byte ranges applied from the end of the text to the start, so
/// the ranges not yet applied stay valid.
pub fn rewrite_record_citations(
    text: &str,
    defined: &BTreeSet<String>,
    old_key: &str,
    new_key: &str,
) -> (String, RewriteCounts) {
    let mut counts = RewriteCounts {
        rewritten: 0,
        skipped_link_text: 0,
        skipped_url: 0,
    };
    let links = link_text_ranges(text);
    // (byte range to replace, replacement)
    let mut edits: Vec<(Range<usize>, String)> = Vec::new();
    for SpannedCitation { citation, range } in extract_spanned(text) {
        match citation.reference {
            Reference::Bare { token } if defined.contains(&token) => {
                if links
                    .iter()
                    .any(|l| l.start <= range.start && range.end <= l.end)
                {
                    counts.skipped_link_text += 1;
                } else if in_url_word(text, &range) {
                    counts.skipped_url += 1;
                } else {
                    edits.push((range, format!("[[{new_key}:{token}]]")));
                    counts.rewritten += 1;
                }
            }
            Reference::Entry {
                repo: None, key, ..
            }
            | Reference::File { repo: None, key }
                if old_key != new_key && key.as_str() == old_key =>
            {
                // The key follows `[[` and any ASCII whitespace the grammar trims.
                let Some(raw) = text.get(range.clone()) else {
                    continue;
                };
                let inner = raw.strip_prefix("[[").unwrap_or(raw);
                let lead = raw.len() - inner.len() + (inner.len() - inner.trim_start().len());
                let start = range.start + lead;
                if text.get(start..start + old_key.len()) == Some(old_key) {
                    edits.push((start..start + old_key.len(), new_key.to_string()));
                    counts.rewritten += 1;
                }
            }
            _ => {}
        }
    }
    let mut out = text.to_string();
    edits.sort_by_key(|(r, _)| r.start);
    for (range, replacement) in edits.into_iter().rev() {
        out.replace_range(range, &replacement);
    }
    (out, counts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(tokens: &[&str]) -> BTreeSet<String> {
        tokens.iter().map(|t| t.to_string()).collect()
    }

    fn counts(rewritten: usize, skipped_link_text: usize, skipped_url: usize) -> RewriteCounts {
        RewriteCounts {
            rewritten,
            skipped_link_text,
            skipped_url,
        }
    }

    #[test]
    fn a_defined_bare_token_becomes_a_qualified_citation() {
        // Two tokens, so the second edit proves the first did not shift it; the leading
        // multi-byte text (9 bytes, 3 chars) proves offsets are bytes.
        let (out, c) =
            rewrite_record_citations("日本語 F-1 and F-22.", &set(&["F-1", "F-22"]), "k", "k");
        assert_eq!(out, "日本語 [[k:F-1]] and [[k:F-22]].");
        assert_eq!(c, counts(2, 0, 0));
    }

    #[test]
    fn an_undefined_bare_token_is_left() {
        let (out, c) = rewrite_record_citations("see F-1 and R-2", &set(&["F-1"]), "k", "k");
        assert_eq!(out, "see [[k:F-1]] and R-2");
        assert_eq!(c, counts(1, 0, 0));
    }

    #[test]
    fn tokens_in_code_link_text_and_urls_are_left() {
        // Code spans and fences yield no citation from the grammar (plan ruling P-R1), so they
        // need no rule here.
        let text = "`F-1` [F-1](b.md) http://x.org/F-1\n\n```\nF-1\n```\n";
        let (out, c) = rewrite_record_citations(text, &set(&["F-1"]), "k", "k");
        assert_eq!(out, text);
        assert_eq!(c, counts(0, 1, 1));
    }

    #[test]
    fn a_key_change_rewrites_old_key_citations_only() {
        let (out, c) = rewrite_record_citations(
            "[[old:R-1]] [[old]] [[other:R-1]] [[older]]",
            &set(&[]),
            "old",
            "new",
        );
        assert_eq!(out, "[[new:R-1]] [[new]] [[other:R-1]] [[older]]");
        assert_eq!(c, counts(2, 0, 0));
    }

    #[test]
    fn a_key_change_keeps_a_label() {
        // The grammar parses `[[key:TOKEN|label]]` (target is the part before `|`), so the
        // citation is found and only the key bytes change.
        let (out, c) = rewrite_record_citations("[[old:F-1|x]] [[old|y]]", &set(&[]), "old", "new");
        assert_eq!(out, "[[new:F-1|x]] [[new|y]]");
        assert_eq!(c, counts(2, 0, 0));
    }

    #[test]
    fn an_empty_defined_set_leaves_bare_tokens() {
        let (out, c) = rewrite_record_citations("F-1 and [[old:F-1]]", &set(&[]), "old", "new");
        assert_eq!(out, "F-1 and [[new:F-1]]");
        assert_eq!(c, counts(1, 0, 0));
    }

    #[test]
    fn an_equal_key_does_not_rewrite_key_citations() {
        let (out, c) = rewrite_record_citations("[[k:F-1]] F-1", &set(&["F-1"]), "k", "k");
        assert_eq!(out, "[[k:F-1]] [[k:F-1]]");
        assert_eq!(c, counts(1, 0, 0));
    }

    #[test]
    fn the_rewrite_is_idempotent() {
        let text = "日本語 F-1, [[old:R-1|x]], [F-1](b.md), http://x/F-1";
        let defined = set(&["F-1"]);
        let (once, _) = rewrite_record_citations(text, &defined, "old", "new");
        let (twice, c) = rewrite_record_citations(&once, &defined, "old", "new");
        assert_eq!(
            once,
            "日本語 [[new:F-1]], [[new:R-1|x]], [F-1](b.md), http://x/F-1"
        );
        assert_eq!(twice, once);
        assert_eq!(c.rewritten, 0);
    }
}
