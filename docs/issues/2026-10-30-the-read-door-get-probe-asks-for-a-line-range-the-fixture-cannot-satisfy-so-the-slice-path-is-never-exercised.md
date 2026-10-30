---
id: '4ffa75bf5548c47e'
kind: bug
status: open
title: 'BUG: the read-door probe for doc(get) asks for lines 1-40 of a 17-line body, so the line-slice path is refused and never exercised'
tags:
- cluster/unclassified
- read-door
- test-vacuity
opened: 2026-10-30
owner: marius
severity: low
unverified: Inferred from counting the fixture's body lines and reading the guard; site_get was not run to see the refusal.
---

# BUG: the read-door probe for doc(get) asks for lines 1-40 of a 17-line body, so the line-slice path is refused and never exercised

## Summary

`site_get` in `src/librarian/tools/read_door_tests.rs` probes the three body selectors of `doc(get)`: `full`, `start_line`/`end_line` and `heading`. The line-range call sends `start_line: 1, end_line: 40`. The fixture's body (`doc_text()`) has 17 lines. `doc(get)` refuses a line beyond the body, so this call returns a refusal that names no marker and never reaches the slice code. The probe reports "no leak" for that selector because it never read anything.

## Class fit

A check that cannot fail on the path it names. The result reads as coverage of the line slice, and it is not.

## Symptom (Effect)

None observed. `full` and `heading` still read the file, so a read-door regression that affected every selector would still be caught. A regression that affected only the line slice would not.

## Reproduction

Read `doc_text()` in the same file: after the 4-line frontmatter and one blank line the body has 17 lines (counted from the format string on 2026-10-30). The old guard refused any line above the body's line count. After `e9eaf5f26` the valid file lines are 6 to 22, and 40 is refused as well. Not run: I counted the lines and read the guard, and did not call `site_get` with a debugger to see the refusal.

## Environment

`experiments` at `e9eaf5f26`.

## Root cause

Not established. The range reads like a generous bound ("the first 40 lines") that nobody checked against the fixture's length. I did not look at the git history to see whether the out-of-range guard came after the probe. Nothing asserts that the call returns a body.

## Evidence

`site_get` (read_door_tests.rs, `let lines = get::call(&ctx, json!({"id": id, "start_line": 1, "end_line": 40}))`); `doc_text()` above it. Since `e9eaf5f26` line 1 is also inside the frontmatter, which is refused too.

## Hypotheses tried

None.

## Fix

Not implemented. Candidate: send a range that lies inside the body, for example the file lines 6 to 22 derived from `doc_text()`, and assert in the positive-control layout that the call returns a body. This needs a positive control, because a refusal and a clean read look the same to `out_anyhow`.

## Tests added

N/A: this file records a finding.

## Workarounds

None needed; the `full` and `heading` calls still probe `doc(get)`.

## Resume

Read `site_get` and its positive-control tests; decide whether the control belongs in `site_get` or in a separate test.

## References

- docs/issues/archive/2026-09-24-residual-doc-get-line-range-frame-decision.md (found while fixing it)
