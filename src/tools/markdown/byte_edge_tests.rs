//! Byte-edge probes for `read_markdown`, driven through the REAL `ReadFile::call_content`.
//!
//! The inline limit is measured on the COMPACT SERIALIZED response, and JSON escaping makes a
//! raw byte cost 1 (ASCII), 2 (`"`, `\`, newline) or 6 (`\x01`) bytes, while a multibyte
//! character costs its UTF-8 width. A builder that decides on raw bytes, or on part of the
//! response, returns something `call_content` then buffers again under `@tool_*`: a second
//! handle (ADR 2026-10-05) or, where the tool gave none, a handle for content it believed was
//! inline. Each sweep below crosses the 10,003 B edge in every content class and asserts, per
//! case: no `@tool_*`, at most one handle, a response with no handle fits, and every
//! `read_file(...)` route the response names runs.

use crate::agent::Agent;
use crate::lsp::LspManager;
use crate::tools::read_file::ReadFile;
use crate::tools::{RecoverableError, Tool};
use crate::util::text::json_escaped_len;
use serde_json::{json, Value};

async fn ctx() -> crate::tools::ToolContext {
    crate::tools::ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: LspManager::new_arc(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(50)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(
            crate::tools::guide_ledger::GuideLedger::mid_session(),
        )),
        workspace_override: None,
    }
}

/// Content classes and the serialized cost of one unit of each.
const CLASSES: [(&str, &str); 6] = [
    ("ascii", "a"),
    ("quote", "\""),
    ("backslash", "\\"),
    ("control", "\u{1}"),
    ("euro", "\u{20ac}"),
    ("emoji", "\u{1F600}"),
];

/// Payload sizes, in serialized bytes of the payload alone: the response around it adds a
/// little, so the RESPONSE sweeps across the 10,003 B edge.
fn sweep() -> impl Iterator<Item = usize> {
    (9_000..=10_500).step_by(50)
}

/// Lines of `unit` (40 per line) whose JSON-escaped length is `escaped` bytes, to within one
/// unit. No line starts with `#`, so the payload adds no heading.
fn payload(unit: &str, escaped: usize) -> String {
    let line = unit.repeat(40);
    let per_line = json_escaped_len(&line) + 2;
    let full = escaped / per_line;
    let rest = (escaped - full * per_line) / json_escaped_len(unit);
    let mut lines = vec![line; full];
    if rest > 0 {
        lines.push(unit.repeat(rest));
    }
    lines.join("\n")
}

/// Every distinct buffer handle named in `text`.
fn handles_in(text: &str) -> std::collections::BTreeSet<String> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '@' || c == '_'))
        .filter(|t| {
            ["@file_", "@tool_", "@cmd_", "@bg_"]
                .iter()
                .any(|p| t.starts_with(p))
        })
        .map(str::to_owned)
        .collect()
}

/// What the caller is shown, as text to scan, and the compact size the limit is judged on.
/// An `Err` is put inline by the server (`error`, `hint`, then `extra`), so its body is what is
/// measured; its text lists the values raw so routes in it can be read back.
async fn deliver(ctx: &crate::tools::ToolContext, input: &Value) -> (String, usize) {
    match ReadFile.call_content(input.clone(), ctx).await {
        Ok(blocks) => {
            let text = blocks[0]
                .as_text()
                .map(|t| t.text.clone())
                .unwrap_or_default();
            let compact = ReadFile
                .call(input.clone(), &self::ctx().await)
                .await
                .map(|v| v.to_string().len())
                .unwrap_or(0);
            (text, compact)
        }
        Err(e) => {
            let rec = e
                .downcast_ref::<RecoverableError>()
                .unwrap_or_else(|| panic!("not a RecoverableError: {e}"));
            let mut body = serde_json::Map::new();
            body.insert("error".into(), json!(rec.message));
            body.insert("hint".into(), json!(rec.hint().unwrap_or_default()));
            let mut text = format!("{}\n{}\n", rec.message, rec.hint().unwrap_or_default());
            for (k, v) in rec.extra.iter() {
                body.insert(k.clone(), v.clone());
                match v {
                    Value::String(s) => text.push_str(s),
                    Value::Array(a) => {
                        for item in a {
                            match item {
                                Value::String(s) => text.push_str(s),
                                other => text.push_str(&other.to_string()),
                            }
                            text.push('\n');
                        }
                    }
                    other => text.push_str(&other.to_string()),
                }
                text.push('\n');
            }
            let compact = Value::Object(body).to_string().len();
            (text, compact)
        }
    }
}

