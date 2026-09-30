---
id: '9b760f69322e2730'
kind: bug
status: open
title: 'BUG: edit_file''s post-edit syntax warning never fires for a Python indentation error tree-sitter does not flag'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
- edit_file
- python
- indentation
closed: ''
opened: 2026-09-30
related:
- edit-code-syntax-guard-accepts-python-indentation-errors-tree-sitter-does-not-flag
severity: low
---

# BUG: edit_file's post-edit syntax warning never fires for a Python indentation error tree-sitter does not flag

## Summary

After an exact-match `edit_file`, `src/tools/edit_file/mod.rs` writes the file and then asks
`ast::has_syntax_errors` whether it still parses, returning `"warning": "syntax error detected
after edit — file may be malformed"` if not. The check is tree-sitter's ERROR-node test, which for
Python does not see a dedent to a column no enclosing block has, nor a tab/space mix (the blind spot
of the sibling `edit_code` guard, fixed in `c64e0c65`). So the one signal `edit_file` gives about a
malformed result is absent for exactly those Python errors.

## Symptom (Effect)

`edit_file` on a Python file returns `"ok"` with no warning, and the file fails at import with
`IndentationError: unindent does not match any outer indentation level`.

## Reproduction

Run 2026-09-30 on the live tool (a build that predates the `edit_code` fix, which does not touch
`edit_file`). Scratch file `def f(): return 1` / `def g(): return 2` (each body on its own
4-space line). `edit_file(old_string="    return 2", new_string="    return 2\n  x = 3")` returned
`"ok"`, and `python3 -m py_compile` then reported `IndentationError: unindent does not match any
outer indentation level (line 7)`.

## Environment

`src/tools/edit_file/mod.rs`: the warning at the end of the exact-match path; the two
whitespace-normalized gates (`after && !before`), which are unreachable for Python because the
normalized repair is disabled for indentation-significant languages; and
`tools::edit_repair::finalize_edit_content`, whose `Introduced` arm is written anyway.

## Root cause

The warning and the gates call `has_syntax_errors` only. `ast::has_indentation_errors`
(`src/ast/indent.rs`) now answers the question tree-sitter cannot, and `edit_file` does not ask it.

## Fix

Not decided. The contained form is to make the exact-match warning ask both questions. A scanner
false positive there costs a spurious warning, not a refusal or a rewrite, so it is the safe place
to start. Do **not** widen `finalize_edit_content`: its `Repaired` arm decodes literal escapes
(`\n`, `\t`) in whatever it flags, so a false positive could rewrite a string literal, which is
why the `edit_code` fix kept its own guard out of it.

## Severity

Low. `edit_file` is the exact-text editor and warns instead of refusing by design, so the caller
is already responsible for every byte of indentation; what is missing is only the warning.

## Tests added

None.

## Workarounds

After an `edit_file` on a Python file, run `python3 -m py_compile <file>` or the file's tests.

## References

- `docs/issues/archive/2026-09-30-edit-code-syntax-guard-accepts-python-indentation-errors-tree-sitter-does-not-flag.md`
  (the `edit_code` half, fixed; its checker is the thing to reuse)
