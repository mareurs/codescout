---
status: open
opened: 2026-05-09
closed:
severity: medium
owner: marius
related: []
tags: ["read_file", "buffer", "pagination", "offset"]
---

# BUG: `read_file(@buf, start_line=N, end_line=M)` returned empty content past buffer midpoint

## Summary

Reading an `@tool_*` buffer's line range past roughly the midpoint returns empty `content` with no error. The same buffer reads correctly with smaller `start_line`. Total bytes reported correctly; only `start_line` past a threshold triggers the empty response.

## Symptom (Effect)

```
read_file(path="@tool_xxx", start_line=10, end_line=20)   # buffer has 200 lines
→ { "content": "<populated>", "lines": 10 }

read_file(path="@tool_xxx", start_line=150, end_line=160) # same buffer
→ { "content": "", "lines": 0 }
```

No error returned in the empty case.

## Reproduction

Cache any large tool response (a buffer with ≥ 100 lines), then query a line range past roughly the midpoint.

## Environment

- Date observed: 2026-05-09
- Tool: `mcp__codescout__read_file` with `start_line` / `end_line` against `@tool_*` buffer

## Root cause

Unknown. Possible pagination or buffer-chunk offset miscalculation. Buffer total bytes reported correctly; only `start_line` past a threshold triggers the empty response.

## Evidence

Multiple instances during the i1-refactor session log (F-2 in `docs/trackers/archive/i1-session-friction.md`). Same buffer + start_line < N succeeds, start_line > N returns empty.

## Hypotheses tried

1. **Hypothesis:** Buffer is chunked internally and the chunk-walker may not advance past the first chunk. **Test:** Not yet verified. **Verdict:** Deferred.

## Fix

Open. Investigation queued.

## Tests added

N/A — open.

## Workarounds

- Read filesystem path directly (`force=true`) — skips the buffer layer entirely.
- Fetch from line 1 in two passes (line 1 → midpoint, then midpoint+1 → end).

## Resume

Concrete next action: locate `@tool_*` buffer handler in `src/tools/buffer/`, identify the chunk-walker (if any), write a regression test that caches a 200-line response then reads `start_line=150, end_line=160`. If the threshold is reproducible, instrument the chunk-walker with `tracing::debug!` showing each chunk boundary.

## References

- Originally tracked as **#3** in `docs/issues/bug-tracker.md` (retired after migration to per-file system).
- Promoted from **F-2** in `docs/trackers/archive/i1-session-friction.md`.
