---
id: '9fabc6ccc51d865f'
kind: bug
status: fixed
title: 'BUG: run_command''s unfiltered_output ref carries no size/emptiness signal — an agent cannot tell a 2-line buffer from a 20,000-line one without a blind round-trip'
tags:
- run_command
- progressive-disclosure
- usability
- cluster/capped-result-presented-as-complete
closed: 2026-08-26
opened: 2026-08-26
owner: marius
related: []
severity: low
---

## Summary

`run_command`'s unfiltered-tee mechanism (`docs/superpowers/plans/2026-03-04-unfiltered-output-capture.md`)
attaches an `unfiltered_output: "@cmd_xxx"` ref whenever a piped command ends in a
known filter (`grep`, `head`, `tail`, `sed`, …) and the pre-filter capture is
non-empty. The response carries no size, line-count, or emptiness signal for
either the filtered `stdout` or the unfiltered buffer — so `{"exit_code": 1,
"unfiltered_output": "@cmd_xxx"}` is the response whether that buffer holds 2
lines or 20,000, and whether the filter matched nothing or crashed. An agent has
no way to judge whether reading the ref is worth a round-trip.

## Symptom (Effect)

```
run_command("cat /home/marius/work/claude/codescout/.codescout/project.toml 2>/dev/null | grep -A2 '\[peer\]'")
→ {"exit_code": 1, "unfiltered_output": "@cmd_3ed5384d"}
```

No `stdout` field (grep matched nothing, so it's omitted rather than set to
`""`). No indication the buffer holds real content. Reading it back:

```
wc -l @cmd_3ed5384d → 57 /tmp/.tmpAQizkA
```

The full 57-line file was captured and buffered, and the response gave zero
signal that there was anything there — or how much.

## Reproduction

```
git rev-parse HEAD   # 3988f0ff (experiments)
```

Any `run_command` call piping a bounded producer (`cat`, `ls`) into a filter
(`grep`, `head`, `tail`) where the filter's output is empty reproduces this —
the unfiltered ref still attaches with no size hint whenever the pre-filter
capture is non-empty.

## Environment

- codescout @ branch `experiments`, `src/tools/run_command/output.rs`

## Root cause

`handle_successful_output` (`src/tools/run_command/output.rs:101-317`) has two
independent code paths that never share information:

1. **Result shaping** (the `else` branch of the `needs_summary` check, around
   `:280-296` in the un-summarized short-output case): builds `result` from
   `raw_stdout`/`raw_stderr` — the **filtered** output. When `raw_stdout` is
   empty, `if !raw_stdout.is_empty() { r["stdout"] = ... }` never fires, so the
   key is *absent*, not `""`. There is no structural difference in the response
   between "we don't know what stdout was" and "stdout was empty."
