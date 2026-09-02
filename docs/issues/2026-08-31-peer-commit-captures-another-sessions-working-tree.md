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
`core.hooksPath` is broken (`docs/issues/archive/2026-08-30-core-hookspath-points-at-pre-rename-path.md`),
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
`docs/issues/archive/2026-08-30-a-transient-uncoordinated-mutation-during-an-announced-window.md`
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

## Instance 6 — 2026-09-02, and the capture split a TWO-FILE change across the commit boundary

| fact | value |
|---|---|
| capturing commit | `2fc064f7` — *"fix(prompts): the activation banner named a param two of its three tools drop"* |
| captured hunk | `TOOL_SURFACE_CHAR_BUDGET` raise 56_519 → 56_547 in `src/server.rs`, plus its ~13-line rationale doc comment |
| capturing commit's `--stat` | `src/server.rs | 183 ++++-` alongside `src/prompts/mod.rs`, `src/tools/memory/{mod,tests}.rs`, one spec |
| captured session's own files | `src/agent/mod.rs`, `src/peer/server.rs`, `src/tools/config/mod.rs`, one guide — all still uncommitted, none captured |

**Verification, not inference.** `git log -S'read-only-true-is-inert' -- src/server.rs` and
`git log -S'still wrong about half its domain' -- src/server.rs` each return `2fc064f7` and
nothing else. Both strings were authored in the captured session, in a doc comment that did not
exist before it. The pickaxe names the commit; no adjacency argument was needed.

### What is new: the capture left a defect at HEAD that NEITHER session can see from its own side

The captured hunk was a **budget raise**. The change that justified it — a 31-char schema
description in `src/tools/config/mod.rs` — was **not** captured and is still uncommitted. So
HEAD now carries `TOOL_SURFACE_CHAR_BUDGET = 56_547` while the bytes accounting for 31 of it
are absent. That is precisely what the constant's own doc comment forbids:

> Set to the exact measured total, never rounded up: the ratchet still bites on the very next
> added byte, which is the only thing keeping this honest.

The ratchet is now silently loosened at HEAD, and `tool_surface_under_budget` passes with slack
rather than on the line. The blindness is symmetric, which is why it belongs in this ledger:

- the **capturing** session cannot see it — the raise is not theirs, so it reads as
  pre-existing, and their own change is complete and consistent;
- the **captured** session cannot see it either without separately inspecting HEAD, because
  *its* working tree is consistent: both halves are present there.

So this is a sharper harm than "an unrelated hunk under a wrong message," which is how instances
1–5 read. It is an **internally inconsistent hunk** under a wrong message — a coupled two-file
change bisected by the capture. Any check that asks "is this commit's diff self-consistent?"
would have to know the coupling, and nothing records it but the prose in the doc comment that
travelled with the wrong half.

**Detection that worked, again for free:** the capturing commit's own `--stat` (§ *Detection*).
183 changed lines in `src/server.rs` under a message about a prompt banner is legible on its
face. The content check would also have fired here, but only for a reader who knew which
strings to pickaxe — which the captured session did and the capturing one could not.

**Noted, not attributed:** the same tree carries an untracked file literally named
``head.\naab0c4ef'\"s`` — a shell-quoting accident from some session, sitting in the repo root.
Recorded only as a measure of how much concurrent shell traffic this checkout carries.

### Correction, same session: the coupling got STRONGER, because a third party then measured against the captured tree

The section above is right that HEAD carries a raise whose justifying bytes are uncommitted, and
right that the slack is 31. What it missed, because it happened minutes later, is what the
capturing side did **next** — and that is the part worth keeping:

| step | commit | total | budget | note |
|---|---|---|---|---|
| baseline | — | 56_516 | 56_519 | 3 chars of headroom |
| captured session's `+31` schema fix | *uncommitted* | 56_547 | — | measured in the shared tree |
| raise captured | `2fc064f7` | — | **56_547** | raise + rationale swept in under a prompts message |
| peer trims `memory`'s schema by 50 | `a55396ec` | 56_497 | **56_497** | *"the first payback, and it retires one of the two debts above by its exact size"* |

