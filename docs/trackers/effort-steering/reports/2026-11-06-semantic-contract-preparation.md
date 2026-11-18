---
id: d2a7dd6b2f164989
kind: research
status: active
title: Semantic-contract preparation and observed assertion discrimination
tags:
- effort-steering
- generic-mechanism
- development-evidence
---

# Semantic-contract preparation and observed assertion discrimination

## What was established

A local, zero-provider control confirms a concrete verification gap in the old Nushell/low candidate: its new regression test passes even after its production correction is removed. Replacing the outer-type assertion with equality against the previous observable state makes the same case fail on original and low-patched production, and pass on xhigh-patched production.

| Focused assertion | Original production | Low production | Xhigh production |
|---|---|---|---|
| Original agent test: outer collection type | PASS1/1 | PASS1/1 | Not run |
| Stronger test: exact previous value | FAIL0/1 | FAIL0/1 | PASS1/1 |

These are five actual executions of one named focused test, not five model runs. The low production deletion was applied and observed surviving before the stronger assertion was written into the test source. This supports the assertion-discrimination finding on the exposed development case. It does not prove that instructing a low model to write stronger checks will make it do so or produce a correct repair.

Xhigh's whole candidate still has the independently measured preservation regression from the private audit. Passing this focused assertion does not establish complete candidate correctness or a general xhigh advantage.

## Preserved instrument error and continuation

Initial control@bg_00000104 ran three phases, then stopped because its expected failure exit101 did not match this custom harness's actual exit1. The actual named test and0passed/1failed summary were present, with List(Error...) versus List(Record...) assertion details. This is issue32efc4f2deafd517, a measurement-instrument assumption. Initial registration/progress/logs remain unchanged.

A separately registered continuation@bg_00000106 ran only the two unexecuted low-strong and xhigh-strong phases. It requires the named one-test result, actual assertion detail for failures, and nonzero failure rather than a guessed exact code. No first phase was rerun. Both containers were removed and independently absent. First logs preserve stdout then stderr, not a chronological merge; continuation preserves separate streams. No private oracle was injected, no model/provider request or credential read, and original candidates were not edited.

## Generic hypothesis frozen

Design28aab59cb5842630: [semantic-contract pilot design](../../../superpowers/specs/2026-11-06-semantic-contract-pilot-design.md). Fresh independent builder check returned nothing important left. Plan c1947b8eba0620dd: [preparation plan](../plans/2026-11-06-semantic-contract-pilot.md).

The exact153-whitespace-word protocol asks low to trace actual producers/representations, identify a reported failure and closest legitimate behavior to preserve, write and execute discriminating checks, then implement and retest. No task-specific function or hidden expected result appears in the protocol. This follows the earlier RA/bindgen analysis and the new observed Nushell assertion gap. It remains a proposed prompt treatment, not a validated skill or enforced gate.

## Concrete prompt/input bundle

ROOT /home/marius/.cache/codescout-resident/codescout/semantic-contract-pilot-20261106 contains six unique prompt/input bundles: two exposed tasks times low-control,low-contract,xhigh-control. Only the two low-contract prompts receive the exact added block. Other common task bytes match the old low/xhigh pair; frozen global/project rules are copied from existing source-only inputs, not silently replaced with mutable live config.

The freeze verified all selected pristine runtime payloads:985NU and965RA. Onboarding calls0.24 selected old candidate/config/receipt/grade files were rehashed and preserved. Five non-destructive input mutations were actually applied and rejected: duplicate variant ID, wrong actual effort, removing treatment while updating its local digest, adding treatment to a control with updated digest, and changed common rules. These test configuration invariants, not model quality.

Stage is explicitly prompt-inputs-only and launch_admitted=false. There are no new writable runtime copies, executable prepared.json files, actor contexts, six-arm synthetic CLI wire proof or paid controller. Those are remaining work, not implied by the word prepared.

Budget arithmetic is frozen but unspent: existing tracked subtotal58.180531399999999104USD; six20USD reserves/19USD CLI caps gives worst reserved178.180531399999999104USD under the existing200USD ceiling. Historical once-only authors17.837117099999999USD remain in the subtotal and are not charged again. These are scoped model-client amounts, not whole-invoice or compute costs.

## Remaining gates and coordination

The requesting Claude session owns the two grader corrections in a separate worktree. New paid calls remain blocked until its reviewed committed hand-back, a prospective complete-status parser policy, fresh negative/gold/repeat controls, six isolated native states and actual synthetic CLI wire checks, and a new source-bound controller registration. The peer's audit-only glued-status recovery cannot silently become a primary live verdict.

