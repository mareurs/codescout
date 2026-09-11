---
id: 860023f8bf00006c
kind: bug
status: fixed
title: 'BUG: grep''s context-mode overflow publishes the shown count as `total` and omits `total_is_lower_bound`, so two calls differing only in `context_lines` answer 39 and 58'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
- progressive-disclosure
- measurement
- grep
topic: a cap-marking fix applied at two of three sibling construction sites
claimed_at: '2026-09-11T13:30:00Z'
claimed_by: f3c594ce-c424-40d3-a603-9693cfef3f63
---

## Summary

`grep`'s **context mode** (`context_lines > 0`) builds its own `overflow` object at
`src/tools/grep.rs:529` and sets `"total": shown_count` at `:1081`. That object carries `shown`
and `hint` and **not** `total_is_lower_bound`. The sibling non-context branch sets that flag at
`:486`, and `format_grep` at `:600-605` reads *that very flag* to decide whether the header prints
`matches (capped)` — so at this site the qualifier is not merely missing, it is **switched off by
the absent flag**.

The consequence is measurable rather than stylistic: **two calls differing only in `context_lines`
answer the same question 39 and 58**, and only one of them says so.

## Symptom (Effect)

Same pattern, same scope, same `limit=40`, at `b678e0f3`:

```
grep(pattern="\.take\([0-9]+\)", path="src", glob="*.rs", limit=40, context_lines=3)
  -> 39 matches
     … showing first 39 — Showing first 39 matches (cap hit). Narrow with one of: …

grep(pattern="\.take\([0-9]+\)", path="src", glob="*.rs", limit=40)
  -> 58 matches in 33 files
     … showing 40 of 58 — Showing 40 of 58 matches across 33 files. …
```

The `overflow` object from the first call, read in full:

```json
{ "shown": 39, "hint": "Showing first 39 matches (cap hit). Narrow with one of: …" }
```

Two keys. No `total_is_lower_bound`. The second call reports an exact denominator because
collection oversamples to `limit * 4` and therefore counts every match; the first stops at the
display limit and never learns 58.

**39 is not a rounding of 58.** A reader who takes the headline is off by 19 in a direction the
output does not name, and `39` is what a session published into a commit message.

## Reproduction

Deterministic at `b678e0f3`. Any `grep` with `context_lines > 0` whose match count exceeds
`limit`. The `\.take\([0-9]+\)` probe above is convenient because the control run recovers the
true total through the same tool.

The **control is the point** — run both. A single capped call cannot distinguish "the total is
unknowable" from "this branch declined to count it", and those have different remedies.

## Environment

`experiments` at `b678e0f3`. Observed through the live MCP server, not a unit test, which is why
the header rendering is part of the evidence rather than the JSON alone.

## Root cause

`src/tools/grep.rs` builds `overflow` at **three** sites. Two set the floor flag; one does not.

| site | path | sets `total_is_lower_bound` |
|---|---|---|
| `:486` | non-context, grouped | **yes** |
| `:1067` | buffer (`grep_in_buffer`) | **yes** |
| `:529` | **context mode** (filesystem walk) | **no** |
| `grep_in_buffer`, context branch | buffer path's own context-mode construction | **no** |

**Correction, 2026-09-11:** this table under-counted by one when filed. `grep_in_buffer` carries its OWN context-mode overflow-construction site, separate from the grouped/non-context buffer branch the row above already covers — same gap, same shape, not enumerated here because the review that produced this table checked the buffer path's non-context branch and stopped. Found by re-reading the code before implementing rather than trusting this table's count (`bug-fix-session-log:F-134` records the parallel case: this bug's own "prefer oversample" recommendation also didn't survive contact with the code). Four sites, not three; both `no` rows are fixed below.

The `else` arm at `:497` is commented *"Context mode: keep flat matches[], preserve legacy shape
for format_grep"* — the legacy shape is the defect, and the comment is the record of a deliberate
carve-out that later grew a fix it never received.

