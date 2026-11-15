---
id: '28c8a999b2f3132b'
kind: bug
status: open
title: 'BUG: cutover plan builds its citation index before entry recovery, so the second plan differs from the first'
owners:
- marius
tags:
- cluster/unclassified
opened: 2026-11-15
unverified: no regression test yet; the finding is from the Plan D final re-review (Opus), and the controller verified only the stage order at plan.rs:257-259, not the three failing inputs
---

# BUG: cutover plan builds its citation index before entry recovery, so the second plan differs from the first

## Summary

`plan_export` (`src/librarian/cutover/plan.rs`) builds `CitationTables` at line 257 from the `HEAD` texts. Only afterwards does `plan_augmentations` (line 259) add the `## F-2 — two` sections that recovered params entries create. After `export` and a commit, the second plan's index includes those sections. An entry token that only a recovered params entry defines is resolved one way by the first plan and another way by the second.

## Class fit

No class in the roster states this mechanism: a planner whose later stage adds text that an earlier stage's index should already have seen. It is closest to the "second run differs from the first" traits but no class names the planner-index ordering, so `cluster/unclassified` is used until a class review.

## Symptom (Effect)

After the first `export` and commit, `cutover build` refuses with "the export is not committed: n files would change". A second `export` and commit converge. No data is lost.

## Reproduction

The reviewer's inputs (ledger fixture `LEDGER_MD` with a params entry `F-2`, through `assert_fixed_point`):

1. `docs/src.md` says `see F-2 here.` The first plan reports nothing. The second writes `[[ledger:F-2]]`.
2. `docs/src.md` says `see ledger:F-2`. The first plan reports it unrecoverable ("the file of that name does not define the token"). The second rewrites it.
3. A note event whose text is `see ledger:F-2` (runs through `added_text_rewrite`). The second plan rewrites the committed note.

## Environment

Branch `feat/cutover-tool-20261114`, landed on `integration/identity-lifecycle-20261114` at `ead2f7b31`.

## Root cause

Stage order in `plan_export`: `plan_file_renames`, `plan_skipped_events`, `plan_write_cites`, `CitationTables::build`, `plan_citations`, `plan_augmentations`, `plan_events_and_links`, `plan_outside_links`. The index is built before `plan_augmentations` inserts entry sections.

## Evidence

Review: `.superpowers/sdd/2026-11-14-artifact-identity-plan-d-cutover-tool/final-rereview.md` (git-ignored, in the cutover worktree), finding C1-a.

## Hypotheses tried

N/A

## Fix

Proposed by the reviewer: run entry recovery first with a rewrite that changes nothing, build the index from the recovered texts, then run the real plan. Add the three inputs above as tests.

Parked minors from the same review (report-only or test gap, no file written):

- C1-b: one base citation is reported twice because `insert_section` trims trailing spaces of the last base line (`entries.rs:296`), so `added_lines` (`rewrite.rs:176`) counts it as added.
- C1-c: a note holding `[O]: other.md` turns `[O]` in the base text into a reference link, so the second plan reports a new item.
- C1-d: a mutant that drops observation texts from `added_texts` survives the suite. Add a probe test (an observation citing an id only history knows).
- I1-a: the `.json` and `.txt` code-ref exclusions also skip configuration that a program reads by path (`CMakeLists.txt`, `package.json`). No instance on this repository.

## Tests added

None yet.

## Workarounds

Run `export`, commit, run `export` again, commit, then `build`.

## Resume

Fix on a branch from integration; the plan text (D-R9) already names this gap.

## References

Plan: `docs/superpowers/plans/2026-11-14-artifact-identity-plan-d-cutover-tool.md`.
