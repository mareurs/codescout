---
id: '0e002d054f03768a'
kind: plan
status: done
title: Call graph identity dogfood — isolated investigation
tags:
- ecc
- codex
- dogfood
- call-graph
topic: ecc-integration
---

# Call graph identity dogfood plan

> For agentic workers: continue the user-authorized inline dogfood workflow with superpowers:executing-plans after the confirmed design is recorded. The user's go continues inspection, reproduction, bounded correction, verification and local integration; no new replay or rollout.

**Goal:** Reproduce the path-pinned duplicated-Rust-name observation on a fresh current-source server and correct only a demonstrated contract violation.

**Architecture:** Trace seed resolution, one-hop edge caching and traversal as separate boundaries. First compare cold and warmed actual MCP results against the same tiny Rust source; record a bounded design before production changes.

**Tech stack:** Rust, SQLite edge cache, real stdio MCP and isolated Rust fixture.

**Spec:** Report/observed draft records the intended file-pinned graph boundary separately from baseline implementation behavior. Buge5016b19fb0f75da is initially an older-runtime observation, not current-source proof.

## Global constraints

Frozen baseline69ffdd12e6d51f4884c0190a1a6c4edeca4289e4, tree f1c0f44c76cc02ec677466b6b73320cb989d1e34. Owned branch feat/ecc-callgraph-identity-20261109 in .worktrees/ecc-callgraph-identity. Native inventory transport failed; Git fallback created fresh ignored worktree. Preserve exact pre-action provenance and excluded evaluator material; no no-ECC replay, profile changes, shared-server replacement or push.

## Review focus

- Same-name functions in different files must retain the requested seed identity after prior calls and restart.
- A cached subset from another query direction must not stand in for the requested complete adjacency.
- Valid empty results, recoverable partial-graph warnings and fatal errors must remain distinguishable.
- Edits/invalidation and persisted legacy cache rows must not revive invalid graph data.
- Deeper-hop short-name ambiguity must be explicitly bounded; do not claim full declaration-identity traversal from a seed-only proof.

## Task1 — Reproduce and choose the bounded correction

- [x] Freeze Git base/ref, eleven input snapshots, neutral brief, explicit skill bodies and private pre-action host history before source changes.
- [x] Run existing call_graph/call_edges baseline and retain source-bound build/binary identity (77/0).
- [x] Run current-source private MCP cold/warm/restart/direction controls; cold A/B pass, warm/restart B return A; incoming0 control is nonqualifying.
- [x] Record the observed failure and responsible boundary: unscoped accumulated-edge cache hit before definition lookup.
- [x] Record correction alternatives and final design, then exact failing Rust regression assertions before production code (77pass/2wrong-adjacency failures).
- [x] Verify correction through dedicated tests, actual MCP, independent applied-mutation review and required full gate.
- [x] Commit/integrate locally by explicit paths, preserve peer index blobs, update report/index/replay metadata and archive only a verified fixed bug.

## Confirmed design and inline execution

The fresh baseline module passed77/0. Actual MCP returned A's outgoing edges for B after warming A, including after process restart; cold B returned B's own two edges. The raw incoming control returned zero, so it does not establish direction contamination by itself. Cache source inspection and Rust regression tests cover that boundary separately.

Approaches considered: (1) disable cache reads, preserving correctness with repeated analysis; (2) add a completed-resolution cache keyed by project/symbol/definition file+line+column/direction; (3) redesign BFS around declaration identities. Controller chooses2 for this bounded correction: preserve useful caching while stopping accumulated edges from impersonating a completed query. Full multi-hop declaration identity is explicitly outside this seed-cache correction. This is an implementation choice within the user's repeated go/inline dogfood authorization, not a new user-approved architecture claim.

### Task2 — Isolate completed one-hop results

Files: src/tools/symbol/call_edges/cache.rs owns additive completed-resolution schema/API and invalidation; src/tools/symbol/call_graph/mod.rs owns definition-first cache lookup and its consumer tests. Existing raw edge table/API remains an edge projection, never trusted as a completed one-hop resolution. No Edge/BFS/public tool parameter change.

- [x] Add two consumer regression tests using the real SQLite cache and real Rust tree-sitter resolution: same-name A→B→A with per-definition cache reuse; a partial accumulated edge must not suppress another outgoing edge. Run against unchanged production and require wrong returned adjacency, not compilation failure.
- [x] Add ResolutionKey {symbol,file,line,col,direction}; EdgeCache::lookup_resolution(key) -> Result<Option<Vec<Edge>>> and store_resolution(key,edges) -> Result<()>; additive call_edge_resolutions table keyed with project_id. Never seed it from legacy raw rows. Cache successful nonempty adjacency only, preserving retryability of empty/recoverable results.
- [x] In one_hop, resolve the definition first, use the scoped key for reads/writes, and continue storing raw edges for existing projection/invalidation consumers. Existing recoverable warnings and fatal error mapping remain.
- [x] invalidate_file keeps its raw-row return-count semantics and clears this project's completed-resolution snapshots conservatively because an edited call site can affect another file's incoming adjacency. Other projects survive.
- [x] Pin key field discrimination, project isolation, completed snapshot replacement, empty/recoverable retryability, legacy-cache rejection and invalidation. Observe GREEN and actual MCP warm/restart/legacy controls before review/full gate.

Review focus: actually mutate key dimensions at the consumer/query call sites and invalidation; observe kills/survivors before adding any survivor-specific test. Build leases must be acquired inside each isolated mutation tree, never carried across source roots. Retain failed controls, exact baseline/provenance and all review populations.

## Verification progress

Completed: final84-test module, qualified native6/6 with calibrated scorer5/5, independent initial ten-mutation review8killed/2survived/0inconclusive, and two exact coordinate survivor kills after one consumer-test strengthening. Production prefixes unchanged; final executable byte-identical to the qualified candidate.

Source555f2817cbb790bcbd5de118e6194f45bbbdc15b integrated4776e44bd49347da8b7c962f72bbed44b6f2fe99, stable patch-id82470b4a164f780eab0ce409ca01dfafc29f0d7a. Final candidate full gate passed; separate exact integrated-tree gate passed (lean6325, default9222, Python1765/1skip/554subtests). Same owned source hashes and patch-id; whole trees differ through peer commits. Six foreign staged blobs preserved. Fixed bug archived as e5016b19fb0f75da. Pre-existing resolver-doc drift recorded separately as81436f2c48ad8c74.

Ruling: retain durable tracker, evidence and provenance worktrees. Full multi-hop declaration identity remains outside scope. No repair replay, shared-server rollout or push. Report links exact evidence and coverage limits.
