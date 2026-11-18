---
id: d8be1a58471ad76d
kind: spec
status: done
title: Public-only uncertainty resolution after baseline oracle authoring
tags:
- effort-steering
- evaluation-integrity
---

# Public-only uncertainty resolution

## Intent and prior evidence

Find a generic, low-cost route from an incomplete acceptance contract to executable regression coverage. The closed baseline-first pair (report bf3e845faf0b862a) established valid negative results for both low and xhigh: each accepted a saved wrong repair and retained the qualified reference. Both independently left a behavior dimension unresolved with no test. This is an exposed development case, not a held-out low/high capability gap.

The operator delegated experiment choices and autonomous execution under a USD 200 series ceiling. This design is the controller's choice under that authority, not a new operator-selected implementation. No renewed design approval is required under that standing instruction.

## Approaches considered

1. Reject any unresolved contract immediately. Cheap and useful as an assurance limit, but it cannot supply missing coverage; abstention alone is not a repaired task.
2. Give one bounded low follow-up its own original contract and tests, require executable exploration of material uncertainty using only public baseline evidence, and freeze the revised output before comparison. Selected: it tests whether uncertainty can be resolved without candidate anchoring or private guidance, while reusing original onboarding and the qualified runtime.
3. Add a high-effort reviewer or repeat fresh low/xhigh authors. Deferred: prior xhigh missed the same gap, and another broad rerun would not isolate this missing workflow step.

## Mechanism and visibility

A fresh low author receives the original public issue/source/guidance, the exact previous low contract and test patch that were frozen before private/reference evaluation, and a generic follow-up instruction. It does not receive candidate patches, verdicts, private tests, reference patches, earlier diagnostic reports, xhigh outputs, or the missing example. Its writable native state is a new physical clone of the original source-only snapshot; no repeat onboarding LLM call.

Generic follow-up: inventory each unresolved/material assumption; execute a minimal public-baseline observation for it; distinguish observed behavior from intended behavior using documented interfaces, callers and existing tests; test relevant boundary classes implied by those interfaces rather than merely repeating the reported input. A justified preservation assertion must pass the baseline and remain outside the required bug change. A requirement assertion must be justified by the public report. If public evidence cannot select an expected behavior, retain explicit uncertainty and do not claim complete assurance. Do not turn all observations into mandatory invariants. No project-specific missing case is added to this generic instruction.

Retain the prior output protocol: only oracle_contract.json, a new baseline_oracle.rs and its one module-registration line; no production repair. Allow at most eight focused contrast tests. Both output validity and runtime validity are checked independently of functional outcomes. Old clauses may be refined, but every test belongs to exactly one supported requirement/preservation clause; unresolved clauses have no assertions. No new attestation schema or unverified boolean is used as evidence of actual exploration: inspect the saved tool transcript and executable observations.

## Fixed diagnostic measurement

One new low follow-up only, no new xhigh/control model run. Comparator is the frozen prior author output; extra time/seed input/stochastic draw are confounded, so improvement is descriptive development evidence, not a causal estimate of the prompt's effect.

After the output is frozen, execute its declared assertions unchanged on original baseline, the previously saved wrong proposal, and the qualified reference, using the reviewed offline evaluator. Success requires all cells valid, baseline-to-reference requirement distinction, all preservation assertions passing baseline and reference, at least one justified preservation failure on the wrong proposal, and retained reference. An unresolved final contract is not a successful complete contract; report abstention separately even if incidental tests discriminate. Malformed output, infra invalidity, or missing statuses are separate outcomes.

Requiring test names is not evidence of their semantics: review the actual bodies and public evidence after freeze. A private witness is only a host-side outcome control. No changes to frozen source, tests, selection or expectations after seeing that control.

## Cost, execution and stop rules

Known series USD 64.106761199999998654 plus original unresolved USD 4 hold; standing ceiling USD 200. New follow-up CLI cap USD 0.75, admission reservation USD 1.00 (not a guaranteed hard provider cap). Preserve final/unknown actual costs on failure. Charge original low USD 0.2914831 plus follow-up cost for cumulative oracle-author cost; compare descriptively with xhigh USD 1.4046817999999996. This is not complete repair-workflow cost.

Qualified author profile and runtime unchanged: 32 GiB / 4 CPU native cap; 24 GiB host admission and 16 GiB continuous floor. Offline profile: 8 GiB / 4 CPU, CARGO_BUILD_JOBS=1, 28 GiB host admission, 24 GiB continuous floor, network none. No silently relaxed guards, no peer process cleanup.

One author attempt; no automatic model retry, no additional prompt tuning on this case within this plan. A negative or unresolved result closes this development hypothesis; a valid positive result only admits a separately registered repair/transfer study on tasks selected without outcomes. Existing gate and onboarding/source receipts are reused after hash/clean-tree verification. New orchestration requires focused red-green tests and an independent review with applied mutations before paid admission. No merge/push, no staging foreign paths.
