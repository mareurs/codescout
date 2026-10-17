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

Decision taken: return the flag and publish it, and make the flag mean harm.

- `paths_touched_since` returns a `TouchedWalk { paths, ceiling_reached, ceiling_reached_in_window, unavailable }`. `ceiling_reached` is set only on the `break`, i.e. only when a commit exists beyond the ceiling, so a history of exactly `max_commits` is complete and is not flagged. `ceiling_reached_in_window` additionally requires that the last commit read is still at or after the cutoff, so the unread tail may hold in-window commits. It means "may be partial", not "is wrong" (committer times are not monotonic, so an unread newer commit cannot be ruled out by the walk). `unavailable` is set when `revwalk` or `push_head` fails.
- **Why not one boolean (review of `426feb36`):** measured 2026-10-17, this repo has 7,739 commits against the 4,000 ceiling, and the 4,000th/4,001st commit in `git log --date-order` is ~51 days older than the 7-day cutoff. The first version's `walk_truncated` was therefore `true` on every call here and told a reader nothing, and replacing the logic with a hard-coded `true` changed nothing anyone could see. Now `walk_truncated` is `true` only for the harmful case; the ordinary state is published beside it as `walk_ceiling_reached`.
- **The failure case is honest (review):** an unopenable history used to return `(empty, false)`, which the health comment defined as "walked to the end". It now sets `walk_unavailable`.
- The scan is split into `scan_open_bug_cited_from_source` (a wrapper that pins `MAX_WALK_COMMITS` = 4000 and carries the `cap-class` annotation) and `scan_open_bug_cited_from_source_walking(.., max_walk_commits)`, so a test can inject a small ceiling. The check's doc comment now sits on the `_walking` function.
- The health block at `catalog_health.open_bug_source_citations` gains `walk_truncated`, `walk_ceiling_reached`, `walk_unavailable` (always present, `false` included) and `walk_max_commits`.
- `src/tools/core/cap_probe.rs`: `doctor.recently_touched_walk` moves `NoMarker` to `Probed`, marker `$.catalog_health.open_bug_source_citations.walk_truncated` (the shape the caller reads, not the scan-level object), mutation `Killed`, cited to a `call()`-level test; the row's reason carries the 2026-10-17 figures instead of the superseded 2026-09-09 ones (5,582 commits, tail ~115 days).

## Tests added

- `paths_touched_since_reports_whether_its_commit_ceiling_cut_the_walk_short` (`doctor.rs`): a 5-commit fixture with ceilings 3 (reached; the set is exactly 3 files, `filler-1.txt` and `src/first.rs` absent), 5 (exactly the history: not reached, all 5 files) and 4000 (not reached, not unavailable).
- `the_ceiling_is_only_harmful_when_it_bites_inside_the_settle_window` (`doctor.rs`): six commits aged 0.5, 2.5, 4.5, 6.5, 8.5, 10.5 days against a 7-day cutoff. Ceiling 3 is reached in-window (harmful); ceiling 5 is reached but past the cutoff (harmless: `ceiling_reached` true, `ceiling_reached_in_window` false); ceiling 6 is complete.
- `an_unreadable_history_is_reported_unavailable_not_complete` (`doctor.rs`): an unborn HEAD sets `unavailable` and no ceiling flag.
- `open_bug_cited_from_source_health_says_when_the_walk_was_truncated` (`doctor.rs`): the same three cases through the `_walking` seam and the health block: `walk_truncated` true / false / false, `walk_ceiling_reached` true / true / false.
- `call_publishes_walk_truncated_when_the_production_ceiling_bites_in_the_window` (`doctor.rs`): `call()` over a 4,002-commit repo (one shared tree, all in-window) against the real 4,000 bound; asserts `/catalog_health/open_bug_source_citations/walk_truncated` is `true` and `walk_max_commits` is 4000. This is the test the probe row cites. It builds in about a second.

Mutations observed red (2026-10-17): `ceiling_reached_in_window` forced `true` (`the_ceiling_is_only_harmful_...`: `the last commit read is 8.5 days old, past the cutoff ... must NOT read as truncation`, and the health test); forced `false` (`the_ceiling_is_only_harmful_...`, the health test, and `call_publishes_walk_truncated_...`: `4,002 in-window commits against the 4,000 ceiling`); `seen >= max_commits` changed to `seen > max_commits` (`paths_touched_since_reports_...`: `exactly the newest 3 commits are walked - one more means the ceiling moved: {filler-1, ..., filler-4}`, left 4 right 3; this survived every test of the first version); `unavailable` not set on a failed `push_head` (`an_unreadable_history_...`); `walk_truncated` published as `walk.ceiling_reached`, the first version's meaning (the health test's harmless side). The first version's mutations (health field forced false; `truncated = true` removed) were also red.

## Resume

Nothing outstanding. Not archived on purpose; archive when the fix is on the shipping branch.

## Fix provenance

- **SHA:** `426feb369598cd6efc563f9c31a3b9618cc4f125` (branch `worktree-agent-a4845d73e6020762a`; one commit that also fixes `2026-10-16-doctor-exposure-floor-drops-worklist-rows-without-a-count.md`), patch-id `ab2ff3ec97ecf247f9fa4368d716ae92b4044981` (`git show 426feb36 | git patch-id --stable`)
- **SHA:** `b232271cefaa2a30a602717eeda2cfe529d05c93` (same branch; review fixes: harm-meaning flag, `unavailable`, caller-shape marker), patch-id `79f2a4ea13ddd63e8222cba2e92c7c5ef1fbc407` (`git show b232271c | git patch-id --stable`)

## References

- `src/librarian/tools/doctor.rs:7519-7580,7628,7762`
- `src/tools/core/cap_probe.rs` row `doctor.recently_touched_walk`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`
