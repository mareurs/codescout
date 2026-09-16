---
status: open
opened: 2026-09-15
closed:
severity: high
owner: marius
related: []
tags: [cluster/selector-narrower-than-its-population]
kind: bug
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
notification and for one that prints nothing. The `all` path has coverage of its verdict and
none of its output, which is why this survived a suite that tests the notification carefully
on every other path.

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

Not applied. Two changes, both needed:

1. `acked()`'s wildcard arm must accumulate rather than short-circuit — set a flag
   *and* fall through to the per-sid append, so `ack_matched` holds real sids under `all`.
2. `:309` must stop gating the notification on `ack_matched != "all"`. The per-token
   staleness loop keeps that condition; the sid-naming branch does not.

A regression test must assert on **stderr**, not the exit code: `has "ack=all: names the
sessions not yet told" "$OUT" "<sid>"`. An assertion that the block *printed something* is
monotone under printing the wrong sids.

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
