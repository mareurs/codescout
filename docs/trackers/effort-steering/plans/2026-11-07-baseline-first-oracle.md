---
id: ad278011e5b097f9
kind: plan
status: done
title: 'Baseline-only contract authors before candidate access: one low/xhigh pair'
tags:
- effort-steering
- evaluation-integrity
---

# Baseline-first oracle implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline under the operator's standing autonomous execution instruction; no per-task subagent dispatch.

**Goal:** Test whether independently frozen baseline-only contracts can reject the saved wrong repair while retaining the qualified reference, with a narrow low/xhigh effort comparison.

**Architecture:** Reuse the clean qualified adapter worktree and resident legacy-negotiation/capture helpers. A new preparer creates two baseline-only author states. The controller closes and freezes both outputs before evaluating any wrong/reference patch.

**Tech Stack:** Python 3 standard library, pinned Claude CLI 2.1.309/Sonnet 5.5, Docker, native Codescout, offline Rust tests.

**Spec:** [Baseline-first design](../../../superpowers/specs/2026-11-07-baseline-first-oracle-design.md), c40495db60f87d27.

## Global constraints

Resident root: /home/marius/.cache/codescout-resident/codescout/baseline-first-oracle-20261107. Immutable source worktree: /home/marius/.codex/worktrees/ca35/codescout, fe8bb4b34f5e7912f9b16c4c0bf1ff73a3a10a23. Baseline image sha256:3767f31b9b1a3288ad7132740dcff712737d5e855029079a4171a8065a2040ef. Reuse source-only onboarding and exact original project/global guidance. Candidate image 1434a21e and all proposal/observation/private mounts forbidden to authors. No global configuration, product source, old cohort or pending grader edits. No merge/push.

Fixed author order low then xhigh, one draw each, eight continuation maximum, CLI cap USD 3/reserve USD 4 each. Prior known USD 62.410596299999999054 plus original unresolved USD 4 hold; standing ceiling USD 200. Actual request effort must be inspected. Native 32 GiB/4 CPU/network none/exec 4 GiB tmpfs; host admission 24 GiB for three samples and continuous 16 GiB/two seconds. Offline cells 8 GiB/4 CPU/host 24 GiB. Reuse already-green controlled source gate and qualified legacy compatibility; do not repeat gate/onboarding/old native controls.

## Review focus

- A proposal path/image or observation mount accidentally inherited from the copied preparer must fail admission before credentials/provider access.
- Cached state must be a fresh independent source-only clone, never the previous verifier's writable post-run knowledge.
- Both outputs must close and freeze before any reference evaluation; no eager per-arm grading or output replacement.
- Production or existing-test edits, undeclared test functions, empty detectors and unresolvable clauses must be classified explicitly; syntax success is not a correct oracle.
- Cost finalization must precede later audit assertions; no duplicate attempts on an exception or cumulative-turn double counting.

## Task 1: Bound baseline-only author preparation
Latest: both real author states are prepared, not started. Original source-only manifest was physically cloned twice and every registered payload is a separate inode across arms. Actual baseline image check confirms empty tracked diff at original public HEAD, no /proposal, no contract file or new module. Qualified external.validate_prepared passes both with reserve USD 4/cap USD 3 and no credential read/provider call. Prompt SHA256 8f183e2168957704884af0fde67d6e559738dfd0cd9e46292695b16e5273a489. Capture and complete-pair freezing fixtures bring the suite to 22 passing tests; independent review is active before admission. No scientific outcome yet. Earlier checkpoint below is retained as construction history.
Checkpoint: new resident baseline_pair.py and test_baseline_pair.py contain only the visibility/order guard layer so far. Observed no-op RED: nine assertion failures out of ten tests; implemented GREEN: ten tests pass. Five isolated actual source mutations (image, mount inventory, volume alias, prompt exclusions and eager evaluation) are all killed, zero survivors; source-bound barrier-check.json and raw mutant logs retained. This does NOT qualify the real image/mount boundary or complete preparation/adapter/evaluator integration. Those remaining steps are open; no prepared author, provider call or reference evaluation has started. Existing ca35 source worktree was rechecked clean at the registered fe8bb4b34 HEAD; no source gate rerun.

