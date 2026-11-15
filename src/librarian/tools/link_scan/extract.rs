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

use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd};
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
    /// A citation with 2+ qualifier segments before the entry token
    /// (`<repo>:<file-stem>:<ID>`). The resolver has no notion of a second
    /// qualifier segment, and the extraction regex for a single qualifier SLIDES
    /// past an unmatchable prefix rather than failing to match — so without this
    /// variant, a malformed double-qualified citation silently collapses to a
    /// working single-qualifier one with no signal to the author.
    /// docs/issues/archive/2026-08-26-link-scan-double-qualified-citation-silently-drops-repo-prefix.md
    MalformedQualifier,
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
            CitationKind::MalformedQualifier => "MalformedQualifier",
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

#[derive(Debug, Default, Clone)]
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

/// Double-qualified citation: `codescout:statement-validity-session-log:F-2`. Matches
/// the FULL `<qualifier>(:<qualifier>)+:<TOKEN>` span directly — two or more
/// colon-separated qualifier segments before the entry token — so it wins the
/// leftmost-match race against `cross_repo_re` and the outer qualifier is never
/// silently dropped by that regex's own slide.
///
/// `cross_repo_re` only ever anchors on the LAST qualifier segment because its
/// pattern has no notion of an earlier one: for `codescout:statement-validity-
/// session-log:F-2` it fails to match starting at `codescout` (the character after
/// `codescout:` is `s`, not `[A-Z]`) and slides to the next `\b`, which is right
/// after the dropped segment's colon. This regex is checked FIRST in `scan_tokens`
/// specifically so the whole span is claimed before that slide has a chance to fire.
///
/// No resolver support is planned for this form — see the bug this fixes. The point
/// is visibility, not resolution: a citation shaped like this is malformed, not
/// merely unresolved.
fn double_qualified_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\b[a-z][a-z0-9_-]{1,119}(?::[a-z][a-z0-9_-]{1,119})+:[A-Z]{1,3}-\d+\b")
            .unwrap()
    })
}

/// Definition shape: heading's first text starts with an entry token followed
/// by a whitespace-delimited dash separator (`A-11 — title`). `A-9 Addendum`
/// (no dash) is a section ABOUT A-9, not a re-definition.
fn def_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s*([A-Z]{1,3}-\d+)\s+[—–-]\s+").unwrap())
}

/// Results remembered for the length of one [`ExtractMemoScope`], keyed by the text's content.
#[derive(Default)]
struct ExtractMemo {
    extracts: std::collections::HashMap<(u64, usize), DocExtract>,
    sections: std::collections::HashMap<(u64, usize), Vec<EntrySection>>,
}

thread_local! {
    static MEMO: std::cell::RefCell<Option<ExtractMemo>> = const { std::cell::RefCell::new(None) };
}

/// While one of these is alive on a thread, [`extract_memoized`] and
/// [`entry_sections_memoized`] remember what they computed for that thread, so a run that
/// scans the same files several times extracts each once.
///
/// `doctor`'s report is the user: about a dozen of its scans each re-extract the same corpus,
/// and `extract` alone was 53% of the instructions of a whole run (callgrind, 2026-10-30,
/// 74 G instructions, 6,617 files). The memo is keyed by the TEXT, never by a path, so it
/// cannot serve a stale answer for a file that changed; it only skips work whose input is
/// byte-identical. Dropping the scope frees it. A thread without a scope computes every time.
pub struct ExtractMemoScope(());

impl ExtractMemoScope {
    pub fn begin() -> Self {
        MEMO.with(|m| *m.borrow_mut() = Some(ExtractMemo::default()));
        Self(())
    }
}

impl Drop for ExtractMemoScope {
    fn drop(&mut self) {
        MEMO.with(|m| *m.borrow_mut() = None);
    }
}

/// Length is part of the key so a 64-bit hash collision would also have to collide in size.
fn content_key(text: &str) -> (u64, usize) {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    (h.finish(), text.len())
}

fn memo_is_on() -> bool {
    MEMO.with(|m| m.borrow().is_some())
}

/// [`extract`], remembered inside an [`ExtractMemoScope`] and plain outside one.
pub fn extract_memoized(text: &str) -> DocExtract {
    if !memo_is_on() {
        return extract(text);
    }
    let key = content_key(text);
    if let Some(hit) = MEMO.with(|m| {
        m.borrow()
            .as_ref()
            .and_then(|m| m.extracts.get(&key).cloned())
    }) {
        return hit;
    }
    let fresh = extract(text);
    MEMO.with(|m| {
        if let Some(memo) = m.borrow_mut().as_mut() {
            memo.extracts.insert(key, fresh.clone());
        }
    });
    fresh
}