Two things follow from the one omission, which is why it reads as two bugs:

1. `"total": shown_count` (`:1081`) publishes the shown count under the name `total`.
2. `format_grep` (`:600-605`) computes `total_is_floor` by reading
   `overflow.total_is_lower_bound`. Absent ⇒ `false` ⇒ `format_search_simple_mode`'s
   `(_, true) => "matches (capped)"` arm at `:655` never fires. **The header qualifier is
   disabled by the missing flag, not independently forgotten.**

## Evidence

### The guard is implemented at one site and tested at exactly that site

`src/tools/grep.rs:~1800` — the regression test for the floor flag calls

```rust
json!({ "pattern": "target", "path": path, "limit": 4 })
```

with **no `context_lines`**, so it exercises only the `:486` branch. It asserts
`total_is_lower_bound == Some(true)` and `hint.contains("true total is unknown")`.

Adding `"context_lines": 1` to that same call would red it on both assertions. That is the
mutation to run, and it is on the production path rather than on a copy.

This matters because the coverage is otherwise genuinely good — `:1935` asserts the flag is
**absent** when nothing was cut (*"nothing was cut off, so the total is exact and must not be
flagged a floor"*), which is the correct second direction and rules out the stamp-everything
implementation. Both directions, one site.

### The class ledger already advertises this as gated

`docs/trackers/issue-clusters/IC-20-floor-published-under-the-name-of-a-total.md` § *Mechanism
status* reads `partial`, and says the rename is *"live at `src/tools/grep.rs:486` and `:1067`"*
and *"regression-tested in both directions"*. Every word of that is accurate — **it names the two
sites it covers** — and a reader takes it as the class being handled. The third construction site
is not contradicted anywhere; it is simply not enumerated.

## Hypotheses tried

1. **`IC-20` — a floor published under the name of a total. REJECTED by that class's own
   falsification test.** `IC-20` states: *"Falsified by a member whose true total was recoverable
   — that is an ordinary reporting bug, fixed by reporting it, and this class claims the value is
   gone."* The control run recovers **58** through the same tool with one parameter changed, so
   the value is not gone. This was the class proposed when the defect was handed over, and it
   would have been `IC-20`'s second member, taking it from `n=1` to the count bar — promoting a
   class on a member its own text excludes.
2. **`IC-13` — capped result presented as complete. REJECTED.** The widened clause deliberately
   excludes *a marker the caller can see*, and `(cap hit)` is in the hint the caller reads.
3. **`IC-19` — truncated window ordered by the wrong key. REJECTED.** Nothing here turns on
   ordering; the count is wrong, not the selection within it.
4. **A misreading of an adequate output. REJECTED as the primary cause, though it was offered
   first by the session that hit it.** `(cap hit)` is present and was overlooked — but the header
   the codebase's own 2026-08-16 fix installed *because "the header is what a reader anchors on"*
   was absent at this site. Attributing it to the reader stops the inquiry one step before the
   carve-out.

## Fix

**Applied, 2026-09-11, commit `89518fdb2748f94d9fa0de10d18aa253419123b9` (patch-id `a4f8972e1512a1bfb518596bb537556f29edc3fa`).**

**Not the recommended shape.** This file's own text preferred oversampling context mode's collection like simple mode does, "on the ground that ... it is recoverable by a sibling code path in the same file." Reading `src/tools/grep.rs:121` before implementing surfaced why that ground doesn't hold: simple mode's oversample exists **only** to feed `cap_grouped`'s display-capped file-diversity round-robin (BL-31) — the extra collected matches are thrown away before rendering, never returned. Context mode has no equivalent display-cap step; it returns `matches[]` flat, exactly as collected. Oversampling context mode's WALK the same way would therefore oversample its OUTPUT by the same factor (`COLLECTION_OVERSAMPLE`, 4x by default) — a real regression shipped alongside the fix, not a detail. Recorded as `bug-fix-session-log:F-134` before implementing.

**Took the second, "cheap and honest" option instead**, at all four construction sites (see the corrected Root-cause table above — two were already fixed, two were not):

- Set `overflow["total_is_lower_bound"] = true` at both previously-unflagged sites (filesystem-walk context mode, `grep_in_buffer`'s context mode) whenever `hit_cap` is true. Collection behavior is completely unchanged — `shown_count` (block count) is still exactly what it was.
- `format_grep`'s `matches[]` header branch previously never read `total_is_lower_bound` at all (a gap this bug's own root-cause section didn't separately name — it only diagnosed the flag's absence, not that the header path never consulted it): it composed `"{total} {match_word}\n"` directly rather than going through `format_search_simple_mode`'s floor-aware noun logic. Threaded the same `(total, total_is_floor) => noun` match arms in, so a capped context-mode result's header now reads `"N matches (capped)"`, matching simple mode's established pattern and the project's own stated principle that "the header is what a reader anchors on."

**What this does NOT do:** it does not recover the true total (58 in this bug's own reproduction) for context mode — `overflow` still carries no numeric `total` field, so `format_overflow`'s "showing X of Y" sentence still falls back to "showing first N" for context-mode results. A caller now correctly learns the number is a floor; it does not learn what the true count is. This is exactly the tradeoff the bug's own § Fix named as the cheaper alternative, taken deliberately rather than by default.
## Tests added

Two, both in `src/tools/grep.rs`'s test module, mutation-verified red-without-the-fix (confirmed by temporarily reverting the three production hunks and re-running each test individually — both failed with the expected `left: None, right: Some(true)` message — then restoring):

- `grep_context_mode_capped_collection_marks_the_total_as_a_floor` — filesystem-walk context mode. Asserts a capped call's `overflow.total_is_lower_bound == Some(true)`, a complete call carries no `overflow` at all, and — the cross-row assertion, on the header line a reader anchors on — the capped result's `format_compact()` first line contains "capped" while the complete one does not.
- `grep_buffer_context_mode_capped_collection_marks_the_total_as_a_floor` — the `grep_in_buffer` twin, same assertions, over a `@tool_*`-style buffer input.

Deliberately does not add the test this bug originally specified ("the `total` reported with `context_lines=1` must equal the `total` reported without it") — that assertion is only meaningful under the oversample fix, which was not the shape shipped. See § Fix for why.
## Workarounds

For any count, drop `context_lines` — or use `mode="files"`, which walks to completion and reports
`total` exactly. Treat a context-mode headline as a floor whether or not it says so.

## References

- `src/tools/grep.rs:497-533` — the context-mode `else` arm and its `overflow` construction.
- `src/tools/grep.rs:486`, `:1067` — the two sites that do set the flag.
- `src/tools/grep.rs:600-605`, `:655-658` — `format_grep` reading the flag, and the
  `matches (capped)` arm it gates.
- `docs/issues/archive/2026-08-15-grep-showing-n-of-n-when-collection-hit-cap.md` — the
  2026-08-16 fix (`4b77dff5`) whose three surfaces this site received none of. Its § *Fix* names
  the header surface explicitly and gives the reason: *"the qualifier travels with the number
  rather than sitting two lines below it, because the header is what a reader anchors on."*
- `docs/trackers/issue-clusters/IC-20-floor-published-under-the-name-of-a-total.md` — the class
  considered and rejected, and the row whose *Mechanism status* this bug corrects.
- `docs/issues/2026-09-02-overflow-summary-promotes-the-count-and-elides-its-caveat.md` — open,
  and **distinct**: that one is `describe_payload_shape` reducing object-valued keys to bare
  names. Here the hint text arrived in full; what is missing is a field that was never computed.
- Found by sessionId `ba061586-6581-4656-b0c5-acad83474de5`, who published the `39` into a commit
  message and then supplied the reproduction. Root-caused and filed by sessionId
  `cda3afe5-17b8-4863-9f4c-9fe4eadbc17b`.
