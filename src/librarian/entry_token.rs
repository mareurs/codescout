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
pub fn entry_tokens_by_line(source: &str) -> Vec<Option<String>> {
    let mut out: Vec<Option<String>> = vec![None];
    let mut current: Option<String> = None;
    let mut fence = crate::util::markdown_fence::FenceState::new();

    for line in source.lines() {
        let is_delimiter = fence.feed(line);
        if !is_delimiter && !fence.in_fence() {
            if let Some(tok) = heading_defines_entry(line) {
                current = Some(tok);
            }
        }
        out.push(current.clone());
    }
    out
}

/// The token this line DEFINES, if it is an entry-defining heading.
fn heading_defines_entry(line: &str) -> Option<String> {
    let rest = line.strip_prefix("##")?;
    let rest = rest.trim_start_matches('#');
    let rest = rest.strip_prefix(' ')?;
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
    (!tail.trim().is_empty()).then(|| token.to_string())
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
