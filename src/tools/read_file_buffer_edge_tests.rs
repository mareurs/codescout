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
        Ref::File => ctx.output_buffer.store_file("@probe.txt".into(), text),
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
/// minted `@tool_` at 10,004 B. A response over the limit is a page of the value on the file's
/// own path, which mints no handle (it was a `file_id` snapshot until ADR 2026-10-08's "Revisit
/// when" item 2 was built), and whose `next` chain reads the value to its end.
#[tokio::test]
async fn a_json_path_or_toml_key_read_of_a_real_file_keeps_one_handle() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let json_text = |v: &Value| v.to_string();
    // A JSON string or array literal is a valid TOML basic string or inline array (`\uXXXX`,
    // `\"`, `\\`). The `toml` serializer overflows on these values in a debug build.
    let toml_text = |v: &Value| format!("k = {}\n", v["k"]);
    // A JSON string or array literal is also a YAML double-quoted scalar or flow sequence
    // (`\uXXXX`, `\"`, `\\`): a YAML key read takes the same `value_of_a_file` as TOML.
    let yaml_text = |v: &Value| format!("k: {}\n", v["k"]);
    let mut paged = 0;
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
                    assert!(v.get("file_id").is_none(), "{label}: {v:.300}");
                    if v.get("shown_lines").is_none() {
                        assert!(v.get("content").is_some(), "{label}: no content: {v:.300}");
                        continue;
                    }
                    paged += 1;
                    let path = p.to_str().unwrap();
                    let (nav_name, key) = nav.as_object().unwrap().iter().next().unwrap();
                    let (_, _, last, total) = follow_value(
                        &ctx,
                        path,
                        nav_name,
                        key.as_str().unwrap(),
                        input.clone(),
                        &label,
                    )
                    .await;
                    assert_eq!(last, total, "{label}: did not reach the last line");
                }
            }
        }
    }
    assert!(paged > 0, "no read took the paged arm");
}

/// `siblings` of a `toml_key` read has no cap, in count or in width. A read whose
/// page would be over the limit with about 16 KB of sibling names beside it drops them,
/// marked `siblings_omitted`, so the page is not buffered again under `@tool_*`.
#[tokio::test]
async fn a_toml_key_read_with_many_siblings_drops_them_from_the_page() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    // Siblings are not capped, in count or in width: 40 tables with 400-byte
    // names put about 16 KB of them beside the value.
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
        assert!(
            v.get("shown_lines").is_some(),
            "{label}: not a page: {v:.300}"
        );
        assert!(v.get("file_id").is_none(), "{label}: {v:.300}");
    }
}
/// A page of a `toml_key` value drops `siblings` when the page WITH them is over the
/// limit, measured on that whole page skeleton. Siblings that fit ALONE but not beside the page's
/// other keys (`shown_lines`, `next`, the clamp `hint`, `line_range`, ...) are the case a measure
/// of `siblings` by itself misses: kept, `call_content` buffered the response under a second
/// handle. The sweep puts the sibling list just under the limit on its own; it is ASCII because
/// bare TOML keys are, and the dimension it varies is the other keys, not escaping.
#[tokio::test]
async fn a_toml_key_read_whose_siblings_fit_alone_but_not_beside_the_page_drops_them() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    // The limit a response is judged by: over it, `call_content` buffers it under `@tool_*`.
    let limit = crate::tools::INLINE_MAX_RESPONSE_LEN;
    // Over the limit alone, so the read is paged, where `siblings` is droppable.
    let value = "a".repeat(12_000);
    let mut in_window = 0;
    for w in 200..=300 {
        let siblings: String = (0..40)
            .map(|i| format!("[s{i:02}{}]\nx = 1\n", "n".repeat(w)))
            .collect();
        let text = format!("[k]\nv = \"{value}\"\n{siblings}");
        let extracted = crate::tools::file_summary::extract_toml_key(&text, "k").unwrap();
        let alone = json!(extracted.siblings).to_string().len();
        // Fits alone (10 B of margin), by less than the page's other keys need beside it (they
        // are well over 150 B: the clamp `hint` alone names the file twice).
        if !(limit - 150..=limit - 10).contains(&alone) {
            continue;
        }
        in_window += 1;
        let p = dir.path().join(format!("sib-alone-{w}.toml"));
        std::fs::write(&p, &text).unwrap();
        let input = json!({ "path": p.to_str().unwrap(), "toml_key": "k" });
        let label = format!("siblings of width {w} ({alone} B alone)");
        let (v, _) = page(&ctx, &input, "", &label).await;
        assert!(
            v.get("shown_lines").is_some(),
            "{label}: not a page: {v:.300}"
        );
        assert_eq!(v["siblings_omitted"], json!(true), "{label}: {v:.300}");
    }
    assert!(
        in_window > 0,
        "no width put the sibling list just under the limit alone"
    );
}

/// The other half of the droppable rule a page applies to `siblings` (the rule
/// `page_beside_coverage` applies to `coverage`): siblings that FIT beside a page are dropped
/// only when they would cost its first line whole and the line shows whole without them, and are
/// kept otherwise. YAML, whose `toml_key` value starts on the key's own line, so that line is the
/// page's first: about 6 KB of sibling keys beside a 5 KB first line, and beside 600 short ones.
#[tokio::test]
async fn a_toml_key_page_keeps_siblings_unless_they_cost_its_first_line() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let siblings: String = (0..40)
        .map(|i| format!("s{i:02}{}: 1\n", "n".repeat(150)))
        .collect();
    let rows: String = rows().iter().map(|r| format!("  - {r}\n")).collect();
    for (shape, value, dropped) in [
        ("wide", format!(" {}\n", "a".repeat(5_000)), true),
        ("rows", format!("\n{rows}"), false),
    ] {
        let p = dir.path().join(format!("first-{shape}.yaml"));
        std::fs::write(&p, format!("k:{value}{siblings}")).unwrap();
        let path = p.to_str().unwrap();
        let input = json!({ "path": path, "toml_key": "k" });
        let label = format!("first line {shape}");
        let v = read_minting_nothing(&ctx, &input, "", &label)
            .await
            .unwrap();
        assert!(
            v.get("shown_lines").is_some(),
            "{label}: not a page: {v:.300}"
        );
        assert_eq!(
            v["siblings_omitted"] == json!(true),
            dropped,
            "{label}: {v:.300}"
        );
        assert_eq!(v.get("siblings").is_some(), !dropped, "{label}: {v:.300}");
        assert!(
            v.get("line_truncated").is_none(),
            "{label}: the first line was cut: {v:.300}"
        );
        follow_value(&ctx, path, "toml_key", "k", input, &label).await;
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
/// marker; a page's `next` quotes the key whole or says it in prose, never the echo. A key of
/// 40 segments of 290 B each makes `breadcrumb` long though every entry fits: it is dropped,
/// marked `breadcrumb_omitted`.
#[tokio::test]
async fn an_overlong_json_path_or_key_echo_keeps_one_handle() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let mut resolved = BTreeSet::new();
    let (mut clipped, mut followed) = (0, 0);
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
                    if v.get("shown_lines").is_none() || !input_ref.is_empty() {
                        continue;
                    }
                    // A real file's value over one page is paged on the file's path: every
                    // `next`, a route quoting the whole key or prose past the floor, is followed
                    // to the end, minting nothing.
                    let path = input["path"].as_str().unwrap();
                    let (nav, key) = if let Some(jp) = input["json_path"].as_str() {
                        ("json_path", jp)
                    } else {
                        ("toml_key", input["toml_key"].as_str().unwrap())
                    };
                    let (_, _, last, total) =
                        follow_value(&ctx, path, nav, key, input.clone(), &label).await;
                    assert_eq!(last, total, "{label}: did not reach the last line");
                    followed += 1;
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
    assert!(followed > 0, "no real file's value was paged and followed");
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
        let file = buf.store_file("@probe.txt".into(), body.clone());
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

// ---------------------------------------------------------------------------------------------
// A `json_path` or `toml_key` value of a REAL file is paged on the file's own path and mints no
// handle (ADR 2026-10-08, "Revisit when" item 2). It used to be stored as a snapshot under a
// `@file_*` handle beside the file's own (`OutputBuffer::store_file_excerpt`, now deleted) and
// browsed by line range there: a second handle for one file.
// ---------------------------------------------------------------------------------------------

/// A context over a pool at the PRODUCTION capacity (50, `src/server.rs`), so a mint evicts.
async fn ctx50() -> crate::tools::ToolContext {
    let mut ctx = ctx().await;
    ctx.output_buffer = std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(50));
    ctx
}

