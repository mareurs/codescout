---
id: 1a50fb0a669bf116
kind: plan
status: active
title: Fresh matched complete-workflow calibration driver
tags:
- effort-steering
- evaluation
---

# Complete-workflow calibration implementation plan

> Implement inline with superpowers:executing-plans and test-driven-development; one independent final review with actually applied mutations. Operator already selected autonomous execution.

**Goal:** Run a fresh low/xhigh diagnostic pair whose public workflow, private acceptance and complete cost are independently observable.

**Architecture:** New tools/hint_authority_workflow/driver.py reuses the verified worker and private grader. A validated optional CLI budget in run.one preserves the existing default. Fresh root, immutable start marker, atomic ledger and composition fingerprint prevent cohort reuse and unknown-cost continuation.

**Tech stack:** Python standard library; existing pinned CLI/native Docker, proxy and public verifier.

**Spec:** Report328a44811943317e and standing operator instructions; implementation choices below are controller decisions under the granted autonomy, not new operator quotations.

## Global constraints

- Same frozen hce-07-build_chunks task, prompt, complete global/project CLAUDE.md, recon skill/onboarding and private oracle; no new onboarding or oracle feedback. Diagnostic task is exposed; no generic transfer claim.
- Two cells:low then xhigh, declaration absent. Public four-command manifest unchanged. Full-workflow deadline1800s, per-check900s, max4continuations; CLI max32turns/output8192 retained.
- New calibration cap USD8, per-arm proxy and CLI ceiling USD4 (operator explicitly allowed a larger allocation). Allocate from existing thread cap USD40/spent31.797419199999999964; old pilot2.57626490000000013/4 remains closed and unmodified. Reserve both arms before first paid call. Unknown cost stops campaign; no automatic retry/resume.
- Verified disk storage>=30GiB, CPU2/init/PID512 unchanged. Private grade occurs only on an isolated copy after the repair loop. Charge known cost before grading, including failed/censored runs. Distinguish completed workflow, private acceptance and context integrity.
- Final content fingerprint includes worker/proxy/preparer, lifecycle, old ledger validators, new driver/tests, public manifest, campaign plan and frozen runtime/input files. Review binds that fingerprint; changed inputs fail before credentials. No merge/push.

## Review focus

Unknown-cost or stale-terminal continuation; missing second-arm reserve; old-spend reset or drift; changes outside legacy fingerprint; grade exception losing known cost. Tests must exercise each through driver execution or admission. Reviewer must apply at least8 candidate mutations privately and report observed killed/survived counts.

## Task 1: Implement and qualify the matched driver

Files:create tools/hint_authority_workflow/driver.py, test_driver.py, README.md; modify tools/hint_authority_pilot/run.py and agent_lifecycle/test_runtime.py only for validated opt-in client_budget_usd.

Interfaces:run.one(label,effort,declaration,scripted,*,campaign_remaining=None,workflow=None,client_budget_usd='0.65'). New budget must be positive<=4, <=paid proxy allowance, and custom value requires workflow. Default legacy/scripted calls retain0.65. Driver check(root) returns composition digest after validating registered plan/history/review. execute_pair(root,check,run_cell,grade_cell) persists known cost before grading and halts on unknown cost or unsafe transport/cleanup. CLI --prepare, --check and --paid are explicit modes; preparation/check never read credential or launch provider.

- [x] Write through-worker budget tests and driver tests for pair reserve, exclusive restart, exact cells, cost reconciliation, pre-grade cost persistence and composition mutation; observe RED.
- [x] Implement validated opt-in budget and minimal driver/fingerprint/ledger; run lifecycle, pilot/completion/resume and new driver suites GREEN. Keep real dependencies at their external boundaries in tests.
- [x] Prepare a new exclusive disk-backed campaign root reusing frozen source/plugin/reference. Independently reconcile all old costs and validate private acceptance controls; record final registration. No paid calls yet.
- [x] Review entire change with applied mutations; file each issue immediately MAIN. Fix Important findings with observed RED/GREEN and recheck actual mutations. Run final actual CLI/native controls with new budget argv and full V development gate; commit exact source paths locally.
- [ ] Bind reviewed final fingerprint, run no-cost preflight, then launch only if complete pair reserve/admission passes. Observe both final receipts/public rounds/private grade and actual cleanups; preserve raw evidence and update reports/trackers/memory. No success claim from a CLI terminal alone.

