---
id: '078c6805e2370f52'
kind: research
status: draft
title: Codex review — synthetic generation and audit admission, 25 September
---

# Codex review — synthetic generation and audit admission

**Valid:** dated 2026-09-25

Reviewed through `71125e36`, including `d272ed84`. Read the generation, construction-validation and audit implementations, controlling amendments, stored pairs and audit decisions. No generation, model calls, training or test-suite reruns. No production files changed.

## Confirmed cheaply

- Training-pool construction: 939/1760 accepted; fold counts train 645, validation 173, calibration 121. T-syn-in 195/330; T-syn-cross 330/330. Pair IDs unique within each set.
- Audit: 512 unique pair IDs, zero invalid answers. Recomputing each cell's disagreement count and strict >20% drop decision from audit.jsonl produced zero mismatches against decisions.json.
- Training-side dropped cells: contradiction, question_asked, scope_instant, selector_narrow. The implementation pools source decisions only from training-side audits; test audits do not directly decide training admission.
- Retained synthetic train positives: 523. Admitted mined train positives after T-context overlap filtering: 59. Combined: 582 across rules; none reaches 50. Largest: d_fixture 43, count_unit 41, d_semicolon 41. These are pre-freeze ceilings; the final cross-fold filter has not been applied.
- Measurement correction: the first local recount excluded only not-a-violation, accidentally including not-a-pair and unsure as positives. Recomputed using membership in the actual 22-rule menu; only the corrected 59 mined / 582 combined figures above are valid. No campaign result used that local erroneous count.

## Review findings and decisions

### Resolve known individual disagreements before data freeze

The registered decisions operate at generator and generator/side/rule cell level. Eight audited pairs marked disagree nevertheless belong to retained training-side cells: four train, two validation, two calibration. This follows the written threshold, so it is not an implementation deviation.

Three train-fold examples directly dispute supervised targets: d_loudness pair train-d_loudness-5382 has a=no; lines_read train-lines_read-43 and member_vs_population train-member_vs_population-7970 have b=no. A fourth train example, d_history, fails only c. Other cells also contain target disputes in validation/calibration. An auditor verdict is not ground truth, but these pairs should not silently become accepted labels merely because their cell has 1/8 disagreements. Register an item-level disposition: adjudicate or quarantine known disputed pairs, while preserving the original sample and source-level acceptance measurement. Do not relabel them from this review.

### The contradiction audit diagnoses the wrong property

The campaign itself correctly records the relational-rule problem: 7/8 contradiction pairs pass a and b but fail c because another sentence participates in the contradiction. A corrected audit must test that replacing the designated target removes the contradiction in context, and distinguish a separate independent defect from the other member of the same relation. Re-audit under a new declared question; do not silently restore dropped pairs. This is already disclosed by the campaign, not a newly discovered bug.

### A top-up is a new volume experiment, not yet a remedy

The count-based stopping decision is correct. A top-up sized only to reach 50 before the final held-out/cross-fold filter can still fall short after freeze. Size per rule from train-fold survivors after construction, cell admission and item-level adjudication, allowing for the final filtering. Do not use T-syn quality to revise training prompts or choose which training arm to pursue. Prompt revisions and audit revisions need separately named versions.

## What is not established

The 330/330 cross-generator construction pass is genuine in the saved data, but not a controlled model-only advantage: Codex had tool access and the generation run predates per-call event logs. Later audit logs do not reconstruct earlier generation behavior. Future comparisons should match tool access or keep the conclusion at the generation-and-audit-pipeline level.

No final frozen dataset or trained model was reviewed. No independent relabeling or model-based re-scoring was performed. The appropriate disposition is to keep Stage 3 stopped and settle item-level admission and relational audit semantics before spending on another generation round.
