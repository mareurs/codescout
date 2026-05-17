---
status: mitigated
opened: 2026-05-01
closed:
severity: low
owner: marius
related: []
tags: ["call_graph", "lsp", "tree-sitter", "language-coverage"]
---

# BUG: `call_graph direction=callees` required LSP callHierarchy; no tree-sitter fallback (Phase B residual)

## Summary

`call_graph(direction="callees")` originally required `prepare_call_hierarchy` LSP support — every language without it returned `RecoverableError`. Resolved 2026-05-15 (depth 1) and 2026-05-16 (depth ≥ 2) for Rust, Python, TypeScript/JavaScript (incl. TSX/JSX), Kotlin, Java via `resolve_callees_via_ts` and `CachedResolver::lookup_pos_via_ts_in_seed_files`. **Phase B residual:** workspace-wide search when the definition lives in a sibling file is not yet implemented — the same-file scan returns `None` for cross-file identifiers without LSP, BFS continues silently, edges are dropped. The original entry was a "Known design limitation (not bugs)"; the residual keeps it open as a mitigated entry rather than fixed.

## Symptom (Effect)

Pre-2026-05-15: `call_graph(direction="callees")` on any non-LSP language returned a `RecoverableError` with the "activate-an-LSP" hint.

Post-2026-05-15 (residual): for the supported language set, cross-file callees without LSP are silently dropped — BFS completes but the edge graph is incomplete. `direction="callers"` is unaffected (it had a tree-sitter fallback all along).

## Reproduction

Pre-fix: any callees call on a non-LSP language. Residual: callees call on a supported-set language whose target callee is defined in a sibling file, with LSP disabled.

## Environment

- Date opened: 2026-05-01 (during Task 6 implementation)
- Components: `src/tools/symbol/call_edges/resolver.rs::resolve_via_ts`, `src/tools/symbol/call_graph/mod.rs::CachedResolver::lookup_pos`
- Eval anchor: `C-11` (`a → b → c → a` cycle, depth = 5) grades CORRECT with the full edge set in round 1 of 2026-05-16.

## Root cause

When `prepare_call_hierarchy` returned `None` and the caller requested `direction=callees`, `resolve_one_hop` returned a `RecoverableError` for every language. `direction=callers` had a tree-sitter fallback (via `LspClientOps::references` from the callee site outward) but the symmetric `references-from-callsite-to-callee` traversal is fundamentally different and required a tree-sitter implementation written specifically.

For depth ≥ 2: `CachedResolver::lookup_pos` previously gave up when both pre-seeded `positions` and LSP `workspace_symbols` failed, returning `None` and silently yielding empty hops from `one_hop`.

## Evidence

Eval case `C-11` (cycle case, depth = 5) was the deterministic anchor across rounds.

## Hypotheses tried

1. **Hypothesis:** Gate callees behind LSP only and surface a `RecoverableError` for ts-only languages. **Verdict:** Adopted as the pre-2026-05-15 status quo, then superseded by the ts-fallback fixes. **Evidence link:** see Fix.
2. **Hypothesis:** Walk the AST descendants of the enclosing function node, collect call-kind nodes, extract callee identifiers with per-grammar rules. **Verdict:** Confirmed — adopted as the depth-1 fix. **Evidence link:** see Fix.
3. **Hypothesis:** Add a same-file scan for top-level / impl-method definitions matching BFS-discovered identifiers. **Verdict:** Confirmed — adopted as the depth-≥-2 fix. **Evidence link:** see Fix.
4. **Hypothesis (Phase B, deferred):** Walk the project's source tree (bounded), parse each candidate file with tree-sitter, and match against the identifier. **Verdict:** Feasible but expensive without an index — deferred.

## Fix

**Depth 1 (2026-05-15):** `resolve_callees_via_ts` walks the AST descendants of the enclosing function node (found via `enclosing_function_node`), collects every call-kind node from `call_kinds_for(language_id)`, and extracts the callee identifier with per-grammar rules (`callee_identifier`). One `Edge` is emitted per call site with `EdgeSource::Ts`.

**Depth ≥ 2 (2026-05-16):** `CachedResolver::lookup_pos` now falls back to `lookup_pos_via_ts_in_seed_files`, which reuses `extract_symbols_from_source` to walk the seed file(s) for a top-level / impl-method definition whose name matches the BFS-discovered identifier. Scope is intentionally narrow: only files already present in `positions` are searched. The nav-eval case `C-11` grades CORRECT in round 1 of 2026-05-16.

**Phase B (not yet implemented):** workspace-wide search when the definition lives in a sibling file. Today the same-file scan still returns `None` for cross-file identifiers without LSP; BFS then continues for the rest of the graph and the edge is silently dropped. A future fix could walk the project's source tree (bounded), parse each candidate file with tree-sitter, and match against the identifier — feasible but expensive without an index.

## Tests added

*N/A — migrated from compact form; the original entry tracked eval grade rather than naming specific unit tests. Eval anchor: `C-11`.*

## Workarounds

- For unsupported languages: activate a language server for the file.
- For cross-file cases on supported languages without LSP: same — activate LSP. `direction=callers` already had a tree-sitter fallback and is unchanged.

## Resume

If the Phase B residual blocks a real session: implement bounded workspace-wide tree-sitter search in `lookup_pos_via_ts_in_seed_files`. Concrete next action: open `src/tools/symbol/call_graph/mod.rs`, locate `CachedResolver::lookup_pos`, plan a bounded BFS that consults `extract_symbols_from_source` on each candidate file. Cap fan-out by file count; cache results within the resolver lifetime.

## References

- Originally tracked as **LIMIT-001** in `docs/TODO-tool-misbehaviors.md` (deprecated 2026-05-09; superseded by per-file system).
- Plan: `docs/superpowers/plans/2026-05-01-call-graph.md` Task 6 (where the limitation was first surfaced).
- Eval anchor: `C-11` (cycle case, depth = 5).
