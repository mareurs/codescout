---
id: dc8c75578af622dd
kind: plan
status: done
title: Call graph BFS identity dogfood — implementation plan
tags:
- ecc
- codex
- call-graph
topic: ecc-integration
---

# Call graph multi-hop identity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for inline execution; one independent whole-branch review with actual mutations at the end.

**Goal:** Preserve same-name declaration branches and disclose unresolved next-hop identity.

**Architecture:** A traversal node carries a display symbol and optional concrete definition location. LSP edges retain an optional next-hop target; AST edges resolve their call-site context in a separate identity module. Completed payloads preserve target data; legacy LSP payloads without it are misses.

**Tech Stack:** Rust, tree-sitter, existing LSP/SQLite completed cache and MCP transport.

**Spec:** [Bounded contract](../reports/2026-11-10-bfs-identity-contract.md).

## Global Constraints

Exact base7f0dbba5fd6ba25e4a50a8aad5c25403a11ae345; preserve all prior replay inputs/worktrees. No repair replay, push, dependencies, shared runtime/profile changes or mutation of peer staged blobs. Source edits through codescout edit_code; managed docs through doc. Parent builds use source-bound slot0; reviewer slot1 inside mutation worktree. Run required full gate before completion and exact integrated-tree verification if peer compiled surfaces drift.

## Review Focus

- Same-name seed and next hop, including same-file different columns: identity cannot be erased by visited sets or canonicalization.
- Cached/restarted LSP and AST results: target provenance survives; old payloads are recomputed safely.
- Unsupported or ambiguous AST bindings: warnings carry through JSON and compact output; no arbitrary branch or trustworthy-empty claim.
- Cycles, repeated targets and depth/cap boundaries: no extra expansion/warning beyond requested or retained levels; preserve path counts and parallelism.
- Source/file lookup failures and bounded workspace coverage: actual incomplete scope is disclosed; no type/alias inference by name coincidence.

### Task 1: Preserve identity through traversal and resolution

**Files:** Modify src/tools/symbol/call_edges/resolver.rs (Definition metadata/LSP endpoints), cache.rs (legacy-payload check), call_graph/traversal.rs (TraversalNode and neighbor scheduling), call_graph/mod.rs (consumer wiring); create call_graph/identity.rs and call_graph/identity_tests.rs. Existing Edge literals/test doubles receive only the new field/signature adaptation.

**Interfaces:** Definition is hashable file/line/col; Edge.target is optional declaration metadata with serde default. TraversalNode holds symbol and optional Definition. OneHopResolver.one_hop consumes a node; next_node resolves an edge to the next node (default named behavior for graph-only test doubles). BFS seed is a concrete node. CachedResolver consumes concrete nodes without short-name re-lookup, and records unresolved next-hop analysis through the existing warning set.

- [x] Freeze baseline, installed skill bodies, private host prefix, replay ref and foreign index blobs; observe84/0 related baseline.
- [x] Add outer-tool regression for the exact two-module duplicate-name topology and an ambiguous unqualified control. Run before any production change: expected failure is missing leaf_b and missing completeness warning respectively. Add same-file/LSP/cache/cycle controls against the interfaces as they become available; observe intended RED before each implementation portion.
- [x] Implement the definition metadata, typed queue/visited, LSP neighbor positions and safe context resolution. Rust ordinary declared module paths resolve concretely; unsupported qualified syntax and non-unique short-name fallbacks are incomplete. Preserve all existing tests/assertions, layout and output fields.
- [x] Run the related module suite to GREEN. Build immutable baseline/candidate executables; run real MCP cold/warm/restart/unique/depth controls with full edge-multiset and warning oracles. Qualify scorer axes with actual perturbations; record native child exit independently even if another check fails.
- [x] Freeze final source, run full gate, dispatch one strong reviewer to apply at least6 targeted mutations (file/column identity, endpoint forwarding, ambiguity disclosure, cache target preservation/legacy handling) and report actual killed/survived/inconclusive. Observe each survivor before writing its regression. One fix pass, exact rechecks, final full gate bound to final source.
- [x] Commit explicit owned source paths and integrate locally under existing authorization. Preserve source/integration SHA, stable patch-id, tree/hash and peer delta; run exact integrated-tree full gate when required. Archive only verified bug, update report/indices/replay provenance, validate current links/IDs/raw hashes/privacy/foreign blobs, commit evidence and verify committed bytes. Keep the count-instrument bug outside this source patch.

