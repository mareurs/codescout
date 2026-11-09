---
id: bb2dea1f4c57158f
kind: bug
status: fixed
title: call_graph hides a recoverable hop failure as an unqualified empty edge set
tags:
- cluster/unclassified
- call-graph
- diagnostics
topic: ecc-integration
closed: 2026-11-09
fix_commit: 3fe613bddca6ddd344164f6ece3278be56186c1a
fix_patch_id: 4b7cad6bf7b5eda6f600304f4969fb13929b5eeb
opened: 2026-11-09
related:
- f8d7f28200680f34
- '895721298144d276'
severity: medium
unverified: 'CLEARED 2026-11-09: outer Tool and real MCP regression, applied mutations, independent review and full frozen-tree gate passed. Integrated source/build/test surfaces match verified state; intervening peer research files are separately disclosed.'
---

# BUG: call_graph hides recoverable hop failures as empty analysis

## Summary

CachedResolver::one_hop catches RecoverableError from resolve_one_hop and substitutes an empty edge vector so traversal can continue. It does not disclose that failed hop. The outer tool only collects definition-scan incompleteness, so a failed resolver can be rendered like a fully analyzed empty/partial graph.

## Class fit

Unclassified diagnostic-integrity failure at a best-effort analysis boundary. A recoverable inability to analyze a particular symbol/direction is erased rather than made visible beside the surviving result. Unlike the related fixed Phase B issues, this concerns errors after a definition has already been located, not incomplete definition discovery.

## Symptom (Effect)

Conditional path: a seed or non-seed location is available → no usable call-hierarchy item → callees fallback cannot support/read/locate a function → RecoverableError → CachedResolver::one_hop returns no edges → normal BFS/format_output result with no completeness_warning for that failure. The compact consumer has no reason to distinguish the result from a genuine leaf.

## Reproduction

Observed on frozen baseline `b3d02d482eaeb3cabf329f4f37a2d3649a6057b7`: a real Python file has a class, a function calling the class, and a leaf function. Controlled external LSP capability returns no call-hierarchy item, forcing the real tree-sitter fallback. The outer CallGraph::call test first confirms the leaf is empty without a warning, then confirms the build→Marker incoming TS edge survives and callees is empty. Only then it fails at the absent completeness_warning for the actual no-enclosing-function recoverable error. RED exit 101, 1 test failed. The earlier real MCP class probe is a control, not failure evidence: that backend can provide class call hierarchy.

## Environment

Original navigation baseline b3d02d482eaeb3cabf329f4f37a2d3649a6057b7, tree34977f2047694714e5f852ee427c2dfca435d9e8. Source worktree feat/ecc-navigation-truth-20261109 committed as4a0ba58564a39e4bb5688f1f2421c0e0172234b3. Frozen full-gate worktree includes doc commit09c35b678 plus exact source correction. Local experiments integration3fe613bddca6ddd344164f6ece3278be56186c1a has identical code/build/test surfaces; peer research-only differences are disclosed in the integration record.

## Root cause

src/tools/symbol/call_graph/mod.rs, impl OneHopResolver for CachedResolver::one_hop: RecoverableError arm returns vec![] and records no failure. CallGraph::call only attaches resolver.incomplete() (definition-walk coverage). format_compact prefixes completeness_warning if present, but this path never creates it.

## Evidence

Inspected enclosing call, catch, BFS, fallback and renderer bodies, plus actual references/call_graph caller navigation. ADR 2026-08-27 requires suspicious negatives to name scope and trustworthy negatives to stay quiet. Existing scan warnings are retained behavior and are not rediscovered as new defects. [Plan](../../trackers/ecc-integration/plans/2026-11-09-navigation-dogfood.md).


Actual stdio MCP with real Python LSP also reproduced the missing disclosure for module-level constant TOKEN. Hash-verified baseline binary returns callees:0 without a warning; the candidate adds TOKEN/callees and the actual no-enclosing-function reason, while both keep a true leaf quiet and a real caller edge. [Report and raw evidence](../../trackers/ecc-integration/reports/2026-11-09-navigation-dogfood.md). This regression probe is not an agent replay.

## Hypotheses tried

- All resolver errors propagate: rejected by the explicit RecoverableError catch.
- Existing scan warning covers the failure: rejected; that state records definition lookup/walk gaps only.
- Every empty graph is wrong: rejected; a fully analyzed leaf should remain quiet.

## Fix

Preserve best-effort edges, record actual recoverable failed symbol/direction/reason, and merge into completeness_warning alongside existing definition-scan disclosure. Display five failure descriptions and the actual total. Source commit `4a0ba58564a39e4bb5688f1f2421c0e0172234b3`, integrated on experiments as `3fe613bddca6ddd344164f6ece3278be56186c1a`, stable patch-id `4b7cad6bf7b5eda6f600304f4969fb13929b5eeb`. Full gate passed the frozen verification tree. Integrated code/build/test surfaces match that tree; only three intervening peer research files differ globally. No push or shared-server rollout.

## Fix provenance

- **SHA:** `3fe613bddca6ddd344164f6ece3278be56186c1a` (`experiments`)
- **patch-id:** `4b7cad6bf7b5eda6f600304f4969fb13929b5eeb`

## Tests added

Outer Tool regression plus helper regression for actual observed merge/bound survivors: focused suite 36 passed. Real stdio MCP baseline/candidate probe confirms missing vs truthful constant-analysis disclosure, quiet leaf and preserved positive edge. Independent review initially applied five mutants (three killed/two survived); both exact survivors were reapplied after the new test and killed, each 35 passed/1 failed. Source and isolated restoration verified. Full gate: lean6309 passed/48 skipped; default9198 passed/62 skipped; Python1764 passed/1 skipped/554 subtests; all lanes zero. [Report](../../trackers/ecc-integration/reports/2026-11-09-navigation-dogfood.md).

## Workarounds

Treat an unqualified empty graph cautiously if the language/file/symbol cannot support its requested analysis; inspect direct references and the relevant definition. This is a workaround, not a claim every zero is incomplete.

## Resume — 2026-11-09

Fixed and integrated locally on experiments at `3fe613bddca6ddd344164f6ece3278be56186c1a`, stable patch-id `4b7cad6bf7b5eda6f600304f4969fb13929b5eeb`; archived after verification. Baseline remains b3d02d482 for future replay; no replay run. Next user-requested task audits active call_graph prompt promotion and removes superseded history from active guidance.

## References

- [Negative-result contract](../../adrs/2026-08-27-negative-results-name-their-scope.md)
- [Navigation dogfood plan](../../trackers/ecc-integration/plans/2026-11-09-navigation-dogfood.md)
- [Resolver](../../../src/tools/symbol/call_edges/resolver.rs)
- [Outer tool](../../../src/tools/symbol/call_graph/mod.rs)
