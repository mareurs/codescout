---
id: c60133a0c8103156
kind: plan
status: done
title: Correct reviewed prospective policy evidence boundaries
tags:
- effort-steering
- evaluation
- review-corrections
topic: effort-steering
---

# Prospective Policy Review Corrections Implementation Plan

> **For agentic workers:** Execute inline with superpowers:executing-plans and test-driven-development. This is the bounded correction pass requested by the operator's forwarded review; preserve the reviewed source and frozen studies.

**Goal:** Correct confirmed F1/F2/F4/F5 behavior and verify actual surviving guard mutations without changing the experiment's outcome policy.

**Architecture:** One source-scoped line iterator supplies both compiler and infrastructure classification. Per-step evidence is interpreted independently, with full printed-log context reused when a suffix can be located despite a real UTF-8 replacement at its cut. Explicit mapping validation and executor-compatible optional-rebuild handling keep malformed inputs distinct from functional failures.

**Tech Stack:** Existing Python policy, unittest/pytest, real installed nextest 0.9.150 for an isolated dependency-free capture control. No paid/provider/private-grade calls.

**Spec:** Correction contract below, the forwarded 2026-11-09 review of 23bd63dca, and the existing [policy attribution plan](2026-11-07-policy-diagnostic-attribution.md).

## Correction contract

- Captured libtest and nextest test data cannot become top-level compiler or infrastructure evidence merely because it contains diagnostic text.
- Recognize the real nextest indented output block observed in native-capture/stderr.log; keep the reviewer's older synthetic STDERR-header case separate. Preserve original raw logs and native stdout/stderr hashes.
- Reproduce the actual multilang_control decoder's 4096-byte cut; retain the full capture context when matching its decoded suffix. Do not borrow test diagnostics to explain a failed rebuild.
- Genuine top-level process/storage/permission failure remains invalid; ambiguous compiler provenance remains invalid. Do not turn lack of evidence into success.
- Bad mapping inputs return invalid rather than raising. Missing/null/empty/blank rebuild commands mean no rebuild, matching the executor.
- F3 missing-status/abort treatment remains unchanged and explicitly unresolved for the operator. No dropping or selective rerunning of trials is authorized by this correction. F6 ordinary-linker coverage and F7 shell-encoded signals are recorded limits, outside this bounded production pass.

## Global constraints

Worktree /home/marius/.codex/worktrees/e646/codescout, branch fix/grader-policy-review-20261110, base 23bd63dca161eb491190d877a2207303458e8921. Do not amend preservation/integration commits, add2, the peer branch, any frozen parser, result, policy-v2 receipt or cohort. No main merge or push. Bug records go immediately to the main checkout. Scratch is /home/marius/.cache/codescout-resident/codescout/grader-policy-review-20261110. Test/mutation outcome counts must come from observed full populations; parser/collection errors are not mutation kills.

## Review Focus

1. Headerless tails cut in a multibyte character must use real decoder behavior and preserve complete-log capture scope.
2. Nextest stderr is separate from its JSON print log; actual captured diagnostics and JSON test payloads must not invalidate selected outcomes.
3. Scope must not leak between unrelated steps; a genuine later top-level infrastructure fault must remain visible.
4. Malformed inputs must hit the intended boundary; raw=None and boolean test-step returncodes must not be replaced or preempted by fixture helpers.
5. Surviving guards must be applied before adding their regressions; unknown parser strings, duplicate expected names and full-verdict contradictions require internally coherent inputs that reach the guard.

### Task 1: Reproduce and correct scoped log evidence

**Files:** tools/setup-ab-eval/grade_policy.py; tools/setup-ab-eval/test_grade_policy.py; small characterized capture fixtures under tools/setup-ab-eval/fixtures/multilang/ when needed.
**Interfaces:** Keep compiler_diagnostic, infrastructure_issue, classify_public, step_compiler_failure and adjudicate_private signatures; introduce scoped_lines(log) and per-step context helper if required.

