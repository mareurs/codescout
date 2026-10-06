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
        assert!(
            v["hint"].as_str().is_some_and(|h| h.contains("grep -o")),
            "{label}: a truncated line names no route off it: {:?}",
            v["hint"]
        );
    }
    (v, compact)
}

/// Read from `input` to the end, following every `next`. Returns the largest compact size seen.
/// A `next` that carries `force=true` (a forced range of a real file) is followed with it.
async fn read_through(
    ctx: &crate::tools::ToolContext,
    mut input: Value,
    input_ref: &str,
    label: &str,
) -> usize {
    let route = regex::Regex::new(
        r#"^read_file\("([^"]+)", start_line=(\d+), end_line=(\d+)(, force=true)?\)$"#,
    )
    .unwrap();
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
        if c.get(4).is_some() {
            input["force"] = json!(true);
        }
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

/// `count` is part of the json_path response, so the inline decision must measure it. The
/// coarse sweep can step over the 10-byte window `,"count":1` opens at the edge; this walks it
/// one byte at a time.
#[tokio::test]
async fn json_path_inline_decision_counts_the_count_key() {
    let ctx = ctx().await;
    let mut spilled = 0;
    for len in 9_900..=10_000 {
        let r = ctx
            .output_buffer
            .store_tool("probe", json!({ "v": ["a".repeat(len)] }).to_string());
        let input = json!({ "path": r, "json_path": "$.v" });
        let (v, _) = page(&ctx, &input, &r, &format!("count/{len}")).await;
        spilled += usize::from(v.get("file_id").is_some());
    }
    assert!(spilled > 0, "the walk never crossed the edge");
}

/// A page's `next` can resume at a many-digit line, and the room must count those digits. A
/// clamped line deep in a buffer with a line after it puts `line_truncated`, `hint` and a
/// three-digit `next` in one response that the clamp fills to the byte.
#[tokio::test]
async fn a_clamped_line_deep_in_a_buffer_counts_the_digits_of_next() {
    let ctx = ctx().await;
    for kind in REFS {
        for (class, unit) in CLASSES {
            let mut lines = vec!["short".to_string(); 99];
            lines.push(one_line(unit, 30_000));
            lines.push("tail".into());
            let r = park(&ctx, kind, lines.join("\n"));
            let input = json!({ "path": r, "start_line": 100, "end_line": 900 });
            let label = format!("deep {kind:?}/{class}");
            read_through(&ctx, input, &r, &label).await;
        }
    }
}

/// A buffer that holds only a prefix says so on every read (`buffer_truncated`, about 280 B).
/// The notice was attached by `call` AFTER `read_from_buffer` had sized its response, so a page
/// or an inline read at the edge went over by the notice and was buffered under `@tool_*`.
#[tokio::test]
async fn a_truncated_buffer_counts_its_notice_in_every_read() {
    let ctx = ctx().await;
    for (class, unit) in CLASSES {
        for size in sweep().chain([12_000]) {
            for body in [payload(unit, size), one_line(unit, size)] {
                let kept = body.lines().count();
                let r = ctx.output_buffer.store_truncated(
                    "probe".into(),
                    body,
                    String::new(),
                    0,
                    Some(crate::tools::output_buffer::Truncation {
                        kept_lines: kept,
                        total_lines: kept * 10,
                    }),
                );
                for input in [
                    json!({ "path": r }),
                    json!({ "path": r, "start_line": 1, "end_line": 100_000 }),
                ] {
                    let label = format!("truncated {class}/{size}/{kept} {input}");
                    let first = ReadFile.call(input.clone(), &ctx).await.unwrap();
                    assert!(
                        first["buffer_truncated"][0]
                            .as_str()
                            .is_some_and(|n| n.contains(&r)),
                        "{label}: the read does not say the buffer is a prefix: {first:.300}"
                    );
                    read_through(&ctx, input, &r, &label).await;
                }
            }
        }
    }
}

/// A line range read from a REAL file (`read_with_line_range`). It chose its inline arm on the
/// slice's raw bytes, sized its page without the other keys, and had no clamp for one line
/// wider than the page: measured before the fix, a ~9,990 B multi-line range came back at
/// 10,005-10,007 B and a 12 KB line at 12,017-12,177 B, both as `@tool_*` (with a `file_id` as
/// well for ascii, euro and emoji).
#[tokio::test]
async fn a_real_file_range_at_the_byte_edge_keeps_one_handle() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let mut largest = 0;
    for (class, unit) in CLASSES {
        let bodies = sweep()
            .map(|s| (format!("lines{s}"), payload(unit, s)))
            .chain([
                ("lines9990".into(), payload(unit, 9_990)),
                ("one12k".into(), one_line(unit, 12_000)),
                ("one60k".into(), one_line(unit, 60_000)),
            ]);
        for (shape, body) in bodies {
            for tail in ["", "\nz\n"] {
                let p = dir
                    .path()
                    .join(format!("{class}-{shape}-{}.txt", tail.len()));
                std::fs::write(&p, format!("{body}{tail}")).unwrap();
                let path = p.to_str().unwrap().to_string();
                let input = json!({ "path": path, "start_line": 1, "end_line": 100_000 });
                let label = format!("real range {class}/{shape}/{}", tail.len());
                largest = largest.max(read_through(&ctx, input, &path, &label).await);
            }
        }
    }
    eprintln!("real range: largest response judged = {largest} B");
}

