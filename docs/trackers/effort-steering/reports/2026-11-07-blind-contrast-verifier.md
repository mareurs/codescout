---
id: f4b73341a2d56211
kind: report
status: done
title: 'Blind low verifier: all wrong proposals pass, qualified reference fails'
tags:
- effort-steering
- evaluation-integrity
- negative-result
---

# Blind contrast verifier — oracle inversion

## Decision

**NO-GO for the tested free-form low verifier as an acceptance gate.** It rejected zero of three saved wrong proposal variants and would reject the qualified reference. This is a stronger diagnostic than another task-level 1/2 total: we directly tested whether the generated assertions distinguish a wrong repair from a correct one, and their ordering was reversed.

This is one exposed Nushell development task and one valid verifier draw. The three proposals share the same semantic error; they are not three independent tasks. No population success rate, variance estimate or generic low/xhigh equivalence follows from these results.

## Frozen comparison

T1 is contrast_error_value_result_keeps_ans_readable. T2 is contrast_error_inside_stream_keeps_ans_readable. All five final cells had a successful build, both named statuses, valid sampled resource evidence and zero observed container OOM kills.

| Variant | T1 | T2 | Would pass generated gate? |
|---|---|---|---|
| Public baseline | pass | fail | no |
| Saved low-control, privately rejected earlier | pass | pass | yes |
| Saved low-contract, privately rejected earlier | pass | pass | yes |
| Saved xhigh-control, privately rejected earlier | pass | pass | yes |
| Qualified reference repair | pass | fail | no |

The reference is the same gold patch used in the previous closed positive controls, which passed the registered F2P/P2P checks. This experiment executes only the newly generated public tests; it does not rerun the private suite or alter earlier verdicts. The four evaluation patch inputs are frozen in the evidence bundle.

## What the verifier found, and what it got wrong

The fresh verifier independently chose this public input:

```nu
[1 2 3] | each {|x| if $x == 2 { error make {msg: 'boom'} } else { $x }}
```

It therefore did find a partial-success case without receiving the known private witness. However, its assertion demanded that the stored payload contain no error values. In both baseline and reference the observed payload is a list containing Int(1), an error value, and Int(3). Their ans_value call returned a record successfully; the test fails later on the extra no-error assertion. The public requirement that fetching ans remain usable does not imply discarding every result containing an error.

The three wrong proposals discard this result and keep the earlier error-free value, so the generated assertion rewards their shared over-correction. The test input was useful; the expected-result rule was wrong.

T1 runs an integer list followed by str length and requires the earlier stored value. It already passes baseline. Its label refers to the reported bug, but its actual assertion does not reproduce that bug. This is why naming a regression test or observing green tests is insufficient evidence.

The verifier's final answer reported no justified counterexample and acknowledged that discarding useful stream rows could be an over-correction. Its recorded navigation reads the proposal and test harness; it makes no symbols call to inspect producer/caller bodies and does not execute the baseline. Available guidance requested that tracing, but the prompt did not enforce it. This is an observation about this draw, not a causal attribution to one missing tool call.

## Inputs and isolation

The unchanged adapter came from the clean worktree at fe8bb4b34f5e7912f9b16c4c0bf1ff73a3a10a23. The verifier used Sonnet 5.5, CLI 2.1.309 and low effort. All 15 saved request files show that model/effort and max_tokens 8192. Global/project CLAUDE instructions, the pinned skill-only plugin, Codescout and the existing source-only onboarding were retained. No onboarding LLM call was repeated.

The public base image was extended only with the saved low-control proposal. Actual native mounts are retained in launch-preflight.json in the archive. The actor received neither reference/private test patch, private grades, the previous diagnosis, prior actor transcript nor outcome labels. The controller already knows those outcomes, so this remains an exposed development control. The reference entered the host-only evaluator after verifier closure. No private feedback was returned to the verifier.

