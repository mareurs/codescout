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

/// What the literal scanner is in the middle of.
enum Scan {
    Code,
    /// A `"`/`'` literal opened on the current line, not yet known to span lines.
    Quoted(char),
    /// A literal confirmed to run until `close` appears — however many lines that
    /// takes. `escapes` is false only for raw strings, where `\` is data and must
    /// not hide the closing token.
    Spanning {
        close: String,
        escapes: bool,
    },
}

/// A line-spanning literal opener at the start of `rest`: the bytes it consumes
/// and the state it puts the scanner in.
fn spanning_opener(rest: &str) -> Option<(usize, Scan)> {
    for triple in ["\"\"\"", "'''"] {
        if rest.starts_with(triple) {
            return Some((
                triple.len(),
                Scan::Spanning {
                    close: triple.to_string(),
                    escapes: true,
                },
            ));
        }
    }
    if rest.starts_with('`') {
        return Some((
            1,
            Scan::Spanning {
                close: "`".to_string(),
                escapes: true,
            },
        ));
    }
    // Rust raw string: r"…", r#"…"#, br##"…"##. The close token carries the same
    // number of hashes the opener used.
    let after_prefix = rest.strip_prefix("br").or_else(|| rest.strip_prefix('r'))?;
    let hashes = after_prefix.len() - after_prefix.trim_start_matches('#').len();
    if !after_prefix[hashes..].starts_with('"') {
        return None;
    }
    Some((
        rest.len() - after_prefix.len() + hashes + 1,
        Scan::Spanning {
            close: format!("\"{}", "#".repeat(hashes)),
            escapes: false,
        },
    ))
}

/// Advance the scanner across one line and report the state it ends in.
fn scan_line(line: &str, entry: Scan) -> Scan {
    let mut state = entry;
    let mut i = 0;
    while i < line.len() {
        let rest = &line[i..];
        let one_char = rest.chars().next().map_or(1, char::len_utf8);
        // Width of a `\x` escape: the backslash plus whatever follows it, or just the
        // backslash when it ends the line. Lazy because `rest[1..]` is only a valid
        // boundary once the leading byte is known to be the one-byte `\`.
        let escape = || 1 + rest[1..].chars().next().map_or(0, char::len_utf8);
        match &state {
            Scan::Spanning { close, escapes } => {
                let escapes = *escapes;
                let consumed = rest
                    .strip_prefix(close.as_str())
                    .map(|after| rest.len() - after.len());
                match consumed {
                    Some(n) => {
                        i += n;
                        state = Scan::Code;
                    }
                    None if escapes && rest.starts_with('\\') => i += escape(),
                    None => i += one_char,
                }
            }
            Scan::Quoted(quote) => {
                let quote = *quote;
                if rest.starts_with('\\') {
                    // Skip the escape and what it escapes, so `\"` does not close.
                    i += escape();
                } else {
                    if rest.starts_with(quote) {
                        state = Scan::Code;
                    }
                    i += one_char;
                }
            }
            Scan::Code => {
                if rest.starts_with("//") {
                    // Nothing in a line comment can open a literal. Bailing here is
                    // what keeps markdown backticks in a doc comment from opening a
                    // phantom line-spanning literal.
                    break;
                }
                if let Some((consumed, opened)) = spanning_opener(rest) {
                    i += consumed;
                    state = opened;
                } else if rest.starts_with('"') || rest.starts_with('\'') {
                    state = Scan::Quoted(rest.chars().next().unwrap_or('"'));
                    i += one_char;
                } else {
                    i += one_char;
                }
            }
        }
    }
    match state {
        // A `"` still open at end of line has reached the next line one of two ways:
        // a raw newline, which Rust permits inside `"…"` and which is the commonest
        // multi-line fixture shape there, or a trailing `\` continuation. A `'` can
        // only ever do the latter — no language spans lines with an unescaped `'…'`.
        // Either way the literal is now *confirmed* multi-line, so it runs to its
        // closing quote instead of resetting at every subsequent end of line.
        Scan::Quoted(quote) if quote == '"' || line.ends_with('\\') => Scan::Spanning {
            close: quote.to_string(),
            escapes: true,
        },
        // A lone `'` is a lifetime (`&'a T`) or an apostrophe in prose, not a literal.
        // Resetting bounds the misreading to the line it appeared on.
        Scan::Quoted(_) => Scan::Code,
        other => other,
    }
}

