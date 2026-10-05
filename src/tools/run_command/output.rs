//! Output formatting and buffering helpers for run_command.

use serde_json::{json, Value};

use super::super::ToolContext;
use super::inner::TmpfileGuard;
use crate::util::redact::{note_in, redact_credentials};

/// Reassemble a buffered command summary with a stable, reader-friendly field order.
///
/// Dynamic field appending (`obj["key"] = val`) always places fields last, which
/// caused `output_id` (the buffer reference) to land after `stdout`/`failures`/
/// `first_error` (the bulk content). Correct order:
///   type → exit_code → output_id → [counts] → [content]
pub(crate) fn rebuild_buffered_summary(raw: Value, output_id: &str) -> Value {
    // These are large text fields — always go last.
    const CONTENT_FIELDS: &[&str] = &["stdout", "failures", "first_error"];

    let mut map = serde_json::Map::new();

    // 1. Status identity
    if let Some(v) = raw.get("type") {
        map.insert("type".into(), v.clone());
    }
    if let Some(v) = raw.get("exit_code") {
        map.insert("exit_code".into(), v.clone());
    }

    // 2. Buffer reference — most action-relevant, agent needs this to query results
    map.insert("output_id".into(), json!(output_id));

    // 3. Type-specific compact fields (counts, not content)
    let raw_obj = raw.as_object().expect("summary is always an object");
    for (k, v) in raw_obj {
        if !["type", "exit_code"].contains(&k.as_str()) && !CONTENT_FIELDS.contains(&k.as_str()) {
            map.insert(k.clone(), v.clone());
        }
    }

    // 4. Content fields last — bulk payload
    for field in CONTENT_FIELDS {
        if let Some(v) = raw_obj.get(*field) {
            map.insert((*field).into(), v.clone());
        }
    }

    // 5. The stderr tail's remedy names `<output_id>.err`; this is the one place that
    // holds the id, so the placeholder becomes a call the reader can run as written.
    if let Some(Value::String(s)) = map.get_mut("stderr") {
        *s = s.replace(
            crate::tools::command_summary::OUTPUT_ID_PLACEHOLDER,
            output_id,
        );
    }

    Value::Object(map)
}

/// The inline response for a libtest run that `compact_libtest_output` accepted.
///
/// The raw streams go into a `@cmd_*` buffer — the same handle a buffered run gets — so
/// nothing the command printed is lost; the reader gets the compacted text with a trailer
/// naming that handle, plus the `passed`/`failed`/`ignored` counts `format_run_command`
/// renders for `type: "test"`. Every diagnostic was already computed from the raw streams
/// by the caller, so none of them can be silenced here.
fn compacted_test_response(
    c: crate::tools::libtest_compact::CompactedTest,
    command: &str,
    raw_stdout: &str,
    raw_stderr: &str,
    exit_code: i32,
    ctx: &ToolContext,
) -> Value {
    let output_id = ctx.output_buffer.store(
        command.to_string(),
        raw_stdout.to_string(),
        raw_stderr.to_string(),
        exit_code,
    );
    let mut summary =
        super::super::command_summary::summarize_test_output(raw_stdout, raw_stderr, exit_code);
    // The compacted text already carries the failure detail and the stderr, so the
    // summarizer's excerpts of both would say it twice.
    if let Some(obj) = summary.as_object_mut() {
        obj.remove("failures");
        obj.remove("stderr");
    }
    let trailer = c.trailer(&output_id);
    summary["stdout"] = json!(if c.stdout.is_empty() {
        trailer
    } else {
        format!("{}\n{trailer}", c.stdout)
    });
    if !c.stderr.is_empty() {
        summary["stderr"] = json!(c.stderr);
    }
    rebuild_buffered_summary(summary, &output_id)
}

/// Name the cause when the shell performed command substitution the caller did not intend.
///
/// `run_command` hands its string to `sh -c` verbatim, so a backtick or `$(…)` is evaluated
/// **even inside a quoted argument**. The commonest victim in this repo is a commit message:
/// the house style cites symbols and paths in backticks, and `git commit -m "…"` is the
/// most frequent multi-line command in the workflow, so the two collide constantly.
///
/// What makes this worth a dedicated diagnostic rather than leaving stderr to speak: the
/// shell emits a cluster of unrelated-looking errors and often *ends* with
/// `Argument list too long`. That line is a plausible, self-consistent, and wrong
/// explanation — the message genuinely was long — and acting on it means shortening the
/// commit body, which fixes nothing and loses content. The real cause sits mid-cluster in
/// `sh: command substitution:`, which reads as noise unless you already know.
///
/// Detection anchors on the **shell's own marker**, not on command shape. A command that
/// genuinely wanted substitution and got it emits no such marker, so this cannot fire on
/// working substitution — and no repair is attempted, per the repair-and-continue rule that
/// a write must never have its target guessed
/// (`docs/adrs/2026-07-10-repair-and-continue-input-handling.md`).
///
/// See `docs/issues/archive/2026-08-16-run-command-backticks-substituted-in-quoted-message.md`.
/// Name an empty test selection, which libtest reports with the same word and the same exit
/// code as a real pass.
///
/// `cargo test <filter>` is a **selector** over the test namespace. A filter matching nothing
/// selects an empty set, and the harness then reports success over that empty set:
/// `test result: ok.`, exit `0`. Nothing distinguishes *"every test I asked for passed"* from
/// *"I asked for a test that does not exist"*, so a verification step silently becomes a no-op
/// that certifies itself. In the reported incident the filter named a **helper function**
/// rather than a test; the caller read `ok` as verification and was one step from committing
/// on it.
///
/// What makes this worth a dedicated diagnostic rather than leaving the numbers to speak: the
/// discriminating field is **already printed**. `filtered out: N` beside `0 passed` is the
/// tell, on screen, on every run, and unread. That is the same shape as
/// `substitution_diagnostic`'s `Argument list too long` — the misleading signal and the real
/// one arrive together, and the misleading one reads like an explanation.
///
/// **Decided over the WHOLE output, never one `test result:` line at a time.** `cargo test
/// <filter>` builds every target in the workspace and each prints its own summary, so a target
/// holding no match prints `0 passed; N filtered out` *legitimately* while a sibling runs the
/// match. A per-line predicate would fire on every successful filtered workspace run, which is
/// the one failure mode that gets a warning ignored rather than read.
///
/// Anchored on libtest's own summary rather than on command shape, per the rule
/// `substitution_diagnostic` follows: an unfiltered run cannot reach `filtered out > 0`, so the
/// output alone proves the selection was empty and the caller's command never needs parsing.
///
/// Silent on a non-`ok.` summary (a red is loud already and `wip_authors` routes it) and on a
/// selection whose every match was `#[ignore]`d (that selection was not empty, and its remedy
/// is `-- --ignored`, so claiming otherwise would send the reader somewhere useless).
///
/// See `docs/issues/archive/2026-09-13-a-test-filter-that-matches-nothing-reports-success.md`.
pub(crate) fn empty_test_selection_diagnostic(stdout: &str) -> Option<String> {
    let (passed, ignored, filtered) = test_summary_counts(stdout)?;

    if filtered == 0 || passed > 0 || ignored > 0 {
        return None;
    }

    Some(format!(
        "This run selected NO tests: `{filtered} filtered out` against `0 passed`, so the `ok` \
         and the exit code report success over an EMPTY SET and would read identically if the \
         code were broken. It is not evidence that anything was verified. Check that the filter \
         names a test rather than a helper function or a module, or drop it and run the whole \
         target, which cannot select empty."
    ))
}