/// A whole read of a REAL file whose raw bytes are under the limit but whose escaped bytes
/// are not (`read_full_file` decided on raw bytes).
#[tokio::test]
async fn a_real_file_whole_read_of_escape_heavy_text_keeps_one_handle() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    for (class, unit) in CLASSES {
        for raw in (1_000..=10_000).step_by(500) {
            let n = raw / unit.len();
            for (shape, body) in [
                ("one", unit.repeat(n)),
                ("lines", {
                    let line = unit.repeat(40);
                    vec![line; (n / 40).max(1)].join("\n")
                }),
            ] {
                let p = dir.path().join(format!("w-{class}-{raw}-{shape}.txt"));
                std::fs::write(&p, &body).unwrap();
                let path = p.to_str().unwrap().to_string();
                let label = format!("real whole {class}/{raw}/{shape}");
                read_through(&ctx, json!({ "path": path }), &path, &label).await;
            }
        }
    }
}

/// A real-file range response also carries `coverage` (a markdown file read with
/// `force=true` while sections stay unread), and `next` can resume at a many-digit line. Both
/// are counted: the inline decision walks the edge in 5-byte steps with `coverage` present,
/// and a clamped line at line 100 fills its page to the byte beside `coverage` and a
/// three-digit `next`.
#[tokio::test]
async fn a_real_file_range_counts_coverage_and_the_digits_of_next() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let prime = |p: &str| json!({ "path": p, "start_line": 1, "end_line": 1, "force": true });
    let mut covered = 0;
    for size in (9_900..=10_000).step_by(5) {
        let p = dir.path().join(format!("cov-{size}.md"));
        let body = payload("a", size);
        let n = body.lines().count();
        std::fs::write(&p, format!("# Top\n{body}\n## Later\n")).unwrap();
        let path = p.to_str().unwrap().to_string();
        ReadFile.call(prime(&path), &ctx).await.unwrap();
        let input = json!({ "path": path, "start_line": 2, "end_line": n + 1, "force": true });
        let (v, _) = page(&ctx, &input, &path, &format!("coverage/{size}")).await;
        covered += usize::from(v.get("coverage").is_some());
    }
    assert!(covered > 0, "no response carried coverage");
    for (class, unit) in CLASSES {
        let p = dir.path().join(format!("deep-{class}.md"));
        let mut lines = vec!["# Top".to_string()];
        lines.extend(vec!["short".to_string(); 98]);
        lines.push(one_line(unit, 30_000));
        lines.push("tail".into());
        lines.push("## Later".into());
        std::fs::write(&p, lines.join("\n")).unwrap();
        let path = p.to_str().unwrap().to_string();
        ReadFile.call(prime(&path), &ctx).await.unwrap();
        let input = json!({ "path": path, "start_line": 100, "end_line": 101, "force": true });
        let (v, _) = page(&ctx, &input, &path, &format!("deep md {class}")).await;
        assert!(
            v.get("coverage").is_some() && v.get("next").is_some(),
            "deep md {class}: the page lacks coverage or next: {v:.300}"
        );
    }
}

