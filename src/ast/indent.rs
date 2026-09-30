//! Python indentation errors that tree-sitter's error recovery does not flag.
//!
//! [`crate::ast::has_syntax_errors`] asks tree-sitter whether the tree contains an ERROR
//! node. For Python that is blind to the three indentation errors the *tokenizer* raises:
//! a dedent to a column no enclosing block has, a tab/space mix whose reading depends on
//! tab width, and the dedent variant of that mix. All three make CPython refuse the file at
//! import, and none of them is an ERROR node — measured 2026-09-30 against `python3`'s own
//! `compile()`. See
//! `docs/issues/2026-09-30-edit-code-syntax-guard-accepts-python-indentation-errors-tree-sitter-does-not-flag.md`.
//!
//! This is a *reimplementation of the tokenizer's indentation rule*, not a parser. It
//! answers only "would CPython's tokenizer reject this file's indentation?", and it states
//! the limits of that answer instead of guessing past them:
//!
//! - it does not report the parser-level errors (`unexpected indent`, `expected an indented
//!   block`) — tree-sitter does flag those, so this check would only duplicate it;
//! - it knows `'`/`"` strings (single and triple), `#` comments, bracket depth and a
//!   trailing backslash, which are the only things that make a physical line *not* start a
//!   logical line. It does **not** model 3.12's nested same-quote f-strings, so a file that
//!   relies on them can be mis-scanned. [`crate::symbol::edit::syntax_regressed`] only
//!   asks about a file that was clean *before* the edit, which keeps a mis-scan from
//!   refusing an edit unless the edit itself changed what the scanner sees.

/// Which rule of the tokenizer's indentation logic rejected the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentationError {
    /// A dedent landed on a column that no enclosing block sits at
    /// (`IndentationError: unindent does not match any outer indentation level`).
    UnindentMatchesNoLevel { line: usize },
    /// The same block is indented with tabs in one place and spaces in another, so its
    /// depth depends on the tab width (`TabError: inconsistent use of tabs and spaces`).
    InconsistentTabs { line: usize },
}

/// Tab stop the interpreter uses for its primary column, and the one it compares against
/// to detect a tab/space mix: with tab width 1 the mix must read the same way.
const TAB_SIZE: usize = 8;

/// One entry of the tokenizer's indentation stack: the column with tabs at [`TAB_SIZE`],
/// and the "alternate" column with tabs counting as one.
#[derive(Clone, Copy)]
struct Level {
    col: usize,
    alt: usize,
}

/// Where a string opened in an earlier physical line is still open.
#[derive(Clone, Copy)]
struct OpenString {
    quote: char,
    triple: bool,
}

/// The first indentation error the tokenizer would raise for `source`, if any.
pub fn first_indentation_error(source: &str) -> Option<IndentationError> {
    let mut levels = vec![Level { col: 0, alt: 0 }];
    let mut depth = 0usize;
    let mut string: Option<OpenString> = None;
    let mut continued = false;

    for (idx, raw) in source.split('\n').enumerate() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let number = idx + 1;

        // Only the first physical line of a logical line carries indentation. A line inside
        // brackets, inside a string, or after a backslash continuation is not one.
        let starts_logical = depth == 0 && string.is_none() && !continued;
        continued = false;

        let mut body = line;
        if starts_logical {
            let (col, alt, rest) = measure_indent(line);
            // Blank and comment-only lines never change the indentation.
            if rest.is_empty() || rest.starts_with('#') {
                continue;
            }
            if let Some(err) = apply_indent(&mut levels, col, alt, number) {
                return Some(err);
            }
            body = rest;
        }

        continued = scan_line(body, &mut depth, &mut string);
    }
    None
}

/// `true` when the tokenizer would reject `source`'s indentation. Python only: any other
/// language answers `false`, because nothing is asserted about a grammar this does not model.
pub fn has_indentation_errors(source: &str, lang: &str) -> bool {
    lang == "python" && first_indentation_error(source).is_some()
}

