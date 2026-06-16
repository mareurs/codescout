//! Shared escape-decode auto-repair for the structural edit tools.
//!
//! Both `edit_file` and `edit_code` can receive a payload whose line breaks
//! arrived as literal escape sequences (a `new_string` / `body` sent with a
//! backslash-n instead of a real newline), which collapses multi-line code onto
//! one physical line and breaks syntax. This module centralizes the recovery:
//! try the edit, and if it *introduces* a parse error, retry once with the
//! inserted fragment's escapes decoded — keeping the decoded result only when it
//! parses.
//!
//! The repair logic is shared; the *fallback policy* when an introduced error
//! cannot be repaired stays with each caller. `edit_file` is non-fatal and
//! warns; `edit_code`'s insert path rejects without writing, since it has no LSP
//! round-trip to self-heal a malformed insert.

use std::path::Path;

/// Note attached to a response when an edit was auto-repaired.
pub(crate) const REPAIR_NOTE: &str =
    "auto-corrected literal newline/tab escapes in the payload to real characters";

/// Shared escape-decoding scan. When `decode_quotes` is false this is the
/// conservative decoder (newline/tab/carriage-return only); when true it also
/// decodes escaped quotes (`\"` and `\'`). A doubled backslash (`\\`) is never
/// decoded under either mode — that is genuinely dangerous (regex literals,
/// Windows paths). Returns `None` when nothing was decoded so callers can
/// cheaply skip the repair path.
fn decode_literal_escapes_inner(s: &str, decode_quotes: bool) -> Option<String> {
    if !s.contains('\\') {
        return None;
    }
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let mut changed = false;
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some('n') => {
                    out.push('\n');
                    chars.next();
                    changed = true;
                }
                Some('t') => {
                    out.push('\t');
                    chars.next();
                    changed = true;
                }
                Some('r') => {
                    out.push('\r');
                    chars.next();
                    changed = true;
                }
                Some('"') if decode_quotes => {
                    out.push('"');
                    chars.next();
                    changed = true;
                }
                Some('\'') if decode_quotes => {
                    out.push('\'');
                    chars.next();
                    changed = true;
                }
                _ => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    changed.then_some(out)
}

/// Decode the literal escape sequences an MCP client may deliver un-decoded
/// (newline, tab, carriage-return as backslash-n / backslash-t / backslash-r).
/// Returns `None` when the input contains none of them, so callers can cheaply
/// skip the repair path. Single left-to-right pass; a backslash before any other
/// character is left intact, so escaped quotes, doubled backslashes, and regex
/// escapes survive untouched.
pub(crate) fn decode_literal_escapes(s: &str) -> Option<String> {
    decode_literal_escapes_inner(s, false)
}

/// Aggressive recovery variant: decodes `\"` and `\'` in addition to the
/// conservative `\n`/`\t`/`\r`. Used only as a second-tier recovery in
/// `perform_edit` after the conservative decode fails to produce a unique
/// match — a common MCP-client failure where the client over-escapes interior
/// quotes. Still never decodes `\\`. The unique-match gate at the call site
/// keeps it as safe as the conservative tier.
pub(crate) fn decode_literal_escapes_incl_quotes(s: &str) -> Option<String> {
    decode_literal_escapes_inner(s, true)
}

/// Outcome of [`finalize_edit_content`].
pub(crate) enum RepairResult {
    /// The candidate parses cleanly (or the file was already unparseable, or the
    /// language is unknown). Write it as-is.
    Clean(String),
    /// The edit would have introduced a parse error, but decoding literal escapes
    /// in the inserted fragment produced valid content. Write it and surface a
    /// note ([`REPAIR_NOTE`]).
    Repaired(String),
    /// The edit introduces a parse error that escape-decoding cannot fix. The
    /// caller applies its own policy (warn-and-write, or reject-without-writing).
    Introduced(String),
}

impl RepairResult {
    /// The content regardless of variant — for callers that do not branch on the
    /// fallback policy (e.g. a path that has already recovered the match).
    pub(crate) fn into_content(self) -> String {
        match self {
            RepairResult::Clean(c) | RepairResult::Repaired(c) | RepairResult::Introduced(c) => c,
        }
    }
}

