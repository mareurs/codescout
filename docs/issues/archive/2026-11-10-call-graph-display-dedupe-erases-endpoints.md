---
id: 278ed8254992c41a
kind: bug
status: fixed
title: Call graph display deduplication erases distinct LSP endpoints before expansion
tags:
- call-graph
- cluster/addressing-without-an-escape-hatch
closed: 2026-11-10
opened: 2026-11-10
owner: marius
severity: medium
---

# BUG: display deduplication erases target identity

## Summary

The BFS identity candidate retains concrete LSP targets on edges, but merges those edges by display location before scheduling next hops. Distinct same-name targets sharing the same source call range collapse into one target, silently omitting a branch.

## Class fit

The display dedupe key omits the declaration endpoint that distinguishes valid traversal nodes. Unlike the original visited-set collision, the endpoint identity exists on the edge but is discarded by a presentation aggregation before the queue sees it. This is an addressing namespace without its necessary disambiguator.

## Symptom (Effect)

One displayed entry→helper edge has paths2, but only helper→leaf_a expands; helper→leaf_b is absent without a completeness warning.

## Reproduction

The isolated D01 diagnostic changes the existing mocked-wire same-file/same-line LSP fixture so both concrete helper endpoints use from_range line3,column13; their distinct selection-range columns remain. The expected display multiset contains one entry→helper and both helper→leaf branches. `cargo test --no-default-features --lib tools::symbol::call_graph::identity_tests::lsp_same_file_same_line_helpers_preserve_columns_and_cached_target_metadata -- --exact --nocapture` actually failed0passed/1failed. [Exact diagnostic patch](../../trackers/ecc-integration/evidence/2026-11-10-bfs-identity-dogfood/review/D01.patch), [raw log](../../trackers/ecc-integration/evidence/2026-11-10-bfs-identity-dogfood/review/D01.log).

## Environment

Frozen candidate eight-file manifest on base7f0dbba5fd6ba25e4a50a8aad5c25403a11ae345; test-only diagnostic executed in retained isolated mutation tree. Parser/cache/CallGraph are real, external LSP wire is mocked. This diagnostic is separate from the13 production mutations. Original test/source bytes restored.

## Root cause

src/tools/symbol/call_graph/traversal.rs:126 calls dedupe_with_paths before next_node scheduling at lines140-145. dedupe_with_paths at lines171-177 keys only caller/callee/display file/line/column, discards the subsequent Edge carrying its distinct target, and sums paths. Thus the endpoint never reaches declaration-keyed visited handling.

## Evidence

D01.log's actual JSON has entry→helper paths2 plus helper→leaf_a; expected rows also require helper→leaf_b. D01-result.json contains command and raw-byte log hash. Source was restored after the single failing diagnostic.

## Hypotheses tried

Adjusted expected display rows to preserve existing aggregation semantics; failure remains specifically the missing second leaf, not the merged display row.

## Fix

Accepted raw endpoint-bearing level edges feed next_node placement while display dedupe still sums path counts. Whole-level caps are checked before placement and the final requested depth does not analyze neighbors. Source537fa820d verified; integrated locally on experiments as dc769e430b7df552733e55693391f9bdf31b4b1b.

## Fix provenance

- **SHA:** `dc769e430b7df552733e55693391f9bdf31b4b1b` (`experiments`)
- **patch-id:** `52c9b1d039bbdd7de6183eefca168f32318d8cd8`
- **Source:** `537fa820d95b3298393cc515ab104776a57455c7`
- **Gated integration tree:** `115cd5bb196cd91c7763deede84d3c4e93070059`

Both required full gates passed; final15 mutation runs killed all candidates and restored the ten frozen source hashes after each run.

## Tests added

shared_callsite_keeps_both_targets_and_display_path_count observes one entry→helper row with paths2 and both different leaf branches. Final M15 restores display-only frontier scheduling and is actually killed; all pre-existing path-count/cap/parallelism controls remain in the95-test related suite.

## Workarounds

Request the concrete target graphs separately.

## Resume

**CLOSED 2026-11-10 — verified and integrated on experiments.** Source537fa820d; integrationdc769e430b7df552733e55693391f9bdf31b4b1b. Related95/0,15actual mutation rechecks killed, source and exact integrated-tree full gates passed. Initial counterexamples and unsuccessful attempts above remain historical evidence. Separate nested-body collector/navigation/count-instrument issues remain open. No repair replay.

## References

[Repair contract](../../trackers/ecc-integration/reports/2026-11-10-bfs-identity-contract.md), original visited-key bug 382466d3e15a3d1d.
