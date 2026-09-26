---
id: d318fb53bf1b8e7a
kind: research
status: draft
title: Codex Phase 1b Stage 1 review — reproduced results and a counterexample split gap
---

# Codex review of current Phase 1b state

**Valid:** dated 2026-09-26

Review baseline: `c69a99e4`, after `9dd84a05`. Scope: registered post-stop diagnostics, Stage 1 training recipe changes, six saved Stage 1 runs, result aggregation, Stage 2 draft, cue selection and counterexample miner. No model inference, training or expensive tests were rerun. Shared peer edits and staged memory files were left intact.

## Finding — P2: complete the counterexample split filter before Stage 2

`mine_counterexamples.py` compares candidates only with the old frozen folds. It does not compare new candidates with each other across folds. The source-group assignment does not enforce the separate eight-token content-overlap rule.

Actual --count-only execution reproduces every draft eligible count, including 20 held-out drops. Capturing its output pool without changing the code finds 399 distinct eligible paragraphs; 64 distinct cross-fold shared eight-word shingles touch 28 paragraphs. A hypothetical application of the exact registered RNG and caps, before the still-missing Codex clean-text filter, selects 197 candidates with 5 shared shingles touching 11 candidates. This is a prospective gap; no Phase 1b Stage 2 draw or contaminated training run is asserted.

One in-memory mutation disabled all miner shingle checks. The existing 8-test mining module remained green: one applied mutation, one survivor. It currently exercises cue choice and clean-text consistency, not the filtering pipeline.

Filed: docs/issues/2026-09-26-codex-counterexample-cross-fold-gap.md (`8c17b1dbcf88a097`). Final augmented-fold validation plus a deterministic collision policy and a new-versus-new regression are needed before mining/admission is relied on.

## Stage 1 results independently reproduced offline

Read each run's local fold-logits.json (paths recorded in the committed per-run reports) and recomputed AUC as the exact positive-versus-negative pair ranking statistic, with half credit for ties. All six pooled AUCs and every per-rule AUC agree with the committed diagnose outputs to floating-point precision:

| recipe | seed | pooled own-cell val AUC |
|---|---|---|
| s1-r1 | 20260935 | 0.9822576 |
| s1-r1 | 20260937 | 0.9630710 |
| s1-r1 | 20260940 | 0.9550987 |
| s1-r2 | 20260935 | 0.9754495 |
| s1-r2 | 20260937 | 0.9521662 |
| s1-r2 | 20260940 | 0.9664898 |

Both clear the registered learned threshold at all three seeds. Worst-seed difference is about 0.00293, below 0.005; choosing s1-r1 matches the registered tie rule. The result is about this recipe/data and these runs, with the broad 3-seed uncertainty already disclosed. It is not proof of reliable deployment or rule understanding.

The new recipe bundles lower learning rates, longer training/warmup, pair accumulation, separate clipping and a tokenisation correction; s1-r2 also adds feature LayerNorm. This experiment supports the bundle, not causal attribution to any single optimiser change. Training and diagnostic loading both select encoding/feature handling from the recipe.

The recorded lower-bound-temperature heads are indeed perfectly separated on their actual cal logits in these six runs (checked all 17 such run/head cases). Landing on a lower bound is not in general proof of perfect separation; here the stronger fact is independently observable from the logits.

Own-negative cal reporting is a useful correction to the known threshold-fold reporting issue. Cal still fitted temperatures, so it is not an untouched test set. Cross-rule firing remains a firing rate on unknown cells, not adjudicated false positives. Its reported 42–54% was inspected in the artifacts and producer but not recomputed by fresh inference.

## Diagnostics and interpretation

Recomputed from saved own-cell logits:
- phase-1 reference AUC: 0.9705570292;
- second phase-1 seed: 0.5253168288;
- within-rule permutation run: 0.4978485116.

Thus the numeric null passes its registered band, and the original recipe's second seed did not learn by the later definition. The surface-probe artifact reports 12 of 14 rules at AUC >=0.9 and control mean about 0.491; this pass inspected the fitting code and saved outputs without rerunning the probe.

**Narrow the null conclusion:** the preregistration says the split carries no link but the labels. A near-chance shuffled-label run only establishes that this run did not recover useful held-out label signal after that permutation. It does not certify the absence of every leakage path, particularly when the same recipe sometimes fails to learn real labels. The pass is a diagnostic, not a proof that the entire split is clean.

The claimed epoch-0 maximum is the maximum of running means logged every 200 rows. It is not an observation of every intermediate step; phrase the overshoot result at that sampling resolution.

## What is ready and what remains

Stage 1 is complete and its selection is supported. Its three s1-r1 checkpoints become Stage 2 baseline B. Stage 2 remains an unregistered draft; the operator decisions name NC as ship candidate and N as diagnostic, with three seeds and a 3-of-3 ship bar. The audit, Codex clean texts, final negative data and N/NC training are not shown as completed in the inspected artifacts.

The new comparison is more useful than the earlier old-recipe control: B and N now share the selected recipe, all seeds are counted, and common-menu comparison is specified. The miner split gap should be fixed before this next stage starts. Preserve Phase 1's failed gate result and do not interpret Stage 1's own-cell AUC as a new gate pass.
