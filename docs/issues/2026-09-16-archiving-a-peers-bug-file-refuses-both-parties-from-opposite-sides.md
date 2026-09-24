---
kind: bug
status: fixed
tags:
- cluster/shared-resource-carries-no-owner
closed: 2026-09-16
opened: 2026-09-16
owner: marius
related: []
severity: high
---

# Archiving a peer's bug file refuses both parties, from opposite sides of one guard

## Summary

`CLAUDE.md` **mandates** archiving through `doc(action="move")`, never a bare `git mv`.
When one session archives bug files another session wrote, and that archive is coupled to
a tracker a second session is editing, the whole change sits in one shared `.git/index`
and `pre-commit-foreign-index.sh` refuses **both** parties — each refusal naming the other
as its remedy, and that other party being themselves refused.

**The splitting act is `git add`, not `doc(action="move")`.** The guard is keyed on who
STAGED a blob, not who wrote it; § *Root cause* carries the measurement, in which
authorship and staging point in opposite directions for the same six files. An earlier
version of this file said the mandated archive procedure splits a path's *authorship* and
that the pair therefore has no legal state. Both halves were wrong: the procedure is not
what splits anything, and a single session staging the whole coupled set is a legal state
the guard accepts today. What survives is narrower and sharper — the refusal never names
that route.

**This is not the already-filed empty intersection.**
`docs/issues/archive/2026-09-01-two-correct-pre-commit-guards-have-an-empty-intersection.md`
(`cecd0bb43d082406`) is **two different guards** — `foreign-index` × `ledger-counts` —
whose acceptance sets do not overlap. This is **one guard** refusing both parties. Same
cluster, different mechanism.
## Symptom (Effect)

Measured 2026-09-16 on six files, from both sides of the same guard within one hour.

**Side A — the archiver** (`9e022ef0-eb76-49f0-b175-4d68979290cf`, moving six bug files
whose `d3a2c24f` carries `9403d62d`):

```
Refusing this pathspec commit: it captures content another session wrote.
  theirs:  docs/issues/2026-09-13-architecture-probe-*.md   (all six)
```

**Side B — the author** (`9403d62d-116b-46ea-ac9b-004acff2b1cb`, committing the same six):

```
Refusing this pathspec commit: it captures content another session wrote.
  theirs: <all six old paths AND all six archive paths>
  You cannot narrow further: the contested path IS one you named.
  Every path you named is contested, so there is nothing to narrow to.
  Then ask the owner below to commit theirs.  -> 9e022ef0
```

Both refusals are **correct**. Side A is refused over the source half, side B over the
current content. The guard's remedy — *"ask the owner to commit theirs"* — resolves to a
party who is themselves refused, so following it terminates in the refusal it came from.

**BOTH BLOCKS ABOVE ARE STAMPED, and without the stamps this file contradicts § *Root
cause*.** They quote refusals produced **before** `9403d62d` re-staged the coupled set, when
the twelve rename paths were `9e022ef0`'s. § *Root cause* reports the same lookup returning
those twelve as `9403d62d`'s — a measurement taken **after** that `git add`. Both readings
are correct at their own instant and cannot both be correct at one, which is exactly the
failure this repo files under *a count is valid only at its instant*: ownership here is not
a property of a path, it is a property of a path **at a moment**, and `git add` moves it.

Raised by a design review reading the file as self-contradictory — a fair reading of what
was written, since neither block carried a time. The correction is the stamp, not either
number. **Nothing downstream of this file should cite an owner from it without one**, and
that matters most where it would be cited as justification for relaxing the guard.

## Why the suggested remedy is worse than the refusal

The guard directs a blocked caller to narrow by pathspec. On a `doc(action="move")` that is
**actively harmful**: naming only the destination half commits the addition and leaves the
deletion staged, so the file exists at **both** paths at HEAD — a silent duplicate, and the
archive appears to have succeeded. The guard already detects this case and says
`You cannot narrow further`, which is why the damage was not done here; a caller who
narrows *before* reading that line does it anyway.

## Why "one of them uses --no-verify" is not the fix