/// [`entry_sections`], remembered inside an [`ExtractMemoScope`] and plain outside one.
pub fn entry_sections_memoized(text: &str) -> Vec<EntrySection> {
    if !memo_is_on() {
        return entry_sections(text);
    }
    let key = content_key(text);
    if let Some(hit) = MEMO.with(|m| {
        m.borrow()
            .as_ref()
            .and_then(|m| m.sections.get(&key).cloned())
    }) {
        return hit;
    }
    let fresh = entry_sections(text);
    MEMO.with(|m| {
        if let Some(memo) = m.borrow_mut().as_mut() {
            memo.sections.insert(key, fresh.clone());
        }
    });
    fresh
}

/// Entries held by the memo on this thread (extracts, sections); `(0, 0)` with no scope.
#[cfg(test)]
pub fn memo_len() -> (usize, usize) {
    MEMO.with(|m| {
        m.borrow()
            .as_ref()
            .map_or((0, 0), |m| (m.extracts.len(), m.sections.len()))
    })
}

#[cfg(test)]
mod memo_tests {
    use super::*;

    const LEDGER: &str =
        "# L\n\n## A-1 — first\n\nSee A-2 and codescout:B-7.\n\n## A-2 — second\n\ntext\n";

    #[test]
    fn outside_a_scope_nothing_is_remembered() {
        let _ = extract_memoized(LEDGER);
        let _ = entry_sections_memoized(LEDGER);
        assert_eq!(memo_len(), (0, 0));
    }

    #[test]
    fn inside_a_scope_the_memoized_answers_equal_the_plain_ones() {
        let _scope = ExtractMemoScope::begin();
        let plain = extract(LEDGER);
        for _ in 0..2 {
            let got = extract_memoized(LEDGER);
            assert_eq!(got.definitions, plain.definitions);
            assert_eq!(got.citations, plain.citations);
            assert_eq!(got.declared_prefixes, plain.declared_prefixes);
            assert_eq!(entry_sections_memoized(LEDGER), entry_sections(LEDGER));
        }
        assert_eq!(
            memo_len(),
            (1, 1),
            "two calls on one text hold ONE entry each"
        );
    }

    /// The reason the key is the text and not a path: a file that changed is a different
    /// input and must get its own answer, never the remembered one.
    #[test]
    fn a_changed_text_is_a_different_key_and_gets_its_own_answer() {
        let _scope = ExtractMemoScope::begin();
        let before = extract_memoized(LEDGER);
        let edited = LEDGER.replace("A-2 — second", "A-3 — second");
        let after = extract_memoized(&edited);
        assert_ne!(before.definitions, after.definitions);
        assert_eq!(after.definitions, extract(&edited).definitions);
        assert_eq!(memo_len().0, 2);
    }

    #[test]
    fn dropping_the_scope_frees_the_memo() {
        {
            let _scope = ExtractMemoScope::begin();
            let _ = extract_memoized(LEDGER);
            assert_eq!(memo_len().0, 1);
        }
        assert_eq!(memo_len(), (0, 0));
    }
}

pub fn extract(text: &str) -> DocExtract {
    // Frontmatter yields no definitions and no citations — the offset guard in `scan_events`
    // drops every event inside it. It does, however, DECLARE the artifact's id namespaces, and
    // that declaration is what lets the dangling gate tell a wholly-undefined ledger apart
    // from `UTF-8` in prose. Read with the guard's own parser rather than a second one:
    // `librarian_guard` compiles under `--no-default-features` where
    // `librarian::frontmatter` does not exist, and the two readers' agreement on every
    // YAML form is already pinned by `both_entry_prefix_readers_agree_on_every_yaml_form`.
    let mut out = DocExtract {
        declared_prefixes: crate::util::librarian_guard::declared_entry_prefixes(text),
        ..Default::default()
    };
    // (kind, raw) -> index into `out.citations`, so a repeat mention appends its line to
    // the Citation already emitted instead of being dropped. A set would lose the
    // position; see `Citation::repeat_lines`.
    let mut seen_cites = std::collections::BTreeMap::new();
    // No ranges: `extract` is the hot path of every doctor scan and nothing here reads one.
    out.definitions = scan_events(text, false, &mut |s| {
        push_citation(&mut out, &mut seen_cites, s.raw, s.kind, s.line)
    });
    out
}