## Rulings and progress

Ruling: Existing repeated continue/go instructions preserve inline execution and local integration authorization; do not reopen an approval workflow. The user's tracker organization overrides default superpowers paths. Preserve evidence/worktrees rather than deleting the SDD workspace, because future replay data is explicitly required.

Pre-flight: one task, no inter-task interfaces. Its metadata, traversal, resolver and cache consumers are one contract-bearing change reviewed together. Native worktree inventory transport is unavailable; verified ignored .worktrees Git fallback used. Source implementation has not started.

## TDD execution observations

First two named-slot0 dispatches did not execute tests (exit2). The source-bound slot2 run actually executed both new tests and failed0passed/2failed: module-qualified case loses helper→leaf_b; unqualified ambiguous case follows helper→leaf_a arbitrarily. Fresh baseline native seven-case execution exited0:4/4 qualifying controls matched,3/3 duplicate-depth2 cases remained clean but wrong, scorer calibration5/5. Production implementation began only after observing those RED failures.

## Build-slot ruling

Ruling: Slot0 was refused twice with exit2 and no test execution. Use the already existing named slot2 for parent targeted builds while slot0 is occupied, keeping the lease inside the actual source worktree; reviewer remains on slot1. This changes resource placement, not test/source semantics, and avoids interfering with the holder. Both refused attempts remain retained.

## Interface rulings

Ruling: Preserve the existing OneHopResolver.one_hop(&str) and bfs seed parameter for existing graph-only adapters/tests. Add seed_node/one_hop_at/next_node hooks; production seed_node produces a concrete TraversalNode before visited/queue insertion. Default hooks retain named behavior only for graph-only adapters. This minimizes unrelated changes while the actual CachedResolver path uses definition keys.

Ruling: Keep completed-cache storage/schema unchanged; the actual CachedResolver consumer refuses legacy LSP payloads lacking endpoint metadata and recomputes. Existing aggregate row decoding sets target=None. A separate rust_target.rs holds declared-module filesystem rules to keep identity.rs focused.

Ruling: A complete fallback scan that finds no declaration remains a quiet miss, preserving existing trustworthy-negative tests. Ambiguity or unexamined scope is disclosed; unsupported receiver/qualified expressions do not fall through to an arbitrary short-name candidate. Macro/type/alias inference remains outside this bound.

## Position and test-wire rulings

Ruling: A canonical visited identity is separate from the provider's operational query position. Initial canonicalization replaced the latter and broke3 existing outer-tool scope tests (83pass/3fail). Separating the queue key from the actual LSP position restored86/0, including both new regressions. Keep those red/green logs as evidence of this implementation correction.

Ruling: Existing MockLspClient keyed hierarchy replies only by display name. To test genuine same-name same-file/line targets without changing their names, its test-only hierarchy methods optionally use opaque string item.data as the scripted response key. Default name-key behavior remains for existing fixtures. This uses the real LSP opaque-data field and adds no production wire API.

## Independent review correction ruling

