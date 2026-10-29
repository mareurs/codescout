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
    compacted_test_value(&c, raw_stdout, raw_stderr, exit_code, &output_id)
}

/// The response [`compacted_test_response`] returns, around a given `output_id`, with nothing
/// stored: pure, so the summary-or-inline gate can MEASURE the compacted response (with a
/// stand-in handle of the real length) before deciding to use it.
fn compacted_test_value(
    c: &crate::tools::libtest_compact::CompactedTest,
    raw_stdout: &str,
    raw_stderr: &str,
    exit_code: i32,
    output_id: &str,
) -> Value {
    let mut summary =
        super::super::command_summary::summarize_test_output(raw_stdout, raw_stderr, exit_code);
    // The compacted text already carries the failure detail and the stderr, so the
    // summarizer's excerpts of both would say it twice.
    if let Some(obj) = summary.as_object_mut() {
        obj.remove("failures");
        obj.remove("stderr");
        // The cut record names fields whose text the SUMMARIZER cut. Both are gone or about to hold
        // compacted text it never cut, so neither name may stay: a stale `stderr` would spare the
        // compacted stderr from the backstop, and the record is bytes the caller never receives.
        let emptied = match obj.get_mut(crate::tools::CUT_FIELDS_KEY) {
            Some(Value::Array(names)) => {
                names.retain(|n| !matches!(n.as_str(), Some("failures" | "stderr")));
                names.is_empty()
            }
            _ => false,
        };
        if emptied {
            obj.remove(crate::tools::CUT_FIELDS_KEY);
        }
    }
    let trailer = c.trailer(output_id);
    summary["stdout"] = json!(if c.stdout.is_empty() {
        trailer
    } else {
        format!("{}\n{trailer}", c.stdout)
    });
    if !c.stderr.is_empty() {
        summary["stderr"] = json!(c.stderr);
    }
    rebuild_buffered_summary(summary, output_id)
}

/// A stand-in for the `@cmd_*` handle a compacted response will carry, of the REAL length (the
/// prefix and eight hex digits), for measuring the response before the buffer entry exists.
const MEASURING_OUTPUT_ID: &str = "@cmd_00000000";

/// Does the compacted response, with everything attached after it, fit the inline limit?
///
/// The gate that chooses between "summarize" and "inline" measures the RAW streams, and a run that
/// compacts to a few hundred bytes can be over the limit raw: 85 empty-target blocks are ~9.6 KB
/// and ~10 KB serialized, where every newline costs two. Asked of the raw run, that gate summarized
/// a response that would have fit compacted with 9 KB to spare, and the compacted stdout, the one
/// thing a reader of a test run wants, never reached them. So the gate asks THIS first.
fn compacted_fits(
    c: &crate::tools::libtest_compact::CompactedTest,
    raw_stdout: &str,
    raw_stderr: &str,
    exit_code: i32,
    late: &serde_json::Map<String, Value>,
) -> bool {
    let mut response =
        compacted_test_value(c, raw_stdout, raw_stderr, exit_code, MEASURING_OUTPUT_ID);
    // A compacted response always carries `stdout`, so the empty-`stdout` a tee capture adds to a
    // response that had none never applies here.
    attach(&mut response, late.clone(), false);
    crate::tools::response_fits(&response)
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
const BUFFER_STDERR_BYTE_BUDGET: usize = 2000;

/// The handle the caller's buffer query names (`@cmd_0bdbc0aa`), read from the command text.
/// `@file_*` handles count too; an `.err` suffix is dropped, so the result is always the bare
/// handle and a caller can append `.err` or not.
fn queried_ref(command: &str) -> Option<&str> {
    let (start, end) = queried_span(command)?;
    Some(&command[start..end])
}

/// Byte span of the bare handle in `command`, `.err` not included. `@bg_*` counts: a background
/// job's log is read through its handle like any buffer, and a hint that cannot name it would say
/// a placeholder instead.
fn queried_span(command: &str) -> Option<(usize, usize)> {
    let start = command
        .find("@cmd_")
        .or_else(|| command.find("@file_"))
        .or_else(|| command.find("@tool_"))
        .or_else(|| command.find("@bg_"))?;
    let rest = &command[start..];
    let end = rest
        .char_indices()
        .skip(1)
        .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '_'))
        .map_or(rest.len(), |(i, _)| i);
    Some((start, start + end))
}

/// The stream the caller's query reads: the handle WITH its `.err` suffix when the query had one.
/// A paging or grep hint must name this, not the bare handle: `sed -n '81,180p' @cmd_X` after a
/// query of `@cmd_X.err` pages STDOUT, which may be one short line, and returns nothing.
fn queried_stream(command: &str) -> Option<&str> {
    let (start, end) = queried_span(command)?;
    let end = if command[end..].starts_with(".err") {
        end + ".err".len()
    } else {
        end
    };
    Some(&command[start..end])
}

/// The stderr a buffer query carries: at most `max_lines` lines AND at most
/// `BUFFER_STDERR_BYTE_BUDGET` JSON-ESCAPED bytes, marker included, the latter by eliding the
/// MIDDLE. Returns `(text, lines_shown, lines_total)` like `truncate_lines`; `lines_total` counts
/// the stream, so a wide single line reads `1/1` and the marker, which travels with the data, is
/// what says it was cut. A cut by the LINE cap alone carries one too, after the last line kept,
/// holding the same remedy; a stream that fits whole carries none.
///
/// Escaped, not raw: a control character is 1 byte here and 6 in the response, so a raw cut of
/// 2,000 B of `\x01` serialized to ~12 KB and the response was re-buffered under `@tool_*`.
///
/// `own` says where the text came from, which decides what the marker may promise. The entry's
/// STORED stderr is behind `<handle>.err`; the query's OWN stderr (an `awk`'s, a pipeline's) is
/// stored nowhere, so naming `.err` for it would send the reader to a stream that does not hold it.
fn bound_buffer_stderr(
    stderr: &str,
    max_lines: usize,
    query: &str,
    own: bool,
) -> (String, usize, usize) {
    let (by_lines, shown, total) = crate::tools::command_summary::truncate_lines(stderr, max_lines);
    let remedy = if own && can_wrap_query(query) {
        // WRAPPED, not appended: a redirection applies to the LAST command of a pipeline only, so
        // `a | b >&2 | c 2>&1 >/dev/null | cut` read `c`'s stderr and left `b`'s on the terminal. A
        // subshell gives every stage the same stderr. Use `( q; )`, not `{ q; }`: when `q` is one
        // redirected subshell, dash puts the group's redirects on that subshell node in place of
        // its own, so `{ (cmd) >&2; } 2>&1 >/dev/null` loses the `>&2`. `( q; )` adds a new node.
        format!(
            "this is the query's own stderr and is stored nowhere: rerun the whole query as \
             `( QUERY; ) {OWN_STDERR_STAGE}` to read a window of it"
        )
    } else if own {
        // A list hands the stage the LAST command's stderr only, and a shape the splitter does not
        // model (`$(…)`, a comment, a heredoc) may swallow it or run it as a second line.
        "this is the query's own stderr and is stored nowhere: narrow the query so it prints \
         less of it"
            .to_string()
    } else {
        match queried_ref(query) {
            Some(handle) => format!("all of it: {handle}.err"),
            None => "all of it: the stored buffer's `.err` handle".to_string(),
        }
    };
    let bounded = crate::util::text::elide_middle_escaped(
        &by_lines,
        stderr.len(),
        BUFFER_STDERR_BYTE_BUDGET,
        "stderr",
        &remedy,
    );
    // Cut by the byte budget: the marker in the middle holds the remedy, and its `of N bytes` counts
    // the whole stream, so it covers a line cut as well. Cut by nothing: silent, as it always was.
    if shown == total || bounded != by_lines {
        return (bounded, shown, total);
    }
    // Cut by the LINE cap alone. The twenty lines fit the byte budget, so the cut above never ran,
    // and the counters (`stderr_shown`, `stderr_total`) were all the caller had: for the query's
    // own stderr, which is stored nowhere, no route to line 21 existed in the response. Measured
    // 2026-10-09: `cat @A >&2` over 3,000 short lines answered 20 lines and no marker. The same
    // remedy follows the lines, in the same `\n--- stderr: .. ---` frame, inside the budget: the
    // text the marker leaves room for is cut by bytes if it must be, and that cut's marker then
    // stands in for this one.
    let marker = format!("\n--- stderr: {shown} of {total} lines shown; {remedy} ---");
    let room =
        BUFFER_STDERR_BYTE_BUDGET.saturating_sub(crate::util::text::json_escaped_len(&marker));
    let body =
        crate::util::text::elide_middle_escaped(&by_lines, stderr.len(), room, "stderr", &remedy);
    if body == by_lines {
        (format!("{body}{marker}"), shown, total)
    } else {
        (body, shown, total)
    }
}

/// The line a PLAIN SLICE of the queried stream starts at, in that stream's own numbering: `1` for
/// `cat <ref>`, `A` for `sed -n 'A,Bp' <ref>` (quotes single, double or none; the ref may carry
/// `.err`). `None` for every other command.
///
/// Only a plain slice prints a contiguous run of the queried stream, so only for one can a page
/// route be written as `sed -n '..' <ref>`. Anchored to the WHOLE command: matched anywhere,
/// `grep x @cmd_X | sed -n '5,10p'` read as a slice starting at line 5 of `@cmd_X`, and a
/// `grep -n "" @cmd_X` cut at line 100 was sent to `sed -n '101,200p' @cmd_X`, which prints
/// source lines without the `N:` prefix (and, for a filtering grep, lines that never matched).
///
/// Anything after the ref, a redirection included, makes the command a non-slice:
/// `sed -n '2,3000p' @cmd_X 2>&1` gets the appended-pager route, which works. Only that hint's
/// prose ("not a slice") is inexact there, and that is accepted rather than modelled.
fn plain_slice_first_line(command: &str) -> Option<usize> {
    plain_slice_window(command).map(|(first, _)| first)
}

/// The window a PLAIN SLICE prints, in the queried stream's own numbering: `(1, None)` for
/// `cat <ref>`, `(A, Some(B))` for `sed -n 'A,Bp' <ref>`. `None` for every other command, by
/// [`plain_slice_first_line`]'s rule (the whole command, nothing after the ref).
fn plain_slice_window(command: &str) -> Option<(usize, Option<usize>)> {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let caps = RE
        .get_or_init(|| {
            regex::Regex::new(
                r#"^\s*(?:cat|sed\s+-n\s+(?:'(\d+),(\d+)p'|"(\d+),(\d+)p"|(\d+),(\d+)p))\s+@(?:cmd|file|tool|bg)_[A-Za-z0-9_]+(?:\.err)?\s*$"#,
            )
            .expect("static pattern")
        })
        .captures(command)?;
    match [(1, 2), (3, 4), (5, 6)]
        .into_iter()
        .find_map(|(a, b)| Some((caps.get(a)?, caps.get(b)?)))
    {
        Some((a, b)) => Some((
            a.as_str().parse::<usize>().ok()?.max(1),
            Some(b.as_str().parse::<usize>().ok()?),
        )),
        None => Some((1, None)),
    }
}

/// Whether `stage` appended to `query` reads all of the query's output AND would be run: the query
/// is one pipeline, and the IL3 gate does not refuse the joined command. Asked of the gate itself
/// (`pipe_refusal`, foreground), so a route a hint offers is one `run_command` accepts; a list of
/// "safe" producers kept here would drift from it. A pager (`| sed -n`) and `| cut` are terminal
/// filters, so the gate refuses the joined command only where the capture cannot be promised and an
/// unbounded producer is piped into a trimmer: a buffer ref in the text, a `|&` first pipe, a first
/// pipe nested in a group or substitution, a comment, a heredoc, `$(` or a backtick after it, or a
/// splice the shell does not parse (the full list is `pipe_refusal`'s).
fn can_append_stage(query: &str, stage: &str) -> bool {
    crate::util::path_security::is_one_pipeline(query)
        && super::pipe_refusal(&format!("{query} {stage}"), false).is_none()
}

