---
status: fixed
opened: 2026-06-04
severity: low
owner: marius
related: [2026-06-04-rust-ast-drops-assoc-items-macros, 2026-06-04-ts-extractor-drops-arrow-fn-consts, 2026-06-04-ts-extractor-drops-namespace-abstract-class]
tags: [ast, docstrings, tree-sitter, include_docs]
kind: bug
closed: 2026-06-04
---

# BUG: docstring extractors lag the symbol extractors in node-kind coverage and are top-level-only

# Summary
The `extract_*_docstrings` family in `src/ast/parser.rs` (the structural sibling of the symbol
extractors) has two coverage gaps that surfaced while auditing the symbol-extractor bug family:

1. **Node-kind lag.** The "next-sibling declaration" match that links a doc comment to a
   `symbol_name` lists only the original node kinds. Docs on the constructs recently made
   extractable as symbols — Rust `macro_rules!`/`union`, TS `abstract class`/`namespace`/arrow-fn
   `const` — record with `symbol_name=None` (unassociated), so `symbols(include_docs=true)` can't
   attach them.
2. **Top-level only (pre-existing).** Each extractor walks `node.children()` once and does **not**
   recurse into `impl`/`trait`/`class`/`namespace` bodies. Docs on impl methods, trait associated
   items, class members, and namespace-nested declarations are never extracted at all.

Impact is limited to `include_docs` completeness — no effect on `edit_code`, `symbols` navigation,
or `symbol_at`. Hence **low severity**. Logged for completeness during the 2026-06-04 extractor
audit; not fixed in that session.

# Symptom (Effect)
`symbols(path, include_docs=true)` omits docstrings for: top-level macros/unions/arrow-consts/
abstract-classes/namespaces (recorded but unassociated), and for any symbol nested inside an
impl/trait/class/namespace body (not extracted).

# Reproduction
Verified 2026-06-04 via `extract_docstrings_from_source` (unit harness, temporary probe):
```
### RUST
  sym=Some("top") content="top fn doc"
  sym=None        content="macro doc"     # macro_rules! — unassociated
  sym=None        content="union doc"     # union — unassociated
  (impl method's "/// method doc" — NOT extracted at all: no recursion)
### TS
  sym=Some("top") content="top fn"
  sym=None        content="arrow doc"     # const X = () => {} — unassociated
  sym=None        content="abstract doc"  # abstract class — unassociated
  sym=None        content="ns doc"        # namespace — unassociated
  (namespace-nested class's "/** inner */" — NOT extracted: no recursion)
```

# Root cause
- `extract_rust_docstrings` sibling match lacks `macro_definition`, `union_item`, `associated_type`.
- `extract_ts_docstrings` sibling match (incl. its `export_statement` unwrap) lacks
  `abstract_class_declaration`, `internal_module`/`module`, `lexical_declaration`.
- All six `extract_*_docstrings` iterate only the root's direct children — no descent into
  declaration bodies. (Likely mirrors the original symbol extractors before nesting was added.)

# Fix

**Facet 1 (node-kind lag) implemented 2026-06-04 on `experiments`.** Extended `extract_rust_docstrings` and `extract_ts_docstrings` sibling-match to the kinds the symbol extractor now emits:
- Rust: `macro_definition`, `union_item`, `associated_type` (names via the `first_named_child_text` fallback where not under a `name` field).
- TS: `abstract_class_declaration`, `internal_module`/`module` (incl. the `expression_statement`-wrapped top-level namespace), `lexical_declaration` (first declarator's name), plus an `export_statement`/`ambient_declaration`/`expression_statement` unwrap. Existing arms (incl. `impl_item`) untouched.

**Facet 2 (top-level-only / no recursion) reclassified as a documented limitation, NOT a regression — not fixed here.** Docstrings on impl methods / class members / nested decls still don't associate; all six docstring extractors walk only the root's direct children by long-standing design. Wiring recursion across all six is a separate enhancement, deferred unless `symbols(include_docs=true)` on nested symbols becomes a priority.

clippy clean; full parser suite green (27 tests).
# Tests added

- `ast::parser::tests::docstrings_associate_new_node_kinds` — doc comments on Rust `macro_rules!`/`union` and TS arrow-const/`abstract class`/`namespace` associate with the symbol name (red pre-fix: `symbol_name=None`).
# Workarounds
- Read the doc comment directly with `symbols(name=..., include_body=true)` (the body includes the
  leading doc comment) instead of relying on `include_docs` association.

# Resume

**Facet 1 fixed 2026-06-04 on `experiments`** (see ## Fix). Facet 2 (docstring recursion into bodies) is a deferred enhancement / documented limitation, not an open bug. Not yet on master — ship via Standard Ship Sequence, then `git mv` to `docs/issues/archive/` citing the **master-side** SHA.
# References
- `src/ast/parser.rs` `extract_docstrings_from_source` + `extract_*_docstrings` family.
- Siblings (symbol-side, fixed): `2026-06-04-rust-ast-drops-assoc-items-macros.md`,
  `2026-06-04-ts-extractor-drops-arrow-fn-consts.md`,
  `2026-06-04-ts-extractor-drops-namespace-abstract-class.md`.