/// Every hit [`extract`] folds into a citation, one per occurrence and not one per
/// `(kind, raw)`, with the bytes it came from. `extract`'s citations are these folded by
/// `push_citation`, because both run [`scan_events`].
///
/// Hits come in the scanner's own order: event by event, and inside one text chunk the
/// malformed qualifiers, then the cross-repo tokens, then the entry tokens, then the ids.
/// Sort by `range.start` for source order.
///
/// A range is a slice of `text`, and exists only where the hit's text is its own source slice.
/// It is `None` for: a hit in inline code (`in_code`; the code span's content is not the bytes
/// between its backticks), a hit at an escape edge (`source_range`: `F-1\_x` renders `F-1_x`),
/// and a link that is not inline (`[t][x]` with `[x]: a.md`: its destination lies in another
/// line). An entity is its own text event, so the text after it keeps its exact range
/// (`&amp; F-1` gives `F-1` the range `6..9`). A backslash escape splits one run of prose into
/// several text events (`R\-91` is Text `R` and Text `-91`), so no hit spans an escape. A hit in
/// a bare URL has a range (this scanner reads plain text, URLs included); test it with
/// `identity::grammar::in_url_word`. A path link's range is the whole `[text](dest)`.
///
/// A hit or bare token inside an inline link's text lies inside that link's `RelPathLink` range;
/// a rewrite of the link must drop it. For `[see F-1](b.md)` this scanner's own `EntryToken` `F-1`
/// and the grammar's bare token `F-1` both have exact ranges inside the link's range, and
/// applying an edit for each would overlap.
///
/// Never hits: frontmatter, fenced and indented code, HTML (a comment included), and link
/// reference definitions (the parser consumes them and emits no event). A hit in inline code is
/// kept, with `in_code` set.
pub(crate) fn citation_spans(text: &str) -> Vec<CitationSpan> {
    let mut spans = Vec::new();
    scan_events(text, true, &mut |s| spans.push(s));
    spans
}

/// The event loop behind [`extract`] and [`citation_spans`]. Citation hits go to `sink`;
/// definitions are returned. With `ranges` false no hit carries a range, and no source slice is
/// compared.
fn scan_events(text: &str, ranges: bool, sink: &mut impl FnMut(CitationSpan)) -> Vec<Definition> {
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

    let mut definitions = Vec::new();
    let mut seen_defs = std::collections::BTreeSet::new();

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
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                ..
            }) if !in_code_block => {
                if let Some(target) = link_target(dest_url.as_ref()) {
                    sink(CitationSpan {
                        kind: CitationKind::RelPathLink,
                        raw: target,
                        line,
                        // Only an inline link holds its destination in its own bytes.
                        range: (ranges && link_type == LinkType::Inline).then(|| span.clone()),
                        in_code: false,
                    });
                }
            }
            Event::Code(content) if !in_code_block => {
                // Inline code: real citations live here, but a code-first
                // heading (`### `A-9` …`) must not define.
                if in_heading && heading_first_inline {
                    heading_first_inline = false;
                }
                scan_tokens(sink, content.as_ref(), line, None, true);
            }
            Event::Text(content) if !in_code_block => {
                let mut rest = content.as_ref();
                if in_heading && heading_first_inline {
                    heading_first_inline = false;
                    if let Some(m) = def_re().captures(rest) {
                        let token = m.get(1).unwrap().as_str().to_string();
                        if seen_defs.insert(token.clone()) {
                            definitions.push(Definition { token, line });
                        }
                        // The defining token is not a citation of itself;
                        // scan only the remainder of the heading text.
                        rest = &rest[m.get(0).unwrap().end()..];
                    }
                }
                // `rest` is a suffix of `content`, so it sits `content.len() - rest.len()`
                // bytes into the event's source when the event is its own source slice.
                // Defensive under pulldown-cmark 0.13.3: no input found where a text event
                // differs from its source slice; re-check on a parser upgrade.
                let src = (ranges && text.get(span.clone()) == Some(content.as_ref()))
                    .then(|| (text, span.start + (content.len() - rest.len())));
                scan_tokens(sink, rest, line, src, false);
            }
            _ => {}
        }
    }

    // pulldown_cmark's CommonMark HTML-block rule can swallow an ATX heading whole: an
    // unclosed `<!--` on an earlier line opens an HTML block that continues until its
    // close condition, and every line in between — heading included — is emitted as one
    // `Event::Html` payload, never as `Event::Start(Tag::Heading)`. `def_re` above only
    // ever sees heading text inside a real heading event, so a swallowed heading defines
    // nothing — and an entry `extract()` cannot define is an entry nothing can cite.
    // `entry_sections` already sidesteps this by deriving headings from the shared
    // line-oriented `headings::parse` (fence-aware, HTML-oblivious) instead of cmark's
    // event stream — see its doc comment: "so the three can never disagree". Backfill any
    // heading that parser finds but the event loop above missed, so `extract` agrees with
    // it by construction rather than by coincidence. Confirmed via the actual event
    // stream (`Start(HtmlBlock)` spanning the heading as `Html(...)`), not inferred.
    // docs/issues/archive/2026-08-20-extract-loses-heading-after-html-comment-block.md
    //
    // Deliberately partial: only the definition itself is backfilled. A citation
    // embedded in a swallowed heading's remainder text (rare — the common case is a
    // whole entry, not just its heading, ending up inside the comment) is not scanned,
    // since recovering it would mean re-parsing `Event::Html` payloads for inline
    // citations too.
    let frontmatter_last_line = line_of(&line_starts, frontmatter_end).saturating_sub(1);
    for h in crate::librarian::preview::headings::parse(text) {
        let line = h.line as u32;
        if line <= frontmatter_last_line {
            continue;
        }
        if let Some(m) = def_re().captures(&h.text) {
            let token = m.get(1).unwrap().as_str().to_string();
            if seen_defs.insert(token.clone()) {
                definitions.push(Definition { token, line });
            }
        }
    }

    definitions
}

