---
id: 44160e1378b2e3f5
kind: bug
status: fixed
title: doc(create) leaves partial artifacts when catalog finalization fails
tags:
- librarian
- ecc-dogfood
- cluster/unclassified
closed: 2026-11-09
opened: 2026-11-09
owner: marius
severity: medium
---

# BUG: doc(create) leaves partial artifacts when catalog finalization fails

## Summary

A catalog error during slug minting, augmentation insertion or worktree registration returns an error after earlier create stages have already persisted. The disk-error cleanup branch does not cover these catalog stages.

## Class fit

This is a multi-stage persistence atomicity failure: the operation stops after a prior durable mutation without rolling it back. Unlike the earlier disk-write diagnostic defect, the failure is in the catalog phase before disk cleanup is entered. The IC-1..IC-24 roster was read; none names this catalog transaction atomicity defect without stretching its claim. The sanctioned unclassified tag is retained.

## Symptom (Effect)

On frozen038fffb3447b55900e617a251b6472486259e07c, three independent SQLite-trigger probes reach the intended database failure and leave two artifact rows where only the pre-existing keeper should remain. This breaks the atomic creation-with-augmentation contract documented on AugmentSpec and produces partial state on a failed create. Exact disk and registration residuals are separately inspected/probed; no stronger runtime claim is inferred from the row assertion alone.

## Reproduction

In an isolated in-memory test catalog, create an unrelated keeper. Inject BEFORE UPDATE OF slug ON artifact, BEFORE INSERT ON artifact_augmentation, or BEFORE INSERT ON worktree_registration with RAISE(ABORT). Invoke real create::call with a fresh path and augmentation. Require the actual SQLite failure marker, then inspect durable rows and filesystem. The three new finalization tests fail at the artifact-count assertion: actual2, expected1. Existing48 tests still pass.

## Environment

Owned branch feat/ecc-create-finalization-20261109 in .worktrees/ecc-navigation-gate. Base038fffb3447b55900e617a251b6472486259e07c; source clean before tests, baseline48/0. Private test catalogs only; shared MCP build differs and is not the current-source runtime oracle.

## Root cause

src/librarian/tools/create.rs::call writes artifact/slug, then augmentation via separate catalog calls without a transaction spanning them. ensure_registration is called after file writing. Only filesystem-write errors enter undo_failed_create. The sibling worktree::resolve_write_target already uses a single unchecked_transaction around linked catalog effects.

## Evidence

[ECC plan](../../trackers/ecc-integration/plans/2026-11-09-create-finalization-dogfood.md); baseline/red logs, source/test patch and exact base inputs under docs/trackers/ecc-integration/evidence/2026-11-09-create-finalization-dogfood/. Tests exercise real create::call, SQLite and filesystem, not a mocked result.

## Hypotheses tried

Three catalog-stage failures each reproduced a leftover artifact row. No production fix applied at this recording.

## Fix

Artifact/slug/augmentation/worktree registration now share one catalog transaction, committed before directory/file creation. Existing best-effort disk cleanup and original IO cause remain unchanged; no cross-store crash-atomicity guarantee is added.

Source `6377002c1111fc05c7fb708f68341c0f1293e271`; integrated locally on experiments as `6b2befb1eb9344e7a37f1fe660ec7de501276028`; stable patch-id `ba636938caee459b5457133127e8d919038a6758`. Committed trees are identical to each other and the gated candidate. [Integration binding](../../trackers/ecc-integration/evidence/2026-11-09-create-finalization-dogfood/integration.json).

Final module51/0; actual private MCP5 failure/retry/read controls passed; full gate passed (lean6316/default9208/Python1764; skips/limits in report). Review applied7 initial mutations:6 killed,1 survived. After observing M7, both slug/augmentation fixtures were expanded to regular/worktree contexts; exact M7 was killed49/2 and restored module passed51/0. Production prefix unchanged; initial and later populations remain separate.

## Fix provenance

- **SHA:** `6b2befb1eb9344e7a37f1fe660ec7de501276028` (`experiments`)
- **patch-id:** `ba636938caee459b5457133127e8d919038a6758`

Reviewed source commit `6377002c1111fc05c7fb708f68341c0f1293e271` has the same stable patch-id and committed tree.

## Tests added

failed_slug_finalization_leaves_no_partial_create; failed_augmentation_finalization_leaves_no_partial_create; failed_registration_finalization_leaves_no_partial_create. All three first failed on actual residual rows after the intended SQLite cause. Final fixtures check no residual row/augmentation/registration/directory, preserve an unrelated artifact and retry successfully after removing only the fault trigger. Slug and augmentation each exercise regular/worktree contexts, closing observed M7 survivor (bug-fix-session-log:F-223).

## Workarounds

Inspect and remove a failed-create residual before trusting catalog/file consistency. Do not inject failures into the shared catalog.

## Resume

Resolved and integrated locally. [Final report](../../trackers/ecc-integration/reports/2026-11-09-create-finalization-dogfood.md) retains baseline, RED/GREEN, applied mutations, exact integration and future replay provenance. No no-ECC repair replay, push, release or shared-server rollout.
