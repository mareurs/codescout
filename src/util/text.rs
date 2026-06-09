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

/// Count lines in a string. An empty string has 0 lines.
pub fn count_lines(s: &str) -> usize {
    if s.is_empty() {
        return 0;
    }
    s.lines().count()
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
/// Leading whitespace (indentation) of a line — the prefix before the first
/// non-whitespace character. Empty for an unindented or all-blank line.
pub fn leading_ws(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}

/// The common base indentation of a block: the leading whitespace of the
/// least-indented non-blank line. Blank lines carry no indentation signal and
/// are ignored. Returns `""` for an empty or all-blank block.
///
/// Picking the *minimum* (rather than the first line's indent) keeps re-basing
/// correct even when the first line is more indented than a later one.
pub fn min_indent(block: &str) -> &str {
    block
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(leading_ws)
        .min_by_key(|ws| ws.len())
        .unwrap_or("")
}

/// Re-base an indented block from `agent_base` to `file_base`, preserving the
/// relative (inner) indentation of every line.
///
/// For each non-blank line: strip the `agent_base` prefix if present and prepend
/// `file_base`; for a ragged line that does not start with `agent_base`, fall
/// back to `file_base` + the trimmed line. Blank lines are emitted empty.
pub fn reindent_block(new_string: &str, agent_base: &str, file_base: &str) -> String {
    let mut out = String::with_capacity(new_string.len());
    for (idx, line) in new_string.split('\n').enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        if line.trim().is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix(agent_base) {
            out.push_str(file_base);
            out.push_str(rest);
        } else {
            out.push_str(file_base);
            out.push_str(line.trim_start());
        }
    }
    out
}

/// Re-base a block so its least-indented line sits at `target_base`, preserving
/// inner structure. Returns the block **unchanged** when it is already based at
/// `target_base` — so correctly-indented input is never disturbed (which, in
/// particular, keeps the transform off lines inside multi-line string literals
/// in the common path).
pub fn reindent_to(block: &str, target_base: &str) -> String {
    let agent_base = min_indent(block);
    if agent_base == target_base {
        return block.to_string();
    }
    reindent_block(block, agent_base, target_base)
}

