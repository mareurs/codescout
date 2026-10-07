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
///
/// A read of a buffer ref (`input_ref` starts with `@`) mints no handle at all: every view of a
/// buffer is addressed through the ref itself. A read of a real file may mint one `@file_*`.
async fn page(
    ctx: &crate::tools::ToolContext,
    input: &Value,
    input_ref: &str,
    label: &str,
) -> (Value, usize) {
    let before = ctx.output_buffer.handles();
    let v = ReadFile
        .call(input.clone(), ctx)
        .await
        .unwrap_or_else(|e| panic!("{label}: {input} failed: {e}"));
    let compact = v.to_string().len();
    let blocks = ReadFile.call_content(input.clone(), ctx).await.unwrap();
    let text = crate::tools::hint_probe::primary_text(&blocks);
    if input_ref.starts_with('@') {
        assert_eq!(
            ctx.output_buffer.handles(),
            before,
            "{label}: a read of {input_ref} changed the buffer's handles: {:.300}",
            text
        );
    }
    let mut minted = handles_in(&text);
    minted.remove(input_ref);
    assert!(
        !minted.iter().any(|h| h.starts_with("@tool_")),
        "{label}: a second handle was minted for a {compact} B response ({minted:?}): {:.300}",
        text
    );
    if input_ref.starts_with('@') {
        assert!(
            minted.is_empty(),
            "{label}: a read of {input_ref} named another handle {minted:?}: {:.300}",
            text
        );
    }
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
/// A `next` that carries `force=true` (a forced range of a real file) is followed with it, and
/// one that carries a `json_path` (a page of a value extracted from a `@tool_*` ref) must repeat
/// the `json_path` it was asked for, quoted as a JSON string literal.
async fn read_through(
    ctx: &crate::tools::ToolContext,
    mut input: Value,
    input_ref: &str,
    label: &str,
) -> usize {
    let route = regex::Regex::new(
        r#"^read_file\("([^"]+)", (?:json_path=("(?:[^"\\]|\\.)*"), )?start_line=(\d+), end_line=(\d+)(, force=true)?\)$"#,
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
        let jp: Option<String> = c
            .get(2)
            .map(|m| serde_json::from_str(m.as_str()).expect("a JSON string literal"));
        assert_eq!(
            jp.as_deref(),
            input["json_path"].as_str(),
            "{label}: next does not keep the json_path it was asked for"
        );
        let start: u64 = c[3].parse().unwrap();
        let shown_end = v["shown_lines"][1].as_u64().unwrap();
        assert_eq!(
            start,
            shown_end + 1,
            "{label}: next does not resume after the page"
        );
        assert!(start > prev_start, "{label}: next does not advance");
        prev_start = start;
        input = json!({ "path": input_ref, "start_line": start, "end_line": c[4].parse::<u64>().unwrap() });
        if let Some(jp) = jp {
            input["json_path"] = json!(jp);
        }
        if c.get(5).is_some() {
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
/// chosen on the extracted value's RAW bytes. A value over the limit used to be stored under a
/// new `@file_*` handle; it is now paged through the `@tool_*` ref itself, with a `next` that
/// repeats the `json_path`, so the read is followed to its end on that one handle.
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
                assert!(v.get("file_id").is_none(), "{label}: {v:.300}");
                if v.get("shown_lines").is_some() {
                    spilled += 1;
                    largest = largest.max(read_through(&ctx, input, &r, &label).await);
                }
            }
        }
    }
    assert!(spilled > 0, "no case reached the paged arm");
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
/// one byte at a time. A value that does not fit inline is paged (`shown_lines`) on the ref.
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
        spilled += usize::from(v.get("shown_lines").is_some());
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
/// The handle arm of a `toml_key` read drops `siblings` when the arm WITH them is over the
/// limit, measured on that whole arm. Siblings that fit ALONE but not beside the arm's other
/// keys (`file_id`, `hint`, `line_range`, ...) are the case a measure of `siblings` by itself
/// misses: the arm kept them and `call_content` buffered it under a second handle. The sweep
/// puts the sibling list just under the limit on its own; it is ASCII because bare TOML keys
/// are, and the dimension it varies is the other keys, not escaping.
#[tokio::test]
async fn a_toml_key_read_whose_siblings_fit_alone_but_not_beside_the_handle_drops_them() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    // The limit a response is judged by: over it, `call_content` buffers it under `@tool_*`.
    let limit = crate::tools::INLINE_MAX_RESPONSE_LEN;
    // Over the limit alone, so the read takes the handle arm, where `siblings` is droppable.
    let value = "a".repeat(12_000);
    let mut in_window = 0;
    for w in 250..=400 {
        let siblings: String = (0..40)
            .map(|i| format!("[s{i:02}{}]\nx = 1\n", "n".repeat(w)))
            .collect();
        let text = format!("[k]\nv = \"{value}\"\n{siblings}");
        let extracted = crate::tools::file_summary::extract_toml_key(&text, "k").unwrap();
        let alone = json!(extracted.siblings).to_string().len();
        // Fits alone (10 B of margin), by less than the arm's other keys need beside it (they
        // are well over 150 B: the `hint` alone names the handle twice).
        if !(limit - 150..=limit - 10).contains(&alone) {
            continue;
        }
        in_window += 1;
        let p = dir.path().join(format!("sib-alone-{w}.toml"));
        std::fs::write(&p, &text).unwrap();
        let input = json!({ "path": p.to_str().unwrap(), "toml_key": "k" });
        let label = format!("siblings of width {w} ({alone} B alone)");
        let (v, _) = page(&ctx, &input, "", &label).await;
        assert!(v.get("file_id").is_some(), "{label}: {v:.300}");
        assert_eq!(v["siblings_omitted"], json!(true), "{label}: {v:.300}");
    }
    assert!(
        in_window > 0,
        "no width put the sibling list just under the limit alone"
    );
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
            // Unread headings remain on every page, so each carries `coverage` or the marker
            // that it was dropped, never neither; and a short range stays inline.
            assert!(
                v.get("coverage").is_some() != (v["coverage_omitted"] == json!(true)),
                "{label}: coverage and its marker disagree: {v:.300}"
            );
            if shape == "short" {
                assert!(
                    v.get("file_id").is_none(),
                    "{label}: a short range was buffered"
                );
            }
            omitted += usize::from(v["coverage_omitted"] == json!(true));
            largest = largest.max(read_through(&ctx, input, &path, &label).await);
        }
    }
    assert!(omitted > 0, "no response marked coverage_omitted");
    eprintln!("forced md range with oversized coverage: largest response = {largest} B");
}
/// `page_beside_coverage` drops `coverage` when the page skeleton WITH it is over the limit,
/// measured on that whole skeleton. A `coverage` that fits ALONE but not beside the page's other
/// keys is the case a measure of `coverage` by itself misses. With a first line too wide to show
/// whole with or without `coverage`, only that measure can drop it: missed, the arm kept it and
/// returned a response over the limit, which `call_content` buffered under a second handle.
/// Swept so `coverage` serializes just under the limit in every content class.
#[tokio::test]
async fn a_forced_markdown_range_whose_coverage_fits_alone_but_not_beside_the_page_drops_it() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    // The limit a response is judged by: over it, `call_content` buffers it under `@tool_*`.
    let limit = crate::tools::INLINE_MAX_RESPONSE_LEN;
    // Wider than any room a page has, so it is cut whether or not `coverage` rides beside it,
    // and the "a line would show whole without it" reason never drops `coverage`.
    let wide = "a".repeat(12_000);
    let prime = |p: &str| json!({ "path": p, "start_line": 1, "end_line": 1, "force": true });
    for (class, unit) in CLASSES7 {
        // About 25 serialized bytes per unread heading, so the sweep steps finely past the edge.
        let k = (18 / json_escaped_len(unit)).max(1);
        let mut in_window = 0;
        for n in 300..=500 {
            let sections: String = (1..=n)
                .map(|i| format!("## {i:04}{}\n", unit.repeat(k)))
                .collect();
            let text = format!("# Top\n{wide}\n{sections}");
            // `coverage` as the read reports it: `Top` is read (primed below), the rest unread.
            let unread: Vec<String> = crate::tools::file_summary::parse_all_headings(&text)
                .into_iter()
                .map(|h| h.text)
                .filter(|t| t != "Top")
                .collect();
            let alone = json!({ "read": 1, "total": n + 1, "unread": unread })
                .to_string()
                .len();
            // Fits alone (10 B of margin), by less than the page's other keys need beside it
            // (well over 150 B: `file_id`, `hint`, a `next` naming the temp path).
            if !(limit - 150..=limit - 10).contains(&alone) {
                continue;
            }
            in_window += 1;
            let p = dir.path().join(format!("cov-alone-{class}-{n}.md"));
            std::fs::write(&p, &text).unwrap();
            let path = p.to_str().unwrap().to_string();
            ReadFile.call(prime(&path), &ctx).await.unwrap();
            let input = json!({ "path": path, "start_line": 2, "end_line": 2, "force": true });
            let label = format!("forced md range {class}/{n} (coverage {alone} B alone)");
            let (v, _) = page(&ctx, &input, &path, &label).await;
            assert_eq!(v["coverage_omitted"], json!(true), "{label}: {v:.300}");
            assert!(v.get("coverage").is_none(), "{label}: {v:.300}");
        }
        assert!(
            in_window > 0,
            "{class}: no section count put coverage just under the limit alone"
        );
    }
}

