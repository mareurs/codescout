---
id: 8d9d2424c81a3744
kind: bug
status: mitigated
title: 'BUG: an unpushed count correct for my commits is published under the name of the branch''s'
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
unverified: STANDING — No mechanism exists. The mitigation is a PROBES.md row -- a read surface a reporter must think to consult -- and both instances were produced by sessions that had read the class, one of them 3.5h after editing its axis list. So nothing fires when a session writes "all mine", and the remedy is a policy, not the check-that-runs-when-nobody-is-worried that OB position 3 asks for.
---

# BUG: an unpushed count correct for *my* commits is published under the name of *the branch's*

## Summary

Two sessions in this checkout, independently and hours apart on 2026-09-16, reported the
unpushed pile as **"N unpushed, all mine"** where `N` was the count of *their own* commits
and the pile was larger and multi-session. The number was right. The **scope word** bound it
to the wrong set.

This is the first tagged member on `IC-24`'s **scope-word** axis, which the class has named in
its claim since it opened but never carried an instance for.

## Symptom (Effect)

Both, verbatim:

```
sessionId 29420e72 : "8 unpushed, all mine"
                     actual: 18 across 4 sessions, 6 theirs

sessionId 9403d62d : "Unpushed: 3 commits, all mine"
                     actual: 5 across 3 sessions, 3 mine
```

Neither produced an error, a refusal, or a disagreeing instrument. Every check aimed at the
value confirms it, because the value is not what is broken.

## Reproduction

The derivation is one line and was available to both reporters throughout:

```
$ git fetch --quiet origin
$ git log origin/experiments..experiments \
      --format='%h%x09%(trailers:key=Session-Id,valueonly,separator=%x2C)'
```

Run at 2026-09-16T19:24:02Z it returns **12 across 6 sessions, 3 mine**. Run 41 minutes
earlier it returned 5 across 3. Neither reporter lacked the command; both cited a value
instead of re-running it.

## Environment

`experiments`, shared checkout, 18 live sessions across 3 profiles at
2026-09-16T21:41:09+03:00, six of them in this tree. Peers landed 7 commits in the 41 minutes
between the two readings quoted above.

## Root cause

A value computed over the set `{commits whose Session-Id is mine}` is published under a name
denoting `{commits on this branch not on origin}`. The frame is the **scope word** — `IC-24`'s
claim names it beside coordinate space, unit, citation form and verdict word.

The value is exactly right and exactly recoverable: `3` *is* the number of my commits. Only
the binding between value and name is wrong.

**Why not `IC-20` (floor published under the name of a total).** A floor arises from an
incomplete scan — the instrument saw part of the population and reported as if it saw all. Here
nothing was incomplete: the count is a complete and correct count *of a different set*. Running
the same instrument longer or wider would not have changed it.

**Why not `IC-18` (selector narrower than its population).** `IC-18` is about a selector that
cannot reach members it should. Here the selector was correct for the question actually asked;
the defect is in which question the sentence claims to answer.

## Evidence

### My instance isolates the axis; the other confounds it

`29420e72`'s figure was **also an hour stale** — 6 were theirs, not 8, so their number was
wrong for *both* sets. Mine was derived minutes before I published it and `3` was exactly right
for `{mine}` at that instant. So this record's own instance separates the scope-word error from
the staleness error, which matters because the natural remedy for staleness — re-derive more
often — does nothing here. I re-derived, correctly, and still wrote the wrong noun.

Verified by commit timestamps: the two peer commits I omitted (`78feb953` 19:58, `71781d7e`
19:56) both predate my last two (`601043b2` 19:59, `8273331d` 20:01), so they were in the tree
when I spoke. Not churn.

### Knowing the class prevented nothing — measured, not asserted

I edited `IC-24`'s own axis list at **16:34** that day:

```
852fa5e7 16:34 9403d62d  docs(clusters): IC-24 gains a per-claim frame boundary
                         and two non-librarian axes
```