/// Sum libtest's own `test result:` summary line(s) across a whole `cargo test` invocation's
/// stdout — `(passed, ignored, filtered)`. Shared by [`empty_test_selection_diagnostic`] (the
/// full-miss case) and [`partial_test_selection_diagnostic`] (the partial-miss case): both need
/// the same counts, and both must go silent under the same condition.
///
/// `None` when no `test result:` line was seen at all, OR when any target's summary was not
/// `ok.` (a `FAILED` target bails the WHOLE function, not just that line) — a red is loud
/// already, per both diagnostics' own doc comments.
fn test_summary_counts(stdout: &str) -> Option<(usize, usize, usize)> {
    let (mut passed, mut ignored, mut filtered) = (0usize, 0usize, 0usize);
    let mut saw_summary = false;

    for line in stdout.lines() {
        let Some(rest) = line.trim_start().strip_prefix("test result:") else {
            continue;
        };
        // `FAILED.` and friends: not a vacuous pass, and already loud.
        let rest = rest.trim_start().strip_prefix("ok.")?;
        saw_summary = true;
        for field in rest.split(';') {
            let Some((n, label)) = field.trim().split_once(' ') else {
                continue;
            };
            let Ok(n) = n.parse::<usize>() else {
                continue;
            };
            match label {
                "passed" => passed += n,
                "ignored" => ignored += n,
                "filtered out" => filtered += n,
                _ => {}
            }
        }
    }

    saw_summary.then_some((passed, ignored, filtered))
}

/// Extract the free-standing (non-flag) name filters a `cargo test … -- …` invocation supplies
/// to the test binary, plus the same invocation with its filters swapped for `--list` — or
/// `None` when the command does not look like a `cargo test` run naming two or more filters.
///
/// **Heuristic, not a shell parser**: splits on ASCII whitespace, so a quoted filter containing
/// a space would confuse it. Rust test names are `module::path::ident` and never contain spaces,
/// which is the shape this bug and its reproduction are both about.
///
/// Gated at two-or-more filters deliberately: a single named filter's total miss is already
/// `empty_test_selection_diagnostic`'s to report, and the ordinary one-name-per-run shape is by
/// far the commonest `cargo test` invocation — this is what keeps the `--list` cost rare rather
/// than paid on every filtered run.
fn multi_filter_test_command(original_command: &str) -> Option<(Vec<String>, String)> {
    if !original_command.contains("cargo test") {
        return None;
    }
    // Split a GLUED trailing `;` into its own token first — `2>&1;` and `bb;` both carry the
    // separator attached, and `bb` is a real filter that dropping the whole token would lose.
    let tokens: Vec<&str> = original_command
        .split_whitespace()
        .flat_map(|t| match t.strip_suffix(';') {
            Some(stem) if !stem.is_empty() => vec![stem, ";"],
            _ => vec![t],
        })
        .collect();

    // Split into shell SEGMENTS before looking for `--`, and read only the segment that runs
    // `cargo test`. Without this the scan ran to the end of the line, so a piped `grep`, a
    // redirect target or a whole `; next-command` were reported as filters that "matched
    // NOTHING", and `rposition("--")` could pick a LATER command's `--` (`; git log -- src`).
    // docs/issues/archive/2026-09-24-partial-test-selection-reads-shell-pipeline-tokens-as-test-filters.md
    let is_boundary = |t: &str| matches!(t, "|" | "||" | "&&" | ";" | "&");
    let mut start = 0;
    let mut segment = None;
    for (i, t) in tokens.iter().enumerate() {
        let boundary = is_boundary(t);
        if boundary || i + 1 == tokens.len() {
            let end = if boundary { i } else { i + 1 };
            if tokens[start..end]
                .windows(2)
                .any(|w| w[0] == "cargo" && w[1] == "test")
            {
                segment = Some((start, end));
                break;
            }
            start = i + 1;
        }
    }
    let (seg_start, seg_end) = segment?;
    let dash_idx = seg_start
        + tokens[seg_start..seg_end]
            .iter()
            .rposition(|&t| t == "--")?;

    // Within the segment, a redirection is not a filter either: `2>&1`, `>file`, and a bare
    // `>` / `2>` whose target is the NEXT token.
    let mut filters = Vec::new();
    let mut skip_target = false;
    for t in &tokens[dash_idx + 1..seg_end] {
        if skip_target {
            skip_target = false;
            continue;
        }
        if t.contains('>') || t.contains('<') {
            skip_target = t.ends_with('>') || t.ends_with('<');
            continue;
        }
        if !t.starts_with('-') {
            filters.push(t.to_string());
        }
    }
    if filters.len() < 2 {
        return None;
    }
    // Everything up to and including the cargo segment's `--`, so a `cd sub &&` prefix still
    // runs the listing in the right directory.
    let list_command = format!("{} --list", tokens[..=dash_idx].join(" "));
    Some((filters, list_command))
}

