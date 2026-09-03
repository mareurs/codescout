//! Which ledger entry (`W-81`, `BL-71`) encloses a given line of a tracker.
//!
//! The grammar is the citation resolver's, not a new one: a token is
//! `[A-Z]{1,3}-\d+`, and it is DEFINED only by a heading shaped
//! `## <ID> — <title>` — token, whitespace, dash (— – -), whitespace, text.
//! A heading with no title defines nothing; a table row defines nothing. See
//! `get_guide("tracker-conventions")` § Entry headings.
//!
//! Fenced code blocks are skipped. Documentation that teaches this syntax
//! quotes real-looking headings, and counting one would make every guide a
//! ledger — the fence is the escape this parser owes its namespace.
//!
//! Fence tracking is delegated to [`crate::util::markdown_fence::FenceState`]
//! rather than re-implemented here. An earlier hand-rolled version tracked
//! only run length (`ticks >= open ⇒ close`), which is CommonMark-incomplete
//! in exactly the way that module's own doc comment describes: it also
//! missed that a closer must be followed by nothing but whitespace, and that
//! a backtick fence's info string may not contain a backtick. Both gaps are
//! real in this corpus, not hypothetical — see the regression test below,
//! reproducing `docs/trackers/bug-fix-session-log.md:3002-3004`, where a
//! nested-fence example with trailing content on its "closer" line wrongly
//! closed the outer fence and then desynced state for 120 real entry
//! headings after it (measured via a grep-vs-parser agreement check on the
//! live corpus). Two implementations of one fence rule is this project's own
//! "two implementations" defect class; delegating closes it rather than
//! growing a third.

/// The entry token in scope at each 1-indexed line. Index 0 is unused padding
/// so callers can index by line number directly.
///
/// **An entry's scope ENDS**, at the next heading whose level is *at or above*
/// the defining heading's own — the ordinary markdown sectioning rule. Before
/// 2026-09-04 nothing ever cleared `current`, so an entry owned every line to
/// the next entry or to EOF, and a file with prose sections after its last entry
/// attributed all of them to it. Measured: `## History` at L425 of
/// `docs/trackers/2026-08-16-iron-law-gate-firing-audit.md` carried
/// `entry_token: GF-8`, whose heading is at L131 — 294 lines and 15 non-entry
/// `##` sections later. Corpus-wide, **6,983 lines across 90 of 1,446 files**
/// changed owner when this landed, every one of them from a wrong entry to
/// `None`. See
/// `docs/issues/2026-09-04-an-entrys-scope-never-ends-so-trailing-sections-inherit-its-token.md`.
///
/// LEVEL-AWARE, and it has to be: this corpus defines entries at `##` (1,082),
/// `###` (545) **and** `####` (64). A fixed-level terminator is wrong in both
/// directions — it would let an `##` heading run through a `###` entry, and let
/// a `#####` note terminate a `####` one. The refused simplification is
/// "terminate at any heading", which would end a `## W-1` entry at its own
/// `### Observed` sub-heading.
pub fn entry_tokens_by_line(source: &str) -> Vec<Option<String>> {
    let mut out: Vec<Option<String>> = vec![None];
    let mut current: Option<String> = None;
    // Only meaningful while `current.is_some()`; reset with it.
    let mut current_level: usize = 0;
    let mut fence = crate::util::markdown_fence::FenceState::new();

    for line in source.lines() {
        let is_delimiter = fence.feed(line);
        if !is_delimiter && !fence.in_fence() {
            if let Some((level, tok, _title)) = heading_defines_entry_parts(line) {
                current = Some(tok);
                current_level = level;
            } else if let Some(level) = heading_level(line) {
                // A heading that does not DEFINE an entry can still END one, and
                // treating those as the same fact is the whole defect: the
                // grammar could say where an entry begins and had no way to say
                // where one stops.
                if current.is_some() && level <= current_level {
                    current = None;
                    current_level = 0;
                }
            }
        }
        out.push(current.clone());
    }
    out
}

/// ATX heading level of `line` (1..=6), or `None` when it is not a heading.
///
/// Requires the space after the hashes, so `#hashtag` and a bare `###` are not
/// headings — the same rule `codescout_embed::chunker::heading_level` applies,
/// restated here rather than shared because that one lives in another crate and
/// this module deliberately owns no dependency on the chunker.
fn heading_level(line: &str) -> Option<usize> {
    let stripped = line.trim_start_matches('#');
    let hashes = line.len() - stripped.len();
    (1..=6)
        .contains(&hashes)
        .then_some(hashes)
        .filter(|_| stripped.starts_with(' '))
}