Files in resident root: create baseline_pair.py and test_baseline_pair.py; copy qualified runtime_compat.py and supervision_headroom.py without changing their source. Consume fresh original source-only snapshot manifest and original public prepared prompt. Produce independent low/xhigh prepared directories, oracle_contract.json output schema and a registration containing all immutable source/input hashes.

- [x] Write failing fixtures for admission rejecting the wrong image, proposal/observation mounts or prompt spill, shared writable state and premature evaluation.
- [x] Observe failure before implementing preparation/validation. Read actual existing prepare_judge.main, run_verifier_recovery.main, evaluator freeze/cell and runtime materialize contracts before copying.
- [x] Implement baseline-only protocol/preparation and validate exact image/mount/prompt boundaries. Record any setup-only tracked diff independently before authorship; reject actor production/configuration/existing-body changes on capture.
- [x] Run fixtures green and actually mutate candidate visibility and freeze ordering guards; report observed killed/surviving counts. Do not infer enforcement from a prompt phrase.

## Task 2: One fixed pair, outputs frozen before grading
Actual execution checkpoint: job /tmp/codescout-bg/2674124-d9ea3495-bg_00000051.json is running; low has its started marker, no closed/frozen receipt yet; xhigh has not started. Neither both-frozen manifest nor pair stop/results exists. This is execution state only, not a model-quality or final-cost result. Do not duplicate this controller.
Pre-admission review/fix pass closed. Original independent review rejected four defects plus coverage gaps; all were filed immediately in MAIN. Actual controller/preparer fixtures and capture regressions now give 36 passing tests. Final applied mutations: 20, 19 killed, one explicitly redundant surviving second-visibility-check mutation. review-admission.json records the original review verdict, parent fix verification, final source hashes and this ruling; it does not claim independent re-review. Original pair-registration.json and pre-fix source preserved; pair-registration-v2.json rebinds only two corrected source hashes. Public prompt, image, guidance and cloned context bytes are unchanged. No paid author started before this close-out.

Interface: baseline_pair.py prepares states and then runs the already qualified author adapter sequentially, writing per-author attempt barriers/cost receipts/frozen captures and an all-authors-frozen manifest. No grading function receives a reference path until that manifest is complete. Consume only registered baseline inputs; author output includes declared contrast_ tests and clause justification.

- [x] Freeze registration before the bounded host-resource admission wait. Recheck source/input/visibility guards and remaining reserved budget before either author.
- [x] Execute exactly low then xhigh. Close background delivery, capture, runtime and cleanup for each; reconcile final cost once even if a later guard fails. Stop infrastructure-invalid without a model retry.
- [x] Freeze both complete captured outputs/contracts/test sets; confirm actual request model/effort and no private/candidate input mounts. Only then authorize the offline evaluator. Record abstention distinctly.

## Task 3: Differential matrix and decision
### Final measured close-out — 2026-11-08

All final xhigh cells completed in offline-single-job-2 and independently audited raw logs agree with the result JSON. Baseline fail/fail/pass/pass; wrong repair pass/pass/pass/pass; reference pass/pass/pass/pass. Low's previously valid fail/fail/pass, all-pass, all-pass matrix is unchanged. Both authors retain the reference but fail wrong-repair rejection: measured NO-GO on this exposed task, not a population or general effort-equivalence claim. Final report bf3e845faf0b862a and docs/research/data/2026-11-08-baseline-first-xhigh-audit.json record evidence. The scientific archive preserves 62 allowlisted payloads plus manifest, all reopened/hash-verified. Both authors, old invalid cell and wait-only attempts are retained. No active job; no new provider calls; no merge/push. The resource-block paragraphs below are historical observations, superseded by this completed matrix. Next candidate is explicit uncertainty handling, requiring separate registration before any paid run.
HISTORICAL RESOURCE BLOCK: job54 also closed after 907 seconds before creating a native test cell: 88 saved host samples, 8.764–26.451 GiB, last 19.945 GiB, below its 28 GiB launch floor. No extra provider call, compilation or scientific outcome occurred. Read-only own/shared-resource checks found no safely reclaimable identified own allocation sufficient for this headroom; no peer process or evidence was removed. Both bounded wait artifacts and the original xhigh resource-invalid cell remain unchanged. No further retry is running. Resume only the frozen xhigh offline cells when the registered headroom is available; do not repeat authors, low cells, source gate or onboarding.
Continuation52 closed after 901 seconds without creating any evaluator cell: 90 admission samples, available host memory 13.286–31.610 GiB, below the new 32 GiB launch requirement throughout. Zero new provider/native test calls. The stopped artifact is retained.