/// Whether `query` can be wrapped in a subshell, `( query; ) 2>&1 >/dev/null | cut -c1-4000`, to
/// read ALL of its stderr. [`can_append_stage`] asks the same of a stage that reads stdout, where
/// "last command" is right: a pipeline's stdout IS its last command's. A pipeline's stderr is every
/// stage's, and only a group can redirect them together. The wrapping needs no model of how many
/// stages there are; it needs the query to be one pipeline, so that the `; )` closes it and nothing
/// else.
///
/// No IL-3 question is asked, and that is on purpose: the query already passed the gate when it
/// ran, and the wrap adds only `) 2>&1 >/dev/null | cut -c1-4000`, whose last stage `cut` is a field
/// selector the gate never counts as a trimmer. (The group also hides an unbounded producer from
/// the gate's segment scan, so a verdict on the wrapped text would say nothing the query's own run
/// had not already said.) If the stage ever becomes a trimmer, this is the place that needs the
/// question back.
fn can_wrap_query(query: &str) -> bool {
    crate::util::path_security::is_one_pipeline(query)
}

/// The stage a non-slice query appends to read a window of a wide line.
const WIDE_LINE_STAGE: &str = "| cut -c1-4000";

/// The stage that follows the query's subshell to read a window of its own stderr, which no
/// buffer holds: see [`can_wrap_query`].
const OWN_STDERR_STAGE: &str = "2>&1 >/dev/null | cut -c1-4000";

/// How to read a window of a line wider than the response budget. These routes work on a wide
/// line: `sed -n` and a bare `grep` return the whole line or nothing, `jq` needs JSON.
///
/// The routes name the queried ref only when the output starts at line 1 of it (`cat <ref>`,
/// `sed -n '1,Bp' <ref>`): then `cut -c1-4000 <ref>` reaches the clipped line. For any other
/// command the clipped line is the COMMAND's, not the ref's. A filtering `grep` may print source
/// line 300 first, and `cut` on the ref stops at line 100. A transforming `jq` prints a line the
/// ref does not hold at all. So the cut is appended to the query instead, or, when it cannot be,
/// the caller is told to narrow the query.
fn wide_line_route(command: &str) -> String {
    match (plain_slice_first_line(command), queried_stream(command)) {
        (Some(1), Some(r)) => format!(
            "read a window of it with `grep -o 'TEXT.\\{{0,200\\}}' {r}` or `cut -c1-4000 {r}`"
        ),
        _ if can_append_stage(command, WIDE_LINE_STAGE) => {
            format!("read a window of it by appending `{WIDE_LINE_STAGE}` to this query")
        }
        _ => "it is stored nowhere, so narrow the query to print less of it".to_string(),
    }
}

/// The marker a clipped wide line carries: see [`wide_line_route`].
fn wide_line_remedy(command: &str) -> String {
    format!(
        "this line is wider than the response budget; {}",
        wide_line_route(command)
    )
}

/// `(A, B)` when `query` ends in the pager a hint advises, `<base> | sed -n 'A,Bp'`, and `<base>`
/// can take that stage (see [`can_append_stage`]). Its output is then lines `A..` of the BASE's
/// output, so the next page replaces the last stage instead of stacking a second `sed` on it, and
/// is numbered from what this page actually showed: a page cut by bytes at 48 of 100 lines goes
/// on at line `A + 48`, not at `A + 100`.
///
/// Any trailing ranged `sed` is read this way, a caller's own `| sed -n '1,500p'` included: its
/// next pages run past line 500 of the base. The hint says "of the query before its last stage",
/// so that is what it promises. Telling a caller's window from a hint's would need state no query
/// carries, and capping at `B` stops a hint's own 100-line window cut by bytes short of the end.
fn trailing_pager(query: &str) -> Option<(usize, usize)> {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let caps = RE
        .get_or_init(|| {
            regex::Regex::new(
                r#"(?s)^(.*\S)\s*\|\s*sed\s+-n\s+(?:'(\d+),(\d+)p'|"(\d+),(\d+)p"|(\d+),(\d+)p)\s*$"#,
            )
            .expect("static pattern")
        })
        .captures(query)?;
    let base = caps.get(1)?.as_str();
    let (a, b) = [(2, 3), (4, 5), (6, 7)]
        .into_iter()
        .find_map(|(x, y)| Some((caps.get(x)?, caps.get(y)?)))?;
    let (a, b) = (
        a.as_str().parse::<usize>().ok()?,
        b.as_str().parse::<usize>().ok()?,
    );
    (a >= 1 && b >= a && can_append_stage(base, &format!("| sed -n '{a},{b}p'"))).then_some((a, b))
}

/// The hint on a buffer query that was NOT cut, when there may still be more: the query ends in
/// a hint's own pager (a 100-line window), and the page filled it. Without this, a page of short
/// lines that fits whole carries no hint, and a reader following hints alone stops at line 200.
fn full_page_hint(query: &str, lines: usize) -> Option<String> {
    use crate::tools::command_summary::BUFFER_QUERY_INLINE_CAP;
    let (a, b) = trailing_pager(query)?;
    (b - a + 1 == BUFFER_QUERY_INLINE_CAP && lines == BUFFER_QUERY_INLINE_CAP).then(|| {
        format!(
            "This page is lines {a} to {b} of the query before its last stage, which may print \
             more. Next page: replace that last stage with `sed -n '{},{}p'`.",
            b + 1,
            b + BUFFER_QUERY_INLINE_CAP
        )
    })
}

/// The hint on a PLAIN SLICE that was NOT cut, when the stream it reads has more: a window of
/// exactly one page (`sed -n 'A,Bp' <ref>` with `B - A + 1` equal to [`BUFFER_QUERY_INLINE_CAP`],
/// the window every hint names) that filled it and ends before the last line of the stream.
/// Without this the page the hint itself advised fits whole as 100 short lines, carries no hint,
/// and a reader following hints alone stops at line 200 of a longer buffer.
///
/// The same rule [`full_page_hint`] applies to a trailing pager, with one more fact: a plain slice
/// reads a stream the buffer holds, so its length is known (`stream_lines`, asked of the queried
/// stream only once the window is a full page) and the last window names no further one. A
/// trailing pager reads the query's own output, whose length is stored nowhere, and has to say
/// "may print more".
///
/// Only a window of the hint's own page width counts. A caller's own `sed -n '1,100p'` has that
/// width too and gets the hint; a caller's `sed -n '101,150p'` is a window of their choosing, and
/// answering it with a next page would be advice nobody asked for.
fn next_window_hint(
    query: &str,
    lines: usize,
    stream_lines: impl FnOnce(&str) -> Option<usize>,
) -> Option<String> {
    use crate::tools::command_summary::BUFFER_QUERY_INLINE_CAP;
    let (a, Some(b)) = plain_slice_window(query)? else {
        return None;
    };
    let stream = queried_stream(query)?;
    if b < a || b - a + 1 != BUFFER_QUERY_INLINE_CAP || lines != BUFFER_QUERY_INLINE_CAP {
        return None;
    }
    let total = stream_lines(stream)?;
    (b < total).then(|| {
        format!(
            "This page is lines {a} to {b} of {stream}, which has {total} lines. \
             Next page: sed -n '{},{}p' {stream}. \
             Or grep 'keyword' {stream} for targeted search.",
            b + 1,
            b + BUFFER_QUERY_INLINE_CAP
        )
    })
}

/// The number of lines `sed` and `cat` see in the stream a buffer query names (`stream`: the
/// handle, with its `.err` suffix when the query had one), counted on the text `run_command` hands
/// the shell. That is the stored stream, except a `@tool_*` ref, which the shell reads
/// pretty-printed when it holds JSON (`OutputBuffer::resolve_refs`): counted as stored it is one
/// line, and counted as `read_file` numbers it (`\n` expanded) it has lines the shell never sees.
/// A `@bg_*` log is the file on disk, read now. `None` when the handle no longer resolves.
///
/// COST, MEASURED 2026-10-09 (release build, a 54.9 MB stream of 7,000,000 short lines, 20 pages
/// of exactly 100 lines each through `RunCommand::call_content`, against 20 pages of 99 lines,
/// which do not ask): the count added 91 ms per page on a `@cmd_*` buffer (284.8 -> 376.1 ms
/// median) and 136 ms on a `@bg_*` log (234.8 -> 371.2 ms). Pieces timed alone: `str::lines().count()`
/// 86 ms, the clone `OutputBuffer::get` makes 7 ms, `fs::read` 15 ms, the lossy UTF-8 copy 35 ms.
/// So the cost was the counting, then the copies. After: a byte count ([`count_newline_lines`]) and
/// a log read in chunks and never held whole, measured the same way, add 29 ms per page on a
/// `@cmd_*` buffer (278.2 -> 306.8 ms median) and 23 ms on a `@bg_*` log (236.2 -> 259.2 ms). The
/// clone of a stored entry stays (7 ms; a pool accessor was not worth a change to its file).
/// Not measured: a `@tool_*` ref, whose JSON is parsed and pretty-printed to count.
fn stream_line_count(ctx: &ToolContext, stream: &str) -> Option<usize> {
    let base = stream.strip_suffix(".err").unwrap_or(stream);
    if base.starts_with("@bg_") {
        let log = ctx.output_buffer.get_background_log(base)?;
        return count_file_lines(&log, FILE_COUNT_CHUNK);
    }
    let entry = ctx.output_buffer.get(base)?;
    if stream.ends_with(".err") {
        Some(count_newline_lines(entry.stderr.as_bytes()))
    } else if base.starts_with("@tool_") {
        let pretty = serde_json::from_str::<Value>(&entry.stdout)
            .ok()
            .and_then(|v| serde_json::to_string_pretty(&v).ok());
        Some(count_newline_lines(
            pretty.as_deref().unwrap_or(&entry.stdout).as_bytes(),
        ))
    } else {
        Some(count_newline_lines(entry.stdout.as_bytes()))
    }
}

/// The bytes `count_file_lines` reads at a time.
const FILE_COUNT_CHUNK: usize = 1 << 20;

/// The lines `sed -n '$='` counts in `bytes`: each `\n`, plus one for a last line that has none.
/// Equal to `count_lines` (`str::lines().count()`) on any UTF-8 text, and on the lossy decode of
/// any bytes: a `\n` is one ASCII byte that no multi-byte sequence holds, and the lossy decode
/// keeps it. Over bytes because the count of a 55 MB stream is the hot part of a 100-line page
/// (see [`stream_line_count`]).
fn count_newline_lines(bytes: &[u8]) -> usize {
    let newlines = bytes.iter().filter(|&&b| b == b'\n').count();
    newlines + usize::from(bytes.last().is_some_and(|&b| b != b'\n'))
}

/// [`count_newline_lines`] of the file at `path`, read `chunk` bytes at a time so a large log is
/// never held whole. `None` when it cannot be read.
fn count_file_lines(path: &std::path::Path, chunk: usize) -> Option<usize> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let mut buf = vec![0u8; chunk.max(1)];
    let (mut newlines, mut last) = (0usize, None);
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        newlines += buf[..n].iter().filter(|&&b| b == b'\n').count();
        last = Some(buf[n - 1]);
    }
    Some(newlines + usize::from(last.is_some_and(|b| b != b'\n')))
}

/// The hint a buffer query that was NOT cut carries when its window filled a page and there may be
/// more: a trailing pager's ([`full_page_hint`]) or a plain slice's ([`next_window_hint`]). The two
/// shapes cannot both match one command.
fn full_window_hint(ctx: &ToolContext, query: &str, lines: usize) -> Option<String> {
    full_page_hint(query, lines)
        .or_else(|| next_window_hint(query, lines, |stream| stream_line_count(ctx, stream)))
}

