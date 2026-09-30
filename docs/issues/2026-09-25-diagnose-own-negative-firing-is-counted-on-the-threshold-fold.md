---
id: '2bac7e0a27fbc392'
kind: bug
status: mitigated
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


## Measurement (2026-09-30)

`phase1b/measure_own_negative_firing.py`, offline from the six Stage 1 runs' `fold-logits.json`, `calibration.json` and `thresholds.json`. **Control:** its validation-fold count equals the committed `own_negatives_fired` on all 84 run-rule cells; the calibration-fold pooled counts it produces (21, 15, 9, 12, 13, 13 of 180) equal the `cal own neg` column of the committed `stage1/summary.txt`, which `stage1_summary.py` computes separately.

- **The count is a function of the threshold rule, and the finding above understates how far.** `own_negatives_fired` equals `thresholds.json`'s `val_fp` in 84 of 84 run-rule cells, and `val_fp` equals ⌊`val_tp`/9⌋ in 84 of 84, not only on completely separated rules. The field carries nothing `thresholds.json` does not already hold.
- **The 13-of-14 figure is one seed's.** Counts equal between s1-r1 and s1-r2: 13/14 rules (seed 20260935), 11/14 (20260937), 12/14 (20260940); fired ids differ on 9, 8 and 9 of 13 rules with any fire.
- **Pooled, the two folds behave differently across the same six models.** Validation: 16-19 of 260 in every run. Calibration: 9-21 of 180. Across seeds of one recipe (s1-r1) the calibration count spans 12; s1-r1 minus s1-r2 within a seed is +9, +2, -4. So seed moves the calibration figure more than recipe does, and six runs cannot separate model signal from training noise there.
- **The calibration fold is disjoint from validation** (0 ids in common) and small per rule: 4-23 negatives, so a per-rule count is 0-8 events. Pooled it is usable; per rule it is not a ranking.

**Where the honest figure already lives:** `stage1_summary.py` reports the calibration-fold figure, and the phase-1b preregistration's Stage 2 section describes `step4.py` as measuring calibration-fold firing on each head's own frozen negatives. `own_negatives_fired` is written by `diagnose_run.py` and read by no script; the preregistration's Stage 1 results already say it "is not reported", and the Stage 2 section says `diagnose_run.py` "stays as it is"; the Stage 1 section lists only AUC and cross-rule firing as what it is used for.


## Decision (2026-09-30)

**Operator ruling: option 4, leave `diagnose_run.py` as it is.** Status `mitigated`, not `fixed`: the misnamed field is still written into `stage1/*.json` and the script is unchanged, so the defect stands and is contained. Containment is that nothing reads the field, the preregistration says it is not reported, and the calibration-fold figure is the one `stage1_summary.py` and `step4.py` report. A reader of a `stage1/*.json` should take `own_negatives_fired` as `thresholds.json`'s `val_fp` (see Measurement) and not as out-of-sample firing.

Reopen if a script starts reading the field. Not done, and why: moving it to the calibration fold or renaming it edits an instrument the Stage 2 registration says stays as it is, for a field no consumer reads; dropping it loses nothing but changes the same bytes.

## What is not affected

- Stage 1's registered verdict, pooled validation AUC, which uses no threshold.
- `other_rule_cells_fired`. Threshold selection reads only each rule's own cells, so other-rule cells did not enter it. The figure still depends on how permissive the chosen threshold is (d_adjacency `precision_t` 0.069 in s1-r1).

## Expected correction

Report own-negative firing on the calibration fold, which threshold selection never reads (`fold-logits.json` holds the calibration logits), or drop the field. The calibration fold did fit the per-rule temperature, so it is out of sample for the threshold but not for T. T is monotone per rule, so it leaves the ranking unchanged.

Not changed while the six Stage 1 runs are in flight, so all six are measured by one instrument. The replacement figure will be reported as post-hoc and unregistered.

## Scope

Introduced in `02511d99` by the registering session (session `571eb3d6`), and found by it while reading the first two Stage 1 diagnose outputs. Class: IC-24. The count is correct as "validation negatives at or above a threshold chosen on validation", and it is published under a name that reads as out-of-sample firing.
