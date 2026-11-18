---
id: '28aab59cb5842630'
kind: spec
status: done
title: 'Semantic contract protocol: bounded development pilot with reused onboarding'
tags:
- effort-steering
- generic-mechanism
- development-pilot
---

# Semantic contract pilot design

## Intent and authority

Find whether a short, generic repair protocol helps low effort match or exceed xhigh repair quality at lower complete workflow cost. The user explicitly delegated autonomous research and expandable spending, with work isolated in worktrees. The choices below are the controller's decisions under that delegation, not new user-supplied requirements. No merge or push is included.

This is a development pilot on two now-exposed tasks, not held-out validation. Its immediate deliverable is a reviewable frozen protocol and a zero-provider prepared experiment; paid launch requires the explicit admission conditions below. Do not repeat source-only onboarding or the closed diagnostic regrade.

## Design check and execution scope

Fresh builder check /root/semantic_contract_builder_check returned: nothing important left. Under the user's explicit autonomous-work delegation, proceed with zero-provider implementation/preparation; no extra confirmation gate is introduced. All paid-launch conditions below remain mandatory. The protocol is a hypothesis, not a validated skill.

## Evidence and hypothesis

The original external cohort is closed. Both rust-analyzer captured candidates pass all8,182 selected tests. Nushell/low's original linker-crash grade was invalid; one registered unchanged-patch replay now observes both acceptance failures and all8,620 preservation passes. Nushell/xhigh passes both acceptance tests but has one real preservation regression. All original whole workflows remained incomplete under the noexec actor profile. Separate corrected native baselines now pass every registered public command on unchanged source.

Fresh artifact inspection: Nushell/low modifies only one value-representation branch and asserts merely that the stored result has the outer collection type; its assertion does not distinguish correct content from error-bearing content of the same shape. Xhigh reads/changes streaming behavior and asserts exact previous state, but its any-error guard also rejects a legitimate mixed result. This motivates a generic hypothesis already suggested independently by the earlier RA/bindgen gap analysis: explicitly trace the actual representation, and require discriminating checks for both the intended change and the closest behavior to preserve. This evidence derives the hypothesis; it does not validate it.

Sources: reportfce51ffc4baf7906, memoryresearch/effort-steering/low-recon-gap-analysis-20261104, immutable external-matched-repairs-20261105 candidates, and archives89dfc97b... (closed cohort/audit/replay) and a6f22446... (corrected native full baselines).

## Approaches considered

1. Repeat only low/xhigh under the corrected environment. This measures a usable baseline but cannot identify a new generic mechanism.
2. Add a low arm receiving one concise semantic-contract protocol, alongside low control and xhigh control, with common tools/onboarding/environment. Chosen: one small factor, complete paired development evidence, no new skill implementation or extra authoring call.
3. Change Recon, Codescout retrieval and checker policy together. Deferred because effects and costs would be inseparable. Tool availability and existing Recon remain common across this pilot.

## Fixed tasks and arms

Use the same exact task images, source commits, public issue texts, frozen global/project instructions and prepared project knowledge as the closed external cohort:

- nushell__nushell-18863; baseline5ec462b99bb0773843538cb545a741db84d0da92.
- rust-lang__rust-analyzer-22992; baseline3678da48aebe17797d2f792bde07a46f5bf4a827.

Each task has exactly three new attempts:

| Variant ID | Actual model effort | Added protocol |
|---|---|---|
| low-control | low | none |
| low-contract | low | exact text below |
| xhigh-control | xhigh | none |

Use admitted pinned Sonnet5.5 and the existing pinned CLI/plugin/native binary. Names are labels independent of model effort: do not pass low-contract as an API effort. Each attempt has a unique output directory and fresh physical writable state. Do not clone a paid arm's mutated memories, session history or testbed overlay.

Freeze order before model calls, independently of outcomes: NU/low-control, RA/xhigh-control, NU/low-contract, RA/low-control, NU/xhigh-control, RA/low-contract. The order balances the first effort across tasks but is not a randomized or replicated population design; do not claim it eliminates time/order effects. No outcome-driven substitute or rerun.

## Exact treatment text

Append this block once to the common task prompt of low-contract, with an explicit separator. No task names, hidden tests, private diagnostics, previous candidates or answers may be appended.

