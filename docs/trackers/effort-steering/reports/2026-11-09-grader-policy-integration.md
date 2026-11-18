---
id: '92115bb035d5c9c8'
kind: report
status: active
title: Grader policy source preservation and integration — 2026-11-09
tags:
- effort-steering
- evaluation
- source-custody
topic: effort-steering
---

# Grader policy preservation and integration

## Current state

**CHANGES REQUIRED — 2026-11-10.** The later independent review found F1/F2/F4/F5 defects at 23bd63dca; they were reproduced before correction. Its earlier green gate and byte-preservation statements below remain dated evidence, not approval to integrate it. The completed [correction report](2026-11-10-policy-review-corrections.md) owns follow-up commit f586d35f4 on branch fix/grader-policy-review-20261110, with a new checked capsule and fresh gate. Main integration remains separate. No main merge or paid/private rerun.

Preservation and the integration candidate are committed and verified. Preservation commit: 1d48e48d5. Integration candidate: 23bd63dca161eb491190d877a2207303458e8921 on fix/grader-policy-integration-20261109. The latter includes ten source/test paths and a checked evidence capsule, with no tracked working-tree delta. Runtime provenance counter/lock files remain untracked.

The candidate is tested against experiments base d124e7738. Main advanced during qualification to e7eff4c76 with changes in three Rust paths; 23bd63dca is not in its ancestry. No main merge or push is claimed. No new model run, private regrade, paid call, historical parser rewrite or historical verdict change. The experiment's scientific conclusion is unchanged.

## Preserved source

Commit 1d48e48d58402e77d5af9a7ab5c4bee224bb0949 on fix/grader-policy-preserve-20261109 preserves the six reviewed policy-v2/classifier corrections. Stable patch-id: d71f94f9c8eece79f35bf1a807ce9574914c9e44. All six committed blobs match the previously frozen snapshot. All 5,179 tracked source paths captured by the old final gate matched before and after the fresh preservation gate. The original add2 working tree and its staged source remain in place; its dated UNCOMMITTED receipt is not rewritten.

Fresh preservation validation: baseline five suites 245 passed / 119 subtests, corrected five suites 263 passed / 129 subtests. Full product gate exited 0 in 427 seconds, with Python light, fmt, clippy, lean and default lanes green. Its printed heavy/feature/ONNX exclusions remain. The setup-ab-eval suites are separate from the gate's Python test collection; their counts must not be added. Normal commit hooks passed, including ownership and exact committed-byte checks, without bypass.

The commit carries Session-Id codex:01a15ca9-f939-7603-acb5-01c6b02f68e2 and Co-Authored-Session-Id b3485581-2540-4da8-a440-3bed2e9c517f. The peer's classifier source corrections are byte-identical to 6d6a1dd52. No peer branch was changed by this session.

## Integration candidate

Workspace /home/marius/.codex/worktrees/e646/codescout; branch fix/grader-policy-integration-20261109, parent d124e7738f78b220f64080b5ea2c612daa5631ba. Commit 23bd63dca161eb491190d877a2207303458e8921; stable patch-id 3fc60bac014324590a75b9d6e1d7696b67a31c4b. The combined source patch includes a2ffc1432, 867c68092, the reviewed corrections preserved in 1d48e48d5, and both test changes from peer commit 2794bb196. The peer research-note delta is not copied over the canonical bug records.

Five directly relevant suites pass: 272 tests / 133 subtests in 20.81 seconds. The larger count includes newer-main tests and is not an effect size. The full product gate exited 0 in 407 seconds: Python light, fmt, clippy, lean and default all green, with the same printed exclusions. Its Python population is separate from the five directly relevant suites.

All 5,818 captured tracked-file hashes match before/after that gate. All ten committed source blobs match that gated source. The two evidence-only data files were added afterward and reopened byte-for-byte, including every capsule member. Normal commit hooks passed with both sessions' correct trailers; the informational bare-peer-ID warning correctly describes a Claude author. The main checkout's source and existing index were not changed.

## Policy boundaries

The live grader sets non-null error plus infra only for a recognized toolchain crash with zero parsed statuses. The prospective policy deliberately invalidates a genuine toolchain fatal signal even with statuses; its declared contract differs from the older live-grader plan. Candidate test-binary aborts are not toolchain crashes. Captured stdout/stderr, nested Cargo output and unselected doctest errors do not automatically override selected acceptance; ambiguous post-capture compiler evidence invalidates the measurement. Failed rebuilds require their own compiler evidence.

These changes qualify measurement, not low-effort repair quality. They do not turn the historical exposed pair into independent validation. The generic evidence-protocol pilot remains closed to paid admission until its own current arm/runtime/evaluator checks pass.

## Evidence custody

[Manifest](../evidence/grader-policy-integration-20261109/manifest.json) and [verification capsule](../evidence/grader-policy-integration-20261109/verification.tar.gz) are committed in 23bd63dca and copied byte-for-byte to the canonical main tracker folder. Capsule: 1,344,570 bytes, SHA-256 f6dab3ea168585abaa5d7e4a18c69824bb293946a25e1440b9130b8a6e85a5c4. All 13 named members were reopened and matched their recorded sizes and hashes. The capsule contains raw fresh logs, source-hash receipts and the exact staged source patches; it contains no new model or private-grade outcome.

The final commit-identity receipt and full scratch remain in /home/marius/.cache/codescout-resident/codescout/grader-policy-commit-20261109. Old frozen bundles and original source worktrees remain in place. Related records: [policy attribution bug](../../../issues/2026-11-07-grade-policy-misattributed-unselected-doctest-compiler-error.md) and [linker classification bug](../../../issues/2026-11-06-private-grader-linker-crash-counted-as-model-failure.md).