It is available, it works, and the guard itself says it is the wrong habit. The reason it is
wrong here specifically: the guard's purpose is **attribution**, and the archive genuinely
does carry two sessions' work under one commit message. Bypassing does not make the
attribution correct, it makes it unrecorded. Whatever the fix is, it should let the commit
**say** that the path has two authors rather than suppress the question.

## Reproduction

1. Session A files a bug file; it carries A's `Session-Id`.
2. Session B records the fix SHA + patch-id on it, flips `status` through the catalog, and
   archives it via `doc(action="move")`.
3. B commits by pathspec → refused, source half is A's.
4. A commits by pathspec → refused, current content is B's.

Deterministic. Any archive of a peer-authored bug file reaches it.

## Environment

Verified before hitting the deadlock, so the refusal is the only blocker:

- `d3a2c24f` carries `9403d62d` and **is** the fix — it introduces
  `scripts/architecture-boundary-probe.py` (1438 lines) and
  `tests/test_architecture_boundary_probe.py` (218 lines).
- **14/14 regression tests pass at current HEAD**, `self-test: ok` — re-run rather than
  cited, because `40fb2843` touched the probe after `d3a2c24f`.
- No live citation to the old paths outside `docs/issues/archive/`, with a control
  confirming the same grep finds the new ones.

## Root cause

One `.git/index`, several sessions, and ownership recorded at `git add` time. A change
that is logically one unit — six renames plus the two trackers citing them — gets staged
in pieces by whichever session is doing that piece, so the index holds one change with
two recorded stagers. `pre-commit-foreign-index.sh` then refuses each session the paths
the other staged, and each refusal's remedy names the other party.

**THE MECHANISM IS A STAGING SPLIT, NOT AN AUTHORSHIP SPLIT — corrected 2026-09-16, and
the correction decides which fixes are even relevant.** The guard never reads authorship.
`scripts/post-index-change-stage-log.sh` records *"WHICH SESSION staged each blob
currently in the index"* (its own header, line 3) under the rule *"THE STAGER WINS, NOT
THE FIRST OBSERVER"*. The contested set is produced by `git add`, never by
`doc(action="move")`.

**The two point in OPPOSITE directions here, which is what makes this a fact rather than a
wording preference.** All six bug files were authored by `9403d62d` — `d3a2c24f`, read
from its `Session-Id` trailer — and archived by `9e022ef0`. Yet **at the instant this was
measured, after `9403d62d` had re-staged the coupled set**, the guard's
own `(blob, path)` lookup against `.git/session-stage-log` returned all twelve rename
paths as `9403d62d`'s, `route=named`, because that session ran `git add` last. The guard
called a session's own bug files its own, correctly by its own rule, while the session
that *performed the archive* held none of them.

**There IS a legal state, and this file's first version said there was none.** Any single
session that stages the whole coupled set owns all of it and commits cleanly. What the
guard does not do is *say so*: its remedy names the other party and offers narrowing,
never *"one of you should stage the entire set"*. A correct predicate with a remedy that
omits the performable route is CLAUDE.md § *Testing Discipline*'s *"a suite tests a
guard's PREDICATE and never its REMEDY TEXT"*, once more.

**The shortcut that looks like the remedy is not one.** `git add`-ing the other session's
file makes the recorder call it yours and the guard go silent. That is gaming the
instrument rather than satisfying it: it files their prose under your commit message with
no refusal emitted — the exact capture the guard exists to prevent. Named here because
nothing in the guard closes it.
## Fix

**Directions 2, 4, 5 and 6 SHIPPED 2026-09-16. 1 is shipped-but-unreached and 3 is dead.**
The deadlock is resolved — both parties now have a route, and the table below says which.

**Not designed.** Directions recorded so the next session does not re-derive them — and
two of the original three are now measured dead, which is most of what this section is
worth.

