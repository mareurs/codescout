---
status: open
opened: 2026-05-09
closed:
severity: medium
owner: marius
related: []
tags: ["grep", "buffer", "router", "false-negative"]
---

# BUG: `grep(pattern, path="@tool_*")` false-negatives on strings present in the buffer

## Summary

`grep` on an `@tool_*` buffer returns `{"matches": [], "total": 0}` for patterns verifiably present in the buffer (confirmed via `read_file` line-range on the same buffer immediately afterward). The tool emits a misleading suggestion: *"Pattern looks like a symbol name. Consider: symbols(name='…')."* Likely two layered causes: grep on `@tool_*` may not operate on raw buffer text the way it does on filesystem paths, and the symbol-name-suggestion router may intercept queries containing underscores / identifier-shaped tokens before the search runs.

## Symptom (Effect)

```
grep("foo_bar", path="@tool_xxx")
→ { "matches": [], "total": 0,
    "hint": "Pattern looks like a symbol name. Consider: symbols(name='foo_bar')." }
```

Verify with `read_file(@tool_xxx, start_line=1, end_line=200)` — output contains `foo_bar` verbatim.

## Reproduction

1. Cache any tool response that contains an identifier-shaped string (e.g. `module_name`, `tool_id`).
2. Call `grep(pattern="<that string>", path="@tool_xxx")`.
3. Observe empty matches + the symbol-router hint.

## Environment

- Date observed: 2026-05-09
- Tool: `mcp__codescout__grep` against `@tool_*` buffer

## Root cause

Two layered causes (hypothesis):

1. `grep` on `@tool_*` may not operate on raw buffer text the way it does on filesystem paths.
2. The symbol-name-suggestion router may intercept queries containing underscores / identifier-shaped tokens before the search runs.

## Evidence

Multiple instances during the i1-refactor session log (F-11 in `docs/trackers/archive/i1-session-friction.md`). Same buffer + patterns that should match → no matches; identifier-shape always triggers the same router hint.

## Hypotheses tried

1. **Hypothesis:** Router intercept before grep dispatches. **Test:** Not yet verified — would require tracing the grep tool's dispatch path for `@tool_*` refs. **Verdict:** Deferred.
2. **Hypothesis:** Grep against `@tool_*` doesn't expand the ref the same way `read_file` does. **Test:** Not yet verified. **Verdict:** Deferred.

## Fix

Open. Investigation queued.

## Tests added

N/A — open.

## Workarounds

- Use `read_file(path=@tool_, json_path=…)` for structured fields.
- Use `read_file(@tool_, start_line, end_line)` for sequential inspection.
- Reserve `grep` for filesystem paths.

## Resume

Concrete next action: locate the grep tool's dispatch in `src/tools/search/grep.rs`, audit how it handles `@tool_*` ref expansion, and write a regression: cache a buffer containing `foo_bar_baz`, then assert `grep("foo_bar_baz", path="@tool_xxx")` returns a match. If the router intercept is the cause, gate it on filesystem-path lookups only.

## References

- Originally tracked as **#4** in `docs/issues/bug-tracker.md` (retired after migration to per-file system).
- Promoted from **F-11** in `docs/trackers/archive/i1-session-friction.md`.
