---
kind: bug
status: mitigated
tags:
- cluster/record-asserts-an-unchecked-completion
closed: 2026-09-10
opened: 2026-09-10
owner: marius
related: []
severity: low
unverified: 'residual 3 is NOT closed and cannot be: nothing binds the ack''s sid list to what the operator was actually told. Only the false assurance was removed, per the bug''s own prescribed reachable move. Residuals 1 and 2 are genuinely fixed with regression tests.'
---

# BUG: the inert-ack note cannot say *nothing was examined*, and nothing states that an ack grants only on the pusher's behalf

## Summary

> **PARTIALLY RESOLVED 2026-09-10 — residuals 1 and 2 fixed, residual 3 mitigated only, and
> a fourth item added that was not in this file when it was written.** Fixed on `experiments`;
> SHA and patch-id in § *Fix*. Applied by sessionId `26cb9b5b-2c9c-489e-97d9-3a907c8b2941`
> **on its operator's explicit instruction** — which is the standing the two earlier sessions
> correctly judged they did not have. The reason these sat filed rather than applied was
> sound and is worth preserving: a second author arriving with an unrequested patch to a
> shared safety surface is its own hazard.
>
> - **(1) fixed.** The inert-ack note now branches on `foreign_report`. An ack set on a push
>   with no foreign commits says the population was empty and that the push was allowed on
>   that basis rather than on the ack — said once for the range, not once per token, because
>   the fact is about the range. The old per-token wording is unchanged for the case it was
>   right about.
> - **(2) fixed.** The mirror now sits beside *"a peer CANNOT grant"*: an ack records the
>   pusher's operator's decision and does not speak for the operators of the sessions it
>   names.
> - **(3) MITIGATED, NOT FIXED, and the gap is unchanged.** Only the false assurance was
>   removed, exactly as § *Fix* prescribed: the banner now says the guard computed those sids
>   from the range and did not witness the operator's decision about them. Neither rejected
>   direction was attempted. `unverified:` in frontmatter carries this so a query can read it.
> - **(4) NEW, not from this file.** The banner never said what taking the ack route *does*
>   to the three-state question. It overtakes it: an ack is not a fourth author state and
>   moves nobody into `cleared`, so every author below stays UNCLEARED and the table cannot
>   describe the outcome. Raised independently by **3 of 5** authors polled after the
>   2026-09-10 ack push — sids `26cb9b5b`, `343d53e1`, `59112612` — the sharpest form being
>   *"never resolved, only overtaken"*. Recorded here rather than opened as a fifth file
>   because it is the same surface and the same class.
>
> **Line numbers in § *Resume* below are stale** — `0a6a2c9d` moved them. The inert-ack note
> is no longer at `:194-196`, and `:341` is now the rung-author sentence, not the sid list.
>
> **Review not obtained.** sessionId `343d53e1-2c36-4063-9517-7459472e9b31` volunteered in
> § *Resume* and has not been asked; the change is committed on the operator's instruction
> with a green suite, not on a reviewer's sign-off. Surfaced to my operator as an open
> option rather than treated as satisfied.

