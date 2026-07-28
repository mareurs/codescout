---
status: fixed
opened: 2026-06-04
closed: 2026-06-04
severity: high
owner: marius
related: [2026-06-04-kotlin-ast-drops-nested-classes]
tags: [edit_code, ast, typescript, javascript, tree-sitter, namespace, abstract-class]
kind: bug
---

# BUG: TS/JS AST extractor drops `namespace` contents and `abstract class` entirely → zero symbols, `edit_code` fails

## Summary
`extract_ts_symbols` had no match arm for `internal_module` (`namespace Foo {}` / `module Foo {}`)
or for `abstract_class_declaration` (`abstract class X`). tree-sitter-typescript emits these as
node kinds **distinct** from the handled `class_declaration`, and wraps a top-level `namespace` in
an `expression_statement`. So any symbol inside a namespace, and any abstract class plus all its
members, **never entered codescout's tree-sitter symbol tree**. A TS file whose top-level
declaration is an `abstract class` (or a `namespace`) extracted to an **empty** symbol list.

Found by audit while exploring siblings of the Kotlin/Java nested-type bug
(`2026-06-04-kotlin-ast-drops-nested-classes`) — same `edit_code` "AST parse failed" symptom
family, but a different mechanism (missing match arm, not wrong-node recursion) and a **wider
blast radius**: it drops **top-level** symbols, so it also blinds `symbols(path)` overview and
`symbol_at`, not just `edit_code`.

## Symptom (Effect)
For a `.ts`/`.tsx`/`.js` file whose declarations live under a `namespace`/`module`, or that
declares an `abstract class`:
- `symbols(path=…)` → those symbols are absent (file may show as having **no symbols** at all).
- `edit_code(symbol=…, action="insert"/"replace")` → `cannot determine end of '<sym>' for … —
  AST parse failed` (the BUG-051 anti-corruption guard fires on the empty AST), even though the
  file is valid TS and the LSP resolves the symbol.

## Reproduction
Minimal (verified 2026-06-04 via `extract_symbols_from_source`, unit-test harness):
```typescript
namespace Outer {
    export class Inner {
        method(): void {}
    }
}

abstract class Base {
    abstract foo(): void;
    bar(): number { return 1; }
}
```
Pre-fix: `extract_symbols_from_source(src, Some("typescript"), …)` → `[]` (zero symbols).
Post-fix: `Outer`, `Outer/Inner`, `Outer/Inner/method`, `Base`, `Base/foo`, `Base/bar`.

## Environment
codescout (current `experiments` HEAD), tree-sitter-typescript grammar. Reproduced deterministically
in the unit-test harness; not (yet) observed against a live workspace — surfaced by the parser
audit, not a usage.db row.

## Root cause
`src/ast/parser.rs` — `extract_ts_symbols` match had arms only for `function_declaration`,
`class_declaration`, `interface_declaration`, `enum_declaration`, `type_alias_declaration`, and an
`export_statement` unwrap. The tree-sitter parse tree (confirmed by a node-kind dump probe) is:
```
program
  expression_statement
    internal_module            ← `namespace Outer {}`  — UNHANDLED (no arm)
      identifier "Outer"       ← name is an `identifier` child, not under field "name"
      statement_block          ← body
        export_statement
          class_declaration "Inner"   ← reachable ONLY if we recurse into the namespace
  abstract_class_declaration   ← `abstract class Base` — UNHANDLED (≠ class_declaration)
    type_identifier "Base"
    class_body
      abstract_method_signature "foo"  ← UNHANDLED by extract_ts_class_members (≠ method_definition)
      method_definition "bar"
```
Three coordinated gaps: (1) `internal_module` had no arm → namespace contents dropped; the
namespace also nests under `expression_statement`, which had no unwrap arm; (2)
`abstract_class_declaration` was not in the class arm → abstract class + members dropped;
(3) `extract_ts_class_members` matched only `method_definition`/`public_field_definition`, so
`abstract_method_signature` members were dropped.

