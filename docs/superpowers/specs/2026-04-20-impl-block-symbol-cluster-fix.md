# Nested-Symbol & Container-Block Editing — Cluster Fix Design

**Date:** 2026-04-20
**Status:** Design
**Related bugs:** BUG-030, BUG-031, BUG-034, BUG-037, BUG-044 (open)
**Related tool:** `replace_symbol`, `remove_symbol`, `insert_code`

## Problem

Five bugs (and counting) trace to the same root: `replace_symbol` / `remove_symbol` / `insert_code` compute an editing range from LSP `DocumentSymbol.range` + a walk-back heuristic in `editing_start_line`, but the range is trusted in isolation from the symbol's structural parent. When the symbol is nested inside a container (`impl`, `mod`, `class`, `object`, `namespace`, etc.), drift in either boundary corrupts sibling symbols or the container header.

| Bug     | Failure mode                                                                                      | Status         |
|---------|---------------------------------------------------------------------------------------------------|----------------|
| BUG-030 | `replace_symbol("tests")` on `mod tests` eats preceding `write_message` function body             | Mitigated      |
| BUG-031 | `replace_symbol` on standalone fn leaves old doc-comment+signature, appends new → duplicate       | Fixed (walk-back) |
| BUG-034 | Stale LSP data points child `range_start_line` at parent's attribute line → eats module header    | Fixed (parent-start clamp) |
| BUG-037 | `replace_symbol` on `impl Trait for Type` drops outer `#[async_trait]` attribute                  | Fixed (attr-aware walk-back) |
| BUG-044 | `replace_symbol("impl LeafOp/parse")` replaces entire outer `impl LeafOp` block, dropping `sql`   | Open           |

Each fix so far patched `editing_start_line`. No fix yet clamps `editing_end_line`. BUG-044 indicates the `end` side is now the weakest boundary.

Current guard (symbol.rs:2409):

```rust
if let Some(parent) = find_parent_symbol(&symbols, &sym.name_path) {
    let parent_body_start = parent.start_line as usize + 1;
    if start < parent_body_start {
        start = parent_body_start;
    }
}
// NOTE: no corresponding clamp on `end`
```

## Goals

1. **Asymmetric → symmetric parent clamp.** Clamp both `start` and `end` to parent body boundaries.
2. **Language-parametric behavior.** Walk-back heuristics in `editing_start_line` already special-case Rust `#[...]` and Kotlin `/** */`. Generalize to a small matrix covering every container-capable supported language.
3. **Reproducible fixtures.** One regression test file per (language × container-construct × failure-mode) cell. Kept minimal — a dozen lines each.
4. **Fail-loud instead of corrupt-silently.** When post-write AST count of sibling symbols drops, roll back and return `RecoverableError` (already implemented for dropped-symbol case; extend to dropped-sibling case).

## Non-goals

- Rewriting the LSP range contract. We accept that rust-analyzer et al. emit ranges that exclude preceding attributes.
- Fixing LSP staleness itself (that is BUG-041's domain; we assume stale data can still appear and clamp defensively).
- Adding a new symbol-path grammar. `name_path` stays `"Parent/child"`.

## Design

### 1. Symmetric parent clamp

In `replace_symbol::call` and `remove_symbol::call`, after computing `start` and `end`, clamp **both** to the parent's body range when `sym` is nested:

```rust
if let Some(parent) = find_parent_symbol(&symbols, &sym.name_path) {
    let parent_body_start = parent.start_line as usize + 1;
    let parent_body_end = parent.end_line as usize; // last line is the `}` line, inclusive
    start = start.max(parent_body_start);
    end = end.min(parent_body_end);
}
```

Rationale: every container in every supported language delimits its children with a header line and a closer (`{`/`}`, `:` + indent dedent, `end`). The closer line belongs to the parent, not to any child. `end` must not cross it.

Edge case: when the parent uses indentation-based scope (Python), `parent.end_line` from tree-sitter is the last statement of the class body. Clamp still holds — a method cannot validly extend past its class's last line.

### 2. Sibling-drop guard (post-write integrity)

Extend the existing pre/post AST count snapshot (symbol.rs:2428–2470) to cover **siblings**, not just the replaced symbol itself:

```rust
let pre_sibling_names: Vec<String> = if let Some(parent) = find_parent_symbol(&symbols, &sym.name_path) {
    parent.children.iter().map(|c| c.name_path.clone()).collect()
} else {
    Vec::new()
};
// ... perform write ...
let post_syms = crate::ast::extract_symbols(&full_path).unwrap_or_default();
for sibling in &pre_sibling_names {
    if sibling == &sym.name_path { continue; }
    if count_symbols_by_name_path(&post_syms, sibling) == 0 {
        // roll back, return RecoverableError
    }
}
```

This catches BUG-044's failure mode: if `replace_symbol("impl LeafOp/parse", …)` silently eats `impl LeafOp/sql`, the post-write AST scan discovers `sql` missing and rolls back.

Cost: one extra tree-sitter parse. Already parsed once for the pre-count — reuse the symbol tree.

### 3. Language-parametric walk-back table

Replace the current ad-hoc string matches in `editing_start_line` with a small table keyed by language. Each row declares:

- Lines that are **decorators** (treated as part of the symbol — walk back past them if they lie above).
- Lines that are **container attributes** (must never be absorbed into a child edit — stop the walk-back).

| Language   | Decorator prefixes (walk through)              | Container attribute prefixes (stop) |
|------------|-----------------------------------------------|-------------------------------------|
| rust       | `///`, `//!`, `#[`, `/**`, `/*`, `*/`           | `#[` on top-level impl/mod/trait `*` |
| python     | `@`                                            | —                                   |
| typescript | `@`, `/**`, `*/`, `//`                         | `@` on class (decorator above class) |
| javascript | `@`, `/**`, `*/`, `//`                         | `@` on class                        |
| tsx / jsx  | same as ts/js                                  | same as ts/js                       |
| java       | `@`, `/**`, `*/`, `//`                         | `@` on class                        |
| kotlin     | `@`, `/**`, `*/`, `//`                         | `@` on class/object                 |
| go         | `//`                                           | —                                   |
| c / cpp    | `//`, `/**`, `*/`, `/*`, `__attribute__`       | —                                   |
| csharp     | `///`, `[`, `/**`, `*/`, `//`                  | `[` on class                        |
| ruby       | `#`                                            | —                                   |
| bash       | `#`                                            | —                                   |

(*) Rust's "stop" rule is already implemented correctly in BUG-037's fix. We codify it.

Implementation: a `WalkbackPolicy` struct with two `&[&str]` slices, selected by `ast::detect_language(path)` in `editing_start_line`. Tests verify the policy; the core walk-back loop is language-agnostic.

### 4. `insert_code` parent clamp (BUG-029 / BUG-036 neighborhood)

`insert_code(position="after", symbol="Parent/child")` must land at `min(child.end_line + 1, parent.end_line)`, clamped like `replace_symbol`. `position="before"` at `max(child.start_line, parent.start_line + 1)`. Currently unguarded; the cluster's sibling bugs (BUG-029, BUG-036) suggest the same asymmetric-range problem.

## Test Matrix

One fixture per cell. Fixtures live under `tests/fixtures/symbol_cluster/<lang>/<case>.<ext>`. Each test:

1. Creates a tempdir, writes fixture.
2. Starts the relevant LSP (skip with eprintln! if missing — CI has them; local may not).
3. Calls the tool under test.
4. Asserts the post-write file matches a checked-in expected result, or asserts `RecoverableError` + no file change.

### Rust (rust-analyzer)

| Case ID | Container             | Target                | Mutation            | Asserts                           | Bug ref |
|---------|-----------------------|-----------------------|---------------------|-----------------------------------|---------|
| RS-01   | `impl Type`           | `Type/method_a`       | replace body        | sibling `method_b` intact         | BUG-044 |
| RS-02   | `impl Trait for Type` | outer                 | replace whole impl  | outer `#[async_trait]` preserved  | BUG-037 |
| RS-03   | `mod tests`           | module                | replace whole mod   | preceding fn body intact          | BUG-030 |
| RS-04   | `mod tests`           | first child fn        | replace             | `#[cfg(test)] mod tests {` header intact | BUG-034 |
| RS-05   | `impl Type` + doc     | `Type/m` w/ `///` doc | replace             | no duplicated doc + signature     | BUG-031 |
| RS-06   | adjacent inherent + trait impls on same Type | trait impl | replace      | inherent impl braces intact       | BUG-037 remaining limitation |
| RS-07   | `trait T { fn default(); }` | `T/default`      | replace             | siblings intact                   | new     |
| RS-08   | nested mod            | `outer/inner/f`       | replace             | `outer` and `inner` headers intact | new     |

