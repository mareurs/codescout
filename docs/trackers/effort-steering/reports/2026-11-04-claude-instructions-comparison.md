---
id: d41545d3126609b1
kind: report
status: draft
title: CLAUDE instruction change audit and comparison with effort-steering baselines
tags:
- effort-steering
- harness
- evaluation
---

# CLAUDE instruction change audit — 2026-11-04

## Request and unresolved treatment identity

Operator asks to inspect the CLAUDE.md change and its commit, understand the behavior mechanism, and measure it against the effort-steering work. The operator confirmed that the requested treatment is the authority declaration for [cs-hint]. The inspected permanent files do not yet contain it; test absent/present variants privately without changing the global installation.

Read the main checkout CLAUDE.md, its latest commit, all-refs history for this path on 2026-11-04, all four global Claude profile files, and the committed native-context evidence. Findings:

- Latest main-file modification is b697e19bdba230c615abad313387881a1df57501, at 11:30 +0200. It replaces the unconditional downward-only prompt-budget wording with a pointer allowing raises for recorded harness reasons. That commit changes documentation, comments and test messages in CLAUDE.md, src/prompts/README.md, src/prompts/mod.rs and src/server.rs; it does not introduce the hook-endorsement clause. No budget increase is asserted from this sentence.
- Four actual global files ~/.claude{,-sdd,-kat,-stef}/CLAUDE.md are byte-identical, 3014 bytes, SHA74a3eb181f6ec2ba5c3275dda9da35102144fe8cc665abaf67a778a29771d8c6. This exactly matches inputs/global-CLAUDE.md in the previous native-context evidence archive. None of those inspected files contains the proposed hook-endorsement clause.
- Commit1db3277a13b5761c5c9933960eb8e6d2490a611c records Stage3 hook-authority results. Its three changed paths are the preregistration, stage3.jsonl and analyze_stage3.py. It does not permanently edit CLAUDE.md. The existing runner appends the declaration only to temporary A3/W3 task copies.

## Existing measurement reaggregated offline

Re-ran the committed analysis over all150 Stage3 score records; no new model runs and no independent re-scoring of raw transcripts. Model claude-sonnet-5-5, frozen client2.1.309, source set project,local in every recorded row. All150 have valid=true and stopped outcome. The runner has no explicit --effort flag; these results are not a low/xhigh comparison.

| Recorded measure | A1: right hint only | A3: right hint + CLAUDE declaration | W1: wrong hint only | W3: wrong hint + declaration |
|---|---:|---:|---:|---:|
| Late target lookup, among informative records | 1/11 | 5/9 | 1/10 | 3/10 |
| Target references/call-graph lookup before stop, all valid | 15/25 | 21/25 | 16/25 | 18/25 |
| Any recognized caller check, including Grep/Bash search | 24/25 | 25/25 | 25/25 | 25/25 |
| Unrelated decoy lookup after hint | 0/25 | 0/25 | 0/25 | 16/25 |
| Mean client cost USD per recorded run | 0.13445114 | 0.138901336 | 0.132557168 | 0.147757544 |

The all-valid lookup and alternative-search summaries are exploratory additions to the registered analysis, not substituted primary metrics. Informative records exclude runs with an already-issued target lookup; the denominator differs by arm. Do not report 1/11 versus5/9 as task pass rates or general improvement in caller checking. Almost every arm already performed some recognized caller check. The experiment terminates at the stop boundary before the edit; no accepted patch, preserved regression, whole task completion or xhigh parity was measured. A decoy lookup measures an unnecessary lookup, not demonstrated damage to product code.

Source: [preregistration AM-10/AM-11](../../../evals/harness-channel-hint-authority-preregistration.md), [analysis](../../../evals/data/2026-11-04-harness-channel-hints/analyze_stage3.py), [scored records](../../../evals/data/2026-11-04-harness-channel-hints/stage3.jsonl). Stage3's20.760USD is another session's recorded campaign cost, not fresh spend by this effort-steering thread.

## Relationship to our earlier experiments

The latest real-client low/xhigh context checks used scripted loopback responses, not real model decisions. Their actual launch disables hooks, pins companion1.20.26, explicitly loads user,project,local, and verifies complete global/project/Recon/native guidance transport. Adding channel authority to those unchanged scripted checks cannot measure model compliance or task quality. Reuse them only to prove the treatment reaches the actual request.

Earlier low/Recon quality cases were read-only native-file-tool treatments without native Codescout. They reveal missed semantic assumptions and invariants, but cannot serve as a causal control for a new native MCP plus hooks plus endorsement condition. A new lower error rate under several simultaneous changes would not identify the declaration's effect. Exposed RA22849 and bindgen3390 remain development/negative controls, not fresh acceptance evidence.

