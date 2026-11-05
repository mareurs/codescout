---
id: aa7901a4f6773d8d
kind: plan
status: draft
title: Declaration header from tree-sitter Implementation Plan
---

# Declaration header from tree-sitter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `declaration_changed` reads a declaration header from the tree-sitter parse of the replaced text, so it answers `Some(_)` only when it parsed one declaration cleanly, and the text scanner (`declaration_header`, `header_end` and their helper types in `src/symbol/edit.rs`) is deleted.

**Architecture:** A new module `src/symbol/declaration.rs` parses the old and the new text with the grammar for the file's language, finds the single declaration that owns a body, and compares the leaf tokens (comments excluded) from the start of the declaration to the start of its body. A body-only edit cannot change those tokens, so `Some(true)` on a body-only edit is impossible by construction; any text that does not parse cleanly, holds no single declaration with a body, or is in a language without a grammar gives `None`.

**Tech Stack:** Rust, `tree-sitter` with the grammars already in `Cargo.toml` (`tree_sitter_rust`, `_python`, `_typescript`, `_java`, `_kotlin_ng`), `crate::ast::get_ts_language`.

**Spec:** `docs/superpowers/specs/2026-11-04-harness-delivered-hints-design.md` (id `95abaa31c77daa39`), `## Rollout`, hardening wave and the decision owed before slice 2. Prior plan: `docs/superpowers/plans/2026-11-04-harness-delivered-hints-server-slice.md`. The ledger of that plan holds the rulings of the five scanner rounds.

## Global Constraints

- `declaration_changed(old, new, language_id)` returns `Some(true)` or `Some(false)` only when BOTH texts parse with no `ERROR` or `MISSING` node, each holds exactly one top-level declaration (comments, Rust `attribute_item`/`inner_attribute_item` and blank text ignored) that is an owner listed in Task 1, and that owner has a body. Everything else is `None`.
- `language_id` is the AST id that `check_callers` already receives (`ast_language_for_lsp_id`); `crate::ast::get_ts_language` resolves it. A language without a grammar gives `None`.
- No new dependency and no change to `Cargo.toml`.
- `check_callers`, `count_calls_named`, `classifies_calls` and the `CallersCheck` variants keep their behavior and signatures, except that `check_callers` passes `language_id` to `declaration_changed`.
- A false `Some(true)` on a body-only edit is the worst outcome. A `Some(false)` on a real header change, or a `None` where a header could be read, are acceptable losses and are listed as known limits.
- Format with `rustfmt --edition 2021` (the worktree refuses `scripts/fmt-mine.sh`); `cargo clippy -D warnings`; run cargo through `scripts/with-slot.sh`.

## Review Focus

- A replaced text that does not parse alone: an indented Python method, a Java or TS method with no enclosing class. Expectation: the module retries on a dedented text and, for `java` and the TS family, inside a wrapper class; the result is still `Some` only when the retry parses cleanly to one owner.
- A replaced text that is not one declaration: two functions, a function plus a statement, a bare statement. Expectation: `None`.
- An edit that leaves the text unparsable (a missing brace). Expectation: `None`, never `Some(true)`.
- A header change hidden among body changes, and a body change hidden in a header-looking text (the cases the five scanner reviews found: `x ? {a:1}`, nested arrows, multi-line decorators, a quote leak, `const x =c ? {`, a Python one-liner with `=`, a Python `class`). Expectation: `Some(false)` for each body-only edit, `Some(true)` for each real header change, never `Some(true)` on a body-only edit.
- A Kotlin expression body (`fun f(x: Int) = x + 1`) and a Kotlin class (`class A(val x: Int) : B()`): the body is a child node, not a field. A Rust trait method without a body gives `None`.

---

### Task 1: the header reader

