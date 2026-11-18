---
id: '40d2b6ca2ea35b68'
kind: plan
status: done
title: Qualify prospective ordinary-linker failure classification
tags:
- effort-steering
- implementation-plan
topic: effort-steering
---

# Prospective Ordinary-Linker Classification Implementation Plan

> **For agentic workers:** use superpowers:executing-plans inline. Standing operator autonomy opts out of recurring design/execution approval. Keep the existing isolated grader-stream-provenance worktree; preserve its entire current source WIP before edits. No source commit under publication hold. One strongest final reviewer with actual guard mutations; one bounded correction pass, no re-review loop.

**Goal:** Close F6 for new complete-stream grading while preserving historical tail-only replay and existing invalid-measurement protections.
**Architecture:** Recognize a canonical ordinary Rust linker diagnostic only when the prospective entry point already qualified complete streams and runner provenance. Existing selected-test semantics and infrastructure precedence remain. New prospective policy identity semantic-stream-contract-v2.
**Tech Stack:** Python stdlib unittest, existing capture/stream/evaluator modules, two pinned local Rust images.
**Spec:** [short design](../designs/2026-11-10-prospective-linker-classification.md), issue2f8500976a76db35 and parent evidence-protocol-plan-20261109.md.

## Global constraints

- Existing grade_policy.adjudicate_private and compiler_diagnostic legacy behavior unchanged; no old grade/result/log/capsule rewritten.
- Positive ordinary-linker evidence only on a failed command, after complete-stream/runner qualification. Successful step or arbitrary payload cannot manufacture failure.
- Fatal toolchain/storage/temporary-execution evidence still measurement_invalid; preserve physical-source/runtime/stream limits and F3/F7 residuals.
- No model/provider/author/onboarding call, new task-specific policy branch, private oracle/gold change, merge/push/global config or old-resource deletion.
- Native controls use the exact two frozen image IDs, networknone, readonlyroot, UID1000,4CPU/8GiB/swap8/init, capdropALL,pids4096, owned writable tmpfs and checked closure. Preserve failed controls too.
- Current historical known64.401093499999998654USD plus unresolved4USDhold under200USDceiling. Monetary cost unknown stays unknown.

## Review focus

1. A linker-looking string inside complete nextest JSON/test output is data, not compiler evidence.
2. An ordinary linker error after unstructured captured output remains ambiguous, never confidently functional.
3. Toolchain fatal signals or storage faults accompanying ordinary linker headers win as infrastructure; no selective exclusion of candidate crashes.
4. Nonmatching/exit-zero/header-substring/source-excerpt text cannot produce compile_failure; nonzero step required.
5. Historical replay and policy-version binding cannot silently acquire the new semantics.

### Task 1: Prospective linker classifier and consuming tests

**Files:** modify tools/setup-ab-eval/grade_policy.py and test_stream_policy.py; add narrowly scoped fixtures/tests only if required.
**Interfaces:** linker_diagnostic(text:str)->str|None recognizes exact top-level ordinary linker evidence using existing scoped_lines; compiler(step) in _adjudicate calls it only with qualified diagnostics. adjudicate_streams emits policy_version semantic-stream-contract-v2. Existing call signatures and legacy recognizer unchanged.

- [x] Preserve baseline source/diff/hashes; verify current affected suites and isolated branch identity.
  Run python3 -B -m unittest discover -s tools/setup-ab-eval -p test_stream_policy.py -v. Expected: current suite green; this is baseline, not proof F6 works.
- [x] Add consuming tests for failed rebuild/test ordinary link, retained selected status/missing status, successful-step refusal, captured linker words, fatal/storage precedence, malformed headers/ambiguous framing, legacy invalid behavior and v2 identity. Reuse real complete captures and existing public parser fixtures.
  Run same focused command. Expected RED on ordinary-link classification/version before production edits; retain exact output.
- [x] Implement prospective-only recognition and identity bump. No historical helper widening.
  Run same focused command and python3 -B -m unittest discover -s tools/setup-ab-eval -p test_grade_policy.py -v. Expected: all green, historic test unchanged.

### Task 2: Native compiler controls, review and checked custody

**Files:** owned model-free compiler helper and output/registration under this plan’s scratch; report and checked capsule under this tracker. Product code from Task1 only.
**Interfaces:** each control writes exact image/argv/rustc version/full stdout/stderr/state/cleanup evidence and feeds those bytes through the real prospective capture/policy entry point. No task/candidate execution or actual expected benchmark result changes.

- [x] Generate dependency-free undefined-symbol and valid-link controls on BOTH exact task images; bind helper/source/runtime before execution. Feed failed compiler output into separate synthetic selection receipts through complete-stream contract. Expect native failed-link nonzero plus prospective valid compile_failure; valid link has no false compiler evidence. Preserve native fatal/storage negative controls without intentionally exhausting host resources.
- [x] Run all tools/setup-ab-eval test_*.py suites and required ./scripts/gate.sh --check in the isolated worktree. Expected: green, every skip/exclusion/red disclosed; direct tools suite is required because gate does not include it.
- [x] Seal final source WIP against the baseline, then ONE fresh strongest reviewer applies at least8load-bearing guard mutations on isolated copies, runs actual tests and reports observed killed/survived/invalid counts. Read survivor diffs/logs before fixes. Critical/Important get one RED/GREEN correction and affected whole-suite replay; Minor/declined scope get explicit rulings.
- [x] Preserve source/diffs/native control inputs/streams/review/mutants/tests/gate with manifest and actual archive reopen. Update F6 issue/report/README/parent admission checkpoint; paid_readyfalse. No source commit or premature parent-goal completion.

## Self-review

Actual code scout: _adjudicate receives qualified diagnostic texts only from adjudicate_streams; legacy adjudicate_private calls the same helper without diagnostics. Infrastructure scan precedes compile classification. Existing rebuild records failed status without aborting subsequent tests/prints. Only one test pins the old prospective version; update its assertion as part of explicit v2 behavior. The new classifier must use existing scoping and resist inherited source-gutter text. Native tests characterize compiler bytes on actual images; they do not regrade benchmark candidates or establish efficacy.

## Completion — 2026-11-10

Both tasks complete locally. [Final report](../reports/2026-11-10-prospective-linker-classification.md) retains native controls, original independent review (10 killed / 2 survived), one bounded correction, both exact survivor replays now killed, final 73-policy and 1,392-evaluator test passes, green final gate and independently reopened 1,949-payload / 99-source closure. Three historical helper bodies remain byte-equal. Source is unpublished WIP, no paid admission or efficacy claim. F3/F7 and parent controller/admission remain open.