The generated candidate was captured as SHA256 e1f8465d91523380d74e7f2ae8a31ec851888913cff9d3a5b3d9cb7cced79c94. Its production and gitignore segments equal the supplied proposal; only two test functions were added. The extracted test patch was frozen as SHA256 55e155775730175ce246d5a7b8dd194d549765d3b43d90a7c67e4f03eb960b45 before evaluation and remained unchanged.

## Invalid attempts and measurement corrections

A first verifier attempt was invalid because the controller set zero continuations, preventing background-job delivery and candidate capture. It is preserved and unscored. A separately registered technical follow-up used the identical public prompt/proposal/image, fresh state/session and four permitted lifecycle continuations; it settled in three turns with both background outcomes delivered. Its workflow hygiene check was git diff --check, not detector evidence.

Offline evaluation first stopped before any assertion because Docker cp into tmpfs reported success while the copied file was absent from the running mount namespace. An exact-profile diagnostic reproduced this and stdin patch transfer passed. The stdin revision then ran the baseline but incorrectly assumed every assertion failure exits 101. The real harness returned ordinary exit 1 with complete statuses; saved baseline evidence was reclassified without rerunning it. Original evaluator outputs are retained. These are local orchestration errors, not evidence of a model-quality result or a fault in the old frozen grader.

A later xhigh cell was invalidated when host available memory fell below the unchanged 48 GiB floor. It had no assertion statuses and was not scored. After headroom recovered, only that cell and the unstarted reference ran in fresh containers; baseline and both low cells were reused. Final cells used 4 CPU, 8 GiB, network none and executable 4 GiB tmpfs. Resource observations are sampled at nominal 10 seconds, not a continuous guarantee.

The reused parser was calibrated against all 8622 selected statuses in the previously closed Nushell xhigh log before parsing new assertion names. A final audit re-parsed all five logs, checked source/test/log bindings and sampled resource evidence, and verified all owned evaluation container IDs were absent. That audit launched no model or native test rerun.

## Cost

The valid verifier's final cumulative client cost was $0.2291159. The invalid first call cost $0.1780582 and remains included: total new verifier spending is **$0.4071741**. The known tracked series subtotal is **$62.153943199999999054**, within the standing $200 ceiling. Resumed-turn cumulative snapshots were not added together. These are recorded client model costs, not a complete workflow/invoice or human-time estimate. Offline executions made no provider calls.

## Next research decision

Separate input discovery from expected-result selection. A concrete next mechanism should execute each generated probe against public baseline and proposal, preserve both observations, and require a public requirement or an observed behavioral contract to justify an intentional difference before turning it into an acceptance assertion. A baseline discrepancy is a question to classify, not automatically a regression: the reported bug itself must change.

Do not expand this verifier into another outcome cohort or add more instructions to the same prompt. First establish an observable, valid oracle contrast while retaining correct fixes. A public-only baseline/proposal differential stage is the next development hypothesis; it is not implemented or measured by this report. Only after that works should an outcome-blind transfer population measure repair gains and full workflow cost.

## Evidence

- [Audited assertion matrix](../../../research/data/2026-11-07-blind-contrast-verifier-audit.json).
- [Evidence archive](../../../research/data/2026-11-07-blind-contrast-verifier-evidence.tar.gz): 110 payloads plus manifest, every payload reopened and verified; SHA256 a12dd34af4b43fa0b3a6f3843c381405eec0e1b9291da19c56ff7a76ac434d8a. Raw resident state and excluded runtime/credential stores were retained separately.
- [Execution plan](../plans/2026-11-07-blind-contrast-verifier.md).
- [Oracle inversion issue](../../../issues/2026-11-07-generated-contrast-tests-invert-repair-oracle.md).
- [Prior semantic diagnosis](2026-11-07-semantic-contract-diagnosis.md) and [closed pilot results](2026-11-07-semantic-contract-pilot-results.md).

No product source was modified, merged or pushed by this probe. The pending staged grader changes and previous frozen cohorts remain separate.
