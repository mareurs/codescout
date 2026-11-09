---
id: fff9b8f9de9ac540
kind: report
status: done
title: 'ECC dogfood: navigation recoverable failures'
tags:
- ecc
- dogfood
- navigation
topic: ecc-integration
---

# Navigation dogfood — 2026-11-09

## Current result

Confirmed and corrected recoverable-hop failure disclosure. Source commit `4a0ba58564a39e4bb5688f1f2421c0e0172234b3`, integrated locally on experiments as `3fe613bddca6ddd344164f6ece3278be56186c1a`, stable patch-id `4b7cad6bf7b5eda6f600304f4969fb13929b5eeb`. Focused suite36/0, real MCP baseline/candidate controls passed, independent review approved after actual mutations closed both observed gaps. Full frozen-tree gate passed: lean6309/48 skipped, default9198/62 skipped, Python1764/1 skipped/554 subtests; all stages zero. [Gate result](../evidence/2026-11-09-navigation-dogfood/gate-result.json), [log](../evidence/2026-11-09-navigation-dogfood/gate.log), [integration record](../evidence/2026-11-09-navigation-dogfood/integration.json).

The integrated whole tree differs from the verified frozen tree only in three intervening peer research files under docs/research; source/build/test surfaces match exactly. The script's final tree-equality assertion correctly failed after commits had succeeded; it did not indicate commit failure or source mismatch. Difference is explicitly recorded, rather than claiming an exact-tree post-integration gate. Foreign staging preserved. No push/shared-server rollout or no-ECC replay. No general ECC advantage established.

Original replay baseline remains `b3d02d482eaeb3cabf329f4f37a2d3649a6057b7`, tree `34977f2047694714e5f852ee427c2dfca435d9e8`. [Plan](../plans/2026-11-09-navigation-dogfood.md), [baseline](../evidence/2026-11-09-navigation-dogfood/baseline.json), [archived bug](../../../issues/archive/2026-11-09-call-graph-hides-recoverable-hop-failures.md). Next user-requested task audits active call_graph prompt promotion and current-only guidance.

## Confirmed findings

**Recoverable analysis failure is erased into an unqualified empty/partial graph — medium.** Location: src/tools/symbol/call_graph/mod.rs, CachedResolver::one_hop and CallGraph::call. Failure path: definition found → no usable LSP call hierarchy → actual tree-sitter fallback cannot locate an enclosing function → RecoverableError → catch returns an empty vector → ordinary graph without completeness_warning. The consumer cannot distinguish failed analysis from analyzed absence. The [negative-result ADR](../../../adrs/2026-08-27-negative-results-name-their-scope.md) requires suspicious negatives to name scope and genuine negatives to remain quiet. It supports disclosing the failure while retaining the intentional best-effort traversal; it does not require discarding useful edges.

The deterministic outer-tool test uses a real Python file, parser and SQLite cache; only the external LSP's unavailable hierarchy is controlled. It checks an actual incoming build→Marker edge and a genuine no-warning leaf before failing at the missing warning. Initial RED: 1 failed, exit 101. This is stronger than the existing lower-level unsupported-language test, which never reached the catch.

Recommendation implemented: keep recoverable-hop failures separate from definition-walk gaps, collect across directions, append true symbol/direction/reason to the existing completeness_warning field and keep partial edges. Display at most five failure descriptions and the actual total; the existing compact renderer carries the warning.

## Execution evidence

- [RED record](../evidence/2026-11-09-navigation-dogfood/red-record.json), [log](../evidence/2026-11-09-navigation-dogfood/red.log.gz): failed at absence of completeness_warning after edge/leaf assertions.
- [First corrected run](../evidence/2026-11-09-navigation-dogfood/first-corrected-prefix-failure.log.gz): 34 passed, 1 failed because the new assertion omitted the established compact renderer's `warning: ` prefix. Actual output already contained the correct warning. Test expectation corrected; product code unchanged by that correction.
- [Focused GREEN record](../evidence/2026-11-09-navigation-dogfood/green-record.json), [log](../evidence/2026-11-09-navigation-dogfood/green.log.gz): 35 passed, 0 failed.
- [Initial real MCP controls](../evidence/2026-11-09-navigation-dogfood/baseline-mcp.json): real leaf callees empty, Marker has an LSP incoming edge, references identify definition/use. Marker callees empty alone is not evidence of failed analysis; that backend can supply class hierarchy.
- Build follow-up did not run in two attempts: slot-3 does not exist; slot-1 was leased by another run. No product inference from either operational refusal.


