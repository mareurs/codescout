// src/librarian/tools/link_scan/extract.rs
//! Pure extraction stage: one pulldown_cmark walk per artifact body,
//! collecting entry-token DEFINITIONS (from headings) and CITATIONS
//! (entry tokens, 16-hex artifact ids, rel-path link targets, cross-repo
//! tokens). No catalog access — resolution happens in `resolve`.
//!
//! Exclusion rules (each guards a measured false-positive source):
//! - frontmatter (`ENABLE_YAML_STYLE_METADATA_BLOCKS` + MetadataBlock skip):
//!   without the option, `---` frontmatter parses as a setext heading and its
//!   `id:` 16-hex would self-cite every artifact;
//! - fenced/indented code blocks: templates and how-to examples are full of
//!   entry-token lookalikes (`F-NNN`, sample ids);
//! - inline code IS scanned — real citations live there ("id: `f2ec…efb`").

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    pub token: String,
    pub line: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CitationKind {
    EntryToken,
    ArtifactId,
    RelPathLink,
    CrossRepoToken,
}

impl CitationKind {
    /// The wire value for `kind` in `link_scan`'s finding arrays.
    ///
    /// Was `format!("{:?}", kind)`. `Debug` is a developer-facing rendering with no
    /// stability promise, so using it as an API meant a variant rename silently changed
    /// the output — the failure mode is a consumer's filter quietly matching nothing,
    /// which is the same shape as the `raw`/`token` split this accompanies
    /// (`docs/issues/archive/2026-08-17-link-scan-names-the-same-field-raw-in-dangling-and-token-in-ambiguous.md`).
    ///
    /// The strings are deliberately IDENTICAL to what `Debug` produced, so this is a
    /// fragility fix and not an output change — pinned by
    /// `citation_kind_wire_values_match_what_debug_emitted`.
    pub fn as_str(self) -> &'static str {
        match self {
            CitationKind::EntryToken => "EntryToken",
            CitationKind::ArtifactId => "ArtifactId",
            CitationKind::RelPathLink => "RelPathLink",
            CitationKind::CrossRepoToken => "CrossRepoToken",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Citation {
    pub raw: String,
    pub kind: CitationKind,
    pub line: u32,
}

#[derive(Debug, Default)]
pub struct DocExtract {
    pub definitions: Vec<Definition>,
    pub citations: Vec<Citation>,
    /// The id namespaces this artifact's frontmatter DECLARES via `entry_prefix`.
    ///
    /// Not a definition and not a citation — a third, orthogonal channel, and the only
    /// one that can tell a wholly-undefined ledger apart from an acronym in prose. It
    /// rides on `DocExtract` rather than being threaded through
    /// [`super::resolve::DefinitionIndex::build`]'s arguments precisely so no caller has
    /// to remember to pass it: `extract` already holds the whole file text, frontmatter
    /// included, so the wire cannot be forgotten.
    /// docs/issues/archive/2026-08-18-link-scan-dangling-count-is-prefix-gated-so-a-whole-namespace-reads-as-healthy.md
    pub declared_prefixes: Vec<String>,
}

/// Entry token: `A-11`, `F-3`, `T-007`, `WIN-4`, `BUG-40`. Width-agnostic on
/// both sides; `\b` means suffixed sub-entries like `F-6a` deliberately do
/// NOT match (digit→letter is not a word boundary).
fn entry_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b[A-Z]{1,3}-\d+\b").unwrap())
}

/// Artifact id: sha256(abs_path)[..16] — exactly 16 lowercase hex chars.
/// `\b` on both sides rejects substrings of longer hex runs (40-hex git shas).
fn id_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b[0-9a-f]{16}\b").unwrap())
}

/// Cross-repo citation: `codescout:A-11`, `prompt-engineering:L-14`.
/// Recognized so the entry-token scan can skip the embedded token; edges
/// cannot span workspaces, so these become report-only findings.
fn cross_repo_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b[a-z][a-z0-9_-]{1,30}:[A-Z]{1,3}-\d+\b").unwrap())
}

