---
kind: bug
status: fixed
title: Plan preview cuts `open_next` to 3 tasks and each task to 100 chars and stamps neither cut
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-10-16
severity: low
closed: 2026-10-17
---

## Summary

`preview::plan::extract` bounds two things in the `tasks` object it returns and discloses neither bound: `open_next` keeps the first `OPEN_NEXT_LIMIT` (3) open tasks, and every task string is cut to `TASK_TEXT_MAX` (100) characters. Filed from the `PROBE_ROWS` classification (`preview.plan_open_next`, `preview.plan_task_text` as `Coverage::NoMarker`) — see `docs/issues/archive/2026-09-03-deferred-conflates-untested-with-no-marker-exists.md`.

## Class fit

A capped result presented as complete: the `tasks.open_next` array and each task string arrive with no field saying a bound was applied, so a three-element array reads as "these are all the open tasks" and a 100-character string reads as the whole task. It differs from the class's loudest members in that the first cut is *derivable*: `tasks.total - tasks.done` is the true open count, so a careful reader can detect it; nothing says to.

## Symptom (Effect)

For a plan body with five unchecked tasks the preview carries `tasks: {total: 5, done: 0, open_next: [<3 strings>]}`. For a task of 150 characters the string is exactly 100 characters, with no ellipsis.

## Reproduction

Read `src/librarian/preview/plan.rs`: the cap at `plan.rs:41` (`if open_next.len() < OPEN_NEXT_LIMIT`) and the cut at `plan.rs:73-79` (`truncate_task_text`, `s.chars().take(TASK_TEXT_MAX).collect()`). The existing tests `open_next_returns_first_three_unchecked` and `task_text_truncated_to_limit` assert the cuts and no marker, which is how the absence is pinned.

## Root cause

`extract` (`plan.rs:14-71`) builds `open_next` with a length guard and `truncate_task_text` with a bare `take`; the returned `json!` (`plan.rs:60-67`) has keys `total`, `done`, `open_next` and no `open_next_truncated` / `tasks_truncated` sibling. Contrast `headings::stamp_truncation(&mut v, dropped, last_heading)` on the next line, which discloses the heading cut in the same object. Read from source at `experiments` f5f1c6f7 — inferred, not measured at runtime.

## Fix

Fixed in `src/librarian/preview/plan.rs`, judging that a window named `open_next` still needs to say it is one (the task-text cut had no such reading):

- `extract` counts every open task and, only when more exist than `OPEN_NEXT_LIMIT`, stamps `tasks.open_total` (the stated count, not left to `total - done`, which also disagrees for a `[~]`-style marker) and `tasks.open_next_truncated: true`. Absent otherwise, so a small plan's preview stays lean; mirrors `headings::stamp_truncation`. The window itself is unchanged.
- `truncate_task_text` appends `…` to a cut task (same shape as `memory.rs` `truncate_text`).
- `src/tools/core/cap_probe.rs`: `preview.plan_open_next` and `preview.plan_task_text` move from `NoMarker` to `Probed` (`Mutation::Killed`).

Fix commit `ae2033c9` on branch `worktree-agent-a56ad6e632040ceff`; patch-id `f11133a15f9b4ce03a63b16ae3177dcf3431782d`.

## Tests added

In `src/librarian/preview/plan.rs`: `open_next_cut_is_signaled_with_the_true_open_count` (5 open tasks vs cap 3, plus a `[~]` task that is neither open nor done), `open_next_within_the_cap_carries_no_truncation_marker`, `a_cut_task_text_ends_with_an_ellipsis`, `a_task_text_exactly_at_the_limit_is_not_marked_as_cut` (boundary). `task_text_truncated_to_limit` now allows the one `…` (`TASK_TEXT_MAX + 1`).

Red first: `open_next_cut_is_signaled_with_the_true_open_count` failed with `left: Null right: true` before the `extract` change. The ellipsis test was written before its fix but the first red build picked up the `truncate_task_text` edit, so its red is evidenced by the pre-existing `task_text_truncated_to_limit` failing on the 101st character and by the mutation run: deleting the `out.push('…')` and the `open_next_truncated` stamp reds both cited tests (`a_cut_task_text_ends_with_an_ellipsis`, `open_next_cut_is_signaled_with_the_true_open_count`).

## Resume

N/A once merged. Not done here: the `tasks.open_next_truncated` / `tasks.open_total` fields are not described in any prompt surface or manual page (none mentions the plan preview's `tasks` shape); `tests/result_caps.rs` passed unchanged.

## References

- `src/librarian/preview/plan.rs:9-12,41,73-79`
- `src/tools/core/cap_probe.rs` rows `preview.plan_open_next`, `preview.plan_task_text`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`
