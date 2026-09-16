---
kind: bug
status: fixed
tags:
- cluster/selector-narrower-than-its-population
closed: 2026-09-16
opened: 2026-09-15
owner: marius
related: []
severity: high
---

# `CODESCOUT_PUSH_ACK=all` publishes every foreign session's work and tells none of them

## Summary

`scripts/pre-push-foreign-session-guard.sh` prints, for an ack that authorised foreign
commits, a note naming **which sessions have not been told** their work is now public. That
note is unreachable on the `all` form — the highest-blast-radius ack path, and the one the
script's own header says exists to be avoided.

Two independent failures, and either alone is sufficient:

1. **The block is gated off.** `:309` is `if [ -n "$ack" ] && [ "$ack_matched" != "all" ]`,
   and `acked()` sets `ack_matched="all"` on the wildcard (`:98`).
2. **The data it would print was never collected.** `acked()`'s wildcard arm is
   `[ "$_a" = "all" ] && { ack_matched="all"; return 0; }` — it returns at `:98` and
   **never reaches `:103`**, where per-sid accumulation happens. So `ack_matched` holds the
   literal string `all` and no sids. Removing the gate would reach a branch with nothing to
   name.

## Symptom (Effect)

A pusher sets `CODESCOUT_PUSH_ACK=all`, which the script documents as *"authorise whatever
is in this push"*. Foreign commits are authorised, the push proceeds, and **stderr carries
no list of sessions owed a notification** — not a partial list, not a "could not determine"
line. Nothing.

The sessions whose work just became public learn about it if someone tells them.

## Reproduction

1. A range containing commits from ≥2 sessions other than the pusher.
2. `CODESCOUT_PUSH_ACK=all git push`.
3. Read stderr.

Expected: the `your ack authorised N commit(s) by another session … it leaves each of them
exactly as UNCLEARED as they were` note, followed by the sids.
Actual: nothing.

## Root cause

`acked()` answers *"is this sid authorised?"* and, as a side effect, **accumulates the sids
it matched** — `ack_matched` is the notification's data source, not merely its condition.
The wildcard arm is correct for the question and skips the side effect, so one resolution
path satisfies the predicate over the full population while collecting over none of it.

`:309`'s `!= "all"` then reads as a deliberate exclusion. It is defensible for one of the
block's *three* jobs and wrong for the other two:

| the block's job | correct to skip on `all`? |
|---|---|
| per-token *"X authored no commit in this push"* | **yes** — no tokens were named |
| *"no foreign population, nothing needed authorising"* | yes — orthogonal |
| **naming the sessions not yet told** | **no** — this is exactly when they exist |

One condition gates three purposes. It is right for two.

## Why this is not the empty-population design decision

`tests/…:998` reasons explicitly that *"a notify block printed unconditionally would satisfy
6g and be worse than none — it would send a pusher to message sessions whose work was not in
the range."* That is sound, and it justifies `[ -n "$ack" ]` plus the empty-population
branch. **It says nothing about `!= "all"`.** Row 6a's population is *empty*; the `all` case
is the opposite — a population exists and is maximal. The two are not the same condition and
the reasoning for one was not written about the other.

## Evidence that the missing half is the load-bearing half

The sid-naming step was added **2026-09-15**, in this same file, with its own measurement
(`:346-352`): the note previously *"stated a residual obligation and gave the reader no party
to discharge it against"*. Measured on push `56f33bd2` — 33 commits across four sessions,
the note printed `24`, and **three sessions had work published with zero told**, surfacing
three hours later only because one of them noticed origin had moved.

That fix landed on the **named-sid** path. The `all` path was left where the named path had
just been proven harmful, and is strictly worse: not an unactionable note, but no note.

**Live confirmation, from this session and in the direction that shows the mechanism
works.** Twice on 2026-09-15 another session pushed commits authored by
`f5f48b42-6d84-482e-84a4-8eaebb0ce60f`. Both used the **named** form, both notifications
fired, and this session learned its work was public from that note rather than by noticing
origin had moved. The remedy is not theoretical — it is the only reason the author of this
file knew. Had either pusher reached for `all`, the same push would have been silent.

## Test coverage

`tests/pre-push-foreign-session-guard.sh:268` is `eq "ack=all allows" "$EC" 0` — an
**exit-code** assertion. It cannot observe stderr, so it is green for a run that prints the
notification and for one that prints nothing. The `all` path had coverage of its verdict
and none of its output, which is why this survived a suite that tests the notification
carefully on every other path.

**Closed by block `6h`** — 7 rows, all asserting on **stderr**. Observed RED first: 5 of
the 7 failed against the unfixed script, while `:268` stayed green throughout, which is
the gap demonstrated rather than argued. Suite went 129 → 136 rows, 0 failed.

Two rows carry reasoning that generalises past this bug:

- **The sid rows discriminate; the prose row does not.** Under the site-1 mutation,
  `says they have not been told` **passed** — the sentence survived intact and only the
  list was wrong. A notification bug is invisible to an assertion about the notification's
  wording. Assert the sids.
