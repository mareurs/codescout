---
id: a8a3516e64f5f652
kind: report
status: done
title: 'Why the semantic-contract pilot stayed at 1/2: real process changes, missing semantic boundary'
tags:
- effort-steering
- diagnosis
- evaluation-integrity
---

# Why the semantic-contract pilot stayed at 1/2

## Answer

The three variants have the same final acceptance, but their execution was not identical. The added protocol produced observable pre-edit reproduction/testing on Nushell; xhigh did more representation investigation and verification. None of the three solutions chose the missing mixed-success/error boundary. The precise result is a shared semantic generalization error that the public tests did not detect, not demonstrated equivalence between low and xhigh.

The current protocol failed its predeclared development improvement gate. That is a decision about this candidate mechanism on these two exposed tasks, not proof that all semantic-contract protocols are ineffective or that low is generally as capable as xhigh.

## The actual error, with an exact preserved example

Seed the interactive session with two records:

```nu
[{name: 'ab'} {name: 1}]
$ans.last | str length name
```

The first row transforms successfully to name2. The second row becomes an error. Baseline behavior and gold retain the useful transformed row plus the error row. All three candidates instead preserve the old records. Each saved private log directly reports the first wrong value: String("ab") versus expected Int(2).

The common new rule is effectively **any error means discard the entire transformed result**. Gold's rule is **a nonempty result containing only errors should not replace the previous value; mixed successful results still should**. Gold also changes nested-error rendering so retaining partial results does not make the surrounding ans record unprintable. All three candidates omit that rendering correction.

| Result class | Preserve previous payload? | Candidate rule | Gold rule |
|---|---|---|---|
| Ordinary successful transformation | No | No | No |
| Nonempty all-error result | Yes | Yes | Yes |
| Mixed successful values and errors | No | Yes: wrong | No |

This table is source-derived; the mixed first-field mismatch was actually observed in all three saved executions. No new native run was needed. Full input/representation/printing trace and direct source/log spans: [source diagnosis](../../../research/data/2026-11-07-semantic-contract-source-diagnosis.md).

## What the actors actually did

Every captured request used the intended effort: low for both low variants, xhigh for xhigh, all Sonnet5.5/adaptive thinking. Both low-contract initial requests contain the exact added protocol; low controls lack it. Therefore a missing prompt or mislabeled effort does not explain this result.

| Nushell arm | Pre-edit regression reproduction | Added tests in final patch | Missing distinction |
|---|---|---|---|
| Low-control | No new error reproduction before production | None | Mixed success/error |
| Low-contract | Observed failing regression before production | All-error preservation and wholly successful length | Mixed success/error |
| Xhigh-control | Observed failing regression before production; extra representation probe | All-error stream and single-error preservation | Mixed success/error |

Low-contract's second test uses length and expects scalar2. It is a valid ordinary-success test but does not distinguish a correct error-only rule from the overly broad any-error rule. Its tests can demonstrate an improvement over the broken baseline while still approving the wrong boundary.

The xhigh actor investigated stream versus collected-value representation, observed an extra collected-list probe already passing, removed it and narrowed its production edit. That is real extra work, but not the missing distinction. We cannot infer internal model attention from request counts.

The rust-analyzer arm is another useful warning: low-contract explicitly admitted it did not add the separate nearest-legitimate-case test, yet passed private grading. Xhigh added both a reproduction and an ordinary anon-const preservation probe and changed deeper producer/merge logic. All three passed the selected acceptance set. Equal final scores therefore also conceal different verification coverage on this task.

Full six-arm evidence, event ordering, successful versus blocked commands and citations: [actor diagnosis](../../../research/data/2026-11-07-semantic-contract-actor-diagnosis.md). Key NU stream lines: low production31/33/36 and final56; contract test45, baseline failure87, production93/95, postcheck103; xhigh tests93, baseline result124, extra probe178-196, production198/200, postcheck231. Rust-analyzer contract omission is stream149.

## Why green public verification did not help

All six workflows settled, with no budget stop, and all twelve declared independent public checks exited0. Private acceptance was3of6attempts, specifically all three rust-analyzer attempts. There are only two distinct task units, not six independent problem samples.

The exact mixed-result test is private and injected by record.test_patch. It was absent from the actor-visible baseline test file. P2P means baseline-passing, not actor-visible. The private test preserves existing source behavior and is compatible with the bug report, but the report alone does not explicitly discuss partial successes. Thus the models were not shown a visible failing test and allowed to ignore it.

