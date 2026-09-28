---
id: d59ef0849a8f45cb
kind: bug
status: archived
title: An archive made without Fix provenance is invisible to every check, because doctor skips all archived records
tags:
- librarian
- doctor
- cluster/guard-narrower-than-its-name
closed: 2026-09-28
opened: 2026-09-28
severity: medium
---

## Summary

`doctor`'s `terminal_status_without_fix_anchor` is the only check for the `## Fix provenance` section that `get_guide("tracker-conventions")` § *Bug files* requires **at archive time**. It skips every `archived` record (`terminal_status_without_fix_anchor_leaves_archived_records_alone`, `src/librarian/tools/doctor.rs`). So a record archived without the section is reported nowhere, and the one moment the rule applies is the moment its check stops looking. The skip covers the whole archive rather than a date range: its doc comment justifies it by the 297 of 355 archived files that predated the rule, and it hides a record archived today just the same.

## Symptom (Effect)

No finding, before or after the move. A `fixed` record without the section reports under `terminal_status_without_fix_anchor`; flip it to `archived` and move it, and the finding disappears whether or not the section was added. The drop in the count reads as the repair.

## Reproduction

1. A `fixed` bug with no `## Fix provenance` section: `librarian(action="doctor")` reports it under `terminal_status_without_fix_anchor`.
2. `doc(action="update", patch={status: "archived"})`, then `doc(action="move")` into `docs/issues/archive/`.
3. `doctor` again: the finding is gone.

Observed 2026-09-28 by session `82cff72e`. It archived `e76b043bbdd97622` and `60fcfdf99e3288c6` without the section, one with the pair in prose and one with it only in the commit message, and no check reported either. It surfaced only when both were grepped while archiving five others (`8dfc251e`), and the doctor drop for that pass, 15 to 7 on this check, would have been identical with or without those five sections.

## Environment

`experiments` at `8dfc251e`; release binary built from `6a6a321e`.

## Root cause

A scope decision made for the backlog and applied to every record: `scan_terminal_status_without_fix_anchor` excludes `archived` by status, never by when the record was archived.

## Evidence

Derived 2026-09-28 on the tree at `8dfc251e`, by parsing every `docs/issues/archive/*.md` whose frontmatter says `kind: bug` (923 files). A file counts as missing when it has neither a `## Fix provenance` heading outside a fenced block nor a non-empty `no_fix_commit:`:

| `closed:` in | archived bug files | missing both |
|---|---|---|
| 2026-09-14 to 2026-09-20 | 59 | 45 |
| 2026-09-21 to 2026-09-27 | 54 | 30 |

Both are upper bounds on the defect. They include records closed `wontfix`, which owe no pointer, and records whose pair sits in prose. The check would count the prose records as missing too, but a reader could still recover the pair from them.

## Hypotheses tried

None needed: the skip is deliberate and named by its own test.

## Fix

**Chosen: check at the transition, and refuse** (the operator's choice, 2026-09-28, over a warning or a date-scoped doctor check).

- **One predicate.** `doctor::declares_fix_anchor` is true when the file has a parseable `## Fix provenance` pointer or a non-empty `no_fix_commit:`. `terminal_status_without_fix_anchor` and both guards call it, so the check and the guards cannot disagree about what "anchored" means. `doctor::in_archive_dir` replaces three byte-identical inline copies of the archive-path test.
- **One guard, two surfaces.** `doctor::refuse_unanchored_archive` refuses when the record is a bug, its status is `fixed` or `mitigated`, and no anchor is declared. The refusal names both remedies: add the section, or declare `no_fix_commit`.
  - `update` calls it when `patch.status == "archived"`, against `new_content`, the file as it will be written. So one update may add the section and archive in the same call.
  - `move` calls it when the destination is under `archive/` and the source is not.
- **Out of scope, as they are for the check:** `wontfix`, `open` and non-bug records, and a move within `archive/`.

The backlog of post-rule archives that already lack an anchor is not touched. The guard only stops new ones.

A sibling on the same check is still open: `5394e9b7bdd83069` (a SHA with no patch-id discharges it).

## Fix provenance

- **SHA:** `04973710` (`experiments`)
- **patch-id:** `a2ca027a70958e42e1137dd0ab4f6d6bc49c560e`

## Tests added

Eight tests, in `src/librarian/tools/update.rs` and `src/librarian/tools/mv.rs`. Three were red before the fix:

- `archiving_a_fixed_or_mitigated_bug_with_no_fix_anchor_is_refused_and_writes_nothing` (red)
- `a_non_empty_no_fix_commit_discharges_the_archive_guard_and_an_empty_one_does_not` (red)
- `moving_an_unanchored_fixed_or_mitigated_bug_into_archive_is_refused_and_moves_nothing` (red)
- `archiving_passes_when_the_same_update_adds_the_provenance_section`
- `moving_an_anchored_fixed_bug_into_archive_succeeds`
- `the_archive_guard_leaves_wontfix_open_bugs_and_non_bugs_alone`
- `the_move_guard_leaves_non_archive_destinations_already_archived_bugs_and_archive_internal_moves_alone`
- `an_update_that_does_not_archive_is_not_guarded`

Each control input is admitted by every condition except the one it names.

Nine mutations were run through `scripts/mutation-probe.sh`, one per guarded site, and all were KILLED:

- the `update` trigger;
- `move`'s destination and source conditions;
- the helper's kind, status and anchor conditions. The status mutant also kills 5 pre-existing `mv` tests that archive `open` bugs.
- the predicate's `no_fix_commit` and pointer branches. Both also kill pre-existing doctor tests, which shows the check really goes through the shared predicate.
- `original` substituted for `new_content`.

The gate at the fix: `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`, and all 8 tests pass in the default lane.

## Workarounds

Before archiving, grep the file for `^## Fix provenance` yourself; do not read the check's count going down as confirmation.

## Resume

Closed. The guard reaches a session only once it runs a server built from `04973710` or later (`./scripts/rb.sh`, then `/mcp` in each session).

## References

- `docs/issues/archive/2026-08-19-terminal-bug-file-with-no-recoverable-fix-anchor.md` (the check's origin)
- `docs/trackers/bug-fix-session-log.md` (the session entries for this pass)
