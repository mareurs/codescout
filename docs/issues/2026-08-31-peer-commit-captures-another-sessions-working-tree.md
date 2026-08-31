---
id: e421be689a23ae2a
kind: bug
status: open
title: A peer session's commit captured this session's in-flight working-tree changes, filing them under an unrelated message
tags:
- concurrency
- shared-checkout
- git
- provenance
- multi-session
---

---
status: open
opened: 2026-08-31
severity: medium
owner: marius
related: [docs/issues/2026-08-31-cross-account-agents-cannot-see-each-other.md]
tags: [concurrency, shared-checkout, git, provenance, multi-session]
kind: bug
---

# A peer session's commit captured this session's in-flight working-tree changes, filing them under an unrelated message

## What happened

Two Claude Code sessions worked the same checkout. This session made two tracker writes
and left them staged-in-worktree while it finished verifying. Both were swept into a peer
session's commits before this session ran `git commit`.

| this session wrote | landed in | under the message |
|---|---|---|
| `open-issue-work-queue.md`, +18/−14 | `c269494c` | `docs(trackers): verify-open sweep — 9 BL-N rows reconciled against shipped work` |
| `tracker-hygiene-log.md`, entry `HY-23` | `1ec456a4` | `feat(augmentations): export the 14 shapes this host still held — corpus 9 to 23` |

HEAD was `e0f98fcf` at session start and `1ec456a4` four commits later, none of them
written by this session.

**No work was lost, and that is what makes this worth filing rather than shrugging at.**
The content is correct and on `experiments`. What is wrong is the *provenance*: `HY-23` —
a hygiene-ledger entry proposing a new drift detector — is recorded inside a commit whose
subject is an augmentation export. Every later reader of that history, human or `git
log -S`, gets a false answer about why it was written.

## Verification, not inference

Three independent checks, because "the peer happened to do identical work" is the
competing explanation and it is not idle:

- `git show --stat c269494c` reports `18 insertions(+), 14 deletions(-)` on that one file
  — byte-identical to the `git diff --stat` this session had read minutes earlier.
- Prose unique to this session is present at HEAD: `the decision was taken and shipped`,
  `Note 2026-08-31 (not yet acted on)`, `closed by the 2026-08-30 gate swap` — 3 hits.
- `git log -S'Miss: the queue holds THREE representations'` resolves to `1ec456a4`.

## Mechanism

A commit that stages by directory or by `-a` — `git add -A`, `git commit -a`, `git add
docs/trackers/` — takes whatever is dirty, and on a shared checkout that includes files a
peer is mid-edit on. The peer is not doing anything unusual; it is doing the ordinary
thing. The working tree is the shared mutable state, and `git add` has no notion of
"mine".

This is the write-side twin of
`docs/issues/2026-08-31-cross-account-agents-cannot-see-each-other.md`. That file is about
**discovery** — a session cannot enumerate who else might write. This one is about what
happens once one of them does: discovery would have let this session *know*, but even a
fully-informed peer running `git add -A` would still have captured these files. The two
need different fixes, which is why they are separate records.

## Candidate remedies

1. **Path-scoped commits as standing practice** — `git commit -- <explicit paths>`, never
   `-a` or a bare `add -A`, in any checkout that may host a second session. Cheapest, and
   it degrades safely: on a single-session checkout it costs nothing.
2. **A worktree per session** for anything longer than a single edit
   (`superpowers:using-git-worktrees`). Correct but heavy, and the librarian already
   documents worktree-shadow-row costs (`librarian(action="merge_worktree")`).
3. **Commit promptly rather than batching** — narrows the window without closing it, and
   trades against the reviewed-before-commit discipline. Weakest of the three.

Nothing here should be adopted by default without deciding (1) vs (2); this record exists
so the decision is made once rather than rediscovered.

## Observer note

The party that cannot see this is the **committing** session: `git add -A` succeeds, the
diff it commits looks like its own work plus some tracker churn, and no tool reports that
another process authored part of it. The party who can is the session whose write it was
— and only if it re-checks `git status` after finishing, which it has no reason to do
once its own writes returned `ok`. That asymmetry is the `OB-N` shape; see
`docs/trackers/observer-blindness.md`.

## Resume

Decide remedy (1) vs (2) and record it in `docs/RELEASE.md` § git workflow, which today
says nothing about concurrent sessions in one checkout.

