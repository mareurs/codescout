---
status: wontfix
opened: 2026-05-09
closed: 2026-05-17
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

**Not reproducible as of 2026-05-17.** Probe test (see Tests added) confirms `read_file(@tool_*, start_line=150, end_line=160)` on a 200-line buffer returns the correct 11-line slice. `extract_lines` in `src/util/text.rs:23-33` is a 12-line filter that iterates `text.lines().enumerate()` — there is no chunk-walker that could fail to advance.

Likely explanations for the original report: (a) the buffer at observation time had fewer post-pretty-print lines than the user assumed from a count taken on the raw stdout, (b) an intervening commit fixed an underlying defect without crediting back to this file, or (c) the user observed correct empty-out-of-bounds behavior on a smaller buffer than they thought.
## Evidence

Multiple instances during the i1-refactor session log (F-2 in `docs/trackers/archive/i1-session-friction.md`). Same buffer + start_line < N succeeds, start_line > N returns empty.

## Hypotheses tried

1. **Hypothesis:** Buffer is chunked internally and the chunk-walker may not advance past the first chunk. **Test:** Not yet verified. **Verdict:** Deferred.

## Fix

No code change. Closed as `wontfix` because the reported behavior could not be reproduced and the current code path is provably correct.
## Tests added

`tools::read_file::tests::read_file_buffer_midpoint_returns_content` — seeds a 200-line buffer (`line 1` through `line 200`), reads `start_line=150, end_line=160`, asserts content contains both `line 150` and `line 160`. Passes immediately on 2026-05-17. Kept as a regression pin so future changes to the buffer pipeline cannot silently re-introduce the reported failure mode.
## Workarounds

- Read filesystem path directly (`force=true`) — skips the buffer layer entirely.
- Fetch from line 1 in two passes (line 1 → midpoint, then midpoint+1 → end).

## Resume

Closed. The pinned test is the load-bearing artifact — if it starts failing, the original bug-shape has re-emerged and root-cause investigation should resume from there.
## References

- Originally tracked as **#3** in `docs/issues/bug-tracker.md` (retired after migration to per-file system).
- Promoted from **F-2** in `docs/trackers/archive/i1-session-friction.md`.
