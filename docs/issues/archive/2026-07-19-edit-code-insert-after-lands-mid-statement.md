---
kind: bug
status: fixed
tags:
- edit_code
- lsp
- mcp-tooling
closed: 2026-07-20
opened: 2026-07-19
owner: marius
related:
- fb33085544512c73
severity: high
---

# BUG: edit_code insert/after on a `#[test] fn` anchor uses a wrong AST-repaired line, splicing new code mid-statement

## Summary
`edit_code(path, symbol="tests/<fn>", action="insert", position="after", body=...)`
targeting a `#[test]`-annotated function as the anchor inserted the new
`body` in the *middle* of the anchor function's last statement (a
multi-line `assert_eq!(...)` macro call) instead of after the function's
closing brace. The call itself warned that the LSP range was truncated
and that it "repaired from the AST," but the repaired line was still
wrong. In this instance the result was invalid Rust (caught immediately
by `cargo test`); for a differently-shaped statement it could plausibly
splice into a syntactically-valid-but-semantically-wrong position
without a compiler error to catch it.

## Symptom (Effect)
Tool response:
```json
{
  "status": "ok",
  "inserted_at_line": 61,
  "position": "after",
  "warning": "LSP returned a truncated range for 'meta_roundtrip_and_grace_default_and_cutoff' (ended at line 59); repaired from the AST to line 64. The edit used the AST extent — verify the result."
}
```
Resulting file (`src/librarian/catalog/gc.rs`), the anchor function's own
body was split — new code landed between the `assert_eq!(` call opener and
its arguments:
```rust
        let now = 1_000_000_000_000i64;
        assert_eq!(

    #[test]
    fn set_meta_overwrites_existing_key() {
        ...
    }

    #[test]
    fn grace_days_falls_back_on_invalid_override() {
        ...
    }

            visibility_cutoff_ms(&cat.conn, now).unwrap(),
            now - 7 * 86_400_000
        );
    }
}
```
`symbols(path=...)` afterward showed only 1 `Function` under `tests`
(spanning the enlarged 48-82 range) instead of 3 siblings — confirming
the new code was nested inside the anchor's body, not inserted after it.

## Reproduction
1. On branch `experiments`, commit `d5ee0464` (HEAD at the time), file
   `src/librarian/catalog/gc.rs` has one `#[test] fn
   meta_roundtrip_and_grace_default_and_cutoff() { ... }` inside `mod
   tests`, ending with a multi-line `assert_eq!(\n    lhs,\n    rhs\n);`.
2. Call:
   ```
   edit_code(path="src/librarian/catalog/gc.rs",
             symbol="tests/meta_roundtrip_and_grace_default_and_cutoff",
             action="insert", position="after",
             body="<two new #[test] fn ...>")
   ```
3. Observe the `warning` field in the response (truncated-range +
   AST-repair message) and inspect the file — the body lands inside the
   anchor's last statement, not after its closing `}`.

## Environment
codescout MCP server, rust-analyzer-backed LSP, project `codescout`,
branch `experiments`. Anchor symbol was a `#[test]`-attributed fn ending
in a multi-line macro-call statement (`assert_eq!` spanning 3 lines).

## Root cause
Confirmed via a minimal, controlled reproduction (see Tests added) —
not a tree-sitter/AST end-line bug at all. `find_ast_end_line_in` /
`editing_end_line_strict` correctly resolve the CHILD symbol's own end
line from the AST in every case tested, including a multi-line macro
call as the last statement.

The actual defect is in `do_insert`'s parent-boundary safety clamp
(`src/tools/symbol/edit_code.rs`, `do_insert`). After resolving the
child's insert position via `editing_end_line_strict` (AST-repaired,
correct), the code clamps that position against the ENCLOSING PARENT's
`end_line` — but `parent` comes from `find_parent_symbol(&symbols, ...)`,
where `symbols` is the raw, un-repaired LSP `document_symbols` list. The
child's own end gets AST-repaired; the parent's never does.

