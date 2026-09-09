---
id: '194220fd8464b68d'
kind: bug
status: open
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

Not applied. Branch the remedy on whether the pusher authors anything in the range, which the
guard already knows:

- **owns ≥1 commit** — current text, unchanged. The refspec advice is correct and load-bearing.
- **owns 0 commits** — do not name a refspec. Route to *ask the author of the rung below*, and say
  explicitly that there is no push of their own to make, so a branch push here publishes only
  someone else's work.

**Do not fix by deleting the refspec sentence** — for a pusher who does own commits it prevents a
real hazard, and register 1's repair now makes it a form the guard can see.


## Tests added

None — nothing is fixed. This is row 5 of the plan in
`tests/pre-push-foreign-session-guard.sh` § *which FIELD names the branch depends on the push
form*, where it is already written up as deliberately absent so no reader credits that block with
covering it.

The assertion, when the branch exists: a refusal shown to a pusher authoring **0** commits in the
range must route to *ask the rung's author* and must **not** contain a refspec form. Pair it with
the ≥1 case still naming one — otherwise the assertion is monotone under the refspec advice being
deleted for everybody, which is the fix this file rejects.

**Why no existing assertion reaches it:** the suite's ~96 assertions are about the guard's
predicate — who is refused. The remedy text is untested by construction, which is the parent bug's
own finding and `CLAUDE.md` § *Testing Discipline*'s named gap.


## Workarounds

Ask the author of the commit below you and let them push it. This is what the guard's *other*
remedy paragraph already says (*"Ask the AUTHOR which of three states they are in"*), so the
correct action is in the text — just not in the branch that a zero-commit reader is steered into.


## Resume

Add the zero-commit branch to the refusal, then write row 5 in
`tests/pre-push-foreign-session-guard.sh` and demand an observed red by reverting the branch.

**Check the third register while you are there:** the refspec sentence is *correct and
load-bearing* for the ≥1 case, so the two repairs must not collide.


## References

- `docs/issues/archive/2026-09-09-a-sha-refspec-push-bypasses-the-foreign-session-guard-which-its-own-remedy-recommends.md`
  — the parent; registers 1–3 enumerated in its `## Fix`
- `tests/pre-push-foreign-session-guard.sh` — row 5, annotated absent
- `docs/issues/2026-09-06-a-push-publishes-commits-their-author-was-withholding.md` — the pathspec
  capture that produces this state, and its instance 2
- `CLAUDE.md` § *Testing Discipline* — a suite tests a guard's predicate and never its remedy text
