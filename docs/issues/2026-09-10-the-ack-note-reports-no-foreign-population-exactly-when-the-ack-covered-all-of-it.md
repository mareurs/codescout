---
id: '28f197a703b6f903'
kind: bug
status: open
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

Not applied. Test the thing the sentence claims, not the thing the guard refuses on. `ack_matched`
is already in scope and is non-empty exactly when the ack applied to something:

- `-n "$ack_matched"` → the ack **did** apply; name what it covered. This is the case that
  currently misreports, and it deserves a positive note rather than silence: *"authorised N
  commit(s) from M session(s)"* is the record the ack exists to leave.
- `-z "$ack_matched"` and no foreign commits → the current "nothing needed authorising" text,
  which is correct here.
- `-z "$ack_matched"` and foreign commits remain → unreachable (the guard refuses first), but
  worth an explicit arm rather than falling through a two-way `if`.

**Do not fix by counting `$foreign_sids`** — it is populated after the same `continue` and is empty
for the identical reason. The pre-ack population is not currently recorded anywhere; a counter
incremented at `:173`, before the ack test, is the smallest honest addition.

## Tests added

None — nothing is fixed. The suite has the fixtures for this: `run <pusher> <ack> <stdin-line>`
already takes an ack argument and `hasnt` already exists. The discriminating case is a **two-row
pair over one fixture**, because either row alone is monotone:

- foreign commits present, ack names all of them → must NOT say *"no commits by another session"*
- no foreign commits at all, ack set → must say it

Asserting only the first passes under a note that never fires; asserting only the second passes
under today's bug. The pair is what discriminates, which is the same shape as row 5's
`mine_n` pair added earlier the same day.

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
