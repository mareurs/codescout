---
id: fed27cae5971c526
kind: bug
status: open
title: call_graph multi-hop traversal collapses distinct same-name callees and silently omits a branch
tags:
- call-graph
- cluster/addressing-without-an-escape-hatch
opened: 2026-11-10
owner: marius
severity: medium
---

# BUG: multi-hop traversal collapses distinct same-name callees

## Summary

A Rust entry function calls both a::helper() and b::helper(), and those helpers call different leaves. A path-pinned depth2 call_graph returns both entry call sites but only a::helper's leaf, presenting three edges as a clean result instead of the four expected edges. This is separate from the previously fixed completed-cache identity defect.

## Class fit

IC-6 applies: the internal traversal addressing scheme has no declaration disambiguator. AST call targets discard qualifiers, frontier/visited keys contain only the short name, and position lookup chooses one definition. Unlike a delimiter collision, this is a namespace collision between valid declarations in different modules. The source namespace is meaningful but has no representation in the next-hop key. No class Members line is modified.

## Reproduction

Use [exact private fixture files](../trackers/ecc-integration/evidence/2026-11-10-resolver-followup/bfs-fixtures.json). src/lib.rs declares modules a/b and entry calls a::helper() on line4 and b::helper() on line5. Each helper calls its own leaf in its own file. Request call_graph(symbol=entry,path=src/lib.rs,direction=callees,max_depth=2,detail_level=full) on a fresh current-source private server.

## Expected behavior

Both distinct declarations reached by the two unambiguous source call sites retain their outgoing branches. If a fallback cannot establish target identity, disclose incomplete/ambiguous analysis rather than silently presenting one arbitrarily chosen declaration as the traversal of both call sites.

## Actual behavior

Cold, process-restarted and warmed-after-both-pinned-helper depth2 requests each return entry→helper at lines4 and5 plus helper→leaf_a in src/a.rs. The helper→leaf_b edge is missing. Each response declares3 edges across2 files, max_depth_reached2 and no completeness warning. Unique-name depth2 returns all4 edges; duplicate entry depth1 returns both call sites; path-pinned helperA and helperB depth1 each return the correct own leaf. These are4 qualifying controls and3 separate observed failures, not7 correct cases.

## Environment

Frozen source5b40efc48d31ccdcebbf9a9a2429c768d7aa6ca3 plus a doc-comment-only resolver correction. Immutable executable SHA2568f83ac66e5692ae19628cd2e72624bb39458306412c8320d3c2cbf788fb72a4d. Source/worktree/base/binary hashes are retained in [baseline](../trackers/ecc-integration/evidence/2026-11-10-resolver-followup/baseline.json) and binary.json. All reported edges in these responses use tree-sitter; this does not establish universal LSP behavior. Shared runtime/cache/config were not changed.

## Root cause

Source-inspected identity loss: callee_identifier extracts only the final target segment, bfs visited/current/next keys are String short names, and CachedResolver::lookup_pos chooses one position per name (first workspace-symbol match or AST fallback). Thus both helper call sites schedule a single helper expansion. The prior definition-scoped completed cache cannot recover identity already discarded between hops.

## Evidence

[Raw cases](../trackers/ecc-integration/evidence/2026-11-10-resolver-followup/bfs-cases.json), [summary](../trackers/ecc-integration/evidence/2026-11-10-resolver-followup/bfs-summary.json), [report](../trackers/ecc-integration/reports/2026-11-10-resolver-followup.md). Oracle compares full caller/callee/file/line/depth multisets, declared totals and warnings. Actual-positive plus wrong-file/wrong-depth/wrong-callee/guide-only perturbations give5/5 expected scorer verdicts. No independent general efficacy or exhaustive graph correctness claim. Focused active catalog title/semantic searches did not identify an existing live report; unindexed files and the full duplicate space were not exhaustively audited.

## Fix

Pending. Design choices include propagating declaration identity/call-site context between hops, or explicitly reporting ambiguous next-hop resolution as incomplete. Preserve cap/direction/path-count/error semantics. Do not fold this behavior change into the documentation-only correction.

## Tests added

No production regression test added yet. A retained real-MCP probe reproduces the failure and preserves qualifying controls. The84-test existing related module remains green despite this topology failure; passing it is not evidence this case is guarded.

## Resume — 2026-11-10

Open, unclaimed next behavioral dogfood candidate. Start from the frozen ref and exact fixture/response evidence; record a bounded design before changing the traversal interface. No repair replay ran.
