---
id: '12a40a83ebe82821'
kind: plan
status: active
title: Blinded candidate integrity gate and actor-free grading resume
tags:
- effort-steering
- integrity
- prospective
topic: effort-steering
---

# Blinded candidate integrity boundary Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline under standing operator autonomy, TDD, and the parent's one strongest independent actually-applied mutation review. No repeated approval round. While fixture-complete-controls-v1 jobs195/196 remain live, do not edit their bound protocol source; planning/read-only seam inspection is safe.

**Goal:** Require every prospective candidate's bound terminal integrity audit before private results can enter the low/xhigh comparison, and resume a closed batch without re-running an actor.

**Architecture:** Freeze actors/captures/costs first. Build opaque, portable review inputs from the original public task, complete baseline/candidate and cumulative patch; no private assertion bytes, gold, arm, cost, outcome or model narrative. Native grading requires a terminal integrity proof; controller audits the full closed batch before any private grade. A separate resume API consumes the same sealed captures/ledger and never calls actor invocation.

**Tech stack:** Python stdlib, existing receipts.input_digest, evaluator_entry.verify_candidate, NativeCampaignBackend, feedback_runner closure, exclusive JSON artifacts.

**Spec:** private-test-input-closure plan § Blinded assertion audit contract —2026-11-12; parent native-public-feedback plan Global Constraints/Tasks3–5. This is a mandatory validity boundary, not a new repair mechanism or evidence of low efficacy.

## Global constraints and design ruling

- Preserve all original actor sources, canonical private roots/assertions/labels, pure/complete reference controls and costs. Do not edit the frozen99-file grader or the three private helpers.
- Standalone canonical restore remains separate. Audit complete candidate changes including inline assertions, fixtures/config/selectors/runner/parser bypass and grader-specific production behavior.
- Review input is opaque ID plus original public issue, full baseline and captured source, cumulative patch, protected-root names only. Exclude invocation/capture JSON, system/model narrative, host absolute paths, gold, private labels/results and arm/effort/cost metadata. Private controller index maps candidates to opaque IDs outside reviewer inputs.
- Verify exact cumulative patch/source equivalence before packaging, plus full source digests including modes, links and empty directories. Empty patch alone never proves unchanged source.
- Deterministic clear is allowed only for an empty patch AND exact full baseline/candidate digest equality. Nonempty/mode-only changes require a fresh blinded review; absence or untrusted self-asserted clear is never approval.
- Terminal decisions: clear, confirmed-bypass, unresolved. A known bypass cannot count as primary successful repair; preserve raw functional verdict and all costs. Any unresolved/missing audit stops full-pilot interpretation without dropping/replacing/retrying candidates.
- Every actor and cost closure precedes any private grade; every batch audit is settled before the first private result. Resume reopens original registration, all16 schedule slots, immutable attempt/capture/closure/ledger inputs. No provider/actor invocation, reservation release/recreation or ledger rewrite in resume.
- Paid112 allocation remains unadmitted under200 ceiling, known69.375358899999998524 plus4hold. Review/root/compute costs remain unknown until measured; do not claim zero or complete-workflow cost superiority from missing audit costs. Actor-free qualification must show zero forwarded provider calls.
- Preserve WIP/source capsules instead of local commits while publication is withheld. No merge/push/global configuration or destructive cleanup.

## Observed seams

NativeCampaignBackend.grade currently validates closure, unique native capture and cumulative patch bindings, then immediately invokes the isolated private evaluator. It has no audit guard. feedback_runner._run closes all actor attempts/ledger first, then loops private grades immediately; result.json is exclusive. verify_closed already reopens exact attempt identities/cleanup/source digests and16slot accounting. NativeExecutor captures exclude tool-role metadata and nested .codescout, preserving original .claude for the new policy. Existing evaluator_entry.verify_candidate is reusable unchanged for patch equivalence; full input_digest equality must additionally protect mode-only changes.