/// Units of a value such that `cost(units)`, the serialized bytes of the content a read returns
/// for it, is about `size`. `cost` is linear in the units, so two samples fix it.
fn units_for(size: usize, cost: impl Fn(usize) -> usize) -> usize {
    let base = cost(0);
    let per_1000 = cost(1_000) - base;
    (size.saturating_sub(base) * 1_000 / per_1000).max(1)
}

/// A `json_path` or `toml_key` read of a REAL file had no inline gate: the value went out
/// whole and `call_content` buffered it as `@tool_*`. Measured before the fix: a json_path read
/// minted `@tool_` at 10,004 B. A response over the limit must come back as one `file_id`
/// whose `hint` names a line route that reads it to the end.
#[tokio::test]
async fn a_json_path_or_toml_key_read_of_a_real_file_keeps_one_handle() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let hint_route =
        regex::Regex::new(r#"read_file\("(@file_[0-9a-f]+)", start_line=N, end_line=M\)"#).unwrap();
    let json_text = |v: &Value| v.to_string();
    // A JSON string or array literal is a valid TOML basic string or inline array (`\uXXXX`,
    // `\"`, `\\`). The `toml` serializer overflows on these values in a debug build.
    let toml_text = |v: &Value| format!("k = {}\n", v["k"]);
    // A JSON string or array literal is also a YAML double-quoted scalar or flow sequence
    // (`\uXXXX`, `\"`, `\\`): a YAML key read takes the same `inline_or_file_id` as TOML.
    let yaml_text = |v: &Value| format!("k: {}\n", v["k"]);
    let mut buffered = 0;
    for (class, unit) in CLASSES.into_iter().chain([("esc", "\u{1b}")]) {
        // One string value, and a value of many 40-unit lines.
        let one = |n: usize| json!({ "k": unit.repeat(n) });
        let many = |n: usize| json!({ "k": vec![unit.repeat(40); n] });
        let sizes: Vec<usize> = sweep()
            .chain((9_950..=10_010).step_by(3))
            .chain([12_000, 60_000])
            .collect();
        for size in sizes {
            for (fmt, ext, nav, text_of) in [
                (
                    "json",
                    "json",
                    json!({ "json_path": "$.k" }),
                    &json_text as &dyn Fn(&Value) -> String,
                ),
                ("toml", "toml", json!({ "toml_key": "k" }), &toml_text),
                ("yaml", "yaml", json!({ "toml_key": "k" }), &yaml_text),
            ] {
                for (shape, make) in [("one", &one as &dyn Fn(usize) -> Value), ("many", &many)] {
                    let n = units_for(size, |u| json_escaped_len(&text_of(&make(u))));
                    let p = dir
                        .path()
                        .join(format!("{fmt}-{class}-{shape}-{size}.{ext}"));
                    std::fs::write(&p, text_of(&make(n))).unwrap();
                    let mut input = nav.clone();
                    input["path"] = json!(p.to_str().unwrap());
                    let label = format!("{fmt} {class}/{shape}/{size}");
                    let (v, _) = page(&ctx, &input, "", &label).await;
                    let Some(fid) = v["file_id"].as_str() else {
                        assert!(v.get("content").is_some(), "{label}: no content: {v:.300}");
                        continue;
                    };
                    buffered += 1;
                    let hint = v["hint"].as_str().unwrap_or_default();
                    let c = hint_route
                        .captures(hint)
                        .unwrap_or_else(|| panic!("{label}: the hint names no line route: {hint}"));
                    assert_eq!(&c[1], fid, "{label}: the hint names another handle");
                    let total = v["total_lines"].as_u64().unwrap();
                    let route = json!({ "path": fid, "start_line": 1, "end_line": total });
                    read_through(&ctx, route, fid, &label).await;
                }
            }
        }
    }
    assert!(buffered > 0, "no read took the file_id arm");
}

