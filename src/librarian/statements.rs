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

// Line-anchored by construction: prose and field share a vocabulary, so a bare
// keyword match would also count sentences ABOUT the field, not just declarations
// of it — `get_guide("tracker-conventions")` records `grep -c 'Status:'` counting
// sentences about Status as a mistake made twice in one pass by one agent.
fn valid_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^\*\*Valid:\*\*[ \t]+(.+?)[ \t]*$").unwrap())
}

fn rests_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^\*\*Rests on:\*\*[ \t]+(.+?)[ \t]*$").unwrap())
}

fn iso_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap())
}

/// Parse a declared class. `Ok(None)` means the section declares nothing.
pub fn parse_validity(section_text: &str) -> Result<Option<Validity>, RecoverableError> {
    let Some(c) = valid_re().captures(section_text) else {
        return Ok(None);
    };
    let rest = c[1].trim();

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
    rests_re()
        .captures(section_text)
        .map(|c| c[1].trim().to_string())
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
    /// a CRLF-authored file leaves a trailing `\r` in the captured value, since `$`
    /// (multi-line mode) matches immediately before `\n`, not before `\r\n` as a unit.
    /// Only the explicit `.trim()` on `rest` normalizes that away. Mutation-tested:
    /// dropping that `.trim()` breaks exactly this case while every other test still
    /// passes, because the regex's own construction otherwise excludes leading/trailing
    /// ASCII whitespace from the capture.
    #[test]
    fn crlf_line_endings_do_not_break_parsing() {
        assert_eq!(
            parse_validity("**Valid:** invariant\r\n").unwrap(),
            Some(Validity::Invariant),
            "a trailing \\r from CRLF line endings must not leak into the parsed class"
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
    fn bare_conditional_is_refused() {
        let err = parse_validity("**Valid:** conditional\n").unwrap_err();
        assert!(
            err.to_string().contains("condition"),
            "a condition nobody named can only produce 'go re-read this'"
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