```text
Before editing production code, check the behavioral contract:
1. Trace the reported value or state through its producer and the representations or lookups used at the proposed change. Read the relevant implementation bodies; do not infer behavior from a name or outer type.
2. Identify two observable cases from the task and repository: the reported failure, and the closest legitimate behavior your new condition could accidentally reject. Include state, diagnostics or side effects when relevant.
3. Add focused regression checks for both. Assertions must distinguish the intended value, output or state from a wrong result of the same shape; type checks or absence of a crash alone are insufficient. Run the checks before the production change. If the failure is not reproduced, investigate or state what remains unverified.
4. Implement the change, rerun those checks and the required project verification, and repair any regression. Use only the supplied task and accessible repository evidence.
```

This is a prompt treatment, not a claimed enforcement gate. A filled narrative or successful skill Read is not compliance evidence. Do not install a new skill, change the global rules, add hidden-test-specific examples or grant extra tools. Changing the protocol after an outcome closes this registration and requires a new development iteration.

## Common environment and reuse

Use source version36fec1201b0f809d6f21cc31082def4d651b6cf8 for corrected native policy:4CPU,32GiB RAM and one private /tmp tmpfs rw,exec,size=4g,uid1000,gid1000. Both full public baselines and generated executable/doctest probes have passed under that profile. Keep the internal12GiB default out of external prepared configs.

Public commands remain exactly the registered commands. NU retains only its preregistered plugin-persistence skip; RA keeps RUN_SLOW_TESTS=1. No new exclusion based on observed failures. Keep1800s per public command,3600s workflow and8continuations for every arm. CLI terminal success is not workflow completion.

Reuse the immutable source-only logical knowledge and coherent runtime snapshots under external-logical-knowledge-20261105 and external-runtime-snapshots-20261105. Their project/source/rules/toolchain/binary identity must match. Copy SQLite backups physically per arm, never shared WAL or hardlinks between writable files. Preserve global model cache readonly. Reuse the exact frozen common guidance in the actual CLI context; its prior delivery defect must remain contained. No new LLM onboarding or task-specific packet generation.

Each exact prepared prompt, settings, tool permissions, model effort, common guidance, source image, native argv and writable clone manifest is hashed before launch. A synthetic actual-CLI wire check must prove the treatment delta is confined to the designated prompt block and that all six arms receive full global/project rules and native Codescout. The control task prompt must match the old common task bytes; prepared native paths/resources differ only as registered.

Use a new resident-scratch root outside the repo and /tmp for campaign data and orchestration backups. Exception: the existing validate_prepared contract requires each CLI actor context under /tmp to avoid host-home ancestry; make those small ephemeral contexts reproducible from hashed durable inputs, with all source/scripts/results backed outside /tmp. Keep all old roots immutable. Isolate code work in a new branch/worktree; do not edit the peer's tools/setup-ab-eval seam.

## Grading and admission

2026-11-06 handoff correction: the user relayed that the requesting Claude session ran only205 baseline tests, made no edits/commit and never started Task1. Codex01a15ca9 has now taken over the existing grader-validity worktree and plan144495d0b8e0f23e. Qualify and commit the actual fix before new paid admission; do not silently use an uncommitted version. Historical raw record/log/results bytes remain unchanged.

Before any provider call:

1. Bind new exact native/lifecycle/controller/grader source hashes and all immutable inputs in a new registration. Old wrapper source hashes are historical and cannot authorize new source.
2. Verify the source-bound actor runtime and instructions remain admitted and unchanged; a prior baseline receipt is reusable only when these inputs match.
3. Qualify a prospective grade policy with empty-patch negative and gold positive/repeat controls on both tasks. Genuine candidate compiler errors must remain semantic failures; fatal toolchain/storage/OOM errors must remain infrastructure-invalid. Missing/ambiguous statuses must remain measurement-invalid rather than fabricated test failures.
4. Settle parser recovery prospectively. The peer's glued-status helper is explicitly audit-only. It cannot silently become the primary live verdict. Either bind and independently qualify a new normalization/adjudication policy for this cohort before launch, or keep launch unadmitted. A post-outcome manual correction is not an admission strategy.
5. Use the already closed NUlow unchanged-patch replay as historical evidence; do not execute the peer draft's redundant regrade step unless it answers a separately registered new question.
6. Verify current host headroom and no overlapping heavy stage from our campaign. At least80GiB free before each heavy stage; record root/tmp availability and cgroup memory pressure through stages, keep a finite resource stop policy. External writers mean this threshold is not a capacity reservation.
7. Check all six untouched writable clones, reserve accounting and zero prior start markers. Admission must be false while any grader/source/input/control is unresolved.

