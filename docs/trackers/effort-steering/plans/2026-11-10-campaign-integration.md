---
id: d96c176fdac2802f
kind: plan
status: done
title: Integrate the fixed evidence-gated campaign and native evaluator
tags:
- effort-steering
- implementation-plan
topic: effort-steering
---

# Fixed Campaign Integration Implementation Plan

> **For agentic workers:** use superpowers:executing-plans inline, preserving current cf66 WIP. Standing operator autonomy opts out of repeated signoffs. One final whole-admission reviewer with actual mutations, one bounded correction, no nested review loop. No source commit under the existing publication hold.

**Goal:** Execute the registered eight-attempt L/R/P/X comparison through one concrete entry point, freeze every cost/public output/candidate before independent grading, and admit payment only after actual integrated qualification.
**Architecture:** A generic host controller owns order, immutable intents, accounting, batch closure and analysis. A native backend owns per-phase isolated preparation, source-only knowledge reuse, actual CLI/native capabilities/resources, existing phase adapter and the separate frozen prospective evaluator.
**Tech Stack:** Python stdlib, existing effort_protocol/lifecycle, pinned local images/CLI/plugin and reviewed complete-stream evaluator.
**Spec:** [parent design](../evidence-protocol-design-20261109.md), [parent plan](../evidence-protocol-plan-20261109.md), [qualified runtime](../reports/2026-11-10-shared-public-runtime.md), [prospective evaluator](../reports/2026-11-10-prospective-linker-classification.md).

## Global constraints

- Policy/prompts remain task-agnostic. IDs are opaque; public command adapters/canaries are data. No new task diagnosis/hints/private acceptance into actors.
- Exactly two repository-distinct frozen development tasks; schedule taskA L/R/P/X and taskB X/P/R/L. Never replace/retry after an actor outcome.
- L/R/P reservation3USD each and X6USD each; P/R phase budgets0.60/1.80/0.60, CLI caps0.55/1.65/0.55. L CLI2.75 and X5.50. Total attempts30USD plus setup4 and contingency4 =38USD.
- Preserve decimal costs, unsuccessful/incomplete attempts and unknown charges. Series ceiling200USD; historical known64.401093499999998654 and unresolved4USDhold must not be zero-filled. Paid admission requires reconciliation.
- P/R at most three invocations with at most two continuations each. Malformed P submission or functional final-check failure ends incomplete without another actor.
- Protocol completion and independent acceptance are separate. A captured, valid candidate from an incomplete workflow remains eligible for quality grading.
- All actor outputs/costs/candidates close before any hidden grade. Any infrastructure invalidity, unknown charge, drift/leak or uncertain cleanup stops the campaign. Stopped/unexecuted attempts remain explicit.
- New prospective evaluator semantic-stream-contract-v2 only; historical grades/helpers unchanged. Preserve F3/F7 residual decisions visibly.
- No merge/push/global config/new onboarding/destructive cleanup. No paid run until final frozen registration passes concrete preflight.

### Accounting clarification — 2026-11-11

Reconciliation now binds the previously recorded29.221154299999999834USDopeningbalance and30individually rehashed final native receipts totaling35.17993919999999882USD. Known client subtotal64.401093499999998654USDmatches the recorded series. Earlier opening-balance charges were NOT freshlyrescanned. Evidence: ../evidence/campaign-accounting-20261111/reconciliation.json.

The original pre-provider-startup UUID98aaa6c0-110a-4a19-8789-b39df522284b remains costnull/unknowntrue and4USDunsettled. The executor clarifies admission under the operator's existing200USDdelegation: permit only an explicitly registered, hash-bound carried historical hold, include its full4USDreservation in commitments, and never mark it settled or costzero. All unmatched/new unknown charges or holds still block or stop the campaign. A38USDnewallocation plus known balance and held4USD equals nominal106.401093499999998654USD; reservations are not provider hardcaps or invoice claims. This does not increase the seriescap, replace any task, retry the old attempt or relax current-campaign unknown-cost stops.

