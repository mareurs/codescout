---
id: '01e5326bc30647f8'
kind: research-note
status: archived
title: 'Historical: 2026-10-30-prechecked-repair'
tags:
- effort-steering
- historical
- retained-worktree
topic: effort-steering
historical: true
original_rel_path: docs/research/2026-10-30-prechecked-repair.md
source_body_sha256: 11b28c0df8ebb4bb8ffd8e2d01789729337b1158786cb915ac535b06f8cc4808
source_snapshots:
- source: /home/marius/.codex/worktrees/615e/codescout/docs/research/2026-10-30-prechecked-repair.md
  sha256: '11b28c0df8ebb4bb8ffd8e2d01789729337b1158786cb915ac535b06f8cc4808'
  body_sha256: '11b28c0df8ebb4bb8ffd8e2d01789729337b1158786cb915ac535b06f8cc4808'
- source: /home/marius/.codex/worktrees/ca35/codescout/docs/research/2026-10-30-prechecked-repair.md
  sha256: '11b28c0df8ebb4bb8ffd8e2d01789729337b1158786cb915ac535b06f8cc4808'
  body_sha256: '11b28c0df8ebb4bb8ffd8e2d01789729337b1158786cb915ac535b06f8cc4808'
---

# Historical retained-worktree snapshot

This is archived evidence, not a current plan or launch permission. Start with the [experiment README](../../README.md). Original source paths, hashes and any body variants are recorded in [the manifest](../manifest.json).

Original logical path: docs/research/2026-10-30-prechecked-repair.md

---

# Prechecked repair diagnostic — 2026-10-30

An independently observed failure, delivered before repair, produces successful low repairs in both diagnostic calls. Generic low review succeeds once and leaves the defect unchanged once; generic xhigh succeeds. Both diagnostic prompts are verified in their actual sessions. It supports this repair component on one selected development defect, without establishing general low/xhigh parity or a population-level improvement.

## Mechanism and fixed comparison

The mechanism is: build and check the exact draft; give low the actual failing command, complete input/output and expected output before it starts repair; independently grade the final source and preserve compatibility. No source fix or successful implementation is supplied.

Every call starts from the exact saved U implementation of the preceding phase. That source already handles empty suffixes, but loses JSON highlighting when custom `-old` overlaps built-in `.dpkg-old`. All actors get the same generic merge-preparation review instructions; only D1/D2 receive the precheck diagnostic. Model claude-sonnet-5-5, privately pinned CLI2.1.309, tools and initial source are fixed. Separate profiles/workspaces deny access to the other arms and earlier studies.

Order is fixed before payment: G1,D1,X,D2,G2. The two low pairs reverse treatment order around one descriptive xhigh reference. No model subagents, paid retry, task replacement, outcome-conditioned suppression or protocol restart occurs.

| Slot | Assigned prompt / effort | Final CLI cases | Fixed upstream tests | Diagnostic observed | Client USD | CLI seconds |
|---|---|---|---|---|---|---|
| G1 | generic / low | 6/7 | 80/80 | no | 0.0717037 | 58.99 |
| D1 | exact precheck / low | 7/7 | 80/80 | yes | 0.0688737 | 56.40 |
| X | generic / xhigh | 7/7 | 80/80 | no | 0.3170116 | 357.72 |
| D2 | exact precheck / low | 7/7 | 80/80 | yes | 0.0573732 | 52.45 |
| G2 | generic / low | 7/7 | 80/80 | no | 0.1047363 | 105.53 |

Both full diagnostic prompts are present verbatim in their actual primary-session transcripts with matching session and workspace. Generic prompts are likewise verified; they contain no diagnostic. This is initial prompt delivery, rather than a Stop-hook message. The frozen primary population is seven CLI cases plus the same named 80-function upstream target for every slot.

## What the source and checks show

G1's complete final inventory and patch are identical to the initial source. Its only failed CLI case is the overlapping suffix; all existing upstream tests still pass. This failure remains in the result.

D1 and D2 each change the shared helper to continue trying matching suffixes when a callback finds no syntax, stopping only on a successful match. They add overlap regressions and retain prior assertions, empty filtering and error propagation. G2 independently discovers the same repair without diagnostic information, adding a different overlap regression and shell completions. X also fixes fallback, adding helper/integration regressions, documentation, help and completion updates. Complete deltas were reviewed and each review is bound to its final patch.

