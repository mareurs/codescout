---
id: c39fb635878514d3
kind: bug
status: fixed
title: 'BUG: edit_file''s post-edit syntax warning never fires for a Python indentation error tree-sitter does not flag'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
- edit_file
- python
- indentation
closed: 2026-09-30
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
outer indentation level (line 5)`.

**A second site, reproduced the same day after this file first said it was unreachable.** The same
file with CRLF line endings and an `old_string` that differs from it only in line endings
(`"def g():\n    return 2"`) takes the CRLF-tolerant branch, which wrote the identical dedent and returned
`status: ok, applied_via: crlf-tolerant match`; `py_compile` failed the same way. That branch has
its own `after && !before` gate, asked of tree-sitter only.

## Environment

`src/tools/edit_file/mod.rs`, in `perform_edit`: the warning at the end of the exact-match path and
the CRLF-tolerant gate. The whitespace-normalized gate below them is unreachable for Python,
because `indentation_significant` returns before it; the CRLF branch is not, since it runs first.
This file originally called both gates unreachable, from reading the code; running the CRLF case
showed that half was wrong. `tools::edit_repair::finalize_edit_content` also stays tree-sitter-only.

## Root cause

The warning and the CRLF gate call `has_syntax_errors` only. `ast::has_indentation_errors`
(`src/ast/indent.rs`) answers the question tree-sitter cannot, and `edit_file` did not ask it.

## Fix

Both reachable sites now ask both questions. The exact-match warning is
`has_syntax_errors(new) || has_indentation_errors(new)`; a scanner false positive there costs a
warning, not a refusal. The CRLF gate calls `symbol::edit::syntax_regressed`, which is tree-sitter
plus the scanner behind the rule "clean before, broken after", so a file that was already
flagged is not refused for the edit that did not cause it.

Not widened, deliberately: `finalize_edit_content`, whose `Repaired` arm decodes literal escapes
(`\n`, `\t`) in whatever it flags, so a false positive could rewrite a string literal; and the
whitespace-normalized gate, which Python cannot reach, so a change there has no test that can
fail and no input that can tell it from the old line.

## Severity

Low. `edit_file` is the exact-text editor and warns instead of refusing by design, so the caller
is already responsible for every byte of indentation; what is missing is only the warning.

## Tests added

In `src/tools/edit_file/tests.rs`: `edit_file_warns_on_a_python_dedent_tree_sitter_does_not_flag`,
`edit_file_stays_silent_on_a_python_edit_that_indents_cleanly` (the control that an always-warn
change cannot pass), and `edit_file_crlf_tolerant_match_refuses_a_python_dedent_tree_sitter_does_not_flag`.
Both fixtures assert their post-image is clean under `has_syntax_errors`, so a grammar upgrade that
starts flagging the dedent fails the test instead of silently leaving it guarding nothing. Observed
red before the fix (warning test: no warning; CRLF test: the dedent written). Mutated on the final
bytes, one per site: dropping the `||` clause kills the warning test; replacing it with
`lang == "python"` kills the control alone; reverting the CRLF gate to tree-sitter only kills the
CRLF test. Each death names the one test that should die.

## Fix provenance

- **SHA:** `83ff4a769ca5a526cd43fad346c7f3b84daae446` (`experiments`)
- **patch-id:** `8ae7d5485e96e1ec3fbbe3c837bf346fbc3e819d` (`git show <sha> | git patch-id --stable`)

Verified on `experiments` 2026-09-30: gate `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`; the three new tests read
by name out of both test lanes; one mutation per site observed killed on the final bytes. Not yet
observed on the live MCP tool: that binary predates this commit until the next `./scripts/rb.sh`
and `/mcp`.

## Workarounds

After an `edit_file` on a Python file, run `python3 -m py_compile <file>` or the file's tests.

## References

- `docs/issues/archive/2026-09-30-edit-code-syntax-guard-accepts-python-indentation-errors-tree-sitter-does-not-flag.md`
  (the `edit_code` half, fixed; its checker is the thing to reuse)
