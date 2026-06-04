---
status: fixed
opened: 2026-06-04
closed: 2026-06-04
severity: low
owner: marius
related: [2026-06-04-rust-ast-drops-assoc-items-macros]
tags: [ast, go, tree-sitter, generics, name-path]
kind: bug
---

# BUG: Go AST extractor loses the receiver type for methods on generic receivers (`*Stack[T]`)

# Summary
`extract_go_receiver` unwrapped `*Type` (pointer_type) by looking for a `type_identifier` child,
but a **generic** receiver `*Stack[T]` nests the type identifier one level deeper inside a
`generic_type` node. The lookup returned nothing, so the method was emitted with a bare name_path
(`Push`) instead of the type-qualified `Stack/Push`. Quality bug, not a hard drop — the method
symbol still exists, just mis-pathed.

# Symptom (Effect)
`symbols(path)` / the AST tree show generic-type methods at the wrong hierarchy level (top-level,
no `Type/` prefix). Mostly cosmetic; can weaken `edit_code`'s name_path tiebreaker if two generic
types share a method name. No "AST parse failed" (the symbol is present, name+line still match).

# Reproduction
Verified 2026-06-04 via `extract_symbols_from_source` (unit harness):
```go
package main
type Stack[T any] struct { items []T }
func (s *Stack[T]) Push(x T) {}
func (s Stack[T]) Len() int { return 0 }
```
Pre-fix: `Push` (bare). Post-fix: `Stack/Push`, `Stack/Len`.

# Environment
codescout (`experiments` HEAD), tree-sitter-go. Node kinds confirmed via dump:
`parameter_declaration > pointer_type > generic_type > type_identifier "Stack"`.

# Root cause
`extract_go_receiver` handled only `pointer_type > type_identifier`. For a generic receiver the
pointee is a `generic_type`, so `find(type_identifier)` inside `pointer_type` returned `None` →
empty receiver → `make_name_path` fell back to the bare method name.

# Fix
**Implemented 2026-06-04 on `experiments`** (`src/ast/parser.rs`): generalized `extract_go_receiver`
to unwrap `pointer_type` to its first named pointee, then unwrap a `generic_type` to its base
`type_identifier`. Covers `Type`, `*Type`, `Type[T]`, `*Type[T]`.

**Verification (2026-06-04):** regression test below; clippy clean; full lib suite green
(2611 pass, 7 ignored, 0 fail).

# Tests added
- `ast::parser::tests::go_generic_receiver_keeps_type_path` — `Stack/Push` (pointer generic) and
  `Stack/Len` (value generic) resolve with the type prefix.

# Workarounds
- None needed (cosmetic); the method was always reachable by name.

# Resume
**Fixed 2026-06-04 on `experiments`** (see ## Fix). Not yet on master. Ships alongside the other
extractor fixes; on landing `git mv` to `docs/issues/archive/` and cite the **master-side** SHA.

# References
- `src/ast/parser.rs` `extract_go_receiver`.