/// Fill `ctx`'s pool to its capacity of 50: from here every mint evicts the least recently used
/// entry, so a guard on the SET of handles sees it even though the count holds.
fn fill_pool(ctx: &crate::tools::ToolContext) {
    while ctx.output_buffer.handles().len() < 50 {
        ctx.output_buffer
            .store("filler".into(), "f".into(), String::new(), 0);
    }
    assert_eq!(ctx.output_buffer.handles().len(), 50, "fixture: pool full");
}

/// Follow a `json_path`/`toml_key` read of the REAL file `path` until there is no `next`. A
/// route-shaped `next` is executed EXACTLY as written: its path must be `path`, its navigation
/// parameter `nav`, and its literal must parse back to `key`. A prose `next` must name `path`,
/// `nav` and both line numbers, and is followed with the caller's own `key`. Every page goes
/// through [`read_minting_nothing`] with no handle of its own, so it must leave the pool's set
/// of handles as it found it and name no handle at all. Each page must resume on the line after
/// the last one shown, and `complete` must be false exactly when it carries a `next`. Returns
/// the pages' contents, every `next` emitted, and the last line reached with `total_lines`.
async fn follow_value(
    ctx: &crate::tools::ToolContext,
    path: &str,
    nav: &str,
    key: &str,
    first: Value,
    label: &str,
) -> (Vec<String>, Vec<String>, u64, u64) {
    let route = regex::Regex::new(
        r#"^read_file\("([^"]+)", (json_path|toml_key)=("(?:[^"\\]|\\.)*"), start_line=(\d+), end_line=(\d+)\)$"#,
    )
    .unwrap();
    let prose = regex::Regex::new(
        r"^call read_file on (.+) again with the (json_path|toml_key) you passed and start_line=(\d+), end_line=(\d+)$",
    )
    .unwrap();
    let (mut input, mut got, mut nexts) = (first, Vec::new(), Vec::new());
    let mut last = input["start_line"].as_u64().unwrap_or(1) - 1;
    let mut total = 0;
    for _ in 0..2_000 {
        let v = read_minting_nothing(ctx, &input, "", label)
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
        if let Some(complete) = v["complete"].as_bool() {
            assert_eq!(
                v["next"].is_string(),
                !complete,
                "{label}: complete={complete} with next={}",
                v["next"]
            );
        }
        got.push(v["content"].as_str().unwrap().to_string());
        let Some(next) = v["next"].as_str() else {
            return (got, nexts, last, total);
        };
        nexts.push(next.to_string());
        let (p, n, k, s, e) = if let Some(c) = route.captures(next) {
            let k: String = serde_json::from_str(&c[3]).expect("a JSON string literal");
            (
                c[1].to_string(),
                c[2].to_string(),
                k,
                c[4].to_string(),
                c[5].to_string(),
            )
        } else {
            let c = prose.captures(next).unwrap_or_else(|| {
                panic!("{label}: next is neither a route nor the prose: {next:.300}")
            });
            (
                c[1].to_string(),
                c[2].to_string(),
                key.to_string(),
                c[3].to_string(),
                c[4].to_string(),
            )
        };
        assert_eq!(p, path, "{label}: next leaves the file's path");
        assert_eq!(n, nav, "{label}: next changes the navigation parameter");
        assert_eq!(k, key, "{label}: next is not the key given");
        input = json!({
            "path": p,
            n: k,
            "start_line": s.parse::<u64>().unwrap(),
            "end_line": e.parse::<u64>().unwrap(),
        });
    }
    panic!("{label}: the next chain does not end");
}

/// 600 rows of 50 B: a value far over one page.
fn rows() -> Vec<String> {
    (1..=600)
        .map(|i| format!("row {i:04} {}", "v".repeat(40)))
        .collect()
}

/// A JSON string value, a JSON array value, a TOML table (a run of the file's lines, below its
/// line 1), a TOML dotted group with no header (a run of the file's lines too), and a YAML
/// top-level key, each over one page. Through the REAL `call_content`, in
/// a pool at production capacity and FULL, with and without a whole-file handle for the file:
/// the first page, whole or ranged to the end, then every `next` executed as written. The pages
/// joined are the value's lines, the chain ends on the value's last line, and no call changes
/// the set of handles or names one.
#[tokio::test]
async fn a_value_of_a_real_file_is_paged_on_its_path_and_mints_no_handle() {
    let dir = tempfile::tempdir().unwrap();
    let rows = rows();
    let joined = rows.join("\n");
    let write = |name: &str, body: String| {
        let p = dir.path().join(name);
        std::fs::write(&p, body).unwrap();
        p.to_str().unwrap().to_string()
    };
    let json_file = write(
        "v.json",
        serde_json::to_string_pretty(&json!({ "k": joined, "arr": rows, "other": 1 })).unwrap(),
    );
    let table: String = rows
        .iter()
        .enumerate()
        .map(|(i, r)| format!("r{i:04} = \"{r}\"\n"))
        .collect();
    // Not at line 1: a value numbered in the FILE's lines would be off by the 3 lines above it.
    let toml_table = write(
        "t.toml",
        format!("[pre]\nx = 1\n\n[k]\n{table}\n[other]\nx = 1\n"),
    );
    let dotted: String = rows
        .iter()
        .enumerate()
        .map(|(i, r)| format!("k.r{i:04} = \"{r}\"\n"))
        .collect();
    let toml_dotted = write("d.toml", format!("top = 1\n{dotted}"));
    let list: String = rows.iter().map(|r| format!("  - {r}\n")).collect();
    // Not at line 1, as the TOML table above.
    let yaml_file = write("y.yaml", format!("pre: 1\nk:\n{list}other: 1\n"));

    let read = |p: &str| std::fs::read_to_string(p).unwrap();
    let fs = crate::tools::file_summary::extract_json_path;
    let cases: Vec<(&str, &str, &str, &str, String)> = vec![
        (
            "json string",
            &json_file,
            "json_path",
            "$.k",
            joined.clone(),
        ),
        (
            "json array",
            &json_file,
            "json_path",
            "$.arr",
            fs(&read(&json_file), "$.arr").unwrap().0,
        ),
        (
            "toml table",
            &toml_table,
            "toml_key",
            "k",
            crate::tools::file_summary::extract_toml_key(&read(&toml_table), "k")
                .unwrap()
                .content,
        ),
        (
            "toml dotted",
            &toml_dotted,
            "toml_key",
            "k",
            crate::tools::file_summary::extract_toml_key(&read(&toml_dotted), "k")
                .unwrap()
                .content,
        ),
        (
            "yaml",
            &yaml_file,
            "toml_key",
            "k",
            crate::tools::file_summary::extract_yaml_key(&read(&yaml_file), "k")
                .unwrap()
                .content,
        ),
    ];
    // Fixture checks: a `toml_key` value is a run of the file's own lines, long, and neither starts
    // at the file's line 1, so a value numbered in the FILE's lines would be off by the lines above.
    assert!(
        read(&toml_table).contains(&cases[2].4) && !read(&toml_table).starts_with(&cases[2].4),
        "fixture: toml table"
    );
    assert!(
        read(&toml_dotted).contains(&cases[3].4)
            && !read(&toml_dotted).starts_with(&cases[3].4)
            && cases[3].4.lines().count() >= 600,
        "fixture: the dotted group must be a long run of the file's lines below line 1: {:.200}",
        cases[3].4
    );
    for (label, path, nav, key, value) in &cases {
        let want: Vec<&str> = value.lines().collect();
        for has_handle in [false, true] {
            for ranged in [false, true] {
                let ctx = ctx50().await;
                let handle = if has_handle {
                    let v = ReadFile.call(json!({ "path": path }), &ctx).await.unwrap();
                    Some(
                        v["file_id"]
                            .as_str()
                            .expect("fixture: whole read buffered")
                            .to_string(),
                    )
                } else {
                    None
                };
                fill_pool(&ctx);
                let before = ctx.output_buffer.handles();
                let mut first = json!({ "path": path, *nav: key });
                if ranged {
                    first["start_line"] = json!(1);
                    first["end_line"] = json!(100_000);
                }
                let label = format!("{label} handle={has_handle} ranged={ranged}");
                let (got, nexts, last, total) =
                    follow_value(&ctx, path, nav, key, first, &label).await;
                assert!(!nexts.is_empty(), "{label}: the value was never paged");
                assert_eq!(total as usize, want.len(), "{label}: total_lines");
                assert_eq!(last, total, "{label}: did not reach the last line");
                assert!(
                    got.join("\n") == want.join("\n"),
                    "{label}: the pages joined are not the value"
                );
                assert_eq!(ctx.output_buffer.handles(), before, "{label}");
                if let Some(h) = handle {
                    assert_eq!(
                        ctx.output_buffer.get_stream(&h),
                        Some(read(path)),
                        "{label}: the file's one handle no longer holds the file"
                    );
                }
            }
        }
    }
    // An independent oracle for one case: the string value's own lines are the rows.
    assert_eq!(cases[0].4, joined);
}

