---
id: '17b2b7d9c9865040'
kind: report
status: active
title: 'Fixed-task oracle diagnostics: baseline mismatch and runtime dependencies'
tags:
- effort-steering
- evaluation
- oracle-admission
topic: effort-steering
---

# Fixed-task oracle diagnostics

## Terminal closure — 2026-11-10 11:39 UTC

All six V2 controls completed with no setup error and no missing selected statuses; reference/repeat maps agree for both tasks. **Neither task is admitted.** Lean's soleF2P passes in base, reference and repeat; the same reapingP2P fails in allthree. Obscura fixes bothF2P under reference/repeat but the same three networkP2P fail. [Final closure](../evidence/fixed-oracle-diagnostic-v2-20261110/closure.json) records every predicate and outcome. All six control containers were removed and freshly confirmed absent; grader sources match preregistered hashes.

[Checked capsule](../evidence/fixed-oracle-diagnostic-v2-20261110/verification.tar.gz):2890464bytes,139regularmembers,SHA-256 2b8f12d6c5e54ec6e39ed7b26c03c25498ec032e3bcf368f3f6d8035159764b0. Every member reopened and compared byte-for-byte. Includes all V1/V2 controls, statuses/full captures/runtime snapshots, exact grader source, helper scripts and both init probes. The416520576-byte binary remains at its recorded original path, excluded from compact capsule; its digest is preserved. No source commit/publication, model arm run or cost reconciliation.

## Current result — 2026-11-10

**Paid admission is blocked by observed control defects.** No experimental model/provider repair was run. Frozen tasks and expected test arrays remain unchanged; no tests were dropped and no task was replaced after results.

[Initial diagnostic registration](../evidence/fixed-oracle-diagnostic-20261110/registration.json) and [explicit setup-corrected registration](../evidence/fixed-oracle-diagnostic-v2-20261110/registration.json) precede their controls. V1's six setup-invalid attempts remain recorded. V2 controls require every original F2P/P2P and use fresh private containers, UID0,4CPU/8GiB,total memory+swap8GiB, network none, capabilities dropped. Writable private root is required to apply patches; actor/public observation profiles are unchanged. This diagnostic runtime does not yet establish whole-campaign parity or host resource-floor qualification.

| Task | Base | Reference | Reference-repeat | Admission finding |
|---|---|---|---|---|
| Obscura421 | Both2F2P fail;3P2P fail;0missing | Both2F2P pass; same3P2P fail;0missing | Same selected status map as reference | External-service P2P incompatible with network-none runtime |
| Lean1168 | SoleF2P already pass;1P2P fail;0missing | Same selected status map as base | Same selected status map as reference | Declared F2P does not discriminate base/reference; process-reaping runtime defect |

Obscura has331expected tests (2F2P+329P2P); Lean8587 (1F2P+8586P2P). These are parser/status-set measurements for the entire original selected population, not summed nested-suite counts or an arm success rate.

## Obscura: intended transition works, environment prevents acceptance

The recorded reference changes both structured-clone F2P from fail to pass. All expected statuses are present. The same three selected MCP-client P2P fail on base, reference and repeat: navigation/snapshot, title evaluation and selector wait.

All three actual source bodies navigate to https://example.com. The first captured event explicitly reports a network error; the other assertions receive an empty title or time out on h1. Registered network none excludes that dependency. Reference/repeat selected-status maps are identical. See [issue](../../../issues/2026-11-10-obscura-reference-network-dependent-p2p.md). A hermetic-service or controlled-egress positive control remains unqualified; simply dropping these P2P would alter the acceptance contract.

Actual image nextest is0.9.128 (96bc6d4f4056abd8af1e27f9f18148d0d1f447ae). Real stderr uses modern indented output ─── frames under the record's --no-fail-fast command:5base,3reference,3repeat. The prospective complete-stream qualifier accepts all3actual controls with exact report binding and no top-level infrastructure/compiler diagnostics. This answers the peer's version/framing question for the actual task image. Historical tail-only adjudication remains replay-only.

## Lean: negative control fails the expected-transition check

The task's public problem is preserving cached upstream HTTP error status instead of replaying200. Gold changes response-cache/proxy transport/bridge code. Its test_patch updates an OCLA integration fixture, while the record runs cargo test --lib.

The declared soleF2P instead checks forwarding a captured CODEX_THREAD_ID into a shell child. Its actual test body captures thread-from-hook and asserts TID=thread-from-hook. It passes in all three completed base/reference/reference-repeat controls. This is an observed acceptance/task mismatch; the origin of the label, image contamination and flakiness are unproven. [Issue](../../../issues/2026-11-10-lean-expected-f2p-already-passes-base.md).

The sole selected P2P failure asserts that a killed shell process group has been fully reaped. A minimal same-image lifecycle control observed two orphan zombies under PID1 without init and no remaining group with --init. A second registered control executes the exact same416520576-byte Rust test binary with only Docker --init changed: no-init fails rc101 after1.21s; init passes rc0 after0.20s. Both probe containers were removed and freshly confirmed absent. No test/source edit. This establishes a generic runtime correction for that assertion; full-suite and F2P eligibility remain separate. [Issue](../../../issues/2026-11-10-lean-private-evaluator-descendant-reaping.md).

The earlier last-line observation at background_pipe_holder did finish. No hang, machine RAM shortage or OOM-kill cause is claimed.

## Initial setup failure and correction

V1 used public derivative images' UID1000 against root-owned /testbed. Base failed Git safe ownership; reference/repeat failed patch writes, before any rebuild/test step. All six were setup-invalid and removed, with full errors retained ([issue](../../../issues/2026-11-10-private-grader-public-image-ownership-mismatch.md)).

V2 registers a private evaluator UID0 and explicit swap bound, leaving images, source, expected arrays and test commands unchanged. Actual source ownership/write probe succeeded. Both selected reference patch file sets are disjoint from test_patch file sets, so the generic restore-order concern does not erase their reference implementations.

## Evidence custody and remaining controls

Host-only grader ledger .superpowers/sdd/2026-11-10-grader-stream-capture contains fixed-oracle-diagnostic-v1, fixed-oracle-diagnostic-v2, orphan-reaping-probe-v1 and exact-reaping-probe-v1. It retains registrations, helpers, all raw verdicts/status maps/full captures, before/final Docker inspections/cgroups, positive/negative lifecycle probes and cleanup. The terminal closure and checked139-member capsule above preserve all completed controls. No registered control remains running. Existing public source/selection and [budget capsule](../evidence/observation-budget-20261110/registration.json) remain immutable.

No new experimental provider/model request or cost reconciliation occurred. Previously known charges and unresolved$4hold remain carried. Actual local verification compute is distinct from client model cost.

## Next decision

The registered repeat and evidence preservation are complete. Retain this cohort's failed admission in full. Generic evaluator admission must require a reproducible failing negative control, passing reference/repeat over an unchanged complete expected population, qualified source/runtime and no hidden missing statuses. Root identity, init reaping and external service declarations belong to a common runtime/setup contract prepared once per project and reused across arms.

The current frozen pair cannot grant paid admission from these results. Any later dataset/fixture/acceptance revision needs a new explicit preregistration that retains this failed cohort; it must not silently replace tasks, relabel outcomes or discard inconvenient P2P. Mechanism efficacy remains open. The next paid comparison should follow a valid diagnostic contract, rather than produce another ambiguous small aggregate score.
