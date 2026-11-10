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

Traced 2026-11-10. The tree-sitter callee fallback `resolve_callees_via_ts` (`src/tools/symbol/call_edges/resolver.rs`) walks the queried function's subtree with a stack and pushed EVERY child. It never stopped at a nested callable declaration, so a call inside a nested `fn helper` became an edge of the outer function. `enclosing_function_node` and `callee_identifier` are not involved. `count_calls_named` walks the whole file and has no enclosing-function scope, so it is not affected.

## Evidence

Case2 response first content block declares three edges, including `4: entry → right (depth=1, ts)`. Fixture hashes and successful rustc exit are retained with that response. The independently fixed lexical target issue is tracked separately; suppressing an arbitrary depth2 target does not correct this depth1 edge.

## Fix

Uncommitted in the working tree at the time of writing (cite the SHA and patch-id when it lands). New `nested_callable_kinds_for(language_id)` lists the declaration kinds that own a callable: Rust `function_item`, Python `function_definition`, TS/JS `function_declaration` and `method_definition`, Kotlin `function_declaration`, Java `method_declaration`. The walk skips a child of those kinds. Closures, lambdas, arrow functions and function expressions are NOT skipped, so their calls stay with the enclosing function.

## Tests added

`resolve_callees_via_ts_rust_excludes_calls_inside_a_nested_fn` (watched RED: returned `[helper, right]`, expected `[helper]`; GREEN after the fix) and `resolve_callees_via_ts_rust_keeps_calls_inside_a_closure` (guards the boundary; passes before and after). Only Rust is covered by a test. The Python, TS/JS, Kotlin and Java entries in `nested_callable_kinds_for` have no regression test. Measured: `cargo test --lib call_edges::resolver` 26 passed; `cargo clippy --lib --tests -- -D warnings` clean. The full `./scripts/gate.sh` was not run.

## Resume — 2026-11-10

Open, outside the current identity pass. Targeted catalog title searches found no existing record; unindexed files and all semantic variants were not exhaustively audited.
