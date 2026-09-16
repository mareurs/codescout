---
status: open
opened: 2026-09-16
closed:
severity: high
owner: marius
related: []
tags: [cluster/shared-resource-carries-no-owner]
kind: bug
---

# Archiving a peer's bug file refuses both parties, from opposite sides of one guard

## Summary

`CLAUDE.md` **mandates** archiving through `doc(action="move")`, never a bare `git mv`.
On a bug file **authored by one session and archived by another**, that procedure
necessarily splits authorship of the same path: the source content is the author's, the
current content (status flip, fix SHA, patch-id) is the archiver's.

`pre-commit-foreign-index.sh` is keyed on single authorship per path. So it refuses the
archiver — the source half is the author's — **and it refuses the author** — the
working-tree content is the archiver's. Each refusal's prescribed remedy names the other
party. Neither can commit, and no narrowing exists, because every path in the change is
contested.

**This is not the already-filed empty intersection.**
`docs/issues/2026-09-01-two-correct-pre-commit-guards-have-an-empty-intersection.md`
(`1efc6488cb2b8946`) is **two different guards** — `foreign-index` × `ledger-counts` —
whose acceptance sets do not overlap. This is **one guard** refusing both parties, and the
splitting act is a procedure `CLAUDE.md` requires rather than a choice either party made.
Same cluster, different mechanism.

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

`doc(action="move")` is the mandated archive path **and** the act that splits a path's
authorship. The guard is keyed on a property — one author per path — that the mandated
procedure destroys by construction. Neither component is wrong; the pair has no legal
state.

## Fix

**Not designed.** Directions, uncosted, recorded so the next session does not re-derive
them:

1. **Teach the guard about renames.** Treat an `R` pair as one unit and accept it when the
   source's author and the destination's author are the only two parties on it. Narrowest,
   and it is the shape the archive actually has.
2. **Let a commit declare two authors.** A second trailer, so the guard can pass on a
   commit that *names* the split rather than hides it. Addresses attribution rather than
   routing around it.
3. **Move the frontmatter edit off the archiver.** If the author records the SHA and flips
   status, and the peer only moves, the content stops being contested — but this shifts work
   to the party who may not be present, which is the situation that produced the stale
   `status: investigating` lines in the first place.

## Workarounds

None that preserve attribution. The six files remain archived-on-disk and uncommitted;
nothing was discarded. **Do not `git checkout` or `git stash` the paths** — the archiver's
work exists only in the working tree, and the guard says so explicitly.

## Resume

The deadlock is measured from both sides and the state is parked, not lost. Whoever picks
this up: the evidence for both refusals is in this file, and the fix is a guard change
rather than a procedure change, because the procedure is the one `CLAUDE.md` mandates.

Credit: the collision was hypothesised by `9e022ef0-eb76-49f0-b175-4d68979290cf` from side
A's refusal alone, with the three-option framing (`go ahead` / `leave them` / `you commit`)
that made it diagnosable rather than a stuck commit. This file adds side B.
