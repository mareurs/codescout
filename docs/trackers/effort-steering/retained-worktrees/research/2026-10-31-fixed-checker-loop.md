---
id: '61de41a0aaa04acc'
kind: research-note
status: archived
title: 'Historical: A fixed callable checker lets low repair the known Ruff failure'
tags:
- effort-steering
- historical
- retained-worktree
topic: effort-steering
historical: true
original_rel_path: docs/research/2026-10-31-fixed-checker-loop.md
source_body_sha256: 52b87bb475d4f055f724d40bb16613c15fbcf1f867453a071d75c81b3569c3c4
source_snapshots:
- source: /home/marius/.codex/worktrees/615e/codescout/docs/research/2026-10-31-fixed-checker-loop.md
  sha256: '40273cd6a73bb5ca8f210e02ccc84f642161947e5eedbbb70275233b7194367d'
  body_sha256: '52b87bb475d4f055f724d40bb16613c15fbcf1f867453a071d75c81b3569c3c4'
- source: /home/marius/.codex/worktrees/ca35/codescout/docs/research/2026-10-31-fixed-checker-loop.md
  sha256: '40273cd6a73bb5ca8f210e02ccc84f642161947e5eedbbb70275233b7194367d'
  body_sha256: '52b87bb475d4f055f724d40bb16613c15fbcf1f867453a071d75c81b3569c3c4'
---

# Historical retained-worktree snapshot

This is archived evidence, not a current plan or launch permission. Start with the [experiment README](../../README.md). Original source paths, hashes and any body variants are recorded in [the manifest](../manifest.json).

Original logical path: docs/research/2026-10-31-fixed-checker-loop.md

---


# A fixed callable checker lets low repair the known Ruff failure

## Result

On the same saved failing Ruff draft, both low-effort repairs with a callable fixed acceptance checker and explicit authority/command instructions passed **9/9**. Both diagnostic-only controls stayed at **8/9**. Treatment repair cost averaged **$0.1221**, versus **$0.2716** for control, in model-client estimates.

Recorded in the mechanism tracker as LMC-2. The prospectively registered candidate criterion is met: treatment **2/2 accepted**, control **0/2**, all four runs valid and source-reviewed. This supports the bundle on this deliberately known development case. It does not establish low/xhigh parity, a population success rate, external validation, or which component caused the difference.

## Registered contrast

Task: `astral-sh__ruff-15394`. Every actor starts from the exact saved low draft in the preceding snapshot-workflow phase, with the same original issue and complete independent failure log. Four fresh sessions run in fixed order **G1, T1, T2, G2**, all `claude-sonnet-5-5` at low effort, pinned Claude CLI 2.1.309.

Control retains the existing author Cargo shim, which checks editable author fixtures/snapshots, and generic repair instructions. Treatment adds a generic rule to satisfy the fixed acceptance contract rather than update it, plus a standalone check command. Its exact configured Cargo invocation captures current source and runs the independent checker with official test paths restored in an ephemeral offline container:

`cargo test --color never --package ruff_linter --lib rules::flake8_pie`

The actual log and exit status return to the actor. Fixed-oracle artifacts are not exported. Other Cargo commands retain their existing author behavior. No token-level solution, previous successful patch or new diagnosis is added to either prompt.

Per-call allowances are $0.80, with a $0.40 reserve: **$3.60 phase reservation**, inside the existing **$40 cumulative authorization**. Registration and source-bound negative/positive controls preceded payment. No paid restart, replacement or retry occurred.

## All outcomes and cost units

| Slot | Arm | Independent result | Change from common seed | Repair client USD |
|---|---|---|---|---:|
| G1 | Generic diagnostic | 8/9, rejected | Author snapshot only | 0.2521139 |
| T1 | Fixed checker + scaffold | 9/9, accepted | Production fix only | 0.1028876 |
| T2 | Fixed checker + scaffold | 9/9, accepted | Production fix only | 0.141226 |
| G2 | Generic diagnostic | 8/9, rejected | Author snapshot only | 0.2911095000000001 |

Control total: **0.5432234000000001**. Treatment total: **0.2441136**. Phase total: **0.7873370000000001**. Cumulative model-client estimate: **22.631681199999999770**; remaining authorization: **17.368318800000000230**.

These are four distinct CLI trajectories, not four API requests. The accounting predicate is one raw total per started slot, with matching session, receipt and raw SHA. Invalid started calls would count too; all four actual runs passed the registered validity checks. The independent raw-charge reader was calibrated against all six charges of the sealed predecessor, reproducing every charge and its **2.139629699999999885** total exactly, ratio 1; all new charges also match the inherited consumer.

Repair cost is measured here. Adding the historical draft-generation cost **0.2021449** per attempt gives reconstructed mean draft-plus-repair costs **0.47375660000000005** for control and **0.3242017** for treatment. Those are accounting reconstructions: this phase reused the draft rather than regenerating it four times. They are not newly measured end-to-end workflow costs. Actual billing, native orchestration/reviewer effort, CPU cost and a latency comparison are unknown. No fresh matched xhigh cohort was run in this phase.

## What the mechanism changed

Both controls left production code byte-identical to the failed seed. Their final complete patches are byte-identical to one another: each rewrote the editable snapshot with stripped trailing spaces, then encountered snapshot/permission problems. Generated author artifacts were actually read. Local snapshot work did not address the fixed acceptance failure.

