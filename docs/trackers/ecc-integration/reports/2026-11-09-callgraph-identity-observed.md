---
id: ddb5d7d4145391a5
kind: spec
status: done
title: Call graph completed-resolution cache — bounded observed contract
tags:
- ecc
- observed-behavior
- call-graph
topic: ecc-integration
---

# Call graph completed-resolution cache — observed contract

> Controller-authored bounded correction criteria derived from the path-pinned tool contract and executed reproducers. This does not make all baseline implementation behavior intended. Base69ffdd12e6d51f4884c0190a1a6c4edeca4289e4; [report](2026-11-09-callgraph-identity-dogfood.md).

## Scope and provenance

Inspected CallGraph::call, CachedResolver::one_hop/lookup_pos, EdgeCache schema/queries/upsert/invalidation, resolver::resolve_one_hop/resolve_via_lsp and BFS. Exact EdgeCache references establish raw-query consumers and Agent invalidation entry points. Existing related module77/0. Actual private MCP cold A/B succeed; warm B after A and restarted B return A's edges. Rust consumer tests observe77pass/2fail: wrong callee and partial outgoing adjacency. New API compile-red is separate evidence, not another behavioral failure.

## Acceptance criteria

- A path-pinned one-hop callee query for B returns B's edges after querying a same-name A and after process restart.
- Accumulated partial/legacy raw edges are not accepted as a completed adjacency for the requested definition.
- Successful nonempty completed adjacency is reused only for the same project, symbol, definition file, line, column and direction; replacement does not accumulate obsolete edges.
- File invalidation clears this project's completed snapshots conservatively, including incoming results owned by another file, and preserves another project's snapshots and existing raw-row return-count semantics.
- Empty/recoverable analyses remain retryable, valid quiet leaf results and recoverable completeness warnings remain distinct, and corrupt persisted results report an error rather than an empty graph.

## Baseline enforcement and defect

The tool resolves the seed within the supplied path, but one_hop queried the edge projection before resolving its definition. The projection is keyed/filterable by project+caller/callee name and contains observations from several queries; it does not establish complete adjacency for one path/direction. This contradicts the requested seed boundary in the executed fixture.

## Correction boundary

Implemented in source555f2817c and integrated4776e44bd: an additive completed-resolution store keys actual definition identity and direction separately from the raw edge projection. Legacy raw rows never seed trusted snapshots. Successful nonempty results are cached; empty/recoverable analyses remain retryable. The raw table remains available to existing consumers. Final84-test module and qualified native6/6 bind to the byte-identical executable; full final candidate gate passed. Exact integration gate includes peer changes and is recorded separately in the report.

## Bounded verification mapping

Path-pinned warm/restart behavior: qualified candidate-mcp-summary.json (exact caller/callee/file/depth tuples and totals), bound by final-binary.json. Legacy/partial rejection: completed_resolutions_do_not_trust_accumulated_partial_edges and retained legacy DB case. Definition/project/symbol/file/line/column/direction: completed_resolution_keys_discriminate_every_identity_dimension plus consumer same-name and coordinate tests; applied coordinate survivors were observed before stronger tests, then exactly killed. Replacement/invalidation/raw count/other projects: completed_resolution_replaces_and_invalidates_only_its_project and existing raw invalidation tests. Empty/recoverable/corrupt distinctions: no-persist-empty and corrupt-payload tests, existing outer recoverable-warning tests, native quiet-leaf case. This is a mapping to executed bounded checks, not exhaustive LSP or whole-graph correctness.

## Unknowns and limits

The initial incoming MCP control returned zero, so it does not establish a runtime direction-contamination result. Rust tests exercise that cache-subset boundary deterministically. Full declaration-identity BFS, external edits outside the existing invalidation protocol, LSP index readiness and concurrent mutation/analysis linearizability are outside this correction; no stronger guarantee is claimed. No default-profile/causal ECC claim, shared-server rollout or repair replay.