/// The token this line DEFINES, its heading LEVEL, and its TITLE text.
///
/// All three are parsed out of the same split, and two of them were discarded
/// for as long as this function existed: the level decides where the entry's
/// scope ends ([`entry_tokens_by_line`]), and the title is what makes a
/// mid-entry chunk semantically findable ([`entry_titles_by_token`]). Neither is
/// re-derived by a second pass over the same bytes.
fn heading_defines_entry_parts(line: &str) -> Option<(usize, String, String)> {
    let rest = line.strip_prefix("##")?;
    let deeper = rest.trim_start_matches('#');
    let level = 2 + (rest.len() - deeper.len());
    let rest = deeper.strip_prefix(' ')?;
    let (token, tail) = rest.split_once(char::is_whitespace)?;

    let (alpha, digits) = token.split_once('-')?;
    if alpha.is_empty() || alpha.len() > 3 || !alpha.chars().all(|c| c.is_ascii_uppercase()) {
        return None;
    }
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    // A dash with text after it is what separates a definition from a mention.
    let tail = tail.trim_start();
    let tail = tail
        .strip_prefix('—')
        .or_else(|| tail.strip_prefix('–'))
        .or_else(|| tail.strip_prefix('-'))?;
    let title = tail.trim();
    (!title.is_empty()).then(|| (level, token.to_string(), title.to_string()))
}

