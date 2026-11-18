---
id: a828d5f31294730b
kind: plan
status: done
title: Correct phase transport headroom without retuning the repair mechanism
tags:
- effort-steering
- budget-policy
- implementation-plan
topic: effort-steering
---

# Transport Headroom Correction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans. Execute inline in the existing cf66 worktree; preserve all closed registrations and outputs. No implementation or paid-readiness claim is made by this plan.

**Goal:** Obtain an interpretable L/R/P/X pilot by preventing the conservative transport guard from censoring small phases below their economic CLI targets.

**Architecture:** Retain the existing request-pricing guard and the frozen actor mechanism. Version the financial schedule: P/R transport reservations become3/4/3USD, L6USD and X9USD; existing CLI targets stay0.55/1.65/0.55, L2.75 and X5.50USD. This is a finite transport-envelope increase, not a guarantee of future completion. The frozen comparison still scores actual complete-workflow client cost, not reserved dollars.

**Tech stack:** Existing Python campaign, native Claude CLI/CodeScout adapter and independent frozen evaluator.

**Spec:** ../evidence-protocol-design-20261109.md plus this narrow prospective financial amendment. This plan is the controller's call under the operator's standing autonomous delegation and200USDseries ceiling, not a newly attributed user choice.

## Evidence and approaches

Actual old stop: issue7e84dd9c59e9a780. R/investigate needed0.77591415USDconservative total against0.60USD, while final client cost was0.1636965USD. OOM0/resource_oktrue. Prior pilot remains stopped2/8 with no grades, known new client charges0.3678025USD. Full closed outputs are preserved in evidence/paid-budget-stop-20261111/evidence.tar.gz,503072529bytes,SHA18c1665382759b1e8cb27c7663aa6da63f4b9c40a1c43588b0c54d2afe82a014;19329payloads reopened,17104regular payloads, originals unchanged.

Considered: (1) relax the conservative pricing formula; rejected because it weakens a financial safety bound and needs new price/count calibration. (2) add a shared campaign transport pool separate from phase caps; semantically cleaner, but requires a broader state/accounting contract change. (3) increase the existing frozen per-phase transport envelopes while keeping actor CLI targets and actual-cost scoring; chosen as the smallest bounded correction. Source scouting found this is NOT configuration-only: campaign.attempt_schedule and paid_admission.verify_accounting hardcode the old schedule/38USD allocation. The earlier commentary suggesting a configuration-only change was corrected.

Actual offline replay: transport-headroom-replay-v1/result.json replays all21recorded request/count pairs:17have observed provider usage,4are denied retry requests with no response. Proposed envelopes admit their recorded bounds; this does not invent4future responses. An oversized400000token control remains denied at3USD. Conditional headroom at1.1times CLI target and max8192output: investigate/challenge261407inputtokens, repair237543, L327316, X324475. These are algebraic conditions, not measured future context limits.

## Global constraints

- Standing200USDseries ceiling. New reservation70actor+4setup+4contingency=78USD. Reconcile all intervening final native costs and preserve historical4USDhold before admission; do not turn missing cost into zero.
- Frozen issue text, mandatory global/project rules/onboarding/Recon, selected pair, arm order, effort, phase prompts, public commands, evaluator, resources and scientific screen remain unchanged.
- All8new attempts form a new separately registered diagnostic version. Do not splice the already completed L into it; this avoids a carry algorithm and outcome-based selection. Report the old0.3678025USD operational overhead separately and in series totals.
- Do not privately grade old candidates to choose the new settings. No same-task prompt tuning. Reusing these already exposed tasks only diagnoses runtime/financial policy; external validation still needs an outcome-blind held-out population.
- Existing reports/qualified controls remain immutable. Changed source or schedule invalidates prior exact bindings; do not label a new financial schedule administratively equivalent.
- New bugs immediately MAIN docs/issues. No commit, merge, push, global configuration change or cleanup.

## Review focus

1. Actor effort/CLI caps or invocation/turn limits accidentally increase with transport reservation: assert exact original targets and order in schedule tests.
2. Accounting accepts a declared allocation inconsistent with actual schedule/setup/contingency: reject mismatch and ceiling crossing.
3. A larger envelope erases the real oversized-request/unknown-usage guards: execute actual Budget.reserve controls, preserve pending-cost rejection.
4. Old registration/runtime qualification is silently reused under new schedule/source: consuming preflight must reject old exact bindings.
5. Native budget abort is reclassified as success or a missing candidate becomes a baseline substitute: preserve the stopped record and existing negative tests; do not weaken capture checks.

## Task 1: Version the frozen financial policy and pin the actual denial

**Files:** tools/effort_protocol/campaign.py (attempt_schedule), paid_admission.py (verify_accounting), test_campaign.py, test_paid_admission.py, test_runner.py. Inspect actual bodies/callers before edits. The exact policy revision must be explicit in new registration/provenance; do not mutate old artifacts.

