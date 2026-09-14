//! Command type detection and smart output summarization.
//!
//! Detects whether a command is a test runner, a build tool, or something else,
//! then produces a structured summary appropriate for that command type.

use regex::Regex;
use serde_json::{json, Value};
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// Thresholds
// ---------------------------------------------------------------------------

/// Inline line cap for buffer-only queries (e.g. `grep/sed @cmd_xxx`).
/// Kept separate from the summarization threshold so "when to buffer" and "how much
/// to return from a buffer query" can be tuned independently.
// cap-class: RESULT_CAP command_summary.buffer_query_lines — probed
pub(crate) const BUFFER_QUERY_INLINE_CAP: usize = 100;

/// Number of lines to keep from the top in generic summaries.
const HEAD_LINES: usize = 20;

/// Number of lines to keep from the bottom in generic summaries.
const TAIL_LINES: usize = 10;

/// Lines of `stderr` carried inline in a summarized `test` / `build` envelope.
///
/// Deliberately the same number as `STDERR_BUDGET` in `run_command`'s
/// buffer-query branch (`run_command/output.rs`): that is the same question —
/// "how much stderr is worth inlining beside an elided stdout" — and two
/// independently-chosen answers to it would drift apart silently.
// cap-class: RESULT_CAP command_summary.stderr_tail_lines — probed
const STDERR_SUMMARY_LINE_BUDGET: usize = 20;

/// Byte ceiling on that same field.
///
/// A line budget alone does not bound a field, because a line has no length
/// bound: one 200 KB line satisfies `take(20)` and pushes the envelope past
/// `TOOL_OUTPUT_BUFFER_THRESHOLD`. That re-buffers the whole response and
/// replaces it with `format_run_command`'s one-line summary — which carries no
/// stderr — so an unbounded field hides the verdict one layer up instead of
/// delivering it, defeating the field's only purpose.
///
/// **This is the normal path for a workspace test run, not an edge case.**
/// `needs_summary` is `(stdout.len() + stderr.len()) / 4 > MAX_INLINE_TOKENS` —
/// it sums BOTH streams, and on any real `cargo test --workspace` the first term
/// settles it alone: measured 2026-09-14, one gate run buffered 831,766 B across
/// 10,263 lines, 83× the threshold. So this summarizer is entered on every such
/// run, including every green one, whatever the stderr size. A narrowly filtered
/// run (`--lib <name>`, ~4.5 KB) stays under and is returned inline with its
/// stderr intact — which is why the loss looked intermittent, and why the
/// `type: "test"` classification looked like the discriminator when the real
/// gate is combined output volume.
///
/// Sizing follows from that: the field is the routine rendering path for the
/// repo's most-run command, so it is bounded tightly rather than generously.
/// Raising the threshold instead would not work — the input is unbounded and
/// the threshold is not.
// cap-class: RESULT_CAP command_summary.stderr_tail_bytes — probed
const STDERR_SUMMARY_BYTE_BUDGET: usize = 2000;

/// Leading token of the marker prefixed to a `stderr` field that was cut.
///
/// Distinct from `summarize_generic`'s `--- N lines omitted ---`, and the
/// distinction is the point: that marker elides a MIDDLE, this one drops a
/// HEAD. A reader who mistakes which end was cut goes looking for the missing
/// lines in the wrong place — and both markers sit in the same envelope.
pub(crate) const STDERR_TAIL_MARKER: &str = "--- stderr TAIL:";

// ---------------------------------------------------------------------------
// CommandType
// ---------------------------------------------------------------------------

/// Broad category of the command being run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandType {
    Test,
    Build,
    Generic,
}

// ---------------------------------------------------------------------------
// Regex patterns (compiled once via OnceLock)
// ---------------------------------------------------------------------------

/// Defines a `fn $name() -> &'static Regex` backed by its own lazily-initialized
/// `OnceLock`. All five regex accessors below repeated this exact
/// static-cell/get_or_init/expect skeleton around a different pattern —
/// this macro is the single place that skeleton lives now.
macro_rules! cached_regex_fn {
    ($name:ident, $pattern:expr, $what:literal) => {
        fn $name() -> &'static Regex {
            static RE: OnceLock<Regex> = OnceLock::new();
            RE.get_or_init(|| Regex::new($pattern).expect($what))
        }
    };
}
cached_regex_fn!(
    test_re,
    r"(?x)
    (?:^|\s|/)
    (?:
        cargo\s+test
      | pytest
      | npm\s+test
      | npx\s+jest
      | jest
      | go\s+test
      | mvn\s+test
      | gradle\s+test
    )
    (?:\s|$)",
    "test regex"
);

cached_regex_fn!(
    build_re,
    r"(?x)
    (?:^|\s|/)
    (?:
        cargo\s+(?:build|clippy|check)
      | npm\s+run\s+build
      | make(?:\s|$)
      | tsc(?:\s|$)
      | gcc(?:\s|$)
      | g\+\+(?:\s|$)
      | clang(?:\s|$)
      | javac(?:\s|$)
      | go\s+build
    )",
    "build regex"
);

