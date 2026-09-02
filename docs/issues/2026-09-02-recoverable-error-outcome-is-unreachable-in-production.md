---
id: '60b006d8acef1ff9'
kind: bug
status: open
title: 'BUG: recoverable_error is never written to usage.db — 0 rows in 57k calls, and two queries filter on it'
tags:
- cluster/declared-not-wired
- usage-db
- telemetry
- error-handling
- analyze-usage
- misleading-instrument
closed: null
opened: 2026-09-02
owner: marius
related: []
severity: medium
---

# BUG: `recoverable_error` is never written — 0 rows in 57k calls, and two queries filter on it

## Summary

`usage.db`'s `outcome` column is documented and queried as a three-value
taxonomy — `success | error | recoverable_error`. In 30 days of live traffic it
holds **two** values. `recoverable_error` is unreachable in production because
the recorder classifies before the router that would distinguish it, so the
column cannot tell a forced-retry (a guard firing correctly) from a hard
failure.

## Symptom (Effect)

**measured 2026-09-02**, `.codescout/usage.db`, 30-day rolling window:

```
success              54,895
error                 2,536
recoverable_error         0
```

Two SQL queries filter on the dead value and therefore evaluate a union whose
second term is always empty:

- `src/usage/db.rs:989` (`query_stats`)
- `src/usage/db.rs:1079` (`recent_errors`)

`/analyze-usage` reads both. Its error breakdown silently reports hard failures
and Iron-Law refusals as one class.

## Root cause

Ordering, at `src/server.rs:1118-1145`. `record_content` wraps
`tool.call_content(...)`, which returns `Err(RecoverableError)`. The recorder's
`Err(e)` arm classifies that as `"error"`. `route_tool_error` — the function
that converts a `RecoverableError` into an `isError: false` response — runs
**afterward**, at `:1145`.

`classify_content_result` (`src/usage/mod.rs:163`) *can* emit
`"recoverable_error"`, but only for a tool that returns `Ok(content)` whose JSON
body carries a top-level `error` key. No live tool does that: codescout's
recoverable errors travel as `Err`, per `get_guide("error-handling")`.

*Ordering read directly this session at the cited lines. The 57k-row
distribution is a subagent measurement, re-derivable from the query below.*

## Evidence

### The tests pin the value without traversing the path that kills it

`src/usage/mod.rs:236` and `src/usage/db.rs:1150` both exercise
`classify_content_result` **directly**. Neither goes through
`record_content` → `route_tool_error`, which is the ordering that makes the
value unreachable. This is the project's own *"loudness is a property of a
PATH"* law: the classifier works, and nothing reaches it.

### The consequence is not cosmetic

`err_family` (taxonomy at `src/usage/db.rs:551-603`) classifies 2,465 of 2,537
error rows, and the top families are Iron-Law refusals: `il3_pipe_to_trimmer`
594, `il3_shell_on_source` 320, `il1_read_overlaps_symbol` 260,
`librarian_managed_artifact` 229. Those are **guards working**. Under `outcome`
they are indistinguishable from crashes — so any analysis ranking shapes by
`outcome='error'` ranks the *most heavily guarded* shapes highest and reads
existing enforcement as unmet need.

## Hypotheses tried

1. **Hypothesis:** no recoverable error occurred in the window.
   **Test:** `SELECT err_family, COUNT(*) ... GROUP BY err_family` — 2,465
   classified rows, dominated by Iron-Law refusal families that are recoverable
   by construction.
   **Verdict:** rejected. They occurred and were recorded as `error`.

## Fix

**Not yet applied.** Two candidate shapes:

1. Classify **after** `route_tool_error`, so the recorder sees the routed
   disposition rather than the raw `Err`. Smallest change, correct value.
2. Have `record_content` downcast the error to `RecoverableError` in its `Err`
   arm and classify on that.

Whichever lands, the regression test must traverse `record_content` end to end
— asserting a stored row's `outcome`, not `classify_content_result`'s return —
because the existing tests already pass against the broken path.

Consider also whether `err_family` should subsume `outcome`'s error split
entirely; it is the taxonomy that actually carries information, and a
three-value column whose third value is dead is a worse instrument than a
two-value one that admits it.

## Tests added

None yet. See *Fix* for the shape the regression test must take — a test that
calls the classifier directly reproduces the existing blind spot exactly.

## Workarounds

Use `err_family` rather than `outcome` when distinguishing guard-fired from
crashed. Treat `/analyze-usage`'s error counts as "errors and refusals
combined".

## Resume

Read `src/server.rs:1118-1145` — specifically the `Err(e)` arm of
`record_content` and the position of `route_tool_error` relative to it. Move
classification after the routing, then write a test that drives a real
`RecoverableError`-returning tool through `record_content` and asserts the
stored row reads `recoverable_error`. Confirm it reds before the change.

## References

- `src/server.rs:1118-1145` — the ordering
- `src/usage/mod.rs:163` — `classify_content_result`
- `src/usage/mod.rs:236`, `src/usage/db.rs:1150` — the tests that bypass the path
- `src/usage/db.rs:989`, `:1079` — the two queries on the dead value
- `src/usage/db.rs:551-603` — `err_family` taxonomy
- Distribution query: `SELECT outcome, COUNT(*) FROM tool_calls GROUP BY outcome` against `.codescout/usage.db`

