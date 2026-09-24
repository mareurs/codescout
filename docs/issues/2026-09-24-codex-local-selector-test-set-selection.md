---
id: '9b128c319302592b'
kind: bug
status: fixed
title: 'Codex review: local classifier plan uses held-out T to select C1'
tags:
- cluster/unclassified
opened: 2026-09-24
owner: marius
severity: medium
---

# Codex review: local classifier plan uses held-out T to select C1

**Valid:** dated 2026-09-24

## Contradictory contract

At `a8835d06`, docs/evals/phase1-local-classifier-preregistration.md Stage 3 says model selection uses validation loss only and T is never read during selection. Stage 4 defines C1 as the best local arm by T any-fire rate. Following Stage 4 therefore consumes T for model selection before presenting it as the primary held-out evaluation.

## Consequence

This is a prospective protocol defect, not evidence of contamination in a trained run: Stage 2/3 results have not been established in this review. Existing zero-shot results are unaffected.

## Expected

Select the C1 backbone and its thresholds using validation data only, freeze that choice, then evaluate on T. Alternatively call the existing T a selection set and reserve another untouched final test. Preserve source-document/incident grouping across all folds, including correction pairs and synthetic variants.

## Verification

Both conflicting sections were read directly. No model calls, training or permutation experiment was run; this finding is about the written selection procedure, not measured inflation of a score.

## Fix

**Fixed in `c061be8b`** (patch-id `aeb4cc2d4b059afd53ade49b8c13a18553cba66a`), an amendment to `docs/evals/phase1-local-classifier-preregistration.md` committed before Stage 2 starts. C1's first-stage arm, and Score B's standalone arm, are now chosen on the **validation fold** (lowest any-fire rate at the recall-0.9 threshold, ties to lower validation loss). T is read only after every choice is fixed. The same amendment adopts this file's second expectation: folds are split by incident across train, validation and calibration, with the shingle filter also run across folds.

**Verified** by reading both sections again after the amendment: the Stage 3 promise and the amended Stage 4 procedure now agree. No run existed to re-score, since Stages 2–4 have not started.

**Class left `cluster/unclassified`, after looking.** The nearest class, `IC-11` (doc contradicted by code), requires a statement that was true when written and decayed; this contradiction was present from authoring, between two sections of one document. No other class claims a self-contradictory protocol.
