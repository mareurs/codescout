//! The closed citation grammar: what in a document's text is a reference to another
//! document or to an entry, and nothing else.

use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd};
use regex::Regex;
use std::ops::Range;
use std::sync::OnceLock;

use super::key::Key;
use super::repo_name::RepoName;

/// What one citation points at, as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reference {
    /// `[[key]]` or `[[repo/key]]`: a whole document.
    File { repo: Option<RepoName>, key: Key },
    /// `[[key:TOKEN]]` or `[[repo/key:TOKEN]]`: one entry of a document.
    Entry {
        repo: Option<RepoName>,
        key: Key,
        token: String,
    },
    /// A bare `TOKEN`: resolves only inside its own document.
    Bare { token: String },
    /// A wiki link whose target is not in the grammar; `raw` is the trimmed target.
    Malformed { raw: String },
}

/// One citation and the 1-indexed line of its first byte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Citation {
    pub reference: Reference,
    pub line: u32,
}

/// An entry token: one to three ASCII capitals, a hyphen, digits (`F-3`, `R-91`).
fn is_token(s: &str) -> bool {
    match s.split_once('-') {
        Some((letters, digits)) => {
            (1..=3).contains(&letters.len())
                && letters.bytes().all(|b| b.is_ascii_uppercase())
                && !digits.is_empty()
                && digits.bytes().all(|b| b.is_ascii_digit())
        }
        None => false,
    }
}

/// Parse the target of a wiki link: `[repo/]key[:TOKEN]`, outer ASCII whitespace trimmed.
/// Every part is validated by its own type; anything else is `Malformed`.
pub fn parse_target(dest: &str) -> Reference {
    let raw = dest.trim_matches(|c: char| c.is_ascii_whitespace());
    parse_trimmed(raw).unwrap_or_else(|| Reference::Malformed {
        raw: raw.to_string(),
    })
}

fn parse_trimmed(raw: &str) -> Option<Reference> {
    let (doc, token) = match raw.split_once(':') {
        Some((_, rest)) if rest.contains(':') => return None,
        Some((doc, token)) if is_token(token) => (doc, Some(token)),
        Some(_) => return None,
        None => (raw, None),
    };
    let (repo, key) = match doc.split_once('/') {
        Some((_, rest)) if rest.contains('/') => return None,
        Some((repo, key)) => (Some(RepoName::parse(repo).ok()?), key),
        None => (None, doc),
    };
    let key = Key::parse(key).ok()?;
    Some(match token {
        Some(token) => Reference::Entry {
            repo,
            key,
            token: token.to_string(),
        },
        None => Reference::File { repo, key },
    })
}

/// Every citation in `text`, in source order. Never citations: YAML frontmatter, code (fenced,
/// indented, inline), wiki-link labels and embeds, `<...>` autolinks, raw HTML (an HTML block
/// arrives as `Event::Html`, so `<details>R-91 [[a-doc]]</details>` yields nothing, as in
/// CommonMark), and bare tokens in headings.
///
/// Frontmatter is cut off by `frontmatter::parse` and never handed to the Markdown parser, so
/// nothing inside it (a fence in a block scalar, say) can leak into the body. When that parse
/// fails (a BOM before the opening `---`, malformed YAML, a closing `--- ` with trailing
/// spaces) the whole text is scanned as body, exactly as `link_scan::extract` does.
///
/// Inside a table cell the wiki-link label separator must be escaped (`[[key\|label]]`):
/// GFM splits the cell at an unescaped `|` before the wiki link is seen, so `[[key|label]]`
/// there is not a link at all. The one trailing `\` the escape leaves on the target is stripped.
///
/// `[[x]](url)` is a Markdown link whose text is `[x]`, not a citation: a `(` right after the
/// closing `]]` takes the wiki link out of the citations. With a space between them (`[[x]] (y)`),
/// or a `[` after it, the wiki link is still a citation.
pub fn extract(text: &str) -> Vec<Citation> {
    extract_spanned(text)
        .into_iter()
        .map(|s| s.citation)
        .collect()
}

/// One citation and the bytes of the text it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpannedCitation {
    pub citation: Citation,
    /// Bytes of the text handed to [`extract_spanned`]: the whole `[[...]]` for a wiki link, the
    /// token for a bare token.
    pub range: Range<usize>,
}

