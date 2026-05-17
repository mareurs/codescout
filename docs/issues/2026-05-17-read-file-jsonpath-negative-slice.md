---
status: fixed
opened: 2026-05-17
closed: 2026-05-18
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

Shipped on `experiments` 2026-05-18 across 4 commits:

- `59f6b53c` — `feat(file_summary): add Segment enum for typed json_path grammar`
- `ff4c3301` — `feat(file_summary): add parse_segments_v2 returning Vec<Segment>`
- `932eb443` — `feat(file_summary): add resolve_segment_v2 + extract_json_path_v2 scaffold`
- `f6eb7ed1` — `refactor(file_summary): cut over to typed Segment grammar`

Implementation in `src/tools/file_summary/file_summary.rs`. The stringly-typed segment parser was replaced with a typed `enum Segment { Key, Index, NegIndex, NegSliceFrom }`. Resolver returns `Result<Cow<'a, Value>, RecoverableError>`. Walk in `extract_json_path` threads `Cow` through the segment chain, flipping to owned on first slice.

Live MCP verification (2026-05-18): 4/4 probes pass against `src/server.rs` symbols buffer — `[-1]` returns last symbol, `[-3:]` returns array of 3, `[1:3]` rejects with `"unsupported json_path segment '[1:3]'"`, `[-9999]` rejects with `"index -9999 out of bounds for array of length 23"`.
## Tests added

In `src/tools/file_summary/tests.rs`:

**Parser (13 tests):** `parse_empty_path_returns_empty_segments`, `parse_root_only`, `parse_negative_single_index`, `parse_negative_slice_from`, `parse_chained_negative_after_positive`, `parse_top_level_negative_index`, `parse_rejects_positive_slice`, `parse_rejects_slice_with_step`, `parse_rejects_open_end_positive`, `parse_rejects_negative_zero`, `parse_rejects_non_integer_bracket`, `parse_rejects_negative_zero_slice`, `parse_rejects_positive_sign`.

**Resolver (8 tests):** `extract_root_returns_parsed`, `extract_top_level_negative_index`, `extract_negative_index_returns_last_element`, `extract_negative_slice_returns_tail`, `extract_negative_index_oob_returns_clear_error`, `extract_negative_slice_oob_returns_clear_error`, `extract_mid_path_slice_then_index`, `extract_unsupported_syntax_distinguished_from_not_found`.

**Regression pin (existing, kept green):** `read_file_buffer_json_path_array_element_returns_value` at `src/tools/read_file.rs:1003` continues to pass — positive index + property access (`$.symbols[0].body`) is unchanged.
## Workarounds

- Bound the slice manually given known array length: for a 83-element array, use `$.symbols[80]`, `$.symbols[81]`, `$.symbols[82]` in three calls — or fetch full array and pick in the agent.
- Use `symbols(path=..., offset=80, limit=3)` for symbols specifically — codescout-native pagination handles tail-of-array via offset.

## Resume

Fixed and verified. No further action on `experiments`. Standard Ship Sequence next: cherry-pick the 4 commits (`59f6b53c`, `ff4c3301`, `932eb443`, `f6eb7ed1`) to `master`, then `git mv` this file to `docs/issues/archive/`.
## References

- Related: `docs/issues/2026-05-09-read-file-json-path-array-elements.md` (wontfix; covers positive-index + property — distinct feature).
- Surfaced by: Pika scan U-bound `cc_session_id=42874b1a-1ef5-44ce-ad64-4eb5b84cf93f`, `pika_observations.id=58`.
