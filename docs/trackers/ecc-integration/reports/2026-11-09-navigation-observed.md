---
id: ba1b43d24d165b8e
kind: spec
status: draft
title: call_graph — bounded observed-behavior draft
tags:
- ecc
- observed-behavior
- navigation
topic: ecc-integration
---

# call_graph — bounded observed behavior, 2026-11-09

## Scope and currency

> Final currency: correction committed/integrated as3fe613bdd; focused36/0, MCP, applied mutation review and full frozen-tree gate passed. The observations below preserve baseline/development evidence states; pending labels are historical and superseded by the [final report](2026-11-09-navigation-dogfood.md).

Observed source baseline `b3d02d482eaeb3cabf329f4f37a2d3649a6057b7`, tree `34977f2047694714e5f852ee427c2dfca435d9e8`; isolated workspace `/home/marius/work/claude/codescout/.worktrees/ecc-navigation-truth`. The baseline is preserved in [evidence](../evidence/2026-11-09-navigation-dogfood/baseline.json). Current uncommitted correction is distinguished below; this draft does not elevate every observed fallback into a desired contract. [Plan](../plans/2026-11-09-navigation-dogfood.md), [execution report](2026-11-09-navigation-dogfood.md).

Inspected: CallGraph::call/format_compact, format_output, CachedResolver::lookup_pos/one_hop/incomplete, scan_incomplete_warning/attach_scan_incomplete, BFS, resolve_one_hop, resolve_via_lsp/resolve_via_ts/resolve_callees_via_ts and their relevant regression assertions. Callers traced with codescout references/call_graph. Deferred: full references::call_with_settle audit, every grammar/backend, unreadable reference-file skips.

## Observed requirements

### Requirement: analyzed empty callees stay quiet

- **Behavior:** WHEN a leaf has no outgoing call expressions and fallback analysis succeeds, THEN the outer tool returns empty callees without completeness_warning.
- **Enforcement:** src/tools/symbol/call_edges/resolver.rs resolve_callees_via_ts; src/tools/symbol/call_graph/mod.rs CallGraph::call.
- **Evidence state:** executed-pass for real leaf control in both baseline RED test (before later failure) and corrected focused suite; initial real MCP control also returns zero. Test `recoverable_hop_failure_is_disclosed_without_losing_edges_or_warning_on_a_leaf`; selected suite 35 passed.
- **Contract status:** observed implementation, supported by [negative-result ADR](../../../adrs/2026-08-27-negative-results-name-their-scope.md) clause requiring trustworthy negatives to remain unremarked.
- **Unknowns:** does not establish every empty graph was fully analyzed.

### Requirement: recoverable hop error permits traversal to continue

- **Behavior:** WHEN resolve_one_hop returns RecoverableError for a located node, THEN CachedResolver::one_hop returns no edges for that hop and BFS preserves other traversed edges.
- **Enforcement:** src/tools/symbol/call_graph/mod.rs impl OneHopResolver for CachedResolver::one_hop; traversal.rs bfs.
- **Evidence state:** code-inspected; outer regression executed at baseline and after correction confirms actual build→Marker edge while class callee fallback fails.
- **Contract status:** observed best-effort implementation; desired continuation rather than whole-graph failure is the bounded correction choice. No stronger global propagation rule inferred.
- **Unknowns:** fatal errors and unavailable definition lookup are separate paths; cache completeness guarantees are not established by this sample.

### Requirement: suspicious negatives disclose their failed scope

- **Behavior:** WHEN the located class has no usable LSP hierarchy and real callee fallback cannot locate an enclosing function, THEN baseline output is an unqualified empty callee vector. The uncommitted correction adds true symbol/direction/reason through completeness_warning.
- **Enforcement:** baseline recoverable catch in CachedResolver::one_hop; correction analysis_failures aggregation and attach_analysis_failures in CallGraph::call.
- **Evidence state:** executed-fail baseline outer regression at missing warning, after confirming leaf/positive controls; executed-pass corrected suite 35/0. Applied mutation review and full gate pending in the report.
- **Contract status:** baseline violates the negative-result ADR's suspicious-negative disclosure requirement; correction is intended to satisfy it while preserving partial edges.
- **Unknowns:** real backends may supply hierarchy for a class and never enter this branch. The initial MCP class zero is not proof of the failure.

### Requirement: scan gaps have their own disclosure

- **Behavior:** WHEN definition discovery's bounded walk did not cover relevant source, THEN the outer graph attaches completeness_warning, a bounded symbol list and a total. A completed miss is not itself a warning.
- **Enforcement:** CachedResolver::lookup_pos/incomplete, scan_incomplete_warning, attach_scan_incomplete and CallGraph::call.
- **Evidence state:** code/test-inspected; existing cap, unreadable-file and completed-miss module tests executed in the passing 35-test selected suite.
- **Contract status:** observed pre-existing behavior, consistent with the ADR; not a new finding.
- **Unknowns:** coexistence with newly added analysis failure disclosure is under applied mutation review; do not infer merge coverage from separate green tests.

## Open questions

The callers fallback continues past unreadable referenced files. Reachability and completeness implications need a separate concrete probe. The references tool has retry/warm-up/completeness mechanisms; this draft makes no whole-surface claim about it. No no-ECC replay or paired benefit evaluation has run.