/// [`extract`] with the byte range of each citation in `text`; `extract` is this mapped to
/// `.citation`, so the two cannot disagree.
///
/// A wiki link's range is the whole `[[...]]` (the range of its link event). A bare token's range
/// is the token. Both are slices of the original bytes: a bare token has a range only when its
/// text event equals its source slice (`bare_tokens`), so nothing that spans a backslash escape
/// or an entity gets one. A token inside a bare URL keeps its citation and its range, because the
/// scanner reads it (`a_token_in_a_bare_url_is_scanned`); a rewrite must test it with
/// [`in_url_word`].
///
/// A hit or bare token inside an inline link's text lies inside that link's `RelPathLink` range
/// (`link_scan::extract::citation_spans`); a rewrite of the link must drop it. Here that is a bare
/// token in `[see F-1](b.md)`: the grammar does not treat ordinary link text as opaque, so `F-1`
/// has an exact range inside the link.
pub fn extract_spanned(text: &str) -> Vec<SpannedCitation> {
    // A suffix start of `text` (`len - body.len()`), so a char boundary.
    let base = frontmatter_end(text);
    let starts = line_starts(text);
    let cite = |reference, range: Range<usize>| SpannedCitation {
        citation: Citation {
            reference,
            line: starts.partition_point(|&s| s <= range.start) as u32,
        },
        range,
    };

    let mut out = Vec::new();
    let (mut in_code, mut in_heading, mut in_cell) = (false, false, false);
    // One flag per open link/image: true when its text is not prose (wiki label, URL, embed).
    let mut opaque: Vec<bool> = Vec::new();
    let opts = Options::ENABLE_WIKILINKS | Options::ENABLE_TABLES;
    for (event, range) in Parser::new_ext(&text[base..], opts).into_offset_iter() {
        // Offsets are relative to the body; every offset below is absolute in `text`.
        let range = range.start + base..range.end + base;
        match event {
            Event::Start(Tag::CodeBlock(_)) => in_code = true,
            Event::End(TagEnd::CodeBlock) => in_code = false,
            Event::Start(Tag::Heading { .. }) => in_heading = true,
            Event::End(TagEnd::Heading(_)) => in_heading = false,
            Event::Start(Tag::TableCell) => in_cell = true,
            Event::End(TagEnd::TableCell) => in_cell = false,
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                ..
            }) => {
                let wiki = matches!(link_type, LinkType::WikiLink { .. });
                // CommonMark reads `[[x]](url)` as a link whose text is `[x]`; the wiki-link
                // extension takes the brackets first. A `(` straight after the closing `]]`
                // makes it that link, so it is no citation. It stays opaque (its text is not
                // prose). `get` keeps the check off a non-boundary offset.
                let is_markdown_link_text =
                    wiki && text.get(range.end..).is_some_and(|s| s.starts_with('('));
                if wiki && !is_markdown_link_text {
                    let dest: &str = &dest_url;
                    let dest = if in_cell {
                        dest.strip_suffix('\\').unwrap_or(dest)
                    } else {
                        dest
                    };
                    out.push(cite(parse_target(dest), range.clone()));
                }
                opaque.push(wiki || matches!(link_type, LinkType::Autolink | LinkType::Email));
            }
            // An embed `![[x]]` is an image: not a citation, and its alt text is not prose.
            Event::Start(Tag::Image { .. }) => opaque.push(true),
            Event::End(TagEnd::Link | TagEnd::Image) => {
                opaque.pop();
            }
            Event::Text(s) if !in_code && !in_heading && !opaque.contains(&true) => {
                for (offset, token) in bare_tokens(text, &s, &range) {
                    let span = offset..offset + token.len();
                    out.push(cite(Reference::Bare { token }, span));
                }
            }
            _ => {}
        }
    }
    out
}

