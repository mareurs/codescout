---
id: '83a000004102c832'
kind: research
status: draft
title: Codex Stage 3–4 review — stopping decision supported, causal claims narrower
---

# Codex Stage 3–4 review

**Valid:** dated 2026-09-25

Scope: `d643c001..805c2a83`, including the freeze correction, train_arm.py, phase1-local-trained.py, cross_rule_firing.py and saved training/gate results. Read-only code review plus offline numeric checks; no training, model inference, checkpoint reload or model gate rerun. The only tests executed were the tiny Stage 2 module to investigate its direct entry point, and an import-discovered run with one in-memory mutation.

## Finding — P3: direct execution skips the freeze tests

`tests/test_stage2_synthetic.py:109` invokes unittest.main before FreezeMenuGuard is defined. Direct execution exits 0 with 16 tests; import-based discovery reaches all 21. One in-memory mutation removed the under-target rejection: discovery ran 21 tests and produced four failures, zero errors (one mutation applied, zero survivors). This confirms the regression tests work when reached; it does not contradict the reported earlier 21-test pass.

Filed: docs/issues/2026-09-25-codex-freeze-tests-after-main.md (`5d4e9ab75d686fed`). Move the entry point after the definitions. No fix applied.

## Freeze correction

The prior bug's implementation now checks emitted positive rows before writing files and reconciles them with pre-emission items minus dropped positives. The five added tests exercise the helper; the prior issue explicitly discloses that the reconciliation assertion is not unit-tested. Source review agrees with that scope. Reused the recorded baseline hashes and earlier mutation evidence rather than repeat the full freeze.

## Offline checks of training results and thresholds

For each arm, saved val logits cover exactly the 521 frozen val IDs and cal logits cover exactly the 359 frozen cal IDs, without duplicates or rule/label mismatches. Independently recomputed threshold selection from stored logits and temperatures: all 14 precision and recall thresholds per arm agree, including F0.5 fallback selection.

Weighted validation BCE recomputed in Python double precision:
- L1-MBERT: 0.693829497866844; stored selected loss 0.693829494451607.
- L2-QWEN: 0.23077894932633453; stored selected loss 0.23077894889142944.

Differences are consistent with arithmetic precision, not a different epoch or scoring formula. Logs select the minimum recorded validation loss: epoch 4 for L1, epoch 1 for L2. This checks saved artifact consistency; checkpoint parity was not rerun.

Training and inference share encoding, marker states, head order and unit aggregation. Training's loss supervises only the labelled target/rule cell, matching the masking amendment. Calibration and thresholding similarly use only own-rule cells. The scorer uses the maximum over eligible units for each rule, which is a broader decision than the trained target-cell loss; the known calibration/generalisation limitation matters here.

## Gate assessment

The saved reports show L1 1/8 and L2 3/8, with zero errored rows; L1 span 0/2 and L2 span 2/2. Each gate JSONL contains 112 rows, and every saved verdict agrees with its saved probability and threshold. With the current deterministic serial gate order, the five clean-text blocks have the following numbers of probabilities >=0.5:
- L1: 8, 8, 7, 7, 7.
- L2: 3, 1, 3, 5, 3.

Thus simply replacing the tiny learned thresholds by 0.5 would still fail every clean text. This is an offline diagnostic, not proposed threshold tuning. The gate logs lack case/run IDs and are interpretable here through the current serial execution order plus the text report; future records should carry those identities directly.

The stopping decision is supported: neither arm passes the registered gate, so no C1/Score B evaluation is warranted under this registration. No new code finding in this pass overturns that decision.

## Narrow the causal conclusions

The cross-rule table sums to 2,614 firings over 6,773 cells (521 texts times 13 other heads). Its source counts firing on unknown cells, not false positives against adjudicated labels. Only the aggregate table is saved; other-head logits/decisions are not present in fold-logits.json, so I verified its arithmetic and inspected the producer but did not independently reproduce the 2,614 model outputs.

Missing diverse negatives is a plausible explanation supported by the training objective and cross-rule activations. It is not a measured causal ablation. Cross-rule activations within val also do not prove that distribution shift contributes nothing to gate failure. Preserve the distinction between a symptom and its causal attribution; a future registration can label a sample of unknown cells and test the intervention on training/validation data.

The claim that the L1 overfit check and pair alignment rule out an engineering cause is too broad. Memorising 32 examples demonstrates gradient flow and some capacity, while pair alignment checks labels against text. Neither excludes optimiser/hyperparameter interactions, full-data learning dynamics or every implementation issue. The supported result is that this L1 recipe and seed failed at this data volume, not that ModernBERT cannot learn these rules.

The registered stopping decision should remain intact. A subsequent attempt needs a new registration, adjudicated cross-rule negatives and validation that measures the all-head firing behaviour actually used by inference. Unknown cells must not silently become negatives.