/// Every `read_file(...)` call a response names, as tool inputs: line ranges, and headings
/// quoted as JSON strings (a heading that is not valid JSON is a route nobody can paste back).
fn routes_in(text: &str) -> Vec<Value> {
    let ranges =
        regex::Regex::new(r#"read_file\((?:path=)?"([^"]+)", start_line=(\d+), end_line=(\d+)\)"#)
            .unwrap();
    let headings =
        regex::Regex::new(r#"read_file\("(@file_[0-9a-f]+)", heading=("(?:[^"\\]|\\.)*")\)"#)
            .unwrap();
    let mut out = Vec::new();
    for c in ranges.captures_iter(text) {
        out.push(json!({
            "path": &c[1],
            "start_line": c[2].parse::<u64>().unwrap(),
            "end_line": c[3].parse::<u64>().unwrap(),
        }));
    }
    for c in headings.captures_iter(text) {
        let heading: String = serde_json::from_str(&c[2])
            .unwrap_or_else(|e| panic!("heading route {:?} is not a JSON string: {e}", &c[2]));
        out.push(json!({ "path": &c[1], "heading": heading }));
    }
    out
}

/// The per-case contract. Returns the compact size judged.
async fn assert_one_handle_and_fits(input: Value, label: &str) -> usize {
    let ctx = ctx().await;
    let (text, compact) = deliver(&ctx, &input).await;
    assert!(
        !text.contains("@tool_"),
        "{label}: a second handle was minted for a {compact} B response: {:.300}",
        text
    );
    let handles = handles_in(&text);
    assert!(handles.len() <= 1, "{label}: handles {handles:?}");
    assert!(
        !crate::tools::exceeds_inline_limit_len(compact),
        "{label}: {compact} B is over the inline limit"
    );
    for route in routes_in(&text) {
        // A route resolves relative to the handle or the real file the response named.
        let (followed, _) = deliver(&ctx, &route).await;
        assert!(
            !followed.contains("@tool_"),
            "{label}: route {route} itself minted a second handle: {followed:.400}"
        );
        assert!(
            ReadFile.call(route.clone(), &ctx).await.is_ok()
                || followed.contains("exceeds inline threshold"),
            "{label}: route {route} does not work: {followed:.300}"
        );
    }
    compact
}

/// Gap (a): one section, read by heading, whose body crosses the edge. The success arm was
/// chosen on the section's RAW bytes, so 6,000 quotes (12 KB serialized) were returned as a
/// success and buffered under `@tool_*`.
#[tokio::test]
async fn a_single_heading_section_at_the_byte_edge_keeps_one_handle() {
    let dir = tempfile::tempdir().unwrap();
    let mut largest_inline = 0;
    for (class, unit) in CLASSES {
        for size in sweep().chain([12_000]) {
            let path = dir.path().join(format!("sec-{class}-{size}.md"));
            let body = format!(
                "# Top\n\nintro\n\n## Sec\n{}\n\n## Other\n\nx\n",
                payload(unit, size)
            );
            std::fs::write(&path, &body).unwrap();
            let input = json!({ "path": path.to_str().unwrap(), "heading": "## Sec" });
            let compact = assert_one_handle_and_fits(input, &format!("{class}/{size}")).await;
            largest_inline = largest_inline.max(compact);
        }
    }
    eprintln!("single heading: largest response judged = {largest_inline} B");
}

/// Gap (b): a whole read whose body crosses the edge, below and above the summary threshold.
#[tokio::test]
async fn a_whole_read_at_the_byte_edge_keeps_one_handle() {
    let dir = tempfile::tempdir().unwrap();
    let mut largest = 0;
    for (class, unit) in CLASSES {
        for size in sweep().chain([18_000, 30_000]) {
            let path = dir.path().join(format!("whole-{class}-{size}.md"));
            let body = format!("# Top\n\n## A\n{}\n\n## B\n\nx\n", payload(unit, size));
            std::fs::write(&path, &body).unwrap();
            let input = json!({ "path": path.to_str().unwrap() });
            let compact = assert_one_handle_and_fits(input, &format!("{class}/{size}")).await;
            largest = largest.max(compact);
        }
    }
    eprintln!("whole read: largest response judged = {largest} B");
}

/// A file of `## NNNN <unit x 20>` headings whose list serializes to about `escaped` bytes.
fn many_headings(unit: &str, escaped: usize) -> String {
    let mut s = String::new();
    let mut used = 0;
    let mut i = 0;
    while used < escaped {
        i += 1;
        let h = format!("## {i:04} {}", unit.repeat(20));
        used += json_escaped_len(&h) + 16; // {"h":"..","l":N},
        s.push_str(&h);
        s.push_str("\n\n");
    }
    s
}

/// Gap (c): the heading-not-found list. It listed every heading with no bound (27.6 to 32.7 KB
/// measured), and `call_content` buffered it under `@tool_*`.
#[tokio::test]
async fn a_heading_not_found_list_at_the_byte_edge_keeps_one_handle() {
    let dir = tempfile::tempdir().unwrap();
    let mut cut_seen = false;
    for (class, unit) in CLASSES {
        for size in sweep().chain([30_000]) {
            let path = dir.path().join(format!("nf-{class}-{size}.md"));
            std::fs::write(&path, many_headings(unit, size)).unwrap();
            let input = json!({ "path": path.to_str().unwrap(), "heading": "## Absent" });
            let ctx = ctx().await;
            let (text, _) = deliver(&ctx, &input).await;
            cut_seen |= text.contains("entries omitted");
            assert_one_handle_and_fits(input, &format!("{class}/{size}")).await;
        }
    }
    assert!(
        cut_seen,
        "a list over the limit must be cut with an `entries omitted` marker"
    );
}

/// Gap 2: `HEADING_ECHO_CLIP` is a cap on what the echo COSTS in the response, so it counts
/// serialized bytes. In raw bytes a 200-byte clip of `\x01` serialized to 1,200 B in each of the
/// map, the breadcrumb and `next_actions`.
#[tokio::test]
async fn a_control_char_heading_echo_is_clipped_in_escaped_bytes() {
    let dir = tempfile::tempdir().unwrap();
    for (class, unit) in CLASSES.into_iter().chain([("esc", "\u{1b}")]) {
        let path = dir.path().join(format!("echo-{class}.md"));
        // 1,000 units: over the clip in every class, yet one line still fits a range read once
        // escaped. A wider line is clamped by whichever arm serves the range: a `.md` path, and
        // the `@file_` a WHOLE markdown file is stored under, go to `read_markdown_line_range`
        // (`a_markdown_range_over_a_wide_line_keeps_one_handle`); a section's `@file_` is an
        // excerpt with no source path, so its ranges go to `read_from_buffer`
        // (`an_oversized_section_route_reaches_the_end_of_the_section`).
        let sub = unit.repeat(1_000);
        let mut body = format!("## Big {sub}\n### {sub}\nfirst body\n\n");
        for i in 2..=40 {
            body.push_str(&format!("### Sub {i:03}\n{}\n\n", "b".repeat(300)));
        }
        std::fs::write(&path, &body).unwrap();
        let input = json!({ "path": path.to_str().unwrap(), "heading": "## Big" });
        let ctx = ctx().await;
        let err = ReadFile.call(input.clone(), &ctx).await.unwrap_err();
        let rec = err
            .downcast_ref::<RecoverableError>()
            .expect("an oversized section is a RecoverableError");
        let first = &rec.extra["section_map"][0];
        let echoed = serde_json::to_string(&first["h"]).unwrap();
        assert!(
            echoed.len() <= 200 + 2,
            "{class}: the map echoes {} serialized bytes",
            echoed.len()
        );
        assert!(
            first["h_bytes"].as_u64().unwrap() as usize >= sub.len(),
            "{class}: a clipped echo carries its true length as h_bytes: {first}"
        );
        for crumb in rec.extra["breadcrumb"].as_array().unwrap() {
            let n = serde_json::to_string(crumb).unwrap().len();
            assert!(n <= 200 + 2, "{class}: a breadcrumb echoes {n} B");
        }
        // An echo quoted inside another string is escaped twice (`"` -> `\"` -> `\\\"`). The cap
        // bounds what is DELIVERED, so the quoted heading costs at most 200 B in the response
        // beside the call around it: measured before, `"` cost 398 B there and `\` 398 B.
        let action = rec.extra["next_actions"][0].as_str().unwrap();
        let fid = rec.extra["file_id"].as_str().unwrap();
        let around = json_escaped_len(&format!("read_file(\"{fid}\", heading=\"\")"));
        let echo = json_escaped_len(action) - around;
        assert!(
            echo <= 200,
            "{class}: next_actions[0] echoes {echo} B: {action:.120}"
        );
        let routed: String =
            serde_json::from_str(&action[action.find("heading=").unwrap() + 8..action.len() - 1])
                .unwrap();
        assert!(
            format!("### {sub}").starts_with(&routed),
            "{class}: the route names no prefix of the heading"
        );
        let lines = rec.message.split(" spans ").nth(1).unwrap();
        let around = json_escaped_len(&format!("section \"\" spans {lines}"));
        let echo = json_escaped_len(&rec.message) - around;
        assert!(
            echo <= 200,
            "{class}: the message echoes {echo} B: {:.120}",
            rec.message
        );
        assert_one_handle_and_fits(input, class).await;
    }
}

/// `n` sections `## Section NNNN <40 x 'w'>`, each with one short body line: a file whose
/// heading list (and so `coverage.unread` and `siblings`) is far over the limit.
fn many_sections(n: usize) -> String {
    (1..=n)
        .map(|i| format!("## Section {i:04} {}\nbody {i}\n\n", "w".repeat(40)))
        .collect()
}

/// `coverage` and `siblings` echo every other heading and have no length of their own. A short
/// section must come back as a success without them, each marked `<key>_omitted`, not be
/// refused as oversized, and not be buffered under `@tool_*`.
#[tokio::test]
async fn a_short_section_of_a_many_heading_file_drops_coverage_and_siblings() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("many.md");
    std::fs::write(&path, many_sections(600)).unwrap();
    let input = json!({ "path": path.to_str().unwrap(), "heading": "## Section 0300" });
    let ctx = ctx().await;
    let result = ReadFile
        .call(input.clone(), &ctx)
        .await
        .expect("a short section is a success");
    assert!(
        result["content"].as_str().unwrap().contains("body 300"),
        "{result:.300}"
    );
    assert_eq!(result["coverage_omitted"], json!(true), "{result:.300}");
    assert_eq!(result["siblings_omitted"], json!(true), "{result:.300}");
    assert!(!crate::tools::exceeds_inline_limit(&result.to_string()));
    assert_one_handle_and_fits(input, "section of 600").await;
}

/// The line-range arm: a short range keeps its content inline and drops `coverage`; a long
/// range is buffered with a bounded chunk and also drops `coverage`. Both with one handle at
/// most and within the limit.
#[tokio::test]
async fn a_line_range_of_a_many_heading_file_drops_coverage_to_fit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("many.md");
    std::fs::write(&path, many_sections(600)).unwrap();
    let p = path.to_str().unwrap();
    let ctx = ctx().await;

    let short = json!({ "path": p, "start_line": 4, "end_line": 6 });
    let result = ReadFile.call(short.clone(), &ctx).await.unwrap();
    assert!(result.get("file_id").is_none(), "{result:.300}");
    assert_eq!(result["coverage_omitted"], json!(true), "{result:.300}");
    assert_one_handle_and_fits(short, "short range").await;

    let long = json!({ "path": p, "start_line": 1, "end_line": 900 });
    let result = ReadFile.call(long.clone(), &ctx).await.unwrap();
    assert!(result.get("file_id").is_some(), "{result:.300}");
    assert_eq!(result["coverage_omitted"], json!(true), "{result:.300}");
    assert_one_handle_and_fits(long, "long range").await;
}