The finite result is **2/2 diagnostic low accepted, 1/2 generic low accepted, 1/1 generic xhigh accepted**. These are repeated trajectories on one selected defect, not five tasks. Generic G2 shows that the diagnostic is not necessary for every successful low repair. Two trajectories per low condition and one xhigh reference cannot establish a general success rate, causal effect size or effort-equivalence claim. All accepted snapshots agree on these checks, without a claim of identical overall behavior or patch scope.

## Evaluator preparation and verification

Before payment, fresh source-bound controls show the initial draft passes six cases and fails only overlap; the saved successful F control passes seven. All fourteen CLI case processes in those controls exit zero. Output comparison, rather than process success alone, detects the defect: nonempty cases must match the frozen ANSI JSON reference exactly. The six prior boundary cases are preserved; empty suffixes may be ignored with input preserved or rejected cleanly with a suffix-specific diagnostic. Aborts/timeouts cannot pass.

Independent prepayment review finds an actual lost launch lock and three groups of regression gaps. Its deterministic no-network probe reaches two mocked launch boundaries, with zero real model calls. The new phase restores a nonblocking per-phase lock and routes CLI dispatch through it. The [issue record](../issues/2026-10-30-prechecked-worker-bypasses-paid-launch-lock.md) distinguishes private experiment code from shipped production code.

The reviewer actually applies six guard mutations against five tests: one killed, five survived, zero invalid. After observed survivors, new tests reach the actual evaluator, persisted cost/review guards, real prompt verifier and inherited worker callback. Reapplying the same six to the final prepayment suite kills all six. Two separate applied launch controls, removing the lock or bypassing serialized dispatch, are both killed. All eleven frozen prepayment tests pass. Three later evidence-closure tests bring the suite to fourteen; two separate closure controls, removing budget verification or using the obsolete global cap, are both killed. These are distinct mutation populations.

The complete project gate exits zero after 1681.630649897037 seconds: PYTHON_LIGHT,FMT,CLIPPY,LEAN,DEFAULT pass. Printed limits remain: heavy Python suites, server-stack, local ONNX weight tests and librarian code in LEAN; DEFAULT runs librarian tests. No production source changes are published.

## Verification boundaries and accounting

All five actors have at least one successful full Cargo test receipt on their final Rust/Cargo source. The audit retains nine actual Cargo process receipts, including earlier-source and targeted X checks; every recorded process exits zero. Parent verification independently builds and grades every final exact snapshot. The fixed upstream target does not by itself certify every added actor regression; actor execution and parent acceptance are kept distinct.

The same restricted policy permits Cargo test/check, while format/clippy and some compound requests are denied. Tool-request audit checks file paths and reads complete Bash requests; request records do not prove all requested commands ran. Configuration is not a hostile-agent sandbox claim. CLI wall time includes actor work and check overhead on a shared host during the project gate; it is not a general latency comparison.

New client estimate: **USD0.61969849999999995**. Cumulative: **USD17.397892399999999825 of USD40**, remaining **USD22.602107600000000175**. Five calls, zero paid retries. Frozen call allowances total USD3.00 plus USD0.75 reserve; no reserve is needed. Native orchestration/review cost and billed totals remain unknown. These are repair-only costs on an existing draft, not end-to-end generation costs or general savings.

## Decision and next validation

Keep **precheck → precise diagnostic → low repair → independent acceptance** as a candidate mechanism. Do not replace every low call with xhigh merely because one generic review misses a defect; the observed diagnostic low repairs and generic G2 demonstrate alternatives. Do not promote this to general parity from a selected known case.

Freeze this mechanism before the next validation. Use disjoint tasks and a prespecified complete compatibility target, record every draft and any inactive checks, compare with matched xhigh where needed, and count the full workflow's client spend. A check that finds no failure supplies no evidence about diagnostic repair. Do not tune prompts/tests after those held-out outcomes and then reuse them as external validation.

[Design](../superpowers/specs/2026-10-30-prechecked-repair-design.md), [plan](../superpowers/plans/2026-10-30-prechecked-repair.md), tracker LEM-17. Private phase prechecked-repair-20261030 retains registration SHA256448a2509b036719e81688b65d8377a8a9cb8c3e733fa51c40f4d4c1acbd59478, all741input hashes, raw five-session receipts/transcripts, precheck/control/source inventories, complete deltas/reviews, seven-case and 80-function grades, mutation/test/gate logs and commit-bound closure. Previous sealed anchor natural-failure-feedback-20261029/final-outcome.json SHA256df23f863e3428aa28c8ac2fba28396df259d1b00036cdd70e810ebae86c8bce8 remains unchanged.
