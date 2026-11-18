---
id: '5a3fc304bd67fa61'
kind: plan
status: done
title: Prospective specification oracle preserving the frozen acceptance population
tags:
- effort-steering
- implementation-plan
- oracle-admission
topic: effort-steering
---

# Specification Superset Oracle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans inline; one fresh strong final reviewer with actually applied mutations. Standing user delegation opts out of recurring approval rounds. Preserve WIP/evidence; no commit, merge, push or deletion.

**Goal:** Qualify a private functional check of the actual Lean cache-status requirement before any model trial, retaining the frozen pair and every original expected test.

**Architecture:** A new prospective acceptance version requires the original full selected population AND a private public-specification oracle. The old failed admission and original dataset remain immutable. Append only a private test module to forward/tests.rs in fresh evaluator containers; use the same public forward_request interface on base and reference, real local HTTP upstream and actual OCLA cache. No gold-specific type shim or source-text correctness check.

**Tech Stack:** Existing Rust/Axum/Tokio/reqwest, Python host controls, original task image and existing complete stream capture.

**Spec:** Public Lean issue: cached provider errors replay as200 instead of retaining the original status; [actual failed controls](../reports/2026-11-10-fixed-oracle-diagnostic.md). Detailed design below.

## Design, approaches and ruling

Alternatives: replace the cohort (would abandon the currently frozen pair); drop/relabel expected tests (would weaken/change the old acceptance contract); or preserve all old tests and preregister additional semantic checks before model outputs. Choose the superset. Original F2P label remains recorded as nondiscriminating; its test is still required to pass. A negative control must fail the newly declared specification probes, not a renamed old label. No old verdict is rescored.

Use status cases200,400,429,500 with literal body sentinels. Two identical requests through real forward_request must preserve status/body. The200 case additionally proves cache functionality by observing exactly one real upstream request. Error responses may be safely uncached; no requirement to match the gold caching strategy. Always200, always500, corrupted bodies and globally disabled caching must be rejected. Public fake-upstream traffic is loopback-only, no provider/token/Internet.

Oracle files remain host-private and are installed only after candidate source is frozen. Reuse existing onboarding once; no task/gold/oracle knowledge written into actor memories/prompts. Supplemental paths must be disjoint from gold production paths because live_grader restores test_patch files after applying candidate. Use existing forward/tests.rs plus a new included test file, never forward/mod.rs. A compile-only baseline failure is insufficient.

## Global constraints

- Frozen Lean1168/Obscura421 selection and original dataset bytes/expected arrays immutable.
- New derived registration is explicitly an acceptance superset; every old expected status still required, no drops or post-model tuning.
- Actual base/reference must compile the same oracle and exercise real proxy/cache behavior with literal expected values.
- UID0,4CPU/8GiB,total memory+swap8GiB,capabilities dropped,init reaper,network none for Lean; fresh owned evaluator containers, no shared writable build cache.
- No model/provider request, paid admission, source publication or global change.
- Preserve all setup failures, RED/GREEN, source hashes, controls and exact mutation outcomes.

## Review focus

1. A missing/bypassed cache cannot make all checks pass;200 upstream counter must be1 after2requests.
2. A correct fix that avoids caching errors remains acceptable; no gold-strategy equality assertion.
3. Cold and repeated responses must independently match literal status/body for400/429/500, with a genuine runtime baseline assertion failure.
4. Restoring private test files must not erase the candidate/reference; supplemental paths disjoint from gold production paths, source hashes checked.
5. Task-specific oracle must stay private and only evaluation-side; old full expected population remains required in any new admission, no paid gate from filtered tests.

### Task 1: Real private behavioral RED/GREEN

**Files:** plan-owned ledger and effort_status_oracle.rs; host qualify_spec_oracle.py helper; private generated test patch, no edits to actor source or production grader.
**Interfaces:** Oracle run_case(expected:u16) performs2actual forward_request calls and returns after literal assertions; host register/run modes freeze oracle/helper/source/image hashes before controls.

- [x] Record isolated grader HEAD and existing source hashes; initialize plan ledger and brief.
- [x] Write200positive cache and400/429/500 status/body tests first; static fake-upstream interface, fresh data directory and environment restoration.
- [x] Generate supplemental test-only patch and assert no overlap with the original gold production paths.
- [x] Preregister filtered base/reference controls; run same oracle on fresh base, then fresh reference with complete capture, real inspections/cgroups and cleanup.
- [x] Observe baseline runtime status assertion failures and reference4/4pass; any compile/setup error is invalid setup and must be fixed without task-production changes. Preserve every attempt.

## Task 1 observed result — 2026-11-10

Base compiles and200passes;400/429/500 each fail on the second response becoming200 (133.1s). Identical oracle on known reference:4/4pass (122.1s). Complete streams, registered limits/init/network-none, two actual cgroup/inspect observations, OOM0 and fresh container absence verified; production grader hashes unchanged. Completion command: python3 .superpowers/sdd/2026-11-10-spec-superset-oracle/verify_task1.py. Filtered diagnostic only; original full population and host resource floor remain unqualified, paid_readyfalse.

### Task 2: Integrity and semantic review

**Files:** immutable reviewed oracle/control source snapshot, applied mutation copies, final report/checkpoint.
**Interfaces:** review input contains public issue, oracle/control/registration and observed RED/GREEN, no model results.

- [x] One fresh strongest reviewer applies at least8 meaningful cache/status/body/control mutations and runs the actual relevant tests, reporting observed killed/survived/invalid counts.
- [x] Read surviving actual diffs and logs before one bounded correction pass; replay exact mutants after any correction. Record declined scopes and minors.
- [x] Preserve checked source/patch/complete controls and mutation evidence. No new root gate needed for unchanged product source; disclose prior gate scope and perform native oracle verification required here.
- [x] Bind a new diagnostic checkpoint with paid_readyfalse. Full old-population reference/repeat, unresolved Obscura service/runtime, F6, all-arm/context/controller/accounting remain admission prerequisites.


## Task 2 observed result — 2026-11-10

Independent scoped PASS:8distinct semantic mutants killed,0survived/invalid/timeouts; original base1pass/3fail reproduced, reference4pass and base-derived success-only-cache alternative4pass. Two unchanged-mutant repeats finish source binding and are not additional mutants. No source finding or correction pass needed. Review deviation:17generated bytecode-cache files retained, all90registered files unchanged. Root audit reparsed/requalified13native results/streams and freshly confirmed13container absences; checked374member capsule preserves source/review/control evidence. [Final report and scope rulings](../reports/2026-11-10-spec-superset-oracle.md), [closure](../evidence/spec-superset-oracle-v1-20261110/closure.json). This private4case slice is complete; full original population/reference-repeat, Obscura hermetic dependency,F6,resource floor and all-arm/controller/accounting remain open. paid_readyfalse; no source commit/publication or new experimental model repair.

## Self-review and rulings

Current public forward_request signature and ProxyState/cache constructors were read. The gold production paths include forward/mod.rs, so appending its module declaration there would be erased by RESTORE_SH; test-only forward/tests.rs avoids that seam. Config/data environment restoration and actual upstream observations are part of Task1. The superset is a controller choice under standing delegation, not user-specified oracle details. If wrong, cost is a rejected/new diagnostic contract, never a relabelled original score or paid launch.