Implemented default-block behavior plus the explicit evidence-checked carry branch in campaign.admit_history/reserve_attempt and runner._reserve. The47controller/accountingtests pass, including forgedproof,changedreceipt,omittedhold,changedamount,extraunknown,capoverflow,retainedholdaftercompletion and first-new-unknown-stop. An actual reconciled-ledger reservation probe preserves the4USDhold and yields106.401093499999998654USDcommitted with the38USDallocation. Native paid admission must still verify actual qualification/public-command/runtime/source predicates and pass the ONEwhole-admission review before any provider call. This dated clarification supersedes the implementation's unconditional rejection of all historical unsettled holds; it does not itself admit payment.

## Candidate-source readiness correction — 2026-11-11, before paid repair

Controlled R challenge input with an invalid Cargo manifest reproduced a false infrastructure classification (issue21a0cf77020b065f). The active project, expected languages, tool advertisement, current code/memory index and all RPC frames were valid. References and symbol_at lost their old positive semantic witnesses; call_graph and semantic_search still returned positive results. A separate registered cargo check on that exact input completed/resource-valid with exit101 and a TOML diagnostic. No model/provider call or private grade was made by these controls.

Correct the generic readiness contract before freezing the pilot: bind a physical immutable source_baseline to the registered initial task and its digest. Require every positive semantic canary on that initial source. After a candidate changes that source, retain all runtime identity, active-root/language/tool, index, argument and RPC-success checks, while recording which old semantic witnesses remain observed. A changed source is allowed to change semantic answers. Report probes_verified as the actual positive witness count, probes_observed separately, and readiness_scope as changed-source-capabilities. Never call a missing witness a positive result, invent valid RPCs, relax the initial baseline, or hide missing tools/index data.

This rule applies equally to P and R and preserves R's chance to repair its own intermediate source. Public semantic gates remain P-only. Prompts, phase budgets, task population, source/image identities and private evaluator stay fixed. Caller-supplied baseline flags must not bypass physical baseline hashing or its equality to the initial registered task. Default callers without the explicit baseline retain strict positive-witness behavior. The current unit regressions pass33readiness/95combinedtests; the corrected actual invalid-candidate control completed:2positive witnesses of4observed, actor started, valid context/capture/cleanup and measurement_invalid false; it remains incomplete because final public verification failed, with final client cost0 and no provider call. These are pre-pilot runtime corrections, not repair efficacy or outcome-based task replacement.

## Mandatory first-request context correction — 2026-11-11, before paid repair

Before freezing the new pilot, append the exact frozen raw CodeScout onboarding and complete Recon SKILL.md to the CLI system input for every L/R/P/X invocation. Retain user,project,local CLAUDE loading and the original public task. Copy the unchanged Recon supporting files to an actor-local .codescout/prepared-context/reconnaissance path; bind all bytes. No onboarding author call or per-arm rewrite is allowed. Audit the ordered visible preload lines in the actual system payload before any provider forwarding, and independently verify delivery in request-0.json. Model-selected Skill/onboarding-read calls are observations, not admission requirements. Do not invent acknowledgments. Legacy controls lacking this preload remain historical controls; qualify the corrected path with an actual CLI first request that makes neither call, then freeze the persistent eight-attempt runtime qualification. This is a context-validity correction, not evidence of repair efficacy.

## Persistent full-runtime qualification freeze — 2026-11-11

The new persistent driver uses the full original171-task population,36exposure registry IDs/10intersecting exclusions, original seed and ordered Lean1168/Obscura421 pair; fixed L/R/P/X then X/P/R/L schedule. Before this freeze, replace context-file-only synthetic CLI checks with the same already-qualified public cargo check for every arm, retaining its registered working directory. P/R host public_checks use that same command. Set the invocation deadline3600seconds and both CLI/public check deadlines900seconds, identically across arms. Keep phase budgets,2continuations, private99-file evaluator and original issue fixed. Cached onboarding and the new context preload are reused unchanged.

