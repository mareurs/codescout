---
id: 01c1d8c003260e0f
kind: plan
status: done
title: Integrate verified worker storage and native resource configuration
tags:
- effort-steering
- evaluation
---

# Verified worker runtime implementation plan

> Implement inline with superpowers:executing-plans and test-driven-development; request one independent final review with applied mutations. Standing operator autonomy applies.

**Goal:** Apply the reference-qualified storage and native resource settings to opt-in verified workflows before the next matched model calibration.

**Architecture:** Keep the existing lifecycle/public-verification implementation. Add small storage/argv helpers in tools/agent_lifecycle/workflow.py and call them only from the workflow branch of hint_authority_pilot.run.one. Legacy runs preserve their existing actor/argv behavior.

**Spec:** Situation report c5dc7388ef84e4b5, final reference evidence docs/research/data/2026-11-05-public-profile-final-verification.json, parent plan c3836cf55c7b4b87. Source base 0998c3b64f81c13331db26e0091ff25404272c66 in /home/marius/.codex/worktrees/615e/codescout.

## Constraints and review focus

- Reuse frozen task, global/project instructions, onboarding, model/effort flags and private oracle. No new provider calls in this implementation/qualification stage.
- Verified actors and build outputs live under their exclusive launch output directory on a non-RAM filesystem, with at least 30 GiB free observed before copying/credentials. Record filesystem/capacity/path; this is a preflight observation, not a disk reservation.
- Native verified launches add --init and --cpus 2 while preserving all other Docker isolation, mounts and image settings. Reject malformed/conflicting argv; do not mutate the legacy argv list.
- Storage/probe failure must stop before credentials, proxy/CLI/native launch. Invalid manifests already fail before setup and must keep doing so.
- Legacy actor/argv behavior stays intact. Each attempt still has exclusive directories and independent candidate state; no sharing of task diagnosis, code or runtime DB between arms.
- Boundary cases: RAM filesystem despite abundant free space; one byte below threshold versus equality; probe errors/unknown type; conflicting resource flags; accidentally applying settings to legacy mode.

## Task 1: Integrate, qualify and preserve the runtime

Files: modify tools/agent_lifecycle/workflow.py and tools/hint_authority_pilot/run.py; add tools/agent_lifecycle/test_runtime.py; adapt only the existing workflow test fixture's storage/native-argv boundary in test_workflow.py; update tools/agent_lifecycle/README.md.

Interfaces: prepare_actor(out) returns the disk-backed candidate path and writes runtime-preflight.json. verified_native_argv(argv) returns a copy configured for CPU2/init. run.one consumes these only when workflow is present.

- [x] Write through-worker tests for actor placement, resource argv, precredential RAM/space/probe rejection, threshold equality and legacy isolation; observe the missing behavior fail.
- [x] Implement the two helpers and minimal opt-in wiring; keep source/config/oracle unchanged. Run new tests and the existing lifecycle/pilot/completion/resume suites.
- [x] Qualify actual native/CLI success, repair-feedback and incomplete controls on the new composition without credentials/provider access. Observe actual resource/actor placement and exact cleanup; retain prior full Rust reference control as separate prerequisite evidence.
- [x] Independent reviewer applies candidate mutations to placement, RAM/free-space guards, resource flags and opt-in isolation in a private copy, runs the real suites and reports observed killed/surviving counts. File any finding immediately in MAIN docs/issues.
- [x] Run the V development gate, inspect complete exits, commit exact source paths with the canonical Session-Id trailer; no merge/push. Update plan, reports/issues and durable memory.
- [x] Prepare the fresh calibration composition fingerprint/admission requirements. Do not reopen old cohorts or buy a partial pair from the old pilot balance.

## Execution ledger

Ruling: keep the execution ledger and review artifacts under the path returned by scripts/resident-scratch.sh verified-worker-runtime-20261105, outside the repo. The project convention docs/conventions/resident-scratch-location.md overrides the skill's in-tree .superpowers scratch directory; this avoids the documented rust-analyzer symlink/watch issue. Cost if wrong: some skill helper automation is unavailable; retain equivalent explicit task/test/review records.

Pre-flight: one task; helper contracts are consumed by one existing worker entry point. No interface dependency between independently committed tasks. Source worktree freshly clean. Main documents remain local under the existing provenance caveat; foreign staged files must stay untouched.

## Completed checkpoint

Source commit `7e57bf8f91a2fd369411a2d89db51cf97c070d9f`, stable patch-id `528637cc65548ba4cb3f84424d2c8ebf725ec148`, canonical Session-Id, five exact source paths, V clean. Full V gate exited0/all five lanes0. Final42lifecycle tests,28pilot/9completion/3resume tests and4actual CLI/native scripted cells pass their registered expectations. Review: initial12applied/11killed/1survived; focused final8variants/5killed/3redundant survivors plus3greencontrols. Findings immediately filed and fixed in MAIN records. No provider cost, token read, merge or push.

Report328a44811943317e records exact scope and limits. Data/2026-11-05-verified-runtime-verification.json plus archive SHA2569d76aba72de8a72c1928ba88665754a1c9f3145005fb459882440bca5292b2ef preserve349hash-verifiedpayloadfiles. Preparation inventory36files digestbb3ee8c6c57e09c6b848ae7b543d62049104d9a9c10aca6dadec73a5319fb1ed; paid_admitted=false. Next work is final fresh matched driver/admission then actual model measurement. This plan's infrastructure stage is complete; research objective remains open. MAIN docs/evidence remain local under known provenance coverage caveat.
