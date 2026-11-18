---
id: cf294f85865d4344
kind: report
status: done
title: 'Differential observations did not correct low''s inverted repair oracle'
tags:
- effort-steering
- evaluation-integrity
---

# Differential oracle development probe

## Current status

The development probe is closed with a negative result. A separately registered technical recovery completed one low Sonnet 5.5 oracle author for USD 0.2566531, followed by three valid offline evaluator cells. The frozen generated assertions accept the saved known-wrong proposal and reject the qualified reference. This mechanism does not meet the registered requirement of wrong-proposal rejection plus reference retention.

[Independent closure audit](../../../research/data/2026-11-07-differential-oracle-audit.json) rechecked all nine named assertion outcomes directly from raw logs, successful builds, frozen source/patch/log bindings, every saved evaluator resource sample, final samples and fresh absence of all four owned author/evaluator container IDs. No model or native run was repeated for this audit. The three evaluator sample populations contain 11, 11 and 12 rows; minimum sampled available host memory is respectively 35.43, 34.92 and 34.05 GiB, above the registered 24 GiB floor. No observed OOM kills. These samples do not establish continuous resource availability or an exact lifetime peak.

| Version | Scalar preservation | Mixed stream: no errors stored | Record-stream reproduction |
|---|---|---|---|
| Public baseline | pass | fail | fail |
| Saved wrong low proposal | pass | pass | pass |
| Qualified reference | pass | fail | pass |

The actor labels P2 as intended change and asserts that the public issue requires no error anywhere inside ans. It cites the proposal's modified store_list_stream_prefix as evidence. The reference instead preserves the mixed stream. Thus useful differential observations were available, but the actor still inferred an over-broad expected behavior. The record-stream assertion does distinguish baseline from reference; the mixed-stream assertion invalidates the combined oracle. The failure is oracle selection, not absence of an executable witness.

This is one exposed development task and one saved wrong proposal, not held-out generic efficacy or a causal estimate of the observation packet's effect. Earlier blind-verifier output is a separate negative control on the same task. Known series model subtotal is USD 62.410596299999999054; the original zero-observed-forwarding startup attempt still has an unresolved USD 4 reservation and unknown final provider cost. Final cumulative author cost is counted once across its three CLI turns.

## What was measured

Three public programs ran against the public baseline and saved low proposal. P1 and P2 reuse programs chosen by the previous blind verifier. P3 is a record-stream reproduction derived from the public issue. The new instrumentation records stored payload before/after, operation exit, and a separate actual evaluation of ans and its exit. It imposes no expected payload assertions.

| Probe | Baseline after operation | Proposed repair after operation | Operation exit, both | ans evaluation exit, both |
|---|---|---|---|---|
| P1: integer list then str length | original integer list | original integer list | 1 | 0 |
| P2: each yields an error for the middle row | list with 1, error, 3 | original list 1, 2, 3 | 1 | 0 |
| P3: record list then str length | list of error values | original record list | 1 | 0 |

These are observations, not expected-result labels. In particular, exit 0 alone cannot decide whether the stored payload is semantically right. Interactive.ans_value reads a stack variable; its name does not establish end-to-end REPL behavior. The new probe separately calls eval_source, whose return type is an integer exit code. Raw unmarked console output can interleave because the test harness runs the three probes concurrently; only OBS records have reliable per-probe attribution. The packet explicitly discloses that limitation.

## Evidence and corrections
Closed recovery evidence: [allowlisted archive](../../../research/data/2026-11-07-differential-oracle-evidence.tar.gz), 53 payloads plus manifest, 74,077 bytes, SHA256 b35b3ff0347ff1526b86ca7ca5698f90cd52541183459887ceae6a58e9fff63e. Every payload was reopened and hash-verified. Raw CLI debug, auth/client/native homes, model requests/responses and large runtime inspection files stay resident and are excluded. [Closure audit](../../../research/data/2026-11-07-differential-oracle-audit.json) SHA256 9cbaffad925bdf52c3b1942628dedfe5762c56d42297a201eef8399f94c2a196. Source/runtime preparation is retained in the separately completed technical-recovery plan. Public observations below describe the earlier model-free stage.