**Three** unfixed residuals on the `CODESCOUT_PUSH_ACK` surface of
`scripts/pre-push-foreign-session-guard.sh`. The first two were split out of
`docs/issues/archive/2026-09-09-pre-push-guard-filters-on-the-local-ref-shape-so-a-refspec-push-bypasses-it.md`
so that archiving its titular defect — fixed at `d6847322` — did not bury them; the third
arrived later, from inside the second instance below. None was touched by that fix. (The
file's name predates the third and names only the first two.)

1. The inert-ack note reports *"authored no commit in this push"* for two different states,
   one of which it makes false.
2. Nothing in the banner says an ack records **one operator's decision** rather than the
   named authors' consent — the mirror of a rule the banner already states in one direction.
3. **Nothing binds the ack's sid list to what the operator was actually told.** The deeper
   cut of (2): even the lesser claim an ack *does* make — *"my operator decided about these
   sids"* — is unchecked, because the operator decided over the pusher's **prose** and the
   ack carries **sids**, with no surface connecting them.

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

**(3)** The ack's entire lifecycle is: read `$CODESCOUT_PUSH_ACK` from the environment
(`:84`), normalise it (`:96-106`), match its tokens against the sids in the range. **There is
no step anywhere that connects the list to a record of what the operator was told or decided**
— verified by construction at `752450b2`, not inferred. The comment at `:80-81` states the
intent the code cannot enforce: *"the ack is meant to record a decision, not dismiss a
prompt."*

**And the banner makes the sids correct-by-construction, which is the sharp edge.** `:341`
prints `CODESCOUT_PUSH_ACK="$foreign_sids" git push <args>` — the guard computes the list for
the pusher. So the sid half can be machine-perfect, copied from the guard's own output, while
the authorisation was formed over a prose sentence naming a different set. **The more
trustworthy the list looks, the less it says about what was authorised.**

Raised by sessionId `343d53e1-2c36-4063-9517-7459472e9b31` from inside their own instance,
where it was a near-miss rather than a failure: their ack named this file's author and
`c86ebb51` correctly, while the sentence their operator decided over said *"5 commits from the
live peer"* and was wrong about who. They re-read the refusal and corrected it before acting.
Had they not, **the ack would have carried two correct sids under an authorisation formed over
one — machine-truthful and substantively false, with nothing anywhere able to tell the
difference.** They offered it as a sharpening of (2); it is numbered separately because the
mechanisms differ — (2) is about the *scope* of the consent an ack represents, (3) about the
*fidelity* of the record to the decision that produced it — and because (3) survives (2) being
fixed.

## Evidence

### Second instance, 2026-09-10, disclosed by the pusher

`origin/experiments` moved `7e60f305..1691ca0f` — **8 commits spanning 3 distinct sessionIds**,
one of them this file's author's (`1691ca0f`), verified independently after the fact. The
pushing session, `343d53e1-2c36-4063-9517-7459472e9b31` (sid re-derived from the socket its
message arrived on, not from its signature), reported it unprompted and described its own
reasoning in the words this bug is about:

> *"I simply took my operator's decision as covering the set […] the honest description is
> that your commit was published without your session being consulted."*

That is defect (2) stated by the party it runs through, which is the strongest form this file
can carry. Their conduct was otherwise the one the banner prescribes and worked: the guard
refused the branch push and printed the stack with authors resolved live; they had described
it to their operator as *"5 commits from the live peer"* and, reading the refusal, **corrected
their own attribution before acting** — four were one peer's, one was this author's; their
operator then authorised the full set knowing it spanned two other sessions; and they pushed
**by sha after re-deriving the range**, so the decided eight went and nothing that landed in
between.

**Nothing here was withheld and nothing is owed back.** What the instance shows is narrower
and exactly this file's subject: every mechanism fired correctly, an informed operator
decided, and there is still **no surface on which the named authors' operators are asked** —
so the resulting `origin` state is indistinguishable from one where they had been.

**The counterfactual they drew is now false, and that is the fix's value.** They wrote that
had they reached for the refspec route first, the commit *"would have gone out with nobody
asked and no record that anyone should have been."* True of the world before `d6847322` —
and no longer, since the refspec form is scanned too (re-verified 2026-09-10 by the parent
file's reproduction). The route the banner recommends is now the safe one, which is why this
instance produced a disclosure instead of a silent publish.

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

> **APPLIED 2026-09-10.** SHA `ca977f51` on **`experiments`**; patch-id
> `9b7bc9fd70a5d44a0e2696b76d661619221b63cb`. The SHA dies on the next rebase of
> `experiments`; the patch-id survives rebase and cherry-pick, so cite that one.
>
> Residuals 1 and 2 are fixed as § *Summary*'s banner describes. Residual 3 is **mitigated
> only** — the disclaimer beside the prefilled list, which is precisely the reachable move
> prescribed below and nothing more. Neither rejected direction was attempted. A fourth item,
> not in this file when it was written, shipped in the same commit: the banner now says the
> ack route **overtakes** the three-state question rather than answering it.
>
> Regression tests: Row 6 of `tests/pre-push-foreign-session-guard.sh`, four assertions, all
> watched RED first. 6a and 6b are behavioural and are each other's control — 6b holds the
> old wording under test for the case it is right about, so a fix that merely deleted it reds
> there rather than passing. 6c–6e are shape assertions on prose, annotated as such in the
> file: they establish that each step still exists, never that it is correct. 112 passed, 0
> failed.

Not applied.

**(1)** Distinguish the two states at the site: an ack that matched nothing because the
population was empty should say the population was empty. That requires the note to read a
count the guard already has.

**(2)** One line in the banner, beside the existing *"a peer CANNOT grant"*: an ack records
the pusher's operator's decision and does not speak for the named authors' operators.

**(3)** No obvious fix, and saying so is the honest state rather than a placeholder. The gap
is between a machine-readable list and a human decision formed in prose, and nothing in a git
hook can witness the second. Two directions that do **not** work, both rejected here so nobody
re-derives them: requiring the pusher to paste what they told their operator produces a
second unverifiable prose artefact; and refusing the ack unless the sids were copied from the
guard's own output makes the machine-perfect half *more* authoritative, which is the defect
rather than its remedy. The reachable move is to stop the banner implying the binding exists
— `:341` hands the pusher a ready-made list, and a line saying **the guard computed these
sids, it did not witness your operator's decision about them** costs one sentence and removes
the false assurance without pretending to close the gap. Filed as *no obvious fix* per this
repo's own habit of recording the asymmetry rather than filling it (see `IC-18` § *Mechanism
status*, which does the same for author-written selectors).

**All three are edits to a shared safety surface that gates every session's push on this
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

For (3), add the one-line disclaimer beside `:341` rather than attempting a binding. Do not
reach for the two rejected directions recorded in § *Fix*.

**A reviewer is available and has volunteered:** sessionId
`343d53e1-2c36-4063-9517-7459472e9b31`, who raised (3), offered to read carefully on the
grounds that *"I have now been the failure case, which is a poor qualification for authority
and a decent one for review"*. They explicitly declined to touch the guard themselves — a
second author arriving with an unrequested patch to a shared safety mechanism being its own
hazard — which is the same reason these are filed and not applied.

## References

- `docs/issues/archive/2026-09-09-pre-push-guard-filters-on-the-local-ref-shape-so-a-refspec-push-bypasses-it.md`
  — the parent, whose titular defect is fixed at `d6847322`.
- `scripts/pre-push-foreign-session-guard.sh:194-196` (the note), `:283-340` (the banner).
- `docs/trackers/observer-blindness.md` OB-20 — why the guard exists.
