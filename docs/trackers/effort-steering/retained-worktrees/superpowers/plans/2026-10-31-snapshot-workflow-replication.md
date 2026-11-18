---
id: c319f664f59a4621
kind: research-note
status: archived
title: 'Historical: 2026-10-31-snapshot-workflow-replication'
tags:
- effort-steering
- historical
- retained-worktree
topic: effort-steering
historical: true
original_rel_path: docs/superpowers/plans/2026-10-31-snapshot-workflow-replication.md
source_body_sha256: f7f1d7d0d3070d7540f810a59686f749dd6735af7341095b7680b6d50620e833
source_snapshots:
- source: /home/marius/.codex/worktrees/615e/codescout/docs/superpowers/plans/2026-10-31-snapshot-workflow-replication.md
  sha256: f7f1d7d0d3070d7540f810a59686f749dd6735af7341095b7680b6d50620e833
  body_sha256: f7f1d7d0d3070d7540f810a59686f749dd6735af7341095b7680b6d50620e833
- source: /home/marius/.codex/worktrees/ca35/codescout/docs/superpowers/plans/2026-10-31-snapshot-workflow-replication.md
  sha256: f7f1d7d0d3070d7540f810a59686f749dd6735af7341095b7680b6d50620e833
  body_sha256: f7f1d7d0d3070d7540f810a59686f749dd6735af7341095b7680b6d50620e833
---

# Historical retained-worktree snapshot

This is archived evidence, not a current plan or launch permission. Start with the [experiment README](../../../README.md). Original source paths, hashes and any body variants are recorded in [the manifest](../../manifest.json).

Original logical path: docs/superpowers/plans/2026-10-31-snapshot-workflow-replication.md

---

# Snapshot-visible workflow replication implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline. Steps use checkbox syntax for tracking.

**Goal:** Obtain a valid, cost-accounted low/xhigh diagnostic comparison with equal snapshot-visible checking workflows.

**Architecture:** Add a thin private phase wrapper over the existing validated controller and offline containers. Bind it to the committed author-artifact adapter and fresh registration; preserve all old sources and records.

**Tech Stack:** Existing Python standard library, Docker, Git, frozen Claude CLI; no new dependency.

**Spec:** docs/superpowers/specs/2026-10-31-snapshot-workflow-replication-design.md

## Global Constraints

- New private phase snapshot-workflow-20261031, new sessions and /tmp/sw31-author- roots; no edits to sealed prior phases.
- Tasks Tokio4384 and Ruff15394; fixed order D-L,D-R,C-L,C-R,B-L,B-R,A-L,A-R.
- low ceilings L.80/R1.00; xhigh L1.25/R1.75;9.60allowances+.40reserve <=10phase and40cumulative.
- Prior anchor19.704714499999999785; no token value in logs or registration; no shared auth change.
- Same task/source/checker/tools/artifact access; repair only from its own reviewed failed initial at the same effort.
- Default initial task prompt unchanged; full own checker log for repair; no cross-arm source or best-patch choice.
- Invalid completion/cost/source/evaluator/effort/session stops payment; no retry.
- Known-task replication, not held-out generalization or isolated artifact-effect proof.
- Single fresh strongest prepayment review with applied mutations, one Important/Critical fix pass, no re-review. Standing user autonomy supplies execution and routine choices.

## Review Focus

1. A new author Cargo subprocess uses the new artifact adapter and phase-local offline container, rather than silently loading the old sealed adapter.
2. A repair presented with another unit's source, effort or checker log is refused before any model call.
3. Actual invalid or slightly over-ceiling charges remain in accounting and prevent a misleading cheap valid comparison.
4. A passing initial is skipped only in its own repair slot; a failed or unreviewed initial cannot skip or advance.
5. A changed prior anchor, frozen source, registered prompt or task/control population blocks a launch.

## Task 1: Register and validate the phase

**Files:** new private .codescout/resident-private/snapshot-workflow-20261031/{workflow.py,adapter_cli.py,prepare.py,freeze.py,test_protocol.py,progress.md}; public design/plan above.

**Interfaces:** workflow.e retains inherited worker/frozen/admission/grade/state contracts. New wrapper setup(label) creates unique roots, admit(states,label) enforces the10phase budget/order, totals(states) uses whole-workflow cost. prepare produces fresh source-bound controls; freeze registers immutable runtime/input/receipt hashes and new setup files exactly once.

- [x] Write consuming tests for all five Review Focus cases; run and observe concrete contract REDs before fixes.
- [x] Implement the thin wrapper with new adapter/root/order/budget bindings; keep original issue text and helper behavior.
- [x] Run focused tests, fresh BASE/GOLD controls and the actual author-artifact smoke; inspect all outputs and denominators.
- [x] One fresh strongest review: actually apply at least6load-bearing mutations, report observed killed/survived/invalid counts, retain diffs/logs. Grade findings and apply one necessary fix pass with RED/GREEN; replay.
- [ ] Write immutable registration with actual cost anchor, transparent prior exposure, runtime/input hashes, limits/order/decision and prepayment evidence; verify first admission without a model call.

## Task 2: Execute and close

**Consumes:** immutable admitted phase from Task1; no registration amendments after the first paid call.

- [ ] Run the exact admitted slots one at a time, charging every started call; do not retry invalid calls. Observe effort/session/prompt/source binding.
- [ ] For each valid completed call, independently read its complete diff and actual checker output, record the source review, then advance or take its own legal skip. Invalid outcomes stop payment and remain in totals.
- [ ] Recompute whole initial+repair costs and quality from complete raw population; separately classify artifact consumption and repair effects.
- [ ] Publish the bounded conclusion, append the mechanism tracker via catalog allocation, verify old seals, run required project verification and commit exact owned public files with normal hooks. Record final Git/evidence receipt externally, no merge/push.

## Execution ledger

Pre-flight: Task2 consumes Task1's immutable registration and source-bound observations; no model call is permitted before that gate. Ruling: retain the existing managed worktree and current committed adapter, because no production source change is intended; new private phase and author roots provide experiment isolation. Ruling: preserve all earlier results and count these tasks as exposed diagnostic replications. Docker cleanup runs separately with archives and exact-resource checks; it does not alter experiment inputs.

This prospective plan is frozen before the first model call. After registration, execution status belongs in the private progress ledger and new final report; do not edit registered plan bytes to tick later outcomes.

Preparation observed actual protocol/worker REDs for old adapter/root/admission-budget bindings, then GREEN. The actual separate-process Cargo snapshot regression also failed with the old shim returning no artifact, then passed with the committed fix. Fresh controls: Ruff BASE8/9,GOLD9/9; Tokio BASE intendedtraitcompileE0277 with separate116library compatibility, GOLD118/118. The single fresh strongest review applied8mutations:4killed,4survived,0invalid. Parent read the survivor diffs/logs first and added4consuming regression cases. Exact-mutant replay now kills8/8, with0survivors/invalid;23tests pass including the real Docker subprocess case. Existing guard implementations were unchanged. Registration is the next unpaid step; no new model-client call has been made.

Docker cleanup complete:15unused backend images older14days and2exited closed-review containers removed after1624files archived. All26previously running services and all volumes retained. Docker-reported stored image size336.8GB→331.4GB and container writable size4.182GB→2.302GB; these are before/after daemon reports, not sums of virtual image sizes.
