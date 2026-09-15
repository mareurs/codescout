---
status: fixed
opened: 2026-09-15
closed: 2026-09-15
severity: high
owner: marius
related: []
tags: [cluster/transient-shared-state-lies-to-readers]
kind: bug
---

# `git log -1` answers "what is HEAD" and is read as "what did I just commit"

## Summary

`git log -1` on a shared checkout returns a **correct** answer to a question the caller
is not asking. It names HEAD; the caller means *their own last commit*. The two coincide
only while nobody else commits, which on this checkout is a condition that holds for
seconds at a time.

Observed 2026-09-15. I committed `4f21a6b1`, then ran

```
git log -1 --format=%B > msg.txt     # intending: my own commit's message
# ... edit msg.txt ...
git commit --amend -F msg.txt
```

Between those two commands **three** peer commits landed — `8289a448`, `65796b00`, and
`83d55f9f` (session `29420e72`). So `msg.txt` held *their* message, and the `--amend`
folded my staged `src/librarian/catalog/migrate_v6.rs` into **their** commit, producing
`66dd88ba` carrying their subject line and their `Session-Id` trailer.

## Symptom (Effect)

A peer's commit is silently rewritten with another session's file in it, under the
peer's own authorship trailer. Nothing errors. `git commit --amend` reported success
with a subject line I had never written — which is the only tell, and it is one a reader
skimming for "did it commit" will pass over.

The victim's exposure is real but bounded: the sha changes, so anyone who recorded it
gets a dangling reference. The author of the amend sees a stranger's subject in their
own terminal.

## Why the guard I had did not fire

The edit step asserted the message contained my anchor text and **failed correctly** —
it could not find my sentence in their message. But it was a separate statement on its
own line, so the `git commit --amend` beneath it ran anyway. `set -e` was not in force
and the two were not chained.

That is worth stating plainly: **the predicate was right and fired, and the failure
still shipped, because nothing connected the predicate to the action.** This is the
CLAUDE.md § *Testing Discipline* point about loudness being a property of a PATH, in its
cheapest possible form — an alarm nothing is downstream of.

## Fix

The positive identifier for *your own* last commit, contributed by session
`29420e72-c262-4236-82c2-52d769fdc549` after verifying this account:

```
git log --format='%H %(trailers:key=Session-Id,valueonly)' | grep <your-sid> | head -1
```

The trailer is **on the commit object**, so it survives concurrent commits, rebases and
anyone else's HEAD. `git log -1` is a question about shared state; this is a question
about yours.

**It narrows the window; it does not close it,** and the same peer said so unprompted
rather than handing over something that looked airtight: reading the sha and then
amending is still two steps, and a peer can commit between them. No form that closes it
is known. What this buys is that the *read* is now about the right object, so the
failure mode shrinks from "amends a stranger's commit" to "amends nothing, because your
sha is no longer HEAD" — which `--amend` refuses rather than silently doing.

## Repro

1. On a checkout several sessions commit to, commit something.
2. `git log -1 --format=%B > msg.txt`
3. Wait for any peer to commit.
4. `git commit --amend -F msg.txt` with anything staged.

Their commit now carries your file, their trailer, and a new sha.

## This is an instance of a rule already written here, not a new discovery

`docs/conventions/shared-checkout-commit-sequence.md` § 2 already says: *identify your own
work **positively**, by the `Session-Id` trailer or `scripts/file-provenance.py` — never by
a commit range*, because *"a range is a proxy for authorship and stops being one the moment
anyone else commits."* `git log -1` is that same proxy with a window of one.

The page does not name `git log -1` or the `--amend` path, which is the only thing this file
adds. **The rule was written down, on this checkout, and I did not read it before acting** —
I learned it existed when the pre-commit hook printed the whole sequence back at me on the
commit that filed this bug.

## Recovery, and its own cost

`git reset` back to the peer's original object restores it byte-identically — the object
is untouched in the odb and reachable from the reflog, so this is a restore, not a
reconstruction. Verified: `git rev-parse <original>` and `git rev-parse HEAD` matched
after the repair, and the peer independently confirmed their commit's shape
(51 insertions, one file, their trailer) and that the bad commit was unreferenced.

**But `--mixed` is the wrong flag for that repair on a shared checkout, and the reason
is its own bug:**
`docs/issues/2026-09-15-git-reset-mixed-silently-unstages-every-peer-on-a-shared-checkout.md`.
