---
id: '1fdc987a7075a032'
kind: bug
status: open
title: 'BUG: edit_code re-basing a tab-indented body onto a space-indented file leaves mixed tab and space indentation'
tags:
- cluster/unclassified
- edit_code
- indentation
---

# BUG: edit_code re-basing a tab-indented body onto a space-indented file leaves mixed tab and space indentation

## Summary

`reindent_block` (`src/util/text.rs`) treats indentation as an opaque string prefix. It
strips `agent_base` from each line and prepends `file_base`, keeping the remainder as is.
When the caller's body is indented with tabs and the file with spaces, the base changes
unit but every inner step keeps its tabs, so one block carries both.

## Symptom (Effect)

Observed 2026-09-30 on the live tool, scratch Python class at 4-space indent. Replacing
`Foo/d` with a body indented by tabs (`\tdef d(self):` then `\t\treturn 40`) wrote:

        def d(self):        <- 4 spaces
        <TAB>return 40      <- 4 spaces then a tab

Python 3 accepted this file, so nothing refused it, but the line's depth depends on tab
width and the block now mixes units. A stricter language or linter (Python with `-tt`,
YAML, a formatter) would reject or rewrite it. No error or hint was emitted.

## Reproduction

Run 2026-09-30, `experiments` at `09334f05`. Scratch file with `class Foo:` and four
4-space-indented methods; `edit_code(action="replace", symbol="Foo/d", body="\tdef d(self):\n\t\treturn 40")`.
Result: `replaced_lines: 11-12`, file parses, second line is `    ` plus a tab.

Not run: the reverse direction (spaces body into a tab-indented file), which the same
code path should turn into a tab plus spaces line. Confirm before asserting it.

## Root cause

`reindent_block`'s per-line step is `strip_prefix(agent_base)` then `file_base + rest`.
`rest` still holds the caller's indentation unit. Only the shared base is translated. The
"ragged line" fallback (`file_base + line.trim_start()`) is a second path that discards
inner structure entirely when a line does not start with the base.

## Fix

Not decided. Options:
- (a) Detect a unit mismatch between `agent_base` and `file_base` and refuse with a
  RecoverableError naming both units.
- (b) Convert the caller's unit to the file's before shifting, which needs the tab width
  and is a guess in a file that already mixes.

Option (a) matches the project's stance elsewhere: a loud refusal beats a silent
reinterpretation.

## Tests

(pending)

## Workarounds

Indent the body with the file's own unit.

## References

- `src/util/text.rs` (`reindent_block`, `reindent_to`)
- `docs/issues/archive/2026-09-30-edit-code-reindent-takes-its-base-from-the-shallowest-line-so-one-column-0-line-double-indents-a-pre-indented-body.md` (found while reproducing it)
