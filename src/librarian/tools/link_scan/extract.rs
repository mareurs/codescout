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

/// One entry's section: its defining heading plus every line up to (not including)
/// the next heading of the SAME OR HIGHER level.
///
/// **Why the level bound and not "the next definition".** Measured 2026-08-20 over 12
/// ledgers: attributing a citation to the nearest preceding definition, without the
/// bound, is wrong on 12.1% of attributed citations — and the error is one mechanism,
/// the LAST entry in a file absorbing every citation in the trailing `## Summary` /
/// `## Template` sections. Four ledgers carried 109 of 123 errors. See
/// `docs/superpowers/specs/2026-08-20-entry-validity-and-attestation-design.md`
/// § Layer 3 → Attribution, and `scripts/probe_entry_attribution.py`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntrySection {
    pub id: String,
    pub level: usize,
    /// 1-indexed line of the defining heading.
    pub heading_line: u32,
    /// 1-indexed last line of the section, inclusive.
    pub end_line: u32,
    /// The section's text, heading line included.
    pub text: String,
}

/// Split a body into entry sections, sharing `def_re`, the frontmatter byte-offset
/// computation, and the fence-aware heading scan with [`extract`] — and with
/// [`crate::librarian::preview::headings::parse`], the same shared ATX heading
/// parser every other artifact-preview consumer already uses — so the three can
/// never disagree about what a definition, a fence, or frontmatter is.
///
/// Delegating fence-tracking to `headings::parse` (built on
/// [`crate::util::markdown_fence::FenceState`]) rather than a hand-rolled
/// `starts_with("```")` parity toggle is required, not incidental: a bare toggle
/// flips on ANY line starting with backtick/tilde characters, including a longer
/// backtick run nested inside an already-open fence whose info string itself
/// contains backticks — exactly the shape CommonMark rejects as a closer (a closer
/// may be followed only by whitespace; a backtick fence's info string may not
/// contain a backtick) but a bare toggle does not know to reject. That shape is not
/// hypothetical: it appears in this repo's own
/// `docs/trackers/bug-fix-session-log.md:2909` (a four-backtick run wrapping a
/// three-backtick block, the fixture behind
/// `docs/issues/archive/2026-08-11-artifact-nested-fence-closes-outer-fence.md`), and
/// a bare toggle desyncs there, silently dropping every definition for the rest of
/// the file.
pub fn entry_sections(text: &str) -> Vec<EntrySection> {
    // Frontmatter is excluded by LINE, not by re-deriving frontmatter rules from
    // scratch: reuse the exact byte-offset computation `extract()` uses (same
    // function, same `Err(_) => 0` fallback), then convert that offset to a line
    // number with the `line_starts`/`line_of` helpers `extract()` already shares.
    let frontmatter_end = match crate::librarian::frontmatter::parse(text) {
        Ok((_, body)) => text.len() - body.len(),
        Err(_) => 0,
    };
    let starts = line_starts(text);
    // `line_of(starts, frontmatter_end)` is the 1-indexed line the body starts on;
    // every heading strictly before it is inside frontmatter and does not count.
    let frontmatter_last_line = line_of(&starts, frontmatter_end).saturating_sub(1);

    let headings: Vec<crate::librarian::preview::headings::Heading> =
        crate::librarian::preview::headings::parse(text)
            .into_iter()
            .filter(|h| h.line as u32 > frontmatter_last_line)
            .collect();

    // `str::lines()`, not `text.split('\n')`: the latter phantoms a trailing empty
    // element for any text ending in '\n' — i.e. almost every real markdown file —
    // which overcounts `last` by one (a definition on the true last line reads as
    // ending one line past EOF) and would append a bogus empty final line to that
    // section's `text`. `lines()` has no such trailing artifact either way.
    let lines: Vec<&str> = text.lines().collect();
    let last = lines.len() as u32;

    let defs: Vec<(String, usize, u32)> = headings
        .iter()
        .filter_map(|h| {
            def_re()
                .captures(&h.text)
                .map(|c| (c[1].to_string(), h.level as usize, h.line as u32))
        })
        .collect();

    defs.into_iter()
        .map(|(id, level, heading_line)| {
            let end_line = headings
                .iter()
                .find(|h| h.line as u32 > heading_line && h.level as usize <= level)
                .map(|h| h.line as u32 - 1)
                .unwrap_or(last);
            let text = lines[(heading_line as usize - 1)..(end_line as usize)].join("\n");
            EntrySection {
                id,
                level,
                heading_line,
                end_line,
                text,
            }
        })
        .collect()
}