**Scouted ruling:** Verification also receives the expected issue and protected-root names from the frozen campaign/config, not from its own descriptor. A self-consistent review package could otherwise replace the task text or protected scope while retaining candidate hashes. New unbound helper/test files may be developed while public controls run; none of their existing bound inputs may be modified until terminal audit.

## Review Focus

1. A clear audit for another candidate, changed patch/source/root manifest, duplicated opaque ID or forged unchanged-source claim must fail before the isolated evaluator executes.
2. Baseline and candidate copies preserve complete source bytes/modes/links/empty directories while review artifacts contain no arm/cost/outcome/model/gold/absolute-host-path metadata. Reject unsupported/escaping link entries, not silently omit them.
3. One unresolved later candidate blocks all private grades, including earlier clear candidates; confirmed bypass retains raw outcome/cost while being excluded from primary success, not excluded from its denominator.
4. A resumed batch reconstructs native invocation identities from sealed local receipts, never a fresh actor call or cost mutation. A stale or mutated closure/capture/registration refuses resume.
5. Legacy three-phase campaigns remain unchanged. New-policy scripted16qualification must exercise the same mandatory gate; no absent-audit fallback or mock-only proof authorizes paid work.

### Task 1: Portable audit inputs and mandatory native grade guard

**Files:** Create tools/effort_protocol/feedback_assertion_audit.py and test_feedback_assertion_audit.py. Modify native_campaign.py and consuming test_feedback_backend.py/test_feedback_native.py only where the new policy grades.

**Interfaces:** prepare_audit(request:dict, issue:str, protected_roots:list[str], output:Path)->dict; verify_audit(descriptor:dict, request:dict, decision:dict|None, *, issue:str, protected_roots:list[str])->dict. Request is the existing exact candidate/baseline/patch/capture binding packet without private config. Descriptor binds opaque_id, review-input archive/manifest hashes and original capture identity in controller-only metadata. A review decision binds the exact opaque ID and reviewer-input hash, status, reviewer provenance and file/line evidence. Changed candidates require a separate review record; the controller cannot synthesize clear.

- [x] Write test_unchanged_requires_empty_patch_and_full_digest_equality: exact baseline copy with empty cumulative patch clears deterministically; same patch plus changed empty-directory mode does not clear. Observe feature-missing RED.
- [x] Implement physical exclusive output, cumulative equivalence through unchanged verify_candidate, complete portable inventories/archive, neutral names/metadata and separate private mapping. Bind both source digests and complete review-input bytes; fail closed on aliases/special files/unsafe links.
- [x] Write negative tests for source/patch/manifest/archive/decision drift, wrong candidate/opaque ID, nonempty or mode-only changes claiming unchanged, missing/extra review members and forbidden metadata sentinel strings. Actually apply a bypass mutant before adding its regression.
- [ ] Add NativeCampaignBackend.audit plus new-policy grade guard. Consuming tests must show missing/unresolved/wrong-candidate decision refuses before subprocess.run/private-request publication; valid clear reaches original raw grader, and known bypass is explicitly retained as integrity evidence without changing the grader's raw verdict file.
- [x] Run new and affected feedback suites, then whole protocol; preserve exact logs/source capsule. Expected: all pass with runtime skips explicitly stated, no actor/provider call. Parent review/gate remains mandatory.


**Review delivery seam — 2026-11-12:** Add `feedback_assertion_review.py` and `test_feedback_assertion_review.py`. `review_brief(descriptor, navigation_workspace)->str` reproduces the measured neutral brief; `verify_review(descriptor, decision)->dict` reopens physical SHA-bound dispatch/brief/raw-response artifacts, binds exact fresh-context dispatch and returned task receipt to opaque candidate/archive, validates response coverage/evidence/observed test records, and returns terminal status plus provenance bindings. Decision is a pointer record, never a controller-authored clear label. Original capture/archive verification remains in `verify_audit`; it delegates changed-candidate decisions only after that verification. Tests must reject wrong candidate, inherited context, altered brief/response, partial changed-path coverage and malformed numerical/mutation evidence. This validates recorded delivery provenance, not OS isolation or semantic correctness by schema. The two fixed, separately dispatched clear/bypass controls calibrate delivery and integrity classification only; no repair-efficacy claim. Preserve both raw final message texts, exact observed dispatch arguments/receipts, prior source bindings and calibration registration before editing previously bound files. Reviewer/root computation cost remains unknown.

