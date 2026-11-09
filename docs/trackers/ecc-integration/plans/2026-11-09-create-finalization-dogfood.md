---
id: '31b7eabea0a3688c'
kind: plan
status: done
title: ECC create finalization dogfood — database failure atomicity
tags:
- ecc
- codex
- dogfood
- librarian
topic: ecc-integration
---

# Create finalization dogfood implementation plan

> For agentic workers: use superpowers:executing-plans for inline execution. Existing authorization continues the established dogfood/reproduce/fix/review/integrate workflow; no repair replay now.

**Goal:** Determine whether failures while creating the catalog row, slug, augmentation or worktree registration leave a partially created artifact; correct a reproduced contract violation and preserve exact replay inputs.

**Architecture:** Scope is src/librarian/tools/create.rs::call and its direct catalog/registration callees. Prefer one bounded catalog transaction before disk writes if actual failure probes establish that partial catalog state violates the atomic-create contract. Preserve the existing disk-error cleanup semantics and original error causes. No shared catalog fault injection.

**Tech stack:** Rust, rusqlite, in-memory test catalogs and private native MCP probes.

**Spec:** Existing doc(create) observed draft is historical; AugmentSpec documents atomic creation with augmentation. A new bounded observed draft/report will distinguish current behavior from intended requirements and executed evidence.

## Baseline and replay boundary

Frozen commit038fffb3447b55900e617a251b6472486259e07c/tree4a0f76c8e8a0068974c2ba95afc872c309a43435. Branch feat/ecc-create-finalization-20261109 reuses the owned verification worktree .worktrees/ecc-navigation-gate, preserving its untracked native-provenance artifacts. Main source file is clean at start. Native worktree inventory transport was unavailable; no pending creation or installation was started. Capture source blob IDs/hashes, neutral candidate brief, current skill versions/hashes and runtime/build identity before edits. Retain solution/evaluator data separately. This is a new dogfood case, not the deferred no-ECC replay or an effectiveness A/B.

## Approaches and decision

1. Inspect and fault-test only; ship no production change if no contract violation is reproduced.
2. If database-stage partial state is reproduced, wrap the existing catalog writes in a single transaction, include worktree admission before file creation and commit that phase before disk writes. This preserves the existing best-effort disk cleanup boundary.
3. Add bespoke cleanup after each database call: avoid unless transaction semantics cannot express the observed contract, because cleanup can itself fail and duplicates state bookkeeping.

The controller chooses approach1 first and approach2 only on observed RED evidence. No speculative refactor or wider catalog transaction migration.

## Review focus

- Slug failure after row insertion leaves no new row or directory; original SQLite cause survives.
- Augmentation insertion failure leaves no artifact/augmentation/file; unrelated artifact remains unchanged.
- Worktree registration failure leaves no created file or catalog artifact; successful retry persists registration.
- Normal augmented create persists row, slug, augmentation and file.
- Disk-write failure still attempts cleanup, preserves the original IO cause and warns about uncertain residual state.

## Task1 — Reproduce, correct and verify the finalization boundary

Files: modify/test src/librarian/tools/create.rs only if a defect is reproduced; durable plan/report/evidence under this ECC tracker.

- [x] Capture frozen baseline/replay inputs and run the existing create test module (48/0).
- [x] Add actual SQLite-trigger failure tests through create::call for slug, augmentation and registration; all3 observed RED on residual artifact count2 vs expected1; original48 remain green.
- [x] Record confirmed bug44160e1378b2e3f5 and AugmentSpec atomic-create contract; incidental older-runtime navigation mismatch tracked separately ase5016b19fb0f75da.
- [x] Implement the catalog transaction/moved admission; module51/0 and real private-MCP3 failure+retry+read controls passed.
- [x] Independent stronger-model review with actual applied call-site mutations and observed outcomes; close any observed survivors.
- [x] Full required gate on a frozen source tree, then authorized commit/integration with explicit pathspecs preserving peer index entries.
- [x] Record exact source/integration identities, patch-id, verification limits and future replay inputs; update indexes and archive only verified fixed bugs.


## Progress — 2026-11-09

Baseline48/0, RED48pass+3fail at actual residual-row assertions, GREEN51/0 after catalog transaction and moving registration before disk. Formatted candidate source SHA25668f4fdaa9030bd0bd78939ee7debb37c6dd5b30917db07e43278505fee27d3bd. Build/private executable snapshot and full gate run frozen in slot0; independent stronger-model reviewer uses isolated mutation work, no edits to candidate. No source commit or integration yet. Original disk-error cleanup semantics preserved. Candidate neutral brief and exact source/blob/skill hashes captured before tests/implementation.

Ruling: Reuse the owned verification worktree on a new branch at exact038fffb3 after native worktree inventory transport failed; preserve previous untracked provenance artifacts. This avoids modifying the shared checkout; retained source inputs identify the actual base.

Ruling: Use the user-requested durable ECC tracker plan/evidence as the execution ledger and retain provenance worktrees/evidence for future replay. Generic skill cleanup of scratch/ledger is superseded by that user requirement. One task has no cross-task interface conflicts.


Replay local ref pinned, exact pre-action host history retained privately and versioned provenance recorded; configured controller model/effort gpt-6.1-sol/high. Used skill-body snapshots match installed0.1.0 bytes. Current-source MCP3 failure+retry/read controls and full frozen gate passed. Review independently passed51-test baseline, observed drop-commit mutation KILLED, and continues its batch. Additional M7 challenges registration moved before the transaction for downstream linked-worktree slug/augmentation failures. No parent regression added before observing that candidate. Source remains frozen68f4fdaa pending review.


Review M7 actually SURVIVED51/0; original6 variants KILLED. Parent read M7-result.json and exact descriptor before strengthening the two slug/augmentation tests to loop false/true worktree contexts. Production prefix remains byte-identical. Final checks @bg_00000083 rerun exact M7 using mutation-probe.sh with with-slot acquisition INSIDE the isolated tree (no inherited lease across source roots), then module, final build and full gate. Old evidence remains immutable; final artifacts use final- names. Reviewer preserves a diagnostic and resolves the initial restored-control stale-artifact result separately. No further reviewer variants requested.

## Completion — 2026-11-09

Final source `a6febbcb5bada021e0384cfeda9ccd4e527e840c35c9639ed9d043389af27e49`. Initial review:7 mutations,6 KILLED/1 SURVIVED/0 inconclusive. The observed M7 survivor led to both slug/augmentation fixtures exercising regular and worktree contexts. Exact M7 then KILLED49/2; restored module51/0; production prefix unchanged. Full final gate and5 actual private-MCP failure/retry/read cases passed. See [final report](../reports/2026-11-09-create-finalization-dogfood.md) for limits and retained false controls.

Source `6377002c1111fc05c7fb708f68341c0f1293e271`; integrated on experiments `6b2befb1eb9344e7a37f1fe660ec7de501276028`; stable patch-id `ba636938caee459b5457133127e8d919038a6758`. [Identity checks](../evidence/2026-11-09-create-finalization-dogfood/integration.json) establish identical committed trees and preserved peer index blobs. Fixed bug archived as44160e1378b2e3f5 through the managed move. Historical progress entries above describe their recorded stages. No replay, push, release or shared-MCP rollout. The separate call_graph follow-upe5016b19fb0f75da was subsequently reproduced and corrected by the identity cycle at4776e44bd.