/// Leading whitespace of `line` as `(col, alt_col, rest)`. A form feed resets both columns,
/// as in the tokenizer.
fn measure_indent(line: &str) -> (usize, usize, &str) {
    let (mut col, mut alt) = (0usize, 0usize);
    for (i, c) in line.char_indices() {
        match c {
            ' ' => {
                col += 1;
                alt += 1;
            }
            '\t' => {
                col = (col / TAB_SIZE + 1) * TAB_SIZE;
                alt += 1;
            }
            '\x0c' => {
                col = 0;
                alt = 0;
            }
            _ => return (col, alt, &line[i..]),
        }
    }
    (col, alt, "")
}

/// The tokenizer's indent/dedent rule for one logical line, mutating the level stack.
fn apply_indent(
    levels: &mut Vec<Level>,
    col: usize,
    alt: usize,
    line: usize,
) -> Option<IndentationError> {
    let top = *levels
        .last()
        .expect("the stack always holds the column-0 level");
    if col == top.col {
        if alt != top.alt {
            return Some(IndentationError::InconsistentTabs { line });
        }
    } else if col > top.col {
        if alt <= top.alt {
            return Some(IndentationError::InconsistentTabs { line });
        }
        levels.push(Level { col, alt });
    } else {
        // The column-0 level is never popped: no `usize` column is below 0, so `landed`
        // below always exists. (A `levels.len() > 1` guard here was measured inert.)
        while levels.last().is_some_and(|l| col < l.col) {
            levels.pop();
        }
        let landed = *levels
            .last()
            .expect("the stack always holds the column-0 level");
        if col != landed.col {
            return Some(IndentationError::UnindentMatchesNoLevel { line });
        }
        if alt != landed.alt {
            return Some(IndentationError::InconsistentTabs { line });
        }
    }
    None
}

