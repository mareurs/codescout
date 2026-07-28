---
status: fixed
opened: 2026-06-04
closed: 2026-06-04
severity: high
owner: marius
related: [2026-06-04-kotlin-ast-drops-nested-classes, 2026-06-04-ts-extractor-drops-namespace-abstract-class]
tags: [edit_code, ast, rust, tree-sitter, macro, associated-items]
kind: bug
---

# BUG: Rust AST extractor drops `macro_rules!`, `union`, and associated `const`/`type` in impl/trait → `edit_code` fails on them

# Summary
`src/ast/parser.rs` dropped several Rust item kinds from the tree-sitter symbol tree:
`macro_rules!` (`macro_definition`), `union` (`union_item`), trait associated types
(`associated_type`), and — the highest-impact — associated `const`/`type` inside `impl` blocks
(`extract_rust_impl_methods` matched only `function_item`). Any `edit_code` op on one of these
symbols failed with the misleading "AST parse failed" guard, because the LSP (rust-analyzer)
resolves them but the AST returned no matching candidate. Found by the post-Kotlin/Java extractor
audit; affects this very codebase (Rust).

# Symptom (Effect)
`edit_code(symbol="MyTrait::Item" | "Foo::OUTPUT" | "my_macro", action="insert"/"replace")` →
`cannot determine end of '<sym>' for … — AST parse failed`, even though the file is valid Rust.
`symbols(path)` also omits these symbols.

# Reproduction
Verified 2026-06-04 via `extract_symbols_from_source` (unit harness):
```rust
macro_rules! my_macro { () => {}; }
union MyUnion { a: i32 }
pub trait MyTrait { const LIMIT: i32; type Item; fn required(&self); }
impl S { const NAME: &'static str = "s"; type Output = i32; fn method(&self) {} }
```
Pre-fix extracted: `MyTrait`, `MyTrait/LIMIT`, `MyTrait/required`, `S/method` only.
Missing: `my_macro`, `MyUnion`, `MyTrait/Item`, `S/NAME`, `S/Output`.

# Environment
codescout (`experiments` HEAD), tree-sitter-rust. Reproduced in the unit harness via an
extractor-coverage probe; node kinds confirmed with a parse-tree dump.

# Root cause
- `extract_rust_symbols` had no arm for `macro_definition`, `union_item`, or `associated_type`
  (the trait form of `type X;` — distinct from the standalone/impl `type_item`).
- `extract_rust_impl_methods` matched only `function_item`, silently dropping `const_item` and
  `type_item` associated items inside `impl` blocks.

Downstream chain is the shared AST-vs-LSP one: rust-analyzer resolves the symbol →
`ast_confirmed_end_line` → `extract_symbols_from_source` (missing the sym) → `find_ast_end_line_in`
→ 0 candidates → `None` → `do_insert` refuses.

# Fix
**Implemented 2026-06-04 on `experiments`** (`src/ast/parser.rs`):
- Added `extract_rust_symbols` arms: `macro_definition` (→ Function; name = first `identifier`
  child via new `first_named_child_text` fallback), `union_item` (→ Struct), `associated_type`
  (→ TypeParameter).
- Rewrote `extract_rust_impl_methods` as a `match`: `function_item` → Method, `const_item` →
  Constant, `type_item` → TypeParameter.
- New helper `first_named_child_text(node, source, kind)` for names not exposed under a `name`
  field (Rust `macro_definition`).

**Verification (2026-06-04):** regression test below; `cargo clippy --all-targets -- -D warnings`
clean; full lib suite green (2611 pass, 7 ignored, 0 fail). Live needs a `/mcp` restart.

# Tests added
- `ast::parser::tests::rust_assoc_items_and_macros_are_extracted` — asserts `my_macro`, `MyUnion`,
  `MyTrait/Item`, `S/NAME`, `S/Output`, `S/method` all extracted. Red pre-fix on all but the last.

# Workarounds
- `edit_file` with explicit context (when not blocked by `debug_enforce_symbol_tools`).

# Resume
**Fixed 2026-06-04 on `experiments`** (see ## Fix). Not yet on master. Ships alongside the other
extractor fixes; on landing `git mv` to `docs/issues/archive/` and cite the **master-side** SHA.

# References
- `src/ast/parser.rs` `extract_rust_symbols`, `extract_rust_impl_methods`, `first_named_child_text`.
- Siblings: `2026-06-04-kotlin-ast-drops-nested-classes.md`,
  `2026-06-04-ts-extractor-drops-namespace-abstract-class.md`.
