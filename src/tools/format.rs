//! Shared formatting helpers used by multiple tool format_compact implementations.

use serde_json::Value;

/// Format a line range like "L35-50" or "L35" if start == end.
pub(crate) fn format_line_range(start: u64, end: u64) -> String {
    if start == end || end == 0 {
        format!("L{start}")
    } else {
        format!("L{start}-{end}")
    }
}

/// Truncate a path to max_len chars, replacing the middle with "…".
#[allow(dead_code)] // Used by format_compact impls in tool modules.
pub(crate) fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        return path.to_string();
    }
    if max_len < 5 {
        let end = crate::tools::floor_char_boundary(path, max_len);
        return path[..end].to_string();
    }
    let keep_end = max_len / 2;
    let keep_start = max_len - keep_end - 1; // 1 for the ellipsis char
    let start = crate::tools::floor_char_boundary(path, keep_start);
    let tail_offset = crate::tools::floor_char_boundary(path, path.len() - keep_end);
    format!("{}…{}", &path[..start], &path[tail_offset..])
}

/// Format an overflow hint as a compact one-liner.
pub(crate) fn format_overflow(overflow: &Value) -> String {
    let shown = overflow["shown"].as_u64().unwrap_or(0);
    let total = overflow["total"].as_u64().unwrap_or(0);
    let hint = overflow["hint"].as_str().unwrap_or("");
    if total > shown {
        format!("  … showing {shown} of {total} — {hint}")
    } else {
        format!("  … showing first {shown} — {hint}")
    }
}

/// The overflow note rendered for placement at the **head** of a compact summary,
/// above the variable-length rows.
///
/// Returns `""` when the result carries no overflow object, so a caller can push it
/// unconditionally into its header block without a surrounding `if let`.
///
/// **Head placement is load-bearing, not cosmetic.** `call_content`'s overflow path shows
/// the caller `truncate_compact(format_compact(val), soft, hard)` and nothing else, and
/// `truncate_compact` (`src/tools/core/types.rs`) keeps only the PREFIX up to the last
/// newline inside the hard cap. A note appended *after* the rows is therefore cut first,
/// on exactly the results big enough to need it: the summary keeps "here are some rows"
/// and drops the sentence saying how many were withheld, so an incomplete answer reads as
/// a complete one.
///
/// Nine call sites across five surfaces tail-appended it. Note the cutter itself is
/// correct and deliberately unchanged — a tail cut is right for prose; the defect was
/// producer-side ordering.
///
/// See `docs/issues/archive/2026-08-15-truncate-compact-tail-cut-destroys-overflow-signal.md`.
pub(crate) fn overflow_head(val: &Value) -> String {
    match val.get("overflow").filter(|o| o.is_object()) {
        Some(overflow) => format!("{}\n", format_overflow(overflow)),
        None => String::new(),
    }
}

/// The buffer-truncation notice rendered for placement at the **head** of a result.
///
/// Sibling of [`overflow_head`], and head-placed for the same load-bearing reason: a
/// notice appended after the content is cut first on exactly the results big enough to
/// need it. Here the stakes are sharper still — the whole point of the notice is that
/// the content above it is a PREFIX, so letting that content push it off the end would
/// reproduce the defect the notice exists to report.
///
/// Returns `""` when the result carries no `buffer_truncated` array, so callers can
/// push it unconditionally without a surrounding `if let`.
///
/// BUG docs/issues/archive/2026-08-27-unfiltered-output-lines-counts-the-source-not-the-buffer.md
pub(crate) fn truncation_head(val: &Value) -> String {
    match val.get("buffer_truncated").and_then(|v| v.as_array()) {
        Some(notices) if !notices.is_empty() => notices
            .iter()
            .filter_map(|n| n.as_str())
            .map(|n| format!("{n}\n"))
            .collect(),
        _ => String::new(),
    }
}

/// Place `extra` immediately below `body`'s first line.
///
/// Two requirements meet here and neither yields. The overflow note must land inside the
/// prefix `truncate_compact` keeps (see [`overflow_head`]) — but it must not displace the
/// first line, which is the one a reader anchors on and which several surfaces make
/// load-bearing: `grep`'s count header carries the `capped` marker that stops a
/// collection-capped result from reading as a complete one
/// (`grep_capped_collection_never_renders_as_a_complete_result`).
///
/// Slotting in second satisfies both: the header is still first, and the note is metres
/// from the top of a budget measured in kilobytes.
///
/// `extra` should end with a newline. An empty `extra` returns `body` untouched.
pub(crate) fn insert_below_header(body: String, extra: &str) -> String {
    if extra.is_empty() {
        return body;
    }
    match body.find('\n') {
        Some(i) => format!("{}\n{}{}", &body[..i], extra, &body[i + 1..]),
        None => format!("{body}\n{extra}"),
    }
}

