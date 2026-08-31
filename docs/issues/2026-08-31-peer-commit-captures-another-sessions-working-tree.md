---
id: e421be689a23ae2a
kind: bug
status: open
title: A peer session's commit captured this session's in-flight working-tree changes, filing them under an unrelated message
tags:
- cluster/shared-resource-carries-no-owner
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
   tree instead of coordinating access to it. **Strengthened 2026-09-01**: it is the only
   remedy that changes *what is shared* rather than the order in which a shared thing is
   touched — see *Remedy (2) is a capture vector too* below, which falsifies (2) exactly as (1)
   was falsified, on the index instead of the working tree.
2. ~~**`git add` then a bare `git commit`**~~ — **withdrawn 2026-09-01.** It was promoted here
   as "protects the peer from you"; it does the opposite. `git add` writes to the one shared
   `.git/index`, so a peer's bare commit — this exact prescription — carries your staged work.
   Observed at `1b40dabd`, and the `unreviewed-content` gate reads such a commit as clean
   **by construction**, because nothing about it is unstaged.
3. ~~Path-scoped `git commit -- <paths>`~~ — **withdrawn.** It reads the working tree, so it
   is a capture vector wearing the costume of a mitigation.

**All three commit-side remedies are now withdrawn, and they failed for one reason:** each
operates on state that is per-**checkout** (the working tree, the index) or on timing the sender
does not control (message delivery). Git has no concept of a session, so no commit-side
discipline can express "mine". Only (1) removes the sharing.

**Remedy (1) is not merely insufficient — it is scoped to the wrong axis, and instance 5 is the
proof.** The capturing session reports it did **not** use `git add -A` for `e0525462`: it staged
two explicit paths, which is exactly the remedy it had drawn from the first capture two hours
earlier and told its user it had learned. *(That is its account of its own action, which git
cannot confirm after the fact; the only corroboration available is that the commit touches
exactly two files, consistent with explicit paths and not proof of them.)* It captured five of a
peer's edits regardless — because explicit-path staging defends against sweeping in **other
files**, and both sessions were editing **one file**. The two diffs are merged in the working
tree before `git add` ever runs, so the pathspec never had anything to exclude.

