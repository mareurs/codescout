---
id: 2a9fd7654cf82013
kind: bug
status: fixed
title: 'BUG: grep''s file-diversity round-robin never runs, so overflow hints name walk-order files, not hot ones'
tags:
- grep
- progressive-disclosure
- file_group
closed: 2026-08-16
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

Fixed on `experiments`. **Fix (1) — over-collect, then cap** — chosen after answering the
question § Resume asked, which changed where the fix belongs.

### The sibling check came first, and moved the boundary

§ Resume said: *check whether `symbol/display.rs` and `symbol/references.rs` share the
collect-at-budget-then-cap-at-budget shape — if they do, the fix belongs closer to
`cap_grouped` than to `grep`.*

- **`references.rs:264` is clean.** Its `locations` come from the LSP's full reference list
  and are never bounded before the call, so `budget < total` is the normal case and the
  round-robin already runs there.
- **`display.rs` no longer calls `cap_grouped` at all** — `references(cap_grouped)` returns
  three files, and that is not one of them. The line cited in § Resume is stale.
- **`grep.rs` has TWO call sites, not one.** `Grep::call` (`:340`) *and* `grep_in_buffer`
  (`:902`), each with its own collect-at-`max` loop. The filing named only the first.

So the defect is grep's, not `cap_grouped`'s — and it is in both of grep's paths. Fixing only
the reported one would have left the buffer path exactly as broken, with nothing to prompt
anyone to look again.

### What changed

`COLLECTION_OVERSAMPLE = 4`. Collection walks to `limit * 4` candidates and `cap_grouped`
trims back to `limit`, so `budget < total` becomes the normal case and the round-robin
finally runs.

Three properties keep this safe:

- **`MAX_TOTAL_MATCH_BYTES` is untouched and remains the authoritative payload bound.** The
  4.4M-token incident § Hypotheses cites was fixed by bounding *bytes*, not by bounding the
  count — so oversampling the count does not re-open it. On a heavy corpus the byte budget
  still stops the walk first.
- **Oversampling changes what the trimmer chooses *from*, not how many it returns.** Output
  is still capped at `limit`.
- **Simple mode only.** Context mode returns merged blocks flat, without `cap_grouped`, so
  oversampling there would return more blocks than were asked for. `collect_limit` falls
  back to `max` when `context_lines > 0`.

The overflow hint's `stopped_at` had to change with it: collection now stops at the candidate
cap, so `"Collection stopped at limit=4"` would be false. It reads
`"the candidate cap (16) for limit=4"`. *"Raise limit"* stays the right advice, since
`collect_limit` scales with it.

### Interaction with BL-2 — the dead branch came back to life

BL-2 left the `"Showing N of M"` branch in place with a comment: *"Currently unreachable …
kept correct rather than deleted, so decoupling the display budget from the collection cap
stays a one-line change."* That decoupling is this fix, and the branch is now reachable and
correct: when collection completes inside the candidate cap, every match **was** counted, so
printing the denominator is honest. BL-2 forbade printing a denominator that was never
counted — not printing one at all.
## Tests added

**`grep_capped_result_spans_files_by_diversity_not_walk_order`** — the caller-level test §
Tests added asked for. 3 files x 3 matches, `limit=4`. Asserts the visible set spans **all
three** files at exactly `2/1/1` (round-robin gives every file one before any file gets a
second), that the budget is still 4, that `total` is the true 9, that the hint prints
`"Showing 4 of 9"`, and that `total_is_lower_bound` is **absent** because nothing was cut off.

Before the fix that same corpus returned 3 matches from one file and 1 from another — two
files, not three. `cap_grouped`'s own unit tests were green throughout, which is exactly why
this had to be asserted at the caller.

**`grep_capped_collection_never_renders_as_a_complete_result` (BL-2) — fixture widened, not
relaxed.** Its 3x3 corpus with `limit=4` no longer caps, because 9 candidates fit inside the
cap of 16 — and "Showing 4 of 9" there is *honest*, so the old assertions would have been
pinning the wrong thing. The corpus is now 8 files x 3 = 24, which genuinely exhausts the
candidate cap, and every original assertion survives unchanged: the floor flag, the
"true total is unknown" wording, and the header `capped` marker.

That distinction is the whole reason to widen rather than weaken: BL-2's invariant is
*never print a denominator you did not count*, and it still holds exactly.

Gate: **3980 tests**, `cargo clippy --all-targets -- -D warnings`, `cargo fmt`.
## Workarounds

Obsolete. The hint's file list is now ranked over a 4x-wider sample, so it names files with
more matches rather than files the walker reached first.

`mode="files"` is still the right tool for a genuine per-file ranking — it counts without the
collection cap at all — and the hint still points at it.
## Resume

None. Both grep call sites fixed, `references.rs` checked and clean, `display.rs`'s cited
call site confirmed stale.

One number is a judgement rather than a measurement: `COLLECTION_OVERSAMPLE = 4`. It is large
enough to give the round-robin real choice and small enough that the byte clamp still
dominates on heavy corpora. If a future measurement of real overflow rates suggests a
different value, it is a one-line change with a doc comment explaining what it trades.
## References

- `docs/issues/2026-08-15-grep-showing-n-of-n-when-collection-hit-cap.md` — found while fixing it
- `docs/PROGRESSIVE_DISCOVERABILITY.md` — overflow-hint contract (Pattern 1: concrete + copy-paste-ready)