### Task 2: Batch-wide audit barrier, primary analysis and no-actor resume

**Files:** Modify feedback_runner.py and native_campaign.py; create test_feedback_audit_resume.py; extend existing runner/paid-runtime tests and real16qualification driver.

**Interfaces:** feedback_runner.resume_grading(registration:dict, backend)->dict; NativeCampaignBackend.restore_closed(registration:dict, root:Path)->None. Native audit dispatch returns bound status/proof; controller persists the private opaque-ID mapping and terminal decisions separately from frozen attempts. Analysis reads integrity result plus unchanged raw private grade; confirmed-bypass contributes primary pass=False and remains in denominator/costs.

- [x] Write test_later_unresolved_blocks_every_private_grade and test_confirmed_bypass_retains_raw_pass_but_primary_fails; observe missing barrier RED before changing controller.
- [x] Separate closed-batch audit and grading. Missing/unresolved audit writes an immutable pending checkpoint; no private evaluator executes. Final analysis cannot call unresolved grades failed/clear or omit their costs.
- [ ] Write test_resume_uses_closed_captures_without_any_actor_or_ledger_write. Build a real filesystem16slot closure, initially pending audit, then terminal decisions; poison backend.invoke/authorization and assert no calls, byte-identical attempt/capture/ledger files and exact original candidates/grades. Add changed receipt/registration/source/cleanup/decision and duplicate-resume refusal cases.
- [x] Implement native rehydration from exact sealed invocation receipts under original schedule; restore no authorization to start actors. Preserve pending result.json; final resumed result is a separate exclusive result-final.json and names the original checkpoint/closure hashes. Reopen guards before/after every grade.
- [ ] Require actual audit records in prospective paid runtime proof for all16grades, not only helper invocation counts. Scripted full16CLI qualification uses unchanged source/empty patch where appropriate and proves deterministic audit path; changed-candidate barrier has separate actual isolated consuming controls.
- [ ] Run complete effort/lifecycle suites and source custody. Parent then runs full project gate, fresh strongest applied-mutation whole-branch review, exact16native qualification and independent evidence reopen before any paid reservation.


**Paid-runtime consuming seam:** Add `feedback_runtime_audits.verify_runtime_audits(registration, root, closed, result)->int`, called unconditionally by paid_admission.verify_runtime when feedback=True. Reopen the SHA-bound full index and every attempt's persisted grade/integrity-audit.json, original descriptor, review archive/manifest, optional exact decision/dispatch/brief/raw response and canonical original issue/root names. Recompute verify_audit from the original private-request capture packet and require exact proof equality with both index/result and persisted grade artifact, unique opaque IDs and a full16 terminal census. Bind every reopened control artifact in the admission registration. Legacy runtime path stays unchanged. The result-vs-frozen-attempt comparison excludes only derived grade and integrity_audit for feedback. Consuming tests must remove/corrupt the last attempt's proof and forge a self-declared clear without bypassing real audit verification; no helper-call count stands in for the sixteen actual records.

## Actual actor-free resume control — 2026-11-12

The normal16CLI qualification v3 stays unchanged and supplies the paid-runtime proof if it closes. A separate scripted16CLI control v4 will deliberately defer only the final audit at the controller boundary after the real native backend has exported every descriptor. All original-source captures remain unchanged; the delay is an explicitly artificial safe unresolved result, not an external review or hidden changed candidate.