/// Which entry a line belongs to: the **innermost** section whose range contains it.
///
/// **Sections overlap by construction, so "innermost" is the rule, not a tie-break.**
/// [`entry_sections`] bounds a section at the next heading of the same or higher level,
/// so a `###` entry's section sits wholly *inside* its enclosing `##` entry's. A line in
/// the child is contained by both, and taking the first or the outermost match
/// attributes the child's citations to its parent.
///
/// That is the error [`entry_sections`]' own doc comment describes at the file level,
/// arriving one level down: the measured 12.1% mis-attribution came from a container
/// absorbing citations belonging to something more specific. Choosing the outermost here
/// re-creates it inside the section tree instead of across it.
///
/// Innermost is found by the greatest `heading_line`. Ranking by deepest `level` is
/// **equivalent, not worse**: [`entry_sections`] bounds a section at the next heading of
/// the same or higher level, so two sections at one level can never overlap, and the
/// sections containing a given line always form a chain of strictly increasing level.
/// Both keys therefore pick the same section on every input this module can produce —
/// verified by mutation, where swapping to `level` leaves the suite green.
///
/// `heading_line` is preferred because it does not *depend* on that invariant. If the
/// bounding rule is ever relaxed and two containing sections come to share a level,
/// position still separates them and depth no longer does.
///
/// Returns `None` for a line outside every entry — frontmatter, a preamble before the
/// first definition, or a trailing `## Summary` that defines nothing. That is a real
/// answer rather than a failure: such a citation belongs to the FILE and has no
/// entry-grain source, and inventing one is exactly the absorption this avoids.
pub fn entry_section_at(sections: &[EntrySection], line: u32) -> Option<&EntrySection> {
    sections
        .iter()
        .filter(|s| line >= s.heading_line && line <= s.end_line)
        .max_by_key(|s| s.heading_line)
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
    /// FIRST occurrence in the document. The anchor `link_scan::finding` reports, and
    /// the only position that existed before entry-grain attribution needed more.
    pub line: u32,
    /// Every LATER occurrence, ascending. Empty for a token mentioned once.
    ///
    /// **Deliberately excludes `line`** — one position, one owner, so there is no
    /// invariant like `repeat_lines[0] == line` for a later edit to violate silently.
    /// Iterate with [`Citation::occurrences`] rather than reading either field alone;
    /// reading only `line` is the defect this field exists to fix.
    ///
    /// **The citation COUNT is unchanged by this field, and that is the point.**
    /// `push_citation` still emits exactly one `Citation` per `(kind, raw)` per
    /// document, so `doctor::entry_indegree` — which increments once per `Citation` and
    /// derives its file-level exposure guarantee from that emergent property — keeps
    /// counting exactly what it counted before. Emitting one `Citation` per occurrence
    /// instead would have moved that metric, and three shipped `doctor` checks are
    /// gated on it.
    pub repeat_lines: Vec<u32>,
}

