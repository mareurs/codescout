---
id: e421be689a23ae2a
kind: bug
status: open
title: A peer session's commit captured this session's in-flight working-tree changes, filing them under an unrelated message
tags:
- cluster/blast-radius-exceeds-visibility
- concurrency
- shared-checkout
- git
- provenance
- multi-session
opened: 2026-08-31
owner: marius
related:
- docs/issues/2026-08-31-cross-account-agents-cannot-see-each-other.md
severity: medium
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

## Instance 3 — it fired again during the commit of this file, which changes the remedy

The file you are reading was captured by the mechanism it describes, ~4 minutes after
being written. This session staged it path-scoped and ran
`git commit -F … -- <that one path>`. Git answered **`nothing to commit, working tree
clean`**: the peer's next commit, `9741e418` (*"close BL-50 — the zero that could not
answer the question it was asked"*), had already taken it.

So the count for one session is **three** — the work-queue repair, `HY-23`, and this bug
file — and the third arrived under active guard against it.

**This falsifies remedy (1) as written.** Path-scoped committing is a discipline the
*writing* session applies to its own commits, and the capture is performed by the *other*
session's `git add -A`. A session cannot protect its own uncommitted files by being
careful about how it commits them; by the time it commits, the window has already passed.
Remedy (1) is real but it is **collective** — it only works if every session on the
checkout adopts it, which is unenforceable and, per the sibling discovery bug, not even
observable.

**Remedy (2), a worktree, is the only unilateral defense**, because it removes the shared
working tree rather than coordinating access to it. Re-rank accordingly: (2) is the
remedy, (1) is a courtesy that reduces how often *you* are the capturing party, and (3)
is noise.

One consolation worth stating so the severity is not over-read: because the capture is a
plain `git add`, the *content* is committed intact every time. Nothing here is a
data-loss report. All three instances are provenance corruption, and the fix has no
urgency beyond the next reader of the log.

### Why this instance is the useful one

The first two were noticed retrospectively, by a session checking `git status` for an
unrelated reason. This one was observed **prospectively** — the remedy was chosen, applied
and defeated inside four minutes. That is the difference between a hazard someone believes
in and one whose mitigation has been tested; the belief-only version had already produced
the wrong remedy ranking above, and it took the live failure to correct it.

## Instance 4 — path-scoped committing failed, in a way this file said it would not

2026-08-31 23:34, commit `cab5c9e3`. **The capturing party this time was the session that had
spent the evening writing about this mechanism**, ten minutes after filing `IC-10` (*authorship
on a shared checkout is unrecoverable after the fact*) and while committing a bug file about a
different silent-corruption class.

It used the remedy this file recommends. `git commit -F - -- <two explicit paths>` — no `-a`,
no `add -A`, no directory pathspec. One of those paths was `docs/trackers/issue-clusters.md`,
where the session had changed a single table cell. Between that edit and the commit, a peer
added a complete new entry to the same file: `## IC-11`, its index row, and the
`entry_high_water_IC` bump. All of it landed inside a commit whose subject is a frontmatter
bug, and whose body does not mention `IC-11`.

**This is a mechanism the file did not previously cover, and it narrows remedy (1) further.**
`git commit -- <path>` does not commit the index; it commits the **working tree** at that path.
So path-scoping defends against sweeping in *unrelated* files, and gives no defence at all when
a peer is editing *the same file you are committing*. The earlier instances were all
unrelated-file captures, which is why the remedy looked sufficient. On a shared checkout there
is no commit-side discipline that protects a shared **file**: by the time you commit, the peer's
edit is already in the tree you are reading, and it is indistinguishable from your own.

So the ranking stands as Instance 3 left it, for a second and independent reason. **(2) a
worktree is the only unilateral defence** — it is the only option that stops you and a peer
sharing a working tree at all. (1) path-scoped committing is now demonstrated to be *narrower*
than it looked: a courtesy that reduces unrelated captures, and no protection whatsoever on
contended files. (3) remains noise.

One thing this instance does add in mitigation: the captured content was **complete and
correct** — `IC-11`'s heading, index row and high-water mark were internally consistent, and
`link_scan` resolves the token. As with the first three, this is provenance corruption and not
data loss. The cost is that `git log` now attributes a taxonomy addition to a bug-file commit,
and `git log -S'IC-11'` answers the question "why was this class added?" with the wrong story.

