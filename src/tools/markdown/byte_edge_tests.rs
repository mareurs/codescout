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
    for (class, unit) in CLASSES {
        let path = dir.path().join(format!("echo-{class}.md"));
        // 1,000 units: over the clip in every class, yet one line still fits a range read once
        // escaped (a line wider than that is the buffer reader's case, outside this test).
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
        let action = serde_json::to_string(&rec.extra["next_actions"][0]).unwrap();
        // An echo quoted inside another string is escaped twice (`"` -> `\"` -> `\\\"`), so the
        // bound on an embedded echo is twice the cap plus the call around it. A raw-byte clip of
        // `\x01` was 1,200 B per echo before that doubling.
        assert!(
            action.len() <= 2 * 200 + 100,
            "{class}: next_actions[0] is {} B",
            action.len()
        );
        assert!(
            serde_json::to_string(&rec.message).unwrap().len() <= 2 * 200 + 100,
            "{class}: the message is {} B",
            rec.message.len()
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