**Files:**
- Create: `src/symbol/declaration.rs`
- Modify: `src/symbol/mod.rs` (declare `pub mod declaration;`)
- Test: tests inside `src/symbol/declaration.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `crate::ast::get_ts_language(&str) -> Option<tree_sitter::Language>`.
- Produces:
  - `pub fn declaration_changed(old: &str, new: &str, language_id: &str) -> Option<bool>` — `Some(true)` headers differ, `Some(false)` headers equal, `None` cannot tell.
  - `fn declaration_tokens(text: &str, language_id: &str) -> Option<Vec<String>>` — the comment-free leaf tokens from the start of the single top-level declaration to the start of its body.

- [ ] **Step 1: Write the failing tests** (names and assertions; all use `declaration_changed(old, new, lang)`):
  - Rust, `"rust"`: `fn a() -> u8 {\n    1\n}` -> body `3`: `Some(false)`; add a parameter: `Some(true)`; `fn a() {}` -> `pub fn a() {}`: `Some(true)`; `fn a(x: u8) -> u8 {` vs `fn  a(x: u8)\n    -> u8 {` (after closing braces): `Some(false)`; `/// doc\n#[inline]\nfn a() {}` vs `fn a() {}`: `Some(false)`; CRLF vs LF: `Some(false)`; `fn a() { // one\n 1\n}` vs `// two`: `Some(false)`; a signature-only `fn a(x: u8);` and a Rust `static R: Lazy<u8> = Lazy::new(|| {\n    1\n});` body edit: `None`; two functions in the text: `None`; `fn a() -> impl Iterator<Item = u8> {` header unchanged with body edit: `Some(false)`; `impl S {\n    fn a(&self) { 1 }\n}` with the method body changed: `Some(false)` (the single declaration is the `impl_item`, its body holds the edit, the header tokens are equal).
  - Python, `"python"`: `def f(a):\n    return 1` body edit `Some(false)`, `def f(a, b):` change `Some(true)`; trailing comments on the header line, URL and `#` inside string defaults (`def f(url="http://x", a=2, h="#x"):`), `def f(a): x = 1` -> `y = 1`: `Some(false)`; `def reset(self): self.cache = {}` -> `self.items = {}`: `Some(false)`; multi-line header `def f(\n    a,\n    b,\n):`; `class A:\n    def __init__(self):\n        self.cache = {}` -> `self.items`: `Some(false)`, `class A` -> `class B` and `class A(B)` -> `class A(C)`: `Some(true)`; decorators (`@pytest.mark.parametrize(\n    "a", [1, 2],\n)\ndef test_x(a):\n    x()`, body edit): `Some(false)`; decorator added or changed above an unchanged `def`: `Some(false)` (a decorator is outside `function_definition`, state it); an indented method (`    def f(self):\n        return 1`): dedent then `Some(false)`; the single-quote leak case `def f(a='('):\n    for x in g(')'):\n        pass` with `x` -> `y`: `Some(false)`.
  - TypeScript, `"typescript"` and `"tsx"` and `"javascript"` and `"jsx"`: `function f(a: number): number {` body edit `Some(false)`, parameter added `Some(true)`, `async function f(a: number): Promise<void> {` return type changed `Some(true)`; `const f = (x) => {\n  a();\n}` body edit `Some(false)`, `(x, y) =>` `Some(true)`; `const f = (x: number) => x + 1;` -> `x + 2`: `Some(false)`, `(x, y)`: `Some(true)`; `const save = debounce((x) => {\n  a();\n}, 100);` body edit: `None` (the declared value is a call, not a function); `export const M = React.memo((p) => {...})`: `None`; `const f = (x) => x ? {a: 1} : {b: 2};` condition `x` -> `!x`: `Some(false)`; the nested arrow ternary `const f = (x) => x ? (y) => {\n  a();\n} : null;` with `x` -> `!x`: `Some(false)` (the owner is the outer arrow and its body is the ternary; say so in the test comment); `class A extends B {\n  m() { return {} }\n}` -> change inside `m`: `Some(false)`, `extends C`: `Some(true)`; a method alone `foo(x) {\n  return 1;\n}` (wrapper retry): `Some(false)`, `foo(x, y) {`: `Some(true)`; `@Component({\n  selector: "x",\n}) export class A {\n  m() {}\n}`: `Some(false)` on a body edit of `m` and `Some(true)` when the decorator argument (`selector: "x"` -> `"y"`) changes (a decorator inside the class node is part of the header tokens; assert it and say so in the test comment); `const x =c ? {a: 1} : {b: 2};` with a spaceless `=` and any edit: `None` (the declared value is not a function, so there is no body; this is the shape the scanner read wrongly).
  - Java, `"java"`: a method alone `public int f(int a) {\n  int[] x = {1};\n  return a;\n}` (wrapper retry): body edit `Some(false)`, `f(int a, int b)`: `Some(true)`; `@Override public String toString() {` body edit `Some(false)`, `@Override` removed: `Some(true)`; `@SuppressWarnings({\n  "a"}) public int f() {\n  log();\n  int[] a = {1};\n}` body edit `Some(false)`; a class `public class A implements B {\n  void m() {}\n}`: `implements C`: `Some(true)`; an interface method without a body `int f();`: `None`.
  - Kotlin, `"kotlin"`: `fun f(x: Int): Int {\n    log()\n    return run {\n        1\n    }\n}` body edit `Some(false)`, `fun f(x: Int, y: Int)`: `Some(true)`; `fun f(x: Int) = x + 1` -> `x + 2`: `Some(false)`, `(x: Int, y: Int)`: `Some(true)`; `fun f() =\n    when (x) {\n        1 -> 2\n        else -> 3\n    }` discriminant `x` -> `x + 1`: `Some(false)`; `@JvmStatic fun f(): Int {...}` body edit `Some(false)`; `class A<T : Any>(x: T) {` -> `(x: T, z: T)`: `Some(true)`; `class A : B {` -> `class A : C {`: `Some(true)`; `data class P(val x: Int)` (no body): `None`.
  - Gaps: `""`, `"// only a comment"`, `"fn a() {}"` vs `""`: `None`; text that does not parse (`fn a( {`): `None`; two functions: `None`; a language without a grammar (`"ruby"`, `"scala"`, `"c"`, `"unknown"`): `None` for any text; language-id aliases: the same `fn` pair gives the same result under `"rust"` and `"RUST"`.
- [ ] **Step 2: Run to verify they fail** — `scripts/with-slot.sh cargo test --lib -- declaration::` — Expected: compile error (module missing).
- [ ] **Step 3: Implement.** Parse with `tree_sitter::Parser` and `get_ts_language(language_id)?`. Procedure for one text, in order: (a) for `python`, strip the common leading whitespace of the non-blank lines; (b) parse; if the tree is clean (`!root.has_error()`) and yields exactly one owner (below), use it; (c) otherwise, for `java`, `typescript`, `tsx`, `javascript`, `jsx` only, parse `class W {\n` + text + `\n}` and take the single owner that is a direct child of the wrapper class body; else `None`. Top-level children: skip nodes whose kind is `comment`, `line_comment`, `block_comment`, `attribute_item`, `inner_attribute_item`; anything left must be exactly one node. Owner resolution per language: Rust `function_item`, `impl_item`, `trait_item`, `struct_item`, `enum_item`, `union_item`, `mod_item`, `foreign_mod_item` with a `body` field; Python `function_definition` or `class_definition` with a `body` field, reached directly or through `decorated_definition.definition`; TS family `function_declaration`, `generator_function_declaration`, `class_declaration`, `abstract_class_declaration`, `interface_declaration`, `enum_declaration`, `method_definition` with a `body` field, reached directly or through `export_statement.declaration`, and `lexical_declaration`/`variable_declaration` holding exactly one `variable_declarator` whose `value` is an `arrow_function`, `function_expression` or `generator_function` (the header then starts at the start of the declaration and ends at that function's `body`); Java `method_declaration`, `constructor_declaration`, `class_declaration`, `interface_declaration`, `enum_declaration`, `record_declaration`, `annotation_type_declaration` with a `body` field; Kotlin `function_declaration` (body = child of kind `function_body`) and `class_declaration`/`object_declaration` (body = child of kind `class_body` or `enum_class_body`). A node of any other kind, or an owner with no body, gives `None`. `declaration_tokens` collects the leaves (`child_count() == 0`, or a `string_literal`-like node with no named children) whose byte range lies inside `[declaration.start_byte, body.start_byte)` and whose kind is not a comment, as their source text, in order. Walk with an iterative cursor, as `count_calls_named` does in `src/tools/symbol/call_edges/resolver.rs`, so recursion depth never follows nesting. `declaration_changed` is `Some(tokens(old)? != tokens(new)?)`.
- [ ] **Step 4: Run to verify they pass** — same command — Expected: all pass. Run `scripts/with-slot.sh cargo clippy --lib --tests -- -D warnings`.
- [ ] **Step 5: Commit** — `git add src/symbol/declaration.rs src/symbol/mod.rs`; message `feat(edit_code): declaration_changed reads the header from the tree-sitter parse`.

### Task 2: wire it in and delete the scanner

**Files:**
- Modify: `src/tools/symbol/callers_check.rs` (the call in `check_callers`), `src/symbol/edit.rs` (delete), `src/tools/symbol/tests.rs`, `CHANGELOG.md`

**Interfaces:**
- Consumes: `crate::symbol::declaration::declaration_changed(old: &str, new: &str, language_id: &str) -> Option<bool>` from Task 1.
- Produces: nothing new. `check_callers` keeps its signature.

- [ ] **Step 1: Update the layer tests first.** In `src/tools/symbol/tests.rs`, every test that asserted `advice.verified == false` or "no advice" because the old scanner answered `None` must be re-derived from the new reader and changed deliberately (for example a TS `debounce` body edit still gives `None`, so `verified: false` stays; a Python one-liner with `=` now gives `Some(false)`, so no advice). In your report list each test whose expectation changed, the old and new value and why. Add one layer test per language family (Rust, Python, TypeScript, Kotlin) with a changed signature and an outside by-name caller, asserting `advice.verified == true`, and one with a body-only edit asserting no `advice`.
- [ ] **Step 2: Run to verify** — `scripts/with-slot.sh cargo test --lib -- edit_code_replace callers_check` — Expected: failures where the signature differs.
- [ ] **Step 3: Implement.** Change the call in `check_callers` to `declaration_changed(old_text, new_text, language_id)`. Delete from `src/symbol/edit.rs`: `declaration_header`, `skip_annotation_line`, `annotation_tail_is_blank`, `depth_after`, `Lead`, `HeaderEnd`, `HeaderScan`, `Token`, `header_end`, the old `declaration_changed` and every `declaration_changed_*` test (the corpus now lives in `declaration.rs`). Keep `skip_lead_region` and anything else the rest of `edit.rs` uses; check each deleted name with `references` first. Reword the `CHANGELOG.md` `[Unreleased]` bullet about `edit_code replace` so it states the new contract: advice with `verified: true` only when both texts parse to one declaration whose header tokens differ and an outside call exists; `verified: false` when the header changed but the check cannot decide; no advice when the header is equal; a language without a grammar, or text that does not parse, gives the unverified advice of today. Update `## Rollout` of the design: the accepted limits of the scanner are gone, with the new commit SHA and the known limits of the reader.
- [ ] **Step 4: Run to verify they pass** — `scripts/with-slot.sh cargo test --lib -- declaration:: edit_code_replace callers_check`; `rustfmt --edition 2021 --check` on the changed files; `scripts/with-slot.sh cargo clippy --lib --tests -- -D warnings`.
- [ ] **Step 5: Commit** — `git add` the changed files by name; message `refactor(edit_code): delete the declaration header text scanner; check_callers passes the language to the tree-sitter reader`.
