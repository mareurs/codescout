---
id: c0e1069c9ef03f0b
kind: plan
status: done
title: Implement the fixed six-arm campaign controller without paid admission
tags:
- effort-steering
- campaign-controller
- zero-provider
---

# Semantic-contract campaign controller implementation plan

> **For agentic workers:** Execute inline using superpowers:executing-plans under existing user autonomy. One fresh-context final review with actual mutations, one offline fix pass; no repeated delivery/model cases.

**Goal:** Implement and test the remaining fixed-cohort controller, then bind a real zero-provider preflight that mechanically refuses paid launch until grader, live policy and fresh controls are qualified.

**Architecture:** A private standard-library state machine consumes a source/input-bound registration and a separately qualified admission record. Reuse unchanged external.launch and workflow driver reconciliation from .worktrees/semantic-contract-runtime-01a15ca9. The controller closes six candidates/costs and writes repairs-closed; it never invokes private grading. RealBackend handles only existing launch/reconcile/native readiness and independent cleanup queries. Tests replace the external model boundary with deterministic filesystem actors; evidence and ledger stay outside repository.

**Tech Stack:** Python unittest, Decimal, SHA256/JSON, existing corrected native lifecycle. No product-source/grader edit.

**Spec:** docs/superpowers/specs/2026-11-06-semantic-contract-pilot-design.md (28aab59cb5842630).

## Global Constraints

- Six fixed arm IDs/order/actual efforts from frozen matrix. Prior58.180531399999999104USD, ceiling200, reserve20 each and CLI19. Finite nonnegative Decimal only. Remaining fixed cohort must fit before each attempt; every charge preserved even if run invalid.
- Keep historical cohorts, real prepared states, sources and plugin bytes unchanged during tests/preflight. No provider/credential/onboarding/CLI/native actor this stage. Preserve all attempted executions; no automatic retry/resampling/resume.
- Paid entry requires source-bound true admission plus nonempty qualified proof maps for reviewed grader, prospective live parser/infra policy, fresh2tasknegative/gold/repeat controls, runtime/wire/plugin and controller review. Missing/unknown/drifted prerequisites reject BEFORE first launch.
- All immutable arm/global/source/plugin inputs rechecked before each launch; only future arms' writable states must match pristine hashes after prior arms legitimately mutate. Existing start/receipt markers in any future arm reject.
- Resource floors80GiB root,4GiB /tmp free and48GiB MemAvailable; no overlapping heavy campaign stage. Save start/end resource/cgroup pressure observations per stage. Boundary measurements are not reservations or continuous monitoring; finite native/workflow timeouts remain3600/1800/8. Stop subsequent calls on resource, transport/context/unknown cost/capture/background/cleanup failure. Genuine functional failure alone continues fixed cohort.
- All six terminal costs and captured candidates close before any private evaluation. This controller emits only repairs-closed, never private grades.
- Peer owns tools/setup-ab-eval; no competing fix or redundant closedNUlowdiagnostic. Worktree stillclean4c90f8fab and graderissuesopen at this turn's fresh check. Controller work can proceed independently; actual admission remains false.
- Qualified product source unchanged from closed gate10e; MAIN red1dfa43ac9834a309 retained. No commit/merge/push. Archive private analysis code/results rather than changing product source.

## Review Focus

- Partial cost unknown/receipt mismatch or terminal exception must not advance to a second paid attempt or lose the attempted arm's failure record.
- Variant/effort/order/path alias errors, current/future state drift, plugin/source/proof edits must refuse before the next call.
- Functional public-check failure must not be silently classified as infrastructure or selectively stop the fixed cohort.
- Recorded known charges survive later capture/cleanup failure; no completed-cohort marker on partial execution.
- Tests must distinguish completed writable state from still-pristine future states, actual malformed field types from false booleans and missing prerequisites from fabricated qualified proof labels.

### Task 1: Implement the state machine with behavior tests

**Files:** resident semantic-contract-controller-20261106/campaign.py,test_campaign.py; no product code.

**Interfaces:** run(root: Path, registration: Path, admission: Path, backend) -> dict; preflight(registration: Path, admission: Path, backend, index=0) -> dict. Backend validate(row),resources(row),launch(row,reserve,cli),reconcile(row,receipt,reserve),cleanup(row) represent existing external boundaries. run itself reads actual configs/hash maps/candidate bytes and writes durable one-shot events/known costs/stops/closure.

- [x] Write and run failing tests for fixed order/effort/budget; absent/false/drifted admission; source/plugin/immutable/future writable drift; prior starts/path aliases; functional failure still6calls; unknown cost/context/transport/capture/background/cleanup/resource failures stop; known cost survives downstream errors; closed candidates/costs only after6 and no grading call; repeat invocation refuses.
- [x] Implement only specified interfaces; run whole private controller suite. Registration of any attempt is durable BEFORE backend.launch, exceptions preserve stop record and never retry.