cached_regex_fn!(
    cargo_test_result_re,
    r"(\d+)\s+passed;\s+(\d+)\s+failed;\s+(\d+)\s+ignored",
    "cargo test regex"
);

cached_regex_fn!(rust_error_code_re, r"^error\[E\d+\]", "rust error regex");

cached_regex_fn!(warning_re, r"^warning(\[.+\])?:", "warning regex");

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Classify a command string into Test, Build, or Generic.
pub fn detect_command_type(command: &str) -> CommandType {
    // Test takes priority over build (e.g. "cargo test" is Test, not Build).
    if test_re().is_match(command) {
        CommandType::Test
    } else if build_re().is_match(command) {
        CommandType::Build
    } else {
        CommandType::Generic
    }
}

/// Returns the byte offset of the `|` pipe character that separates a command
/// from a terminal filter stage (grep, head, tail, sed, awk, cut, wc, sort,
/// uniq, tr, rg, egrep, fgrep).
///
/// Returns `None` if the command has no pipe, or if the last pipe stage is not
/// a known terminal filter. Pipe characters inside quoted strings are ignored.
pub fn detect_terminal_filter(cmd: &str) -> Option<usize> {
    const TERMINAL_FILTERS: &[&str] = &[
        "grep", "egrep", "fgrep", "rg", "head", "tail", "sed", "awk", "cut", "wc", "sort", "uniq",
        "tr",
    ];

    let mut last_pipe: Option<usize> = None;
    let mut in_single = false;
    let mut in_double = false;
    let mut escape_next = false;

    for (i, ch) in cmd.char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }
        match ch {
            '\\' if !in_single => escape_next = true,
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '|' if !in_single && !in_double => {
                // Skip `||` (logical OR) — not a pipeline pipe.
                let next_char = cmd[i + 1..].chars().next();
                let prev_char = if i > 0 { cmd[..i].chars().last() } else { None };
                if next_char != Some('|') && prev_char != Some('|') {
                    last_pipe = Some(i);
                }
            }
            _ => {}
        }
    }

    let pipe_pos = last_pipe?;

    // Extract the first token of the stage after the pipe
    let after_pipe = cmd[pipe_pos + 1..].trim_start();
    let token = after_pipe
        .split(|c: char| c.is_whitespace())
        .next()
        .unwrap_or("");

    // Strip any path prefix so "/usr/bin/grep" matches "grep"
    let name = token.rsplit('/').next().unwrap_or(token);

    if TERMINAL_FILTERS.contains(&name) {
        Some(pipe_pos)
    } else {
        None
    }
}

/// Returns `true` when the combined output is large enough to benefit from
/// summarization rather than raw output.
pub fn needs_summary(stdout: &str, stderr: &str) -> bool {
    (stdout.len() + stderr.len()) / 4 > crate::tools::MAX_INLINE_TOKENS
}

