---
id: '60f77d69c08a644d'
kind: research-note
status: archived
title: 'Historical: Contract-to-assertion diagnostic: observed sensitivity, no discovery'
tags:
- effort-steering
- historical
- retained-worktree
topic: effort-steering
historical: true
original_rel_path: docs/research/2026-11-02-contract-assertion.md
source_body_sha256: d0b77ca8ed95a55eba63c33de6bdcc31f89cf885fdba09939c0352501c40cb61
source_snapshots:
- source: /home/marius/.codex/worktrees/615e/codescout/docs/research/2026-11-02-contract-assertion.md
  sha256: '7396e4aa2f9ae04292e2e5a0064e1eec428ea143faaea4dfb815410a1829820a'
  body_sha256: d0b77ca8ed95a55eba63c33de6bdcc31f89cf885fdba09939c0352501c40cb61
- source: /home/marius/.codex/worktrees/ca35/codescout/docs/research/2026-11-02-contract-assertion.md
  sha256: '7396e4aa2f9ae04292e2e5a0064e1eec428ea143faaea4dfb815410a1829820a'
  body_sha256: d0b77ca8ed95a55eba63c33de6bdcc31f89cf885fdba09939c0352501c40cb61
---

# Historical retained-worktree snapshot

This is archived evidence, not a current plan or launch permission. Start with the [experiment README](../../README.md). Original source paths, hashes and any body variants are recorded in [the manifest](../manifest.json).

Original logical path: docs/research/2026-11-02-contract-assertion.md

---


# Contract-to-assertion diagnostic — 2026-11-02

## Result and decision

**The mechanism did not discover the known remaining defect on this exposed Axum case.** One valid Sonnet5.5 low call produced a public-API test that independently passes the challenged candidate, fixed xhigh reference and gold. The source-bound discovery certificate is false. No second task, retry, repair or sharper hint was run.

There is a useful procedural observation: the agent asserted a directly observed response body, actually changed its expected value, observed a runtime assertion failure, restored it and reran successfully. This verifies sensitivity of that selected assertion. It does not establish that the selected contracts cover the missing behavior.

Do not promote this procedure as a replacement for xhigh. The next development gate is a way to select distinct behavioral cases and establish what each exercises before buying more discovery runs. Another wording-only retry on this exposed case would not supply external evidence.

## Fixed experiment

The frozen [spec](../superpowers/specs/2026-11-02-contract-assertion-design.md) and [plan](../superpowers/plans/2026-11-02-contract-assertion.md) configured only A-L, with a fresh placeholder test on prior Q/E-L for Axum1934, Q/F-L as the fixed xhigh reference, and gold. Added instructions were generic: contract inventory → public operation and observed value → assertion → actual temporary negative control → restore and final run. The original issue was unchanged apart from inherited newline normalization; no hidden fallback diagnosis or previous failing probe was supplied.

The allowance was $0.60 plus a $0.20 reserve. CLI 2.1.309, low effort, model and task/source identities were pinned. The single call completed normally; the worker recorded exit 0 and 58.7895 seconds for the CLI. The inherited acceptance suite also passed, but that endpoint was not the discovery criterion.

## Exact proposal adjudication

| Source role | Exact final reviewer_counterexample | Interpretation |
|---|---|---|
| Challenged low candidate | 1 passed, 0 failed, 0 ignored | No counterexample |
| Fixed xhigh reference | 1 passed, 0 failed, 0 ignored | Positive control passes |
| Gold | 1 passed, 0 failed, 0 ignored | Positive control passes |

All three are behavior executions, with exact source, fixture, command, listing and output bindings revalidated. The only final changed path is axum/tests/challenger_review.rs; production, manifests and existing tests remain fixed. The final test SHA256 is 7b819a124698739678ba38a7c21eeddeb93e2ca2883873843203df40be0e715c.

The test constructs a nested service inside fallback_service and checks HTTP 200 plus a body containing the original request path and false for MatchedPath-extension presence. The known parent control instead exercises a direct fallback handler with Option<MatchedPath>. Those are different selected cases. The retained prepayment control fails the same candidate production source and passes reference/gold; the new proposal does not reproduce that contrast.

The preceding generic reviewer also missed this exposed defect, costing $0.0647718. This new call cost $0.0746715. Different prompts, budgets and single observations do not establish a causal improvement or a success-rate comparison.

## Procedure audit

The file comments record two selected properties: C1 status/non-panic, and C2 original path plus extension presence in the response body. Both have direct observations and assertions.

