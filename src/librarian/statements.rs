//! Parsing a Statement's declared fields out of one entry section.
//!
//! A **Statement** is the claim an entry asserts. An entry is the markdown section;
//! not every entry is a Statement — a backlog item asserts nothing. Declaring a
//! validity class is what makes an entry one.
//!
//! This module is pure: no git, no catalog, no clock. `resolve_validity` takes the
//! fallback date as a parameter so the default-is-decay rule is testable without a
//! fixture repository.

use crate::librarian::tools::RecoverableError;
use crate::util::markdown_fence::FenceState;
use regex::Regex;
use std::sync::OnceLock;

/// The decay class a Statement declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validity {
    /// A law. No expiry. What gets promoted.
    Invariant,
    /// True of an instant. The ISO `YYYY-MM-DD` it was true on.
    Dated(String),
    /// True until a named event fires.
    Conditional { condition: String },
}

const FORMS: &str = "**Valid:** invariant | dated YYYY-MM-DD | conditional — <event>";

// Column-0 anchored by construction: prose and field share a vocabulary, so a bare
// keyword match would also count sentences ABOUT the field, not just declarations of
// it — `get_guide("tracker-conventions")` records `grep -c 'Status:'` counting
// sentences about Status as a mistake made twice in one pass by one agent. Matching
// happens one line at a time (see `first_declaration_line`), never with a single
// whole-text regex, so a fenced example that teaches the syntax is never mistaken
// for a declaration of it.
fn valid_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\*\*Valid:\*\*[ \t]+(.+?)[ \t]*$").unwrap())
}

fn rests_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\*\*Rests on:\*\*[ \t]+(.+?)[ \t]*$").unwrap())
}

fn iso_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap())
}

/// Scan `section_text` line by line, skipping fenced code blocks, and return the
/// capture of the FIRST line matching `re`.
///
/// A section's `text` legitimately contains fences (worked examples teaching the
/// syntax) and always will — a token written to teach the syntax is otherwise
/// extracted identically to one written to declare it (the CAP-8 contamination
/// pattern). Delegating fence-tracking to [`FenceState`] rather than a hand-rolled
/// toggle is required, not incidental, for the same reason Task 1's `entry_sections`
/// was: a bare toggle flips on any line starting with backtick/tilde characters,
/// including ones that are not real delimiters.
///
/// When a section carries more than one declaration line, the FIRST one wins — see
/// `first_of_duplicate_valid_declarations_wins`.
///
/// Fence delimiters may be indented, so fence-state tracking feeds the
/// whitespace-trimmed line (mirroring `headings::parse`'s convention). The
/// declaration regex itself is matched against the ORIGINAL, untrimmed line: an
/// indented `**Valid:**` is prose-under-a-list-item, not a declaration, and column-0
/// strictness is what keeps it that way (silent decay, not an error — see
/// `indented_valid_line_does_not_declare_and_decays_silently`).
fn first_declaration_line<'a>(section_text: &'a str, re: &Regex) -> Option<&'a str> {
    let mut fence = FenceState::new();
    for line in section_text.lines() {
        let trimmed = line.trim_start();
        if fence.feed(trimmed) {
            continue;
        }
        if fence.in_fence() {
            continue;
        }
        if let Some(c) = re.captures(line) {
            return Some(c.get(1).unwrap().as_str());
        }
    }
    None
}

