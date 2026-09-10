---
id: d9d291b44775e50d
kind: bug
status: open
title: 'BUG: a push publishes every local commit, and nothing records that an author was withholding one pending their operator''s say-so'
tags:
- cluster/unclassified
- multi-session
- shared-checkout
- git-workflow
- authorisation
closed: null
opened: 2026-09-06
owner: marius
severity: high
---

# BUG: a push publishes every local commit, and nothing records that an author was withholding one pending their operator's say-so

## Summary

`git push` sends every commit on the branch, not the pusher's own. On a checkout
shared by several agent sessions, that means **one session's push performs an
outward-facing action on behalf of every session that has committed** — including
a session that committed deliberately and is holding the push because its operator
has not authorised publication.

Nothing in git distinguishes the two states. A commit withheld pending an
operator's say-so is **byte-identical** to one that is merely not-yet-pushed.
There is no field, trailer, ref or note that answers *may this be published* — and
the only party who knows is the author, who is not the one pushing.

**This file is the INSTANCE. The class is `OB-20`** in
`docs/trackers/observer-blindness.md`, filed the same day by the peer session that
held the other half of the incident.

## Symptom (Effect)

No error. The push succeeds, reports normally, and the withheld work is public.

Measured 2026-09-06 on this checkout. A session pushed its own commit and carried
out two others:

```
8320d5b0  docs(trackers): retire the queue entries whose blockers had already shipped
29c5b461  docs(issues,clusters): body_edits names four of its five actions, …
```

Their author had told its operator it would commit and **not** push — "commit the
reconciliation" was on its task list and "push" was not — and had said so to a
third session, explicitly asking that nobody plan around those commits landing.
The pusher had no way to learn any of that.

**The cost does not stop at the two parties, and the second half was found the
same evening.** A held commit blocks every **authorised** write stacked above it,
because `git push` is all-or-nothing over the branch. Within an hour of this file
being written, two further sessions — both fully authorised by their own operators,
neither having done anything wrong — were sitting on commits they could not
publish, because an unresolved held commit lay beneath them.

**And the block is invisible until someone tries to push.** Nothing announces it;
the commits look landed locally, `git status` is clean, and the queue is only
discovered by a party attempting the very action that would breach it. So the
rule *"a session that cannot publish must not commit"* is not merely about the
withholder's own tidiness: their commit is a **branch-wide write barrier** that
nobody else can see and only they can lift.

*(Contributed 2026-09-06 by the peer session that hit the barrier. Recorded here
rather than in their tracker because it is an effect of this defect, and the
blocked party is a third party — which is precisely what makes it worth stating.)*

## Root cause

Publication is a **branch-scoped** operation over a **session-scoped** permission.
The unit of the action and the unit of the authorisation do not line up, and the
mismatch is resolved silently in favour of publishing.

**Corrected 2026-09-06, and the correction is narrower and more useful than the
claim it replaces.** This section first read *"`git push` has no per-commit
granularity: there is no way to send only your own commits."* The second clause is
true; the first is not. `git push origin <sha>:<branch>` publishes history up to
and including `<sha>` and **nothing above it**, so granularity exists — it is
**prefix-only**, not per-commit.

That distinction decides who it helps, and it is worth stating because it is not
symmetric:

- a withheld commit **above** yours — push to your own sha and it stays unpublished;
- a withheld commit **beneath** yours — no refspec reaches past it **in one step**,
  so you cannot act alone. But the wait is not for a blanket authorisation: once
  that one commit is published it is no longer beneath you, and your own sha then
  publishes exactly your own commit. Relief arrives in **two steps**, and what you
  are waiting on is a **single named commit**, not permission covering yours.

Measured on the incident that produced this file, where the withheld commit sat
**beneath** the pusher's:

```
git rev-list --count origin/experiments..91cbdb4f   -> 1   the withheld one alone
git rev-list --count origin/experiments..c65b143f   -> 2   it, plus the pusher's
```

So publishing the single withheld docs commit costs exactly one commit; the
pusher's own then lands alone, leaving a `feat(embed)` above it still unpublished.
That converts *"authorise my whole pile"* into *"clear one docs commit"* — a
materially smaller thing to put to an operator, and what the author did put to
theirs.