A distinct model-free resource amendment serializes Cargo compilation via CARGO_BUILD_JOBS=1 for all three xhigh evaluator variants. It keeps the registered 8 GiB/4 CPU hard profile, commands/test bytes and 24 GiB continuous stop floor, with three prelaunch samples at 28 GiB. Actual container Config.Env is independently captured; a missing environment proof invalidates the cell after normal owned cleanup. Wrapper fixture tests first failed against a no-op, then passed; full resident suite has 39 tests, and three applied wrapper mutations are all killed. New output root offline-single-job-1, no overwrite of old invalid/wait-only outcomes, no model/author retry. This is a diagnostic compile-scheduling amendment, not a matched wall-time comparison with default Cargo concurrency.
Job51 closed after both valid author workflows/frozen outputs. Final costs: low USD 0.2914831, xhigh USD 1.4046817999999996; known series USD 64.106761199999998654 plus original unresolved USD 4 hold. Low has three valid cells: baseline fail/fail/pass, wrong proposal pass/pass/pass, reference pass/pass/pass. It retains the reference but does not reject the wrong proposal. Xhigh baseline build passed, but test compilation was terminated when sampled host available memory fell to 22.375 GiB below the registered 24 GiB floor; zero observed OOM kills, no complete named assertions, owned absence freshly checked. This cell is infrastructure-invalid, not xhigh quality failure.

Model-free offline continuation is registered separately at offline-continuation-1: repeat only interrupted xhigh baseline and run previously unstarted wrong/reference, with the exact frozen patch, source/parser/variants, 8 GiB/4 CPU/native profile and 24 GiB continuous floor. Prelaunch admission is tightened to three samples at 32 GiB to leave room for the 8 GiB cap; no runtime floor is lowered. Original invalid and all valid low cells stay unchanged. No author/model retry or additional provider calls.

Reuse reviewed evaluate_v3.cell with each frozen new test patch and declared named assertions; source and parser are bound before use. Produce new per-arm baseline/wrong/reference cells in separate folders; no actor feedback or test edits after freeze.

- [x] Execute all named assertions on each of the three variants with resource, log-hash and cleanup evidence. Stop infrastructure-invalid separately from functional failures.
- [x] Independently audit raw named statuses, requirement/preservation clause membership, successful builds, final samples, input bindings and fresh owned absence. No native regrade merely for the audit.
- [x] Report the exact low/xhigh matrices and cost. A useful detector needs qualified-reference retention, wrong-repair rejection via justified preservation and a baseline-to-reference requirement distinction. No aggregate 1/2 or population claim from this exposed task.
- [x] Preserve compact evidence, update issues/memories/roadmap and choose transfer versus no-go. Existing commit-attribution blocker remains separate; no guard bypass or publish.

## Self-review and execution scope

Design requirements map to the three tasks. New code concerns visibility/order/capture binding; adapter lifecycle, cost reconciliation, monitoring and native test execution are reused. Reviews must test the new guards rather than repeating completed source/runtime qualifications. The operator already delegated the how and autonomous progress; this plan records the executor's choice, not new operator approval. No paid author has started at plan creation.
