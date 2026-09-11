#[derive(Debug, PartialEq)]
pub struct FilterResult {
    /// Filtered markdown: preamble + matched section bodies.
    pub content: String,
    /// True if at least one requested section was found.
    pub matched: bool,
    /// Requested sections not found — preserves caller-supplied casing.
    pub missing: Vec<String>,
    /// Every boundary-level heading present in the file, normalized (trimmed), in
    /// file order. The level is whatever `boundary_level` resolved for this
    /// content, so this is `##` headings in most memories and `###` in some.
    pub available: Vec<String>,
}

/// Heading levels eligible to be a section boundary.
///
/// `#` (H1) is deliberately excluded. In 19 of this project's 21 memories H1 is
/// the document title and appears exactly once — treating it as a boundary would
/// make the title the only section and nest every real section inside it, which
/// breaks filtering outright. `development-commands.md` (5 H1 + 9 H2) is the one
/// memory that uses H1 structurally; excluding H1 still leaves its 9 H2 sections
/// individually addressable, with the stray H1s absorbed into the preceding
/// section's body. That is strictly better than the previous behaviour, where
/// nothing in it was addressable at all.
const BOUNDARY_LEVELS: std::ops::RangeInclusive<usize> = 2..=6;

/// Parse `line` as an ATX heading eligible to be a section boundary.
///
/// Returns `(level, text)`. Requires the hashes at column 0 — an indented
/// `" ### Fake"` is body content, not a boundary — and a space after them, so
/// `"##notaheading"` is not one either.
fn heading_at(line: &str) -> Option<(usize, &str)> {
    let hashes = line.len() - line.trim_start_matches('#').len();
    if !BOUNDARY_LEVELS.contains(&hashes) {
        return None;
    }
    line[hashes..].strip_prefix(' ').map(|text| (hashes, text))
}

/// The heading level that acts as the section boundary for `content`: the
/// SHALLOWEST level present among [`BOUNDARY_LEVELS`].
///
/// Picking the shallowest rather than a fixed level is what lets one function
/// serve both conventions in use: a memory written with `##` sections gets `##`
/// boundaries, while one written with `###` sections (and `####` sub-headings
/// nested inside them) keeps `###` boundaries and `####` as body. A fixed `###`
/// was the original bug — it made `sections=` a no-op on the 15 `##`-only
/// memories, 87 headings in total.
/// See docs/issues/archive/2026-07-28-memory-sections-filter-matches-h3-only.md.
fn boundary_level(content: &str) -> Option<usize> {
    content
        .lines()
        .filter_map(|line| heading_at(line).map(|(level, _)| level))
        .min()
}

