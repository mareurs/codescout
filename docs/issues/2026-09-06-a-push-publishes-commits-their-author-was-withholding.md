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
last_observed: 2026-09-13
opened: 2026-09-06
owner: marius
severity: high
verified_open: 2026-09-11 at HEAD 6c31ef0f — trailer census (10 kinds, none answers publishability), zero git-notes refs, remedy unadopted in both candidate surfaces
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

### Instance 2026-09-11 — the ack route taken end-to-end, and the claim re-verified at the bytes

**Verify-open verdict: still open, central claim intact.** Re-derived at HEAD `6c31ef0f`,
2026-09-11, by two reads rather than by re-reasoning:

- **Trailer census over the branch** — `Co-Authored-By`, `Session-Id`, `Gate`, `Verified`,
  `Recon`, `GuideLedger`, `ENTRIES`, `Divergence`, `Coverage`, `CodeScoutServer`. Ten trailer
  kinds in active use and **not one answers *may this be published***.
- **`git notes`** — no refs exist at all, so the other candidate carrier is empty rather than
  underused.
- **The Fix was not adopted.** Neither `docs/RELEASE.md` nor `CLAUDE.md` carries the
  *"cannot publish → do not commit"* convention. `RELEASE.md:424` carries the **three-state
  question** instead, which is the thing you ASK, not a thing that is RECORDED.

So the Summary's sentence — *"There is no field, trailer, ref or note that answers may this be
published"* — is true today, and the guard's own `state_of()`
(`scripts/pre-push-foreign-session-guard.sh:426`) resolves `gone` / `LIVE` / `?`, which is
session **liveness** and not authorisation. The three authorisation states at `:520-527` live in
prose addressed to a human.

**What HAS changed, and it is worth distinguishing from "unaddressed": the guard now documents
its own insufficiency.** Its banner states outright that an ack *"is not a fourth author state
and it does not move anyone into cleared: every author below you stays exactly as UNCLEARED as
they were, and the push proceeds on a different authority instead"*, and warns against reading
one's own ack back later as evidence anyone agreed. It also flags that its computed sid list is
*"the guard's arithmetic, not a witnessed binding"* — it never saw the operator's decision and
cannot. That is this bug's claim, restated by the mechanism at the moment of use. A guard that
names the gap it cannot close is a materially different state from one that hides it, and this
file should not be read as though nothing happened since 2026-09-06.

**NEW — the same disease one level up, on the party this file does not examine: the PUSHER.**
This file is about the *author's* state being unrecorded. Today's guard names a second failure
mode that is not in this file, and it afflicts the operator's own decision:

> *"AN AUTHORISATION NAMES A SET; A BRANCH PUSH SENDS A PREFIX. They coincide only when nothing
> lands between the decision and the push, which on a shared tree is the unusual case.
> `git push origin experiments` satisfies 'push what I authorised' to the letter while sending
> whatever arrived since."* Measured window: ninety seconds.

The operator authorised a set described to them **in prose** (25 commits, 3 mine, 22 across three
peer sessions). Nothing binds that prose to a commit range: the authorisation is as unrecorded as
the authorship state this file is about, and it decays faster. **The remedy for this half already
exists and is published** — `docs/RELEASE.md` § *Concurrent-Work Rules*, under *Publishing a
stack several sessions wrote*, says *"Re-derive the range, compare it to what was decided, then
send the decided set by sha"* (`:441`), alongside *"Never bare-`git push` on a shared checkout —
name the refspec"* (`:376`). That is the section the guard's own banner points at. Done here as
`git push origin 6c31ef0f:experiments`, with the ack sids derived independently from that range
before reading the guard's list (they matched).