/// Definition shape: heading's first text starts with an entry token followed
/// by a whitespace-delimited dash separator (`A-11 — title`). `A-9 Addendum`
/// (no dash) is a section ABOUT A-9, not a re-definition.
fn def_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s*([A-Z]{1,3}-\d+)\s+[—–-]\s+").unwrap())
}

pub fn extract(text: &str) -> DocExtract {
    // `ENABLE_TABLES` only — NOT `ENABLE_YAML_STYLE_METADATA_BLOCKS`. That option
    // pairs ANY two bare `---` lines anywhere in the document as a YAML metadata
    // block, not just a leading frontmatter block. Session-log trackers use a
    // bare `---` as an inter-entry separator, so it silently swallowed every
    // other entry's heading + body as "metadata" (never extracted as a
    // Definition or scanned for citations). Frontmatter is skipped instead via
    // an explicit byte-offset guard computed from the same delimiter-finding
    // logic `frontmatter::parse` already uses.
    let opts = Options::ENABLE_TABLES;
    let line_starts = line_starts(text);
    let frontmatter_end = match crate::librarian::frontmatter::parse(text) {
        Ok((_, body)) => text.len() - body.len(),
        Err(_) => 0,
    };

    // Frontmatter yields no definitions and no citations — the offset guard below drops
    // every event inside it. It does, however, DECLARE the artifact's id namespaces, and
    // that declaration is what lets the dangling gate tell a wholly-undefined ledger apart
    // from `UTF-8` in prose. Read with the guard's own parser rather than a second one:
    // `librarian_guard` compiles under `--no-default-features` where
    // `librarian::frontmatter` does not exist, and the two readers' agreement on every
    // YAML form is already pinned by `both_entry_prefix_readers_agree_on_every_yaml_form`.
    let mut out = DocExtract {
        declared_prefixes: crate::util::librarian_guard::declared_entry_prefixes(text),
        ..Default::default()
    };
    let mut seen_defs = std::collections::BTreeSet::new();
    let mut seen_cites = std::collections::BTreeSet::new();

    let mut in_code_block = false;
    let mut in_heading = false;
    let mut heading_first_inline = false;

    for (event, span) in Parser::new_ext(text, opts).into_offset_iter() {
        if span.start < frontmatter_end {
            continue;
        }
        let line = line_of(&line_starts, span.start);
        match event {
            Event::Start(Tag::CodeBlock(_)) => in_code_block = true,
            Event::End(TagEnd::CodeBlock) => in_code_block = false,
            Event::Start(Tag::Heading { .. }) => {
                in_heading = true;
                heading_first_inline = true;
            }
            Event::End(TagEnd::Heading(_)) => in_heading = false,
            Event::Start(Tag::Link { dest_url, .. }) if !in_code_block => {
                if let Some(target) = link_target(dest_url.as_ref()) {
                    push_citation(
                        &mut out,
                        &mut seen_cites,
                        target,
                        CitationKind::RelPathLink,
                        line,
                    );
                }
            }
            Event::Code(content) if !in_code_block => {
                // Inline code: real citations live here, but a code-first
                // heading (`### `A-9` …`) must not define.
                if in_heading && heading_first_inline {
                    heading_first_inline = false;
                }
                scan_tokens(&mut out, &mut seen_cites, content.as_ref(), line);
            }
            Event::Text(content) if !in_code_block => {
                let mut rest = content.as_ref();
                if in_heading && heading_first_inline {
                    heading_first_inline = false;
                    if let Some(m) = def_re().captures(rest) {
                        let token = m.get(1).unwrap().as_str().to_string();
                        if seen_defs.insert(token.clone()) {
                            out.definitions.push(Definition { token, line });
                        }
                        // The defining token is not a citation of itself;
                        // scan only the remainder of the heading text.
                        rest = &rest[m.get(0).unwrap().end()..];
                    }
                }
                scan_tokens(&mut out, &mut seen_cites, rest, line);
            }
            _ => {}
        }
    }
    out
}

