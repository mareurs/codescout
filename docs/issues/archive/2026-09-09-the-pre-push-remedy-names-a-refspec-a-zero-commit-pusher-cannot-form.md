---
id: 84589e9e5a644ec7
kind: bug
status: fixed
title: 'BUG: the pre-push remedy names a refspec a zero-commit pusher cannot form, and steers them to the branch push it warns against'
owners:
- marius
tags:
- cluster/hint-composed-without-the-request
- git
- guards
---

## Summary

`scripts/pre-push-foreign-session-guard.sh`'s refusal prescribes *"Use a refspec at EVERY rung —
pushing the branch name publishes the whole stack including commits above you:
`git push origin <your-sha>:experiments`"*.

**A pusher who owns no commit in the range has no `<your-sha>` to substitute.** The advice is
composed from the *response* — a stack exists, here is its ladder — and not from the *request*:
nothing in it consults whether the reader authored anything in the range. A reader in that state
follows the instruction, finds no sha of their own, and the natural next move is the branch form
the same paragraph warns against.

This is **register 2 of three** identified on the parent bug
`docs/issues/archive/2026-09-09-a-sha-refspec-push-bypasses-the-foreign-session-guard-which-its-own-remedy-recommends.md`,
and the one that **survives that bug's fix**. Register 1 (the sha form bypassing the guard) is
fixed and archived; a reader of that archived file will see *fixed* while this remains live.


## Symptom (Effect)

The refusal's own remedy is unfollowable for a whole class of readers, and fails toward the
hazard: it routes them to `git push origin <branch>`, which publishes every commit beneath them —
exactly what the sentence exists to prevent.

No error, no warning. The reader reads correct-looking advice, cannot apply it, and improvises.


## Reproduction

Observed live 2026-09-09 by sessionId `26cb9b5b-2c9c-489e-97d9-3a907c8b2941`, in that state on a
real push:

Their own ledger writes (`F-129`, `W-120`) had been swept into **another session's** pathspec
commit on a shared tracker file, so their unpushed range held exactly one commit, authored by
`b80a27d4-9729-40ef-8c28-ad8982df6d13`, and **none of it was theirs**. The refusal named the
ladder correctly and told them to use a refspec at their rung. They had no rung and no sha.

The state is not exotic — it is the ordinary consequence of a peer committing by pathspec over a
shared ledger, which is itself a filed defect
(`docs/issues/2026-09-06-a-push-publishes-commits-their-author-was-withholding.md`, and the
staging-window bug beside it).


## Environment

`experiments`, 2026-09-09. Present in the guard's refusal text at the `## Fix`-cited lines
regardless of the register-1 repair, which changed only the stdin field filter.


## Root cause

The remedy is generated from the guard's computed **ladder** — which commits exist, whose they
are, which rung is lowest-clear — and the ladder is a property of the *range*. Whether the
**reader** owns a commit in it is a different question that the message never asks, even though
the guard already holds the answer: it has the trailers and the pusher's own
`CLAUDE_CODE_SESSION_ID`, so it can determine "you author 0 of these N" before composing a
sentence that presumes otherwise.

That is `IC-22` exactly: the hint is composed from the response shape, not from the request.


## Evidence

- The prescribed command string contains `<your-sha>`, a placeholder with no binding for a
  zero-commit pusher.
- The guard computes and prints `Your session id: <you>` in the same banner, so the input needed
  to branch the advice is present and unused.
- Live instance above: one commit in range, zero authored by the pusher, remedy names a refspec.

**Not measured:** how many pushes land in this state. It requires a peer's pathspec commit to have
captured the pusher's staged work, which is itself unmeasured. Stating the frequency would be
inventing a denominator — the same refusal this corpus made for the parent bug's coverage ratio.


## Hypotheses tried

- **`IC-14` (`guard-narrower-than-its-name`)?** Rejected. The guard's *coverage* is correct here —
  it refuses exactly the right pushes. What is wrong is the *message*, and tagging it there would
  move a class whose claim is about predicates on a defect about prose.
- **Part of the parent bug?** It was, and that is the problem: the parent's register 1 is fixed, so
  the parent archives, and this would archive with it while still being live. Split so the live
  half has a live file.
