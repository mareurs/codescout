---
id: '2bac7e0a27fbc392'
kind: bug
status: open
title: diagnose_run reports own-negative firing on the fold its thresholds were chosen on
owners:
- marius
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
opened: 2026-09-25
severity: low
---

# diagnose_run reports own-negative firing on the fold its thresholds were chosen on

**Valid:** dated 2026-09-25

**Observed at:** `24426921`.

## Finding

`docs/evals/data/2026-09-24-rule-tell/phase1b/diagnose_run.py` reports `own_negatives_fired` per rule. It counts validation negatives whose calibrated probability reaches `thresholds.json`'s `precision_t`.

`train_arm.thresholds` (`stage3/train_arm.py:260`) chooses that threshold on the same validation fold: the smallest t with precision ≥ 0.9. On any rule whose validation pairs the model separates completely, the count is therefore set by the precision constraint, ⌊TP/9⌋ false positives, and not by the model. The field is named for a property of the model and measures the threshold rule.

## Evidence

Phase-1b Stage 1, seed 20260935, recipes s1-r1 and s1-r2 (two independently trained checkpoints):

- `own_negatives_fired` is identical on 13 of 14 rules, e.g. d_adjacency 2/26, question_asked 2/32, closed_population 1/15. Only d_visibility differs (1/15 against 0/15).
- The items that fire differ. Of the 13 rules where any negative fired in either run, the fired ids are different in 9, e.g. d_adjacency `train-d_adjacency-10179:neg` against `train-d_adjacency-9161:neg`. Recomputed offline from each run's `fold-logits.json`, `calibration.json` and `thresholds.json`.
- `thresholds.json`, d_adjacency, both runs: val_tp 26, val_fp 2. 26/28 = 0.929 meets the target; a third false positive, 26/29 = 0.897, does not.

## What is not affected

- Stage 1's registered verdict, pooled validation AUC, which uses no threshold.
- `other_rule_cells_fired`. Threshold selection reads only each rule's own cells, so other-rule cells did not enter it. The figure still depends on how permissive the chosen threshold is (d_adjacency `precision_t` 0.069 in s1-r1).

## Expected correction

Report own-negative firing on the calibration fold, which threshold selection never reads (`fold-logits.json` holds the calibration logits), or drop the field. The calibration fold did fit the per-rule temperature, so it is out of sample for the threshold but not for T. T is monotone per rule, so it leaves the ranking unchanged.

Not changed while the six Stage 1 runs are in flight, so all six are measured by one instrument. The replacement figure will be reported as post-hoc and unregistered.

## Scope

Introduced in `02511d99` by the registering session (session `571eb3d6`), and found by it while reading the first two Stage 1 diagnose outputs. Class: IC-24. The count is correct as "validation negatives at or above a threshold chosen on validation", and it is published under a name that reads as out-of-sample firing.