- **`hasnt "never prints the literal token as a sid"` was vacuous before the fix.** Nothing
  printed at all, and an absence assertion is monotone under removal, so it passed against
  the very defect it was written for. It is load-bearing only because the `has` rows above
  it red when the block is silent — and it earned that place by catching `      all` under
  the site-1 mutation, a plausible-looking line naming a party that does not exist.
## Who cannot see it, and why that decides the fix

The defect is invisible from the only side positioned to notice it, and that is not incidental
— it is the shape.

**Who structurally cannot see it:** the foreign sessions whose work is published. Their signal
that a push happened is *the notification itself*. When it does not fire they receive nothing,
and **receiving nothing is byte-identical to nobody having pushed.** No amount of attention on
their side distinguishes the two. They cannot audit a message that was never sent.

**Who can:** the pusher, who holds `$ack` and can see which form they typed — and who has no
reason to look, because from their side the push succeeded either way. The guard prints its note
on one path and stays silent on the other, and silence after a successful push reads as *nothing
to report*.

**Measured from the receiving end, 2026-09-16, by sessionId
`f0b1a4c7-e991-4478-bf22-b088483b6821`:** every *"your commits were pushed"* message they
received across four pushes existed because two peers **happened to choose the named form**.
Five messages, all discretionary. Their words, and they are the point of this section: *"the
tell-after discipline I have been relying on all day is a policy with a known silent failure
mode, not a mechanism — and I could not have discovered that from the receiving end, because its
failure looks exactly like nobody having pushed."*

**Why this matters for the fix rather than being commentary.** § *Observer Blindness* position 3
asks for a check that runs when nobody is worried, and the two changes under *Fix* are exactly
that — they make the correct path end in a safe state, so the notification cannot be switched off
by choosing the shorter ack. A remedy of the form *"remember to use the named form"* would be a
policy layered on a policy, and the party it protects cannot verify compliance. **Do not close
this by documenting `all` as discouraged.** The script's header already discourages it; that is
precisely the state in which this was found.

## Fix

**Applied** — `fbddd86d929887dbf20f91b95db655e203e4172f`, patch-id
`77fced910e5cf94da98108342a9a9867416e9128`.

Both changes landed:

1. `acked()`'s wildcard arm sets a separate `ack_is_wildcard` flag and falls through to
   the **same** accumulation the named form uses, so `ack_matched` holds real sids under
   `all`. One shared append path rather than a copy per branch — a second copy is what
   lets the two drift, and a branch that skipped the append is the defect being replaced.
2. The `!= "all"` clause is gone from the gate. The per-token staleness loop keeps that
   condition on its own `elif`, annotated as unreachable-today-and-guarded-anyway so it is
   not mistaken for a live branch.

`ack_is_wildcard` is initialised at its declaration because `set -u` is on (`:69`).
Written without that line first, and the unset read aborted the script mid-block — the
red is recorded at the declaration, because of *which* assertion caught it: of the three
covering the killed note, only the one `has` row failed. Both neighbouring `hasnt` rows
passed on the dead script, an absence assertion being monotone under removal.

### The two defects are not independent in the direction this file claimed

The Summary says *"two independent failures, and either alone is sufficient"*. That is
right about the **defect** and wrong about the **fix**, and only a mutation per site showed
it:

| mutation | state it recreates | verdict |
|---|---|---|
| wildcard short-circuit restored | site 1 broken, site 2 fixed | **KILLED** — 3 rows |
| `!= "all"` gate restored | site 1 fixed, site 2 broken | **SURVIVED** — 0 rows |

Fixing site 1 alone **would** have closed the bug: with real sids in `ack_matched`, the old
gate's `!= "all"` is true and the block runs. Fixing site 2 alone would not — that is
exactly the first mutation, which prints the literal token `all` as a session to go and
notify. So site 2's change removes a clause that site 1's fix renders inert.

That is a **third** reading of `SURVIVED` beyond the two in `CLAUDE.md` § *Testing
Discipline*: not untested, and not unreachable-without-a-seam, but **semantically inert** —
the mutated condition cannot be false on any input the fixed code can produce. Worth
naming because the two documented readings both send you to write something, and this one
asks for nothing.

The independence claim was reasoned from reading the code. Running it inverted the
relationship.
## Provenance

Found by `/code-review` on 2026-09-15 while pointed at the wrong target — it was given PR
#20 and reviewed the local working tree instead
(`embedder-stack-ops-session-log:F-3`). Confirmed at the bytes after the code landed, by
sessionId `f5f48b42-6d84-482e-84a4-8eaebb0ce60f`, including the check that the
empty-population reasoning does not cover this case.

## References

- `scripts/pre-push-foreign-session-guard.sh:98` — the wildcard short-circuit
- `scripts/pre-push-foreign-session-guard.sh:103` — the per-sid accumulation it skips
- `scripts/pre-push-foreign-session-guard.sh:309` — the gate
- `scripts/pre-push-foreign-session-guard.sh:346-352` — the 2026-09-15 sid-naming fix and its
  measurement on push `56f33bd2`
- `tests/pre-push-foreign-session-guard.sh:268` — the exit-code-only `ack=all` row
- `tests/pre-push-foreign-session-guard.sh:998` — the empty-population reasoning, which is
  about a different condition