/// Describe a buffered payload's shape, for tools with no bespoke `format_compact`.
///
/// The fallback this feeds used to read `"Result stored in @tool_abc (18618 bytes)"` and
/// nothing else — which **restates the envelope's own `output_id`** and adds a byte count
/// the caller can already see. The one slot that could say something about the result said
/// nothing, so the call returned no answer and cost a second round-trip to find out what
/// was in there. Measured 2026-08-16: every librarian `find` / `graph` / `state_at`, and
/// every `get` whose body fits under the soft cap, took that path.
///
/// What comes back instead is a map — the top-level keys, so a `json_path` can be aimed;
/// each array's length; and short scalars verbatim, since those are frequently the answer
/// outright (`status`, `title`, a count).
///
/// Bounded on purpose: a wide object must not spend the whole summary budget listing keys.
/// Returns `None` for a payload with no describable shape (a bare scalar), leaving the
/// caller its own wording.
///
/// See `docs/issues/archive/2026-08-16-content-free-overflow-envelope-costs-a-round-trip.md`.
pub(crate) fn describe_payload_shape(val: &Value) -> Option<String> {
    /// Wide objects exist (`doc(get)` alone carries ~15); listing every key would
    /// crowd out the arrays and scalars below, which carry more per byte.
    const MAX_KEYS: usize = 24;
    /// Long enough for a title or a status, short enough that a stray blob cannot
    /// monopolise the line.
    const MAX_SCALAR_LEN: usize = 60;
    const MAX_SCALARS: usize = 8;

    match val {
        Value::Object(map) if !map.is_empty() => {
            let keys: Vec<&str> = map.keys().map(String::as_str).collect();
            let shown = keys.len().min(MAX_KEYS);
            let mut out = format!("{} keys: {}", keys.len(), keys[..shown].join(", "));
            if keys.len() > shown {
                out.push_str(&format!(", … +{} more", keys.len() - shown));
            }

            let arrays: Vec<String> = map
                .iter()
                .filter_map(|(k, v)| v.as_array().map(|a| format!("{k}[{}]", a.len())))
                .collect();
            if !arrays.is_empty() {
                out.push_str(&format!("\n  arrays: {}", arrays.join(", ")));
            }

            let scalars: Vec<String> = map
                .iter()
                .filter_map(|(k, v)| match v {
                    Value::String(s) if s.len() <= MAX_SCALAR_LEN => Some(format!("{k}={s:?}")),
                    Value::Number(n) => Some(format!("{k}={n}")),
                    Value::Bool(b) => Some(format!("{k}={b}")),
                    _ => None,
                })
                .take(MAX_SCALARS)
                .collect();
            if !scalars.is_empty() {
                out.push_str(&format!("\n  {}", scalars.join(", ")));
            }
            Some(out)
        }
        Value::Array(items) => {
            let mut out = format!("array of {} items", items.len());
            // The element keys are what a `[*]` projection needs to name a field.
            if let Some(Value::Object(first)) = items.first() {
                let keys: Vec<&str> = first.keys().map(String::as_str).take(MAX_KEYS).collect();
                out.push_str(&format!("\n  item keys: {}", keys.join(", ")));
            }
            Some(out)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_range_single() {
        assert_eq!(format_line_range(35, 35), "L35");
    }

    #[test]
    fn line_range_span() {
        assert_eq!(format_line_range(35, 50), "L35-50");
    }

    #[test]
    fn line_range_zero_end() {
        assert_eq!(format_line_range(10, 0), "L10");
    }

    #[test]
    fn overflow_head_is_empty_without_an_overflow_object() {
        // Callers push this unconditionally into their header block, so the no-overflow
        // case must contribute nothing at all — not a stray newline.
        assert_eq!(overflow_head(&serde_json::json!({})), "");
        assert_eq!(overflow_head(&serde_json::json!({"overflow": null})), "");
        // A non-object `overflow` is not a shape this can render; treat it as absent
        // rather than printing "showing 0 of 0".
        assert_eq!(overflow_head(&serde_json::json!({"overflow": 3})), "");
    }

    #[test]
    fn insert_below_header_keeps_the_first_line_first() {
        // The whole point: the note lands second, not first and not last.
        let body = "12 matches\n  row a\n  row b".to_string();
        let out = insert_below_header(body, "  … showing 2 of 12 — narrow it\n");
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "12 matches", "header must stay first: {out}");
        assert_eq!(lines[1], "  … showing 2 of 12 — narrow it");
        assert_eq!(lines[2], "  row a", "rows must survive intact: {out}");
        assert_eq!(lines[3], "  row b");
    }

    #[test]
    fn insert_below_header_handles_a_single_line_body_and_an_empty_extra() {
        assert_eq!(
            insert_below_header("0 lines".to_string(), "note\n"),
            "0 lines\nnote\n",
            "a body with no newline still gets the note appended below it"
        );
        assert_eq!(
            insert_below_header("12 matches\n  row".to_string(), ""),
            "12 matches\n  row",
            "an empty extra must not perturb the body"
        );
    }

    /// Every compact surface must keep its overflow note inside the prefix that
    /// `truncate_compact` preserves.
    ///
    /// This is the regression test for the defect itself, and it is written end-to-end on
    /// purpose: it renders through each tool's real `format_compact`, then applies the
    /// real caps `call_content` uses, and asserts the note is still there. A test that
    /// only exercised [`overflow_head`] would pass while all nine call sites went on
    /// appending it at the tail — which is exactly the state this replaced.
    ///
    /// See `docs/issues/archive/2026-08-15-truncate-compact-tail-cut-destroys-overflow-signal.md`.
    #[test]
    fn every_surface_keeps_its_overflow_note_above_the_truncation_cap() {
        use crate::tools::core::types::truncate_compact;
        use crate::tools::{Tool, COMPACT_SUMMARY_HARD_MAX_BYTES, COMPACT_SUMMARY_MAX_BYTES};

        let overflow = serde_json::json!({
            "shown": 50, "total": 4321, "hint": "NARROW-WITH-THIS"
        });

        // Each payload is deliberately far past the hard cap — that is the condition
        // under which the note used to disappear.
        let long_rows: Vec<serde_json::Value> = (0..300)
            .map(|i| serde_json::json!({"file": format!("src/dir/file_{i}.rs"), "count": i}))
            .collect();
        let entries: Vec<String> = (0..300).map(|i| format!("src/dir/file_{i}.rs")).collect();
        let results: Vec<serde_json::Value> = (0..120)
            .map(|i| {
                serde_json::json!({
                    "file_path": format!("src/dir/file_{i}.rs"),
                    "start_line": 1, "end_line": 2,
                    "content": "x".repeat(120),
                })
            })
            .collect();

        let cases: Vec<(&str, String)> = vec![
            (
                "grep",
                crate::tools::grep::Grep
                    .format_compact(&serde_json::json!({
                        "total": 4321, "files": long_rows, "files_count": 300,
                        "overflow": overflow,
                    }))
                    .expect("grep has a compact form"),
            ),
            (
                "tree",
                crate::tools::tree::Tree
                    .format_compact(&serde_json::json!({
                        "entries": entries, "overflow": overflow,
                    }))
                    .expect("tree has a compact form"),
            ),
            (
                "semantic_search",
                crate::tools::semantic::SemanticSearch
                    .format_compact(&serde_json::json!({
                        "results": results, "total": 4321, "overflow": overflow,
                    }))
                    .expect("semantic_search has a compact form"),
            ),
            (
                "read_file",
                crate::tools::read_file::ReadFile
                    .format_compact(&serde_json::json!({
                        "content": "some line of content\n".repeat(400),
                        "total_lines": 4321,
                        "overflow": overflow,
                    }))
                    .expect("read_file has a compact form"),
            ),
        ];

        for (surface, rendered) in cases {
            assert!(
                rendered.len() > COMPACT_SUMMARY_HARD_MAX_BYTES,
                "{surface}: the fixture must exceed the hard cap or this test proves \
                 nothing — got {} bytes",
                rendered.len()
            );
            let cut = truncate_compact(
                &rendered,
                COMPACT_SUMMARY_MAX_BYTES,
                COMPACT_SUMMARY_HARD_MAX_BYTES,
            );
            assert!(
                cut.contains("NARROW-WITH-THIS"),
                "{surface}: the overflow hint was cut away — it must sit above the rows, \
                 not after them. Cut summary:\n{cut}"
            );
            assert!(
                cut.contains("4321"),
                "{surface}: the withheld-count must survive the cut too. Cut summary:\n{cut}"
            );
        }
    }

    /// The generic fallback must describe the payload, not restate the envelope.
    ///
    /// `"Result stored in @tool_abc (18618 bytes)"` repeats `output_id` — a field the
    /// caller already holds — and adds a byte count. It answers nothing, so the call is
    /// spent and a second round-trip is needed to learn what is in the buffer. This
    /// asserts the replacement carries something the envelope does not.
    ///
    /// See `docs/issues/archive/2026-08-16-content-free-overflow-envelope-costs-a-round-trip.md`.
    #[test]
    fn the_generic_fallback_describes_the_payload_instead_of_the_envelope() {
        // Shaped like a real librarian `doc(get)` response — the measured case.
        let val = serde_json::json!({
            "id": "9a892c2a5976e296",
            "kind": "tracker",
            "status": "active",
            "title": "Open-Issue Work Queue (BL-N)",
            "tags": ["backlog", "triage"],
            "body": "x".repeat(20_000),
        });
        let shape = describe_payload_shape(&val).expect("an object has a describable shape");

        // The keys are what lets a caller aim a json_path without a second call.
        assert!(shape.contains("body"), "must name the big field: {shape}");
        assert!(shape.contains("6 keys"), "must count the keys: {shape}");
        // Array lengths, so `tags[*]` is known to be worth projecting.
        assert!(shape.contains("tags[2]"), "must size the arrays: {shape}");
        // Short scalars are frequently the answer outright.
        assert!(
            shape.contains("Open-Issue Work Queue (BL-N)"),
            "a short scalar is often the answer and must appear verbatim: {shape}"
        );
        assert!(
            shape.contains(r#"status="active""#),
            "must carry short scalars: {shape}"
        );
        // The 20 KB body must be named but never inlined — that is the whole point of
        // buffering it in the first place.
        assert!(
            !shape.contains("xxxxxxxxxx"),
            "a large value must be named, not inlined: {shape}"
        );
        assert!(
            shape.len() < 600,
            "the description must stay a summary, got {} bytes: {shape}",
            shape.len()
        );
    }

    #[test]
    fn describe_payload_shape_handles_arrays_and_declines_scalars() {
        let arr = serde_json::json!([{"id": "T-1", "verdict": "ok"}, {"id": "T-2"}]);
        let shape = describe_payload_shape(&arr).expect("an array has a shape");
        assert!(shape.contains("array of 2 items"), "{shape}");
        assert!(
            shape.contains("item keys: id, verdict"),
            "element keys are what a [*] projection needs to name a field: {shape}"
        );

        // Nothing useful to say about these — the caller keeps its own wording rather
        // than being handed a description of a scalar.
        assert!(describe_payload_shape(&serde_json::json!("hi")).is_none());
        assert!(describe_payload_shape(&serde_json::json!(7)).is_none());
        assert!(describe_payload_shape(&serde_json::json!({})).is_none());
    }

    #[test]
    fn truncate_short_path() {
        assert_eq!(truncate_path("src/main.rs", 30), "src/main.rs");
    }

    #[test]
    fn truncate_long_path() {
        let long = "src/tools/very/deeply/nested/path/to/file.rs";
        let result = truncate_path(long, 25);
        assert!(
            result.chars().count() <= 25,
            "got len {} for '{}'",
            result.chars().count(),
            result
        );
        assert!(result.contains('…'));
    }

    #[test]
    fn truncate_path_unicode_does_not_panic() {
        // Each '─' (BOX DRAWINGS LIGHT HORIZONTAL) is 3 bytes.
        // Build a path whose byte length > max_len but where max_len falls
        // inside a multi-byte char — without floor_char_boundary this panics.
        let unicode_segment = "─".repeat(30); // 90 bytes
        let path = format!("src/tools/{}/file.rs", unicode_segment);
        // max_len=25: keep_start=12, keep_end=12 — both fall inside multi-byte chars.
        let result = truncate_path(&path, 25);
        assert!(result.contains('…'), "must contain ellipsis");
        // Must be valid UTF-8 (no panic = passes, but also verify well-formed)
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }

    #[test]
    fn overflow_with_total() {
        let ov = serde_json::json!({
            "shown": 50, "total": 234, "hint": "narrow with path="
        });
        let result = format_overflow(&ov);
        assert!(result.contains("50 of 234"));
        assert!(result.contains("narrow with path="));
    }

    #[test]
    fn overflow_without_total() {
        let ov = serde_json::json!({
            "shown": 50, "total": 50, "hint": "use more specific pattern"
        });
        let result = format_overflow(&ov);
        assert!(result.contains("first 50"));
    }
}
