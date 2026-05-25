---
status: fixed
opened: 2026-05-25
closed: 2026-05-25
severity: low
owner: marius
related: []
tags:
  - call_graph
  - tree-sitter
  - false-positive
  - lsp-fallback
kind: bug
---

# BUG: tree-sitter callee fallback emits spurious self-edges for leaf functions

## Summary
When `call_graph` walks depth>1 over a symbol whose LSP `prepare_call_hierarchy`
returns None (e.g., `#[test]` functions or other leaf functions RA doesn't
model in its call hierarchy), the tree-sitter fallback in
`src/tools/symbol/call_edges/` emits a self-edge — claiming the function
calls itself. Pre-existing behavior surfaced by the depth>1 timeout fix
(commit `bd8fdbb4`), which let the BFS actually complete depth=2 traversal
for the first time.

## Symptom (Effect)
`call_graph(symbol="bfs", direction="callers", max_depth=2)` against this
repo returns 5 spurious self-edges in the depth=2 layer, one per test
function in `src/tools/symbol/call_graph/traversal.rs::tests`:

```
193: bfs_reaches_max_depth_then_stops → bfs_reaches_max_depth_then_stops (depth=2, ts)
216: bfs_handles_cycle_without_infinite_loop → bfs_handles_cycle_without_infinite_loop (depth=2, ts)
239: bfs_depth_coherent_cap_preserves_full_levels → bfs_depth_coherent_cap_preserves_full_levels (depth=2, ts)
266: bfs_dedupes_parallel_paths → bfs_dedupes_parallel_paths (depth=2, ts)
319: bfs_parallelizes_one_hop_within_level → bfs_parallelizes_one_hop_within_level (depth=2, ts)
```

Notable shape: every edge has `source: ts` (tree-sitter fallback path)
and `caller_sym == callee_sym`. Test functions in `#[cfg(test)]` modules
are entry points called by the harness — they don't call themselves and
they don't have user-code callers. The depth=2 layer for them should be
empty.

The 7 depth=1 edges (which are LSP-sourced and correct) are unaffected:
2 production callers in `src/tools/symbol/call_graph/mod.rs` (BFS dispatch
sites) + 5 test callers in `tests/` (the 5 test fns calling `bfs()`).

## Reproduction

Branch: `experiments` and `master` (converged at `4e56c29c`).
Codescout version: 0.13.0.
Git HEAD: `4e56c29c3a982c89316f7e79151351041e7d5575`.

Steps:

```
call_graph(
  symbol="bfs",
  path="src/tools/symbol/call_graph/traversal.rs",
  direction="callers",
  max_depth=2,
)
```

Expected: 7 edges (2 production + 5 test depth=1 callers), `max_depth_reached: 2`.
Got: 12 edges — the 7 correct depth=1 + 5 spurious depth=2 self-edges.

## Environment

- OS: Linux 7.0.9-zen1-1-zen
- Rust toolchain: per `rust-version = "1.88"` in `Cargo.toml`
- LSP backend: rust-analyzer
- Pre-existing behavior — was masked by the `call_graph(depth>1)` 60s timeout
  bug (`docs/issues/archive/2026-05-25-call-graph-depth2-timeout.md`).
  The fix in commit `bd8fdbb4` let depth=2 traversal complete, surfacing
  this downstream defect for the first time.

## Root cause

Confirmed by deterministic regression test. The TS callers fallback in `src/tools/symbol/call_edges/resolver.rs::resolve_via_ts` invokes `client.references(sym_path, sym_line, sym_col, lang)` and then iterates the returned locations. For each location it runs `position_is_call` to filter non-call references and `enclosing_function_name` to determine the caller. The edge is built with `caller_sym = enclosing_function_name(...)` and `callee_sym = sym_name` (the query symbol).

When rust-analyzer cannot model a symbol in its call hierarchy (e.g. `#[test]` fns inside `#[cfg(test)] mod tests`), `prepare_call_hierarchy` returns None and `references()` may return locations INSIDE the symbol's own body — for example, the call site of a different function (`bfs`) at line 192 inside `bfs_reaches_max_depth_then_stops`. The AST walk-up resolves that location's enclosing function back to the queried symbol itself, producing `caller_sym == callee_sym == sym_name` — a spurious self-edge.

The pre-existing test `resolve_one_hop_ts_fallback_callers` already exercised the cross-function case (refs INSIDE another function's body) and passed. The new bug case (refs INSIDE the queried symbol's OWN body) was untested until 2026-05-25 and surfaced only after the depth>1 timeout fix (commit `bd8fdbb4`) let depth=2 traversal actually complete.
## Evidence

### E1 — Self-edges only appear under tree-sitter source

Every spurious edge in the symptom output has `source: ts`. The LSP-sourced
edges (depth=1) are correct. This narrows the defect to
`resolve_via_ts` (`src/tools/symbol/call_edges/resolver.rs`) — the
tree-sitter callers fallback path taken when `prepare_call_hierarchy`
returns None.

### E2 — Line numbers match the depth=1 call sites, not the function declarations

The reported line numbers (193, 216, 239, 266, 319) are the lines where
each test fn calls `bfs(...)` — NOT the line of the test fn's own
declaration (e.g., `bfs_reaches_max_depth_then_stops` declared at L183).

This is the same line that appears as the depth=1 caller location.
So the depth=2 expansion may be reusing the depth=1 reference location
instead of independently looking up references TO the test fn.

Two hypotheses for this E2 shape — see H3 / H4 in Hypotheses tried.

### E3 — All affected symbols are entry points the LSP doesn't fully model

`#[test]` functions and other functions inside `#[cfg(test)] mod tests`
blocks. RA's `prepare_call_hierarchy` typically returns None for these
(they're not callable from user code in the strict sense — the test
harness invokes them via attribute macros). The tree-sitter fallback is
forced for these symbols.