/// A `toml_key` value is the key's own lines of the file, so what pages is what the file says: a
/// multi-line string of 600 rows (it was ONE line of escaped text, which paging cannot read), an
/// array of 300 tables (it was the first element), and a table with 200 children under their full
/// headers (they lost the parent's name). The oracle is built from the rows, not from the
/// extractor: the pages joined are the file's lines, the chain ends on the value's last line, and
/// every page carries the `line_range` of the lines in the FILE (so the value's line 1 is the
/// file's line `line_range[0]`).
#[tokio::test]
async fn a_toml_key_value_over_one_page_pages_the_files_own_lines() {
    let dir = tempfile::tempdir().unwrap();
    let rows = rows();
    let string_value = format!("s = \"\"\"\n{}\n\"\"\"", rows.join("\n"));
    let tables: Vec<String> = (0..300)
        .map(|i| format!("[[k]]\nname = \"e{i:03}\"\npad = \"{}\"", "p".repeat(40)))
        .collect();
    let array_value = tables.join("\n\n");
    let children: Vec<String> = (0..200)
        .map(|i| format!("[k.c{i:03}]\nv = \"{}\"", "x".repeat(50)))
        .collect();
    let table_value = format!("[k]\nown = 1\n\n{}", children.join("\n\n"));
    // Each value sits below a line or more of other keys and above a foreign one, so the value's
    // own line 1 is never the file's line 1 and the lines after it are not its.
    let cases = [
        (
            "string",
            format!("top = 1\n{string_value}\nafter = 2\n"),
            "s",
            2,
        ),
        (
            "array of tables",
            format!("top = 1\n\n{array_value}\n\n[tail]\nz = 1\n"),
            "k",
            3,
        ),
        (
            "table with children",
            format!("top = 1\n\n{table_value}\n\n[tail]\nz = 1\n"),
            "k",
            3,
        ),
    ];
    for (label, file, key, first_line) in cases {
        let p = dir.path().join(format!("{}.toml", label.replace(' ', "-")));
        std::fs::write(&p, &file).unwrap();
        let path = p.to_str().unwrap();
        let want: String = file
            .lines()
            .skip(first_line - 1)
            .take_while(|l| !l.starts_with("after") && !l.starts_with("[tail]"))
            .collect::<Vec<_>>()
            .join("\n");
        let want = want.trim_end().to_string();
        let ctx = ctx50().await;
        fill_pool(&ctx);
        let first = json!({ "path": path, "toml_key": key });
        let (got, nexts, last, total) =
            follow_value(&ctx, path, "toml_key", key, first, label).await;
        assert!(!nexts.is_empty(), "{label}: the value was never paged");
        assert_eq!(total as usize, want.lines().count(), "{label}: total_lines");
        assert_eq!(last, total, "{label}: did not reach the last line");
        assert!(
            got.join("\n") == want,
            "{label}: the pages joined are not the file's lines"
        );
        // Every page names the lines of the file the value came from.
        let last_line = first_line + want.lines().count() - 1;
        let mut input = json!({ "path": path, "toml_key": key });
        let mut pages = 0;
        loop {
            let v = ReadFile.call(input.clone(), &ctx).await.unwrap();
            assert_eq!(
                v["line_range"],
                json!([first_line, last_line]),
                "{label} page {pages}: {v:.200}"
            );
            pages += 1;
            let Some(n) = next_value_input(&v) else { break };
            input = n;
        }
        assert!(pages > 1, "{label}: fixture: not paged");
    }
}