Known source-loading defect b8c35dc28bd3e8dd is already recorded in MAIN. Do not copy the other campaign's old local-only flag into this one. Verify the actual serialized instruction text and actual delivered hook/tool population instead of relying on file existence or flag names.

## Proposed comparison once the exact addition is confirmed

Use a matched fresh baseline under the same current frozen runtime. Change only the confirmed clause between absent/present variants; retain task CLAUDE.md, approved global rules, cached onboarding, Codescout data, skill assets, tool permissions, hook code, model, task and executable checks identically. Do not re-onboard. Historical results remain context, not the control arm.

For the confirmed hook-endorsement treatment, use a 2x2 comparison of effort low/xhigh and declaration absent/present, with identical actual hooks in all four cells. This distinguishes an overall prompt effect from a reduced low/xhigh gap. First establish actual instruction/hook delivery without provider calls. Then verify that the chosen real model accepts and receives the requested effort levels; the previous scripted Sonnet transport is not proof of that API behavior.

Primary outcomes: independent functional acceptance and regression preservation; whole-workflow client cost including failures, validation and repair; completion. Secondary observations: whether a useful hint was delivered, whether its prerequisite was actually checked, whether irrelevant advice was rejected, latency and tool use. Prompt opening, references count and filled evidence fields are not correctness scores. Include a harmless known-irrelevant hint as a separate diagnostic of discrimination; never silently pool it with normal-task quality.

Use new tasks with existing verified onboarding where available, rotate paired run order, isolate caches/state per actor, freeze exact inputs and held-out oracle boundaries, and record invalid attempts and stopping behavior. Start with a bounded diagnostic pilot rather than promise a powered quality conclusion from the remaining budget. No paid run has been launched by this audit. Total authorized40USD; thread spend29.221154299999999834, remaining10.778845700000000166, unchanged.

## Restart observation

The answering Codescout server is now PID455879, clean aebb0e76f, exe_deleted=false. Independent SHA256 of /proc/455879/exe is7729fa77d00ffa5f7445d02600f46e58b5e6ec671c5aa65b0ff9cda2a052e94d, matching its build_id. This is a newer binary than the prior controlled8f567f17 artifact; do not claim that prior artifact was deployed or transfer its exact-artifact qualification automatically. No process restart, release build or profile edit was performed by this audit.

## Validation status

The exact-path local commit was refused by the foreign-write guard, which attributes the three doc-tool-created notes to another Codex SID despite this thread's positive tool-call and environment identity evidence. No commit or guard bypass occurred. Incident: docs/issues/2026-11-04-doc-write-provenance-conflicts-with-own-codex-thread.md. Notes/evidence remain reviewable in the shared working tree/index; preserve them together until attribution is resolved.

Offline Stage3 aggregation succeeds. Frozen input archive SHA27db2006f5952d36b6e5243c2055cd53179bce1533abcabc29e500cf21f8e73d,3915bytes/5verifiedentries; no model or credential access. Actual hook-delivery qualification and functional pilot have not run.

The required full `./scripts/gate.sh --check` was run on the changing shared checkout and is RED: Python0, formatter1, clippy0, lean100, default100. Onboarding content-block assertions and librarian path assertions fail; formatter reports server.rs/onboarding.rs. Main incident2622024ed547c915 preserves the observed facts and [full log](../../../research/data/2026-11-04-hint-authority-gate.log.gz); no cause or authorship is assigned from timing. This audit edits only reports/plan/memory/evidence, and does not fix or qualify those shared source changes. Do not describe the checkout or proposed paid workflow as green.

## Resume

Preparation completed without provider calls: canonical declaration and prior-context absent/present instruction templates frozen in `/home/marius/.cache/codescout-resident/codescout/hint-authority-effort-20261104/frozen-inputs.json`. Global3014B unchanged, project2274B versus2405B, explicit four effort/declaration cells. Candidate frozen client hash4d704a8a and previously real-store-tested guest Codescout packagecdc53728 are recorded. This is instruction-template preservation, not actual hook delivery, a fresh functional task, or paid admission. See [frozen inputs](../../../research/data/2026-11-04-hint-authority-inputs.tar.gz) and [execution plan](../plans/2026-11-04-hint-authority-effort-comparison.md).

Target confirmed: hook-authority declaration. Freeze the exact absent/present instruction bytes and effective common runtime before paid comparison. Diagnose delivery and hint discrimination first; retain independent task-quality evaluation as the end goal.