*(Corrected 2026-09-06 — the **third** correction to this section, and the third
found by running a command rather than re-reasoning about one. It first said the
mechanism "changed nothing for the pusher". That is true of **unilateral action**
and false of the **outcome**, and the two had been collapsed. The pattern is worth
more than the fact: every correction in this file came from re-executing, none
from re-thinking.)*

So the accurate root cause is not that git cannot express partial publication. It
is that **the default form of the command is all-or-nothing and nothing surfaces
the other form**, so every party reasons from the default's shape and concludes a
limit that is real only for it. Both this file and the peer's said "there are two
states" while a third was one refspec away.

The pusher's available instruments answer adjacent questions and answer them
correctly:

| instrument | answers | does not answer |
|---|---|---|
| `git log origin/<branch>..HEAD --stat` | what am I about to send | may it be sent |
| `%an` / `%ae` / a `Session-Id` trailer | who wrote it | whether they may publish it |
| `scripts/file-provenance.py` | who touched a path | whether they may publish it |
| asking the peer | anything they know | requires knowing to ask, and who |

So the diligent path and the negligent path produce the same outcome, which is
what makes this worth a file rather than a note.

## Why this is `cluster/unclassified`

Two near misses, both rejected on their own claim text rather than on feel — and
the second is rejected on a distinction this ledger already makes elsewhere.

- **`IC-1`** (`blast-radius-exceeds-visibility`) asserts a session *cannot know its
  own blast radius*. Here the blast radius was known exactly: two commits, named,
  with authors, read before pushing. What was unknowable was not the extent of the
  action but its **permissibility**.
- **`IC-17`** (`shared-resource-carries-no-owner`) asserts a shared resource
  *records what changed and never who*. Git records who, accurately, and the owner
  field this class asks for is already present and populated. The missing attribute
  is a different one.

`IC-17`'s **remedy** does fit — *isolate the resource* — which is a reason to watch
the pair, not to merge them. Classifying by remedy rather than by claim is the
error this ledger warns about; if a second instance arrives, the right move is
probably a new class rather than stretching either of these, and the candidate
claim is written below.

**Candidate claim for a class, if a second instance appears:** *authorisation is
not recoverable from a shared artifact, and unlike authorship it cannot be
recovered by asking either, because the party who would have to be asked is
invisible in the artifact.* Authorship was proven recoverable three separate times
on this checkout the same evening — a session can always quote its own sessionId
from its scratchpad path. Authorisation lives in a conversation with an operator
that no peer can see, query, or infer.

## Evidence

The incident above, reported by the pushing session and confirmed by the
withholding session's own peer, both on 2026-09-06.

Recorded as a **limitation, not a finding**: at filing this was one occurrence. The
claim that no git field records authorisation is checkable and was checked (no
trailer, note, or ref carries it, and `git push` has no per-commit selector); the
claim that it **recurs** was not yet supported at filing and is now supported by a
second occurrence the following day — see below.

**One sub-claim is better evidenced than the rest, and is worth separating rather
than averaging in.** *"A push freeze does not remedy this"* is not an inference —
it was demonstrated by this very incident, which contained a real freeze, agreed
and honoured by four sessions, that the withheld commits sailed straight through
on the first push after it lifted. See `## Hypotheses tried` → 2. The occurrence
count for the defect is one; the count for the freeze being no remedy is also one,
but it is an observation rather than an argument.

**Second occurrence, 2026-09-07 — and the withholding author was the session that filed
this file.** The stack reached 13 commits with three authors interleaved. A session on
the `.claude-sdd` profile (sid `8dba66b0-af4b-4cda-a333-54a0605b318e`) put the specifics
to its operator — which commits, whose, and that pushing would publish work whose
authors had not cleared it — was instructed to push all of it, and used the `pre-push`
guard's `CODESCOUT_PUSH_ACK` escape naming three sids. Verified independently by the
withholding session rather than taken on report: its four commits are on
`origin/experiments`, the range `d5b20fbb..4b30601c` exists and holds 13 commits, and
that tip is current.

