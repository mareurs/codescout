---
id: fd426fd1bfd17068
kind: bug
status: fixed
title: 'BUG: edit_code''s post-edit syntax guard accepts Python indentation errors that tree-sitter does not flag'
tags:
- cluster/guard-narrower-than-its-name
- edit_code
- python
- indentation
closed: 2026-09-30
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


**Reproduced end to end 2026-09-30 through `edit_code` on the current release binary** (built
after `62a6b903`; the new wording of the sibling-drop bug's fix was confirmed present in it), by
session `e41af068`. On a scratch Python class with methods `a`, `b`, `c` at 4-space indent,
`edit_code(action="replace", symbol="Foo/b")` with the body

    def b(self):
            x = 1
        y = 2

(first line at column 0, `x = 1` at 8, `y = 2` at 4) returned `{"status": "ok",
"replaced_lines": "5-6"}`. The re-base shifted every line by 4, so the file on disk was

    5      def b(self):
    6              x = 1
    7          y = 2

and `py_compile` rejected it: `IndentationError: unindent does not match any outer indentation
level (line 7)`. The control in the same session, an unclosed paren, was refused correctly. So
the blind spot in the table above is not only a property of synthetic strings: the tool
accepted, wrote and reported success for a file the interpreter refuses.

## Environment

`src/symbol/edit.rs` (`syntax_regressed`, `corruption_verdict`), `src/ast/parser.rs`
(`has_syntax_errors`), the two call sites in `src/tools/symbol/edit_code.rs` (`do_remove`,
`do_replace`). tree-sitter Python grammar as vendored at `68f432fe`.

## Root cause

tree-sitter's Python error recovery does not mark an inconsistent dedent, a between-levels dedent, or a tab/space conflict as an ERROR node, while CPython's tokenizer rejects all three. That is a property of the external indentation scanner's recovery, inferred from the measured outputs above, not read from the grammar source.

The consequence follows from the code: `corruption_verdict` returns `Clean` whenever no symbol name is lost and `syntax_regressed` is false, and `Clean` writes the file and answers `ok`.

Reproduced through `edit_code` on the live tool before the fix (session `3e2b9cc8`, 2026-09-30): scratch class `Foo` with methods `a`, `b`, `c`; `replace` of `Foo/b` with `    def b(self):` / `            return 2` / `        x = 1` returned `{"status": "ok", "replaced_lines": "5-6"}`, and `python3 -m py_compile` then reported `IndentationError: unindent does not match any outer indentation level (line 7)`.

## Evidence

The table above, and the one live occurrence. The `syntax_regressed` docstring states the claim
this contradicts: "the symbol-level checks compare name sets, so they cannot see damage that drops
no name … `SyntaxBroken` is the net for damage the name checks structurally CANNOT see". For
indentation in Python the net has a hole that a name check cannot cover either.

## Hypotheses tried

None beyond the probe. Not tried: other indentation-sensitive languages the tool edits (YAML is
the obvious one); whether a comment or blank-line position changes tree-sitter's answer.

## Fix

Option (a), implemented, for Python only.

- `src/ast/indent.rs` reimplements the tokenizer's indentation rule: the stack of (column, alternate column) levels with tabs counting 8 and 1, the dedent that must land on an existing level, and the alternate-column comparison that makes a tab/space mix a `TabError`. It models the only things that stop a physical line from starting a logical one: brackets, `'`/`"` strings (single and triple, with escapes), `#` comments and a trailing backslash. It answers "would the tokenizer reject this?", not "does this parse": the parser-level errors (`unexpected indent`, `expected an indented block`) stay with tree-sitter, which does flag them.
- `syntax_regressed` now asks both questions, on the pre-image gate and the post-image check, so `replace` and `remove` are covered at once and a file whose indentation was already wrong stays editable.
- `edit_code insert` reaches the same check through its own guard after `finalize_edit_content`, which is left alone: `edit_file` shares it and runs an escape-decoding repair on whatever it flags, so a scanner false positive there could decode a `\n` inside a string literal.
- The refusal text needed no change: `replace_syntax_broken_reason` and the `insert` message already name indentation first, from the sibling bug's fix.

Options (b) and (c) were not taken: (b) adds a process and a dependency and does nothing for other languages; (c) leaves the failure silent.

## Tests added

Red first, then green: `replace_symbol_refuses_a_python_body_that_dedents_to_no_enclosing_level` and `insert_code_refuses_a_python_body_that_dedents_to_no_enclosing_level` (`tests/symbol_lsp.rs`, truthful mock ranges so the syntax check is the only guard that can refuse) answered `Ok` before the change and refuse after it. Their controls, `replace_symbol_accepts_a_python_body_with_consistent_nesting` and `insert_code_accepts_a_consistently_indented_python_method`, pass both ways.

Unit: `syntax_regressed_sees_the_python_indentation_errors_tree_sitter_does_not_flag` (`src/symbol/edit.rs`: asserts its own premise that tree-sitter accepts both inputs, then the post-image site, the pre-image gate for indentation-only breakage, the repair case and the language gate) and six in `src/ast/indent.rs`: one source per rule the tokenizer can reject (every expectation checked against `python3`), valid controls including a tab-consistent file and two spellings of one level, the continuation-line cases, the inverse cases where a bracket or quote inside a comment or string must not hide a later error, form feed and CRLF, and the language gate.

## Measurements

**Against CPython, 2026-09-30.** Corpus: the 604 stdlib files that compile (tests and `lib2to3` excluded) plus 1,324 single-line whitespace mutants of 500 of them, each labelled by `python3`'s own `compile()`. 1,254 valid files (604 originals and 650 mutants that stayed valid): **0** flagged. 433 mutants CPython rejects at the tokenizer (287 `unindent`, 146 `TabError`): **0** missed. 241 mutants CPython rejects some other way: the checker flagged 14, and in all 14 CPython stopped one or two lines earlier at a parser-level `unexpected indent`, which the checker deliberately does not report, with a tokenizer error just after it. These counts are over real files and a synthetic mutation distribution, not over `edit_code` traffic.

**Mutations.** 35 over the new code, one per guarded site (every tokenizer rule, line classification, scanner construct, both halves of `syntax_regressed`, the insert guard), through `scripts/mutation-probe.sh`; all killed on the final bytes (`86a52ee1`). The first pass, before two added assertions, one deleted clause and the formatter, killed 31 of 36, with two survivors and three inconclusive runs. The survivors were a dead clause (`levels.len() > 1` could not be false, since no `usize` column is below the column-0 level), deleted, and a missing case (a quote, one character, a quote inside a triple-quoted string), added. A third survivor found on the next pass, a tab counted as two alternate columns, was closed with a case where two spellings reach one level. Each inconclusive run was environmental, or a pattern the formatter had invalidated; each re-run was killed.

## Workarounds

After an `edit_code` on a Python file, especially with a multi-line body, run the file's own
tests or `python3 -m py_compile <file>` before continuing. Indent the body with the file's own
unit and give every continuation line its depth relative to the first line.

## Not covered

- `edit_file` has the same blindness through `finalize_edit_content` and three gates of its own, and was left alone on purpose: it is the exact-text editor, the caller controls every byte of indentation, and it is where an escape-decoding repair runs on whatever a flag reaches. Filed as its own bug.
- 3.12 f-strings that reuse the enclosing quote character are not modelled. A file relying on them can be mis-scanned; `syntax_regressed` only asks about a file that was clean before the edit, so a mis-scan cannot refuse an edit unless the edit changes what the scanner sees.
- YAML and other indentation-significant languages: no rule and no grammar here, so nothing is refused on a guess.
- The two parser-level indentation errors, `unexpected indent` and `expected an indented block`, which tree-sitter flags on its own.

## References

- `docs/issues/archive/2026-09-30-edit-code-sibling-drop-refusal-blames-a-stale-lsp-range-for-an-unparseable-body.md`
  (the message this guard's hit should not reuse)
- `docs/issues/archive/2026-09-30-edit-code-rebasing-a-tab-indented-body-onto-a-space-file-mixes-indent-units.md`
  (related indentation defect in the re-base step; fixed in the same session)
- `docs/issues/2026-09-30-edit-file-post-edit-syntax-warning-is-blind-to-python-indentation-errors.md`
  (the `edit_file` half, left open)
- `docs/issues/archive/2026-08-07-edit-code-remove-ast-repair-over-deletes.md` (the case
  `syntax_regressed` was added for)

## Fix provenance

- **SHA:** `c64e0c65bee44df633a2856661ba011fa08eb8b7` (`experiments`) — the tokenizer-rule checker, `syntax_regressed`, and the tests
- **patch-id:** `fcc4f1e5cc15e9b9c86a6a33ec3c851e6eba0ef8`
- **SHA:** `9d2049ddcb77327cc709f4a9ba25568ea4d3fd6f` (`experiments`) — the `insert` guard those tests exercise
- **patch-id:** `4b6fd63e7ccca2862572389d7edd97276d54200a`

Two commits because the shared-index ownership recorder stamped the first snapshot of `edit_code.rs` `-` and `commit-mine` left it out, so the first commit carried the insert tests without the guard (`fe8affc7141081ef`). Read them as one change.

Gate: `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0` on 2026-09-30, on a tree holding both commits plus the then-uncommitted fix for the tab/space sibling (`86a52ee1`). An earlier gate on the combined tree was red only on `tool_surface_under_budget`, from another session's uncommitted `reindent` parameter description, and on a refused `fmt-mine` for files another session also wrote; neither was this change. The gate is vacuous for nothing this change touches: it is neither librarian code nor `server-stack` nor the ONNX path.