/// The TEXT a `toml_key` read returns (what the model reads; the JSON is not shown to it) places
/// the value in its file: lines of the span that belong to other keys were left out, and the value's
/// line numbers are not the file's. Through the real `call_content`, on every page of a paged
/// value, for TOML and YAML; and a read whose value starts the file, or any other form of
/// read, gets no such line.
#[tokio::test]
async fn the_text_of_a_toml_key_read_places_the_value_in_its_file() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let write = |name: &str, body: &str| {
        let p = dir.path().join(name);
        std::fs::write(&p, body).unwrap();
        p.to_str().unwrap().to_string()
    };
    let text_of = |input: Value| {
        let ctx = &ctx;
        async move {
            crate::tools::hint_probe::primary_text(
                &ReadFile.call_content(input, ctx).await.unwrap(),
            )
        }
    };

    // A contiguous value below line 1 names its lines; one that starts the file names nothing.
    let below = write("below.toml", "top = 1\n[a]\nx = 1\n");
    let t = text_of(json!({ "path": below, "toml_key": "a" })).await;
    assert_eq!(t, "2 lines\n  file lines 2-3\n\n[a]\nx = 1", "{t}");
    let first = write("first.toml", "k = 1\nz = 2\n");
    let t = text_of(json!({ "path": first, "toml_key": "k" })).await;
    assert_eq!(t, "1 line\n\nk = 1", "{t}");

    // Not contiguous: it says how many of the span's lines are the value.
    let gap = write("gap.toml", "k.x = 1\nz = 3\nk.y = 2\n");
    let t = text_of(json!({ "path": gap, "toml_key": "k" })).await;
    assert!(
        t.starts_with(
            "2 lines\n  file lines 1-3; 2 of them are this value (the other lines of the span belong to other keys)\n\n"
        ),
        "{t}"
    );

    // YAML: a key below line 1.
    let yaml = write("y.yaml", "pre: 1\nk:\n  - a\nother: 1\n");
    let t = text_of(json!({ "path": yaml, "toml_key": "k" })).await;
    assert!(t.starts_with("2 lines\n  file lines 2-3\n\n"), "{t}");

    // 300 `[[k]]` elements with a foreign section between the two halves: paged, and every page
    // (not only the first) carries the line.
    let element = |i: usize| format!("[[k]]\nname = \"e{i:03}\"\npad = \"{}\"", "p".repeat(40));
    let half = |from: usize, to: usize| (from..to).map(element).collect::<Vec<_>>().join("\n\n");
    let file = format!(
        "top = 1\n\n{}\n\n[foreign]\nz = 1\n\n{}\n",
        half(0, 150),
        half(150, 300)
    );
    let big = write("big.toml", &file);
    let v = ReadFile
        .call(json!({ "path": big, "toml_key": "k" }), &ctx)
        .await
        .unwrap();
    let (a, b) = (
        v["line_range"][0].as_u64().unwrap(),
        v["line_range"][1].as_u64().unwrap(),
    );
    assert_eq!(a, 3, "{v:.200}");
    assert_eq!(
        b as usize,
        3 + half(0, 150).lines().count() + 4 + half(150, 300).lines().count() - 1
    );
    let in_value = v["total_lines"].as_u64().unwrap();
    assert_eq!(
        in_value as usize,
        half(0, 150).lines().count() + half(150, 300).lines().count()
    );
    let want = format!(
        "  file lines {a}-{b}; {in_value} of them are this value (the other lines of the span belong to other keys)\n"
    );
    let mut input = json!({ "path": big, "toml_key": "k" });
    let mut pages = 0;
    loop {
        let blocks = ReadFile.call_content(input.clone(), &ctx).await.unwrap();
        let t = crate::tools::hint_probe::primary_text(&blocks);
        assert!(t.contains(&want), "page {pages}: {t:.400}");
        pages += 1;
        let v = ReadFile.call(input.clone(), &ctx).await.unwrap();
        let Some(n) = next_value_input(&v) else { break };
        input = n;
    }
    assert!(pages > 1, "fixture: not paged");

    // No other form of read grows the line: a markdown section (its JSON has a `line_range` too),
    // a JSON value, a plain range and a whole file say what they said before.
    let md = write("m.md", "# A\n\ntext\n\n## B\nbody\n");
    let js = write("v.json", "{\"a\": 1, \"k\": [1, 2]}");
    for input in [
        json!({ "path": md, "heading": "## B" }),
        json!({ "path": md }),
        json!({ "path": js, "json_path": "$.k" }),
        json!({ "path": below, "start_line": 2, "end_line": 3 }),
        json!({ "path": below }),
    ] {
        let t = text_of(input.clone()).await;
        assert!(!t.contains("file lines"), "{input}: {t}");
    }

    // A sibling list too long to show is dropped, marked in the JSON, and now in the text.
    let siblings: String = (0..40)
        .map(|i| format!("[key_{i:04}{}]\nx = 1\n", "n".repeat(400)))
        .collect();
    let sib = write("sib.toml", &format!("[k]\nv = \"a\"\n{siblings}"));
    let input = json!({ "path": sib, "toml_key": "k" });
    let v = ReadFile.call(input.clone(), &ctx).await.unwrap();
    assert_eq!(v["siblings_omitted"], json!(true), "{v:.300}");
    let t = text_of(input).await;
    assert!(
        t.contains("\n  sibling sections not listed (too many to show)\n"),
        "{t:.400}"
    );
}
/// The placement line of a `toml_key` read claims lines "belong to other keys" only when the value
/// really left some of its span out. A YAML key's value runs to the next top-level key, blank lines
/// and comments included (so a key followed by a blank line is `n` lines of the file but `n - 1`
/// to `str::lines`, which drops a final empty line); it is one run, so it never claims that. A TOML
/// key claims it exactly when its runs are not contiguous. Through the real `call_content`.
#[tokio::test]
async fn the_placement_line_claims_other_keys_only_for_lines_really_left_out() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let write = |name: &str, body: &str| {
        let p = dir.path().join(name);
        std::fs::write(&p, body).unwrap();
        p.to_str().unwrap().to_string()
    };
    let placement = |input: Value| {
        let ctx = &ctx;
        async move {
            let t = crate::tools::hint_probe::primary_text(
                &ReadFile.call_content(input, ctx).await.unwrap(),
            );
            let lines: Vec<String> = t
                .lines()
                .filter(|l| l.starts_with("  file lines") || l.contains("belong to other keys"))
                .map(str::to_string)
                .collect();
            (lines, t)
        }
    };
    const CLAIM: &str = "belong to other keys";

    // YAML with blank separators: the first, a middle and the last key.
    let blanks = write("blanks.yaml", "a: 1\n\nb:\n  - x\n  - y\n\nc: 3\n");
    for (key, want) in [
        ("a", Vec::<&str>::new()),
        ("b", vec!["  file lines 3-6"]),
        ("c", vec!["  file lines 7-7"]),
    ] {
        let (lines, t) = placement(json!({ "path": blanks, "toml_key": key })).await;
        assert_eq!(lines, want, "{key}: {t}");
        assert!(!t.contains(CLAIM), "{key}: {t}");
    }

    // Comments and blank lines between keys: all of them are in the value of the key above.
    let comments = write("comments.yaml", "a: 1\n# about b\nb: 2\n# tail\n\nc: 3\n");
    for (key, want) in [
        ("a", Vec::<&str>::new()),
        ("b", vec!["  file lines 3-5"]),
        ("c", vec!["  file lines 6-6"]),
    ] {
        let (lines, t) = placement(json!({ "path": comments, "toml_key": key })).await;
        assert_eq!(lines, want, "{key}: {t}");
    }

    // A value that ends the file, with and without the final newline, and with blank lines.
    for (name, body, key, want) in [
        ("e1.yaml", "x: 0\na: 1", "a", "  file lines 2-2"),
        ("e2.yaml", "x: 0\na: 1\n", "a", "  file lines 2-2"),
        ("e3.yaml", "x: 0\na: 1\n\n", "a", "  file lines 2-3"),
        ("e4.yaml", "x: 0\na: 1\n\n\n", "a", "  file lines 2-4"),
    ] {
        let (lines, t) = placement(json!({ "path": write(name, body), "toml_key": key })).await;
        assert_eq!(lines, [want], "{name}: {t}");
    }
    let (lines, t) = placement(json!({ "path": write("e5.yaml", "a: 1"), "toml_key": "a" })).await;
    assert!(lines.is_empty(), "{t}");
    let (lines, t) =
        placement(json!({ "path": write("e6.yaml", "a: 1\n\n"), "toml_key": "a" })).await;
    assert!(lines.is_empty(), "{t}");

    // A YAML value over one page, ending in blank lines: every page is silent about other keys
    // (the paged JSON carries `total_lines` from `str::lines`, which is one short of the span).
    let rows: String = (0..400)
        .map(|i| format!("  - item {i:04} {}\n", "p".repeat(40)))
        .collect();
    let big = write("big.yaml", &format!("pre: 1\nk:\n{rows}\n\nnext: 1\n"));
    let mut input = json!({ "path": big, "toml_key": "k" });
    let mut pages = 0;
    loop {
        let (lines, t) = placement(input.clone()).await;
        assert_eq!(
            lines,
            [format!("  file lines 2-{}", 2 + 400 + 2)],
            "{t:.300}"
        );
        pages += 1;
        let v = ReadFile.call(input.clone(), &ctx).await.unwrap();
        let Some(n) = next_value_input(&v) else { break };
        input = n;
    }
    assert!(pages > 1, "fixture: not paged");

    // TOML contiguous: silent about other keys, with and without the final newline.
    for (name, body) in [
        ("c1.toml", "x = 0\n[k]\nv = 1"),
        ("c2.toml", "x = 0\n[k]\nv = 1\n"),
    ] {
        let (lines, t) = placement(json!({ "path": write(name, body), "toml_key": "k" })).await;
        assert_eq!(lines, ["  file lines 2-3"], "{name}: {t}");
    }
    // TOML not contiguous: it claims, naming the span and how many of its lines are the value,
    // at the end of the file with and without the final newline, and when the span starts at 1.
    for (name, body, want) in [
        (
            "n1.toml",
            "x = 1\nk.a = 1\nz = 2\nk.b = 2",
            "  file lines 2-4; 2 of them are this value (the other lines of the span belong to other keys)",
        ),
        (
            "n2.toml",
            "x = 1\nk.a = 1\nz = 2\nk.b = 2\n",
            "  file lines 2-4; 2 of them are this value (the other lines of the span belong to other keys)",
        ),
        (
            "n3.toml",
            "k.a = 1\nz = 2\ny = 3\nk.b = 2\n",
            "  file lines 1-4; 2 of them are this value (the other lines of the span belong to other keys)",
        ),
    ] {
        let (lines, t) = placement(json!({ "path": write(name, body), "toml_key": "k" })).await;
        assert_eq!(lines, [want], "{name}: {t}");
    }
}
/// A missing key in a file of thousands of keys: the hint that names the keys it could have been is
/// held to a quarter of the inline byte budget and says how many names it left out. Measured
/// 2026-10-09 through `call_content` BEFORE the cap: 5,000 keys gave a 55,024 B hint (a 55,098 B
/// error body, rendered whole by `route_tool_error`) for TOML tables, TOML scalars and YAML alike.
/// The names are built by this test (`key_00000` ..), so the count of shown plus omitted is checked
/// against a number the extractor did not produce.
#[tokio::test]
async fn a_missing_key_in_a_huge_file_gets_a_capped_hint() {
    let ctx = ctx().await;
    let dir = tempfile::tempdir().unwrap();
    const KEYS: usize = 5000;
    let nested = format!(
        "[t]\n{}",
        (0..KEYS)
            .map(|i| format!("key_{i:05} = {i}\n"))
            .collect::<String>()
    );
    for (label, ext, key, prefix, body) in [
        (
            "toml tables",
            "toml",
            "nope",
            "Available top-level keys: ",
            (0..KEYS)
                .map(|i| format!("[key_{i:05}]\nv = {i}\n"))
                .collect::<String>(),
        ),
        (
            "toml scalars",
            "toml",
            "nope",
            "Available top-level keys: ",
            (0..KEYS)
                .map(|i| format!("key_{i:05} = {i}\n"))
                .collect::<String>(),
        ),
        (
            "toml nested",
            "toml",
            "t.nope",
            "Available keys in 't': ",
            nested,
        ),
        (
            "yaml",
            "yaml",
            "nope",
            "Available keys: ",
            (0..KEYS)
                .map(|i| format!("key_{i:05}: {i}\n"))
                .collect::<String>(),
        ),
    ] {
        let p = dir.path().join(format!("many.{ext}"));
        std::fs::write(&p, &body).unwrap();
        let path = p.to_str().unwrap();
        let err = ReadFile
            .call_content(json!({ "path": path, "toml_key": key }), &ctx)
            .await
            .expect_err(label);
        let rec = err
            .downcast_ref::<crate::tools::RecoverableError>()
            .unwrap_or_else(|| panic!("{label}: not a recoverable error: {err}"));
        let hint = rec.guidance.as_ref().map(|g| g.text()).unwrap_or("");
        let names = hint
            .strip_prefix(prefix)
            .unwrap_or_else(|| panic!("{label}: {hint:.200}"));
        // The whole error body is far under the inline budget, not 55 KB.
        let wire = serde_json::to_string_pretty(&json!({
            "ok": false, "error": rec.message, "hint": hint,
        }))
        .unwrap();
        assert!(
            wire.len() < crate::tools::INLINE_BYTE_BUDGET / 2,
            "{label}: the error body is {} B",
            wire.len()
        );
        // It says how many it left out, and shown + left out is the number of keys.
        assert!(hint.contains("more not shown"), "{label}: {hint:.300}");
        let mut parts: Vec<&str> = names.split(", ").collect();
        let marker = parts.pop().unwrap();
        let omitted: usize = marker
            .strip_prefix("… (+")
            .and_then(|m| m.strip_suffix(" more not shown)"))
            .and_then(|n| n.parse().ok())
            .unwrap_or_else(|| panic!("{label}: marker {marker:?}"));
        assert_eq!(parts.len() + omitted, KEYS, "{label}");
        // What it shows is the first keys of the file, in file order, and each resolves.
        assert!(parts.len() > 100, "{label}: only {} shown", parts.len());
        for (i, name) in parts.iter().enumerate() {
            assert_eq!(*name, format!("key_{i:05}"), "{label}");
        }
        let last = parts[parts.len() - 1];
        let want = if ext == "toml" && key == "t.nope" {
            format!("t.{last}")
        } else {
            last.to_string()
        };
        let v = ReadFile
            .call(json!({ "path": path, "toml_key": want }), &ctx)
            .await
            .unwrap();
        assert!(v["content"].as_str().unwrap().contains(last), "{label}");
    }
}