and committed this instance of it roughly **three and a half hours later**. That is
CLAUDE.md § *Observer Blindness*'s opening measurement reproduced: four instances in one
evening, every one by an author actively writing about the class. Recorded here because a
class whose instances are all authored by people who know it is evidence that "check harder"
is the wrong instrument, not that the authors were careless.

### The first derivation I ran after being corrected was ALSO wrong

`%(trailers:key=Session-Id,valueonly)` emits a **trailing newline**, so a
`%h|%trailer|%s` format puts each commit on two lines and commit subjects land in the session
column — the first tally reported `6 docs(iss` as a session id. Caught only because the
subject-shaped strings were obviously not sids. A corpus where every subject happened to look
like a hex prefix would have produced a clean, wrong table.

## Hypotheses tried

1. **Hypothesis:** a script would have prevented it.
   **Verdict:** rejected, by the other reporter's own account — they had the one-line
   derivation throughout and cited a stale value rather than running it, and record that they
   "would have cited a script's earlier output just as readily." My instance strengthens this:
   I *did* run the derivation, and the scope word was still wrong.

2. **Hypothesis:** the authoritative derivation does not exist yet.
   **Verdict:** rejected. `scripts/pre-push-foreign-session-guard.sh` has carried a
   merge-aware, trailer-correct implementation all along — but it runs **only on push**, so its
   answer is published to the one audience that already knows it. That is § *Observer
   Blindness* position 3 exactly.

## Fix

**Mitigated, not fixed.** `docs/PROBES.md` gained a row (line 190) giving the derivation a read
surface and prescribing the reporting form:

> `N unpushed across M sessions, K mine, at <UTC>`

Deliberately **no new script** — the row says so and gives the reason: a second copy of a
derivation that already exists in the push guard would drift, and the defect was never a
missing implementation.

### Fix provenance

- **SHA:** `f9d77076` (`experiments`) — not mine; `29420e72`'s
- **patch-id:** `01eaa5523753cb7b30354343e638e1662b7f5191`

**What is still owed:** a read surface is a policy, not a mechanism. Nothing fires when a
session writes "all mine". § *Observer Blindness* position 3 asks for a check that runs when
nobody is worried, and this record does not have one.

## Fix provenance

- **SHA:** `f9d77076` (`experiments`)
- **patch-id:** `01eaa5523753cb7b30354343e638e1662b7f5191`

Formalised 2026-09-23 from a pair this record already stated in § *Fix* prose. `doctor`'s
`terminal_status_without_fix_anchor` cannot parse a hash in running text, so the record read as
anchored while nothing resolved it -- which is the failure mode that check exists to name. The
pair was not guessed: the SHA resolves and is an ancestor of `experiments`, and its patch-id
recomputed from the diff equals the one this file already carried. The SHA is positional and
dies on the next rebase of `experiments`; the patch-id is a content hash of the diff and
survives rebase and cherry-pick, which is why both are recorded rather than either.

## Tests added

None. The defect is in a sentence a session writes, not in code — there is no production path
to mutate. Stated rather than left blank: an empty `Tests added` normally means the bug is not
really closed, and here it means the bug is not the kind a test can reach.

## Workarounds

Report the unit explicitly and derive it at the instant you report it. Never carry a figure
forward across a turn: six peer pushes landed on this checkout that day, so an unpushed figure
is valid only at its instant, exactly like a peer count.

## Resume

If a third instance appears, that is the trigger to stop documenting and build the mechanism —
the obvious candidate being the same derivation attached to something a session does anyway,
rather than to `push`, which only the already-informed reach.

## References

- `docs/trackers/issue-clusters/IC-24-value-correct-in-a-frame-its-name-does-not-state.md`
- `docs/PROBES.md` line 190 — the read surface and the reporting form
- `scripts/pre-push-foreign-session-guard.sh` — the authoritative derivation, wrongly audienced
- CLAUDE.md § *Observer Blindness* — why "check harder" is the wrong instrument here
