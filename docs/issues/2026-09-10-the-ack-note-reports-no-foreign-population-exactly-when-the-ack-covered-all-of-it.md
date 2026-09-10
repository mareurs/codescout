---
id: '28f197a703b6f903'
kind: bug
status: taken
title: 'BUG: the ack note reports "no commits by another session" exactly when the ack covered every one of them'
owners:
- marius
tags:
- cluster/gate-keyed-on-unobservable-event
- git
- guards
---

## Summary

`scripts/pre-push-foreign-session-guard.sh:222` decides which ack note to print by testing
`[ -z "$foreign_report" ]`, and prints:

> note: CODESCOUT_PUSH_ACK was set, but this push carries **no commits by another session** — there
> was no foreign population for it to apply to, and the push was allowed **on that basis, not on the
> ack**. Nothing was authorised because nothing needed authorising.

`$foreign_report` is appended to **only for foreign commits that were NOT acked** — `:174` is
`acked "$sid" && continue`, and the append is at `:179`, after it. So a push in which the ack
matched **every** foreign sid leaves `$foreign_report` empty and takes this branch.

**The note therefore fires exactly when the ack did all of its work, and tells the operator it did
none.**

## Symptom (Effect)

An operator who names every foreign sid — the careful form the guard's own escape-hatch comment
asks for, as against `all` — is told their authorisation was unnecessary and that the push would
have been allowed anyway. It would not: without the ack the guard refuses.

The failure is a **plausible sentence**, not an error, and it argues against the behaviour the
guard wants. A reader who believes it drops the ack next time and meets a refusal, or reaches for
`CODESCOUT_PUSH_ACK="all"` — the blanket form the same file says exists to be avoided *"so the
guard cannot be turned off by habit"*.

## Reproduction

Observed live 2026-09-10 at 11:06:09Z by sessionId `c86ebb51-7ae3-477d-b755-f25db6180782`, on a
real push, not a fixture:

```
CODESCOUT_PUSH_ACK="<6 sids>" git push origin 8fe763ef:experiments
```

- range `c1cb2d9e..8fe763ef` = **52 commits**
- **7 distinct `Session-Id` trailers**, 6 of them not the pusher's; 0 untrailered
- all 6 named in the ack
- guard printed the note above, then allowed the push

Independently enumerated before the push (`git log --format='%(trailers:key=Session-Id,valueonly)'
| sort | uniq -c`): 27 / 14 / 5 / 2 / 2 / 1 / 1 across the seven sids. A second session
(`b80a27d4-…`) re-derived 27 for its own sid and 7 distinct sids in range, matching. So the
population the note denies is corroborated by two instruments and two sessions.

## Environment

`experiments`, 2026-09-10. Present at `scripts/pre-push-foreign-session-guard.sh:222` as of
`8fe763ef`.

## Root cause

The branch needs to answer *"was there a foreign population?"* and instead tests *"is anything
UNACKED left?"* — a proxy that collapses two distinguishable states:

| real state | `$foreign_report` | note printed | correct? |
|---|---|---|---|
| no foreign commits at all | empty | "no foreign population" | yes |
| foreign commits, all acked | empty | "no foreign population" | **no — inverted** |

**The code comment argues for the borrowing explicitly, and the argument is sound about the wrong
question.** It says testing `:228`'s discriminator *"cannot disagree with it: if `-z` were the
wrong question, the guard would already be refusing on the wrong population."* True — and
`foreign_report` is the right discriminator for **whether to refuse**. It is not the discriminator
for **whether an ack applied**, because the ack is precisely what empties it. Two questions, one
variable, and the reasoning transferred the correctness of one to the other.

The comment also asks a future editor to keep the two in sync if `:228` changes. That coupling is
real; it is just not this defect, and having been written makes the branch read as considered.

## Evidence

- `:174` `acked "$sid" && continue` precedes the `:179` `foreign_report` append — read, not
  inferred.
