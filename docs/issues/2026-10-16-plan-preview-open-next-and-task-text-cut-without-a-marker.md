---
kind: bug
status: open
title: Plan preview cuts `open_next` to 3 tasks and each task to 100 chars and stamps neither cut
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-10-16
severity: low
---

## Summary

`preview::plan::extract` bounds two things in the `tasks` object it returns and discloses neither bound: `open_next` keeps the first `OPEN_NEXT_LIMIT` (3) open tasks, and every task string is cut to `TASK_TEXT_MAX` (100) characters. Filed from the `PROBE_ROWS` classification (`preview.plan_open_next`, `preview.plan_task_text` as `Coverage::NoMarker`) — see `docs/issues/2026-09-03-deferred-conflates-untested-with-no-marker-exists.md`.

## Class fit

A capped result presented as complete: the `tasks.open_next` array and each task string arrive with no field saying a bound was applied, so a three-element array reads as "these are all the open tasks" and a 100-character string reads as the whole task. It differs from the class's loudest members in that the first cut is *derivable*: `tasks.total - tasks.done` is the true open count, so a careful reader can detect it; nothing says to.

## Symptom (Effect)

For a plan body with five unchecked tasks the preview carries `tasks: {total: 5, done: 0, open_next: [<3 strings>]}`. For a task of 150 characters the string is exactly 100 characters, with no ellipsis.

## Reproduction

Read `src/librarian/preview/plan.rs`: the cap at `plan.rs:41` (`if open_next.len() < OPEN_NEXT_LIMIT`) and the cut at `plan.rs:73-79` (`truncate_task_text`, `s.chars().take(TASK_TEXT_MAX).collect()`). The existing tests `open_next_returns_first_three_unchecked` and `task_text_truncated_to_limit` assert the cuts and no marker, which is how the absence is pinned.

## Root cause

`extract` (`plan.rs:14-71`) builds `open_next` with a length guard and `truncate_task_text` with a bare `take`; the returned `json!` (`plan.rs:60-67`) has keys `total`, `done`, `open_next` and no `open_next_truncated` / `tasks_truncated` sibling. Contrast `headings::stamp_truncation(&mut v, dropped, last_heading)` on the next line, which discloses the heading cut in the same object. Read from source at `experiments` f5f1c6f7 — inferred, not measured at runtime.

## Fix

Not designed. Candidate: stamp `tasks.open_next_dropped` (count) and mark a cut task with `…`, mirroring `headings::stamp_truncation`. Whether `open_next` is a window by name (a "next 3" is not a claim of completeness) or a defect is a judgement for the owner; the task-text cut has no such reading.

## Tests added

None — filing only. Once a marker exists the two `preview.*` rows can become `Probed` (the existing tests read `open_next` through a `let` and `.len()`, which the cap-probe gate cannot certify).

## Resume

1. Decide whether a window named `open_next` needs a marker.
2. If so, add the marker and move `preview.plan_open_next` / `preview.plan_task_text` from `NoMarker` to `Probed` in `src/tools/core/cap_probe.rs`.

## References

- `src/librarian/preview/plan.rs:9-12,41,73-79`
- `src/tools/core/cap_probe.rs` rows `preview.plan_open_next`, `preview.plan_task_text`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`
