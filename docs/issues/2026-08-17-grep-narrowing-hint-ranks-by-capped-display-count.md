---
id: a2f2dba5c76dad95
kind: bug
status: open
title: 'BUG: grep''s overflow narrowing hint ranks and reports the per-file CAPPED display count, so it recommends 3-match files and never names the 20-match one'
---

## Summary

When `grep(mode="content")` overflows, the hint offers `path=` values to narrow
with, annotated `(N matches)`. `N` is the **post-cap displayed** count, not the
file's real match count, and the candidates are ranked by that capped number. Since
the diversity cap flattens almost every file to the same displayed count, the
ranking is effectively arbitrary: the hint recommends files holding 3 of 115
matches while the file holding 20 is never mentioned.

## Symptom (Effect)

One call, `2026-08-17`, `grep(pattern="render_template", glob="src/**/*.rs", mode="content")`:

```
115 matches in 26 files
  … showing 50 of 115 — Showing 50 of 115 matches across 26 files. To trim,
  narrow with one of:
    path=".../src/librarian/catalog/augmentation.rs" (3 matches),
    path=".../src/librarian/catalog/mod.rs" (3 matches),
    path=".../src/librarian/tools/augment.rs" (3 matches).
  Or mode="files" for a per-file count summary.
```

The same pattern via `mode="files"`, run in the same minute:

```
115 matches in 26 files
    20  src/librarian/tools/tracker_design.rs
    18  src/librarian/tools/augment.rs
    11  src/librarian/catalog/augmentation.rs
     9  src/librarian/tools/context.rs
     5  src/librarian/catalog/mod.rs
     …
```

Every annotation in the hint is wrong, and so is the selection:

| File | Hint says | Actually has |
|---|---|---|
| `catalog/augmentation.rs` | 3 | **11** |
| `catalog/mod.rs` | 3 | **5** |
| `tools/augment.rs` | 3 | **18** |
| `tools/tracker_design.rs` | *not offered* | **20** |

An agent that follows the hint to "trim" lands on a file with 3 of 115 matches,
having been told that is one of the three biggest. To find where the matches
actually concentrate you must ignore the hint and re-run with `mode="files"` —
which the hint mentions last, as an aside.

## Reproduction

`git rev-parse HEAD` → `66487591`, branch `experiments`.

1. `grep(pattern="render_template", glob="src/**/*.rs", mode="content")` — read
   the `(N matches)` annotations in the narrowing hint.
2. `grep(pattern="render_template", glob="src/**/*.rs", mode="files")` — read the
   true per-file counts.
3. Compare. Any pattern whose per-file distribution is skewed and whose total
   exceeds the display budget reproduces it; the flatter the cap makes the
   display, the more arbitrary the ranking.

Note the totals agree (115/26 in both modes) — the bug is confined to the
per-file annotation and the candidate selection, which is what makes it easy to
trust.

## Environment

Linux, codescout `experiments` @ `66487591`, stdio MCP, release binary built
2026-08-17. Requires a `mode="content"` grep that overflows the display budget.

## Root cause

Unknown — under investigation. Not yet traced to a line.

The observable behaviour is consistent with the narrowing candidates being
computed from the **already-capped result rows** rather than from the raw
per-file tally: every displayed file is truncated to at most 3 rows by the
diversity cap (26 files × ≤3 = the 50 shown), so a ranking over displayed rows
ties nearly everything at 3 and then falls back to display order. That is a
hypothesis from the numbers, not a read of the code.

Likely interacts with `d8c2b23d` *(fix(grep): give cap_grouped something to
choose from, so diversity actually runs)* — that commit made the per-file cap
actually bind, which is exactly the condition that flattens the counts the hint
ranks on. Before it, the ranking may well have been correct because the cap was
not reducing anything. Worth checking whether the hint was built when
`cap_grouped` was a no-op.

Measured 2026-08-17: the two calls quoted under **Symptom**, run back to back
against the rebuilt binary.

## Evidence

### The totals are right, which is what makes the annotations credible

Both modes independently report `115 matches in 26 files`. So the overflow
signal itself is sound — this is not the silent-cap class of bug. The defect is
narrower and more insidious: an accurate total beside per-file figures that are
off by up to 6×, in a hint whose entire purpose is choosing where to look next.

### An earlier pair of calls disagreed, and did NOT indicate a bug

The same two modes run ~40 minutes apart returned `113/25` and `115/26`. That
gap is a concurrent session committing to `src/` between the calls, not a mode
discrepancy — re-running both back to back agreed exactly. Recorded here so a
later reader does not chase it: **compare the modes at the same instant**, since
this repo routinely has two sessions writing.

## Hypotheses tried

1. **Hypothesis:** the two modes disagree on totals as well, making the whole
   envelope unreliable.
   **Test:** ran both back to back rather than 40 minutes apart.
   **Verdict:** rejected — both report 115/26. The earlier 113/25-vs-115/26 gap
   was a peer session's commits landing between the calls.
   **Evidence:** § Evidence, second subsection.

## Fix

Not implemented. The hint should rank candidates by the **true** per-file tally
and report that number — the same tally `mode="files"` already computes and
prints, so the data exists on the same code path and no extra scan is needed.

With true counts the example hint becomes actionable: *narrow with
`tracker_design.rs` (20), `augment.rs` (18), `augmentation.rs` (11)* — three
files holding 49 of 115 matches, versus the current three holding 19.

Two smaller points worth folding in:

- Where a suggested path's true count still exceeds the display budget, say so,
  or the agent narrows once and overflows again.
- Consider promoting `mode="files"` ahead of the path list when the distribution
  is flat, since a per-file summary is strictly more useful than an arbitrary
  pick from a tie.

## Tests added

None yet. The regression test wants a fixture whose per-file distribution is
skewed *past* the cap — e.g. one file with 10 matches and five with 1 — then
asserts the hint's first candidate is the 10-match file and that its annotation
reads 10, not the capped 3. A flat fixture cannot fail, which is presumably how
this shipped: any fixture small enough not to trigger the cap makes the capped
count equal the true count, and the bug is invisible.

## Workarounds

Treat the `(N matches)` annotations and the path selection as unreliable
whenever a `mode="content"` grep overflows. Run `grep(..., mode="files")` to see
the real distribution, then narrow with `path=` to the file you actually want.
The totals in the envelope are trustworthy; only the per-file breakdown is not.

## Resume

Locate where the narrowing candidates are assembled in `src/tools/grep.rs` and
determine whether they are derived from the capped rows or from a raw tally
(`cap_grouped` is the function `d8c2b23d` touched — start there). Confirm the
hypothesis in § Root cause before changing anything; if the candidates already
use a raw tally, the defect is in the annotation only and the fix is smaller.
Then add the skewed-distribution test under **Tests added**.

## References

- `src/tools/grep.rs` — grep implementation, incl. `cap_grouped`
- `d8c2b23d` — *fix(grep): give cap_grouped something to choose from, so diversity actually runs*
- `docs/PROGRESSIVE_DISCLOSURE.md` — output sizing and overflow-hint conventions

