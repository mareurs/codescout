---
id: dcecb5f9c3475b38
kind: bug
status: fixed
title: Call graph AST fallback ignores lexical bindings and silently invents unrelated branches
tags:
- call-graph
- cluster/addressing-without-an-escape-hatch
closed: 2026-11-10
opened: 2026-11-10
owner: marius
severity: medium
---

# BUG: AST fallback ignores lexical bindings

## Summary

The frozen BFS identity candidate resolves unsupported lexical bindings to unrelated same-name declarations and returns a clean but wrong depth2 graph. Four valid Rust fixtures reproduce this through the immutable candidate's real MCP transport.

## Class fit

The addressing scheme retains module names and declaration coordinates but omits the lexical binding that selects the declaration. Unlike the existing multi-hop visited-key collision, this defect invents a concrete target before the visited key is formed. It fits addressing-without-an-escape-hatch because a valid source namespace distinction cannot be represented in the fallback lookup.

## Symptom (Effect)

A block-local import, a callable parameter, a nested function or a generic type parameter shadows a file-level helper/type. Each observed result contains helper→wrong at depth2 without a completeness warning. The imported and nested helper actually call right; the parameter/generic target is not statically established.

## Reproduction

Use the four fixtures and arguments in [review native results](../../trackers/ecc-integration/evidence/2026-11-10-bfs-identity-dogfood/review/native-results.json). Example: file-level helper calls wrong; entry has `use a::helper; helper();`, where a::helper calls right. Request call_graph on entry, callees, max_depth2. All four fixture sources independently compile with rustc exit0.

## Environment

Frozen base7f0dbba5fd6ba25e4a50a8aad5c25403a11ae345 plus the eight-file candidate.patch; candidate-source.json binds source. Immutable executable SHA2561fa4600ccbba259e65fce6c28905e24ee391b10d462302fa3698797b74f4acd3. Fresh isolated MCP server and project per fixture; observed edges are tree-sitter. No source fixes or regression tests applied by reviewer.

## Root cause

Candidate src/tools/symbol/call_graph/identity.rs:169-254 extracts the call expression and module ancestors but no lexical imports, parameters, local bindings or generic type parameters. identity.rs:338-351 then accepts rust_target::resolve or the unique workspace fallback. rust_target.rs:129-143 resolves simple names at module scope and its type-qualified fallback at lines66-77 accepts a coincidentally matching concrete type name. The enclosing binding can select a different declaration or require semantic resolution.

## Evidence

Raw MCP payloads, fixture byte hashes, compiler statuses: review/native-results.json. Exact fixture files and rustc stderr are retained under evidence/2026-11-10-bfs-identity-dogfood/review/native/. All four first content blocks show the wrong leaf without a warning; guide text is excluded from this observation.

## Hypotheses tried

The fixtures might be invalid Rust; all four compiled. The missing warning might be hidden in guide text; actual graph payloads themselves begin with call_graph and include no warning.

## Fix

The Rust call-site context records enclosing lexical imports, parameters, generic type parameters, visible local bindings and nested items. Semantic goto-definition results take precedence. Unsupported lexical placement stops before exact/module/workspace name coincidence and emits a completeness warning. This does not implement semantic binding inference. Source537fa820d is verified; integrated locally on experiments as dc769e430b7df552733e55693391f9bdf31b4b1b.

## Fix provenance

- **SHA:** `dc769e430b7df552733e55693391f9bdf31b4b1b` (`experiments`)
- **patch-id:** `52c9b1d039bbdd7de6183eefca168f32318d8cd8`
- **Source:** `537fa820d95b3298393cc515ab104776a57455c7`
- **Gated integration tree:** `115cd5bb196cd91c7763deede84d3c4e93070059`

Both required full gates passed; final15 mutation runs killed all candidates and restored the ten frozen source hashes after each run.

## Tests added

A table-driven outer Tool regression executes seven lexical-shadowing forms and checks no arbitrary wrong leaf plus warning in JSON and compact output; an expired/unrelated binding control retains the ordinary helper branch. Final native4/4 lexical predicates and3/3 scorer controls pass. The nested-body collector's extra depth1 edge remains separately open in bug0f9e7f9b87799265, so no whole-graph correctness claim follows.

## Workarounds

Use a functioning semantic provider or request only depth1 for unsupported bindings.

## Resume

**CLOSED 2026-11-10 — verified and integrated on experiments.** Source537fa820d; integrationdc769e430b7df552733e55693391f9bdf31b4b1b. Related95/0,15actual mutation rechecks killed, source and exact integrated-tree full gates passed. Initial counterexamples and unsuccessful attempts above remain historical evidence. Separate nested-body collector/navigation/count-instrument issues remain open. No repair replay.

## References

[Repair contract](../../trackers/ecc-integration/reports/2026-11-10-bfs-identity-contract.md), existing visited collision 382466d3e15a3d1d.
