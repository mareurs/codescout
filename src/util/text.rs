//! Text processing helpers.

/// Truncate a string to at most `max_chars` characters, appending `…` if cut.
pub fn truncate(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        chars[..max_chars].iter().collect::<String>() + "…"
    }
}

/// Count lines in a string (1-indexed: an empty string has 1 line).
pub fn count_lines(s: &str) -> usize {
    if s.is_empty() {
        1
    } else {
        s.lines().count()
    }
}

/// Extract a line range from text (1-indexed, inclusive). Returns empty string
/// if the range is out of bounds.
pub fn extract_lines(text: &str, start_line: usize, end_line: usize) -> String {
    text.lines()
        .enumerate()
        .filter(|(i, _)| {
            let line = i + 1;
            line >= start_line && line <= end_line
        })
        .map(|(_, line)| line)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_short_unchanged() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_exact_length_unchanged() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn truncate_long_appends_ellipsis() {
        assert_eq!(truncate("hello world", 5), "hello…");
    }

    #[test]
    fn truncate_unicode_counts_chars_not_bytes() {
        // "é" is 2 bytes but 1 char
        assert_eq!(truncate("héllo", 3), "hél…");
    }

    #[test]
    fn count_lines_empty() {
        assert_eq!(count_lines(""), 1);
    }

    #[test]
    fn count_lines_single() {
        assert_eq!(count_lines("hello"), 1);
    }

    #[test]
    fn count_lines_multi() {
        assert_eq!(count_lines("a\nb\nc"), 3);
    }

    #[test]
    fn extract_lines_full_range() {
        assert_eq!(extract_lines("a\nb\nc", 1, 3), "a\nb\nc");
    }

    #[test]
    fn extract_lines_middle() {
        assert_eq!(extract_lines("a\nb\nc\nd\ne", 2, 4), "b\nc\nd");
    }

    #[test]
    fn extract_lines_single() {
        assert_eq!(extract_lines("a\nb\nc", 2, 2), "b");
    }

    #[test]
    fn extract_lines_out_of_bounds_returns_empty() {
        assert_eq!(extract_lines("a\nb", 10, 20), "");
    }

    #[test]
    fn extract_lines_first_line() {
        assert_eq!(extract_lines("first\nsecond\nthird", 1, 1), "first");
    }
}