/// Run the assembled `cargo test … -- --list` command and return its raw stdout, or `None` on
/// any failure to introspect (spawn error, non-zero exit, timeout). Split out from
/// [`partial_test_selection_diagnostic_with`] so tests exercise the diff logic against a canned
/// listing instead of spawning a real `cargo test -- --list` — slow, environment-dependent, and
/// no part of what this bug is about.
async fn run_list_command(list_command: String, work_dir: std::path::PathBuf) -> Option<String> {
    let mut cmd = tokio::process::Command::new("sh");
    cmd.arg("-c")
        .arg(&list_command)
        .current_dir(&work_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    let child = cmd.spawn().ok()?;
    let out =
        match tokio::time::timeout(std::time::Duration::from_secs(30), child.wait_with_output())
            .await
        {
            Ok(Ok(o)) if o.status.success() => o,
            _ => return None,
        };
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Name any `cargo test` filter that matched NOTHING when other named filters in the same
/// invocation matched something — the partial-miss half `empty_test_selection_diagnostic`
/// cannot cover, because libtest's own summary has no per-filter breakdown (see that function's
/// sibling doc comment and `docs/issues/archive/2026-09-16-a-partial-test-filter-reports-success-over-a-silently-narrowed-selection.md`).
///
/// Pays for `cargo test … -- --list` (enumerate test names, no run) ONLY when
/// [`multi_filter_test_command`] finds two-or-more named filters in `original_command` AND the
/// run actually produced an `ok.` summary with `passed > 0 || ignored > 0` — i.e. never on the
/// ordinary single-filter run, never on a full miss (already named elsewhere), never on a red.
pub(crate) async fn partial_test_selection_diagnostic(
    original_command: &str,
    stdout: &str,
    work_dir: &std::path::Path,
) -> Option<String> {
    partial_test_selection_diagnostic_with(original_command, stdout, work_dir, run_list_command)
        .await
}

/// The body of [`partial_test_selection_diagnostic`], with the `--list` invocation as a
/// parameter — split out exactly as `wip_author_diagnostic`/`wip_author_diagnostic_with` are, so
/// a test can drive the diff logic with a fake listing rather than a real subprocess.
async fn partial_test_selection_diagnostic_with<F, Fut>(
    original_command: &str,
    stdout: &str,
    work_dir: &std::path::Path,
    lister: F,
) -> Option<String>
where
    F: FnOnce(String, std::path::PathBuf) -> Fut,
    Fut: std::future::Future<Output = Option<String>>,
{
    let (passed, ignored, filtered) = test_summary_counts(stdout)?;
    // A full miss is `empty_test_selection_diagnostic`'s to report — stay out of its way.
    if filtered == 0 || (passed == 0 && ignored == 0) {
        return None;
    }

    let (filters, list_command) = multi_filter_test_command(original_command)?;
    let listing = lister(list_command, work_dir.to_path_buf()).await?;
    let listed_names: Vec<&str> = listing
        .lines()
        .filter_map(|l| {
            l.strip_suffix(": test")
                .or_else(|| l.strip_suffix(": benchmark"))
        })
        .collect();
    if listed_names.is_empty() {
        return None;
    }

    let unmatched: Vec<&String> = filters
        .iter()
        .filter(|f| !listed_names.iter().any(|n| n.contains(f.as_str())))
        .collect();
    if unmatched.is_empty() {
        return None;
    }

    Some(format!(
        "This run named {total} test filters but {bad_n} matched NOTHING in `cargo test … -- \
         --list`: {bad}. The filters that DID resolve passed, and the exit code reports success \
         over that narrowed set — it says nothing about the filter(s) that selected empty. \
         Check for a rename, deletion or typo.",
        total = filters.len(),
        bad_n = unmatched.len(),
        bad = unmatched
            .iter()
            .map(|s| format!("`{s}`"))
            .collect::<Vec<_>>()
            .join(", "),
    ))
}

pub(crate) fn substitution_diagnostic(command: &str, stderr: &str) -> Option<String> {
    if !stderr.contains("command substitution:") {
        return None;
    }
    // Only claim a cause we can point at in the command the caller actually sent.
    let culprit = if command.contains('`') {
        "a backtick"
    } else if command.contains("$(") {
        "a `$(...)`"
    } else {
        return None;
    };

    let mut msg = format!(
        "The shell performed command substitution on {culprit} in this command. \
         run_command passes the string to `sh -c` verbatim, so backticks and `$(...)` are \
         evaluated even inside a quoted argument."
    );
    if stderr.contains("Argument list too long") {
        msg.push_str(
            " The `Argument list too long` line is a CONSEQUENCE of that substitution, not \
             the cause — shortening the argument will not fix it.",
        );
    }
    msg.push_str(
        " For a commit message, write it to a file and use `git commit -F <file>`, or \
         single-quote the message (escaping any inner single quotes).",
    );
    Some(msg)
}

/// Byte ceiling on the STORED stderr a buffer query carries back.
///
/// A line budget alone does not bound it, because a line has no length: `STDERR_BUDGET` (20
/// lines) let one 50 KB stderr line through whole, the response crossed the inline limit, and
/// `call_content` buffered it under `@tool_*` — hiding the query's own answer, often a bare `0`.
/// The comment above that cap said it prevented exactly this; it did not for a wide line.
/// Measured 2026-10-05: `grep -c zzz @cmd_X` on a buffer with a 50,001 B stderr line returned a
/// 50,044 B `@tool_*` envelope.
///
/// Equal to `STDERR_SUMMARY_BYTE_BUDGET` on purpose (both answer "how much stderr is worth
/// inlining beside the stream a reader came for") and a literal rather than an alias, because
/// the cap-marker gate reads the declaration. Two such fields beside a stdout still fit under
/// `TOOL_OUTPUT_BUFFER_THRESHOLD`: the stdout budget below is computed from what this ACTUALLY
/// emitted.
// cap-class: RESULT_CAP run_command.buffer_stderr_bytes — probed
pub(super) const BUFFER_STDERR_BYTE_BUDGET: usize = 2000;

/// Escaped bytes a buffer-query response spends on everything that is NOT the stdout and stderr
/// text: the keys (`exit_code`, `truncated`, `stdout_shown`, `stdout_total`, `stderr_shown`,
/// `stderr_total`, `hint`, about 160 B) and the longest hint `capped_hint` composes (about 425 B
/// with the wide-line sentence and a 7-digit total). 300 B was reserved here before, which was
/// below even the old hint plus its keys, so a truncated response could serialize past the
/// inline limit by the difference and be re-buffered under `@tool_*`. Pinned by
/// `the_buffer_query_overhead_covers_the_longest_hint_and_every_key`.
const BUFFER_QUERY_JSON_OVERHEAD: usize = 800;

/// The handle the caller's buffer query names (`@cmd_0bdbc0aa`), read from the command text.
/// `@file_*` handles count too; an `.err` suffix is dropped, so the result is always the bare
/// handle and a caller can append `.err` or not.
fn queried_ref(command: &str) -> Option<&str> {
    let start = command
        .find("@cmd_")
        .or_else(|| command.find("@file_"))
        .or_else(|| command.find("@tool_"))?;
    let rest = &command[start..];
    let end = rest
        .char_indices()
        .skip(1)
        .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '_'))
        .map_or(rest.len(), |(i, _)| i);
    Some(&rest[..end])
}

/// The stored stderr a buffer query carries: at most `max_lines` lines AND at most
/// `BUFFER_STDERR_BYTE_BUDGET` bytes, the latter by eliding the MIDDLE behind a marker that names
/// the `.err` handle holding all of it. Returns `(text, lines_shown, lines_total)` like
/// `truncate_lines`; `lines_total` counts the stored stream, so a wide single line reads `1/1`
/// and the marker, which travels with the data, is what says it was cut.
fn bound_buffer_stderr(stored: &str, max_lines: usize, query: &str) -> (String, usize, usize) {
    let (by_lines, shown, total) = crate::tools::command_summary::truncate_lines(stored, max_lines);
    let remedy = match queried_ref(query) {
        Some(handle) => format!("all of it: {handle}.err"),
        None => "all of it: the stored buffer's `.err` handle".to_string(),
    };
    let bounded = crate::util::text::elide_middle_bytes(
        by_lines,
        stored.len(),
        BUFFER_STDERR_BYTE_BUDGET,
        "stderr",
        &remedy,
    );
    (bounded, shown, total)
}

/// The line the queried range starts at: `A` for `sed -n 'A,Bp' @ref`, else 1.
///
/// A page hint counts lines of the OUTPUT it follows. For a whole-buffer read that is the buffer's
/// own numbering; for a ranged `sed` it starts at `A`, so "next page: sed -n '1,100p'" sent a
/// reader that had just read lines 2..101 back to the top. Only this one literal shape is
/// recognised, because it is the shape the hint itself advises; any other command keeps the
/// old relative numbering.
fn query_first_line(command: &str) -> usize {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"sed\s+-n\s+'?(\d+),\d+p'?").expect("static pattern"))
        .captures(command)
        .and_then(|c| c[1].parse::<usize>().ok())
        .map_or(1, |a| a.max(1))
}