The frozen initial candidate gate exited 1 only for clippy `unnecessary_sort_by`; all executed Rust/Python tests passed. The reviewer actually applied 13 mutations: 8 killed, 5 survived, 0 inconclusive, with restoration checked in both trees. Survivors were incoming endpoint forwarding (M05), legacy LSP cache guard (M07), new language-family scan guard (M09), next-target canonicalization (M12), attributed-module guard (M13). Add behavior-observing tests only after these observed survivors, then recheck the exact mutations. Four separate rustc-valid native lexical-shadowing fixtures produce wrong clean branches; fix filed bug `dcecb5f9c3475b38` in the same pass, using explicit incomplete analysis when fallback cannot establish the binding. Ordinary call-site context remains bounded; no new semantic type/import engine. Preserve all initial candidate hashes, logs, binary, native responses and reviewer findings; final source and verification use separate manifests.

## Final source verification and integration scope

Final ten-file source is frozen separately from the immutable eight-file initial candidate. Related suite95/0 and full source gate exit0 (lean6398/default9301 passed; Python1827passed,7skipped,554subtests; all format/clippy lanes0). Final binary45206b13: original native7 graph cases match; extended4/4 plus2warningcontrols; lexical4/4 wrong-target-removal-and-warning checks plus3controls. Nested collector extra edge remains parked in bug0f9e7f9b87799265; the lexical predicate deliberately does not certify the whole graph.

The initial `src/scripts/tests/crates/Cargo` diff selector was too narrow to characterize all peer changes: complete diff at195317920ea2877ead34435c99df0a650f86d7b5 has four `tools/setup-ab-eval/*.py` changes. The user-facing claim that peer changes were only documentation was corrected immediately after the complete comparison. A separate retained prospective integrated-tree full gate includes that exact peer HEAD and the final source patch. Do not claim whole-tree equivalence from the earlier narrow selector. Subsequent source, integration and mutation results are recorded in Completion; this paragraph preserves the earlier verification-scope correction.

## Completion — 2026-11-10

Source537fa820d95b3298393cc515ab104776a57455c7 integrateddc769e430b7df552733e55693391f9bdf31b4b1b on experiments; patch-id52c9b1d039bbdd7de6183eefca168f32318d8cd8. Exact integration tree115cd5bb196cd91c7763deede84d3c4e93070059 matches the separately full-gated tree with the four peer Python harness changes. Both gates pass; final95related tests and15actual mutations pass, native populations are scoped in the report. Four verified bugs archived; live aliases and outbound links updated in this coupled evidence commit. Historical raw findings keep original identities. Six foreign index blobs preserved. Runtime state/compiler outputs remain local and excluded from staged evidence. Retain worktrees, scratch and private exact inputs; no replay/push/shared runtime/profile changes.

Ruling: The controller-authored R1–R5 contract is adjudicated against the executed source/tests and review findings; no additional human approval is inferred or requested for routine implementation choices already authorized. The separate spec-drift skill was inspected for applicability but not invoked: it would require a newly confirmed extraction for this nonstandard controller requirements format. This does not replace the actual acceptance evidence and mutation review.

## LSP endpoint RED

The same-file/same-line LSP target test actually ran and failed0passed/1failed: both helper targets had only their shared name/file and were correctly treated as ambiguous, so neither leaf appeared. After this RED, resolve_via_lsp now carries valid incoming.from/outgoing.to selection positions into Edge.target; invalid/empty hierarchy ranges remain unspecified and use the bounded fallback. The opaque-data mock change is test-only and preserves default scripts.

## Initial related GREEN

After preserving valid LSP endpoint selection positions, the related suite passed87/0, including same-file/same-line helper columns and a cached repeat after wire replies were cleared. Removed one unused import and formatted the owned files. The frozen initial candidate now undergoes independent native/gate collection; candidate-source.json and candidate.patch bind all8 owned files. Any review fix must create a separate final manifest, preserving this candidate's data.

## Candidate inventory correction

The first verification wrapper stopped before freezing/building: two generated .codescout/native-provenance-v1 counter/lock files appeared in the worktree untracked inventory alongside the8 source files. The refused helper snapshot is retained; the corrected inventory explicitly excludes only those two runtime paths and records them separately. No source change, native/gate execution or gate verdict followed from that refused attempt. Unknown additional files still fail the inventory assertion.
