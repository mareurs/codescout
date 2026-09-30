---
id: 2b8943f405df498a
kind: bug
status: fixed
title: 'BUG: edit_code''s sibling-drop refusal blames a stale LSP range for a failure the tool''s own re-indent caused'
tags:
- cluster/hint-composed-without-the-request
- edit_code
- error-message
closed: 2026-09-30
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

Observed by a peer session (`3e2b9cc8`) on 2026-09-30: an unclosed-paren replace body
(`fn b() -> u8 {\n    (2\n}`) reaches `SyntaxBroken`, not `SiblingsDropped`, and its old
message read "No symbol was dropped, so the edit most likely overshot into adjacent code".
So the misattribution is in both arms, which is why the fix covers both.

Not reproduced end to end: which input reaches `SiblingsDropped` on the current binary now
that the reindent fix stops the original input failing. The message itself is covered by unit
tests on the helper, not by an `edit_code` call.

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


**Corrections and measurements, 2026-09-30, made while fixing it:**

- **Only `replace` is affected, not `remove`.** The paragraph above says both sites carry
  wording that must change. `remove` takes no body, so a stale or overshooting range is the
  plausible cause there and its messages are right. In `replace` the caller supplied the
  body, so it is the first suspect. Two arms assert an unsupported cause: `SiblingsDropped`
  ("likely a stale LSP range") and `SyntaxBroken` ("the edit most likely overshot ... took a
  delimiter with it"). A peer session independently reproduced the second with an unclosed
  paren.
- **`syntax_regressed` is false for the input that started this.** Measured on the real
  function, in an isolated worktree, on Python: the pre-fix output of the original report (a
  column-4 body with one column-0 comment, everything else shifted) gives `false`; a dedent
  to a level no enclosing block has gives `false`; a consistent over-indent gives `false`
  (correct); an unclosed paren gives `true`. tree-sitter-python does not flag indentation
  errors. So a message that changes only when the flag is true would have left "stale LSP
  range" in place for the very case that motivated this bug. The guard's blind spot was filed
  as `docs/issues/archive/2026-09-30-edit-code-syntax-guard-accepts-python-indentation-errors-tree-sitter-does-not-flag.md`
  and is not fixed here (it was fixed afterwards, separately).

## Fix

In `src/tools/symbol/edit_code.rs`, `do_replace` now builds its two affected messages
through pure helpers, so they can be tested without an LSP:

- `replace_siblings_dropped_reason(name_path, dropped, syntax_regressed)`. With the flag
  true, the message says the file stopped parsing (the phrase the telemetry classifier keys
  on for `edit_would_break_syntax`), names the lost symbols, and ranks the replacement body
  first and the range second. With the flag false it still ranks the body first and the
  range second, and keeps the `would have dropped sibling symbols` phrase so the failure
  stays in the `replace_dropped_sibling` family. The false branch is deliberately NOT the
  old text: the flag being false does not clear the body (see Root cause).
- `replace_syntax_broken_reason(name_path)`: body first, range second.
- The hints send the caller to the body before any index refresh, and the refresh step is
  kept for the case where the body is sound.
- `do_remove`'s messages are unchanged, for the reason in Root cause.

Option (c) from the earlier draft (reorder `corruption_verdict` so `SyntaxBroken` outranks
`SiblingsDropped`) stays rejected: the docstring's own case, a genuinely vanished symbol with
a broken parse as the consequence, would lose its actionable message.

**Wording is part of a contract.** `usage::db::normalize_err_family` routes on substrings of
these messages. The fix was written against it and the tests pin the resulting family, so a
rewording that drops a phrase moves failures between telemetry families without any test
about the message itself going red.

Fixed in `62a6b903` on `experiments`.

Gate evidence: one `gate.sh` run on the final tree exited with `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`, and the new tests appear as `ok` in both lanes. The tree it ran on carried peers' uncommitted work, which compiled. The live binary has NOT been rebuilt with this change, so no `edit_code` call has exercised the new wording.

## Fix provenance

- **SHA:** `62a6b903` (`experiments`)
- **patch-id:** `db49d215a6be844f2b99a14505c014bc914d57d0`

## Tests

Three tests in `src/tools/symbol/tests.rs`, written against the OLD wording first and
observed RED (two failed, the third, a since-rewritten bound, passed):

- `replace_names_the_body_first_when_an_unparseable_edit_also_lost_siblings`
- `replace_names_the_body_before_the_range_even_when_the_syntax_check_is_clean`
- `replace_syntax_broken_names_the_body_before_the_range`

They pin which cause a message names, in what order, and which telemetry family it lands in,
not its sentences. Ordering assertions search for the phrase `replacement body` and not for
`body`, because the fallback clause ("If the body is sound") satisfies the latter and would
let the instruction move unnoticed.

**One test's premise was wrong and was rewritten.** The first bound test asserted that with
`syntax_regressed` false the old stale-range wording must stay, on the belief that false meant
a genuine overshoot. The measurement in Root cause disproved that. It is now the second test
above, which pins the body-first ordering for the false branch.

**Mutation results**, run with `scripts/mutation-probe.sh` in isolated worktrees, every
mutation killed by the test built for it: guard forced false, guard forced true, each range
fallback clause removed (true branch, false branch, and the `SyntaxBroken` helper), each
body-before-range ordering swapped (all three messages), the syntax phrase removed from the
true branch, the `dropped sibling` phrase removed from the false branch, and the hint's body
advice and refresh step removed. Nine mutations were run on the first version of the helper
and ten on the revised one, after the false branch changed; the two sets were not
deduplicated against each other.

The probe printed `patch did not apply (still 1)` on the false-branch ordering mutation, in
several attempts. That line is the probe's own post-check (`scripts/mutation-probe.sh`, the
`after` count) that the `--find` literal is gone after the mutation, and my `--replace` text
ended with the whole `--find` text, so the literal was always still present. It was my
mutation design and not the probe or the tree. It was first misdiagnosed here as a failure of
the carried working tree, and that diagnosis was wrong: a rewrite whose replacement does not
contain its own find-text ran on the same tree and was killed by the ordering assertion.

## Workarounds

Read your own body first. If it is a nested symbol, check its indentation and its brackets
before refreshing the symbol index.

## References

- `src/tools/symbol/edit_code.rs` (`do_replace`, `do_remove`, `CorruptionVerdict`)
- `docs/issues/archive/2026-09-30-edit-code-reindent-takes-its-base-from-the-shallowest-line-so-one-column-0-line-double-indents-a-pre-indented-body.md` (the case that exposed it)