Both treatments changed the production fix. One collects individual opening-parenthesis ranges; the other filters leading token ranges. Both replace a broad leading deletion with separate deletions of syntax tokens, preserving intervening comments and blank-line trivia. Existing trailing and empty-dictionary handling remains. No Cargo/dependency change, assertion removal or independent-oracle bypass was observed.

Each treatment actually executes the fixed checker on the seed and gets 8/9, edits production, then executes it on its final source and gets 9/9. The final actual tool result matches the complete source-bound success log. T1 has one additional **unpaid preflight** trace, excluded explicitly from agent checks. Opaque UUID names are not execution order.

The operational candidate is: **make acceptance expectations immutable; expose a source-bound checker during repair; tell the actor which contract to satisfy; independently accept the resulting code**. This finite comparison supports that combined workflow. It does not separate prompt authority from callable feedback, prove the check was necessary, or measure model comprehension.

## Delivery, adherence and limits

Actual primary assistant/user records were matched by unique tool IDs, excluding historical API-request repeats: G1 27/27 tool uses/results, T1 8/8, T2 12/12, G2 29/29. These count tool records, not executed shell subcommands or API requests. All recorded commands and matched result metadata were inspected; full results remain private.

T1's first command adds a pipe and git diff despite the standalone-first instruction; its underlying Cargo argv still routes through the fixed oracle, and its final command is standalone. T2 follows the standalone instruction initially and finally. Permission denials are preserved; denied requests are not executed actions. Piped control commands sometimes return a successful pipeline status while their Cargo output reports failure. No strong confinement or full prompt-adherence claim follows from registered source/model/evaluator validity.

The controller had already seen the previous xhigh correction. This task and seed are intentionally exposed development material, not a holdout. Assignment was a fixed order, not randomized; shared-host execution and the earlier project gate are not latency controls. The endpoint is the fixed nine named tests plus source-bound task-preservation review, not all authored tests or all Ruff correctness. General validation requires disjoint tasks and a frozen workflow.

## Verification and implementation limits

The full required project gate exited 0: Python light, formatting, clippy, lean and default lanes. Its printed exclusions remain: three heavy Python suites, server-stack and two local ONNX weight tests; librarian is disabled in lean and exercised in default.

One fresh strongest-model prepayment reviewer applied six selected mutations: initially **3 killed, 3 survived, 0 invalid**. The parent read each survivor before writing regressions. One fix pass addressed five Important findings: stale model-validity flags, an omitted dynamic checker dependency, and gaps covering success status, actual worker source validation and receipt-cost agreement. Final pristine suite: **15 passing tests**; replaying the same six selected mutations: **6 killed, 0 surviving, 0 invalid**. This is not exhaustive mutation coverage. No second review was dispatched; no Minor findings were deferred.

Two unpaid setup/parser mistakes were reproduced and fixed before registration; partial setup and actual failures were retained. A later task-helper rerun hit a fixture tied to the live registration directory. The unchanged suite passes in the isolated copy; frozen tests were preserved. The helper failure was inspected only after a batched call had advanced to G1; this process deviation is recorded. A trace analysis initially mistook UUID lexical order for chronology and failed before review creation; the next admission safely refused the missing review, then passed after corrected source/result binding. Neither event caused a paid retry.

## Execution decisions

The private ledger records these decisions and their limits:

- Continue inline under standing autonomous authorization; an unhelpful diagnostic could consume at most the $3.60 reservation.
- Preserve the research ledger/evidence for auditability; the tradeoff is storage.
- Judge paid behavior and cost after actual outcomes; otherwise the mechanism claim would be unsupported.
- Report confinement/adherence limits separately; otherwise protocol adherence would be overstated.
- Keep billed/native/CPU costs unknown; otherwise total-cost claims would be misleading.
- Limit acceptance to named tests plus source review; otherwise correctness/generalization would be overstated.
- Parent verifies registration, gate, commit and seal; otherwise publication could be premature.
- Bound review to six selected mutations; otherwise coverage/concurrency confidence would exceed the evidence.

## Evidence and next test

Private evidence root: `/home/marius/work/claude/codescout/.codescout/resident-private/fixed-checker-loop-20261031`. Registration, controls, raw sessions/charges, exact sources, reviews, tool results, applied mutation logs, gate output, analysis and progress ledger are retained there. The [private analysis](/home/marius/work/claude/codescout/.codescout/resident-private/fixed-checker-loop-20261031/analysis.json) records the complete outcome. After commit, the final outcome seal binds the commit and immutable evidence. The registered plan/spec remain frozen, so their checkbox state is not the completion authority.

The next informative step is external validation of this unchanged bundle on disjoint tasks, counting initial generation, checks and repair, with a matched xhigh workflow as needed. Do not tune the prompt or acceptance tests on those outcomes and then call them validation.

Related records: [registered design](../superpowers/specs/2026-10-31-fixed-checker-loop-design.md), [implementation plan](../superpowers/plans/2026-10-31-fixed-checker-loop.md), [preceding matched replication](2026-10-31-snapshot-workflow-replication.md), [mechanism tracker continuation](../trackers/low-effort-quality-mechanisms-snapshot-workflow.md).