- [x] Reproduce reviewer F1/F2 on unchanged source and run the actual 4096-byte decoder cut. Retain the uninformative first Unicode fixture separately.
- [x] Run an actual nextest JSON-plus command and retain independent stdout/stderr and their hashes; verify its header shape rather than adopting the assumed one.
- [x] Apply the peer's original mutation catalog on fresh base-source copies before changing tests; retain all patterns, logs and classifications.
- [x] Add failing public/private tests for captured libtest and both nextest header shapes, genuine top-level infrastructure controls, separate streams and the real UTF-8 suffix cut; run them RED.
- [x] Implement the smallest shared scoping/context correction; run those tests GREEN, then the complete policy suite.


Observed Task 1: actual initial mutation population is 48, with 31 killed / 17 survived / 0 invalid / 0 not applied, rather than the peer's reported 33/15. Regression RED log retains 15 failed checks; corrected complete policy suite is 35 passed / 52 subtests (task-done replay 10.79s). The shared iterator handles characterized nextest indentation and framed JSON payloads; infrastructure scans independent streams. This does not prove arbitrary unframed/nocapture text has attributable provenance.

### Task 2: Restore fail-closed input compatibility

**Files:** same policy and test files.
**Interfaces:** adjudicate_private continues returning a result mapping; no exception swallowing beyond known malformed evidence.

- [x] Add direct-call malformed raw/record mapping tests (including explicit None), and missing/null/empty/whitespace rebuild cases. Run them RED on the existing boundary.
- [x] Add explicit input validation and match the executor's optional rebuild normalization. Run GREEN and all five related suites.


Observed Task 2: direct malformed-input/optional-rebuild RED is 7 failed checks; corrected five-suite run is 282 passed / 162 subtests in 30.65s. The task-done focused replay is 3 passed / 17 subtests. Explicit None reaches the actual API rather than a default-substituting test helper.

### Task 3: Pin observed mutation gaps and preserve the result

**Files:** policy tests, dated report/evidence and main bug records.
**Interfaces:** mutation receipts distinguish applied/killed/survived/invalid/not-applied with exact transformations and pytest logs.

- [x] Read actual initial survivors before regressions. Pin coherent selected sets, cgroup observations, source gutters, full verdicts, returned recovered statuses, unknown/nondeterministic parsers, returncodes and rebuild normalization.
- [x] Replay actual mutations on fresh copies. Initial original48:31 killed/17 survived. Three-case runner calibration matches. Final59:55 killed/four redundant verdict survivors, zero invalid/not-applied. All11 new mutations killed. N10 was observed surviving before its regression. Four finite-control survivor explanations are in the report; no universal equivalence claim.
- [x] Final five suites:293 passed/180 subtests. Full gate exit0 in549s (script TOTAL548); Python1765 passed/1 skipped/554 subtests; FMT/CLIPPY/LEAN/DEFAULT all0. Setup-ab-eval population is separate; printed exclusions retained. All5823 tracked entries unchanged across final gate.
- [x] Preserve source and checked capsule in f586d35f4b0d41768931648dd1a9a33f1cdd450f, stable patch-id53c4b6e6027c6135823b4fc1b0994c1a719b7301. All232 members and committed source blobs rechecked. Canonical report, README and bug records updated. Old source/capsules remain immutable; no main merge/push, paid call, private grade or historical rewrite. F3/F6/F7 explicitly unchanged. Handback through operator; no socket send.

The [final correction report](../reports/2026-11-10-policy-review-corrections.md) retains actual outcomes, native nextest/linker controls, hash custody and limits. Capsule/final documentation were added after the source gate.

## Self-review and execution rulings

Tasks 1/2/3 share the same policy boundary; execute sequentially. Initial mutation evidence is from immutable base 23bd63dca even if current-source corrections start later. The independent Opus review already supplied the requested fresh whole-change review; this is its single bounded RED/GREEN correction pass, not an invitation to loop reviewers. The user's standing autonomous-execution instruction supplies the implementation method; no new approval gate is inferred for reversible local fixes. F3 is excluded from implementation until an explicit operator decision.