/// One hit of [`citation_spans`]: what `extract` records as a citation, with where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CitationSpan {
    pub kind: CitationKind,
    pub raw: String,
    /// The line of the hit's event, as `extract` reports it.
    pub line: u32,
    /// Bytes of `text`. `None` at an escape edge (`source_range`), for a hit in inline code, and
    /// for a link that is not inline. An entity is its own text event, so the text after it keeps
    /// its exact range.
    pub range: Option<std::ops::Range<usize>>,
    /// True for a hit inside an inline code span: `extract` counts it, a rewrite never edits it.
    pub in_code: bool,
}

/// Scan one text chunk for double-qualified (malformed), cross-repo, and entry
/// tokens, and artifact ids. Double-qualified matches are found first and masked so
/// `cross_repo_re`'s own slide never re-claims their tail as a working two-part
/// citation; cross-repo matches are then found and masked so the embedded entry
/// token is not double-reported.
fn scan_tokens(
    sink: &mut impl FnMut(CitationSpan),
    chunk: &str,
    line: u32,
    src: Option<(&str, usize)>,
    in_code: bool,
) {
    // `src` is the document and the offset of `chunk[0]` in it, when `chunk` is its own source
    // slice; a hit then carries its bytes as its range if the source agrees with the match (see
    // `source_range`), and no range otherwise.
    let mut emit = |m: &regex::Match<'_>, kind: CitationKind| {
        sink(CitationSpan {
            kind,
            raw: m.as_str().to_string(),
            line,
            range: src.and_then(|(text, base)| {
                let qualified = matches!(
                    kind,
                    CitationKind::CrossRepoToken | CitationKind::MalformedQualifier
                );
                source_range(text, base + m.start()..base + m.end(), qualified)
            }),
            in_code,
        });
    };
    let mut masked: Vec<(usize, usize)> = Vec::new();
    // Both qualified shapes end in an entry token (`:<TOKEN>`), and a `\b` holds between
    // the `:` and the token, so a chunk where `entry_re` finds nothing cannot match either
    // (pinned by `a_chunk_without_an_entry_token_matches_neither_qualified_pattern`).
    // Their qualifier stem starts at every word boundary and reads up to 119 bytes, so on
    // prose they are the dearest patterns here and the ones with nothing to find.
    if entry_re().is_match(chunk) {
        for m in double_qualified_re().find_iter(chunk) {
            masked.push((m.start(), m.end()));
            emit(&m, CitationKind::MalformedQualifier);
        }
        for m in cross_repo_re().find_iter(chunk) {
            if masked.iter().any(|&(s, e)| m.start() >= s && m.end() <= e) {
                continue;
            }
            masked.push((m.start(), m.end()));
            emit(&m, CitationKind::CrossRepoToken);
        }
    }
    for m in entry_re().find_iter(chunk) {
        if masked.iter().any(|&(s, e)| m.start() >= s && m.end() <= e) {
            continue;
        }
        emit(&m, CitationKind::EntryToken);
    }
    for m in id_re().find_iter(chunk) {
        emit(&m, CitationKind::ArtifactId);
    }
}