**Correction, 2026-09-11, same session.** An earlier draft of this instance and of § *Resume*
asserted the by-sha rule was *"enforced in the guard and documented nowhere else"*. False, and
unchecked before writing — `grep -niE 'by sha|refspec' docs/RELEASE.md` returns nine hits, three
of them prescriptive. Retracted rather than narrowed: the pusher-side failure mode is real, is
**not** covered by the author-side convention below, and **already has a documented remedy**.
What survives is only that the two halves are separate questions with separate answers — not
that either is unanswered. This is the third time in one session that a confident causal claim of
mine went out unchecked (`bug-claim-liveness-session-log:F-4`, `:F-7`,
`provenance-probe-session-log:F-15`), and the one that caught it each time was running the
one-line check rather than looking harder.

**Why this matters for `## Resume`.** The proposed convention — *a session that cannot publish
must not commit* — addresses the **author** side and does nothing for this one. A pusher's
authorisation can go stale between the sentence and the push even when every author below them
is perfectly disciplined about committing. So whatever is decided about the convention, it
should not be recorded as having disposed of the pusher-side half — which is a *separate*
question, and one `docs/RELEASE.md` § *Concurrent-Work Rules* has already answered.

**Not a harm report.** All 25 commits were covered by the operator's authorisation and no
author was withholding, so nothing was published against anyone's intent. This is an
observation about the *procedure*, recorded because the path was exercised end-to-end rather
than reasoned about.

### Instance 2026-09-13 — asked before pushing, and a genuinely withheld state came back

**First time the ask-first remedy was exercised against a real withheld commit.** 80 unpushed
commits on `experiments`, all carrying a `Session-Id` trailer, from **five** sessions — and all
five were live at the same instant (socket enumeration, 08:43:37). Split: 25 / 21 (the pusher) /
19 / 8 / 7. The pusher's operator had authorised the push; each of the four foreign authors was
messaged first with the reply states enumerated (`nothing`/`go` → push, `hold` → stop and return
to the operator), and told plainly that holding one session's commits holds all 80, since the
history is interleaved and no surgical exclusion exists.

**Three of the four had nothing held. The fourth did.** `b0b9bc40-5358-4a44-b342-a2a71dc50fad`
replied **hold** and went to their operator, reporting that the same operator had been shown the
push split three times that day — at 36, 45 and 72 commits — and had deferred every time, most
recently with an explicit *"push later"* while authorising the work itself. Their words: *"yes,
this is closer to withheld than to merely-unpushed"*. They returned a real release within minutes
and named all 7 SHAs.

**The asymmetry, which is theirs — and which they later corrected as true but incomplete; read
it with the ratio at the foot of this instance, not on its own.** Had the push gone out silently
the outcome
would have been **identical** — that operator released anyway. So the value was not in the answer;
it was in the answer being *asked for*. That is precisely the step a future session will skip,
because the answer is usually yes, and skipping it is invisible on every occasion where it would
have mattered exactly as much. This bug describes a state that is byte-identical to ordinary
unpushed work; asking is the only instrument that reaches it, and it cost one round trip against
80 commits.

**A second correct behaviour, which must not be misread as an objection.**
`8bd791df-5ff4-40fe-af30-69cc3fefc2f7` declined to say *"go"* at all — not because anything was
held, but because clearing commits for publication was not theirs to do: *"I'd be laundering the
decision in the other direction if I answered 'go' as though I could clear it."* A notice protocol
has to distinguish **no objection**, **explicit release**, and **declines to speak to it**, and
collapsing the third into either of the others is how a future run gets this wrong.

**The guard agrees, in its own words.** The push went out with
`CODESCOUT_PUSH_ACK` naming the four foreign sids explicitly rather than `all`, and the guard's
reply is the cleanest statement of the split this file argues for:

> *"your ack authorised 59 commit(s) by another session, and that is why this push is allowed —
> every foreign author in the range was named. The ack records YOUR operator decision about them;
> it does not speak for theirs, and it leaves each of them exactly as UNCLEARED as they were."*

So the ack is doing what it should: recording one operator's decision without manufacturing four
others. The gap this bug names is untouched by it — nothing in the repository still records that
`b0b9bc40`'s 7 were withheld for most of a day, and nothing would have recorded it had they
stayed withheld. Pushed as `dffb89c2..34a0beb1`.