- `:222` `if [ -z "$foreign_report" ]`.
- The live push above: 6 acked foreign sids, note claims none.
- `ack_matched` already holds the answer and is in scope, unused by this branch.

**Not measured:** how often the fully-acked path is taken. It needs a push whose ack names every
foreign sid, which the guard's own text recommends, so it is likely the common form of a
correctly-executed ack rather than an edge case — but that is an argument, not a count.

## Hypotheses tried

- **`IC-2` (`gate-keyed-on-unobservable-event`) — chosen.** The note needs an event it does not
  record (a foreign population *was* present) and substitutes an observable proxy (nothing unacked
  remains) that cannot separate two states. `IC-2`'s falsification clause is *"a member whose proxy
  failure surfaced as an error rather than a plausible result"*; this one surfaces as a confident,
  well-written English sentence, so the clause does not fire.
- **`IC-14` (`guard-narrower-than-its-name`)?** Rejected. The guard's coverage is exactly right —
  it refuses the right pushes and allowed this one for the right reason. The defect is in a note
  about what happened, not in the predicate.
- **`IC-16` (`assertion-that-cannot-fail`)?** Rejected. Nothing here is an assertion; the branch
  fires and produces output, it is just wrong.

## Fix

**TAKEN 2026-09-10 by sessionId `26cb9b5b-2c9c-489e-97d9-3a907c8b2941`**, the branch's author, who
confirmed the defect at the bytes and is fixing it as part of the work their operator directed.
They will use the counter at `:173`, before the ack test, and three arms rather than two: no
foreign population / foreign population fully acked / foreign commits remaining. Do not duplicate
this — the file is contended and a rename sweep is in flight.

**AND THE TEST-SIDE FINDING IS THEIRS, recorded here because it is sharper than anything in this
file and would otherwise live only in a message.** The branch shipped with two new rows, and
*both* test the same state:

| row | fixture | state exercised |
|---|---|---|
| 6a | `ALICE` authors both commits, ack names `BOB` | ack matched **nothing** |
| 6b | ack names `BOB`, `CAROL` authored the foreign commit | ack matched **nothing** |

*"6b was the control I was pleased with and it controlled the wrong axis: population empty versus
not, when the axis that mattered was ack-matched-**all** versus none. Three states, two tested, and
the untested one is the defect."*

That is `CLAUDE.md` § *Testing Discipline*'s population law with a twist worth naming: **a
deliberate control can be a second sample of the same member.** Two rows that differ visibly —
different sids, different authorship shapes — were one observation, because the property they vary
is not the property under test. Widening the fixture would not have found it; enumerating the
states would.

**SHARPENED BY THE SAME AUTHOR AFTER THE FIX LANDED, and the sharper form is the one to carry — it
is a general claim about test design and this file is only where it happens to be written down.**
The nastier property is not that a control can repeat a member; it is that **the fixture varied
convincingly and the state did not.** Rows 6a and 6b differ on *three* visible axes — sid,
authorship shape, and whether a foreign population exists at all — and none of the three is the
axis the predicate branches on. In their words: *"every additional row I could have written by
varying what looked variable would have been a third sample of the same state."*

So *"widen the sample"* is not merely insufficient against this — it is **actively reassuring**,
because each new row looks different from the last. That is the failure mode `CLAUDE.md` records
for the recording-filter law, arriving from the input side instead of the output side.

**The remedy is enumeration OF THE STATES THE PREDICATE CAN SEE, not of the inputs.** Here the
predicate distinguished three — none acked, some acked, all acked — and two rows existed, neither
of them `all`. *"Nothing about the fixtures would have told me that; only asking the predicate what
it distinguishes would."* **Promotion candidate** for § *Testing Discipline* /
`docs/conventions/what-green-is-evidence-for.md`; deliberately not promoted from here, because a
law belongs in that file with its derivation and this is one instance.