/// The six classes above and ESC (`\x1b`, six bytes escaped like `\x01`).
fn classes7() -> impl Iterator<Item = (&'static str, &'static str)> {
    CLASSES.into_iter().chain([("esc", "\u{1b}")])
}

/// One line of `unit` whose JSON-escaped length is about `escaped` bytes.
fn one_line(unit: &str, escaped: usize) -> String {
    unit.repeat((escaped / json_escaped_len(unit)).max(1))
}

/// The per-page contract of a line-range read through the REAL `call_content`: no `@tool_*`,
/// at most one handle minted counting `file_id` (the handle the input itself names is not
/// minted by this page), the compact response fits, and a truncated line names a route off it.
async fn md_page(ctx: &crate::tools::ToolContext, input: &Value, label: &str) -> (Value, usize) {
    let v = ReadFile
        .call(input.clone(), ctx)
        .await
        .unwrap_or_else(|e| panic!("{label}: {input} failed: {e}"));
    let compact = v.to_string().len();
    let blocks = ReadFile.call_content(input.clone(), ctx).await.unwrap();
    let text = crate::tools::hint_probe::primary_text(&blocks);
    let mut minted = handles_in(&text);
    if let Some(p) = input["path"].as_str() {
        minted.remove(p);
    }
    // A prose mention ("a @tool_* ref") is not a handle.
    minted.retain(|h| !["@file_", "@tool_", "@cmd_", "@bg_"].contains(&h.as_str()));
    assert!(
        !minted.iter().any(|h| h.starts_with("@tool_")),
        "{label}: a second handle was minted for a {compact} B response ({minted:?}): {:.300}",
        text
    );
    assert!(minted.len() <= 1, "{label}: handles minted {minted:?}");
    assert!(
        !crate::tools::exceeds_inline_limit_len(compact),
        "{label}: {compact} B is over the inline limit"
    );
    if v["line_truncated"] == json!(true) {
        assert!(
            v["hint"].as_str().is_some_and(|h| h.contains("grep -o")),
            "{label}: a truncated line names no route off it: {:?}",
            v["hint"]
        );
    }
    (v, compact)
}