1. **Teach the guard about renames.** **Implemented at `b37b888a`, and it does not reach
   this case.** The `joint` predicate accepts a contested path only when it is half of a
   rename whose partner belongs to the committer. Against the live deadlock it evaluates
   to **0 from both sides**: side A's contested set was one plain `M` tracker with no
   rename partner; side B's was thirteen paths including a second plain `M`, and the six
   sources' partners are the six destinations, also the other party's. So
   `CODESCOUT_INDEX_ACK` is inert for **both** parties — it is gated on `joint`. The change
   is correct for a pure joint archive, was mutation-tested, and is exit-compatible with
   the original script on the same fixture; it simply never fires on the shape that
   actually occurred. **Assert the mechanism's REACH, not just its logic** — the law
   landing on the commit written to satisfy it.

2. **Let a commit declare two authors.** A second trailer, so the guard can pass on a
   commit that *names* the split rather than hides it. Still open, and now the most
   promising of the set.

3. **~~Move the frontmatter edit off the archiver.~~ DEAD.** It targets authorship, and
   the guard never reads authorship — whoever runs `git add` owns the row regardless of who
   wrote the bytes. This direction would have cost a session real work and changed nothing.
   Recorded rather than deleted precisely because it is the direction a reader of the
   original framing would reach for first.

4. **Name the performable route in the refusal text.** New, and the cheapest of the four:
   the refusal should say that one party staging the whole coupled set is a legal state.
   Nothing about the predicate changes, and it is the half a 101-assertion suite does not
   test.

5. **Warn on a partial commit that moves a path cited from outside the commit.** Also new,
   and it is the one that would have caught the `215a5cad` window in § *Workarounds*. The
   data is already there: the guard holds the staged rename pairs, and
   `git grep <old path> HEAD` over the complement is one call.

