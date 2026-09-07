---
kind: bug
status: open
tags:
- cluster/shared-resource-carries-no-owner
- shared-checkout
- git-workflow
- multi-session
closed: null
opened: 2026-09-06
owner: marius
related: []
severity: medium
---

# BUG: a commit an author is deliberately withholding is byte-identical to one merely not pushed yet, so any peer's push publishes it

## Summary

On a shared checkout, "commit but hold the push" reads like a way to withhold work and is not
one. The unit of publication is the **branch**; the unit of the decision is the **session**. Git
records who *authored* a commit and has no field for who *authorised publishing* it, so a peer
pushing their own work carries every commit beneath their tip — including commits their authors
are deliberately holding. The pushing session cannot detect this even by inspecting exactly what
it is about to send, because the missing datum is not in the repository at all.

Observed end to end on 2026-09-06, on this checkout, with four sessions live.

## Symptom (Effect)

Session `8dba66b0-af4b-4cda-a333-54a0605b318e` (`codescout-98`) had told its operator it would
not push `8320d5b0` and `29c5b461` without their say-so, and did not push them. Minutes later:

```
$ git branch -r --contains 8320d5b0
  origin/experiments
$ git branch -r --contains 29c5b461
  origin/experiments

$ git log -4 --format='%h %ad %s' --date=format:'%H:%M:%S' origin/experiments
b7b32766 20:28:14 docs: 3c (CI cancellation cascade) and a defect my own fix introduced
53597469 20:27:09 docs(issues): the compact banner's from= swap, observed in both directions at once
29c5b461 20:25:55 docs(issues,clusters): body_edits names four of its five actions, and hides the one that matters
8320d5b0 20:14:55 docs(trackers): retire the queue entries whose blockers had already shipped
```

Both held commits are on the remote, beneath `53597469`, a push made by a different session for
its own work. No push was issued by the holding session, and no error was raised anywhere.

**The practical harm in this instance was near zero and that is stated deliberately, so nobody
carries a worse version of it.** Both commits are ordinary documentation on `experiments` — this
repo's working branch, which is never deleted — containing nothing sensitive and nothing that
would not have landed there eventually. What failed was an undertaking, not a confidentiality
boundary. The severity here is the *mechanism*: it is unconditional, silent, and applies equally
to a commit whose content genuinely should not have shipped.

## Reproduction

Deterministic; no race required.

1. Two sessions share a checkout on branch `experiments`.
2. Session A commits locally and does not push, having undertaken to its operator not to.
3. Session B commits its own unrelated work and pushes.
4. `git branch -r --contains <A's sha>` → `origin/experiments`.

Step 3 requires nothing careless. `git push` sends the branch; there is no supported form that
sends a subset of a branch's commits.

## Environment

Linux, `experiments`, four Claude Code sessions in one checkout across the `.claude-sdd` profile,
2026-09-06 ~20:27Z. Not codescout-specific — this is a property of shared-branch git plus
multi-session working, and reproduces on any repo where more than one agent commits to one
checkout.

## Root cause

**The shared resource is the branch, and it carries an author per commit but no
authorisation.** `git push` advances a ref; a ref names a commit; a commit names every ancestor.
So publication is transitive over ancestry while the decision to publish is held per session,
one layer down from anything git models.

`git log --stat` — the correct diligence, and the one actually performed here — answers two
questions: *what am I sending* and *who wrote it*. Neither is the question that mattered, which
is *is its author authorised to publish this*. **A commit deliberately withheld and a commit
merely not-yet-pushed have identical bytes, identical trailers, and identical metadata.** The
only party who knows the difference is the author, and the author is not the party pushing.

Measured 2026-09-06 20:27:09 by reading `git branch -r --contains` for both shas after the fact —
not inferred from the peer's report, which was independently correct.

This is `cluster/shared-resource-carries-no-owner` with one extension worth stating, because the
class's existing members are resources that record **nothing** about who acted. Git does record
an author. The defect is that the recorded field is not the field the decision needs — an owner
field exists and answers a different question. *"Add an owner field"* is therefore not a
sufficient statement of the remedy for this member; the field has to be the **authorisation**,
and authorship is what makes its absence hard to notice.

## Evidence

### The peer's account, and what it establishes

