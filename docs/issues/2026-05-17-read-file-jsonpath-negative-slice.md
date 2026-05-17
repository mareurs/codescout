---
status: open
opened: 2026-05-17
closed:
severity: low
owner: marius
related: ["2026-05-09-read-file-json-path-array-elements.md"]
tags: ["read_file", "json_path", "jsonpath", "negative-slice"]
---

# BUG: `read_file(json_path="$.symbols[-3:]")` rejects negative slice syntax

## Summary

`read_file` with a `json_path` using Python-style negative slice syntax (`[-3:]`, presumably also `[-N]`, `[:-1]`, `[::-1]`) fails with a "path segment not found" error. The handler appears to accept only non-negative integer indexing, but the error message hints at array length rather than syntax support, making the failure mode misleading.

## Symptom (Effect)

```
read_file(path="@tool_3785903c", json_path="$.symbols[-3:]")
→ error: path segment '[-3:]' not found at '$.symbols[-3:]'
  hint: Available: array with 83 elements (0..82)
```

The hint suggests the segment was *looked up* and not found — but `[-3:]` is a slice operator, not an index. The user was asking for "last 3 elements", which the path syntax does not support.

## Reproduction

```
1. Call any tool returning a json buffer with an array (e.g. symbols on a large file)
2. read_file(path="@tool_xxx", json_path="$.<array>[-3:]")
3. Observe the rejection.
```

Captured live: `tool_call_id=21618`, `cc_session_id=42874b1a-1ef5-44ce-ad64-4eb5b84cf93f`.

```sql
SELECT input_json, error_msg FROM tool_calls WHERE id=21618;
-- {"json_path":"$.symbols[-3:]","path":"@tool_3785903c"}
-- path segment '[-3:]' not found at '$.symbols[-3:]' — hint: Available: array with 83 elements (0..82)
```

## Environment

- Branch: `experiments` @ `88e38bfe854a2d9e7cfb57d5c6b6d64fee623459`
- MCP transport: stdio
- Tool: `mcp__codescout__read_file`

## Root cause

Unknown — under investigation. Suspected location: `parse_json_path_segments` / `resolve_json_segment` in `src/tools/file_summary/file_summary.rs` (cited in the related bug `2026-05-09-read-file-json-path-array-elements.md` as the json_path machinery). Negative-index + slice syntax likely never implemented; the error path catches the unparsed segment with a generic "not found" message instead of "unsupported syntax".

## Evidence

### `pika_observations` row

```sql
SELECT * FROM pika_observations WHERE id=58;
-- 58 | tool_call_id=21618 | kind=tool_bug | subkind=jsonpath_negative_slice
--    | verdict=NULL | severity=low | recurrence=1 | cc_session_id=42874b1a-...
```

Surfaced by Pika scan 2026-05-17 (Phase 2b, scope `cc_session_id=42874b1a-…`).

## Hypotheses tried

1. **Hypothesis:** Negative-index parsing is supported but the slice form `[-N:]` is not.
   **Test:** Ran `read_file(path="@tool_37b05135", json_path="$.symbols[-1]")` against a fresh 23-element symbols buffer.
   **Verdict:** Rejected. Error returned: `path segment '[-1]' not found at '$.symbols[-1]'`. Negative single-index fails identically to the slice form — the json_path parser does not handle negative anything.
   **Evidence link:** See Evidence below; same error shape as the original `[-3:]` case.
## Fix

Plan: either (a) implement Python-style negative slice in `resolve_json_segment` (least surprise — matches jsonpath dialects like jsonpath-rust 0.5+), or (b) reject the syntax with a clearer error ("unsupported json_path syntax: slices are not supported, use start_line/end_line or a bounded index"). Option (b) is the smaller change and matches the project's "explicit failure mode" pattern.

## Tests added

None yet. Regression test target on fix: `tools::read_file::tests::read_file_buffer_json_path_negative_slice_returns_clear_error` (or `_returns_last_n_elements` if implementing).

## Workarounds

- Bound the slice manually given known array length: for a 83-element array, use `$.symbols[80]`, `$.symbols[81]`, `$.symbols[82]` in three calls — or fetch full array and pick in the agent.
- Use `symbols(path=..., offset=80, limit=3)` for symbols specifically — codescout-native pagination handles tail-of-array via offset.

## Resume

Probe complete (2026-05-17). Negative index AND slice both unsupported — single axis, not two. Decision point: option (a) implement negative-index + slice in `resolve_json_segment`, or option (b) reject with a clearer error message (`"unsupported json_path syntax: negative indices and slices not supported"`). Option (b) is the smaller change; option (a) matches jsonpath-rust 0.5+ semantics. Code lives in `src/tools/file_summary/file_summary.rs` near `parse_json_path_segments` and `resolve_json_segment`.
## References

- Related: `docs/issues/2026-05-09-read-file-json-path-array-elements.md` (wontfix; covers positive-index + property — distinct feature).
- Surfaced by: Pika scan U-bound `cc_session_id=42874b1a-1ef5-44ce-ad64-4eb5b84cf93f`, `pika_observations.id=58`.
