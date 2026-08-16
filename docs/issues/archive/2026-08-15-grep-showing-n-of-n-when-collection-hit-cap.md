---
kind: bug
status: fixed
tags:
- grep
- progressive-disclosure
- external-report
closed: 2026-08-16
opened: 2026-08-15
owner: marius
related: []
severity: medium
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

Reproduced on this host 2026-08-16, in the RED phase of the fix. Three files with
three matches each (9 total), `limit=4`:

```
{"shown":4,"total":4,
 "hint":"Showing 4 of 4 matches across 2 files. Narrow with one of: ..."}
```

No special setup was needed — see *Root cause*, the shape is not conditional.
## Environment

Reported on macOS against `experiments @ d7988aca`. Mechanism verified on Linux at
`821f9d0d` from source.

## Root cause

Two independent notions of "truncated" are conflated — and one of them is unreachable.

`max` is bound **once** (`src/tools/grep.rs:81`, `let max = optional_u64_param(&input,
"limit").unwrap_or(50)`) and then serves as *both*:

- the collection break threshold — `if matches.len() >= max || emitted_bytes >=
  MAX_TOTAL_MATCH_BYTES { hit_cap = true; break 'outer; }`
- `cap_grouped`'s display budget — `let max_matches = max; cap_grouped(matches,
  max_matches)` (`:339`)

So `matches.len() <= max` on exit, `cap_grouped` is always called with `budget >= total`,
and its early return hands back the input unchanged. **`visible.len() == total`
unconditionally**, which makes the second disjunct of

```rust
let truncated = hit_cap || total > visible.len();
```

**dead**. `truncated` *is* `hit_cap`, and the hint therefore renders `Showing {N} of {N}`
**every time it fires** — this is not an edge case reachable when `budget >= total`, it is
the only thing the simple-mode hint has ever printed.

The real total is not merely unreported — after `hit_cap` it is **unknown**, because the
walk stopped.

*Verified 2026-08-16 by reading `:81`, `:200-300`, `:330-410` (`read_file force=true`),
`symbols(name="cap_grouped", include_body=true)`, and confirming `max` has no other
binding or mutation (`grep` over `grep.rs` returns exactly `:81`, `:339`, `:340`,
`:712`). Confirmed at runtime by the RED test output above.*
## Evidence

### `cap_grouped` returns the input length as `total`

`src/tools/file_group.rs:50-109`. Its own test `cap_grouped_budget_exceeds_total`
asserts `visible.len() == 2` and `total == 2` for `budget=100` — the exact shape that
produces "N of N" when paired with `hit_cap`.

### The same defect exists in the buffer path

`src/tools/grep.rs:775` repeats the identical expression inside `grep_in_buffer`, with
its own hint at `:783`. Any fix must cover both.


### It produced a false claim in a committed artifact (2026-08-16)

The bug was filed from an external report and reasoned about at the source. This is
the first record of it **causing a wrong conclusion in shipped prose**, which
changes its severity argument: the cost is not a confusing response, it is an
unfalsifiable-looking one.

While establishing which files codescout's librarian guard actually covers:

```
grep(pattern="^id: ", glob="docs/**/*.md", limit=12)
-> 12 matches in 11 files … Showing 12 of 12 matches across 11 files.
```

Every one of those 12 held a **quoted** or `null` id. Read as complete — "12 of
12" — it supports the conclusion that was then written into an archived bug file:

> every `id:` in `docs/issues/` and `docs/adrs/` is quoted or `null` … so the guard
> effectively covers `docs/trackers/` and little else.

Collection had stopped at the `limit`. Counted without one:

```
unquoted 16-hex `id:` — what the guard matches — by directory:
    27  docs/issues/archive      <- the largest group
    12  docs/trackers
     3  docs/issues
     2  docs/trackers/archive
     2  docs/plans
     4  one each elsewhere

191 files carry any `^id:`; 141 are quoted or null.
```

**50** guarded files, and the biggest group is not the one the claim named. The
same glob with a narrower pattern found `docs/trackers/tool-usage-patterns.md`
immediately — the glob was never the problem, the denominator was.

**Why this shape is worse than a plain truncation.** A hint saying "showing 12 of
847" invites a follow-up. "Showing 12 of 12" **closes the question**: it is the
exact string a complete result prints, so there is nothing to notice and no reason
to re-run. The reader cannot distinguish a capped sample from an exhaustive one,
and a capped sample that happens to be homogeneous — as this one was — reads as a
clean finding.

Correction recorded in
`docs/issues/archive/2026-08-16-edit-file-replace-all-bypasses-the-librarian-guard.md`
§ *And a second one, which was wrong*.

### The renderer already knew better, and the hint overrode it

`format_overflow` (`src/tools/format.rs:32-41`) branches on `total > shown` and, when
they are equal, prints the honest form:

```rust
if total > shown { format!("  … showing {shown} of {total} — {hint}") }
else             { format!("  … showing first {shown} — {hint}") }
```

So the rendered text was a **three-way contradiction** in five lines:

```
12 matches in 11 files                       <- header, states 12 as fact
...
  … showing first 12 — Showing 12 of 12 matches across 11 files. Narrow with ...
     ^ honest             ^ re-asserts completeness