/// How to read a window of a line wider than the response budget, in terms of the ref the caller
/// queried. These are the routes that work on a wide line: `sed -n` and a bare `grep` return the
/// whole line or nothing, `jq` needs JSON.
fn wide_line_remedy(command: &str) -> String {
    let r = queried_ref(command).unwrap_or("@ref");
    format!(
        "this line is wider than the response budget; read a window of it with \
         `grep -o 'TEXT.\\{{0,200\\}}' {r}` or `cut -c1-4000 {r}`"
    )
}

/// The hint on a capped buffer query: the next page, in the buffer's own line numbers, and the
/// route for a wide first line when one was clipped.
fn capped_hint(
    query: &str,
    shown: usize,
    total: usize,
    stderr_note: &str,
    clipped_wide: bool,
) -> String {
    use crate::tools::command_summary::BUFFER_QUERY_INLINE_CAP;
    let r = queried_ref(query).unwrap_or("@ref");
    let first = query_first_line(query);
    let next_start = first + shown;
    let next_end = first - 1 + shown + BUFFER_QUERY_INLINE_CAP;
    let mut hint = format!(
        "Output capped at {BUFFER_QUERY_INLINE_CAP} lines \
         (stdout {shown}/{total}{stderr_note}). \
         Next page: sed -n '{next_start},{next_end}p' {r}. \
         Or grep 'keyword' {r} for targeted search."
    );
    if clipped_wide {
        hint.push_str(&format!(
            " The first line is wider than the response budget and is shown clipped: read a \
             window of it with `grep -o 'TEXT.\\{{0,200\\}}' {r}` or `cut -c1-4000 {r}`."
        ));
    }
    hint
}

/// The hint on the banded arm (output a few hundred bytes under the summary threshold), chosen by
/// the KIND of ref the caller queried. `read_file(.., json_path=..)` reads `@tool_*` refs only and
/// is refused on `@cmd_*` / `@file_*`; this arm used to say it for every ref, with a literal
/// `@tool_abc` in place of the handle, so on the refs `run_command` normally takes it named a
/// route that could not work.
fn banded_hint(query: &str, shown: usize, total: usize, clipped_wide: bool) -> String {
    match queried_ref(query) {
        Some(r) if r.starts_with("@tool_") => format!(
            "Output cut to fit the response ({shown}/{total} lines shown). This ref holds compact \
             JSON: read one field with read_file(\"{r}\", json_path=\"$.<field>\"), or browse the \
             pretty-printed result with read_file(\"{r}\", start_line=N, end_line=M)."
        ),
        _ => capped_hint(query, shown, total, "", clipped_wide),
    }
}

/// Serialized bytes `unfiltered_output`, `unfiltered_output_lines` and the two truncation flags add
/// when a tee capture is attached: the handle (`@cmd_` + 8 hex), the line count, and the keys.
const TEE_KEYS_LEN: usize = 160;

/// Serialized bytes the `stderr_shown` / `stderr_total` counters add to a buffer-query response.
const BUFFER_QUERY_COUNTER_KEYS_LEN: usize = 48;

/// Bytes `handle_successful_output` adds to the response BEYOND the two streams, for the summary-or-
/// inline gate: each present diagnostic (`,"key":"` + its escaped text + `"`), the tee keys when a
/// capture is attached, and the counters a buffer query carries. Without it the gate judged the
/// streams alone and a failing run whose `wip_authors` was 3 KB could be called small.
fn response_extras_len(
    diagnostics: &[(&str, Option<&str>)],
    tee: bool,
    buffer_only: bool,
) -> usize {
    let diag: usize = diagnostics
        .iter()
        .filter_map(|(key, value)| {
            value.map(|v| key.len() + 6 + crate::util::text::json_escaped_len(v))
        })
        .sum();
    diag + if tee { TEE_KEYS_LEN } else { 0 }
        + if buffer_only {
            BUFFER_QUERY_COUNTER_KEYS_LEN
        } else {
            0
        }
}