/// True when the whitespace-delimited word around `range` holds `://`: the range is part of a
/// URL (or sits against one), and a rewrite must leave it alone. A range that is not on char
/// boundaries of `text` counts as in a URL, so the answer errs towards no rewrite.
///
/// It recognises `://` only, which is the plan's definition of a bare URL (D-R15). So `www.x.org/R-91`
/// and the `mailto:` and `doi:` forms are NOT flagged. A link whose destination is a URL
/// (`[see F-1](https://h/)`) IS flagged, because the destination sits in the same word as the
/// token; that false positive only produces a report line, never a rewrite.
#[allow(dead_code)] // read by the cutover citation rewrite (Plan D task 4); tests read it now
pub(crate) fn in_url_word(text: &str, range: &Range<usize>) -> bool {
    let (Some(before), Some(slice), Some(after)) = (
        text.get(..range.start),
        text.get(range.clone()),
        text.get(range.end..),
    ) else {
        return true;
    };
    let head = before.rsplit(char::is_whitespace).next().unwrap_or("");
    let tail = after.split(char::is_whitespace).next().unwrap_or("");
    [head, slice, tail].concat().contains("://")
}

/// Byte length of the leading YAML frontmatter block, as `link_scan::extract` computes it.
fn frontmatter_end(text: &str) -> usize {
    match crate::librarian::frontmatter::parse(text) {
        Ok((_, body)) => text.len() - body.len(),
        Err(_) => 0,
    }
}

/// Byte offset of each line start; a lookup is a binary search.
fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(text.match_indices('\n').map(|(i, _)| i + 1))
        .collect()
}

fn token_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("[A-Z]{1,3}-[0-9]+").expect("static regex"))
}

/// The bare tokens of one text event, each with its absolute byte offset in `text`.
///
/// The event string equals its source slice for plain text. Only an entity (`&amp;` arrives as
/// Text `&` over five source bytes) differs from its slice; such an event is skipped, as it cannot
/// hold a token the source spells. A backslash escape does not differ: in pulldown-cmark 0.13.3
/// `R\-91` arrives as Text `R` (0..1) and Text `-91 ...` (2..), so no token spans it. Word
/// boundaries are judged on the source around the match, because the parser splits one run of
/// prose into several events at escapes, entities and emphasis markers.
fn bare_tokens(text: &str, event_text: &str, range: &Range<usize>) -> Vec<(usize, String)> {
    // Defensive under pulldown-cmark 0.13.3: no input found where a text event differs from its
    // source slice; re-check on a parser upgrade.
    if text.get(range.clone()) != Some(event_text) {
        return Vec::new();
    }
    token_re()
        .find_iter(event_text)
        .filter_map(|m| {
            let (start, end) = (range.start + m.start(), range.start + m.end());
            let before = text[..start].chars().next_back();
            let after = text[end..].chars().next();
            (!before.is_some_and(is_word_char) && !after.is_some_and(is_word_char))
                .then(|| (start, m.as_str().to_string()))
        })
        .collect()
}