/// The hint on a capped buffer query: the next page and, when the first line was clipped, the
/// route for a wide line.
///
/// Two populations. A PLAIN SLICE (`cat <ref>`, `sed -n 'A,Bp' <ref>`) prints a run of the queried
/// stream, so its next page is `sed -n '..' <ref>` in that stream's own numbering. Any other
/// command printed its OWN lines, which no buffer holds: the next page is the same query with a
/// pager appended, or, when it already ends in one ([`trailing_pager`]), with that pager replaced.
/// Every number comes from what this page SHOWED, so following the hints alone reaches every line
/// once, however the pages are cut. The query itself is never echoed: it can be arbitrarily wide,
/// and this hint is measured before the stdout it sits beside is cut.
///
/// Every ref kind takes this rule, `@tool_*` included: its own arm offered
/// `read_file(.., json_path="$.<field>")` to every query, slice or not, with a placeholder in it.
fn capped_hint(
    query: &str,
    shown: usize,
    total: usize,
    stderr_note: &str,
    clipped_wide: bool,
    by_bytes: bool,
) -> String {
    use crate::tools::command_summary::BUFFER_QUERY_INLINE_CAP;
    let stream = queried_stream(query);
    // The reason is the TRUE one: a cut at line 24 of 60 by the byte budget is not "capped at 100
    // lines", and a reader told so goes looking for a line cap that was never reached.
    let why = if by_bytes {
        "Output cut to fit the response budget".to_string()
    } else {
        format!("Output capped at {BUFFER_QUERY_INLINE_CAP} lines")
    };
    let mut hint = match (plain_slice_first_line(query), stream) {
        (Some(first), Some(r)) => {
            let next_start = first + shown;
            let next_end = first - 1 + shown + BUFFER_QUERY_INLINE_CAP;
            format!(
                "{why} \
                 (stdout {shown}/{total}{stderr_note}). \
                 Next page: sed -n '{next_start},{next_end}p' {r}. \
                 Or grep 'keyword' {r} for targeted search."
            )
        }
        _ => {
            let mut hint = format!("{why} ({shown}/{total} lines{stderr_note}).");
            match trailing_pager(query) {
                Some((a, _)) if shown < total => {
                    let from = a + shown;
                    hint.push_str(&format!(
                        " These are lines {a} to {} of the query before its last stage, and the \
                         rest is stored nowhere. Next page: replace that last stage with \
                         `sed -n '{from},{}p'`.",
                        from - 1,
                        from + BUFFER_QUERY_INLINE_CAP - 1
                    ));
                }
                Some(_) => {
                    if let Some(full) = full_page_hint(query, total) {
                        hint.push(' ');
                        hint.push_str(&full);
                    }
                }
                None if shown < total => {
                    hint.push_str(&match stream {
                        Some(r) => format!(
                            " This output is not a slice of {r}, and the rest of it is stored \
                             nowhere."
                        ),
                        None => " The rest of this output is stored nowhere.".to_string(),
                    });
                    let pager = format!(
                        "| sed -n '{},{}p'",
                        shown + 1,
                        shown + BUFFER_QUERY_INLINE_CAP
                    );
                    if can_append_stage(query, &pager) {
                        hint.push_str(&format!(
                            " Next page: append `{pager}` to this query. Each page names the next."
                        ));
                    } else {
                        hint.push_str(
                            " To read it, narrow the query so it prints fewer lines, for example \
                             with a tighter pattern.",
                        );
                    }
                }
                None => {}
            }
            if let Some(r) = stream {
                hint.push_str(&format!(" Or grep 'keyword' {r} for targeted search."));
            }
            hint
        }
    };
    if clipped_wide {
        hint.push_str(&format!(
            " The first line is wider than the response budget and is shown clipped: {}.",
            wide_line_route(query)
        ));
    }
    hint
}

/// The hint on a truncated buffer query. Every ref kind takes [`capped_hint`]'s two-population
/// rule. A `@tool_*` ref had its own arm here until 2026-10-08, which told every query on it,
/// `grep -n '"i"' @tool_X` included, to `read_file("@tool_X", json_path="$.<field>")` or
/// `start_line=N, end_line=M`: routes on the SOURCE, not the query's output, with placeholders.
/// `sed -n` on a `@tool_*` handle reads the same pretty-printed lines `run_command` showed;
/// `read_file`'s `start_line` does not always (it numbers the text with escaped newlines expanded).
fn truncation_hint(
    query: &str,
    shown: usize,
    total: usize,
    stderr_note: &str,
    clipped_wide: bool,
    by_bytes: bool,
) -> String {
    capped_hint(query, shown, total, stderr_note, clipped_wide, by_bytes)
}

/// Byte ceiling on the `wip_authors` diagnostic a red run carries.
///
/// The diagnostic lists every uncommitted file the failure names, with who wrote it, so its size is
/// files x peers and nothing else bounds it. Measured 2026-10-05 against `scripts/attribute-red.py`
/// on a probe repo: a red naming 5 dirty files made a 1,466 B answer and one naming 150 made
/// 35,976 B (the test fixture's longer paths make its 150-file answer 36,277 B). At that size the
/// answer
/// alone pushed the response over the inline limit, and the only thing standing between the caller
/// and a content-free `@tool_*` envelope was a backstop in `call_content` whose marker says the
/// tool's own buffer holds the cut text. For this field that is false: the diagnostic is computed
/// from the streams and stored nowhere.
///
/// Cut here, at the source, with a remedy that is true, and by eliding the MIDDLE: the head is the
/// first files named and the tail is the scope footer that keeps the diagnostic's silence from
/// being read as an exoneration, which a head-only cut would drop. 3,000 B holds about 40 files of
/// the shape measured, ahead of a footer of about 350 B.
// cap-class: RESULT_CAP run_command.wip_authors_bytes — probed
pub(super) const WIP_AUTHORS_BYTE_BUDGET: usize = 3000;

/// `wip_authors`, cut to [`WIP_AUTHORS_BYTE_BUDGET`] behind a marker whose remedy is true.
/// `git status --short` lists every uncommitted file in any repository, which is what a reader
/// needs when the list was cut.
pub(super) fn bound_wip_authors(text: String) -> String {
    // ESCAPED bytes, the unit the response is measured in: a raw cut let a diagnostic dense in
    // control characters (a path or a porcelain line carrying one) serialize to several times its
    // budget. The marker's total stays the diagnostic's own length.
    crate::util::text::elide_middle_escaped(
        &text,
        text.len(),
        WIP_AUTHORS_BYTE_BUDGET,
        "wip_authors",
        "every uncommitted file: `git status --short`",
    )
}

/// Keys the CALLER of `handle_successful_output_with` knows and the response must also carry.
#[derive(Default)]
pub(crate) struct LateKeys {
    /// Credentials redacted from the command's own streams: the `redacted_credentials` note.
    pub redacted: usize,
    /// Why a wanted tee capture was not made: `unfiltered_output_skipped`.
    pub tee_skipped: Option<String>,
    /// The keys `run_command` puts on every response shape (`super::envelope_keys`):
    /// `buffer_truncated`, `jobs`, `timeout_hint`.
    pub envelope: serde_json::Map<String, Value>,
    /// Prepended to `stdout` where the response has one (`CallNotes::stdout_prefix`). Priced by
    /// the gate in the escaped bytes of the stdout it joins.
    pub stdout_prefix: String,
    /// The producer's exit status the spliced wrapper recorded ([`read_producer_status`]): the
    /// `producer_exit_code` key. `None` when none was recorded: no capture, a producer that is not a
    /// plain command, a status file that could not be made or read.
    pub producer_exit_code: Option<i32>,
    /// The signal that ended the command ([`terminating_signal`]): the `terminated_by_signal` key.
    /// A signal death has no exit code, so `exit_code` reads `-1` and says nothing else. `None`
    /// for every normal exit, so the key's presence is never a sign of an ordinary failure.
    pub terminated_by_signal: Option<i32>,
}

/// The signal that ended a process, or `None` when it exited on its own (any code) or the platform
/// has no signals. `ExitStatus::code` is `None` exactly when this is `Some`.
pub(crate) fn terminating_signal(status: &std::process::ExitStatus) -> Option<i32> {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        status.signal()
    }
    #[cfg(not(unix))]
    {
        let _ = status;
        None
    }
}

/// What `"stdout":""` adds to a response that had no `stdout` key. The tee block inserts it when a
/// capture sits behind an empty filtered stdout.
const EMPTY_STDOUT_KEY: &str = r#","stdout":"""#;

/// Every key attached to the response AFTER the streams, as one map: the four diagnostics, the tee
/// keys, the redaction note, the tee-skipped note and the envelope keys (`LateKeys::envelope`).
/// Built once, MEASURED by the summary-or-inline gate and then APPLIED by `attach`, so the gate
/// cannot judge keys the response does not carry or miss keys it does.
fn attachments(
    diagnostics: &[(&str, &Option<String>)],
    unfiltered: &Option<(
        String,
        Option<crate::tools::output_buffer::Truncation>,
        usize,
        Vec<String>,
    )>,
    redacted: usize,
    tee_skipped: &Option<String>,
    envelope: &serde_json::Map<String, Value>,
) -> serde_json::Map<String, Value> {
    let mut keys = serde_json::Map::new();
    if let Some((ref_id, truncation, line_count, pool_cut)) = unfiltered {
        keys.insert("unfiltered_output".into(), json!(ref_id));
        keys.insert("unfiltered_output_lines".into(), json!(line_count));
        if let Some(t) = truncation {
            keys.insert("unfiltered_truncated".into(), json!(true));
            // `unfiltered_output_lines` counts the lines of the capture READ, before the store cut;
            // this key describes the HANDLE sitting next to it. Naming only the first left a
            // reader to assume one number covered both, which is the misread this key exists to
            // prevent. The count is the whole stream only when the stream fit the capture file. The
            // file is bounded a little over `CAPTURE_STORE_LIMIT_BYTES` for EVERY filter (the
            // splice's `ulimit -f`; the read is bounded too), and an early-exit filter (`| head -N`)
            // also stops the producer after the drain reads that much. Either way the count is then
            // a LOWER BOUND of the stream, and this `unfiltered_truncated` is what marks it.
            keys.insert("unfiltered_buffered_lines".into(), json!(t.kept_lines));
        }
        if !pool_cut.is_empty() {
            keys.insert("output_truncated".into(), json!(pool_cut));
        }
    }
    for (key, value) in diagnostics {
        if let Some(text) = value {
            keys.insert((*key).into(), json!(text));
        }
    }
    let mut note = json!({});
    note_in(&mut note, redacted);
    if let Value::Object(note) = note {
        keys.extend(note);
    }
    if let Some(why) = tee_skipped {
        keys.insert("unfiltered_output_skipped".into(), json!(why));
    }
    keys.extend(envelope.clone());
    keys
}

/// Apply [`attachments`] to a response. A tee capture behind a response with no `stdout` gets an
/// explicit `"stdout": ""`, so the response is not read as "nothing happened".
fn attach(result: &mut Value, keys: serde_json::Map<String, Value>, tee_present: bool) {
    let Some(obj) = result.as_object_mut() else {
        return;
    };
    if tee_present && !obj.contains_key("stdout") {
        obj.insert("stdout".into(), json!(""));
    }
    obj.extend(keys);
}