/// Parse a declared class. `Ok(None)` means the section declares nothing.
pub fn parse_validity(section_text: &str) -> Result<Option<Validity>, RecoverableError> {
    let Some(raw) = first_declaration_line(section_text, valid_re()) else {
        return Ok(None);
    };
    let rest = raw.trim();

    if rest == "invariant" {
        return Ok(Some(Validity::Invariant));
    }

    if let Some(d) = rest.strip_prefix("dated ") {
        let d = d.trim();
        if !iso_re().is_match(d) {
            return Err(RecoverableError {
                message: format!("`**Valid:** dated {d}` is not an ISO date"),
                hint: Some(format!(
                    "Use `dated YYYY-MM-DD`. The three forms are: {FORMS}"
                )),
            });
        }
        return Ok(Some(Validity::Dated(d.to_string())));
    }

    if let Some(after) = rest.strip_prefix("conditional") {
        // A word character immediately following means this ISN'T the `conditional`
        // class — `conditionally speaking` names no class, it just happens to start
        // with the same syllables. Mirrors the `dated ` boundary above: a separator
        // or end-of-value is required, not more word characters. Without this check,
        // an unknown class silently becomes a Statement nobody declared, which is
        // worse than refusing it.
        let is_boundary = after.chars().next().is_none_or(|c| !c.is_alphanumeric());
        if is_boundary {
            let cond = after.trim().trim_start_matches(['—', '–', '-']).trim();
            if cond.is_empty() {
                return Err(RecoverableError {
                    message: "`**Valid:** conditional` names no condition".to_string(),
                    hint: Some(format!(
                        "Name the event that ends validity: `conditional — <event>`. A \
                         condition nobody named can only produce \"go re-read this\". {FORMS}"
                    )),
                });
            }
            return Ok(Some(Validity::Conditional {
                condition: cond.to_string(),
            }));
        }
    }

    Err(RecoverableError {
        message: format!("`**Valid:** {rest}` is not a known class"),
        hint: Some(format!("The three forms are: {FORMS}")),
    })
}

/// Apply default-is-decay: an undeclared Statement is `dated <fallback_date>`.
pub fn resolve_validity(
    section_text: &str,
    fallback_date: &str,
) -> Result<Validity, RecoverableError> {
    Ok(parse_validity(section_text)?.unwrap_or_else(|| Validity::Dated(fallback_date.to_string())))
}