**Nobody misbehaved, which is what makes it evidence.** The pushing session followed
this guard's documented remedy exactly — *"THEN ASK YOUR OPERATOR, and do not stop at
the author"* — and its operator answered. The defect is structural and survived correct
conduct by every party, which is the second occurrence's actual contribution: the first
showed a freeze is no remedy, this one shows **the documented remedy is not one either**,
because it resolves the question without reaching the party whose work is published.

**The sharpest detail is about the escape's audit trail.** `CODESCOUT_PUSH_ACK` is a
string the *pusher* supplies, naming the authors being acked. It therefore records
**which authors were overridden**, never **that they agreed** — the two are
indistinguishable in the trail afterwards, and the withholding author here was never
contacted before the push. That is not a hole in the escape so much as a statement of
what it is for: it releases the pusher's operator's decision, not the authors'. A reader
of a future ack should not read the named sids as consent.

**And the parties could not see each other.** The deciding sessions and the withholding
author were on different profiles (`.claude-sdd` and `.claude`), so `ListAgents` showed
neither to the other in either direction; the pushing session reached the author only
afterwards, by walking `/run/user/1000/cc-socks` and resolving each PID's sessionId from
its own profile registry, matching on the `Session-Id` trailers of the commits it was
about to publish. Attribution was never the missing piece here — it worked, and it is
what made the ack precise. What was missing is a channel for *asking*, and the two are
not the same instrument.


### Instance 2026-09-09 — the author was LIVE, IN DIALOGUE, and still not asked

Strictly stronger than the founding instance, where the withholding author was invisible in the
artifact. Here the author was **reachable, mid-conversation with the pusher, and had stated the
hold in writing** — and the commit went out anyway, correctly, by the route the guard prescribes.

Sequence, all 2026-09-09:

1. `c86ebb51-7ae3-477d-b755-f25db6180782` commits `f3e7c08b`, then **declines to push it**:
   `d71f0aac` (`b80a27d4-9729-40ef-8c28-ad8982df6d13`) is its ancestor, so no push form publishes
   one without the other, and `d71f0aac` post-dated the set that session's operator had
   authorised. It surfaces the decision to its operator and says so to the peer.
2. `26cb9b5b-2c9c-489e-97d9-3a907c8b2941`, holding its **own** operator's *"push all"*, sees
   `f3e7c08b` in its range, acks both foreign sids and pushes the branch form.
   `c7de80bb..f3e7c08b`, exit 0, 14:12:26Z.
3. The messages crossed. The pusher discloses it unprompted, states it assumed rather than asked,
   and asks the author to tell its operator the commit went out under a peer's authorisation.

**What this adds to the root cause.** `CODESCOUT_PUSH_ACK` is granted by the **pusher's** operator
over **other sessions'** commits. There is no channel by which the acked author consents, refuses,
or is even notified — the ack names their sid *to the guard*, not *to them*. So the guard's
authorisation model is sound about the branch (one operator owns it) and silent about the
commit (its author is not a party). An author actively holding a commit and an author who never
existed are **byte-identical to the ack**, which is this file's claim extended one level: not only
does git not record the withholding, the guard's own consent mechanism has no slot for it.

**And the diligent path does not help, which is the tell.** The pusher read its range, resolved
every author, obtained its operator's authorisation and used the prescribed ack. The author
committed, verified, declined to push and surfaced upward.

**CORRECTION, 2026-09-09, to the first form of this entry.** It read *"'Ask the author first' is
the missing step and it is not in the guard's text, which routes the pusher to their operator and
stops."* **That is false and was written by a session that had the text in front of it.**
`scripts/pre-push-foreign-session-guard.sh:304-315` says *"Ask the AUTHOR which of three states
they are in: withheld / not withheld, UNCLEARED / cleared"*, then *"THEN ASK YOUR OPERATOR"*.
Corrected on the pusher's own objection, which cost them rather than helped them — see below.

**Provenance of the correction, recorded because the row otherwise implies the wrong thing.**
Neither party's first position was line-sourced. The author asserted the step's absence from
memory of a refusal it had read; the pusher objected that it exists, also from that refusal, having
not opened the script — and said so. The `:304-315` citation is the **first line-level source
either produced**, and the author produced it while withdrawing its own claim. So the sequence was
**unverified assertion → unverified objection → verification by the party being corrected.** A
reader would otherwise infer the objector checked the file and the author did not; neither is true.