The payback is correct arithmetic and exactly what the constant's doc comment asks for — and it
is **measured against a tree containing another session's uncommitted 31 bytes.** So HEAD's
budget is now only honest *if that session's change lands*. Revert it and the total drops to
56_466 against a 56_497 budget: 31 chars of silent slack, in a constant whose entire purpose is
to have none.

So the capture's harm compounds rather than sitting still. A hunk under a wrong message is a
bookkeeping problem; a **shared invariant re-derived from a tree that mixes committed and
uncommitted work from two sessions** is a correctness problem, and neither party can detect it:
the measurer's reading was accurate for the tree in front of them, and the captured session's
tree is self-consistent. The only state that is wrong is the one nobody's working copy shows —
HEAD.

**What this adds to § *Candidate remedies*:** a check on the *commit* cannot catch it, because
no single commit is wrong. It needs a measurement taken against a **clean** tree — `git stash`
is the obvious way and is itself a capture vector (§ *The read-side twin*), so on a shared
checkout the affordable form is to derive the number from HEAD in a scratch worktree, never
from the working copy. Any budget, baseline or golden value re-derived in a shared checkout has
this exposure, not just this constant.

**Credit where due:** the payback itself is the mechanism working. The peer read the rationale
that travelled with the captured hunk and acted on it — which is the one thing that stopped the
raise from becoming permanent, and an argument for recording *why* at the constant rather than
in a commit message that the capture would have detached anyway.

**Closed 2026-09-02 at `1559daa5`** (patch-id `3e86d303136e5192d1761b91058b65bdeb3612df`). The
second half of the split — `src/tools/config/mod.rs` — landed, so HEAD's
`TOOL_SURFACE_CHAR_BUDGET = 56_497` is once more the exact measured total rather than carrying
31 chars of slack. The window was roughly 90 minutes.

Two things that window taught, neither of which is "commit faster":