/// Clip `s` to at most `max` bytes, on a `char` boundary.
fn clip_to_bytes(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Render the `stderr` field for a summarized envelope: the LAST lines, bounded
/// in both lines and bytes, behind an explicit marker when anything was cut.
///
/// **Tail, not head, and that choice is the whole point of the field.** A
/// wrapper script writes its verdict *after* the command it wraps has finished,
/// so its verdict is the last thing on stderr; a compiler writes its diagnostics
/// first. The head is already mined by the type-specific extractors
/// (`first_error`, `failures`), both of which read stderr as part of `combined`
/// — so taking the head here would re-report what is already covered and drop
/// the only thing that is not.
///
/// Returns `None` for empty stderr so the key is omitted rather than rendered
/// empty, matching [`summarize_generic`].
fn summarize_stderr(stderr: &str) -> Option<String> {
    if stderr.is_empty() {
        return None;
    }
    let lines: Vec<&str> = stderr.lines().collect();
    let total = lines.len();

    // Walk backwards, so when the byte ceiling binds it drops the OLDEST line
    // kept rather than the newest. Forwards, a long compile log would spend the
    // whole budget on `Compiling …` and cut off exactly the verdict.
    let mut kept: Vec<&str> = Vec::new();
    let mut bytes = 0usize;
    let mut clipped = false;
    for line in lines.iter().rev().take(STDERR_SUMMARY_LINE_BUDGET) {
        let needed = line.len() + 1; // +1 for the '\n' rejoining it
        if bytes + needed > STDERR_SUMMARY_BYTE_BUDGET {
            // A single line wider than the entire budget is still carried, clipped:
            // an elided verdict beats an absent one. This is the only branch that can
            // reach an empty-ish field on non-empty input, and it is why `clipped` is
            // tracked separately from the dropped-line count — with `total == 1` the
            // count is zero and the marker would otherwise claim nothing was lost.
            if kept.is_empty() {
                kept.push(clip_to_bytes(line, STDERR_SUMMARY_BYTE_BUDGET));
                clipped = true;
            }
            break;
        }
        bytes += needed;
        kept.push(line);
    }
    kept.reverse();

    let dropped = total - kept.len();
    if dropped == 0 && !clipped {
        // Nothing lost: hand back the stream verbatim, trailing newline and all, so a
        // complete small stderr renders byte-identically to `summarize_generic`'s.
        return Some(stderr.to_string());
    }

    let mut notes: Vec<String> = Vec::new();
    if dropped > 0 {
        notes.push(format!("{dropped} earlier line(s) dropped"));
    }
    if clipped {
        notes.push("last line clipped to the byte ceiling".to_string());
    }
    Some(format!(
        "{STDERR_TAIL_MARKER} {notes}; {shown} of {total} line(s) shown. \
         Full stderr is NOT in the @cmd_* buffer — buffer reads return stdout only. ---\n{body}",
        notes = notes.join("; "),
        shown = kept.len(),
        body = kept.join("\n"),
    ))
}

/// Produce a structured summary of test-runner output.
///
/// Parses cargo-test-style result lines, sums across multiple test binaries,
/// and extracts failure details.
pub fn summarize_test_output(stdout: &str, stderr: &str, exit_code: i32) -> Value {
    let mut passed: u64 = 0;
    let mut failed: u64 = 0;
    let mut ignored: u64 = 0;

    let combined = if stderr.is_empty() {
        stdout.to_string()
    } else {
        format!("{stdout}\n{stderr}")
    };

    let re = cargo_test_result_re();
    for line in combined.lines() {
        if let Some(caps) = re.captures(line) {
            passed += caps[1].parse::<u64>().unwrap_or(0);
            failed += caps[2].parse::<u64>().unwrap_or(0);
            ignored += caps[3].parse::<u64>().unwrap_or(0);
        }
    }

    let failures = extract_test_failures(&combined);

    let mut result = json!({
        "type": "test",
        "exit_code": exit_code,
        "passed": passed,
    });

    if failed > 0 {
        result["failed"] = json!(failed);
    }
    if ignored > 0 {
        result["ignored"] = json!(ignored);
    }
    if let Some(f) = failures {
        result["failures"] = Value::String(f);
    }
    // A test harness writes its RESULTS to stdout, so a test run's stderr is the
    // compiler's diagnostics and any wrapper script's commentary — exactly what a
    // reader wants when the news is bad, and until 2026-09-14 the only shape that
    // never carried it. BUG docs/issues/2026-09-14-run-commands-test-envelope-drops-the-stderr-a-wrapper-puts-its-verdict-on.md
    if let Some(err) = summarize_stderr(stderr) {
        result["stderr"] = Value::String(err);
    }

    result
}

/// Produce a structured summary of compiler / build-tool output.
///
/// Counts errors (with error codes) and warnings, and extracts the first
/// error block for quick diagnosis.
pub fn summarize_build_output(stdout: &str, stderr: &str, exit_code: i32) -> Value {
    let combined = if stderr.is_empty() {
        stdout.to_string()
    } else if stdout.is_empty() {
        stderr.to_string()
    } else {
        format!("{stdout}\n{stderr}")
    };

    let mut errors: u64 = 0;
    let mut warnings: u64 = 0;
    let mut first_error: Option<String> = None;

    let err_re = rust_error_code_re();
    let warn_re = warning_re();
    let lines: Vec<&str> = combined.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if err_re.is_match(line) {
            errors += 1;
            if first_error.is_none() {
                first_error = Some(extract_error_block(&lines, i));
            }
        } else if warn_re.is_match(line) {
            warnings += 1;
        }
    }

    let mut result = json!({
        "type": "build",
        "exit_code": exit_code,
    });

    if errors > 0 {
        result["errors"] = json!(errors);
    }
    if warnings > 0 {
        result["warnings"] = json!(warnings);
    }
    if let Some(err) = first_error {
        result["first_error"] = Value::String(err);
    }
    // Same omission, same fix. `first_error` mines the HEAD of the combined stream;
    // this carries the TAIL, which is where a wrapper's verdict and the final
    // `error: could not compile` line both live.
    if let Some(err) = summarize_stderr(stderr) {
        result["stderr"] = Value::String(err);
    }

    result
}

