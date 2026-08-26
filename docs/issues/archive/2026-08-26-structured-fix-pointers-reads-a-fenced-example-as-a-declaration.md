---
id: '962695c782f41ebe'
kind: bug
status: fixed
title: structured_fix_pointers reads a fenced worked example as a real fix declaration
tags:
- librarian
- doctor
- doc-example-hazard
closed: 2026-08-26
opened: 2026-08-26
owner: marius
related:
- docs/issues/archive/2026-08-26-doctor-reports-a-cross-repo-fix-sha-as-dead.md
severity: low
---

# BUG: `structured_fix_pointers` reads a fenced worked example as a real fix declaration

## Summary

`structured_fix_pointers` scans every line of a bug file for `- **SHA:**`, with no
awareness of fenced code blocks. A file that *quotes* a provenance block — to explain
one, to teach the shape, or to reproduce a defect — has that quotation read as its own
declaration, and the SHA inside it verified as though the file claimed it.

Sibling of the doc-example hazard already documented on `corpus_cited_tokens`, and the
same class `tracker-conventions` handles for `**Valid:**` / `**Rests on:**`: *"a line
inside a fenced code block is skipped — a worked example teaching the syntax is never
mistaken for a declaration."* This parser never got that treatment.

## Symptom (Effect)

Self-demonstrating: `docs/issues/archive/2026-08-26-doctor-reports-a-cross-repo-fix-sha-as-dead.md`
quotes the pointer it is *about* inside a fence, and the scan counted it as a second
declared pointer — `skipped_cross_repo_pointer: 2` where only one file declares one.

Benign in that instance, because the quoted pointer happens to take the same branch as
the real one. It is not benign in general: a file quoting a **dead** SHA as an example
is reported as having a rotted fix pointer, which is the confident-wrong-answer failure
the check's own doc comment takes pains to avoid for freeform prose — *"reporting either
as a dead fix SHA would be a confident wrong answer about a commit the file itself
exonerates."* The prose sweep was rejected for exactly this reason; the fenced case
walked in through the structured parser instead.

## Reproduction

Measured 2026-08-26 across `docs/issues/**/*.md`:

```
declared SHA pointer lines: 75
  outside fences (real declarations): 74
  INSIDE fences (worked examples):     1   ← this bug's sibling file
```

## Root cause

`structured_fix_pointers` (`src/librarian/tools/doctor.rs`) iterates `content.lines()`
and matches `- **SHA:**` / `- **patch-id:**` on every one. Nothing tracks whether the
line sits inside a ``` or ~~~ fence.

## Fix

Shipped. `structured_fix_pointers` now tracks fence state and skips lines inside one.

Safe by measurement, not by assumption: of 75 declared pointer lines in this repo, 74
are outside fences and the only fenced one is the worked example in this bug's sibling
file. Skipping fenced lines removes the false-positive class without losing a single
real declaration.

Confirmed on the live catalog after the change: `skipped_cross_repo_pointer` went 2 → 1
and `scanned` 73 → 72, i.e. the quoted example stopped being counted as a declaration.

## Tests added

`structured_fix_pointers_ignores_a_fenced_worked_example`
(`src/librarian/tools/doctor.rs`), verified red first. The real declaration outside the
fence is the control — without it, "return nothing" passes and the parser is silently
disabled, which is the same shape of vacuous green this file is about.

## Fix provenance

- **SHA:** `7b5325a9`
- **patch-id:** `26ecd2ee0c70fbdef6b4a44bfc10d0c3ebc41714`
## Workarounds

None needed at present — the one live instance is benign. Quoting a real provenance
block in prose rather than a fence would avoid it, which is the wrong trade: fencing is
correct markdown and the parser should tolerate it.

## References

- `docs/issues/archive/2026-08-26-doctor-reports-a-cross-repo-fix-sha-as-dead.md` — found while fixing it
- `corpus_cited_tokens` doc comment (`src/librarian/tools/doctor.rs`) — the same hazard, already documented there
