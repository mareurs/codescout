---
id: df0c18734b20fddd
kind: bug
status: taken
title: An armed mutation is a deliberate red, and no observer can distinguish it from a broken test
tags:
- cluster/transient-shared-state-lies-to-readers
topic: shared-checkout mutation testing
claimed_at: 2026-09-08
claimed_by: 5399543d-22d6-4ed9-9ebb-876be459989f
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

**None, and the mechanism field is genuinely open rather than merely unbuilt.** Two directions,
neither built, stated so the next reader does not mistake absence for oversight:

- **Announcement** — the arming session tells the tree before it mutates. Cheap, and a *policy the
  arming session must remember*, which `skill-frictions:SKF-22` records as the failure mode of
  every trigger a model must notice.
- **A marker other sessions can read** — e.g. a file the arming session touches for the duration,
  which a gate run could surface as *"a peer is mid-mutation; this red may be theirs and
  deliberate."* This is the shape `OB-1`'s third position prefers, since it runs when nobody is
  worried. Unbuilt.

The second is the only one that survives the arming session forgetting.

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
