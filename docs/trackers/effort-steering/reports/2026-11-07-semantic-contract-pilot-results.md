---
id: ce4dbca3d65db946
kind: report
status: done
title: 'Semantic-contract pilot: six valid saved-candidate grades, no incremental acceptance'
tags:
- effort-steering
- evaluation
- negative-result
---

# Semantic-contract pilot results

## Decision

NO-GO for extending the tested low-contract protocol to held-out evaluation as an improvement over low-control. All six saved-candidate grades are valid, but the protocol wins zero additional tasks and loses zero: each arm accepts rust-analyzer22992 and rejects Nushell18863. Low-contract costs more than low-control for the same observed acceptance.

This rejects the pilot's improvement gate, not the general possibility that a different mechanism can help low. Two exposed development tasks with one attempt per arm cannot establish general equivalence between low and xhigh.

## Mechanism and comparison

All arms shared qualified Codescout/onboarding, instructions, plugin and public workflow. The low-contract intervention additionally required tracing the actual producer and representation, identifying the reported failure and nearest legitimate behavior, running discriminating regression checks before production edits, then fixing and reverifying. Low-control had no added protocol; xhigh-control provided the effort reference. Exactly the six previously completed candidates were privately evaluated once in their original fixed order, with no new model call or candidate replacement.

| Task | Low-control | Low-contract | Xhigh-control |
|---|---|---|---|
| Nushell18863 | Reject | Reject | Reject |
| rust-analyzer22992 | Accept | Accept | Accept |
| Accepted tasks | 1/2 | 1/2 | 1/2 |
| Sum of repair client costs | $0.5965270 | $0.8705975 | $2.0991132 |

The low-contract repair-cost premium over low-control is approximately46%; it is approximately59% below xhigh, but low-control already achieved the same observed acceptance at lower cost. These are reconciled repair client costs, not invoice totals or a fully amortized onboarding/author workflow claim. Six repair costs sum to$3.5662377. Known tracked-series subtotal remains$61.7467691; no model/provider call was added by qualification, grading or this audit.

## What failed

All three Nushell candidates pass both registered F2P tests and8619of8620selected P2P tests. The same P2P fails in all three: mixed_str_length_result_does_replace_last. Thus the common failure is preservation of legitimate mixed-input behavior, not inability to satisfy the reported bug's two F2P tests. No causal account of why each actor missed that distinction is claimed before transcript/source comparison.

The low-control raw Nushell parse initially misses one different passing P2P status. The declared v2 audit recovers exactly that key, leaving adjusted_missing0 and the real mixed-string failure unchanged. Low-contract and xhigh have raw_missing0. None is rejected merely for a parser omission.

All rust-analyzer candidates pass the registered1F2P and8181P2P tests.


Visibility correction, confirmed during the user's follow-up analysis: mixed_str_length_result_does_replace_last is a private test injected by record.test_patch, absent from the actor-visible baseline test file. P2P here means passing at baseline, not already present in the actor's checkout. The failed test coherently probes preservation of baseline mixed-result behavior, but this result does not show actors ignoring a visible test. Earlier user-facing wording calling it an existing test was imprecise; issue docs/issues/2026-11-07-p2p-test-visibility-misreported.md records the correction. Raw grades/costs/paired outcomes do not change.

## Qualification and closed audit

Native qualification job08 closed0 at11:13:46UTC. Both negative controls discriminate and gold/repeat selected-status maps match for both tasks. Continuation job0a closed0 at11:54:05UTC after the closed-control audit, immutable saved-candidate registration and six native grades.

A separate offline audit reopens all twelve cells (six controls plus six saved candidates), recomputes adjudication from raw logs and exact task records, checks all188saved-manifest bindings and candidate hashes, compares receipts, inspects every recorded cgroup/resource sample and independently queries fresh daemon absence for the twelve owned containers. All twelve cells are valid; every sample has zero oom_kill and meets the registered resource floors. The sampling is not continuous historical telemetry.

The audit's control overlap is calibrated against the previous closed-control audit on pass/F2P/P2P/missing/recovered/sample/gap/minimum/wall-time fields. It creates a derived artifact only: no native regrade, provider call or historical result rewrite.

Evidence root: /home/marius/.cache/codescout-resident/codescout/semantic-contract-policy-v2-20261107. Primary files: saved-closed-audit.json, saved-closed-audit.log, closed-audit.json, saved-grades/results.json, saved-candidates-registration.json and continuation-closed.json. Native records: /tmp/codescout-bg/2674124-d9ea3495-bg_00000008.json and /tmp/codescout-bg/2674124-d9ea3495-bg_0000000a.json.


Durable derived audit: [2026-11-07-semantic-contract-pilot-audit.json](../../../research/data/2026-11-07-semantic-contract-pilot-audit.json), copied byte-for-byte from the closed resident audit. It preserves all twelve row summaries, exact candidate/source bindings, paired decision and closed evidence hashes.

## Policy and source limits

Policy semantic-contract-v2 was amended after model execution but before any private candidate outcome was read. This is an amended development pilot, not strict original preregistration. Original frozen sources/registrations/costs/candidates/receipts remain unchanged.

The exact tested source is an explicitly UNCOMMITTED working-tree snapshot over base867c680924cb75575bec7ccd7068b8dfd7fd5889. The base SHA is not the corrected source commit. Six-path patch SHA25602fe093d2bce948120d2abf8ec0f8cc05488692e5ecfeb10bc8c9848c3d1d312, control registration3f3e08f2a698a11c5306dd352b7bf3eb5a35a0905f372ea652512e8833e9023b, saved manifestcc477a2331dd5c1be33ea06972c62baba9bcca1436bd7cda97d75a6eaf7600e9. Local commit remains blocked by the separately filed telemetry/ownership issue4b4fac5682a4303b; no hook bypass, merge or push.

Relevant policy suites263tests/129subtests and full source-bound gate passed before qualification. New saved driver passed25tests/19subtests and killed all3actuallyapplied post-fix driver mutants. General gate Python1642tests/529subtests is a separate root-suite population and does not include setup-ab-eval. Full gate exclusions and review limits remain recorded in the policy plan and source evidence.

## Next step

Close this comparison as a negative result. Do not launch another identical cohort or count the cheaper low arms as evidence that the added protocol works. The next useful question is offline: compare all three Nushell patches, task instructions and actual public checks to identify the shared missing legitimate-behavior distinction, then decide whether a different generic verification mechanism deserves a new registered experiment. No new paid run is admitted by this report.


Offline diagnosis is now closed: [semantic-contract-diagnosis](2026-11-07-semantic-contract-diagnosis.md), reportbfb2a49230efdf94. All six actual request configurations and process traces inspected; treatment delivery is confirmed but compliance partial. NUcontract performs real pre-edit regression testing yet selects all-error plus wholly successful cases, not mixed success/error; RAcontract explicitly omits its second legitimate-case test. Xhigh does extra semantic probing but misses the same NU partition. This supports a case-selection/verification-boundary diagnosis on this pilot, not generic effort equivalence. No new model/native run. A future mechanism must exhibit actual discrimination of a plausible wrong guard before another effect/cost cohort; the proposal is not yet validated.