/// Scan one text chunk for cross-repo tokens, entry tokens, and artifact ids.
/// Cross-repo matches are found first and their spans masked so the embedded
/// entry token is not double-reported.
fn scan_tokens(
    out: &mut DocExtract,
    seen: &mut std::collections::BTreeSet<(CitationKind, String)>,
    chunk: &str,
    line: u32,
) {
    let mut masked: Vec<(usize, usize)> = Vec::new();
    for m in cross_repo_re().find_iter(chunk) {
        masked.push((m.start(), m.end()));
        push_citation(
            out,
            seen,
            m.as_str().to_string(),
            CitationKind::CrossRepoToken,
            line,
        );
    }
    for m in entry_re().find_iter(chunk) {
        if masked.iter().any(|&(s, e)| m.start() >= s && m.end() <= e) {
            continue;
        }
        push_citation(
            out,
            seen,
            m.as_str().to_string(),
            CitationKind::EntryToken,
            line,
        );
    }
    for m in id_re().find_iter(chunk) {
        push_citation(
            out,
            seen,
            m.as_str().to_string(),
            CitationKind::ArtifactId,
            line,
        );
    }
}

fn push_citation(
    out: &mut DocExtract,
    seen: &mut std::collections::BTreeSet<(CitationKind, String)>,
    raw: String,
    kind: CitationKind,
    line: u32,
) {
    if seen.insert((kind, raw.clone())) {
        out.citations.push(Citation { raw, kind, line });
    }
}

/// A markdown link target that could name a catalog artifact: not an external
/// URI, not a pure fragment; fragment suffix stripped.
fn link_target(dest: &str) -> Option<String> {
    if dest.is_empty() || dest.starts_with('#') || has_uri_scheme(dest) {
        return None;
    }
    let stripped = dest.split('#').next().unwrap_or(dest);
    if stripped.is_empty() {
        return None;
    }
    // Only markdown files can be catalog artifacts.
    if !stripped.ends_with(".md") {
        return None;
    }
    Some(stripped.to_string())
}

fn has_uri_scheme(s: &str) -> bool {
    if let Some(colon) = s.find(':') {
        let scheme = &s[..colon];
        !scheme.is_empty()
            && scheme.chars().all(|c| c.is_ascii_alphabetic() || c == '-')
            && s[colon..].starts_with("://")
    } else {
        false
    }
}

/// Byte offsets of line starts — O(n) once per document, O(log n) per lookup
/// (the audit parser's per-event prefix rescan is O(n·events); don't copy it).
fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(
            text.bytes()
                .enumerate()
                .filter(|(_, b)| *b == b'\n')
                .map(|(i, _)| i + 1),
        )
        .collect()
}