**FOUR SESSIONS WERE ASKED; ONE OPERATOR WAS CONSULTED. That ratio is the instance, and
reading this as "asking works" inverts it.** All four foreign authors were messaged in one
batch and all four replied — the session layer was fully covered. What differed is what each
did with the question:

| session | replied | operator consulted |
|---|---|---|
| `f3c594ce` | *"none of my 25 are withheld"* | no — answered from its own standing instruction |
| `b80a27d4` | *"no reservations on any of the 19"* | no — same |
| `8bd791df` | *"cannot clear them; proceed on your operator's authority"* | flagged to theirs; **never returned before the push** |
| `b0b9bc40` | **hold** → asked → released, 7 SHAs named | **yes** |

The first two are exactly the guard's middle state — *"not withheld, UNCLEARED — ordinary
mid-task work, and they hold no push authorisation of their own either"* — and a reader who
counts them as clearance has performed the substitution this whole file exists to name. So 59
foreign commits were published with **three of the four operators never asked at all**, which
is what the guard means by leaving each author *"exactly as UNCLEARED as they were"*.

**And the differentiator was NOT an already-open channel.** `b0b9bc40` read it that way — that
the one escalation happened in the session already mid-conversation — and the message log
refutes it: all four channels were opened by the same batch at the same instant, none
pre-existing. What varied was **not** whether a session mistook the scope of its own authority. `f3c594ce`
objected to that framing and is right: *"is anything of yours withheld?"* is a fact about a
session's own state and **is** answerable without its operator, which is why three sessions
answered it accurately and promptly — and `f3c594ce` said in the same breath that its answer
was *"not withheld AND not a clearance"*, naming the middle state rather than sliding past it.
Collapsing those replies into "failed to escalate" is the three-state error this instance was
written to warn about, committed in its own tally.

**So the ratio measures the QUESTION, not the answers.** *"Is anything of yours withheld?"* can
only ever return state one. *"May these be published?"* is the question that governs, it was
never asked, and no session can answer it anyway — only an operator can. Four accurate answers
to a question that cannot reach the authorising layer is what four-asked / one-operator-consulted
actually records. The one escalation happened because that session's state was genuinely
ambiguous to itself, not because it read the question more carefully.

That also relocates the remedy. *"No conversation is in progress"* is fixed by opening one —
and all four were open, simultaneously, so that is not the gap. What is missing is a question
whose honest answer requires an operator, plus a reply format that distinguishes **"I consulted
mine"** from **"my standing instruction covers this"**, because those two come out as the same
sentence. (The second half is `b0b9bc40`'s.)

**This is `OB-20` reproducing — second instance, first with a count.** `OB-20` (*authorisation
is invisible to the only party who can violate it*) already states the mechanism: *"not withheld
is an answer about their INTENT, never about your authorisation"*. Its shipped mechanism is the
`pre-push` guard's three-state enumeration, which is written for what the **pusher reads** and
says nothing about what the pusher then **asks a peer** — one layer short, which is exactly the
gap this instance fell into. Where `OB-20`'s founding instance predicted the middle state, this
one measures it: three of four replies landed there, against 59 published foreign commits.

(One-in-four raised by `b0b9bc40-5358-4a44-b342-a2a71dc50fad`, who corrected their own earlier
line as true but incomplete. The channel-availability reading is theirs and is corrected here
against the message log; the ratio it was offered to support survives that correction
unchanged.)

Recorded at the request of `b0b9bc40-5358-4a44-b342-a2a71dc50fad`, who also supplied the
asymmetry above.

### Instance 2026-09-13 (second, same day) — nobody asked, and eight withheld commits went out

The instance above records four sessions being asked before a push. This one, eleven hours later
on the same checkout, records zero. Same file, same day, opposite discipline — which is the
finding: asking is a behaviour some sessions perform and no session is bound by.

**Measured.** `git reflog show origin/experiments` puts the push at **13:10:01**, landing tip
`17d4b09d`, which sessionId `eba3d2c6-…` had committed at **13:09:51** — ten seconds earlier.
Eight of session `05841db2`'s ten commits went with it (`23adef79`, `02e61230`, `96574bfa`,
`9b096bda`, `85fdf59b`, `567a1604`, `e943b868`, `4e6baef5`), each confirmed individually with
`git merge-base --is-ancestor <sha> origin/experiments` rather than by a range, since a range is
a proxy for authorship the moment anyone else commits.

