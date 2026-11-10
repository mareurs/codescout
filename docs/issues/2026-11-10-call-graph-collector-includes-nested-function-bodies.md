---
id: '0f9e7f9b87799265'
kind: bug
status: open
title: Call graph AST collector attributes nested function calls to the enclosing function
tags:
- call-graph
- cluster/unclassified
closed: null
opened: 2026-11-10
owner: marius
severity: medium
---

# BUG: nested function calls are attributed to the outer function

## Summary

The independent BFS identity review observed an extra `entry → right` depth1 edge for a call syntactically inside a nested helper. This collector behavior is distinct from next-hop lexical target placement and is parked outside that bounded repair.

## Class fit

Unclassified pending collector tracing. The observed edge loses its enclosing callable boundary, whereas the current traversal repair preserves declaration identity after an edge has been emitted. No class roster is modified.

## Symptom (Effect)

For `fn entry() { fn helper() { right(); } helper(); }`, the initial candidate's native response includes `entry → helper` and `entry → right`, both at depth1. The `right()` call belongs to the nested helper body.

## Reproduction

Use case `nested-function-shadows-root` in [raw native reviewer results](../trackers/ecc-integration/evidence/2026-11-10-bfs-identity-dogfood/review/native-results.json). The complete source compiles with rustc exit0. The immutable initial candidate binary is bound by candidate-binary.json and source manifest in that evidence directory. Request callees for entry, max_depth2, full detail; inspect first MCP content block only.

## Environment

Frozen base `7f0dbba5fd6ba25e4a50a8aad5c25403a11ae345` plus initial candidate patch; private fresh Rust MCP project. Observed edge source is tree-sitter.

## Root cause

Unknown — scope collector traversal has not been traced in this bounded pass. The raw graph demonstrates the incorrect containing-caller attribution, but does not establish which recursive collector branch owns it.

## Evidence

Case2 response first content block declares three edges, including `4: entry → right (depth=1, ts)`. Fixture hashes and successful rustc exit are retained with that response. The independently fixed lexical target issue is tracked separately; suppressing an arbitrary depth2 target does not correct this depth1 edge.

## Fix

Pending. Bound AST call collection to the queried callable and exclude nested callable bodies without dropping calls made by the enclosing function.

## Tests added

None for this collector defect. Current lexical regression checks absence of an invented wrong leaf and presence of an incomplete-binding warning; it does not certify the extra depth1 right edge.

## Resume — 2026-11-10

Open, outside the current identity pass. Targeted catalog title searches found no existing record; unindexed files and all semantic variants were not exhaustively audited.