/// Produce a head+tail summary for generic command output.
///
/// If stdout fits within HEAD_LINES + TAIL_LINES, it is returned verbatim.
/// Otherwise the middle is replaced with an "N lines omitted" marker.
pub fn summarize_generic(stdout: &str, stderr: &str, exit_code: i32) -> Value {
    let stdout_lines: Vec<&str> = stdout.lines().collect();
    let total_stdout_lines = stdout_lines.len();

    let summarized_stdout = if total_stdout_lines > HEAD_LINES + TAIL_LINES {
        let head: Vec<&str> = stdout_lines[..HEAD_LINES].to_vec();
        let tail: Vec<&str> = stdout_lines[total_stdout_lines - TAIL_LINES..].to_vec();
        let omitted = total_stdout_lines - HEAD_LINES - TAIL_LINES;
        format!(
            "{}\n--- {} lines omitted ---\n{}",
            head.join("\n"),
            omitted,
            tail.join("\n")
        )
    } else {
        stdout.to_string()
    };

    let mut result = json!({
        "type": "generic",
        "exit_code": exit_code,
    });

    if !summarized_stdout.is_empty() {
        result["stdout"] = Value::String(summarized_stdout);
    }
    if !stderr.is_empty() {
        result["stderr"] = Value::String(stderr.to_string());
    }

    result
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub fn count_lines(s: &str) -> usize {
    if s.is_empty() {
        0
    } else {
        s.lines().count()
    }
}

/// Truncate `text` to at most `max_lines` lines.
///
/// Returns `(truncated_text, lines_shown, lines_total)`.
/// When `lines_total <= max_lines`, `text` is returned unchanged and
/// `lines_shown == lines_total`.
#[allow(dead_code)]
pub(crate) fn truncate_lines(text: &str, max_lines: usize) -> (String, usize, usize) {
    let total = count_lines(text);
    if total <= max_lines {
        return (text.to_string(), total, total);
    }
    let truncated = text.lines().take(max_lines).collect::<Vec<_>>().join("\n");
    (truncated, max_lines, total)
}

/// Truncate `text` to at most `max_lines` lines **and** at most `max_bytes` bytes.
///
/// Both limits are applied; whichever is more restrictive wins. Truncation
/// always occurs on a line boundary so output is never split mid-line.
///
/// Returns `(content, lines_shown, total_lines)`.
pub(crate) fn truncate_lines_and_bytes(
    text: &str,
    max_lines: usize,
    max_bytes: usize,
) -> (String, usize, usize) {
    let total = count_lines(text);
    let mut result = String::new();
    let mut lines_shown = 0;

    for line in text.lines().take(max_lines) {
        // Each line adds the line itself plus a '\n' separator (except the first).
        let needed = if result.is_empty() {
            line.len()
        } else {
            line.len() + 1 // +1 for '\n'
        };
        if result.len() + needed > max_bytes {
            break;
        }
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(line);
        lines_shown += 1;
    }

    (result, lines_shown, total)
}

/// Extract text between the first `failures:` section markers in cargo test output.
fn extract_test_failures(output: &str) -> Option<String> {
    // Cargo test outputs failures between two "failures:" markers.
    // The first "failures:" is followed by stdout of failing tests.
    // The second "failures:" is followed by test names.
    let lines: Vec<&str> = output.lines().collect();
    let mut start = None;
    let mut end = None;

    for (i, line) in lines.iter().enumerate() {
        if line.trim() == "failures:" {
            if start.is_none() {
                start = Some(i);
            } else {
                end = Some(i);
                break;
            }
        }
    }

    // If we found at least one "failures:" marker, collect everything after it
    // up to the second marker (or end of output).
    if let Some(s) = start {
        let e = end.unwrap_or(lines.len());
        // Include from start through the second failures block
        let section: Vec<&str> = if let Some(end_idx) = end {
            // Find the end of the second failures block (next "test result:" line or EOF)
            let block_end = lines[end_idx..]
                .iter()
                .position(|l| l.starts_with("test result:"))
                .map(|p| end_idx + p)
                .unwrap_or(lines.len());
            lines[s..block_end].to_vec()
        } else {
            lines[s..e].to_vec()
        };

        let text = section.join("\n").trim().to_string();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    } else {
        None
    }
}

/// Extract an error block: the error line plus continuation lines until
/// the next blank line or next error/warning.
fn extract_error_block(lines: &[&str], start: usize) -> String {
    let err_re = rust_error_code_re();
    let warn_re = warning_re();
    let mut block = vec![lines[start]];
    for line in &lines[start + 1..] {
        // Stop at blank lines or next top-level diagnostic
        if line.is_empty()
            || err_re.is_match(line)
            || warn_re.is_match(line)
            || line.starts_with("error:")
        {
            break;
        }
        block.push(line);
    }
    block.join("\n")
}

/// Strip ANSI CSI escape sequences from `s` (e.g. `\x1b[32m`, `\x1b[0m`).
///
/// Call this on buffered command output before byte-budget calculations. ANSI
/// codes are opaque to LLMs and inflate byte lengths — on ANSI-colored log
/// files a single grep match line can be several KB of escape codes around a
/// few dozen bytes of visible text, silently exhausting the byte budget and
/// causing `stdout_shown=0`.
///
/// Only CSI sequences (`ESC '['` … final ASCII letter) are removed. Other
/// escape types (rare in log files) pass through unchanged.
pub(crate) fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            match chars.peek().copied() {
                Some('[') => {
                    chars.next(); // consume '['
                    for c in chars.by_ref() {
                        if c.is_ascii_alphabetic() {
                            break; // final byte consumed
                        }
                    }
                }
                _ => {
                    // Non-CSI escape — preserve; rare in structured log output.
                    result.push(ch);
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- detect_command_type --

    #[test]
    fn strip_ansi_codes_removes_color_sequences() {
        let input = "\x1b[32mINFO\x1b[0m some message";
        assert_eq!(strip_ansi_codes(input), "INFO some message");
    }

    #[test]
    fn strip_ansi_codes_removes_256_color_sequences() {
        // 256-color and bold sequences have multiple parameters separated by ';'
        let input = "\x1b[1;38;5;220mWARN\x1b[0m something";
        assert_eq!(strip_ansi_codes(input), "WARN something");
    }

    #[test]
    fn strip_ansi_codes_plain_text_unchanged() {
        let input = "no escape codes here\nline two";
        assert_eq!(strip_ansi_codes(input), input);
    }

    #[test]
    fn strip_ansi_codes_preserves_newlines() {
        let input = "\x1b[32mline1\x1b[0m\nline2\n\x1b[31mline3\x1b[0m";
        assert_eq!(strip_ansi_codes(input), "line1\nline2\nline3");
    }

    #[test]
    fn strip_ansi_codes_non_csi_escape_preserved() {
        // ESC followed by non-'[' is not a CSI sequence — preserved as-is
        let input = "\x1b=hello";
        assert_eq!(strip_ansi_codes(input), "\x1b=hello");
    }

    #[test]
    fn strip_ansi_codes_empty_string() {
        assert_eq!(strip_ansi_codes(""), "");
    }

    #[test]
    fn detect_test_command() {
        assert_eq!(detect_command_type("cargo test"), CommandType::Test);
        assert_eq!(
            detect_command_type("cargo test --release"),
            CommandType::Test
        );
        assert_eq!(detect_command_type("pytest tests/"), CommandType::Test);
        assert_eq!(detect_command_type("npm test"), CommandType::Test);
        assert_eq!(detect_command_type("npx jest"), CommandType::Test);
        assert_eq!(detect_command_type("go test ./..."), CommandType::Test);
    }

    #[test]
    fn detect_build_command() {
        assert_eq!(detect_command_type("cargo build"), CommandType::Build);
        assert_eq!(
            detect_command_type("cargo clippy -- -D warnings"),
            CommandType::Build
        );
        assert_eq!(detect_command_type("npm run build"), CommandType::Build);
        assert_eq!(detect_command_type("make"), CommandType::Build);
        assert_eq!(detect_command_type("tsc"), CommandType::Build);
        assert_eq!(detect_command_type("gcc main.c"), CommandType::Build);
    }

    #[test]
    fn detect_generic_command() {
        assert_eq!(detect_command_type("echo hello"), CommandType::Generic);
        assert_eq!(detect_command_type("ls -la"), CommandType::Generic);
        assert_eq!(detect_command_type("cat file.txt"), CommandType::Generic);
    }

    // -- needs_summary --

    #[test]
    fn short_output_not_summarized() {
        assert!(!needs_summary("hello\nworld\n", ""));
    }

    #[test]
    fn long_output_needs_summary() {
        // Generate output exceeding MAX_INLINE_TOKENS * 4 bytes (~10KB)
        let stdout: String = (1..=3000).map(|i| format!("line {}\n", i)).collect();
        assert!(needs_summary(&stdout, ""));
    }

    // -- summarize_test_output --

    #[test]
    fn summarize_cargo_test_all_pass() {
        let stdout = "running 5 tests\ntest a ... ok\ntest b ... ok\ntest c ... ok\ntest d ... ok\ntest e ... ok\n\ntest result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s\n";
        let summary = summarize_test_output(stdout, "", 0);
        assert_eq!(summary["passed"], 5);
        assert!(
            summary.get("failed").is_none(),
            "failed:0 should be omitted"
        );
        assert!(
            summary.get("ignored").is_none(),
            "ignored:0 should be omitted"
        );
        assert!(summary.get("failures").is_none() || summary["failures"].is_null());
    }

    #[test]
    fn summarize_cargo_test_with_failures() {
        let stdout = "running 3 tests\ntest ok_test ... ok\ntest failing_test ... FAILED\ntest another ... ok\n\nfailures:\n\n---- failing_test stdout ----\nthread 'failing_test' panicked at 'assertion failed'\n\nfailures:\n    failing_test\n\ntest result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out\n";
        let summary = summarize_test_output(stdout, "", 1);
        assert_eq!(summary["passed"], 2);
        assert_eq!(summary["failed"], 1);
        let failures = summary["failures"].as_str().unwrap();
        assert!(failures.contains("failing_test"));
    }

    #[test]
    fn summarize_cargo_test_multiple_binaries() {
        let stdout = "\
running 3 tests
test a ... ok
test b ... ok
test c ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 2 tests
test d ... ok
test e ... FAILED

failures:

---- e stdout ----
assertion failed

failures:
    e

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
";
        let summary = summarize_test_output(stdout, "", 1);
        // Sums across both binaries
        assert_eq!(summary["passed"], 4);
        assert_eq!(summary["failed"], 1);
    }

    // -- summarize_build_output --

    #[test]
    fn summarize_build_errors() {
        let stderr = "error[E0308]: mismatched types\n --> src/main.rs:5:20\n  |\n5 |     let x: String = 42;\n  |                     ^^ expected `String`, found integer\n\nwarning: unused variable: `y`\n --> src/main.rs:3:9\n  |\n3 |     let y = 1;\n  |         ^ help: consider prefixing with an underscore: `_y`\n\nerror: aborting due to 1 previous error; 1 warning emitted\n";
        let summary = summarize_build_output("", stderr, 1);
        assert_eq!(summary["errors"], 1); // only error[E...], not "error: aborting"
        assert_eq!(summary["warnings"], 1);
        assert!(summary["first_error"].as_str().unwrap().contains("E0308"));
    }

    #[test]
    fn summarize_build_no_errors() {
        let stderr = "warning: unused variable: `x`\n --> src/main.rs:2:9\n";
        let summary = summarize_build_output("", stderr, 0);
        assert!(
            summary.get("errors").is_none(),
            "errors:0 should be omitted"
        );
        assert_eq!(summary["warnings"], 1);
        assert!(summary.get("first_error").is_none() || summary["first_error"].is_null());
    }

    // -- summarize_generic --

    #[test]
    fn summarize_generic_head_tail() {
        let lines: String = (1..=100).map(|i| format!("line {}\n", i)).collect();
        let summary = summarize_generic(&lines, "", 0);
        let output = summary["stdout"].as_str().unwrap();
        assert!(output.contains("line 1"));
        assert!(output.contains("line 20"));
        assert!(output.contains("lines omitted"));
        assert!(output.contains("line 100"));
    }

    #[test]
    fn summarize_generic_short_output_verbatim() {
        let stdout = "line 1\nline 2\nline 3\n";
        let summary = summarize_generic(stdout, "", 0);
        let output = summary["stdout"].as_str().unwrap();
        assert_eq!(output, stdout);
        assert!(!output.contains("omitted"));
    }

    #[test]
    fn summarize_generic_includes_stderr() {
        let summary = summarize_generic("out\n", "err\n", 1);
        assert!(summary.get("stderr").is_some());
        assert_eq!(summary["stderr"].as_str().unwrap(), "err\n");
    }

    #[test]
    fn summarize_generic_omits_empty_stderr() {
        let summary = summarize_generic("out\n", "", 0);
        assert!(summary.get("stderr").is_none() || summary["stderr"].is_null());
    }

    #[test]
    fn summarize_generic_omits_empty_stdout() {
        let summary = summarize_generic("", "err\n", 1);
        assert!(summary.get("stdout").is_none() || summary["stdout"].is_null());
        assert_eq!(summary["stderr"].as_str().unwrap(), "err\n");
    }

    // -- summarized stderr on the test / build shapes --
    //
    // BUG docs/issues/2026-09-14-run-commands-test-envelope-drops-the-stderr-a-wrapper-puts-its-verdict-on.md
    //
    // What these guard is a DIRECTION, not a presence. A wrapper's verdict is the
    // LAST thing on stderr, so a fix that carried the head would satisfy every
    // "stderr is present" assertion and still drop the only line the bug is about.
    // Each test below therefore asserts a presence AND an absence.

    #[test]
    fn summarize_test_output_carries_the_wrapper_verdict_on_stderr() {
        let stdout = "running 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n";
        let stderr = "mutation-probe: INCONCLUSIVE — the runner selected 0 tests\n";
        let summary = summarize_test_output(stdout, stderr, 0);
        // The regression: `passed: 0` + `exit_code: 0` is the byte-identical rendering of
        // a SURVIVED mutant, so without this field the envelope supplies a plausible
        // WRONG verdict rather than an obviously missing one.
        assert_eq!(summary["passed"], 0);
        assert_eq!(summary["exit_code"], 0);
        assert!(
            summary["stderr"].as_str().unwrap().contains("INCONCLUSIVE"),
            "the verdict must survive the test envelope; got {:?}",
            summary.get("stderr")
        );
    }

    #[test]
    fn summarize_test_output_omits_empty_stderr() {
        let summary = summarize_test_output("running 0 tests\n", "", 0);
        assert!(summary.get("stderr").is_none() || summary["stderr"].is_null());
    }

    #[test]
    fn summarize_build_output_carries_stderr_beside_first_error() {
        let stderr = "error[E0308]: mismatched types\n --> src/main.rs:5:20\nerror: could not compile `x` (lib) due to 1 previous error\n";
        let summary = summarize_build_output("", stderr, 1);
        // `first_error` mines the HEAD; the new field carries the TAIL. Both, not either:
        // the closing `could not compile` line is absent from `first_error`'s block.
        assert!(summary["first_error"].as_str().unwrap().contains("E0308"));
        assert!(summary["stderr"]
            .as_str()
            .unwrap()
            .contains("could not compile"));
    }

    /// The load-bearing one: `summarize_stderr` keeps the END of the stream.
    ///
    /// The absence half is what discriminates — a head-biased implementation passes
    /// every other test in this block.
    #[test]
    fn summarized_stderr_keeps_the_tail_and_drops_the_head() {
        let mut stderr = String::from("HEAD_COMPILING_NOISE\n");
        // LOAD-BEARING: these lines are SHORT on purpose, ~6 bytes each, so 200 of them
        // total ~1.2 KB — comfortably under STDERR_SUMMARY_BYTE_BUDGET. Lengthen them and
        // the byte ceiling starts binding too, at which point raising the LINE budget to
        // infinity leaves this test green: the sibling cap would silently do the dropping
        // instead, and the head would still be absent. Two caps rescuing each other reads
        // exactly like coverage (CLAUDE.md § Testing Discipline — "two aggregates can be
        // worse than one"). Keeping them short is what makes the line cap the only
        // mechanism that can produce this result.
        for i in 1..=200 {
            stderr.push_str(&format!("c-{i}\n"));
        }
        stderr.push_str("WRAPPER_VERDICT: INCONCLUSIVE\n");
        assert!(
            stderr.len() < 2000,
            "fixture must stay under the BYTE budget or it stops isolating the LINE budget; \
             got {} bytes",
            stderr.len()
        );

        let summary = summarize_test_output("running 0 tests\n", &stderr, 0);
        let rendered = summary["stderr"].as_str().unwrap();

        assert!(
            rendered.contains("WRAPPER_VERDICT: INCONCLUSIVE"),
            "the last line must survive"
        );
        assert!(
            !rendered.contains("HEAD_COMPILING_NOISE"),
            "the head must be dropped, not the tail — this is the whole direction claim"
        );
        // The cut is announced rather than silent: an unmarked tail is the same
        // `cluster/capped-result-presented-as-complete` shape the bug itself is.
        assert!(rendered.contains("--- stderr TAIL:"));
        assert!(rendered.contains("earlier line(s) dropped"));
    }

    #[test]
    fn summarized_stderr_returns_a_short_stream_verbatim_with_no_marker() {
        let summary = summarize_test_output("running 0 tests\n", "a\nb\nc\n", 0);
        let rendered = summary["stderr"].as_str().unwrap();
        assert_eq!(rendered, "a\nb\nc\n");
        assert!(!rendered.contains("--- stderr TAIL:"));
    }

    /// One enormous line satisfies a LINE budget and defeats it.
    ///
    /// The envelope re-buffers once `(stdout + stderr) / 4 > MAX_INLINE_TOKENS` —
    /// **~10 KB combined**, bracketed 2026-09-14 at 9,024 B inline / 14,304 B buffered.
    /// Past it the response is `format_run_command`'s one-line summary, which carries no
    /// stderr, so an unbounded field hides the verdict one layer up. A line-only budget
    /// reproduces that here, and `summarize_test_output` is reached by every `cargo test`
    /// this repo's gate runs — a few crates' worth of diagnostics clears 10 KB, so the
    /// common case is the one at risk, not a pathological one.
    #[test]
    fn summarized_stderr_bounds_a_single_enormous_line_by_bytes() {
        let huge = format!("{}\n", "x".repeat(200_000));
        let summary = summarize_test_output("running 0 tests\n", &huge, 0);
        let rendered = summary["stderr"].as_str().unwrap();
        assert!(
            rendered.len() < 4_000,
            "one 200 KB line must not reach the envelope; got {} bytes",
            rendered.len()
        );
        assert!(rendered.contains("--- stderr TAIL:"));
        assert!(rendered.contains("clipped to the byte ceiling"));
    }

    #[test]
    fn summarized_stderr_clips_on_a_char_boundary() {
        // A multi-byte char straddling the ceiling must not panic or emit invalid UTF-8.
        let huge = format!("{}\n", "é".repeat(200_000));
        let summary = summarize_test_output("running 0 tests\n", &huge, 0);
        assert!(summary["stderr"].as_str().unwrap().contains("é"));
    }

    // -- helpers --

    #[test]
    fn count_lines_empty() {
        assert_eq!(count_lines(""), 0);
    }

    #[test]
    fn count_lines_normal() {
        assert_eq!(count_lines("a\nb\nc"), 3);
    }

    #[test]
    fn truncate_lines_short_returns_unchanged() {
        let text = "a\nb\nc";
        let (out, shown, total) = truncate_lines(text, 10);
        assert_eq!(out, text);
        assert_eq!(shown, 3);
        assert_eq!(total, 3);
    }

    #[test]
    fn truncate_lines_exact_limit_not_truncated() {
        let text: String = (1..=5)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let (out, shown, total) = truncate_lines(&text, 5);
        assert_eq!(shown, 5);
        assert_eq!(total, 5);
        assert_eq!(out, text);
    }

    #[test]
    fn truncate_lines_long_truncates_correctly() {
        let text: String = (1..=10)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let (out, shown, total) = truncate_lines(&text, 3);
        assert_eq!(shown, 3);
        assert_eq!(total, 10);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "line 1");
        assert_eq!(lines[2], "line 3");
    }

    #[test]
    fn truncate_lines_empty_string() {
        let (out, shown, total) = truncate_lines("", 10);
        assert_eq!(out, "");
        assert_eq!(shown, 0);
        assert_eq!(total, 0);
    }

    #[test]
    fn truncate_lines_and_bytes_line_limit_wins() {
        // 3 lines, byte budget is generous — line limit (2) wins.
        let text = "aaa\nbbb\nccc";
        let (out, shown, total) = truncate_lines_and_bytes(text, 2, 1000);
        assert_eq!(out, "aaa\nbbb");
        assert_eq!(shown, 2);
        assert_eq!(total, 3);
    }

    #[test]
    fn truncate_lines_and_bytes_byte_limit_wins() {
        // 5 short lines, byte budget forces truncation after the 2nd line.
        // "aaa\nbbb" = 7 bytes; adding "\nccc" = 11 bytes — over the 8-byte budget.
        let text = "aaa\nbbb\nccc\nddd\neee";
        let (out, shown, total) = truncate_lines_and_bytes(text, 10, 8);
        assert_eq!(out, "aaa\nbbb");
        assert_eq!(shown, 2);
        assert_eq!(total, 5);
    }

    #[test]
    fn truncate_lines_and_bytes_both_fit() {
        let text = "a\nb\nc";
        let (out, shown, total) = truncate_lines_and_bytes(text, 10, 1000);
        assert_eq!(out, text);
        assert_eq!(shown, 3);
        assert_eq!(total, 3);
    }

    #[test]
    fn truncate_lines_and_bytes_long_lines_stay_under_byte_budget() {
        // Simulate log output: 200-char lines, budget = 10_000 bytes (TOOL_OUTPUT_BUFFER_THRESHOLD).
        let long_line = "x".repeat(200);
        let text: String = (0..100).map(|_| format!("{long_line}\n")).collect();
        let (out, shown, _total) = truncate_lines_and_bytes(&text, 100, 10_000);
        // Must fit within budget.
        assert!(
            out.len() <= 10_000,
            "output {} bytes exceeds budget",
            out.len()
        );
        // Should have shown fewer than 100 lines (200-char lines can't all fit in 10KB).
        assert!(shown < 100, "expected byte truncation, shown={shown}");
    }

    // -- detect_terminal_filter --

    #[test]
    fn terminal_filter_grep() {
        let pos = detect_terminal_filter("cargo build 2>&1 | grep error");
        assert!(pos.is_some());
    }

    #[test]
    fn terminal_filter_head() {
        let pos = detect_terminal_filter("cat big_file.log | head -20");
        assert!(pos.is_some());
    }

    #[test]
    fn terminal_filter_tail() {
        let pos = detect_terminal_filter("journalctl | tail -100");
        assert!(pos.is_some());
    }

    #[test]
    fn terminal_filter_no_pipe() {
        assert!(detect_terminal_filter("cargo build").is_none());
    }

    #[test]
    fn terminal_filter_non_filter_pipe() {
        // Second stage is not a known filter
        assert!(detect_terminal_filter("cat file | cargo install").is_none());
    }

    #[test]
    fn terminal_filter_quoted_pipe_ignored() {
        // Pipe inside quotes is not a real pipe
        assert!(detect_terminal_filter("echo 'foo | bar'").is_none());
    }

    #[test]
    fn terminal_filter_nested_filters_last_wins() {
        // cmd | sed | grep  →  finds the grep (last) pipe
        let cmd = "cat file | sed 's/x/y/' | grep foo";
        let pos = detect_terminal_filter(cmd);
        assert!(pos.is_some());
        // The position should be the second pipe (before grep), not the first
        let pipe_pos = pos.unwrap();
        assert!(cmd[pipe_pos + 1..].trim_start().starts_with("grep"));
    }

    #[test]
    fn terminal_filter_returns_pipe_position() {
        let cmd = "cargo build | grep error";
        let pos = detect_terminal_filter(cmd).unwrap();
        // Character at pipe_pos should be '|'
        assert_eq!(&cmd[pos..pos + 1], "|");
    }

    #[test]
    fn terminal_filter_logical_or_not_a_pipe() {
        // || is logical OR, not a pipeline — should not trigger tee injection
        assert!(detect_terminal_filter("ls || grep foo").is_none());
    }

    #[test]
    fn terminal_filter_path_prefixed_filter() {
        // /usr/bin/grep should be recognized as grep
        let pos = detect_terminal_filter("ls | /usr/bin/grep foo");
        assert!(pos.is_some());
    }

    #[test]
    fn bash_stderr_pipe_not_treated_as_filter() {
        // |& is bash's stderr pipe — the & becomes part of the filter name
        // ("&grep"), which doesn't match any known filter, so correctly returns None
        assert!(detect_terminal_filter("cmd |& grep foo").is_none());
    }
}
