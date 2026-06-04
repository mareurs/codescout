---
status: fixed
opened: 2026-06-04
closed: 2026-06-04
severity: high
owner: marius
related: [2026-05-29-edit-code-kotlin-stale-lsp-range]
tags: [edit_code, ast, kotlin, tree-sitter, nested-class]
kind: bug
---

# BUG: Kotlin AST extractor drops nested classes/objects → `edit_code` insert/replace fails for any symbol inside a nested class

## Summary
`extract_kotlin_class_members` emits nothing for nested `class_declaration` / `object_declaration`
members, so codescout's tree-sitter symbol tree is missing every nested class and its methods.
Any `edit_code` op that relies on AST-confirmed symbol boundaries (notably `action="insert"
position="after"`, and `replace`) then fails for those symbols with a **misleading** "AST parse
failed" error, even though the file is valid Kotlin. Surfaced in `backend-kotlin` adding a test
inside a `@Nested inner class`.

## Symptom (Effect)
```
{ "ok": false,
  "error": "cannot determine end of 'ConvertToPinnedStage1LessonsTests' for insert-after — AST parse failed",
  "hint": "The file likely has syntax errors that broke tree-sitter's parse, or the symbol has
           duplicate-name siblings without a clear name_path. Fix the syntax errors first, or use
           edit_file with explicit context." }
```
The hint is wrong on both counts: the file parses and compiles, and the names are unique.

## Reproduction
Minimal (verified 2026-06-04 on the live release binary):
```kotlin
// /tmp/cs_repro_plainnest.kt
class Outer {
    class Nested {
        fun foo() { val y = 1 }
    }
}
```
1. `edit_code(symbol="Nested", path=…, action="insert", position="after", body="…")`
   → `cannot determine end of 'Nested' for insert-after — AST parse failed`.
2. `edit_code(symbol="Outer", …)` (top-level) → **succeeds**.
3. A *flat* `@Test`/`fun` directly in a top-level class → **succeeds**.
Trigger is **nested-class membership**, not backticks, `=` in names, or the `inner`/`@Nested`
keywords (all ruled out by variants). Real-world repro:
`…/backend-kotlin/.worktrees/weekly-pattern/ktor-server/src/test/kotlin/edu/planner/solver/services/stage1/PinnedLessonManagerImplTest.kt`
(`@Nested inner class ConvertToPinnedStage1LessonsTests` + its `@Test` methods).

## Environment
codescout (release binary, current `experiments` HEAD), tree-sitter-kotlin grammar. kotlin-lsp
262.4739.0 for the workspace's LSP side. Observed via usage.db rows 3893/3894 (errors) vs
3889/3891 (flat backtick `@Test` inserts that succeeded) in the worktree's `.codescout/usage.db`.

## Root cause
`src/ast/parser.rs` — `extract_kotlin_class_members` (~L1117), nested-member arm:
```rust
"class_declaration" | "object_declaration" => {
    let inner = extract_kotlin_symbols(child, source, file, prefix); // WRONG
    members.extend(inner);
}
```
`extract_kotlin_symbols(node)` iterates `node.children()` matching declaration node kinds — it
assumes `node` is a **container** (file root, or implicitly a body). But `child` here IS the
nested `class_declaration`; its direct children are `modifiers` / `"class"` / type-identifier /
`class_body` — none match, so it returns `[]`. The nested class and everything under it vanish
from the symbol tree.

Downstream chain (`src/tools/symbol/edit_code.rs::do_insert`):
`fetch_validated_symbol` resolves `sym` from the **LSP** (`document_symbols`, which DOES nest) →
`editing_end_line_strict(&sym)` → `ast_confirmed_end_line` → `extract_symbols_from_source` →
`extract_kotlin_symbols` (missing the nested sym) → `find_ast_end_line_in` finds 0 candidates →
`None` → `do_insert` refuses (the BUG-051 anti-corruption guard, working as designed; the bad
input is the empty AST). The misleading "AST parse failed" text is from that guard.

The top-level `class_declaration` arm in `extract_kotlin_symbols` extracts correctly (builds the
SymbolInfo, recurses `class_body` via `extract_kotlin_class_members`); the sibling
`companion_object` arm in `extract_kotlin_class_members` also recurses correctly. Only the nested
`class_declaration`/`object_declaration` arm calls the wrong function.

## Evidence
- `symbols(name="foo", path=/tmp/cs_repro_plainnest.kt)` → `Outer/Nested/foo` (3-5) — the LSP
  path nests fine, proving the file is valid and the symbol exists.
