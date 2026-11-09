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

Partial state after augmentation/registration failure, catalog-location discovery and canonicalization fallbacks remain open contract/reachability questions, not extra confirmed defects.

## Scope boundary

Only two adapted Codex skills are in this pilot. Full ECC adoption, automatic orchestration changes and broader rollout are separate decisions. Keep the user's backup/revert path in [notes](notes/README.md).