Downstream chain identical to the Kotlin bug: LSP (tsserver) resolves the symbol →
`ast_confirmed_end_line` → `extract_symbols_from_source` → `extract_ts_symbols` (missing the sym)
→ `find_ast_end_line_in` finds 0 candidates → `None` → `do_insert` refuses.

## Evidence
- Node-kind dump probe (temporary test) printed the tree above — proves `internal_module`,
  `abstract_class_declaration`, `abstract_method_signature` are the real grammar node kinds and
  that none matched the existing arms.
- Pre-fix probe asserting the symbols exist → `names=[] paths=[]` (the whole file extracted empty).
- Code read: `extract_ts_symbols` arms vs. the dumped node kinds.

## Hypotheses tried
1. **Same wrong-node recursion as Kotlin/Java** (passing declaration vs. body to the recursive fn).
   **Test:** read the TS member-extractor — it is correctly handed `class_body`. **Verdict:**
   REJECTED — TS's bug is a *missing arm*, not wrong-node recursion.
2. **Only `namespace` is affected.** **Test:** the abstract class `Base`/`bar` were ALSO absent
   from the empty result. **Verdict:** REJECTED — `abstract class` is independently unhandled.
3. **`abstract class` / `namespace` are ordinary `class_declaration` / `module` with a modifier.**
   **Test:** node-kind dump. **Verdict:** REJECTED — they are distinct node kinds
   (`abstract_class_declaration`, `internal_module`).

## Fix
**Implemented 2026-06-04 on `experiments`** (`src/ast/parser.rs`):
- `extract_ts_symbols`: merged `abstract_class_declaration` into the `class_declaration` arm
  (identical `class_body` extraction); added an `internal_module` | `module` arm that emits a
  `Module` symbol and recurses into its `statement_block`; extended the unwrap arm to
  `expression_statement` and `ambient_declaration` (so `namespace`/`declare` wrappers are
  transparently descended — safe, since only declaration arms match). The namespace name falls
  back to the `identifier`/quoted-`string` child when no `name` field is present.
- `extract_ts_class_members`: added `abstract_method_signature` to the method arm so abstract
  method signatures are extracted alongside concrete `method_definition`s.

**Verification (2026-06-04):** the regression test below; `cargo clippy --all-targets -- -D warnings`
clean; full lib suite green (2608 pass, 7 ignored, 0 fail). Live confirmation needs a `/mcp` restart
to load the rebuilt binary.

## Tests added
- `ast::parser::tests::ts_namespace_and_abstract_class_are_extracted` — asserts (via recursive tree
  walk) that namespace contents (`Outer`, `Outer/Inner`, `Outer/Inner/method`) and the abstract
  class with **both** member kinds (`Base`, `Base/foo` abstract signature, `Base/bar` concrete)
  are extracted. Red pre-fix: the whole file extracted to `[]`.

## Workarounds
- Use `edit_file` with explicit surrounding context (when not blocked by `debug_enforce_symbol_tools`).

## Resume
**Fixed 2026-06-04 on `experiments`** (see ## Fix). Not yet on master. Ships alongside the Kotlin/Java
nested-type fix (`c4bfa008`) via Standard Ship Sequence + frog audit; on landing `git mv` to
`docs/issues/archive/` and cite the **master-side** SHA. Possible follow-ups (not done):
`declare module "x" {}` ambient-module name handling beyond the identifier/string fallback; verify
`export default abstract class` and deeply-nested namespaces; the shared `do_insert` hint-softening
already noted in the Kotlin bug file.

## References
- `src/ast/parser.rs` `extract_ts_symbols` + `extract_ts_class_members`.
- `src/tools/symbol/edit_code.rs::do_insert`; `src/symbol/query.rs::find_ast_end_line_in`.
- Sibling: `docs/issues/2026-06-04-kotlin-ast-drops-nested-classes.md` (same AST-vs-LSP family).