/// The serialized bytes `keys` add to a response that already has other keys: each is
/// `,"name":value`, so the object's own length minus its braces plus one comma. `0` for none.
/// MEASURED from the built keys, never a reserve: reserves for these were 160 B for the tee keys
/// (they take 66 B, or 126 B with the two truncation keys) and 48 B for the counters (they take
/// 36 B), so each reserve wrongly summarized output that fit.
fn extras_len(keys: &serde_json::Map<String, Value>, empty_stdout_key: bool) -> usize {
    let keys_len = if keys.is_empty() {
        0
    } else {
        Value::Object(keys.clone()).to_string().len() - 1
    };
    keys_len
        + if empty_stdout_key {
            EMPTY_STDOUT_KEY.len()
        } else {
            0
        }
}

/// Escaped bytes of `stdout` text that fit beside `skeleton`, a copy of the response WITHOUT its
/// `stdout` key: the inline limit minus the skeleton's compact length minus what the key itself
/// costs. A response built to this lands ON the limit, which is what the limit is for.
fn inline_stdout_room(skeleton: &Value) -> usize {
    crate::tools::response_room(skeleton).saturating_sub(EMPTY_STDOUT_KEY.len())
}

/// The bytes of an unfiltered capture the `@cmd_*` handle behind `unfiltered_output` keeps. A
/// capture over it is cut at the first line boundary at or past the limit (the line that crosses it
/// is kept whole, so the handle can hold a little more than this) and marked; one of at most this
/// many bytes is kept whole.
///
/// It was `MAX_INLINE_TOKENS * 4` (10 KB), the size of an INLINE response, though the handle is
/// not inline: a 100,000-line stream was cut to ~2,200 lines behind a ref whose point is to be
/// re-read. The pool's own per-entry cap (`OutputBuffer::entry_cap`, half the pool) still applies
/// on top of it. The capture FILE is bounded too, a little over this (`splice_tee`'s
/// `ulimit -f`), and so is the read of it ([`CAPTURE_READ_LIMIT_BYTES`]).
// cap-class: RESULT_CAP run_command.capture_store_bytes — probed
pub(super) const CAPTURE_STORE_LIMIT_BYTES: usize = 4 * 1024 * 1024;

/// The most bytes of the capture FILE [`read_capture`] reads into server memory. The file is
/// already bounded on disk (`splice_tee` runs `tee` under `ulimit -f`, a little over
/// [`CAPTURE_STORE_LIMIT_BYTES`]); this is the bound for a shell where that `ulimit` does nothing
/// (Git Bash may lack it), so a `git log --all -p` (268 MB measured) can never be read whole.
/// Above the store limit by construction, so a capture cut here is always cut again by
/// [`store_capture`] and flagged `unfiltered_truncated`; its line count is then a lower bound.
// cap-class: RESULT_CAP run_command.capture_read_bytes — probed
pub(super) const CAPTURE_READ_LIMIT_BYTES: usize = CAPTURE_STORE_LIMIT_BYTES + 1024 * 1024;

const _: () = assert!(CAPTURE_READ_LIMIT_BYTES > CAPTURE_STORE_LIMIT_BYTES);

/// Read the tee capture behind `tmpfile`, scrubbed of credentials (see [`CaptureRead`]).
///
/// At most [`CAPTURE_READ_LIMIT_BYTES`] are read (`Read::take`), never the whole file whatever its
/// size: `read_to_string` read a 268 MB `git log --all -p` capture into server memory. The bytes
/// are decoded LOSSILY: `read_to_string` failed on one byte that is not UTF-8 (a latin-1 file under
/// `rg`), and the whole handle vanished with no word; now such bytes become U+FFFD and the handle
/// survives. A read cut at the bound can also split a character, which decodes the same way.
///
/// `None` on a read error, which is logged. `.ok()` here used to discard the error, and that is
/// how a transient read failure became an ABSENT key group rather than a reported one: the caller
/// sees a response with no `unfiltered_output`, indistinguishable from a run that produced no
/// capture at all. Named as the highest-value instrumentation point in
/// `docs/issues/archive/2026-08-26-wine-lane-flakes-under-load-on-three-tests.md` § Resume, after
/// a load-contention flake on the wine lane dropped exactly this key group — the tell being that
/// the surviving keys are set in the same block as the missing ones, so the response could not
/// have come from that block. The response path also says so (`unfiltered_output_skipped`).
///
/// Bug bc0cb248b224d1dd: this is the UNFILTERED stream, read from a file, so it never passed the
/// decode in `run_command_inner` that scrubs `stdout`. `env | grep PATH` shows one line inline
/// while this buffer holds the whole environment.
pub(super) fn read_capture(tmp: &TmpfileGuard) -> Option<CaptureRead> {
    use std::io::Read;
    let mut bytes = Vec::new();
    let read = std::fs::File::open(&tmp.0).and_then(|file| {
        let file_len = file.metadata()?.len();
        file.take(CAPTURE_READ_LIMIT_BYTES as u64)
            .read_to_end(&mut bytes)?;
        Ok(file_len)
    });
    match read {
        Ok(file_len) => {
            // Cut by the read bound (the file is longer than what was read), or the file reached
            // the bound `splice_tee`'s `ulimit -f` puts on it (512-byte units: what `sh` counts).
            // Either way the stream may go on past it, whatever the store limit makes of the
            // text after redaction, so the store marks the handle truncated.
            let file_bound =
                (super::super::command_summary::capture_file_blocks(CAPTURE_STORE_LIMIT_BYTES)
                    * 512) as u64;
            let cut = file_len > bytes.len() as u64 || file_len >= file_bound;
            let content = String::from_utf8_lossy(&bytes);
            let scrubbed = redact_credentials(&content);
            Some(CaptureRead {
                redacted: scrubbed.count,
                text: scrubbed.text.into_owned(),
                cut,
            })
        }
        Err(e) => {
            tracing::warn!(
                path = %tmp.0,
                error = %e,
                "tee capture unreadable; unfiltered_output keys will be absent from \
                 this response"
            );
            None
        }
    }
}

/// The exit status of the producer, which the spliced wrapper `{ <command>; s=$?; echo $s >| '<file>'; exit $s; }`
/// wrote to `file` when the producer ended. A pipeline's own status is its last stage's, so for
/// `cargo test | grep FAILED` the response's `exit_code` is grep's; this is what `cargo test` returned.
/// `None` when the file holds no status: the producer was killed before it could be written (a
/// timeout), the shell never reached the `echo`, or the file cannot be read. A status that is not a
/// shell exit status (outside 0 to 255) is not trusted either.
///
/// A producer stopped by the filter closing the pipe has status 141 (`128 + SIGPIPE`): that is not a
/// failure of the command, and happens only when the stream outran the bounded drain (4 MiB).
pub(super) fn read_producer_status(file: &TmpfileGuard) -> Option<i32> {
    use std::io::Read;
    let mut text = String::new();
    // A status is at most three digits and a newline; the bound keeps a file that holds anything
    // else from being read whole.
    std::fs::File::open(&file.0)
        .ok()?
        .take(16)
        .read_to_string(&mut text)
        .ok()?;
    let code: i32 = text.trim().parse().ok()?;
    (0..=255).contains(&code).then_some(code)
}

/// What [`read_capture`] read of a capture file.
#[derive(Debug)]
pub(super) struct CaptureRead {
    /// The text read, decoded lossily and scrubbed of credentials.
    pub(super) text: String,
    /// How many credential values the scrub replaced.
    pub(super) redacted: usize,
    /// The file is longer than what was read, or reached its on-disk bound: the stream may go on
    /// past `text`, so [`store_capture`] marks the handle truncated even if `text` fits the store.
    pub(super) cut: bool,
}

