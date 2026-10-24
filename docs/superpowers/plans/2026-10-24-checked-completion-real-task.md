# Checked completion on a real repository issue

> **For agentic workers:** Use superpowers:executing-plans for this local trial. The user delegated autonomous execution; no design approval round is required.

**Goal:** Fix issue 70f4cad102afb762 with fixed-low implementation, mandatory acceptance and at most one repair; measure the surrounding work rather than quote only implementation cost.

**Architecture:** Preserve committed repository base 32d0c713de272738ae7ccba9e1e4fccb004600fe in an isolated worktree. Freeze subprocess tests over hand-authored SQLite records before implementation. Reuse the existing paid runner and Stop controller from research commit d9d4283aeaf25bc0e6e37481fb1bfc476c9f0077. Independently restore/check owned tests and review the extracted change.

**Tech stack:** Python standard library, SQLite, pinned Claude Code/Sonnet 5.5, existing isolated Docker runner.

## Contract and scope

Errors mean exactly hard error and recoverable_error. Include both in totals, coverage, repeats, recovery, runs, null-detail, calibration and SQL split reports. Preserve success/unknown rows in sequences and call denominators, session boundaries, date and clean-build filtering. Correct the stale help text. Unknown outcomes are not silently classified as errors. This task does not add a hard-only flag or a calibrated warning threshold; those were suggestions in the issue, not necessary for its correction.

The implementer sees the complete repository snapshot, issue behavior and visible acceptance tests, with native file tools in the container. The outer researcher has read the issue and source. This is a single-task feasibility trial, not blinded efficacy evidence or a comparison against ordinary low effort.

## Global constraints

- One paid implementation conversation, fixed low effort, max one Stop repair, $1.25 client soft cap; no paid restart.
- Previous actual experiment spend $2.9529524; cumulative client allowance $10.
- No merge, push, global hooks or global configuration changes.
- Trusted checks/controller live outside agent-editable source; caller rejects absent, failed or error verdicts.
- Record preparation, implementation, repair, validation and review durations; collect available orchestrator/reviewer usage, and leave unknown prices explicit.

## Review focus

All report entry points must use the same exact error population. A fix confined to totals or Python predicates misses SQL. Filtering the input rows to errors destroys denominators and recovery. Treating every nonsuccess as an error overcounts unknown outcomes. A change must preserve hard-only and success-only corpora.

### Task 1: Freeze and calibrate acceptance

**Files:** tests/test_friction_probe.py; private runtime registration/control logs.
**Interfaces:** Produces trusted subprocess acceptance command and frozen source hashes.

- [x] Run the tests on unchanged source and inspect the intended failures.
- [x] Verify a separate mechanical full-population control passes, without changing candidate source.
- [x] Apply partial fixes (totals-only, Python-only, SQL-only, broad nonsuccess, error-prefilter) and observe test failure.
- [x] Freeze exact image, prompt, tests, controller and accounting inputs before spending.

### Task 2: Run one checked-low implementation

**Files:** scripts/friction-probe.py; private runner outputs.
**Interfaces:** Consumes frozen acceptance command; produces candidate patch, session verdict and spend row.

- [x] Run one fresh fixed-low conversation through the established budget/accounting driver.
- [x] Require the session-matching final hook verdict and independently rerun trusted checks.
- [x] Preserve failures and any repair events; do not silently restart.

### Task 3: Independently verify and hand off

**Files:** tests/test_friction_probe.py; issue status/provenance; real-task report.
**Interfaces:** Consumes extracted source and complete run evidence.

- [x] Fresh reviewer applies candidate mutations to each load-bearing report path and reports observed outcomes.
- [x] Run the required repository gate and reconcile all known model-run spend.
- [x] Record orchestration/review usage and unpriced costs, commit only owned paths, retain the worktree.

## Execution ledger

Pre-flight: no shared implementation interfaces. Acceptance output is consumed by both the visible Stop hook and independent grading. Tests and controller must be immutable in the agent container; the independent host check restores the frozen test file. User-authorized isolation and autonomy supersede interactive skill handoff questions.

Observed: 13 frozen tests fail 11 on base, pass a separate full-population control, and reject five partial-fix controls. One paid low attempt passes independent grading for $0.0407358, first Stop passed, zero repairs. All 32 historical/current payments reconcile to $2.9936882 at seven decimal places.

Review found one genuine survivor among 16 applied site mutations. The saved unknown-next-call mutation was reproduced before adding a matching-signature regression case. That new test is RED against the preserved mutant and GREEN against unchanged production code; all 16 saved mutations are assertion-killed by the 14-test suite. No second model review or paid rerun was needed.

The full Python light lane passes after the test-only addition (1,287 passed, 424 subtests passed). The full repository gate passed. Its Python lane ran with the original 13-test version; production source did not change. No Rust rerun is needed for the test-only addition because the complete Python lane was rerun separately.

Review dispositions: parent retains responsibility for the full gate, session-matching hook evidence and accounting. No causal quality-gain claim is made because no ordinary-low control ran. Full dollar cost remains unavailable because orchestrator/reviewer/infrastructure rates are unknown; observed tokens and elapsed intervals are recorded. Historical issues outside this outcome-population fix remain outside the deliverable. No minor findings were deferred.
