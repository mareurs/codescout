---
id: '7fbedafeb8bf9985'
kind: research-note
status: archived
title: 'Historical: Contract-first ripgrep transfer diagnostic'
tags:
- effort-steering
- historical
- retained-worktree
topic: effort-steering
historical: true
original_rel_path: docs/research/2026-11-02-contract-transfer-ripgrep.md
source_body_sha256: c13a97a84024425fa86058e1702b80325470f7a90529e91b7be84af11a790fc8
source_snapshots:
- source: /home/marius/.codex/worktrees/615e/codescout/docs/research/2026-11-02-contract-transfer-ripgrep.md
  sha256: '34a03fba3c4904e727375391463598fd47beb5f0b9e0aa13c24354f52a647d7d'
  body_sha256: c13a97a84024425fa86058e1702b80325470f7a90529e91b7be84af11a790fc8
- source: /home/marius/.codex/worktrees/ca35/codescout/docs/research/2026-11-02-contract-transfer-ripgrep.md
  sha256: '34a03fba3c4904e727375391463598fd47beb5f0b9e0aa13c24354f52a647d7d'
  body_sha256: c13a97a84024425fa86058e1702b80325470f7a90529e91b7be84af11a790fc8
---

# Historical retained-worktree snapshot

This is archived evidence, not a current plan or launch permission. Start with the [experiment README](../../README.md). Original source paths, hashes and any body variants are recorded in [the manifest](../manifest.json).

Original logical path: docs/research/2026-11-02-contract-transfer-ripgrep.md

---


# Contract-first ripgrep transfer diagnostic

## Contract and selection

**Valid:** dated 2026-11-02

The [Axum topology prototype](2026-11-02-case-simplification.md) supplied a detector chosen after seeing an exposed diagnosis. This offline transfer asks whether a probe matrix can be fixed from another task's public contract before any solution or score inspection.

Three public problem statements were inspected: ripgrep2576, Tokio6603 and Ruff15309. Ripgrep was selected because its statement describes whole-word flag behavior and comparison with explicit regex boundaries. The original issue example is public task input; the original defect is therefore known. The registered agent patches and residual outcomes were uninspected when the matrix was frozen. This is an outcome-uninspected development transfer, not a guaranteed uncontaminated benchmark or low/xhigh comparison.

For controlled ASCII subjects and regex matches with word-character endpoints, compare exact output of --word-regexp with the same pattern wrapped in explicit boundaries. Independently require literal expected matches, so two equally wrong outputs cannot pass. Nine fixed cases cover the reported domain example, punctuation contexts, underscore/digit word characters and absent matches. Exit1 with empty output is a valid no-match, distinct from exit2 or timeout.

## Fixed roles and provenance

Roles were chosen before scores: dataset base, dataset gold, recorded Sonnet5.5 bare seed0, and recorded Sonnet5.5 cs seed0. No replacement seed/arm/task after outcomes. Run specs name model/arm/seed but do not establish effort; these are not labeled low/xhigh. Gold is an evaluation reference, not an input to the oracle generator. No new model call or repair.

Private root /home/marius/.cache/codescout-research/codescout/contract-transfer-ripgrep-20261102 retains public-input.json, registration.md, cases.json, freeze.json, runtime fingerprints, copied exact role patches, commands, stdin/output hashes and raw process receipts.

## Source preparation deviation

The first attempt used the existing SAB arm image. Its Git history was reinitialized and lacked the dataset base object. All four source setups were invalid before build or CLI evaluation; no behavioral verdict is inferred. All four containers were removed. The runner continued the unavailable archive across the four roles rather than classifying the first archive error as a stopping source-binding error; this deviation remains in attempt-1.

The bundle's original source image was locally available. Read-only preflight confirmed HEAD equals dataset base fed4fea217abbc502f2e823465de903c8f2b623d and base tree d624887feded5c4a5f609f345b29684afb714e23. The second attempt changes only the pinned image; public cases/oracles/role selection and copied patch bytes stay fixed. Images and runtime fingerprints are recorded separately. This is preparation correction before any CLI observation, not an outcome-driven test revision.

Every role archives the registered Git tree into a new source directory inside a disposable container, verifies the full initial index tree, applies only its registered patch, builds offline with a locked dependency file and a fresh target, and checks source tree identity before/after probes. No source implementation is read to choose a case. No untracked image tools/configuration are copied into the archive. All containers have networking disabled.

## Results

| Registered source role | Passing cases | Failing cases | Validity |
|---|---:|---:|---|
| Dataset base | 8 | 1 | Behavioral result |
| Dataset gold | 9 | 0 | Behavioral result |
| Recorded Sonnet5.5 bare seed0 | 9 | 0 | Behavioral result |
| Recorded Sonnet5.5 cs seed0 | 9 | 0 | Behavioral result |

There are 36 case-role evaluations and 72 real CLI processes: nine fixed subjects, two command forms, four source roles. Every CLI process exited0 or1 without stderr or timeout. Source trees, patch stdin hashes, image, binary identity and raw output hashes revalidate. All four second-attempt containers were removed.

The base fails only issue-domain-multiline: flag output is some.domain.com / ome.domain.com / some.domain.com on the three lines; explicit-boundary output is some.domain.com on all three. Gold and both agent patches produce the registered literal expected results in both command forms. Zero residual agent defects were detected by this matrix.

The four invalid first-attempt source setups are retained separately; they built no binary and executed no CLI case. No invalid setup is counted as a behavioral failure or silently removed.

## Interpretation

The public-contract matrix calibrates on another task: it detects the reported base bug and gold passes. It was fixed before opening solution code or scores. However, both registered agent solutions already satisfy every case, so this transfer adds no observed discovery of a residual agent error and no evidence that low quality improved.

The outcome is an acceptance ceiling for this matrix. Do not choose a replacement seed or add a new failing case after seeing it and call that a fresh test. The two old Sonnet artifacts are not a controlled low/xhigh cohort; their original costs and orchestration are not charged to a newly claimed zero-cost workflow.

The next useful experiment needs a finite, outcome-unseen task population fixed before new solutions: common independent acceptance, task-independent detector instructions and bounded variant generation, explicit unsupported/false-positive handling, and complete discovery/repair/escalation costs. Count all cases, including tasks with no observed residual defect. This transfer alone does not justify another paid retry on Axum or ripgrep.

## Validation and budget

Four focused oracle tests pass, including two equally wrong outputs, invalid-vs-no-match handling and partial-match failure. The runtime verifier recalculates all case grades from raw receipts and checks frozen contract/code/patch identities. A separate audit rechecks original Git base tree, full before/after source trees, patch application, binary receipt, all 72 CLI process streams, cleanup and the four initial invalid setups. Raw source setup failures are retained separately from behavior denominators. Current model-client estimate stays24.269845199999999810 of40; remaining15.730154800000000190. No new model call or credential read; CPU/native orchestration dollars unmeasured. Existing project gate limitation remains as recorded in the witness report; runtime/test source in the repo is unchanged.
