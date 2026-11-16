---
id: b2ec2d2e35fbbf30
kind: bug
status: fixed
title: 'BUG: a doc(link) rel spelled `Supersedes` writes the backward Supersedes line but does not mark the destination superseded'
owners:
- marius
tags:
- cluster/unclassified
- librarian
closed: 2026-11-16
opened: 2026-11-14
severity: low
---

# BUG: a doc(link) rel spelled `Supersedes` writes the backward Supersedes line but does not mark the destination superseded

## Summary
`link::call` (`src/librarian/tools/link.rs`) treats a link as superseding only when `a.rel == "supersedes"` (exact match). `label_for_rel` lowercases first, so a rel spelled `Supersedes` or `SUPERSEDES` gives the label `Supersedes`. That label is in `lifecycle::relations::BACKWARD_LABELS`. The source document then declares a backward `**Supersedes:** [[dst]]` line, but the destination's status stays unchanged and no `superseded_by` event is written.

## Class fit
cluster/unclassified: two readers (the exact-match code and the case-insensitive label) disagree on what one word means.

## Symptom (Effect)
The text says document A supersedes document B. Document B keeps `status: active`. The lifecycle checks read `Supersedes` as provenance, so they ignore it as a forward wait, and nothing reports the mismatch.

## Reproduction
Not reproduced. Read path: `doc(link, src=A, dst=B, rel="Supersedes")`; compare B's frontmatter before and after.

## Environment
experiments at 5fc3680f4.

## Root cause
The branch `superseding = a.rel == "supersedes"` is case-sensitive. The label mapping is not.

## Fix

Fixed by Plan E2 Task 9. One predicate, `is_superseding(rel)` in `src/librarian/catalog/link_text.rs`, is `label_for_rel(rel).is_ok_and(|label| rel_of_label(&label) == "supersedes")`. `link::call` and `artifact::write_target_id` both use it, so every spelling (`Supersedes`, `SUPERSEDES`) marks the destination superseded and the result names the destination file. A superseding row stores the canonical `supersedes`. Tests: `a_supersedes_rel_in_any_spelling_marks_the_destination_superseded` (link.rs) and `an_update_that_changes_status_names_no_path_in_either_spelling` (artifact.rs, the `Supersedes`, `SUPERSEDES` and `Super sedes` cases). Canonical rel rule: E2-R4 (bug `84e6e61e03beace6`).

## Fix provenance

- **SHA:** `dfb931dbb` (`integration/identity-lifecycle-20261114`; reaches `experiments` with the next integration merge)
- **patch-id:** `b2a26cd3773115a316877e85bb2cbb5b4577fe33`
- Follow-up (one predicate for the result path): `0bcee8145`, patch-id `71f8ec31c88464337618d5aa460655f68b7cc4c9`; the predicate moved beside `label_for_rel` in the final wave (`1adf383b2`).

## Resume

N/A. Fixed 2026-11-16 by Plan E2 Task 9; an older stored row with a variant spelling is not migrated (a repeated `Supersedes` link adds a second `supersedes` row: E2 execution state, known limits).

## References
- `src/librarian/tools/link.rs` (`call`, the `superseding` flag)
- `src/librarian/lifecycle/relations.rs` (`BACKWARD_LABELS`)
- `docs/research/2026-11-14-artifact-identity-plan-c2-execution-state.md`
