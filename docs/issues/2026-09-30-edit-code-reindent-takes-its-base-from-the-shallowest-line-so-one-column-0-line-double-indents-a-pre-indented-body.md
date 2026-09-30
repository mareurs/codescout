---
id: b72b082b562ddea8
kind: bug
status: open
title: 'BUG: edit_code re-indents a body the caller already indented to the symbol''s column whenever ONE line is shallower, and the refusal blames a stale LSP range'
tags:
- cluster/addressing-without-an-escape-hatch
- edit_code
- indentation
- python
---

# BUG: edit_code re-indents a body the caller already indented to the symbol's column whenever ONE line is shallower, and the refusal blames a stale LSP range

## Summary

`reindent_to` (`src/util/text.rs`) decides whether a body needs shifting by comparing
the **minimum** indent over its code lines with the target column. A caller who already
indented the body to the symbol's column, but whose body holds one shallower line (a
column-0 comment, a top-level sibling), makes that minimum 0. The no-op guard then fails
and the *whole* block is shifted by the target's indent: every other line lands too deep,
the shallow line stays put, and the structure is broken.

## Symptom (Effect)

Reported by an agent editing nested Python symbols: "my edit tool auto-indents each line
to the symbol's level, but I'd already added indentation myself, causing a syntax error
from double-indented code". The agent worked around it by rewriting four bodies relative
to column 0.

The refusal text is itself wrong. `do_replace` catches the parse failure through its
sibling-drop guard and says *"the edit range overshot into adjacent code (likely a stale
LSP range)"*. Nothing overshot; the body was mis-indented by the tool.

## Reproduction

Run 2026-09-30 against the live `edit_code`, on a scratch Python file:

    class Foo:
        def a(self):
            return 1
        def b(self): ...
        def c(self): ...
        def d(self): ...

`edit_code(action="replace", symbol="Foo/a", body=...)` with a body already at column 4
and one column-0 comment inside it:

        def a(self):
            x = 1
    # a col-0 comment
            return x

Result: `{"ok": false, "error": "edit_code replace('Foo/a') would have dropped sibling
symbols: Foo/b, Foo/c, Foo/d ... (likely a stale LSP range). File restored."}`

Also observed on the same file: a body whose first line is at column 0 but whose inner
lines were pre-indented to the absolute column (`def b(self):\n        return 20`) lands
with `return 20` at column 12. That input is genuinely ambiguous (see Fix) and still parses.

## Root cause

`reindent_to`: `agent_base = min_indent_outside_literals(block, &mask)`, then
`if agent_base == target_base { return block }`. One shallow line sets `agent_base`, and
`reindent_block` applies that single shift to every line. The 2026-06-07 fix chose the
minimum on purpose ("keeps re-basing correct even when the first line is more indented
than a later one") and did not consider a body that is already at the target with one
line below it.

The open sibling `2026-09-28-edit-code-insert-rebases-a-mixed-level-body-silently-nesting-top-level-code.md`
is the same mechanism reached through `insert`: a method at the target column plus a
column-0 class. Its root-cause section is still a hypothesis; this reproduces the
mechanism.

## Fix

In `reindent_to`, treat a body whose FIRST code line is already at `target_base` as
already based, whatever the later lines do. The first line is the declaration, the line
that has to land at the target; when it is there the caller has placed the block, and
one uniform shift cannot improve a block whose levels differ.

## Tests added

In `src/util/text.rs::tests`:

- `reindent_to_trusts_a_first_line_already_at_the_target_over_a_shallower_later_line`
  (the reproduced replace shape), `reindent_to_leaves_a_mixed_level_body_alone_when_its_first_line_is_at_the_target`
  (the shape from the sibling insert bug). Both observed RED before the change, with
  the double-indented output the report describes, and GREEN after.
- `reindent_to_still_shifts_when_the_first_line_is_not_at_the_target` bounds the fix
  from the other side: a column-0 first line with a column-0 comment below must still
  be re-based.

Not covered: no test drives `edit_code(replace)` end to end on this shape. The live
binary was not rebuilt, so the original refusal was not re-run against the fix.

## Workarounds

Write the body relative to column 0 so the tool indents it (the agent's own remedy), or
keep column-0 lines out of a pre-indented body.

## Not fixed here

- The tab case: a body indented with tabs re-based onto a space-indented file keeps its
  inner tabs (`    \treturn 40`), mixing units. Parses in Python 3, not sound.
- The misleading "stale LSP range" wording of the sibling-drop refusal.

## References

- `src/util/text.rs` (`reindent_to`, `reindent_block`, `min_indent_outside_literals`)
- `src/tools/symbol/edit_code.rs` (`do_replace`, `do_insert`)
- `docs/issues/archive/2026-06-07-edit-code-no-reindent-nested-symbols.md` (origin of the reindent)