2. **Unfiltered ref attachment** (`:302-308`, "Attach unfiltered_output ref if
   we captured via tee"): runs unconditionally after step 1, based solely on
   whether the tee capture was non-empty (`:128-135` skips only a **fully**
   empty capture). It has no visibility into what step 1 decided, and attaches
   nothing but the bare ref id — no line count, no byte count, no "N lines
   available."

The two steps compose into a response that tells the agent "something is behind
this ref" without saying anything about what, or how much.

`inferred from src/tools/run_command/output.rs:101-317 — read 2026-08-26, not
independently measured against a range of buffer sizes.`

## Evidence

### Live reproduction (`run_command` + `wc -l`, 2026-08-26)

See § Symptom. Confirms the buffer held the full unfiltered file (57 lines),
undetectable from the response alone.

### The design doc's own stated goal, not fully met

`docs/superpowers/plans/2026-03-04-unfiltered-output-capture.md:5`: *"silently
capture the unfiltered stream via `tee`, buffer it, and add
`unfiltered_output: "@cmd_xxxx"` to the response so the LLM can look wider
without re-running the expensive base command."* The ref achieves "don't
re-run the expensive command," but "look wider" requires the agent to already
suspect there's something to look at — nothing in the response suggests that
either way.

### `unfiltered_output` is the only exception to a documented universal invariant

`docs/trackers/reconnaissance-patterns.md`'s distilled Law B ("The instrument decides the
answer") states, as the project's own codescout-specific corollary: *"every buffered
response reports `buffered_bytes` and names an `@ref`; if the byte count exceeds what the
summary could account for, the summary is not the result. That number is the cheapest
completeness check available and it is already in the response."* This is stated as a
universal law other reconnaissance work relies on (R-50's whole technique is "check
`buffered_bytes` before trusting a summary").

A repo-wide search for the shape that attaches a secondary buffer reference outside the
standard `call_content` overflow-envelope path (`grep '= json!(ref_id)\|_output"\] = json'
across src/**/*.rs`) returns exactly **one** hit: `unfiltered_output` at
`src/tools/run_command/output.rs:306`. `buffered_bytes` itself is constructed in exactly
one place, `src/tools/core/types.rs:855` inside `Tool::call_content` — the shared path
every other tool's overflow envelope goes through. `unfiltered_output` bypasses that path
entirely; it is not one instance of a widespread pattern, it is the sole counterexample to
a law the project believes holds everywhere.

This narrows the fix: no broader sweep of `src/` turned up a second offender. What likely
reads as "a lot similar" (Marius, 2026-08-26) is frequency of *encounter*, not count of
distinct code sites — any bounded-producer-into-filter command whose filter narrows to
nothing hits this exact, singular gap, and does so often given how common that shape is
(`cat|grep`, `ls|grep`, `find|grep`, all IL3-compliant and all routing through this code).
## Hypotheses tried

None yet — this is a first-pass root cause from reading the function, not a
debugged regression. No hypothesis-testing was needed: the code path is
unconditional and directly explains the observed shape.

## Fix

Implemented options 1 + 2 from the original analysis, in
`handle_successful_output` (`src/tools/run_command/output.rs`):

1. **`stdout` is now explicit, not omitted, when an `unfiltered_output` ref is
   attached.** The "attach unfiltered_output ref" step now sets
   `result["stdout"] = ""` if the key is absent, right before attaching the
   ref — scoped to exactly the case that was ambiguous (a piped-filter command
   whose filtered result is empty). Plain commands with genuinely empty
   output and no unfiltered capture are unaffected.
2. **`unfiltered_output_lines` reports the FULL pre-truncation line count.**
   `unfiltered_ref`'s tuple grew a third field, computed via `count_lines`
   (already imported) on the raw tee capture *before* the inline-storage
   byte-budget truncation runs — so a truncated buffer still reports its true
   size, not just what fit inline.

**SHA:** `c172fe10` (`experiments`)
**patch-id:** `e192f49af22e1302b18473795e6d725917d989d2`
## Tests added

`tools::run_command::tests::unfiltered_output_carries_a_line_count_and_explicit_empty_stdout`
and `tools::run_command::tests::unfiltered_output_line_count_survives_inline_truncation`
(`src/tools/run_command/tests.rs`). Verified RED before GREEN: both failed with
`left: Null` (the fields genuinely absent) before the fix, and pass after.
The second test's fixture asserts `unfiltered_truncated` is actually present
before checking the line count, so it can't pass vacuously on a fixture too
small to exercise truncation at all.
## Workarounds

Read the ref anyway (one extra round-trip) when the filter's exit code doesn't
give you high confidence about emptiness, or when you need to distinguish "no
match" from "the buffer might have something a narrower filter missed."

## Resume

Pick a fix option in § Fix (1+2 recommended), locate the exact insertion point
(`raw_stdout.is_empty()` branch around `:280-296` for option 1; the
`unfiltered_ref` construction around `:128-160` for option 2, since `content`
and its line count are already in scope there before being discarded to just
`ref_id`), write the regression test, verify against a live `run_command` call
the way this bug's own reproduction did.

## References

- `src/tools/run_command/output.rs:101-317` — `handle_successful_output`
- `docs/superpowers/plans/2026-03-04-unfiltered-output-capture.md` — the
  original design; §5 states the "look wider" goal this bug shows is only
  half-delivered
- Observed during this session's `peer` opt-in work (commit `86cec0d0`, verifying
  `codescout`'s own `.codescout/project.toml` had no `[peer]` section) —
  reported directly by Marius as a recurring friction ("we have a lot similar
  like this"), not self-discovered