/// Filter markdown content to only the requested sections.
///
/// The section level is whatever [`boundary_level`] finds — the shallowest
/// heading among [`BOUNDARY_LEVELS`] — so this works on `##`-sectioned and
/// `###`-sectioned memories alike. Headings deeper than the boundary are part of
/// their section's body.
///
/// # Precondition
///
/// `sections` must be non-empty. Enforced via `debug_assert!` (fires in debug
/// builds / `cargo test`; compiled out in `--release`). The caller in
/// `Memory::call` checks `sections.is_empty()` before calling this function.
///
/// # Returns
///
/// Always returns a `FilterResult`. The caller checks `result.matched` to
/// decide whether to return content or a `RecoverableError`.
pub fn filter_sections(content: &str, sections: &[&str]) -> FilterResult {
    debug_assert!(
        !sections.is_empty(),
        "precondition: sections must be non-empty"
    );

    // Which level splits this document. `None` = no eligible heading anywhere, so
    // everything is preamble and nothing can match.
    let boundary = boundary_level(content);

    // --- Parse content into preamble + blocks ---
    // Each block: (normalized_heading, Vec of raw lines including the heading line)
    let mut preamble_lines: Vec<&str> = Vec::new();
    let mut blocks: Vec<(String, Vec<&str>)> = Vec::new();
    let mut in_preamble = true;

    for line in content.lines() {
        // Only a heading AT the boundary level splits; deeper ones fall through to
        // the body arms below, which is what keeps `####` inside its `###`.
        let is_boundary = match (boundary, heading_at(line)) {
            (Some(b), Some((level, _))) => level == b,
            _ => false,
        };

        if is_boundary {
            // Normalize: strip the hashes + one space, trim surrounding whitespace.
            // The raw line is preserved in the block's line vec for output.
            let normalized = heading_at(line)
                .map(|(_, text)| text.trim().to_string())
                .unwrap_or_default();
            blocks.push((normalized, vec![line]));
            in_preamble = false;
        } else if in_preamble {
            preamble_lines.push(line);
        } else if let Some(block) = blocks.last_mut() {
            block.1.push(line);
        }
    }

    // available: normalized heading text of every block, in file order
    let available: Vec<String> = blocks.iter().map(|(h, _)| h.clone()).collect();

    // missing: requested sections with no match, in request order, caller casing
    let missing: Vec<String> = sections
        .iter()
        .filter(|&&s| !blocks.iter().any(|(h, _)| h.eq_ignore_ascii_case(s)))
        .map(|&s| s.to_string())
        .collect();

    // matched_lines: all lines from matching blocks, in file order
    let matched_lines: Vec<&str> = blocks
        .iter()
        .filter(|(h, _)| sections.iter().any(|s| s.eq_ignore_ascii_case(h)))
        .flat_map(|(_, lines)| lines.iter().copied())
        .collect();

    let matched = !matched_lines.is_empty();

    // Reconstruct output: preamble + matched section lines, joined by "\n".
    // Append "\n" if the original content ended with a newline (lines() strips it).
    let output: Vec<&str> = preamble_lines
        .iter()
        .copied()
        .chain(matched_lines)
        .collect();
    let mut result_content = output.join("\n");
    if content.ends_with('\n') {
        result_content.push('\n');
    }

    FilterResult {
        content: result_content,
        matched,
        missing,
        available,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# Language Patterns

Intro line.

### Rust

Rust anti-patterns here.

#### Sub-heading

More Rust content.

### TypeScript

TypeScript patterns here.

### Python

Python patterns here.
";

    #[test]
    fn filter_sections_returns_matching_section() {
        let r = filter_sections(SAMPLE, &["Rust"]);
        assert!(r.matched);
        assert!(r.content.contains("### Rust"), "should include heading");
        assert!(
            r.content.contains("Rust anti-patterns here."),
            "should include body"
        );
        assert!(
            r.content.contains("# Language Patterns"),
            "should include preamble"
        );
        assert!(
            !r.content.contains("### TypeScript"),
            "should exclude TypeScript"
        );
    }

    #[test]
    fn filter_sections_case_insensitive() {
        let r = filter_sections(SAMPLE, &["rust"]);
        assert!(r.matched);
        assert!(r.content.contains("### Rust"));
    }

    #[test]
    fn filter_sections_multiple_sections() {
        let r = filter_sections(SAMPLE, &["Rust", "TypeScript"]);
        assert!(r.matched);
        assert!(r.content.contains("### Rust"));
        assert!(r.content.contains("### TypeScript"));
        assert!(!r.content.contains("### Python"));
        assert!(r.missing.is_empty());
    }

    #[test]
    fn filter_sections_preserves_preamble() {
        let r = filter_sections(SAMPLE, &["Rust"]);
        assert!(r.content.starts_with("# Language Patterns"));
    }

    #[test]
    fn filter_sections_no_match_returns_not_matched() {
        let r = filter_sections(SAMPLE, &["Go"]);
        assert!(!r.matched);
        assert_eq!(r.missing, vec!["Go"]);
        assert_eq!(r.available, vec!["Rust", "TypeScript", "Python"]);
    }

    #[test]
    fn filter_sections_partial_match_returns_missing() {
        // "typescript" matches (case-insensitive); "Go" does not
        let r = filter_sections(SAMPLE, &["Rust", "typescript", "Go"]);
        assert!(r.matched);
        assert!(r.content.contains("### Rust"));
        assert!(r.content.contains("### TypeScript"));
        // missing preserves caller-supplied casing
        assert_eq!(r.missing, vec!["Go"]);
        assert!(
            !r.content.contains("### Python"),
            "unrelated section should be excluded"
        );
    }

    #[test]
    fn filter_sections_duplicate_headings_both_included() {
        let content = "### Rust\n\nFirst block.\n\n### Rust\n\nSecond block.\n";
        let r = filter_sections(content, &["Rust"]);
        assert!(r.matched);
        assert!(r.content.contains("First block."));
        assert!(r.content.contains("Second block."));
        assert_eq!(r.available, vec!["Rust", "Rust"]);
    }

    #[test]
    fn filter_sections_nested_h4_included_in_body() {
        let r = filter_sections(SAMPLE, &["Rust"]);
        assert!(
            r.content.contains("#### Sub-heading"),
            "h4 should be part of section body"
        );
        assert!(r.content.contains("More Rust content."));
    }

    #[test]
    fn filter_sections_heading_whitespace_normalized() {
        // Double space after ### and trailing space
        let content = "###  Rust  \n\nContent.\n";
        let r = filter_sections(content, &["rust"]);
        assert!(r.matched, "should match despite whitespace");
        assert!(
            r.content.contains("Content."),
            "body should be included when matched via whitespace"
        );
        assert_eq!(r.available, vec!["Rust"]);
    }

    #[test]
    fn filter_sections_no_headings_in_file_returns_not_matched() {
        let content = "Just a preamble\nno headings here\n";
        let r = filter_sections(content, &["Rust"]);
        assert!(!r.matched);
        assert!(r.available.is_empty());
        assert_eq!(r.missing, vec!["Rust"]);
    }

    #[test]
    fn filter_sections_indented_heading_not_a_boundary() {
        // Leading space — NOT a section boundary
        let content = "### Real\n\nBody.\n\n ### Fake\n\nNot a section.\n";
        let r = filter_sections(content, &["Real"]);
        assert!(r.matched);
        assert_eq!(r.available, vec!["Real"]);
        // The indented line is part of the "Real" section body
        assert!(r.content.contains(" ### Fake"));
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "precondition")]
    fn filter_sections_empty_sections_is_caller_error() {
        // debug_assert! fires in debug builds (including `cargo test`).
        // This test will NOT catch the precondition violation in `--release` builds.
        filter_sections("### Rust\nContent\n", &[]);
    }

    #[test]
    fn filter_sections_available_in_file_order() {
        let r = filter_sections(SAMPLE, &["Python"]);
        assert_eq!(r.available, vec!["Rust", "TypeScript", "Python"]);
    }

    /// The bug this function was fixed for: a memory sectioned with `##` used to
    /// return `matched: false` and an "no ### sections" hint despite being richly
    /// sectioned. `gotchas.md` shape — H1 title + `##` sections — and the exact
    /// section CLAUDE.md routes agents to by name.
    #[test]
    fn filter_sections_matches_h2_sectioned_memory() {
        // Escaped newlines, not a multi-line literal: `edit_code insert` re-indents
        // the interior of a raw multi-line string, which silently turns column-0
        // headings into indented body and makes this test fail for the wrong reason.
        // See docs/issues/archive/2026-07-28-edit-code-reindent-shifts-string-literal-contents.md.
        let content = "# Gotchas\n\nPreamble line.\n\n## MCP Binary Symlink\n\n\
             `~/.cargo/bin/codescout` is a symlink.\n\n## LSP\n\nKotlin notes here.\n";
        let r = filter_sections(content, &["MCP Binary Symlink"]);
        assert!(r.matched, "## sections must be matchable");
        assert!(r.content.contains("## MCP Binary Symlink"));
        assert!(r.content.contains("is a symlink"));
        assert!(
            !r.content.contains("Kotlin notes"),
            "must exclude other sections"
        );
        assert_eq!(r.available, vec!["MCP Binary Symlink", "LSP"]);
    }

    /// H1 must never be a boundary. Nineteen of twenty-one memories carry exactly
    /// one H1 as their title; treating it as a section would make the title the
    /// only block and nest every real section inside it — filtering would appear
    /// to "work" while always returning the whole document.
    #[test]
    fn filter_sections_h1_title_is_not_a_boundary() {
        let content = "# Title\n\n## Alpha\n\nA body.\n\n## Beta\n\nB body.\n";
        let r = filter_sections(content, &["Beta"]);
        assert!(r.matched);
        assert_eq!(r.available, vec!["Alpha", "Beta"]);
        assert!(r.content.starts_with("# Title"), "title stays as preamble");
        assert!(r.content.contains("B body."));
        assert!(!r.content.contains("A body."));
    }

    /// `development-commands.md`'s shape: H1 used structurally (5 of them) next to
    /// H2 sections. Excluding H1 keeps every H2 addressable; the stray H1s land in
    /// the preceding section's body. Documents the accepted trade rather than
    /// leaving it to be rediscovered as a bug.
    #[test]
    fn filter_sections_multiple_h1_still_addresses_h2_sections() {
        let content = "# One\n\n## Cargo\n\ncargo body.\n\n# Two\n\n## Scripts\n\nscripts body.\n";
        let r = filter_sections(content, &["Scripts"]);
        assert!(r.matched);
        assert_eq!(r.available, vec!["Cargo", "Scripts"]);
        assert!(r.content.contains("scripts body."));
        // The mid-file H1 is absorbed into the preceding H2's body, not dropped.
        let r2 = filter_sections(content, &["Cargo"]);
        assert!(
            r2.content.contains("# Two"),
            "stray H1 rides with its section"
        );
    }

    /// Shallowest-level selection, not "every level is a boundary": in a document
    /// that leads with `##`, a `###` must stay inside its parent rather than
    /// becoming a sibling. This is the case that rules out the naive
    /// "match any heading" fix.
    #[test]
    fn filter_sections_deeper_headings_nest_under_the_boundary_level() {
        let content =
            "## Parent\n\nparent body.\n\n### Child\n\nchild body.\n\n## Other\n\nother.\n";
        let r = filter_sections(content, &["Parent"]);
        assert!(r.matched);
        assert_eq!(
            r.available,
            vec!["Parent", "Other"],
            "### is not a section here"
        );
        assert!(r.content.contains("### Child"), "child heading rides along");
        assert!(r.content.contains("child body."));
        assert!(!r.content.contains("other."));
        // And the child is NOT independently addressable at this boundary level.
        assert!(!filter_sections(content, &["Child"]).matched);
    }

    /// Hashes with no following space are not a heading.
    #[test]
    fn filter_sections_hashes_without_space_are_not_a_boundary() {
        let content = "## Real\n\nBody.\n\n##NotAHeading\n\nStill body.\n";
        let r = filter_sections(content, &["Real"]);
        assert!(r.matched);
        assert_eq!(r.available, vec!["Real"]);
        assert!(r.content.contains("##NotAHeading"));
    }

    /// A memory with only a title and prose has no boundary level at all — the
    /// caller must get `matched: false` with an empty `available`, which is what
    /// drives the "read it without `sections`" hint.
    #[test]
    fn filter_sections_title_only_memory_has_no_sections() {
        let content = "# Reconnaissance\n\nJust prose, no sections.\n";
        let r = filter_sections(content, &["anything"]);
        assert!(!r.matched);
        assert!(r.available.is_empty());
        assert_eq!(r.missing, vec!["anything"]);
    }
}
