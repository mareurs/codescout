//! Command type detection and smart output summarization.
//!
//! Detects whether a command is a test runner, a build tool, or something else,
//! then produces a structured summary appropriate for that command type.

use regex::Regex;
use serde_json::{json, Value};
use std::sync::OnceLock;

use crate::util::text::{clip_to_bytes, elide_middle_bytes};

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

/// Byte ceiling on each stream (`stdout`, `stderr`) of a [`summarize_generic`] envelope.
///
/// The same defect `STDERR_SUMMARY_BYTE_BUDGET` was written for, on the shape it did not
/// reach: `summarize_generic` bounded its streams by LINE count only, and a line has no
/// length bound. A command printing one 95 KB JSON document, or twenty-five 1 KB lines,
/// satisfied `HEAD_LINES + TAIL_LINES` and came back verbatim — so the "summary" was the
/// whole output, the response re-buffered under a `@tool_*` handle, and the caller got
/// `format_run_command`'s one-line `✓ exit 0  (query @cmd_…)` plus a `json_path="$.field"`
/// hint that no `read_file` route can honour (the payload is a string, not a field).
///
/// Kept equal to `STDERR_SUMMARY_BYTE_BUDGET` on purpose: both answer "how much of one
/// stream is worth inlining beside an elided remainder". Two streams at this ceiling stay
/// under `TOOL_OUTPUT_BUFFER_THRESHOLD` even when every byte JSON-escapes to two.
/// A literal rather than an alias, because the cap-marker gate reads the declaration.
// cap-class: RESULT_CAP command_summary.generic_field_bytes — probed
const GENERIC_FIELD_BYTE_BUDGET: usize = 2000;

/// Byte ceiling on the `failures` field of a `test` envelope and the `first_error` field of
/// a `build` envelope.
///
/// The same defect as [`GENERIC_FIELD_BYTE_BUDGET`], on the two fields that one did not
/// reach. `extract_test_failures` returns the WHOLE failure section — no line bound, no byte
/// bound — and `extract_error_block` stops at a blank line, which is a structural bound and
/// not a size bound. A wide failure therefore pushed the envelope over
/// `TOOL_OUTPUT_BUFFER_THRESHOLD`, `call_content` re-buffered it under `@tool_*`, and the
/// failing text — the reason to read a red run — was left behind a handle whose
/// `json_path="$.field"` hint reaches nothing. Measured 2026-10-05: one 60 KB line in a
/// `failures:` block gave a 60,116 B `@tool_*` envelope.
///
/// Larger than the generic budget on purpose. Failure sections of 2–9 KB fit inline today
/// and are the useful middle of the range, so a 2,000 B cap would cut output that never
/// needed cutting. Sizing: this field sits beside at most one `stderr`
/// (`STDERR_SUMMARY_BYTE_BUDGET` 2,000 B plus its marker), so the worst raw envelope is
/// about 7,500 B, and it stays under the 10,000 B threshold unless JSON escaping inflates
/// the text by more than ~30%.
// cap-class: RESULT_CAP command_summary.failure_field_bytes — probed
const FAILURE_FIELD_BYTE_BUDGET: usize = 5000;

/// Leading token of the marker prefixed to a `stderr` field that was cut.
///
/// Distinct from `summarize_generic`'s `--- N lines omitted ---`, and the
/// distinction is the point: that marker elides a MIDDLE, this one drops a
/// HEAD. A reader who mistakes which end was cut goes looking for the missing
/// lines in the wrong place — and both markers sit in the same envelope.
pub(crate) const STDERR_TAIL_MARKER: &str = "--- stderr TAIL:";

/// Stands for the envelope's own `output_id` inside a summarized `stderr` field's remedy.
/// `summarize_stderr` cannot know the handle; `rebuild_buffered_summary` — which every
/// buffered envelope passes through, and which holds the id — replaces it. Chosen to read
/// sensibly if a caller ever renders the field without that pass.
pub(crate) const OUTPUT_ID_PLACEHOLDER: &str = "<output_id>";

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
    inline_response_exceeds_limit(0, stdout, stderr, 0)
}