```

The one honest clause sits between two that contradict it, and is the shortest of the
three. The header is read first and anchors; the hint is read last and confirms. Fixing
only the hint would have left the header still stating a floor as a count, which is why
the fix touches both.
## Hypotheses tried

1. **Hypothesis:** `total` is the true match count and `hit_cap` only affects display.
   **Test:** read `cap_grouped` and the `hit_cap` assignment sites.
   **Verdict:** rejected — `hit_cap` is set during the walk, before `cap_grouped` ever
   sees the vector, so `total` is post-cap by construction.

2. **Hypothesis:** "N of N" is the `budget >= total` special case, so the display-capped
   branch handles the normal case correctly.
   **Test:** trace `max` from its single binding (`:81`) through both uses.
   **Verdict:** rejected — the display-capped branch is unreachable in this call path.
   The bug is unconditional, not a corner. This is what the original filing got wrong.

3. **Hypothesis:** report a truthful denominator instead.
   **Verdict:** rejected — not available. `cap_grouped` never counts past the cap, and the
   walk has already stopped. Any number after "of" invites the reader to treat it as the
   total. The honest rendering is an explicit incompleteness marker.
## Fix

Implemented 2026-08-16 on `experiments` in `4b77dff5` (fast-forward promotion:
`git rev-list --left-right --count master...experiments` = `0 789`, so this SHA is the
master SHA — no second one to record). Three surfaces, because the misleading claim
appears three times in one rendering:

1. **`Grep::call` simple-mode hint** (`src/tools/grep.rs:~350`) — branches on `hit_cap`.
   The collection-capped arm prints no denominator at all:

   > `Collection stopped at limit=4, so the true total is unknown — 4 matches across 2
   > files is a floor, not a count. To see more, raise limit, or narrow with one of: … `

   The byte-capped variant says `raising limit will not help` instead, since it won't.
   The display-capped arm is kept correct but marked unreachable in a comment.

2. **`overflow.total_is_lower_bound: true`** — the machine-readable twin. `shown ==
   total` cannot carry the distinction on its own, and a consumer reading JSON never sees
   the prose hint.

3. **The header line** (`format_search_simple_mode`) — takes a `total_is_floor` flag and
   renders `4 matches (capped) in 2 files`. The qualifier travels with the number rather
   than sitting two lines below it, because the header is what a reader anchors on.

`grep_in_buffer` gets the same treatment plus the `byte_capped` flag it was missing, so
`@cmd_*` / `@tool_*` searches report which cap fired.
## Tests added

- `grep_capped_collection_never_renders_as_a_complete_result` — table with **two rows**,
  capped (`limit=4`) and complete (`limit=50`), over the same 9 matches. The complete row
  **was green before the fix** and is the point: the defect is that the two renderings
  were byte-identical, so a test asserting only on the capped string would have passed
  against the buggy output. Asserts the capped hint carries no `of N matches`, that it
  names the total as unknown, that `total_is_lower_bound` is set, and that the capped
  *first line* differs from the complete one.
- `grep_buffer_capped_collection_marks_the_total_as_a_floor` — the `grep_in_buffer` twin.
  It carries its own copy of the collect-then-`cap_grouped` sequence, so a one-site fix
  would have left it reporting `shown == total` with nothing marking the floor.

Both were watched fail first; failure output is quoted under *Reproduction*.
## Workarounds

No longer needed on `experiments`. Before the fix: when a `grep` result said "Showing N of
N", treat N as a floor, not a total, and re-run with a narrower pattern or `mode="files"`.
## Resume

N/A — fixed in `4b77dff5`, gate green, verified live against the running MCP server on
2026-08-16:

```
grep(pattern="^use serde_json::json;", glob="src/**/*.rs", limit=5)

5 matches (capped) in 5 files
  … showing first 5 — Collection stopped at limit=5, so the true total is unknown —
    5 matches across 5 files is a floor, not a count. To see more, raise limit, or …
```

The three claims that used to contradict each other now agree. The same call on the old
build rendered `5 matches in 5 files` / `Showing 5 of 5 matches across 5 files.`

**Follow-ups, both filed, neither blocking:**

- `docs/issues/2026-08-16-grep-file-diversity-round-robin-never-runs.md` (BL-31) — the
  same single-`max` binding makes `cap_grouped`'s round-robin unreachable. Note the
  interaction: a capped grep usually spans **one** file, and `render_grouped` suppresses
  its header when `files <= 1`, so the `(capped)` marker added here is invisible in the
  common case. The overflow line still carries it. Fixing BL-31 restores the header
  marker to that path.
- Cosmetic, unfiled: the hint pluralizes unconditionally (`across 1 files`,
  `(1 matches)`). Pre-existing — the old format strings had the same `{} files` — so not
  a regression, but the rewritten hint makes it more visible.
## References

- `docs/trackers/bistriceanu/index.md` § B-5
- `docs/PROGRESSIVE_DISCOVERABILITY.md` — overflow-hint contract (Pattern 1: concrete + copy-paste-ready)
