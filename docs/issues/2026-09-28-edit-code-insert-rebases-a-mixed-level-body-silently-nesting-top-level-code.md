---
kind: bug
status: open
tags:
- cluster/addressing-without-an-escape-hatch
closed: null
opened: 2026-09-28
owner: marius
related: []
severity: medium
---

# BUG: edit_code insert re-bases a mixed-level body onto the target's column, silently nesting top-level code

## Summary

`edit_code(action="insert", position="after")` re-bases the whole inserted body
onto the target symbol's indentation column. That behaviour was added by the fix
for the archived bug `2026-06-07-edit-code-no-reindent-nested-symbols`. When the
body mixes levels, for example two methods (indented) plus two top-level classes
(column 0), and the target is a method, the whole block shifts by the method's
indent. The top-level classes then become nested inside the class that holds
the method.

In Python that is still VALID syntax, so nothing fails: the new classes are
simply not collected as tests. The caller has no way to write "these lines are
top-level" in an insert body. That is the no-escape half of IC-6.

## Symptom (Effect)

Reported by an SDD implementer on the system1 measurement plan, Task 13 fix
round 3 (session `3c5b02df-b6ce-45f5-9d03-1194e38465c0`), in
`tests/test_measure_observability.py`. After the insert, the new top-level
classes came out indented 4 extra spaces, inside a test method. pytest
collected 48 tests instead of 54 and reported "48 passed". The implementer
caught it only by comparing the test count, and fixed the indentation by hand.

## Reproduction

NOT YET REPRODUCED by the controller. The implementer's report describes the
shape. To reproduce, in a Python test file:

1. Pick a class `A` whose method `A.test_x` is the target.
2. Insert, with `position="after"`, a body holding one method at 4-space indent followed by `class B(unittest.TestCase):` at column 0.
3. Check B's column after the insert.

Expected on this reading: `B` lands at column 4 or deeper, nested in `A`.


**Reproduced 2026-09-30** on the rebuilt binary, against a scratch Python class, inserting
after the class's last method. Two inputs, and they split the bug:
- A body whose first line is at the target column, then a column-0 class: `class B` stays
  at column 0. Fixed by `2da4e2fe`.
- A body whose first line is at column 0, then a column-0 class: everything is shifted,
  the method lands at column 4 and `class C` nests inside the enclosing class. The file
  parses, so nothing refuses it. **This is what remains open.** Option (a) in the Fix
  section does NOT cover it, because no line here is shallower than the first line (see the
  correction there).

## Environment

codescout `experiments` circa 2026-09-28; the Python LSP; a shared checkout.

## Root cause

Hypothesis, to be confirmed by the reproduction: `reindent_to` (src/util/text.rs)
shifts the body's minimum indent to the target's column, and applies that one
shift to every line. A body whose lines belong to different nesting levels
cannot be placed correctly by one shift, so the tool has to either refuse it or
place each level. Today it does neither, and reports success.

## Evidence

The implementer's report (the SDD workspace, gitignored), the Task 13 report's § "Fix round 3",
Concern 2.

## Hypotheses tried

None yet.

## Fix

Options, to be decided after reproducing:
- (a) REFUSE, with a RecoverableError, an insert whose body contains a line indented LESS than its first line. That signals a multi-level body, and the error would tell the caller to insert each level against its own target.
- (b) Insert relative to the target's PARENT when the body's first line is at column 0 and is a class or def.

Option (a) is the conservative one. It turns a silent wrong placement into a loud refusal, and the remedy it names is something the caller can do.


**Update 2026-09-30 (partial, supersedes the framing above).** Option (a) as written
would also refuse a body whose first line is ALREADY at the target column, which is
the shape reported here (a method at the target's column, then a column-0 class). That
shape needs no refusal: `reindent_to` now returns a body unchanged when its first code
line is at the target column, so `class B` keeps column 0. See
`docs/issues/archive/2026-09-30-edit-code-reindent-takes-its-base-from-the-shallowest-line-so-one-column-0-line-double-indents-a-pre-indented-body.md`.
Verified at the `reindent_to` function, and now end to end: on 2026-09-30, on the rebuilt
binary, `edit_code(insert)` after the last method of a class with a body holding a method
at the target column and a column-0 class placed the class at column 0, top-level.

What stays open: a body whose first line is NOT at the target column and which holds
several items at that same shallow column, for example a column-0 method plus a column-0
class, inserted after a nested method (verified live 2026-09-30: the class ends up nested
inside the enclosing class and the file still parses). One shift cannot place both
levels, and nothing in the text says which the caller meant: "both are members of the
class" and "both are top-level" read identically.

**Correction, same day, to the line this replaces.** It said option (a) "still applies" here.
It does not: (a) refuses a body with a line indented LESS than its first line, and in this
case no line is shallower, because the first line and the class are both at column 0. Rule
(a) as written would let this case through. Whatever is decided has to be a different rule,
or an explicit caller switch, and it has to avoid refusing the ordinary case the re-base was
built for: two sibling methods dedented to column 0 and meant to become members.

## Tests added

(pending)

## Workarounds

Insert each nesting level separately, against a symbol at that level. For
top-level code, insert after a top-level symbol. After any insert into a test
file, check the collected-test count.

## Resume

Reproduce first, then choose between (a) and (b).

## References

- `src/tools/symbol/edit_code.rs` (`do_insert`)
- `src/util/text.rs` (`reindent_to`)
- the origin fix: `docs/issues/archive/2026-06-07-edit-code-no-reindent-nested-symbols.md`
