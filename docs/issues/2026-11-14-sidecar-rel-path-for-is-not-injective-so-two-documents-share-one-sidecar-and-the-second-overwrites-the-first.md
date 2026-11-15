---
kind: bug
status: open
title: 'BUG: rel_path_for is not injective, so two documents can share one augmentation sidecar and the second overwrites the first'
owners:
- marius
tags:
- cluster/unclassified
- librarian
opened: 2026-11-14
severity: medium
unverified: 'Read from the code, not run. Not established: whether any real repository holds two documents that collide (docs/a-b.md and docs/a/b.md is the shape), and what doctor(export_augmentations) prints for the second document. The cutover planner (Plan D Task 6) guards its own export; doctor is not guarded.'
---

# BUG: rel_path_for is not injective, so two documents can share one augmentation sidecar and the second overwrites the first

## Summary
`augmentation_sidecar::rel_path_for` joins the components of a repo-relative path with `-`. `docs/a-b.md` and `docs/a/b.md` both give `docs/augmentations/docs-a-b.yaml`. Its doc comment says the mapping is injective. It is not. A writer that checks only whether the sidecar exists in `HEAD` overwrites the first document's sidecar with the second one's.

## Class fit
cluster/unclassified: a name derived from a path by a lossy join, used as an identity for a file, with a doc comment that promises injectivity and no check that holds it. The writer trusts the promise.

## Symptom (Effect)
Two documents are stamped `expects_augmentation:` with the same sidecar path. The sidecar holds the shape of the document that was written last. The first document's catalog shape is lost, and its export item reports success.

## Reproduction
Not run. Read path:
- `src/librarian/augmentation_sidecar.rs:89-105` `rel_path_for`: `rel.with_extension("").components()` mapped to strings, `join("-")`. `a-b` and `a/b` give one stem.
- `src/librarian/tools/doctor.rs:2054-2083` `export_augmentation_sidecars`: checks the target path, not whether another document in the same run already claimed it.
- The cutover planner (`src/librarian/cutover/plan.rs`, Plan D Task 6) calls `sidecar_rel` for each final path and inserts a `FileChange` per path. The review found the same overwrite there; Task 6 fix round 1 guards the planner only (both documents become `unrecoverable`).

## Environment
Branch `feat/cutover-tool-20261114`, base `4c30def22`.

## Root cause
The mapping chose a readable, "reversible by eye" name over a collision-free one. Hyphens are valid in both file stems and folder names, so the join cannot be reversed. Renames to canonical stems (Plan D) make hyphenated stems more common.

## Evidence
Plan D Task 6 review (Opus, 2026-11-15), Critical 1. Code read as listed above.

## Hypotheses tried
None.

## Fix
Not started. Options: make the mapping injective (escape `-` inside a component, or use a path separator that cannot occur in a component), or keep the name and refuse a second claim in every writer. Changing the name moves every existing sidecar and needs a migration decision.

## Tests added
N/A: not fixed. A fix needs a test with both paths in one run, row order reversed.

## Workarounds
Do not keep two documents whose paths differ only in `-` versus `/` at the same position.

## Resume
Decide the mapping change versus the writer guard. The cutover planner already refuses the collision.

## References
- `docs/superpowers/plans/2026-11-14-artifact-identity-plan-d-cutover-tool.md` (Task 6, D-R24)
