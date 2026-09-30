---
id: '0148d05a47da5457'
kind: bug
status: open
title: 'BUG: edit_code''s sibling-drop refusal blames a stale LSP range for a failure the tool''s own re-indent caused'
tags:
- cluster/hint-composed-without-the-request
- edit_code
- error-message
---

# BUG: edit_code's sibling-drop refusal blames a stale LSP range for a failure the tool's own re-indent caused

## Summary

When `edit_code(replace)` writes a body that no longer parses, `do_replace`'s corruption
check sees sibling symbols vanish from the re-extracted AST, restores the file, and refuses
with "the edit range overshot into adjacent code (likely a stale LSP range)". The verdict
`CorruptionVerdict::SiblingsDropped` records THAT siblings disappeared and nothing about
why. The message then asserts one cause, a stale range, and the hint sends the caller to
refresh the symbol index and retry. When the real cause is the body's indentation, neither
step helps, and the caller is pointed away from the fault.

## Symptom (Effect)

Observed 2026-09-30 on the live tool. A replace body already at the symbol's column, with
one column-0 comment, was refused with:

    edit_code replace('Foo/a') would have dropped sibling symbols: Foo/b, Foo/c, Foo/d.
    The edit range overshot into adjacent code (likely a stale LSP range). File restored.
    hint: Try symbols(path) to refresh, then retry; or narrow the edit via edit_file ...

Nothing overshot. `reindent_to` shifted every line but the comment, the result had an
`IndentationError`, and the re-parse lost the rest of the class. The agent that hit this
diagnosed it as double indentation only by reading its own body; the refusal never said so.
Retrying after `symbols(path)` would have produced the identical refusal.

## Reproduction

Base: `experiments` before `2da4e2fe`, where the indentation cause is reachable. On a
scratch Python class with methods `a`..`d`, replace `Foo/a` with a body at column 4 holding
one column-0 comment line. The refusal above is the result. After `2da4e2fe` that particular
input no longer fails, so reproduce the message with any body that makes the file stop
parsing, for example an unclosed bracket in the replacement.

Not yet run: whether an unclosed-bracket body reaches the SAME branch with the SAME text.
Confirm before relying on it.

## Root cause

`corruption_verdict` (`src/symbol/edit.rs`) checks the name sets BEFORE `syntax_regressed`,
and its docstring gives the reason: "when a symbol actually vanished, that is the cause and
the broken parse is its consequence". For a body that fails to parse the causality is the
reverse. The parse break is the cause, tree-sitter then loses the symbols after it, and
`SiblingsDropped` fires first and wins. The verdict variant carries only the dropped names,
so by the time a message is built the fact that the file also stopped parsing is gone from
the variant.

The information is not gone from the caller: `do_replace` computes `syntax_regressed`
(`src/tools/symbol/edit_code.rs`, just before the `corruption_verdict` call) and still has
it in scope at the `SiblingsDropped` message site. It is available and not consulted.

Both message sites carry the wording: `remove` (around line 861) and `replace` (around
line 1178).

## Fix

Not decided. Options, cheapest first:
- (a) At each `SiblingsDropped` message site, branch on `syntax_regressed` (already in scope
  in `do_replace`; check `do_remove`): when true, say the edit left the file unparseable and
  point at the body's indentation and brackets; keep the stale-range wording only when it
  parsed. Leaves `corruption_verdict`'s ordering and docstring intact.
- (b) Carry the flag in the variant (`SiblingsDropped { dropped, syntax_regressed }`), so
  the message is built from what the verdict knew. Larger, touches every match on the enum.
- (c) Reorder so `SyntaxBroken` outranks `SiblingsDropped`. Rejected on its face: the
  docstring's own case (a genuinely vanished symbol with a broken parse as the consequence)
  would then lose its actionable "body must be the complete declaration" message.

Both sites carry the wording, so a fix has to reach both or it repairs one and leaves the
other, as happened with the two worktree notices.

## Tests

(pending) Per the guard-remedy rule in CLAUDE.md, assert the message's SHAPE (it names the
parse failure when there is one) rather than its sentences.

## Workarounds

Read your own body first. If it is a nested symbol, check its indentation and its brackets
before refreshing the symbol index.

## References

- `src/tools/symbol/edit_code.rs` (`do_replace`, `do_remove`, `CorruptionVerdict`)
- `docs/issues/archive/2026-09-30-edit-code-reindent-takes-its-base-from-the-shallowest-line-so-one-column-0-line-double-indents-a-pre-indented-body.md` (the case that exposed it)
