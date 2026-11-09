---
id: '6ee892808b15af87'
kind: bug
status: taken
title: call_graph hides a recoverable hop failure as an unqualified empty edge set
tags:
- cluster/unclassified
- call-graph
- diagnostics
topic: ecc-integration
claimed_at: 2026-11-09
claimed_by: 01a1bf68-e650-7f20-911e-b53f86597c27
opened: 2026-11-09
related:
- f8d7f28200680f34
- '895721298144d276'
severity: medium
unverified: Code-inspected conditional reporting defect; consumer-boundary reproduction and correction are pending.
---

# BUG: call_graph hides recoverable hop failures as empty analysis

## Summary

CachedResolver::one_hop catches RecoverableError from resolve_one_hop and substitutes an empty edge vector so traversal can continue. It does not disclose that failed hop. The outer tool only collects definition-scan incompleteness, so a failed resolver can be rendered like a fully analyzed empty/partial graph.

## Class fit

Unclassified diagnostic-integrity failure at a best-effort analysis boundary. A recoverable inability to analyze a particular symbol/direction is erased rather than made visible beside the surviving result. Unlike the related fixed Phase B issues, this concerns errors after a definition has already been located, not incomplete definition discovery.

## Symptom (Effect)

Conditional path: a seed or non-seed location is available → no usable call-hierarchy item → callees fallback cannot support/read/locate a function → RecoverableError → CachedResolver::one_hop returns no edges → normal BFS/format_output result with no completeness_warning for that failure. The compact consumer has no reason to distinguish the result from a genuine leaf.

## Reproduction

Static dataflow confirmed at baseline b3d02d482eaeb3cabf329f4f37a2d3649a6057b7. A lower-level existing test, resolve_one_hop_callees_without_lsp_returns_recoverable_error, verifies an unsupported-language error but does not exercise the outer catch. A proposed consumer-level fixture is a resolved non-function symbol with no LSP call hierarchy, alongside a real leaf and a positive incoming edge control. Fixture viability and actual response still need execution; they are not yet claimed.

## Environment

ECC dogfood navigation worktree `/home/marius/work/claude/codescout/.worktrees/ecc-navigation-truth`, branch feat/ecc-navigation-truth-20261109. Frozen baseline and source fingerprints in the navigation evidence directory; no production edits yet.

## Root cause

src/tools/symbol/call_graph/mod.rs, impl OneHopResolver for CachedResolver::one_hop: RecoverableError arm returns vec![] and records no failure. CallGraph::call only attaches resolver.incomplete() (definition-walk coverage). format_compact prefixes completeness_warning if present, but this path never creates it.

## Evidence

Inspected enclosing call, catch, BFS, fallback and renderer bodies, plus actual references/call_graph caller navigation. ADR 2026-08-27 requires suspicious negatives to name scope and trustworthy negatives to stay quiet. Existing scan warnings are retained behavior and are not rediscovered as new defects. [Plan](../trackers/ecc-integration/plans/2026-11-09-navigation-dogfood.md).

## Hypotheses tried

- All resolver errors propagate: rejected by the explicit RecoverableError catch.
- Existing scan warning covers the failure: rejected; that state records definition lookup/walk gaps only.
- Every empty graph is wrong: rejected; a fully analyzed leaf should remain quiet.

## Fix

Pending. Preserve best-effort edges and disclose actual failed symbol/direction/reason through the established completeness_warning path, without pretending it was a scan-cap failure.

## Tests added

None yet. Consumer-boundary RED and actual MCP controls are planned; inspected helper tests are not credited as outer-tool verification.

## Workarounds

Treat an unqualified empty graph cautiously if the language/file/symbol cannot support its requested analysis; inspect direct references and the relevant definition. This is a workaround, not a claim every zero is incomplete.

## Resume — 2026-11-09

Live investigation owned by the current Codex session. First finish the clean baseline integration gate, then reproduce the reporting boundary before changing code. No no-ECC replay has run.

## References

- [Negative-result contract](../adrs/2026-08-27-negative-results-name-their-scope.md)
- [Navigation dogfood plan](../trackers/ecc-integration/plans/2026-11-09-navigation-dogfood.md)
- [Resolver](../../src/tools/symbol/call_edges/resolver.rs)
- [Outer tool](../../src/tools/symbol/call_graph/mod.rs)
