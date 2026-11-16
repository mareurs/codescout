---
id: '28c8a999b2f3132b'
kind: bug
status: mitigated
title: 'BUG: cutover plan builds its citation index before entry recovery, so the second plan differs from the first'
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-11-15
opened: 2026-11-15
unverified: 'Only C1-c (a note holding `[O]: other.md`) and I1-a (config read by path under the .json/.txt exclusions) remain open; two nits (a test for the line-end conversion order, a stale comment on texts_with_entries) are parked. F1, F2 and the CRLF case are fixed and re-reviewed.'
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

Review: `.superpowers/sdd/2026-11-14-artifact-identity-plan-d-cutover-tool/final-rereview.md` (git-ignored, local to this machine; in the main checkout's `.superpowers/sdd/` since 2026-11-16), finding C1-a.

## Hypotheses tried

N/A

## Fix

Done (commits in `## Fix provenance`): the citation index is built from the `HEAD` texts with the params-entry sections recovered by a rewrite that changes nothing (`texts_with_entries`, `plan.rs`). The first plan now rewrites the three inputs, so the plan after export and commit writes nothing.

Also fixed: C1-b (`insert_section` at the end of a text strips only line ends) and C1-d (an observation probe test).

Still parked: C1-c (a note holding `[O]: other.md` turns `[O]` in the base text into a reference link, so the second plan reports a new item) and I1-a (the `.json` and `.txt` code-ref exclusions also skip configuration that a program reads by path, such as `CMakeLists.txt` and `package.json`; no instance on this repository). F1, F2 and the CRLF case are listed under `## Resume`.

## Tests added

Added by the fix on the feature branch: nine tests (the three C1-a fixed-point inputs, the D-R20 checks for a self-defining source, two definers and a frozen ledger, the C1-b trailing-line case, and the C1-d observation probe). Four mutations each went red.

## Workarounds

Run `export`, commit, run `export` again, commit, then `build`.

## Resume

F1, F2 and the CRLF case are fixed (`bd8d56a45`, `1d9a92428`; see `## Fix provenance`). `section_texts` in `entries.rs` now builds from `entry_sections`, the reader behind `defined_tokens`, so recovery and the index read definers the same way. A CRLF document keeps CRLF for the text the planner adds.

Still open and parked: C1-c, I1-a (see `## Fix`), plus two nits from the last re-review: mutation MC (the line-end conversion moved after the "unchanged" check) survives, and the doc comment on `texts_with_entries` (plan.rs, around L483) overstates its rule (say: it adds no line end or fence character, and never edits a line's leading definer token). Out of scope and also open: in a lone-CR ledger `insert_section` turns the final `\r` into `\n` (a fixed point, no content lost).

## References

Plan: `docs/superpowers/plans/2026-11-14-artifact-identity-plan-d-cutover-tool.md`. Execution state: `docs/research/2026-11-15-artifact-identity-plan-d-execution-state.md`.

## Fix provenance

- **SHA:** `51f0d971a` (feature branch `feat/cutover-tool-20261114`, fast-forwarded onto `integration/identity-lifecycle-20261114`)
- **patch-id:** `e4c02e175ebd5b83f6bb1d04d0637cced48f8624`
- **SHA:** `052140615` (same branch)
- **patch-id:** `aa20d437c4202c1d8789286482311727a7f61cfb`
- **SHA:** `308f9f5df` (same branch)
- **patch-id:** `9a86d11bfca031b03bffd0f867c064534fba5629`

- **SHA:** `bd8d56a45` (same branch)
- **patch-id:** `6fb6993b85939bed46c8e0676eacf5f240c9fad7`
- **SHA:** `1d9a92428` (same branch; integration tip `19e87e370`)
- **patch-id:** `d93009868529592ed25172b5e82863d01c18cda6`