### Python (pyright)

| Case ID | Container       | Target          | Mutation       | Asserts                                 | Bug ref |
|---------|-----------------|-----------------|----------------|-----------------------------------------|---------|
| PY-01   | `class Foo:`    | `Foo/bar`       | replace method | siblings intact, no decorator loss      | new     |
| PY-02   | `class Foo:` with `@staticmethod` | `Foo/bar` | replace | `@staticmethod` decorator kept | new     |
| PY-03   | nested class    | `Outer/Inner/m` | replace        | outer class intact                      | new     |
| PY-04   | `class Foo:` first method w/ class docstring above | replace | class docstring intact | new |

### TypeScript / JavaScript (typescript-language-server)

| Case ID | Container     | Target      | Mutation       | Asserts                              | Bug ref |
|---------|---------------|-------------|----------------|--------------------------------------|---------|
| TS-01   | `class C {`   | `C/m`       | replace method | siblings intact                      | new     |
| TS-02   | `class C {` with `@decorator` on class | `C/m` | replace | class decorator preserved | new |
| TS-03   | `namespace N {` | `N/f`     | replace        | namespace braces intact              | new     |
| TS-04   | `class C {` first method, JSDoc above class | replace | JSDoc intact | new |
| JS-01   | parity with TS-01 on `.js`                  | replace    | siblings intact                      | new     |
| TSX-01  | `export default class C` in `.tsx`          | replace method | JSX context untouched           | new     |

### Java (jdtls)

| Case ID | Container         | Target    | Mutation       | Asserts                            | Bug ref |
|---------|-------------------|-----------|----------------|------------------------------------|---------|
| JV-01   | `class C {`       | `C/m`     | replace method | siblings intact, `@Override` kept  | new     |
| JV-02   | inner class       | `Outer/Inner/m` | replace  | outer class body intact            | new     |
| JV-03   | `interface I { default ... }` | `I/m` | replace | siblings intact                   | new     |

### Kotlin (kotlin-lsp)

| Case ID | Container              | Target     | Mutation       | Asserts                                | Bug ref |
|---------|------------------------|------------|----------------|----------------------------------------|---------|
| KT-01   | `class C {`            | `C/m`      | replace        | siblings intact                        | new     |
| KT-02   | `companion object {`   | `C/Companion/m` | replace   | surrounding `companion object` braces intact | new |
| KT-03   | `object O {`           | `O/m`      | replace        | siblings intact                        | new     |
| KT-04   | `class C { /** doc */ fun m …}` | `C/m` | replace | unclosed `/**` regression check (BUG-027 clone) | BUG-027 |

### Go (gopls)

| Case ID | Container          | Target         | Mutation       | Asserts                         | Bug ref |
|---------|--------------------|----------------|----------------|---------------------------------|---------|
| GO-01   | two methods on same receiver (same file) | `Type/MethodA` (via workspace/symbol container) | replace | `MethodB` intact | new |
| GO-02   | `type T struct {...}` with two methods below | `T.MethodA` | replace | struct decl intact | new |

*Note:* Go methods are not syntactically nested inside the struct — they sit at file scope. The `Parent/child` clamp is a no-op here. Tests exist to prove the absence of regression rather than demonstrate a fix.

### C / C++ (clangd)

| Case ID | Container             | Target       | Mutation       | Asserts                       | Bug ref |
|---------|-----------------------|--------------|----------------|-------------------------------|---------|
| CP-01   | `class C { void m(); };` | `C/m` def | replace        | class braces intact           | new     |
| CP-02   | `namespace ns {` containing function | `ns/f` | replace | namespace braces intact  | new     |
| CP-03   | `class C` w/ template specialization | `C<T>/m` | replace | template params preserved | new     |

### C# (OmniSharp)

| Case ID | Container              | Target     | Mutation       | Asserts                               | Bug ref |
|---------|------------------------|------------|----------------|---------------------------------------|---------|
| CS-01   | `class C {`            | `C/M`      | replace        | siblings intact, `[Attribute]` kept   | new     |
| CS-02   | `namespace N { class C {} }` | `N/C/M` | replace  | namespace + class braces intact       | new     |
| CS-03   | partial class across files   | `C/M`    | replace        | other partial file untouched          | new     |

### Ruby (solargraph)