- `edit_code(symbol="Nested"/"Inner"/"sets isStage=false…")` → "AST parse failed"; `edit_code`
  on top-level `Outer` and on a flat method → ok. Isolated across 4 repro variants.
- Code read: `extract_kotlin_class_members` nested arm vs the correct top-level / companion arms.

## Hypotheses tried
1. **`=` in backtick test name** breaks the parse. **Test:** no-`=` nested variant. **Verdict:** REJECTED — also fails; `=` was assignment-ubiquitous, a red herring.
2. **`inner`/`@Nested` keyword/annotation** specific. **Test:** plain `class Nested {}` (no `inner`, no annotation). **Verdict:** REJECTED — also fails.
3. **Real syntax error / duplicate-name siblings** (the error's own hint). **Test:** file compiles; names unique; LSP resolves them. **Verdict:** REJECTED.
4. **AST extractor omits nested classes.** **Test:** read `extract_kotlin_class_members` — nested arm calls `extract_kotlin_symbols(child)` on the class_declaration node, which returns `[]`. **Verdict:** CONFIRMED.

## Fix

**Implemented 2026-06-04 on `experiments`** (`src/ast/parser.rs`). Both the Kotlin bug and the confirmed Java twin:

- **Kotlin** — added `extract_kotlin_type_decl(node, …)`; routed both the top-level arms of `extract_kotlin_symbols` and the nested arm of `extract_kotlin_class_members` through it. The nested arm previously called `extract_kotlin_symbols(child)` on the *declaration* node (a container-expecting fn) → it matched no declaration children and **dropped** the nested type entirely.
- **Java (twin, confirmed during audit)** — added `extract_java_type_decl(node, …)`; the nested arm of `extract_java_class_members` previously called `extract_java_symbols(body)` on the **parent** body, re-scanning it once per nested type → **duplicated** every nested type (N nested → each ×N). The duplicates then tripped `find_ast_end_line_in`'s ambiguity guard (`matches.len() > 1 → None`), same `edit_code` symptom via a different mechanism. Now extracts the `child` node exactly once.

The `do_insert` "AST parse failed" hint is unchanged — the BUG-051 refusal guard is correct; the bad input was the empty/duplicated AST. Softening that hint (when the LSP found a symbol the AST didn't) is a noted follow-up.

**Verification (2026-06-04):** the 3 tests below; `cargo clippy --all-targets -- -D warnings` clean; full lib suite green (2637 pass, 0 fail); release rebuilt. Live `edit_code`-on-nested confirmation needs a `/mcp` restart to load the new binary.
## Tests added

- `ast::parser::tests::kotlin_nested_classes_and_members_are_extracted` — nested `@Nested inner class` + its backtick-`=` `@Test` method + a nested `object` all extracted with correct name_paths (`Outer/Inner`, `Outer/Inner/\`…\``).
- `ast::parser::tests::java_nested_types_are_extracted_without_duplication` — two nested classes each appear **exactly once** (red pre-fix: each ×2), correct nested name_path + member.
- `symbol::query::backtick_match_tests::find_ast_end_line_in_resolves_nested_kotlin_symbols` — symptom-level guard: a nested class and a method inside it resolve to their AST end lines (previously `None`).
## Workarounds
- Use `edit_file` with explicit surrounding context (when not blocked by `debug_enforce_symbol_tools`).
- Anchor the insert on a **top-level** symbol or a method **directly** in the top-level class.

## Resume

**Fixed 2026-06-04 on `experiments`** (Kotlin + Java twin; see ## Fix). Not yet on master. Next: ship via Standard Ship Sequence + frog audit; on landing `git mv` to `docs/issues/archive/` and cite the **master-side** SHA. After a `/mcp` restart, optionally confirm live `edit_code` insert on a nested Kotlin/Java class now succeeds (unit + symptom tests already cover it deterministically). Separate follow-up: soften the `do_insert` 'AST parse failed' hint when the LSP resolved a symbol the AST didn't — point at extractor gaps, not 'syntax errors'.
## References
- `src/ast/parser.rs` `extract_kotlin_symbols` (L874) + `extract_kotlin_class_members` (L1026, nested arm ~L1117).
- `src/tools/symbol/edit_code.rs::do_insert` (L710, refusal at L743); `src/symbol/edit.rs::ast_confirmed_end_line` (L159); `src/symbol/query.rs::find_ast_end_line_in` (L364) + `collect_ast_candidates` (L409).
- Related: `docs/issues/2026-05-29-edit-code-kotlin-stale-lsp-range.md` (same edit_code/AST-vs-LSP family).
- Surfaced in `backend-kotlin` worktree session; usage.db rows 3893/3894.