When the LSP under-reports the PARENT's own end_line (the same
truncation class the whole repair mechanism exists to fix one level
down — here, because the parent's last child ends in a multi-line macro
call, confusing the LSP's own boundary computation for the *enclosing*
module too), `parent_body_end_exclusive = parent.end_line + 1` becomes
smaller than the correctly-resolved child insert position. The
`.min(parent_body_end_exclusive)` clamp then silently drags the insert
backward into the child's own body — landing wherever the parent's
truncated end happened to fall, in the real occurrence squarely inside
the child's `assert_eq!(...)` argument list.
## Evidence
Tool response for the insert call (this session):
```
{"status":"ok","inserted_at_line":61,"position":"after","warning":"LSP returned a truncated range for 'meta_roundtrip_and_grace_default_and_cutoff' (ended at line 59); repaired from the AST to line 64. The edit used the AST extent — verify the result."}
```

`symbols(path="src/librarian/catalog/gc.rs")` immediately after the
insert:
```
Module  43-83  tests
    Function  48-82  tests/meta_roundtrip_and_grace_default_and_cutoff  fn()
```
(only one Function under `tests`, spanning the corrupted range — the two
new fns were not recognized as siblings because they were lexically
nested mid-statement).

`read_file(force=true)` on the same range showed the literal splice
(reproduced in Symptom above).

## Hypotheses tried
1. **Hypothesis:** The LSP's returned range for a `#[test]` fn stops at
   the attribute-decorated `fn` signature rather than the body.
   **Test:** compared the warning's "ended at line 59" against the
   function's actual pre-edit extent (47-60 per `symbols` before this
   edit).
   **Verdict:** plausible but not fully explanatory — 59 is close to but
   short of 60, consistent with a small under-count, not the ~4-6 line
   miss that put the splice inside the macro call.
2. **Hypothesis:** the "repaired from the AST to line 64" fallback
   computed the wrong node's end (e.g. resolved to the macro invocation
   `assert_eq!(...)` node's opening rather than the parent `fn` item's
   closing brace).
   **Test:** not yet isolated with a minimal repro outside this task;
   deferred.
   **Verdict:** deferred — best current lead.

## Fix
Applied in `da03f149` (branch `experiments`). `do_insert`'s parent clamp
now resolves the parent's authoritative end via `editing_end_line(parent)`
(the same AST-repair mechanism already used for the child) instead of
trusting `parent.end_line` raw. `editing_end_line` is the lenient variant
— it falls back to the raw LSP value when AST can't pin the parent down,
so the clamp is never worse than before, only more accurate when AST can
confirm a larger true end.
## Tests added
- `src/tools/symbol/tests.rs::editing_end_line_strict_multiline_macro_last_statement`
  — isolates the child-only AST end-line resolution for a `#[test]` fn
  whose last statement is a multi-line `assert_eq!(...)` call. Passed even
  before the fix, ruling out `find_ast_end_line_in` itself as the culprit
  and narrowing the search to the insert-application path.
- `tests/symbol_lsp.rs::insert_code_after_stale_parent_lsp_end_clamps_into_multiline_macro_body`
  — the actual root-cause reproduction: mocks a truncated child end_line
  AND a truncated parent (`mod tests`) end_line via `MockLspClient`, then
  calls the real `EditCode` insert-after path. Failed before the fix
  (new fn spliced inside the sibling's `assert_eq!` argument list,
  reproducing the exact corruption shape from the original incident);
  passes after `da03f149`.

Full suite: 3368 passed, 0 failed (`cargo test`), `cargo clippy --all-targets -- -D warnings` clean.
## Workarounds
Re-run `symbols(path=...)` (or re-read the file) immediately after any
`insert`/`position` edit whose response carries a `warning` field, and
manually reconcile via `edit_file`/`edit_code` if the anchor's own body
was corrupted — do not trust `"status": "ok"` alone when a `warning` is
present.

## Resume
Fixed on `experiments` at `da03f149`. Per project convention, kept `open`
(not `archived`) until this ships to `master` (`cargo rb` + `/mcp` live
verify), then archive per `git branch --contains da03f149`.
## References
- `src/librarian/catalog/gc.rs` (file affected)
- Related bug (same tool, different failure mode — explicit refusal
  rather than silent corruption): `docs/issues/2026-07-10-edit-code-impl-method-selection-range-refusal.md`