The already-closed unchanged-patch Nushell/low replay must not be duplicated merely because an older peer plan lists a regrade. Its final result is two acceptance failures,8,620 preservation passes,missing0; source/candidate/original receipt hashes unchanged. Direct coordination through the explicitly used reaching-peer-sessions plugin was refused before delivery because the peer CLI is2.1.319 while the measured protocol permits2.1.312. No socket fallback or broadcast. The operator can relay the closed replay path from memoryresearch/effort-steering/private-grade-audit-20261106.

## Evidence closure

Independent raw-artifact audit confirmed the five named focused-test results, complete pristine payload populations (985NU/965RA),24 protected old files, common prompts/rules, six exact input rows and153-word protocol. It audited existing cleanup receipts; the closure archive separately queries current daemon inventory.

Review actually removed eight verifier guards. Four mutations survived the original five-case negative suite, exposing missing test coverage (51f42e8ae1499528). Separately, the original verifier accepted a unique wrong arm_id and changed registration stage (af727dad8cd9ce91), and accepted zero/negative or over-reserve CLI caps and consistent nonpositive reserves (cd0ad8d33b2aae97). None changes the observation that the original frozen bundle itself is valid and launch_admitted=false.

Original helper and five-case verification are preserved as v1. The corrected verifier binds arm IDs and registration stage, requires finite amounts/nonnegative prior/positive cap no greater than reserve, and retains existing source/shared-input checks. A separate read-only v2 runner actually rejects39 invalid configurations with expected reasons while accepting the unchanged real bundle. Independent reapplication is closed:12 actual guard-removal mutations (the original eight plus four new guards) were detected,0 survived. Ten were caught because they newly accepted invalid inputs; two were caught because they lost the required rejection contract (duplicate-population reason and explicit finite-budget handling). Those latter two are diagnostic-contract controls, not proof the malformed input would reach paid execution. The reviewer independently reran all39 negative cases, matched preserved v1 bytes, and confirmed original registration/matrix/verification bytes unchanged.

Every saved resource sample was audited:8 initial and7 continuation samples. Root free-space minima were326,517,981,184 and326,798,979,072bytes respectively; recorded memory events including OOM were zero. These are sampled observations, with no host-/tmp-free/PSI series or continuous guarantee. No provider request, product source change, merge or push occurred. Archive hashes will be added after evidence packaging.

## Durable archive and verification scope

[Evidence archive](../../../research/data/2026-11-06-semantic-contract-preparation-evidence.tar.gz), SHA256 f796140a42a00f2d83e91fe55c1d2be373f4fe7ec652616acbda99649966e089;245,506bytes;174 payload files plus MANIFEST.json. Every member was reopened and SHA256/size checked. [Machine-readable receipt](../../../research/data/2026-11-06-semantic-contract-preparation-evidence.json) records the measured scope. The24 protected old inputs match both current bytes and the earlier private-grade archive manifest. A fresh successful docker ps -aq --no-trunc query independently confirmed both focused-control containers absent.

Archive includes original and continuation controls, all five raw test outcomes, resource/cleanup evidence, unchanged prompt/input data, preserved v1 and corrected v2 helpers, exact mutation sources/logs and final independent review. Pristine snapshot manifests are included; database/model weights, credentials, image layers and private oracle are excluded. These are development examples already exposed to the controller, not heldout validation.

MAIN ./scripts/gate.sh --check was also launched as @bg_00000109 (record /tmp/codescout-bg/455879-a720241d-bg_00000109.json). It checks the shared checkout separately and was still running at archive closure. It is not the source of the focused-test or verifier claims above, nor evidence of runtime/paid admission. No commit, merge or push was made.

## Subsequent shared-checkout gate result

@bg_00000109 closed exit1, 370seconds. Python light1642 tests plus529 subtests passed; formatter/clippy exit0. LEAN6161/6162 and DEFAULT9029/9030 passed; each has the same single failing test server::tests::companion_surfaces_reference_only_real_tools. It flags deliberately invalid-looking tool strings in sibling cs-verified-relocate.test.mjs; scanner excludes .test.sh but not .test.mjs. Filed immediately in MAIN as1dfa43ac9834a309 (docs/issues/2026-11-06-companion-tool-gate-scans-negative-mjs-fixtures.md). No product source fix is claimed. This result supersedes the still-running note at archive closure; the immutable preparation archive remains unchanged.