- **The coupling was only knowable from one side.** The measuring session could not have
  detected it — its reading was correct for the tree in front of it — and it acted correctly
  when told, declining to touch the file (*"it is yours, it is staged in your bug file, and the
  31-byte slack is only real if your `src/tools/config/mod.rs` does not land"*). So the repair
  channel was a message, not a check. Worth recording because § *Candidate remedies* is a list
  of mechanisms, and this instance was closed by a peer being **told** rather than by any of
  them firing.
- **A failed commit strips ownership from the very rows the next attempt is judged on.**
  Retrying the commit was refused by `foreign-index` with `theirs:` listing all ten of *my*
  paths and `Staged by: (unrecorded) — not a staging command`. Cause is in the hook's own text:
  pre-commit stashes unstaged files, and that index write comes from a parent the recorder
  cannot attribute, overwriting rows that carried this session's id minutes earlier. So the
  guard's failure mode under retry is to call your own staged work foreign — which is the safe
  direction, and it is also indistinguishable from a real capture at the point of use. Filed
  separately; the pathspec form the hook recommends is the working route.
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


### The competing check that looks stronger and is structurally blind — 2026-09-01, `0c32bb85`

Same layer, third occurrence, **not** filed as a new instance for the reason given above. It
is recorded here because it falsifies a check a reader of this section would plausibly reach
for *instead of* the stat comparison, believing it more precise.

**What happened.** Session `c2a08c22` verified `git diff -- docs/trackers/bug-fix-session-log.md`
in its own call: **23 insertions / 3 deletions**, its own W-91 correction and nothing else. Ran
`git add <that one path> && git commit -- <that one path>` roughly 30 seconds later. Peer
`codescout-e6` wrote `F-93` into the same file inside that gap. The commit landed **66
insertions / 4 deletions**, carrying that entry and the `entry_high_water_F: 93` frontmatter
bump under a message about a law citation.

The stat comparison worked exactly as this section says: `66/4` against a remembered `23/3` is
unmissable, and it is what caught it.

**The new part is the check that ran alongside it and returned a false all-clear.** In the same
verification call:

```
git diff docs/trackers/bug-fix-session-log.md | grep -c "F-92\|F-91\|W-93"   ->  0
```

Zero, correctly — and worthless. The pattern is an **enumeration of the peer entries already
known to exist**, so it cannot match `F-93`, and a new entry is the only kind a peer writes.
Confirmed at the bytes: `echo 'F-93 entry' | grep -c "F-92\|F-91\|W-93"` returns `0`. The
check was incapable of firing at any time, on any content, in the direction that mattered.

**Why this belongs on the page rather than in a session log.** A content grep *reads* as the
stronger instrument — it names the thing you are afraid of, where a line count is merely a
line count. It is strictly weaker, and it fails in the reassuring direction: it is a gate whose
predicate is derived from what its author already believed was in the file, which is `R-5`'s
self-validating shape (*"a check computed from the thing it judges cannot fail"*). The stat
comparison is dumber and it is the one that holds, for the reason this section already gives
— **a foreign hunk cannot leave the count unchanged**, and it needs to know nothing about who
the peer is or what they wrote.

The sharper statement of why, from `codescout-e6`, who explained the inversion rather than
just recording it: **a count makes no claim about what the foreign content is, which is
exactly why nothing about the foreign content can defeat it.** Every content predicate is a
hypothesis about what the peer wrote, and the peer is the one party you cannot poll.

**This is an `OB-1` instance** — *the parameter your own context supplies for free*. The grep
pattern was under-specified in exactly that class's sense: its author read it already holding
the set of entries they believed were in the file, so the missing parameter (*which entries
might arrive*) was invisible to them and to no one else. That class's *Who can see it* field
predicts the finder correctly — not a more careful version of the same author, but a reader
not sharing their context. Cited here so the instance is reachable from the class; the class
needs no new row.

So the rule stays exactly as stated: compare `--stat` to `--stat`. Do not substitute a content
search for it, and do not treat a content search as corroboration — a grep whose pattern you
wrote from memory is evidence about your memory.

**Disposition of the captured content — and the entry is SPLIT, which is the part worth
carrying forward.** `F-93` is `codescout-e6`'s work. Its **body** was captured into
`c2a08c22`'s `0c32bb85` (05:05:57); its **index row** landed twenty seconds later in that
session's own `26f5b496` (05:06:17). Verified both directions:

```
git log -S"## F-93 —"            -- <tracker>  ->  0c32bb85   (captured)
git log -S"| F-93 | 2026-09-01"  -- <tracker>  ->  26f5b496   (author's own)
```

So a single ledger entry can be **split across two commits with two different owners**, and
neither `git log -S` on the body nor on the row alone reveals it — each returns one commit
and looks complete. This follows directly from this repo's own append discipline, which
writes the section first and the index row *after*: that ordering puts a commit boundary
where a peer's `git add` can fall. Anyone reconstructing authorship from `git log -S` should
probe **both** shapes of the same entry.

Note also that `%an` reads `Marius Ailinca` on both commits — git's author field is constant
across sessions (`IC-10`), so the `Session-Id` trailer is the only thing separating them,
and it exists on both because both were committed. During the window it did not.

Not rewritten: `git reset` on a shared index is the vector *Remedy (1)* documents, and
trading a durable attribution error for a live one is a bad exchange. Disclosed to that
session directly, per *Instance 5*'s finding that disclosure is what prevents the downstream
damage — specifically a captured session re-running `append_entry` for content already in
`HEAD`.

### Second detection, 2026-09-01 `0c32bb85` — and this time a CONTENT check gave a false all-clear

Same layer, same shape, disclosed by the capturing session (`codescout-68`) rather than found by
the captured one. Verified `git diff` on `docs/trackers/bug-fix-session-log.md` at **23/3**, ran
`git add && git commit -- <path>` ~30s later, and it landed **66/4**. The 43-line gap is this
session's `F-93` entry body, written into the window. Confirmed from the other side:
`git log -S"## F-93 —"` names `0c32bb85`, while `git log -S"| F-93 | 2026-09-01"` names
`26f5b496` twenty seconds later — **one entry split across two commits under two authors**, the
body filed under a message about a law citation and the index row under its own.

Not rewritten. `git reset` on a shared index is the vector *Remedy (1)* documents, so the trade
is a durable attribution error against a live one, and the durable one is cheaper.

**The new finding is the check that did NOT fire.** Alongside the stat comparison, the capturing
session ran a content check and got a clean zero:

```
git diff docs/trackers/bug-fix-session-log.md | grep -c "F-92\|F-91\|W-93"   ->  0
```

That zero is correct and worthless. **The pattern enumerates the peer entries its author already
knew existed** — and a peer's next write is, necessarily, an entry that does not yet exist.
Measured on the actual captured text: the pattern returns **0** against `## F-93 — Two counts
over a live append-only transcript`, and **1** against `## F-92`. It fires only on what is
already known and never on what is arriving, so it could not have fired in the direction that
mattered — at any time, on any content, no matter how long the window.

This is `reconnaissance-patterns` R-5's shape (a check whose predicate is derived from the thing
it judges, so it cannot fail) meeting the negative-search law (a predicate written from memory
fails silently while the instrument answers in its own terms). What makes it worth recording
here rather than there is the **ranking inversion**:

| check | reads as | actually |
|---|---|---|
| `grep` for known entry ids in the diff | specific — it names the thing you fear | **unable to fire**; blind to every new entry |
| `--stat` against a remembered count | dumb — it is just two numbers | **held**; a foreign hunk cannot leave the count unchanged |

The content grep is the one a careful author reaches for, because it looks like it is checking
the *substance*. It is checking a list of things already accounted for. Prefer the number: it
makes no claim about what the foreign content is, which is exactly why nothing about the foreign
content can defeat it.

**Cross-reference added 2026-09-01: the shipped guard PREDICTED this instance, and neither
document pointed at the other.** `scripts/pre-commit-foreign-index.sh:30-50` splits the class in
two — **CROSS-path** (*my index holds YOUR file*) which it covers, and **INTRA-path** (*my file
holds YOUR lines*) which it does not — and says of the intra case, in its own comment: *"a bare
commit sees only their own staged path and this guard exits 0. **It would have passed.** Path
ownership was never in dispute — the contamination was inside a path both parties agree is
yours, so no ownership check can see it."* `0c32bb85`, documented above, is that sentence borne
out. The guard cites this file; this file did not cite the guard, so a reader arriving at either
one got half the picture — the prediction without its instance, or the instance without the
reason no gate caught it.

Re-measured here independently rather than taken from the section above, and it agrees:
`0c32bb85`'s subject is *"W-91 cited the wrong law"*, its diff adds `## F-93`, and its message
contains **0 of 6** markers of `F-93`'s content (`F-93`, `append-only`, `Write 8`, `187`,
`doctor.rs`, `false within the hour`). A first pass reported **1**, which was the pattern `80`
matching the SHA `800f1dec` inside the message — a loose predicate matching something its own
result did not contain, which is the negative-search law firing on the check written to confirm
this very section.