/// Follow `next` from `input` until the read completes. Returns the largest compact response
/// and the last line the chain delivered. A `force=true` read must stay in the raw range arm
/// (no `format` key) on every page, so each `next` it names must carry `force=true` too.
async fn md_read_through(
    ctx: &crate::tools::ToolContext,
    mut input: Value,
    label: &str,
) -> (usize, u64) {
    let route = regex::Regex::new(
        r#"^read_file\("([^"]+)", start_line=(\d+), end_line=(\d+)(, force=true)?\)$"#,
    )
    .unwrap();
    let forced = input["force"] == json!(true);
    let path = input["path"].as_str().unwrap().to_string();
    let mut largest = 0;
    let mut prev_start = 0u64;
    for _ in 0..400 {
        let (v, compact) = md_page(ctx, &input, label).await;
        largest = largest.max(compact);
        if forced {
            assert!(
                v.get("format").is_none(),
                "{label}: a force=true read left the raw range arm: {v:.300}"
            );
        }
        let Some(next) = v["next"].as_str() else {
            let last = v["shown_lines"][1]
                .as_u64()
                .unwrap_or_else(|| input["end_line"].as_u64().unwrap());
            return (largest, last);
        };
        let c = route
            .captures(next)
            .unwrap_or_else(|| panic!("{label}: next {next:?} is not a line route"));
        assert_eq!(&c[1], path, "{label}: next leaves the path");
        if forced {
            assert!(
                c.get(4).is_some(),
                "{label}: next {next:?} drops force=true"
            );
        }
        let start: u64 = c[2].parse().unwrap();
        let shown_end = v["shown_lines"][1].as_u64().unwrap();
        assert_eq!(
            start,
            shown_end + 1,
            "{label}: next does not resume after the page"
        );
        assert!(start > prev_start, "{label}: next does not advance");
        prev_start = start;
        input =
            json!({ "path": path, "start_line": start, "end_line": c[3].parse::<u64>().unwrap() });
        if c.get(4).is_some() {
            input["force"] = json!(true);
        }
    }
    panic!("{label}: next chain did not terminate");
}

