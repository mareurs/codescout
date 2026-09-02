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

pub(crate) const FORMS: &str = "**Valid:** invariant | dated YYYY-MM-DD | conditional — <event>";

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

// Exact 4-2-2 digit width, no more, no less. `chrono::NaiveDate::parse_from_str`'s
// `%Y-%m-%d` alone is MORE lenient than this on year/month/day width — measured
// 2026-08-20: it parses `26-08-20`, `2026-8-20` and `2026-08-2` without complaint,
// which would silently start accepting shapes the old parser refused. This regex is
// the shape gate; `chrono` (in `parse_validity`) is the calendar gate. Both are
// required — neither alone closes
// docs/issues/archive/2026-08-20-impossible-date-hides-a-statement-from-every-check.md,
// whose root cause was a shape-only check that let a calendar-impossible date through.
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

/// Scan for the first line matching `re`, then continue consuming its
/// **paragraph** — the hard-wrapped continuation authors actually write.
///
/// [`first_declaration_line`]'s single-line capture is correct for a closed
/// grammar like `**Valid:**` and wrong for free-text `**Rests on:**`. Measured
/// 2026-09-02 over every entry section in `docs/**`: **145 of 208** `Rests on:`
/// declarations are followed by a non-blank continuation line, and **25** carry
/// their only resolvable target below line 1 — so line-anchoring was discarding
/// a third of the targets authors had already written, and rendering the
/// fragment as the whole field. Reading the paragraph moves the corpus from
/// 47% to 59% resolvable. See
/// `docs/issues/archive/2026-09-02-parse-rests-on-truncates-at-line-one.md`.
///
/// **Only the declaring line is column-0 anchored.** Continuation lines are
/// expected to be indented — that is the hanging-indent form this field's own
/// design spec uses — so they are trimmed and joined with a single space.
///
/// Four stop conditions, each a real shape in this corpus:
///
/// - a **blank line** — ordinary paragraph end;
/// - a **heading** — the section ended;
/// - a **fence delimiter** — a worked example follows, and its contents are not
///   the declaration (the same CAP-8 contamination rule [`FenceState`] exists
///   for);
/// - **the next bold field label** (`**Valid:**`, `**Status:**`). This one is
///   load-bearing rather than defensive: entry sections in this project put
///   those on lines *adjacent* to `**Rests on:**` with no blank between, so
///   without it the validity class is swallowed into the rests-on value and
///   [`parse_validity`] is silently asked about different text. Pinned by
///   `rests_on_stops_at_the_next_bold_field`.
///
/// Deliberately NOT used by [`parse_validity`]: its grammar is closed
/// (`invariant` / `dated <ISO>` / `conditional — <event>`) and its tests require
/// a trailing em-dash after `dated` to be *rejected*, so consuming a paragraph
/// there would change what parses.
fn first_declaration_paragraph(section_text: &str, re: &Regex) -> Option<String> {
    fn is_bold_field_label(line: &str) -> bool {
        line.strip_prefix("**").is_some_and(|r| r.contains(":**"))
    }

    let mut fence = FenceState::new();
    let mut parts: Vec<&str> = Vec::new();

    for line in section_text.lines() {
        let trimmed = line.trim_start();
        let is_delim = fence.feed(trimmed);

        if parts.is_empty() {
            // Still hunting the declaring line — identical to
            // `first_declaration_line`, including matching the UNTRIMMED line so
            // an indented `**Rests on:**` stays prose-under-a-list-item.
            if is_delim || fence.in_fence() {
                continue;
            }
            if let Some(c) = re.captures(line) {
                parts.push(c.get(1).unwrap().as_str());
            }
            continue;
        }

        if is_delim || trimmed.is_empty() || trimmed.starts_with('#') || is_bold_field_label(line) {
            break;
        }
        parts.push(trimmed);
    }

    if parts.is_empty() {
        return None;
    }
    Some(
        parts
            .iter()
            .map(|s| s.trim())
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string(),
    )
}

