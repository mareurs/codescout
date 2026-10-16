---
kind: bug
status: open
title: doctor's recently-touched-paths walk breaks at 4,000 commits and returns a bare set with nothing marking it partial
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-10-16
severity: low
---

## Summary

`paths_touched_since` (used by the `open_bug_cited_from_source` check) walks `git2` history newest-first, stops at `max_commits` (`MAX_WALK_COMMITS`, 4,000) and returns a plain `HashSet<String>`. Nothing marks the set partial. The row's own measurement says the ceiling is reached on every call in this repo (5,582 commits against 4,000, 2026-09-09), but the walk is time-sorted, so the unreached tail is ~115 days older than the 7-day cutoff and no real answer changes today. Filed from the `PROBE_ROWS` classification of `doctor.recently_touched_walk` as `Coverage::NoMarker`.

## Class fit

A cap that truncates and returns a value that reads as complete: the set is consumed as "every path touched in the last 7 days". It differs from the other members in being, today, harmless in practice: the bound exists to tolerate non-monotonic committer-time order across merges and rebases (`doctor.rs:7519-7527`), and the truncated tail would have to be *newer than the cutoff* to matter. The defect is the missing disclosure on a condition that can occur, not an observed wrong answer.

## Symptom (Effect)

No wrong answer observed. On a repository whose first 4,000 commits in time order do not include every commit newer than the cutoff, a source citation of a live bug file is judged "unsettled" against an incomplete `recently_touched` set, and the check's output does not say the walk was cut. Inferred from source — not reproduced (needs a >4,000-commit fixture with non-monotonic committer times across the boundary).

## Reproduction

`src/librarian/tools/doctor.rs`: `fn paths_touched_since` (`:7534`) breaks at `seen >= max_commits`; the call site is `:7762` (`paths_touched_since(&repo, cutoff, MAX_WALK_COMMITS)`, `MAX_WALK_COMMITS` at `:7628`). The old row reason also records the measurement: 5,582 commits against a 4,000 walk, 2026-09-09. Read at `experiments` f5f1c6f7 — source inferred; the commit count was not re-measured here.

## Root cause

The walk's two bounds are documented in the function's doc comment, but the return type has no field for "the commit ceiling was hit". `scan_open_bug_cited_from_source` therefore cannot tell its caller (or its report) that `recently_touched` is a lower bound.

## Fix

Not designed. Candidate: return `(set, truncated: bool)` and put `walk_truncated: true` in the check's summary `Value` (the check already returns a `Value` alongside its violations, used for notes elsewhere in the same function).

## Tests added

None — filing only. The old row notes why a probe is hard: the fixture needs >4,000 commits with non-monotonic committer time across the boundary; an injectable `max_commits` (already a parameter of `paths_touched_since`) makes a small fixture enough.

## Resume

1. Return the truncation flag and surface it in the check's summary.
2. Drive `paths_touched_since` with a tiny `max_commits` in a fixture repo; move `doctor.recently_touched_walk` to `Probed`.
3. Shares `doctor.rs` with the catalog owner — coordinate before editing.

## References

- `src/librarian/tools/doctor.rs:7519-7580,7628,7762`
- `src/tools/core/cap_probe.rs` row `doctor.recently_touched_walk`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`