/// Each entry token in `source` mapped to its heading's TITLE text.
///
/// Keyed by token rather than by line ON PURPOSE. The caller
/// (`crate::librarian::indexer::embed_queue_items`) holds chunks whose
/// `start_line` is FILE-relative while this map is built over the BODY, and
/// reconciling those two frames is exactly the arithmetic that is currently
/// wrong for 378 of 3,729 chunks
/// (`docs/issues/2026-09-02-chunk-line-ranges-are-body-relative-but-published-as-file-lines.md`).
/// A token lookup needs no coordinate at all, so it cannot inherit that defect.
///
/// FIRST definition wins when a token is defined twice — a real case, since a
/// ledger's archive section can restate an old id. First-wins keeps the live
/// entry's title rather than the restatement's.
pub fn entry_titles_by_token(source: &str) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    let mut fence = crate::util::markdown_fence::FenceState::new();
    for line in source.lines() {
        let is_delimiter = fence.feed(line);
        if is_delimiter || fence.in_fence() {
            continue;
        }
        if let Some((_, tok, title)) = heading_defines_entry_parts(line) {
            out.entry(tok).or_insert(title);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_heading_defines_the_token_for_every_line_below_it() {
        let src = "# Log\n\npreamble\n\n## W-81 — a title\n\nbody\n\n## F-3 — another\n\nmore\n";
        let by_line = entry_tokens_by_line(src);
        assert_eq!(by_line[3], None, "preamble is outside every entry");
        assert_eq!(
            by_line[5].as_deref(),
            Some("W-81"),
            "the heading line itself"
        );
        assert_eq!(by_line[7].as_deref(), Some("W-81"), "body below it");
        assert_eq!(by_line[11].as_deref(), Some("F-3"), "next entry takes over");
    }

    /// An entry's scope ENDS at the next heading at or above its own level.
    ///
    /// Reproduces the shape found live 2026-09-04 in
    /// `docs/trackers/2026-08-16-iron-law-gate-firing-audit.md`, where
    /// `## History` at L425 was stored with `entry_token: GF-8` whose heading is
    /// at L131 — 294 lines and 15 non-entry `##` sections later.
    ///
    /// LOAD-BEARING: the trailing section is `## History`, a NON-entry heading.
    /// Every pre-2026-09-04 fixture in this module seeds a body whose entries run
    /// to EOF, so the terminating case was not merely untested — it was
    /// unrepresentable, and no assertion over those fixtures could have failed.
    #[test]
    fn a_non_entry_heading_at_the_same_level_ends_the_entry() {
        let src = "# Log\n\n## W-81 — a title\n\nbody\n\n## History\n\ntrailer\n";
        let by_line = entry_tokens_by_line(src);
        assert_eq!(by_line[5], Some("W-81".into()), "inside the entry");
        assert_eq!(
            by_line[7], None,
            "the `## History` heading itself is outside"
        );
        assert_eq!(
            by_line[9], None,
            "and so is its body — a section after an entry is not part of it"
        );
    }

    /// A DEEPER heading is part of the entry, not a terminator.
    ///
    /// This is the assertion that makes the rule level-aware rather than "any
    /// heading ends an entry", which would end `## W-1` at its own
    /// `### Observed` — the shape almost every entry in this corpus uses.
    #[test]
    fn a_deeper_heading_inside_an_entry_does_not_end_it() {
        let src = "## W-1 — t\n\na\n\n### Observed\n\nb\n\n#### Detail\n\nc\n";
        let by_line = entry_tokens_by_line(src);
        for (line, what) in [
            (5, "the sub-heading"),
            (7, "its body"),
            (11, "a deeper one"),
        ] {
            assert_eq!(
                by_line[line],
                Some("W-1".into()),
                "{what} belongs to the entry it sits inside"
            );
        }
    }

    /// A SHALLOWER heading ends it too, which "same level only" would miss.
    ///
    /// Entries exist at `##`, `###` and `####` in this corpus (1,082 / 545 / 64),
    /// so the terminator cannot be a fixed level. Here a `###` entry is ended by
    /// an `##` that is neither an entry nor its own level.
    #[test]
    fn a_shallower_heading_ends_a_deeper_entry() {
        let src = "## Group\n\n### T-5 — t\n\na\n\n## Another Group\n\nb\n";
        let by_line = entry_tokens_by_line(src);
        assert_eq!(by_line[5], Some("T-5".into()), "inside the ### entry");
        assert_eq!(
            by_line[9], None,
            "an ## section after a ### entry is outside it — a same-level-only \
             terminator would still report T-5 here"
        );
    }

    /// The terminator does not fire inside a fenced block.
    ///
    /// The fence is IC-6's escape and it has to cover the new branch as well as
    /// the defining one: a guide quoting `## History` as an example must not end
    /// a real entry any more than a quoted `## W-1 — x` may start one.
    #[test]
    fn a_heading_inside_a_fence_neither_starts_nor_ends_an_entry() {
        let src = "## W-1 — t\n\na\n\n```md\n## History\n```\n\nstill inside\n";
        let by_line = entry_tokens_by_line(src);
        assert_eq!(
            by_line[9],
            Some("W-1".into()),
            "a fenced `## History` is an example, not a terminator"
        );
    }

    #[test]
    fn only_the_dash_form_defines_an_entry() {
        // Mirrors get_guide("tracker-conventions") § Entry headings exactly.
        let src = "## R-91\n\na\n\n### A-9 Addendum\n\nb\n\n| R-5 | row |\n\nc\n";
        let by_line = entry_tokens_by_line(src);
        assert_eq!(by_line[3], None, "no title, no dash -> defines nothing");
        assert_eq!(by_line[7], None, "no dash -> a section ABOUT A-9");
        assert_eq!(by_line[11], None, "a table row never defines");
    }

    #[test]
    fn a_heading_inside_a_fenced_block_is_an_example_not_a_definition() {
        // LOAD-BEARING: docs teaching the syntax quote real-looking headings.
        // Counting one would make every guide a ledger. This is the IC-6
        // "parsers over a namespace owe an escape" case; the fence IS the escape.
        let src = "# Guide\n\n```\n## W-99 — not real\n```\n\nafter\n";
        let by_line = entry_tokens_by_line(src);
        assert_eq!(by_line[4], None, "inside the fence");
        assert_eq!(by_line[7], None, "and it must not leak past the fence");
    }

    #[test]
    fn h4_entries_are_recognised() {
        // 64 of 1,482 entries in this corpus are defined at ####.
        let src = "# T\n\n#### BL-71 — deep\n\nbody\n";
        assert_eq!(entry_tokens_by_line(src)[5].as_deref(), Some("BL-71"));
    }

    #[test]
    fn a_nested_fence_with_trailing_content_does_not_close_the_outer_fence() {
        // Reproduces docs/trackers/bug-fix-session-log.md:3002-3004 verbatim in
        // shape: a real corpus line, sitting inside an already-open fence,
        // undercounted 120 real entries by prematurely closing it. LOAD-BEARING:
        // line 4's leading run (4 backticks) is >= the opener's run (3), which is
        // what a run-length-only rule (CommonMark-incomplete) mistakes for a
        // valid closer. The trailing content after that run (" ```md ````") is
        // what makes it NOT a closer per CommonMark — remove the trailing
        // content and this line becomes an ordinary (valid) closer, and the test
        // stops discriminating between the fixed rule and the old buggy one.
        let src = "# T\n\n```\n```` ```md ````\n```\n\n## W-1 — real entry\n\nbody\n";
        let by_line = entry_tokens_by_line(src);
        assert_eq!(
            by_line[7].as_deref(),
            Some("W-1"),
            "the outer fence closes at line 5, not line 4 — W-1 is real prose, not fenced"
        );
        assert_eq!(by_line[9].as_deref(), Some("W-1"), "body below the heading");
    }

    #[test]
    fn an_inline_code_span_of_a_fence_does_not_open_a_block() {
        // A backtick fence's info string may not itself contain a backtick —
        // CommonMark forbids it, and FenceState enforces it. Without this rule,
        // a line like the opener below would start a fence that never closes,
        // swallowing every heading after it for the rest of the file.
        let src = "# T\n\n```` ```lang ````\n\n## W-2 — still real\n";
        let by_line = entry_tokens_by_line(src);
        assert_eq!(
            by_line[5].as_deref(),
            Some("W-2"),
            "the ```` ```lang ```` line never opened a fence"
        );
    }
}
