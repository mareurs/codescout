---
id: '8c576c065fee4554'
kind: bug
status: open
title: 'BUG: edit_code''s post-edit syntax guard accepts Python indentation errors that tree-sitter does not flag'
tags:
- cluster/guard-narrower-than-its-name
- edit_code
- python
- indentation
closed: ''
opened: 2026-09-30
owner: marius
related:
- edit-code-sibling-drop-refusal-blames-a-stale-lsp-range-for-an-unparseable-body
- edit-code-rebasing-a-tab-indented-body-onto-a-space-file-mixes-indent-units
severity: medium
---

# BUG: edit_code's post-edit syntax guard accepts Python indentation errors that tree-sitter does not flag

## Summary

`edit_code` rolls back an edit that turns a parsing file into one that does not, through
`syntax_regressed` (`src/symbol/edit.rs`), which asks `ast::has_syntax_errors` (tree-sitter,
`root_node().has_error()`). For Python that check is blind to the indentation errors Python
itself rejects: an unindent that matches no enclosing level, a dedent between two levels, and a
tab after spaces at one level. The guard is named for syntax and covers what tree-sitter's error
recovery flags, so `edit_code` reports `status: ok` for a file that fails on import.

## Symptom (Effect)

Observed 2026-09-30 on the live tool, session `3e2b9cc8`. An `edit_code(replace)` of a Python
test method with a two-method body whose continuation lines were indented deeper than its first
line returned `{"status": "ok", "replaced_lines": "1091-1098"}`. The file then failed to import,
at test-collection time, with `IndentationError: unindent does not match any outer indentation
level`. Nothing in the edit's response said the file no longer parsed. The failure was found by
running the test suite, one step later and in another tool.

## Reproduction

**Measured 2026-09-30, `experiments` at `68f432fe`, on synthetic strings only.** Not measured:
the same inputs driven through `edit_code` itself on the current binary; the observation above is
the only end-to-end evidence and it predates this probe by about an hour.

A throwaway integration test (kept out of the repo; it printed and never asserted):

```rust
use codescout::ast::has_syntax_errors;
use codescout::symbol::edit::syntax_regressed;
const PRE: &str = "class Foo:\n    def a(self):\n        return 1\n\n    def b(self):\n        return 2\n\n    def c(self):\n        return 3\n";
// for each POST below: has_syntax_errors(POST, "python"), syntax_regressed(PRE, POST, "python")
```

| POST (a replacement of `Foo/b`) | tree-sitter `has_syntax_errors` | `syntax_regressed` | real `python3 compile()` |
|---|---|---|---|
| control: `return (2` (unclosed paren) | true | true | (syntax error) |
| def at 4, body at 12, then a second def at 8 | **false** | **false** | `IndentationError: unindent does not match any outer indentation level` |
| body at 8 then a line at 6 (dedent between levels) | **false** | **false** | `IndentationError: unindent does not match any outer indentation level` |
| one level indented with spaces, then a tab | **false** | **false** | `TabError: inconsistent use of tabs and spaces in indentation` |
| def at 4, body at 12, no second def (consistent over-indent) | false | false | compiles |

`PRE` itself reports `has_syntax_errors=false`. The control row is what shows the probe can
return true; the last row is a correct negative (the file is valid Python).

## Environment

`src/symbol/edit.rs` (`syntax_regressed`, `corruption_verdict`), `src/ast/parser.rs`
(`has_syntax_errors`), the two call sites in `src/tools/symbol/edit_code.rs` (`do_remove`,
`do_replace`). tree-sitter Python grammar as vendored at `68f432fe`.

## Root cause

Not fully established. What is measured is the boundary: tree-sitter's Python error recovery does
not mark an inconsistent dedent, a between-levels dedent, or a tab/space conflict as an ERROR
node, while the interpreter rejects all three. That is a property of the external indentation
scanner's recovery, inferred from the outputs above, not read from the grammar source.

The consequence follows from the code: `corruption_verdict` returns `Clean` whenever no symbol
name is lost and `syntax_regressed` is false, and `Clean` writes the file and answers `ok`.

## Evidence

The table above, and the one live occurrence. The `syntax_regressed` docstring states the claim
this contradicts: "the symbol-level checks compare name sets, so they cannot see damage that drops
no name … `SyntaxBroken` is the net for damage the name checks structurally CANNOT see". For
indentation in Python the net has a hole that a name check cannot cover either.

## Hypotheses tried

None beyond the probe. Not tried: other indentation-sensitive languages the tool edits (YAML is
the obvious one); whether a comment or blank-line position changes tree-sitter's answer.

## Fix

Not decided. Options:
- (a) A small indentation-consistency check for languages where indentation is syntax, run beside
  `has_syntax_errors`: every dedent must land on an enclosing level, and one block must not mix
  tabs and spaces ambiguously. Rust-only, no dependency, and it states its own scope.
- (b) Shell out to `python3 -m py_compile` when present. Exact, but adds a process and a
  dependency the tool does not have today, and does nothing for YAML.
- (c) Document the limit at the refusal site and in the tool description: the guard is
  tree-sitter-shaped, and for Python it cannot see indentation. Cheapest, and honest, but the
  failure stays silent.

Option (a) matches the project's stance elsewhere (a loud refusal beats a silent acceptance) and
is the one to prototype. Whatever is chosen, the message on a hit should say the file
stopped parsing and name indentation, not the stale-range wording, which is the subject of the
sibling bug `0148d05a47da5457`.

## Tests added

None. Owed, per the guard-remedy rule in `CLAUDE.md`: a case per row of the table above, each
built so no other guard refuses it first (a body that drops a sibling would be refused by the name
check and prove nothing here), and the consistent-over-indent row as the correct-negative
control. Then a mutation of the new predicate per clause.

## Workarounds

After an `edit_code` on a Python file, especially with a multi-line body, run the file's own
tests or `python3 -m py_compile <file>` before continuing. Indent the body with the file's own
unit and give every continuation line its depth relative to the first line.

## Resume

Reproduce end to end first: drive `edit_code(replace)` with the second row's body against a
scratch Python class on the current binary and confirm `status: ok`. Then prototype option (a) in
`src/symbol/edit.rs`. Files `src/symbol/edit.rs`, `src/tools/symbol/edit_code.rs` and
`src/tools/symbol/tests.rs` were being edited by another live session (`codescout-4c`) on
2026-09-30 for the sibling bug, so read `git status` before touching them.

## References

- `docs/issues/2026-09-30-edit-code-sibling-drop-refusal-blames-a-stale-lsp-range-for-an-unparseable-body.md`
  (the message this guard's hit should not reuse)
- `docs/issues/2026-09-30-edit-code-rebasing-a-tab-indented-body-onto-a-space-file-mixes-indent-units.md`
  (related indentation defect in the re-base step)
- `docs/issues/archive/2026-08-07-edit-code-remove-ast-repair-over-deletes.md` (the case
  `syntax_regressed` was added for)
