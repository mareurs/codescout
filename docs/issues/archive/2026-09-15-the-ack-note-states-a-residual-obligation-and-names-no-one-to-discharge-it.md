---
kind: bug
status: fixed
tags:
- cluster/unclassified
closed: 2026-09-15
opened: 2026-09-15
owner: marius
related: []
severity: medium
---

# BUG: the ack note reports how many foreign commits it authorised, never which sessions, so the obligation its last sentence states cannot be acted on

## Summary

On a fully-acked push `scripts/pre-push-foreign-session-guard.sh` prints:

> note: your ack authorised **N** commit(s) by another session, and that is why this push is
> allowed — every foreign author in the range was named. The ack records YOUR operator
> decision about them; it does not speak for theirs, and **it leaves each of them exactly as
> UNCLEARED as they were.**

That last clause states a **residual obligation** and names no party, no action, and no
count of parties. `ack_matched` holds the matching sids at exactly that point and is not
printed. So the reader is told a state persists and given nothing to do about it.

This is the repo's own remedy-text law one step further on. `CLAUDE.md` § *Testing
Discipline* says to name the next action a guard's message produces and ask whether that
party can perform it. Here the message produces **no** next action, which is the case the
law does not cover: it is not that the addressee cannot answer — it is that there is no
addressee.

## Symptom (Effect)

The pusher reads a true, well-written sentence, agrees with it, and does nothing. Nobody is
told their work was published.

## Reproduction

Measured 2026-09-15, and the reporter is the instance.

Push `56f33bd2` at 07:45 carried **33 commits across four sessions** — 9 mine, 12
`aa272bed`, 7 `d52899fd`, 5 `f0b1a4c7`. The ack named all three foreign sids explicitly
rather than `all`. The guard printed the note above with `24`, correctly.

**Three sessions had work published. Zero were notified.**

It surfaced ~3 hours later when `aa272bed` noticed `origin/experiments` had moved and asked
whether it was me — having inferred it from a passing reference to the pushed sha in an
unrelated message about a different fix. They then had to reconstruct the range themselves,
and undercounted their own commits as 5 when 12 were carried, because reconstructing from
the outside is exactly the work the note could have eliminated.

Their standing instruction, like every session's, is *push only when the user asks*, and
their operator had not asked. So they had to surface to their operator that their work had
reached `origin` without their operator's sanction — a report assembled from inference.

## Root cause

Not read beyond the print block. `ack_matched` (`:85`, appended at `:103`) holds the
comma-separated sids that matched; the `elif [ -z "$foreign_report" ]` branch at `:337`
prints `foreign_pre_ack_n` alone. The identity is computed, held in scope, and discarded at
the print — the same shape as `doc(action="move")` discarding which scan matched
(`eefbf76ba062b263`, fixed earlier today), one layer up.

## Why "be careful" is the wrong instrument

The pusher is the one party who cannot notice the omission: they know who is in the range —
they derived it to write the ack — so the note reads as complete to them and only to them.
Every other reader of that output is a session that is *not* being told. `CLAUDE.md`
§ *Observer Blindness* position 3: the check has to run when nobody is worried.

**I read this note, quoted its UNCLEARED sentence back to my operator approvingly, and still
did not draw the conclusion inside it.** Knowing the class prevented nothing, which is that
section's own claim about itself.

## Fix

**SHIPPED `ab735e4a`**, patch-id `0108c2d1ef0c7e1805eb70178b2ab287c7d32360`.

The branch now names each acked sid, states that they have not been told, points at
`/codescout-companion:reaching-peer-sessions` to resolve a sid to a live session, and says
**why** it is owed rather than only that it is. The sid is printed and the route is not:
pid, socket and registry name all decay, and a git hook cannot know who is alive.

**Not a refusal, and placed after the push is allowed.** The push is correct and
authorised; this is a courtesy owed afterwards, and blocking on it would punish the exact
path the guard exists to encourage — naming every sid rather than reaching for `all`.

### A bug in the fix, caught by its own test before it shipped

`printf '%s'` emits no trailing newline, so `tr` hands `while read` a final unterminated
line and the loop **silently drops the last sid**. That is this bug one level down: a list
of who is owed a notification, quietly omitting one of them. Fixed to `printf '%s
'`.


## Tests added

Seven assertions on row 6f of `tests/pre-push-foreign-session-guard.sh`, plus two
discrimination rows on 6a and 6b. 129 passed, 0 failed.

Mutations, against an isolated copy of the **whole** `scripts/` directory:

| mutation | result |
|---|---|
| N1 drop the notify header | **KILLED** — "have not been told" |
| N2 revert the trailing newline | **KILLED** — "names the first acked sid" |
| N3 drop the why-it-is-owed clause | **KILLED** — "says why it is owed" |
| N4 drop the procedure pointer | **KILLED** — "names a procedure" |
| N5 make the block unconditional | **KILLED** — the discrimination row |

**N5 is what shows rows 6a/6b are not padding**: a notify block printed unconditionally
would tell a *refused* pusher to notify sessions whose work never went out.

**The first mutation attempt was invalid and its CONTROL is what caught it.** Copying only
the script and its test left the suite missing an installer and generator it needs; the
control failed **26**, and two mutations then returned clean-looking "27 failures" that
meant nothing. Copying all of `scripts/` brought the control to 129/0. Without running the
control first, both readings would have been recorded as kills.

## Gate

Shell-only change; `grep` of `src/` and `tests/*.rs` for this script and `PUSH_ACK` is
empty, so the shell suite is the relevant gate. `./scripts/gate.sh` was **deliberately not
run**: a peer had `src/tools/run_command/*` and `output_buffer.rs` uncommitted in the
shared tree at the time, so its lanes would have compiled their in-flight work and a red
would not have been attributable to either of us.

## References

- `docs/issues/archive/2026-09-10-the-ack-note-reports-no-foreign-population-exactly-when-the-ack-covered-all-of-it.md`
  — the sibling that produced this branch. That one had the note firing on the wrong
  condition; this one is the branch being right and incomplete. Its fix is what created the
  sentence this bug is about.