/// Build the response for a command that ran to completion — at any exit code.
///
/// The name says "successful" about the *process*, not the *outcome*: a failing
/// `cargo test` reaches here too, and that is the path `wip_authors` exists for.
///
/// Eight parameters against clippy's seven. Bundling them into a struct was considered
/// and rejected: every one is consumed exactly once by a different concern (two move
/// large `String`s, one is a drop guard), so a struct buys a name and costs the move
/// semantics that keep the copies down on the hot path.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_successful_output(
    original_command: &str,
    raw_stdout: String,
    raw_stderr: String,
    exit_code: i32,
    buffer_only: bool,
    unfiltered_tmpfile: Option<TmpfileGuard>,
    // Where the command actually ran. Needed because the WIP-author diagnostic resolves a
    // repo from its child's cwd, and `ctx` carries no working directory — only the caller
    // has it, from `resolve_work_dir`.
    work_dir: &std::path::Path,
    ctx: &ToolContext,
) -> anyhow::Result<Value> {
    use super::super::command_summary::{
        count_lines, detect_command_type, inline_response_exceeds_limit, strip_ansi_codes,
        summarize_build_output, summarize_generic, summarize_test_output, truncate_lines_and_bytes,
        CommandType, BUFFER_QUERY_INLINE_CAP,
    };

    // Buffer-only queries strip ANSI codes — they are opaque to LLMs and bloat byte counts.
    let raw_stdout = if buffer_only {
        strip_ansi_codes(&raw_stdout)
    } else {
        raw_stdout
    };
    let raw_stderr = if buffer_only {
        strip_ansi_codes(&raw_stderr)
    } else {
        raw_stderr
    };

    // --- Step 6.5: Read tee capture and store as unfiltered_output ref ---
    // Values scrubbed from the capture, added to the response where the ref is attached.
    let mut tee_redacted = 0usize;
    let unfiltered_ref: Option<(
        String,
        Option<crate::tools::output_buffer::Truncation>,
        usize,
    )> = if let Some(ref tmpfile) = unfiltered_tmpfile {
        // `.ok()` here used to discard the error, and that is how a transient read
        // failure became an ABSENT key group rather than a reported one: the caller
        // sees a response with no `unfiltered_output`, indistinguishable from a run
        // that produced no capture at all. Named as the highest-value instrumentation
        // point in `docs/issues/archive/2026-08-26-wine-lane-flakes-under-load-on-three-tests.md`
        // § Resume, after a load-contention flake on the wine lane dropped exactly this
        // key group — the tell being that the surviving keys are set in the same block
        // as the missing ones, so the response could not have come from that block.
        //
        // Behaviour is deliberately unchanged: still `None` on error, no retry, no
        // timeout tuning — that file rules both out as actively harmful on one
        // unreproduced occurrence. The only difference is that the next occurrence
        // leaves a trace instead of a silence.
        let capture = match std::fs::read_to_string(&tmpfile.0) {
            // Bug bc0cb248b224d1dd: this is the UNFILTERED stream, read from a file, so it never
            // passed the decode in `run_command_inner` that scrubs `stdout`. `env | grep PATH` shows
            // one line inline while this buffer holds the whole environment.
            Ok(content) => {
                let scrubbed = redact_credentials(&content);
                tee_redacted = scrubbed.count;
                Some(scrubbed.text.into_owned())
            }
            Err(e) => {
                tracing::warn!(
                    path = %tmpfile.0,
                    error = %e,
                    "tee capture unreadable; unfiltered_output keys will be absent from \
                     this response"
                );
                None
            }
        };
        // tmpfile drops at function exit — TmpfileGuard::drop() removes the file.
        // Skip empty captures: when the terminal filter matched nothing, both
        // raw_stdout and the tee file are empty — surfacing a handle is misleading.
        capture.and_then(|content| {
            if content.is_empty() {
                return None;
            }
            // Counted on the FULL capture, before any inline-storage truncation below —
            // the whole point is telling the caller how much is behind the ref, not how
            // much of it happened to fit inline.
            let line_count = count_lines(&content);
            let (stored, truncation) = if crate::tools::exceeds_inline_limit(&content) {
                let mut byte_budget = crate::tools::MAX_INLINE_TOKENS * 4;
                let capped: String = content
                    .lines()
                    .take_while(|line| {
                        if byte_budget == 0 {
                            return false;
                        }
                        byte_budget = byte_budget.saturating_sub(line.len() + 1);
                        true
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let t = crate::tools::output_buffer::Truncation {
                    kept_lines: count_lines(&capped),
                    total_lines: line_count,
                };
                // The marker travels WITH the data, so every reader trips over it
                // without having to know the field exists: `tail` shows it, `wc -l`
                // counts it, any slice near the end hits it, and a grep that returns
                // nothing at least sits beside something explaining why. The inline
                // summary path has used this device for a while
                // (`--- N lines omitted ---`); it just never reached this buffer.
                //
                // The trailing newline is deliberate. Without it the sentinel's own
                // line has no terminator, `wc -l` lands back on the content count,
                // and a test asserting the two differ would pass for the wrong reason.
                let marked = format!(
                    "{capped}\n{}\n",
                    crate::tools::output_buffer::truncation_marker(t)
                );
                (marked, Some(t))
            } else {
                (content, None)
            };
            let ref_id = ctx.output_buffer.store_truncated(
                original_command.to_string(),
                stored,
                String::new(), // unfiltered capture is stdout-only
                exit_code,
                truncation,
            );
            Some((ref_id, truncation, line_count))
        })
    } else {
        None
    };

    // Computed BEFORE the branch below: one arm moves `raw_stderr` into the response, and
    // the diagnostic has to cover every output shape — buffered summary included.
    let shell_cause = substitution_diagnostic(original_command, &raw_stderr);

    // Same placement, same reason. Also: the buffered-summary arm below hands the caller a
    // SUMMARY instead of the numbers, so on exactly that path they cannot read `filtered out`
    // for themselves even if they think to. The diagnostic has to be computed before the
    // branch to reach it.
    let empty_selection = empty_test_selection_diagnostic(&raw_stdout);

    // Same placement, computed after the total-miss case so it can stay out of that one's
    // way. Unlike the two diagnostics above, this one can spawn a subprocess (`cargo test …
    // -- --list`) — but only on the narrow gate documented on
    // `partial_test_selection_diagnostic`, so an ordinary run pays nothing extra.
    let partial_selection =
        partial_test_selection_diagnostic(original_command, &raw_stdout, work_dir).await;

    // Same placement, same reason. The non-zero-exit gate lives inside the callee, not
    // here, so the one place that decides "is this a red" is the one place documenting
    // what it costs to be wrong about it. A green command spawns nothing at all.
    let wip_authors = super::attribution::wip_author_diagnostic(
        exit_code,
        &format!("{raw_stdout}\n{raw_stderr}"),
        work_dir,
    )
    .await;

    // Computed BEFORE the branch below, and the HOIST is the fix rather than tidying.
    // A `@cmd_*` entry stores both streams, but `grep`'s and `read_file`'s buffer
    // branches each materialize `.stdout` alone — so this is the only surface that can
    // hand a reader the stored stderr. Until 2026-09-14 the lookup sat inside the
    // `needs_summary` arm below, which meant a buffer query received the stored stderr
    // exactly when its own output already exceeded ~10 KB, and never when it returned
    // `0` — the one case where absence and loss are indistinguishable. The gate was
    // anti-correlated with need: the more precise the query, the more certain the loss.
    // BUG docs/issues/archive/2026-09-14-every-reader-of-a-cmd-buffer-takes-stdout-only-so-the-stored-stderr-reaches-nobody.md
    //
    // Deliberately NOT fed into `needs_summary`: that predicate decides whether a new
    // buffer ref is minted, and widening its input would move the buffering threshold
    // for every caller. This changes what a buffer query REPORTS, never what it stores.
    // cap-class: RESULT_CAP run_command.stderr_lines — probed
    const STDERR_BUDGET: usize = 20;
    let buffer_stderr: String = if buffer_only && raw_stderr.is_empty() {
        original_command
            .find("@cmd_")
            .or_else(|| original_command.find("@file_"))
            .and_then(|pos| {
                original_command[pos..]
                    .split_whitespace()
                    .next()
                    .and_then(|tok| ctx.output_buffer.get(tok))
            })
            .map(|e| e.stderr)
            .unwrap_or_default()
    } else {
        raw_stderr.clone()
    };

    // --- Step 6: Decide whether to buffer + summarize ---
    //
    // Decided on the SERIALIZED response, not the raw streams: see `inline_response_exceeds_limit`.
    // For a buffer query the stderr that counts is the bounded stored one, because that is what the
    // response emits.
    let gate_stderr = if buffer_only {
        bound_buffer_stderr(&buffer_stderr, STDERR_BUDGET, original_command).0
    } else {
        raw_stderr.clone()
    };
    let extras = response_extras_len(
        &[
            ("shell_cause", shell_cause.as_deref()),
            ("wip_authors", wip_authors.as_deref()),
            ("empty_test_selection", empty_selection.as_deref()),
            ("partial_test_selection", partial_selection.as_deref()),
        ],
        unfiltered_ref.is_some(),
        buffer_only,
    );
    let summary_needed =
        inline_response_exceeds_limit(exit_code, &raw_stdout, &gate_stderr, extras);
    let mut result = if summary_needed {
        if buffer_only {
            // Buffer-only: return inline, never create a new buffer ref (avoids infinite loop).
            let stderr_budget = STDERR_BUDGET.min(count_lines(&buffer_stderr));
            let stdout_budget = BUFFER_QUERY_INLINE_CAP - stderr_budget;

            let (stderr_out, stderr_shown, stderr_total) =
                bound_buffer_stderr(&buffer_stderr, STDERR_BUDGET, original_command);

            // Byte budget: keep final JSON under TOOL_OUTPUT_BUFFER_THRESHOLD to avoid re-buffering loop.
            const JSON_OVERHEAD: usize = BUFFER_QUERY_JSON_OVERHEAD;
            let stdout_byte_budget = crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD
                .saturating_sub(JSON_OVERHEAD)
                .saturating_sub(crate::util::text::json_escaped_len(&stderr_out));

            let cut = truncate_lines_and_bytes(
                &raw_stdout,
                stdout_budget,
                stdout_byte_budget,
                &wide_line_remedy(original_command),
            );
            let clipped_wide = cut.clipped_wide;
            let (stdout_out, stdout_shown, stdout_total) = (cut.text, cut.shown, cut.total);

            let was_truncated =
                stdout_shown < stdout_total || clipped_wide || stderr_shown < stderr_total;

            let mut result = json!({"exit_code": exit_code});
            if !stdout_out.is_empty() {
                result["stdout"] = json!(stdout_out);
            }
            if !stderr_out.is_empty() {
                result["stderr"] = json!(stderr_out);
            }
            if was_truncated {
                result["truncated"] = json!(true);
                result["stdout_shown"] = json!(stdout_shown);
                result["stdout_total"] = json!(stdout_total);
                if stderr_total > 0 {
                    result["stderr_shown"] = json!(stderr_shown);
                    result["stderr_total"] = json!(stderr_total);
                }
                let stderr_note = if stderr_total > 0 {
                    format!(", stderr {stderr_shown}/{stderr_total}")
                } else {
                    String::new()
                };
                result["hint"] = json!(capped_hint(
                    original_command,
                    stdout_shown,
                    stdout_total,
                    &stderr_note,
                    clipped_wide,
                ));
            }
            // buffer_only => tee injection was skipped (unfiltered_tmpfile is None).
            // This path returns early, so it needs its own attachment — the one at the
            // bottom of the function cannot reach it.
            if let Some(cause) = shell_cause {
                result["shell_cause"] = json!(cause);
            }
            if let Some(who) = wip_authors {
                result["wip_authors"] = json!(who);
            }
            if let Some(empty) = empty_selection {
                result["empty_test_selection"] = json!(empty);
            }
            if let Some(partial) = partial_selection {
                result["partial_test_selection"] = json!(partial);
            }
            return Ok(result);
        }

        let output_id = ctx.output_buffer.store(
            original_command.to_string(),
            raw_stdout.clone(),
            raw_stderr.clone(),
            exit_code,
        );

        let cmd_type = detect_command_type(original_command);
        let cmd_summary = match cmd_type {
            CommandType::Test => summarize_test_output(&raw_stdout, &raw_stderr, exit_code),
            CommandType::Build => summarize_build_output(&raw_stdout, &raw_stderr, exit_code),
            CommandType::Generic => summarize_generic(&raw_stdout, &raw_stderr, exit_code),
        };

        // Rebuild with correct field order so output_id appears before content fields.
        rebuild_buffered_summary(cmd_summary, &output_id)
    } else {
        // Short output — apply byte budget for buffer-only to prevent re-buffering loop.
        if buffer_only
            && raw_stdout.len() + raw_stderr.len()
                > crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD.saturating_sub(300)
        {
            const JSON_OVERHEAD: usize = BUFFER_QUERY_JSON_OVERHEAD;
            // Capped like the summarized path above, then stdout is budgeted against
            // what the stderr ACTUALLY costs. Budgeting against `raw_stderr` here was
            // wrong twice: on a buffer query it is empty, so it under-counted by the
            // whole stored stream, and it was never the text being emitted.
            let (stderr_out, stderr_shown, stderr_total) =
                bound_buffer_stderr(&buffer_stderr, STDERR_BUDGET, original_command);
            let byte_budget = crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD
                .saturating_sub(JSON_OVERHEAD)
                .saturating_sub(crate::util::text::json_escaped_len(&stderr_out));
            let cut = truncate_lines_and_bytes(
                &raw_stdout,
                BUFFER_QUERY_INLINE_CAP,
                byte_budget,
                &wide_line_remedy(original_command),
            );
            let clipped_wide = cut.clipped_wide;
            let (stdout_out, stdout_shown, stdout_total) = (cut.text, cut.shown, cut.total);
            let mut r = json!({"exit_code": exit_code});
            if !stdout_out.is_empty() {
                r["stdout"] = json!(stdout_out);
            }
            if !stderr_out.is_empty() {
                r["stderr"] = json!(stderr_out);
            }
            if stderr_shown < stderr_total {
                r["stderr_shown"] = json!(stderr_shown);
                r["stderr_total"] = json!(stderr_total);
            }
            if stdout_shown < stdout_total || clipped_wide {
                r["truncated"] = json!(true);
                r["hint"] = json!(banded_hint(
                    original_command,
                    stdout_shown,
                    stdout_total,
                    clipped_wide,
                ));
            }
            r
        } else if let Some(c) = (!buffer_only
            && detect_command_type(original_command) == CommandType::Test)
            .then(|| {
                crate::tools::libtest_compact::compact_libtest_output(&raw_stdout, &raw_stderr)
            })
            .flatten()
        {
            // A short libtest run is ~40% cargo progress and empty sibling targets
            // (docs/research/2026-09-24-rtk-evaluation.pdf § 7). Compacted only when that
            // pays; otherwise the raw arm below returns it exactly as before.
            compacted_test_response(
                c,
                original_command,
                &raw_stdout,
                &raw_stderr,
                exit_code,
                ctx,
            )
        } else {
            let mut r = json!({"exit_code": exit_code});
            if !raw_stdout.is_empty() {
                r["stdout"] = json!(raw_stdout);
            }
            if buffer_only {
                // THE REPRODUCED PATH. `grep -c MARKER @cmd_abc` returns two bytes, so
                // `needs_summary` is false and control arrives HERE — which is exactly
                // the query whose `0` a reader cannot tell apart from a stream that was
                // never surfaced. Capped like the path above so a large stored stderr
                // cannot re-trigger buffering on a query whose own output was short.
                let (stderr_out, stderr_shown, stderr_total) =
                    bound_buffer_stderr(&buffer_stderr, STDERR_BUDGET, original_command);
                if !stderr_out.is_empty() {
                    r["stderr"] = json!(stderr_out);
                }
                if stderr_shown < stderr_total {
                    r["stderr_shown"] = json!(stderr_shown);
                    r["stderr_total"] = json!(stderr_total);
                }
            } else if !raw_stderr.is_empty() {
                // Ordinary command: unchanged, uncapped. The cap above is a property of
                // reading someone else's stored stream, not of stderr generally.
                r["stderr"] = json!(raw_stderr);
            }
            r
        }
    };

    // Attach unfiltered_output ref if we captured via tee. A non-empty capture behind an
    // empty filtered `stdout` is exactly the case docs/issues/archive/2026-08-26-unfiltered-output-ref-carries-no-size-signal.md
    // covers: without an explicit `"stdout": ""` and a line count, the response looks
    // identical whether the ref holds 2 lines or 20,000.
    if let Some((ref ref_id, truncation, line_count)) = unfiltered_ref {
        note_in(&mut result, tee_redacted);
        if result.get("stdout").is_none() {
            result["stdout"] = json!("");
        }
        result["unfiltered_output"] = json!(ref_id);
        result["unfiltered_output_lines"] = json!(line_count);
        if let Some(t) = truncation {
            result["unfiltered_truncated"] = json!(true);
            // `unfiltered_output_lines` describes the STREAM; this describes the
            // HANDLE sitting next to it. Naming only the first left a reader to
            // assume one number covered both, which is the misread this bug is.
            result["unfiltered_buffered_lines"] = json!(t.kept_lines);
        }
    }

    if let Some(cause) = shell_cause {
        result["shell_cause"] = json!(cause);
    }
    if let Some(who) = wip_authors {
        result["wip_authors"] = json!(who);
    }
    if let Some(empty) = empty_selection {
        result["empty_test_selection"] = json!(empty);
    }
    if let Some(partial) = partial_selection {
        result["partial_test_selection"] = json!(partial);
    }

    Ok(result)
}

/// Format a compact one-liner summary of a run_command result for `format_compact`.
pub(crate) fn format_run_command(result: &Value) -> String {
    let mut s = if result["output_id"].is_string() {
        let output_id = result["output_id"].as_str().unwrap_or("");
        match result["exit_code"].as_i64() {
            // A backgrounded job that has not exited carries NO `exit_code` — the payload is
            // `output_id` and `hint`, nothing else. Since 2026-09-15 that response is emitted at
            // spawn time rather than after a 5s warm-up, so "running" is true by construction
            // here; the job's OUTCOME arrives separately, in the `jobs` field of any later
            // command naming the handle. This arm used to be
            // `unwrap_or(0)`, which made that absence indistinguishable from a clean exit and
            // rendered `✓ exit 0` for a run that had already failed to compile. Absence is a
            // third state, not a default: say "running" and assert nothing.
            // docs/issues/archive/2026-09-14-a-backgrounded-gate-command-was-summarised-as-exit-0-while-its-buffer-held-the-failure.md
            None => format!("… running  (query {output_id})"),
            Some(exit) => {
                let check = if exit == 0 { "✓" } else { "✗" };
                match result["type"].as_str() {
                    Some("test") => {
                        let passed = result["passed"].as_u64().unwrap_or(0);
                        let failed = result["failed"].as_u64().unwrap_or(0);
                        let ignored = result["ignored"].as_u64().unwrap_or(0);
                        let mut s = format!("{check} exit {exit} · {passed} passed");
                        if failed > 0 {
                            s.push_str(&format!(" · {failed} FAILED"));
                        }
                        if ignored > 0 {
                            s.push_str(&format!(" · {ignored} ignored"));
                        }
                        s.push_str(&format!("  (query {output_id})"));
                        s
                    }
                    Some("build") => {
                        let errors = result["errors"].as_u64().unwrap_or(0);
                        if errors > 0 {
                            format!("{check} exit {exit} · {errors} errors  (query {output_id})")
                        } else {
                            format!("{check} exit {exit}  (query {output_id})")
                        }
                    }
                    _ => format!("{check} exit {exit}  (query {output_id})"),
                }
            }
        }
    } else if result["timed_out"].as_bool().unwrap_or(false) {
        "✗ timed out".to_string()
    } else {
        // NOTE: this `unwrap_or(0)` is the same defaulting the branch above was fixed for, and
        // is deliberately left. An inline result is by construction a COMPLETED one, so there
        // is no caller that reaches here with an absent `exit_code`; adding a branch nothing
        // reaches would be decoration, and untestable decoration at that (CLAUDE.md
        // § Testing Discipline — loudness is a property of a PATH). If an inline shape ever
        // gains a pending state, this is the second site and it needs the same treatment.
        let exit = result["exit_code"].as_i64().unwrap_or(0);
        let stdout_lines = result["stdout"]
            .as_str()
            .map(|s| s.lines().count())
            .unwrap_or(0);
        let check = if exit == 0 { "✓" } else { "✗" };
        format!("{check} exit {exit} · {stdout_lines} lines")
    };

    // Background job state, rendered first among the appended notices because it answers the
    // question the caller actually asked. A `tail @bg_x` reports the READER's exit code; this
    // line is the only place the JOB's outcome appears, and a field this function does not read
    // reaches nobody.
    if let Some(jobs) = result["jobs"].as_array() {
        for job in jobs {
            let handle = job["handle"].as_str().unwrap_or("");
            let state = job["state"].as_str().unwrap_or("");
            s.push_str(&format!("\n⎈ job {handle}: {state}"));
        }
    }

    // Append timeout hint after all branch logic so it covers every output shape.
    if let Some(hint) = result["timeout_hint"].as_str() {
        s.push_str(&format!("\n⚠ timeout: {hint}"));
    }

    // Same placement, same reason: a shell-substitution cause applies to any exit shape,
    // and the compact renderer is what `call_content` shows — a field this function does
    // not read reaches nobody, which is the defect filed as
    // `docs/issues/archive/2026-08-17-allocate-outcome-frontmatter-max-dropped-at-the-mcp-boundary.md`.
    if let Some(cause) = result["shell_cause"].as_str() {
        s.push_str(&format!("\n⚠ cause: {cause}"));
    }

    // Rendered here for the reason stated above: a field this function does not read reaches
    // nobody. This one is worth the line specifically because its JSON sits beside an
    // `exit 0` that the reader has every reason to believe.
    if let Some(empty) = result["empty_test_selection"].as_str() {
        s.push_str(&format!("\n⚠ {empty}"));
    }
    if let Some(partial) = result["partial_test_selection"].as_str() {
        s.push_str(&format!("\n⚠ {partial}"));
    }

    // An EDITED response must say so where the reader looks, and this renderer is what `call_content`
    // shows: a `redacted_credentials` key it does not read would leave a scrubbed output looking like
    // the command's own (bug bc0cb248b224d1dd).
    if let Some(n) = result[crate::util::redact::REDACTED_KEY].as_u64() {
        s.push_str(&format!(
            "\n⚠ {n} credential-shaped value{} redacted from this output",
            if n == 1 { "" } else { "s" }
        ));
    }

    // Last, and unconditional across output shapes for the same reason. This one is
    // about WHO holds the file rather than what went wrong, so it reads after the cause
    // — and it must be rendered here or it reaches nobody, exactly as above. It is
    // already multi-line and self-labelled, so it is appended verbatim rather than
    // prefixed: re-wrapping it would strip the scope footer that keeps its silence from
    // being read as an exoneration.
    if let Some(who) = result["wip_authors"].as_str() {
        s.push('\n');
        s.push_str(who);
    }

    s
}

#[cfg(test)]
mod tests {
    use super::*;

    // A realistic `cargo test --lib -- <filter1> <filter2>` summary: one filter resolved to
    // one passing test, the rest of the workspace's tests were filtered out. Shape lifted from
    // `docs/issues/archive/2026-09-16-a-partial-test-filter-reports-success-over-a-silently-narrowed-selection.md`'s
    // own reproduction.
    const PARTIAL_MATCH_STDOUT: &str = "\nrunning 1 test\n\
         test foo::bar::real_test ... ok\n\n\
         test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 5617 filtered out\n\n";

    async fn fake_listing_with_one_real_name(
        _list_command: String,
        _work_dir: std::path::PathBuf,
    ) -> Option<String> {
        Some("foo::bar::real_test: test\n\n1 tests, 0 benchmarks\n".to_string())
    }

    async fn fake_listing_with_two_real_names(
        _list_command: String,
        _work_dir: std::path::PathBuf,
    ) -> Option<String> {
        Some(
            "foo::bar::real_test: test\nfoo::bar::second_test: test\n\n\
             2 tests, 0 benchmarks\n"
                .to_string(),
        )
    }

    #[tokio::test]
    async fn a_filter_naming_one_real_and_one_bogus_test_names_the_bogus_one() {
        let out = partial_test_selection_diagnostic_with(
            "cargo test --lib -- foo::bar::real_test zzz_no_such_test_anywhere_at_all",
            PARTIAL_MATCH_STDOUT,
            std::path::Path::new("."),
            fake_listing_with_one_real_name,
        )
        .await;

        let msg = out.expect("a name that matched nothing in --list must be named, not silent");
        assert!(
            msg.contains("zzz_no_such_test_anywhere_at_all"),
            "message must name the bogus filter verbatim: {msg}"
        );
        assert!(
            !msg.contains("real_test`"),
            "the filter that DID resolve must not be reported as unmatched: {msg}"
        );
    }

    #[tokio::test]
    async fn a_filter_naming_only_real_tests_stays_silent() {
        let out = partial_test_selection_diagnostic_with(
            "cargo test --lib -- foo::bar::real_test foo::bar::second_test",
            PARTIAL_MATCH_STDOUT,
            std::path::Path::new("."),
            fake_listing_with_two_real_names,
        )
        .await;

        assert!(
            out.is_none(),
            "every named filter resolved in --list, so this must stay silent: {out:?}"
        );
    }

    #[test]
    fn multi_filter_test_command_extracts_filters_and_builds_the_list_form() {
        let (filters, list_command) = multi_filter_test_command(
            "cargo test --lib -- foo::bar::real_test zzz_no_such_test_anywhere_at_all",
        )
        .expect("two free-standing filters after `--` must be recognised");
        assert_eq!(
            filters,
            vec![
                "foo::bar::real_test".to_string(),
                "zzz_no_such_test_anywhere_at_all".to_string()
            ]
        );
        assert_eq!(list_command, "cargo test --lib -- --list");
    }

    #[test]
    fn multi_filter_test_command_declines_a_single_filter() {
        // The ordinary shape — one named test — is `empty_test_selection_diagnostic`'s to
        // cover on a miss; this function must not pay the `--list` cost for it.
        assert!(multi_filter_test_command("cargo test --lib -- foo::bar::real_test").is_none());
    }

    #[test]
    fn multi_filter_test_command_declines_a_non_test_command() {
        assert!(multi_filter_test_command("cargo build --workspace").is_none());
    }

    /// Shell syntax after the filters is not a filter. Measured 2026-09-24 on both shapes: a
    /// piped grep reported `2>&1`, `|` and grep's pattern as six filters that "matched
    /// NOTHING"; a peer's `> log 2>&1; grep …` chain reported seven, including the redirect
    /// target and the whole following command.
    /// docs/issues/archive/2026-09-24-partial-test-selection-reads-shell-pipeline-tokens-as-test-filters.md
    #[test]
    fn multi_filter_test_command_stops_at_shell_syntax() {
        for (cmd, want) in [
            (
                r#"cargo test --lib -- aa bb 2>&1 | grep -E "x|y""#,
                vec!["aa", "bb"],
            ),
            ("cargo test -- aa bb && echo done", vec!["aa", "bb"]),
            ("cargo test -- aa bb > out.log 2>&1", vec!["aa", "bb"]),
        ] {
            let (filters, _) = multi_filter_test_command(cmd)
                .unwrap_or_else(|| panic!("two real filters must still be found: {cmd}"));
            assert_eq!(filters, want, "{cmd}");
        }
        // The peer's exact shape: the only name (`zz`) sits BEFORE `--`, so after the syntax
        // is discarded there are no filters at all — previously seven phantom ones. Load-bearing:
        // `2>&1;` has the `;` GLUED on, which a check for a bare `;` token misses.
        assert!(
            multi_filter_test_command(
                r#"cargo test --lib zz -- --ignored --nocapture > $S/r.log 2>&1; grep -E "REPLAY|test result" $S/r.log"#
            )
            .is_none()
        );
    }
    /// The reserve for everything that is not stdout/stderr text must cover the LONGEST hint and
    /// every key a truncated buffer-query response can carry. 300 B did not, and a clipped
    /// response serialized 43 B past the inline limit and was re-buffered under `@tool_*`.
    #[test]
    fn the_buffer_query_overhead_covers_the_longest_hint_and_every_key() {
        // Worst case on every axis: a 7-digit total, 100 shown, a stderr note, the wide-line
        // sentence, a ranged sed whose first line is 7 digits, and a long handle.
        let query = "sed -n '1000000,1000099p' @cmd_0bf0a111";
        let hint = capped_hint(query, 100, 9_999_999, ", stderr 20/20", true);
        let non_text = json!({
            "exit_code": -1, "stdout": "", "stderr": "", "truncated": true,
            "stdout_shown": 100, "stdout_total": 9_999_999,
            "stderr_shown": 20, "stderr_total": 20, "hint": hint,
        })
        .to_string()
        .len();
        assert!(
            non_text <= BUFFER_QUERY_JSON_OVERHEAD,
            "keys plus hint take {non_text} B but only {BUFFER_QUERY_JSON_OVERHEAD} are reserved"
        );
    }
    #[test]
    fn response_extras_len_counts_diagnostics_tee_keys_and_counters() {
        // A present diagnostic costs `,"key":"` (key + 6) plus its escaped text.
        let two_quotes = "\"\"";
        assert_eq!(
            response_extras_len(&[("shell_cause", Some(two_quotes))], false, false),
            "shell_cause".len() + 6 + 4,
            "quotes escape to two bytes each"
        );
        // An absent one costs nothing.
        assert_eq!(
            response_extras_len(&[("wip_authors", None)], false, false),
            0
        );
        assert_eq!(response_extras_len(&[], true, false), TEE_KEYS_LEN);
        assert_eq!(
            response_extras_len(&[], false, true),
            BUFFER_QUERY_COUNTER_KEYS_LEN
        );
        assert_eq!(
            response_extras_len(&[("a", Some("x"))], true, true),
            "a".len() + 6 + 1 + TEE_KEYS_LEN + BUFFER_QUERY_COUNTER_KEYS_LEN,
            "the three parts add"
        );
    }

    /// A page hint counts the lines of the output it follows. After `sed -n '2,101p'` those start
    /// at 2, so the next page is 102.., not 1.. — which sent a reader back to the top.
    #[test]
    fn a_ranged_sed_query_gets_a_next_page_in_the_buffers_own_numbering() {
        let hint = capped_hint("sed -n '2,101p' @cmd_abc12345", 100, 500, "", false);
        assert!(hint.contains("sed -n '102,201p' @cmd_abc12345"), "{hint}");
        // And a plain read keeps the old numbering, which already was the buffer's.
        let plain = capped_hint("cat @cmd_abc12345", 100, 500, "", false);
        assert!(plain.contains("sed -n '101,200p' @cmd_abc12345"), "{plain}");
        assert!(
            !plain.contains("grep -o"),
            "the wide-line sentence only appears when a line was clipped"
        );
    }

    /// The `--list` command is built from the `--` of the segment that runs `cargo test`, and
    /// keeps anything before it. Load-bearing: the later `git log -- src` has its own `--`,
    /// which the old `rposition` over the whole line would have picked; and `cd sub &&` must
    /// survive, or the listing runs in the wrong directory.
    #[test]
    fn multi_filter_test_command_lists_from_the_cargo_segment() {
        let (filters, list) =
            multi_filter_test_command("cargo test -- aa bb; git log -- src").unwrap();
        assert_eq!(filters, vec!["aa", "bb"]);
        assert_eq!(list, "cargo test -- --list");

        let (_, list) = multi_filter_test_command("cd sub && cargo test -- aa bb | tail").unwrap();
        assert_eq!(list, "cd sub && cargo test -- --list");
    }
}
