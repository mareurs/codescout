---
status: open
opened: 2026-07-19
closed:
severity: high
owner: marius
related: ["fb33085544512c73"]
tags: [edit_code, lsp, mcp-tooling]
kind: bug
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
Unknown — see Hypotheses tried. The tool's own diagnostic names the
mechanism at a high level (LSP gave a truncated range for the symbol;
AST-based repair kicked in) but the *repaired* extent was still short of
the true function end. Likely the AST repair walks to the end of some
inner node (e.g. the last top-level statement's *macro invocation
token*, not the enclosing block) rather than the full `fn` item's
closing brace — consistent with the splice point landing exactly at
`assert_eq!(` (a macro call node boundary) rather than after the
statement's trailing `;` and the block's `}`.

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
Not yet attempted. Immediate workaround applied to unblock Task 2 (see
Workarounds). No code fix proposed in this bug file yet — needs
maintainer to trace the AST-repair path in the `edit_code` implementation
(likely `src/tools/edit_code.rs` or the LSP mux repair helper) for the
case where the anchor's last statement is a multi-line macro call.

## Tests added
N/A — this is a bug in the MCP tool itself (codescout's own `edit_code`),
not in the `codescout` Rust library under edit; no Rust regression test
applies. A regression test would live in codescout's own tool test suite
(outside this task's scope) exercising `edit_code(insert, after)` against
a `#[test] fn` anchor whose last statement is a multi-line macro call.

## Workarounds
Re-run `symbols(path=...)` (or re-read the file) immediately after any
`insert`/`position` edit whose response carries a `warning` field, and
manually reconcile via `edit_file`/`edit_code` if the anchor's own body
was corrupted — do not trust `"status": "ok"` alone when a `warning` is
present.

## Resume
Isolate a minimal reproduction: a file with a single `#[test] fn` whose
last statement is a multi-line macro call (e.g. `assert_eq!(\n a,\n b\n);`),
then call `edit_code(insert, after)` targeting that fn and inspect whether
the truncated-range warning + wrong AST-repair line reproduces outside
`gc.rs`. If confirmed, trace the AST-repair line-resolution code path in
the `edit_code` tool implementation.

## References
- `src/librarian/catalog/gc.rs` (file affected)
- Related bug (same tool, different failure mode — explicit refusal
  rather than silent corruption): `docs/issues/2026-07-10-edit-code-impl-method-selection-range-refusal.md`