impl Citation {
    /// Every occurrence in the document, ascending, `line` first.
    ///
    /// Attribution must walk all of them: a token's first mention is routinely a
    /// preamble or `## Index` row, while the entry that genuinely rests on it cites it
    /// further down. Reading `line` alone attributes to whatever contains the first
    /// mention and silently drops the rest
    /// (`docs/issues/archive/2026-08-21-entry-attribution-follows-the-first-mention-only.md`).
    pub fn occurrences(&self) -> impl Iterator<Item = u32> + '_ {
        std::iter::once(self.line).chain(self.repeat_lines.iter().copied())
    }
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
///
/// The qualifier bound is generous ON PURPOSE, because when it is too small this
/// pattern does not fail — it SLIDES. `-` is a non-word character, so there is a `\b`
/// before every hyphen-separated segment, and the engine takes the leftmost position
/// from which the whole pattern matches. A stem longer than the bound therefore yields
/// a SHORTER qualifier that names no file, which resolves to a legitimate-looking
/// `cross_repo` row and never becomes an edge — while `edges_missing` stays 0, so the
/// report reads as clean.
///
/// That is not hypothetical: at `{1,30}` (31 chars) the same-repo citation
/// `prompt-surface-compaction-session-log:F-4` (37) was captured as
/// `surface-compaction-session-log:F-4` and silently dropped. `-session-log` alone is 12
/// characters, and one other ledger in this repo missed the old cap by exactly one.
/// See `docs/issues/archive/2026-08-18-qualified-citation-silently-truncated-when-file-stem-exceeds-31-chars.md`.
///
/// 120 clears every stem in the repo (longest today: 90) with headroom. It cannot
/// over-match into prose: the pattern still requires `:` immediately followed by an
/// entry token, so a longer allowance only extends a run of `[a-z0-9_-]` that is
/// already adjacent to a `:PREFIX-N`.
fn cross_repo_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b[a-z][a-z0-9_-]{1,119}:[A-Z]{1,3}-\d+\b").unwrap())
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
    // (kind, raw) -> index into `out.citations`, so a repeat mention appends its line to
    // the Citation already emitted instead of being dropped. A set would lose the
    // position; see `Citation::repeat_lines`.
    let mut seen_cites = std::collections::BTreeMap::new();

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
    seen: &mut std::collections::BTreeMap<(CitationKind, String), usize>,
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
    seen: &mut std::collections::BTreeMap<(CitationKind, String), usize>,
    raw: String,
    kind: CitationKind,
    line: u32,
) {
    match seen.entry((kind, raw.clone())) {
        std::collections::btree_map::Entry::Vacant(slot) => {
            slot.insert(out.citations.len());
            out.citations.push(Citation {
                raw,
                kind,
                line,
                repeat_lines: Vec::new(),
            });
        }
        // Still ONE Citation per (kind, raw) per document — the count every existing
        // consumer reads is unchanged. Only the position is no longer thrown away.
        std::collections::btree_map::Entry::Occupied(slot) => {
            out.citations[*slot.get()].repeat_lines.push(line);
        }
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

    /// Regression: a qualifier longer than the bound must be captured WHOLE.
    ///
    /// The failure this pins is not a non-match, it is a SLIDE. `-` is a non-word
    /// character, so every hyphen-separated segment starts at a `\b`, and the engine
    /// takes the leftmost position from which the whole pattern matches. Under the old
    /// `{1,30}` bound a 37-character stem yielded the 30-character suffix
    /// `surface-compaction-session-log:F-4` — a qualifier naming no file, correctly
    /// classified `cross_repo`, and therefore never turned into an edge. `edges_missing`
    /// stayed 0 throughout, so nothing in the report distinguished it from a deliberate
    /// cross-repo reference.
    ///
    /// Asserting on the captured STRING rather than the count is the point: a
    /// count-only assertion passes on the truncated capture, which is exactly how the
    /// defect survived.
    #[test]
    fn long_file_stem_qualifier_is_captured_whole_not_truncated_to_a_suffix() {
        // 37 characters — the real ledger stem that surfaced this.
        let stem = "prompt-surface-compaction-session-log";
        assert_eq!(stem.len(), 37, "fixture stem length is load-bearing");

        let text = format!("Related: {stem}:F-4 for the write path.\n");
        let ex = extract(&text);

        assert_eq!(
            tokens(&ex, CitationKind::CrossRepoToken),
            vec![format!("{stem}:F-4")],
            "the qualifier must be captured whole; a shorter capture means the bound \
             truncated it into a qualifier that names no file, which resolves to a \
             report-only cross_repo row and silently loses the edge"
        );
        assert!(
            tokens(&ex, CitationKind::EntryToken).is_empty(),
            "embedded token must still be masked"
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

    #[test]
    fn a_repeated_token_stays_one_citation_and_records_every_line() {
        // Two properties in one test because they are in tension and the pair is the
        // whole design:
        //
        //   COUNT is unchanged  — `doctor::entry_indegree` increments once per Citation
        //                         and derives its file-level exposure guarantee from that
        //                         emergent property. One Citation per occurrence would
        //                         have silently converted exposure into an occurrence
        //                         count, moving a metric three shipped doctor checks are
        //                         gated on.
        //   POSITION is kept    — attribution needs every occurrence, because a token's
        //                         first mention is routinely a preamble or `## Index` row.
        //
        // Asserting only the first would let a fix for either half break the other.
        let ex = extract("cites R-9 here\nfiller\nand R-9 again\nand once more R-9\n");

        let r9: Vec<&Citation> = ex.citations.iter().filter(|c| c.raw == "R-9").collect();
        assert_eq!(
            r9.len(),
            1,
            "three mentions must still yield ONE Citation: {:?}",
            ex.citations
        );
        assert_eq!(r9[0].line, 1, "`line` stays the FIRST occurrence");
        assert_eq!(
            r9[0].repeat_lines,
            vec![3, 4],
            "and every later occurrence is kept, ascending, excluding `line` itself"
        );
        assert_eq!(
            r9[0].occurrences().collect::<Vec<_>>(),
            vec![1, 3, 4],
            "`occurrences()` is the only correct way to read positions — reading `line` \
             alone is the first-mention defect this field exists to fix"
        );
    }

    #[test]
    fn a_token_mentioned_once_has_no_repeat_lines() {
        // The overwhelmingly common case must not allocate a misleading singleton.
        // Pins that `repeat_lines` EXCLUDES `line`: a `vec![1]` here would mean the two
        // fields overlap, and every consumer iterating `occurrences()` would double-count
        // the first position.
        let ex = extract("cites R-9 once\n");
        let r9: Vec<&Citation> = ex.citations.iter().filter(|c| c.raw == "R-9").collect();
        assert_eq!(r9.len(), 1);
        assert!(
            r9[0].repeat_lines.is_empty(),
            "no repeats means empty, not [line]: {:?}",
            r9[0]
        );
        assert_eq!(r9[0].occurrences().collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn entry_section_ends_at_next_same_or_higher_heading() {
        let md = "\
## R-1 — first
alpha
### a subheading inside R-1
beta
## R-2 — second
gamma
## Template for new entries
delta
";
        let s = entry_sections(md);
        assert_eq!(s.len(), 2, "two entries defined");
        assert_eq!(s[0].id, "R-1");
        assert_eq!(s[0].heading_line, 1);
        assert_eq!(
            s[0].end_line, 4,
            "the ### subheading is INSIDE R-1; the section ends before ## R-2"
        );
        assert!(s[0].text.contains("a subheading inside R-1"));
        assert_eq!(s[1].id, "R-2");
        assert_eq!(
            s[1].end_line, 6,
            "R-2 ends before `## Template`, a same-level non-entry heading — \
                 the last entry must NOT absorb the trailing sections"
        );
        assert!(!s[1].text.contains("delta"));
    }

    #[test]
    fn entry_sections_skip_fences_and_frontmatter() {
        let md = "\
---
kind: tracker
entry_prefix: R
---
## R-1 — real
body
```
## R-99 — inside a fence, defines nothing
```
tail
";
        let s = entry_sections(md);
        assert_eq!(s.len(), 1, "the fenced heading defines nothing");
        assert_eq!(s[0].id, "R-1");
    }

    #[test]
    fn entry_sections_do_not_define_a_heading_shaped_line_inside_frontmatter() {
        // The `## R-1 ...` line is a valid YAML full-line comment, so the
        // frontmatter block still parses; it must not be read as a heading.
        let md = "\
---
kind: tracker
## R-1 — hidden inside frontmatter, must not define
entry_prefix: R
---
## R-2 — real
body
";
        let s = entry_sections(md);
        assert_eq!(
            s.len(),
            1,
            "a heading-shaped line inside frontmatter must not define: {s:?}"
        );
        assert_eq!(s[0].id, "R-2");
    }

    #[test]
    fn nested_heading_levels_are_captured_and_bound_the_section_correctly() {
        let md = "\
## R-1 — top
alpha
### R-2 — nested inside R-1
beta
## R-3 — sibling of R-1
gamma
";
        let s = entry_sections(md);
        assert_eq!(s.len(), 3);

        assert_eq!(s[0].id, "R-1");
        assert_eq!(s[0].level, 2);
        assert_eq!(
            s[0].end_line, 4,
            "R-1 ends before ## R-3, a same-level heading; the ### R-2 \
                 subsection is deeper and stays inside"
        );
        assert_eq!(
            s[0].text,
            "## R-1 — top\nalpha\n### R-2 — nested inside R-1\nbeta"
        );

        assert_eq!(s[1].id, "R-2");
        assert_eq!(
            s[1].level, 3,
            "R-2's level must come from its own ### run, not R-1's"
        );
        assert_eq!(s[1].end_line, 4);
        assert_eq!(s[1].text, "### R-2 — nested inside R-1\nbeta");

        assert_eq!(s[2].id, "R-3");
        assert_eq!(s[2].level, 2);
        assert_eq!(
            s[2].end_line, 6,
            "R-3 has no following heading; it runs to EOF"
        );
        assert_eq!(s[2].text, "## R-3 — sibling of R-1\ngamma");
    }

    #[test]
    fn last_entry_reaches_the_true_last_line_not_one_past_it() {
        let md = "\
## R-1 — first
alpha
## R-2 — last, ends the file
";
        let s = entry_sections(md);
        assert_eq!(s.len(), 2);
        assert_eq!(s[1].id, "R-2");
        assert_eq!(s[1].heading_line, 3);
        assert_eq!(
            s[1].end_line, 3,
            "the file has exactly 3 real lines; a phantom trailing line from \
                 split('\\n') would push this to 4"
        );
        assert_eq!(s[1].text, "## R-2 — last, ends the file");
    }

    #[test]
    fn nested_entry_sections_genuinely_overlap() {
        // The premise `entry_section_at` rests on. If this ever stops holding — if
        // `entry_sections` starts bounding a parent at its nested child — the innermost
        // rule becomes dead code rather than a correction, and the next reader should
        // find out from a failing test rather than by re-deriving it.
        let md = "\
## R-1 — parent
prose citing A-1
### R-2 — nested child
prose citing A-2
## R-3 — sibling
";
        let s = entry_sections(md);
        let parent = s.iter().find(|x| x.id == "R-1").unwrap();
        let child = s.iter().find(|x| x.id == "R-2").unwrap();
        assert!(
            child.heading_line > parent.heading_line && child.end_line <= parent.end_line,
            "the ### child must sit wholly inside the ## parent: parent {}-{}, child {}-{}",
            parent.heading_line,
            parent.end_line,
            child.heading_line,
            child.end_line
        );
    }

    #[test]
    fn entry_section_at_picks_the_innermost_not_the_enclosing_entry() {
        let md = "\
## R-1 — parent
prose citing A-1
### R-2 — nested child
prose citing A-2
## R-3 — sibling
prose citing A-3
";
        let s = entry_sections(md);

        // Line 2 is only inside the parent.
        assert_eq!(entry_section_at(&s, 2).unwrap().id, "R-1");
        // Line 4 is inside BOTH R-1 and R-2. The child owns it — attributing it to R-1
        // is the container-absorption error this function exists to prevent.
        assert_eq!(
            entry_section_at(&s, 4).unwrap().id,
            "R-2",
            "a citation inside a nested child belongs to the child, not its parent"
        );
        // The child's own heading line belongs to the child.
        assert_eq!(entry_section_at(&s, 3).unwrap().id, "R-2");
        // A sibling after the child is not absorbed by it.
        assert_eq!(entry_section_at(&s, 6).unwrap().id, "R-3");
    }

    #[test]
    fn entry_section_at_returns_none_outside_every_entry() {
        // A citation in a preamble or a trailing non-defining section has no
        // entry-grain source. `None` is the correct answer — manufacturing one is the
        // 12.1% mis-attribution in a new form.
        let md = "\
preamble citing A-1
## R-1 — only entry
body
## Summary
trailing prose citing A-2
";
        let s = entry_sections(md);
        assert!(
            entry_section_at(&s, 1).is_none(),
            "a preamble citation belongs to the file, not to an entry"
        );
        assert_eq!(entry_section_at(&s, 3).unwrap().id, "R-1");
        assert!(
            entry_section_at(&s, 5).is_none(),
            "`## Summary` defines no entry, so the trailing citation must NOT be \
             absorbed by the last real entry — that is the exact 12.1% mechanism"
        );
    }

    #[test]
    fn hashtag_without_a_space_does_not_define_or_count_as_a_heading() {
        let md = "\
## R-1 — real
#R-2 — no space after the hash, not a heading
body
";
        let s = entry_sections(md);
        assert_eq!(
            s.len(),
            1,
            "a hash run with no following space is not an ATX heading"
        );
        assert_eq!(s[0].id, "R-1");
        assert_eq!(
            s[0].end_line, 3,
            "the non-heading `#R-2` line must not bound R-1's section either"
        );
    }

    #[test]
    fn a_definition_shaped_line_that_is_not_a_heading_does_not_define() {
        let md = "\
## R-1 — real
R-2 — this reads like a definition but has no leading `#`, it is prose
body
";
        let s = entry_sections(md);
        assert_eq!(
            s.len(),
            1,
            "def_re must only fire on actual heading lines, gated by is_heading"
        );
        assert_eq!(s[0].id, "R-1");
    }

    #[test]
    fn entry_sections_handle_unbalanced_nested_fence_like_extract_does() {
        // The exact shape from docs/trackers/bug-fix-session-log.md:2909 (a
        // four-backtick run wrapping a three-backtick block) — the regression
        // fixture for docs/issues/archive/2026-08-11-artifact-nested-fence-closes-outer-fence.md.
        // A bare `starts_with("```")` parity toggle desyncs on the embedded
        // four-backtick line and silently drops R-2 for the rest of the file.
        let md = "\
## R-1 — first
before
```
```` ```markdown ````
inside
```
## R-2 — after the real close
after
";
        let s = entry_sections(md);
        let ids: Vec<&str> = s.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["R-1", "R-2"],
            "the embedded four-backtick line must not desync fence parity"
        );
        assert_eq!(s[0].end_line, 6, "R-1 ends right before ## R-2");
        assert_eq!(s[1].heading_line, 7);
        assert_eq!(s[1].end_line, 8, "R-2 runs to the true EOF");
    }
    #[test]
    fn entry_section_level_reflects_heading_depth() {
        // Mixes a `##` and a `###` entry so `level` is pinned at more than one depth,
        // and so the same-or-higher bound is exercised across differing levels: the
        // nested ### section is closed by the following ## (higher, not merely equal).
        let md = "\
## R-1 — first
alpha
### R-2 — nested
beta
## R-3 — third
gamma
";
        let s = entry_sections(md);
        assert_eq!(s.len(), 3, "three entries defined");
        assert_eq!(s[0].id, "R-1");
        assert_eq!(s[0].level, 2, "R-1 is a level-2 heading");
        assert_eq!(
            s[0].end_line, 4,
            "R-1's section absorbs the nested ### R-2 heading and runs to just before ## R-3"
        );
        assert_eq!(s[1].id, "R-2");
        assert_eq!(
            s[1].level, 3,
            "R-2 is a level-3 heading nested inside R-1's section"
        );
        assert_eq!(
            s[1].end_line, 4,
            "R-2's section is bounded by the level-2 ## R-3, a SAME-OR-HIGHER heading"
        );
        assert_eq!(s[2].id, "R-3");
        assert_eq!(s[2].level, 2);
        assert_eq!(s[2].end_line, 6, "R-3 runs to EOF");
    }

    #[test]
    fn entry_section_text_includes_its_final_line() {
        // Pins the LAST line of `text` by exact equality (not `contains`, which the
        // existing tests use on interior lines and so cannot catch a slice-end
        // off-by-one). Covers both a mid-file section (bounded by the next heading)
        // and an EOF-terminated final section.
        let md = "\
## R-1 — first
alpha
R-1-LAST-LINE
## R-2 — second
beta
R-2-LAST-LINE
";
        let s = entry_sections(md);
        assert_eq!(s.len(), 2);
        assert_eq!(
            s[0].text, "## R-1 — first\nalpha\nR-1-LAST-LINE",
            "text must include the section's true final line, not stop one line early"
        );
        assert_eq!(
            s[1].text, "## R-2 — second\nbeta\nR-2-LAST-LINE",
            "the EOF-terminated final section must also include its true final line"
        );
    }

    #[test]
    fn entry_sections_do_not_define_from_a_definition_shaped_body_line() {
        // A body line shaped like `def_re` (e.g. "R-5 — ...") is prose, not a heading,
        // and must not define a section — the parity between extract() and
        // entry_sections() that the review flagged as unverified.
        let md = "\
## R-1 — first
alpha
R-5 — this looks like a definition but is prose inside R-1's body, not a heading
## R-2 — second
gamma
";
        let s = entry_sections(md);
        let ids: Vec<&str> = s.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["R-1", "R-2"],
            "a definition-shaped line that is not an ATX heading must not define a section: {s:?}"
        );
    }

    /// `entry_sections()` and `extract()` are two independent readers of the same
    /// thing — which `## <ID> — <title>` headings a markdown file defines (see the
    /// doc comment on `entry_sections` for why they must never diverge silently).
    /// Task 1's re-review measured 104/104 agreement between them across the whole
    /// `docs/**/*.md` corpus after the frontmatter/fence-parity fix, but noted the
    /// agreement was pinned by nothing in CI — the nested-fence regression test above
    /// pins the *mechanism* that broke, not the *property* that the two readers agree
    /// (`.superpowers/sdd/2026-08-20-statement-validity-layers-1-2/task-1-re-review.md`,
    /// Deferred minor 2). This test is that missing pin, modelled on
    /// `both_entry_prefix_readers_agree_on_every_yaml_form`
    /// (`src/librarian/catalog/augmentation.rs`) — same idea, two independent readers
    /// of one property, checked against real inputs rather than trusted by a comment.
    ///
    /// Walking the live corpus, rather than a frozen fixture copy, is deliberate: a
    /// fixture would exercise only the shapes the unit tests above already cover and
    /// would pin nothing new, where the live corpus can surface a real file that trips
    /// a path neither reader's unit tests anticipated — which is exactly how the
    /// nested-fence and HTML-comment-block cases below were both found. The cost is
    /// sensitivity to unrelated doc edits: a new or edited file under `docs/` can turn
    /// this test red for a reason that has nothing to do with whatever change
    /// triggered the run. That cost is paid off by the failure message, not avoided —
    /// every assertion below names the file, the ids each reader saw and did not see,
    /// and the direction of the difference, so a legitimate new-doc failure is
    /// diagnosable from the test output alone rather than mysterious.
    ///
    /// One disagreement is a known, *named* exception, not a count budget: `CAP-4` in
    /// `docs/trackers/capability-proposals.md` is a pre-existing `extract()` defect —
    /// a pulldown_cmark HTML comment block swallows the following heading, so
    /// `extract()` never sees it while `entry_sections` (a different, line-oriented
    /// parser) does — filed at
    /// `docs/issues/2026-08-20-extract-loses-heading-after-html-comment-block.md`,
    /// which prescribes this exact check. It predates this branch and is not fixed
    /// here.
    ///
    /// The exception is matched by identity (file + the full id-set diff), not
    /// counted: a bare "at most 1 disagreeing file" tolerance would stay green if
    /// `CAP-4` got fixed and a different, real drift appeared in its place — same
    /// count, wrong file, silent guard. Matching identity catches either failure mode
    /// on its own: an unexpected file trips the first assertion regardless of how many
    /// total disagreements there are, and `CAP-4` no longer appearing trips the
    /// second — which is the loud failure this test should produce the day that bug is
    /// fixed, as the prompt to delete the exception below.
    #[test]
    fn entry_sections_and_extract_agree_on_the_live_corpus() {
        // A known, pre-existing extract() defect — not fixed here. See the doc
        // comment above and
        // docs/issues/2026-08-20-extract-loses-heading-after-html-comment-block.md.
        const KNOWN_EXCEPTION_PATH: &str = "docs/trackers/capability-proposals.md";
        const KNOWN_EXCEPTION_SECTIONS_ONLY: &[&str] = &["CAP-4"];

        let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let docs_root = repo_root.join("docs");
        let mut files: Vec<std::path::PathBuf> = ignore::WalkBuilder::new(&docs_root)
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
            .map(|e| e.path().to_path_buf())
            .filter(|p| p.extension().map(|ext| ext == "md").unwrap_or(false))
            .collect();
        files.sort();
        assert!(
            files.len() > 500,
            "expected docs/**/*.md to hold hundreds of markdown files; found {} — the \
             walk is looking in the wrong place ({docs_root:?}) and every assertion \
             below would pass vacuously",
            files.len()
        );

        let mut unexpected = Vec::new();
        let mut known_exception_seen = false;

        for path in &files {
            let text = std::fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            let rel = path
                .strip_prefix(&repo_root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");

            let sections = entry_sections(&text);
            let real_lines = text.lines().count() as u32;
            let overshooting: Vec<(String, u32, u32)> = sections
                .iter()
                .filter(|s| s.end_line > real_lines)
                .map(|s| (s.id.clone(), s.end_line, real_lines))
                .collect();
            if !overshooting.is_empty() {
                unexpected.push(format!(
                    "{rel}: entry_sections() end_line runs past EOF for \
                     {overshooting:?} (id, end_line, real_line_count)"
                ));
            }

            let extract_ids: std::collections::BTreeSet<String> = extract(&text)
                .definitions
                .into_iter()
                .map(|d| d.token)
                .collect();
            let section_ids: std::collections::BTreeSet<String> =
                sections.into_iter().map(|s| s.id).collect();
            if extract_ids == section_ids {
                continue;
            }

            let extract_only: Vec<&String> = extract_ids.difference(&section_ids).collect();
            let sections_only: Vec<&String> = section_ids.difference(&extract_ids).collect();
            let sections_only_strs: Vec<&str> = sections_only.iter().map(|s| s.as_str()).collect();

            if rel == KNOWN_EXCEPTION_PATH
                && extract_only.is_empty()
                && sections_only_strs == KNOWN_EXCEPTION_SECTIONS_ONLY
            {
                known_exception_seen = true;
                continue;
            }

            unexpected.push(format!(
                "{rel}: extract()-only (extract() saw, entry_sections() missed)=\
                 {extract_only:?}; entry_sections()-only (entry_sections() saw, \
                 extract() missed)={sections_only:?}"
            ));
        }

        assert!(
            unexpected.is_empty(),
            "entry_sections() and extract() disagree beyond the known CAP-4 exception \
             on {} file(s):\n  {}\n\n\
             Each line names the file and, per reader, the ids only that reader saw — \
             investigate why the two readers see different definitions there. Do not \
             widen this into a count tolerance (see the doc comment above for why that \
             is the wrong guard); if the corpus genuinely grew a new instance of the \
             known CAP-4-shaped defect, name it explicitly alongside CAP-4 rather than \
             loosening the check.",
            unexpected.len(),
            unexpected.join("\n  ")
        );

        assert!(
            known_exception_seen,
            "the known CAP-4 exception ({KNOWN_EXCEPTION_PATH}, extract() dropping the \
             heading after an HTML comment block, filed at \
             docs/issues/2026-08-20-extract-loses-heading-after-html-comment-block.md) \
             did not reproduce this run — either that pre-existing extract() defect has \
             been fixed (delete KNOWN_EXCEPTION_PATH/KNOWN_EXCEPTION_SECTIONS_ONLY and \
             this assertion) or the file/heading was renamed (update the constants above \
             to match). Do not leave this test silently downgraded by removing the \
             check instead."
        );
    }
}