**The accurate claim, and it is mechanizable where the false one was not.** The step exists as
**policy**. What does not exist is any **mechanism tying its outcome to the ack**:
`CODESCOUT_PUSH_ACK` encodes *who* the authors are and cannot encode *what they said*, so
performing the ask and skipping it produce **identical input and identical exit 0**. That is
`skill-frictions:SKF-22` — a trigger the model must notice is a policy, not a mechanism. **Fix
shape falls straight out:** the ack must carry the author's answer, not merely their sid, so the
three states are representable to the guard rather than only to the reader.

**Why the correction makes the pusher's part worse, in their own framing.** Had the step not
existed, skipping it would be no defect. It exists, they had read it, and they **performed it for
one author and not the other in the same push** — asked `b80a27d4` the three-state question, got
`UNCLEARED`, acted on it, and did not ask the author one rung up. So the asymmetry was not
ignorance of the step; it was that a broad authorisation over the **branch** felt like it had
already answered a **party's** position on their own commit. That is `F-127`'s modal compression
with the roles swapped — there a dependency remark was read as consent to an act, here an
operator's *"push all"* was read as covering another session's stated hold. An operator can
authorise the push and cannot answer for the author.

**So the row carries a pair, and only the second generalises:** the pusher's error is a **policy
violation**, and this file's finding is **why a policy violation there emits no signal**. Recorded
at the pusher's insistence that *"both parties followed every written rule"* not stand — they
followed all but one, and the one they skipped is in the text.


**A liveness detail worth carrying, because it nearly inverted the reading.** One call before the
push, the guard's own output rendered `b80a27d4`'s address as `[]` where minutes earlier it had
shown `[LIVE]`. Had that been read as *"author gone, nobody to ask"* the ack would have looked
like the only route rather than one of two. The address decayed; the sid did not — § *Observer
Blindness*'s rule about which component to attribute by, arriving inside the decision it governs.

**Also falsifies the fallback this file's § *Workarounds* leans on — twice over, and the compound
is the part to carry.** The refspec escape is **narrower than advertised** *and* **silent when it
does apply**, which are two independent defects in one recommended hatch:

1. `git push origin <sha>:<branch>` sends everything **reachable** from that sha, so where the
   uncleared commit is an *ancestor* there is no refspec that excludes it. Verified:
   `git merge-base --is-ancestor d71f0aac f3e7c08b` → true. Both sessions believed the refspec
   route was available as a fallback and neither had it.
2. In the cases it *does* cover, the form bypasses the guard entirely —
   `docs/issues/archive/2026-09-09-a-sha-refspec-push-bypasses-the-foreign-session-guard-which-its-own-remedy-recommends.md`
(register 1 fixed `d6847322`, patch-id `03c1fcd5aee6ca1fb98399a0386437e33c586b06`; **the fix makes
the guard SEE that form, it does not make the form safe to reach for** — the reachability point
above is unaffected, and register 2 is now fixed too at
`docs/issues/archive/2026-09-09-the-pre-push-remedy-names-a-refspec-a-zero-commit-pusher-cannot-form.md`
— the guard branches its remedy on whether the reader authors anything in the range, so the
state this instance was in no longer receives a refspec it cannot form).

So a reader who reaches for it in the one case it fits **gets exit 0 from a check that never ran.**


**Not filed as a new bug.** Same mechanism, same class; recorded here so the count is a count and
not two half-records. Reported by the pusher, `26cb9b5b`, who disclosed it before the author read
it off the remote.
## Hypotheses tried

1. **Hypothesis:** reading `git log origin/<branch>..HEAD --stat` before pushing
   catches it.
   **Test:** done, in the measured incident — the commits were read, with authors,
   before the push.
   **Verdict:** **rejected.** It is the right check and it cannot see this. The
   question it answers is *what am I sending*; the question that mattered is
   *may I send it*. Worth keeping anyway: it catches every other kind of surprise,
   and dropping a check because it missed the case that prompted it is the wrong
   lesson from a real miss. Know its scope instead.

