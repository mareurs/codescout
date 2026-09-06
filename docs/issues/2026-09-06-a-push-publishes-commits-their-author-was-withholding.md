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

## Root cause

Publication is a **branch-scoped** operation over a **session-scoped** permission.
`git push` has no per-commit granularity: there is no way to send only your own
commits, so the unit of the action and the unit of the authorisation do not line
up, and the mismatch is resolved silently in favour of publishing.

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

Recorded as a **limitation, not a finding**: this is one occurrence. The claim that
no git field records authorisation is checkable and was checked (no trailer, note,
or ref carries it, and `git push` has no per-commit selector); the claim that this
recurs is not yet supported.

## Hypotheses tried

1. **Hypothesis:** reading `git log origin/<branch>..HEAD --stat` before pushing
   catches it.
   **Test:** done, in the measured incident — the commits were read, with authors,
   before the push.
   **Verdict:** **rejected.** It is the right check and it cannot see this. The
   question it answers is *what am I sending*; the question that mattered is
   *may I send it*.

2. **Hypothesis:** a push freeze covers it.
   **Test:** reasoned against the incident — the freeze had been released, and the
   withholding was never about the freeze.
   **Verdict:** **rejected, and it is the more dangerous of the two**, because a
   freeze looks like it addresses this. A freeze coordinates *timing* between
   parties who all intend to publish eventually. This is a party who intends not to.

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
## Resume

Decide whether the "cannot publish → do not commit" convention is adopted, and
where it lives — `docs/RELEASE.md` and `docs/conventions/shared-checkout-commit-sequence.md`
are the two candidate surfaces, and the latter already carries the numbered
sequence this would extend.

Then decide whether it earns a mechanism or stays a policy. A convention nobody can
be reminded of at the moment it matters is a policy, and this ledger's own standing
position is that a trigger the model must notice is a policy rather than a mechanism.