So a session following remedy (1) exactly, deliberately, immediately after being burned by its
absence, still captured a peer. That is a stronger statement than "the remedy is weak": a weak
defence fails sometimes, and this one **cannot** engage on the contended-file axis at all. Pair
it with the read side — a session cannot write the record of this mechanism without the record
being taken by it (instance 5's own capture) — and the two say the same thing from opposite
ends: one names why the obvious fix does not reach the case, the other names how reliably the
case arrives.
### Mechanism status: shipped, by the other session

`scripts/pre-commit-unreviewed-content.sh` refuses a pathspec commit whose content differs
from what was staged, verified against four cases including the staged-then-changed-under-you
case that is instance 4. That is the shape CLAUDE.md § *Observer Blindness* asks for — a
check that runs when nobody is worried — and it closes this class for both directions of the
pathspec half. Worth noting who built it: the party who had just performed a capture, not
the party who had just documented one. Neither session could have written it from its own
evidence alone.

**What it does not close, named by the party who walked into it.** The gate makes you *read*
what you are about to commit; it does not make co-authored hunks **distinguishable**. Had the
capturing session read `git diff --cached` on `e0525462`, it would have seen six hunks in a file
it had itself edited three times that evening, with nothing in the diff marking which three were
the peer's. The gate closes *committed without looking*. It does not close *looked and could not
tell* — and on a contended file that is the harder half, because the reader is not being careless
and more care does not help. Closing it needs per-hunk provenance the working tree does not
carry, which is `issue-clusters:IC-10` (*authorship on a shared checkout is unrecoverable after
the fact*) stated as a tooling gap rather than a narrative one. State the boundary wherever the
gate is cited, so the next reader does not take it for more than it is.
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

## Remedy (2) is a capture vector too — third falsification, and the shipped gate cannot see this one

2026-09-01, commit `1b40dabd` (*DC-4 — four deferral rationales outlived the counts beneath
them*). It carries this session's `OB-6` promotion in full — 31 lines of `observer-blindness.md`
and 20 of `issue-clusters.md`, verified by three distinct marker strings each present exactly
once in the commit — under a subject about neither.

**The vector is new, and it is the remedy.** Every previous instance was a *working-tree*
capture. This one is an **index** capture: `git add` writes to `.git/index`, and a checkout has
exactly one. The capturing session reports running `git add <one path>` and then `git commit`
with no pathspec — the precise form *Re-ranking, third time* promoted as remedy (2) — and a bare
commit commits **the whole index**, not the paths named to `add`. None of the three files in
`1b40dabd` overlaps the two sessions' edits at all: they had correctly avoided each other's
*files* this time, and sharing the index was enough on its own.

**So remedy (1) fails at three distinct layers, which is a better statement than "the wrong
axis".** Each layer is a different instance, and "stage explicitly" answers only the first:

| layer | what defeats it | instance |
|---|---|---|
| `git add -A` sweeps untracked files | nothing was scoped at all | 3 (`9741e418`) |
| explicit paths, **co-edited file** | both diffs merge in the working tree before `add` runs | 5 (`e0525462`) |
| explicit paths, **peer pre-staged** | the index already holds their content and `commit` takes all of it | 6 (`1b40dabd`) |

**And the gate does not fire — but "blind" understates it: the gate steers you off the safe
form.** `unreviewed-content` refuses *a pathspec commit carrying unstaged content*. The
capturing run printed `Passed`, correctly, because it was **not** a pathspec commit. Now note
the inversion: this session was **refused** minutes earlier for `git commit -- <paths>` — and a
pathspec commit **ignores the index**, so it is precisely the form that would have prevented
this capture. The hook guards a real and different hazard (committing working-tree content
nobody reviewed), but the two hazards pull in opposite directions, and the consequence is
concrete: **a session holding unstaged content in its own target paths currently cannot use the
index-safe form.** Its only route is stage-then-bare-commit, which is the vector above.

That tension is not resolvable by tuning either check, and neither session should resolve it
unilaterally — recorded here as an open design question. What would close both at once is the
same per-session provenance named under *Mechanism status*: with a record of what this session
wrote, a pathspec commit could be allowed when the unstaged content in those paths is its own,
and a bare commit refused when the index holds anyone else's.

**The self-inflicted half, which generalises furthest.** The capturing session did run
`git diff --cached --name-only`, and it did print all three files — but it was chained to the
commit in one `&&` sequence, so the output arrived *after* the decision was made and could only
be read in the transcript afterwards. **A verification chained to its own action with `&&` is
decoration: it produces evidence and cannot act on it.** The remedy is ordering rather than
care — stage, stop, read `--cached`, then commit as a separate call.

**But ordering closes only the outbound half, and this session is the evidence.** It staged,
stopped, read the cached diff in its own call, confirmed 6 hunks all its own — and was captured
anyway, because `1b40dabd` landed in the gap between that read and its `commit`.

**That is a time-of-check-to-time-of-use window on shared index state, not a discipline
failure** — the check was correct when it ran and false when it was acted on, with **no action
of this session's in between**. The distinction matters because it settles what can be asked of
a session: the outbound half (*do not commit what you never read*) is closeable by ordering, and
the inbound half is **not closeable by any per-session behaviour at all**, because the state it
depends on is written by a process this session can neither observe nor lock. State that flatly
wherever the remedies are ranked, so the next reader does not go looking for a discipline that
cannot exist. It is also why the ranking now has one entry: a worktree is not the best available
discipline, it is the removal of the shared object the others contend over.

Two **structural** candidates are named rather than recommended, because neither has been
probed: per-session index isolation via `GIT_INDEX_FILE`, and the pathspec commit form, which
ignores the index entirely — the same form the `unreviewed-content` gate currently refuses, which
is the inversion above. Both need a probe before either is written down as advice. Recorded this
way deliberately: three of tonight's wrong claims came from asserting an unprobed mechanism, and
two of those were remedies.

That makes three remedies falsified by the mechanism they were proposed against, each in the
form *the thing you were told to do is how it happens*:

| remedy | prescribed because | falsified by |
|---|---|---|
| `git commit -- <paths>` | excludes unrelated files | commits the **working tree** at those paths — no defence at all on a contended file (instance 4) |
| announce via `SendMessage` | coordinate before the write | messages drain at the receiver's **next tool round**; a mid-turn peer never sees it, and the send returns `success: true` (instance 5) |
| `git add` + bare `git commit` | "protects the peer from you" | writes to the **shared index**, which a peer's bare commit then takes — and the gate reads it as clean (this section) |

The common shape, stated once: **every commit-side remedy operates on state that is
per-checkout rather than per-session** — the working tree, the index, or delivery timing the
sender does not control. A session is not a unit git knows about, so no commit-side discipline
can express *mine*. That is why a worktree is not merely the best of these: it is the only one
that changes **what is shared**, where the others rearrange who touches a shared thing first.

**Not filed as instance 6.** The count stopped being the informative variable two instances ago;
what this adds is a vector and a falsification, which is what the ranking consumes.

## Detection — the commit's own `--stat` is the only check that fired, and it fired for free

2026-09-01, commit `d617051b`, capturing session `codescout-d9`. This is **layer 2** of the table
in *Remedy (2)* above (explicit paths, co-edited file), so it is **not filed as a new instance** —
the vector is already recorded and the count stopped being the informative variable. What is new
is that the capture was *detected*, within one call, and the detector is cheap enough to make
deliberate.

**What happened.** Verified `git diff -- docs/trackers/issue-clusters.md` in its own call: 23
insertions / 16 deletions, 9 hunks, offsets recorded, all mine. Ran
`git add <that one path> && git commit -F <msg>` roughly 40 seconds later. A peer wrote its
`IC-1` / `OB-8` split work into the same file inside that gap. The commit landed **58 insertions
/ 19 deletions**, carrying three hunks belonging to another session under a message describing
only mine.

**Every guard passed, and each was right to.** `unreviewed-content` printed `Passed` — this was
an **index** commit, not a pathspec one, so the hook is out of scope by its own § *WHAT IT DOES
NOT CATCH*, which names this case and grants it *"the content was staged and is presumed
reviewed"*. **That presumption is the hole.** `git add <path>` stages the working tree *as of the
add*, so "reviewed" and "staged" are different instants; a review performed before the add
measures a state the add is not obliged to see. The gap needs no peer malice and no file
ambiguity — only elapsed time.

**The one signal that survived is the number.** `git commit` prints its own `--stat`, and `58/19`
against a remembered `23/16` is unmissable. A foreign hunk cannot leave the count unchanged. That
is the property worth leaning on: it holds regardless of which layer the capture came through,
whether the two sessions' files overlap, and whether the peer is enumerable at all — the three
axes every other check on this page turns out to depend on. It is **detection, not prevention**:
the commit has already happened when the number arrives.

**So the discipline is one comparison, not one more step.** Measure the count before staging;
read the commit's own stat line against it. Both halves already happen in any careful sequence —
what is missing is comparing them. This does not close the inbound half, and this file already
explains why nothing per-session can. But a *detected* capture is disclosable, and *Instance 5*
together with `bug-fix-session-log:W-70` both argue disclosure is what prevents the downstream
damage — a captured session re-running `append_entry` and allocating a fresh id for content
already in `HEAD`.

**One measurement trap, from a peer that ran the check successfully the same hour** (`5d405b67`):
`git diff | grep -c '^+'` counts the `+++ b/<path>` header line, so its raw 117/3 was really
116/2 — which is what `--stat` then reported. Compare `--stat` against `--stat`, or subtract the
two header lines. A one-line discrepancy in either direction reads exactly like a small foreign
hunk, which makes the trap worse than a wrong number: it manufactures a false positive for the
very mechanism the check exists to find.

## Candidate remedies

> **Superseded by *Re-ranking, third time* above — kept for the reasoning, not the ranking.**
> This list was written after instance 1, when every observed capture was of an *unrelated*
> file, and it ranks path-scoped commits first on exactly that evidence. Instances 4 and 5
> falsified it: (1) is withdrawn, (2) is promoted to first. Preserved rather than rewritten
> because the argument below was correct about the axis it could see — which is the point,
> not an excuse for it.

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
