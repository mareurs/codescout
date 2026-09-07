---
id: b75d2660ef37198c
kind: bug
status: open
title: 'BUG: grep''s context-mode overflow publishes the shown count as `total` and omits `total_is_lower_bound`, so two calls differing only in `context_lines` answer 39 and 58'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
- progressive-disclosure
- measurement
- grep
topic: a cap-marking fix applied at two of three sibling construction sites
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
| `:529` | **context mode** | **no** |

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

Not applied. Two candidate shapes, and the choice is a decision rather than a detail:

- **Oversample in context mode too**, matching the non-context branch, so `total` is exact and the
  flag is unnecessary. Costs the extra walk that `COLLECTION_OVERSAMPLE` already buys elsewhere;
  gives the caller the number they wanted.
- **Set `total_is_lower_bound` at `:529`** and let the existing header path render
  `39 matches (capped)`. Cheap and honest, and still refuses the caller a total this tool can
  compute.

Prefer the first, on the ground that `IC-20`'s remedy is a rename only when the value is
unrecoverable, and here it is recoverable by a sibling code path in the same file.

## Tests added

None yet. The specification is the part worth getting right, and the direction to guard is **not**
"a marker appears":

- Parameterise the existing floor-flag test over `context_lines` in `{0, 1}` — one site's pass
  must not stand in for the other's.
- Assert the two calls agree: for a corpus of known size N with `limit < N`, the `total` reported
  with `context_lines=1` must equal the `total` reported without it. **This is the assertion that
  reds on the defect as observed**, and a marker-only assertion does not: a `(capped)` header on
  `39` satisfies "it marked itself" while still answering 39 to a question whose answer is 58.

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