- **`cluster/unclassified`?** Considered and not needed — `IC-22`'s claim text fits without
  forcing, and the escape hatch is for when nothing does.


## Fix

Applied. The guard counts the pusher's own commits in the range (`mine_n`, incremented in the
same scan that builds `commit_rows`) and branches the remedy paragraph on it, into `$remedy`,
before the banner heredoc:

- **owns >= 1 commit** — the previous text verbatim, refspec advice included. It is correct
  and load-bearing there, and register 1's field-3 repair is what finally lets the guard see
  that push form.
- **owns 0 commits** — no refspec is named. The banner states the count with its unit (*"YOU
  AUTHOR 0 OF THE N COMMIT(S) IN THIS PUSH"*), says there is nothing of theirs to send, routes
  to *ASK THAT AUTHOR TO PUSH IT THEMSELVES*, and names the branch push as the move the state
  invites and the one to refuse.

The three-state enumeration is emitted **outside** `$remedy` and therefore reaches both
branches — which is the answerability half rather than the arrival half, and the reason the
zero-commit route names a party whose reply has a branch the reader can use.

**`docs/RELEASE.md` § *Publishing a stack several sessions wrote* was changed in the same
commit, and that half is not optional.** The banner's closing line cites that page for the
derivations, so a remedy branched in the guard and unconditional in the page sends a reader
from a correct refusal straight to the sentence the refusal was rewritten to avoid. Branching
one surface and not the other would have left the defect reachable by the guard's own
footnote.
## Tests added

Row 5 of `tests/pre-push-foreign-session-guard.sh` § *which FIELD names the branch depends on
the push form*, which previously carried a `DELIBERATELY ABSENT` annotation. Now 8 assertions
in two rows over **one fixture with one variable**: 5a and 5b push the identical range and
differ only in `$me`, so nothing about the commits, the ladder or the rung moves between them
and any difference in the banner is attributable to the pusher alone.

- **5a (owns 1 of 2)** — refused; `git push origin <your-sha>:main` still present; does not
  say *"YOU AUTHOR 0 OF"*.
- **5b (owns 0 of 2, same range)** — refused; contains no `<your-sha>:`; states
  `0 OF THE 2 COMMIT(S)`; routes to *ASK THAT AUTHOR TO PUSH IT THEMSELVES*; the three-state
  enumeration reaches it.

**Observed RED, both directions, 2026-09-10** — mutating the *production* path, on a copy of
the guard so the live one was never briefly wrong on a tree several sessions share. Forcing the
branch always-true (the pre-fix behaviour) reds 5b's three assertions and leaves 5a green;
forcing it always-false reds 5a's two and leaves 5b green. Each direction has its own witness,
which a single-sided pair cannot have — and 5a exists precisely because `hasnt <your-sha>:`
alone is monotone under deleting the refspec advice for everybody, the fix this file rejects.

**One assertion is INERT for the branch and is annotated as such on its own line:** *"the three
states reach it"* survives both polarities, because that block is emitted unconditionally
outside `$remedy`. It is kept as a guard against a future edit moving the enumeration *into*
the `mine_n >= 1` branch — the regression `OB-20`'s ceiling predicts — and not credited with
covering the branch itself.

Suite: **104 passed, 0 failed** (was 96).
## Workarounds

Ask the author of the commit below you and let them push it. This is what the guard's *other*
remedy paragraph already says (*"Ask the AUTHOR which of three states they are in"*), so the
correct action is in the text — just not in the branch that a zero-commit reader is steered into.


## Resume

Nothing owed. **Register 3** — the third register enumerated on the parent bug — was checked
while here and did not collide: the refspec sentence stays intact and unedited on the `>= 1`
path, which is what the parent's `## Fix` asked.
## References

- `docs/issues/archive/2026-09-09-a-sha-refspec-push-bypasses-the-foreign-session-guard-which-its-own-remedy-recommends.md`
  — the parent; registers 1–3 enumerated in its `## Fix`
- `tests/pre-push-foreign-session-guard.sh` — row 5, annotated absent
- `docs/issues/2026-09-06-a-push-publishes-commits-their-author-was-withholding.md` — the pathspec
  capture that produces this state, and its instance 2
- `CLAUDE.md` § *Testing Discipline* — a suite tests a guard's predicate and never its remedy text
