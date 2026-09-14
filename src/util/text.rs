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
///
/// A thin wrapper over [`scan_line_into`] that discards the mask. `["//"]` is the
/// line-comment set its reindent callers have always assumed.
fn scan_line(line: &str, entry: Scan) -> Scan {
    scan_line_into(line, entry, &["//"], None)
}

/// The byte written in place of every non-code byte by [`scan_line_into`].
///
/// **Not a space, and that is the whole point.** `find_def_keyword`'s needles carry a
/// TRAILING SPACE to supply their right word boundary (`"class "`, `"fn "`), so while the
/// filler was a space the mask could satisfy a needle the source never contained: a
/// possessive read as a string opener blanked `class's` to `class` + spaces, and the guard
/// then refused an edit naming a keyword the caller could search for and never find. A
/// filler that cannot appear in a needle makes that unrepresentable rather than policed.
///
/// Requirements, all load-bearing — check them before substituting anything else: one byte
/// in UTF-8, so the per-line byte-length guarantee survives; absent from every needle;
/// neither alphanumeric nor `_`, so a left word-boundary test still sees a boundary; and
/// not whitespace, so `trim_start()` in a caller's line-leading comment filter behaves as
/// it did.
const MASK_FILL: char = '\0';

/// The scanner body, shared by [`scan_line`] and [`blank_non_code`] so the two can
/// never disagree about where a literal begins or ends.
///
/// When `out` is `Some`, every byte is written to it as it is consumed: verbatim
/// when the scanner classifies it as code, and as [`MASK_FILL`] otherwise — string-literal
/// delimiters and contents, and everything from a line-comment opener to end of line.
/// Filler rather than removal, so byte length and column positions survive and a
/// caller can index the mask exactly as it would index `line`. That filler is deliberately
/// NOT a space — see [`MASK_FILL`] for why a space-filled mask could manufacture the very
/// evidence its one consumer reads.
///
/// `line_comment` is a parameter rather than the hardcoded `//` it used to be:
/// comment syntax is per-language and this scanner runs on fragments in every
/// language `edit_code` supports. A caller that does not care passes `["//"]`.
fn scan_line_into(
    line: &str,
    entry: Scan,
    line_comment: &[&str],
    mut out: Option<&mut String>,
) -> Scan {
    /// [`MASK_FILL`], not deletion — see the byte-offset guarantee above.
    fn push(out: &mut Option<&mut String>, seg: &str, code: bool) {
        let Some(buf) = out.as_mut() else { return };
        if code {
            buf.push_str(seg);
        } else {
            for _ in 0..seg.len() {
                buf.push(MASK_FILL);
            }
        }
    }

    let mut state = entry;
    let mut i = 0;
    while i < line.len() {
        let rest = &line[i..];
        let one_char = rest.chars().next().map_or(1, char::len_utf8);
        // Width of a `\x` escape: the backslash plus whatever follows it, or just the
        // backslash when it ends the line. Lazy because `rest[1..]` is only a valid
        // boundary once the leading byte is known to be the one-byte `\`.
        let escape = || 1 + rest[1..].chars().next().map_or(0, char::len_utf8);
        // Every arm computes the byte width it consumes, hands exactly that slice to
        // `push` with its classification, and then advances by the same number. The
        // single `i += n` at the bottom is what keeps mask and scanner in step: an arm
        // cannot advance without having classified what it advanced over.
        let n = match &state {
            Scan::Spanning { close, escapes } => {
                let escapes = *escapes;
                let consumed = rest
                    .strip_prefix(close.as_str())
                    .map(|after| rest.len() - after.len());
                let n = match consumed {
                    Some(n) => {
                        state = Scan::Code;
                        n
                    }
                    None if escapes && rest.starts_with('\\') => escape(),
                    None => one_char,
                };
                push(&mut out, &rest[..n], false);
                n
            }
            Scan::Quoted(quote) => {
                let quote = *quote;
                let n = if rest.starts_with('\\') {
                    // Skip the escape and what it escapes, so `\"` does not close.
                    escape()
                } else {
                    if rest.starts_with(quote) {
                        state = Scan::Code;
                    }
                    one_char
                };
                push(&mut out, &rest[..n], false);
                n
            }
            Scan::Code => {
                if line_comment.iter().any(|t| rest.starts_with(t)) {
                    // Nothing in a line comment can open a literal. Bailing here is
                    // what keeps markdown backticks in a doc comment from opening a
                    // phantom line-spanning literal.
                    push(&mut out, rest, false);
                    break;
                }
                if let Some((consumed, opened)) = spanning_opener(rest) {
                    state = opened;
                    push(&mut out, &rest[..consumed], false);
                    consumed
                } else if rest.starts_with('"') || rest.starts_with('\'') {
                    state = Scan::Quoted(rest.chars().next().unwrap_or('"'));
                    push(&mut out, &rest[..one_char], false);
                    one_char
                } else {
                    push(&mut out, &rest[..one_char], true);
                    one_char
                }
            }
        };
        i += n;
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

/// Blank the non-code spans of every line in `block`: string-literal delimiters and
/// contents, and everything from a line-comment opener to end of line. Code bytes
/// survive verbatim, every other byte becomes [`MASK_FILL`], so each line keeps its byte
/// length and its column positions.
///
/// The filler is **not a space**, and substituting one back reintroduces a shipped bug:
/// this function's one consumer matches needles that end in a space, so space filler let
/// the mask satisfy a needle the source did not contain. [`MASK_FILL`] carries the
/// argument and the constraints on any replacement.
///
/// Each line is scanned INDEPENDENTLY, starting in code. That is deliberate, and it
/// is what makes the function correct for its caller rather than merely cheaper:
/// `find_def_keyword` passes the lines an edit CHANGED, which are not contiguous
/// source. A quote on one changed line and a quote three changed lines later never
/// opened a literal in the file, and carrying scanner state between them would blank
/// the real code lying between two unrelated quotes — a false negative, which for
/// that caller is the strictly worse direction.
///
/// Block comments (`/* … */`) are NOT recognised: a keyword inside one, on a line
/// that does not itself begin with `/*` or `*`, still reads as code. Callers that
/// care handle the line-leading shapes themselves; the residual is named at
/// `edit_file`'s refusal site rather than silently narrowed here.
pub fn blank_non_code(block: &str, line_comment: &[&str]) -> String {
    let mut out = String::with_capacity(block.len());
    for (n, line) in block.lines().enumerate() {
        if n > 0 {
            out.push('\n');
        }
        scan_line_into(line, Scan::Code, line_comment, Some(&mut out));
    }
    out
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
///
/// # Deprecated: the caller class this serves is empty
///
/// `7712d8e6` (2026-08-25) migrated all four call sites to
/// [`extract_lines_to_json_budget`] and kept this one on the stated grounds that
/// it "keeps its raw-byte contract for callers whose budget really is raw
/// bytes". Measured 2026-08-29: that class has **no members and no place to
/// acquire one**. Every use of `INLINE_BYTE_BUDGET` in this crate feeds content
/// into a `json!({...})` tool response, where the escaped cost is the binding
/// one — which is the entire reason the JSON-aware sibling exists.
///
/// Retained rather than removed only because this crate is published and this
/// function is reachable as `codescout::util::text::extract_lines_to_budget`;
/// deleting a `pub` item is a breaking change, not a cleanup. Scheduled for
/// removal in the next breaking release — see `docs/RELEASE-TODO.md`.
///
/// Its tests are NOT dead weight: they are the direct coverage of the shared
/// [`extract_lines_with_cost`] core, including the only test that pins the
/// safety valve. Retarget them before removing this wrapper.
#[deprecated(
    note = "the raw-byte caller class is empty — every budgeted path in this crate \
            serializes to JSON. Use extract_lines_to_json_budget. Scheduled for removal \
            in the next breaking release."
)]
pub fn extract_lines_to_budget(
    text: &str,
    start_line: usize,
    end_line: usize,
    byte_budget: usize,
) -> (String, usize, bool) {
    extract_lines_with_cost(text, start_line, end_line, byte_budget, |line| {
        line.len() + 1 // +1 for the \n join separator
    })
}

/// Like [`extract_lines_to_budget`], but charges each line what it will cost
/// **after JSON string escaping** — which is the size the response is actually
/// measured against.
///
/// A tool that inlines a chunk gets buffered by `call_content` when its
/// *serialized* form exceeds `TOOL_OUTPUT_BUFFER_THRESHOLD`, yet the raw budget
/// charges `line.len() + 1`. Inside a JSON string every `\n` is two bytes, so
/// the escaping charge scales with line COUNT and short lines overshoot: a
/// 9000-byte chunk of ~1000 short lines serialized to 10169 bytes against a
/// 10000-byte threshold (measured 2026-08-25). The caller then received a
/// `@tool_*` envelope wrapping the response instead of the response itself —
/// and that envelope's hint routes into yet another buffer, which is the
/// nesting reported in
/// `docs/issues/archive/2026-08-25-run-command-nested-buffer-recursion.md`.
///
/// Use this for every budgeted chunk in this crate. The raw-byte variant
/// [`extract_lines_to_budget`] is deprecated and its caller class is empty:
/// measured 2026-08-29, every use of `INLINE_BYTE_BUDGET` here feeds a
/// `json!({...})` tool response, so the escaped charge is always the binding
/// one. This doc line previously pointed readers at that variant "where the
/// budget really is raw bytes" — a class that had zero members from the moment
/// the sentence was written, in the same commit that emptied it.
pub fn extract_lines_to_json_budget(
    text: &str,
    start_line: usize,
    end_line: usize,
    byte_budget: usize,
) -> (String, usize, bool) {
    extract_lines_with_cost(text, start_line, end_line, byte_budget, |line| {
        json_escaped_len(line) + 2 // the \n separator escapes to two bytes
    })
}

/// Byte length of `s` inside a JSON string literal, matching `serde_json`'s
/// default escaping. Computed rather than serialized, so budgeting a chunk
/// costs no allocation per line.
fn json_escaped_len(s: &str) -> usize {
    s.chars()
        .map(|c| match c {
            '"' | '\\' | '\n' | '\r' | '\t' | '\u{08}' | '\u{0c}' => 2,
            c if (c as u32) < 0x20 => 6, // \u00XX
            c => c.len_utf8(),
        })
        .sum()
}

/// Shared walk behind the two budgeted extractors — they differ only in what a
/// line is charged.
///
/// **Safety valve:** always includes at least 1 line (even if it exceeds the
/// budget) to prevent infinite retry loops where the caller keeps requesting the
/// same range. Exception: a zero budget returns nothing.
fn extract_lines_with_cost(
    text: &str,
    start_line: usize,
    end_line: usize,
    byte_budget: usize,
    cost: impl Fn(&str) -> usize,
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

        let line_bytes = cost(line);
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

// `extract_lines_to_budget` is deprecated but deliberately still exercised here:
// its nine tests are the direct coverage of the shared `extract_lines_with_cost`
// core, and one of them is the only test pinning the safety valve. Scoped to the
// test module so a deprecated call in production code still fails the gate.
#[allow(deprecated)]
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
        // reported the whole block as dedented and shifted the string's value by 4.
        //
        // The fixture is a real multi-line literal held open by a raw newline, which is
        // both the natural way to write one and the form the first cut of this fix did
        // not cover. edit_code wrote it through its own reindent, so a regression in
        // either half would corrupt this fixture rather than fail loudly elsewhere.
        let body = "    fn t() {
        let content = \"\\
# Gotchas

## MCP Binary Symlink
\";
        assert!(content.starts_with('#'));
    }";
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
    fn blank_non_code_keeps_code_and_blanks_comments_and_literals() {
        use super::blank_non_code;

        // Expectations are built from `MASK_FILL` rather than written as literals. Two
        // reasons, and the second is new: hand-counted padding is a fixture detail that
        // goes wrong silently and reads as a bug here, AND a literal would re-pin the
        // filler byte, which is `MASK_FILL`'s to choose and not this test's to assert.
        // What this test is about is WHICH SPANS are blanked, never what they become.
        let fill = |n: usize| super::MASK_FILL.to_string().repeat(n);

        // Byte length per line is the contract callers index against, so assert it
        // directly rather than trusting the shapes below to imply it.
        let line = "    1 // mentions a fn";
        let masked = blank_non_code(line, &["//"]);
        assert_eq!(masked.len(), line.len(), "mask must preserve byte length");
        assert_eq!(masked, format!("    1 {}", fill("// mentions a fn".len())));

        // The literal's delimiters go too — otherwise `"fn ` still reads as a word start.
        assert_eq!(
            blank_non_code(r#"assert!(s.contains("fn "));"#, &["//"]),
            format!("assert!(s.contains({}));", fill(r#""fn ""#.len()))
        );

        // Code survives verbatim, including a keyword that really is one.
        assert_eq!(
            blank_non_code("pub fn real() {", &["//"]),
            "pub fn real() {"
        );

        // The token set is a parameter: `#` is a comment in Python and an attribute in
        // Rust, and passing the wrong set silently blanks real code.
        let py = "x = 1  # class later";
        assert_eq!(
            blank_non_code(py, &["#"]),
            format!("x = 1  {}", fill("# class later".len())),
            "`#` opens a comment in python"
        );
        assert_eq!(
            blank_non_code(py, &["//"]),
            py,
            "`#` is NOT a comment when the token set says `//` — nothing is blanked"
        );
    }

    /// The mask must never MANUFACTURE the evidence its caller reports.
    ///
    /// Blanked bytes used to be spaces, and every `find_def_keyword` needle carries a
    /// TRAILING SPACE to supply its right word boundary — so mask spaces and source
    /// spaces were the same byte. A possessive read as a string opener (`class's`
    /// blanks to `class` + filler) therefore satisfied `"class "`, a sequence appearing
    /// nowhere in the input, and the caller could search for what the refusal named and
    /// not find it. `4d2e4052e1272a69`.
    ///
    /// The apostrophe reading itself is deliberate and stays: it is what stops a char
    /// literal containing a quote (`let q = '"';`) latching the scanner across the rest
    /// of a block for `edit_code`'s reindent path, which reads the STATE this function
    /// also produces. The filler is the half that only the mask consumer sees.
    ///
    /// Mutation that must kill this: make the non-code branch of `push` emit `' '` again.
    #[test]
    fn blank_non_code_cannot_manufacture_a_word_boundary() {
        use super::blank_non_code;

        let prose = "each class's field is the authoritative copy";
        let masked = blank_non_code(prose, &["#"]);
        assert_eq!(
            masked.len(),
            prose.len(),
            "the byte-length contract still holds"
        );
        assert!(
            !masked.contains("class "),
            "no needle may be satisfied by filler the source did not supply: {masked:?}"
        );
        // The opposite direction, and the reason this is a pair: a filler change that
        // also stopped blanking would satisfy the assertion above and fail this one.
        assert!(
            !masked.contains("field"),
            "everything after the apostrophe is still blanked: {masked:?}"
        );
    }

    /// Lines are scanned independently, and this is the test that would red if someone
    /// "optimised" `blank_non_code` by threading scanner state across them — which reads
    /// like a correctness improvement and is the one change that breaks its caller.
    ///
    /// The two lines are NON-ADJACENT in the file they came from: `find_def_keyword`
    /// receives the lines an edit changed, joined. Carried state would treat the first
    /// quote as opening a literal that the second closes, blanking `fn genuine` in
    /// between — a definition silently admitted.
    #[test]
    fn blank_non_code_does_not_carry_literal_state_between_lines() {
        use super::blank_non_code;

        let changed_lines = "let a = \"open;\nfn genuine() {}\nlet b = \"close;";
        let masked = blank_non_code(changed_lines, &["//"]);
        assert!(
            masked.contains("fn genuine"),
            "a code line between two unrelated quotes must survive: {masked:?}"
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

    // ── extract_lines_to_json_budget ────────────────────────────────────────
    //
    // Added 2026-08-29. Until then this function had FOUR production callers
    // and ZERO direct tests, while its unused sibling above had zero callers
    // and nine tests — the coverage was attached to the wrapper that was
    // retired, not the one that shipped. Everything below tests the live path.

    /// The reason this function exists, stated as a difference the code must
    /// exhibit. If someone ever "simplifies" it to delegate to the raw cost
    /// closure, these two counts become equal and this test dies.
    #[test]
    fn json_budget_charges_escaping_so_it_fits_fewer_lines_than_the_raw_budget() {
        // 20 quotes per line: 20 raw bytes, 40 escaped (each `"` -> `\"`).
        let text: String = std::iter::repeat_n("\"".repeat(20), 20)
            .collect::<Vec<_>>()
            .join("\n");

        let (_, raw_lines, _) = extract_lines_to_budget(&text, 1, 100, 100);
        let (_, json_lines, _) = extract_lines_to_json_budget(&text, 1, 100, 100);

        // raw: 20+1 = 21/line -> 4 lines (84), a 5th would reach 105.
        assert_eq!(raw_lines, 4, "raw cost model charges 21 per line");
        // json: 40+2 = 42/line -> 2 lines (84), a 3rd would reach 126.
        assert_eq!(json_lines, 2, "escaped cost model charges 42 per line");
        assert!(
            json_lines < raw_lines,
            "the whole point of the JSON variant is that it fits FEWER lines \
             into the same budget; equal counts mean the escaping charge was lost"
        );
    }

    /// The contract in the units that actually bind: a chunk is measured after
    /// JSON escaping, because that is the form the response is serialized in.
    ///
    /// The raw variant is exercised on the same input as a control — it
    /// overshoots, which is the miniature of the measured 10169-vs-10000
    /// overshoot that caused
    /// `docs/issues/archive/2026-08-25-run-command-nested-buffer-recursion.md`.
    /// Without the control this test would pass against any budget large
    /// enough, and prove nothing about the cost model.
    #[test]
    fn json_budget_chunk_still_fits_the_budget_after_serialization() {
        // cap-class: NOT_A_CAP — a unit test's own budget argument; it bounds no shipped path
        const BUDGET: usize = 200;
        let text: String = std::iter::repeat_n("ab", 100)
            .collect::<Vec<_>>()
            .join("\n");

        // `to_string` wraps the value in quotes; the payload is what is budgeted.
        let escaped_len = |s: &str| serde_json::to_string(s).unwrap().len() - 2;

        let (json_chunk, _, _) = extract_lines_to_json_budget(&text, 1, 200, BUDGET);
        assert!(
            escaped_len(&json_chunk) <= BUDGET,
            "escaped chunk must fit the budget it was measured against; got {} vs {BUDGET}",
            escaped_len(&json_chunk)
        );

        let (raw_chunk, _, _) = extract_lines_to_budget(&text, 1, 200, BUDGET);
        assert!(
            escaped_len(&raw_chunk) > BUDGET,
            "control: the raw-byte variant must OVERSHOOT once escaped, or this \
             fixture cannot tell the two cost models apart; got {} vs {BUDGET}",
            escaped_len(&raw_chunk)
        );
    }

    /// The safety valve, exercised through the wrapper production actually
    /// calls. It was previously pinned only via the retired sibling, so the
    /// live path's guarantee rested on a test of a function nothing calls.
    ///
    /// `read_file`'s `clamp_over_budget_line` depends on this behaviour: it
    /// exists precisely to catch the oversized line the valve emits here.
    #[test]
    fn json_budget_safety_valve_yields_one_line_even_when_it_busts_the_budget() {
        let text = "x".repeat(1000);
        let (content, lines_shown, complete) = extract_lines_to_json_budget(&text, 1, 1, 50);
        assert_eq!(lines_shown, 1, "the valve must always make progress");
        assert!(
            complete,
            "reaching end_line is 'complete' even when oversized"
        );
        assert_eq!(
            content.len(),
            1000,
            "the line is emitted WHOLE — callers that inline it must clamp it \
             themselves (see read_file::clamp_over_budget_line)"
        );
    }
}
