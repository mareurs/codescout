---
status: open
opened: 2026-06-04
severity: low
owner: marius
related: [2026-06-04-rust-ast-drops-assoc-items-macros, 2026-06-04-ts-extractor-drops-arrow-fn-consts, 2026-06-04-ts-extractor-drops-namespace-abstract-class]
tags: [ast, docstrings, tree-sitter, include_docs]
kind: bug
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

# Fix (not yet implemented)
Two independently-shippable steps:
1. **Cheap / consistency** — extend each docstring extractor's next-sibling match to the node
   kinds the symbol extractors now emit (mirror the 2026-06-04 symbol fixes). Closes facet 1.
2. **Larger** — make the docstring extractors recurse into `impl`/`trait`/`class`/`namespace`
   bodies (or associate by line-adjacency post-hoc), mirroring how the symbol extractors recurse.
   Closes facet 2. This is a structural change across all six language extractors.

# Tests added
None yet (finding only).

# Workarounds
- Read the doc comment directly with `symbols(name=..., include_body=true)` (the body includes the
  leading doc comment) instead of relying on `include_docs` association.

# Resume
Open finding from the 2026-06-04 extractor-coverage audit. Decide whether facet 1 (cheap) is worth
shipping for consistency with the symbol fixes, and whether facet 2 (recursion) is worth the
cross-language refactor given the low severity. If fixing, add a docstring-coverage regression test
mirroring `rust_assoc_items_and_macros_are_extracted` / `ts_arrow_function_consts_are_extracted`.

# References
- `src/ast/parser.rs` `extract_docstrings_from_source` + `extract_*_docstrings` family.
- Siblings (symbol-side, fixed): `2026-06-04-rust-ast-drops-assoc-items-macros.md`,
  `2026-06-04-ts-extractor-drops-arrow-fn-consts.md`,
  `2026-06-04-ts-extractor-drops-namespace-abstract-class.md`.