## Execution ledger and rulings

Use resident scratch outside repo per docs/conventions/resident-scratch-location.md, overriding skill in-tree scratch helper. Existing isolated V worktree is clean at7e57bf8f9. One coupled task: budget argv, driver and tests must land together. No implementation task delegation; final independent review only. MAIN docs/provenance caveat remains; foreign staged bug-triage file untouched.

Ruling: increase only this fresh calibration's CLI/proxy allowance to4USD per arm within8USD allocation; the prior0.65CLI ceiling could censor a continuation. This changes registered runtime limits equally for both arms; old results are historical, not directly comparable completed workflows. Cost if wrong: pair remains diagnostic and can still be censored, all costs retained.

## Cohort withdrawal and next admission

Source c278effb9b committed/gated/reviewed and no-cost admission passed. Paid cohort then observed LOW deadline-censored before controlled public checks; private patch passes, known cost USD0.4249394. Remaining xhigh admission withdrawn deliberately; one-arm cohort preserved under docs/research/data/2026-11-05-complete-workflow-*. No re-run/reset. Last paired objective remains unfulfilled.

Warm reference-cache proof did not qualify the cold total-workflow deadline (issuef458f9840337469e). Next zero-provider cold control runs unchanged four commands at CPU4/memory12GiB/init/PID512, deadline3600/percheck1800, no cache mount. Keep original cohort source/proofs immutable. Before fresh paid admission: finish cold control, repair/qualify background owner ID39785443931155d4 and log retention7aa44115b3456514, register a new cohort and preserve prior spend32.222358599999999964. User permits a larger budget; keep concrete finite allocations and reserves. Generic low/xhigh mechanism remains unproved.

## Recovery implementation — scoped prospective change

Controller allocation: raise the cumulative thread ceiling from40 to60USD under the operator's explicit permission for more. New pair allocation remains8USD/4perarm. Prior32.222358599999999964 includes the aborted LOW; do not reset history. This is a controller-selected finite ceiling, not an exact amount quoted by the operator.

Cold public control completed all four unchanged commands in634.6421497459523s, zero provider cost; independent archive/readback complete. Runtime resources prospectiveCPU4/memory12GiB/init/PID512 and total3600/check1800. No source or gate relaxation.

Recovery work is first staged in /tmp/workflow-recovery-01a15ca9-20261105 while V stays frozen for the cold control. Owner read pin plus log preservation and cleanup pin have actual failing boundary tests followed by GREEN; lifecycle50tests pass in staging. New cleanup gap68180be293c5d09a filed immediately.

Remaining sequence: finish evidence guard tests; update verified resource consumer and public profile; update campaign registration to independently reconcile the aborted cohort and bind cold proof; test failure paths; promote only owned paths after V baseline hashes match; one independent applied-mutation review of this recovery delta, focused real native qualifications, full gate and exact-source commit. Use a new exclusive campaign root, never old aborted root. Last paired objective remains open.

## Recovery execution checkpoint

Exact11source paths committed locally0f7fb1dd69f6807e4a5564c25ce379bf267c7fc5/patch-id31f764c7402839b87c20a0afb29d34ee5ca8ca46. Full gate@bg6d EXITED0, independent review accepted exact finalcomposition55e1053bc4b8592424c398a3deb73ae951a5b84a7e99aaaa739cc31927470711. Native v2 qualification@bg6f EXITED0, reused onboarding/private controls and actualcoldproof bound, no-cost --check passed. Preflight471-file archive hash765ae0f4bbe356fdaa821e3aeff69e6824a42e5bd39ca5dd0e0702428f965b9a. Paidfreshroot launched once@bg73; outcomes/costs awaiting, last paired objective remains open. Source frozen until final evidence preservation; no rerun/reset or new source edits while active. Commit not merged/pushed.