**Second INTRA-path instance the same day, and this one is not a capture.** `94d22b4d` committed
`docs/trackers/reconnaissance-patterns.md` carrying `R-139` and `R-154`, authored by other
sessions, alongside this session's `R-153`. Their index rows sat in the **same diff hunk**, so no
hunk-level split existed and `git add -p` is unavailable in this harness; the entries were
verified structurally complete and attributed explicitly in the commit message instead. Both
pre-commit guards — *refuse a pathspec commit carrying unstaged content* and *refuse an index
commit carrying another session's staged paths* — **passed**, correctly and uselessly, on exactly
the axis the guard's comment says they cannot see. So the intra-path hole now has two measured
instances rather than one prediction, and they differ usefully: `0c32bb85` was unnoticed until
the captured session found it, `94d22b4d` was noticed before the commit and mitigated in prose
rather than by any mechanism. **No new record and no new class** — the guard already assigns this
to `IC-14` and already names the remedy (record each path's blob at `git add`, re-hash at
pre-commit, refuse if it moved) as **not built**. This paragraph exists to close the citation, not
to re-open the finding.

### The seam is created by the append convention, and it is 5.9% of this ledger

`codescout-68`'s reading of the `F-93` split, measured rather than accepted. The capture was not
an accident of timing: **this repo's documented append discipline puts a commit boundary exactly
where a peer's `git add` can fall.** `artifact(append_entry)` writes the `## <ID> — <title>`
section, and CLAUDE.md then says *"THEN add the Index / Wins Index row, using the id the call
returned — never before"*. Two writes, necessarily ordered, with a gap between them. Every entry
written correctly has that window.

So it should be visible in history, and it is. Over all **186** `F-N`/`W-N` entries in
`docs/trackers/bug-fix-session-log.md`, comparing the first commit to introduce `## <ID> — `
against the first to introduce `| <ID> | 20`:

| | count |
|---|---:|
| body and index row in one commit | 173 |
| **split across two commits** | **11** (5.9%) |
| one shape not findable by `-S` (format drift) | 2 |

**Then the part that matters, and it is not the 11.** The `Session-Id` trailer is what would
classify a split as capture-or-tidiness — and it is present on **both** sides of exactly one:

| | count |
|---|---:|
| **no trailer on either side — unclassifiable, permanently** | **10** |
| trailer on both sides — classifiable | 1 |
| trailer on one side only | 0 |

**One number, stated with its denominator: of the 1 classifiable split, 1 is cross-session** —
`F-93`, `c2a08c22` → `d91c1155`, the case this section was written about. That is confirmation the
instrument reports what it should, and it is not a rate. A reader mining this corpus for *"how
often does a split indicate capture?"* has n=1 and no answer.

> This table first carried a third peer row, `same-session — 0`, alongside the 1 and the 10.
> Withdrawn on `codescout-68`'s challenge: its denominator is **1**, not 11, so the zero was the
> arithmetic complement of a single already-known case rather than a finding about same-session
> splits. Rendered as three peer rows the three read as independent results of comparable weight,
> and `1/(1+0)` invites a **100% capture rate off n=1** — a far stronger claim than anything
> measured. Same asymmetry as this page's other numbers, one turn on: a zero beside a one and a
> ten reads as comparable when its population is a tenth the size. Independently sampled by that
> session (every-6th-id, 3 splits in 31, all three untrailered, dated 2026-05-21 to 2026-08-26) —
> different draw, same conclusion.

The trailer is younger than the corpus, so ten of eleven splits can never be classified. And
`%an` reads the identical name on every one of them — `IC-10` in a line — so their default
reading is the **benign** one, *"same person split their own work"*, which is both the likelier
prior and completely unfalsifiable. A capture in that set is indistinguishable from tidiness.

**Two consequences for anyone reconstructing authorship here.** First, `git log -S` on one shape
of an entry returns one commit and **looks complete** — the answer is well-formed, singular, and
wrong for 5.9% of entries. Probe the heading *and* the index row; disagreement is the signal.
Second, this is the honest scope of the trailer: it is an excellent positive instrument going
forward and it does nothing for the past. The ten are not a backlog to resolve, they are the
measured cost of having added the instrument late.
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