/// Store an already-scrubbed capture as an `@cmd_*` handle, cut at [`CAPTURE_STORE_LIMIT_BYTES`].
///
/// Returns `(handle, cut record the pool kept, line count of the capture READ, pool-cut notices)`,
/// the tuple `unfiltered_ref` carries. The count is of everything read, before the store cut: the
/// whole stream when it fit the capture file, a LOWER BOUND of it when the file or the read was
/// cut (`source_cut`, which sets the cut record whatever the size, so `unfiltered_truncated` marks
/// it). `None` for an empty capture: when the terminal filter
/// matched nothing, both `raw_stdout` and the tee file are empty, and surfacing a handle is
/// misleading.
pub(super) fn store_capture(
    content: String,
    source_cut: bool,
    original_command: &str,
    exit_code: i32,
    ctx: &ToolContext,
) -> Option<(
    String,
    Option<crate::tools::output_buffer::Truncation>,
    usize,
    Vec<String>,
)> {
    use super::super::command_summary::count_lines;
    if content.is_empty() {
        return None;
    }
    // Counted on everything read, before any storage truncation below — the whole point is
    // telling the caller how much is behind the ref, not how much of it happened to fit. A lower
    // bound of the stream when the capture file or the read was cut, which this cut then marks.
    let line_count = count_lines(&content);
    // `source_cut` (the capture FILE or its read was cut, [`CaptureRead::cut`]) marks the handle even
    // when the redacted text fits: the stream went on past what is kept. By construction such a
    // text is over the limit anyway unless the scrub shrank it by a megabyte or more.
    let (stored, truncation) = if source_cut || content.len() > CAPTURE_STORE_LIMIT_BYTES {
        let mut byte_budget = CAPTURE_STORE_LIMIT_BYTES;
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
    let (ref_id, held) = ctx.output_buffer.store_truncated_kept(
        original_command.to_string(),
        stored,
        String::new(), // unfiltered capture is stdout-only
        exit_code,
        truncation,
    );
    // `take_while` keeps the line that empties the budget, however long, so the pool can cut
    // the capture again. What the response reports is the HANDLE's own record, and a cut the
    // pool made is announced (review probe: a 1.5 MB single line reported 1 buffered line
    // while the handle kept 0 of 1). The record comes from the store's own lock hold: looked
    // up afterwards, it was lost when another store evicted the handle in between.
    let pool_cut = match held {
        Some(t) if held != truncation => {
            vec![crate::tools::output_buffer::cut_notice(&ref_id, t)]
        }
        _ => Vec::new(),
    };
    Some((ref_id, held, line_count, pool_cut))
}

/// The `hint` of a timed-out filtered command whose partial capture was kept (`capture_incomplete`).
/// It replaces the plain timeout hint, which sends the caller to a higher `timeout_secs`: that does
/// not help a producer that never ends or is slower than any timeout.
// cap-class: NOT_A_CAP — hint text, not a bound
pub(super) const CAPTURE_INCOMPLETE_TIMEOUT_HINT: &str =
    "The producer was killed at the timeout. The result above is the filter run on the partial \
     capture (or `filter_not_run` / `filter_grace_expired` says why there is none); \
     `unfiltered_output` holds that partial stream. Raising timeout_secs will not help an endless \
     or slow producer: bound the producer (`timeout 10 cmd`, `ping -c 3`) or run it bare.";

/// Add what a timed-out filtered command recovered to its `timed_out` response: what its filter
/// made of the partial capture (`run`, its streams already scrubbed) and the capture's handle
/// (`unfiltered`, from [`store_capture`]).
///
/// The response gains `capture_incomplete` (its `hint` becomes [`CAPTURE_INCOMPLETE_TIMEOUT_HINT`]),
/// the `unfiltered_*` keys and the redaction note through
/// [`attachments`], the helper the normal path uses for them, then by `run`:
/// - `Ran`: `filter_exit_code` when it is not 0 (grep's 1 included: it says "no match", which an
///   empty `stdout` alone does not), `filter_stderr` when there is any (cut to
///   [`FILTER_STDERR_BYTE_BUDGET`] behind a marker), and `stdout`;
/// - `GraceExpired`: `filter_grace_expired`, no `stdout`;
/// - `NotRun(reason)`: `filter_not_run: reason` (cut like `filter_stderr`), no `stdout`.
///
/// `stdout` is cut to the room every other key leaves within the inline limit, MEASURED from the
/// response itself: nothing bounds a filter's output, and this response is built outside
/// `handle_successful_output_with`'s gate. A cut `stdout` says so with `truncated`, `stdout_shown`
/// and `stdout_total`, the keys a cut buffer query carries.
///
/// A response with no handle (`unfiltered` is `None`: an empty capture) is left as it is.
pub(super) fn attach_partial_capture(
    response: &mut Value,
    run: super::inner::FilterRun,
    unfiltered: Option<(
        String,
        Option<crate::tools::output_buffer::Truncation>,
        usize,
        Vec<String>,
    )>,
    redacted: usize,
    original_command: &str,
) {
    use super::super::command_summary::{count_lines, truncate_lines_and_bytes};
    use super::inner::FilterRun;
    if unfiltered.is_none() {
        return;
    }
    let keys = attachments(&[], &unfiltered, redacted, &None, &serde_json::Map::new());
    let Some(obj) = response.as_object_mut() else {
        return;
    };
    obj.insert("capture_incomplete".into(), json!(true));
    // The plain timeout hint ("pass a higher timeout_secs") is wrong here: an endless or slow
    // producer into a filter (`tail -f log | grep -m1 ready`) times out again at any timeout.
    obj.insert("hint".into(), json!(CAPTURE_INCOMPLETE_TIMEOUT_HINT));
    obj.extend(keys);
    let text = match run {
        FilterRun::GraceExpired => {
            obj.insert("filter_grace_expired".into(), json!(true));
            return;
        }
        FilterRun::NotRun(reason) => {
            obj.insert(
                "filter_not_run".into(),
                json!(bound_filter_not_run(&reason)),
            );
            return;
        }
        FilterRun::Ran {
            stdout,
            stderr,
            exit_code,
        } => {
            if exit_code != 0 {
                obj.insert("filter_exit_code".into(), json!(exit_code));
            }
            if !stderr.is_empty() {
                obj.insert("filter_stderr".into(), json!(bound_filter_stderr(&stderr)));
            }
            stdout
        }
    };
    // Output that fits beside the response as it stands is kept byte for byte, as the normal path
    // keeps it: the cut rejoins lines and would drop a trailing newline.
    if crate::util::text::json_escaped_len(&text) <= inline_stdout_room(response) {
        response["stdout"] = json!(text);
        return;
    }
    let total = count_lines(&text);
    // The cut keys at their widest (`stdout_shown` can be no larger than the total).
    let mut skeleton = response.clone();
    skeleton["truncated"] = json!(true);
    skeleton["stdout_shown"] = json!(total);
    skeleton["stdout_total"] = json!(total);
    let cut = truncate_lines_and_bytes(
        &text,
        usize::MAX,
        inline_stdout_room(&skeleton),
        &wide_line_remedy(original_command),
    );
    if cut.shown < cut.total || cut.clipped_wide {
        response["truncated"] = json!(true);
        response["stdout_shown"] = json!(cut.shown);
        response["stdout_total"] = json!(cut.total);
    }
    response["stdout"] = json!(cut.text);
}

/// The most JSON-escaped bytes of a timed-out command's filter stderr the response carries
/// (`filter_stderr`). It reports why the filter failed (a bad regex, a missing command), which is
/// one or two lines; a filter that writes more is cut in the middle behind a marker. Stored
/// nowhere: the filter can be re-run on the `unfiltered_output` handle for all of it.
// cap-class: RESULT_CAP run_command.filter_stderr_bytes — probed
pub(super) const FILTER_STDERR_BYTE_BUDGET: usize = 500;

/// `filter_stderr`, cut to [`FILTER_STDERR_BYTE_BUDGET`] escaped bytes behind a marker whose
/// remedy is true.
fn bound_filter_stderr(stderr: &str) -> String {
    crate::util::text::elide_middle_escaped(
        stderr,
        stderr.len(),
        FILTER_STDERR_BYTE_BUDGET,
        "filter_stderr",
        "re-run the filter on the `unfiltered_output` handle for all of it",
    )
}

/// `filter_not_run`, cut like `filter_stderr` to [`FILTER_STDERR_BYTE_BUDGET`] escaped bytes
/// behind a marker. The reason quotes the caller's own text (a `sed` script, a flag), which nothing
/// bounds, so a long one would otherwise ride into the response whole.
fn bound_filter_not_run(reason: &str) -> String {
    crate::util::text::elide_middle_escaped(
        reason,
        reason.len(),
        FILTER_STDERR_BYTE_BUDGET,
        "filter_not_run",
        "the cut part quotes the command as you wrote it",
    )
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
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_successful_output(
    original_command: &str,
    raw_stdout: String,
    raw_stderr: String,
    exit_code: i32,
    buffer_only: bool,
    unfiltered_tmpfile: Option<TmpfileGuard>,
    work_dir: &std::path::Path,
    ctx: &ToolContext,
) -> anyhow::Result<Value> {
    handle_successful_output_with(
        original_command,
        raw_stdout,
        raw_stderr,
        exit_code,
        buffer_only,
        unfiltered_tmpfile,
        work_dir,
        ctx,
        LateKeys::default(),
    )
    .await
}

/// [`handle_successful_output`] for a caller that knows keys the response must also carry
/// (`LateKeys`). They are passed IN, not added afterwards, because the summary-or-inline gate
/// measures the response including them: a key added after the gate ran could tip a response
/// that exactly fit over the inline limit and have it re-buffered under `@tool_*`.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_successful_output_with(
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
    mut late_keys: LateKeys,
) -> anyhow::Result<Value> {
    use super::super::command_summary::{
        count_lines, detect_command_type, inline_response_exceeds_limit, strip_ansi_codes,
        summarize_build_output_within, summarize_generic_within, summarize_test_output_within,
        truncate_lines_and_bytes, CommandType, BUFFER_QUERY_INLINE_CAP,
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
        Vec<String>,
    )> = if let Some(ref tmpfile) = unfiltered_tmpfile {
        // tmpfile drops at function exit — TmpfileGuard::drop() removes the file.
        // `read_capture` is `None` on a logged read error, which the response then says
        // (`unfiltered_output_skipped`); `store_capture` is `None` for an empty capture (the
        // terminal filter matched nothing, so there is no handle to show).
        let read = read_capture(tmpfile);
        if read.is_none() {
            late_keys.tee_skipped.get_or_insert_with(|| {
                "the unfiltered-output capture was made but could not be read back (logged); \
                 only the `unfiltered_output` buffer is missing"
                    .to_string()
            });
        }
        read.and_then(|read| {
            tee_redacted = read.redacted;
            store_capture(read.text, read.cut, original_command, exit_code, ctx)
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
    .await
    .map(|text| {
        let bounded = bound_wip_authors(text.clone());
        // Cut iff changed: `elide_middle_escaped` returns fitting text unchanged, and a cut is
        // shorter in escaped bytes than its input. Recorded in the envelope below, not read back
        // from the marker: the diagnostic quotes file paths, which can hold any text.
        let cut = bounded != text;
        (bounded, cut)
    });
    let wip_authors_cut = wip_authors.as_ref().is_some_and(|(_, cut)| *cut);
    let wip_authors = wip_authors.map(|(text, _)| text);

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
    // The bounded stderr IS fed into the summary-or-inline gate below (`stderr_cut`): that gate
    // measures the response an arm will actually emit, and that response carries this stderr.
    // What this never changes is what is STORED.
    // cap-class: RESULT_CAP run_command.stderr_lines — probed
    const STDERR_BUDGET: usize = 20;
    // Where the stderr a buffer query carries comes from decides what its marker may promise: the
    // entry's STORED stderr is behind `<handle>.err`, the query's OWN stderr is stored nowhere.
    //
    // A query of `<handle>.err` takes NO stored stderr: it reads that stream, so the stream is its
    // payload, its stdout, and the side channel would carry it a second time. Measured before this
    // rule: `cat @cmd_X.err` over 300 stored stderr lines answered `stdout 80/300, stderr 20/300`,
    // the twenty lines spent on a repeat of lines the reader was already paging. The query's OWN
    // stderr (a failing `awk`) is still carried: it is in neither stream.
    let queries_stored_stderr =
        queried_stream(original_command).is_some_and(|s| s.ends_with(".err"));
    let stderr_is_stored = buffer_only && raw_stderr.is_empty() && !queries_stored_stderr;
    let buffer_stderr: String = if stderr_is_stored {
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
    // Bounded ONCE, here: the gate and every arm below read this one value, so they cannot
    // disagree about what the response carries.
    let stderr_cut: Option<(String, usize, usize)> = buffer_only.then(|| {
        bound_buffer_stderr(
            &buffer_stderr,
            STDERR_BUDGET,
            original_command,
            !stderr_is_stored,
        )
    });

    // Every key attached after the streams, built ONCE: the gate measures it, the end of this
    // function applies it. The two cannot drift because there is one map.
    let mut late = attachments(
        &[
            ("shell_cause", &shell_cause),
            ("wip_authors", &wip_authors),
            ("empty_test_selection", &empty_selection),
            ("partial_test_selection", &partial_selection),
        ],
        &unfiltered_ref,
        tee_redacted + late_keys.redacted,
        &late_keys.tee_skipped,
        &late_keys.envelope,
    );
    // Added to the one map the gate measures, so the summary-or-inline gate prices it. Not an
    // `attachments` parameter: it is not part of the capture's own keys, which that function builds.
    if let Some(code) = late_keys.producer_exit_code {
        late.insert("producer_exit_code".into(), json!(code));
    }
    if let Some(signal) = late_keys.terminated_by_signal {
        late.insert("terminated_by_signal".into(), json!(signal));
    }

    // --- Step 6: Decide whether to buffer + summarize ---
    //
    // The refresh lines (`LateKeys::stdout_prefix`) go INTO `stdout` where the response has one,
    // so each arm prices them there. Escaping is per character, so the escaped prefix plus the
    // escaped stdout is exactly the escaped joined string.
    let prefix = late_keys.stdout_prefix.as_str();
    let prefix_cost = json!(prefix).to_string().len() - 2;
    let with_prefix = |text: &str| format!("{prefix}{text}");
    //
    // Decided on the SERIALIZED response, not the raw streams: see `inline_response_exceeds_limit`.
    // The extras are the exact serialized cost of the keys attached after the streams: the
    // diagnostics, the tee keys, the redaction note, and, for a buffer query, the stderr counters.
    let stderr_for_gate: &str = match &stderr_cut {
        Some((text, _, _)) => text,
        None => &raw_stderr,
    };
    let mut gate_keys = late.clone();
    if let Some((_, shown, total)) = &stderr_cut {
        if shown < total {
            gate_keys.insert("stderr_shown".into(), json!(shown));
            gate_keys.insert("stderr_total".into(), json!(total));
        }
    }
    // A buffer query that fits whole but filled a hint's own 100-line window carries the route to
    // the next window: a trailing pager's, or a plain slice's while its stream has a line left.
    // Measured here, with the other late keys, so a page that fits only without it is cut instead
    // of re-buffered.
    let full_page = buffer_only
        .then(|| full_window_hint(ctx, original_command, count_lines(&raw_stdout)))
        .flatten();
    if let Some(h) = &full_page {
        gate_keys.insert("hint".into(), json!(h));
    }
    let extras = extras_len(
        &gate_keys,
        unfiltered_ref.is_some() && raw_stdout.is_empty(),
    ) + if raw_stdout.is_empty() {
        0
    } else {
        prefix_cost
    };
    // A libtest run is compacted BEFORE the gate judges it, and the gate is asked about the
    // COMPACTED response when there is one: see `compacted_fits`. Only for runs whose raw bytes fit
    // the limit, the population that was returned inline (and compacted) before the gate counted
    // serialized bytes: a run over it was summarized then and is now, unchanged. Compaction that
    // does not fit either falls through to the raw gate, as it always did.
    let compacted = (!buffer_only
        // A refresh prefix is not priced by `compacted_fits`: such a run takes the raw gate.
        && prefix.is_empty()
        && raw_stdout.len() + raw_stderr.len() <= crate::tools::INLINE_MAX_RESPONSE_LEN
        && detect_command_type(original_command) == CommandType::Test)
        .then(|| crate::tools::libtest_compact::compact_libtest_output(&raw_stdout, &raw_stderr))
        .flatten()
        .filter(|c| compacted_fits(c, &raw_stdout, &raw_stderr, exit_code, &late));
    let summary_needed = compacted.is_none()
        && inline_response_exceeds_limit(exit_code, &raw_stdout, stderr_for_gate, extras);
    let mut result = if summary_needed {
        if let Some((stderr_out, stderr_shown, stderr_total)) = stderr_cut.clone() {
            // Buffer-only: return inline, never create a new buffer ref (avoids infinite loop).
            let line_budget =
                BUFFER_QUERY_INLINE_CAP - STDERR_BUDGET.min(count_lines(&buffer_stderr));
            let stdout_lines = count_lines(&raw_stdout);
            let stderr_note = if stderr_total > 0 {
                format!(", stderr {stderr_shown}/{stderr_total}")
            } else {
                String::new()
            };
            let wide_remedy = wide_line_remedy(original_command);

            // The room for stdout text is what is left of the inline limit after the truncated
            // response's OTHER keys, MEASURED from a skeleton that has them all (a stdout-less
            // copy of the response about to be built, its hint at its widest: `shown` at its
            // largest possible value, `total`). It was a reserve of 300 B and then 800 B. Measured,
            // for a one-line query the keys and hint take 211 B (383 B with the clipped-line
            // sentence), which leaves room for 9,780 B of stdout where the 800 B reserve left 9,200 B.
            let room_for = |clipped_wide: bool| {
                // The hint at its widest. A non-slice query's hint names a next page only while
                // lines remain, so `shown == total` is not always the widest: measure `total - 1`
                // too. Every number in the hint grows with `shown`, so these two bound every value
                // `shown` can take.
                let widest_hint = [stdout_lines.saturating_sub(1), stdout_lines]
                    .into_iter()
                    .map(|shown| {
                        truncation_hint(
                            original_command,
                            shown,
                            stdout_lines,
                            &stderr_note,
                            clipped_wide,
                            // The byte-budget wording is the longer one, so the room measured with
                            // it can only be short of the true room, never over it.
                            true,
                        )
                    })
                    .max_by_key(String::len)
                    // The hint of a window that fits whole is not shorter than every cut hint:
                    // price it too.
                    .into_iter()
                    .chain(full_page.clone())
                    .max_by_key(String::len)
                    .unwrap_or_default();
                let mut skeleton = json!({
                    "exit_code": exit_code,
                    "truncated": true,
                    "stdout_shown": stdout_lines,
                    "stdout_total": stdout_lines,
                    "hint": widest_hint,
                });
                if !stderr_out.is_empty() {
                    skeleton["stderr"] = json!(stderr_out);
                }
                if stderr_total > 0 {
                    skeleton["stderr_shown"] = json!(stderr_shown);
                    skeleton["stderr_total"] = json!(stderr_total);
                }
                for (key, value) in &late {
                    skeleton[key.as_str()] = value.clone();
                }
                inline_stdout_room(&skeleton).saturating_sub(prefix_cost)
            };
            let mut cut =
                truncate_lines_and_bytes(&raw_stdout, line_budget, room_for(false), &wide_remedy);
            if cut.clipped_wide {
                // The clipped-line sentence makes the hint longer; budget for it and cut again.
                cut = truncate_lines_and_bytes(
                    &raw_stdout,
                    line_budget,
                    room_for(true),
                    &wide_remedy,
                );
            }
            let clipped_wide = cut.clipped_wide;
            let cut_by_bytes = cut.by_bytes;
            let (stdout_out, stdout_shown, stdout_total) = (cut.text, cut.shown, cut.total);

            let was_truncated =
                stdout_shown < stdout_total || clipped_wide || stderr_shown < stderr_total;

            let mut result = json!({"exit_code": exit_code});
            if !stdout_out.is_empty() {
                result["stdout"] = json!(with_prefix(&stdout_out));
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
                result["hint"] = json!(truncation_hint(
                    original_command,
                    stdout_shown,
                    stdout_total,
                    &stderr_note,
                    clipped_wide,
                    cut_by_bytes,
                ));
            } else if let Some(h) = &full_page {
                // Not cut after all, but a full page window: the route to the next one. Its room
                // is priced above (`room_for` counts it among the widest hints).
                result["hint"] = json!(h);
            }
            result
        } else {
            let output_id = ctx.output_buffer.store(
                original_command.to_string(),
                raw_stdout.clone(),
                raw_stderr.clone(),
                exit_code,
            );
            // Streams bigger than the pool's byte limit are kept as a marked prefix. The caller is
            // told here, inside the render so `fit_summary` measures it, and by every later read
            // of the handle.
            let output_cut = ctx.output_buffer.truncation_notices_in(&output_id);

            let cmd_type = detect_command_type(original_command);
            let tee_present = unfiltered_ref.is_some();
            // The summary is built by `fit_summary` against the SERIALIZED response it will be:
            // each render is the final object, handle and late keys included, so what is measured is
            // what is returned. The streams are bounded in escaped bytes there, from the raw text,
            // once. (`attach` is repeated at the end of this function for every arm; applying the
            // same keys twice changes nothing.)
            super::super::command_summary::fit_summary(|budget| {
                let cmd_summary = match cmd_type {
                    CommandType::Test => {
                        summarize_test_output_within(&raw_stdout, &raw_stderr, exit_code, budget)
                    }
                    CommandType::Build => {
                        summarize_build_output_within(&raw_stdout, &raw_stderr, exit_code, budget)
                    }
                    CommandType::Generic => {
                        summarize_generic_within(&raw_stdout, &raw_stderr, exit_code, budget)
                    }
                };
                // Rebuild with correct field order so output_id appears before content fields.
                let mut response = rebuild_buffered_summary(cmd_summary, &output_id);
                attach(&mut response, late.clone(), tee_present);
                if !output_cut.is_empty() {
                    response["output_truncated"] = json!(output_cut);
                }
                // Inside the render, so `fit_summary` measures it. A summary with no `stdout` (a
                // test or build summary) gets one holding just the refresh line, which was
                // otherwise never delivered on that shape.
                if !prefix.is_empty() {
                    let text = response.get("stdout").and_then(Value::as_str).unwrap_or("");
                    response["stdout"] = json!(with_prefix(text));
                }
                response
            })
        }
    } else if let Some(c) = compacted {
        // A short libtest run is ~40% cargo progress and empty sibling targets
        // (docs/research/2026-09-24-rtk-evaluation.pdf § 7). Compacted only when that
        // pays AND the compacted response fits (`compacted_fits`); otherwise the raw arm below
        // returns it exactly as before.
        compacted_test_response(
            c,
            original_command,
            &raw_stdout,
            &raw_stderr,
            exit_code,
            ctx,
        )
    } else {
        // The response fits: the gate above measured exactly this object. A buffer query's
        // output is returned WHOLE here, at any line count: there is no second, narrower cap for
        // output the gate has already said fits (an arm that cut at 9,700 B did exactly that, for
        // nothing, while the gate measured the real size).
        let mut r = json!({"exit_code": exit_code});
        if !raw_stdout.is_empty() {
            r["stdout"] = json!(with_prefix(&raw_stdout));
        }
        if let Some(h) = &full_page {
            // Measured by the gate above (`gate_keys`).
            r["hint"] = json!(h);
        }
        if let Some((stderr_out, stderr_shown, stderr_total)) = &stderr_cut {
            // THE REPRODUCED PATH. `grep -c MARKER @cmd_abc` returns two bytes, so
            // `needs_summary` is false and control arrives HERE — which is exactly
            // the query whose `0` a reader cannot tell apart from a stream that was
            // never surfaced. Bounded like the summary arm so a large stored stderr
            // cannot re-trigger buffering on a query whose own output was short.
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
    };

    // Attach everything built into `late` above. A non-empty tee capture behind an empty filtered
    // `stdout` is exactly the case docs/issues/archive/2026-08-26-unfiltered-output-ref-carries-no-size-signal.md
    // covers: without an explicit `"stdout": ""` and a line count, the response looks
    // identical whether the ref holds 2 lines or 20,000.
    attach(&mut result, late, unfiltered_ref.is_some());
    // AFTER every arm measured its response: the record is stripped by the backstop before
    // delivery, so no gate above may count it. `wip_authors` is the one late key cut at its source;
    // the summarizers record their own fields.
    if wip_authors_cut && result.get("wip_authors").is_some() {
        crate::tools::record_cut(&mut result, "wip_authors");
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

    /// A page hint counts the lines of the output it follows. After `sed -n '2,101p'` those start
    /// at 2, so the next page is 102.., not 1.. — which sent a reader back to the top.
    #[test]
    fn a_ranged_sed_query_gets_a_next_page_in_the_buffers_own_numbering() {
        let hint = capped_hint("sed -n '2,101p' @cmd_abc12345", 100, 500, "", false, false);
        assert!(hint.contains("sed -n '102,201p' @cmd_abc12345"), "{hint}");
        // And a plain read keeps the old numbering, which already was the buffer's.
        let plain = capped_hint("cat @cmd_abc12345", 100, 500, "", false, false);
        assert!(plain.contains("sed -n '101,200p' @cmd_abc12345"), "{plain}");
        assert!(
            !plain.contains("grep -o"),
            "the wide-line sentence only appears when a line was clipped"
        );
    }
    /// Only the WHOLE command `cat <ref>` or `sed -n 'A,Bp' <ref>` is a slice. A ranged `sed`
    /// anywhere else in a command used to set the numbering of a hint about a different output.
    #[test]
    fn only_a_whole_cat_or_ranged_sed_of_one_ref_is_a_plain_slice() {
        for (command, want) in [
            ("cat @cmd_abc12345", Some(1)),
            ("  cat @cmd_abc12345.err  ", Some(1)),
            ("sed -n '2,101p' @cmd_abc12345", Some(2)),
            ("sed -n \"7,9p\" @file_abc12345", Some(7)),
            ("sed -n 7,9p @cmd_abc12345.err", Some(7)),
            ("sed -n '0,9p' @cmd_abc12345", Some(1)),
            ("grep x @cmd_abc12345 | sed -n '5,10p'", None),
            ("sed -n '5,10p' @cmd_abc12345 | grep x", None),
            ("cat @cmd_abc12345 @cmd_abc12346", None),
            ("cat -n @cmd_abc12345", None),
            ("cat @cmd_abc12345 > out.txt", None),
            ("cat @cmd_abc12345; ls", None),
            ("cat @bg_abc12345", Some(1)),
            // Ends like a slice, starts like something else: the START anchor's cases.
            ("grep -c x @cmd_abc12345; cat @cmd_abc12345", None),
            ("echo x | cat @cmd_abc12345", None),
            ("grep -n \"\" @cmd_abc12345", None),
        ] {
            assert_eq!(plain_slice_first_line(command), want, "{command}");
        }
    }

    /// `plain_slice_window` reads the END of the range too, in every quote form, and nothing but a
    /// whole `cat` or ranged `sed` of one ref is a window.
    #[test]
    fn a_plain_slice_window_has_both_ends_in_every_quote_form() {
        for (command, want) in [
            ("cat @cmd_abc12345", Some((1, None))),
            ("sed -n '101,200p' @cmd_abc12345", Some((101, Some(200)))),
            (
                r#"sed -n "101,200p" @cmd_abc12345.err"#,
                Some((101, Some(200))),
            ),
            ("sed -n 101,200p @tool_abc12345", Some((101, Some(200)))),
            ("sed -n '5,3p' @file_abc12345", Some((5, Some(3)))),
            ("grep x @cmd_abc12345 | sed -n '101,200p'", None),
            ("sed -n '101,200p' @cmd_abc12345 2>&1", None),
        ] {
            assert_eq!(plain_slice_window(command), want, "{command}");
        }
    }

    /// The next-window hint of a plain slice: a full page (exactly 100 lines, from a window of
    /// exactly 100) with a line left in the stream. The stream is asked only for such a window.
    #[test]
    fn next_window_hint_wants_a_full_page_and_a_line_left() {
        use std::cell::Cell;
        let ask = |query: &str, lines: usize, total: usize| {
            let asked = Cell::new(false);
            let hint = next_window_hint(query, lines, |_| {
                asked.set(true);
                Some(total)
            });
            (hint, asked.get())
        };
        let q = "sed -n '101,200p' @cmd_abc12345";

        // A full page with a line left names the next window, on the stream that was queried.
        let (hint, asked) = ask(q, 100, 201);
        assert!(asked);
        let hint = hint.expect("a line is left");
        assert!(
            hint.contains("Next page: sed -n '201,300p' @cmd_abc12345."),
            "{hint}"
        );
        // The same page at the end of the stream, and short of it.
        assert!(ask(q, 100, 200).0.is_none());
        assert!(ask(q, 100, 199).0.is_none());
        // A page that did not print 100 lines is the end, whatever the stream is said to hold, and a
        // stream count that disagrees with the page is not believed in the other direction either.
        assert!(ask(q, 99, 5_000).0.is_none());
        assert!(ask(q, 101, 5_000).0.is_none());
        // Only the hint's own window width asks the stream at all.
        for other in [
            "sed -n '101,199p' @cmd_abc12345",
            "sed -n '101,201p' @cmd_abc12345",
            "sed -n '101,100p' @cmd_abc12345",
            "cat @cmd_abc12345",
            "grep x @cmd_abc12345 | sed -n '101,200p'",
        ] {
            let (hint, asked) = ask(other, 100, 5_000);
            assert!(hint.is_none() && !asked, "{other}");
        }
        // The `.err` suffix and the quote forms keep the stream.
        let (hint, _) = ask(r#"sed -n "1,100p" @cmd_abc12345.err"#, 100, 150);
        let hint = hint.expect("a line is left");
        assert!(
            hint.contains("Next page: sed -n '101,200p' @cmd_abc12345.err."),
            "{hint}"
        );
    }

    /// Why the room for a cut `stdout` need not price a full window's hint (it does anyway, as a
    /// guard): the gate measures `{exit_code, stdout, hint}` and the cut skeleton holds those plus
    /// `truncated`, `stdout_shown` and `stdout_total`, so a window the gate calls too big is cut
    /// whenever the full hint is no longer than the widest cut hint plus those three keys. The cut
    /// hint is built with the inputs `room_for(false)` gives it for an uncut full window: no stderr
    /// note and no clipped-line sentence (either would only add slack).
    #[test]
    fn a_full_window_hint_never_outgrows_the_keys_a_cut_response_adds() {
        for handle in [
            "@cmd_0bf0a111",
            "@cmd_0bf0a111.err",
            "@tool_0bf0a111",
            "@file_0bf0a111",
            "@bg_00000034",
        ] {
            for (a, total) in [(1usize, 101usize), (100_001, 9_999_999)] {
                let query = format!("sed -n '{a},{}p' {handle}", a + 99);
                let full = next_window_hint(&query, 100, |_| Some(total)).expect("a line is left");
                let widest_cut = [99usize, 100]
                    .into_iter()
                    .map(|shown| truncation_hint(&query, shown, 100, "", false, true))
                    .max_by_key(String::len)
                    .unwrap();
                let gate = json!({ "exit_code": 0, "stdout": "", "hint": full }).to_string();
                let skeleton = json!({
                    "exit_code": 0,
                    "truncated": true,
                    "stdout_shown": 100,
                    "stdout_total": 100,
                    "stdout": "",
                    "hint": widest_cut,
                })
                .to_string();
                assert!(
                    gate.len() <= skeleton.len(),
                    "{query}: the full hint ({} B) outgrew the cut skeleton by {} B",
                    full.len(),
                    gate.len() - skeleton.len()
                );
            }
        }
    }

    /// What `can_wrap_query` decides: the query is one pipeline, so `( query; )` closes it and
    /// nothing else. A list, a comment, a heredoc and a `$(…)` are not. (It asks the IL-3 gate
    /// nothing; see its doc comment for why that is sound for the `| cut` stage.)
    #[test]
    fn can_wrap_query_decides_whether_the_query_is_one_pipeline() {
        for (q, want) in [
            ("grep -c a @cmd_abc12345", true),
            ("grep -c a @cmd_abc12345 | cat", true),
            ("cat @cmd_abc12345 >&2 | tr a b | cat", true),
            ("! cat @cmd_abc12345 >&2", true),
            ("grep -c a @cmd_abc12345; cat @cmd_abc12345", false),
            ("grep -c a @cmd_abc12345 && cat @cmd_abc12345", false),
            ("grep -c a @cmd_abc12345 # note", false),
            ("cat @cmd_abc12345 <<EOF\nx\nEOF", false),
            ("echo $(cat @cmd_abc12345)", false),
            ("grep -c a @cmd_abc12345\n", false),
        ] {
            assert_eq!(can_wrap_query(q), want, "{q:?}");
        }
    }

    /// A non-slice query is paged by appending to it, numbered from 1, and never by a `sed` on the
    /// handle; one that already ends in a pager has that pager REPLACED, numbered from what was shown;
    /// the hint never echoes the query.
    #[test]
    fn a_non_slice_hint_appends_a_pager_and_does_not_echo_the_query() {
        // A ranged `sed` FIRST is not the numbering of the output that follows it.
        let hint = capped_hint(
            "sed -n '5,3000p' @cmd_abc12345 | grep 1",
            100,
            296,
            "",
            false,
            false,
        );
        assert!(
            hint.contains("append `| sed -n '101,200p'` to this query"),
            "{hint}"
        );
        assert!(!hint.contains("p' @cmd_abc12345"), "{hint}");
        assert!(!hint.contains("grep 1"), "the query is not echoed: {hint}");
        // Ends in a pager: lines 5.. of the grep, so the next page starts after the 48 shown.
        let paged = capped_hint(
            "grep 1 @cmd_abc12345 | sed -n '5,104p'",
            48,
            100,
            "",
            false,
            true,
        );
        assert!(
            paged.contains("replace that last stage with `sed -n '53,152p'`"),
            "{paged}"
        );
        assert!(!paged.contains("append"), "a stacked second sed: {paged}");
        // A list cannot take a stage that reads all of its output.
        let listed = capped_hint(
            "grep -c 1 @cmd_abc12345; grep 1 @cmd_abc12345",
            100,
            296,
            "",
            false,
            false,
        );
        assert!(!listed.contains("| sed"), "{listed}");
        assert!(listed.contains("narrow the query"), "{listed}");
        // Nothing left: no next page.
        let done = capped_hint("grep 1 @cmd_abc12345", 1, 1, "", true, true);
        assert!(!done.contains("sed -n"), "{done}");
        assert!(done.contains("appending `| cut -c1-4000`"), "{done}");
    }

    // -- the gate and the budgets measure the response; nothing here is a reserve --

    fn full_attachments() -> serde_json::Map<String, Value> {
        attachments(
            &[
                ("shell_cause", &Some("a \"quoted\"\ncause".to_string())),
                ("wip_authors", &Some("w".repeat(40))),
                ("empty_test_selection", &None),
                ("partial_test_selection", &None),
            ],
            &Some((
                "@cmd_0bf0a111".to_string(),
                Some(crate::tools::output_buffer::Truncation {
                    kept_lines: 5,
                    total_lines: 9_999_999,
                }),
                9_999_999,
                Vec::new(),
            )),
            3,
            &Some("no space left".to_string()),
            &serde_json::Map::from_iter([
                (
                    "buffer_truncated".to_string(),
                    json!(["@cmd_0bf0a222 holds a \"prefix\""]),
                ),
                (
                    "timeout_hint".to_string(),
                    json!("timeout_secs: 0 is invalid"),
                ),
            ]),
        )
    }

    /// `extras_len` must equal what the keys really add, computed from a BUILT response and not
    /// from any constant: reserves for these keys were 160 B and 48 B and each was wrong.
    #[test]
    fn extras_len_is_the_exact_serialized_cost_of_the_attached_keys() {
        let keys = full_attachments();
        for expected in [
            "unfiltered_output",
            "unfiltered_output_lines",
            "unfiltered_truncated",
            "unfiltered_buffered_lines",
            "shell_cause",
            "wip_authors",
            "redacted_credentials",
            "unfiltered_output_skipped",
        ] {
            assert!(keys.contains_key(expected), "missing {expected}: {keys:?}");
        }
        assert!(
            !keys.contains_key("empty_test_selection"),
            "an absent diagnostic is not a key"
        );

        let base = json!({"exit_code": 0, "stdout": "x"});
        let mut with = base.clone();
        with.as_object_mut().unwrap().extend(keys.clone());
        assert_eq!(
            with.to_string().len() - base.to_string().len(),
            extras_len(&keys, false),
            "the gate's extras are not what the keys cost"
        );

        assert_eq!(extras_len(&serde_json::Map::new(), false), 0);
        assert_eq!(
            extras_len(&serde_json::Map::new(), true),
            json!({"exit_code": 0, "stdout": ""}).to_string().len()
                - json!({"exit_code": 0}).to_string().len(),
            "an explicit empty stdout costs what the key costs"
        );
    }

    #[test]
    fn a_redaction_count_of_zero_adds_no_key() {
        let keys = attachments(&[], &None, 0, &None, &serde_json::Map::new());
        assert!(keys.is_empty(), "{keys:?}");
    }

    #[test]
    fn attach_adds_an_empty_stdout_only_to_a_tee_capture_that_has_none() {
        let mut no_stdout = json!({"exit_code": 0});
        attach(&mut no_stdout, serde_json::Map::new(), true);
        assert_eq!(no_stdout["stdout"], "");
        let mut has_stdout = json!({"exit_code": 0, "stdout": "kept"});
        attach(&mut has_stdout, serde_json::Map::new(), true);
        assert_eq!(has_stdout["stdout"], "kept");
        let mut no_tee = json!({"exit_code": 0});
        attach(&mut no_tee, serde_json::Map::new(), false);
        assert!(no_tee.get("stdout").is_none());
    }

    /// A response built to `inline_stdout_room` lands exactly ON the limit and one byte more is
    /// over it, at the COMPACT length `call_content` measures.
    #[test]
    fn the_stdout_room_lands_the_response_exactly_on_the_limit() {
        let skeleton = json!({"exit_code": 0, "truncated": true, "hint": "h", "stderr": "e\n"});
        let room = inline_stdout_room(&skeleton);
        let build = |n: usize| {
            let mut r = skeleton.clone();
            r["stdout"] = json!("a".repeat(n));
            r.to_string().len()
        };
        assert_eq!(build(room), crate::tools::INLINE_MAX_RESPONSE_LEN);
        assert!(!crate::tools::exceeds_inline_limit_len(build(room)));
        assert!(crate::tools::exceeds_inline_limit_len(build(room + 1)));
    }
    /// `inline_stdout_room` measures with `response_room`, which strips a cut record. That equals the
    /// skeleton's compact length, the measure it had before, only because the skeleton never
    /// carries one: the summary arm builds it from fixed keys plus `late`, which `attachments`
    /// alone builds, and the one record `handle_successful_output_with` writes (`wip_authors`) goes
    /// on the RESULT after every arm has measured. Pinned at the producer, with every key
    /// `attachments` can emit present and a `wip_authors` diagnostic that WAS cut at its source.
    #[test]
    fn the_stdout_room_skeleton_never_carries_a_cut_record() {
        let raw_wip = "w".repeat(WIP_AUTHORS_BYTE_BUDGET * 3);
        let cut_wip = bound_wip_authors(raw_wip.clone());
        assert_ne!(cut_wip, raw_wip, "fixture: the diagnostic must be cut");
        let late = attachments(
            &[
                ("shell_cause", &Some("cause".to_string())),
                ("wip_authors", &Some(cut_wip)),
                ("empty_test_selection", &Some("empty".to_string())),
                ("partial_test_selection", &Some("partial".to_string())),
            ],
            &Some((
                "@cmd_0bf0a111".to_string(),
                Some(crate::tools::output_buffer::Truncation {
                    kept_lines: 1,
                    total_lines: 2,
                }),
                2,
                Vec::new(),
            )),
            1,
            &Some("skipped".to_string()),
            &serde_json::Map::from_iter([
                ("buffer_truncated".to_string(), json!(["@cmd_0bf0a222"])),
                (
                    "jobs".to_string(),
                    json!([{"handle": "@bg_1", "state": "done", "command": "x"}]),
                ),
                ("timeout_hint".to_string(), json!("hint")),
            ]),
        );
        assert!(
            !late.contains_key(crate::tools::CUT_FIELDS_KEY),
            "late keys carry a cut record: {late:?}"
        );
        // The summary arm's skeleton, every optional key present.
        let mut skeleton = json!({
            "exit_code": 1,
            "truncated": true,
            "stdout_shown": 9,
            "stdout_total": 9,
            "hint": "h",
            "stderr": "e",
            "stderr_shown": 1,
            "stderr_total": 2,
        });
        for (key, value) in &late {
            skeleton[key.as_str()] = value.clone();
        }
        assert!(skeleton.get(crate::tools::CUT_FIELDS_KEY).is_none());
        assert_eq!(
            crate::tools::delivered_len(&skeleton),
            skeleton.to_string().len()
        );
        assert_eq!(
            inline_stdout_room(&skeleton),
            crate::tools::INLINE_MAX_RESPONSE_LEN
                - skeleton.to_string().len()
                - EMPTY_STDOUT_KEY.len(),
            "the room is the pre-migration formula"
        );
    }
    /// A cut record names fields whose text the SUMMARIZER cut. `compacted_test_value` removes the
    /// summarizer's `failures` and replaces its `stderr` with compacted text it never cut, so after
    /// compaction the record names neither (and, naming nothing else, is gone): a stale `stderr`
    /// would spare the compacted stderr from the backstop and be counted by the gate.
    #[test]
    fn a_compacted_value_never_keeps_a_record_naming_a_field_it_replaced() {
        let mut stdout = String::from("\nrunning 150 tests\n");
        for i in 0..150 {
            stdout.push_str(&format!("test tests::passes_{i:03} ... ok\n"));
        }
        stdout.push_str(
            "\ntest result: ok. 150 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; \
             finished in 0.00s\n\n",
        );
        let stderr: String = (0..25)
            .map(|i| format!("warning: unused variable `x{i}`\n"))
            .collect();
        let c = crate::tools::libtest_compact::compact_libtest_output(&stdout, &stderr)
            .expect("fixture compacts");
        let key = crate::tools::CUT_FIELDS_KEY;

        // `stderr`: 25 lines, over the summarizer's 20-line budget.
        let summary = crate::tools::command_summary::summarize_test_output(&stdout, &stderr, 0);
        assert_eq!(summary[key], json!(["stderr"]), "fixture: {summary}");
        let v = compacted_test_value(&c, &stdout, &stderr, 0, MEASURING_OUTPUT_ID);
        assert!(v.get(key).is_none(), "stale stderr record: {v}");

        // `failures`: a section over its 5,000 B budget, and no stderr to record.
        let red = format!("{stdout}failures:\n{}\n", "z".repeat(6_000));
        let summary = crate::tools::command_summary::summarize_test_output(&red, "", 101);
        assert_eq!(summary[key], json!(["failures"]), "fixture: {summary}");
        let v = compacted_test_value(&c, &red, "", 101, MEASURING_OUTPUT_ID);
        assert!(v.get(key).is_none(), "stale failures record: {v}");
    }

    /// `INLINE_MAX_RESPONSE_LEN` is the same edge `exceeds_inline_limit_len` draws.
    #[test]
    fn the_named_limit_is_the_edge_the_predicate_draws() {
        let edge = crate::tools::INLINE_MAX_RESPONSE_LEN;
        assert!(!crate::tools::exceeds_inline_limit_len(edge));
        assert!(crate::tools::exceeds_inline_limit_len(edge + 1));
        assert!(!crate::tools::exceeds_inline_limit(&"a".repeat(edge)));
        assert!(crate::tools::exceeds_inline_limit(&"a".repeat(edge + 1)));
        assert_eq!(edge, 10_003);
    }

    /// The marker's remedy must be TRUE of where the text came from: a stored stderr names the
    /// `.err` handle; the query's own stderr, stored nowhere, must not.
    #[test]
    fn a_stderr_marker_promises_only_what_its_source_can_deliver() {
        let wide = "e".repeat(5_000);
        let (stored, ..) = bound_buffer_stderr(&wide, 20, "grep -c a @cmd_0bf0a111", false);
        assert!(
            stored.contains("all of it: @cmd_0bf0a111.err"),
            "{stored:.0}"
        );
        let (own, ..) = bound_buffer_stderr(&wide, 20, "awk 1 @cmd_0bf0a111", true);
        assert!(own.contains("stored nowhere"), "{own:.0}");
        assert!(
            !own.contains(".err"),
            "the query's own stderr is not behind an `.err` handle: {own:.0}"
        );
        assert!(own.contains("2>&1 >/dev/null | cut -c1-4000"));
    }

    /// Control characters serialize to 6 bytes, so the stderr bound is in ESCAPED bytes: 50,000
    /// `\x01` must come back inside the budget, marker included.
    #[test]
    fn a_control_character_stderr_is_bounded_in_escaped_bytes() {
        let text = "\u{1}".repeat(50_000);
        let (cut, ..) = bound_buffer_stderr(&text, 20, "grep -c a @cmd_0bf0a111", false);
        let escaped = crate::util::text::json_escaped_len(&cut);
        assert!(escaped <= BUFFER_STDERR_BYTE_BUDGET, "{escaped} B escaped");
        assert!(
            escaped + 20 > BUFFER_STDERR_BYTE_BUDGET,
            "{escaped} B of {BUFFER_STDERR_BYTE_BUDGET}: the cut kept far less than fits"
        );
    }

    /// A stderr cut by the LINE cap alone carries the remedy its source can deliver, in the same
    /// frame as the byte cut's; one that fits whole carries nothing.
    #[test]
    fn a_line_cut_stderr_carries_the_remedy_its_source_can_deliver() {
        let text: String = (1..=30).map(|i| format!("{i}\n")).collect();
        let (own, shown, total) = bound_buffer_stderr(&text, 20, "cat @cmd_0bf0a111 >&2", true);
        assert_eq!((shown, total), (20, 30));
        assert!(own.starts_with("1\n2\n"), "{own:.0}");
        assert!(
            own.contains("\n--- stderr: 20 of 30 lines shown; "),
            "{own:.200}"
        );
        assert!(own.ends_with(" ---"), "{own:.0}");
        assert!(own.contains("( QUERY; ) 2>&1 >/dev/null | cut -c1-4000"));

        // A shape the wrap cannot take: advice, not a route.
        let (list, ..) = bound_buffer_stderr(&text, 20, "echo 1; cat @cmd_0bf0a111 >&2", true);
        assert!(list.contains("narrow the query"), "{list:.0}");
        assert!(!list.contains("QUERY"), "{list:.0}");

        let (stored, ..) = bound_buffer_stderr(&text, 20, "grep -c a @cmd_0bf0a111", false);
        assert!(
            stored.contains("20 of 30 lines shown; all of it: @cmd_0bf0a111.err ---"),
            "{stored:.0}"
        );

        // Whole, at the edge and below: the text itself.
        let twenty: String = (1..=20).map(|i| format!("{i}\n")).collect();
        let (whole, shown, total) = bound_buffer_stderr(&twenty, 20, "cat @cmd_0bf0a111 >&2", true);
        assert_eq!((whole.as_str(), shown, total), (twenty.as_str(), 20, 20));
    }

    /// `count_newline_lines` is what `str::lines().count()` and `sed -n '$='` say, over the shapes
    /// a stream ends in.
    #[test]
    fn count_newline_lines_is_the_line_count_sed_and_str_lines_give() {
        for text in [
            "",
            "a",
            "a\n",
            "\n",
            "\n\n",
            "a\n\n",
            "a\nb",
            "a\nb\n",
            "\n\na",
            "a\r\nb\r\n",
            "a\rb",
            "é\nü",
        ] {
            assert_eq!(
                count_newline_lines(text.as_bytes()),
                crate::tools::command_summary::count_lines(text),
                "{text:?}"
            );
        }
        // The shell's own count, on real bytes.
        #[cfg(unix)]
        {
            let dir = tempfile::tempdir().unwrap();
            for text in ["a", "a\n", "\n", "a\n\nb", "a\nb\n", "x\ny\nz"] {
                let path = dir.path().join("t.txt");
                std::fs::write(&path, text).unwrap();
                let out = std::process::Command::new("sed")
                    .args(["-n", "$="])
                    .arg(&path)
                    .output()
                    .unwrap();
                let sed: usize = String::from_utf8(out.stdout)
                    .unwrap()
                    .trim()
                    .parse()
                    .unwrap_or(0);
                assert_eq!(count_newline_lines(text.as_bytes()), sed, "{text:?}");
            }
        }
    }

    /// A file is counted the same whatever the chunk it is read in: a newline that ends a chunk, a
    /// last line that starts in one and ends in the next, a last line with no newline in a chunk
    /// of its own, and bytes that are not UTF-8 (counted as the lossy decode counts them).
    #[test]
    fn count_file_lines_does_not_depend_on_the_chunk_size() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log");
        let cases: [&[u8]; 6] = [
            b"",
            b"a",
            b"a\n",
            b"ab\ncd\n\nef",
            b"a\xff\nb\xc3\n\xe2\x82",
            b"\n\n\n\n\n\n\n\n\n\n",
        ];
        for bytes in cases {
            std::fs::write(&path, bytes).unwrap();
            let want = crate::tools::command_summary::count_lines(&String::from_utf8_lossy(bytes));
            assert_eq!(count_newline_lines(bytes), want, "{bytes:?}");
            for chunk in [0usize, 1, 2, 3, 4, 5, 7, 1 << 20] {
                assert_eq!(
                    count_file_lines(&path, chunk),
                    Some(want),
                    "{bytes:?} in chunks of {chunk}"
                );
            }
        }
        assert_eq!(count_file_lines(&dir.path().join("missing"), 4), None);
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
    /// `wip_authors` is bounded in the unit the limit counts. A raw 3,000 B cut of a diagnostic
    /// holding control characters (a path with one in it, a tab-heavy porcelain line) serializes to
    /// many times that, so the field alone pushed the response over the limit.
    #[test]
    fn wip_authors_is_bounded_in_escaped_bytes() {
        let text = "\u{1}".repeat(20_000);
        let cut = bound_wip_authors(text);
        let escaped = crate::util::text::json_escaped_len(&cut);
        assert!(
            escaped <= WIP_AUTHORS_BYTE_BUDGET,
            "{escaped} B escaped of {WIP_AUTHORS_BYTE_BUDGET}"
        );
        assert!(cut.contains("of 20000 bytes shown"), "{cut:.0}");
    }
}