Both completed cells built successfully and executed all three named probes. Each log contains exactly the registered three IDs and five marker fields per ID. The audit independently re-parsed statuses/markers, checked probe/source/log bindings, resource samples and fresh absence of the owned container IDs without a native rerun. [The exact audited observations](../../../research/data/2026-11-07-differential-observations.json) were copied byte-for-byte from the resident audit.

An initial cell was resource-invalid under the inherited 48 GiB offline floor and is preserved. Before any valid observation outcome, a separate observation-only recovery was registered with a 24 GiB host floor, retaining the same 4 CPU/8 GiB hard limit, network isolation, executable tmpfs and monitoring. This changed diagnostic admission, so it is not a matched resource comparison with the old benchmark cohort. It did not change the paid actor's profile/floor or old results.

The initial instrumentation also failed compilation because the Nushell test harness print macro evaluates its arguments inside a unit-returning closure; a question-mark expression could not propagate Result there. The corrected probe evaluates payload reads before the macro. Old source, patch and compiler log are retained. [The instrumentation issue](../../../issues/2026-11-07-observation-probe-print-closure.md) records the failure. No failed/incomplete cell was scored as an observation or model-quality outcome.

The corrected probe patch SHA256 is bc7fe427d8f5c962b93c5050a093ba45a2d3106b59a13204884aaeb3b19d4020. Final observation cells have complete healthy sampled resource evidence and zero observed OOM kills.

## Test-population calibration before spending

The new evaluator originally selected every function whose name began with contrast_. Calibration on the completed real probe patch found four such functions but only three executable tests; the fourth was a helper. Before any actor call, the unlaunched waiter was verified and stopped, and a new evaluator revision was frozen. It selects supported test declarations, checks the author's declared test list, and still requires actual runtime statuses. An isolated copy with the old prefix selector restored fails that same calibration: one actual applied mutant, one killed, zero surviving. Unknown declaration forms remain explicitly unscorable. A completed author that produces no executable detector is recorded as abstention, not as infrastructure failure. [The issue](../../../issues/2026-11-07-contrast-selector-counts-helper-functions.md) and source calibration receipt are preserved.


### Recovery-orchestration checks

The exact running recovery helper was exercised with all subprocesses and resource readings mocked: success, red gate, insufficient memory, author failure and evaluator failure. Five baseline scenarios pass. Three isolated mutations were actually applied and run: bypassing a red gate, leaking the gate environment into the author, and removing the memory floor. All three now fail their checks. The first checker version let the memory-floor mutant survive because its mock clock timed out before a third sample; that test defect was observed before correction and recorded immediately in [the issue](../../../issues/2026-11-07-admission-clock-control-misses-third-sample.md). Before correction: two killed/one survived; after correction: three applied/three killed/zero survived. The admission helper itself was not changed, and its running hash matches fixture-orchestration-check.json. These checks cover decision boundaries with simulated processes, not live model quality or the complete runtime.

## Next measurement and limits
The differential-observation variant is NO-GO as an acceptance gate. Do not run more replicas of this same mechanism or extend its generated no-error assertion to other tasks. A next development mechanism must establish an oracle independently of the proposed implementation, then retain qualified correct repairs and reject wrong ones before transfer measurement. Overall low-improvement research remains open. The following resource/admission notes are historical; completed recovery is in the technical-recovery plan and closure audit.

### Resource calibration before changing the actor profile

Offline audit of saved calibration39 is now closed without a native rerun. The existing reviewed split_concatenated_status helper recovers exactly the glued public pass at physical line531; all35named public last_result tests agree with the35passed/0failed summary. There are81saved resource samples, maximum recorded cumulative cgroup peak4477276160bytes (about4.17GiB), minimum sampled host available26.76GiB, zero observed OOM events and fresh owned-container absence. Original invalid.json remains unchanged. Because its status-count assertion failed before the final post-workload sample, this is the maximum recorded cumulative peak, not a claimed exact final lifetime peak. It bounds this workload only, not every action an unconstrained author might take.

The unchanged external adapter explicitly requires a32GiB native hard limit. A hard limit is not reserved memory. Therefore the next proposed amendment keeps that exact32GiB container/profile and public packet, while changing only diagnostic host admission:24GiB prelaunch for three samples and16GiB continuous floor, sampled every2seconds. This is a different host-resource admission policy for an exposed development attempt, not a matched comparison with the old48GiB floor. Bind/test the separate supervisor and runner before spending; do not edit original sources/registrations. No actor has launched yet.