Actor runtime modules/prompts, private package, public inputs and driver dependencies are hash-bound before launch. Host-only paid_admission.py and test files are outside the executed-actor freeze; their final current bytes still require direct tests and the one whole-admission review before payment. No actor runtime source edits during the qualification. Keep all raw outputs on /var/tmp/ep11-01a15 and archive closure before claiming durable readback. Scripted zero-cost accounting is a transport fixture; the historical64.401093499999998654client total and4unknown hold remain in the ledger, not replaced with zero. No efficacy conclusion from synthetic markers.

## Review focus

1. An exception after a provider call must retain its known/unknown charge and any safe captured patch; replay cannot buy another call.
2. A malformed or incomplete P workflow remains an outcome while later arms run; it is not excluded as infrastructure.
3. Private grading cannot start until the entire actor stage and costs are sealed, and its result cannot reach any actor.
4. Registered phase allocations/caps/turn limits and low/low/low/xhigh must reach the actual CLI; summaries alone do not prove delivery.
5. Candidate-driven syntax/canary failures, post-check edits, source/context drift, stale fixtures or a forged backend/admission flag cannot produce favorable exclusion or paid admission.

## File map and interfaces

- Create tools/effort_protocol/runner.py and test_runner.py: generic orchestration and paired analysis.
- Modify adapter.py, native_executor.py and their tests: explicit CLI caps and optional trusted supervisor flow; existing defaults remain compatible.
- Create native_campaign.py, campaign_resources.py and consuming tests: concrete public preparation and measured runtime adapter. Avoid importing private evaluator modules into actor preparation.
- Create evaluator_entry.py and tests only if needed: subprocess loading of the frozen reviewed evaluator package, bound complete streams and unchanged private expected population.
- Registration evidence, helper outputs and final review/custody live under this tracker and this plan's workspace.

### Task 1: Generic immutable campaign, cost and phase orchestration

**Interfaces:** run_campaign(registration:dict,backend)->dict consumes version evidence-campaign-v1, mode scripted|paid, campaign_id, output_root, selected metadata, exact schedule, task records, file bindings and standing ledger. backend.invoke(request:dict,out:Path)->dict writes a host invocation receipt; backend.grade(request:dict,out:Path)->dict is called only after batch closure. backend.verify(registration)->dict performs live native/fixture readiness outside actor control. Paired analysis consumes exactly two task rows and four arms, never counts requests as tasks.

- [x] Preserve cf66 owned WIP/source identity and read current relevant bodies/callers. Initialize plan ledger/brief. Snapshot the 18 existing dirty source files before edits.
- [x] Write failing consuming tests for exact8order/phase opportunities, first-failure stops, duplicate-root/start refusal before backend effects, tampered bindings/baseline/output overlap/private packet fields/wrong backend, unknown/decreasing/duplicate costs, capped budgets, malformed phase/state/turn transitions and ordinary effort. Assert backend call count and preserved artifacts, not just exception class. Run test_runner.py: expect RED before runner exists.
- [x] Implement generic orchestration with exclusive root/registration/intent/receipt/ledger/candidate/batch-close records. Validate cost metadata before further effects, preserve unknown reserves, release only reconciled reservations, never overwrite prior runs. Test incomplete candidate remains eligible and no hidden grade precedes final batch-close. Run all focused suites: expect green.
- [x] Add explicit phase CLI-cap and trusted-supervisor propagation tests before production edits. Ensure invalid cap refuses before any launch/prompt mutation and both L/X and P/R forward the frozen cap. Run targeted tests RED then GREEN.
- [x] Add paired analysis tests for exact advance screen: all8interpretable, bothPclosed, at least one P gain over each L/R, no loss versus either, P accepted count>=X, P total model cost<X. Tie, incomplete, unknown or stopped cohort is NO-GO. Keep complete client cost and shared setup/amortization separate.