/// Truncate an entry section's text at the first NESTED entry definition inside it.
///
/// `entry_sections` bounds a section at the next heading of the same-or-higher level,
/// so a deeper child's definition CAN sit wholly inside its parent's section text.
/// Measured 2026-08-20 against every entry in `docs/**/*.md`: 3 of 1101 sections
/// actually contain a nested entry definition —
/// `docs/superpowers/specs/2026-06-26-c1-output-buffer-dedup-design.md:C-1`,
/// `docs/trackers/prompt-hamsa-audit-log.md:A-28`, and `:A-29`.
///
/// Parsing a `**Valid:**` declaration straight out of an untruncated parent would let
/// [`parse_validity`]'s first-wins rule read the CHILD's declaration as the PARENT's,
/// whenever the parent declares nothing of its own. That is the unsafe direction: it
/// asserts a law nobody declared for the parent, where the correct read is the `dated`
/// default. Truncating at the first nested entry's heading line removes the
/// possibility entirely — a parent with no declaration of its own reads as `None`
/// (the caller's `dated` default), never as its child's class.
///
/// **Call this before [`parse_validity`] or [`parse_rests_on`], never `section.text`
/// directly.** It lives here rather than beside its first caller precisely because that
/// rule is easy to miss: `s.text` where this was required is a defect this project has
/// already shipped once (`docs/trackers/capability-proposals.md` § CAP-9 review, item 6),
/// and a helper reachable only from `doctor.rs` invites the next consumer to repeat it.
pub fn declared_section_text(
    section: &crate::librarian::tools::link_scan::extract::EntrySection,
    all: &[crate::librarian::tools::link_scan::extract::EntrySection],
) -> String {
    let cut = all
        .iter()
        .filter(|other| {
            other.heading_line > section.heading_line && other.heading_line <= section.end_line
        })
        .map(|other| other.heading_line)
        .min();

    match cut {
        Some(nested_line) => {
            let keep = (nested_line - section.heading_line) as usize;
            section
                .text
                .lines()
                .take(keep)
                .collect::<Vec<_>>()
                .join("\n")
        }
        None => section.text.clone(),
    }
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
        // Shape gate (exact 4-2-2 digit width via `iso_re`) AND calendar gate
        // (`chrono::NaiveDate`, real Gregorian validity) — both required. Neither
        // alone closes
        // docs/issues/archive/2026-08-20-impossible-date-hides-a-statement-from-every-check.md:
        // the shape-only regex let `dated 2026-02-30` through as `Ok(Some(Dated(..)))`,
        // and every consumer that later tried to convert it (`iso_to_epoch_days`)
        // silently `continue`d — the record stayed invisible to every check that
        // partitions on class. And `chrono`'s `%Y-%m-%d` ALONE is more lenient than
        // the old shape check on digit width — measured 2026-08-20: it parses
        // `26-08-20`, `2026-8-20` and `2026-08-2` without complaint — so dropping the
        // shape regex in favor of `chrono` alone would silently start accepting
        // shapes the old parser refused.
        let is_valid_date =
            iso_re().is_match(d) && chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok();
        if !is_valid_date {
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
///
/// Reads the declaration's whole **paragraph**, not just its first line — see
/// [`first_declaration_paragraph`] for the stop conditions and the measurement
/// that forced the change.
pub fn parse_rests_on(section_text: &str) -> Option<String> {
    first_declaration_paragraph(section_text, rests_re())
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

    #[test]
    fn dated_rejects_a_calendar_invalid_date_even_when_shape_valid() {
        // `2026-02-30` matched the old shape-only `^\d{4}-\d{2}-\d{2}$` regex, was
        // returned as `Ok(Some(Dated(..)))`, and every consumer then silently
        // dropped it — this is the fix for
        // docs/issues/archive/2026-08-20-impossible-date-hides-a-statement-from-every-check.md.
        // A real calendar parse must refuse it at declaration time instead.
        for bad in ["2026-02-30", "2025-02-29", "2026-99-99", "2020-13-45"] {
            let result = parse_validity(&format!("**Valid:** dated {bad}\n"));
            assert!(
                result.is_err(),
                "{bad} is calendar-invalid and must be Err, got {result:?}"
            );
        }
    }

    #[test]
    fn dated_still_accepts_every_shape_valid_calendar_valid_date() {
        for good in ["2026-08-20", "2024-02-29", "2000-01-01", "9999-12-31"] {
            assert_eq!(
                parse_validity(&format!("**Valid:** dated {good}\n")).unwrap(),
                Some(Validity::Dated(good.to_string())),
                "a real calendar date must still parse: {good}"
            );
        }
    }

    #[test]
    fn dated_rejects_non_padded_or_wrong_width_shapes() {
        // The old regex was `^\d{4}-\d{2}-\d{2}$` — exact 4-2-2 digit width, no
        // more, no less. `chrono`'s `%Y-%m-%d` alone is MORE lenient than that on
        // digit width (measured 2026-08-20: it accepts `26-08-20`, `2026-8-20`,
        // `2026-08-2` without complaint), so the shape regex must stay in addition
        // to the calendar check, not be replaced by it.
        for bad in [
            "26-08-20",
            "2026-8-20",
            "2026-08-2",
            "02026-08-20",
            "2026-08-200",
        ] {
            let result = parse_validity(&format!("**Valid:** dated {bad}\n"));
            assert!(
                result.is_err(),
                "non-4-2-2-digit shape must still be refused: {bad} -> {result:?}"
            );
        }
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

    /// The defect: `parse_rests_on` returned line 1 only, so a hard-wrapped
    /// declaration was truncated mid-value and rendered as the whole field by
    /// `context.rs:427`.
    ///
    /// **Load-bearing fixture shape:** the continuation here is *indented*, the
    /// hanging-indent form this field's design spec uses
    /// (`2026-08-20-entry-validity-and-attestation-design.md:534-535`). Column-0
    /// strictness applies to the DECLARING line only. The corpus also contains
    /// the *unindented* wrap — see
    /// `rests_on_captures_the_real_corpus_declaration_that_was_being_truncated`,
    /// which pins verbatim bytes; both shapes must parse, and testing only this
    /// one would have missed the commoner of the two.
    ///
    /// Measured 2026-09-02: 145 of 208 declarations in `docs/**` are wrapped
    /// like this, and 25 carry their only resolvable target below line 1.
    #[test]
    fn rests_on_captures_a_hard_wrapped_continuation() {
        let text = "**Rests on:** `docs/RELEASE.md` § *Before cherry-pick: read\n  the live output* — and `src/librarian/mod.rs:215`\n";
        assert_eq!(
            parse_rests_on(text).as_deref(),
            Some(
                "`docs/RELEASE.md` § *Before cherry-pick: read the live output* — and `src/librarian/mod.rs:215`"
            ),
        );
    }

    /// The bytes the bug was filed on, verbatim from
    /// `docs/trackers/bug-fix-session-log.md:5114-5116` (`W-57`).
    ///
    /// **Load-bearing, and it corrects the synthetic fixture above:** this
    /// declaration wraps across three lines at **column 0**, not with a hanging
    /// indent. The indented form is what the design spec shows; the unindented
    /// form is what the corpus mostly contains. A continuation rule that
    /// required indentation would pass every synthetic test here and still
    /// truncate the real corpus, which is exactly the defect being fixed.
    ///
    /// Before the fix this returned everything up to `…of any` — cut mid-title,
    /// mid-italic, and rendered as the whole field by `context.rs:427`.
    ///
    /// If `W-57` is ever edited, do not "repair" this fixture to match: copy the
    /// text here into the test and keep it, or replace it with another verbatim
    /// multi-line declaration. Its value is that it is real, not that it is
    /// current.
    #[test]
    fn rests_on_captures_the_real_corpus_declaration_that_was_being_truncated() {
        let text = "**Rests on:** `docs/RELEASE.md` § *Before cherry-pick: read the live output of any\n\
                    tool-facing change (required)* and its two prior datapoints, which this extends\n\
                    rather than establishes.\n\n\
                    **Status:** validated\n";
        assert_eq!(
            parse_rests_on(text).as_deref(),
            Some(
                "`docs/RELEASE.md` § *Before cherry-pick: read the live output of any \
                 tool-facing change (required)* and its two prior datapoints, which this extends \
                 rather than establishes."
            ),
        );
    }

    /// The stop condition that matters most in this corpus: entry sections put
    /// `**Rests on:**` and `**Valid:**` on ADJACENT lines with no blank between,
    /// so a naive paragraph consumer swallows the validity class into the
    /// rests-on value and silently changes what `parse_validity` is asked about.
    #[test]
    fn rests_on_stops_at_the_next_bold_field() {
        let text = "**Rests on:** ADR 2026-07-10\n**Valid:** invariant\n**Status:** open\n";
        assert_eq!(parse_rests_on(text).as_deref(), Some("ADR 2026-07-10"));
        // And the sibling parser still sees its own field, unconsumed.
        assert_eq!(
            parse_validity(text).unwrap(),
            Some(Validity::Invariant),
            "a paragraph-consuming rests-on must not eat the Valid: line"
        );
    }

    #[test]
    fn rests_on_stops_at_a_blank_line_heading_or_fence() {
        assert_eq!(
            parse_rests_on("**Rests on:** one\n\nnot part of it\n").as_deref(),
            Some("one"),
        );
        assert_eq!(
            parse_rests_on("**Rests on:** two\n## A heading\n").as_deref(),
            Some("two"),
        );
        assert_eq!(
            parse_rests_on("**Rests on:** three\n```\nfenced\n```\n").as_deref(),
            Some("three"),
        );
    }
}
