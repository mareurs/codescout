---
status: open
opened: 2026-05-09
closed:
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

Unknown — likely a jsonpath dispatch difference between array-index-with-property access vs. plain object property access. Hypothesis: the buffer-aware jsonpath path may treat `$.symbols[0]` and `$.symbols[0].field` differently from `$.field` when reading from an `@tool_*` ref.

## Evidence

Multiple instances during the i1-refactor session log (F-1 in `docs/trackers/archive/i1-session-friction.md`). Same buffer + different json_path → different success.

## Hypotheses tried

1. **Hypothesis:** The buffer ref handler may strip array indexing. **Test:** Not yet verified — would require inspecting `src/tools/buffer/` and the json_path evaluator. **Verdict:** Deferred.

## Fix

Open. Investigation queued.

## Tests added

N/A — open.

## Workarounds

- Use `read_file(path=@tool_, start_line, end_line)` and parse manually.
- Use `read_file(path="<real-filesystem-path>", force=true)` and traverse via json_path on the on-disk file.
- Fetch the entire array via `json_path="$.array"` then walk in subsequent calls.

## Resume

Concrete next action: open `src/tools/buffer/handlers.rs` (or wherever `@tool_*` ref expansion lives), search for the json_path evaluator wiring, and write a minimal repro test: cache a response with `[{"body": "x"}]`, query `$.[0].body` and `$.body` (on a flat ref), see which path returns 0 lines.

## References

- Originally tracked as **#2** in `docs/issues/bug-tracker.md` (retired after migration to per-file system).
- Promoted from **F-1** in `docs/trackers/archive/i1-session-friction.md`.
