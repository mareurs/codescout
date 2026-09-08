---
id: '822ac8e9cc88ec01'
kind: bug
status: open
title: 'BUG: the archive-move confirmation signal is a rename LETTER, so a correct staged D+A matches neither documented outcome'
tags:
- cluster/selector-narrower-than-its-population
- librarian
- git
- archive-flow
- doc-vs-reality
opened: 2026-09-08
owner: marius
severity: low
---

## Summary

Both the `move` response's `stage_hint` and `get_guide("tracker-conventions")` tell an
archiver to *"confirm that `git status --short` shows a single `R` rename line"*, and name
exactly one failure shape beside it: *"a ` D` plus a `??` is half-staged."*

There is a **third** outcome, and it is correct: staged `D` + staged `A`, with no `R`. Git's
rename detection is **similarity-based**, so an archive move that also rewrites the body —
which is the normal case, since archiving means writing the outcome, the fix SHA and the
patch-id into the file — falls below the default 50% threshold and is reported as an
unrelated delete plus add.

The reader is then holding an outcome that matches neither documented state, with no way to
tell it from the broken one, because **the discriminator they were given is the letter and
the letter is what stops discriminating.**

## Symptom (Effect)

A correct, complete archive reads as unconfirmed. The two readings a follower of the
instruction can reach are *"`R`, good"* and *"no `R`, something is wrong"* — and the genuinely
half-staged case also has no `R`. Both non-`R` states look alike under the stated check.

The cost is not a corrupted archive; it is a reader who either re-stages something already
correct, or — worse — learns that "no `R`" is survivable and stops treating it as a signal at
all, which is the state in which the real half-staged case slips through.

## Reproduction — measured 2026-09-08

Archiving `2026-09-05-doc-update-body-appends-a-trailing-blank-line-every-write.md` at
`f7d61237`, having first written the outcome into the body (140 insertions, 94 deletions):

```
git status --short -- <old> <new>
D  docs/issues/2026-09-05-doc-update-body-appends-a-trailing-blank-line-every-write.md
A  docs/issues/archive/2026-09-05-doc-update-body-appends-a-trailing-blank-line-every-write.md
```

Both staged (column 1), no `R`. The commit landed both halves — `delete mode` + `create mode`,
2 files changed. The archive is correct.

Git's own view, with the threshold lowered:

```
git show --raw f7d61237                      → D + A          (default, 50%)
git show --raw --find-renames=25% f7d61237   → R044 <old> <new>
```

**Similarity 44%.** Six points under the default. Nothing is wrong with git; the check was
written against the case where a move does not touch the body.

## Root cause

The confirmation signal was chosen as a *letter* (`R`) when the property that actually
matters is *staged-ness*. `R` is one rendering of "both halves are staged", produced only
when git also happens to infer a rename — an inference that depends on how much the body
changed, which the archiver controls and the check does not mention.

## Where it is stated

- **Code:** `src/librarian/tools/mv.rs:251-256` — the `stage_hint` string, emitted on every
  move.
- **Guide:** `get_guide("tracker-conventions")` § *Bug files*, the "stage BOTH halves"
  paragraph.

Both were written together and say the same thing, so a reader who cross-checks the guide
against the tool's own hint finds agreement. Two surfaces, one claim, no independent check.

## Why the existing test cannot catch it

`mv.rs`'s `move_names_the_staging_action_not_only_the_two_paths` asserts
`hint.contains("git add --")` — the **imperative**, deliberately, because its own comment
records that data presence did not produce the action across six consecutive opportunities.

That assertion is **monotone under this defect**: the hint could describe the confirmation
step in any way at all, or wrongly, and still contain `git add --`. The test guards the half
that was failing then and says nothing about the half failing now. This is CLAUDE.md's
predicate-vs-remedy split one level in: the test pins *what to do* and never *how you know it
worked*.

## Fix

State the discriminator as staged-ness rather than as a letter, in both surfaces:

> `git status --short` shows either one `R` line, or a `D` and an `A` **both in column 1**.
> A leading space (` D`) or a `??` is half-staged.

Cheap, and it makes the check true for every archive move rather than for the subset that
leaves the body alone. A test asserting the hint names the *column* rather than only the
letter would red on the current text and cannot be satisfied by rewording.

Considered and rejected: passing `--find-renames=25%` in the instruction. It makes the `R`
appear, but it teaches a threshold that has no principled value, and the next archiver whose
body rewrite is larger falls under 25% too.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*

## Severity

Low. No archive was corrupted; this one committed correctly. It is filed because the
instruction's *failure* mode is silent agreement — the reader sees an outcome the check does
not describe, and the most available resolution is to stop trusting the check.

## Resume

Unclaimed. Noticed while archiving a bug in the ordinary way, and the ordinary way is what
triggers it: writing the fix SHA and outcome into a bug file before moving it is exactly what
pushes similarity under the threshold.

## References

- The half-staged case the current wording exists to prevent, and which is still real:
  `docs/issues/archive/2026-09-02-tracked-only-staging-commits-half-an-archive-move.md`
- The move that produced the measurement: `f7d61237`.