| Case ID | Container         | Target     | Mutation       | Asserts                     | Bug ref |
|---------|-------------------|------------|----------------|-----------------------------|---------|
| RB-01   | `class C ... end` | `C/m`      | replace        | `end` preserved             | new     |
| RB-02   | `module M ... end` containing class | `M/C/m` | replace | module `end` preserved | new |

### Non-LSP safety net

For languages without a running LSP on CI, add an AST-level test using tree-sitter output only (bypassing LSP). Validates the parent clamp against tree-sitter `end_line`:

- `parent_clamp_unit_tests` — feed fabricated `SymbolInfo` trees with known ranges, call `compute_editing_range(sym, parent)` helper (extracted from current `replace_symbol::call`), assert output range is clamped to parent body.

### Sibling-drop integration test

`tests/symbol_lsp.rs::replace_symbol_rolls_back_when_sibling_drops`:

1. Fixture with two sibling methods in an `impl`.
2. Monkey-patch LSP response so child A's `range.end` overshoots into child B (simulating rust-analyzer's bad range).
3. Call `replace_symbol` on child A with a well-formed new body.
4. Assert: file unchanged, error is `RecoverableError` containing "sibling dropped".

## Implementation Plan

| Task | Owner | Est. |
|------|-------|------|
| T1. Extract `compute_editing_range(sym, parent, lines) -> (start, end)` pure helper from `replace_symbol::call` and `remove_symbol::call`. | — | 0.5 d |
| T2. Add `end` clamp + unit tests covering every (lang, container) cell via fabricated `SymbolInfo` (no LSP). | — | 0.5 d |
| T3. Implement `WalkbackPolicy` table; replace string-literal logic in `editing_start_line`. | — | 1 d |
| T4. Sibling-drop post-write guard + rollback + unit tests. | — | 0.5 d |
| T5. Language-by-language integration tests (RS → TS → PY → JV → KT → GO → CP → CS → RB). Skip cleanly if LSP absent. | — | 2 d |
| T6. `insert_code` parent clamp (Goal §4). | — | 0.5 d |
| T7. Update `docs/TODO-tool-misbehaviors.md`: mark BUG-030, BUG-044, BUG-037 remaining limitation as Fixed with link to this spec. Bump `ONBOARDING_VERSION`. | — | 0.25 d |

## Risks

1. **rust-analyzer `end_line` overshoot on malformed source.** If the parent's own `end_line` is also wrong, clamping to it is no better. Mitigation: AST fallback (`editing_end_line` already calls `find_ast_end_line_in`) — use AST-derived parent end when available.
2. **Python indentation-based parents.** tree-sitter-python reports class `end_line` as the last body line. Methods' `end_line` is likewise the last body line. The clamp is still sound (child ≤ parent's last body line), but requires Python-specific assertions in tests because there is no closing brace to anchor on.
3. **C# partial classes.** A partial class has multiple `DocumentSymbol` entries, one per file. `find_parent_symbol` only looks in the current file's tree — correct, because cross-file partials are structurally separate parents. Document this explicitly.
4. **Go's file-scope receivers.** The clamp becomes a no-op (Go methods have no structural parent in DocumentSymbol). Tests exist to ensure we don't accidentally treat the receiver type as a parent.
5. **Test flakiness on slow LSPs.** Java (`jdtls`) and Kotlin (`kotlin-lsp`) cold-start times are 30–60s. Reuse `mcp_integration_harness` with shared LSP across cases; cap each language's suite at 5 minutes.
6. **Regression scope.** Changing `editing_start_line` touches a heavily mitigated function. T3 lands **after** T1 + T2 so the pure-helper has its own regression net before the heuristic is rewritten.

## Rollout

Single PR per language (T5 rows). T1–T4 + T6 land first behind no flag. Each language's integration suite lands independently. No runtime feature flag — the clamp is always on.

Rollback: revert in reverse order. Pure helper + unit tests are safe to keep even if integration tests are reverted.

## Follow-ups (out of scope)

- **AST-only edit mode.** A companion spec could offer `replace_symbol` variants that bypass LSP entirely for languages with good tree-sitter coverage (Rust, Python, TS, Go, Java, Kotlin). Eliminates LSP staleness class of bugs.
- **LSP range contract test.** Standing test that starts each LSP against fixed fixtures and snapshots `DocumentSymbol.range`. Detects upstream LSP regressions early.
- **Per-symbol lock.** Currently write lock is per-project (`docs/superpowers/specs/2026-04-17-cross-process-write-serialization-design.md`). A per-symbol scheme would allow concurrent non-overlapping edits.