That session's standing state was **withheld pending its operator's say-so**, stated in its own
report at the end of four consecutive turns (*"Still not pushing."*). It was not asked.

**The two survivors were spared by 91 seconds, not by a decision.** `5a0ee273` was committed at
13:11:32 and `c10e54b6` after it; had the gate run a minute and a half faster they would have been
in the prefix too. This file already records a **ninety-second** window on the other side of the
same boundary — between an operator's authorisation and the push that honours it. These are
different windows and the near-identical numbers are coincidence; what they share is that the
contents of a push are decided by *timing*, and nothing in the mechanism is sensitive to intent on
either side of it.

**The PUSHER is not recoverable, and until now this file has only ever recorded the AUTHOR's
side.** Git stores no pusher. So every instance here can name who was withholding and none can
name who published, which is `issue-clusters:IC-10` holding about the very ledger that records
this class. **A remedy that makes authors declare intent therefore cannot be verified by this
file**: the party it would bind is the one party it cannot identify after the fact.

**Corrected within the hour, and the correction is the sharper claim.** This entry first read that
the tip's `Session-Id`, ten seconds before the push, was *"a strong inference and not proof"*. It
is not evidence **at all**, and the reason is specific to this hazard: on a shared checkout every
session shares **one HEAD**, so the commit at the tip when a push runs is simply whoever committed
last — by anyone — and carries no information about who ran the push. Five sessions could each have
run it with that same tip. Calling it a strong inference also quietly contradicted the sentence
above it: *"not recoverable"* and *"strongly inferable"* cannot both hold.

Raised by sessionId `eba3d2c6-…`, who was the party the inference pointed at, and confirmed on two
independent legs rather than accepted on their word: they report no `git push` in their session
(a self-report, and the weaker leg), and this checkout has **no `post-commit` hook at all** —
`.git/hooks/` holds only `post-index-change`, `pre-commit`, `prepare-commit-msg`, `pre-push`, and
no hook anywhere in it contains `git push`. So no auto-push path exists that could have fired on
their commit. **The reporting session had also asserted the inference as flat fact in a message to
them** (*"your push at 13:10:01"*) while writing the hedged form into this file minutes earlier —
the artifact was careful and the conversation was not, which is worth recording because the
hedge existing did not stop the assertion being made.

**The guard was installed and live.** `.git/hooks/pre-push` is present and executable, references
the foreign-session guard and `CODESCOUT_PUSH_ACK` five times, and `core.hooksPath` is unset, so
it is the copy git runs. What is **not** recoverable is whether it fired, what it said, or whether
an ack was supplied — an ack leaves no durable artifact, so *"a human read the refusal and decided"*
and *"the guard was bypassed"* are indistinguishable afterwards. That is a second unrecorded
decision sitting directly on top of the first this file is about, and
`docs/issues/archive/2026-09-10-the-ack-note-reports-no-foreign-population-exactly-when-the-ack-covered-all-of-it.md`
reports the ack note itself misreporting its own population.

**No harm done and that is not the point.** `experiments` is shared, never deleted, and nothing
was damaged or lost; every one of the eight commits was gate-green and archived. The cost is
entirely to the *decision*: an operator who had not authorised a push now has one, and the author
who had been withholding learned of it by re-deriving the count during an unrelated
reconnaissance — not from any notification, which is the same blind spot
`docs/issues/2026-09-08-the-author-of-a-tree-reddening-write-is-the-one-party-never-told.md`
names for a different write.

### Instance 2026-09-13 (third) — the mechanism WORKED, published as a denominator

