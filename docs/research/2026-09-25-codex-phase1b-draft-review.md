---
id: '07c07a1477cc512b'
kind: research
status: draft
title: Codex Phase 1b draft review — audit context identity and causal comparison
---

# Codex Phase 1b draft review

**Valid:** dated 2026-09-25

Committed baseline: `9dd84a05`. New work reviewed: untracked, explicitly DRAFT `docs/evals/phase1b-local-classifier-preregistration.md` and `docs/evals/data/2026-09-24-rule-tell/phase1b/audit-instruction.md`. No Phase 1b execution scripts or results existed in that directory at inspection. Findings below concern the draft contract, not a shipped implementation. No model calls or tests rerun.

## Previous findings addressed

`f0125e0e` moves unittest.main after the new FreezeMenuGuard class. Source diff resolves the direct-entry ordering bug; the issue records direct and discovery runs with 21 tests each, which this pass did not repeat. The same commit explicitly narrows the L1 engineering-cause claim and the L2 cross-rule causal attribution. The old experimental outcome remains unchanged.

## P2 — audit key discards context required by the audit instrument

Step 1 samples distinct `(unit text, text rule)` pairs, then shows each sampled unit in its full paragraph. The instruction explicitly permits the paragraph to determine whether the unit violates a rule. A single deduplicated key can therefore name multiple different adjudication inputs, while the draft does not specify which paragraph is shown or how all contexts are covered.

Offline census using the actual segmenter over the frozen train/val/cal files reproduces 13,753 unit instances and 8,303 distinct keys. **5,422 keys occur in multiple distinct full-paragraph texts, covering 10,851 instances.** None of these keys crosses folds. The count establishes omitted context variation, not that 5,422 labels are wrong. Most multiplicity comes from the intended original/fixed twins, which is precisely where context can change.

Before drawing the sample, define the adjudication unit using the context actually shown: for example `(fold, row_id, unit_index)`, or a canonical full-context-plus-unit key. Alternatively, show every context for a deduplicated key and explicitly adjudicate that union. Keep context identity in returned labels and masking. Do not silently pick one representative paragraph and describe the verdict as covering all its contexts.

## P2 — unique-key audit bound and admitted-instance population differ

The Wilson admission bound is estimated on uniformly sampled distinct keys, while training and calibration consume every admitted cross-cell instance. Multiplicity distribution is 2,860 keys occurring once, 5,438 twice, three occurring three times, and two occurring four times. A bound for uniformly sampled keys is not automatically a bound for this instance-weighted population.

This is a measurement-frame issue, not evidence the unseen prevalence exceeds 5%. State which population the 5% promise covers. Sampling the actual contextual cell population makes that contract direct; retaining cluster sampling requires an explicit population estimate and uncertainty treatment rather than reusing an ordinary binomial bound as though keys were cells.

## Causal comparison needs a fixed reporting surface

The diagnostic D is useful: it applies the new calibration/threshold procedure to the old checkpoint. However Step 4 can prune heads separately for D and L2-1b. Comparing only their final gate pass/fail can then compare different menus. Keep per-head results before pruning and report a common-menu comparison, alongside each arm's operational final-menu result. Otherwise a pass may partly reflect reduced coverage rather than the claimed training improvement.

The final Known limits sentence says D controls the threshold half but not calibration, while D is explicitly taken through Step 4, which refits both. Clarify that D controls the combined post-training procedure on old weights; it does not disentangle calibration from threshold changes or their interaction with training.

## Assessment

The direction addresses the earlier failure more directly: audit unknowns, retain masking for uncertain cells, evaluate cross-rule firing, and include an old-checkpoint diagnostic. The two audit-population issues should be resolved before registration and sampling. This review does not claim fresh negative labels exist, approve blanket unknown-to-zero conversion, or infer a new model result. T and T-syn were not read in this review.