/// The `next` of a page of a real file's value quotes the caller's `json_path` (or `toml_key`)
/// WHOLE, as a route that runs as written, while that leaves the page
/// `JSON_PATH_ROUTE_ROOM_FLOOR` of room, and states the continuation in prose past it, never a
/// placeholder. json_paths of 9, 301, 3,005 and 12,005 B; YAML keys of 4 and 12,000 B; and keys
/// carrying a quote, a backslash, `€` and an emoji, whose literal must parse back to the key.
#[tokio::test]
async fn a_real_file_value_next_is_a_whole_route_or_plain_prose() {
    let ctx = ctx50().await;
    fill_pool(&ctx);
    let dir = tempfile::tempdir().unwrap();
    let value = rows().join("\n");
    let mut kinds = Vec::new();
    let odd = "q\"b\\s€😀".to_string();
    // The odd key in the dot form: the bracket form's `["…"]` takes no escapes, so a key with a
    // quote in it is addressed `$.q"b\s€😀`, and the route must still carry it as a literal that
    // parses back to exactly that.
    let bracket = |key: &str| format!("$[{}]", serde_json::to_string(key).unwrap());
    for (i, (key, jp)) in [
        ("k".repeat(4), bracket(&"k".repeat(4))),
        ("k".repeat(296), bracket(&"k".repeat(296))),
        ("k".repeat(3_000), bracket(&"k".repeat(3_000))),
        ("k".repeat(12_000), bracket(&"k".repeat(12_000))),
        (odd.clone(), format!("$.{odd}")),
    ]
    .into_iter()
    .enumerate()
    {
        let p = dir.path().join(format!("route-{i}.json"));
        std::fs::write(&p, json!({ key.clone(): value }).to_string()).unwrap();
        let path = p.to_str().unwrap();
        let label = format!("json {} B path", jp.len());
        let first = json!({ "path": path, "json_path": jp });
        let (got, nexts, last, total) =
            follow_value(&ctx, path, "json_path", &jp, first, &label).await;
        assert_eq!(last, total, "{label}: did not reach the last line");
        assert!(
            got.join("\n") == value,
            "{label}: the pages are not the value"
        );
        kinds.push((jp.len(), nexts[0].starts_with("read_file(")));
        for n in &nexts {
            assert!(
                !n.contains("<your") && !n.contains("not repeated here"),
                "{label}: a placeholder a caller would copy: {n:.300}"
            );
        }
    }
    eprintln!("route (true) or prose (false) by path length: {kinds:?}");
    let expect = [(9, true), (301, true), (3_005, true), (12_005, false)];
    for ((len, route), (want_len, want_route)) in kinds.iter().zip(expect) {
        assert_eq!(*len, want_len, "fixture: path length");
        assert_eq!(*route, want_route, "a {len} B path: route={route}");
    }
    assert!(kinds[4].1, "the odd key's route must be quoted whole");

    let list: String = rows().iter().map(|r| format!("  - {r}\n")).collect();
    for (key, want_route) in [
        ("k".repeat(4), true),
        ("k".repeat(12_000), false),
        (odd, true),
    ] {
        let p = dir.path().join(format!("route-{}.yaml", key.len()));
        std::fs::write(&p, format!("{key}:\n{list}other: 1\n")).unwrap();
        let path = p.to_str().unwrap();
        let label = format!("yaml {} B key", key.len());
        let first = json!({ "path": path, "toml_key": key });
        let (_, nexts, last, total) =
            follow_value(&ctx, path, "toml_key", &key, first, &label).await;
        assert_eq!(last, total, "{label}: did not reach the last line");
        assert_eq!(
            nexts[0].starts_with("read_file("),
            want_route,
            "{label}: {:.200}",
            nexts[0]
        );
        if !want_route {
            assert!(
                nexts[0].contains("with the toml_key you passed"),
                "{label}: {:.200}",
                nexts[0]
            );
        }
    }
}