/// The buffered arm of `read_markdown_line_range` sized its page with a raw budget and a fixed
/// reserve for its other keys, and never clamped. One line wider than the page came back whole:
/// measured before the fix, a `.md` file of ONE 9,990 B line read 1..100000 was 10,103 B, a
/// 60,000 B line 60,113 B, both buffered by `call_content` as `@tool_*` beside the `file_id`.
/// The `force=true` twin (`read_with_line_range`) dropped `force` from its `next`, so following
/// it left the raw arm for this one. A long path widens `next`, and `coverage` rides along when
/// headings stay unread.
#[tokio::test]
async fn a_markdown_range_over_a_wide_line_keeps_one_handle() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    // About 1,210 B of path, so `next` is far wider than a fixed reserve allowed for.
    let mut deep = dir.path().to_path_buf();
    for i in 0..6 {
        deep.push(format!("{i}{}", "d".repeat(199)));
    }
    std::fs::create_dir_all(&deep).unwrap();
    let mut largest = 0;
    for (class, unit) in classes7() {
        let bodies = sweep()
            .chain((9_950..=10_010).step_by(3))
            .map(|s| (format!("lines{s}"), payload(unit, s)))
            .chain(
                [9_969, 9_972, 9_990, 12_000, 60_000]
                    .map(|s| (format!("one{s}"), one_line(unit, s))),
            );
        for (shape, body) in bodies {
            for tail in ["", "\nz\n"] {
                let p = dir
                    .path()
                    .join(format!("{class}-{shape}-{}.md", tail.len()));
                std::fs::write(&p, format!("{body}{tail}")).unwrap();
                let path = p.to_str().unwrap();
                for force in [false, true] {
                    let mut input = json!({ "path": path, "start_line": 1, "end_line": 100_000 });
                    if force {
                        input["force"] = json!(true);
                    }
                    let label = format!("md range {class}/{shape}/{}/force={force}", tail.len());
                    largest = largest.max(md_read_through(&ctx, input, &label).await.0);
                }
            }
        }
        for (shape, body) in [
            ("one12k", one_line(unit, 12_000)),
            ("lines9990", payload(unit, 9_990)),
        ] {
            let p = deep.join(format!("{class}-{shape}.md"));
            std::fs::write(&p, format!("{body}\nz\n")).unwrap();
            let input =
                json!({ "path": p.to_str().unwrap(), "start_line": 1, "end_line": 100_000 });
            let label = format!("md long path {class}/{shape}");
            largest = largest.max(md_read_through(&ctx, input, &label).await.0);
        }
        // `coverage` present: the range leaves both headings unread.
        for s in [9_990, 12_000] {
            let p = dir.path().join(format!("cov-{class}-{s}.md"));
            let body = one_line(unit, s);
            std::fs::write(&p, format!("# Top\n{body}\nz\n## Later\n")).unwrap();
            // Coverage is reported only for a file with a heading already read.
            let prime = json!({ "path": p.to_str().unwrap(), "start_line": 1, "end_line": 1 });
            ReadFile.call(prime, &ctx).await.unwrap();
            let input = json!({ "path": p.to_str().unwrap(), "start_line": 2, "end_line": 3 });
            let label = format!("md coverage {class}/{s}");
            let first = ReadFile.call(input.clone(), &ctx).await.unwrap();
            assert!(
                first.get("coverage").is_some() || first.get("coverage_omitted").is_some(),
                "{label}: no coverage to count: {first:.300}"
            );
            largest = largest.max(md_read_through(&ctx, input, &label).await.0);
            // The `@file_` a whole read of a markdown file mints is markdown-sourced, so its
            // ranges take this arm too.
            let whole = ReadFile
                .call(json!({ "path": p.to_str().unwrap() }), &ctx)
                .await
                .unwrap();
            let fid = whole["file_id"]
                .as_str()
                .unwrap_or_else(|| panic!("{label}: a whole read minted no file_id: {whole:.300}"));
            assert!(
                crate::tools::markdown::is_markdown_target(fid, &ctx),
                "{label}: {fid}"
            );
            let input = json!({ "path": fid, "start_line": 1, "end_line": 100_000 });
            let label = format!("md whole-file handle {class}/{s}");
            largest = largest.max(md_read_through(&ctx, input, &label).await.0);
        }
    }
    eprintln!("md range: largest response judged = {largest} B");
}