/// Which lines of `block` begin inside a string literal opened on an earlier
/// line — meaning that line's leading whitespace is part of the string's
/// *value*, not code indentation. Reindenting such a line changes what the
/// program says while leaving the surrounding code looking correctly formatted,
/// and rustfmt's default `format_strings = false` will not undo it.
///
/// Deliberately a scanner, not a parser: the `reindent_*` helpers run on
/// fragments in every language `edit_code` supports, so there is no tree to
/// consult. Recognised line-spanning literals, each closed by the token that
/// opened it:
///
/// - a `"` literal left open at end of line, whether by a raw newline (Rust
///   permits those inside `"…"`) or a trailing `\` continuation
/// - a `'` literal held open by a trailing `\` (C, shell)
/// - a triple-quoted Python literal
/// - a backtick literal (JS/TS templates, Go raw strings)
/// - a Rust raw string, at any hash count
///
/// Where the scanner cannot tell, it errs toward calling a line code, because a
/// mis-indented code line is a loud failure — compiler, formatter, review — and
/// a mutated string literal is a silent one. That is why a lone `'` resets at
/// end of line rather than latching: a lifetime or an apostrophe in prose is far
/// likelier than a `'…'` spanning lines, which no supported language allows
/// unescaped. An unbalanced `"` is the opposite bet, and deliberately so — `//`
/// comments open nothing, so outside them an unclosed `"` is much likelier a
/// multi-line literal than a typo.
///
/// The worst case is a line-spanning literal that never closes: it masks every
/// line after the opener, so at most that one line is reindented and the rest
/// comes back byte-for-byte. Known blind spot: a `/* … */` block comment is not
/// tracked, so an unbalanced `"` or an odd backtick count inside one lands in
/// that worst case.
fn literal_continuation_mask(block: &str) -> Vec<bool> {
    let mut mask = Vec::with_capacity(block.split('\n').count());
    let mut state = Scan::Code;
    for line in block.split('\n') {
        mask.push(!matches!(state, Scan::Code));
        state = scan_line(line, state);
    }
    mask
}

/// [`min_indent`], blind to lines that are string-literal content. Their
/// leading whitespace is not indentation, and letting it set the base is what
/// defeats [`reindent_to`]'s no-op guard: a literal whose interior sits at
/// column 0 makes an already-correctly-indented block look dedented, so every
/// line shifts — including the literal that caused it.
///
/// The first line can never be a continuation, so the fallback to [`min_indent`]
/// is reachable only for an empty or all-blank block, where both agree on `""`.
fn min_indent_outside_literals<'a>(block: &'a str, mask: &[bool]) -> &'a str {
    block
        .split('\n')
        .zip(mask.iter().copied())
        .filter(|(line, masked)| !*masked && !line.trim().is_empty())
        .map(|(line, _)| leading_ws(line))
        .min_by_key(|ws| ws.len())
        .unwrap_or_else(|| min_indent(block))
}

