//! I-01 Phase 1: single-source-of-truth template for the prompt surfaces.
//!
//! The three prompt surfaces (`server_instructions.md`, `onboarding_prompt.md`,
//! `build_system_prompt_draft`) drift independently today — renaming a tool or
//! rewording a section requires coordinated edits across all three. Phase 1
//! introduces `source.md` as the single editable document; a tiny extractor
//! slices each surface out by HTML-comment tag.
//!
//! For now this module *only* parses `source.md`; the actual surface constants
//! still load from their original files (Phase 2 will switch them). Y-C's
//! roundtrip test pins the originals; this module's tests pin that
//! `source.md` reproduces those bytes exactly. When both are green and aligned,
//! Phase 2 (`include_str!("source.md")` + `extract_surface`) becomes a
//! mechanical, low-risk swap.
//!
//! ## Tag layout
//!
//! ```text
//! <!-- @surface NAME -->
//! ...content (verbatim, including trailing LF)...
//! <!-- @end -->
//! ```
//!
//! The opening tag's trailing LF is the boundary; the closing tag is matched
//! literally without stripping. Authoring rule: always end content with a
//! single LF before `<!-- @end -->` so the slice preserves the conventional
//! markdown trailing newline.

/// Single-source-of-truth document. Parsed at compile time via `include_str!`
/// so callers (and tests) work against the exact bytes shipped in the binary.
pub const SOURCE: &str = include_str!("source.md");

/// Extract one surface's bytes from `source.md` (or any source-formatted
/// string). Returns `None` if the named surface marker is missing or the
/// closing `<!-- @end -->` tag is absent.
///
/// **Line-anchored matching (F-5 fix):** marker lines must appear ALONE on
/// their own line, modulo leading/trailing whitespace. Prose that quotes
/// the marker shape inline (e.g. `"never put `<!-- @surface foo -->` in
/// body text"`) does not match. Mirrors the editor-side gate in
/// [`crate::tools::markdown::edit_markdown::extract_surface_markers`] (F-7).
/// Both stem from the same `source.md` structural pattern; both must use
/// the same line-anchoring discipline to stay consistent. See F-5 in
/// `docs/trackers/prompt-guide-refactor-session-log.md` for the bug-class
/// history.
pub fn extract_surface<'a>(source: &'a str, surface: &str) -> Option<&'a str> {
    let open = format!("<!-- @surface {surface} -->");
    let close = "<!-- @end -->";

    let mut content_start: Option<usize> = None;
    let mut cursor = 0usize;

    for line in source.split_inclusive('\n') {
        let line_start = cursor;
        cursor += line.len();

        // Trim trailing newline(s) THEN any surrounding whitespace.
        // We compare the whole-line content to the marker — substring
        // matches in prose are intentionally not allowed.
        let trimmed = line.trim_end_matches('\n').trim_end_matches('\r').trim();

        match content_start {
            None => {
                if trimmed == open {
                    content_start = Some(cursor);
                }
            }
            Some(_) => {
                if trimmed == close {
                    return Some(&source[content_start.unwrap()..line_start]);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_server_instructions_byte_for_byte() {
        let extracted = extract_surface(SOURCE, "server_instructions")
            .expect("server_instructions surface present in source.md");
        assert_eq!(
            extracted,
            crate::prompts::SERVER_INSTRUCTIONS,
            "source.md must reproduce server_instructions.md byte-for-byte; \
             diff between extract and constant means source.md drifted from the file"
        );
    }

    #[test]
    fn extracts_onboarding_prompt_byte_for_byte() {
        let extracted = extract_surface(SOURCE, "onboarding_prompt")
            .expect("onboarding_prompt surface present in source.md");
        assert_eq!(
            extracted,
            crate::prompts::RAW_ONBOARDING_PROMPT,
            "source.md must reproduce onboarding_prompt.md byte-for-byte; \
             diff between extract and constant means source.md drifted from the file"
        );
    }

    #[test]
    fn unknown_surface_returns_none() {
        assert!(extract_surface(SOURCE, "no_such_surface").is_none());
    }

    #[test]
    fn extract_handles_inline_string() {
        let src = "<!-- @surface foo -->\nbody\n<!-- @end -->\n";
        assert_eq!(extract_surface(src, "foo"), Some("body\n"));
    }

    #[test]
    fn extract_handles_crlf_line_endings() {
        let src = "<!-- @surface foo -->\r\nbody\r\n<!-- @end -->\r\n";
        assert_eq!(extract_surface(src, "foo"), Some("body\r\n"));
    }

    #[test]
    fn extract_returns_none_when_close_tag_missing() {
        let src = "<!-- @surface foo -->\nbody without close tag\n";
        assert!(extract_surface(src, "foo").is_none());
    }

    #[test]
    fn extract_ignores_marker_quoted_in_prose() {
        // F-5: the previous substring-find() implementation matched the
        // FIRST occurrence of `<!-- @surface foo -->` anywhere in the file,
        // including prose that quoted the marker shape. Line-anchored
        // matching must skip the quoted reference and find the real marker.
        let src = "Editor note: never embed `<!-- @surface foo -->` literal text in prose.\n\
                   <!-- @surface foo -->\n\
                   real body\n\
                   <!-- @end -->\n";
        assert_eq!(extract_surface(src, "foo"), Some("real body\n"));
    }

    #[test]
    fn extract_ignores_close_marker_quoted_in_prose() {
        // Symmetric F-5 case: an inline `<!-- @end -->` reference in body
        // prose must not terminate the surface early.
        let src = "<!-- @surface foo -->\n\
                   body line 1\n\
                   see the `<!-- @end -->` marker below for the actual end.\n\
                   body line 2\n\
                   <!-- @end -->\n";
        let result = extract_surface(src, "foo").unwrap();
        assert!(
            result.contains("body line 2"),
            "extractor terminated early at quoted close marker; got:\n{result}"
        );
        assert!(
            result.contains("see the `<!-- @end -->` marker below"),
            "quoted close marker line should be part of body, got:\n{result}"
        );
    }

    #[test]
    fn extract_tolerates_trailing_whitespace_on_marker() {
        // Defensive: a stray trailing space after the marker (common from
        // editor auto-trim mishaps) should not break extraction.
        let src = "<!-- @surface foo -->   \nbody\n<!-- @end -->  \n";
        assert_eq!(extract_surface(src, "foo"), Some("body\n"));
    }

    #[test]
    fn extract_requires_marker_on_its_own_line() {
        // F-5: a marker that's NOT alone on a line (e.g. prefixed by other
        // text on the same line) should not match. Prevents accidents like
        // `// <!-- @surface foo -->` in code comments from being parsed.
        let src = "// <!-- @surface foo -->\nnot a real surface\n// <!-- @end -->\n";
        assert!(
            extract_surface(src, "foo").is_none(),
            "marker prefixed by `// ` must not be matched as a real surface"
        );
    }
}