fn line_of(starts: &[usize], offset: usize) -> u32 {
    starts.partition_point(|&s| s <= offset) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(ex: &DocExtract, kind: CitationKind) -> Vec<&str> {
        ex.citations
            .iter()
            .filter(|c| c.kind == kind)
            .map(|c| c.raw.as_str())
            .collect()
    }

    #[test]
    fn frontmatter_yields_no_extractions() {
        let text = "---\nid: 59ebeebb6ed05c89\ntags: [A-11, F-3]\nkind: tracker\n---\n\nBody with no tokens.\n";
        let ex = extract(text);
        assert!(ex.definitions.is_empty(), "frontmatter must not define");
        assert!(
            ex.citations.is_empty(),
            "frontmatter must not cite: {:?}",
            ex.citations
        );
    }

    /// Frontmatter yields no definitions and no citations — but it does carry the
    /// `entry_prefix` DECLARATION, and that declaration is the only thing that can
    /// distinguish a wholly-undefined ledger from an acronym in prose.
    #[test]
    fn frontmatter_entry_prefix_is_read_as_a_declaration() {
        let ex = extract("---\nkind: tracker\nentry_prefix: WIN\n---\n\nBody, no tokens.\n");
        assert_eq!(ex.declared_prefixes, vec!["WIN".to_string()]);
        assert!(
            ex.definitions.is_empty(),
            "a declaration is not a definition: {:?}",
            ex.definitions
        );
    }

    /// The discriminating negative: a prefix that merely APPEARS in frontmatter is not a
    /// declaration. `tags: [A-11, F-3]` must stay silent, or the dangling gate would be
    /// widened by any tracker that happens to tag itself with an entry id. YAML *forms*
    /// of `entry_prefix` are covered where the parser lives, by
    /// `both_entry_prefix_readers_agree_on_every_yaml_form`.
    #[test]
    fn frontmatter_declares_nothing_without_an_entry_prefix_key() {
        let ex = extract("---\nid: 59ebeebb6ed05c89\ntags: [A-11, F-3]\n---\n\nBody.\n");
        assert!(
            ex.declared_prefixes.is_empty(),
            "{:?}",
            ex.declared_prefixes
        );
    }

    #[test]
    fn bare_dash_separator_between_entries_is_not_mistaken_for_yaml_metadata() {
        // Session-log trackers separate every entry with a lone `---` line (not
        // frontmatter). `ENABLE_YAML_STYLE_METADATA_BLOCKS` used to pair ANY two
        // bare `---` lines anywhere in the doc as a metadata block, silently
        // swallowing the entry in between — including its heading — as
        // unextracted "metadata".
        let text = "---\nid: abc123\nkind: tracker\n---\n\n## W-1 — First entry\n\n**Status:** validated\n\n---\n\n## W-2 — Second entry\n\n**Status:** validated\n\n---\n\n## W-3 — Third entry\n";
        let ex = extract(text);
        let tokens: Vec<&str> = ex.definitions.iter().map(|d| d.token.as_str()).collect();
        assert_eq!(tokens, vec!["W-1", "W-2", "W-3"]);
    }

    #[test]
    fn fenced_blocks_are_skipped_inline_code_is_scanned() {
        let text = "See `f2ecdd76a6189efb` for the exemplar.\n\n```\nF-3 and 59ebeebb6ed05c89 inside a fence\n```\n";
        let ex = extract(text);
        assert_eq!(
            tokens(&ex, CitationKind::ArtifactId),
            vec!["f2ecdd76a6189efb"]
        );
        assert!(tokens(&ex, CitationKind::EntryToken).is_empty());
    }

    #[test]
    fn heading_with_dash_defines_and_does_not_self_cite() {
        let text = "## A-11 — the shipped rule lacks a verdict\n\nProse mentioning F-8 here.\n";
        let ex = extract(text);
        assert_eq!(
            ex.definitions,
            vec![Definition {
                token: "A-11".into(),
                line: 1
            }]
        );
        // A-11 (its own definition) is not a citation; F-8 is.
        assert_eq!(tokens(&ex, CitationKind::EntryToken), vec!["F-8"]);
    }

    #[test]
    fn heading_can_define_one_token_and_cite_another() {
        let text = "## W-4 — Pre-fix recon caught wontfix bug (BUG-37 was already shipped)\n";
        let ex = extract(text);
        assert_eq!(ex.definitions.len(), 1);
        assert_eq!(ex.definitions[0].token, "W-4");
        assert_eq!(tokens(&ex, CitationKind::EntryToken), vec!["BUG-37"]);
    }

    #[test]
    fn heading_without_dash_separator_does_not_define() {
        let text = "### A-9 Addendum\n";
        let ex = extract(text);
        assert!(ex.definitions.is_empty());
        // It still CITES A-9 (a section about it).
        assert_eq!(tokens(&ex, CitationKind::EntryToken), vec!["A-9"]);
    }

    #[test]
    fn code_first_heading_does_not_define() {
        let text = "### `A-9` — looks like a definition but is code-first\n";
        let ex = extract(text);
        assert!(ex.definitions.is_empty());
        assert_eq!(tokens(&ex, CitationKind::EntryToken), vec!["A-9"]);
    }

    #[test]
    fn prose_acronyms_are_extracted_dumbly() {
        // Extraction is deliberately dumb; the prefix-gate in resolve
        // suppresses UTF-8 / SHA-256 style noise.
        let text = "Encode as UTF-8, hash with SHA-256.\n";
        let ex = extract(text);
        let mut got = tokens(&ex, CitationKind::EntryToken);
        got.sort();
        assert_eq!(got, vec!["SHA-256", "UTF-8"]);
    }

    #[test]
    fn cross_repo_token_masks_embedded_entry_token() {
        let text = "Promoted to codescout:A-11 last week.\n";
        let ex = extract(text);
        assert_eq!(
            tokens(&ex, CitationKind::CrossRepoToken),
            vec!["codescout:A-11"]
        );
        assert!(
            tokens(&ex, CitationKind::EntryToken).is_empty(),
            "embedded token must be masked"
        );
    }

    #[test]
    fn tables_and_blockquotes_are_scanned() {
        let text =
            "| ID | note |\n|---|---|\n| F-3 | see 59ebeebb6ed05c89 |\n\n> quoted W-2 here\n";
        let ex = extract(text);
        let mut entry = tokens(&ex, CitationKind::EntryToken);
        entry.sort();
        assert_eq!(entry, vec!["F-3", "W-2"]);
        assert_eq!(
            tokens(&ex, CitationKind::ArtifactId),
            vec!["59ebeebb6ed05c89"]
        );
    }

    #[test]
    fn link_targets_keep_md_strip_fragment_reject_external() {
        let text = "[log](docs/trackers/x-session-log.md#f-3) and [ext](https://example.com/a.md) and [anchor](#local)\n";
        let ex = extract(text);
        assert_eq!(
            tokens(&ex, CitationKind::RelPathLink),
            vec!["docs/trackers/x-session-log.md"]
        );
    }

    #[test]
    fn duplicate_definitions_and_citations_dedupe() {
        let text =
            "## BUG-40 — first\n\n## BUG-40 — second heading same token\n\nF-3 and F-3 again.\n";
        let ex = extract(text);
        assert_eq!(ex.definitions.len(), 1, "same-artifact dup defs collapse");
        assert_eq!(tokens(&ex, CitationKind::EntryToken), vec!["F-3"]);
    }

    #[test]
    fn hex_substrings_of_longer_runs_do_not_match() {
        // 40-hex git sha must not yield a 16-hex artifact-id citation.
        let text = "fixed in 0de733aa46ffad1060f26f72edd71624b8c25487 yesterday\n";
        let ex = extract(text);
        assert!(tokens(&ex, CitationKind::ArtifactId).is_empty());
    }

    #[test]
    fn suffixed_subentry_does_not_match_base_token() {
        // F-6a: digit→letter is not a word boundary, so no F-6 citation.
        let text = "see F-6a for the follow-up\n";
        let ex = extract(text);
        assert!(tokens(&ex, CitationKind::EntryToken).is_empty());
    }

    /// `as_str` replaced `format!("{:?}", kind)` as the wire rendering of `kind`. This
    /// asserts the swap changed nothing observable, for every variant — which is what
    /// makes it a fragility fix rather than a breaking output change.
    ///
    /// It also fails if a variant is renamed without updating `as_str`, which is exactly
    /// the silent-API-change that using `Debug` invited: the rename would flow straight
    /// into the JSON and a consumer's filter would quietly match nothing.
    #[test]
    fn citation_kind_wire_values_match_what_debug_emitted() {
        for kind in [
            CitationKind::EntryToken,
            CitationKind::ArtifactId,
            CitationKind::RelPathLink,
            CitationKind::CrossRepoToken,
        ] {
            assert_eq!(
                kind.as_str(),
                format!("{kind:?}"),
                "wire value drifted from the Debug rendering it replaced"
            );
        }
    }

    #[test]
    fn line_numbers_are_correct() {
        let text = "first line\n\n## A-2 — def on line three\n\nF-9 on line five\n";
        let ex = extract(text);
        assert_eq!(ex.definitions[0].line, 3);
        let f9 = ex
            .citations
            .iter()
            .find(|c| c.raw == "F-9")
            .expect("F-9 cited");
        assert_eq!(f9.line, 5);
    }
}
