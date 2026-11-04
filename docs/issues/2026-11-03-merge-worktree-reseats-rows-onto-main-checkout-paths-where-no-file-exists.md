---
id: '93a467ab0eb9bbf4'
kind: bug
status: fixed
title: 'BUG: merge_worktree reseats a worktree-born row onto a main-checkout path where no file exists, so each reseated row becomes a missing_file defect'
owners:
- marius
tags:
- cluster/unclassified
- worktree-overlay
- librarian
closed: 2026-11-03
opened: 2026-11-03
owner: marius
related:
- bad49b228663df9a
severity: medium
unverified: 'The code fix and its regression test pass the gate, and the live MCP server now runs the fixed binary (strings present in target/release/codescout, built 2026-11-04). No live run reached the new branch: a dry run on the only remaining active registration (615e) was refused earlier because its branch research/fixed-acceptance-population-20261101 has diverged from experiments. The 78 rows reseated before the fix still point at missing files (missing_file 78 in doctor): this fix stops new cases, it does not repair those. merge_one was read and has no reseat step, so it cannot create a row at a new path.'
---

# BUG: merge_worktree reseats a worktree-born row onto a main-checkout path where no file exists, so each reseated row becomes a missing_file defect

## Summary

`librarian(action="merge_worktree")` reseats each lineage-less worktree row to its main-checkout path without checking that a file exists there. When the file lives only in the removed worktree (or on an unmerged branch), the row now points at a path that is absent on disk. `doctor` then reports one `missing_file` defect per reseated row. Measured 2026-11-03: three merges reseated 78 rows, and `summary.defects` rose from 296 to 374 (`missing_file` 0 to 78).

## Class fit

No existing cluster names this shape: a catalog operation that is correct for its own invariant (move the row to the main path) and breaks a different check (the file must exist). The nearest class is a repair that creates the defect another check reports, but no slug records that, so this record uses `cluster/unclassified`.

## Symptom (Effect)

After `merge_worktree` on `/tmp/codescout-real-task-20261024` (49 reseated), `.codex/worktrees/9369/codescout` (28) and `.codex/worktrees/3940/codescout` (1), `doctor` reports `missing_file` 78 times. Each finding names a main-checkout path such as `docs/issues/2026-10-24-build-dirty-metadata-stays-clean-after-incremental-source-build.md`. None of those files exists in the working tree.

## Reproduction

1. A worktree creates bug records that are committed only on a side branch.
2. The worktree directory is deleted. Its ACTIVE `worktree_registration` stays.
3. `librarian(action="merge_worktree", root=<worktree>)` with the default (no `abandon`).
4. `librarian(action="doctor")`: `missing_file` rises by the number of `reseated` entries in the merge result.

The dry run (`dry_run=true`) lists the same rows under `reseated` and gives no sign that the target paths do not exist.

## Environment

codescout `experiments` at `bec1075f5`. Shared catalog `~/.local/share/librarian/catalog.db`. Observed through the live MCP server after a rebuild.

## Root cause

`reseat_one` (`src/librarian/tools/merge_worktree.rs`) computes `main_path = main_root.join(rel)`, derives `id_m`, and checks only that no catalog row with `id_m` exists (the `reseat_collision` test). It then runs `artifact::upsert` and `graft::graft_rows`. It never tests `main_path.exists()`. Its doc comment assumes a worktree-born row is always committed to the main checkout. That assumption fails when the worktree's work lives on a branch that has not reached the main checkout.

Verified by reading `reseat_one`. Not verified: whether `merge_one` (the shadow-row path) has the same gap.

## Evidence

- The merge result listed 49 + 28 + 1 = 78 `reseated` entries. `missing_file` rose by exactly 78.
- Two sampled files exist in git history on `research/author-cargo-artifacts-20261031`, `research/checked-real-task-20261024` and `research/fixed-acceptance-population-20261101`, and not on `experiments`. Only two of the 78 were sampled.

## Hypotheses tried

None yet.

## Fix

`reseat_one` now tests `main_path.exists()` after the collision check and before the dry-run return. When no file exists it pushes a `reseat_target_missing` conflict (with `shadow`, `main_path` and a hint), leaves the row untouched and returns `Ok(None)`. The dry run reports the same conflict, so the operator sees the cost before choosing `merge` or `abandon`.

## Fix provenance

- **SHA:** `e3d6412baa0a26bf543ab815101457e40fb78993` (`experiments`)
- **patch-id:** `87babf5ee3d2a30f4d31a2051912f8a6e22ab92b`

## Tests added

`a_worktree_born_row_whose_main_path_has_no_file_is_left_untouched` (`src/librarian/tools/merge_worktree.rs`). It was red before the fix: the dry run listed the row under `reseated` with no conflict. It runs the dry run and the real run, and checks that the worktree row stays and no row is written at the main path. `new_worktree_artifact_reseats_to_main_path` now uses real files (`real_wt_ctx`), so it still proves that a reseat happens when the file exists. Gate: `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`.

Live check, 2026-11-04: not done. `merge_worktree` with `dry_run=true` on `615e` returned `has diverged from ... HEAD`, before `reseat_one` ran. The unit test is the only evidence that the new branch works.

## Workarounds

Leave the rows: they point where the files should be once the side branches merge. Or delete the rows with `doc(delete)`, which loses their events. Choose `abandon=true` instead of `merge` when the worktree's files were never merged, accepting the loss of its rows.

## Resume

Decide the direction above, then write a test: a worktree-born row whose main path is absent must not produce a `missing_file` after `merge_worktree`. Related: `bad49b228663df9a` (the stale registrations that led to these merges).

## References

- `src/librarian/tools/merge_worktree.rs` `reseat_one`
- `bad49b228663df9a`