/// Re-base an indented block from `agent_base` to `file_base`, preserving the
/// relative (inner) indentation of every line.
///
/// For each non-blank line: strip the `agent_base` prefix if present and prepend
/// `file_base`; for a ragged line that does not start with `agent_base`, fall
/// back to `file_base` + the trimmed line. Blank lines are emitted empty.
///
/// Lines that are string-literal continuations are emitted **verbatim** — their
/// leading whitespace is part of the string's value, so shifting it would edit
/// the program's data while looking like a formatting change. See
/// [`literal_continuation_mask`].
pub fn reindent_block(new_string: &str, agent_base: &str, file_base: &str) -> String {
    let mask = literal_continuation_mask(new_string);
    let mut out = String::with_capacity(new_string.len());
    for (idx, line) in new_string.split('\n').enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        if mask.get(idx).copied().unwrap_or(false) {
            out.push_str(line);
            continue;
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
/// `target_base` — so correctly-indented input is never disturbed.
///
/// The base is measured over code lines only. Measuring it over every line lets
/// a multi-line string literal whose interior sits at column 0 report the block
/// as dedented, which defeats this no-op guard and shifts the literal's contents
/// along with the code. [`reindent_block`] then leaves any literal continuation
/// in place, so a block that genuinely does need shifting keeps its strings
/// intact too.
pub fn reindent_to(block: &str, target_base: &str) -> String {
    let mask = literal_continuation_mask(block);
    let agent_base = min_indent_outside_literals(block, &mask);
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

    // Every fixture below uses `\n`-escaped strings rather than multi-line literals.
    // That is deliberate: a multi-line literal here would be re-indented by the very
    // defect these tests pin, so writing one would corrupt the fixture on the way in.

    #[test]
    fn reindent_to_leaves_multi_line_literal_contents_alone() {
        // The reported bug. The code lines already sit at the target column, but the
        // literal's interior is at column 0 — measuring the base over every line
        // reported the whole block as dedented and shifted the string's *value* by 4.
        let body = "    fn t() {\n        let content = \"\\\n# Gotchas\n\n## MCP Binary Symlink\n\";\n        assert!(content.starts_with('#'));\n    }";
        assert_eq!(
            reindent_to(body, "    "),
            body,
            "a block whose code is already based at the target must come back byte-for-byte"
        );
    }

    #[test]
    fn reindent_to_shifts_code_and_leaves_the_literal_where_it_was() {
        // The case option 1 in the bug file would not have fixed: the code genuinely
        // does need shifting, and the literal must sit out the shift.
        let body = "fn t() {\n    let s = \"\\\nraw line\n\";\n}";
        let out = reindent_to(body, "    ");
        assert!(
            out.starts_with("    fn t() {\n        let s ="),
            "code lines shift: {out:?}"
        );
        assert!(
            out.contains("\nraw line\n"),
            "literal keeps its value: {out:?}"
        );
        assert!(
            !out.contains("    raw line"),
            "literal must not gain indentation: {out:?}"
        );
        assert!(
            out.contains("\n\";\n"),
            "the closing line is literal content too: {out:?}"
        );
    }

    #[test]
    fn literal_continuation_mask_covers_each_line_spanning_form() {
        // Python triple-quote.
        assert_eq!(
            literal_continuation_mask("s = \"\"\"\nbody\n\"\"\"\nx = 1"),
            vec![false, true, true, false]
        );
        // JS/TS template, Go raw string.
        assert_eq!(
            literal_continuation_mask("let t = `a\nb`;"),
            vec![false, true]
        );
        // Rust raw string, hashed.
        assert_eq!(
            literal_continuation_mask("let r = r#\"\nline\n\"#;"),
            vec![false, true, true]
        );
        // Backslash-continued literal: confirmed multi-line, so it runs to its
        // closing quote instead of resetting at the next end of line.
        assert_eq!(
            literal_continuation_mask("let s = \"\\\none\ntwo\n\";"),
            vec![false, true, true, true]
        );
    }

    #[test]
    fn literal_continuation_mask_covers_a_raw_newline_double_quoted_literal() {
        // Rust permits a raw newline inside `"…"`, with no trailing `\`, and that is how
        // a multi-line fixture is usually written — the commonest shape, and the one the
        // original bug report did not use. Treating an unclosed `"` at end of line as
        // prose would leave exactly this form unprotected.
        assert_eq!(
            literal_continuation_mask("let c = \"\n# Gotchas\n\n## Section\n\";"),
            vec![false, true, true, true, true]
        );
        // The opener line is never masked (its indent is real code) and the closing line
        // always is (the bytes before its quote are still string content).
        let block = "    let c = \"\n# Gotchas\n\";";
        assert_eq!(
            reindent_to(block, "        "),
            "        let c = \"\n# Gotchas\n\";"
        );
    }

    #[test]
    fn literal_continuation_mask_does_not_latch_on_prose_quotes() {
        // A lone lifetime tick leaves the scanner mid-quote at end of line. Latching
        // there would mask the rest of the block and suppress every shift after it.
        assert_eq!(
            literal_continuation_mask("fn f<'a>(x: u8) {\n    body()\n}"),
            vec![false, false, false]
        );
        // A line comment cannot open a literal — otherwise the odd backtick count in
        // a markdown-flavoured doc comment would mask everything below it.
        assert_eq!(
            literal_continuation_mask("// see `a` and `b\n    body()"),
            vec![false, false]
        );
    }

    #[test]
    fn literal_continuation_mask_honours_escapes_inside_a_continued_literal() {
        // Without escape handling the `\"` would read as the closing quote, the mask
        // would end early, and the literal's own closing line would get shifted.
        assert_eq!(
            literal_continuation_mask("let s = \"\\\nhe said \\\"hi\\\" today\n\";"),
            vec![false, true, true]
        );
        // A raw string is the opposite: `\` is data there, so it must not hide `\"#`.
        assert_eq!(
            literal_continuation_mask("let r = r#\"\nc:\\path\n\"#;"),
            vec![false, true, true]
        );
    }

    #[test]
    fn reindent_block_emits_literal_continuations_verbatim() {
        // edit_file's whitespace-normalized-match repair calls this directly with its
        // own bases. Its post-edit syntax check cannot catch a mutated literal, since
        // the shifted result still parses.
        //
        // Both fixtures below are real multi-line literals rather than `\n`-escapes,
        // and that is the point: edit_code wrote them through the very reindent this
        // module fixes. If it still shifted literal interiors, `col 0` would have
        // arrived indented and this assertion would fail rather than pass quietly.
        let block = "    let s = \"\\
col 0
\";";
        assert_eq!(
            reindent_block(block, "    ", "        "),
            "        let s = \"\\
col 0
\";"
        );
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