- [x] Read actual stop, execute real guard replay, inspect source call chain and file issue.
- [x] Replay observed request/count/usage inputs against proposed finite headroom; retain negative guard control and no-provider evidence.
- [x] Add regression tests first: exact mirrored schedule, P/R3/4/3and10total, L6/X9, actor70+setup8=78, unchanged CLI targets and oversized/pending request denial. Observe meaningful failures before production edits.
- [x] Implement the smallest financial-policy revision; update accounting's explicit allocation and expected tests without broad refactoring or weakened identity/capture rules. Confirm previous hardcoded38/phase values fail the new regression.
- [x] Run direct protocol/lifecycle checks and current project gate as required for changed source; preserve all failures/exclusions. Reopen exact source manifest after checks. Final fresh v4:418protocol(3skips),108lifecycle,1392evaluator and all5gate lanes pass. All192before/after/current source hashes match. Verification SHA0b70d4e2d618a9f123453e29764b9688c38f9f15b2ef35c6aef2dff03301b0ab; isolated local OpenSSL3.5.1environment qualified, host unchanged.

## Task 2: Admit and measure one bounded revised diagnostic

- [x] Reconcile the two terminal receipts into current historical accounting and recheck the200USDceiling including all old holds/reservations. Current consuming32-receipt proof verifies64.768895999999998654known+4unresolved hold+78new allocation=146.768895999999998654USDwithin200. Historical opening-balance and invoice/root/review/embedding-cost limits remain.
- [x] Build a fresh zero-forward qualification on the exact new financial schedule/source. Reuse unchanged public onboarding/index/private package, not an LLM author call or a fabricated old qualification. Do not repeat unchanged private compiler baseline/reference controls. Actual closed8workflows/16native/8private/provider0; allsynthetic candidates valid failures. Six consuming proof groups pass at exacttarget29ecc6ff9213b1116a096131c8b4d231faea87b3619d04de7f041bc32188f2e4; independent final review remains pending.
- [x] Obtain a focused independent review of the changed financial admission with actually applied mutations. Reuse old review only as explicitly dated prior evidence; a changed protected schedule requires a truthful fresh target/report. No repeated broad review/correction loop. Fresh report SHA826ef9fbded9af5f798849ecd18235a203d7f936c5584229e92705db52793166 binds target29ecc6ff9213b1116a096131c8b4d231faea87b3619d04de7f041bc32188f2e4. Original12mutants10killed/2survived retained; one81-test correction kills both exact survivors. Same reviewer consumed all6groups and old-binding negative controls; no new important finding. Seventh consuming proof remains required.
- [x] Freeze new registration/source/qualified receipts/final checks/review proof and run actual typed-Path seven-group preflight. Only actual ready permits ONE new8attempt run within remaining approved ceiling. No retry of old launch paths. Finalizer bg5f exited0, ready/paid_ready true; one new reviewed headroom-v3campaign launched15:48UTC(bg62). No closed real attempt or scientific result at this checkpoint. Closed-admission custody bg63runs independently; its first path failure is retained as issueafaa9e16642a8da9.
- [x] After actor/cost batch closure, run private grades and publish paired quality/protocol/full-workflow cost plus limits. All8grades valid, no missing statuses; L/R/P hidden acceptance2/2each, X1/2with the retained mutation-cutoff caveat. P protocols0/2closed; P1.65058059999999998USD versus X1.5943932999999999USD. Fixed advance screen NO-GO; no generic-quality claim from n2. Full batch4.60646289999999987USD including all incomplete charges. Primary report: ../reports/2026-11-11-closed-generic-repair-pilot.md; exact analysis SHA7caf0b9720a890203062ffa931ee5721bc5934beb6f5d3ce38abbd38b533ff08. Paid-custody archival and the broader research goal remain separate follow-up work.

## Self-review

Current verified reconciliation reopens32native client receipts and retains the original4USDhold. Known client charges64.768895999999998654USD; proposed78USDallocation brings commitment to146.768895999999998654USD. Consuming verify_accounting passed. Closed predecessor unspent reservations are explicitly retired in a new audit, never removed from its raw ledger. Earlier opening charges remain a recorded balance, not a fresh full invoice rescan. Full verification v1 failed416tests/1stale42USDexpectation/3skips; corrected expectation82USD preserves the4USDhold, and v2 is running. Fresh exact-schedule native qualification is also running, not yet claimed passed.

The immediate failure is reproduced by actual production guard arithmetic. The chosen revision preserves the repair mechanism and avoids a new shared-pool architecture. The outstanding risk is unobserved future context or functional workflow failure under unchanged CLI targets; replay is not proof of completion. After the offline diagnosis, two production financial-policy functions and three test files were changed. The real-request admission and78USDaccounting regressions were observed RED before correction; targeted campaign23tests and paid-admission30tests now pass. Full current-source verification has been started, and current paid admission is FALSE because old exact bindings no longer match. No new paid invocation has been launched.
