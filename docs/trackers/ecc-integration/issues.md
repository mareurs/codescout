---
id: d28f5200f736af9c
kind: tracker
status: active
title: ECC integration and evaluation issues
tags:
- ecc
- codex
- evaluation
topic: ecc-integration
---

# ECC integration and evaluation issues

## Disabled-control ambiguity — mitigated 2026-11-09

The supplied per-command plugin enabled=false override did not produce a disabled skills/list result. The cause and every runtime consumption path have not been established. The protocol therefore compares explicit adapter use against explicit non-use in the same installed setup; it does not call the baseline plugin-absent. Original evidence: /tmp/ecc-real-doc-create-20261109-1/preflight.json; adapter session-log F-5.

## Evidence limits — active

A single ordered pair cannot establish general accuracy, reliability or performance gains. Both arms expose pilot metadata. Source/config hashes bracketed the runs. The create/guard/adapter/dispatcher/server files matched, but src/tools/core/cap_probe.rs and src/tools/native_provenance_fence/holders.rs changed. The strict whole-checkout unchanged control failed; no frozen causal comparison is claimed. Tool warming, caches and concurrent verification affect elapsed time. Native usage counters are not a monetary bill.

## Findings and code bugs

Both runs identified the same statically confirmed conditional cleanup-reporting defect: create's error asserts the row was taken back even when best-effort deletion may fail. [Code bug](../../issues/archive/2026-11-09-doc-create-claims-rollback-after-best-effort-cleanup.md); [adjudication](reports/2026-11-09-doc-create-comparison.md). That earlier comparison injected no runtime failure and applied no fix. The later authorized [dogfood cycle](reports/2026-11-09-doc-create-dogfood.md) reproduced failed deletion, verified a correction in an isolated worktree with MCP/tests/mutations and a passing full gate; the correction is now integrated locally on experiments as `b3d02d482eaeb3cabf329f4f37a2d3649a6057b7` and the full post-integration gate passed. No shared-server rollout occurred. The [navigation cycle](reports/2026-11-09-navigation-dogfood.md) also reproduced and corrected recoverable-hop suppression through both outer Tool tests and real MCP. [Archived navigation bug](../../issues/archive/2026-11-09-call-graph-hides-recoverable-hop-failures.md), integration3fe613bdd, stable patch-id4b7cad6bf7b5eda6f600304f4969fb13929b5eeb. Full frozen-tree gate passed; code/build/test surfaces match integration, with peer research-only global tree drift disclosed.

The subsequent [create-finalization dogfood](reports/2026-11-09-create-finalization-dogfood.md) confirmed partial catalog state through actual slug/augmentation/registration failure probes; its correction is verified and integrated on experiments as `6b2befb1eb9344e7a37f1fe660ec7de501276028`, stable patch-id `ba636938caee459b5457133127e8d919038a6758`. [Archived atomicity bug](../../issues/archive/2026-11-09-doc-create-leaves-partial-artifacts-on-finalization-failure.md). The initial M7 coverage survivor was observed before closing it with worktree fixture branches and an exact killed recheck (bug-fix-session-log:F-223). An initial stale restored-control binary and source-bound lease workaround remain explicit (F-224). Catalog-location discovery and canonicalization fallbacks remain unconfirmed contract/reachability questions.

## Prompt-routing instrument — fixed-verified 2026-11-09

The [prompt audit](reports/2026-11-09-callgraph-prompt-surface.md) uncovered two scorer false positives through actual trace perturbations, recorded as bug-fix-session-log:F-221. Hardened fixture-specific scoring rejects both (0/2 survive), passes16 controls and preserves the independently verified13 original results. Actual tool evidence is completed Codex events; wire capture contains initialization/list only. Requested model/effort and generated-config hash are retained, without backend attestation or immutable profile inventory. The satisfactory base supports no routing edit in this fixture; it does not establish effectiveness of the default ECC/plugin profile or replay the repair tasks.

## Incidental navigation — fixed 2026-11-09

[Duplicated Rust call-name callee lookup — fixed](../../issues/archive/2026-11-09-call-graph-generic-call-returns-unrelated-callees.md), artifacte5016b19fb0f75da, original observation F-222. The [identity dogfood cycle](reports/2026-11-09-callgraph-identity-dogfood.md) reproduced current-source warm/restart cross-definition contamination and integrated the completed-resolution cache correction as4776e44bd, patch-id82470b4a164f780eab0ce409ca01dfafc29f0d7a. Final84 related tests, qualified native6/6, initial independent review8killed/2survived followed by two exact killed rechecks after test strengthening, and both source/integration full gates passed. Baseline incoming0 runtime control remains nonqualifying; full declaration-identity BFS is outside scope.

[Resolver doc-comment contradiction](../../issues/2026-11-09-call-graph-resolver-doc-denies-outgoing-ts-fallback.md), artifact81436f2c48ad8c74: pre-existing, open and nonblocking. The internal strategy comment still denies the implemented outgoing AST fallback; no source edit for it in this patch.

## Scope boundary

Only two adapted Codex skills are in this pilot. Full ECC adoption, automatic orchestration changes and broader rollout are separate decisions. Keep the user's backup/revert path in [notes](notes/README.md).
