---
status: fixed
opened: 2026-05-02
closed: 2026-05-17
severity: high
owner: marius
related: ["BUG-029", "BUG-031", "BUG-036"]
tags: ["edit_code", "ast", "lsp", "silent-corruption", "residual"]
kind: bug
---

# BUG: `edit_code insert after` injected code mid-function body when symbol body was truncated in display

## Summary

`edit_code(action="insert", position="after")` against a symbol whose `editing_end_line` was under-reported (because LSP returned the last *statement* line instead of the closing `}`, and the AST early-returned on `has_syntax_errors`) inserted the new code mid-body, splitting an open `assert!()` and producing an unbalanced file. Top-level symbol path is now fixed; a smaller residual remains for parented under-extension.

## Symptom (Effect)

- `edit_code insert after` returned success.
- Inserted code landed mid-body of the target function, splitting an open `assert!(err_str.contains("nonexistent"),` and corrupting the file.
- File failed to compile.

## Reproduction

Reproducing commit `629b7eae` (experiments branch) — file `src/tools/symbol/tests.rs`, function `find_unique_symbol_by_name_path_errors_on_ambiguous_name` was ~82 lines long (lines 1081–1162+ in that snapshot).

Exact call:

```json
{
  "symbol": "find_unique_symbol_by_name_path_errors_on_ambiguous_name",
  "path": "src/tools/symbol/tests.rs",
  "action": "insert",
  "position": "after",
  "body": "\n#[test]\nfn find_unique_symbol_by_name_path_suggests_leaf_matches() { ... }"
}
```

Preceding `symbols` call returned `end_line: 1162` and a body cut off mid-`assert!()` at that same line — the function's closing `);` and `}` were NOT shown.

Repro fixture: `tests/fixtures/edit-eval-rust/src/bug051_repro.rs` (commit `b11a2ca0`).

## Environment

- Date: 2026-05-02
- Tool: `mcp__codescout__edit_code`
- Commit at repro: `629b7eae`

## Root cause

Confirmed 2026-05-02: `editing_end_line` in `src/symbol/edit.rs` had an early return when `has_syntax_errors` was true, falling back to LSP's `end_line`. During mid-session editing (prior edits leave broken syntax), LSP frequently reports the last *statement* line rather than the closing `}`. Insertion used this short value as anchor, landing inside the function.

## Evidence

- Compiler error pointing at the split `assert!()` macro call as the unbalanced site.
- `symbols(name=..., include_body=true)` output truncated mid-statement before the fix.

## Hypotheses tried

1. **Hypothesis:** LSP returns the wrong `end_line` mid-edit. **Test:** Compared LSP response to AST-extracted `end_line`. **Verdict:** Confirmed — LSP returned a line ~10 short of the actual closing `}`. **Evidence link:** see Root cause.
2. **Hypothesis:** Use `max(ast, lsp)` to always pick the larger. **Test:** Considered during the fix. **Verdict:** Rejected — regresses BUG-029 when syntax errors coexist with LSP over-extension. **Evidence link:** original entry on `docs/TODO-tool-misbehaviors.md`.

## Fix

**Applied 2026-05-02:** Removed the syntax-error early return in `editing_end_line` (`src/symbol/edit.rs`). AST is run unconditionally and trusted when it finds the symbol (same as on a clean file).

**Residual closed 2026-05-09:** Even after the AST-trust fix, when AST extraction itself succeeded but `find_ast_end_line_in` returned `None` (severely broken parse, ambiguous match, etc.), `editing_end_line` silently fell back to LSP's `end_line`. For top-level symbols with no parent in the symbol tree, the parent-clamp safety net in `do_insert` couldn't recover. Added `editing_end_line_strict` (returns `Option<u32>`, `None` on any AST resolution failure) and wired it into `do_insert`'s "after" branch: when AST cannot pinpoint the end AND the symbol has no parent, the call now returns a `RecoverableError` with actionable guidance instead of corrupting the source. When a parent exists, the existing lenient path runs and the parent clamp keeps the result bounded — preserving the BUG-029/036 recovery path.

**Last residual closed 2026-05-17:** The parented + syntax-error + LSP under-extension case. The parent-clamp in `do_insert` only catches over-extension (LSP value past the parent's `}`); under-extension (LSP value before the target symbol's `}` but still inside the parent body) was unbounded. `do_insert`'s "after" branch now refuses universally when `editing_end_line_strict` returns `None`, not just for top-level symbols. The BUG-029 happy path is preserved because it relies on AST succeeding, and `editing_end_line_strict` still returns `Some` whenever AST resolves the symbol.
## Tests added

- `editing_end_line_with_syntax_errors_uses_ast_not_lsp_fallback` (`src/tools/symbol/tests.rs`)
- `editing_end_line_syntax_errors_do_not_regress_lsp_overextend` (`src/tools/symbol/tests.rs`)
- `editing_end_line_strict_returns_none_when_ast_cannot_find_symbol` (`src/tools/symbol/tests.rs`)
- `editing_end_line_strict_returns_some_when_ast_finds_symbol` (`src/tools/symbol/tests.rs`)
- `insert_code_after_refuses_when_ast_fails_and_no_parent_clamp` (`tests/symbol_lsp.rs`)
- Repro fixture: `tests/fixtures/edit-eval-rust/src/bug051_repro.rs` (commit `b11a2ca0`).

## Workarounds

For the residual case (parented symbol + syntax errors + LSP under-extension): pass `edit_code(action="replace")` on the whole parent symbol, or fix the syntax error first via `edit_file` before inserting.

## Resume

Closed. If a session ever hits the refusal error (`cannot determine end of '<name>' for insert-after — AST parse failed`), the file has severe syntax errors that broke tree-sitter. The error hint already names the workaround: fix the syntax first, or use `edit_file` with explicit context. No further escalation planned — strict-everywhere is the strongest available guarantee without inventing AST.
## References

- Originally tracked as **BUG-051** in `docs/TODO-tool-misbehaviors.md` (deprecated 2026-05-09; superseded by per-file system).
- Related: BUG-029 (LSP over-extension recovery), BUG-031 (`editing_start_line` walk-back), BUG-036 (parent clamp).
- Status note: classified `mitigated` rather than `fixed` because the parented-under-extension residual is still open.
