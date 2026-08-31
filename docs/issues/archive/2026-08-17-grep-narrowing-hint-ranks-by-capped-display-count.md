---
id: '4176d661e5b2a989'
kind: bug
status: fixed
title: 'BUG: grep''s overflow narrowing hint ranks and reports the per-file CAPPED display count, so it recommends 3-match files and never names the 20-match one'
tags:
- cluster/capped-result-presented-as-complete
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

**Traced 2026-08-17.** The hypothesis below was right. `src/tools/grep.rs`, simple
mode:

```rust
let (visible, total, files) = cap_grouped(matches, max_matches);
let truncated = hit_cap || total > visible.len();
let groups = group_by_file(&visible);          // <- post-cap
...
let top: Vec<String> = groups
    .iter()
    .take(3)
    .map(|g| format!("path=\"{}\" ({} matches)", g.file, g.items.len()))
    .collect();
```

`groups` is built from `visible`, the set that survived `cap_grouped`'s
file-diversity round-robin. So `g.items.len()` is the **displayed** row count, and
`group_by_file`'s "size desc, ties by path asc" sort runs over those capped numbers.
The cap flattens nearly every file to the same small count, so almost everything
ties and the path-ascending tiebreak decides the ranking — which is why the offered
candidates looked arbitrary.

`groups` itself is correct for its other use, `groups_to_json(&groups)`: that IS the
displayed grouping. Only the hint needed the pre-cap tally, and it was reusing the
nearest available variable.

**Confined to the filesystem path.** `grep_in_buffer` also calls
`cap_grouped` + `group_by_file(&visible)`, but builds only generic overflow text
("Many matches. Narrow the pattern.") with no per-file candidates — so it never had
this defect and needed no change.

**The `d8c8` interaction was real.** `d8c2b23d` (*give cap_grouped something to
choose from, so diversity actually runs*) decoupled the collection limit from the
display budget. Before it, `cap_grouped` early-returned on `budget >= total` every
time and `visible == matches`, which made the post-cap tally *equal* the pre-cap
tally — so this code was correct when written and was silently invalidated by the
commit that made the cap start binding. The comment at the collection site says as
much about a sibling symptom (BL-31).

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

Implemented in `src/tools/grep.rs`. The narrowing candidates are now computed from
the **pre-cap** match set, before `cap_grouped` consumes it:

```rust
let precap_top: Vec<(String, usize)> = group_by_file(&matches)
    .iter()
    .take(3)
    .map(|g| (g.file.to_string(), g.items.len()))
    .collect();
let (visible, total, files) = cap_grouped(matches, max_matches);
```

Owned `(String, usize)` pairs rather than borrowed `FileGroup`s, because
`group_by_file` borrows `matches` and `cap_grouped` takes it by value — the borrow
has to end before the move. Cost is one extra grouping pass over the collected
matches, bounded by the collection limit.

This reuses `group_by_file`'s existing "size desc, ties by path asc" ordering rather
than introducing a second ranking rule; the only change is *what it is given*.
`groups_to_json(&groups)` still uses the post-cap grouping, which is correct for the
displayed rows.

**Plus the floor marker.** When `hit_cap` is set, collection itself stopped early, so
even the pre-cap tally is a lower bound for that file — the count renders as `16+`
rather than `16`. Without this the fix would have replaced one piece of false
precision with another one level down. This surfaced during testing rather than
design: a `limit=5` probe reported `(16+ matches)` for a file holding 40, because the
candidate cap is a multiple of `limit` and had truncated the walk at 20.

On the example from § Symptom the hint now offers `tracker_design.rs` (20),
`augment.rs` (18), `augmentation.rs` (11) — three files holding 49 of 115 matches,
where it previously offered three holding 19 and misreported each as 3.

Fix SHA: this commit, on `experiments`. `master` is a strict ancestor at fix time,
so the promotion path is fast-forward and this SHA is already the master SHA.
## Tests added

`tools::grep::tests::grep_overflow_hint_counts_and_ranks_before_the_cap`.

The interesting part is why the **existing** test did not catch this.
`grep_overflow_hint_names_top_files` already had a skewed fixture (40 / 2 / 1) and
already overflowed — it sat directly on the bug and stayed green, because it asserts
only that the hint *mentions* `hot.rs` and that the word "matches" appears. It never
asserts a number. A test can occupy exactly the right position and still be blind to
the defect if it checks presence where the defect is in a value.

The new test pins both halves:

- **The count.** `hot.rs" (40 matches)` exactly, with no `+`. Ranking on the post-cap
  set reports a single-digit number here — the mutation below produced `11`.
- **The ranking.** `aaa_decoy.rs` is the discriminator: alphabetically first, only 3
  real matches. Ranking on capped counts ties the files and falls back to path order,
  putting the decoy first; ranking on the true tally puts `hot.rs` first. Without a
  decoy that sorts before the hot file, no fixture can tell the two implementations
  apart.

`limit=15` is chosen deliberately: it caps the display (15 of 44) without capping
collection, so the tally is a count rather than a floor and the assertion can be an
exact number. A smaller limit truncates the walk too — the first draft used
`limit=5` and had to assert `16+`, which is a weaker statement.

**Mutation-verified.** Restoring the original
`groups.iter().take(3).map(|g| … g.items.len())` turns it red with
`path=".../hot.rs" (11 matches)` — the defect verbatim, on a file holding 40.

Gate: `cargo fmt` clean, `cargo clippy --all-targets -- -D warnings` clean,
`cargo test` 4017 passed / 0 failed / 45 ignored.
## Workarounds

Treat the `(N matches)` annotations and the path selection as unreliable
whenever a `mode="content"` grep overflows. Run `grep(..., mode="files")` to see
the real distribution, then narrow with `path=` to the file you actually want.
The totals in the envelope are trustworthy; only the per-file breakdown is not.

## Resume

N/A — root cause traced, fixed, mutation-verified, and confirmed on the wire after
`cargo rb` + `/mcp`. The exact pair of calls from § Symptom, re-run:

```
grep(pattern="render_template", glob="src/**/*.rs", mode="files")
→ 20  tracker_design.rs | 18  augment.rs | 11  augmentation.rs | …

grep(pattern="render_template", glob="src/**/*.rs", mode="content")
→ To trim, narrow with one of: path=".../tracker_design.rs" (20 matches),
  path=".../augment.rs" (18 matches), path=".../catalog/augmentation.rs" (11 matches).
```

Every figure now matches `mode="files"` exactly, the ranking is the true one, and no
`+` appears because collection completed. Before the fix the same call offered three
files annotated `(3 matches)` — holding 11, 5 and 18 — and never named the file
holding 20.

One observation left deliberately unactioned: `grep_overflow_hint_names_top_files`
remains as it was. It is now redundant with the new test on every property it
checks, but it is also the I-5 regression guard for the hint being
copy-paste-ready at all, so it is kept rather than merged.
## References

- `src/tools/grep.rs` — grep implementation, incl. `cap_grouped`
- `d8c2b23d` — *fix(grep): give cap_grouped something to choose from, so diversity actually runs*
- `docs/PROGRESSIVE_DISCLOSURE.md` — output sizing and overflow-hint conventions