2. **Hypothesis:** a push freeze covers it.
   **Test:** **demonstrated, not merely reasoned** — and the demonstration was
   inside this same incident, unnoticed by everyone in it. A freeze WAS in force
   that evening: four sessions agreed it, all four honoured it, and it did the job
   it was for (one CI matrix completed, the first conclusive one in four days).
   The withheld commits went out in **the first push after it lifted**. The
   session that requested and ran the freeze reported not noticing until the
   distinction was named out loud.
   **Verdict:** **rejected, and it is the more dangerous of the two.** A freeze
   coordinates *timing* among parties who all intend to publish eventually; this
   is a party who intends not to. So the freeze was irrelevant to this the entire
   time, while looking — from the inside, to the person running it — exactly like
   protection.
   **The asymmetry is what makes it a trap rather than a gap:** the rule that
   works sounds like extra caution, while the freeze that does not sounds like
   enough. A reader with a freeze in place believes they are covered, which is
   why the trap is the half a reader actually needs. Now carried by `OB-20` with
   this measurement attached, since it belongs to the class rather than to this
   occurrence.

   *(Upgraded 2026-09-06, hours after filing. This entry first recorded the test
   as reasoning rather than observation, which understated evidence that already
   existed — nobody had connected the freeze they were running to the hypothesis
   they were rejecting. The verdict did not change; its evidential weight did,
   and that distinction is the reason this note exists rather than a silent
   rewrite.)*

3. **Hypothesis:** git records nothing that identifies WHICH session authored a
   commit on a shared checkout, so the pusher cannot even name who to ask.
   **Test:** checked `%an` / `%ae`, found `Marius Ailinca` on every session in the
   checkout, and stated the conclusion to a peer.
   **Verdict:** **WRONG, and wrong by a route worth recording.** This repo's commit
   convention writes a `Session-Id:` trailer. Verified independently rather than
   taken on a peer's report: **25 of the last 25** commits carry one, partitioning
   into exactly **four** sids, each matching a live session.
   **The error was inferring an object's contents from one field's silence** — a
   negative result about the **query**, not about the world, which is the ledger's
   most-repeated law, met while writing a file about a different blindness.

   *(AMENDED 2026-09-06, and the amendment matters more than the entry. This first
   read "one format string away", implying the remedy was `%B` or `%(trailers)` —
   query better. **That is false, and it was checkable.** `git log --stat` prints
   the full message body; it always has. The exact pre-push range was reconstructed
   and grepped: `git log 53597469~3..53597469 --stat` is **187 lines** and contains
   `Session-Id:` **five times**, at lines 39, 102, 105, 175 and 178, in **two
   distinct sids** — mine at 39, the withholding session's at the rest. So the
   discriminator was **rendered, in the default view, in output read for exactly
   this purpose.* There was no better query to have run. Found by the peer
   reconstructing the range rather than reasoning about it — which is how their
   correction of me turned out to be wrong too.)*

   **So the class is not "query better", it is this:** a mechanism can be
   **unconditional on the write side and entirely discretionary on the read side**,
   and the default view can file its output where the reader is not looking.
   `scripts/prepare-commit-msg-session-id.sh` stamps every commit — 100% since
   2026-09-04 — and nothing reads it back. Worse, placement defeats it: `Author:`
   is a labelled header on line 2 of every entry, `Session-Id:` is body prose around
   line 35. Thirty years of git convention puts identity in the header block, so an
   eye seeking an owner goes to `Author:`, finds it constant across all four
   sessions, and stops. That inference is **rational given where the field sits**,
   which is why calling it carelessness would predict nothing.
   `git log --format='%(trailers:key=Session-Id)'` exists and nothing calls it.

   **And the real finding is neither error.** That hook was built FOR
   `issue-clusters:IC-10` (`authorship-unrecoverable-after-the-fact`), which its own
   header records as having had `Mechanism status: none yet` until it shipped. The
   mechanism answering "who authored this commit" has worked perfectly every day
   since 2026-09-04 — and on 2026-09-06 two sessions spent an evening on
   identification-by-elimination without knowing to look at it. A shipped mechanism
   nobody knows to read is, at the read end, not yet a mechanism.
   **What survives unchanged:** the trailer identifies the AUTHOR, never the
   AUTHORISATION. It makes the ask *cheap* — a named session instead of a
   broadcast — and does not remove it. A field that is present, accurate, and
   answers the **adjacent** question is this file's own subject, arriving one
   layer down.
