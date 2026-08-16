---
id: '5f6cfe1acdfda38d'
kind: bug
status: open
title: 'BUG: grep''s file-diversity round-robin never runs, so overflow hints name walk-order files, not hot ones'
tags:
- grep
- progressive-disclosure
- file_group
closed: null
opened: 2026-08-16
owner: marius
related:
- docs/issues/2026-08-15-grep-showing-n-of-n-when-collection-hit-cap.md
severity: medium
---

# BUG: `grep`'s file-diversity round-robin never runs, so overflow hints name walk-order files, not hot ones

## Summary

`cap_grouped` exists to preserve file diversity when a result is trimmed — round-robin
across files in count-desc order so a capped result still spans many files. In `grep` it
is a **no-op**: the walk already stopped at `max`, so `cap_grouped` is always handed a
vector no larger than its budget and returns it unchanged. The capped result is therefore
the first `max` matches in filesystem walk order, and the overflow hint's
"Narrow with one of: …" list names whichever files the walker reached first rather than
the ones with the most matches — which is what the hint is for.

## Symptom (Effect)

Given 3 files with 3 matches each and `limit=4`, the hint offers two files, weighted 3/1:

```
Narrow with one of: path="/tmp/.tmpuJ44AQ/c.rs" (3 matches),
                    path="/tmp/.tmpuJ44AQ/b.rs" (1 matches)
```

`c.rs` is not hotter than `a.rs` or `b.rs` — every file has exactly 3 matches. It is
simply the file `WalkDir` reached first (note the order is not even alphabetical). A
diversity-preserving cap of the same corpus would have returned 2/1/1 across **three**
files.

## Reproduction

Observed 2026-08-16 as a side-observation of the BL-2 RED phase, in
`grep_capped_collection_never_renders_as_a_complete_result`
(`src/tools/grep.rs`). Any `grep` whose `limit` is below the true match count
reproduces it; the artificial corpus just makes the bias legible because every file has
an identical match count.

## Environment

Linux, `experiments`, at the BL-2 fix commit. Mechanism is platform-independent.

## Root cause

`max` is bound once (`src/tools/grep.rs:81`) and used as both the collection break
threshold and `cap_grouped`'s budget (`:339`):

```rust
if matches.len() >= max || emitted_bytes >= MAX_TOTAL_MATCH_BYTES {
    hit_cap = true;
    break 'outer;                    // <- collection stops at `max`
}
...
let max_matches = max;
let (visible, total, files) = cap_grouped(matches, max_matches);   // budget == max
```

`cap_grouped` (`src/tools/file_group.rs:50-109`) early-returns before the round-robin
whenever `budget >= total`:

```rust
if budget >= total {
    return (items, total, files);
}
```

Since collection guarantees `matches.len() <= max == budget`, that early return is the
**only** path grep ever takes. The buckets, the count-desc sort, and the round-robin loop
below it are unreachable from `grep`. `symbol/display.rs` and `symbol/references.rs` call
`cap_grouped` too and may not share the defect — they were not checked.

*Verified 2026-08-16 by reading the two call sites and confirming `max` has exactly four
occurrences in `grep.rs` (`:81`, `:339`, `:340`, `:712`) with no mutation, plus the
runtime output quoted under Symptom.*

## Evidence

### The capped result spans fewer files than the policy promises

From the `grep_capped_collection_never_renders_as_a_complete_result` failure output
(9 matches, 3 files, 3 each, `limit=4`):

```
{"shown":4,"total":4,"hint":"Showing 4 of 4 matches across 2 files. ..."}
```

`across 2 files` — one file contributed 3 of the 4, another 1, and the third contributed
nothing. `cap_grouped`'s documented policy ("Round-robin across files, prioritizing
hotter files") would have produced 2/1/1 across 3 files.

### The dead code has tests that pass

