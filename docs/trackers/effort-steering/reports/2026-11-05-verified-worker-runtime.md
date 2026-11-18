---
id: a7e431db83176f1d
kind: research
status: done
title: 'Verified worker runtime: final controls and admission checkpoint'
tags:
- effort-steering
- evaluation
---

# Verified worker runtime: final controls and admission checkpoint

## Outcome and limits

The opt-in worker now keeps its candidate and build target under an exclusive disk-backed launch root. Before copying source or accessing credentials, it requires a supported local filesystem and at least 30 GiB available. Native launches add CPU2 and init. Legacy behavior is preserved.

Source commit: `7e57bf8f91a2fd369411a2d89db51cf97c070d9f`; stable patch-id: `528637cc65548ba4cb3f84424d2c8ebf725ec148`. It is local on research/fixed-acceptance-population-20261101 in /home/marius/.codex/worktrees/615e/codescout. The worktree was observed clean after commit. The five exact source paths carry Session-Id codex:01a15ca9-f939-7603-acb5-01c6b02f68e2. No merge or push.

This removes the storage/resource confounds exposed by the separately qualified full Rust reference control. It does not establish low/xhigh repair parity. No model/provider cost was incurred in this stage.

## Regression and review evidence

Initial through-worker tests failed on the old implementation: 8 tests, 11 failing subtests. The minimal implementation passed 8. Existing workflow fixtures then needed storage/argv doubles updated. Final lifecycle suite: 42 passed. Existing pilot, completion and resume suites passed 28, 9 and 3 tests respectively.

Independent review applied 12 mutations: 11 killed, 1 survived. It also reproduced two functional admission failures before credentials: an unknown nonempty filesystem type and Docker argv without an image. Findings were immediately filed in MAIN as acc630025c46bdd7, 9f5f4d1aefb1bd81 and 12f15765c083ab4b.

Expanded tests first produced RED: 8 tests, 7 failing subtests. After the fixes, they passed. Focused final verification ran 42 tests per baseline, restored, mutant and control suite. Eight applied followup variants yielded 5 killed and 3 redundant-guard survivors. The killed count includes 3 conditional paired mutations; 3 additional parser-relaxed controls passed. Do not report zero survivors or eight independent first-order mutants. Original unknown-storage and missing-image defects were both killed by the new regressions. Parent independently checked all five final source hashes and the recorded followup counts.

## Actual final CLI/native controls

Final root: /home/marius/.cache/codescout-resident/codescout/verified-worker-runtime-20261105-final. Job @bg_0000005c exited 0. Independent readback checked the complete population:

| Arm | Control | Requests | Terminal events | Public exits |
| --- | --- | ---: | ---: | --- |
| low | success | 19 | 2 | [0,0,0,0] |
| low | public failure, then repair | 21 | 3 | [0,1,0,0], then [0,0,0,0] |
| low | unknown timeout | 19 | 2 | format 0, then timeout; incomplete |
| xhigh | success | 19 | 2 | [0,0,0,0] |

Every raw request carries its intended effort. Raw terminal events preserve session continuity. All cells use scripted responses, with zero forwarded provider requests and zero model cost. These are short public probes, not model repair results.

Actual Docker witnesses record init=true, NanoCpus=2000000000, PID limit 512, memory 6 GiB, a writable actor at the exclusive output's project directory, and CARGO_TARGET_DIR=/testbed/target. Storage preflight observed btrfs and more than 30 GiB available. Actor Git baselines exist. Twelve independent absence inspections cover the two owned names and worker ID per cell; this does not mean twelve containers.

One disposable wrapper failed before any cells because it omitted its parent directory. Issue 008c7ca561ab4655 records that setup failure. The corrected wrapper creates the exclusive parent and completed all four cells. The failed setup is not counted as a native control or model run.

## Gate and preservation

Full V `./scripts/gate.sh --check` exited 0 as @bg_00000055: PYTHON_LIGHT=0, FMT=0, CLIPPY=0, LEAN=0, DEFAULT=0. It started before the focused Python boundary fixes; final lifecycle tests, final-byte native controls and source-hash checks cover those later changes. No Rust code changed.

The gate's documented heavy-Python, server-stack and ONNX exclusions apply. The tools lifecycle suite ran separately. Commit ownership/format guards passed. Dead-artifact-id lookup was skipped because the hook had no runnable binary.

Durable artifacts:

- [Verification JSON](../../../research/data/2026-11-05-verified-runtime-verification.json)
- [Bounded evidence archive](../../../research/data/2026-11-05-verified-runtime-evidence.tar.gz)
- [Full gate log](../../../research/data/2026-11-05-verified-runtime-gate.log.gz)
- [Preparation inventory](../../../research/data/2026-11-05-workflow-calibration-runtime-inventory.json)

Archive SHA256: `9d76aba72de8a72c1928ba88665754a1c9f3145005fb459882440bca5292b2ef`. All 349 payload files were individually read and hash-verified. The archive retains raw requests, streams, receipts, public logs, Docker witnesses, applied mutation evidence, source snapshots and preparation scripts. Actor clones, client/native homes and private state are excluded.

MAIN documentation and evidence remain local under provenance issue 421ed73df25f795d. The foreign staged bug-triage blob is unchanged.

## Next measured comparison

Prepare a fresh exclusive complete-workflow calibration with low/xhigh and declaration absent, identical full Codescout/global/project instructions, and reused frozen onboarding. Require completed public verification, private acceptance after the repair loop, known complete provider cost, and exact cleanup. Charge failed/aborted attempts and report complete elapsed time.

The preparation inventory measures 36 composed source/config/document files under five tool directories, plus the legacy frozen runtime/input fingerprint. Digest: `bb3ee8c6c57e09c6b848ae7b543d62049104d9a9c10aca6dadec73a5319fb1ed`. Its source HEAD matches the commit above. It is explicitly preparation_only=true and paid_admitted=false.

The final driver, launch inputs, CLI/budget plan, pair reserve and acceptance/context controls still must enter reviewed admission. Old cohorts remain closed. This exposed diagnostic task cannot establish generic transfer; fresh external tasks follow a successful calibration.

Thread spending remains USD31.797419199999999964 of USD40, leaving USD8.202580800000000036. The original pilot remains USD2.57626490000000013 of USD4 and is not reset. No numeric cap changed.
