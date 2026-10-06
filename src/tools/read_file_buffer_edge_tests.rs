//! Byte-edge probes for `read_file` on a buffer ref (`@file_*`, `@cmd_*`, `@tool_*`), driven
//! through the REAL `ReadFile::call_content`.
//!
//! The inline limit is judged on the COMPACT SERIALIZED response (`exceeds_inline_limit`: more
//! than 10,003 B). JSON escaping makes one raw byte cost 1 (ASCII), 2 (`"`, `\`, newline) or 6
//! (`\x01`) serialized bytes. `read_from_buffer` decided its inline arm, and clamped an
//! over-wide line, on RAW bytes, so a response it believed inline was buffered again by
//! `call_content` under `@tool_*`: a second handle for a result that already had one
//! (ADR 2026-10-05), and a break of its own doc ("Never re-wraps its own result").
//!
//! Each sweep crosses the edge in every content class and asserts, per page: no `@tool_*`
//! minted, at most one handle minted (the input ref named back in `next` is not minted), the
//! compact response fits, a truncated line keeps bytes, and every `next` runs and advances
//! until the read completes.

use super::ReadFile;
use crate::agent::Agent;
use crate::lsp::LspManager;
use crate::tools::Tool;
use crate::util::text::json_escaped_len;
use serde_json::{json, Value};
use std::collections::BTreeSet;

async fn ctx() -> crate::tools::ToolContext {
    crate::tools::ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: LspManager::new_arc(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(500)),
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

/// Content classes, by one unit of each.
const CLASSES: [(&str, &str); 6] = [
    ("ascii", "a"),
    ("quote", "\""),
    ("backslash", "\\"),
    ("control", "\u{1}"),
    ("euro", "\u{20ac}"),
    ("emoji", "\u{1F600}"),
];

/// Payload sizes in serialized bytes of the payload alone; the response around it adds a
/// little, so the RESPONSE crosses the 10,003 B edge.
fn sweep() -> impl Iterator<Item = usize> {
    (9_000..=10_500).step_by(50)
}

/// Lines of `unit` (40 per line) whose JSON-escaped length is about `escaped` bytes.
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

/// One line of `unit` whose JSON-escaped length is about `escaped` bytes.
fn one_line(unit: &str, escaped: usize) -> String {
    unit.repeat((escaped / json_escaped_len(unit)).max(1))
}

/// The kinds of buffer a `read_file` can address.
#[derive(Clone, Copy, Debug)]
enum Ref {
    File,
    Cmd,
    Tool,
}

const REFS: [Ref; 3] = [Ref::File, Ref::Cmd, Ref::Tool];

/// Park `text` behind a handle of kind `kind`. A `@tool_*` ref is line-addressed through its
/// pretty-printed JSON, so `text` is stored as a string field and re-escaped once more there;
/// `escaped_for` compensates so the swept size still lands near the edge.
fn park(ctx: &crate::tools::ToolContext, kind: Ref, text: String) -> String {
    match kind {
        Ref::File => ctx
            .output_buffer
            .store_file_excerpt("probe.txt".into(), text),
        Ref::Cmd => ctx
            .output_buffer
            .store("probe".into(), text, String::new(), 0),
        Ref::Tool => ctx
            .output_buffer
            .store_tool("run_command", json!({ "stdout": text }).to_string()),
    }
}

/// Size to ask a payload builder for, so the line-addressable text of `kind` is about `size`.
fn escaped_for(kind: Ref, unit: &str, size: usize) -> usize {
    match kind {
        Ref::Tool => {
            let quoted = serde_json::to_string(unit).unwrap();
            let inner = &quoted[1..quoted.len() - 1];
            size * json_escaped_len(unit) / json_escaped_len(inner)
        }
        _ => size,
    }
}

/// Every distinct buffer handle named in `text`.
fn handles_in(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '@' || c == '_'))
        .filter(|t| {
            // A prose mention like "a @tool_* ref" is not a handle: require an id after the prefix.
            ["@file_", "@tool_", "@cmd_", "@bg_"]
                .iter()
                .any(|p| t.len() > p.len() && t.starts_with(p))
        })
        .map(str::to_owned)
        .collect()
}