/// ASCII letters, digits and `_`, plus any non-ASCII alphanumeric (so `R-91é` is not a token).
fn is_word_char(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refs(text: &str) -> Vec<Reference> {
        extract(text).into_iter().map(|c| c.reference).collect()
    }
    fn k(s: &str) -> Key {
        Key::parse(s).unwrap()
    }
    fn r(s: &str) -> RepoName {
        RepoName::parse(s).unwrap()
    }
    fn file(key: &str) -> Reference {
        Reference::File {
            repo: None,
            key: k(key),
        }
    }
    fn bare(token: &str) -> Reference {
        Reference::Bare {
            token: token.into(),
        }
    }
    fn malformed(raw: &str) -> Reference {
        Reference::Malformed { raw: raw.into() }
    }

    #[test]
    fn the_four_forms_and_a_label() {
        assert_eq!(
            refs("[[a-doc]] [[a-doc:F-3]] [[other/b-doc]] [[other/b-doc:R-91|see this]]"),
            vec![
                Reference::File {
                    repo: None,
                    key: k("a-doc")
                },
                Reference::Entry {
                    repo: None,
                    key: k("a-doc"),
                    token: "F-3".into()
                },
                Reference::File {
                    repo: Some(r("other")),
                    key: k("b-doc")
                },
                Reference::Entry {
                    repo: Some(r("other")),
                    key: k("b-doc"),
                    token: "R-91".into()
                },
            ]
        );
    }

    #[test]
    fn malformed_targets_are_reported_not_guessed() {
        for raw in [
            "arXiv",
            "two words",
            "a:b:c",
            "a/b/c",
            "a:f-3",
            "deadbeefcafe1234",
            "",
        ] {
            assert_eq!(
                parse_target(raw),
                Reference::Malformed { raw: raw.into() },
                "{raw}"
            );
        }
        assert_eq!(
            parse_target(" tracker-hygiene-log "),
            Reference::File {
                repo: None,
                key: k("tracker-hygiene-log")
            }
        );
    }

    #[test]
    fn code_frontmatter_headings_and_labels_are_never_citations() {
        let text = "---\nrests_on: \"[[x-doc]]\"\n---\n# Heading F-1 and [[h-doc]]\n\nInline `[[c-doc]]` and `F-2`.\n\n```toml\n[[rule]]\nF-3\n```\n\n    [[indented-doc]] F-4\n\n[[l-doc|label F-5]] and bare F-6.\n";
        assert_eq!(
            refs(text),
            vec![
                Reference::File {
                    repo: None,
                    key: k("h-doc")
                },
                Reference::File {
                    repo: None,
                    key: k("l-doc")
                },
                Reference::Bare {
                    token: "F-6".into()
                },
            ]
        );
    }

    #[test]
    fn bare_tokens_need_word_boundaries_and_lines_are_one_indexed() {
        let cs = extract("intro\n\nsee R-91, not XR-91a or R-9x.\n");
        assert_eq!(cs.len(), 1);
        assert_eq!(
            cs[0].reference,
            Reference::Bare {
                token: "R-91".into()
            }
        );
        assert_eq!(cs[0].line, 3);
    }

    #[test]
    fn an_unclosed_fence_hides_what_follows_and_does_not_panic() {
        assert_eq!(
            refs("[[a-doc]]\n\n```\n[[b-doc]]\n"),
            vec![Reference::File {
                repo: None,
                key: k("a-doc")
            }]
        );
    }

    #[test]
    fn a_wiki_link_in_a_table_cell_is_found() {
        assert_eq!(
            refs("| a | [[t-doc:B-2]] |\n|---|---|\n| x | y |\n"),
            vec![Reference::Entry {
                repo: None,
                key: k("t-doc"),
                token: "B-2".into()
            }]
        );
    }

    // Ruling 1: a bare token's boundaries are judged against the source text around the
    // match, not inside one parser text event (the parser splits events at `_`, `*`, ...).
    #[test]
    fn a_bare_token_is_word_bounded_in_the_source_not_in_the_event() {
        assert_eq!(refs("foo_R-91"), vec![]);
        assert_eq!(refs("R-91_x"), vec![]);
        assert_eq!(refs("R-91é"), vec![]);
        assert_eq!(refs("éR-91"), vec![]);
        assert_eq!(refs("**R-91**"), vec![bare("R-91")]);
        assert_eq!(refs("R-91"), vec![bare("R-91")]);
        // An entity before the token is punctuation in the source (`;`), so the token stands.
        assert_eq!(refs("X&amp;R-91"), vec![bare("R-91")]);
    }

    // A `<...>` autolink (or email) is a URL, not prose: it never yields a bare token. A bare URL
    // in prose is plain text (pulldown-cmark has no bare-URL autolinks), so its tokens are scanned.
    #[test]
    fn an_autolink_url_is_not_scanned_for_tokens() {
        assert_eq!(
            refs("<https://example.com/R-91> then R-92"),
            vec![bare("R-92")]
        );
    }

    // Ruling 2: an embed `![[a-doc]]` is an image tag, not a citation; the grammar has no embed form.
    #[test]
    fn an_embed_is_not_a_citation() {
        assert_eq!(refs("![[a-doc]]"), vec![]);
        assert_eq!(
            refs("![[a-doc:F-3|alt F-4]] and [[b-doc]]"),
            vec![file("b-doc")]
        );
    }

    // Ruling 3: wherever the parser yields a wiki link tag, it is a citation.
    #[test]
    fn a_wiki_link_in_a_blockquote_a_list_item_and_a_link_text_is_found() {
        assert_eq!(
            refs("> quote [[q-doc]]\n\n- item [[i-doc]]\n\n[x [[n-doc]] y](https://example.com)\n"),
            vec![file("q-doc"), file("i-doc"), file("n-doc")]
        );
    }

    // Ruling 3: a backslash-escaped opener is text, not a wiki link.
    #[test]
    fn an_escaped_wiki_link_is_not_a_citation() {
        assert_eq!(refs("\\[[a-doc]]"), vec![]);
    }

    // Review focus 3: outer whitespace is trimmed, inner whitespace is malformed.
    #[test]
    fn whitespace_in_a_wiki_link_is_trimmed_outside_and_malformed_inside() {
        assert_eq!(
            refs("[[ tracker-hygiene-log ]]"),
            vec![file("tracker-hygiene-log")]
        );
        assert_eq!(refs("[[two words]]"), vec![malformed("two words")]);
    }

    // Ruling 4: an empty target is malformed with the trimmed target as raw (via parse_target);
    // an empty label after a valid target is still a file citation. pulldown-cmark yields no
    // link at all for `[[]]` or `[[|label]]`, so the extractor reports nothing there (pinned).
    #[test]
    fn empty_targets_are_malformed_and_an_empty_label_is_not() {
        assert_eq!(parse_target("  "), malformed(""));
        assert_eq!(refs("[[]]"), vec![]);
        assert_eq!(refs("[[|label]]"), vec![]);
        assert_eq!(refs("[[a-doc|]]"), vec![file("a-doc")]);
    }

    // Ruling 5: the line is the 1-indexed line of the citation's first byte.
    #[test]
    fn lines_count_from_one_across_crlf_and_frontmatter() {
        assert_eq!(extract("[[a-doc]]")[0].line, 1);
        let crlf = extract("one\r\ntwo\r\n\r\nsee [[a-doc]] and R-4\r\n");
        assert_eq!(crlf.iter().map(|c| c.line).collect::<Vec<_>>(), vec![4, 4]);
        let fm = extract("---\ntitle: x\nrests_on: \"[[x-doc]]\"\n---\nbody\n\n[[a-doc]]\n");
        assert_eq!(fm.len(), 1);
        assert_eq!(fm[0].line, 7);
    }

    // Ruling 6: a token defined by a heading is skipped there; table cells, list items and
    // blockquote paragraphs are scanned.
    #[test]
    fn a_token_in_a_heading_is_a_definition_but_elsewhere_it_is_a_citation() {
        let text = "## F-1 — title\n\n> quote F-2\n\n- item F-3\n\n| a |\n|---|\n| F-4 |\n";
        assert_eq!(refs(text), vec![bare("F-2"), bare("F-3"), bare("F-4")]);
    }

    // Ruling 7: byte order, and the same target twice yields two citations.
    #[test]
    fn citations_come_in_source_order_and_repeats_are_kept() {
        assert_eq!(
            refs("R-1 [[a-doc]] [[a-doc]] R-2"),
            vec![bare("R-1"), file("a-doc"), file("a-doc"), bare("R-2")]
        );
    }
    // Table cells: GFM splits a cell at an unescaped `|`, so `[[t-doc|lbl]]` is not a link there
    // (authors must write `\|`); the escaped form arrives with one trailing `\` on the target.
    #[test]
    fn a_labelled_wiki_link_in_a_table_cell_needs_an_escaped_pipe() {
        assert_eq!(
            refs("| a | [[t-doc\\|lbl]] |\n|---|---|\n| x | y |\n"),
            vec![file("t-doc")]
        );
        assert_eq!(
            refs("| a | [[t-doc:B-2\\|lbl]] |\n|---|---|\n| x | y |\n"),
            vec![Reference::Entry {
                repo: None,
                key: k("t-doc"),
                token: "B-2".into()
            }]
        );
        assert_eq!(
            refs("| a | b |\n|---|---|\n| x | [[t-doc|lbl]] |\n"),
            vec![]
        );
    }

    // Outside a table cell a trailing backslash on the target is not an escape: it stays malformed.
    // (`[[t-doc\]]` is no link at all, `\]` being an escape; the pothole form is the live case.)
    #[test]
    fn a_trailing_backslash_outside_a_table_stays_malformed() {
        assert_eq!(refs("[[t-doc\\|lbl]]"), vec![malformed("t-doc\\")]);
    }

    // The frontmatter is never handed to the parser: an indented fence inside a YAML block scalar
    // must not open a code block that swallows the closing `---` and the whole body.
    #[test]
    fn a_fence_inside_a_frontmatter_block_scalar_does_not_swallow_the_body() {
        let cs = extract("---\ndescription: |\n  ```rust\n  x\n---\nbody R-1 [[a-doc]]\n");
        assert_eq!(
            cs.iter()
                .map(|c| (c.reference.clone(), c.line))
                .collect::<Vec<_>>(),
            vec![(bare("R-1"), 6), (file("a-doc"), 6)]
        );
    }

    // When frontmatter::parse fails (here a BOM before the opening `---`) the whole text is scanned
    // as body: the same as link_scan::extract. This pins that behaviour, it does not endorse it.
    #[test]
    fn a_bom_before_the_frontmatter_makes_it_body_as_in_link_scan() {
        assert_eq!(
            refs("\u{feff}---\nrests_on: \"[[x-doc]]\"\n---\n"),
            vec![file("x-doc")]
        );
    }

    // An HTML block arrives as Event::Html, not Text: nothing in it is a citation (CommonMark).
    #[test]
    fn an_html_block_is_not_scanned() {
        assert_eq!(refs("<details>\nR-91 [[a-doc]]\n</details>\n"), vec![]);
    }

    // A bare URL in prose is text (no bare-URL autolinks), so its token is a bare token.
    #[test]
    fn a_token_in_a_bare_url_is_scanned() {
        assert_eq!(refs("see https://x/issues/R-91 now"), vec![bare("R-91")]);
    }

    // Malformed keeps the trimmed target; repo and token position are validated part by part.
    #[test]
    fn parse_target_edge_shapes() {
        assert_eq!(parse_target(" two words "), malformed("two words"));
        assert_eq!(
            parse_target("a/b:C-1"),
            Reference::Entry {
                repo: Some(r("a")),
                key: k("b"),
                token: "C-1".into()
            }
        );
        for raw in ["a:C-1/b", "/a", "a/", ":F-1", "deadbeefcafe1234/b"] {
            assert_eq!(parse_target(raw), malformed(raw), "{raw}");
        }
    }

    // Token shape edges: ASCII digits only, up to three capitals, and a hyphen continuation is a
    // boundary (as `\b` would be).
    #[test]
    fn token_shape_edges() {
        assert_eq!(refs("R-\u{669}\u{661}"), vec![]);
        assert_eq!(refs("R-91-2"), vec![bare("R-91")]);
        assert_eq!(refs("ABCD-1"), vec![]);
    }

    // CommonMark reads `[[x]](url)` as an ordinary link whose text is `[x]`; the wiki-link
    // extension takes the brackets first, so a `(` right after the closing `]]` means the wiki
    // link is link text, not a citation. A space, or a following `[`, does not change that.
    #[test]
    fn a_wiki_link_followed_by_a_paren_is_a_markdown_link_not_a_citation() {
        assert_eq!(refs("[[a-doc]](https://x)"), vec![]);
        assert_eq!(refs("[[a-doc|lbl]](x)"), vec![]);
        assert_eq!(refs("[[a-doc]] (x)"), vec![file("a-doc")]);
        // The label of such a link is still not prose: no bare token comes out of it.
        assert_eq!(refs("[[arXiv]](https://x/y)"), vec![]);
        assert_eq!(refs("[[R-91]](x)"), vec![]);
        // Reference-style: no rule for a following `[`.
        assert_eq!(
            refs("[[a-doc]][[b-doc]]"),
            vec![file("a-doc"), file("b-doc")]
        );
        // A citation that follows on the same line is still found.
        assert_eq!(refs("[[a-doc]](x) and [[b-doc]]"), vec![file("b-doc")]);
    }

    #[test]
    fn a_wiki_link_followed_by_a_paren_in_a_table_cell_is_not_a_citation() {
        assert_eq!(
            refs("| a | [[a-doc]](https://x) |\n|---|---|\n| x | y |\n"),
            vec![]
        );
        assert_eq!(
            refs("| a | [[a-doc\\|lbl]](x) |\n|---|---|\n| x | y |\n"),
            vec![]
        );
        assert_eq!(
            refs("| a | [[a-doc]] (x) |\n|---|---|\n| x | y |\n"),
            vec![file("a-doc")]
        );
    }
    /// Every text the module's own tests feed `extract`, kept in one place so the span variant
    /// is held to the same inputs.
    const FIXTURE_TEXTS: &[&str] = &[
        "[[a-doc]] [[a-doc:F-3]] [[other/b-doc]] [[other/b-doc:R-91|see this]]",
        "---\nrests_on: \"[[x-doc]]\"\n---\n# Heading F-1 and [[h-doc]]\n\nInline `[[c-doc]]` and `F-2`.\n\n```toml\n[[rule]]\nF-3\n```\n\n    [[indented-doc]] F-4\n\n[[l-doc|label F-5]] and bare F-6.\n",
        "intro\n\nsee R-91, not XR-91a or R-9x.\n",
        "[[a-doc]]\n\n```\n[[b-doc]]\n",
        "| a | [[t-doc:B-2]] |\n|---|---|\n| x | y |\n",
        "foo_R-91 R-91_x R-91é éR-91 **R-91** R-91 X&amp;R-91",
        "<https://example.com/R-91> then R-92",
        "![[a-doc:F-3|alt F-4]] and [[b-doc]]",
        "> quote [[q-doc]]\n\n- item [[i-doc]]\n\n[x [[n-doc]] y](https://example.com)\n",
        "\\[[a-doc]] [[ tracker-hygiene-log ]] [[two words]] [[]] [[a-doc|]]",
        "one\r\ntwo\r\n\r\nsee [[a-doc]] and R-4\r\n",
        "## F-1 — title\n\n> quote F-2\n\n- item F-3\n\n| a |\n|---|\n| F-4 |\n",
        "R-1 [[a-doc]] [[a-doc]] R-2",
        "| a | [[t-doc\\|lbl]] |\n|---|---|\n| x | [[t-doc:B-2\\|lbl]] |\n",
        "[[x-doc]](https://e.com) and [[y-doc]] (z) and [[w-doc]][v]",
        "<details>R-91 [[a-doc]]</details>\n\nsee https://host/runs/R-91 and R-92",
        "R\\-91 and F\\-1 and &amp; F-7 and é [[a-doc]] ü R-3",
    ];

    /// The citations of `text` with the exact source slice each one points at.
    fn slices(text: &str) -> Vec<(Reference, &str)> {
        extract_spanned(text)
            .into_iter()
            .map(|s| (s.citation.reference, &text[s.range]))
            .collect()
    }

    #[test]
    fn extract_is_extract_spanned_without_ranges() {
        let mut total = 0;
        for text in FIXTURE_TEXTS {
            let spanned: Vec<Citation> = extract_spanned(text)
                .into_iter()
                .map(|s| s.citation)
                .collect();
            assert_eq!(extract(text), spanned, "{text:?}");
            total += spanned.len();
        }
        assert!(total >= 30, "the fixtures must hold citations: {total}");
    }

    #[test]
    fn a_wiki_link_span_covers_its_brackets() {
        assert_eq!(
            slices("x [[a-doc:F-1]] y"),
            vec![(
                Reference::Entry {
                    repo: None,
                    key: k("a-doc"),
                    token: "F-1".into()
                },
                "[[a-doc:F-1]]"
            )]
        );
        assert_eq!(
            slices("é [[b-doc|the label]] and [[ c-doc ]]"),
            vec![
                (file("b-doc"), "[[b-doc|the label]]"),
                (file("c-doc"), "[[ c-doc ]]"),
            ]
        );
        // In a table cell the escaped pipe is part of the span.
        assert_eq!(
            slices("| a | [[t-doc\\|lbl]] |\n|---|---|\n| x | y |\n"),
            vec![(file("t-doc"), "[[t-doc\\|lbl]]")]
        );
        // A malformed target keeps its brackets too.
        assert_eq!(
            slices("see [[two words]]"),
            vec![(malformed("two words"), "[[two words]]")]
        );
    }

    #[test]
    fn a_bare_token_span_is_its_token() {
        let text = "é see R-91, and **F-2**\n\n| a |\n|---|\n| B-3 |\n";
        assert_eq!(
            slices(text),
            vec![
                (bare("R-91"), "R-91"),
                (bare("F-2"), "F-2"),
                (bare("B-3"), "B-3"),
            ]
        );
    }

    #[test]
    fn a_span_starts_on_the_line_the_citation_reports() {
        let text = "one\n\n[[a-doc]] and\nR-4\n";
        for s in extract_spanned(text) {
            let line = text[..s.range.start].matches('\n').count() as u32 + 1;
            assert_eq!(s.citation.line, line, "{:?}", s.citation);
        }
        assert_eq!(extract_spanned(text).len(), 2);
    }

    #[test]
    fn an_escaped_run_has_no_span_and_a_neighbour_keeps_its_own() {
        // `R\-91`: Text `R` and Text `-91`, so no token and no span for either.
        assert_eq!(slices("R\\-91 and F\\-1"), vec![]);
        // An escaped opener is text, not a wiki link.
        assert_eq!(slices("\\[[a-doc]]"), vec![]);
        // A run after an escape is its own slice, so its span is exact.
        assert_eq!(
            slices("R\\-91 and F-7 [[a-doc]]"),
            vec![(bare("F-7"), "F-7"), (file("a-doc"), "[[a-doc]]")]
        );
    }

    #[test]
    fn an_entity_is_never_part_of_a_span() {
        // The entity's Text is `&` over five bytes: it is not its source slice, so it is skipped.
        assert_eq!(slices("&amp; R-91"), vec![(bare("R-91"), "R-91")]);
        assert_eq!(
            slices("&amp;R-91 and X&amp;R-92"),
            vec![(bare("R-91"), "R-91"), (bare("R-92"), "R-92")]
        );
    }

    #[test]
    fn frontmatter_code_html_and_labels_have_no_span() {
        let text = "---\nrests_on: \"[[x-doc]] F-9\"\n---\nInline `[[c-doc]]` and ``F-2 `x` [[d-doc]]``.\n\n```toml\n[[rule]]\nF-3\n```\n\n    [[indented-doc]] F-4\n\n<details>R-91 [[e-doc]]</details>\n\n[[l-doc|label F-5]] and ![[m-doc]] and <https://e.com/R-92>.\n";
        assert_eq!(slices(text), vec![(file("l-doc"), "[[l-doc|label F-5]]")]);
    }

    // The scanner reads no HTML comment and no link reference definition (the parser consumes a
    // definition and emits no event for it), so neither yields a span.
    #[test]
    fn an_html_comment_and_a_link_reference_definition_have_no_span() {
        assert_eq!(slices("<!--\n[[a-doc]] R-1\n-->\n\nafter"), vec![]);
        assert_eq!(slices("a <!-- [[a-doc]] R-1 --> b"), vec![]);
        assert_eq!(
            slices("[x]: https://e.com/R-91 \"F-2 [[a-doc]]\"\n\nbody"),
            vec![]
        );
    }

    // The scanner reads tokens in a bare URL (`a_token_in_a_bare_url_is_scanned`), so they keep
    // their citation and their span; `in_url_word` is the guard a rewrite applies to them.
    #[test]
    fn a_token_in_a_bare_url_has_a_span_that_in_url_word_rejects() {
        let text = "see https://host/runs/R-91 and R-92, (https://h.io/x?id=F-3) [[a-doc]]";
        let spans = extract_spanned(text);
        let inside: Vec<(&str, bool)> = spans
            .iter()
            .map(|s| (&text[s.range.clone()], in_url_word(text, &s.range)))
            .collect();
        assert_eq!(
            inside,
            vec![
                ("R-91", true),
                ("R-92", false),
                ("F-3", true),
                ("[[a-doc]]", false),
            ]
        );
    }

    #[test]
    fn every_span_slices_the_exact_original_bytes() {
        let mut seen = 0;
        for text in FIXTURE_TEXTS {
            for s in extract_spanned(text) {
                let slice = text.get(s.range.clone()).expect("a char boundary range");
                match &s.citation.reference {
                    Reference::Bare { token } => assert_eq!(slice, token, "{text:?}"),
                    _ => assert!(
                        slice.starts_with("[[") && slice.ends_with("]]"),
                        "{slice:?} in {text:?}"
                    ),
                }
                seen += 1;
            }
        }
        assert!(seen >= 30, "{seen}");
    }
}