/// The durable route to this Statement's proof, if it declares one.
pub fn parse_rests_on(section_text: &str) -> Option<String> {
    first_declaration_line(section_text, rests_re()).map(|s| s.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_three_classes() {
        assert_eq!(
            parse_validity("## R-1 — x\n\n**Valid:** invariant\n").unwrap(),
            Some(Validity::Invariant)
        );
        assert_eq!(
            parse_validity("**Valid:** dated 2026-08-20\n").unwrap(),
            Some(Validity::Dated("2026-08-20".into()))
        );
        assert_eq!(
            parse_validity("**Valid:** conditional — until the plan edit lands\n").unwrap(),
            Some(Validity::Conditional {
                condition: "until the plan edit lands".into()
            })
        );
    }

    /// `strip_prefix("dated ")` requires the separating space — `dated` glued directly
    /// to a value is not the declared grammar, even if what follows happens to look
    /// like an ISO date. Mutation-tested: `strip_prefix("dated")` (no space) instead
    /// would silently ACCEPT `dated2026-08-20` as `Dated("2026-08-20")`.
    #[test]
    fn dated_prefix_requires_a_separating_space() {
        let err = parse_validity("**Valid:** dated2026-08-20\n").unwrap_err();
        assert!(
            err.to_string().contains("not a known class"),
            "no space after `dated` must not parse as the dated form: {err}"
        );
    }

    /// The regex's `[ \t]*$` only strips trailing ASCII space/tab from the capture —
    /// a CRLF-authored file must not leak a trailing `\r` into the parsed class.
    /// (`.lines()` itself splits CRLF cleanly, and the residual `.trim()` on `rest`
    /// is defense in depth for whitespace variants `.lines()` does not special-case;
    /// see `unicode_whitespace_around_the_value_is_trimmed` for the case that isolates
    /// `.trim()`'s own contribution.)
    #[test]
    fn crlf_line_endings_do_not_break_parsing() {
        assert_eq!(
            parse_validity("**Valid:** invariant\r\n").unwrap(),
            Some(Validity::Invariant),
            "a trailing \\r from CRLF line endings must not leak into the parsed class"
        );
    }

    /// `.lines()` only special-cases ASCII `\r`/`\n`; a non-breaking space (U+00A0)
    /// pasted in place of a normal separator is not consumed by the regex's `[ \t]+`
    /// (ASCII-only) but IS stripped by `rest.trim()` (Unicode-aware). Mutation-tested:
    /// dropping `.trim()` on `rest` leaves the NBSP glued to the front of the value,
    /// so it no longer equals `"invariant"` and the class is refused instead of
    /// recognized — while every other test in this module stays green.
    #[test]
    fn unicode_whitespace_around_the_value_is_trimmed() {
        assert_eq!(
            parse_validity("**Valid:** \u{a0}invariant\n").unwrap(),
            Some(Validity::Invariant),
            "a non-breaking space before the value must not defeat recognition"
        );
    }

    /// Structure, not keyword. Prose that merely NAMES the field is not a declaration —
    /// `get_guide("tracker-conventions")` records that `grep -c 'Status:'` counting
    /// sentences about Status is a mistake made twice in one pass by one agent.
    #[test]
    fn prose_mentioning_the_field_is_not_a_declaration() {
        assert_eq!(
            parse_validity("the **Valid:** field is required on every entry\n").unwrap(),
            None,
            "mid-line mention must not parse"
        );
        assert_eq!(
            parse_validity("Valid: invariant\n").unwrap(),
            None,
            "no bold markers"
        );
    }

    #[test]
    fn dated_requires_an_iso_date() {
        let err = parse_validity("**Valid:** dated soon\n").unwrap_err();
        let t = err.to_string();
        assert!(
            t.contains("YYYY-MM-DD"),
            "must name the required shape: {t}"
        );
        assert!(t.contains("invariant"), "must name all three forms: {t}");
    }

    /// A non-word boundary is required right after `conditional` — `conditionally
    /// speaking` is not the `conditional` class, it merely starts with the same
    /// syllables. Reviewer-observed defect: before this boundary check, that input
    /// silently parsed as `Conditional { condition: "ly speaking" }` — an unknown
    /// class becoming a Statement nobody declared, which is worse than refusing it
    /// (the same defect class `dated_prefix_requires_a_separating_space` already
    /// covers for `dated`, not originally carried across to `conditional`).
    #[test]
    fn conditional_prefix_requires_a_word_boundary() {
        let err = parse_validity("**Valid:** conditionally speaking\n").unwrap_err();
        assert!(
            err.to_string().contains("not a known class"),
            "a word character glued to `conditional` must not parse as the \
             conditional form: {err}"
        );
    }

    #[test]
    fn conditional_supports_en_dash_and_hyphen_separators() {
        assert_eq!(
            parse_validity("**Valid:** conditional – until X\n").unwrap(),
            Some(Validity::Conditional {
                condition: "until X".to_string()
            }),
            "en dash separator"
        );
        assert_eq!(
            parse_validity("**Valid:** conditional - until Y\n").unwrap(),
            Some(Validity::Conditional {
                condition: "until Y".to_string()
            }),
            "hyphen separator"
        );
    }

    /// The bespoke hint text is the point of refusing a bare `conditional` at all —
    /// "go re-read this" is what a condition nobody named can only produce. Asserting
    /// merely `.contains("condition")` is satisfied by the echoed input word
    /// "conditional" itself and passes even if the entire bespoke error is deleted in
    /// favor of a generic one; assert on text that only THIS error produces.
    #[test]
    fn bare_conditional_is_refused() {
        let err = parse_validity("**Valid:** conditional\n").unwrap_err();
        let t = err.to_string();
        assert!(
            t.contains("go re-read this"),
            "must keep the bespoke hint, not a generic error: {t}"
        );
        assert!(
            t.contains("names no condition"),
            "must name the specific defect: {t}"
        );
    }

    #[test]
    fn unknown_class_is_refused() {
        assert!(parse_validity("**Valid:** eternal\n").is_err());
    }

    /// Absence means decay — the conservative reading, and the one that costs the
    /// common case zero writes.
    #[test]
    fn absent_declaration_defaults_to_dated_fallback() {
        assert_eq!(
            resolve_validity("## R-1 — x\n\nno declaration here\n", "2026-06-14").unwrap(),
            Validity::Dated("2026-06-14".into())
        );
        assert_eq!(
            resolve_validity("**Valid:** invariant\n", "2026-06-14").unwrap(),
            Validity::Invariant,
            "an explicit class always wins over the fallback"
        );
    }

    /// No policy previously pinned what happens when a section carries more than one
    /// `**Valid:**` line (e.g. a stale one left behind an edit). The first one wins —
    /// matching what the line-scan already does by returning on its first match.
    #[test]
    fn first_of_duplicate_valid_declarations_wins() {
        assert_eq!(
            parse_validity("**Valid:** invariant\n**Valid:** dated 2026-01-01\n").unwrap(),
            Some(Validity::Invariant),
            "first **Valid:** line wins; policy pinned here"
        );
    }

    /// Column-0 strictness means an indented `**Valid:**` (inside a list item or
    /// blockquote) does not declare at all — this is SILENT DECAY (falls back to the
    /// dated fallback), not an error, which is a real but accepted failure mode: it
    /// costs nothing to notice, unlike a rejected declaration.
    #[test]
    fn indented_valid_line_does_not_declare_and_decays_silently() {
        assert_eq!(
            parse_validity("  **Valid:** invariant\n").unwrap(),
            None,
            "an indented line must not be read as a declaration"
        );
        assert_eq!(
            resolve_validity("  **Valid:** invariant\n", "2026-01-01").unwrap(),
            Validity::Dated("2026-01-01".to_string()),
            "silent decay: falls back to dated instead of raising"
        );
    }

    /// The section `text` this module parses legitimately contains fenced examples
    /// that teach the syntax — a worked example must not itself become a declaration.
    #[test]
    fn valid_line_inside_a_fenced_code_block_is_not_a_declaration() {
        let text = "## R-1 — example\n\nExample syntax:\n\n```\n**Valid:** invariant\n```\n";
        assert_eq!(
            parse_validity(text).unwrap(),
            None,
            "a fenced example teaching the syntax must not itself become a declaration"
        );
    }

    /// A fenced decoy before the real declaration must not swallow it: fence tracking
    /// must correctly re-open scanning once the fence closes.
    #[test]
    fn valid_line_after_a_fenced_decoy_is_still_found() {
        let text = "```\n**Valid:** invariant\n```\n\n**Valid:** dated 2026-01-01\n";
        assert_eq!(
            parse_validity(text).unwrap(),
            Some(Validity::Dated("2026-01-01".to_string())),
            "the real declaration after a fenced decoy must still be found"
        );
    }

    /// `**Rests on:**` shares the same fence-contamination exposure as `**Valid:**` —
    /// a doc teaching its syntax inside a fenced example must not be read back as a
    /// citation.
    #[test]
    fn rests_on_inside_a_fenced_code_block_is_not_a_declaration() {
        let text = "```\n**Rests on:** ADR 2026-01-01 — decoy\n```\n";
        assert_eq!(
            parse_rests_on(text),
            None,
            "a fenced example must not be read as a Rests-on declaration"
        );
    }

    #[test]
    fn rests_on_is_line_anchored_and_optional() {
        assert_eq!(
            parse_rests_on("**Rests on:** ADR 2026-07-10 — repair-and-continue\n"),
            Some("ADR 2026-07-10 — repair-and-continue".to_string())
        );
        assert_eq!(parse_rests_on("no such field\n"), None);
        assert_eq!(
            parse_rests_on("see the **Rests on:** field\n"),
            None,
            "mid-line mention is not a declaration"
        );
    }
}