/// The per-page contract. Returns the response and its compact size.
async fn page(
    ctx: &crate::tools::ToolContext,
    input: &Value,
    input_ref: &str,
    label: &str,
) -> (Value, usize) {
    let v = ReadFile
        .call(input.clone(), ctx)
        .await
        .unwrap_or_else(|e| panic!("{label}: {input} failed: {e}"));
    let compact = v.to_string().len();
    let blocks = ReadFile.call_content(input.clone(), ctx).await.unwrap();
    let text = crate::tools::hint_probe::primary_text(&blocks);
    let mut minted = handles_in(&text);
    minted.remove(input_ref);
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
        let shown = v["content"].as_str().unwrap_or_default();
        assert!(
            shown.chars().any(|c| c != '\n') && !shown.starts_with("\n…"),
            "{label}: a truncated line kept no bytes"
        );
    }
    (v, compact)
}

/// Read from `input` to the end, following every `next`. Returns the largest compact size seen.
async fn read_through(
    ctx: &crate::tools::ToolContext,
    mut input: Value,
    input_ref: &str,
    label: &str,
) -> usize {
    let route =
        regex::Regex::new(r#"^read_file\("([^"]+)", start_line=(\d+), end_line=(\d+)\)$"#).unwrap();
    let mut largest = 0;
    let mut prev_start = 0u64;
    for _ in 0..400 {
        let (v, compact) = page(ctx, &input, input_ref, label).await;
        largest = largest.max(compact);
        let Some(next) = v["next"].as_str() else {
            return largest;
        };
        let c = route
            .captures(next)
            .unwrap_or_else(|| panic!("{label}: next {next:?} is not a line route"));
        assert_eq!(&c[1], input_ref, "{label}: next leaves the ref");
        let start: u64 = c[2].parse().unwrap();
        let shown_end = v["shown_lines"][1].as_u64().unwrap();
        assert_eq!(
            start,
            shown_end + 1,
            "{label}: next does not resume after the page"
        );
        assert!(start > prev_start, "{label}: next does not advance");
        prev_start = start;
        input = json!({ "path": input_ref, "start_line": start, "end_line": c[3].parse::<u64>().unwrap() });
    }
    panic!("{label}: next chain did not terminate");
}

/// Site 1: a line range of a buffer whose slice crosses the edge. The inline arm was chosen on
/// the slice's RAW bytes, so 2,000 `\x01` (12 KB serialized) went inline and were buffered.
#[tokio::test]
async fn range_read_of_a_buffer_at_the_byte_edge_keeps_one_handle() {
    let ctx = ctx().await;
    let mut largest = 0;
    for kind in REFS {
        for (class, unit) in CLASSES {
            for size in sweep().chain([12_000]) {
                let r = park(&ctx, kind, payload(unit, escaped_for(kind, unit, size)));
                let input = json!({ "path": r, "start_line": 1, "end_line": 100_000 });
                let label = format!("range {kind:?}/{class}/{size}");
                largest = largest.max(read_through(&ctx, input, &r, &label).await);
            }
        }
    }
    eprintln!("range read: largest response judged = {largest} B");
}

/// Site 2: ONE line wider than the budget, alone and followed by another line, read by range
/// and whole. The clamp tested the line's RAW bytes and kept half the budget raw, so an
/// escape-heavy line was either not clamped or clamped to a size that still overflowed.
#[tokio::test]
async fn one_over_wide_buffer_line_is_clamped_in_escaped_bytes() {
    let ctx = ctx().await;
    let mut largest = 0;
    let mut truncated_seen = 0;
    for kind in REFS {
        for (class, unit) in CLASSES {
            for size in sweep().chain([12_000, 60_000]) {
                for tail in ["", "\ntail line"] {
                    let line = one_line(unit, escaped_for(kind, unit, size));
                    let r = park(&ctx, kind, format!("{line}{tail}"));
                    for input in [
                        json!({ "path": r, "start_line": 1, "end_line": 1 }),
                        json!({ "path": r, "start_line": 1, "end_line": 50 }),
                        json!({ "path": r }),
                    ] {
                        let label = format!("line {kind:?}/{class}/{size}/{tail:?} {input}");
                        let first = ReadFile.call(input.clone(), &ctx).await.unwrap();
                        truncated_seen += usize::from(first["line_truncated"] == json!(true));
                        largest = largest.max(read_through(&ctx, input, &r, &label).await);
                    }
                }
            }
        }
    }
    assert!(truncated_seen > 0, "no case exercised the clamp");
    eprintln!("over-wide line: largest response judged = {largest} B, {truncated_seen} clamped");
}

/// Site 3: a whole-buffer read whose text crosses the edge. Same raw-byte decision as site 1.
#[tokio::test]
async fn full_read_of_a_buffer_at_the_byte_edge_keeps_one_handle() {
    let ctx = ctx().await;
    let mut largest = 0;
    for kind in REFS {
        for (class, unit) in CLASSES {
            for size in sweep().chain([12_000, 30_000]) {
                let r = park(&ctx, kind, payload(unit, escaped_for(kind, unit, size)));
                let label = format!("full {kind:?}/{class}/{size}");
                largest = largest.max(read_through(&ctx, json!({ "path": r }), &r, &label).await);
            }
        }
    }
    eprintln!("full read: largest response judged = {largest} B");
}

/// Site 4: `json_path` into a `@tool_*` ref whose value crosses the edge. The inline arm was
/// chosen on the extracted value's RAW bytes.
#[tokio::test]
async fn json_path_into_a_tool_ref_at_the_byte_edge_keeps_one_handle() {
    let ctx = ctx().await;
    let mut largest = 0;
    let mut spilled = 0;
    for (class, unit) in CLASSES {
        for size in sweep().chain([12_000]) {
            for value in [json!(payload(unit, size)), json!([payload(unit, size)])] {
                let r = ctx
                    .output_buffer
                    .store_tool("probe", json!({ "v": value }).to_string());
                let input = json!({ "path": r, "json_path": "$.v" });
                let label = format!("json_path {class}/{size}/{}", value.is_array());
                let (v, compact) = page(&ctx, &input, &r, &label).await;
                largest = largest.max(compact);
                if let Some(fid) = v["file_id"].as_str() {
                    spilled += 1;
                    let total = v["total_lines"].as_u64().unwrap();
                    let browse = json!({ "path": fid, "start_line": 1, "end_line": total });
                    read_through(&ctx, browse, fid, &label).await;
                }
            }
        }
    }
    assert!(spilled > 0, "no case reached the file_id arm");
    eprintln!("json_path: largest response judged = {largest} B, {spilled} spilled");
}

/// The reviewer's case, pinned by itself: one line of 2,000 `\x01` (12,039 B serialized as a
/// response) read by `start_line=1, end_line=1` came back as `@tool_*`, a second handle.
#[tokio::test]
async fn two_thousand_control_bytes_on_one_line_keep_one_handle() {
    let ctx = ctx().await;
    let r = park(&ctx, Ref::File, "\u{1}".repeat(2_000));
    let input = json!({ "path": r, "start_line": 1, "end_line": 1 });
    let (v, compact) = page(&ctx, &input, &r, "2000 x \\x01").await;
    assert_eq!(
        v["line_truncated"],
        json!(true),
        "the line was not clamped: {v}"
    );
    eprintln!("2000 x \\x01: response judged = {compact} B");
}

/// The valve's guarantee at the clamp, where no page reaches it: a room smaller than the
/// marker still keeps the line's first character, never zero bytes of a non-empty line.
#[test]
fn clamp_keeps_a_character_of_the_line_in_a_room_too_small_for_one() {
    for (class, unit) in CLASSES {
        let line = unit.repeat(40);
        for room in [0, 1, 5, 20] {
            let (out, cut) = super::clamp_over_budget_line(line.clone(), room);
            assert!(
                cut,
                "{class}/{room}: a line wider than the room was not cut"
            );
            assert!(
                out.starts_with(unit),
                "{class}/{room}: no character of the line survived: {out:?}"
            );
        }
    }
}
