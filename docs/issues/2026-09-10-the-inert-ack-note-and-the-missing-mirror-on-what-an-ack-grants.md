---
status: open
opened: 2026-09-10
closed: ''
severity: low
owner: marius
related: []
tags:
- cluster/record-asserts-an-unchecked-completion
kind: bug
---

# BUG: the inert-ack note cannot say *nothing was examined*, and nothing states that an ack grants only on the pusher's behalf

## Summary

Two unfixed residuals split out of
`docs/issues/archive/2026-09-09-pre-push-guard-filters-on-the-local-ref-shape-so-a-refspec-push-bypasses-it.md`
so that archiving its titular defect — fixed at `d6847322` — does not bury them. Both are on
the `CODESCOUT_PUSH_ACK` surface of `scripts/pre-push-foreign-session-guard.sh`, and neither
was touched by that fix.

1. The inert-ack note reports *"authored no commit in this push"* for two different states,
   one of which it makes false.
2. Nothing in the banner says an ack records **one operator's decision** rather than the
   named authors' consent — the mirror of a rule the banner already states in one direction.

## Symptom (Effect)

**(1)** `scripts/pre-push-foreign-session-guard.sh:194-196`, unchanged as of `752450b2`:

```
note: CODESCOUT_PUSH_ACK named %s, which authored no commit in
this push, so the ack had no effect on it. Check you named the sid
you meant -- an ack matching nothing is silent otherwise.
```

The sentence is true when the named sid authored nothing **in a population the guard
actually built**, and false when the population was empty. Its printed remedy — *"check you
named the sid you meant"* — points at the reader's input in both cases, so the reassuring
reading is the wrong one.

**(2)** Measured 2026-09-09: 39 commits published under an ack naming six sids, 11 of them
authored by a session whose operator had never been asked. No harm — nothing was withheld and
the pushing operator was shown the count and author set — but the **record** of that push is
indistinguishable from one where every author had consented.

## Reproduction

**(1)** Not currently reachable end-to-end, and that is the fix's doing rather than this
bug's: before `d6847322` a refspec push skipped the scan entirely, so an ack could be
supplied against a population of zero and the note fired with its false reading. With the
scan no longer skippable for any branch-updating push, the empty-population branch is much
harder to reach. Filed anyway because the sentence still cannot distinguish the two states and
the next skippable path re-arms it.

**(2)** Read `:283-340` (the refusal banner) at `752450b2` and grep for the mirror:

```
$ grep -c 'cannot grant on the author\|ack cannot grant\|consent aggregator' \
      scripts/pre-push-foreign-session-guard.sh
0
```

The banner does carry the other direction, verbatim: *"A peer can report what they were told;
a peer CANNOT grant."*

## Environment

Linux, `experiments`, shared checkout. Platform-independent.

## Root cause

**(1)** The note's predicate is *"this sid is absent from `ack_matched`"*, which is satisfied
by *no such author among the commits examined* and by *no commits were examined*. One
predicate, two states, one sentence — and the sentence asserts the first.

**(2)** `CODESCOUT_PUSH_ACK` records exactly one fact: the pusher's operator decided to
publish a named set. It is silent on whether each named author's operator would have, and
nothing in the banner says so. An ack naming six sessionIds reads as six authorisations while
being one decision. Raised by sessionId `b0015a98-e290-46de-8ed1-3c94bc73a987`, whose own
phrasing of the incident blurred the same distinction and who flagged it themselves.

## Evidence

The banner already reasons in three states — *withheld*, *not withheld/uncleared*, *cleared*
— and an external decision to publish is a **fourth** that collapses into none of them. That
structure is what makes (2) a gap rather than a nuance: the text is careful about exactly this
class of confusion in the peer direction and silent in the pusher direction.

## Hypotheses tried

1. **Hypothesis** — `d6847322` fixed these along with the ref-shape bypass.
   **Test** — grep the guard at `752450b2` for the note's wording and for any mirror.
   **Verdict** — rejected. `:194-196` is byte-identical to the filing, and the mirror returns
   0 matches.

## Fix

Not applied.

**(1)** Distinguish the two states at the site: an ack that matched nothing because the
population was empty should say the population was empty. That requires the note to read a
count the guard already has.

**(2)** One line in the banner, beside the existing *"a peer CANNOT grant"*: an ack records
the pusher's operator's decision and does not speak for the named authors' operators.

**Both are edits to a shared safety surface that gates every session's push on this
checkout.** Rewording it is not a drive-by, which is why these were recorded rather than
applied.

## Tests added

None. (2) is testable as **shape** rather than prose — assert the banner names both
directions, which reds on deletion and survives rewording, the same discipline
`CLAUDE.md` § *Testing Discipline* records for the two-addressee assertion. (1) wants a
fixture that supplies an ack against an empty population.

## Workarounds

For (1): read `an ack matching nothing` as *either* wrong sid *or* nothing examined, and check
the push's own commit count before trusting it. For (2): state in the push authorisation
itself whose decision it is.

## Resume

Add the empty-population branch to the note at
`scripts/pre-push-foreign-session-guard.sh:194-196`, and the one-line mirror to the banner
beside the existing peer-cannot-grant sentence. Then add the both-directions shape assertion
to `tests/pre-push-foreign-session-guard.sh`.

## References

- `docs/issues/archive/2026-09-09-pre-push-guard-filters-on-the-local-ref-shape-so-a-refspec-push-bypasses-it.md`
  — the parent, whose titular defect is fixed at `d6847322`.
- `scripts/pre-push-foreign-session-guard.sh:194-196` (the note), `:283-340` (the banner).
- `docs/trackers/observer-blindness.md` OB-20 — why the guard exists.
