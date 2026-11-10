---
id: '951f62c22db50699'
kind: report
status: done
title: BFS identity initial candidate independent review — with fixes
tags:
- ecc
- call-graph
- review
topic: ecc-integration
---

# Independent review of the frozen BFS identity candidate

Verdict: **With fixes**. No Critical finding; three Important findings and one Minor gate issue. The initial candidate is not ready to integrate. Acceptance requires the bounded fix pass, discriminating regressions, rerun of observed survivors and a final source-bound green gate.

## Strengths

The concrete query position and canonical visited identity are separate. Executed mutants show the tests discriminate file/column identity, operational query coordinates, outgoing endpoint/cache preservation, ambiguity, unreadable coverage and depth-boundary warning suppression. The actual parser, cache and CallGraph execute in the related tests; only the external hierarchy wire is mocked where necessary.

## Important findings

1. **Unsupported lexical bindings silently select an unrelated declaration.** identity.rs:169 and338, rust_target.rs:129. Four independent real-MCP fixtures compile with rustc exit0 but return helper→wrong without warning: block import, callable parameter, nested function and generic type parameter shadowing. Preserve verified edges and disclose unsupported binding context before fallback. Bug [dcecb5f9c3475b38](../../../issues/archive/2026-11-10-call-graph-ast-fallback-ignores-lexical-shadowing.md).
2. **Display deduplication discards distinct semantic endpoints before expansion.** traversal.rs:126 and171. D01 retains two concrete helper targets sharing one call range; expected one display row paths2 and both leaves, observed0/1 test failure with only leaf_a. Schedule from accepted raw endpoint edges before presentation dedupe. Bug [278ed8254992c41a](../../../issues/archive/2026-11-10-call-graph-display-dedupe-erases-endpoints.md).
3. **Five new safety disciplines are not regression pinned.** resolver.rs:131, mod.rs:473 and544, identity.rs:110, rust_target.rs:85. Actual survivors: incoming endpoint removal, legacy-cache guard bypass, language-family guard removal, next-node canonicalization removal, attributed-module guard bypass. Every survivor executed87tests and returned87/0. Bug [09a013c4d59dfc24](../../../issues/archive/2026-11-10-call-graph-new-identity-guards-unpinned.md).

All source citations above are relative to src/tools/symbol/call_graph except resolver.rs under src/tools/symbol/call_edges; they refer to the frozen candidate hashes, not later fixes.

## Minor gate issue

identity.rs:80 uses sort_by where the required clippy lane requests sort_by_key. The source-bound parent gate artifact records exit1 with unchanged candidate source. Parent owns the final full gate; this reviewer did not repeat it or infer approval from passing test lanes.

## Executed populations

Review baseline87passed/0failed. Thirteen production mutations actually applied in retained isolated trees: **8killed,5survived,0inconclusive**. Counts come from each final top-level Cargo summary, not the known-buggy helper running-N aggregation. All eight source hashes were checked after every mutation. Final observed hashes match in candidate, review root and actual mutation tree.

Four native shadowing fixtures are a separate population; all compile and all return wrong branches. D01 is a separate test-fixture diagnostic, not a fourteenth production mutant:0passed/1failed and restored.

## Evidence and restoration

[Machine-readable review](../evidence/2026-11-10-bfs-identity-dogfood/review/review-result.json) includes full exact mutation commands/replacements, observed summaries, killed-test names, source restoration, strengths and limits. [Mutation records](../evidence/2026-11-10-bfs-identity-dogfood/review/mutation-results.json), [native responses](../evidence/2026-11-10-bfs-identity-dogfood/review/native-results.json), [D01 diagnostic](../evidence/2026-11-10-bfs-identity-dogfood/review/D01.log), [restored hashes](../evidence/2026-11-10-bfs-identity-dogfood/review/restored-source.json). Log hashes use raw read_bytes. Mutant-file hashes are derived from frozen bytes plus the exact replacement; appliedness is independently guarded and logged by mutation-probe. No parent/home source edits, commits, pushes, cleanup or runtime/config changes were made.

## Subsequent parent disposition — 2026-11-10

The review above is the immutable initial-candidate judgment; raw machine evidence preserves original IDs and hashes. Parent later added discriminating tests, observed intended RED, applied one bounded correction pass and killed15/15 final mutations. Source537fa820d integrateddc769e430 after source and exact integrated-tree full gates passed. The [dogfood report](2026-11-10-bfs-identity-dogfood.md) records final source/verification and limitations; this initial review is not a second independent approval of the final bytes. Corrected bugs now resolve through the archive IDs linked above. Nested-body collector0f9e7f9b87799265 remains open.

## Declined to judge

None. Coverage limits do not relax the contract: general cross-language inference, every binding/cycle/cap/coverage interaction and live language-server behavior were not exhaustively exercised. The report claims only inspected bytes and executed observations. A nested-function fixture additionally shows its body's right call attributed to entry at depth1; this is flagged for parent triage without asserting its separate pre-existing cause.

## Review method

Applied the installed requesting-code-review reviewer template and the frozen installed ecc-silent-failure-review body explicitly. Read architecture/gotchas, spec/plan/rulings and relevant workspace/error/progressive-disclosure guides. Source symbols/references yielded stale ranges, so affected bytes were verified with bounded forced reads; parent filed4e7ec7bfc047ac7b. All codescout workspace-sensitive calls pinned the absolute root.

Ledger checked: 382466d3e15a3d1d, e4b663a4f2145663, dcecb5f9c3475b38, 278ed8254992c41a, 09a013c4d59dfc24, 4e7ec7bfc047ac7b.
