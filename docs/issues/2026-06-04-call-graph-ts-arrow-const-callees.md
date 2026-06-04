---
status: fixed
opened: 2026-06-04
closed: 2026-06-04
severity: medium
owner: marius
related: [2026-06-04-ts-extractor-drops-arrow-fn-consts]
tags: [call_graph, callees, typescript, javascript, tree-sitter, arrow-function]
kind: bug
---

# BUG: `call_graph` direction=callees TS fallback can't resolve callees of arrow-function / function-expression consts

# Summary
`enclosing_function_node` (`src/tools/symbol/call_edges/resolver.rs`) — the helper the
tree-sitter callees fallback uses to find the body to scan — walks **ancestors only** from the
resolved symbol position. For a function-valued const (`const f = () => {…}`,
`const f = function(){…}`), the resolved symbol position is the binding **name**, and the function
body is the declarator's *sibling* `value`, not an ancestor. So the walk-up never reaches the body
and the fallback errored with "could not locate enclosing function." `arrow_function` was in the
TS `fn_kinds` list but was **unreachable** from the name position, and `function_expression` was
missing entirely.

Found while auditing `call_graph` as a consumer of the symbol extractor (the same audit that fixed
the arrow-const symbol-extraction bug, `2026-06-04-ts-extractor-drops-arrow-fn-consts`).

# Symptom (Effect)
`call_graph(symbol="App", direction="callees")` on a `const App = () => {…}` returns no callees
when the **LSP callHierarchy is unavailable** (LSP-down / unsupported), where the tree-sitter
fallback is used. Since arrow consts are the dominant modern TS/JS function form, callees were
effectively unavailable for most TS functions in degraded (LSP-down) mode. LSP-up path
(callHierarchy) was unaffected.

# Reproduction
Verified 2026-06-04 via `resolve_one_hop(..., Direction::Callees)` with a mock (no-LSP) client
(unit harness, temporary probe):
```typescript
const a = () => { b(); obj.c(); };
function b() {}
```
`resolve_one_hop(mock, "a", file, 0, 6, "typescript", Callees)` →
pre-fix `Err("could not locate enclosing function for callees fallback")`;
post-fix `Ok([b, c])`.

# Environment
codescout (`experiments` HEAD), tree-sitter-typescript. Mock LSP client (forces the tree-sitter
fallback). Confirmed by reading `enclosing_function_node` + a probe asserting the failure.

# Root cause
`enclosing_function_node` does `descendant_for_byte_range(byte)` then walks `node.parent()` up,
matching `fn_kinds`. For `const a = () => {}` the position lands on the `identifier` `a`; its
ancestor chain is `identifier → variable_declarator → lexical_declaration → program`. The
`arrow_function` is `variable_declarator.value` — a sibling subtree, never on the ancestor path.
`function_expression` was also absent from the TS `fn_kinds`.

# Fix
**Implemented 2026-06-04 on `experiments`** (`src/tools/symbol/call_edges/resolver.rs`):
- Added `function_expression` to the TS `fn_kinds` (covers caller-attribution of call-sites inside
  a `function(){}` body, mirroring the existing `arrow_function`).
- In the walk-up loop, when the current node is a `variable_declarator` (TS/JS) whose `value` field
  is an `arrow_function`/`function_expression`, return that value node — descending from the
  binding name into the sibling function body.

**Verification (2026-06-04):** regression test below; `cargo clippy --all-targets -- -D warnings`
clean; full lib suite green (2612 pass, 7 ignored, 0 fail).

# Tests added
- `tools::symbol::call_edges::resolver::tests::resolve_callees_via_ts_function_valued_consts` —
  callees of `const a = () => {…}` (`b`, `c`) and `const e = function(){…}` (`d`) resolve via the
  TS fallback. Red pre-fix (errored on the enclosing-function lookup).

# Workarounds
- Use `call_graph` with a live LSP (callHierarchy handles function-valued consts correctly).
- `direction=callers` was unaffected (call-site is inside the body, so ancestor-walk finds it).

# Resume
**Fixed 2026-06-04 on `experiments`** (see ## Fix). Not yet on master. Ships alongside the other
extractor/call-graph fixes; on landing `git mv` to `docs/issues/archive/` and cite the
**master-side** SHA.

# References
- `src/tools/symbol/call_edges/resolver.rs` `enclosing_function_node`, `resolve_callees_via_ts`.
- Sibling (symbol-side): `2026-06-04-ts-extractor-drops-arrow-fn-consts.md`.
