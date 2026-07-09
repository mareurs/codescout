---
id: '3a8ae5f8cf96e5ea'
kind: bug
status: open
title: 'BUG: artifact(get) preview.headings silently capped at 20 (MAX_HEADINGS) with no signal, while line_count reflects the full file'
owners:
- marius
tags:
- librarian
- artifact
- preview
- silent-cap
- trackers
topic: null
time_scope: null
opened: '2026-07-10'
related:
- docs/issues/2026-07-07-artifact-get-full-body-silent-truncation.md
- docs/issues/2026-07-09-artifact-get-full-true-body-silent-truncation.md
severity: med
---

## Summary
`artifact(action="get")`'s `preview.headings` array is silently truncated to the first
`MAX_HEADINGS` (= 20) headings, with **no field indicating the cut**, while the sibling
`preview.line_count` reflects the *full* file. For any artifact with >20 headings (every
non-trivial tracker) the two disagree, and a reader trusting `preview.headings` sees a
false "last heading." This is the `preview`-path twin of the now-fixed `full=true` body
truncation (`2026-07-07`/`2026-07-09` bug files); the body path was made loud in
`97a36905`, but this preview path is still silent.

## Symptom (Effect)
`artifact(get, id="2dd9d90bc83f9f49")` (the bug-fix session log, 1841 lines, F-1..F-30 /
W-1..W-22) returns `preview.headings` ending at **F-7** — the 20th heading — while
`preview.line_count` is a correct `1841`. Nothing in the response says the heading list was
capped. An agent allocating the next F-N/W-N ID from `preview.headings` would pick "F-8",
colliding with the ~40 real entries past line 509. This is exactly the collision W-21 in
`docs/trackers/bug-fix-session-log.md` documents (and works around by grepping the raw file).

## Reproduction
1. `artifact(get, id="2dd9d90bc83f9f49")` (or any tracker with >20 headings).
2. Observe `preview.headings` stops at the 20th heading (F-7 for this tracker).
3. Observe `preview.line_count` is the full line count (1841) — no `headings_truncated`,
   `total_headings`, or similar field anywhere.

## Environment
codescout MCP server, Rust, project `codescout`, branch `experiments`, 2026-07-10.
Not platform-specific.

## Root cause
`src/librarian/preview/default.rs:9-23` (`default::extract`, the extractor for tracker kind
and any non plan/spec/memory kind):
```rust
let mut headings = headings::parse(body);   // parses ALL headings from full body
headings.truncate(MAX_HEADINGS);            // MAX_HEADINGS = 20 — silent cut
let line_count = body.lines().count();      // full count → disagrees with headings
```
`headings::parse` (`src/librarian/preview/headings.rs:15`) correctly returns every heading;
the cap is applied afterward with no companion signal. `line_count` is computed from the
full body, so the two fields are derived from different-length views without any marker
reconciling them. (Confirmed by `default.rs`'s own test `headings_are_extracted_and_capped`,
which asserts 25 headings → 20 with no truncation flag.)

## Evidence
- Live 2026-07-10: `artifact(get, id="2dd9d90bc83f9f49")` → `preview.headings` last entry
  "F-7 …" (20th heading), `preview.line_count: 1841`.
- `src/librarian/preview/default.rs:11` — `headings.truncate(MAX_HEADINGS)`.
- Cross-ref: `docs/trackers/bug-fix-session-log.md` W-21 counterfactual (this is failure #2
  of the two it cites; failure #1, the `full=true` body truncation, was fixed in `97a36905`).

## Hypotheses tried
1. **Hypothesis:** the preview slices the body to the first ~500 lines (F-7 sits near the
   500-line soft cap), so headings past that are unseen.
   **Test:** read `get.rs:419-420` (passes the *full* parsed body to `preview::extract`) and
   `default::extract`.
   **Verdict:** rejected — the body is not sliced; headings are parsed in full then
   `.truncate(20)`'d. F-7 being ~line 509 is a coincidence (it's simply the 20th heading).

## Fix
Not yet implemented. Options: (a) when `headings.len() > MAX_HEADINGS`, add a companion
signal to the preview (`total_headings: N` and/or `headings_truncated: true`) so the cut is
loud and `line_count`/`headings` no longer silently disagree; and/or (b) raise/remove the
cap for tracker-kind previews specifically (navigation is the whole point of a tracker's
heading list). (a) is the minimal correctness fix; (b) is a UX improvement. Prefer (a) at
minimum — mirror the "make truncation loud" principle applied to the body path in `97a36905`.

## Tests added
N/A — not yet fixed. A regression test should assert that a >20-heading body yields either
all headings or a `total_headings`/`headings_truncated` signal, and that the signal is
present exactly when `headings.len()` was reduced.

## Workarounds
Grep the raw tracker file for headings/IDs rather than trusting `preview.headings` on large
trackers (the W-21 practice). For a complete heading map, use `read_markdown` on non-managed
files, or a heading-scoped `artifact(get, heading=...)` when you already know the section.

## Resume
Fix in `src/librarian/preview/default.rs::extract` — add a truncation signal when
`headings.truncate(MAX_HEADINGS)` drops entries (option a), optionally raising the cap for
trackers (option b). Add the regression test described above. Same "make the silent cap
loud" shape as the `full=true` summary fix (`97a36905`, `src/librarian/adapter.rs`).

## References
- `docs/issues/2026-07-07-artifact-get-full-body-silent-truncation.md` (fixed) — sibling,
  the `full=true` body-truncation half of the same W-21 counterfactual.
- `docs/issues/2026-07-09-artifact-get-full-true-body-silent-truncation.md` (fixed) — sibling,
  same defect from the `read_file(json_path="$.body")` angle.
- `docs/trackers/bug-fix-session-log.md` W-21 — the ID-collision incident this enables;
  failure #2 of its two-part counterfactual.

