---
id: '3686b609219ed738'
kind: plan
status: done
title: Registered cold-command budget for prospective effort evidence
tags:
- effort-steering
- implementation-plan
topic: effort-steering
---

# Registered Observation Budget Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline; one final independent review with actually applied mutations. User's standing autonomous delegation covers routine implementation choices. No source commit/publication or scratch deletion.

**Goal:** Remove the observed300-second cold-compilation starvation without weakening isolation, changing old receipts or introducing shared writable caches.

**Architecture:** Add host-selected cargo-v2 with the exact cargo-v1 runtime resource contract and an explicit900-second command maximum included in its profile digest. Validate command timeout/log bounds centrally at executor and pre-actor adapter boundaries. Existing fixture-v1 and cargo-v1 dictionaries/digests and300-second limits remain unchanged.

**Tech Stack:** Python stdlib, existing effort_protocol modules and Docker native controls.

**Spec:** [Cold-build issue](../../../issues/2026-11-10-effort-observation-cold-build-exhausts-budget.md), [parent plan](../evidence-protocol-plan-20261109.md), and the design below.

## Design and approaches

The user delegated mechanism/validation choices and opted out of recurring approval questions. My choice: versioned900-second allowance, informed by the completed480-second public Lean control. Alternatives were a source/toolchain-bound immutable dependency seed copied independently, or continued300-second cold targets. The seed needs new cache invalidation and workspace-artifact checks; retain it as a later measured optimization. Existing300-second cold targets already failed public qualification. No cache implementation in this change. This does not prove mechanism efficacy or paid admission.

The new profile exposes max_command_timeout_s=900 only in cargo-v2; old profile values are byte-semantically unchanged. validate_command_budget(name, timeout_s, max_log_bytes) accepts exact int/float positive finite timeout up to the selected maximum, and exact integer1..67108864 log limit. It raises ProfileViolation with existing unsupported_timeout/unsupported_log_limit reasons. The executor maps these to existing ExecutionInvalid reasons. run_phase validates P's required host budgets before creating phase output or calling actor.execute; R keeps its existing no-limit API, but either supplied field requires both to validate. Actor submissions cannot override host limits. Fresh targets and runtime validation stay intact. Register cargo-v2 explicitly for future controls/pilot and retain old studies; do not relabel old receipts.

## Global constraints

- Public cohort identities fixed to Lean1168/Obscura421; no private/model outcomes read for replacement.
- fixture-v1/cargo-v1 runtime semantics and digests unchanged. New900-second limit must be profile-bound, never a global loosening.
- Read-only source/root, network none,4CPU/8GiB cargo resources, independent fresh targets unchanged. No writable shared caches.
- Unknown/malformed/non-finite/over-limit budgets stop before actor spend or Docker/evidence effects; preserve existing costs and errors.
- No paid actor, merge, push, global settings or destructive cleanup. Preserve all RED/GREEN, review, gate and native evidence; source remains WIP with checked patch under publication hold.

## Review focus

1. New profile's900 boundary is actually honored by executor and adapter;900.001 refused, bool/NaN/infinity not admitted.
2. Old profile digests/300 limits stay unchanged and old receipts cannot be relabelled cargo-v2.
3. Invalid timeout/log budgets cause zero actor calls, zero Docker calls, no phase/command output directory.
4. Host-selected budgets/profile cannot be overridden by a submitted command; inspect actual forwarded command request.
5. Real cargo-v2 command resources, source/log identity and cleanup must be observed, not inferred from a mock or prior standalone runner.

### Task 1: Version and consume the command budget

**Files:** Modify tools/effort_protocol/execution_profiles.py, executor.py, adapter.py. Create tools/effort_protocol/test_command_budget.py; reuse existing profile/executor/adapter tests.

**Interfaces:** validate_command_budget(name: str, timeout_s: int|float, max_log_bytes: int) -> None raises ProfileViolation. Existing execute_observation/run_phase interfaces remain compatible.

- [x] Record baseline digests and run existing profile/request-validation tests before edits.
- [x] Write failing consumer tests for cargo-v2 accepting900,900.001 refusal, old300/300.001 behavior, malformed types and limits. Test invalid host budgets at run_phase with a real minimal scripted actor counter and no output; mock only Docker for fail-before-effect checks.
- [x] Observe RED for actual missing/versioned behavior, not import errors; save full logs.
- [x] Add cargo-v2, central validation and both consuming-boundary checks; avoid unrelated runtime WIP changes.
- [x] Run focused budget/profile/request tests and full protocol suite with an explicit existing Docker fixture image; disclose genuine native-input skips.

### Task 2: Actual qualification and bounded review

**Files:** plan-owned .superpowers ledger/helpers/evidence; canonical MAIN report/issue/plan updates.

- [x] Execute one real public Lean cargo-v2 observation with900-second registration, literal public command/image and full immutable-source/resource/log/cleanup proof; keep the earlier300 timeout and refused900 request unchanged. No retry from functional failure.
- [x] One fresh strong reviewer applies at least8 targeted guard mutations on isolated copies and reports observed killed/survived/invalid counts. Read surviving diffs/logs and make at most one bounded RED/GREEN correction pass; replay exact mutants.
- [x] Run required root gate, verify exact reviewed source, preserve checked source patch plus all controls and logs. Root gate coverage limits and unrelated failures remain explicit.
- [x] Bind the new profile/source/control digest into a new admission checkpoint without changing the frozen public selection. Mark this slice only; whole-campaign/controller/F6/private-oracle/all-arm/paid admission stay open.

## Self-review and rulings

Coverage: profile identity, typed limits, both executor/adapter boundaries, host override, backward compatibility and actual runtime controls are assigned above. Existing isolate cf66 is retained. Standing delegation means no new permission round. No commit or workspace deletion because user preservation/publication constraints override skill defaults. CPU/wall overhead stays measurable; no cheaper efficacy claim follows from this correction.

## Local completion — 2026-11-10

[Final report](../reports/2026-11-10-observation-budget.md) and [new admission checkpoint](../evidence/observation-budget-20261110/registration.json) close this bounded slice only. Final protocol242passed/3skipped/140subtests; root gate exit0 with printed exclusions. Independent10mutants:8killed/2survived; one bounded tests-only correction and exact author replay:10killed/0survived/invalid/timeouts. Actual executor public control completed~480s, compile307s, rc101 with retained22public failures; no model efficacy result. All6 prior WIP byte-identical. Checked1055-member capsule contains tracked/new-file patches and all raw evidence. No commit/publication. Private oracle, variable public failure cause, F6, all-arm native admission and whole campaign/controller remain open.