All six candidates and reported costs close before private functional grading. Preserve every attempt and charge, including invalid/unfinished runs. Unknown cost, transport/context invalidity, candidate capture failure or cleanup failure stops subsequent model calls. A genuine functional failure alone does not cancel the fixed remaining attempts. Infrastructure-invalid grading stops interpretation and never triggers selective model resampling. All containers are checked absent independently.


## Prospective policy v1 — execution choice, 2026-11-06

Under the user's existing autonomous execution instruction, use a new explicitly versioned adjudicator around the committed raw grader a2ffc1432. Preserve raw log/grade/record bytes and report raw plus adjusted verdicts. Only expected statuses attributed by the unchanged reviewed audit helper to their actual glued physical line can be recovered; original failures remain. This policy is declared before any new native control or model run.

Alternatives considered: keeping raw missing statuses invalid would avoid recovery but leave the observed Rust logging defect unresolved; replacing record parsers or changing test commands would add a new oracle/task intervention. The selected wrapper preserves the parser, commands and expected set while making recovery an explicit, source-bound policy with fresh controls.

Missing/unknown runtime evidence, OOM, storage/process-fatal signatures, print/capture truncation or print failure invalidate measurement. Missing expected statuses without a discriminated ordinary compiler diagnostic invalidate measurement. A real candidate compiler diagnostic with a nonzero build/test step remains a functional compile failure, even with zero parsed statuses; it is not reported as thousands of observed failed tests. Full steps and raw verdict coherence are checked so stale/cut output cannot silently pass.

Qualify the exact existing4CPU/8GiB private profile separately from the already qualified32GiB public actor profile. Two tasks, one empty/gold/gold-repeat each, six fixed sequential controls; no selective retry. Resource floors80GiBroot/4GiBtmp/48GiBMemAvailable, sampled every10seconds and before cleanup; require initial/final cgroup observations and zero OOM counters, independently verify container absence. No model/provider/token/onboarding call. Source and input hashes fixed before controls; all originals and six repair states remain untouched.

The public classification function is a host-side pure boundary for the later adapter, with no private expected-set input. Actual public integration and paid admission remain false until the concrete adapter is wired and qualified. No repeated six CLI delivery fixtures or closed NUlow diagnostic. One fresh whole-change code/mutation review, one offline fix pass, then the six native controls and archival closure.

## Spending and cost scope

Carry the complete finalized tracked-series subtotal58.180531399999999104USD forward; do not add once-only authors again. The existing finite series ceiling is200USD. Six20USD reserves total120USD, giving worst reserved subtotal178.180531399999999104USD. Per-attempt CLI cap19USD plus20USD transport/workflow allowance. Compute with Decimal from source-bound receipts. A reserve is an admission bound, not an observed cost.

The once-only author cost17.837117099999999USD remains visible in complete-workflow and amortized accounting. Report new model-client repair/continuation charges, per-attempt complete workflow latency/public-check CPU time separately, and both unamortized setup and explicitly assumed N-task amortization. Do not call this a whole invoice or include author costs for one arm but not another. No new provider spend in design/preparation/wire checks.

## Endpoints and observations

Primary per-attempt endpoint: valid instructions/context/cost/candidate/cleanup, completed public workflow, and complete passing frozen private expected test population under the preregistered grade policy. Keep those components separate in raw records. A partial patch pass is not whole-workflow success; a parser or toolchain fault is not model failure.

Secondary: full client cost, workflow/check wall time, source changes, and trace evidence that the treatment was carried out. Process evidence requires actual body reads and executed pre/post focused checks with discriminating assertions; merely mentioning two cases does not count. Code review may locate evidence but must not rewrite outcome labels. No claim that a test assertion proves semantic correctness beyond its checked behavior.