/// An oversized single-heading section names a range route in `next_actions`. It was sized to
/// the lines that fit inline, so when the section's SECOND line was wide the route was
/// `end_line=1`: it returned the `## S` line alone, with no `next`, and led nowhere. Followed
/// with its `next` chain, the route must reach the section's last line.
#[tokio::test]
async fn an_oversized_section_route_reaches_the_end_of_the_section() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let range =
        regex::Regex::new(r#"read_file\("(@file_[0-9a-f]+)", start_line=1, end_line=(\d+)\)"#)
            .unwrap();
    let spans = regex::Regex::new(r"spans (\d+) lines").unwrap();
    for (class, unit) in classes7() {
        for wide in [12_000, 60_000] {
            let p = dir.path().join(format!("sec-{class}-{wide}.md"));
            let body = format!("## S\n{}\n{}\n", one_line(unit, wide), payload("b", 3_000));
            std::fs::write(&p, body).unwrap();
            let label = format!("section route {class}/{wide}");
            let input = json!({ "path": p.to_str().unwrap(), "heading": "## S" });
            let err = ReadFile.call(input, &ctx).await.unwrap_err();
            let rec = err
                .downcast_ref::<RecoverableError>()
                .expect("an oversized section is a RecoverableError");
            let lines: u64 = spans.captures(&rec.message).unwrap()[1].parse().unwrap();
            let actions: Vec<String> = rec.extra["next_actions"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| a.as_str().unwrap().to_string())
                .collect();
            let c = actions
                .iter()
                .find_map(|a| range.captures(a))
                .unwrap_or_else(|| panic!("{label}: no range route in {actions:?}"));
            let route = json!({
                "path": &c[1],
                "start_line": 1,
                "end_line": c[2].parse::<u64>().unwrap(),
            });
            let (_, last) = md_read_through(&ctx, route, &label).await;
            assert!(
                last >= lines,
                "{label}: the route stops at line {last} of {lines}: {actions:?}"
            );
        }
    }
}

