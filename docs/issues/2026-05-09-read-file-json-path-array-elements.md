---
status: wontfix
opened: 2026-05-09
closed: 2026-05-17
severity: medium
owner: marius
related: []
tags: ["read_file", "json_path", "buffer", "jsonpath"]
---

# BUG: `read_file(@buf, json_path="$.array[N].field")` returned 0 lines for array-element paths

## Summary

`read_file(path="@tool_xxx", json_path="$.symbols[0].body")` returns `lines: 0` even though the buffer contains a populated `body` at `symbols[0]`. Object access on the same buffer (e.g. `json_path="$.context"`) works correctly. Array-index-with-property access fails differently from plain object property access.

## Symptom (Effect)

```
read_file(path="@tool_xxx", json_path="$.symbols[0].body")
→ { "lines": 0, "content": "" }
```

vs

```
read_file(path="@tool_xxx", json_path="$.symbols")
→ populated array
```

vs

```
read_file(path="@tool_xxx", json_path="$.context")
→ populated object/string
```

## Reproduction

Cache any tool response with an array-of-objects under a key, then call `read_file` with a json_path that combines array index + property accessor on the buffer.

## Environment

- Date observed: 2026-05-09
- Tool: `mcp__codescout__read_file` with `json_path` against `@tool_*` buffer

## Root cause

**Not reproducible as of 2026-05-17.** Probe test (see Tests added) confirms `read_file(@tool_*, json_path="$.symbols[0].body")` returns the inner string value correctly. `extract_json_path` in `src/tools/file_summary/file_summary.rs:419` handles array indexing via `parse_json_path_segments` + `resolve_json_segment`, and the function-level comment explicitly cites `$.symbols[0].body` as the canonical working case. There is also an existing passing unit test `extract_json_path_array_index` using `$.users[0]`.

Likely explanations for the original report: (a) the value at the json path was an empty string (correct return — misread as "broken"), or (b) an intervening commit fixed an underlying defect without crediting back here.
## Evidence

Multiple instances during the i1-refactor session log (F-1 in `docs/trackers/archive/i1-session-friction.md`). Same buffer + different json_path → different success.

## Hypotheses tried

1. **Hypothesis:** The buffer ref handler may strip array indexing. **Test:** Not yet verified — would require inspecting `src/tools/buffer/` and the json_path evaluator. **Verdict:** Deferred.

## Fix

No code change. Closed as `wontfix` because the reported behavior could not be reproduced and the current code path is provably correct + already unit-tested for array indexing.
## Tests added

`tools::read_file::tests::read_file_buffer_json_path_array_element_returns_value` — seeds an `@tool_*` buffer with `{"symbols":[{"name":"alpha","body":"fn alpha() {}"},...]}` (exact shape from the original report), queries `$.symbols[0].body`, asserts the response content contains `fn alpha`. Passes immediately on 2026-05-17. Kept as a regression pin against the original bug shape.
## Workarounds

- Use `read_file(path=@tool_, start_line, end_line)` and parse manually.
- Use `read_file(path="<real-filesystem-path>", force=true)` and traverse via json_path on the on-disk file.
- Fetch the entire array via `json_path="$.array"` then walk in subsequent calls.

## Resume

Closed. Pinned test is the regression guard.
## References

- Originally tracked as **#2** in `docs/issues/bug-tracker.md` (retired after migration to per-file system).
- Promoted from **F-1** in `docs/trackers/archive/i1-session-friction.md`.