- Real private-catalog MCP reproduced the concrete branch for Python constant TOKEN: [baseline](../evidence/2026-11-09-navigation-dogfood/baseline-constant-mcp.json) returns an unqualified zero; [candidate](../evidence/2026-11-09-navigation-dogfood/constant-mcp.json) prefixes the actual enclosing-function fallback failure, symbol and direction. Both retain a quiet leaf and real incoming LSP edge. [Adjudication](../evidence/2026-11-09-navigation-dogfood/mcp-adjudication.json) checks those exact response text blocks. Baseline executable SHA matched its recorded b3d02d482 build; candidate executable/source hashes are retained. This is a same-fixture binary regression probe, not a no-ECC agent replay.
- Independent applied m3 mutation that replaces the existing scan warning instead of merging SURVIVED the original 35 tests (35 passed/0 failed). This is a real missing regression guard, not a product bug in the current merge code. New coexistence test will be added after observing all review probes/restoration.


- Independent first pass actually applied five distinct mutants: three KILLED, two SURVIVED (merge preservation, displayed-detail bound); a duplicate-literal attempt was refused before arming and is excluded. [Review](../evidence/2026-11-09-navigation-dogfood/review/review-v2.md). After observing both survivors, added one helper-level regression for existing scan-warning coexistence, empty-failure identity, seven true failure descriptions, first-five display, omitted scopes absent, +2 more, real total seven and intact compact output. Original Tool-level test still pins aggregation/attachment. New focused suite: [36 passed / 0 failed](../evidence/2026-11-09-navigation-dogfood/green-after-survivor-tests.log.gz). Reviewer reapplication of the two survivors is pending.
- Full gate running in a separate frozen verification worktree on doc commit 09c35b678 plus the exact reviewed source patch, including committed tracker/replay artifacts. [Input fingerprints](../evidence/2026-11-09-navigation-dogfood/gate-input.json). This isolates shared-checkout changes and includes the newly tracked documentation in the checked tree; source hash equals reviewed d5b158f5… . No production changes after MCP, only the added regression and rustfmt layout.


- Reviewer independently observed the extended baseline 36 passed / 0 failed, then reapplied the two exact former survivors. Both now KILLED, each 35 passed / 1 failed; the new helper regression is the sole failing test in each. Reviewed and isolated files restored byte-identically to SHA256 d5b158f5e38d67119e0abb4ffa077e65b083f9dc8d2c439b965c04af713693c8; own mutation marker absent and pre-state Git status preserved. Five distinct candidates were applied in the first pass; the later two runs reapply existing candidates, not two new distinct mutants. Review approved in scope; parent gate still running.


- Final state supersedes the pending labels in this chronological evidence ledger: MCP and review passed, full gate exit0 at unchanged d5b158f5 source, source committed/integrated as stated above. [Integration record](../evidence/2026-11-09-navigation-dogfood/integration.json) names exact peer research-only drift from the frozen verification tree. No unchecked whole-tree equivalence inferred.

## Open questions

The caller fallback's skip of unreadable referenced source remains a separate lead. references::call_with_settle has dedicated retry/completeness logic but has not been fully audited in this cycle. Neither is counted as an additional confirmed finding.

## Coverage limits and ECC attribution

Both pilot skill bodies were applied for explicit observed behavior and failure/caller tracing, using codescout symbols, references and call_graph. This is dogfooding a bounded workflow, not a paired effectiveness experiment. A no-ECC arm must later run blind with a truly absent skill surface and private catalog; the preserved neutral audit brief and frozen source delta are preparation only.

The deterministic regression exercises actual fallback failure at the Tool boundary with a controlled external capability. Real LSP behavior may choose a different path. Full source-tree compliance, every language/backend and every failure mode are outside this audit. Standard gate excludes its advertised heavy Python/server-stack/local ONNX execution lanes.

## Replay provenance

The [neutral audit brief](../evidence/2026-11-09-navigation-dogfood/candidate-task.txt) was captured before identifying a concrete outer-tool fixture. Navigation baseline fingerprints and committed-tree delta from the first cycle's validated source archive preserve exact starting bytes. The first doc-create replay remains on its original fe965888 baseline; do not silently substitute this cycle's b3d02d482. No replay run.