**Detection was luck, and that is the part to fix.** The capture was noticed only because the
commit's `--stat` reported 32 changed lines in a file where one cell had been edited, and the
session happened to read it. Nothing warns. A pre-commit check comparing each staged path's
working-tree hash against the hash the session last wrote would catch it — but this checkout's
`core.hooksPath` is broken (`docs/issues/2026-08-30-core-hookspath-points-at-pre-rename-path.md`),
so no hook fires here at all.

## Instance 5 — the captured side, and an announce channel that was used and did not help

2026-08-31 23:53, commit `e0525462`. Same file as Instance 4 (`docs/trackers/issue-clusters.md`),
opposite role: this time the session writing about the mechanism was the **captured** party, not
the capturing one. Five edits — IC-4's routing adjudication, IC-7's `two of three` → `two of four`,
IC-9's `Mechanism status` correction and field-block reorder, the Index's `Six` → `Eight of
eleven`, and two Index preamble notes — all landed inside a commit whose subject and 30-line body
are **entirely** about IC-6's promotion to `CLAUDE.md` and name none of them.

Verified rather than inferred: each of the six changed strings is present in `HEAD` exactly once
(`git show HEAD:docs/trackers/issue-clusters.md | grep -c`), and `git log -S'passes admission
test; hook owed'` returns `e0525462` — the IC-6 commit — as the sole introducing commit for an
IC-4 adjudication. Nothing was lost; the record is simply wrong about who did what and why.

**What this instance adds is the failure of the remaining non-worktree remedy.** Two minutes
before the commit, the captured session sent the capturing session a `SendMessage` naming every
field it was editing and asking it not to undo them. The channel existed, was used, was
specific, and was early. It did not prevent the capture, and could not have: cross-session
messages drain at the **receiver's next tool round**, and the receiver was mid-turn on a
commit. So announcing is subject to exactly the defect
`docs/issues/2026-08-30-a-transient-uncoordinated-mutation-during-an-announced-window.md`
records — an announcement is not a lock, and a window announced is still a window.

That closes the remedy space on this side. (1) path-scoping fails on contended files
(Instance 4). (3) is noise. **Announcing fails on delivery latency** — and unlike the other two
it fails *silently to the sender*, whose message returns `success: true`. Only (2) a separate
worktree removes the shared tree that all three are trying to police.

**One thing to check next time, which this instance could not settle after the fact.** The
capturing commit's message ends `Gate green: cargo test --workspace 4991/0`. On a shared
checkout a verification run and the commit citing it bracket a window a peer can write into, so
a green claim in a message attests to a tree that may not be the tree that was tested. Whether
that happened here is **not recoverable** — which is `issue-clusters:IC-10` (*authorship on a
shared checkout is unrecoverable after the fact*) applying to verification rather than to
authorship. A gate claim would need the tree hash it ran against to be checkable at all.

**This instance was itself captured, two minutes after being written** — into `3a5aec7a`, whose
subject is the remedy withdrawal and which does not mention it. Milder than the IC-6 capture
because it is at least the same file on the same subject, but the same defect: `git log -S'the
captured side'` attributes Instance 5 to a commit about something else. Recorded here rather
than as Instance 6 because the count is no longer the interesting variable — **on a contended
file the capture rate approaches one**, and a session cannot write the record of this mechanism
without the record being taken by it. That is the argument for remedy (2) stated as strongly as
it can be stated.
## Remedy (1) is a capture VECTOR, not just an insufficient defence — second falsification

Instance 3 showed path-scoped committing cannot protect *your* uncommitted files, because
the capture is performed by the other session's `git add -A`. This is worse, and it is the
opposite direction: **`git commit -- <pathspec>` can capture THEIRS.**

`git-commit(1)`: *"When pathspec is given on the command line, commit the contents of the
files that match the pathspec without recording the changes already staged."* The pathspec
form commits the **working tree** at those paths, ignoring the index. So on a shared
checkout, naming a path you edited also commits whatever a peer wrote to that same path
since you last looked — and the more careful you are about scoping, the more confident you
are in a command that reads the wrong source.