/// Classify (and where possible repair) an edit before it is written.
///
/// `candidate` is the content the edit would write. `new_fragment` is the
/// inserted text (edit_file's `new_string`, edit_code's `body`). When the edit
/// introduces a parse error the file did not have, `reapply_decoded` is called
/// once with the fragment's escapes decoded to rebuild the candidate; the decoded
/// result is adopted only when it parses.
pub(crate) fn finalize_edit_content<F>(
    path: &Path,
    original: &str,
    candidate: String,
    new_fragment: &str,
    reapply_decoded: F,
) -> RepairResult
where
    F: FnOnce(&str) -> String,
{
    let Some(lang) = crate::ast::detect_language(path) else {
        return RepairResult::Clean(candidate);
    };
    if !crate::ast::has_syntax_errors(&candidate, lang) {
        return RepairResult::Clean(candidate);
    }
    if crate::ast::has_syntax_errors(original, lang) {
        // Pre-existing breakage — don't block an edit to an already-broken file.
        return RepairResult::Clean(candidate);
    }
    if let Some(decoded) = decode_literal_escapes(new_fragment) {
        let repaired = reapply_decoded(&decoded);
        if !crate::ast::has_syntax_errors(&repaired, lang) {
            return RepairResult::Repaired(repaired);
        }
    }
    RepairResult::Introduced(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn decode_literal_escapes_decodes_n_t_r() {
        assert_eq!(decode_literal_escapes("a\\nb").as_deref(), Some("a\nb"));
        assert_eq!(decode_literal_escapes("a\\tb").as_deref(), Some("a\tb"));
        assert_eq!(decode_literal_escapes("a\\rb").as_deref(), Some("a\rb"));
    }

    #[test]
    fn decode_literal_escapes_leaves_other_escapes_intact() {
        // A real backslash-n decodes; an escaped quote alongside it survives.
        assert_eq!(
            decode_literal_escapes("a\\nb\\\"c").as_deref(),
            Some("a\nb\\\"c")
        );
        // Nothing decodable -> None (the escaped quote is left for the caller).
        assert_eq!(decode_literal_escapes("a\\\"b"), None);
    }

    #[test]
    fn decode_literal_escapes_none_when_nothing_to_decode() {
        assert_eq!(decode_literal_escapes("plain text"), None);
    }
    #[test]
    fn decode_incl_quotes_decodes_escaped_quotes() {
        // The quote-inclusive variant decodes \" and \' ...
        assert_eq!(
            decode_literal_escapes_incl_quotes("a\\\"b").as_deref(),
            Some("a\"b")
        );
        assert_eq!(
            decode_literal_escapes_incl_quotes("a\\'b").as_deref(),
            Some("a'b")
        );
        // ... while the conservative one leaves them intact (contract unchanged).
        assert_eq!(decode_literal_escapes("a\\\"b"), None);
    }

    #[test]
    fn decode_incl_quotes_decodes_newline_and_quotes_together() {
        assert_eq!(
            decode_literal_escapes_incl_quotes("x\\nassert(\\\"m\\\")").as_deref(),
            Some("x\nassert(\"m\")")
        );
    }

    #[test]
    fn decode_incl_quotes_leaves_doubled_backslash_intact() {
        // \\ must never decode, even in the aggressive variant.
        assert_eq!(decode_literal_escapes_incl_quotes("a\\\\b"), None);
    }

    #[test]
    fn finalize_clean_when_candidate_parses() {
        let r = finalize_edit_content(
            Path::new("x.rs"),
            "fn a() {}\n",
            "fn a() {}\nfn b() {}\n".to_string(),
            "fn b() {}",
            |d| format!("fn a() {{}}\n{d}\n"),
        );
        assert!(matches!(r, RepairResult::Clean(_)));
    }

    #[test]
    fn finalize_repairs_introduced_error_via_decode() {
        // Candidate as-is carries a literal backslash-n (broken); decoded parses.
        let candidate = "fn a() {}\nfn b() {\\n    let x = 1;\\n}\n".to_string();
        let r = finalize_edit_content(
            Path::new("x.rs"),
            "fn a() {}\n",
            candidate,
            "fn b() {\\n    let x = 1;\\n}",
            |decoded| format!("fn a() {{}}\n{decoded}\n"),
        );
        match r {
            RepairResult::Repaired(c) => {
                assert!(
                    !c.contains("\\n"),
                    "decoded content must use real newlines: {c}"
                );
                assert!(c.contains("let x = 1;"));
            }
            _ => panic!("expected Repaired"),
        }
    }

    #[test]
    fn finalize_introduced_when_unrepairable() {
        // Unbalanced brace, no escape sequences to decode -> Introduced.
        let r = finalize_edit_content(
            Path::new("x.rs"),
            "fn a() {}\n",
            "fn a() {}\nfn b() {\n".to_string(),
            "fn b() {",
            |d| d.to_string(),
        );
        assert!(matches!(r, RepairResult::Introduced(_)));
    }

    #[test]
    fn finalize_clean_when_original_already_broken() {
        // Original already unparseable -> don't block the edit.
        let r = finalize_edit_content(
            Path::new("x.rs"),
            "fn a() {\n",
            "fn a() {\nfn b() {\n".to_string(),
            "fn b() {",
            |d| d.to_string(),
        );
        assert!(matches!(r, RepairResult::Clean(_)));
    }
}
