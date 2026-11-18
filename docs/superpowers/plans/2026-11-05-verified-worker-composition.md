---
id: '22ed6d6c4e0c2288'
kind: plan
status: done
title: Compose live worker with bounded public verification
---

# Verified worker composition implementation plan

> Execute inline with TDD and one fresh independent review. The operator already authorized autonomous work; no new paid runs in this stage.

**Goal:** Integrate same-session background continuation into the repair worker with one continuous provider budget and independently observed public verification.

**Architecture:** Add an opt-in workflow adapter to the existing worker, preserving legacy defaults and closed cohorts. The adapter runs the qualified stream controller against the same live proxy. After yielded jobs are settled and no further background results await delivery, execute a configured manifest of public commands in the exact live native worker. Record every command's exit status and output. On known failure return public feedback to the same CLI session within its existing continuation/deadline limits; unknown results abort. Full workflow success requires successful lifecycle, a complete passing final verification round, reconciled cost, clean transport and exact worker cleanup.

**Spec:** Parent issue bf167b83c0166a24 and lifecycle report7447904928197ebf. This plan measures harness composition, not low/xhigh task performance.

## Constraints

- No token reads or provider forwarding in offline qualification. Preserve old attempts and costs.
- Shared setup: frozen onboarding/global/projectCLAUDE, skill and hint remain present.
- One proxy and Budget object across every CLI turn. Never sum cumulative terminal cost.
- Commands are explicit public configuration, no private oracle feedback. Run all checks after ordinary failures; unknown/timeout stops the round and prohibits success.
- Validate manifest before credential access. Same worker/image/label identity must be witnessed; ambiguity fails closed.
- Report CLI/lifecycle/check/cleanup/cost outcomes separately; never derive full verification from assistant prose.

## Tasks

- [x] Add completion-check callback to stream controller; real-subprocess tests prove feedback continuation, turn/deadline bounds and unchanged no-callback behavior (RED→GREEN).
- [x] Add generic manifest verifier and native execution adapter, with passing/failing/unknown/empty/duplicate/partial-round tests (RED→GREEN).
- [x] Wire worker.one opt-in workflow mode and preserve legacy behavior. Tests exercise real setup and proxy, including campaign reserve exhaustion after a continuation, unknown usage and persisted failure receipts.
- [x] Qualify actual CLI/native composition with frozen full context and scripted responses: all public commands pass; first round fails then same-session repair passes; missing/unknown verification never becomes success. Verify identity, history, zero forwards, terminal statuses and cleanup independently.
- [x] Independent review applies at least8 actual mutations; close observed substantive survivors and reapply. Run full repository gate and relevant suites, commit exact paths, update issues/reports/memory. No push.

## Review focus

A fresh validator round must observe the candidate after the latest agent edit. Failed commands cannot be hidden by a later passing command. A terminal without any checks cannot count as verified. Failed or timed-out rounds must remain durable, even if a later repair succeeds. Provider budget and unknown-cost state must survive every continuation. Cleanup failures invalidate workflow completion without discarding known cost.


Checkpoint: implementation and actual CLI/native qualification v2 complete at zero provider cost. Lifecycle/composition34 tests plus legacy28/9/3 pass. Two Important functional review bugs reproduced RED→GREEN; exact14mutationrecheck13killed,1invariant-equivalent survivor. Test isolation fixed; accidental review-created launch-v1/unused preserved outside the closed root. Full gate @bg_00000040 exited0 with all five lanes0. Source committed 0998c3b64f81c13331db26e0091ff25404272c66 (stablepatch-id 971ae969a0c1417a98d387e1c314a1fec4dbf3b2); research worktree clean, no push or merge. The Rust profile is configured but was not exercised as a repair-quality experiment. Final completion awaits aggregate gate and exact-path commit.

Final: standalone worker composition, offline qualification, mutation verification and source preservation complete. Future paid calibration requires a fresh composition/manifest/runtime fingerprint, acceptance controls and bound review; no new paid calls were launched by this plan.