/// `siblings` of a `toml_key` read is capped in count upstream but not in width. A read whose
/// handle arm would be over the limit with about 12 KB of sibling names beside it drops them,
/// marked `siblings_omitted`, so that arm is not buffered again under `@tool_*`.
#[tokio::test]
async fn a_toml_key_read_with_many_siblings_drops_them_from_the_handle_arm() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    // Siblings are capped in count upstream (about 30), not in width: 40 tables with 400-byte
    // names put about 12 KB of them beside the value.
    let siblings: String = (0..40)
        .map(|i| format!("[key_{i:04}{}]\nx = 1\n", "n".repeat(400)))
        .collect();
    for (shape, value) in [("wide", "a".repeat(12_000)), ("short", "a".into())] {
        let p = dir.path().join(format!("sib-{shape}.toml"));
        std::fs::write(&p, format!("[k]\nv = \"{value}\"\n{siblings}")).unwrap();
        let input = json!({ "path": p.to_str().unwrap(), "toml_key": "k" });
        let label = format!("siblings {shape}");
        let (v, _) = page(&ctx, &input, "", &label).await;
        assert_eq!(v["siblings_omitted"], json!(true), "{label}: {v:.300}");
        assert!(v.get("file_id").is_some(), "{label}: {v:.300}");
    }
}

/// The seven content classes: [`CLASSES`] and the escape character `\x1b`, which serializes to
/// six bytes like `\x01` but is the one terminal output carries.
const CLASSES7: [(&str, &str); 7] = [
    ("ascii", "a"),
    ("quote", "\""),
    ("backslash", "\\"),
    ("control", "\u{1}"),
    ("esc", "\u{1b}"),
    ("euro", "\u{20ac}"),
    ("emoji", "\u{1F600}"),
];

/// Bodies swept across the edge for a range read: multi-line payloads near 10,003 B in 50 B
/// steps, 5 B steps where the response crosses the limit, a short one, and one line of 12 KB
/// and of 60 KB.
fn edge_bodies(unit: &str) -> Vec<(String, String)> {
    sweep()
        .chain((9_950..=10_010).step_by(5))
        .map(|s| (format!("lines{s}"), payload(unit, s)))
        .chain([
            ("short".into(), unit.repeat(10)),
            ("one12k".into(), one_line(unit, 12_000)),
            ("one60k".into(), one_line(unit, 60_000)),
        ])
        .collect()
}

/// `n` sections `## Section NNNN <40 x 'w'>`: a heading list, and so a `coverage.unread`, far
/// over the inline limit once 600 of them are unread (about 33 KB).
fn many_sections(n: usize) -> String {
    (1..=n)
        .map(|i| format!("## Section {i:04} {}\nbody {i}\n\n", "w".repeat(40)))
        .collect()
}

/// `read_with_line_range` put `coverage` in both arms unconditionally. On a markdown file read
/// with `force=true` whose unread headings alone serialize past the limit, no page fits beside
/// it, and `call_content` buffered the response under `@tool_*` (beside the `file_id`, when the
/// range was buffered). It is dropped, marked `coverage_omitted`, as the markdown range arm drops
/// it: when it is over the limit alone, or when a line would show whole without it.
#[tokio::test]
async fn a_forced_markdown_range_whose_coverage_alone_overflows_keeps_one_handle() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let sections = many_sections(600);
    let prime = |p: &str| json!({ "path": p, "start_line": 1, "end_line": 1, "force": true });
    let mut omitted = 0;
    let mut largest = 0;
    for (class, unit) in CLASSES7 {
        for (shape, body) in edge_bodies(unit) {
            let p = dir.path().join(format!("cov-{class}-{shape}.md"));
            std::fs::write(&p, format!("# Top\n{body}\n{sections}")).unwrap();
            let path = p.to_str().unwrap().to_string();
            ReadFile.call(prime(&path), &ctx).await.unwrap();
            let n = body.lines().count();
            let input = json!({ "path": path, "start_line": 2, "end_line": n + 1, "force": true });
            let label = format!("forced md range {class}/{shape}");
            let (v, _) = page(&ctx, &input, &path, &label).await;
            omitted += usize::from(v["coverage_omitted"] == json!(true));
            largest = largest.max(read_through(&ctx, input, &path, &label).await);
        }
    }
    assert!(omitted > 0, "no response marked coverage_omitted");
    eprintln!("forced md range with oversized coverage: largest response = {largest} B");
}