Before resume, require all16actors closed, all client costs final/zero, an original audit_pending checkpoint, and no grades directory/private entry. Snapshot authoritative registration/closure/ledgers, native receipt/patch evidence and full captured candidates. Construct a fresh concrete NativeCampaignBackend with no authorization. Poison actor invoke, backend verify, external launch/provider forwarding and reservation creation; count even attempted calls. Resume through the actual resume_grading API, requiring16real isolated private evaluator executions, identical frozen snapshots/candidate identities/costs, original pending checkpoint retained, separate result-final/index-final, and duplicate-resume refusal with no extra private process.

This independent runtime qualification is required because unit filesystem resume plus a completed-root refusal cannot alone establish the positive actual-native pending→final path. No source mutation, task change, private-label change, paid author call or financial admission. Run v4 only after v3 actors have terminated to avoid shared embedding subnet collision. Keep all failed qualification roots and report root/review/CPU/embedding costs unknown. V4 is a resume control; normal v3 remains the existing paid-runtime verifier's input, which expects an originally completed result.json.

## Resume instrument correction — 2026-11-12

Readonly v4 remains unrun and unbound. Independent review found an unobserved initial-process zero, incomplete authoritative snapshots, and a nonredundant consuming test gap in the correct invocation-binding guard (4 applied mutations:3 killed/1 survived). Version5 replaces the instrument, observing actual parent-process Popen launches across initial and resume phases, matching initial CLI PIDs to lifecycle receipts, and snapshotting root/audits plus authoritative entry kinds/modes/link targets. Full captured candidate input_digest remains separate. This is a scoped parent-launch measurement, not an inventory of all host/descendant processes. Five local real-process/filesystem instrument tests pass; actual frozen CLI calibration and version5 review/execution remain pending. See bugs f97c3f7b8c1be9c7 and b88e9b4afca062dc.

Normal v3 has now stopped on host resource admission after10attempt records, before complete qualification. The v4/v5 preparer prerequisites remain unmet. Do not overwrite or resume that stopped root, lower the80GiB floor, or infer model efficacy. Preserve evidence and establish capacity before a fresh registered batch.

## Verified offline closure — 2026-11-12

The updated full protocol suite is661tests OK/68explicit runtime skips. The full project gate is separately terminal0 using the recorded qualified OpenSSL/private TMPDIR; earlier environment failures remain in custody. Independent72resume tests kill U04/U05;7instrument tests kill all5applied mutants. Actual frozen CLI version-only observation/PID matches. Source142capsule SHA9efd7668… keeps all102legacy inputs unchanged; selected126review/gate evidence members SHAe1cc03b8… and250producer members SHA4dc72f4b… are preserved under the experiment evidence directory. These prove offline/runtime-instrument properties only. Normal v3 stopped after9closed/1invalid/6unexecuted on root capacity and remains untouched. V5 full actual pending→final execution/preregistration and financial admission remain open; no new model/provider call or reservation.

## Fresh registration and capacity guard — 2026-11-12

Normal qualification v4 is prepared, not launched: same16schedule/tasks/plans and source142 with only reviewed consuming tests changed. Scripted preflight ready:true/paid_ready:false. Actual separate launch check refuses below92GiB (unchanged80GiB floor plus12GiB growth margin);7controller boundary tests cover both normal4andresume5call sites. Focused independent guard review is pending. Resume5preparer is readonly/unrun and requires completed normal4before creating its own registration. No cleanup or financial admission. Future command is qualify_native_feedback_campaign_v4.py <resident normal-v4 registration>; never rerun normal3or run oldresume4preparer. All actual launches remain conditional on sufficient fresh capacity and terminal review.

## Capacity handoff — 2026-11-12

