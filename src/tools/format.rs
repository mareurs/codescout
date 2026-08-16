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
/// See `docs/issues/2026-08-15-truncate-compact-tail-cut-destroys-overflow-signal.md`.
pub(crate) fn overflow_head(val: &Value) -> String {
    match val.get("overflow").filter(|o| o.is_object()) {
        Some(overflow) => format!("{}\n", format_overflow(overflow)),
        None => String::new(),
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
    /// See `docs/issues/2026-08-15-truncate-compact-tail-cut-destroys-overflow-signal.md`.
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