### Task 2: Concrete native campaign, evaluator and whole-admission qualification

**Interfaces:** NativeCampaignBackend implements the Task1 methods using existing NativeExecutor/run_phase and a measured resource supervisor; only this exact frozen native backend can be paid. Scripted mode never reads real credentials. Independent evaluator entry takes source/record/runner/runtime bindings and writes full capture plus prospective v2 verdict. preflight(registration)->dict returns every concrete predicate and paid_readyfalse unless all are true.

- [x] Build the native adapter from the already inspected actual CLI controls, passing parameterized public source/rules/knowledge/runtime/fixture/canaries rather than repository-specific policy. Reuse raw onboarding once; clone physical inputs per invocation and bind index to changed public source without another LLM author. Test candidate-driven non-readiness remains an explicit workflow outcome, with no retry.
- [x] Integrate the exact checked 99-source prospective evaluator package as a separate host-only runtime, not an actor import. Recheck native negative/reference/repeat receipts/source/image/control/expected-set bindings and all original names. No full compiler/control rerun unless a changed boundary requires it.
- [x] Add real resource-floor/configuration and supervisor tests, with actual executable tmp probes and owned native/container identity/cgroup/host samples plus final cleanup. Frozen thresholds are configuration, not installed-RAM claims. Record actual floors before qualification/payment.
- [x] Run the actual controller in scripted mode across both frozen projects with L/R/P/X, including all P/R phases, phase-local source changes and no provider forwarding. Verify actual request effort/caps/context/skill bodies, phase gates, final source and cleanup. Exercise malformed/incomplete/unknown-cost/drift controls; retain every control. Synthetic replies qualify runtime only.
- [x] Recompute standing spend from immutable receipts and consult available local trace/usage sources for the4USDhold. Do not manufacture a terminal zero. Offline work continues if this alone blocks payment.
- [x] Run direct effort_protocol/lifecycle/evaluator suites and required project gate; disclose skips/exclusions. Seal exact final source/transitive dependencies/control evidence. Current v3 passed414protocol(3skips)/108lifecycle/1392evaluator/all five gate lanes with192matching source hashes; review/correction capsule closed.
- [x] Request ONE fresh strongest whole-admission review. Actually apply at least8guard mutants on isolated copies and report killed/survived/invalid/timeout. Read survivor evidence before tests; one bounded correction and exact replays, no re-review.
- [x] Freeze a NEW registration and run no-cost preflight. **Resolved by the separately reviewed transport-headroom revision and fresh headroom-v3pilot: all7actual proof groups passed; all8actors and8private grades completed; no old candidate was spliced in. See 2026-11-11-transport-headroom-correction.md and the closed primary report. The remainder of this checkbox records earlier stopped-launch history, not a pending retry:** First real preflight passed, but controller stopped before provider on a caller string/Path error (issue919389d3a2a17044); eight-attempt pilot remains unfulfilled. Administrative rebind and typed Path preflight passed all7groups without another code/mutation review. The real replacement stopped after L closed and R investigation hit the phase transport envelope at0.1636965USD (issue7e84dd9c59e9a780);6attempts/all private grades remain unexecuted. The exact offline reserve denial is reproduced. Preserve0.3678025USDknown new client cost and all raw outputs. Correct reserve semantics prospectively and decide carry/restart eligibility before any private outcome is revealed; a change of envelope needs a new binding, never an implicit retry under the old registration. Stay under the standing200USDseriesceiling. If any prerequisite is missing, preserve the explicit false predicate; do not use mock green.
- [x] Grade eligible captured candidates after actor/cost closure, apply the exact fixed advance screen and publish paired per-task quality/protocol/cost plus caveats. Closed primary report ../reports/2026-11-11-closed-generic-repair-pilot.md: L/R/P2/2private acceptance, X1/2with captured-mutation cutoff retained; P0/2protocol closure and more costly than X. Registered NO-GO closes this version without exposed-task retuning. Batch4.60646289999999987USD; all8valid private grades. Paid capsule2584835393bytes,SHA42d8f9a0ca4d95eccd177761d9aeb7a3db1d289644e06787b471aa9f977b4fb8 verified. The generic goal remains open; source changes remain local/uncommitted, with no merge or push. Further progress requires a justified fresh-task mechanism, not another run of this pair.

