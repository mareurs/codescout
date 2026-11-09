---
id: '81436f2c48ad8c74'
kind: bug
status: open
title: resolve_one_hop documentation denies the implemented outgoing tree-sitter fallback
tags:
- call-graph
- documentation
- cluster/doc-contradicted-by-code
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

[Independent review](../trackers/ecc-integration/evidence/2026-11-09-callgraph-identity-dogfood/review/review-result.json), findingR2. Parent independently read both actual symbols. Focused active catalog title search returned no matching report; unindexed documents and broader duplicate space were not exhaustively audited.

## Fix

Pending: describe the current outgoing AST fallback, including its error/completeness limits. Keep this separate from the completed-cache identity correction.

## Tests added

None: documentation correction deferred. Existing runtime checks are evidence of the capability, not a regression guard on this prose.

## Resume — 2026-11-09

Open, unclaimed, nonblocking follow-up. No source change applied for this report.