/// A `json_path` (or `toml_key`) is the caller's own input and has no length of its own. It was
/// echoed whole as `path` (or `breadcrumb`) and inside the `hint` of the `file_id` arm, so a
/// path of several KB pushed a response that already carried a `file_id` over the limit, and
/// `call_content` buffered it again under `@tool_*`: measured before, a 6 KB YAML key made a
/// 12,287 B response with two handles. Each echo is clipped in ESCAPED bytes with a visible
/// marker; the routes name the handle, never the echo. A key of 40 segments of 290 B each makes
/// `breadcrumb` long though every entry fits: it is dropped, marked `breadcrumb_omitted`.
#[tokio::test]
async fn an_overlong_json_path_or_key_echo_keeps_one_handle() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let hint_route =
        regex::Regex::new(r#"read_file\("(@file_[0-9a-f]+)", start_line=N, end_line=M\)"#).unwrap();
    let mut resolved = BTreeSet::new();
    let mut clipped = 0;
    let segments: Vec<String> = (0..40)
        .map(|i| format!("s{i:02}{}", "q".repeat(287)))
        .collect();
    let deep = dir.path().join("deep.toml");
    std::fs::write(
        &deep,
        format!("{} = \"{}\"\n", segments.join("."), "a".repeat(12_000)),
    )
    .unwrap();
    let deep_case = json!({ "path": deep.to_str().unwrap(), "toml_key": segments.join(".") });
    let (v, _) = page(&ctx, &deep_case, "", "echo deep toml").await;
    assert_eq!(v["breadcrumb_omitted"], json!(true), "deep toml: {v:.300}");
    for (class, unit) in CLASSES7 {
        for key_bytes in [3_000usize, 6_000, 9_000, 12_000] {
            let key = one_line(unit, key_bytes);
            for (vshape, value) in [("tiny", "x".to_string()), ("wide", "a".repeat(12_000))] {
                let doc = json!({ key.clone(): value });
                let stem = format!("k-{class}-{key_bytes}-{vshape}");
                let json_file = dir.path().join(format!("{stem}.json"));
                std::fs::write(&json_file, doc.to_string()).unwrap();
                let yaml_file = dir.path().join(format!("{stem}.yaml"));
                std::fs::write(&yaml_file, format!("{key}: {value}\n")).unwrap();
                let toml_file = dir.path().join(format!("{stem}.toml"));
                std::fs::write(&toml_file, format!("{key} = \"{value}\"\n")).unwrap();
                let tool_ref = ctx.output_buffer.store_tool("probe", doc.to_string());
                let bracket = format!("$[{}]", serde_json::to_string(&key).unwrap());
                let cases = [
                    (
                        "json-dot",
                        json!({ "path": json_file.to_str().unwrap(), "json_path": format!("$.{key}") }),
                    ),
                    (
                        "json-bracket",
                        json!({ "path": json_file.to_str().unwrap(), "json_path": bracket }),
                    ),
                    (
                        "tool-dot",
                        json!({ "path": tool_ref, "json_path": format!("$.{key}") }),
                    ),
                    (
                        "tool-bracket",
                        json!({ "path": tool_ref, "json_path": bracket }),
                    ),
                    (
                        "yaml",
                        json!({ "path": yaml_file.to_str().unwrap(), "toml_key": key }),
                    ),
                    (
                        "toml",
                        json!({ "path": toml_file.to_str().unwrap(), "toml_key": key }),
                    ),
                ];
                for (fmt, input) in cases {
                    let label = format!("echo {fmt} {class}/{key_bytes}/{vshape}");
                    if ReadFile.call(input.clone(), &ctx).await.is_err() {
                        continue;
                    }
                    resolved.insert(format!("{fmt}/{class}"));
                    let input_ref = if fmt.starts_with("tool") {
                        tool_ref.as_str()
                    } else {
                        ""
                    };
                    let (v, _) = page(&ctx, &input, input_ref, &label).await;
                    let text = v.to_string();
                    // Every key here is over the clip, and every response echoes it at least once.
                    assert!(
                        text.contains("bytes shown; the rest is the value you passed"),
                        "{label}: the echo carries no marker: {text:.300}"
                    );
                    clipped += 1;
                    let Some(fid) = v["file_id"].as_str() else {
                        continue;
                    };
                    let hint = v["hint"].as_str().unwrap_or_default();
                    let c = hint_route.captures(hint).unwrap_or_else(|| {
                        panic!("{label}: the hint names no line route: {hint:.300}")
                    });
                    assert_eq!(&c[1], fid, "{label}: the hint names another handle");
                    let total = v["total_lines"].as_u64().unwrap();
                    let route = json!({ "path": fid, "start_line": 1, "end_line": total });
                    read_through(&ctx, route, fid, &label).await;
                }
            }
        }
    }
    eprintln!("overlong echo: resolved {resolved:?}");
    for must in [
        "json-dot/ascii",
        "json-bracket/ascii",
        "tool-dot/ascii",
        "yaml/ascii",
    ] {
        assert!(
            resolved.contains(must),
            "{must} never resolved: {resolved:?}"
        );
    }
    assert!(clipped > 0, "no echo was clipped");
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

/// The input `next` names, parsed back from its literal route. A `json_path` is quoted as a JSON
/// string literal; `None` when there is no `next`. Panics on a `next` that is not a line route.
fn next_input(v: &Value, label: &str) -> Option<Value> {
    let next = v["next"].as_str()?;
    let route = regex::Regex::new(
        r#"^read_file\("([^"]+)", (?:json_path=("(?:[^"\\]|\\.)*"), )?start_line=(\d+), end_line=(\d+)\)$"#,
    )
    .unwrap();
    let c = route
        .captures(next)
        .unwrap_or_else(|| panic!("{label}: next {next:?} is not a line route"));
    let mut input = json!({
        "path": &c[1],
        "start_line": c[3].parse::<u64>().unwrap(),
        "end_line": c[4].parse::<u64>().unwrap(),
    });
    if let Some(jp) = c.get(2) {
        input["json_path"] = json!(serde_json::from_str::<String>(jp.as_str()).unwrap());
    }
    Some(input)
}

/// One read through the REAL `call_content`, and the same input through `call` for its shape,
/// asserting that neither changed the set of live handles and that the text the agent sees names
/// no handle but the one it read. `base` is that handle without a `.err` suffix.
async fn read_minting_nothing(
    ctx: &crate::tools::ToolContext,
    input: &Value,
    base: &str,
    label: &str,
) -> Option<Value> {
    let before = ctx.output_buffer.handles();
    let blocks = ReadFile.call_content(input.clone(), ctx).await;
    assert_eq!(
        ctx.output_buffer.handles(),
        before,
        "{label}: call_content({input}) changed the buffer's handles"
    );
    // A refusal is an `Err` here (the server renders it); it must mint nothing either.
    let text = match &blocks {
        Ok(blocks) => crate::tools::hint_probe::primary_text(blocks),
        Err(e) => e.to_string(),
    };
    let mut named = handles_in(&text);
    named.remove(base);
    assert!(
        named.is_empty(),
        "{label}: the response names {named:?} beside {base}: {text:.300}"
    );
    let v = ReadFile.call(input.clone(), ctx).await.ok();
    assert_eq!(
        ctx.output_buffer.handles(),
        before,
        "{label}: call({input}) changed the buffer's handles"
    );
    assert_eq!(
        v.is_some(),
        blocks.is_ok(),
        "{label}: call and call_content disagree on success"
    );
    v
}

/// What a case must show it reached, so a guard over an arm it never entered cannot pass.
#[derive(Clone, Copy, Debug)]
enum Arm {
    /// Answered inline: content, no page fields.
    Inline,
    /// A first page of several: `complete: false` and a `next`.
    Paged,
    /// A line wider than the page, cut and marked.
    Clamped,
    /// Refused (`json_path` on a raw-text ref): a refusal must not mint either.
    Refused,
}

/// The lines one page of a buffer read covers, in the caller's numbering, and the last line the
/// read that produced it asked for. A page names its span in `shown_lines`; an inline answer
/// covers its whole range (or the whole text, unranged).
fn page_span(input: &Value, v: &Value) -> ((u64, u64), u64) {
    let content = v["content"].as_str().unwrap_or_default();
    let total = v["total_lines"]
        .as_u64()
        .unwrap_or(content.lines().count() as u64);
    let asked_last = input["end_line"].as_u64().map_or(total, |e| e.min(total));
    let span = match (v["shown_lines"][0].as_u64(), v["shown_lines"][1].as_u64()) {
        (Some(a), Some(b)) => (a, b),
        _ => (input["start_line"].as_u64().unwrap_or(1), asked_last),
    };
    (span, asked_last)
}

/// Follow `next` from `first` (the response to `input`) until there is none, every page through
/// [`read_minting_nothing`]. Asserts that each page resumes on the line after the last one shown
/// (no line skipped, none twice), that `complete` is true exactly when a page reached the last
/// line its read asked for and that `next` is there exactly when it is not, that every `next`
/// stays on `r` and keeps the `json_path`, and that the chain ends ON the last line `input`
/// asked for. Unless a line was cut, the pages joined are those lines of `space`, the line space
/// the ref is read in. Returns the pages followed after the first.
async fn follow_to_the_end(
    ctx: &crate::tools::ToolContext,
    input: &Value,
    first: Value,
    r: &str,
    space: &str,
    label: &str,
) -> usize {
    let base = r.strip_suffix(".err").unwrap_or(r);
    let (_, asked_last) = page_span(input, &first);
    let asked_first = input["start_line"].as_u64().unwrap_or(1);
    let (mut page_input, mut v) = (input.clone(), first);
    let (mut got, mut cut, mut last, mut pages) = (Vec::new(), false, asked_first - 1, 0);
    loop {
        assert!(
            v.get("file_id").is_none(),
            "{label}: names a file_id: {v:.300}"
        );
        let ((a, b), page_last) = page_span(&page_input, &v);
        assert_eq!(
            a,
            last + 1,
            "{label}: page {page_input} does not resume after line {last}"
        );
        assert!(b >= a, "{label}: page {page_input} shows no line: {v:.300}");
        last = b;
        if let Some(complete) = v["complete"].as_bool() {
            assert_eq!(
                complete,
                b >= page_last,
                "{label}: complete={complete} on a page ending at {b} of {page_last}: {v:.300}"
            );
            assert_eq!(
                v["next"].is_string(),
                !complete,
                "{label}: complete={complete} with next={}",
                v["next"]
            );
        }
        cut |= v["line_truncated"] == json!(true);
        got.push(v["content"].as_str().unwrap_or_default().to_string());
        let Some(next) = next_input(&v, label) else {
            break;
        };
        assert_eq!(next["path"], json!(r), "{label}: next leaves the ref");
        assert_eq!(
            next["json_path"], input["json_path"],
            "{label}: next drops the json_path"
        );
        v = read_minting_nothing(ctx, &next, base, label)
            .await
            .unwrap_or_else(|| panic!("{label}: following {next} failed"));
        page_input = next;
        pages += 1;
        assert!(pages < 5_000, "{label}: the next chain does not end");
    }
    assert_eq!(
        last, asked_last,
        "{label}: following next stopped at line {last} of {asked_last}"
    );
    if !cut {
        let want = crate::util::text::extract_lines(space, asked_first as usize, last as usize);
        assert!(
            got.join("\n") == want,
            "{label}: the pages joined are not lines {asked_first}-{last} of the ref"
        );
    }
    pages
}

/// R1: a read of an existing buffer refers to THAT handle and mints none. One handle holds the
/// whole buffer; every view of it (a page, a slice, a value a `json_path` extracts) is that
/// handle plus line numbers or a `json_path`. Measured before the fix on the live binary: a
/// ranged read of `@cmd_*` and a `json_path` read of `@tool_*` each minted a fresh `@file_*`
/// on every call, a paged buffer minted one per `next`, and the production pool holds 50.
///
/// Every source kind (`@cmd_*` stdout and `.err`, `@file_*`, `@tool_*`) is read in every arm
/// `read_from_buffer` has, through the REAL `call_content`, in a pool at production capacity
/// and FULL, so a mint would also evict: the guard compares the SET of handles, not a count.
/// Every `next` is followed to the end, each page under the same guard, and the chain must end
/// on the last line the first read asked for ([`follow_to_the_end`]): a whole read of 20,000
/// lines whose `next` named one page-sized window stopped at line 3,046.
#[tokio::test]
async fn no_read_of_an_existing_buffer_mints_a_handle() {
    let mut ctx = ctx().await;
    ctx.output_buffer = std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(50));

    let small = "line one\nline two\nline three".to_string();
    let big: String = (1..=800)
        .map(|i| format!("line {i:04} {}", "a".repeat(40)))
        .collect::<Vec<_>>()
        .join("\n");
    // One escape-heavy line far over the page (5,000 `\x01`, 30,000 B serialized), then a line.
    let wide = format!("{}\ntail line", "\u{1}".repeat(5_000));
    // Long enough that a page-sized `next` window fits inline and so ends the chain early, which
    // is how a whole read stopped at line 3,046 of 20,000 (short) and 178 (varied widths).
    let long_short: String = (1..=20_000)
        .map(|i| format!("l{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let varied = |n: usize| -> String {
        (1..=n)
            .map(|i| format!("{i}:{}", "x".repeat((i * 37) % 200)))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let long_varied = varied(20_000);
    // The kinds read through JSON (`@tool_*`, `json_path`) re-parse their whole payload on every
    // page; at 20,000 varied lines (2 MB) that is about 1,400 pages of 2 MB parses, minutes in a
    // debug build. They read 3,000 varied lines, past the page-sized window the defect needs.
    let varied_3k = varied(3_000);

    let buf = &ctx.output_buffer;
    // (label, ref, input, arm, the line space the ref is read in)
    let mut cases: Vec<(String, String, Value, Arm, std::rc::Rc<String>)> = Vec::new();
    // `through_json`: also read the body through a `@tool_*` envelope and a `json_path` value.
    for (shape, body, through_json) in [
        ("small", &small, true),
        ("big", &big, true),
        ("wide", &wide, true),
        ("long-short", &long_short, true),
        ("long-varied", &long_varied, false),
        ("varied-3k", &varied_3k, true),
    ] {
        let raw = std::rc::Rc::new(body.clone());
        let cmd = buf.store("probe".into(), body.clone(), String::new(), 0);
        let err = format!(
            "{}.err",
            buf.store("probe".into(), "x".into(), body.clone(), 1)
        );
        let file = buf.store_file_excerpt("probe.txt".into(), body.clone());
        let envelope = json!({ "stdout": body }).to_string();
        let tool = buf.store_tool("run_command", envelope.clone());
        let tool_lines = std::rc::Rc::new(crate::tools::output_buffer::line_addressable_text(
            &tool, envelope,
        ));
        for r in [&cmd, &err, &file, &tool] {
            if r == &tool && !through_json {
                continue;
            }
            let ranged = |s: u64, e: u64| json!({ "path": r, "start_line": s, "end_line": e });
            let rows: Vec<(Value, Arm)> = match shape {
                "small" => vec![
                    (json!({ "path": r }), Arm::Inline),
                    (ranged(1, 2), Arm::Inline),
                ],
                "big" => vec![
                    (json!({ "path": r }), Arm::Paged),
                    (ranged(1, 100_000), Arm::Paged),
                    (ranged(3, 700), Arm::Paged),
                ],
                "wide" => {
                    // A `@tool_*` envelope pretty-prints `{` on line 1: the wide line is line 2,
                    // so its whole read pages `{` alone first and reaches the cut on `next`.
                    let at = if r.starts_with("@tool_") { 2 } else { 1 };
                    let whole = if at == 2 { Arm::Paged } else { Arm::Clamped };
                    vec![
                        (ranged(at, at), Arm::Clamped),
                        (ranged(at, at + 1), Arm::Clamped),
                        (json!({ "path": r }), whole),
                    ]
                }
                _ => vec![(json!({ "path": r }), Arm::Paged)],
            };
            let space = if r == &tool { &tool_lines } else { &raw };
            for (input, arm) in rows {
                cases.push((format!("{shape} {r}"), r.clone(), input, arm, space.clone()));
            }
            if !r.starts_with("@tool_") {
                cases.push((
                    format!("{shape} {r} json_path"),
                    r.clone(),
                    json!({ "path": r, "json_path": "$.v" }),
                    Arm::Refused,
                    raw.clone(),
                ));
            }
        }
        // `json_path` into a `@tool_*` ref: a string value (its own lines) and an array (its
        // pretty-printed lines).
        let lines: Vec<&str> = body.lines().collect();
        let values = if through_json {
            vec![json!(body), json!(lines)]
        } else {
            vec![]
        };
        for value in values {
            let t = buf.store_tool("probe", json!({ "v": value }).to_string());
            let space = std::rc::Rc::new(match &value {
                Value::String(s) => s.clone(),
                other => serde_json::to_string_pretty(other).unwrap(),
            });
            let jp = |extra: Value| {
                let mut v = json!({ "path": t, "json_path": "$.v" });
                for (k, x) in extra.as_object().unwrap() {
                    v[k] = x.clone();
                }
                v
            };
            let kind = if value.is_string() { "string" } else { "array" };
            let rows: Vec<(Value, Arm)> = match (shape, kind) {
                ("small", _) => vec![
                    (jp(json!({})), Arm::Inline),
                    (jp(json!({ "start_line": 2, "end_line": 3 })), Arm::Inline),
                ],
                ("big", _) => vec![
                    (jp(json!({})), Arm::Paged),
                    (
                        jp(json!({ "start_line": 1, "end_line": 100_000 })),
                        Arm::Paged,
                    ),
                    (jp(json!({ "start_line": 5, "end_line": 600 })), Arm::Paged),
                ],
                // The wide string value's first line is the wide one; in the array it is line 2.
                ("wide", "string") => vec![
                    (jp(json!({})), Arm::Clamped),
                    (jp(json!({ "start_line": 1, "end_line": 1 })), Arm::Clamped),
                    (jp(json!({ "start_line": 1, "end_line": 2 })), Arm::Clamped),
                ],
                ("wide", _) => vec![(jp(json!({ "start_line": 2, "end_line": 3 })), Arm::Clamped)],
                _ => vec![(jp(json!({})), Arm::Paged)],
            };
            for (input, arm) in rows {
                cases.push((
                    format!("{shape} {t} json_path {kind}"),
                    t.clone(),
                    input,
                    arm,
                    space.clone(),
                ));
            }
        }
    }
    // Fill the pool: from here every mint evicts the least-recently-used entry.
    while buf.handles().len() < 50 {
        buf.store("filler".into(), "f".into(), String::new(), 0);
    }
    assert_eq!(buf.handles().len(), 50, "fixture: the pool is not full");
    for (_, r, ..) in &cases {
        let base = r.strip_suffix(".err").unwrap_or(r);
        assert!(
            buf.handles().iter().any(|h| h == base),
            "fixture: {r} was evicted before it was read"
        );
    }

    let mut pages = 0;
    for (label, r, input, arm, space) in &cases {
        let base = r.strip_suffix(".err").unwrap_or(r);
        let label = format!("{label} {arm:?} {input}");
        let v = read_minting_nothing(&ctx, input, base, &label).await;
        match arm {
            Arm::Refused => {
                assert!(v.is_none(), "{label}: must be refused");
                continue;
            }
            Arm::Inline => {
                let v = v.as_ref().unwrap();
                assert!(
                    v["content"].is_string() && v.get("shown_lines").is_none(),
                    "{label}: not the inline arm: {v:.300}"
                );
            }
            Arm::Paged => {
                let v = v.as_ref().unwrap();
                assert!(
                    v["complete"] == json!(false) && v["next"].is_string(),
                    "{label}: not the paged arm: {v:.300}"
                );
            }
            Arm::Clamped => {
                let v = v.as_ref().unwrap();
                assert_eq!(
                    v["line_truncated"],
                    json!(true),
                    "{label}: not clamped: {v:.300}"
                );
            }
        }
        let v = v.unwrap();
        if input["json_path"].is_string() {
            // The same value, numbered the same way, on every read.
            let again = ReadFile.call(input.clone(), &ctx).await.unwrap();
            assert_eq!(again, v, "{label}: two reads of one value differ");
        }
        pages += follow_to_the_end(&ctx, input, v, r, space, &label).await;
    }
    eprintln!(
        "no-mint guard: {} cases, {pages} pages followed",
        cases.len()
    );
}

/// A `json_path` value paged to its end through the one handle: the pages, joined, are the
/// value — nothing skipped, nothing twice — for a short path and for paths over `INPUT_ECHO_CLIP`
/// (whose `path` echo is clipped but whose `next` quotes them whole) and over the route floor
/// (whose `next` is prose). Both a whole read and a range to the end are followed with `next`
/// alone, and both must end on the value's last line.
#[tokio::test]
async fn a_json_path_value_paged_to_its_end_reassembles_the_value() {
    let ctx = ctx().await;
    let value: String = (1..=600)
        .map(|i| format!("row {i:04} \"q\" \\ {}", "v".repeat(30)))
        .collect::<Vec<_>>()
        .join("\n");
    for key in [
        "abcd".to_string(),
        "k".repeat(296),
        "k".repeat(3_000),
        "k".repeat(12_000),
    ] {
        let tool = ctx
            .output_buffer
            .store_tool("probe", json!({ key.clone(): value }).to_string());
        let jp = format!("$[{}]", serde_json::to_string(&key).unwrap());
        for first in [
            json!({ "path": tool, "json_path": jp }),
            json!({ "path": tool, "json_path": jp, "start_line": 1, "end_line": 100_000 }),
        ] {
            let label = format!(
                "reassemble {} B path, ranged={}",
                jp.len(),
                first["start_line"]
            );
            let (got, nexts, last, total) = follow_json_path(&ctx, &tool, &jp, first, &label).await;
            assert!(nexts.len() > 1, "{label}: the value was never paged");
            assert_eq!(
                (last, total),
                (600, 600),
                "{label}: did not reach the last line"
            );
            assert!(
                got.join("\n") == value,
                "{label}: the pages are not the value"
            );
        }
    }
}

/// Follow a `json_path` read of `tool` until there is no `next`. A route-shaped `next` is
/// executed EXACTLY as written, its `json_path` parsed back from the literal and required to be
/// `jp`; a prose `next` must name `tool` and both line numbers, and is followed with the caller's
/// own `jp`. Every page goes through [`read_minting_nothing`] and must resume on the line after
/// the last one shown. Returns the pages' contents, every `next` emitted, and the last line
/// reached with the value's `total_lines`.
async fn follow_json_path(
    ctx: &crate::tools::ToolContext,
    tool: &str,
    jp: &str,
    first: Value,
    label: &str,
) -> (Vec<String>, Vec<String>, u64, u64) {
    let prose = regex::Regex::new(
        r"^call read_file on (@tool_[0-9a-f]+) again with the json_path you passed and start_line=(\d+), end_line=(\d+)$",
    )
    .unwrap();
    let (mut input, mut got, mut nexts) = (first, Vec::new(), Vec::new());
    let mut last = input["start_line"].as_u64().unwrap_or(1) - 1;
    let mut total = 0;
    for _ in 0..1_000 {
        let v = read_minting_nothing(ctx, &input, tool, label)
            .await
            .unwrap_or_else(|| panic!("{label}: {input:.300} failed"));
        assert!(v.get("file_id").is_none(), "{label}: {v:.300}");
        let ((a, b), _) = page_span(&input, &v);
        assert_eq!(
            a,
            last + 1,
            "{label}: a page does not resume after line {last}"
        );
        last = b;
        total = v["total_lines"].as_u64().unwrap_or(total);
        got.push(v["content"].as_str().unwrap().to_string());
        let Some(next) = v["next"].as_str() else {
            return (got, nexts, last, total);
        };
        nexts.push(next.to_string());
        input = if next.starts_with("read_file(") {
            let n = next_input(&v, label).unwrap();
            assert_eq!(n["path"], json!(tool), "{label}: next leaves the ref");
            assert_eq!(
                n["json_path"],
                json!(jp),
                "{label}: the route is not the path given"
            );
            n
        } else {
            let c = prose.captures(next).unwrap_or_else(|| {
                panic!("{label}: next is neither a route nor the prose: {next:.300}")
            });
            assert_eq!(&c[1], tool, "{label}: the prose names another handle");
            json!({
                "path": tool,
                "json_path": jp,
                "start_line": c[2].parse::<u64>().unwrap(),
                "end_line": c[3].parse::<u64>().unwrap(),
            })
        };
    }
    panic!("{label}: the next chain does not end");
}

/// The `next` of a `json_path` page quotes the caller's path WHOLE, as a route that runs exactly
/// as written, while that leaves the page `JSON_PATH_ROUTE_ROOM_FLOOR` of room; past the floor it
/// states the continuation in prose with the line numbers. It used to name a placeholder for any
/// path over `INPUT_ECHO_CLIP` (`json_path=<your json_path: 3005 bytes, not repeated here>`),
/// which failed when copied: `path segment '<your json_path: …' not found`. No string a caller
/// would copy may carry a `<…>` placeholder or a byte count now. Paths of 9, 301, 3,005 and
/// 12,005 B, and a sweep that crosses the floor.
#[tokio::test]
async fn a_json_path_next_is_a_whole_route_or_plain_prose() {
    let ctx = ctx().await;
    let value: String = (1..=600)
        .map(|i| format!("row {i:04} {}", "v".repeat(40)))
        .collect::<Vec<_>>()
        .join("\n");
    let mut kinds = Vec::new();
    let sweep = (5_000..=9_000).step_by(200);
    for key_len in [4usize, 296, 3_000, 12_000].into_iter().chain(sweep) {
        let key = "k".repeat(key_len);
        let tool = ctx
            .output_buffer
            .store_tool("probe", json!({ key.clone(): value }).to_string());
        let jp = format!("$[{}]", serde_json::to_string(&key).unwrap());
        let label = format!("{} B path", jp.len());
        let first = json!({ "path": tool, "json_path": jp });
        let text = crate::tools::hint_probe::primary_text(
            &ReadFile.call_content(first.clone(), &ctx).await.unwrap(),
        );
        let (_, nexts, last, total) = follow_json_path(&ctx, &tool, &jp, first, &label).await;
        assert_eq!(last, total, "{label}: did not reach the last line");
        for n in nexts.iter().chain([&text]) {
            assert!(
                !n.contains("<your") && !n.contains("not repeated here"),
                "{label}: a placeholder a caller would copy: {n:.300}"
            );
        }
        for n in nexts.iter().filter(|n| !n.starts_with("read_file(")) {
            assert!(
                n.contains("with the json_path you passed")
                    && !n.contains('<')
                    && !n.contains("bytes"),
                "{label}: the prose continuation is not the plain sentence: {n:.300}"
            );
        }
        // Classified by the first page's `next`: a range page prices its route a byte or two
        // differently from the whole-read page, so a path within that of the floor may get one
        // kind on page one and the other on page two. Both are followed above either way.
        assert!(!nexts.is_empty(), "{label}: the value was never paged");
        kinds.push((jp.len(), nexts[0].starts_with("read_file(")));
    }
    eprintln!("route (true) or prose (false) by path length: {kinds:?}");
    for (len, route) in &kinds[..3] {
        assert!(route, "a {len} B path must be quoted whole in a route");
    }
    assert!(
        !kinds[3].1,
        "a 12,005 B path cannot be quoted inside the limit"
    );
    let swept = &kinds[4..];
    let first_prose = swept.iter().position(|(_, r)| !r);
    assert!(
        first_prose.is_some_and(|i| i > 0 && swept[i..].iter().all(|(_, r)| !r)),
        "the sweep must cross the floor once, routes below it and prose above: {swept:?}"
    );
}

/// `json_path` with a line range slices the VALUE's lines: a string's own lines, or a non-string's
/// pretty-printed JSON. It used to ignore the range without a word, and answered with the whole
/// value (or a fresh `@file_*` handle for it). Without a range, an inline value answers exactly as
/// before: `content`, `path`, `value_type`, `format` (and `count` for a container), no
/// `total_lines`. A range that cannot be one is refused, as on any other buffer read.
#[tokio::test]
async fn a_json_path_range_slices_the_value_and_an_unranged_inline_read_is_unchanged() {
    let ctx = ctx().await;
    let keys = |v: &Value| -> Vec<String> { v.as_object().unwrap().keys().cloned().collect() };
    let s = ctx.output_buffer.store_tool(
        "probe",
        json!({ "v": "line one\nline two\nline three" }).to_string(),
    );
    let a = ctx
        .output_buffer
        .store_tool("probe", json!({ "v": ["a", "b", "c"] }).to_string());
    let read = |input: Value| {
        let ctx = &ctx;
        async move { ReadFile.call(input, ctx).await }
    };

    let whole = read(json!({ "path": s, "json_path": "$.v" }))
        .await
        .unwrap();
    assert_eq!(whole["content"], json!("line one\nline two\nline three"));
    assert_eq!(keys(&whole), ["content", "path", "value_type", "format"]);
    let whole = read(json!({ "path": a, "json_path": "$.v" }))
        .await
        .unwrap();
    assert_eq!(
        keys(&whole),
        ["content", "path", "value_type", "format", "count"]
    );

    let slice = read(json!({ "path": s, "json_path": "$.v", "start_line": 2, "end_line": 3 }))
        .await
        .unwrap();
    assert_eq!(slice["content"], json!("line two\nline three"), "{slice}");
    assert_eq!(slice["total_lines"], json!(3), "{slice}");
    assert_eq!(slice["value_type"], json!("string"), "{slice}");
    // `start_line` alone is a 50-line window, as on every other read.
    let alone = read(json!({ "path": s, "json_path": "$.v", "start_line": 2 }))
        .await
        .unwrap();
    assert_eq!(alone["content"], json!("line two\nline three"), "{alone}");

    let slice = read(json!({ "path": a, "json_path": "$.v", "start_line": 2, "end_line": 3 }))
        .await
        .unwrap();
    assert_eq!(slice["content"], json!("  \"a\",\n  \"b\","), "{slice}");
    assert_eq!(slice["total_lines"], json!(5), "{slice}");
    assert_eq!(slice["count"], json!(3), "{slice}");

    let err = read(json!({ "path": s, "json_path": "$.v", "start_line": 3, "end_line": 2 }))
        .await
        .expect_err("an inverted range beside json_path must be refused, not ignored");
    assert!(err.to_string().contains("invalid line range"), "{err}");
}

/// `total_lines` of a buffer, or of a value, whose text ends in a newline counts its lines, not
/// the empty segment after the last one (`str::lines`, not `split('\n')`). A real command's
/// stdout almost always ends in a newline, while every other fixture here is built with
/// `join("\n")` and does not. (From the 2026-10-07 review of this branch, mutant M5.)
#[tokio::test]
async fn total_lines_of_a_buffer_ending_in_a_newline_counts_its_lines() {
    let ctx = ctx().await;
    let body: String = (1..=800)
        .map(|i| format!("line {i:04} {}\n", "a".repeat(40)))
        .collect();
    let cmd = ctx
        .output_buffer
        .store("p".into(), body.clone(), String::new(), 0);
    let t = ctx
        .output_buffer
        .store_tool("probe", json!({ "v": body }).to_string());
    for input in [
        json!({ "path": cmd }),
        json!({ "path": cmd, "start_line": 3, "end_line": 700 }),
        json!({ "path": cmd, "start_line": 3, "end_line": 5 }),
        json!({ "path": t, "json_path": "$.v" }),
        json!({ "path": t, "json_path": "$.v", "start_line": 1, "end_line": 5 }),
    ] {
        let v = ReadFile.call(input.clone(), &ctx).await.unwrap();
        assert_eq!(v["total_lines"], json!(800), "{input}: {v:.200}");
    }
}

/// A clamped line keeps the page's continuation honest. A one-line range whose line is cut
/// showed every line it asked for, so it is `complete` with no `next`; a two-line range whose
/// first line is cut stopped short, so it is not complete and its `next` resumes at line 2.
/// `complete: false` with no `next` is a dead end, and `complete: false` on a range that was
/// shown in full sends the caller after lines that do not exist. (Review mutants M6, M15.)
#[tokio::test]
async fn a_clamped_line_keeps_complete_and_next_consistent() {
    let ctx = ctx().await;
    let wide = format!("{}\ntail line", "\u{1}".repeat(5_000));
    let cmd = ctx
        .output_buffer
        .store("p".into(), wide.clone(), String::new(), 0);
    let t = ctx
        .output_buffer
        .store_tool("probe", json!({ "v": wide }).to_string());
    for (r, jp) in [(&cmd, None), (&t, Some("$.v"))] {
        let mk = |s: u64, e: u64| {
            let mut v = json!({ "path": r, "start_line": s, "end_line": e });
            if let Some(jp) = jp {
                v["json_path"] = json!(jp);
            }
            v
        };
        let one = ReadFile.call(mk(1, 1), &ctx).await.unwrap();
        assert_eq!(one["line_truncated"], json!(true), "{one:.200}");
        assert_eq!(one["complete"], json!(true), "{r} {jp:?}: {one:.300}");
        assert!(one.get("next").is_none(), "{r} {jp:?}: {one:.300}");
        let two = ReadFile.call(mk(1, 2), &ctx).await.unwrap();
        assert_eq!(two["line_truncated"], json!(true));
        assert_eq!(two["complete"], json!(false));
        let next = two["next"].as_str().unwrap_or_else(|| {
            panic!("{r} {jp:?}: a cut page that stops short has no next: {two:.300}")
        });
        assert!(next.contains("start_line=2, end_line=2"), "{next}");
    }
}

/// Every return shape of a `json_path` read carries the keys that say what it answers: a ranged
/// or paged value still echoes its `path`, `value_type` and `format`. (Review mutant M16.)
#[tokio::test]
async fn every_json_path_return_shape_echoes_its_path() {
    let ctx = ctx().await;
    let big: String = (1..=800)
        .map(|i| format!("line {i:04} {}", "a".repeat(40)))
        .collect::<Vec<_>>()
        .join("\n");
    let t = ctx
        .output_buffer
        .store_tool("probe", json!({ "v": big }).to_string());
    for input in [
        json!({ "path": t, "json_path": "$.v" }),
        json!({ "path": t, "json_path": "$.v", "start_line": 2, "end_line": 3 }),
        json!({ "path": t, "json_path": "$.v", "start_line": 2, "end_line": 700 }),
    ] {
        let v = ReadFile.call(input.clone(), &ctx).await.unwrap();
        assert_eq!(v["path"], json!("$.v"), "{input}: {v:.200}");
        assert_eq!(v["value_type"], json!("string"), "{input}");
        assert_eq!(v["format"], json!("json"), "{input}");
    }
}

/// `start_line=0` on a buffer ref is refused, as on a real file: lines are 1-indexed, and a
/// zero read as "from the top" would answer a question nobody asked. (Review mutant M19.)
#[tokio::test]
async fn start_line_zero_on_a_buffer_is_refused() {
    let ctx = ctx().await;
    let cmd = ctx
        .output_buffer
        .store("p".into(), "a\nb\nc".into(), String::new(), 0);
    let t = ctx
        .output_buffer
        .store_tool("probe", json!({ "v": "a\nb\nc" }).to_string());
    for input in [
        json!({ "path": cmd, "start_line": 0, "end_line": 2 }),
        json!({ "path": t, "json_path": "$.v", "start_line": 0, "end_line": 2 }),
    ] {
        let err = ReadFile
            .call(input.clone(), &ctx)
            .await
            .expect_err("start_line=0 must be refused");
        assert!(
            err.to_string().contains("invalid line range"),
            "{input}: {err}"
        );
    }
}

/// A page is sized against the WIDEST `next` it can carry. A `next` resuming at `s + 1` is one
/// digit wider than one at `s` when `s + 1` gains a digit, so a skeleton priced with the
/// narrower route admits a cut page one byte over the limit, which `call_content` then buffers
/// under a second handle: measured under review mutant M13, 10,004 B at `start_line=9`. The
/// cut fills the page to the byte, at every digit boundary.
#[tokio::test]
async fn a_clamped_page_at_a_digit_boundary_of_next_fits() {
    let ctx = ctx().await;
    for s in [9usize, 99, 999, 9_999, 99_999] {
        let mut lines: Vec<String> = (1..s).map(|_| "x".to_string()).collect();
        lines.push("a".repeat(30_000));
        lines.push("tail".into());
        let body = lines.join("\n");
        let cmd = ctx
            .output_buffer
            .store("p".into(), body.clone(), String::new(), 0);
        let t = ctx
            .output_buffer
            .store_tool("probe", json!({ "v": body }).to_string());
        for (input, base) in [
            (
                json!({ "path": cmd, "start_line": s, "end_line": s + 1 }),
                &cmd,
            ),
            (
                json!({ "path": t, "json_path": "$.v", "start_line": s, "end_line": s + 1 }),
                &t,
            ),
        ] {
            let label = format!("digit boundary {input}");
            let v = read_minting_nothing(&ctx, &input, base, &label)
                .await
                .unwrap();
            assert_eq!(v["line_truncated"], json!(true), "{label}");
            assert!(
                !crate::tools::exceeds_inline_limit_len(v.to_string().len()),
                "{label}: {} B is over the inline limit",
                v.to_string().len()
            );
        }
    }
}

/// `end_line` without `start_line` on a buffer ref is refused with the real-file rule's own
/// words (`validate_read_nav_params`), not answered with the whole buffer as if it had not been
/// passed. A `start_line` that is not a non-negative integer reads as absent on both kinds of
/// read, so `start_line=-1` beside an end is refused the same way, and `start_line=-1` alone
/// reads the whole text on both: the buffer arm mirrors the real-file arm, no more.
#[tokio::test]
async fn end_line_without_start_line_on_a_buffer_is_refused_as_on_a_real_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    std::fs::write(dir.path().join("f.txt"), "a\nb\nc").unwrap();
    let mut ctx = ctx().await;
    ctx.agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let real = dir.path().join("f.txt").to_string_lossy().into_owned();
    let cmd = ctx
        .output_buffer
        .store("p".into(), "a\nb\nc".into(), String::new(), 0);
    let t = ctx
        .output_buffer
        .store_tool("probe", json!({ "v": "a\nb\nc" }).to_string());
    for path in [&real, &cmd, &t] {
        let jp = |mut v: Value| {
            if path.starts_with("@tool_") {
                v["json_path"] = json!("$.v");
            }
            v
        };
        for refused in [
            jp(json!({ "path": path, "end_line": 2 })),
            jp(json!({ "path": path, "start_line": -1, "end_line": 2 })),
        ] {
            if path == &real && refused.get("json_path").is_some() {
                continue;
            }
            let err = ReadFile
                .call(refused.clone(), &ctx)
                .await
                .expect_err("an end without a start must be refused");
            assert!(
                err.to_string()
                    .contains("end_line provided without start_line"),
                "{refused}: {err}"
            );
        }
        let whole = ReadFile
            .call(jp(json!({ "path": path, "start_line": -1 })), &ctx)
            .await
            .unwrap();
        assert_eq!(whole["content"], json!("a\nb\nc"), "{path}: {whole}");
    }
}

/// The Text form says how many lines a page SHOWS from `shown_lines`, not by counting the
/// content's lines: a cut line ends in a marker on its own line, and the count read
/// `[2 of 2 lines shown]` beside `shown_lines: [1, 1]` and a `next` resuming at line 2.
#[tokio::test]
async fn the_text_form_counts_the_lines_a_page_shows_not_the_cut_marker() {
    let ctx = ctx().await;
    let wide = format!("{}\ntail line", "\u{1}".repeat(5_000));
    let cmd = ctx
        .output_buffer
        .store("p".into(), wide.clone(), String::new(), 0);
    let value: String = (1..=600)
        .map(|i| format!("row {i:04} {}", "v".repeat(40)))
        .collect::<Vec<_>>()
        .join("\n");
    let t = ctx
        .output_buffer
        .store_tool("probe", json!({ "v": value, "w": wide }).to_string());
    for input in [
        json!({ "path": cmd, "start_line": 1, "end_line": 2 }),
        json!({ "path": t, "json_path": "$.w", "start_line": 1, "end_line": 2 }),
        json!({ "path": t, "json_path": "$.v", "start_line": 100, "end_line": 600 }),
    ] {
        let v = ReadFile.call(input.clone(), &ctx).await.unwrap();
        let (a, b) = (
            v["shown_lines"][0].as_u64().unwrap(),
            v["shown_lines"][1].as_u64().unwrap(),
        );
        let text = crate::tools::hint_probe::primary_text(
            &ReadFile.call_content(input.clone(), &ctx).await.unwrap(),
        );
        let want = format!("[{} of {} lines shown]", b - a + 1, v["total_lines"]);
        assert!(
            text.contains(&want),
            "{input}: want {want:?} in {text:.300}"
        );
    }
}

/// A clamped line of a `@tool_*` ref is offered a field route to the payload's largest field
/// only while the `$.key` path fits the echo clip (`INPUT_ECHO_CLIP`, 300 B) — at EXACTLY 300 B
/// it is offered and following it works; at 301 B it is not (the `grep -o` route stays). From
/// review M (`rm_k_largest_field_path_is_offered_at_exactly_the_echo_clip`, mutant M1: the
/// bound's `<=` made `<`, which survived the suite).
#[tokio::test]
async fn a_largest_field_route_is_offered_at_exactly_the_echo_clip() {
    let ctx = ctx().await;
    let wide = "W".repeat(20_000);
    for (klen, offered) in [(298usize, true), (299, false)] {
        let key = "k".repeat(klen);
        let t = ctx
            .output_buffer
            .store_tool("edge", json!({ key.clone(): wide, "s": 1 }).to_string());
        // Pretty-printed: line 1 is "{", line 2 the wide field ("k…" sorts before "s").
        let v = ReadFile
            .call(json!({ "path": t, "start_line": 2, "end_line": 2 }), &ctx)
            .await
            .unwrap();
        assert_eq!(v["line_truncated"], json!(true), "fixture: {v:.200}");
        let hint = v["hint"].as_str().unwrap();
        let route = format!("json_path=\"$.{key}\"");
        assert_eq!(hint.contains(&route), offered, "klen {klen}: {hint:.200}");
        if offered {
            let r = ReadFile
                .call(json!({ "path": t, "json_path": format!("$.{key}") }), &ctx)
                .await;
            assert!(r.is_ok(), "the offered field route failed: {r:?}");
        }
    }
}