Development signal to pursue: low-contract is a valid complete acceptance on at least one task where low-control is validly rejected, introduces no task-level acceptance loss elsewhere, and its summed client workflow cost does not exceed matched xhigh-control. This is a go-to-held-out-study rule, not proof of parity or a population effect. All six attempts must be interpretable for a complete pilot comparison. Missing grade/context/cost data makes the pilot inconclusive; no deleting invalid charges or substituting extra attempts.

If both low variants pass both cases, report a ceiling/cost observation. If the protocol is worse or fails, inspect the fixed completed traces and reject or revise the hypothesis without rerunning until favorable. If it helps only one case, future claims require fresh outcome-blind cases with the same frozen protocol. These two cases are already exposed and never become held out again.

## Deliverables and limits

Deliver a source-bound manifest/protocol, six fresh prepared configs and actual synthetic wire evidence, strict paid-launch admission, closed raw outcomes/costs if admitted, and a small per-task comparison/report/tracker entry. No production Recon/Codescout change is justified by this pilot alone. Current launch status: NOT ADMITTED pending final qualification of the concrete public/runtime/paid adapter (plan717fe707915d297c). The grader hand-back, prospective policy and six native controls are closed at source867c680924cb75575bec7ccd7068b8dfd7fd5889; archive2bc2fc8706b4b4d7c3d302e6b6cdd0b832704903db3bd4fe4ba4b23e7cf17fbc. Adapter review/fix has closed with20exactmutantskilled; final source gate/commit and one NEW actual-CLI common qualification remain pending. Experimental protocol, tasks, order, endpoints and spending rules above are unchanged. No new paid model run is started by this status update.


2026-11-07 admission closure: original adapter148 INVALID preserved; separately registered compatibility follow-up14c and independent closure audit QUALIFIED (plan82c73e8aba7113bd, archived dc72f66c...). Productfe8 committed/clean; source-bound final143 gate revalidated. New final registration/admission/preflight at /home/marius/.cache/codescout-resident/codescout/semantic-contract-paid-20261107 explicitly resolves all six gate proofs against the closed source/evidence and verifies every original writable clone, instructions, images, plugin and resource/budget boundary. Paid fixed-six launch is now admitted; at this update no paid attempt has yet started. This dated update supersedes the older pending-adapter launch label only; protocol, order, endpoint, no-resampling/private-grade embargo and spending rules above remain exact.


2026-11-07 post-launch admission revoked prospectively: independently reproduced review counterexamples03ce327f7fb3173b (candidate test binary SIGABRT misclassified as infrastructure, including public boundary) and7ee0a86c16915ff6 (unselected doctest compile diagnostic overrides passing selected set). No private candidate grade was opened. The original admission bytes/SHA are preserved as semantic-contract-paid-20261107/admission-at-launch.json; admission.json is explicitly revoked with audit receipt. Intentional epoch mismatch stops subsequent calls at the next campaign checkpoint, allowing any current attempt to finish cost/candidate/cleanup normally. No registered source/input change, client kill or selective retry. Full fixed-six interpretation is unavailable unless a separately registered prospective disposition is justified; do not silently resume or resample this campaign. Genuine toolchain failure invalidation with partial statuses remains an intentional prospective-v1 rule, distinct from the initial raw-grader plan's compatibility rule.


2026-11-07 final closure: six original fixed attempts and costs captured; all6v2controls qualify; all6savedcandidategrades valid; independentofflineauditall12cells/188bindings/resourcepopulation/freshabsence completed. Amendedv2disposition explicitly declared aftermodelruns beforeprivateoutcomes; originalregistrations/source/receipts unchanged. Registered improvement signal absent: NUreject/RAacceptforall3variants, gains0/losses0, low-contractcost+45.94%vslow. Pilotstatusdone denotes a completed NEGATIVE development screen, not achievement of the genericmechanismgoal. Mainresultreporta8e446ea5cef845e; deeperdiagnosisbfb2a49230efdf94. Protocolwasdelivered andchangedobservedNUpreedit testing, but none ofthreeNUactors exercisedmixedsuccess/error; RAcontract also omittedthelegitimatecasecheck. Oldtaskstays exposed; no furtherpaidrun admitted. Productintegration/ownershipblock remainsseparate4b4fac5682a4303b.