/// The buffered range arm keeps `coverage` unless it costs the page: dropped (marked
/// `coverage_omitted`) when a line that does not fit beside it fits whole without it, kept when
/// the line is cut either way, and always dropped when `coverage` alone is over the limit (a
/// range starting on an empty line has a first line that fits any room, so only that check
/// stops a 20 KB `coverage` riding along).
#[tokio::test]
async fn a_markdown_range_drops_coverage_only_to_show_a_line_whole() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let mut dropped = 0;
    let mut cut = 0;
    for w in (9_900..=10_010).chain([12_000]) {
        let p = dir.path().join(format!("cw-{w}.md"));
        std::fs::write(&p, format!("# Top\n{}\nz\n## Later\n", "a".repeat(w))).unwrap();
        let prime = json!({ "path": p.to_str().unwrap(), "start_line": 1, "end_line": 1 });
        ReadFile.call(prime, &ctx).await.unwrap();
        let input = json!({ "path": p.to_str().unwrap(), "start_line": 2, "end_line": 3 });
        let label = format!("coverage width {w}");
        let (v, _) = md_page(&ctx, &input, &label).await;
        if v["coverage_omitted"] == json!(true) {
            dropped += 1;
            assert_ne!(
                v["line_truncated"],
                json!(true),
                "{label}: coverage was dropped and the line is still cut: {v:.300}"
            );
        }
        if v.get("file_id").is_some() && v["line_truncated"] == json!(true) {
            cut += 1;
            assert!(
                v.get("coverage").is_some(),
                "{label}: a line cut either way lost coverage too: {v:.300}"
            );
        }
        md_read_through(&ctx, input, &label).await;
    }
    assert!(
        dropped > 0,
        "coverage was never dropped to show a line whole"
    );
    assert!(cut > 0, "no line was cut beside coverage");

    let p = dir.path().join("many.md");
    std::fs::write(&p, many_sections(600)).unwrap();
    let prime = json!({ "path": p.to_str().unwrap(), "start_line": 1, "end_line": 1 });
    ReadFile.call(prime, &ctx).await.unwrap();
    // Line 3 is the blank line after the first section's body.
    let input = json!({ "path": p.to_str().unwrap(), "start_line": 3, "end_line": 900 });
    let (v, _) = md_page(&ctx, &input, "empty first line").await;
    assert_eq!(v["coverage_omitted"], json!(true), "{v:.300}");
    md_read_through(&ctx, input, "empty first line").await;

    // `coverage` over the limit AND a first line too wide even without it: the line is cut
    // either way, yet `coverage` must still go.
    let p = dir.path().join("many-wide.md");
    std::fs::write(
        &p,
        format!("{}{}\nz\n", many_sections(600), "a".repeat(12_000)),
    )
    .unwrap();
    let prime = json!({ "path": p.to_str().unwrap(), "start_line": 1, "end_line": 1 });
    ReadFile.call(prime, &ctx).await.unwrap();
    let input = json!({ "path": p.to_str().unwrap(), "start_line": 1_801, "end_line": 1_802 });
    let (v, _) = md_page(&ctx, &input, "wide line beside 20 KB coverage").await;
    assert_eq!(v["coverage_omitted"], json!(true), "{v:.300}");
    assert_eq!(v["line_truncated"], json!(true), "{v:.300}");
}