First calibration38 closed invalid before any tests: the guessed Cargo target main does not exist in nu-cli. Four startup-only samples are retained and do not qualify the intended workload. Its exception reporter also failed; both instrument defects were recorded immediately in MAIN (b7741710808e8958 and94df62aeb703906d), and owned-container absence was verified. Corrected reporter was checked against the actual original and corrected exception expressions (old raises AttributeError, new returns the exact invalid receipt).

Native manifest inspection declares target tests with source tests/main.rs. New calibrate_memory_v2.py binds a fresh output root memory-calibration-v2, performs actual native cargo metadata verification before invoking --test tests, and retains the same8GiB/4CPU profile/floors. Job39 /tmp/codescout-bg/2674124-d9ea3495-bg_00000039.json is active, outcome unobserved. No old invalid was relabelled and no model/credential or paid-profile change occurred.

The closed previous verifier log population has15sample rows,14with a live container. The actual recorded fields contain Docker configuration, host availability/pressure and memory.events; they contain no memory.current or memory.peak measurements. Zero OOM is not a measured peak and does not justify a smaller actor cap.

Registered model-free calibrate_memory.py uses an independent source-only snapshot clone, the same public proposal image and native Codescout, a4CPU/8GiB container, networknone and executable4GiB tmpfs. It requires16GiB host memory at launch and monitors12GiB continuously plus unchanged80/4GiB disk floors. It records actual cgroup current/peak/events every2seconds and exercises initialization, workspace status, relevant symbol body and the public nu-cli last_result test module. Source, manifest, native argv and snapshot clone are bound before execution; owned-container absence is checked on cleanup. No model, token or private oracle is involved. This qualifies only the measured workload; paid actor profile remains unchanged pending a separate explicit amendment. Any interrupted/resource-invalid calibration is preserved, not scored as a model failure.

The new actor will classify each observed difference as unchanged, intended change, regression or unresolved, with a public requirement/source basis. It may abstain. It must produce executable assertions plus a decision file, leaving production and existing tests unchanged. The controller freezes them before host-only reference evaluation. A quality signal requires rejecting the wrong proposal while retaining the qualified reference; correct-looking prose or green syntax checks are insufficient.

The actor packet contains only baseline.log, low-control.log and the expectation-free probe source, plus public issue/source and the existing project/global guidance. Reference/private tests, old private grades, previous diagnosis, old actor transcript and expected classifications are excluded. This is an exposed development task, not a held-out efficacy or causal comparison isolating a single prompt feature.

Admission-v2 job2b closed without launch, and source gate2a failed only the known directory-swap fallback test in both Rust lanes (Python/format/clippy all zero). Source HEAD/status stayed unchanged. The original cost-free waiter29 and failed gate are preserved. The exact failing test passed in separate targeted job2e with FILE_PROVENANCE_ROOTS pointing to a new empty directory and FILE_PROVENANCE_CODEX_ROOTS_JSON=[]; this does not turn gate2a green. Registered resume_after_fixture_control.py will require a full gate using that controlled history before a fresh bounded headroom wait and the same one prepared author. Its gate-only environment is never passed to the author. This reuses the known fixture-isolation idea without changing source or importing the absent b33315316 commit. [The existing issue](../../../issues/2026-10-28-attribution-directory-swap-gate-flake.md) records the new pre-fix-source observation; it is not evidence of regression on experiments. Resident root is /home/marius/.cache/codescout-resident/codescout/differential-oracle-20261107. Admission, source/input hashes and every eventual attempt/result are one-shot artifacts there; do not restart an attempt based only on a missing final result. No product code, old frozen cohort, pending staged grader change, merge or push has been changed by this probe.


Execution receipt: recovery job30 is /tmp/codescout-bg/2674124-d9ea3495-bg_00000030.json. Its registration fixture-recovery-registration.json binds the unchanged source and exact actor/evaluator hashes before the full controlled-history gate. At latest inspection it was running Python verification; no model was launched. On closure, inspect actual fixture-source-gate-process.json and eventual author/evaluation receipts, not the old jobs' records. The isolated control changed both suite contention and history input, so its pass alone does not identify the cause of the original failure.