Final independent growth-guard review passes14tests/3mutations all killed; allbindings/modes/dependencies match. Normal-v4registration is prepared but output absent; resume-v5preparer is unrun. Third consecutive current-capacity audit still refuses dispatch:85,892,423,680bytes available vs98,784,247,808required. All relevant background jobs and reviewer are terminal. Cache decision remains pending; no cleanup authorized by automatic continuation. Actual qualification/resume/admission and efficacy remain unachieved. Goal execution is blocked pending enough capacity/operator input; resume with a fresh capacity/binding check, then the prepared normal4entry and subsequently resume5afteritsprerequisites. No newsourcechange is needed merely to reattempt capacity admission.

## Self-review

Both tasks consume the exact same captured request and opaque descriptor; no optional approval fallback. The existing closed-batch protocol supplies actor/cost immutability; the new audit barrier precedes all outcomes. Exact primary classification preserves raw functional results and denominators. Changed-candidate reviewer execution and its provenance must be qualified before a paid pilot; a packet builder, a boolean blind flag or unchanged-source qualifier alone cannot establish that. No repair prompt or task population changes are introduced.


**Current implementation checkpoint:** New audit exporter/verifier8tests pass; actual archive/manifest equality mutant survived before a new lying-manifest regression, then is killed. Native consuming guard5tests pass, with4failures observed under actual guard removal; legacy native11tests pass. Full protocol628/68skips at that checkpoint. Batch barrier/primary classification/replayed audit-index tests4and existingrunner13pass after expected REDs. Full current protocol run pending. External changed-candidate review records, no-actor resume, mandatory paid-runtime audit readback and actual16CLI qualification remain incomplete. Initial new-source bugs are recorded immediately in MAIN as532352453c15a999/13e920eb89899e22; fixes are WIP, not independently reviewed or gated. No paid/model call.


**Current resume checkpoint:** Additional3feature-missing resume tests observed RED, then Native restore/accounting cases RED2, then all9resume/barrier tests GREEN. Actor-free resume reopens original registration,16slot closure, invocation receipt identities and audit index; it never calls actor invoke/verify/credential/preflight/reservation logic. It preserves original result.json and all frozen bytes, writes separate exclusive final index/result, keeps unresolved pending and refuses duplicate evaluation. Header common-cost drift was reproduced and filed immediately as eacc74ea44e75b50; correction binds original cost/mode before restore. Full effort637tests OK(skipped68), lifecycle110OK, source137members/SHA0892602c…,102old inputs unchanged. This proves unit/filesystem control mechanics, not actual NativeCLI or changed-candidate independent review provenance. Remaining mandatory parts are external review transport, actual Native resume/audit qualification, paid-runtime audit reader, parent16CLI/gate/strongest review. No paid/model call.


**Review delivery and paid-reader checkpoint — 2026-11-12:** Two independent fork-none strongest-model reviewers returned the preregistered clear/bypass classifications, each reporting2actually applied candidate mutations with0survivors. Root reopens unchanged export inventories, exact neutral prompt hashes, dispatch arguments/actual receipts, original response texts and prior bound source bytes. Qualification custody17members/SHA37f190b4…; same responses through current core verifier SHA1ccea278…. This proves narrow integrity/delivery calibration only, not OS isolation or repair efficacy. New consuming review tests observed missing-module RED5, then native/core consuming RED before GREEN20assertion tests. Runtime audit admission observed a missing-last-proof RED before implementation; now checks real16record/index/candidate/archive/decision bindings and rejects each of16deleted proofs plus copied clear. Full effort647tests OK(skipped68), lifecycle110OK. Four root temporary protocol mutations killed after clean20baseline; two earlier incomplete import snapshots remain inconclusive. New WIP source141members/SHA4b6b12c6…, producerV24/226members/SHA0950a25e…,102old inputs unchanged. Remaining mandatory scope is actual16NativeCLI/context/private-grade/resume qualification and parent gate/strongest whole-branch applied review. No new paid-author/provider call/reservation/commit/push; reviewer/root compute unknown.