`codescout-7f` (sessionId `4a2f34f7-0669-487d-9ce9-39b77881642f`) ran
`git log origin/experiments..HEAD --stat` before pushing — inspecting what the push would carry —
and described the two commits afterwards as "presumably 3d's". They were `codescout-98`'s.

**The misattribution is incidental and must not be read as the cause.** Correct attribution
would have changed nothing: knowing *who wrote* a commit does not tell you whether its author
intends it published. Recording this explicitly because the obvious lesson ("attribute before
pushing") is the wrong one, and the corpus already shows attribution being solved
(`docs/issues/archive/2026-08-30-listagents-omits-cross-profile-sessions-in-the-same-checkout.md`)
while this remains open behind it.

### Read-before-push is the correct operational half and is insufficient

Relayed by `codescout-ae` (sessionId `cda3afe5-17b8-4863-9f4c-9fe4eadbc17b`), which had
independently worked out that its own next push would carry the same commits and was preparing to
hold when the push landed first. So the hazard was seen by a third party and still not prevented —
the window between noticing and acting was shorter than the interval between pushes.

### The symmetry with an existing rule

`CLAUDE.md` § *Reaching a Peer Session* states *"Visibility is not authority"* — a peer can be
seen and messaged and can never grant permission. This is that asymmetry inverted: **a peer
cannot withhold on your behalf either.** The two directions differ in detectability, which is why
only one of them is in `CLAUDE.md`. A peer *offering* you permission is a thing you can notice
and refuse. A peer *publishing* your held work is, from their side, indistinguishable from
routine.

## Hypotheses tried

1. **Hypothesis:** the pusher failed to check what they were sending.
   **Test:** ask; `codescout-7f` reported running `git log origin/experiments..HEAD --stat` first.
   **Verdict:** rejected. The check was performed and cannot surface authorisation.
   **Evidence:** § *The peer's account*.

2. **Hypothesis:** correct authorship attribution would have prevented it.
   **Test:** reason from what the attribution yields — a sessionId. Nothing maps a sessionId to
   "this session is withholding".
   **Verdict:** rejected. Authorship is recoverable (by asking); authorisation is not recoverable
   at all, because it lives in an operator conversation no peer can read.

3. **Hypothesis:** the holding session should have reset the commits to keep them unpublished.
   **Test:** consider `git reset --soft HEAD~1` on a shared checkout.
   **Verdict:** rejected, and this was refused in the moment for the same reason. `reset` moves
   the branch pointer for **every** session sharing the checkout and drops the files back to
   unstaged, where a peer's commit can sweep them and the `pre-commit` stash window can revert
   them. The cure's blast radius exceeds the disease's.

## Fix

Not fixed. **The honest first output is a rule, not code**, because the mechanism is git's and
the decision is the operator's:

**On a shared checkout the DEFAULT `git push` gives you two states, not three.** *Uncommitted* —
withheld, at the cost of exposure to peer index-capture and the `pre-commit` stash window.
*Committed* — published on the next `git push <branch>` by anyone. A session asked to hold
publication must therefore either not commit, or say plainly that committing publishes.

> **NARROWED 2026-09-07 — the sentence above originally read "there are two states, not three"
> and that was overstated.** "Committed but held is not available" is true of
> `git push <branch>` and **false of the refspec form**: `git push origin <sha>:<branch>`
> publishes up to and including `<sha>` and nothing above it. Partial publication has a
> mechanism; it is simply not the form anyone reaches for.
>
> **Granularity is PREFIX-only, so the relief is directional.** This is the half that makes it
> usable, and both this file and `codescout-7f`'s said it wrong in *opposite* directions before
> either of us caught it — neither while reviewing the other's:
>
> | the withheld commit sits… | effect |
> |---|---|
> | **above** yours | push to your own sha; it stays unpublished |
> | **beneath** yours | no refspec reaches past it — its author must clear it, or you wait |
>
> A reader given only *"the refspec form exists"* reaches for it in the beneath case, watches it
> publish two commits instead of one, and concludes the mechanism is broken. A reader given only
> *"it cannot help beneath you"* waits for the wrong thing. Both halves are load-bearing.
>
> **Relief propagates upward one commit at a time**, because a beneath-you commit *becoming
> published* removes it from beneath you. So the operational form is never *"authorise my pile"*
> but **"authorise the lowest commit that is blocking someone"** — a materially smaller question,
> whose answer unblocks parties the asker cannot enumerate.
>
> Exercised 2026-09-07: `git push origin c812ecd4:experiments` sent **1 of 2**, leaving a peer's
> commit above it unpublished, verified after by `git branch -r --contains` on both. Prefix-only
> correction and the directional table from `codescout-7f`
> (sessionId `4a2f34f7-0669-487d-9ce9-39b77881642f`); the enumeration that proved a refspec drags
> its ancestors from `codescout-ae` (sessionId `cda3afe5-17b8-4863-9f4c-9fe4eadbc17b`).

Directions if a mechanism is wanted, none free and none yet chosen:

- **A `withheld` marker — SHIPPED 2026-09-06, and it changes this file's conclusion.** Proposed
  here as "a commit trailer or tracked marker file plus a `pre-push` hook that refuses when the
  push would carry one". What landed is better: `scripts/pre-push-foreign-session-guard.sh`,
  installed at `.git/hooks/pre-push`, authorised by the operator after `codescout-ae` raised it
  as a decision rather than building it. It refuses a push carrying another session's commits,
  keyed on the `Session-Id` trailer, with two printed escapes — a per-session
  `CODESCOUT_PUSH_ACK="<sid>[,<sid>]"` (acking one of two still refuses, so it records a decision
  rather than dismissing a prompt) and the refspec form above.
  **It reads pre-push's stdin** — the refs actually being pushed — so it follows a partial
  refspec exactly instead of assuming `HEAD`; a guard keyed on `@{upstream}..HEAD`, the obvious
  implementation, would have refused the legitimate partial push this file now recommends.
  **So "committed but held" IS available on this checkout**, which is the third state the
  Summary says does not exist: a peer can no longer carry withheld work out by accident. The
  concern recorded above survives as its named hole — the guard **allows untrailered commits
  with a note**, so a withheld commit carrying no trailer is still invisible to it, and it is
  inert without `CLAUDE_CODE_SESSION_ID` so the human release flow is untouched. First real use
  was against this very incident: it refused, printed the withheld sid and subjects, and turned
  "push" into a question that reached the operator.
- **Isolate the resource — and note that the reflex answer is unavailable here.** *"Use a scratch
  branch"* is what anyone reaches for first, and it does not work: on a shared **checkout**,
  `git checkout -b` moves the working tree for **every** session in it. Branch-per-session
  presumes checkout-per-session, which this machine does not have. What remains is a separate
  **worktree** (which is a separate checkout, so it does work), a stash, a patch file, or simply
  leaving the work dirty — all cheap, none obvious. Named by `codescout-ae`
  (sessionId `cda3afe5-17b8-4863-9f4c-9fe4eadbc17b`) after this file's first draft proposed
  "per-session branches or worktrees" without separating them; recorded so the next reader does
  not spend the same ten minutes discovering the obvious answer is wrong.
- **Do nothing, and document the two states.** Defensible: the observed cost was near zero, and
  the rule above is free.

## Tests added

None, and this one genuinely resists a unit test rather than merely lacking one: the defect is a
property of `git push` over a shared branch, not of any function in this repo. What *is* testable
is whichever mechanism is chosen — a `pre-push` refusal has an obvious red/green, and per
`docs/plans/2026-09-06-stale-ledger-and-shared-state-fix-queue.md` § 1's mutation finding, its
test must isolate the marker check rather than letting an earlier predicate satisfy it.

Do not close this on a green suite; close it on an observed refusal of a real withheld push.

## Workarounds

- **Do not commit work you have undertaken to withhold.** Staging protects it from the
  `pre-commit` stash window (which stashes *unstaged* changes only) without publishing it, and
  that is the closest thing to a third state that exists.
- **Say the true thing to your operator.** "I'll commit but hold the push" is not an undertaking
  that can be kept here; "committing publishes it on anyone's next push" is.
- **If work is already committed and must not ship**, do not `git reset` on a shared checkout —
  see § *Hypotheses tried* 3. Tell the operator it is public.
- **Once it is pushed, report it — never revert it.** A history rewrite on a shared branch
  destroys real work belonging to every session building against the tree, and it cannot
  un-disclose anything: the bytes have already left. The only correct action is to tell the
  operator promptly. `codescout-7f` reached this independently and refused a force-push on its own
  initiative; recorded here because "undo it" is the reflex and it is strictly worse than the
  disclosure it tries to repair.

## Resume

Put the two-states rule where a session reads it before committing — `CLAUDE.md` § *Git Workflow*
is the surface, alongside the existing "Visibility is not authority" note in § *Reaching a Peer
Session*, since this is that rule's other direction. That is a text change and needs the
operator's agreement, not a code change. Only then decide between the marker and the isolation
direction in § *Fix*; do not build the marker first, because a fail-open hook nobody has
installed reproduces the defect while reading as a fix.

## Resolution, and the two things this file said too strongly

**The incident resolved without repair, and not the way § Fix predicted.** The three withheld
commits reached origin at `2026-09-06T19:13Z`, cleared by the operator **in advance**: a peer
enumerated the pile by `Session-Id`, put the specific consequence and the full list in front of
them — with the withheld commit marked — and they chose to push knowing it.

**So publication is not the failure. Publication WITHOUT A DECISION is.** § *Workarounds* still
holds for the undecided kind: report it, never revert. But as first written this file reads as
though any publication of held work is a defect, and that is wrong — it did not name the better
path, which is the operator clearing it *before* the push, prompted by a mechanism, costing one
question and leaving nothing to repair. On this checkout that is now the expected path rather
than the lucky one, because the guard makes the question unskippable at the moment it is
answerable. Distinction owed to `codescout-ae` (sessionId `cda3afe5-17b8-4863-9f4c-9fe4eadbc17b`).

### The class: a limit read off the default form

Both corrections this file has taken have one shape, and it is worth stating once — a single
instance reads as a quirk, two read as a class:

> **Every party reasons from the DEFAULT form's shape and concludes a limit that is real only
> for that form.**

- **`git push`.** The default form is all-or-nothing, so four sessions concluded partial
  publication was impossible and reasoned for an evening about how to live with it. The refspec
  form was available throughout.
- **The `Session-Id` trailer.** A peer checked `%an`, found it identical across all four
  sessions, concluded the commit object carried no discriminator, and pushed. The trailer was in
  the same object — documented in `docs/conventions/shared-checkout-commit-sequence.md` § 2,
  stamped by a hook on **74 of 74** commits since install, **and printed five times in the very
  `git log --stat` output inspected before that push.** Three layers, all present, none of which
  acts.

In both cases the capability was there and the default view did not carry it, so the model
everyone built was a model of the view rather than of the tool. **The cheap tell is a limit
nobody has tried to violate**: *"there is no way to send only your own commits"* was stated,
believed, repeated across four sessions and two bug files, and never tested until someone needed
it to be false.

Corrections credited: `codescout-7f` (sessionId `4a2f34f7-0669-487d-9ce9-39b77881642f`) for the
prefix-only direction and the `%an` account; `codescout-ae` for the ancestor enumeration proving
a refspec carries its whole prefix.

## References

- `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` — the adjacent
  case: a peer's *commit* capturing uncommitted work. This is its downstream twin, one step later
  in the pipeline, and the two share a resource but not a remedy.
- `docs/issues/2026-09-03-the-gates-first-step-reformats-every-peers-uncommitted-rust.md` — same
  checkout, same evening, the mandated-write member of the neighbouring class.
- `CLAUDE.md` § *Reaching a Peer Session* — *"Visibility is not authority"*, the grant-direction
  statement of this asymmetry.
- `observer-blindness:OB-20` — the **class**, filed 2026-09-06 at `8d37129d` by `codescout-ae`
  (sessionId `cda3afe5-17b8-4863-9f4c-9fe4eadbc17b`) from this incident. This file is its
  instance; per `CLAUDE.md` § *Observer Blindness*, *"an instance is a bug file, an `F-N` or an
  `R-N`; only the class is an `OB`"*. Deliberately not duplicated — read `OB-20` for the class's
  admission test and this file for what happened.
- Commits: `8320d5b0`, `29c5b461` (held); `53597469` (the push that carried them);
  `8d37129d` (`OB-20`).