### Task2 integration checkpoint — 2026-11-11

P public-submission-contract-v1 and configured probe directories now reach real CLI request bodies. Two intentionally incomplete synthetic CLI controls settle0modelcost/0forwards with15cgroup samples each and complete cleanup; checked capsule/report [checkpoint](../reports/2026-11-11-campaign-integration-checkpoint.md). Resource implementation/direct tests qualify that plan line, not the whole campaign.

Public clone preparation now preserves the shared code/memory store and all raw onboarding. The initially destructive reset is retained as a failed control and MAIN4bff4a334abf2a95. Four actual v2 unchanged/changed controls across both frozen projects qualify; new public probe chunks and raw static guidance are checked, author/modelcalls0. Registered index updates are incremental and bound to actual public Git HEAD. Native Lean's zero-byte omission stays literal and separately checked.

Ruling: candidate-driven non-readiness still has no actor/model/seed retry. Carry the prior v11 model-free startup handling as explicit lsp_warmup_retries0or1, maxONE across all LSP canaries for the parsed producer30s timeout only, with both replies preserved. Both Lean controls actually consumed1; both Obscura controls consumed0. This mitigates startup, not the unknown cause of the delay. Whole-admission review will inspect this bound.

NativeCampaignBackend, host-only private evaluator entry, actual scripted8attempt controller, final checks/custody/review and accounting/preflight remain uncompleted. No source commit or paid repair.

### Storage recovery and finite remaining admission — 2026-11-11

Historical recovery checkpoint; current status is the bounded whole-admission correction in the current report. The earlier16-native/8-private controller run completed, but its unarchived /tmp output disappeared after restart (issue63a3b73e77b64708,F-14). It is historical producer evidence only. Exact public inputs and99evaluator sources were restored from checked archives onto /var/tmp/ep11-01a15. Current two CLI controls, two private baseline processes and two independently reopened public compilation receipts pass there. Direct373protocoltests(3skips),107lifecycle and47controller/accountingtests pass. Original model/quality outcomes remain unchanged. The new closed-control source/output capsule is undergoing independent readback; no new LLM call.

Remaining implementation is the concrete paid admission path in NativeCampaignBackend.verify/invoke, which still deliberately refuses paid mode. It must consume actual hash-bound cohort/runtime/public-command/private-evaluator/cost/review evidence, never a caller-supplied ready flag. Reuse the original171-task projection/exclusions/seed effort-evidence-stage1-20261110-v1 and the fixed public pair. Do not use the synthetic qualification's two-row universe/seed as the paid selector.

Then perform the fixed eight-attempt zero-forward qualification on persistent scratch with final runtime source, seal its complete output, run required final checks, and use the ONE final strongest whole-admission review with applied mutants and one bounded correction. Freeze the new real registration and only then run its preflight/pilot. No extra slice reviews or routine compiler/control reruns for unchanged boundaries. Runtime qualification is not efficacy; any n2 tie or NO-GO follows the registered stop/validation rule.

## Self-review

Task1 depends on the existing receipt/input digests and phase adapter; Task2 supplies concrete native verification rather than admitting from booleans. The ordinary backend must retain workflow_completed from the actual lifecycle receipt; P/R use host transitions. Private evaluation is separate from the public packet and delayed until closure. No task or private-function literals belong in new policy. The current4USDhold remains an actual paid prerequisite, not permission to ignore accounting; it does not block model-free implementation. Final source remains WIP under the standing publication hold.
