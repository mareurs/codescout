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
pub fn extract_surface<'a>(source: &'a str, surface: &str) -> Option<&'a str> {
    let open = format!("<!-- @surface {surface} -->");
    let marker_end = source.find(&open)? + open.len();
    let bytes = source.as_bytes();
    let mut start = marker_end;
    if bytes.get(start) == Some(&b'\r') {
        start += 1;
    }
    if bytes.get(start) == Some(&b'\n') {
        start += 1;
    }
    let rest = &source[start..];
    let end_offset = rest.find("<!-- @end -->")?;
    Some(&rest[..end_offset])
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
}