/// `heading=` on a NON-markdown real file. Measured: what arm serves it, and how large its
/// response is when the heading is the caller's 12 KB of each class.
#[tokio::test]
async fn heading_on_a_non_markdown_file_is_refused_within_the_limit() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("plain.txt");
    std::fs::write(&p, "## A\nbody\n").unwrap();
    for (class, unit) in CLASSES7 {
        for bytes in [10usize, 12_000] {
            let heading = one_line(unit, bytes);
            let input = json!({ "path": p.to_str().unwrap(), "heading": heading });
            let label = format!("heading on txt {class}/{bytes}");
            let (body, compact) = match ReadFile.call(input.clone(), &ctx).await {
                Ok(v) => (v.clone(), v.to_string().len()),
                Err(e) => {
                    let rec = e
                        .downcast_ref::<crate::tools::RecoverableError>()
                        .unwrap_or_else(|| panic!("{label}: not recoverable: {e}"));
                    let mut b =
                        json!({ "error": rec.message, "hint": rec.hint().unwrap_or_default() });
                    for (k, val) in rec.extra.iter() {
                        b[k] = val.clone();
                    }
                    let n = b.to_string().len();
                    (b, n)
                }
            };
            eprintln!("{label}: {compact} B: {body:.160}");
            assert!(
                !crate::tools::exceeds_inline_limit_len(compact),
                "{label}: {compact} B is over the inline limit"
            );
            let blocks = ReadFile.call_content(input.clone(), &ctx).await;
            if let Ok(blocks) = blocks {
                let text = crate::tools::hint_probe::primary_text(&blocks);
                assert!(
                    handles_in(&text).is_empty(),
                    "{label}: minted a handle: {text:.300}"
                );
            }
        }
    }
}

/// A file of a registered library carries `source: "lib:<name>"`, beside the content in both
/// range arms and in the inline whole-file arm. Measured at the edge in every class, with a
/// short library name and a 2 KB one (a name is the caller's, given to `library(register)`).
#[tokio::test]
async fn a_library_file_read_counts_its_source_tag() {
    let proj = tempfile::tempdir().unwrap();
    let lib = tempfile::tempdir().unwrap();
    for name in ["mylib".to_string(), "n".repeat(2_000)] {
        let ctx = {
            let mut c = ctx().await;
            c.agent = Agent::new(Some(proj.path().to_path_buf())).await.unwrap();
            {
                let mut inner = c.agent.inner.write().await;
                let project = inner.active_project_mut().unwrap();
                project.library_registry.register(
                    name.clone(),
                    lib.path().to_path_buf(),
                    "rust".to_string(),
                    crate::library::registry::DiscoveryMethod::Manual,
                    true,
                );
            }
            c
        };
        let mut tagged = 0;
        let mut largest = 0;
        for (class, unit) in CLASSES7 {
            for (shape, body) in edge_bodies(unit) {
                let p = lib
                    .path()
                    .join(format!("{}-{class}-{shape}.txt", name.len()));
                std::fs::write(&p, &body).unwrap();
                let path = p.to_str().unwrap().to_string();
                let label = format!("lib {} {class}/{shape}", name.len());
                let range = json!({ "path": path, "start_line": 1, "end_line": 100_000 });
                let (v, _) = page(&ctx, &range, &path, &label).await;
                tagged += usize::from(v["source"] == json!(format!("lib:{name}")));
                largest = largest.max(read_through(&ctx, range, &path, &label).await);
                largest =
                    largest.max(read_through(&ctx, json!({ "path": path }), &path, &label).await);
            }
        }
        assert!(tagged > 0, "no response carried the lib:{} tag", name.len());
        eprintln!("lib name {} B: largest response = {largest} B", name.len());
    }
}
