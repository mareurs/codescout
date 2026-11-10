---
id: f8799aadf848d078
kind: bug
status: fixed
title: resolve_one_hop documentation denies the implemented outgoing tree-sitter fallback
tags:
- call-graph
- documentation
- cluster/doc-contradicted-by-code
closed: 2026-11-10
opened: 2026-11-09
owner: marius
severity: low
---

# BUG: resolver documentation denies the implemented outgoing tree-sitter fallback

## Summary

The resolve_one_hop Strategy doc comment says Direction::Callees returns RecoverableError when LSP call hierarchy is unavailable because outgoing discovery is not viable. The implementation already walks outgoing AST calls through resolve_via_ts → resolve_callees_via_ts. This can mislead agents inspecting the resolver contract.

## Class fit

IC-11 applies: prose denies a capability which the current implementation has gained. Unlike a tool schema mismatch, this instance is an internal Rust doc comment left behind by the existing outgoing AST fallback. The archived Phase B fallback fix describes the earlier missing capability; this report concerns the still-current contradictory comment. No class Members line is changed.

## Reproduction

Inspect src/tools/symbol/call_edges/resolver.rs::resolve_one_hop with include_docs=true, then resolve_via_ts. The former says outgoing fallback is unavailable; the latter's Direction::Callees arm invokes resolve_callees_via_ts using the definition file's grammar.

## Environment

Source69ffdd12e6d51f4884c0190a1a6c4edeca4289e4; re-read in the shared checkout during the ECC callgraph-identity review. Pre-existing and outside the two changed source files.

## Root cause

Strategy prose was not updated with the implemented outgoing fallback. Confirmed source/comment contradiction; no new runtime defect follows.

## Evidence

[Independent review](../../trackers/ecc-integration/evidence/2026-11-09-callgraph-identity-dogfood/review/review-result.json), findingR2. Parent independently read both actual symbols. Focused active catalog title search returned no matching report; unindexed documents and broader duplicate space were not exhaustively audited.

## Fix

Corrected the Strategy doc comment to describe the outgoing AST walk and its inspected recoverable failure conditions (unsupported grammar, unreadable/unparseable source, no enclosing function). Only /// lines change in resolver.rs. Source `9aee4be48e446c15a441397f877c662f265f56ac`; integrated on experiments as `cb7d13ff360f8fb2464a8a2b52f0f87342c00391`; stable patch-id `d2258dc0bcbe010b39e15f2a84680d4cb7036f09`. The same commit also contains a separately reproduced test-fixture correction required to unblock the full gate. Exact integrated-tree full gate also passed on cb7d13ff360f8fb2464a8a2b52f0f87342c00391/tree d563673b185d884dbbfe215d39cad286d32dada0: lean6387/49skipped, default9290/63skipped, Python1827/7skipped/554subtests; fmt/clippy/doc-test lanes passed with printed coverage exclusions.

## Fix provenance

- **SHA:** `cb7d13ff360f8fb2464a8a2b52f0f87342c00391` (`experiments`)
- **patch-id:** `d2258dc0bcbe010b39e15f2a84680d4cb7036f09`

Source worktree commit `9aee4be48e446c15a441397f877c662f265f56ac` has the same stable patch-id and owned file hashes. Source and exact integrated-tree full gates passed; [identity record](../../trackers/ecc-integration/evidence/2026-11-10-resolver-followup/integration.json).

## Tests added

No test that merely asserts prose. Actual resolver bodies, existing direct Rust fallback/unsupported-language tests and the84-test related baseline establish the unchanged documented behavior; fresh private MCP qualifying controls exercise outgoing fallback. Final source full gate passes (lean6386,default9289,Python1827 with recorded skips/exclusions). Initial documentation-only review approved; its three actual scorer mutations were killed. Separate fixture review and its three mutations concern the additional test-only correction.

## Resume — 2026-11-09

Closed2026-11-10 after passing source and exact integrated-tree gates, independent review and observed mutation checks. Fix integrated on experiments as `cb7d13ff360f8fb2464a8a2b52f0f87342c00391`, stable patch-id `d2258dc0bcbe010b39e15f2a84680d4cb7036f09`. [Report](../../trackers/ecc-integration/reports/2026-11-10-resolver-followup.md) retains raw failures, controls and provenance. BFS identity correction remains separate. No executable resolver behavior changed.