/// Extract lines from `start_line` to `end_line` (1-indexed, inclusive) without
/// exceeding `byte_budget` bytes. Returns `(content, lines_shown, complete)`.
///
/// - `content`: the extracted lines joined with `\n`
/// - `lines_shown`: number of lines included
/// - `complete`: true if all lines in the requested range were included
///
/// **Safety valve:** always includes at least 1 line (even if it exceeds the budget)
/// to prevent infinite retry loops where the agent keeps requesting the same range.
/// Exception: if byte_budget is 0, returns nothing (edge case for testing).
pub fn extract_lines_to_budget(
    text: &str,
    start_line: usize,
    end_line: usize,
    byte_budget: usize,
) -> (String, usize, bool) {
    // Edge case: zero budget returns nothing
    if byte_budget == 0 {
        return ("".to_string(), 0, false);
    }

    let mut result_lines: Vec<&str> = Vec::new();
    let mut bytes_used: usize = 0;
    let mut hit_end = true; // assume complete unless budget breaks us out

    for (i, line) in text.lines().enumerate() {
        let lineno = i + 1;
        if lineno < start_line {
            continue;
        }
        if lineno > end_line {
            break;
        }

        let line_bytes = line.len() + 1; // +1 for the \n join separator
        if bytes_used + line_bytes > byte_budget && !result_lines.is_empty() {
            hit_end = false;
            break;
        }

        result_lines.push(line);
        bytes_used += line_bytes;
    }

    let lines_shown = result_lines.len();
    (result_lines.join("\n"), lines_shown, hit_end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_short_unchanged() {
        assert_eq!(truncate("hello", 10), "hello");
    }
    #[test]
    fn leading_ws_extracts_indent() {
        assert_eq!(leading_ws("    x"), "    ");
        assert_eq!(leading_ws("\t\tx"), "\t\t");
        assert_eq!(leading_ws("x"), "");
        assert_eq!(leading_ws("   "), "   ");
    }

    #[test]
    fn min_indent_picks_least_indented_nonblank() {
        // signature at 4, body at 8 -> base is the shallower 4.
        assert_eq!(min_indent("    def f():\n        return 1"), "    ");
        // blank lines carry no signal and are ignored.
        assert_eq!(min_indent("\n    a\n\n        b\n"), "    ");
        // all-blank / empty -> "".
        assert_eq!(min_indent("\n  \n"), "");
        assert_eq!(min_indent(""), "");
    }

    #[test]
    fn reindent_to_shifts_dedented_body_to_target() {
        // The reported bug: a method body dedented to column 0, re-based into a
        // class at column 4 — the inner +4 step is preserved (lands at 8).
        let body = "def method(self):\n    return self.x";
        let out = reindent_to(body, "    ");
        assert_eq!(out, "    def method(self):\n        return self.x");
    }

    #[test]
    fn reindent_to_noop_when_already_based() {
        let body = "    def method(self):\n        return self.x";
        // Already at the target column -> returned byte-for-byte unchanged.
        assert_eq!(reindent_to(body, "    "), body);
    }

    #[test]
    fn reindent_to_dedents_when_target_shallower() {
        let body = "        a = 1\n            b = 2";
        assert_eq!(reindent_to(body, ""), "a = 1\n    b = 2");
    }

    #[test]
    fn reindent_to_preserves_blank_lines() {
        let body = "a\n\nb";
        assert_eq!(reindent_to(body, "  "), "  a\n\n  b");
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
        assert_eq!(count_lines(""), 0);
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

    #[test]
    fn extract_lines_to_budget_fits_all() {
        let text = "short\nlines\nhere\n";
        let (content, lines_shown, complete) = extract_lines_to_budget(text, 1, 100, 10_000);
        assert_eq!(lines_shown, 3);
        assert!(complete);
        assert_eq!(content, "short\nlines\nhere");
    }

    #[test]
    fn extract_lines_to_budget_truncates_at_budget() {
        // Each line is 10 bytes ("line NNNN\n"). Budget of 25 bytes fits 2 full lines.
        let text: String = (1..=10).map(|i| format!("line {:04}\n", i)).collect();
        let (content, lines_shown, complete) = extract_lines_to_budget(&text, 1, 100, 25);
        assert_eq!(lines_shown, 2);
        assert!(!complete);
        assert_eq!(content, "line 0001\nline 0002");
    }

    #[test]
    fn extract_lines_to_budget_respects_start_line() {
        let text = "aaa\nbbb\nccc\nddd\neee\n";
        let (content, lines_shown, complete) = extract_lines_to_budget(text, 3, 100, 10_000);
        assert_eq!(lines_shown, 3); // lines 3, 4, 5
        assert!(complete);
        assert_eq!(content, "ccc\nddd\neee");
    }

    #[test]
    fn extract_lines_to_budget_respects_end_line() {
        let text = "aaa\nbbb\nccc\nddd\neee\n";
        let (content, lines_shown, complete) = extract_lines_to_budget(text, 2, 4, 10_000);
        assert_eq!(lines_shown, 3); // lines 2, 3, 4
        assert!(complete); // all requested lines fit
        assert_eq!(content, "bbb\nccc\nddd");
    }

    #[test]
    fn extract_lines_to_budget_budget_hit_before_end_line() {
        // Request lines 1-100 but budget only fits ~2 lines
        let text: String = (1..=100).map(|i| format!("line {:04}\n", i)).collect();
        let (content, lines_shown, complete) = extract_lines_to_budget(&text, 1, 100, 25);
        assert_eq!(lines_shown, 2);
        assert!(!complete);
        assert_eq!(content, "line 0001\nline 0002");
    }

    #[test]
    fn extract_lines_to_budget_zero_budget_returns_nothing() {
        let text = "aaa\nbbb\n";
        let (content, lines_shown, complete) = extract_lines_to_budget(text, 1, 100, 0);
        assert_eq!(lines_shown, 0);
        assert!(!complete);
        assert_eq!(content, "");
    }

    #[test]
    fn extract_lines_to_budget_single_line_exceeds_budget() {
        // A single very long line — must still return at least 1 line if budget > 0
        // to avoid infinite loops (agent would retry same range forever).
        let text = "a".repeat(1000);
        let (content, lines_shown, complete) = extract_lines_to_budget(&text, 1, 1, 50);
        assert_eq!(lines_shown, 1);
        // complete = true because we reached end_line, even though it exceeded budget
        assert!(complete);
        assert_eq!(content.len(), 1000);
    }

    #[test]
    fn extract_lines_to_budget_empty_text() {
        let (content, lines_shown, complete) = extract_lines_to_budget("", 1, 100, 10_000);
        assert_eq!(lines_shown, 0);
        assert!(complete); // no lines to show, so "all" lines were shown
        assert_eq!(content, "");
    }

    #[test]
    fn extract_lines_to_budget_start_beyond_total() {
        let text = "aaa\nbbb\nccc\n";
        let (content, lines_shown, complete) = extract_lines_to_budget(text, 500, 600, 10_000);
        assert_eq!(lines_shown, 0);
        assert!(complete); // no lines in range, nothing to show
        assert_eq!(content, "");
    }
}