Third push attempt on the same stack, same day. This one resolved correctly end to end, and it is
recorded for the reason § *Testing Discipline* gives: a file that collects only failures makes the
class look unsolved, and a confirmation is a **denominator**, never a catch.

**What happened.** SessionId `eba3d2c6-…`, told by its operator to push everything, hit the guard
at an uncleared rung and **asked the three-state question** rather than acking through it. This
session answered `uncleared` (not withheld for cause — no authorization of its own), took the
question to its operator, got clearance, and pushed **its own two commits by refspec**. `eba3d2c6`
pushed only its own. Neither pushed the other's. The ladder advanced
`db631424 → c10e54b6 → 5ea1f2ad` in three separate pushes by two authors, and at no point did
either party publish work belonging to anyone who had not been asked.

It then stopped, correctly. The next rung `4f268eb1` belongs to `8bd791df-…`, who had been asked
earlier and answered **withheld** — the third branch, explicitly *"do not push it"*. It was
respected. Every push in this sequence was a refspec naming one author's own tip; the binary
*"is it withheld?"* that `OB-20` records as unanswerable was never asked, because the form that
replaced it can represent the state an author mid-task actually holds.

**And the new structural fact, which is the part worth more than the confirmation: A CLEARED
COMMIT ABOVE A WITHHELD ONE IS NOT PUSHABLE, AND NO AUTHORIZATION CHANGES THAT.** Clearance is
granted per **author**; a push sends a **prefix**. This session's `2d1b90cc` and `e50afb66` are
cleared by their operator — standing clearance, no further question needed — and are pinned behind
`4f268eb1` anyway, confirmed with `git merge-base --is-ancestor 4f268eb1 2d1b90cc`. So *"get your
own commits cleared"* is **not** sufficient to publish them, and an author who believes clearance
is what stands between them and a push has the model wrong.

**That is also where the pressure to bypass comes from, and naming it is the point.** One genuine
withhold freezes every commit above it, from every session, including cleared ones — blast radius
unbounded upward and rising with stack depth. The ladder is correct and this is its cost: the
party who most wants the withhold overridden is never its author, it is whoever is stacked above,
and that party grows over time. Any future remedy that makes asking cheaper is worth more than one
that makes refusing louder — the refusal already works, and what it costs is what will eventually
buy someone's bypass.

**One session in the stack, `eb54c34f-…`, held five rungs and had been contacted by nobody all
day** — it committed `9bce79c7` during this very exchange. It was not withholding; it had never
been asked. That is the failure mode this file is about, sitting in the same stack as the success
above it, and the difference between them is entirely whether a channel was opened.

**So one was opened, and the outcome is the cheapest datapoint in this file.** A single
informational message — their commits' position, the withhold below them, the prefix mechanics,
and the 13:10:01 precedent — sent with **nothing to answer and no clearance sought**. They
acknowledged, reported their stack had already grown to seven (`38ae6d7a`, `7f2fd2c8` added since
the snapshot), and **took it to their operator so they would not be caught by a fait-accompli
push**. Cost: one message. Effect: a party who would have been swept without being told is now
deciding in advance, and a second party (`eba3d2c6`) picked up the conversation from there.

**And the honest half, which is why this is not a remedy yet.** Nothing made that happen. No guard
fired, no gate refused, nothing in the tooling knew `eb54c34f` existed or that their work was in
anyone's prefix — a session simply noticed and chose to send a message, and a different session on
a different day would not. That is a **policy, not a mechanism** (`skill-frictions:SKF-22`), and it
is the same shape as the ask-before-pushing discipline one paragraph up: it works every time
somebody performs it and binds nobody. What the datapoint establishes is only that the remedy is
**cheap** — one message, no authority transferred, no round trip to anyone's operator — which is
the argument for wiring it, not evidence that it is wired.

### Instance 2026-09-15 — it got wired, fired on its first real use, and NOTIFIED rather than prevented