/// `start_line`/`end_line` beside `json_path` or `toml_key` on a real file slice the VALUE's own
/// lines, by the rules a buffer read applies. They used to be refused outright ("navigation
/// parameters are mutually exclusive with start_line/end_line"). Without a range, an inline value
/// answers exactly as before: the navigation keys and `content`, no `total_lines`.
#[tokio::test]
async fn a_range_beside_json_path_or_toml_key_slices_a_real_files_value() {
    let ctx = ctx50().await;
    let dir = tempfile::tempdir().unwrap();
    let keys = |v: &Value| -> Vec<String> { v.as_object().unwrap().keys().cloned().collect() };
    let j = dir.path().join("s.json");
    std::fs::write(
        &j,
        json!({ "v": "line one\nline two\nline three" }).to_string(),
    )
    .unwrap();
    // Neither key at line 1: a slice numbered in the FILE's lines would land on other text.
    let t = dir.path().join("s.toml");
    std::fs::write(&t, "[o]\nx = 1\n\n[k]\na = 1\nb = 2\n").unwrap();
    let y = dir.path().join("s.yaml");
    std::fs::write(&y, "other: 1\nk:\n  - a\n  - b\n").unwrap();
    let (j, t, y) = (
        j.to_str().unwrap(),
        t.to_str().unwrap(),
        y.to_str().unwrap(),
    );
    let read = |input: Value| {
        let ctx = &ctx;
        async move { ReadFile.call(input, ctx).await }
    };
    let jp = |extra: Value| {
        let mut v = json!({ "path": j, "json_path": "$.v" });
        v.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        v
    };

    let whole = read(jp(json!({}))).await.unwrap();
    assert_eq!(keys(&whole), ["path", "value_type", "format", "content"]);
    let whole = read(json!({ "path": t, "toml_key": "k" })).await.unwrap();
    assert_eq!(
        keys(&whole),
        ["line_range", "breadcrumb", "siblings", "format", "content"]
    );

    let slice = read(jp(json!({ "start_line": 2, "end_line": 3 })))
        .await
        .unwrap();
    assert_eq!(slice["content"], json!("line two\nline three"), "{slice}");
    assert_eq!(slice["total_lines"], json!(3), "{slice}");
    assert_eq!(slice["value_type"], json!("string"), "{slice}");
    // `start_line` alone is a 50-line window, as on every other read.
    let alone = read(jp(json!({ "start_line": 2 }))).await.unwrap();
    assert_eq!(alone["content"], json!("line two\nline three"), "{alone}");
    // Past the end of the value: an empty slice that names the value's length, as a buffer
    // read past its end does.
    let past = read(jp(json!({ "start_line": 10, "end_line": 12 })))
        .await
        .unwrap();
    assert_eq!(past["content"], json!(""), "{past}");
    assert_eq!(past["total_lines"], json!(3), "{past}");
    let text = crate::tools::hint_probe::primary_text(
        &ReadFile
            .call_content(jp(json!({ "start_line": 10, "end_line": 12 })), &ctx)
            .await
            .unwrap(),
    );
    assert!(text.contains("past the end"), "{text}");

    let toml = read(json!({ "path": t, "toml_key": "k", "start_line": 2, "end_line": 3 }))
        .await
        .unwrap();
    assert_eq!(toml["content"], json!("a = 1\nb = 2"), "{toml}");
    assert_eq!(toml["format"], json!("toml"), "{toml}");
    let yaml = read(json!({ "path": y, "toml_key": "k", "start_line": 2, "end_line": 2 }))
        .await
        .unwrap();
    assert_eq!(yaml["content"], json!("  - a"), "{yaml}");

    for (nav, refused, want) in [
        (
            jp(json!({})),
            json!({ "start_line": 3, "end_line": 2 }),
            "invalid line range",
        ),
        (
            jp(json!({})),
            json!({ "start_line": 0, "end_line": 2 }),
            "invalid line range",
        ),
        (
            jp(json!({})),
            json!({ "end_line": 2 }),
            "end_line provided without start_line",
        ),
        (
            json!({ "path": t, "toml_key": "k" }),
            json!({ "start_line": 3, "end_line": 2 }),
            "invalid line range",
        ),
        (
            json!({ "path": y, "toml_key": "k" }),
            json!({ "start_line": 0, "end_line": 2 }),
            "invalid line range",
        ),
    ] {
        let mut input = nav;
        input
            .as_object_mut()
            .unwrap()
            .extend(refused.as_object().unwrap().clone());
        let err = read(input.clone())
            .await
            .expect_err("a range that cannot be one must be refused");
        assert!(err.to_string().contains(want), "{input}: {err}");
    }
}

/// The Text form of a page of a real file's value says how many lines it shows of the value's
/// total, and prints its `next`, which names the file's path and the `json_path`.
#[tokio::test]
async fn the_text_form_of_a_real_files_value_page_counts_and_continues() {
    let ctx = ctx50().await;
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("text.json");
    std::fs::write(&p, json!({ "k": rows().join("\n") }).to_string()).unwrap();
    let path = p.to_str().unwrap();
    for input in [
        json!({ "path": path, "json_path": "$.k" }),
        json!({ "path": path, "json_path": "$.k", "start_line": 100, "end_line": 600 }),
    ] {
        let v = ReadFile.call(input.clone(), &ctx).await.unwrap();
        let (a, b) = (
            v["shown_lines"][0].as_u64().unwrap(),
            v["shown_lines"][1].as_u64().unwrap(),
        );
        let text = crate::tools::hint_probe::primary_text(
            &ReadFile.call_content(input.clone(), &ctx).await.unwrap(),
        );
        let want = format!("[{} of 600 lines shown]", b - a + 1);
        assert!(
            text.contains(&want),
            "{input}: want {want:?} in {text:.300}"
        );
        let next = format!(
            "Next: read_file(\"{path}\", json_path=\"$.k\", start_line={}, end_line={})",
            b + 1,
            input["end_line"].as_u64().unwrap_or(600)
        );
        assert!(
            text.contains(&next),
            "{input}: want {next:?} in {text:.300}"
        );
        assert!(handles_in(&text).is_empty(), "{input}: {text:.300}");
    }
}

/// A context over a pool at production capacity whose project is `root`, so a relative path
/// resolves against it.
async fn ctx50_at(root: &std::path::Path) -> crate::tools::ToolContext {
    let mut ctx = ctx50().await;
    ctx.agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    ctx
}

/// `next` names the file's path AS THE CALLER GAVE IT, never the resolved absolute path: a
/// relative path, a `./` path and a path with a space in it, for `json_path` and `toml_key`.
/// Followed as written to the end, each resolves against the same project. Every other test
/// used an absolute tempdir path, which a `next` naming the resolved path also satisfies.
#[tokio::test]
async fn a_value_page_next_repeats_the_path_as_the_caller_gave_it() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    std::fs::create_dir_all(root.join("sub dir")).unwrap();
    let rows = rows();
    let json_body = json!({ "k": rows.join("\n") }).to_string();
    let list: String = rows.iter().map(|r| format!("  - {r}\n")).collect();
    for rel in ["v.json", "sub dir/v.json"] {
        std::fs::write(root.join(rel), &json_body).unwrap();
    }
    std::fs::write(root.join("v.yaml"), format!("k:\n{list}")).unwrap();
    let ctx = ctx50_at(&root).await;
    fill_pool(&ctx);
    for (path, nav, key) in [
        ("v.json", "json_path", "$.k"),
        ("./v.json", "json_path", "$.k"),
        ("sub dir/v.json", "json_path", "$.k"),
        ("v.yaml", "toml_key", "k"),
        ("./v.yaml", "toml_key", "k"),
    ] {
        let label = format!("{path} {nav}");
        let first = json!({ "path": path, nav: key });
        let v = ReadFile.call(first.clone(), &ctx).await.unwrap();
        let next = v["next"].as_str().expect("fixture: not paged");
        assert!(
            next.starts_with(&format!("read_file(\"{path}\", {nav}=")),
            "{label}: {next}"
        );
        let (_, _, last, total) = follow_value(&ctx, path, nav, key, first, &label).await;
        assert_eq!(last, total, "{label}: did not reach the last line");
    }
}

/// An EMPTY value, read without a range, is the inline answer with the keys it always had:
/// `path`, `value_type`, `format`, `content`, and no page fields.
#[tokio::test]
async fn an_empty_value_of_a_real_file_is_answered_inline() {
    let ctx = ctx50().await;
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("e.json");
    std::fs::write(&p, json!({ "k": "" }).to_string()).unwrap();
    let v = ReadFile
        .call(
            json!({ "path": p.to_str().unwrap(), "json_path": "$.k" }),
            &ctx,
        )
        .await
        .unwrap();
    let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["path", "value_type", "format", "content"], "{v}");
}

