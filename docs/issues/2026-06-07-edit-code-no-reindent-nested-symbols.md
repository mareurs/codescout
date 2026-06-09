---
status: fixed
opened: 2026-06-07
closed: 2026-06-07
severity: high
owner: marius
related:
  - docs/issues/2026-06-05-edit-code-insert-after-last-python-method.md
tags:
  - edit_code
  - indentation
  - python
  - kotlin
kind: bug
---

# BUG: edit_code insert/replace splices bodies verbatim, breaking indentation of nested symbols

## Summary
`edit_code` (`insert` / `replace`) wrote the caller-supplied body into the file
**verbatim**, with no re-indentation. When the symbol is nested (a method inside
a class/object, a nested `def`) and the caller supplied the body dedented to
column 0 — the natural thing after eyeballing a `symbols(include_body=true)`
dump — the spliced code landed at the wrong column: a hard `IndentationError` in
Python, mis-aligned (lint-failing) code in brace languages like Kotlin.

## Symptom (Effect)
- **Python:** inserting/replacing a nested method with a column-0 body produced a
  file that no longer parses (`IndentationError: unexpected indent` / `expected
  an indented block`). `do_insert` had **no** post-write validation, so the
  corrupt file was written and notified silently.
- **Kotlin:** the file still compiles (braces define structure) but the inserted
  member sits at the wrong column — fails ktlint/detekt, reads as broken.
- Top-level symbols (column 0) were unaffected — verbatim == correct there —
  which is why the bug stayed hidden until editing *nested* symbols. The failure
  is conditional on nesting depth, not language alone.

## Reproduction
Base commit: `70c968c8` (branch `experiments`).
1. Python file with a class:
   ```python
   class Foo:
       def bar(self):
           return 1
   ```
2. `edit_code(action="insert", symbol="Foo/bar", position="after",
   body="def baz(self):\n    return 2")`  ← body at column 0
3. Before fix: `baz` lands at column 0, outside the class → `IndentationError`,
   written silently.

## Environment
codescout 0.15.0, MCP stdio, branch `experiments`. Observed against Kotlin
(`~/work/mirela/backend-kotlin`) and Python projects.

## Root cause
Both mutation methods in `src/tools/symbol/edit_code.rs` spliced the body with no
column adjustment:
- `do_insert` — `new_lines.extend(code_lines.iter().copied())` (was
  `edit_code.rs:794`). Body kept the caller's columns. **No** post-write AST
  validation, unlike `do_replace`.
- `do_replace` — `new_lines.extend(effective_body.lines())`. Same verbatim splice.

The target indentation was always knowable from the file: the leading whitespace
of the symbol's own first line (`leading_ws(lines[start])`) for replace, or the
sibling's first line for insert. `edit_file` already had a `reindent_block`
helper for its whitespace-normalized fallback, but it was private to
`src/tools/edit_file/mod.rs` and unused by `edit_code`.

## Evidence
`do_insert` body before the fix (no reindent, no guard):
```rust
let code_lines: Vec<&str> = code.lines().collect();
// ...
new_lines.extend(code_lines.iter().copied());
// ... write_lines(...) with no has_syntax_errors check
```
`do_replace` had AST guards (post-`extract_symbols` count + dropped-sibling
restore) but still wrote `effective_body` verbatim — so a mis-indented Python
replace would parse-fail, drop the symbol, and get *restored* (surfacing as
"edit didn't apply"), while a mis-indented insert *corrupted* the file.

## Hypotheses tried
1. **Hypothesis:** the editors reindent and the bug is in column detection.
   **Test:** read `do_insert`/`do_replace` end-to-end. **Verdict:** rejected —
   there was no reindent at all; bodies were spliced verbatim.
2. **Hypothesis:** a formatter-grade AST reindent is needed.
   **Test:** weighed against multi-line-string / continuation-line corruption
   risk. **Verdict:** rejected — base-shift preserves Python block consistency
   and is language-agnostic; full reindent is high blast radius for low gain.

## Fix
Strategy **A + D** (chosen by user): base-shift reindent + insert AST guard.
Uncommitted on `experiments` as of 2026-06-07 (cite master SHA after cherry-pick
per CLAUDE.md § "After cherry-pick"). Changes:
- **`src/util/text.rs`** — new shared helpers `leading_ws`, `min_indent`,
  `reindent_block` (moved out of `edit_file`), `reindent_to(block, target_base)`.
  `reindent_to` is a **no-op when the body is already based at the target**, so
  correctly-indented input (and lines inside multi-line strings) is untouched.
- **`src/tools/edit_file/mod.rs`** — drops its private `leading_ws`/`reindent_block`,
  imports them from `crate::util::text` (single source of truth).
- **`src/tools/symbol/edit_code.rs`**
  - `do_replace`: re-bases `effective_body` onto `leading_ws(lines[start])`
    after the bounds guard.
  - `do_insert`: re-bases the inserted `code` onto the sibling symbol's column
    (`leading_ws(lines[editing_start_line(sym)])`), **and** adds a pre-write AST
    guard — if the insert introduces a `has_syntax_errors` regression the file
    didn't have before, it returns a `RecoverableError` without writing (mirrors
    `do_replace`'s restore guard, closing the silent-corruption hole).

## Tests added
- `src/util/text.rs::tests` — `leading_ws_extracts_indent`,
  `min_indent_picks_least_indented_nonblank`,
  `reindent_to_shifts_dedented_body_to_target` (the exact reported scenario:
  column-0 method body re-based into a class at column 4, inner step preserved),
  `reindent_to_noop_when_already_based`, `reindent_to_dedents_when_target_shallower`,
  `reindent_to_preserves_blank_lines`.
- Existing `tools::edit_file::tests::reindent_*` still pass against the moved
  helper (glob-import resolution verified by a full `--no-run` compile).
- Full lib suite: 2646 passed, clippy clean.
- The `do_insert`/`do_replace` integration path is exercised by the existing
  LSP-gated `edit_code` tests (skip when no LSP installed); the pure transform is
  covered deterministically by the `util::text` tests above.

## Workarounds
Supply the body already indented to the symbol's real column (match what
`symbols(include_body=true)` shows verbatim). For Python, that avoids the
`IndentationError`; for Kotlin, it avoids the mis-alignment.

## Resume
N/A — fixed. If a multi-line-string corruption case surfaces (base-shift adds
whitespace inside a Python/Kotlin `"""..."""` when the caller dedented the whole
block), harden `reindent_to` to skip lines inside string-literal AST nodes
(tracked as the deferred refinement in the brainstorm). See
`src/util/text.rs::reindent_to` and `src/tools/symbol/edit_code.rs::do_insert`.

## References
- `src/tools/symbol/edit_code.rs` (`do_insert`, `do_replace`)
- `src/util/text.rs` (`reindent_to`, `min_indent`, `reindent_block`, `leading_ws`)
- `src/tools/edit_file/mod.rs` (now imports the shared helpers)
- Sibling clamp bug: `docs/issues/2026-06-05-edit-code-insert-after-last-python-method.md`
