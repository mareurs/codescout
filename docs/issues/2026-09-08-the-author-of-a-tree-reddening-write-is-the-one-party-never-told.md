---
kind: bug
status: open
title: The author of a tree-reddening write is the one party never told
tags:
- cluster/gate-keyed-on-unobservable-event
topic: shared-checkout authorship
---

# BUG: the author of a tree-reddening write is the one party never told

## Summary

On a shared checkout, uncommitted work that does not compile reds every other session's
gate. `de546287` built the **reader-side** half of the answer: a red now names who holds
the dirty files it points at.

**The author is still told nothing.** The reader can route around the lock; only the
author can end it. And the author's own build is fine by construction — theirs is the tree
that compiles once they finish the edit — so nothing in their loop ever mentions that N
other sessions are reading their failure.

## Symptom (Effect)

2026-09-08, ~10:50–10:57. `5399543d`'s uncommitted `src/agent/write_guard.rs` did not
compile. Two sessions' gates went red; a third nearly shipped with an `unverified:` caveat
about a failure that was not theirs and had already cleared.

`5399543d`, in their own words this evening: *"I did not know I had reddened anyone this
morning. I found out from a peer message, minutes later, after two sessions had already
paid."*

They fixed the error because their own test would not build — not because they knew it was
load-bearing for anyone.

## Root cause

Every existing mechanism fires for an observer who is **already worried**:

| mechanism | fires for | author's state |
|---|---|---|
| the gate red itself | the reader | never runs it — their tree is mid-edit |
| `wip_authors` (`de546287`) | the reader | printed on someone else's terminal |
| a peer message | the reader, manually | requires a peer to notice and choose to send |

This is `CLAUDE.md` § *Observer Blindness* position 3 — *the check that runs when nobody
is worried*. At the moment it mattered the author was not worried at all, so nothing
conditioned on concern can reach them.

**It is also why the class's instance count is a lower bound by construction rather than
by sampling.** The system self-heals: the author repairs the file for their own reasons,
and the ordinary outcome of an instance is that it leaves no artifact in any ledger. Only
the instances a peer chose to report exist at all. That is the recording-filter law, where
widening the sample changes nothing.

## Reproduction

Two sessions, one checkout, shared `target/`. A saves uncommitted Rust that does not
compile and keeps working. Nothing in A's session mentions this. B's `cargo test` reds.

## Fix

Not implemented. The shape has to fire **without anyone being worried**, which rules out
anything the author must remember.

Sketch, cheapest first:

1. **Author-side, on write.** When a session writes a source file in a checkout with >1
   live session, compile-check just that file in the background and tell the *author* if it
   breaks. Costs a check per write; the trigger happens anyway, which is what position 3
   asks for.
2. **Author-side, on idle.** Same check when a session goes idle holding dirty files that
   do not compile — cheaper, and idle is when the author can act.
3. **Reader-side broadcast.** The session that gets the red messages the resolved author
   automatically. Rejected as primary: it makes the author's notification depend on a peer
   running a gate, which is the same conditional-on-someone-else's-activity shape.

**Do not build it as advice.** *"Announce before you leave the tree dirty"* is a policy the
author must remember at the one moment they have no reason to — `skill-frictions:SKF-22`'s
failure mode, and the reason this file exists rather than a line in `CLAUDE.md`.

## Workarounds

For the **reader**, which is a different problem and already solved — credited to
`5399543d`:

```bash
git worktree add /tmp/gate-$$ HEAD && cd /tmp/gate-$$ && cargo test --workspace
```

Committed state only, so a peer's dirty tree is *absent* rather than stashed, reverted or
negotiated. Needs nobody and risks nothing of theirs. **This does not help the author** —
it lets readers stop paying, which removes the only signal that currently reaches the
author at all.

## Tests added

None yet — nothing built to test.

## References

- `docs/issues/archive/2026-09-08-a-claimed-bug-file-names-the-author-of-the-wip-that-reds-the-build.md`
  — the reader-side half, fixed at `de546287`. Its closing section names this as the
  unbuilt half.
- `docs/issues/2026-09-03-the-gates-first-step-reformats-every-peers-uncommitted-rust.md` —
  why a reader repairing the author's file is not an option.
- `docs/trackers/issue-clusters/IC-2-gate-keyed-on-unobservable-event.md` — the class.

## Attribution

The observation is `5399543d-22d6-4ed9-9ebb-876be459989f`'s, made against a mechanism I had
just built and reported about their own conduct in the incident — they identified the half
their case still does not cover, having been the author in it. Filed by
`c9ab2c8d-dd74-43f4-9940-25756379a312`.