## Hypotheses tried

1. **Hypothesis (deferred):** RA's `references()` against a `#[test]` fn
   returns the fn's own definition site as a reference, and tree-sitter's
   walk-up-the-AST step reports the enclosing function — which is the
   function itself — as the caller.
   **Test:** Not yet run. Would require enabling RA trace logging
   (`RA_LOG=lsp_server=trace`) and inspecting the `textDocument/references`
   response for one of the affected test fns, then tracing through
   `resolve_via_ts` to see how each reference location is mapped to a
   caller symbol.
   **Verdict:** Deferred. This is the leading hypothesis.

2. **Hypothesis (deferred):** Tree-sitter's enclosing-function walk has an
   off-by-one or boundary bug at the start of a function declaration —
   when the reference IS the declaration token itself, walking up
   incorrectly resolves to the function itself rather than to the
   enclosing module.
   **Test:** Not yet run. Would require unit-testing
   `resolve_via_ts` against a known fixture where the only reference
   to a symbol is its own declaration.
   **Verdict:** Deferred.

3. **Hypothesis (deferred):** The CachedResolver's `lookup_pos` cache
   stores positions discovered during depth=1 traversal, and at depth=2
   uses those positions (which are call-site locations, not declaration
   locations) when querying RA for references — causing RA to look up
   "references to whatever's at line 193 col N" which happens to land
   on the `bfs` callee identifier in that test, returning weird results.
   **Test:** Not yet run. Read `lookup_pos` in
   `src/tools/symbol/call_graph/mod.rs::CachedResolver` and trace
   the symbol-to-position cache lifecycle.
   **Verdict:** Deferred. This is the secondary hypothesis based on E2's
   "line numbers match the call sites, not the declarations" observation.

4. **Hypothesis (deferred):** The tree-sitter callee fallback fails to
   filter references inside the symbol's own definition range. The fix
   would be to compare each reference's location to the symbol's
   `(start_line, end_line)` and skip references that fall within that
   range (the function can't be its own caller via static analysis;
   recursion is a separate case that LSP `incoming_calls` handles correctly).
   **Test:** Not yet run. Would manifest as a one-line filter in
   `resolve_via_ts`.
   **Verdict:** Deferred. This is the leading candidate FIX direction
   regardless of which of H1/H2/H3 is the underlying cause — filtering
   self-references is safe and correct independently.

## Fix

Shipped on master in commit `23746b6e` (`fix(call_graph): filter spurious self-edges from tree-sitter callers fallback`).

Fix is a 4-line filter added to `src/tools/symbol/call_edges/resolver.rs::resolve_via_ts` (callers branch), placed immediately after `enclosing_function_name` resolves the caller:

```rust
if caller == sym_name {
    continue;
}
```

Real self-recursion is modeled by LSP `incoming_calls` (the LSP path in `resolve_via_lsp`), not by this tree-sitter fallback — so dropping `caller == sym_name` edges in the TS path loses no real edges in practice. The fallback is invoked exactly when LSP can't model the symbol; in that case the references list cannot reliably distinguish "external caller" from "internal name token whose enclosing fn happens to be the queried symbol."

**Live verification post-rebuild + `/mcp` reconnect:**
- `call_graph(symbol="bfs_reaches_max_depth_then_stops", direction="callers", max_depth=1)` returns `0 edges` (correct — `#[test]` fns have no real callers; pre-fix returned 1 spurious self-edge).
- `call_graph(symbol="bfs", direction="callers", max_depth=2)` returns no `(depth=2, ts)` self-edges (pre-fix returned 5 spurious self-edges, one per test fn).
## Tests added

`resolve_one_hop_ts_fallback_callers_skips_self_edge` in `src/tools/symbol/call_edges/resolver.rs` (inside the existing `tests` module, immediately after the pre-existing `resolve_one_hop_ts_fallback_callers` test).

Uses `MockLspClient` with `prepare_call_hierarchy_results` empty (forces TS fallback) and `references_results` seeded with a single reference location INSIDE the queried symbol's own body — at the `bar(1)` call inside `fn foo() { bar(1); }`. Queries callers of `foo`, asserts no edge with `caller_sym == callee_sym == "foo"`.

Deterministic. Fails against pre-fix code with `left: ("foo", "foo") right: ("foo", "foo")`. Passes after the 4-line filter in `resolve_via_ts` lands.

The pre-existing 12 sibling tests in `call_edges::resolver::tests` (including `resolve_one_hop_ts_fallback_callers` which exercises the cross-function case at refs inside `bar`'s body querying callers of `foo`) continue to pass unchanged, confirming the filter doesn't break cross-edge resolution.
## Workarounds

- Visually filter `source: ts` self-edges at the caller side (Claude /
  agents reading `call_graph` output can ignore depth>1 edges where
  `caller_sym == callee_sym && source == "ts"`).
- The LSP-sourced edges (depth=1 in this repro, and any depth>1 edge
  where RA models the call hierarchy) are unaffected.

## Resume

N/A — fixed.
## References

- Surfacing event: 2026-05-25 reconnect verification of
  `docs/issues/archive/2026-05-25-call-graph-depth2-timeout.md` fix
  (master commit `bd8fdbb4`). Before the fix, depth=2 timed out before
  reaching the tree-sitter fallback for these symbols, hiding this bug.
- Related tracker: `docs/trackers/lsp-tools-error-rate-2026-04.md`
  (artifact id `f87ff6fcbb6eaa56`) — broader LSP-tools error patterns.
- Code likely involved: `src/tools/symbol/call_edges/resolver.rs::resolve_via_ts`
  and possibly `src/tools/symbol/call_graph/mod.rs::CachedResolver::lookup_pos`.