/// `range` when the document's own bytes around it agree with the match, else `None`.
///
/// Every pattern here is `\b..\b`, judged inside one text event. pulldown-cmark ends an event at
/// a backslash escape, so a match at an event edge can sit against a character the regex never
/// saw: `F-1\_x` renders `F-1_x` (no token), and `foo\-bar:F-5` renders `foo-bar:F-5` (qualifier
/// `foo-bar`, not `bar`). Such a hit stays a citation, as `extract` counts it, but a rewrite
/// must not edit bytes the author's text reads differently. So a range needs a non-word
/// character before it (and, for a qualified token, not `-`) and a non-word character after it
/// that is not a backslash.
fn source_range(
    text: &str,
    range: std::ops::Range<usize>,
    qualified: bool,
) -> Option<std::ops::Range<usize>> {
    let before = text.get(..range.start)?.chars().next_back();
    let after = text.get(range.end..)?.chars().next();
    let word = |c: char| c == '_' || c.is_alphanumeric();
    let bad_before = before.is_some_and(|c| word(c) || (qualified && c == '-'));
    let bad_after = after.is_some_and(|c| word(c) || c == '\\');
    (!bad_before && !bad_after).then_some(range)
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
pub(crate) fn link_target(dest: &str) -> Option<String> {
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

    /// pulldown_cmark's CommonMark HTML-block rule swallows an ATX heading that
    /// follows an unclosed `<!--` on an earlier line: no `Event::Start(Tag::Heading)`
    /// fires, so `def_re` never sees it — confirmed by dumping the event stream
    /// (`Start(HtmlBlock)` spans the whole comment, heading included, as one
    /// `Html(...)` payload). `entry_sections` already avoids this by deriving
    /// headings from the shared line-oriented `headings::parse` instead of cmark's
    /// event stream; `extract` must agree, or an entry nothing can cite exists.
    /// docs/issues/archive/2026-08-20-extract-loses-heading-after-html-comment-block.md
    #[test]
    fn heading_swallowed_by_a_preceding_html_comment_block_still_defines() {
        let text = "<!-- comment opens here\n\n## CAP-4 — a heading\n\nbody\n-->\n\nafter\n";
        let ex = extract(text);
        assert_eq!(
            ex.definitions,
            vec![Definition {
                token: "CAP-4".into(),
                line: 3
            }]
        );
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
    /// `scan_tokens` runs the two qualified-citation patterns only on a chunk where
    /// `entry_re` matches. That skip is sound only if neither pattern can match a chunk
    /// without an entry token: both end in `:<TOKEN>`, and a `\b` holds between the `:`
    /// and the token. This pins that, so a change to any of the three patterns that
    /// breaks it fails here and not as a citation `doctor` silently stops reporting.
    #[test]
    fn a_chunk_without_an_entry_token_matches_neither_qualified_pattern() {
        let chunks = [
            "plain prose with no token at all",
            "codescout:A-11",
            "a:B-2 and x:y:F-2 in one line",
            "foo:ABCD-1 has four capitals, which is not an entry token",
            "foo:A-b has no digits after the dash",
            "foo:a-1 has a lowercase token",
            "see https://example.org:8080/path and a-b-c:d",
            "prompt-engineering:L-14, codescout:statement-validity-session-log:F-2",
            "a:\nA-1 across a line break",
            "",
        ];
        for chunk in chunks {
            if entry_re().is_match(chunk) {
                continue;
            }
            assert!(
                !double_qualified_re().is_match(chunk),
                "double-qualified matched a chunk with no entry token: {chunk:?}"
            );
            assert!(
                !cross_repo_re().is_match(chunk),
                "cross-repo matched a chunk with no entry token: {chunk:?}"
            );
        }
        // The chunks that DO hold an entry token are the ones the patterns still run on.
        assert!(entry_re().is_match("codescout:A-11"));
        assert!(entry_re().is_match("x:y:F-2"));
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
    /// Regression: a THREE-part `<repo>:<file-stem>:<ID>` citation must not silently
    /// collapse to a working two-part `<file-stem>:<ID>` citation via the same SLIDE
    /// mechanism the sibling regression above pins for a single truncated qualifier —
    /// one qualifier segment earlier. The resolver never supported this three-part
    /// form; it must now be FLAGGED rather than silently reinterpreted.
    /// docs/issues/archive/2026-08-26-link-scan-double-qualified-citation-silently-drops-repo-prefix.md
    #[test]
    fn a_double_qualified_citation_is_flagged_not_silently_collapsed_to_the_inner_form() {
        let text = "See `codescout:statement-validity-session-log:F-2` for the rule.\n";
        let ex = extract(text);

        assert_eq!(
            tokens(&ex, CitationKind::MalformedQualifier),
            vec!["codescout:statement-validity-session-log:F-2"],
            "the whole three-part citation must be captured as malformed, not silently \
             reinterpreted"
        );
        assert!(
            tokens(&ex, CitationKind::CrossRepoToken).is_empty(),
            "must not ALSO resolve as a working two-part cross-repo citation -- that is \
             the silent collapse this bug fixes"
        );
        assert!(
            tokens(&ex, CitationKind::EntryToken).is_empty(),
            "the embedded entry token must still be masked"
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
            CitationKind::MalformedQualifier,
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
    /// **No exceptions left.** `CAP-4` in `docs/trackers/capability-proposals.md` was the
    /// last one — a pulldown_cmark HTML comment block swallowed the following heading, so
    /// `extract()` never saw it while `entry_sections` (a different, line-oriented parser)
    /// did. Fixed by backfilling `extract()`'s definitions from the same `headings::parse`
    /// `entry_sections` already uses, for any heading the cmark event stream missed. See
    /// `docs/issues/archive/2026-08-20-extract-loses-heading-after-html-comment-block.md`.
    #[test]
    fn entry_sections_and_extract_agree_on_the_live_corpus() {
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

            unexpected.push(format!(
                "{rel}: extract()-only (extract() saw, entry_sections() missed)=\
             {extract_only:?}; entry_sections()-only (entry_sections() saw, \
             extract() missed)={sections_only:?}"
            ));
        }

        assert!(
            unexpected.is_empty(),
            "entry_sections() and extract() disagree on {} file(s):\n  {}\n\n\
         Each line names the file and, per reader, the ids only that reader saw — \
         investigate why the two readers see different definitions there.",
            unexpected.len(),
            unexpected.join("\n  ")
        );
    }
    // ---- Span variant (`citation_spans`) ----

    /// `citation_spans` folded the way `extract` folds its hits.
    fn folded(spans: Vec<CitationSpan>) -> Vec<Citation> {
        let mut out = DocExtract::default();
        let mut seen = std::collections::BTreeMap::new();
        for s in spans {
            push_citation(&mut out, &mut seen, s.raw, s.kind, s.line);
        }
        out.citations
    }

    /// Hits that carry a range, as (kind, source slice).
    fn sliced(text: &str) -> Vec<(CitationKind, &str)> {
        citation_spans(text)
            .into_iter()
            .filter_map(|s| s.range.map(|r| (s.kind, &text[r])))
            .collect()
    }

    const MIXED: &str = "---\ntags: [F-90]\n---\n# Doc\n\n## F-1 — def and F-8\n\nSee F-2, codescout:F-3, ab:cd:F-4 and 0123456789abcdef. [t](other.md#x) `F-5` F-2 again\n\n```\nF-9\n```\n\n    F-10\n\n&amp; F-6 R\\-7 https://h.io/R-11 My_Notes:F-12\n\n| a |\n|---|\n| F-13 [u](b.md) |\n";

    #[test]
    fn extract_citations_are_the_spans_folded() {
        let plain = extract(MIXED).citations;
        let from_spans = folded(citation_spans(MIXED));
        assert_eq!(plain, from_spans);
        // Not vacuous: every kind, a repeat, and a second line are in it.
        for kind in [
            CitationKind::EntryToken,
            CitationKind::ArtifactId,
            CitationKind::RelPathLink,
            CitationKind::CrossRepoToken,
            CitationKind::MalformedQualifier,
        ] {
            assert!(plain.iter().any(|c| c.kind == kind), "{kind:?}");
        }
        assert!(plain.iter().any(|c| !c.repeat_lines.is_empty()));
        assert!(plain.len() >= 12, "{}", plain.len());
    }

    #[test]
    fn a_span_covers_exactly_its_match() {
        let text = "id 0123456789abcdef, see codescout:F-1 and F-2, [t](a.md) and [u](b/c.md#frag \"ti\")\n";
        assert_eq!(
            sliced(text),
            vec![
                (CitationKind::CrossRepoToken, "codescout:F-1"),
                (CitationKind::EntryToken, "F-2"),
                (CitationKind::ArtifactId, "0123456789abcdef"),
                (CitationKind::RelPathLink, "[t](a.md)"),
                (CitationKind::RelPathLink, "[u](b/c.md#frag \"ti\")"),
            ]
        );
        // A malformed double qualifier covers its whole run, after multibyte text.
        assert_eq!(
            sliced("é ab:b-c:F-4"),
            vec![(CitationKind::MalformedQualifier, "ab:b-c:F-4")]
        );
    }

    #[test]
    fn escapes_split_tokens_and_an_entity_neighbour_keeps_its_range() {
        // `R\-91` is Text `R` and Text `-91`: no token at all.
        assert!(citation_spans("R\\-91").is_empty());
        // So is an entity inside a token: `F&#45;1` is Text `F`, Text `-`, Text `1`.
        assert!(citation_spans("F&#45;1").is_empty());
        // pulldown-cmark gives an entity its own event, so a run after it is its own exact slice.
        assert_eq!(sliced("&amp; F-1"), vec![(CitationKind::EntryToken, "F-1")]);
        assert_eq!(citation_spans("&amp; F-1")[0].range, Some(6..9));
        assert_eq!(
            sliced("R\\-91 F-7"),
            vec![(CitationKind::EntryToken, "F-7")]
        );
        assert_eq!(sliced("\\*F-1"), vec![(CitationKind::EntryToken, "F-1")]);
    }

    // `F-1\_x` renders `F-1_x` and `foo\-bar:F-5` renders `foo-bar:F-5`. The scanner still reads
    // `F-1` and `bar:F-5` (it judges `\b` inside one event; `extract` counts both), but the
    // author's bytes read differently, so those hits carry no range.
    #[test]
    fn a_hit_against_an_escape_has_no_range() {
        for (text, raw, kind) in [
            ("F-1\\_x", "F-1", CitationKind::EntryToken),
            ("F-6\\.", "F-6", CitationKind::EntryToken),
            ("foo\\-bar:F-5", "bar:F-5", CitationKind::CrossRepoToken),
        ] {
            let spans = citation_spans(text);
            assert_eq!(spans.len(), 1, "{text:?}: {spans:?}");
            assert_eq!(
                (spans[0].kind, spans[0].raw.as_str()),
                (kind, raw),
                "{text:?}"
            );
            assert_eq!(spans[0].range, None, "{text:?}");
            assert!(extract(text)
                .citations
                .iter()
                .any(|c| c.raw == raw && c.kind == kind));
        }
    }
    // pulldown-cmark ends an event at an escape or an entity, so an event edge is the one place
    // where the regex's `\b` and the author's bytes can disagree; `source_range` reads the bytes.
    #[test]
    fn source_range_reads_the_bytes_around_a_match() {
        // (text, range, qualified) -> kept?
        for (text, range, qualified, kept) in [
            ("x F-1 y", 2..5, false, true),
            ("F-1", 0..3, false, true),
            ("(F-1)", 1..4, false, true),
            ("aF-1", 1..4, false, false),
            ("_F-1", 1..4, false, false),
            ("F-1b", 0..3, false, false),
            ("éF-1", 2..5, false, false),
            ("F-1\\.", 0..3, false, false),
            ("-F-1", 1..4, false, true),
            ("-bar:F-5", 1..8, true, false),
            ("x-bar:F-5", 2..9, true, false),
            (" bar:F-5", 1..8, true, true),
        ] {
            assert_eq!(
                source_range(text, range.clone(), qualified).is_some(),
                kept,
                "{text:?} {range:?} qualified={qualified}"
            );
        }
        // A range that is not on char boundaries is refused, not sliced.
        assert_eq!(source_range("éF-1", 1..4, false), None);
    }

    #[test]
    fn an_inline_code_hit_is_marked_in_code() {
        for text in ["`F-1`", "``x ` F-1``", "a `codescout:F-1` b"] {
            let spans = citation_spans(text);
            assert_eq!(spans.len(), 1, "{text:?}: {spans:?}");
            assert!(spans[0].in_code, "{text:?}");
            assert_eq!(spans[0].range, None, "{text:?}");
        }
        let both = citation_spans("`F-1` and F-2");
        assert_eq!(
            both.iter().map(|s| s.in_code).collect::<Vec<_>>(),
            vec![true, false]
        );
        assert_eq!(both[1].range, Some(10..13));
    }

    #[test]
    fn fenced_and_indented_code_and_frontmatter_have_no_span() {
        let text = "---\ntags: [F-90]\nid: 0123456789abcdef\n---\n\n```\nF-9 [t](a.md)\n```\n\n~~~\nF-8\n~~~\n\n    F-10 0123456789abcdef\n\nF-1\n";
        assert_eq!(sliced(text), vec![(CitationKind::EntryToken, "F-1")]);
        assert_eq!(citation_spans(text).len(), 1);
    }

    // The scanner does not read an HTML comment or a link reference definition (the parser
    // emits an Html event for the one and no event at all for the other), so neither is a span.
    // A reference-style link is a link whose destination is not in its own bytes: it is a hit
    // with no range.
    #[test]
    fn an_html_comment_and_a_reference_definition_have_no_span() {
        assert!(citation_spans("<!--\nF-1 0123456789abcdef [t](a.md)\n-->\n").is_empty());
        assert!(citation_spans("a <!-- F-1 --> b").is_empty());
        assert!(citation_spans("[x]: a.md \"F-2\"\n").is_empty());
        let spans = citation_spans("see [t][x]\n\n[x]: a.md\n");
        assert_eq!(spans.len(), 1, "{spans:?}");
        assert_eq!(
            (spans[0].kind, spans[0].raw.as_str()),
            (CitationKind::RelPathLink, "a.md")
        );
        assert_eq!(spans[0].range, None);
    }

    #[test]
    fn a_heading_remainder_span_skips_the_definition() {
        let text = "## F-1 — see F-2 and `F-3`\n";
        assert_eq!(sliced(text), vec![(CitationKind::EntryToken, "F-2")]);
    }
    // Overlap pin: the link's range holds every hit and bare token in its text, so a rewrite of
    // the link must drop them or two edits would overlap.
    #[test]
    fn a_token_in_a_path_links_text_lies_inside_the_links_range() {
        let text = "x [see F-1](b.md) y";
        let spans = citation_spans(text);
        let link = spans
            .iter()
            .find(|s| s.kind == CitationKind::RelPathLink)
            .and_then(|s| s.range.clone())
            .expect("an inline link has a range");
        assert_eq!(&text[link.clone()], "[see F-1](b.md)");
        let token = spans
            .iter()
            .find(|s| s.kind == CitationKind::EntryToken)
            .and_then(|s| s.range.clone())
            .expect("the token in the link text has a range");
        assert_eq!(&text[token.clone()], "F-1");
        assert!(link.start <= token.start && token.end <= link.end);
        // The grammar's bare token lies inside the same range.
        let bare = crate::librarian::identity::grammar::extract_spanned(text);
        assert_eq!(bare.len(), 1, "{bare:?}");
        assert_eq!(&text[bare[0].range.clone()], "F-1");
        assert!(link.start <= bare[0].range.start && bare[0].range.end <= link.end);
    }

    // `cross_repo_re` is lowercase-only: a mixed-case qualifier is no qualifier, so the token
    // stands alone and no span covers `My_Notes:F-2`.
    #[test]
    fn a_mixed_case_qualifier_gets_no_cross_repo_span() {
        let text = "My_Notes:F-2 and my-notes:F-3";
        assert_eq!(
            sliced(text),
            vec![
                (CitationKind::CrossRepoToken, "my-notes:F-3"),
                (CitationKind::EntryToken, "F-2"),
            ]
        );
    }

    // A bare URL is plain text to this scanner, so its tokens and ids are hits with ranges;
    // `grammar::in_url_word` is the guard a rewrite applies to them.
    #[test]
    fn a_hit_in_a_bare_url_has_a_range_that_in_url_word_rejects() {
        use crate::librarian::identity::grammar::in_url_word;
        let text = "see https://host/runs/R-91/0123456789abcdef and R-92 [t](a.md)";
        let flags: Vec<(&str, bool)> = citation_spans(text)
            .iter()
            .map(|s| {
                let r = s.range.clone().expect("plain text has a range");
                (&text[r.clone()], in_url_word(text, &r))
            })
            .collect();
        assert_eq!(
            flags,
            vec![
                ("R-91", true),
                ("R-92", false),
                ("0123456789abcdef", true),
                ("[t](a.md)", false),
            ]
        );
    }

    #[test]
    fn spans_agree_with_extract_and_slice_their_source_across_the_live_corpus() {
        let docs_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs");
        let (mut files, mut ranged, mut rangeless) = (0, 0, 0);
        for entry in ignore::WalkBuilder::new(&docs_root)
            .build()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !entry.file_type().is_some_and(|t| t.is_file())
                || path.extension().is_none_or(|e| e != "md")
            {
                continue;
            }
            let text = std::fs::read_to_string(path).unwrap();
            let spans = citation_spans(&text);
            assert_eq!(
                extract(&text).citations,
                folded(spans.clone()),
                "{}",
                path.display()
            );
            for s in spans {
                let Some(r) = s.range else {
                    rangeless += 1;
                    continue;
                };
                let slice = text.get(r.clone()).expect("a char boundary range");
                match s.kind {
                    CitationKind::RelPathLink => assert!(
                        slice.starts_with('[') && slice.contains(&s.raw),
                        "{slice:?} for {:?} in {}",
                        s.raw,
                        path.display()
                    ),
                    _ => assert_eq!(slice, s.raw, "{}", path.display()),
                }
                assert!(!s.in_code);
                ranged += 1;
            }
            files += 1;
        }
        assert!(files > 500, "the walk found {files} files");
        assert!(ranged > 1000, "{ranged} ranged hits");
        eprintln!("live corpus: {files} files, {ranged} ranged hits, {rangeless} rangeless");
    }
}