The common public check could approve all three wrong Nushell fixes, so a repair/escalation loop triggered only by that public result had no failure signal to act on. This is a limit of the observable verification signal in this task population, not a newly demonstrated public-classifier implementation bug. It also shows why more time or more public reruns on the same unchanged test set would not establish the desired semantic distinction.

Visibility correction is recorded in issue6e1a5d01fc6ebb28. Original results, statuses and costs remain unchanged.

## What the days of work established, and what they did not

The earlier external cohort had incomplete public workflows under the noexec runtime; one Nushell grade ended in a linker crash before parsed statuses and another lost a passing status through stdout concatenation. Fixing executable temporary storage, context delivery and grading validity was necessary to obtain interpretable measurements. These are infrastructure gains, not evidence of a low-improving mechanism.

A separate Nov6 five-cell assertion probe had already established something narrower: the old low test passed even after removing the production correction; an exact-state assertion rejected that old low patch and accepted the old xhigh patch. This identified weak observation of state, not a general xhigh advantage. The current trial shows that exact assertions and pre-edit reproduction can improve while the semantic partition remains incomplete.

The prior evidence also already showed rust-analyzer accepted by both efforts and Nushell xhigh rejected on the same mixed-result preservation case. That is explicitly stated in the Nov6 design's Evidence and hypothesis. These two tasks were a development screen for fixing a known shared failure, not a population chosen to estimate a low-versus-xhigh capability gap. They remain useful development/negative controls; they cannot become held out again.

The current low-control now passes both Nushell F2P tests, unlike the older low replay, but fails preservation. Different runtime/workflow and a fresh model draw prevent attributing that improvement to a particular common instruction or tool. Matching xhigh's incomplete solution here is not enough to identify a mechanism.

The research reporting error was to lead with task acceptance1/2 and infrastructure closure instead of explaining the intended behavioral contrast, actual compliance and surviving counterexample. The measured outcome is real, but that summary alone did not answer the user's question.

## Cost and time interpretation

Across two tasks, repair-client costs were low0.596527USD, low-contract0.8705975USD and xhigh2.0991132USD. Contract's observed premium over low is45.94% with zero added acceptances. This is not a population estimate or complete amortized setup cost.

NU workflow wall seconds were861.611/1008.474/1040.011 (low/contract/xhigh); RA319.845/491.823/698.430. Final cumulative API seconds were46.781/79.608/150.616 and64.704/118.876/314.898 respectively. These are final receipt snapshots, not sums over resumed cumulative snapshots. Wall time includes workflow/public verification and is not pure model reasoning time. No share of the entire multi-day elapsed time is estimated from these six receipts.

## What should change in the research question

Do not repeat this whole cohort or append more generic exhortation to test carefully. The next useful hypothesis is a concrete contrast check: can a verifier, given only the public task, baseline and proposed patch, produce and execute a case that distinguishes the proposed guard from a plausible overly broad guard? For collection/error policies that requires testing partial success as a distinct class, not only wholly failing and wholly successful commands.

This is a proposed direction, not an implemented or validated mechanism. A detector must first reject plausible wrong fixes while retaining correct fixes, with observable command/assertion evidence. It cannot pass merely by naming cases or checking that a test fails on baseline. Any future generation of such cases must be blind to private tests/gold; copying this now-known witness into the prompt would be development overfitting.

Only after a concrete mechanism exhibits that discrimination should a new outcome-blind task population be used to measure lowbaseline versus low+mechanism and xhigh. For the effort-bridging goal, include independently established low/xhigh contrasts rather than assuming the expensive arm is a correct oracle. Keep the current two cases as exposed controls. No new run, skill modification or mechanism implementation is admitted by this diagnostic report.

## Evidence and preservation

Primary result: reporta8e446ea5cef845e and data/2026-11-07-semantic-contract-pilot-audit.json. Prior design28aab59cb5842630; actual older audit data/2026-11-06-grader-validity-audit-v2.json and original-profile low replay under the external-grade-audit-20261106 resident root. Actual prior assertion probe progress.json and remaining-checks/results.json under semantic-contract-probe-20261106. All were read during diagnosis.

This investigation was offline and read-only over frozen sources and raw runs. The two supporting analysis reports were mechanically copied from their unique temporary paths; source/input hashes and result populations were not modified. Current combined source remains explicitly uncommitted under ownership issue4b4fac5682a4303b. No new model/provider/native grading call, merge or push.
