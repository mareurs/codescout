//! Shared first-paragraph extractor (see spec "Summary Extractor Rules").

const MAX_SUMMARY_CHARS: usize = 200;

/// Extract the first prose paragraph from a markdown body, trimmed to 200 chars.
///
/// Skips: leading headings (any level), blank lines, lines inside fenced code
/// blocks. Returns an empty string if no prose paragraph exists.
pub fn extract(body: &str) -> String {
    let mut paragraph = String::new();
    let mut in_fence = false;
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if trimmed.is_empty() {
            if paragraph.is_empty() {
                continue;
            } else {
                break;
            }
        }
        // Heading line
        if trimmed.starts_with('#') {
            let hash_count = trimmed.chars().take_while(|c| *c == '#').count();
            if hash_count <= 6 && trimmed.chars().nth(hash_count) == Some(' ') {
                if paragraph.is_empty() {
                    // Skip any leading heading before the first prose line.
                    continue;
                }
                // Heading after prose terminates the paragraph.
                break;
            }
        }
        if !paragraph.is_empty() {
            paragraph.push(' ');
        }
        paragraph.push_str(trimmed);
    }

    let collapsed = paragraph.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_with_ellipsis(&collapsed, MAX_SUMMARY_CHARS)
}

fn truncate_with_ellipsis(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    // Try to trim back to the previous word boundary for nicer output.
    if let Some(last_space) = out.rfind(' ') {
        if last_space > max / 2 {
            out.truncate(last_space);
        }
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_first_paragraph_after_h1() {
        let body = "# Title\n\nFirst paragraph of prose.\n\n## Next\n";
        assert_eq!(extract(body), "First paragraph of prose.");
    }

    #[test]
    fn skips_blank_lines_before_paragraph() {
        let body = "\n\n\nHello there.\n";
        assert_eq!(extract(body), "Hello there.");
    }

    #[test]
    fn collapses_internal_whitespace() {
        let body = "Line one.\nLine two.\n";
        assert_eq!(extract(body), "Line one. Line two.");
    }

    #[test]
    fn truncates_at_200_chars_with_ellipsis() {
        let body = "a".repeat(300);
        let out = extract(&body);
        assert!(out.ends_with('…'));
        assert!(out.chars().count() <= 201); // 200 chars + ellipsis
    }

    #[test]
    fn ignores_lines_inside_fenced_code() {
        let body = "```\ncode block\n```\n\nActual text here.\n";
        assert_eq!(extract(body), "Actual text here.");
    }

    #[test]
    fn empty_body_returns_empty() {
        assert_eq!(extract(""), "");
    }

    #[test]
    fn heading_only_returns_empty() {
        assert_eq!(extract("# Only a heading\n"), "");
    }

    #[test]
    fn stops_at_heading() {
        let body = "First line.\n## Next section\nShould not be included.\n";
        assert_eq!(extract(body), "First line.");
    }

    #[test]
    fn skips_consecutive_leading_headings() {
        // CLAUDE.md pattern: H1 title followed by H2 section then prose.
        let body = "# Title\n\n## Section\n\nActual prose here.\n";
        assert_eq!(extract(body), "Actual prose here.");
    }

    #[test]
    fn skips_leading_h2_without_h1() {
        let body = "## First section\n\nProse.\n";
        assert_eq!(extract(body), "Prose.");
    }

    #[test]
    fn collapses_intra_line_whitespace() {
        let body = "Line  with   multiple\tspaces   here.\n";
        assert_eq!(extract(body), "Line with multiple spaces here.");
    }
}