/// Walk one physical line's text, updating bracket depth and string state. Returns whether
/// the line ends in a backslash continuation.
fn scan_line(body: &str, depth: &mut usize, string: &mut Option<OpenString>) -> bool {
    let chars: Vec<char> = body.chars().collect();
    let mut i = 0usize;
    // Set when a backslash is the last character while a string is open: the string, even a
    // single-quoted one, then continues onto the next physical line.
    let mut string_escapes_newline = false;

    while i < chars.len() {
        let c = chars[i];
        if let Some(open) = *string {
            if c == '\\' {
                if i + 1 >= chars.len() {
                    string_escapes_newline = true;
                }
                i += 2;
                continue;
            }
            if c == open.quote {
                if !open.triple {
                    *string = None;
                } else if chars.get(i + 1) == Some(&open.quote)
                    && chars.get(i + 2) == Some(&open.quote)
                {
                    *string = None;
                    i += 2;
                }
            }
            i += 1;
            continue;
        }
        match c {
            '#' => break,
            '\'' | '"' => {
                let triple = chars.get(i + 1) == Some(&c) && chars.get(i + 2) == Some(&c);
                *string = Some(OpenString { quote: c, triple });
                i += if triple { 3 } else { 1 };
                continue;
            }
            '(' | '[' | '{' => *depth += 1,
            ')' | ']' | '}' => *depth = depth.saturating_sub(1),
            '\\' if i + 1 == chars.len() => return true,
            _ => {}
        }
        i += 1;
    }

    // A single-quoted string cannot span lines unless its newline was escaped.
    if let Some(open) = *string {
        if !open.triple && !string_escapes_newline {
            *string = None;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(source: &str) -> Option<IndentationError> {
        first_indentation_error(source)
    }

    /// Every rule of the tokenizer's indentation logic that can reject a file, one source per
    /// rule. Expectations are what `python3`'s `compile()` reported for the same text
    /// (2026-09-30): the first three are `IndentationError: unindent does not match any
    /// outer indentation level`, the rest `TabError: inconsistent use of tabs and spaces`.
    /// One case per rule, so a mutation of one rule cannot hide behind another's case.
    #[test]
    fn each_rule_the_tokenizer_rejects_is_named_on_its_own_input() {
        use IndentationError::*;
        // Dedent to a column between two levels (levels 0 and 8, line at 6).
        assert_eq!(
            err("if x:\n        a\n      b\n"),
            Some(UnindentMatchesNoLevel { line: 3 })
        );
        // The shape edit_code produced: `def` at 4, body at 12, then a line at 8 — no
        // enclosing block sits at 8 (levels 0, 4, 12).
        assert_eq!(
            err("class C:\n    def b(self):\n            return 2\n        x = 1\n"),
            Some(UnindentMatchesNoLevel { line: 4 })
        );
        // Dedent past several levels and still landing on none.
        assert_eq!(
            err("if a:\n    if b:\n        if c:\n            d\n  e\n"),
            Some(UnindentMatchesNoLevel { line: 5 })
        );
        // Same column as the block, reached with a tab instead of spaces (`col == top`).
        assert_eq!(
            err("if x:\n        a\n\tb\n"),
            Some(InconsistentTabs { line: 3 })
        );
        // Deeper column, but with tab width 1 it is not deeper (`col > top`, `alt <= top`).
        assert_eq!(
            err("if x:\n        a\n\t\tb\n"),
            Some(InconsistentTabs { line: 3 })
        );
        // Dedent landing on an existing level whose alternate column disagrees.
        assert_eq!(
            err("if x:\n        if y:\n                z\n\tw\n"),
            Some(InconsistentTabs { line: 4 })
        );
    }

    /// Valid Python must read clean — the controls that keep the rules above from being
    /// satisfied by a scanner that flags everything.
    #[test]
    fn consistent_indentation_is_clean() {
        assert_eq!(err(""), None);
        assert_eq!(err("x = 1\n"), None);
        assert_eq!(
            err("class C:\n    def a(self):\n        return 1\n\n    def b(self):\n        return 2\n"),
            None
        );
        // Dedenting several levels at once onto an enclosing one.
        assert_eq!(err("if a:\n    if b:\n        c\nd\n"), None);
        // A file indented consistently with tabs is valid (col and alt columns agree on
        // every comparison), so a check that merely disliked tabs would refuse it.
        assert_eq!(err("if x:\n\ta\n\tif y:\n\t\tb\n\tc\n"), None);
        // Two spellings of one level. Seven spaces then a tab reaches column 8 with
        // alternate column 8 — exactly what eight spaces give — so the tokenizer accepts it
        // (verified with `python3`). A tab counted as more than one alternate column would
        // call this a tab error.
        assert_eq!(err("if x:\n        a\n       \tb\n"), None);
        // Blank lines, whitespace-only lines and comment-only lines at odd columns never
        // change the indentation.
        assert_eq!(
            err("def f():\n    a\n  \n\n# note\n        # deeper note\n    b\n"),
            None
        );
    }

    /// A physical line that continues a logical line carries no indentation of its own.
    /// Each source below would be rejected if its middle line were read as a logical line
    /// (column 2 against levels 0 and 4), so a clean result proves the scanner tracked
    /// the construct that makes it a continuation.
    #[test]
    fn continuation_lines_carry_no_indentation() {
        // Inside brackets.
        assert_eq!(err("def f():\n    x = g(1,\n  2)\n    y = 1\n"), None);
        assert_eq!(err("def f():\n    x = [\n  1,\n  2,\n]\n    y = 1\n"), None);
        // Inside a triple-quoted string.
        assert_eq!(
            err("def f():\n    s = \"\"\"a\n  b\n\"\"\"\n    return s\n"),
            None
        );
        assert_eq!(err("def f():\n    s = '''a\n  b'''\n    return s\n"), None);
        // After a backslash.
        assert_eq!(err("def f():\n    x = 1 + \\\n  2\n    return x\n"), None);
        // A triple-quoted string survives a quote, two quotes, and an escaped quote inside
        // it — only three unescaped quotes end it. Each source has a column-2 line after the
        // quote that would be rejected if the string had been closed early.
        assert_eq!(
            err("def f():\n    s = \"\"\"a \"quoted\" b\n  c\n\"\"\"\n    return s\n"),
            None
        );
        assert_eq!(
            err("def f():\n    s = \"\"\"a \"\" b\n  c\n\"\"\"\n    return s\n"),
            None
        );
        // A quote, one character, a quote: two quotes with a gap are not a terminator.
        assert_eq!(
            err("def f():\n    s = \"\"\"he said \"a\" ok\n  c\n\"\"\"\n    return s\n"),
            None
        );
        assert_eq!(
            err("def f():\n    s = \"\"\"a \\\"\"\" b\n  c\n\"\"\"\n    return s\n"),
            None
        );
        // A single-quoted string whose newline was escaped continues onto the next line.
        assert_eq!(err("def f():\n    s = 'a\\\n  b'\n    return s\n"), None);
    }

    /// The inverse of the test above: an error that comes AFTER a bracket, quote or `#`
    /// that must not be counted is still found. A scanner that let a comment's `(` or a
    /// string's `)` move the bracket depth would skip the bad line and pass these as clean.
    #[test]
    fn a_comment_or_string_does_not_hide_the_error_after_it() {
        use IndentationError::*;
        let bad = |head: &str| format!("def f():\n    {head}\n  y = 2\n");
        // An opening bracket inside a comment.
        assert_eq!(
            err(&bad("x = 1  # (")),
            Some(UnindentMatchesNoLevel { line: 3 })
        );
        // A quote inside a comment must not open a string.
        assert_eq!(
            err(&bad("x = 1  # it's \"")),
            Some(UnindentMatchesNoLevel { line: 3 })
        );
        // Brackets inside a string do not move the depth.
        assert_eq!(
            err(&bad("x = ')('")),
            Some(UnindentMatchesNoLevel { line: 3 })
        );
        // A closed triple-quoted string on one line leaves no string open.
        assert_eq!(
            err(&bad("x = \"\"\"a\"\"\"")),
            Some(UnindentMatchesNoLevel { line: 3 })
        );
        // The last two pin the scanner's own state handling, not a CPython verdict: CPython
        // stops earlier, at line 2, with a different `SyntaxError` (`unmatched ')'`,
        // `unterminated string literal`), which tree-sitter flags too. What they keep is that
        // a stray `)` cannot underflow the depth and an unterminated `'` cannot swallow the
        // lines after it.
        // A closing bracket the depth never saw must not underflow it.
        assert_eq!(
            err(&bad("x = 1)")),
            Some(UnindentMatchesNoLevel { line: 3 })
        );
        // An unterminated single-quoted string ends at the newline.
        assert_eq!(
            err(&bad("x = 'oops")),
            Some(UnindentMatchesNoLevel { line: 3 })
        );
    }

    /// Two details of how the tokenizer measures a line, each pinned by an input that flips
    /// the verdict when the detail is dropped.
    #[test]
    fn form_feed_resets_the_column_and_a_carriage_return_is_not_indentation() {
        // A form feed after two spaces restarts the count, so `b` sits at column 0.
        assert_eq!(err("if x:\n    a\n  \x0cb\n"), None);
        // CRLF: the whitespace-only line `  \r\n` is blank, not a logical line at column 2.
        assert_eq!(err("def f():\r\n    x\r\n  \r\n    y\r\n"), None);
        // And an error is still found in a CRLF file.
        assert_eq!(
            err("def f():\r\n    x = 1\r\n  y\r\n"),
            Some(IndentationError::UnindentMatchesNoLevel { line: 3 })
        );
    }

    /// The check answers for Python and for nothing else: a Python-shaped error under any
    /// other language name is not this check's to assert.
    #[test]
    fn only_python_is_judged() {
        let bad = "if x:\n        a\n      b\n";
        assert!(has_indentation_errors(bad, "python"));
        assert!(!has_indentation_errors(bad, "rust"));
        assert!(!has_indentation_errors(bad, "javascript"));
        assert!(!has_indentation_errors(bad, ""));
        assert!(!has_indentation_errors("if x:\n    a\n", "python"));
    }
}