6. **The ack already reconciles this file's own two statements about staging, and it is gated
   on the one predicate that cannot reach this shape.** § *Root cause* says any single session
   staging the whole coupled set "owns all of it and commits cleanly"; the paragraph directly
   after it calls that same act "gaming the instrument rather than satisfying it — the exact
   capture the guard exists to prevent". Both are true, and what separates them is already in
   the guard: `CODESCOUT_INDEX_ACK` and the `Co-Authored-Session-Id` trailers it prints.
   Staging the whole set is a capture when it is SILENT and a recorded two-author commit when
   it is ACKED — in the guard's own words, *"the ack does not make the attribution correct — it
   makes it RECORDED, which `--no-verify` does not."*

   **So what is missing is not remedy text (direction 4) but REACH.** The ack arm is gated on
   `joint`, and direction 1 above already measured `joint` as **0 from both sides** on the live
   deadlock. The one route that would resolve this is unreachable from the one shape that needs
   it — which is also why direction 4 is not sufficient alone: it would print a legal state
   that the guard, reached by this shape, then refuses.

   **Shape of the change, deliberately NOT applied:** admit the ack in the all-contested branch
   — `theirs` non-empty with `mine` empty, the case whose text today reads *"Every path you
   named is contested, so there is nothing to narrow to"* — rather than only for a joint
   rename. The committer stages the whole coupled set, names every other author, and carries
   their trailers.

   **Two things to establish first, both of which sank earlier directions here.** A RED in
   `tests/hooks-discrimination.sh` for the all-contested branch specifically — § 7's `no
   sequencer -> still refuses` case is the control a widening must not break, and both
   relaxations rejected in
   `docs/issues/archive/2026-09-02-foreign-index-prescribes-a-remedy-git-refuses.md` were
   rejected for being wider than their defect. And the two-site `--no-renames` law: that
   pipeline is copy-pasted between `scripts/post-index-change-stage-log.sh` and
   `scripts/pre-commit-foreign-index.sh`, so mutating one site leaves the other's assertion
   green.

   **Why this is a direction and not a fix:** it relaxes when the guard ACCEPTS, which is the
   axis this file's own history says to be slowest about. Raised by sessionId
   `e5691fad-9f78-4cd1-ad14-edfdd1fee41f`, which hit the NEIGHBOURING pair
   (`scripts/pre-commit-unreviewed-content.sh` × `scripts/pre-commit-foreign-index.sh`) rather
   than this one, and that case is **weaker — recorded as such so it is not counted here.**
   `wait` was performable there and cleared it, because only one of the two parties was
   blocked. It is this same empty intersection for an instant, not the permanent one this file
   documents.

   **WITNESS, 2026-09-16 — the same pattern is ungated one layer up and carries real load.**
   `CODESCOUT_PUSH_ACK` in `scripts/pre-push-foreign-session-guard.sh` is this design already:
   name the other authors, proceed on your OWN operator's authority rather than theirs, and
   record what was done. It is gated on nothing but naming the sids, and it carried a single
   push of **36 commits across 7 sessions** in one operation, with every reachable author
   notified; the pushing session's operator authorised it. So the pattern is not what is in
   doubt — an ack that names its other parties is load-bearing at a gate that lets it fire.
   Direction 6 asks for that same shape at the index layer, where `joint` prevents it.

   **Two bounds on what the witness licenses, stated because it would otherwise be read wider.**
   The form used was the NAMED-SIDS one, re-derived from the range by the pushing session rather
   than pasted from the guard's own list — **not** the `all` wildcard. `all` does exist at that
   gate (`:83`, `:117` sets `ack_is_wildcard=1`), and its own history argues for naming instead:
   the arm removed at `:111` was `[ "$_a" = "all" ] && { ack_matched="all"; return 0; }`, a
   correct answer to *"is this sid authorised?"* that accumulated nothing for *"who is owed a
   notification?"* — so `all` published every foreign session's work and told none of them, on
   the ack path whose blast radius is largest. Second bound: the authority was one operator's
   decision on one push. **No reasoning is attributed to them** — the pushing session reported
   the trade-off and did not ask why, and putting a rationale in an operator's mouth is `IC-24`'s
   shape, a value correct in one frame published under a name that states another.
**Three of six shipped. The deadlock is HALF resolved, and which half is the point.**

| direction | state | citation |
|---|---|---|
| 1 teach the guard about renames | shipped, does not reach this shape | `b37b888a` |
| 2 let a commit declare two authors | **shipped, inverted** | `05c6f153`, patch-id `20ef09260e836da6f933f809b865a573970bc3db` |
| 3 move the frontmatter edit off the archiver | **dead** — targets authorship, which the guard never reads | — |
| 4 name the performable route | **shipped** | `fc1ad175`, patch-id `fef5458a8316af21fd0ed2db92289dfbda29ab01` |
| 5 warn when a move strands a citation | **shipped** | `6853c517`, patch-id `3616d2f2fd5ae7e2633c939b568cb4d5cfd28519` |
| 6 give the ack reach | **shipped**, same commit as 4 | `fc1ad175` |

**4 and 6 had to ship together**, which is direction 6's own finding: text alone would have
advertised a remedy the guard then refuses. The predicate is
`pathspec && theirs && mine empty`, restricted to the pathspec form because a bare commit
takes the entire shared index rather than a set the committer named — `mine` empty there is
the whole-index sweep this guard exists for, not a corner with no compliant route.

**WHICH HALF IS UNBLOCKED, stated because it would otherwise be read wider.** Side B named
only foreign paths, so `mine` was empty and the ack now fires for them. Side A held twelve
rename halves of its own, so `mine` was non-empty, `all_contested` does not fire, and side A
stays refused — **correctly**, because a narrowing route still exists for them and the guard
prints it. Anyone citing this file for *"the deadlock is fixed"* is citing it wrong.

**Direction 5 shipped as its own script rather than a branch of this guard**, and the first
design could not have worked: this guard has five early `exit 0` paths and builds its rename
maps after all of them, so `215a5cad` — the commit that stranded the citations — is recorded
as **Passed** by it. A warning hosted here would have been structurally unable to fire on the
case it was built for. Reach verified against real history instead of a fixture: at
`215a5cad^` exactly two files cited the six moved stems, and the new check names the one
that was not in the commit while staying silent about the one that was.

**A defect found by the design review and fixed first, because it is a tightening:**
`CODESCOUT_INDEX_ACK="-"` cleared refusals the ack was never meant to reach — the recorder's
unattributable sentinel was accepted as a session id, a wildcard at a gate whose own text
says it deliberately has none. Filed separately and fixed at `96d839c3`, patch-id
`caa8669f048c56e0dd09ced10823b9f2a2131cc6`. It had to land **before** the widening rather
than after: `all_contested` reaches far more rows than `joint`, and every `-` among them
would have been ackable by one character.

**And the coverage that should have existed first.** `b37b888a` shipped `joint` and the ack
with **zero** tests — `grep -rn "CODESCOUT_INDEX_ACK\|joint" tests/` returned nothing while it
was reported as *"101 passed"*. That backfill landed at `ed0cce92` **before** any change to
the admitting side, so the widening reds against a real baseline rather than against tests
written alongside it.
**DIRECTION 2 SHIPPED INVERTED, and the literal form was impossible.** It asked for *"a
second trailer, so the guard can pass on a commit that NAMES the split"* — but a pre-commit
hook **cannot read the commit message**, which does not exist yet. The guard says so about
itself, and that is precisely why the ack is an env var. No amount of design makes the guard
read a declared trailer.

So the hook **writes** it. `scripts/prepare-commit-msg-session-id.sh` emits one
`Co-Authored-Session-Id` per acked sid when `CODESCOUT_INDEX_ACK` is set, and the paste step
— where all three filed instances of the trailer defect actually broke — is removed rather
than documented better.

**Order is what makes it sound, and it is measured:** git fires `pre-commit` **before**
`prepare-commit-msg`. By the time the stamp runs, the guard has already refused unless the
ack named **every** foreign owner, so the list is one the guard validated as complete on a
commit it let through. The hook decides nothing; it records a decision already checked.

**Two measurements the implementation turns on, both of which the obvious version gets
wrong.** `--if-exists` is global to one `interpret-trailers` call and the two keys need
opposite policies — `Session-Id` must stay `doNothing`, because on a rebase replaying a
peer's commit `addIfDifferent` would add ours beside theirs and invent a second author;
`Co-Authored-Session-Id` must be `addIfDifferent`, because under `doNothing` two distinct
sids write **one** trailer and every co-author after the first is dropped silently. And the
first implementation split the ack with `printf '%s'`, whose missing trailing newline makes
`read` drop the **final** element — with one sid, the only sid.

**That last one produced a test passing for the wrong reason**, which is the part worth
carrying: the dropped element happened to be the `-` sentinel a filter was meant to drop, so
*"a mixed list keeps the real sid"* went green over a filter that never executed. Written
alone it would have certified the filter. Only its neighbours' failures exposed it, and a
mutation afterwards confirmed the filter is now genuinely reached.
## Fix provenance

- **SHA:** `fc1ad175` (`experiments`) — directions 4 + 6, the ack's reach and the remedy text
- **patch-id:** `fef5458a8316af21fd0ed2db92289dfbda29ab01`
- **SHA:** `6853c517` (`experiments`) — direction 5, the orphaned-citation warning
- **patch-id:** `3616d2f2fd5ae7e2633c939b568cb4d5cfd28519`
- **SHA:** `05c6f153` (`experiments`) — direction 2, the acked co-authors written not pasted
- **patch-id:** `20ef09260e836da6f933f809b865a573970bc3db`

**Four commits, not three, and the fourth is not in this list on purpose.** `96d839c3`
(patch-id `caa8669f048c56e0dd09ced10823b9f2a2131cc6`) closed the `-` sentinel bypass and had
to land **before** the widening — `all_contested` reaches far more rows than `joint`, and
every unattributed one among them would have been ackable by a single character. It is filed
as its own record because it is a different defect that this work merely surfaced; citing it
here as a fix for *this* bug would misreport what it repaired.

**And the precondition that is not a fix at all:** `ed0cce92` backfilled the `joint`/ack test
coverage that `b37b888a` shipped without. It changed no behaviour and closes nothing, but the
widening reds against it rather than against tests written alongside the change — which is
the only reason the mutation verdicts in those commits mean anything.

**Archive-eligible and deliberately not archived.** `doc(action="move")` re-keys the artifact
and strands every inbound citation of the old id until they are repointed in the same commit.
This file was cited heavily across 2026-09-16 — by `IC-18`'s member list, by three peer
sessions' entries, and by the guard's own refusal text, which prints this path. The move is
its own act with its own checklist, not a tail of the fix.

## Workarounds

**One, and the second thing this section used to recommend is now measured WRONG.**

- **One party stages the entire coupled set** and commits it, recording the other with a
  `Co-Authored-Session-Id` trailer. Legal under the guard as written, needs no ack, and
  strands nothing. This is the only clean route.

- **~~Split by stager, ordered by citation dependency.~~ FALSIFIED 2026-09-16 — it was
  tried, at `215a5cad`, and it opened the window it was reasoned to avoid.** The reasoning
  was that the dependency is one-way: the six renames depend on nothing, while
  `architecture-boundary-session-log.md:38` cites the post-move PATHS and the peer's
  `architecture-boundary-measurement.md` cites the post-move IDS — so renames-first should
  leave every citation resolving. **Every word of that is true of the STAGED content and
  false of the resulting tree.** The exposure was HEAD's *old* copy of `architecture-boundary-measurement.md` —
  a file deliberately NOT being committed, whose citations pointed at exactly the paths the
  commit moved out from under them. Measured at `215a5cad`: **7 lines** of live
  `../issues/2026-09-13-architecture-probe-*.md` citations against paths `git cat-file -e`
  confirms absent from that tree, plus all six PRE-move ids. Repaired minutes later by
  `53ff4aa0`; nothing consumed the window. **So BOTH orders strand something** — peer-first
  strands the new ids, renames-first strands the old paths — and only the single commit
  strands nothing, which is what the coupling rule was protecting all along.

**Why no control over the staged blob could have caught it.** The check run was: all six
post-move ids appear exactly once in the staged tracker, all six pre-move ids zero times,
with the pre-column making the zeros a measurement. Sound, and blind — the failing artifact
was **not in the staged set**, so the population the control enumerated could not contain
it. This is CLAUDE.md § *Testing Discipline*'s population-vs-member law on an axis it does
not state: not *aggregate read as per-member*, but **the STAGED SET read as the RESULTING
TREE**. A commit's blast radius is every file citing what you moved, not every file you
staged.

**The check that does catch it, and it is one line.** Before a partial commit that moves or
renames anything, grep the *complement*:

```
git grep -nE '<old path pattern>' HEAD   # files you are NOT committing that cite ones you are
```

**Write the pattern against the citation's own form, and verify it fires.** The first run of
this check here used `docs/issues/2026-09-13-…` while every real citation is relative
(`../issues/2026-09-13-…`), returned `1`, and read as confirmation. A near-zero from a
pattern nobody has seen fire is the same bytes as a broken pattern.

**Do not `git checkout` or `git stash` the paths** while a deadlock is parked — the
archiver's work exists only in the working tree.
## Resume

The 2026-09-16 instance is **resolved**: `215a5cad` landed the six archives plus the
session log, `53ff4aa0` repointed the peer's tracker onto the new paths and ids, and HEAD
is consistent — verified with all six pre-move ids at 0 hits and all six post-move ids at
2 files each, the post-column being the control that makes those zeros a measurement.

This file stays **open** for the residue, which is smaller and sharper than what it was
filed for: the refusal text names no performable route (§ *Fix* 4), nothing warns on a
partial commit that orphans citations living outside it (§ *Fix* 5), and `b37b888a`'s
`joint` predicate covers a shape this corpus has not yet produced.

Credit, and it is split three ways because the halves were earned differently.
`9e022ef0-eb76-49f0-b175-4d68979290cf` hypothesised the collision from side A's refusal
alone, with the three-option framing (`go ahead` / `leave them` / `you commit`) that made
it diagnosable rather than a stuck commit — and then caught the `215a5cad` window by
reading HEAD's copy of the file that was left behind, which is the reading the committing
session did not take. `9403d62d-116b-46ea-ac9b-004acff2b1cb` added side B and the
staging-versus-authorship correction that supersedes this file's own first mechanism. The
*"both orders strand something"* finding belongs to the pair: one session measured the
dependency in the staged set, the other measured it in the tree, and neither reading alone
was the answer.