`cap_grouped_round_robin_first` and `cap_grouped_fills_hot_after_breadth`
(`src/tools/file_group.rs:255-298`) both exercise the round-robin directly with
`budget < total`, so the helper is correct and covered. Nothing tests that a *caller*
reaches it. This is why the defect is invisible to the suite: the unit is green and the
integration is never asserted.


### Live, on the real codebase: the hint offers files that cannot help

Observed 2026-08-16 against the running MCP server at `4b77dff5`:

```
grep(pattern="^use serde_json::json;", glob="src/**/*.rs", limit=5)

5 matches (capped) in 5 files
  … Collection stopped at limit=5 … narrow with one of:
     path=".../librarian/tools/worktree.rs" (1 matches),
     path=".../tools/edit_file/tests.rs"    (1 matches),
     path=".../tools/markdown/tests.rs"     (1 matches).
```

Every suggested file holds **one** match. Narrowing to any of them cannot reduce a
result that is already capped at 5 — the advice is not merely unranked, it is inert.
The list is the first three files the walker happened to reach, and because collection
stops at `max` in walk order it can only ever be that.

Two companion observations from the same session, both consequences of the same
collect-at-`max` shape:

- **A capped grep usually spans one file.** `grep(pattern="hit_cap", limit=3)` and
  `grep(pattern="total_is_lower_bound", limit=4)` both returned `across 1 files` — the
  first file walked held enough matches to exhaust the budget on its own. So the common
  capped result is not a broad sample at all, it is one file's worth.
- Because `render_grouped` suppresses its header when `files <= 1`, that also means the
  `(capped)` header marker added for BL-2 is invisible in exactly the single-file case.
  The overflow line still carries it, so nothing is unmarked — but the two defects
  interact, and fixing this one restores the header marker to the common path.
## Hypotheses tried

1. **Hypothesis:** collection-at-`max` is deliberate and diversity is meant to apply only
   to callers that over-collect.
   **Test:** read the comment at `src/tools/grep.rs:336-338` — it explains the rename of
   `budget` → `max_matches` after a `limit: 40` search emitted 4.4M tokens, i.e. it is
   about *size*, and says nothing about diversity.
   **Verdict:** deferred — the rename was a size fix and plausibly did not consider the
   diversity consequence, but that is inference. Ask before changing collection behaviour.

## Fix

Not implemented. Two candidate shapes, with different costs:

1. **Over-collect, then cap.** Walk to `max * K` (or to the byte budget alone) and let
   `cap_grouped` do its job with `budget = max`. Restores diversity and makes `total` a
   real count over a wider sample — but it is exactly the unbounded-collection behaviour
   the 4.4M-token incident was fixed by bounding, so `K` must be small and the byte clamp
   must stay authoritative.
2. **Rank the hint's file list separately.** Leave collection alone and accept that
   `visible` is walk-ordered, but stop advertising the top-3 as if ranked. Cheaper and
   strictly honest, but does not give the caller a more useful sample.

(1) is the better result and the riskier change; (2) is a message fix in the same spirit
as the BL-2 fix. Prefer deciding with a measurement of real `grep` overflow rates rather
than in the abstract.

## Tests added

None — not fixed. Whichever fix lands needs a test asserting a *caller*-level property:
that a capped grep over an N-file corpus spans more than one file's worth of the budget.
The existing `cap_grouped` unit tests cannot catch this, which is the whole point.

## Workarounds

Treat the overflow hint's file list as "files seen first", not "files with the most
matches". For a genuine ranking use `mode="files"`, which counts per file without the
collection cap.

## Resume

Decide between fix (1) and fix (2) above. Before deciding, check whether
`src/tools/symbol/display.rs:223` and `src/tools/symbol/references.rs:363` have the same
collect-at-budget-then-cap-at-budget shape — if they do, the fix belongs closer to
`cap_grouped` than to `grep`.

## References

- `docs/issues/2026-08-15-grep-showing-n-of-n-when-collection-hit-cap.md` — found while fixing it
- `docs/PROGRESSIVE_DISCOVERABILITY.md` — overflow-hint contract (Pattern 1: concrete + copy-paste-ready)