/// Every page of a paged value of a real file states what it is a page of, as its inline answer
/// does: an array's `count` and a TOML key's `line_range` (the key's lines in the file) and
/// `format`, on the first page and on every page `next` leads to.
#[tokio::test]
async fn every_page_of_a_real_files_value_carries_count_and_line_range() {
    let ctx = ctx50().await;
    let dir = tempfile::tempdir().unwrap();
    let rows: Vec<String> = (0..900)
        .map(|i| format!("row-{i:04}-padding-padding"))
        .collect();
    let j = dir.path().join("a.json");
    std::fs::write(&j, json!({ "k": rows }).to_string()).unwrap();
    let table: String = (0..900)
        .map(|i| format!("r{i:04} = \"row-{i:04}-padding\"\n"))
        .collect();
    let t = dir.path().join("t.toml");
    let toml = format!("[pre]\nx = 1\n\n[k]\n{table}\n[post]\ny = 2\n");
    std::fs::write(&t, &toml).unwrap();
    let range = crate::tools::file_summary::extract_toml_key(&toml, "k")
        .unwrap()
        .line_range;
    for (path, nav, key, field, want) in [
        (&j, "json_path", "$.k", "count", json!(900)),
        (&t, "toml_key", "k", "line_range", json!([range.0, range.1])),
        (&t, "toml_key", "k", "format", json!("toml")),
    ] {
        let path = path.to_str().unwrap();
        let mut input = json!({ "path": path, nav: key });
        let mut pages = 0;
        loop {
            let v = ReadFile.call(input.clone(), &ctx).await.unwrap();
            assert_eq!(v[field], want, "{path} page {pages}: {v:.200}");
            pages += 1;
            assert!(pages < 500, "{path}: the next chain does not end");
            let Some(n) = next_value_input(&v) else { break };
            input = n;
        }
        assert!(pages > 1, "{path}: fixture: not paged");
    }
}

/// The input a route-shaped `next` of a value page names, or `None` with no `next`.
fn next_value_input(v: &Value) -> Option<Value> {
    let route = regex::Regex::new(
        r#"^read_file\("(.+)", (json_path|toml_key)=("(?:[^"\\]|\\.)*"), start_line=(\d+), end_line=(\d+)\)$"#,
    )
    .unwrap();
    let next = v["next"].as_str()?;
    let c = route.captures(next).expect("a route-shaped next");
    let key: String = serde_json::from_str(&c[3]).unwrap();
    Some(json!({
        "path": &c[1],
        &c[2]: key,
        "start_line": c[4].parse::<u64>().unwrap(),
        "end_line": c[5].parse::<u64>().unwrap(),
    }))
}

/// YAML whose `toml_key` value starts with ONE wide line (`<key>: a…a`, `w` bytes of `a`), then
/// 300 continuation lines, then `siblings`. The first line is the page's first line.
fn wide_first_line_yaml(key: &str, w: usize, siblings: &str) -> String {
    let tail: String = (0..300).map(|i| format!("  cont{i:04}\n")).collect();
    format!("{key}: {}\n{tail}{siblings}", "a".repeat(w))
}

/// `breadcrumb` (a 290-byte key, echoed) that would cost the page its first line whole is
/// dropped, marked `breadcrumb_omitted`, when the line then shows whole. A lone key, so no
/// `siblings` are there to drop first. Swept over first-line widths across that edge.
#[tokio::test]
async fn a_breadcrumb_that_costs_the_first_line_is_dropped() {
    let ctx = ctx50().await;
    let dir = tempfile::tempdir().unwrap();
    let key = "k".repeat(290);
    let mut dropped_whole = 0;
    for w in (7_000..9_200).step_by(5) {
        let p = dir.path().join(format!("b{w}.yaml"));
        std::fs::write(&p, wide_first_line_yaml(&key, w, "")).unwrap();
        let v = ReadFile
            .call(
                json!({ "path": p.to_str().unwrap(), "toml_key": key }),
                &ctx,
            )
            .await
            .unwrap();
        if v["breadcrumb_omitted"] == json!(true) && v.get("line_truncated").is_none() {
            dropped_whole += 1;
        }
        // A lone key has no siblings: `siblings: []` stays, and nothing is marked omitted.
        assert!(
            v["siblings_omitted"].is_null() && v["siblings"] == json!([]),
            "w={w}: an empty siblings list was dropped or marked: {v:.300}"
        );
    }
    assert!(
        dropped_whole > 0,
        "no width dropped breadcrumb to show the first line whole"
    );
}

/// `siblings` and `breadcrumb` are dropped in that order and judged TOGETHER. About 500 B of
/// sibling keys and a 290-byte key beside a first line swept across the edge:
/// - `breadcrumb` is never dropped while `siblings` stay;
/// - where dropping `siblings` alone shows the line whole, `breadcrumb` stays;
/// - where only dropping BOTH shows it whole, both go. Judged one key at a time, both were kept
///   there and the line was cut.
///
/// The region where only dropping both shows the line whole is FOUND by the sweep, not pinned to
/// literal widths. The response echoes the file's absolute path several times, so the edge sits
/// at a different first-line width under every `TMPDIR` (8,600 to 8,800 B under `/tmp`, measured
/// in review, and lower under a longer temp path): a literal window made this test pass or fail
/// with the host's temp directory rather than with the code. What stays pinned is that the
/// region exists, lies inside the sweep, and that the line is whole exactly up to its end and cut
/// after it.
#[tokio::test]
async fn siblings_go_before_breadcrumb_and_the_pair_is_judged_together() {
    let ctx = ctx50().await;
    let dir = tempfile::tempdir().unwrap();
    let key = "k".repeat(290);
    let siblings: String = (0..4)
        .map(|i| format!("s{i}{}: 1\n", "n".repeat(120)))
        .collect();
    let (lo, hi, step) = (7_000usize, 9_200usize, 5usize);
    let (mut siblings_only, mut both) = (0, 0);
    // First-line widths at which the line came back whole / cut, whatever was dropped.
    let (mut last_whole, mut first_cut) = (0usize, usize::MAX);
    // Widths at which ONLY dropping both showed it whole (siblings and breadcrumb both gone).
    let mut both_whole: Vec<usize> = Vec::new();
    for w in (lo..hi).step_by(step) {
        let p = dir.path().join(format!("o{w}.yaml"));
        std::fs::write(&p, wide_first_line_yaml(&key, w, &siblings)).unwrap();
        let v = ReadFile
            .call(
                json!({ "path": p.to_str().unwrap(), "toml_key": key }),
                &ctx,
            )
            .await
            .unwrap();
        let s = v["siblings_omitted"] == json!(true);
        let b = v["breadcrumb_omitted"] == json!(true);
        let cut = v.get("line_truncated").is_some();
        assert!(
            !b || s,
            "w={w}: breadcrumb dropped while siblings stayed: {v:.200}"
        );
        siblings_only += usize::from(s && !b);
        both += usize::from(s && b);
        if cut {
            first_cut = first_cut.min(w);
        } else {
            last_whole = last_whole.max(w);
            if s && b {
                both_whole.push(w);
            }
        }
    }
    assert!(
        siblings_only > 0,
        "fixture: no width dropped siblings alone"
    );
    assert!(both > 0, "fixture: no width dropped both");
    // The line is whole up to an edge and cut after it: a cut width below a whole one means a
    // width was cut that a wider one was not.
    assert!(
        last_whole < first_cut,
        "a width was cut below one that showed whole: whole up to {last_whole}, first cut {first_cut}"
    );
    // The region the test is about: whole only because both keys went. Judged one key at a time
    // it is empty (the line is cut there), and so this fails.
    assert!(
        !both_whole.is_empty(),
        "no width showed the line whole only by dropping both siblings and breadcrumb \
         (whole up to {last_whole}, first cut {first_cut})"
    );
    // ...and the whole sweep saw it: the edge lies inside it, not past its end.
    assert!(
        both_whole.iter().min().unwrap() > &lo && last_whole + step < hi,
        "fixture: the edge is not inside the sweep {lo}..{hi}: whole up to {last_whole}"
    );
    // Both go for the whole of it: every width in the region, from its first to its last, shows
    // the line whole with both dropped. A width in between that kept either would be the
    // one-key-at-a-time failure.
    let (first, last) = (*both_whole.first().unwrap(), *both_whole.last().unwrap());
    assert!(
        both_whole.len() == (last - first) / step + 1,
        "widths {first}..={last} are not all whole with both dropped: {both_whole:?}"
    );
}