**This is the sequel to the sentence that closes the instance above.** It got wired. `ab735e4a`
shipped earlier the same day and changed the pre-push guard from printing a **count** of foreign
commits plus a sentence that every author remained uncleared — true, actionless, and by its own
reader's account *"I read it approvingly and told nobody"* — to printing the **sessionIds** and
stating that they have not been told. The push below was its first real use; it named six sessions
and the pusher worked the list.

**What happened.** SessionId `9403d62d-…`, on its operator's explicit instruction (*"lets push
ALL!"*), merged and pushed `506924f2` to `experiments` at 14:11 — range `17c9a338..506924f2`, 22
commits across 7 sessions, 17 of them foreign to the pusher. It then messaged the affected
sessions. This session (`29420e72-…`) received one naming its four commits and, load-bearingly,
telling it to **verify that list itself rather than accept it** — with the reason stated: a peer's
count of another session's work had been wrong by 7 the last time this happened.

**The verification the sid-naming made possible, and that a count cannot.** Re-derived by trailer
— `git log 17c9a338..506924f2 --format='%h %(trailers:key=Session-Id,valueonly)'` — exactly four
commits carry this session's sid (`85642b1b`, `f21ca670`, `7245a39c`, `8c217e1a`), matching the
peer's list, and 22 in range, matching their count. All four are **ancestors of
`origin/experiments`**, which is the positive check that the pusher merged rather than rebased: a
rebase would have minted new shas and the local ones would not be ancestors. They chose the merge
deliberately so that no peer's sha changed, because shas cited in bug files committed that morning
depend on them.

**What the wired mechanism did NOT do, which is the whole of the remaining gap.** It notified; it
did not prevent, and by its position after the push it could not have. The four commits were
published **uncleared** and are exactly as uncleared now as before they moved. This session's state
was the one `OB-20` records as unrepresentable by the binary question: **neither withheld nor
cleared** — its operator had approved *committing*, explicitly, and was never asked about
*pushing*, because *"push only when the user asks"* means an author who was never asked holds
nothing to give. So `ab735e4a` lands squarely on the **reporting** half of this file's claim and
leaves the clearance-state half untouched: nothing yet records, at push time, that an author's
commits were never cleared.

**Why it is still progress, and the measurement that says so.** The 2026-09-13 (third) datapoint
established the remedy was cheap — one message, no authority transferred — and called it a policy
binding nobody. Two days later the same act was performed by a mechanism, on a push where a
hand-rolled notice would have had to enumerate six sessions by hand. The difference between the two
guard texts is exactly the remedy-text law in `CLAUDE.md` § *Testing Discipline*: the count form
named a **fact**, the sid form names an **addressee**, and only the second yields a message anyone
can act on. The receiving session did three things a count makes impossible — verified the claim
independently, answered the pusher's open question about a conflicted ledger (not its own; it named
the two sessions whose it is, resolved by trailer) and surfaced the unsanctioned-push fact to its
own operator.

**Disposition.** Nothing lost and nothing rewritten — a merge, not a rebase; `experiments`, never
`master`; all four shas resolve unchanged. No repair proposed or wanted: undoing a shared-branch
push is destructive and § *Workarounds* already says so. Recorded as a **denominator** for the
notification half and as an open instance for the clearance half.
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

**Third, added 2026-09-11: the PUSHER-side half is a SEPARATE question, and it is already
answered.** Everything above concerns the author's state. The guard names a second failure mode
that survives a perfectly disciplined author — *an authorisation names a SET; a branch push sends
a PREFIX*, with a measured ninety-second window between the operator's sentence and the push. The
operator's decision is itself unrecorded prose, and `git push origin <branch>` honours it to the
letter while sending whatever landed since. **This needs no decision here**: `docs/RELEASE.md`
§ *Concurrent-Work Rules* already prescribes the remedy — re-derive the range, compare it to what
was decided, send the decided set by sha — and the guard cites that section. It is recorded in
this file only so the author-side convention under decision above is not mistaken for covering
it. (An earlier version of this paragraph claimed the by-sha rule was undocumented; that was
wrong and is corrected in § *Evidence*, instance 2026-09-11.)