## Fix

Not applied — the useful remedy is a convention, and it should be agreed rather
than declared unilaterally by one session.

**Proposed, in preference order:**

- **A session that cannot publish must not COMMIT to the shared branch.** An
  uncommitted working tree cannot be carried out by anybody; a commit can be, by
  anybody, at any time. This holds with **no coordination at all**, which is its
  advantage over every freeze protocol — it survives a participant who never read
  the convention.

  **The obvious cost-reducer does not exist here, and someone will reach for it.**
  A scratch branch is unavailable in a shared checkout: `git checkout -b` moves
  the working tree for *every* session in it, so branch-per-session presumes
  checkout-per-session. What is actually left is a stash, a patch file, or
  leaving the change dirty. (Correction contributed 2026-09-06 by the peer
  session in the incident; this file proposed a scratch branch before that.)
- **Fail-safe shape, if it can be made cheap:** the correct path ends in a state
  where nothing is armed. A withheld change living only in the working tree is
  exactly that.
- **Weakest, and named so it is not mistaken for sufficient:** the pusher reads
  what it carries. It is worth doing anyway — it catches surprises of every other
  kind — but it is structurally incapable of catching this one.

## Tests added

None, and none is possible at this layer: the missing state does not exist in any
artifact a test could read. That is the finding, not a gap in the work.

The checkable half, if the first remedy is adopted, is a hook refusing a commit to
the shared branch from a session that has declared itself unable to publish — which
requires the declaration to exist first, and that is the part that does not exist
today.

## Workarounds

The withholding session keeps the work **uncommitted** — dirty in the working
tree, stashed, or exported as a patch file.

**Not a scratch branch.** `git checkout -b` moves the working tree for every
session sharing the checkout, so it is not an isolation primitive here; it is a
branch switch performed on four sessions at once.

**The uncommitted workaround has a cost worth naming, because it is another class's
defect.** Work held dirty or stashed carries no `Session-Id` trailer, so it lands in
exactly the state `IC-10` says is unattributable — its instrument table reads
`uncommitted | none exists`. The remedy for the authorisation gap therefore re-opens the
attribution gap: commit and your work can be published without your say-so; do not commit
and nothing can say it is yours. Two classes, opposite remedies, one substrate. Neither
is wrong; there is simply no state that satisfies both today.

**The refspec ladder — a real partial workaround with a measured ceiling.** Instead of
acking, each author publishes only their own commit: `git push origin <sha>:experiments`,
bottom-up, the lowest unpushed commit first. It needs no acks at all, because each push
carries only its own author's work plus already-published ancestors. Three sessions used
it on 2026-09-07 and it **cleared four rungs and then stalled**.

Why it stalls is structural, not operational: a refspec sends a commit **and all its
ancestors**, and cannot skip one underneath. So the ladder's reach is bounded by the
position of the **lowest uncleared author**, and every commit above that author — whoever
wrote it — is unpublishable by this route. It converts *"everyone is blocked"* into
*"everyone above the lowest uncleared author is blocked"*, which is an improvement and not
a remedy.

Its failure mode is also the wrong way round. The chance that some author mid-stack is
uncleared rises with stack depth and with the number of authors, so the ladder is weakest
exactly when the stack is deep — which is when anyone reaches for it. It also needs every
author **cleared**, not merely *identified*: the `Session-Id` trailer answers the second
question completely and the first not at all, which is the distinction the whole file
turns on.
## Resume

Decide whether the "cannot publish → do not commit" convention is adopted, and
where it lives — `docs/RELEASE.md` and `docs/conventions/shared-checkout-commit-sequence.md`
are the two candidate surfaces, and the latter already carries the numbered
sequence this would extend.

Then decide whether it earns a mechanism or stays a policy. A convention nobody can
be reminded of at the moment it matters is a policy, and this ledger's own standing
position is that a trigger the model must notice is a policy rather than a mechanism.
