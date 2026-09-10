---
id: df0c18734b20fddd
kind: bug
status: investigating
title: An armed mutation is a deliberate red, and no observer can distinguish it from a broken test
tags:
- cluster/transient-shared-state-lies-to-readers
topic: shared-checkout mutation testing
---

# BUG: an armed mutation is a deliberate red, and no observer can tell it from a broken test

## Summary

Mutation testing requires putting a **known-bad** version of the code into the tree, running the
suite, and reading the failure as evidence. On a shared checkout that failure is published to every
other session's `cargo test`, where it is **byte-identical to a real regression**.

Observed 2026-09-08. `59112612`'s default lane went red at 14:13Z on
`stripping_a_fixture_root_covers_every_rendering_not_just_the_native_one`. That `... FAILED` line
was not a defect — it was the evidence I was collecting, from a mutation I had armed deliberately,
expected to fail, and restored within a minute.

## Why this is not the same bug as `df517af91b43a5f7`

That one is a **routing** defect: uncommitted WIP reds the build, the author is knowable
(`claimed_by` in committed frontmatter), and nothing that observes a red consults it. Fix the
routing and the observer learns whose it is.

**Perfect routing does not help here.** Knowing the red belongs to `5399543d` still does not say
whether the FAILED line is a broken test or a working one under measurement. The missing bit is not
*whose*, it is *what kind* — and it exists nowhere on disk.

## The observer did everything right and still concluded wrongly

`59112612`'s own account, and it is the reason this earns a file:

- ran `git grep HEAD` for the test name → **0 hits**
- correctly concluded *"not in the committed tree, not mine to fix"*
- declined to stash, repair or commit it, per the shared-checkout sequence's step 6
- reported rather than repaired

Every step sound. The conclusion — *broken WIP* — was still false, because **the evidence available
to an outside observer cannot separate `broken` from `deliberately red right now`.** That is the
`OB` admission test passing cleanly: not carelessness, a party holding a parameter nobody else can
read.

## Why "be careful" is not available as a remedy

The usual mitigation for a peer-visible red is *don't leave the tree broken*. It does not apply,
and the reason is structural rather than a matter of degree:

| | `df517af91b43a5f7` | this |
|---|---|---|
| the red is | accidental | **intentional** |
| the author wants it | no | **yes — it is the measurement** |
| "compile before you step away" | conceivable | **destroys the evidence** |

Removing the failure removes the finding. The window cannot be shortened below the time it takes to
run the suite, and CLAUDE.md mandates mutation on the production path for exactly the guarantees
this project relies on — so the practice generating the hazard is the practice the testing
discipline requires.

## Fix

**Still unbuilt — but the two directions below are no longer symmetric, and the first one is now
known to BACKFIRE.** Measured 2026-09-09 across two announced mutation windows (03:22Z, nine
mutations in an untracked file; 04:16Z, three in tracked files), four peers announced to each time.

### Announcement was tried. It labels, and it SUBTRACTS WITNESSES.

The entry above called announcement *"cheap, and a policy the arming session must remember"* — a
`SKF-22` weakness. That understated it. Announcement is an **intervention on the population it
needs as instruments**:

> The prescribed response to an announcement is *do not investigate, stand down*. Complying is
> what removes the observer whose build log would have resolved the arming session's own anomaly.
> **The better the announcement works, the less it can observe.**

Observed directly. My 04:16Z window produced a reading I could not explain — one mutation reported
a real E-coded compile error batched and none in isolation, same one-character patch. A concurrent
build log would have settled it. `c9ab2c8d` **held off building because I announced**, and their
held-back build was that log. The anomaly is still unexplained; both offered causes were falsified.

And the 03:22Z window looked well covered only **by luck** — two peers happened to be mid-gate. I
cited that coverage as though the announcement had produced it. It had not.

Promoted to `observer-blindness:OB-23` (*a notification that changes the recipient's behaviour
cannot also measure it*), three-way attribution: `c9ab2c8d` supplied the framing and complied with
it, `59112612` retired it, this file's windows made the cost concrete.

**The split that survives:** *"is anyone building?"* is a **query** — it leaves the population
intact and returns a count. *"Here is what you will see"* is an **intervention**. They are
sequential rather than alternative (`ad379a7c`): query first for the witness count, then announce
for the labelling, which is real — without the 03:22Z announcement one peer's next move was to
bisect their own commits.

### An announcement also under-describes its own blast radius, in two ways

**Radius is what others READ, not what you WRITE.** I announced 03:22Z as *"only
`agent::build_check::tests::*` can move — no tracked file is edited"*. Both halves were wrong:
`src/agent/mod.rs` was tracked and modified (the `mod` declaration), and a concurrent
`cargo test --workspace` reads the **whole worktree** regardless of which files I consider mine.
Scoping your writes bounds nothing about a concurrent build.

**One window carried THREE failure kinds under one exit code**, and I predicted one:

| kind | in that window | announced? |
|---|---|---|
| test failures | the mutations themselves | yes |
| **clippy `-D warnings` errors** | 18 `never used` from a then-unwired module | no |
| a peer's gate red | reached a third session's run | no |

The clippy red was found in **another session's gate log**, not by me. An arming session cannot
enumerate its own blast radius, which is the same structural claim this file already makes about
the `broken` / `deliberately red` bit.

### So the marker is the direction — with one property this file did not know to require

A marker other sessions can read remains the shape `OB-1`'s third position prefers, and it is the
only one that survives the arming session forgetting. Tonight adds a constraint:

**The marker must be PASSIVE — it must label without prescribing.** A marker that reads as *"a peer
is mid-mutation, stand down"* reproduces `OB-23` exactly: it changes the reader's behaviour and
destroys the evidence. The correct shape labels a red the reader **was going to see anyway** and
asks for nothing, so the build still happens and its log still exists.

That is the same *informational, never a request* constraint the author-side build check
(`7168c1f0`) was built under, arriving from the opposite direction — there it protects the peer's
autonomy, here it protects the arming session's own evidence.

**Second property, from `59112612`:** prefer a record the acting party writes **unconditionally, at
the moment it acts**. Their pre-push log has that shape and cannot be dismissed by compliance,
because it depends on nobody else doing or not doing anything. A marker written by the arming
session at arm time qualifies; one that depends on peers reading it does not.

**Not built, and deliberately not built tonight.** Announcement is available and imperfect;
building the marker is a design change to a shared gate surface, which is an operator's call.
## Tests added

None, and none is possible from inside the arming session: the state under test is *another
session's reading* of a transient tree. What could be tested is a marker mechanism, once one
exists.

## References

- `docs/issues/2026-09-08-a-claimed-bug-file-names-the-author-of-the-wip-that-reds-the-build.md` —
  the routing sibling. Same substrate, different missing bit.
- `docs/issues/2026-09-03-the-gates-first-step-reformats-every-peers-uncommitted-rust.md` — the
  write-side of the same shared tree.
- `docs/trackers/issue-clusters/IC-12-transient-shared-state-lies-to-readers.md` — the class.

## Attribution

Observed by sessionId `59112612-5fc8-4b31-8c8c-e19220d99eac`, who reported the red rather than
repairing it and then supplied the outside-view account above — including that every step they took
was correct. Armed by `5399543d-22d6-4ed9-9ebb-876be459989f`, who could not have known it was being
read.

Filed by the arming session because the observer **structurally cannot write it**: the whole
content of the class is that the distinguishing signal exists only inside the session that armed
the mutation, and the outside view is precisely the half carrying no information.
