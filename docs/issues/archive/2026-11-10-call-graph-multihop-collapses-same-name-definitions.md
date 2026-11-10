---
id: 382466d3e15a3d1d
kind: bug
status: fixed
title: call_graph multi-hop traversal collapses distinct same-name callees and silently omits a branch
tags:
- call-graph
- cluster/addressing-without-an-escape-hatch
closed: 2026-11-10
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

Use [exact private fixture files](../../trackers/ecc-integration/evidence/2026-11-10-resolver-followup/bfs-fixtures.json). src/lib.rs declares modules a/b and entry calls a::helper() on line4 and b::helper() on line5. Each helper calls its own leaf in its own file. Request call_graph(symbol=entry,path=src/lib.rs,direction=callees,max_depth=2,detail_level=full) on a fresh current-source private server.

## Expected behavior

Both distinct declarations reached by the two unambiguous source call sites retain their outgoing branches. If a fallback cannot establish target identity, disclose incomplete/ambiguous analysis rather than silently presenting one arbitrarily chosen declaration as the traversal of both call sites.

## Actual behavior

Cold, process-restarted and warmed-after-both-pinned-helper depth2 requests each return entry→helper at lines4 and5 plus helper→leaf_a in src/a.rs. The helper→leaf_b edge is missing. Each response declares3 edges across2 files, max_depth_reached2 and no completeness warning. Unique-name depth2 returns all4 edges; duplicate entry depth1 returns both call sites; path-pinned helperA and helperB depth1 each return the correct own leaf. These are4 qualifying controls and3 separate observed failures, not7 correct cases.

## Environment

Frozen source5b40efc48d31ccdcebbf9a9a2429c768d7aa6ca3 plus a doc-comment-only resolver correction. Immutable executable SHA2568f83ac66e5692ae19628cd2e72624bb39458306412c8320d3c2cbf788fb72a4d. Source/worktree/base/binary hashes are retained in [baseline](../../trackers/ecc-integration/evidence/2026-11-10-resolver-followup/baseline.json) and binary.json. All reported edges in these responses use tree-sitter; this does not establish universal LSP behavior. Shared runtime/cache/config were not changed.

## Root cause

Source-inspected identity loss: callee_identifier extracts only the final target segment, bfs visited/current/next keys are String short names, and CachedResolver::lookup_pos chooses one position per name (first workspace-symbol match or AST fallback). Thus both helper call sites schedule a single helper expansion. The prior definition-scoped completed cache cannot recover identity already discarded between hops.

## Evidence

[Raw cases](../../trackers/ecc-integration/evidence/2026-11-10-resolver-followup/bfs-cases.json), [summary](../../trackers/ecc-integration/evidence/2026-11-10-resolver-followup/bfs-summary.json), [report](../../trackers/ecc-integration/reports/2026-11-10-resolver-followup.md). Oracle compares full caller/callee/file/line/depth multisets, declared totals and warnings. Actual-positive plus wrong-file/wrong-depth/wrong-callee/guide-only perturbations give5/5 expected scorer verdicts. No independent general efficacy or exhaustive graph correctness claim. Focused active catalog title/semantic searches did not identify an existing live report; unindexed files and the full duplicate space were not exhaustively audited.

## Fix

Declaration-aware queue/visited keys preserve file/line/column independently from provider query coordinates. Hierarchy metadata survives completed cache hits; AST Rust targets use declared module/type context. Display projection no longer discards semantic endpoints before next-hop placement. Source commit537fa820d95b3298393cc515ab104776a57455c7 verified; exact integrated-tree full gate passed and integration is dc769e430b7df552733e55693391f9bdf31b4b1b.

## Fix provenance

- **SHA:** `dc769e430b7df552733e55693391f9bdf31b4b1b` (`experiments`)
- **patch-id:** `52c9b1d039bbdd7de6183eefca168f32318d8cd8`
- **Source:** `537fa820d95b3298393cc515ab104776a57455c7`
- **Gated integration tree:** `115cd5bb196cd91c7763deede84d3c4e93070059`

Both required full gates passed; final15 mutation runs killed all candidates and restored the ten frozen source hashes after each run.

## Tests added

Outer Tool regressions cover the actual two-module cold/warm/new-provider topology, ambiguous unqualified warning, same-file/same-line LSP columns/cache repeat, shared-callsite path counts, legacy payload recomputation and canonical seed cycle. Related95/0; fifteen actual final mutations killed, including all five initial survivors; real-MCP original7 and extended4 full graph cases match.

## Resume — 2026-11-10

**CLOSED 2026-11-10 — verified and integrated on experiments.** Source537fa820d; integrationdc769e430b7df552733e55693391f9bdf31b4b1b. Related95/0,15actual mutation rechecks killed, source and exact integrated-tree full gates passed. Initial counterexamples and unsuccessful attempts above remain historical evidence. Separate nested-body collector/navigation/count-instrument issues remain open. No repair replay.
