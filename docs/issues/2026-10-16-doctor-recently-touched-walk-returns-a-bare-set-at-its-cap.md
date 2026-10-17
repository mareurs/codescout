---
kind: bug
status: fixed
title: doctor's recently-touched-paths walk breaks at 4,000 commits and returns a bare set with nothing marking it partial
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-10-16
severity: low
closed: 2026-10-17
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

Decision taken: return the flag and publish it.

- `paths_touched_since` returns `(paths, truncated)`. `truncated` is set only on the `break`, i.e. only when a commit exists beyond the ceiling, so a history of exactly `max_commits` is complete and is not flagged. It means "may be partial", not "is wrong": whether the unread tail could hold a commit newer than the cutoff is not determined (that needs reading the tail, the cost the ceiling avoids).
- The scan is split into `scan_open_bug_cited_from_source` (a wrapper that pins `MAX_WALK_COMMITS` = 4000 and carries the `cap-class` annotation) and `scan_open_bug_cited_from_source_walking(.., max_walk_commits)`, so a test can inject a small ceiling. The check's doc comment now sits on the `_walking` function.
- The health block published at `catalog_health.open_bug_source_citations` gains `walk_truncated` (always present, `false` included) and `walk_max_commits`.
- `src/tools/core/cap_probe.rs`: `doctor.recently_touched_walk` moves `NoMarker` to `Probed` (marker `$.walk_truncated`, mutation `Killed`).

Verified before the fix: the code matched the report (`seen >= max_commits` broke with nothing marking the set partial). The commit count (5,582 vs 4,000) was not re-measured.

## Tests added

- `paths_touched_since_reports_whether_its_commit_ceiling_cut_the_walk_short` (`doctor.rs`): a 5-commit fixture with ceilings 3 (truncated, newest 3 files only), 5 (exactly the history: not truncated, all 5 files) and 4000 (not truncated).
- `open_bug_cited_from_source_health_says_when_the_walk_was_truncated` (`doctor.rs`): through the `_walking` seam, `walk_truncated` is `true` with `walk_max_commits` 2 and `false` at 4000.

Mutation: forcing the health field false reds the health test; removing `truncated = true` reds both.

## Resume

Nothing outstanding. Not archived on purpose; archive when the fix is on the shipping branch.

## Fix provenance

- **SHA:** `426feb369598cd6efc563f9c31a3b9618cc4f125` (branch `worktree-agent-a4845d73e6020762a`; one commit that also fixes `2026-10-16-doctor-exposure-floor-drops-worklist-rows-without-a-count.md`)
- **patch-id:** `ab2ff3ec97ecf247f9fa4368d716ae92b4044981` (`git show 426feb36 | git patch-id --stable`)

## References

- `src/librarian/tools/doctor.rs:7519-7580,7628,7762`
- `src/tools/core/cap_probe.rs` row `doctor.recently_touched_walk`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`