/// The multi-heading arm's oversized error (`combined headings span N lines`) echoes each
/// requested section's heading in `requested_headings` and `next_actions`. Measured here with
/// headings of 12 KB in seven classes: an `Err` body is put inline by the server, so it must fit.
#[tokio::test]
async fn a_multi_heading_overflow_error_echoes_bounded_headings() {
    let dir = tempfile::tempdir().unwrap();
    for (class, unit) in CLASSES.into_iter().chain([("esc", "\u{1b}")]) {
        let path = dir.path().join(format!("multi-{class}.md"));
        let long = unit.repeat(12_000 / json_escaped_len(unit));
        let mut body = String::new();
        let mut asked = Vec::new();
        for i in 0..3 {
            body.push_str(&format!("## H{i} {long}\n{}\n\n", "b\n".repeat(2_500)));
            asked.push(format!("## H{i}"));
        }
        std::fs::write(&path, &body).unwrap();
        let input = json!({ "path": path.to_str().unwrap(), "headings": asked });
        let ctx = ctx().await;
        let (text, compact) = deliver(&ctx, &input).await;
        eprintln!("multi {class}: {compact} B");
        assert!(
            text.contains("exceeds inline threshold"),
            "{class}: {text:.300}"
        );
        // Each echo is clipped, so three of them fit and the list is kept, not dropped.
        let err = ReadFile.call(input.clone(), &ctx).await.unwrap_err();
        let rec = err.downcast_ref::<RecoverableError>().unwrap();
        assert_eq!(
            rec.extra["requested_headings"].as_array().map(Vec::len),
            Some(3),
            "{class}"
        );
        assert_one_handle_and_fits(input, &format!("multi {class}")).await;
    }
    // Eighty requested sections: each echo is clipped, but the list is long, so it is dropped.
    let path = dir.path().join("multi-many.md");
    let body: String = (0..80)
        .map(|i| format!("## H{i:02} {}\n{}\n", "w".repeat(300), "b\n".repeat(100)))
        .collect();
    std::fs::write(&path, &body).unwrap();
    let asked: Vec<String> = (0..80).map(|i| format!("## H{i:02}")).collect();
    let input = json!({ "path": path.to_str().unwrap(), "headings": asked });
    let ctx = ctx().await;
    let err = ReadFile.call(input.clone(), &ctx).await.unwrap_err();
    let rec = err.downcast_ref::<RecoverableError>().unwrap();
    assert_eq!(
        rec.extra["requested_headings_omitted"],
        json!(true),
        "{:?}",
        rec.extra
    );
    assert_one_handle_and_fits(input, "multi 80").await;
}
