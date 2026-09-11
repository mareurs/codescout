---
id: '447d98db54393338'
kind: bug
status: open
title: 'BUG: edit_file reads a definition keyword in a COMMENT as a symbol definition, with no escape'
tags:
- cluster/addressing-without-an-escape-hatch
---

## Summary

`edit_file`'s IL-2 guard refuses an edit whose text contains a symbol-definition keyword
(`class `, `fn `, `def `) **regardless of whether the occurrence is code**. An edit that only
touches a comment is refused when the comment's prose happens to use the word — and in a repo
whose defect vocabulary is literally *defect class*, `cluster/<slug>` and "a class gained a
member", that prose is everywhere.

There is **no escape**: no flag, no fence, no quoting form makes the token a mention rather than
a definition. That is `IC-6`'s first half exactly — a parser over a namespace that makes some
inputs unrepresentable.

## Symptom (Effect)

```
edit_file(path="scripts/pre-commit-ledger-counts.py",
          old_string="    # CHECK 2 -- a class that GAINS a member must say something about it.",
          new_string="    # CHECK 2 -- ... \n    # ... a wrong class corrupts the counts ...")
-> edit contains a symbol definition ("class ") — use symbol tools for structural changes
```

Both strings are Python **comments**. Nothing in the edit defines anything.

The hint routes to `edit_code`, which cannot serve this edit either: `edit_code` addresses whole
symbols, so inserting eight comment lines in the middle of a 150-line `main()` means replacing
`main()` wholesale — reproducing its entire body to change a comment, which is the larger and
riskier edit the guard was meant to prevent. **The refusal's remedy is more dangerous than the
act it refuses**, which is the `CLAUDE.md` § *Testing Discipline* point about naming the next
action a guard's message produces.

## Reproduction

1. `edit_file` any `.py` or `.rs` file in this repo.
2. Make `new_string` a comment line containing the word `class ` followed by a space — e.g.
   `# every defect class gains members over time`.
3. Observe the refusal, with no accepted spelling available.

Control, confirming the guard reads the EDIT TEXT and not the file: the same prose is accepted by
the native `Edit` tool, and by `edit_file` if the word is rephrased.

## Environment

codescout MCP, 2026-09-11, `experiments`. Hit twice in one task: once on `class ` in a Python
comment, once earlier on `fn ` while legitimately adding a function (that one was correct).

## Root cause

Not read at the bytes — the observable behaviour is a substring test over the edit text with no
comment/string-literal awareness. The correct scope of the guard is *"does this edit change a
symbol's definition"*, and the implemented predicate is *"does this edit text contain a keyword"*,
which is wider in exactly the direction prose lives. `IC-14`-shaped in that sense, though filed
under `IC-6` because the absent **escape** is the part that makes it unrepresentable rather than
merely over-broad.

## Fix

Not chosen. Sketches, cheapest first:

1. **Require the keyword to begin a line** (after indentation) before treating it as a
   definition. A definition always does; prose almost never. Cheap, no parser, and would have
   accepted both refused edits here.
2. **Skip comment and string spans** when scanning, per language. Correct, more work, and needs
   the same comment-span logic on every language the tool edits.
3. **Give the caller an escape** — an explicit `allow_symbol_text: true`, or honouring the fact
   that `old_string` and `new_string` both sit inside a comment. An escape is what `IC-6` says a
   parser over a namespace owes, and per `CLAUDE.md` § *Parsers Over a Namespace*, where no
   escape is affordable the limitation must be stated **at the refusal site**.

Shape 1 plus a sentence in the refusal naming the limitation would close the reported cost.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*

## Workarounds

Native `Edit`, which has no such guard — used for the refused edit in this task. Rephrasing the
prose also works and is worse: it lets a tool defect edit the documentation's wording.

## Tests added

None yet.

## Resume

Run the reproduction before choosing between the three shapes — in particular check whether the
guard reads `new_string` only or both strings, because shape 1's cost depends on it and the
§ Symptom transcript cannot distinguish them.

## References

- Hit 2026-09-11 while fixing
  `docs/issues/archive/2026-09-09-the-pre-commit-cluster-hook-enforces-a-subset-of-the-gate-it-mirrors.md`.
- `CLAUDE.md` § *Parsers Over a Namespace* — "how does a caller write this token literally", and
  the instruction to state an unaffordable escape at the refusal site.
