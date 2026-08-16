---
status: open
opened: 2026-08-15
closed:
severity: medium
owner: marius
related: []
tags: [grep, progressive-disclosure, external-report]
kind: bug
---

# BUG: `grep` prints a self-refuting "Showing N of N matches" when collection hit the cap

## Summary

`grep`'s overflow hint reports `Showing {visible} of {total} matches`, where `total` is
the count of matches actually collected — not the count that exist. When collection
stops at the internal cap, `total` equals `visible`, so the hint reads
"Showing 400 of 400 matches" while simultaneously asserting the result was truncated.
The line is internally contradictory and hides that the true total is unknown.

## Symptom (Effect)

Reported by an external user as `Showing 400 of 400`. The message only ever appears
when `truncated` is true, so a reader is told both "everything is shown" and "something
was cut."

## Reproduction

Not yet reproduced end-to-end on this host. To reproduce: run `grep` with a pattern
broad enough to hit the collection cap (`hit_cap`) while the display budget is large
enough to show every collected match, i.e. `budget >= total`.

## Environment

Reported on macOS against `experiments @ d7988aca`. Mechanism verified on Linux at
`821f9d0d` from source.

## Root cause

Two independent notions of "truncated" are conflated.

`src/tools/grep.rs:324`:

```rust
let (visible, total, files) = cap_grouped(matches, budget);
let truncated = hit_cap || total > visible.len();
```

`hit_cap` (set at `src/tools/grep.rs:208`, `:254`, `:273`) means **collection** stopped
early — `matches` is already a truncated view of reality. `total > visible.len()` means
**display** was capped.

`cap_grouped` (`src/tools/file_group.rs:50-109`) computes `let total = items.len()` and
returns `(items, total, files)` unchanged when `budget >= total`. So in the
collection-capped case `total` *is* the cap, `visible.len() == total`, and the hint at
`src/tools/grep.rs:342`/`:351` renders `Showing {total} of {total}`.

The real total is not merely unreported — after `hit_cap` it is **unknown**, because
the walk stopped.

*Verified 2026-08-15 by reading `src/tools/grep.rs:300-362` (via `read_file` with
`force=true`) and `symbols(name="cap_grouped", include_body=true)`. Inferred from
source — not measured at runtime on this host.*

## Evidence

### `cap_grouped` returns the input length as `total`

`src/tools/file_group.rs:50-109`. Its own test `cap_grouped_budget_exceeds_total`
asserts `visible.len() == 2` and `total == 2` for `budget=100` — the exact shape that
produces "N of N" when paired with `hit_cap`.

### The same defect exists in the buffer path

`src/tools/grep.rs:775` repeats the identical expression inside `grep_in_buffer`, with
its own hint at `:783`. Any fix must cover both.

## Hypotheses tried

1. **Hypothesis:** `total` is the true match count and `hit_cap` only affects display.
   **Test:** read `cap_grouped` and the `hit_cap` assignment sites.
   **Verdict:** rejected — `hit_cap` is set during the walk (`:208`, `:254`, `:273`),
   before `cap_grouped` ever sees the vector, so `total` is post-cap by construction.

## Fix

Not yet implemented. Distinguish the two cases in the hint:

- **Display-capped** (`total > visible.len()`): "Showing N of M matches" is correct.
- **Collection-capped** (`hit_cap`): say so — e.g. "Showing N matches; the search
  stopped at the collection cap, so the true total is unknown. Narrow the pattern."

Apply to both `src/tools/grep.rs:342`/`:351` and the `grep_in_buffer` twin at `:783`.

## Tests added

None yet. Needs a test constructing `hit_cap == true` with `budget >= total` and
asserting the hint does not claim "N of N".

## Workarounds

When a `grep` result says "Showing N of N", treat N as a floor, not a total. Re-run with
a narrower pattern or `mode="files"` for a per-file count summary.

## Resume

Edit the hint construction at `src/tools/grep.rs:332-360` to branch on `hit_cap` versus
`total > visible.len()` rather than collapsing both into one string; mirror into
`grep_in_buffer` at `src/tools/grep.rs:775-800`. Add a regression test.

## References

- `docs/trackers/bistriceanu/index.md` § B-5
- `docs/PROGRESSIVE_DISCOVERABILITY.md` — overflow-hint contract (Pattern 1: concrete + copy-paste-ready)