/// Would the INLINE response for these streams be over the limit `call_content` enforces?
///
/// The summary-or-inline choice used to compare the RAW output length with the limit. But
/// `call_content` measures the serialized response, in which every quote and newline costs two
/// bytes and the keys cost another ~30. A 175-record pretty-JSON run (~9.3 KB raw, ~11.4 KB
/// serialized) therefore took the inline arm, was found over the limit one layer up, and was
/// buffered under `@tool_*` with no `@cmd_*` handle at all. Plain text between 9,977 and 10,003
/// raw bytes did the same.
///
/// This builds the object the inline arm returns — `exit_code`, plus `stdout` and `stderr` when
/// non-empty — and applies the SAME predicate (`exceeds_inline_limit_len`) to its serialized
/// length. `extras` is the byte cost of the keys added after the streams (diagnostics,
/// `unfiltered_output*`), measured by the caller who holds them. So the inline arm provably stays
/// within the limit `call_content` applies to the same value, and output that FITS is not
/// summarized: no reserve is subtracted, because a reserve would start buffering output that
/// fits today.
///
/// Escaping never shortens text, so raw bytes alone settle the large case without serializing.
pub(crate) fn inline_response_exceeds_limit(
    exit_code: i32,
    stdout: &str,
    stderr: &str,
    extras: usize,
) -> bool {
    use crate::tools::exceeds_inline_limit_len;
    if exceeds_inline_limit_len(stdout.len() + stderr.len() + extras) {
        return true;
    }
    let mut response = serde_json::Map::new();
    response.insert("exit_code".into(), json!(exit_code));
    if !stdout.is_empty() {
        response.insert("stdout".into(), json!(stdout));
    }
    if !stderr.is_empty() {
        response.insert("stderr".into(), json!(stderr));
    }
    exceeds_inline_limit_len(Value::Object(response).to_string().len() + extras)
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
/// **Cargo's progress lines are removed before the tail is taken.** A workspace
/// test run ends in one `Running …` line per target, and twenty of them fill the
/// line budget by themselves, so the warning above them was exactly the line cut.
/// A stderr of nothing but progress is omitted like an empty one; the raw stream
/// stays in the buffer either way.
///
/// The marker's remedy names `<output_id>.err` through [`OUTPUT_ID_PLACEHOLDER`],
/// which `rebuild_buffered_summary` replaces with the real handle. Until 2026-09-24
/// it said the full stderr was NOT in the buffer — true when written, and made false
/// by the `.err` suffix, so it sent readers away from the stream it described.
///
/// Returns `None` for empty stderr so the key is omitted rather than rendered
/// empty, matching [`summarize_generic`].
fn summarize_stderr(stderr: &str) -> Option<String> {
    if stderr.is_empty() {
        return None;
    }
    let all: Vec<&str> = stderr.lines().collect();
    let lines: Vec<&str> = all
        .iter()
        .copied()
        .filter(|l| !crate::tools::libtest_compact::is_cargo_progress_line(l))
        .collect();
    let progress = all.len() - lines.len();
    if lines.iter().all(|l| l.trim().is_empty()) {
        return None;
    }
    let total = lines.len();

    // Walk backwards, so when the byte ceiling binds it drops the OLDEST line
    // kept rather than the newest. Forwards, a long compile log would spend the
    // whole budget on its first lines and cut off exactly the verdict.
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
    if dropped == 0 && !clipped && progress == 0 {
        // Nothing lost: hand back the stream verbatim, trailing newline and all, so a
        // complete small stderr renders byte-identically to `summarize_generic`'s.
        return Some(stderr.to_string());
    }

    let mut notes: Vec<String> = Vec::new();
    if progress > 0 {
        notes.push(format!("{progress} cargo progress line(s) omitted"));
    }
    if dropped > 0 {
        notes.push(format!("{dropped} earlier line(s) dropped"));
    }
    if clipped {
        notes.push("last line clipped to the byte ceiling".to_string());
    }
    Some(format!(
        "{STDERR_TAIL_MARKER} {notes}; {shown} of {total} line(s) shown. \
         Full stderr: read_file(\"{OUTPUT_ID_PLACEHOLDER}.err\") ---\n{body}",
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
        // The section can come from either stream (`combined`), so the marker names both
        // handles rather than claiming one.
        let len = f.len();
        result["failures"] = Value::String(elide_middle_bytes(
            f,
            len,
            FAILURE_FIELD_BYTE_BUDGET,
            "failures",
            "all of it: output_id (stdout) or output_id.err (stderr)",
        ));
    }
    // A test harness writes its RESULTS to stdout, so a test run's stderr is the
    // compiler's diagnostics and any wrapper script's commentary — exactly what a
    // reader wants when the news is bad, and until 2026-09-14 the only shape that
    // never carried it. BUG docs/issues/archive/2026-09-14-run-commands-test-envelope-drops-the-stderr-a-wrapper-puts-its-verdict-on.md
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
        let len = err.len();
        result["first_error"] = Value::String(elide_middle_bytes(
            err,
            len,
            FAILURE_FIELD_BYTE_BUDGET,
            "first_error",
            "all of it: output_id (stdout) or output_id.err (stderr)",
        ));
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
///
/// **Lines are not the only bound.** A line has no length, so each stream is then held to
/// [`GENERIC_FIELD_BYTE_BUDGET`] bytes by [`elide_middle_bytes`]. Without that, one 95 KB
/// line of JSON is "1 line", comes back verbatim, and re-buffers the response under a
/// `@tool_*` handle that carries none of it. Streams within both bounds are unchanged.
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
    let summarized_stdout = elide_middle_bytes(
        summarized_stdout,
        stdout.len(),
        GENERIC_FIELD_BYTE_BUDGET,
        "stdout",
        "all of it: output_id",
    );

    let mut result = json!({
        "type": "generic",
        "exit_code": exit_code,
    });

    if !summarized_stdout.is_empty() {
        result["stdout"] = Value::String(summarized_stdout);
    }
    if !stderr.is_empty() {
        result["stderr"] = Value::String(elide_middle_bytes(
            stderr.to_string(),
            stderr.len(),
            GENERIC_FIELD_BYTE_BUDGET,
            "stderr",
            "all of it: output_id.err",
        ));
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

/// What [`truncate_lines_and_bytes`] kept of a text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LineCut {
    /// The kept text. Never empty when the input was not: see the function's doc.
    pub text: String,
    /// Lines of the input that `text` carries, counting a clipped line as shown.
    pub shown: usize,
    /// Lines in the input.
    pub total: usize,
    /// The FIRST line was wider than the whole budget and is shown clipped, behind a marker.
    /// Needed beside `shown`/`total` because a single wide line reads `1/1` and is still cut.
    pub clipped_wide: bool,
}

/// Escaped bytes held back for the marker [`clip_wide_line`] adds, before it measures the result.
const WIDE_LINE_MARKER_RESERVE: usize = 240;

/// Clip ONE over-wide line to at most `max_escaped` JSON-escaped bytes: head and tail with the
/// marker between. The text is clipped in raw bytes but MEASURED escaped, so a line dense in
/// quotes or newlines (which serialize to two bytes) is cut again until it fits.
///
/// Below `2 * WIDE_LINE_MARKER_RESERVE` there is no room for a marker beside any useful text, so
/// the head alone is returned: showing something beats showing nothing, and no caller in the
/// tree budgets that little.
fn clip_wide_line(line: &str, max_escaped: usize, remedy: &str) -> String {
    use crate::util::text::{clip_to_bytes, elide_middle_bytes, json_escaped_len};
    if max_escaped < 2 * WIDE_LINE_MARKER_RESERVE {
        return clip_to_bytes(line, max_escaped).to_string();
    }
    let mut budget = max_escaped - WIDE_LINE_MARKER_RESERVE;
    loop {
        let clipped = elide_middle_bytes(line.to_string(), line.len(), budget, "stdout", remedy);
        if json_escaped_len(&clipped) <= max_escaped || budget <= WIDE_LINE_MARKER_RESERVE {
            return clipped;
        }
        budget = budget * 3 / 4;
    }
}

/// Truncate `text` to at most `max_lines` lines **and** at most `max_bytes` JSON-ESCAPED bytes.
///
/// Both limits are applied; whichever is more restrictive wins. The unit is escaped bytes
/// (a quote or a newline costs two) because the cut text is what the caller serializes into a
/// response measured in exactly that unit; a raw-byte budget let ~100 lines of JSON-ish text
/// serialize 3% past the limit, enough to re-buffer the whole response.
///
/// Truncation occurs on a line boundary, with ONE exception, and it is the point of the
/// function's contract: **a non-empty input never yields an empty result.** When the first line
/// alone is wider than the budget it is shown clipped (head and tail, behind a marker naming
/// `wide_line_remedy`) and reported through `clipped_wide`. Before this, such a line produced
/// nothing, so a reader got `stdout_shown: 0, stdout_total: 1` and a hint to page forward with
/// `sed -n '1,100p'`, which returned the identical response — a loop. A WIDE LINE THAT IS NOT
/// FIRST is left alone: the lines before it already answered, and the next page starts at it.
pub(crate) fn truncate_lines_and_bytes(
    text: &str,
    max_lines: usize,
    max_bytes: usize,
    wide_line_remedy: &str,
) -> LineCut {
    use crate::util::text::json_escaped_len;
    let total = count_lines(text);
    let mut result = String::new();
    let mut escaped = 0usize;
    let mut shown = 0;
    let mut clipped_wide = false;

    for line in text.lines().take(max_lines) {
        // Each line adds the line itself plus a '\n' separator (except the first), and that
        // separator escapes to two bytes.
        let needed = json_escaped_len(line) + if shown == 0 { 0 } else { 2 };
        if escaped + needed > max_bytes {
            if shown == 0 {
                result = clip_wide_line(line, max_bytes, wide_line_remedy);
                shown = 1;
                clipped_wide = true;
            }
            break;
        }
        if shown > 0 {
            result.push('\n');
        }
        result.push_str(line);
        escaped += needed;
        shown += 1;
    }

    LineCut {
        text: result,
        shown,
        total,
        clipped_wide,
    }
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
    // -- inline_response_exceeds_limit: the gate measures the SERIALIZED response --
    //
    // The limit is on the compact JSON: `len / 4 > 2500`, so 10,003 B passes and 10,004 B does not.
    // `{"exit_code":0,"stdout":""}` is 27 B. Each case sits ON the edge so a drifted constant,
    // a forgotten key, or an unescaped byte moves a result across it.

    #[test]
    fn inline_gate_is_exact_at_the_edge_for_plain_stdout() {
        assert!(!inline_response_exceeds_limit(0, &"a".repeat(9_976), "", 0));
        assert!(inline_response_exceeds_limit(0, &"a".repeat(9_977), "", 0));
    }

    #[test]
    fn inline_gate_counts_the_escaped_size_not_the_raw_size() {
        // 4,988 quotes are 4,988 raw bytes and 9,976 serialized: 27 + 9,976 = 10,003, at the edge.
        assert!(!inline_response_exceeds_limit(
            0,
            &"\"".repeat(4_988),
            "",
            0
        ));
        assert!(inline_response_exceeds_limit(0, &"\"".repeat(4_989), "", 0));
        // Raw, 4,989 B is nowhere near the limit: the old gate called it small.
        assert!(!needs_summary_raw_for_comparison(&"\"".repeat(4_989), ""));
    }

    /// The comparison the gate REPLACED, kept so the test above states what changed.
    fn needs_summary_raw_for_comparison(stdout: &str, stderr: &str) -> bool {
        (stdout.len() + stderr.len()) / 4 > crate::tools::MAX_INLINE_TOKENS
    }

    #[test]
    fn inline_gate_counts_stderr_and_its_key() {
        // With stderr present the keys cost 39 B (`,"stderr":""` adds 12). 5,000 B of stderr
        // leaves 10,003 - 39 - 5,000 = 4,964 B for stdout.
        let stderr = "e".repeat(5_000);
        assert!(!inline_response_exceeds_limit(
            0,
            &"a".repeat(4_964),
            &stderr,
            0
        ));
        assert!(inline_response_exceeds_limit(
            0,
            &"a".repeat(4_965),
            &stderr,
            0
        ));
    }

    #[test]
    fn inline_gate_serializes_the_exit_code() {
        // 9,975 B of text is a 10,002 B response at exit 0, 10,003 at -1, 10,004 at 101: the
        // exit code's digits are part of the response and part of the gate.
        let stdout = "a".repeat(9_975);
        assert!(!inline_response_exceeds_limit(0, &stdout, "", 0));
        assert!(!inline_response_exceeds_limit(-1, &stdout, "", 0));
        assert!(inline_response_exceeds_limit(101, &stdout, "", 0));
    }

    #[test]
    fn inline_gate_adds_the_extras_the_caller_measured() {
        let stdout = "a".repeat(9_876);
        assert!(!inline_response_exceeds_limit(0, &stdout, "", 100));
        assert!(inline_response_exceeds_limit(
            0,
            &"a".repeat(9_877),
            "",
            100
        ));
        // And extras alone can tip an otherwise small response over.
        assert!(inline_response_exceeds_limit(0, "ok", "", 10_000));
    }

    #[test]
    fn inline_gate_leaves_output_that_fits_alone() {
        // No reserve is subtracted: a response that fits must not start being summarized.
        assert!(!inline_response_exceeds_limit(0, "hello\nworld\n", "", 0));
        assert!(!inline_response_exceeds_limit(0, "", "", 0));
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
    // -- summarize_generic: byte bound --
    //
    // BUG docs/issues/archive/2026-10-05-run-command-json-stdout-overflow-has-no-working-json-path-recovery.md
    //
    // A LINE budget does not bound a field: one 95 KB line is "1 line". These tests pin the
    // byte bound from BOTH ends of the kept text, because a bound that kept only the head
    // would satisfy every "is short" and "starts with" assertion and still drop the tail —
    // where a wrapper's verdict, or the end of a document, lives.

    /// Split a byte-elided field into `(head, shown, total, tail)`, asserting the marker's
    /// shape on the way. Panics (so the test reds with the field's real content) when the
    /// field was not elided.
    fn split_elided<'a>(field: &'a str, stream: &str) -> (&'a str, usize, usize, &'a str) {
        let open = format!("\n--- {stream}: ");
        let (head, rest) = field.split_once(&open).unwrap_or_else(|| {
            panic!(
                "no `{stream}` byte-elision marker in a field of {} bytes; starts {:?}",
                field.len(),
                field.chars().take(80).collect::<String>()
            )
        });
        let (marker, tail) = rest
            .split_once(" ---\n")
            .expect("marker is not terminated by ` ---` and a newline");
        let mut words = marker.split_whitespace();
        let shown: usize = words.next().and_then(|w| w.parse().ok()).expect("shown");
        assert_eq!(
            words.next(),
            Some("of"),
            "marker is `<shown> of <total> bytes`"
        );
        let total: usize = words.next().and_then(|w| w.parse().ok()).expect("total");
        (head, shown, total, tail)
    }

    /// One compact JSON array on one line — what `glab api`, `gh api` and `kubectl -o json`
    /// print.
    fn one_line_json_array(records: usize) -> String {
        let body: Vec<String> = (0..records)
            .map(|i| format!(r#"{{"name":"n{i}","status":"created"}}"#))
            .collect();
        format!("[{}]\n", body.join(","))
    }

    #[test]
    fn summarize_generic_bounds_one_enormous_stdout_line_by_bytes() {
        let stdout = one_line_json_array(2000);
        assert!(
            stdout.len() > 20 * GENERIC_FIELD_BYTE_BUDGET,
            "fixture must dwarf the budget"
        );
        assert_eq!(
            stdout.lines().count(),
            1,
            "one line satisfies any line budget"
        );

        let summary = summarize_generic(&stdout, "", 0);
        let field = summary["stdout"].as_str().unwrap();
        let (head, shown, total, tail) = split_elided(field, "stdout");

        assert_eq!(
            total,
            stdout.len(),
            "total is the ORIGINAL stream, not the summary"
        );
        assert_eq!(
            shown,
            head.len() + tail.len(),
            "`shown` must be what is actually shown"
        );
        assert!(shown <= GENERIC_FIELD_BYTE_BUDGET);
        // Both ends, taken from the right ends of the original.
        assert!(!head.is_empty() && stdout.starts_with(head));
        assert!(!tail.is_empty() && stdout.ends_with(tail));
        // The point of the bound: the whole envelope stays inline, so `call_content`
        // never re-buffers it under a `@tool_*` handle.
        let rendered = serde_json::to_string_pretty(&summary).unwrap();
        assert!(
            !crate::tools::exceeds_inline_limit(&rendered),
            "the envelope is {} bytes and would re-buffer",
            rendered.len()
        );
    }

    #[test]
    fn summarize_generic_bounds_a_few_wide_lines_by_bytes() {
        // Five lines: far under HEAD_LINES + TAIL_LINES, so only the byte bound can bind.
        let stdout: String = ('a'..='e')
            .map(|c| format!("{}\n", c.to_string().repeat(20_000)))
            .collect();
        let summary = summarize_generic(&stdout, "", 0);
        let (head, shown, total, tail) =
            split_elided(summary["stdout"].as_str().unwrap(), "stdout");

        assert_eq!(total, stdout.len());
        assert!(shown <= GENERIC_FIELD_BYTE_BUDGET);
        assert!(
            head.starts_with("aaaa"),
            "the head is the FIRST line's start"
        );
        assert!(
            tail.trim_end().ends_with("eeee"),
            "the tail is the LAST line's end"
        );
    }

    #[test]
    fn summarize_generic_reports_the_original_size_after_line_elision_too() {
        // 100 lines of ~500 B: the LINE summary applies (100 > 30) and its 30 kept lines
        // are still ~15 KB, so the byte bound applies on top. `total` must name the
        // original stream, not the already line-elided text it was cut from — otherwise
        // "N of M bytes shown" under-reports what is behind the handle.
        let stdout: String = (1..=100)
            .map(|i| format!("line {i:03} {}\n", "y".repeat(490)))
            .collect();
        let summary = summarize_generic(&stdout, "", 0);
        let (head, shown, total, tail) =
            split_elided(summary["stdout"].as_str().unwrap(), "stdout");

        assert_eq!(total, stdout.len());
        assert!(shown <= GENERIC_FIELD_BYTE_BUDGET);
        assert!(head.starts_with("line 001"));
        // The LINE summary joins its kept lines with `\n` and so drops the stream's trailing
        // newline; compare against the trimmed original, which is the same bytes up to it.
        assert!(
            stdout.trim_end().ends_with(tail),
            "the tail is the end of the original stream"
        );
    }

    #[test]
    fn summarize_generic_bounds_a_huge_stderr_and_leaves_a_short_stdout_alone() {
        // Stdout is tiny, so the STDERR bound is the only one that can refuse this input.
        let stderr = format!("{}\n", "e".repeat(60_000));
        let summary = summarize_generic("ok\n", &stderr, 1);

        assert_eq!(
            summary["stdout"].as_str().unwrap(),
            "ok\n",
            "a short stream is verbatim"
        );
        let field = summary["stderr"].as_str().unwrap();
        let (head, shown, total, tail) = split_elided(field, "stderr");
        assert_eq!(total, stderr.len());
        assert!(shown <= GENERIC_FIELD_BYTE_BUDGET);
        assert!(stderr.starts_with(head) && stderr.ends_with(tail));
        assert!(
            field.contains("output_id.err"),
            "stderr is read through the `.err` handle, and the marker must say so"
        );
        assert!(!crate::tools::exceeds_inline_limit(
            &serde_json::to_string_pretty(&summary).unwrap()
        ));
    }

    #[test]
    fn summarize_generic_cuts_on_char_boundaries() {
        // `€` is 3 bytes and each half-budget (1000) is not a multiple of 3, so a cut at a raw
        // byte offset lands inside a character — a slice there PANICS. The TAIL cut needs the
        // trailing `\n\n`: the text is then 3N+2 bytes and the tail starts at 3N+2-1000, which
        // is NOT a multiple of 3. With one `\n` it is 3N+1-1000 — always on a boundary — and
        // removing the boundary search survived a mutation run (M8, 2026-10-05).
        let stdout = format!("{}\n\n", "€".repeat(40_000));
        let summary = summarize_generic(&stdout, "", 0);
        let (head, shown, _total, tail) =
            split_elided(summary["stdout"].as_str().unwrap(), "stdout");

        assert!(shown <= GENERIC_FIELD_BYTE_BUDGET);
        assert!(head.chars().all(|c| c == '€'));
        assert!(tail.trim_end().chars().all(|c| c == '€'));
        assert!(!head.is_empty() && !tail.trim_end().is_empty());
    }

    #[test]
    fn summarize_generic_byte_bound_is_inclusive_at_the_budget() {
        // Exactly at the budget is verbatim; one byte over is elided. Pins `<=` against `<`.
        let at = "z".repeat(GENERIC_FIELD_BYTE_BUDGET);
        let summary = summarize_generic(&at, "", 0);
        assert_eq!(summary["stdout"].as_str().unwrap(), at);

        let over = "z".repeat(GENERIC_FIELD_BYTE_BUDGET + 1);
        let summary = summarize_generic(&over, "", 0);
        let (_head, _shown, total, _tail) =
            split_elided(summary["stdout"].as_str().unwrap(), "stdout");
        assert_eq!(total, over.len());
    }

    // -- summarized stderr on the test / build shapes --
    //
    // BUG docs/issues/archive/2026-09-14-run-commands-test-envelope-drops-the-stderr-a-wrapper-puts-its-verdict-on.md
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

    /// A workspace run's stderr tail is cargo's `Running …` line per target, and 20 of them
    /// fill the LINE budget alone — so the warning above them, the thing the tail exists to
    /// carry, is the line that gets cut. Load-bearing: 25 progress lines (over the 20-line
    /// budget) at ~60 B each (~1.5 KB, UNDER the 2 KB byte budget), so it is the line cap
    /// that would drop the warning, and removing progress first is the only thing that
    /// keeps it.
    #[test]
    fn cargo_progress_does_not_spend_the_stderr_tail_budget() {
        let mut stderr = String::from("warning: unused variable: `x`\n --> src/a.rs:1:5\n");
        for i in 1..=25 {
            stderr.push_str(&format!(
                "     Running tests/t{i:02}.rs (target/debug/deps/t{i:02}-00000000)\n"
            ));
        }
        assert!(
            stderr.len() < 2000,
            "fixture must stay under the BYTE budget"
        );

        let summary = summarize_test_output("running 0 tests\n", &stderr, 0);
        let rendered = summary["stderr"].as_str().unwrap();
        assert!(
            rendered.contains("warning: unused variable: `x`"),
            "the warning must survive; got {rendered:?}"
        );
        assert!(
            !rendered.contains("Running tests/"),
            "progress must not be shown"
        );
        assert!(
            rendered.contains("25 cargo progress line(s) omitted"),
            "the omission must be announced; got {rendered:?}"
        );
    }

    /// A green run's stderr is often nothing BUT progress; showing it is pure noise, and the
    /// raw stream stays in the buffer behind `.err`.
    #[test]
    fn a_stderr_of_only_cargo_progress_is_omitted() {
        let stderr =
            "    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.31s\n     \
                      Running unittests src/lib.rs (target/debug/deps/a-0000000000000001)\n     \
                      Running tests/b.rs (target/debug/deps/b-0000000000000002)\n";
        let summary = summarize_test_output("running 0 tests\n", stderr, 0);
        assert!(
            summary.get("stderr").is_none(),
            "progress-only stderr must not be rendered; got {:?}",
            summary.get("stderr")
        );
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
    // -- summarized `failures` / `first_error`: byte bound --
    //
    // BUG docs/issues/archive/2026-10-05-run-command-test-envelope-failures-field-has-no-byte-bound.md
    //
    // `extract_test_failures` returns the WHOLE failure section and `extract_error_block`
    // stops only at a blank line, so neither field had a size bound. The inputs below are
    // each over the budget in a way no other bound could refuse: a few wide lines, many
    // short ones, and the exact budget.

    /// A `cargo test` stdout with `n` failing tests, each carrying `detail` as its panic text.
    /// Shaped like libtest's own: the first `failures:` block holds each test's output, the
    /// second lists the failing names, and a `test result:` line closes it.
    fn failing_cargo_run(n: usize, detail: &str) -> String {
        let mut s = format!("running {n} tests\n");
        for i in 0..n {
            s.push_str(&format!("test t{i} ... FAILED\n"));
        }
        s.push_str("\nfailures:\n\n");
        for i in 0..n {
            s.push_str(&format!("---- t{i} stdout ----\n{detail}\n\n"));
        }
        s.push_str("failures:\n");
        for i in 0..n {
            s.push_str(&format!("    t{i}\n"));
        }
        s.push_str(&format!(
            "\ntest result: FAILED. 0 passed; {n} failed; 0 ignored; 0 measured; 0 filtered out\n"
        ));
        s
    }

    #[test]
    fn summarize_test_output_bounds_one_enormous_failure_line_by_bytes() {
        let stdout = failing_cargo_run(1, &"x".repeat(60_000));
        let summary = summarize_test_output(&stdout, "", 101);
        let (head, shown, total, tail) =
            split_elided(summary["failures"].as_str().unwrap(), "failures");

        assert_eq!(
            total,
            extract_test_failures(&stdout).unwrap().len(),
            "total is the extracted section as it was, not the cut text"
        );
        assert_eq!(shown, head.len() + tail.len());
        assert!(shown <= FAILURE_FIELD_BYTE_BUDGET);
        assert!(
            head.starts_with("failures:"),
            "the head is the section's start"
        );
        assert!(
            tail.ends_with("    t0"),
            "the tail must reach the list of failing names; got {:?}",
            tail.chars()
                .rev()
                .take(40)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>()
        );
        assert!(!crate::tools::exceeds_inline_limit(
            &serde_json::to_string_pretty(&summary).unwrap()
        ));
    }

    #[test]
    fn summarize_test_output_bounds_many_short_failures_by_bytes() {
        // 300 failures of ~30 B each: no line is wide, so only the byte bound can bind.
        let stdout = failing_cargo_run(300, "assertion failed: left == right");
        let summary = summarize_test_output(&stdout, "", 101);
        let (head, shown, total, tail) =
            split_elided(summary["failures"].as_str().unwrap(), "failures");

        assert_eq!(total, extract_test_failures(&stdout).unwrap().len());
        assert!(
            total > 3 * FAILURE_FIELD_BYTE_BUDGET,
            "fixture must dwarf the budget"
        );
        assert!(shown <= FAILURE_FIELD_BYTE_BUDGET);
        assert!(
            head.contains("---- t0 stdout ----"),
            "the FIRST failure's text survives"
        );
        assert!(tail.ends_with("    t299"), "the LAST failing name survives");
    }

    #[test]
    fn summarize_test_output_failures_bound_is_inclusive_at_the_budget() {
        // One `failures:` marker, so the section is the whole text: 9 B of marker, 1 B of
        // newline, then filler. Exactly the budget is verbatim; one byte over is elided.
        let at = format!("failures:\n{}", "z".repeat(FAILURE_FIELD_BYTE_BUDGET - 10));
        let summary = summarize_test_output(&at, "", 101);
        assert_eq!(summary["failures"].as_str().unwrap(), at);

        let over = format!("failures:\n{}", "z".repeat(FAILURE_FIELD_BYTE_BUDGET - 9));
        let summary = summarize_test_output(&over, "", 101);
        let (_h, _s, total, _t) = split_elided(summary["failures"].as_str().unwrap(), "failures");
        assert_eq!(total, over.len());
    }

    #[test]
    fn summarize_test_output_keeps_the_envelope_inline_with_failures_and_stderr_both_at_ceiling() {
        // The two budgets must SUM to something that fits: `failures` beside a stderr that is
        // itself at its ceiling. Quotes and newlines are what JSON escaping inflates.
        let stdout = failing_cargo_run(1, &"x".repeat(60_000));
        let stderr: String = (0..3000)
            .map(|i| format!("error: could not compile \"pkg{i}\" (lib) due to 1 previous error\n"))
            .collect();
        let summary = summarize_test_output(&stdout, &stderr, 101);

        assert!(summary["failures"]
            .as_str()
            .unwrap()
            .contains("bytes shown"));
        assert!(summary["stderr"]
            .as_str()
            .unwrap()
            .contains("--- stderr TAIL:"));
        let rendered = serde_json::to_string_pretty(&summary).unwrap();
        assert!(
            !crate::tools::exceeds_inline_limit(&rendered),
            "both fields at ceiling make a {} B envelope, over the re-buffer threshold",
            rendered.len()
        );
    }

    #[test]
    fn summarize_build_output_bounds_one_enormous_error_block_by_bytes() {
        let head_line = "error[E0308]: mismatched types";
        let stdout = format!("{head_line}\n{}\n", "x".repeat(60_000));
        let summary = summarize_build_output(&stdout, "", 101);
        let (head, shown, total, tail) =
            split_elided(summary["first_error"].as_str().unwrap(), "first_error");

        assert_eq!(
            total,
            head_line.len() + 1 + 60_000,
            "total is the block as extracted"
        );
        assert!(shown <= FAILURE_FIELD_BYTE_BUDGET);
        assert!(
            head.starts_with(head_line),
            "the head is the error line itself"
        );
        assert!(tail.ends_with("xxxx"), "the tail is the block's last bytes");
        assert!(!crate::tools::exceeds_inline_limit(
            &serde_json::to_string_pretty(&summary).unwrap()
        ));
    }
    #[test]
    fn summarize_build_output_first_error_bound_is_inclusive_at_the_budget() {
        // The block is the error line, a newline, then filler: exactly the budget is verbatim,
        // one byte over is elided. A boundary test per field, because a field wired to the
        // WRONG budget (the 2,000 B generic one) passes every "is under the budget" assertion
        // and is caught only by an input that sits between the two budgets (M14, 2026-10-05).
        let head_line = "error[E0308]: x";
        let at = format!(
            "{head_line}\n{}",
            "z".repeat(FAILURE_FIELD_BYTE_BUDGET - head_line.len() - 1)
        );
        let summary = summarize_build_output(&at, "", 101);
        assert_eq!(summary["first_error"].as_str().unwrap(), at);

        let over = format!("{at}z");
        let summary = summarize_build_output(&over, "", 101);
        let (_h, _s, total, _t) =
            split_elided(summary["first_error"].as_str().unwrap(), "first_error");
        assert_eq!(total, over.len());
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
        let cut = truncate_lines_and_bytes(text, 2, 1000, "R");
        assert_eq!(cut.text, "aaa\nbbb");
        assert_eq!(cut.shown, 2);
        assert_eq!(cut.total, 3);
    }

    #[test]
    fn truncate_lines_and_bytes_byte_limit_wins() {
        // 5 short lines, byte budget forces truncation after the 2nd line.
        // "aaa\nbbb" = 7 bytes; adding "\nccc" = 11 bytes — over the 8-byte budget.
        let text = "aaa\nbbb\nccc\nddd\neee";
        let cut = truncate_lines_and_bytes(text, 10, 8, "R");
        assert_eq!(cut.text, "aaa\nbbb");
        assert_eq!(cut.shown, 2);
        assert_eq!(cut.total, 5);
    }

    #[test]
    fn truncate_lines_and_bytes_both_fit() {
        let text = "a\nb\nc";
        let cut = truncate_lines_and_bytes(text, 10, 1000, "R");
        assert_eq!(cut.text, text);
        assert_eq!(cut.shown, 3);
        assert_eq!(cut.total, 3);
    }

    #[test]
    fn truncate_lines_and_bytes_long_lines_stay_under_byte_budget() {
        // Simulate log output: 200-char lines, budget = 10_000 bytes (TOOL_OUTPUT_BUFFER_THRESHOLD).
        let long_line = "x".repeat(200);
        let text: String = (0..100).map(|_| format!("{long_line}\n")).collect();
        let cut = truncate_lines_and_bytes(&text, 100, 10_000, "R");
        let (out, shown) = (cut.text, cut.shown);
        // Must fit within budget.
        assert!(
            out.len() <= 10_000,
            "output {} bytes exceeds budget",
            out.len()
        );
        // Should have shown fewer than 100 lines (200-char lines can't all fit in 10KB).
        assert!(shown < 100, "expected byte truncation, shown={shown}");
    }
    // -- truncate_lines_and_bytes: a non-empty input never yields an empty result --
    //
    // Found by the 2026-10-05 sibling sweep: a first line wider than the whole budget produced
    // NOTHING, so a buffer query read `stdout_shown: 0, stdout_total: 1` and was told to page with
    // `sed -n '1,100p'`, which returned the same thing. Each case below is an input every OTHER
    // rule admits, so only the rule it names can decide the outcome.

    fn wide(width: usize) -> String {
        format!("HEAD{}TAIL", "m".repeat(width))
    }

    #[test]
    fn truncate_lines_and_bytes_clips_a_first_line_wider_than_the_budget() {
        let cut = truncate_lines_and_bytes(&wide(5_000), 100, 1_000, "REMEDY-TEXT");
        assert!(cut.clipped_wide);
        assert_eq!(
            (cut.shown, cut.total),
            (1, 1),
            "a clipped line counts as shown"
        );
        assert!(cut.text.starts_with("HEAD") && cut.text.ends_with("TAIL"));
        assert!(cut.text.contains("bytes shown"), "a cut must say so");
        assert!(
            cut.text.contains("REMEDY-TEXT"),
            "the marker carries the caller's remedy"
        );
        assert!(crate::util::text::json_escaped_len(&cut.text) <= 1_000);
    }

    #[test]
    fn truncate_lines_and_bytes_leaves_a_wide_line_that_is_not_first_alone() {
        // The lines before it already answered, and the next page starts AT the wide line, where
        // it will be the first line and get clipped. Clipping it here would hide the boundary.
        let text = format!("short\n{}\nlast", wide(5_000));
        let cut = truncate_lines_and_bytes(&text, 100, 1_000, "R");
        assert_eq!(cut.text, "short");
        assert_eq!((cut.shown, cut.total, cut.clipped_wide), (1, 3, false));
    }

    #[test]
    fn truncate_lines_and_bytes_budget_is_inclusive_in_raw_bytes() {
        let line = "z".repeat(1_000);
        let at = truncate_lines_and_bytes(&line, 100, 1_000, "R");
        assert!(!at.clipped_wide, "exactly the budget fits");
        assert_eq!(at.text, line);
        let under = truncate_lines_and_bytes(&line, 100, 999, "R");
        assert!(under.clipped_wide, "one byte over is clipped");
    }

    #[test]
    fn truncate_lines_and_bytes_measures_escaped_bytes_not_raw() {
        // 400 double quotes are 400 raw bytes and 800 escaped. Raw measurement would fit this
        // under 700; escaped measurement must not.
        let line = "\"".repeat(400);
        assert!(truncate_lines_and_bytes(&line, 100, 700, "R").clipped_wide);
        let fits = truncate_lines_and_bytes(&line, 100, 800, "R");
        assert!(
            !fits.clipped_wide && fits.text == line,
            "exactly 800 escaped bytes fits"
        );
    }

    #[test]
    fn truncate_lines_and_bytes_recuts_a_clipped_line_that_escapes_past_the_budget() {
        // The clip is made in raw bytes and measured escaped: a line of quotes doubles, so the
        // first cut is still too big and must be shrunk again.
        let line = format!("<{}>", "\"".repeat(10_000));
        let cut = truncate_lines_and_bytes(&line, 100, 2_000, "R");
        assert!(cut.clipped_wide);
        assert!(
            crate::util::text::json_escaped_len(&cut.text) <= 2_000,
            "escaped {} bytes",
            crate::util::text::json_escaped_len(&cut.text)
        );
    }

    #[test]
    fn truncate_lines_and_bytes_with_no_room_for_a_marker_returns_the_head_not_nothing() {
        let cut = truncate_lines_and_bytes(&wide(5_000), 100, 100, "R");
        assert!(cut.clipped_wide);
        assert_eq!(cut.text.len(), 100);
        assert!(cut.text.starts_with("HEAD"));
    }

    #[test]
    fn truncate_lines_and_bytes_clips_on_char_boundaries() {
        // `€` is three bytes: a cut at a raw offset inside one would panic.
        let line = format!("{}\n", "€".repeat(5_000));
        let cut = truncate_lines_and_bytes(&line, 100, 1_000, "R");
        assert!(cut.clipped_wide && !cut.text.is_empty());
        assert!(cut.text.starts_with('€') && cut.text.ends_with('€'));
    }
    #[test]
    fn truncate_lines_and_bytes_charges_two_escaped_bytes_for_the_newline_between_lines() {
        // Three 3-byte lines: 3 + (2+3) + (2+3) = 13 escaped bytes. A separator charged as one
        // byte would total 11 and fit a budget of 12; charged as two it must not.
        let text = "aaa\nbbb\nccc";
        let at = truncate_lines_and_bytes(text, 100, 13, "R");
        assert_eq!((at.shown, at.text.as_str()), (3, text), "13 fits exactly");
        let under = truncate_lines_and_bytes(text, 100, 12, "R");
        assert_eq!(
            under.shown, 2,
            "12 does not: the separator escapes to two bytes"
        );
    }

    #[test]
    fn truncate_lines_and_bytes_budget_at_the_marker_reserve_boundary_still_marks_the_cut() {
        // Exactly `2 * WIDE_LINE_MARKER_RESERVE` is the smallest budget with room for a marker;
        // one below it returns the bare head. Both sides pinned so the comparison cannot drift.
        let at = truncate_lines_and_bytes(&wide(5_000), 100, 2 * WIDE_LINE_MARKER_RESERVE, "R");
        assert!(
            at.text.contains("bytes shown"),
            "room for a marker: {:.80}",
            at.text
        );
        let below =
            truncate_lines_and_bytes(&wide(5_000), 100, 2 * WIDE_LINE_MARKER_RESERVE - 1, "R");
        assert!(
            !below.text.contains("bytes shown"),
            "no room: the head alone"
        );
        assert_eq!(below.text.len(), 2 * WIDE_LINE_MARKER_RESERVE - 1);
    }
    #[test]
    fn truncate_lines_and_bytes_uses_nearly_all_of_the_budget_for_a_clipped_line() {
        // The clip must show about as much as fits, not a quarter less. Starting the clip at the
        // whole budget and letting the re-measure loop shrink it by thirds still returns a text
        // that fits, so every "is under the budget" assertion passes; only a lower bound on how
        // much was KEPT tells them apart (mutation T9, 2026-10-05).
        let budget = 10_000;
        let cut = truncate_lines_and_bytes(&wide(30_000), 100, budget, "R");
        let kept = crate::util::text::json_escaped_len(&cut.text);
        assert!(kept <= budget);
        assert!(
            kept >= budget - 2 * WIDE_LINE_MARKER_RESERVE,
            "kept {kept} of {budget}: the clip left more than the marker's reserve unused"
        );
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