### Task 2: Bind real preflight and preserve missing prerequisites

**Files:** real_backend.py,freeze-controller.py,registration.json,admission-pending.json,preflight.json/log in resident root.

**Interfaces:** consumes frozen six prepared arms, full plugin manifest from closed prospective smoke, current exact lifecycle/reconciliation source, readonly inventory and reviewed evidence. Produces one registration plus an explicitly unadmitted record, with grader/livepolicy/controls/controller review initially false.

- [x] Bind all current source/input/immutable/writable hashes and known budget; validate real native configs/readonly inputs and unchanged six arms without starting them. Run real preflight with pending admission; expected REFUSAL listing missing grader/livepolicy/controls/review,0provider/credential/startmarkers.
- [x] Test the adapter boundary with real saved receipts where possible; do not claim faked model calls prove paid enforcement/provider behavior. Do not use audit-only parser as live verdict.

### Task 3: Review and preserve

- [x] One fresh independent whole-change review must inspect actual real preflight and apply at least8 focused code mutations against copied suite, recording observed killed/survived before fixes. File issues immediately MAIN.
- [x] One offline fix pass as needed, exact mutant reapplication, archive small source/tests/logs/proofs/configs with reopened hashes, update report/current memory and next grader critical path. Keep actual paid admission false while prerequisites pending.

## Rulings

Ruling: implement controller while peer grader hand-back is pending — source-independent necessary work, no competing grader edit or model call. If hand-back arrives, incorporate only through a new source/control-bound admission; no frozen record rewrite. Cost if wrong: controller/preflight must be revised before paid entry.

Ruling: finite stage boundary resource/pressure observations plus existing native limits/timeouts are implemented here; continuous host monitoring is not claimed. Future paid admission must settle any remaining mid-stage stop behavior explicitly before provider calls. No silent weakening of registered resource policy.

Ruling: no automatic source commit under this bounded private-analysis plan; qualified source remains unchanged, evidence is durably archived.


Ruling: the concrete adapter in this phase is explicitly Readiness, with NO paid launch/grading method or paid CLI. The tested campaign engine remains injectable at the external actor boundary. Source scout showed that the live public-failure policy must distinguish actual compiler/test failures from linker/storage faults too, while preserving the existing functional-continuation-exhaustion error as a semantic failure. That qualified policy is absent with the pending grader hand-back. Do not wire a paid adapter that would silently guess classifications. Later integration requires its own bound qualification; cost is one remaining adapter step, no provider risk now.

Task1 engine tests: no-op RED17tests/41assertionfailures; GREEN17tests. Every external actor is a filesystem boundary fixture, not evidence of paid-provider behavior. The registered real-six states remain untouched. Private readonly adapter has five direct filesystem controls; they are an additional correctness check, not claimed as a separate RED-GREEN implementation cycle.


Task2 realread-onlypreflight11d CLOSED0 14:38:35UTC. All6nativeconfigs/pinnedimages validated;66immutablehashrecords+5838writablehashrecords (not unique-file count),59sharedhashes;61readonlyrecords across4uniquehostmountpairs; fullplugin unchanged; admission explicitlyrejected grader_review/live_policy/private_controls/controller_review.0provider/credential/onboarding/noactualstartmarkers. Sampledroot341111189504bytes/tmp27188498432/MemAvailable90481410048; pressureboundariesonly. Freshindependentreviewer semantic_campaign_review dispatched forsingleboundedcode/data/mutationpass. Prior17test41failures thenGREEN; readonlyhelpersequencecorrectedbydeletingimplementationbeforeobserving5no-opfailures andrewriting; currentwholeprivate22PASS.


## Bounded closure — 2026-11-06

Engine/readiness work complete: historical four-charge reconciliation calibrated exactly; fresh review fourteen actual mutations initially seven killed/seven survived; one fix pass 32 tests pass and same fourteen mutations fourteen killed/zero survived. Four controller issues fixed. Original v1 remains intact; post-review v2 real readonly preflight11f CLOSED0 and correctly denies paid admission. Archive/report: docs/research/2026-11-06-semantic-contract-controller.md and data/2026-11-06-semantic-contract-controller-evidence.tar.gz. Full declined-review rulings retained in resident progress.md.

Paid adapter/functional classification/negative-gold-repeat controls remain deferred, no new provider calls. User relay now confirms peer grader worktree had only205 baseline tests and no fix; take over existing grader plan next, without repeating the closed NUlow diagnostic or modifying frozen records.