/// The clamp hint on a page of a real file's value names the FILE and says `grep -o` searches it
/// as written; it is not the `@tool_*` wording, which calls the target a buffer.
#[tokio::test]
async fn the_clamp_hint_of_a_real_files_value_names_the_file() {
    let ctx = ctx50().await;
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("w.json");
    std::fs::write(&p, json!({ "k": "z".repeat(30_000) }).to_string()).unwrap();
    let path = p.to_str().unwrap();
    let v = ReadFile
        .call(json!({ "path": path, "json_path": "$.k" }), &ctx)
        .await
        .unwrap();
    let hint = v["hint"].as_str().expect("fixture: not clamped");
    assert!(!hint.contains("buffer"), "{hint}");
    assert!(
        hint.contains(&format!("the whole file {path} as it is written")),
        "{hint}"
    );
}

/// `price_continuation` keeps the route when it leaves EXACTLY `JSON_PATH_ROUTE_ROOM_FLOOR` of
/// room: the floor is the least room a route may leave, so at the floor it is still a route.
#[test]
fn the_route_is_kept_at_exactly_the_floor() {
    let route = |s: usize, e: usize| format!("ROUTE {s} {e}");
    let prose = |s: usize, e: usize| format!("PROSE {s} {e}");
    let at_floor = (0..20_000usize).find(|n| {
        let skeleton = json!({ "pad": "x".repeat(*n), "next": route(1, 2) });
        crate::tools::response_room(&skeleton) == super::JSON_PATH_ROUTE_ROOM_FLOOR
    });
    let n = at_floor.expect("fixture: no pad leaves exactly the floor");
    let skeleton = |next: String| json!({ "pad": "x".repeat(n), "next": next });
    let (_, c) = super::price_continuation(&skeleton, &route, Some(&prose), 1, 2);
    assert_eq!(
        c(1, 2),
        "ROUTE 1 2",
        "at exactly the floor the route must be kept"
    );
    let skeleton = |next: String| json!({ "pad": "x".repeat(n + 1), "next": next });
    let (_, c) = super::price_continuation(&skeleton, &route, Some(&prose), 1, 2);
    assert_eq!(c(1, 2), "PROSE 1 2", "one byte under the floor it is prose");
}

/// `start_line` alone beside `json_path` on a real file is a 50-line window of the VALUE, lines
/// `s..=s+49`, as on every other read.
#[tokio::test]
async fn start_line_alone_beside_json_path_is_a_fifty_line_window() {
    let ctx = ctx50().await;
    let dir = tempfile::tempdir().unwrap();
    let rows: Vec<String> = (0..600).map(|i| format!("r{i:04}")).collect();
    let p = dir.path().join("v.json");
    std::fs::write(&p, json!({ "k": rows.join("\n") }).to_string()).unwrap();
    let v = ReadFile
        .call(
            json!({ "path": p.to_str().unwrap(), "json_path": "$.k", "start_line": 7 }),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(v["content"], json!(rows[6..56].join("\n")), "{v:.300}");
}

/// `json_path` (or `toml_key`) on the `@file_*` handle of a REAL file is still refused, but its
/// hint names the route that works, on the file's path, and that route runs as written. A
/// `@file_*` with no file behind it (a synthetic view) and a `@cmd_*` keep the raw-text hint.
#[tokio::test]
async fn a_value_read_of_a_real_files_handle_names_the_files_route() {
    let ctx = ctx50().await;
    let dir = tempfile::tempdir().unwrap();
    let rows = rows();
    let j = dir.path().join("h.json");
    std::fs::write(
        &j,
        serde_json::to_string_pretty(&json!({ "k": rows })).unwrap(),
    )
    .unwrap();
    let list: String = rows.iter().map(|r| format!("  - {r}\n")).collect();
    let y = dir.path().join("h.yaml");
    std::fs::write(&y, format!("k:\n{list}")).unwrap();
    let route =
        regex::Regex::new(r#"read_file\("([^"]+)", (json_path|toml_key)=("(?:[^"\\]|\\.)*")\)"#)
            .unwrap();
    for (file, nav, key) in [(&j, "json_path", "$.k"), (&y, "toml_key", "k")] {
        let file = file.to_str().unwrap();
        let handle = ReadFile.call(json!({ "path": file }), &ctx).await.unwrap()["file_id"]
            .as_str()
            .expect("fixture: the whole read was not buffered")
            .to_string();
        let err = ReadFile
            .call(json!({ "path": handle, nav: key }), &ctx)
            .await
            .expect_err("a value read of a @file_ ref is refused");
        let hint = err.to_string();
        eprintln!("{nav} on {handle}: {hint}");
        let c = route
            .captures(&hint)
            .unwrap_or_else(|| panic!("{nav}: the hint names no route: {hint}"));
        assert_eq!(&c[1], file, "{nav}: the route names another path");
        assert_eq!(&c[2], nav);
        let k: String = serde_json::from_str(&c[3]).unwrap();
        let v = ReadFile
            .call(json!({ "path": &c[1], &c[2]: k }), &ctx)
            .await
            .unwrap_or_else(|e| panic!("{nav}: the route failed: {e}"));
        assert!(v["content"].is_string(), "{nav}: {v:.200}");
    }
    let view = ctx
        .output_buffer
        .store_file("@view".into(), "{\"k\": 1}".into());
    let cmd = ctx
        .output_buffer
        .store("p".into(), "{\"k\": 1}".into(), String::new(), 0);
    for r in [view, cmd] {
        let err = ReadFile
            .call(json!({ "path": r, "json_path": "$.k" }), &ctx)
            .await
            .expect_err("refused");
        let text = err.to_string();
        assert!(text.contains("raw text"), "{r}: {text}");
    }
}

/// A key of exactly `INPUT_ECHO_CLIP` bytes on a real file's `@file_*` handle is still quoted whole in
/// the route the refusal names (the threshold is inclusive), and the route runs.
#[tokio::test]
async fn a_handle_refusal_quotes_a_key_of_exactly_the_clip_whole() {
    let ctx = ctx50().await;
    let dir = tempfile::tempdir().unwrap();
    let key = "k".repeat(super::INPUT_ECHO_CLIP - 5);
    let jp = format!("$[\"{key}\"]");
    assert_eq!(jp.len(), super::INPUT_ECHO_CLIP, "fixture");
    let j = dir.path().join("clip.json");
    std::fs::write(
        &j,
        serde_json::to_string_pretty(&json!({ key.clone(): rows() })).unwrap(),
    )
    .unwrap();
    let file = j.to_str().unwrap();
    let handle = ReadFile.call(json!({ "path": file }), &ctx).await.unwrap()["file_id"]
        .as_str()
        .expect("fixture: buffered")
        .to_string();
    let err = ReadFile
        .call(json!({ "path": handle, "json_path": jp }), &ctx)
        .await
        .expect_err("refused")
        .to_string();
    let lit = serde_json::to_string(&jp).unwrap();
    assert!(
        err.contains(&format!("read_file(\"{file}\", json_path={lit})")),
        "{err:.400}"
    );
}

/// A refusal on a real file's `@file_*` handle names the file's own route only when the file's type
/// accepts that parameter: `toml_key` on a `.json` file and `json_path` on a `.txt` file are refused on
/// the path too, so the hint falls back to the raw-text advice.
#[tokio::test]
async fn a_handle_refusal_names_no_route_the_file_type_would_refuse() {
    let ctx = ctx50().await;
    let dir = tempfile::tempdir().unwrap();
    let json = dir.path().join("h.json");
    std::fs::write(
        &json,
        serde_json::to_string_pretty(&json!({ "k": rows() })).unwrap(),
    )
    .unwrap();
    let txt = dir.path().join("h.txt");
    std::fs::write(&txt, "x\n".repeat(20_000)).unwrap();
    for (file, nav, value) in [(&json, "toml_key", "k"), (&txt, "json_path", "$.k")] {
        let path = file.to_str().unwrap();
        let handle = ReadFile.call(json!({ "path": path }), &ctx).await.unwrap()["file_id"]
            .as_str()
            .expect("fixture: buffered")
            .to_string();
        let err = ReadFile
            .call(json!({ "path": handle, nav: value }), &ctx)
            .await
            .expect_err("refused")
            .to_string();
        assert!(
            !err.contains("holds the real file"),
            "{nav} on {path}: {err:.400}"
        );
        assert!(
            !err.contains(&format!("read_file(\"{path}\"")),
            "{nav} on {path}: {err:.400}"
        );
    }
}