That is not hypothetical here: the peer session reports capturing this repo's `IC-11` that
way in `cab5c9e3`.

**Audit of this session's six commits: nothing captured.** `14997d36`, `5816c8eb`,
`89550d2a`, `63f135f1`, `70f4eaf6`, `1757ac11` — every file in every one is a file this
session edited. But the reason is `git status --short` run immediately before each commit,
confirming the peer's dirty set was disjoint. That is **discipline, not structure**: it
worked six times and would have failed the first time the two sets overlapped, silently,
with the commit reporting success.

The near-miss is on the record: at `63f135f1` this session edited `docs/TAXONOMY.md`, a
file the peer had committed minutes earlier and was actively working in. It was clean on
disk at that instant. Had it not been, the pathspec commit would have taken their in-flight
edit under a message describing only mine.

### The safe form, and why it is the point rather than a workaround

```
git add <paths> && git diff --cached && git commit
```

Staging first makes the index — a snapshot you chose — the thing that gets committed, and
`git diff --cached` is the read that makes the choice reviewable. `--no-verify` also
silences the check and is the wrong habit.

### Re-ranking, third time

1. **A worktree** — still the only unilateral defence, since it removes the shared working
   tree instead of coordinating access to it.
2. **`git add` then a bare `git commit`** — replaces the old remedy (1). Protects the peer
   from you. Does *not* protect you from their `add -A`.
3. ~~Path-scoped `git commit -- <paths>`~~ — **withdrawn.** It reads the working tree, so it
   is a capture vector wearing the costume of a mitigation.

### Mechanism status: shipped, by the other session

`scripts/pre-commit-unreviewed-content.sh` refuses a pathspec commit whose content differs
from what was staged, verified against four cases including the staged-then-changed-under-you
case that is instance 4. That is the shape CLAUDE.md § *Observer Blindness* asks for — a
check that runs when nobody is worried — and it closes this class for both directions of the
pathspec half. Worth noting who built it: the party who had just performed a capture, not
the party who had just documented one. Neither session could have written it from its own
evidence alone.

## The read-side twin: during a peer's pre-commit run, your uncommitted work vanishes

All of the above is about writes. There is a **read** hazard with the same root, and it was
hit within a minute of the hooks going live — by the session writing this file.

`pre-commit` stashes unstaged changes before running hooks and restores them afterwards. On
a shared checkout that stash covers **every session's** in-flight work, not just the
committing one's. So for the duration of a peer's commit:

- your edited file reverts to its HEAD content;
- `git status` reports it **clean**;
- a `grep` for text you just wrote returns **0**;
- and `git stash list` is **empty**, because pre-commit uses its own patch cache under
  `~/.cache/pre-commit` rather than `git stash` — so the obvious way to detect a stash says
  there is not one.

Measured 2026-08-31: an `artifact(update)` on this file returned `updated: true`, and the
next two reads showed a clean tree and no matching text. The natural conclusion — that a
peer had overwritten the write — was wrong. The catalog's own event log settled it: a
`field_patch` recording `prev_bytes: 9723 -> new_bytes: 12912`, and `wc -c` on the file
agreeing at 12912 once the window closed.

**Why this is worth a section rather than a footnote.** Every symptom points at data loss,
and data loss is the one failure in this file's family that would justify dropping
everything. It is also self-clearing, so a session that reacts — by re-writing the section
from memory — races the restore and can genuinely lose or duplicate work while "recovering"
from a problem that no longer exists.

**The check that distinguishes them, and it is cheap:** the catalog event log
(`artifact_event(action="list", artifact_id=...)`) records byte counts per write and is not
touched by any git operation. If the last `field_patch` matches what you meant to write,
your write landed and you are looking at a transient tree. Re-read after the peer's commit
lands before concluding anything. For non-artifact files the same role is played by the
file's own `wc -c` — compare against `git show HEAD:<path> | wc -c` rather than trusting
`git status`.

**Not a criticism of the hooks.** They close the write half, which is the half that had
already cost three captures. This is the cost side of that trade, it is small, and it is
only dangerous while undocumented.

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
