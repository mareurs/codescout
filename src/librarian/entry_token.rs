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

/// The entry token in scope at each 1-indexed line. Index 0 is unused padding
/// so callers can index by line number directly.
pub fn entry_tokens_by_line(source: &str) -> Vec<Option<String>> {
    let mut out: Vec<Option<String>> = vec![None];
    let mut current: Option<String> = None;
    let mut fence: Option<usize> = None;

    for line in source.lines() {
        let trimmed = line.trim_start();
        let ticks = trimmed.chars().take_while(|c| *c == '`').count();
        if ticks >= 3 {
            match fence {
                // A closing fence must be at least as long as the opener, so a
                // ``` inside a ```` block does not close it.
                Some(open) if ticks >= open => fence = None,
                Some(_) => {}
                None => fence = Some(ticks),
            }
            out.push(current.clone());
            continue;
        }
        if fence.is_none() {
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
}
