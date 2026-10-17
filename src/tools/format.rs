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
/// Every bound discloses itself. An object's key line ends `… +N more`; its scalar line, a
/// nested object's `k: {…}` group and an array's `item keys:` line end `… +N more entries`
/// (`entry` at one). N counts the list's own entries that were held back: an over-length
/// string filtered, an entry (a scalar, or a nested object's group) cut past `MAX_SCALARS`, a
/// key cut past `MAX_ITEM_KEYS`, and — inside a nested object only, where nothing else names
/// them — its array, object and null members. A cut group is one entry however many scalars
/// it held. The marker is absent when nothing was dropped. See
/// `docs/issues/2026-10-16-describe-payload-shape-drops-scalars-without-a-count.md`,
/// `docs/issues/2026-10-17-describe-payload-shape-item-keys-cut-without-a-count.md` and
/// `docs/issues/2026-10-17-describe-payload-shape-nested-object-drops-its-non-scalar-members.md`.
///
/// Returns `None` for a payload with no describable shape (a bare scalar), leaving the
/// caller its own wording.
///
/// See `docs/issues/archive/2026-08-16-content-free-overflow-envelope-costs-a-round-trip.md`.
///
/// **Object-valued keys are descended one level, not dropped.** The scalar pass used to end
/// in `_ => None` for anything that was not a string/number/bool, which silently reduced an
/// object-valued key (e.g. a `hints` field carrying `unindexed_files` / `hidden_archived`) to
/// its bare name in the `keys:` line and nothing else — the magnitude (`count`) reached the
/// reader verbatim while the qualifier that makes it interpretable did not. The descent is
/// deliberately ONE level and scalars-only (no general recursive walk, which would re-open
/// the `MAX_KEYS` / `MAX_SCALARS` budget question this function exists to close) — every
/// known caller's nested object (`hints`) is a flat map of short scalars by construction.
/// See `docs/issues/archive/2026-09-02-overflow-summary-promotes-the-count-and-elides-its-caveat.md`.
pub(crate) fn describe_payload_shape(val: &Value) -> Option<String> {
    /// Wide objects exist (`doc(get)` alone carries ~15); listing every key would
    /// crowd out the arrays and scalars below, which carry more per byte.
    // cap-class: RESULT_CAP format.shape_keys — probed
    const MAX_KEYS: usize = 24;
    /// The element-key list of an array's first item. Its own constant and `PROBE_ROWS` row
    /// from the object key line's, so each cap carries its own evidence.
    // cap-class: RESULT_CAP format.shape_item_keys — probed
    const MAX_ITEM_KEYS: usize = 24;
    /// Long enough for a title or a status, short enough that a stray blob cannot
    /// monopolise the line.
    // cap-class: RESULT_CAP format.shape_scalar_len — probed
    const MAX_SCALAR_LEN: usize = 60;
    // cap-class: RESULT_CAP format.shape_scalars — probed
    const MAX_SCALARS: usize = 8;

    /// What one object field contributes to a scalar line.
    enum ScalarCell {
        /// A renderable `k=v` pair.
        Shown(String),
        /// A string over the length bound: a scalar the line COULD have carried and did not.
        TooLong,
        /// Not a scalar (array, null, object). At the top level the key line and `arrays:`
        /// already name it, so it is not a drop and must not inflate the count. Inside
        /// `flat_scalars` nothing names it, so there it IS counted as an entry held back
        /// (`docs/issues/2026-10-17-describe-payload-shape-nested-object-drops-its-non-scalar-members.md`).
        NotScalar,
    }

    /// Classify one string/number/bool field. Object-valued keys are the caller's concern.
    fn scalar_cell(k: &str, v: &Value, max_len: usize) -> ScalarCell {
        match v {
            Value::String(s) if s.len() <= max_len => ScalarCell::Shown(format!("{k}={s:?}")),
            Value::String(_) => ScalarCell::TooLong,
            Value::Number(n) => ScalarCell::Shown(format!("{k}={n}")),
            Value::Bool(b) => ScalarCell::Shown(format!("{k}={b}")),
            _ => ScalarCell::NotScalar,
        }
    }

    /// Cap `shown` at `max_count`, returning what survived and how many entries were held
    /// back in all — the `too_long` ones filtered earlier plus the ones cut here. The unit is
    /// the line's own ENTRY: an object-valued key cut here is one entry however many scalars
    /// it held. The count is what lets the rendered line say it is partial, as the key line
    /// does.
    fn cap_scalars(
        mut shown: Vec<String>,
        too_long: usize,
        max_count: usize,
    ) -> (Vec<String>, usize) {
        let cut = shown.len().saturating_sub(max_count);
        shown.truncate(max_count);
        (shown, too_long + cut)
    }

    /// The one marker every list in this description ends with when it held entries back:
    /// `… +N more entries` (`entry` at one). Used by the scalar line, a nested object's group
    /// and the array `item keys:` line, so all three read the same and say what they count.
    fn more_entries(dropped: usize) -> String {
        let noun = if dropped == 1 { "entry" } else { "entries" };
        format!("… +{dropped} more {noun}")
    }

    /// Join rendered entries and, when any were dropped, end with `… +N more entries` (`entry`
    /// at one) — the scalar-line counterpart of the key line's `… +N more`. Silent at zero.
    fn join_scalars(mut shown: Vec<String>, dropped: usize) -> String {
        if dropped > 0 {
            shown.push(more_entries(dropped));
        }
        shown.join(", ")
    }

    /// Render a flat object's own scalar fields as `k=v` pairs, bounded the same way the
    /// top-level scalar pass is. Used for the one-level descent into an object-valued key —
    /// never called a second level down, which is what keeps the descent narrow rather than
    /// a general recursive walk. Returns the pairs that fit and the number of entries it
    /// held back: over-length strings, entries past `max_count`, and — unlike the top level,
    /// where `keys:` and `arrays:` name them — the array, object and null members, which
    /// nothing else in the output names.
    fn flat_scalars(
        map: &serde_json::Map<String, Value>,
        max_len: usize,
        max_count: usize,
    ) -> (Vec<String>, usize) {
        let mut shown = Vec::new();
        let mut held_back = 0;
        for (k, v) in map {
            match scalar_cell(k, v, max_len) {
                ScalarCell::Shown(s) => shown.push(s),
                ScalarCell::TooLong | ScalarCell::NotScalar => held_back += 1,
            }
        }
        cap_scalars(shown, held_back, max_count)
    }

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

            let mut scalars: Vec<String> = Vec::new();
            let mut too_long = 0;
            for (k, v) in map {
                match v {
                    // A non-empty object gets ONE level of descent into its own scalars —
                    // e.g. `hints={unindexed_files=1}` — instead of being reduced to its
                    // bare key name. An object whose contents are themselves non-scalar
                    // (nested further, or empty) still contributes nothing here; that is
                    // the narrow-fix boundary, not a silent regression, since it is no
                    // worse than the prior behavior for that shape. What the descent
                    // itself drops is counted inside the braces, and the object holds
                    // ONE slot (one entry) of the top-level cap however many scalars it
                    // carries.
                    Value::Object(o) if !o.is_empty() => {
                        let (nested, nested_dropped) = flat_scalars(o, MAX_SCALAR_LEN, MAX_SCALARS);
                        if !nested.is_empty() || nested_dropped > 0 {
                            scalars
                                .push(format!("{k}: {{{}}}", join_scalars(nested, nested_dropped)));
                        }
                    }
                    _ => match scalar_cell(k, v, MAX_SCALAR_LEN) {
                        ScalarCell::Shown(s) => scalars.push(s),
                        ScalarCell::TooLong => too_long += 1,
                        ScalarCell::NotScalar => {}
                    },
                }
            }
            let (scalars, dropped) = cap_scalars(scalars, too_long, MAX_SCALARS);
            if !scalars.is_empty() || dropped > 0 {
                out.push_str(&format!("\n  {}", join_scalars(scalars, dropped)));
            }
            Some(out)
        }
        Value::Array(items) => {
            let mut out = format!("array of {} items", items.len());
            // The element keys are what a `[*]` projection needs to name a field.
            if let Some(Value::Object(first)) = items.first() {
                let keys: Vec<&str> = first
                    .keys()
                    .map(String::as_str)
                    .take(MAX_ITEM_KEYS)
                    .collect();
                out.push_str(&format!("\n  item keys: {}", keys.join(", ")));
                if first.len() > keys.len() {
                    out.push_str(&format!(", {}", more_entries(first.len() - keys.len())));
                }
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

    /// Regression test for
    /// `docs/issues/archive/2026-09-02-overflow-summary-promotes-the-count-and-elides-its-caveat.md`:
    /// a mixed payload whose object-valued key (`hints`, analogous to the librarian's own
    /// completeness/scope qualifiers) must surface its scalar contents, not just its bare
    /// name in the `keys:` line — the array-valued key already worked (`arrays:` pass), the
    /// scalar-valued key already worked; only the object-valued key was silently dropped.
    #[test]
    fn describe_payload_shape_surfaces_a_nested_object_alongside_scalars_and_arrays() {
        let val = serde_json::json!({
            "count": 48,
            "hints": {"unindexed_files": 1, "reindex_hint": true},
            "items": [1, 2, 3],
        });
        let shape = describe_payload_shape(&val).expect("an object has a shape");

        // The scalar-valued key: unaffected by this fix, must still work.
        assert!(shape.contains("count=48"), "scalar key regressed: {shape}");
        // The array-valued key: unaffected by this fix (handled by the `arrays:` pass).
        assert!(shape.contains("items[3]"), "array key regressed: {shape}");
        // The object-valued key: THE fix. Before it, `hints` contributed only its bare
        // name to `keys:` and nothing else — the magnitude (`count=48`) reached the reader
        // while the qualifier that makes it interpretable (`unindexed_files`) did not.
        assert!(
            shape.contains("unindexed_files=1"),
            "object-valued key's scalar contents must surface, not be reduced to a bare \
             key name: {shape}"
        );
        assert!(
            shape.contains("reindex_hint=true"),
            "every flat scalar inside the nested object must surface, not just the first: {shape}"
        );
    }

    /// The shape's scalar line is the LAST line of the description (the `keys:` and
    /// `arrays:` lines precede it), so a marker asserted on it cannot be satisfied by the
    /// key line's own `… +N more`.
    fn scalar_line(shape: &str) -> &str {
        shape
            .lines()
            .last()
            .expect("a shape has at least a keys line")
    }

    /// Regression test for
    /// `docs/issues/2026-10-16-describe-payload-shape-drops-scalars-without-a-count.md`
    /// (the `MAX_SCALAR_LEN` half): a string over the length bound is filtered out of the
    /// scalar line, and the line must say how many entries that removed — the key line
    /// above it ends `… +N more`, this one used to end in silence.
    ///
    /// Built so only the LENGTH bound can bite: four scalar candidates against a cap of
    /// eight, so `MAX_SCALARS` admits every one. 60 bytes is the longest admitted, 61 the
    /// shortest dropped — the boundary, not just a far-over-length blob.
    #[test]
    fn describe_payload_shape_counts_over_length_scalars_it_drops() {
        let val = serde_json::json!({
            "id": "T-1",
            "edge_ok": "e".repeat(60),
            "edge_over": "e".repeat(61),
            "blob": "b".repeat(20_000),
        });
        let shape = describe_payload_shape(&val).expect("an object has a shape");
        let line = scalar_line(&shape);

        assert!(
            line.contains("id=\"T-1\""),
            "short scalar must show: {shape}"
        );
        assert!(
            line.contains("edge_ok="),
            "a 60-byte string is admitted: {shape}"
        );
        assert!(
            !line.contains("edge_over=") && !line.contains("blob="),
            "over-length strings are named in the key line, never inlined: {shape}"
        );
        assert!(
            line.ends_with("… +2 more entries"),
            "two over-length strings were dropped and the line must say so: {shape}"
        );
    }

    /// Regression test for the same issue (the `MAX_SCALARS` half): scalars past the
    /// eighth are cut by `.take(MAX_SCALARS)` and the line must count them.
    ///
    /// Built so only the COUNT cap can bite: eleven short numbers, none over-length.
    #[test]
    fn describe_payload_shape_counts_scalars_past_the_cap() {
        let mut obj = serde_json::Map::new();
        for i in 0..11 {
            obj.insert(format!("n{i:02}"), serde_json::json!(i));
        }
        let shape = describe_payload_shape(&serde_json::Value::Object(obj)).expect("a shape");
        let line = scalar_line(&shape);

        assert!(
            line.contains("n07=7"),
            "the eighth scalar is the last shown: {shape}"
        );
        assert!(!line.contains("n08="), "the ninth is cut: {shape}");
        assert!(
            line.ends_with("n07=7, … +3 more entries"),
            "three scalars were cut past the cap and the line must say so: {shape}"
        );
    }

    /// Both bounds firing on one object must report the SUM — a counter that only sees one
    /// of the two filters would pass each single-bound test above and fail here.
    ///
    /// **The order is load-bearing: the over-length strings come FIRST.** Were they last, a
    /// mutant in which over-length strings occupy cap slots would still show the first eight
    /// numbers and count the same total, and survive; with them first, such a mutant shows
    /// only six numbers, so `n07=7` going missing is what reds it.
    #[test]
    fn describe_payload_shape_counts_both_scalar_bounds_together() {
        let mut obj = serde_json::Map::new();
        obj.insert("long_a".into(), serde_json::json!("a".repeat(61)));
        obj.insert("long_b".into(), serde_json::json!("b".repeat(61)));
        for i in 0..10 {
            obj.insert(format!("n{i:02}"), serde_json::json!(i));
        }
        let shape = describe_payload_shape(&serde_json::Value::Object(obj)).expect("a shape");
        let line = scalar_line(&shape);

        assert!(
            line.contains("n07=7") && !line.contains("n08="),
            "over-length strings must not take cap slots: exactly n00..n07 show: {shape}"
        );
        // 10 numbers, cap 8 -> 2 cut; 2 over-length -> 2 filtered; 4 in all.
        assert!(
            line.ends_with("… +4 more entries"),
            "2 over-length + 2 past the cap must read +4: {shape}"
        );
    }

    /// The one-level descent into an object-valued key is bounded by the same two limits
    /// (`flat_scalars`) and must disclose them too — a second site, killed independently of
    /// the top-level one.
    #[test]
    fn describe_payload_shape_counts_what_it_drops_inside_a_nested_object() {
        let mut hints = serde_json::Map::new();
        for i in 0..10 {
            hints.insert(format!("h{i:02}"), serde_json::json!(i));
        }
        hints.insert("long".into(), serde_json::json!("l".repeat(61)));
        let val = serde_json::json!({"count": 3, "hints": hints});
        let shape = describe_payload_shape(&val).expect("an object has a shape");

        // 10 numbers, cap 8 -> 2 cut; 1 over-length -> 1 filtered; 3 in all, INSIDE the braces.
        assert!(
            scalar_line(&shape).contains("h07=7, … +3 more entries}"),
            "the nested object's own elision must be counted inside its braces: {shape}"
        );
        assert_eq!(
            shape.matches("more entr").count(),
            1,
            "the top level dropped nothing, so it must add no marker of its own: {shape}"
        );

        // An object whose scalars are ALL over-length used to vanish from the line; it must
        // now say what it held back.
        let all_long = serde_json::json!({"id": "x", "meta": {"a": "z".repeat(100)}});
        let shape = describe_payload_shape(&all_long).expect("an object has a shape");
        assert!(
            scalar_line(&shape).contains("meta: {… +1 more entry}"),
            "a nested object with only over-length scalars must not be silently empty: {shape}"
        );
    }

    /// The `|| dropped > 0` half of the top-level condition: when EVERY scalar candidate was
    /// dropped nothing is left to show, and a line that vanishes is the silent drop again.
    /// The array is there so the `arrays:` line sits above and `scalar_line` cannot be
    /// satisfied by it.
    #[test]
    fn describe_payload_shape_reports_a_line_whose_every_scalar_was_dropped() {
        let val = serde_json::json!({"body": "x".repeat(100), "tags": [1]});
        let shape = describe_payload_shape(&val).expect("an object has a shape");

        assert_eq!(
            scalar_line(&shape).trim(),
            "… +1 more entry",
            "the only scalar was over-length: the line must exist and say so: {shape}"
        );
    }

    /// The unit is the line's ENTRY, not the scalar: an object-valued key cut by the top-level
    /// cap is one entry however many scalars it held, and the label must not claim three
    /// (testing-discipline: a count arrives with its unit). Eight numbers fill the cap, so the
    /// trailing `hints` group is the one cut.
    #[test]
    fn describe_payload_shape_counts_a_cut_nested_object_as_one_entry() {
        let mut obj = serde_json::Map::new();
        for i in 0..8 {
            obj.insert(format!("n{i:02}"), serde_json::json!(i));
        }
        obj.insert("hints".into(), serde_json::json!({"a": 1, "b": 2, "c": 3}));
        let shape = describe_payload_shape(&serde_json::Value::Object(obj)).expect("a shape");
        let line = scalar_line(&shape);

        assert!(
            !line.contains("hints:"),
            "the group is past the cap and must not show: {shape}"
        );
        assert!(
            line.ends_with("n07=7, … +1 more entry"),
            "one entry (the hints group) was cut, however many scalars it held: {shape}"
        );
    }

    /// Regression test for
    /// `docs/issues/2026-10-17-describe-payload-shape-item-keys-cut-without-a-count.md`: the
    /// array branch's `item keys:` line is cut at `MAX_ITEM_KEYS` and must say how many keys
    /// it held back, as the object key line does. Thirty keys against a cap of 24: the line
    /// names `k00..k23` and ends `… +6 more entries`.
    ///
    /// The marker is asserted on the `item keys:` line itself, so nothing else in the shape
    /// can satisfy it. An array of exactly 24 keys, below, must stay silent (ADR
    /// `2026-08-27-negative-results-name-their-scope` clause 2).
    #[test]
    fn describe_payload_shape_counts_item_keys_past_the_cap() {
        let item_of = |n: usize| {
            let mut item = serde_json::Map::new();
            for i in 0..n {
                item.insert(format!("k{i:02}"), serde_json::json!(i));
            }
            serde_json::Value::Array(vec![serde_json::Value::Object(item)])
        };

        let shape = describe_payload_shape(&item_of(30)).expect("an array has a shape");
        let line = scalar_line(&shape);
        assert!(
            line.trim_start().starts_with("item keys:")
                && line.contains("k23")
                && !line.contains("k24"),
            "exactly the first 24 keys are named: {shape}"
        );
        assert!(
            line.ends_with(", … +6 more entries"),
            "six keys were cut past the cap and the line must say so: {shape}"
        );

        let at_the_cap = describe_payload_shape(&item_of(24)).expect("an array has a shape");
        assert!(
            !at_the_cap.contains("more entr"),
            "nothing was cut at exactly the cap, so nothing may be claimed: {at_the_cap}"
        );
    }

    /// Regression test for
    /// `docs/issues/2026-10-17-describe-payload-shape-nested-object-drops-its-non-scalar-members.md`:
    /// inside the one-level descent nothing else names an array, object or null member, so the
    /// group must count them. At the top level `keys:` and `arrays:` already name them, which
    /// is why the top level stays silent about them (see the silence test below).
    #[test]
    fn describe_payload_shape_counts_a_nested_objects_non_scalar_members() {
        let val = serde_json::json!({
            "hints": {"a": 1, "list": [1, 2, 3], "deep": {"x": 1}, "n": null}
        });
        let shape = describe_payload_shape(&val).expect("an object has a shape");
        assert!(
            scalar_line(&shape).ends_with("hints: {a=1, … +3 more entries}"),
            "an array, an object and a null were held back and the group must say so: {shape}"
        );

        // A group with NO scalar member must not vanish: it still has entries to report.
        let all_non_scalar = serde_json::json!({"id": "x", "meta": {"l": [1], "e": {}}});
        let shape = describe_payload_shape(&all_non_scalar).expect("an object has a shape");
        assert!(
            scalar_line(&shape).ends_with("meta: {… +2 more entries}"),
            "a nested object holding only non-scalars must not be silently empty: {shape}"
        );

        // The three removals in one group sum: 9 numbers (cap 8 -> 1 cut), 1 over-length
        // string, 1 array = 3 entries held back.
        let mut hints = serde_json::Map::new();
        for i in 0..9 {
            hints.insert(format!("h{i:02}"), serde_json::json!(i));
        }
        hints.insert("long".into(), serde_json::json!("l".repeat(61)));
        hints.insert("list".into(), serde_json::json!([1]));
        let shape =
            describe_payload_shape(&serde_json::json!({ "hints": hints })).expect("a shape");
        assert!(
            scalar_line(&shape).contains("h07=7, … +3 more entries}"),
            "cut + over-length + non-scalar must sum to 3 inside the braces: {shape}"
        );
    }

    /// The marker is for a bitten line only (ADR `2026-08-27-negative-results-name-their-scope`
    /// clause 2): a payload whose every scalar fits, plus fields that are not scalars at all
    /// (an array, a null, an empty object), reports nothing — a marker on every payload
    /// stops being read.
    #[test]
    fn describe_payload_shape_is_silent_when_no_scalar_was_dropped() {
        let val = serde_json::json!({
            "id": "T-1",
            "count": 2,
            "ok": true,
            "items": [1, 2, 3],
            "nothing": null,
            "empty": {},
            "hints": {"unindexed_files": 1},
        });
        let shape = describe_payload_shape(&val).expect("an object has a shape");

        assert!(
            !shape.contains("more entr"),
            "nothing scalar was dropped, so nothing may be claimed: {shape}"
        );
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
