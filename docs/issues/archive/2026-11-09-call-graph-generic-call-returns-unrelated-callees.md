---
id: e5016b19fb0f75da
kind: bug
status: fixed
title: A path-pinned generic call_graph callee lookup returned unrelated short-name call chains
tags:
- call-graph
- lsp
- cluster/unclassified
closed: 2026-11-09
opened: 2026-11-09
owner: marius
severity: medium
unverified: 'CLEARED 2026-11-09. Was: observed on older shared MCP build9e4db9b9; current-source isolated reproduction and cause not yet established. Fresh source69ffdd12e with private cold/warm/restart fixtures and wrong-adjacency Rust REDs now confirms the completed-cache identity defect. The exact historical shared cache rows remain uninspected.'
---

# BUG: a path-pinned generic call_graph callee lookup returned unrelated chains

## Summary

During ECC create-finalization dogfood, a call_graph request pinned to src/librarian/tools/create.rs::call, direction=callees, returned run_command call chains and a router call site rather than the create body inspected with symbols. The response was not used as evidence about create's outgoing calls.

## Class fit

The confirmed mechanism is missing definition/direction ownership and completion provenance in a graph cache, rather than a missing path validator or capped traversal. The IC-1..IC-24 roster was read. IC-15 is not a precise fit: the requested path is parsed and used to resolve the seed, then the independently keyed cache supplies another declaration's result. IC-13 concerns an actual cap, absent here. Keep the sanctioned unclassified tag rather than stretching either class.

## Symptom (Effect)

Actual response: artifact.rs:548 call→call; run_command/mod.rs:468,512 call→run_command_inner; run_command/inner.rs:870 run_command_inner→handle_successful_output_with. The inspected create::call body uses catalog and filesystem operations and does not establish those outgoing run_command chains. A consumer could analyze another tool's flow under the requested name.

## Reproduction

call_graph(symbol=call,path=src/librarian/tools/create.rs,direction=callees,max_depth=2,detail_level=full,workspace=/home/marius/work/claude/codescout). Exact captured response and caveats are retained in [incidental navigation evidence](../../trackers/ecc-integration/evidence/2026-11-09-create-finalization-dogfood/incidental-navigation.txt).

## Environment

Shared running MCP pid1520813, git_sha a52d68284, dirty=true, build_id9e4db9b94323fb5bc65e3af403904f883459979206077488e448a362291620dc, executable deleted. Main source038fffb3 at observation. This is not a current-source binary reproduction. Earlier private fixture routing audit measured a unique Python symbol and does not cover duplicated Rust call names.

## Current-source reproduction — 2026-11-09

[Dogfood report](../../trackers/ecc-integration/reports/2026-11-09-callgraph-identity-dogfood.md), [exact actual-MCP cases](../../trackers/ecc-integration/evidence/2026-11-09-callgraph-identity-dogfood/baseline-mcp.json). Fresh immutable executable c606d743c71d9f53b6f6a834fc15d5a2a4696b76ce07152ca625afcdef3e1720, built at69ffdd12e. Existing related module77/0. Cold A and B each return their own two outgoing edges; warm B and restart B return A's two instead. Incoming control returned zero and is not used to claim a second runtime defect. No shared cache/server changes.

## Root cause

Confirmed on fresh current source69ffdd12e: CachedResolver::one_hop reads persisted EdgeCache rows by project_id+short symbol before looking up the requested definition. Rows accumulated from another same-name declaration satisfy that read, although they are not a completed adjacency for the requested file/direction. A fresh Rust fixture returns A's edges for B after A is queried; restarting retains the wrong result, while a cold B fixture returns B's own edges. This establishes a current cache identity defect; the exact historical shared-server cache was not instrumented, so its individual rows/cause remain retrospective uncertainty.

## Evidence

The earlier shared-runtime response remains historical evidence with its original runtime/source caveat. The [dogfood report](../../trackers/ecc-integration/reports/2026-11-09-callgraph-identity-dogfood.md) adds current-source cold/warm/restart responses, behavioral RED77pass/2fail, final84-test GREEN, qualified native6/6, scorer controls5/5 and exact source/binary identities. Independent initial ten-mutation population:8killed/2survived/0inconclusive. Observed coordinate survivors were closed with one consumer test and exact killed rechecks (F-226); production prefixes stayed unchanged. No whole-graph correctness claim follows.

## Hypotheses tried

A fresh executable at the frozen source reproduces warm-cache cross-file contamination, so source/runtime drift is not required for the current defect. Cold A and B return their own edges; after A, both warm B and restarted B return A's edges. Definition lookup/cache source inspection and Rust actual REDs localize the incorrect fast path. The original older shared cache was not inspected. The baseline incoming runtime control returned zero and is not used to establish a direction-cache failure; the partial-edge Rust test supplies independent behavioral evidence for that boundary.

## Fix

Implemented in source555f2817cbb790bcbd5de118e6194f45bbbdc15b, integrated4776e44bd on experiments. Completed-resolution cache keys project, symbol, definition file+line+column and direction; one_hop resolves identity first. Accumulated raw edges never seed trusted adjacency. Empty/recoverable analyses stay retryable; project-scoped invalidation conservatively clears completed snapshots while preserving raw-row return counts. Final module84/0 and qualified native6/6 on the identical executable pass. Final isolated-source full gate passed. Separate exact integrated-tree gate passed on4776e44bd: lean6325/48skip, default9222/62skip, Python1765/1skip/554subtests.

## Fix provenance

- **SHA:** `4776e44bd49347da8b7c962f72bbed44b6f2fe99` (`experiments`)
- **patch-id:** `82470b4a164f780eab0ce409ca01dfafc29f0d7a`
- **Source:** `555f2817cbb790bcbd5de118e6194f45bbbdc15b`, same two owned source hashes and stable patch-id. Candidate and integration whole trees differ because peer source changes landed; both exact trees passed their own full gate.

## Tests added

Consumer wrong-definition and accumulated-partial-adjacency tests observed behavioral RED77pass/2fail before the correction. Cache API tests discriminate every key dimension/project, replacement/invalidation, nonpersistent empties and corrupt-payload errors. Independent actual line/column consumer mutations survived83/0 before a same-file coordinate regression was added; exact rechecks each now fail83pass/1fail at wrong adjacency, restored final suite84/0. Separate new-API compile-red remains a different evidence type.

## Workarounds

Do not trust a graph that contradicts the named function body. Inspect exact source and references, then reproduce with fresh current runtime.

## Resume

CLOSED 2026-11-09 — corrected and verified on experiments4776e44bd. The earlier observation and caveat remain historical; current-source reproducer, actual mutation populations and exact integration gate are in the linked report. Full declaration-identity BFS remains outside this bounded completed-cache correction. No repair replay, push or shared-server rollout.