Four source-bound Cargo execution receipts belong to the model session: an initial timer-configuration panic, a passing corrected test, a failing C2 mutation and a passing restoration. A fifth receipt in the same traces directory is the earlier prepayment placeholder shim smoke. Its source hash equals the registered initial placeholder and its receipt predates model start; it is not counted as another model execution.

The C2 mutation changes the expected body suffix from false to true, also changing its explanatory comment. Captured source otherwise equals the final test. The actual failure shows observed false bytes against expected true bytes, exit 101 and an assertion failure at the body comparison. The restored source hash equals the final proposal and its test passes. The C1 status assertion was not separately mutated; the agent explicitly acknowledged this. **Instruction adherence is partial, and the paid negative outcome remains counted.** The initial disabled-Tokio-timers panic is a test setup failure, not a successful negative control or implementation defect.

The canonical transcript contains 6 assistant tool-use request IDs and 6 corresponding serialized api-request-blob blocks. Only assistant requests and user results count as invocations. An initial all-record duplicate-equality check refused their differently escaped command forms before a count was published. The audit retains all 6 canonical request inputs/results. One compound shell-write request was denied; the subsequent Write/Edit and Cargo executions are distinguished from it.

## Instrument verification and limitations

Fresh actual controls observed placeholder passes on all 3 roles, deliberate assertion failures on all 3 roles, and the known counterexample's candidate-fail/reference-and-gold-pass contrast. The new actor's ordinary Cargo shim smoke passed.

The focused baseline passed 26 tests. One fresh independent strongest-model review applied 6 runtime mutations; all 6 failed targeted assertions. The controller read the exact diffs/results and replayed the same 6 with the same detection result. No Critical/Important findings or second review. One [deferred Minor](../issues/2026-11-02-contract-charge-test-replay.md) remains: the named unknown-charge test measures replay rejection. Separate direct actual state/receipt probes reject missing, null, negative, non-finite and mismatched charges, with a valid-receipt control reaching the next guard.

Unpaid preparation corrected the [old actor-prefix assumption](../issues/2026-11-02-contract-diagnostic-actor-prefix.md) and [missing new evidence-root denials](../issues/2026-11-02-contract-diagnostic-evidence-root.md) through observed failing tests. No model used the failing setup.

The full project gate exited 0: Python light had 1,311 passes and 442 subtest passes; formatting, clippy, lean and default lanes all exited 0. Printed exclusions remain: three heavy Python suites, server-stack, and two local ONNX-weight tests; librarian tests run only in default. Normal commit hooks passed except the disclosed missing-runnable-binary dead-artifact-id skip.

During unpaid inspection the controller mistakenly pinned the sealed previous archive as a Codescout workspace. This added three metadata files without changing/removing any sealed file or link. The additions and empty tool directories were archived separately, the exact prior inventory restored, and its full closure of 4,870 files and 0 links plus nested population closure verified again. This intervention is retained; no workspace/LSP initialization targeted the new live actor.

## Closed client accounting

| Quantity | Model-client USD |
|---|---:|
| This phase, one started valid call | 0.07467149999999999 |
| Prior cumulative | 24.195173699999999820 |
| New cumulative | 24.269845199999999810 |
| Remaining under authorized $40 cap | 15.730154800000000190 |

The independent raw-charge reader was calibrated against the preceding closed call and agrees with the primary state on every started call here. These are reconciled client estimates; actual invoice, native controller/review cost and CPU dollars remain unknown. Paid work is permanently stopped for this phase.

## Preservation and next gate

Private evidence: /home/marius/.cache/codescout-research/codescout/contract-assertion-20261102. Keep its runtime, registration, captures, raw transcript/charge, controls, review, analysis, publication and seal. The original blind-challenger archive stays at its sealed in-repository path; invoke the unchanged compat.py child to access migrated dependencies. Do not move archives or create host aliases.

Public worktree: /home/marius/.codex/worktrees/615e/codescout, branch research/fixed-acceptance-population-20261101. The prepayment five-document commit is 5b2418a45497d3d6b56440207549e0650f709d57, patch-id 77efdd216f03cf498b19dc8124c91c1f1233e04a. Final publication identities and complete evidence inventory are recorded separately under analysis/publication.json and seal.json. Native ledgers/worktree remain; no merge or push.

Next: develop the case-selection instrument offline. It must distinguish “this assertion is sensitive” from “this test reaches the preserved behavior at risk,” without using a hidden gold diagnosis as alleged blind discovery. Only a usable detector justifies freezing whole-workflow repair/escalation and a prospective matched comparison on outcome-unseen cases. This one exposed negative supplies neither general failure frequency nor external validation.
