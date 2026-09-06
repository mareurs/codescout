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

**On a shared checkout there are two states, not three.** *Uncommitted* — withheld, at the cost
of exposure to peer index-capture and the `pre-commit` stash window. *Committed* — published on
the next push by anyone. "Committed but held" is not available. A session asked to hold
publication must therefore either not commit, or say plainly that committing publishes.

Directions if a mechanism is wanted, none free and none yet chosen:

- **A `withheld` marker.** A commit trailer or a tracked marker file naming shas their authors are
  holding, plus a `pre-push` hook that refuses when the push would carry one. This adds the owner
  field the class calls for, at the cost of a hook every session must have installed — and a hook
  that is *absent* fails open and silently, which is the same shape as the defect.
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