**Landed 2026-09-10 by the author, 116 passed / 0 failed.** `foreign_pre_ack_n` is incremented in
the `elif [ "$sid" != "$me" ]` branch **before** `acked "$sid" && continue`, giving three arms:
`foreign_pre_ack_n == 0` keeps the existing wording; `$foreign_report` empty now says *"your ack
authorised N commit(s) by another session, and that is why this push is allowed"* **plus the
mirror** — that it records the pusher's operator's decision, does not speak for the authors', and
leaves each of them exactly as UNCLEARED as they were; otherwise the per-token notes are untouched.

**And the `some` arm proved the split rather than a flip, from a row that already existed.** Row 6b
— ack names `BOB`, `CAROL` authored the foreign commit — reaches the third arm and still passes; a
fix that inverted the branch wholesale would red there. That row was written for a different reason
and earned its keep on an axis its author did not have in mind — the one direction this file's own
finding runs the other way.

**The comment is an aggravating factor rather than a mitigation, and that is the author's own
reading.** The coupling argument was supplied by a third session (`343d53e1-…`) as a stronger
justification for a line already written on weaker grounds, and recorded as a comment without
asking which question it was sound about. So a correct observation about one question was promoted
to a defence of another, by a route that made it *more* credible at each hop.

---

Fix as originally specified, retained because the author adopted it and the reasoning is the
testable part:

Test the thing the sentence claims, not the thing the guard refuses on. `ack_matched` is already in
scope and is non-empty exactly when the ack applied to something:

- `-n "$ack_matched"` → the ack **did** apply; name what it covered. This is the case that
  currently misreports, and it deserves a positive note rather than silence: *"authorised N
  commit(s) from M session(s)"* is the record the ack exists to leave.
- `-z "$ack_matched"` and no foreign commits → the current "nothing needed authorising" text,
  which is correct here.
- `-z "$ack_matched"` and foreign commits remain → unreachable (the guard refuses first), but
  worth an explicit arm rather than falling through a two-way `if`.

**Do not fix by counting `$foreign_sids`** — it is populated after the same `continue` and is empty
for the identical reason. The pre-ack population is not currently recorded anywhere; a counter
incremented at `:173`, before the ack test, is the smallest honest addition. `ack_matched` answers
*did the ack apply* but not *how many were there*, which is why both are wanted.
## Tests added

None by me — the author is writing them with the fix. The shape, unchanged from the original
filing and now sharpened by their row-6 finding:

The discriminating case is a **two-row pair over one fixture**, because either row alone is
monotone:

- foreign commits present, ack names all of them → must NOT say *"no commits by another session"*
- no foreign commits at all, ack set → must say it

Asserting only the first passes under a note that never fires; asserting only the second passes
under today's bug.

**Enumerate the states rather than varying the fixture.** The existing rows 6a and 6b vary sid and
authorship and land on the same state twice; the axis is *how much of the foreign population the
ack covered* — none / some / all — and only the third arm reaches this defect. A row for **some**
is worth having too: it is the only one that can catch a fix which flips the branch wholesale
instead of splitting it three ways.
## Workarounds

Ignore the note when you named sids and the push carried other sessions' commits. Verify with
`git log --format='%(trailers:key=Session-Id,valueonly)' <remote-sha>..<local-sha> | sort -u`
before believing it. **Do not respond by switching to `CODESCOUT_PUSH_ACK="all"`** — that is the
behaviour the note's own file exists to discourage, and this defect argues for it.

## Resume

Add the pre-ack counter at `:173`, branch `:222` on `ack_matched`, write the two-row pair, and
demand an observed red by reverting the branch.

## References

- `scripts/pre-push-foreign-session-guard.sh:164-183, 200-235` — the loop and the note
- `docs/issues/2026-09-10-the-inert-ack-note-and-the-missing-mirror-on-what-an-ack-grants.md` — the
  filing this branch was added to answer; that report's *"missing mirror on what an ack GRANTS"* is
  the same gap seen from the other side, and this file is what the answer to it did
- `docs/trackers/observer-blindness.md` `OB-20` — the class the guard belongs to
- `docs/trackers/bug-fix-session-log.md` `W-122` — the push this was observed on
