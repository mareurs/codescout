---
status: fixed
opened: 2026-05-09
closed: 2026-05-17
severity: medium
owner: marius
related: []
tags: ["grep", "buffer", "router", "false-negative"]
kind: bug
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

`Grep::call` had no buffer-ref handling. It passed `@tool_*` / `@cmd_*` / `@file_*` paths through `validate_read_path`, which either rejected the ref outright (no active project) or resolved it to `<project_root>/@tool_*` (non-existent path). `WalkBuilder` then returned 0 matches silently — making the failure look like a search miss rather than a missing feature.

A secondary effect: `is_identifier_pattern` runs unconditionally and injects the "Pattern looks like a symbol name" suggestion regardless of result count. That made the empty result look like an active redirect by a symbol-name router, when in fact no routing was happening — grep simply searched a path that didn't exist.
## Evidence

Multiple instances during the i1-refactor session log (F-11 in `docs/trackers/archive/i1-session-friction.md`). Same buffer + patterns that should match → no matches; identifier-shape always triggers the same router hint.

## Hypotheses tried

1. **Hypothesis:** Router intercept before grep dispatches. **Test:** Not yet verified — would require tracing the grep tool's dispatch path for `@tool_*` refs. **Verdict:** Deferred.
2. **Hypothesis:** Grep against `@tool_*` doesn't expand the ref the same way `read_file` does. **Test:** Not yet verified. **Verdict:** Deferred.

## Fix

Branch on `@` prefix at the top of `Grep::call` and route to a new `grep_in_buffer` helper that:

1. Loads the buffer via `ctx.output_buffer.get(path).stdout`.
2. Pretty-prints `@tool_*` content (matches the `read_from_buffer` convention so identifier-shaped strings sit on their own lines).
3. Iterates lines, runs the same regex (with literal-fallback) used by the filesystem path, emits matches in the same response shape (`file_groups` for simple mode, `matches[]` for context mode).

A `build_grep_regex` helper was extracted alongside, used by the new buffer path.
## Tests added

`tools::grep::tests::grep_buffer_ref_matches_content_in_tool_buffer` — seeds an `@tool_*` buffer with `foo_bar_baz` inside a JSON payload, asserts grep finds it. Pre-fix: panicked on `validate_read_path` with `relative path '@tool_*' requires an active project`. Post-fix: returns `total >= 1`.
## Workarounds

- Use `read_file(path=@tool_, json_path=…)` for structured fields.
- Use `read_file(@tool_, start_line, end_line)` for sequential inspection.
- Reserve `grep` for filesystem paths.

## Resume

Done — fix landed, regression test pinned. If a future bug surfaces with `@cmd_*` or `@file_*` refs specifically, the same `grep_in_buffer` path handles them but they're not exercised by the probe test yet — add another seed if needed.
## References

- Originally tracked as **#4** in `docs/issues/bug-tracker.md` (retired after migration to per-file system).
- Promoted from **F-11** in `docs/trackers/archive/i1-session-friction.md`.
