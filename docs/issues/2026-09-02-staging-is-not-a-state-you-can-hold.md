---
status: open
opened: 2026-09-02
closed:
severity: medium
owner: marius
related:
  - docs/issues/2026-09-01-pre-commit-stash-removes-every-peers-unstaged-work.md
  - docs/issues/archive/2026-09-02-cluster-gate-failure-text-prescribes-the-blindness-that-caused-it.md
tags:
  - cluster/shared-resource-carries-no-owner
kind: bug
---

# BUG: staging is not a state you can hold — any two-step index operation on a shared checkout has a window another session can take

## Summary

`git add` then verify then commit is **three steps and two windows**. The git index is
per-repository, not per-session, so between a session's `git add` and its `git commit` any peer
may remove the staged paths — and on a checkout with seven concurrent sessions this is not a
theoretical race. Measured on this session 2026-09-02: staged paths vanished inside ~60 seconds,
leaving a ledger edit on disk that contradicted the corpus and **red-gating all seven sessions**,
reached by following the documented sequence correctly.

The only shape with no window is issuing `add` and `commit` in **one invocation** with an explicit
pathspec.

> **Corrected within the hour, by the author, against their own commit.** That sentence is true of
> the failure above and **false as a general safety claim**, and the correction is the more
> important half of this file. The single-invocation form stops you *losing staging you hold*. It
> does **not** stop you *capturing edits you never made*: `git commit -- <paths>` commits the
> **working-tree content** of the paths it names, including any concurrent edits a peer has made to
> those same files. The commit that closed the red window, `62d7fa4b`, did exactly that — it swept
> two files carrying another session's uncommitted work. See § *The remedy solves one of two
> problems*.

This is not specific to `docs/trackers/issue-clusters.md` or to counts. It would be true of a
single-class ledger, and of any workflow that stages, checks something, then commits.

## Symptom (Effect)

Timeline, from this session's own shell history, HEAD `e158d4a7`:

```
09:43:xx  git add <3 bug files>                    -> staged, verified: 3 paths, A/M/A
          (edited docs/trackers/issue-clusters.md: IC-11 -> 13, IC-22 -> 3)
09:43:5x  cargo test --test issue_clusters         -> 2 FAILED
            doc-contradicted-by-code — table says 13, corpus has 12
            hint-composed-without-the-request — table says 3, corpus has 2
09:44:09  git diff --cached --name-status
            M  src/engines/coordinator.rs          <- not mine
          git status --porcelain <my two files>
            ?? ...memory-description-omits-the-refresh-anchors-action.md
            ?? ...cluster-gate-failure-text-prescribes-the-blindness-that-caused-it.md
```

For roughly a minute the ledger on disk claimed 13/3 against a tracked corpus of 12/2. That is
`every_index_count_matches_the_corpus` and `every_bare_n_in_a_class_field_matches_the_corpus` red
for **every session in the checkout**, armed by a session that had done nothing procedurally wrong.

The failure is silent at the moment it happens. `git add` reported success; nothing announced the
removal; the only signal was a gate failure whose text describes a *count drift* — a different
problem with a different remedy.

## Reproduction

Deterministic, two shells in one checkout:

```
# shell A
git add path/to/file
git diff --cached --name-only        # -> path/to/file

# shell B
git restore --staged path/to/file    # or `git reset`, or any pre-commit run

# shell A
git diff --cached --name-only        # -> empty. No error, no notification.
git commit -m ...                    # commits nothing, or the wrong set
```

The live instance needed no adversary — see § *Root cause*.

## Environment

Shared checkout, 7 concurrent sessions across 3 profiles, commits landing every few minutes.
Not reproducible single-session, which is exactly what makes the documented sequence look safe.

## Root cause

**The git index is a shared resource that carries no owner.** There is one `.git/index` per
repository. It has no per-session partition, no advisory lock a peer must respect for the duration
of a logical operation, and no record of which session staged which path. `git add` writes to it and
`git restore --staged`, `git reset`, and every `pre-commit` run mutate it, from any session, with no
mechanism relating a staged path to the session that staged it.

So a session's staged set is **not state it holds**. It is a shared value it has written, which any
peer may overwrite before it is read back.

*Measured 2026-09-02, and the mechanism is worth more than the race:* the removal was not
contention. `codescout-26`'s implementer subagent ran `git add` for its own file, found four paths
staged rather than one, and ran `git restore --staged` on the three that were not its own before
committing by pathspec. It **read the hazard correctly and then repaired it**, where reporting was
the whole job — committing by pathspec already ignores paths it does not name, so the unstaging
bought nothing and destroyed another session's staging intent. Reported unprompted by `codescout-26`,
which is the only reason this is documented rather than an unexplained disappearance.

The rule that would have prevented it sits one step earlier than the ones already written down:
**do not repair shared state, report it.** A correct diagnosis followed by an unrequested write to
shared state is not a clean outcome.

### Why the documented sequence is the wrong shape

Every party in this checkout, including this session, recommended some form of:

> stage the pair, run `cargo test --test issue_clusters`, then commit

Three steps, two windows — one between `add` and the gate, one between the gate and `commit`. The
gate's own output is what tempts the split, because it reads the index and therefore *requires*
staging before it can answer. So the instrument that exists to make the operation safe is what
forces it into the unsafe shape.

## Evidence

### The red window, and what closed it

Resolved by collapsing the operation:

```
git add "$P1" "$P2" "$P3" "$P4" && git commit -F <msg> -- "$P1" "$P2" "$P3" "$P4"
```

One shell invocation, explicit pathspec on both halves. Landed at `62d7fa4b`
(patch-id `c31c2e58bafed34ba8556518a28dcb0bd53e1b0d`) with all four pre-commit hooks passing,
including `refuse an index commit carrying another session's staged paths` — which passed precisely
because the pathspec named only this session's files, while `src/engines/coordinator.rs` sat staged
by someone else and was correctly left alone.

The verification did not need a separate step: the `ledger-counts` pre-commit hook reads the index
and runs *inside* the commit, so the gate check and the commit are one atomic action rather than
two.

### The remedy solves one of two problems, and the author found the second by committing it

`62d7fa4b` **captured another session's uncommitted work.** Two of its four paths carried edits this
session did not write:

| path | whose |
|---|---|
| `2026-09-02-index-description-omits-the-verify-action.md` (+58) | § *Two claims in the plan above were wrong* and a rewritten § *Fix*, by `codescout-05` |
| `2026-09-02-memory-description-omits-the-refresh-anchors-action.md` (254, new) | its fix-provenance block and § *This file's own correction was also short*, by `codescout-05` |

Verified at the bytes against the commit object, not inferred. `codescout-05` reported it
unprompted, confirmed the content survived byte-intact, and explicitly declined a revert on the
grounds that on a shared tree the repair destroys work the defect only mislabels. **Nothing is
lost; the authorship record is wrong and that is the whole of the harm.**

The two failures are the same defect seen from opposite ends, and the remedy for one is not the
remedy for the other:

| | mechanism | does the single-invocation form help? |
|---|---|---|
| **losing** staged paths | a peer mutates the shared index between your `add` and your `commit` | **yes** — no window exists |
| **capturing** others' edits | `commit -- <paths>` takes the *worktree* content of those paths | **no** — it is what makes the capture certain |

An explicit pathspec bounds *which files* you commit. It says nothing about *whose edits* are in
them, and no pathspec can, because the index and the worktree both record content without an
author. That is the class claim arriving a third time in one incident.

### The bypassed version is orphaned, not destroyed — and nothing says so

Reproduced independently in two throwaway repos, by `codescout-05` and by this session:

```
git init; echo v1 > f.txt; git add f.txt; git commit -m v1
echo v2-STAGED         > f.txt; git add f.txt     # staged
echo v3-WORKTREE-ONLY  > f.txt                    # worktree diverges
git commit -m v2 -- f.txt                         # pathspec commit

git show HEAD:f.txt          -> v3-WORKTREE-ONLY   # the staged v2 did NOT land
git status --porcelain       -> (empty)            # index resets clean
git fsck --unreachable       -> unreachable blob 21bb6971abf0470d48085a41edb7a31b14e31ff0
git cat-file -p 21bb6971     -> v2-STAGED          # still there
```

Both runs produced the **same blob id**, git being content-addressed — two independent
reproductions of one object.

`git add` writes the blob to the object store before anything references it, so a staged-then-
bypassed version survives as an **unreachable object** until gc prunes it (default two weeks for
unreachable objects; `gc --prune=now` or a `gc.pruneExpire` change collapses that to immediately).

**This lowers the severity and leaves the defect intact, because the loss is not the problem — the
silence is.** Nothing reports the orphaning. `git status` is clean afterwards, the commit succeeds,
and the missing work looks exactly like *"I must not have staged it after all"*. A recoverable loss
nobody is told about is worse than a loud one, and it is the same shape as everything else in this
incident: a plausible state rather than an error.

**Caveat on the recovery, and it is the class again.** `git fsck --unreachable` lists every
unreachable blob in the repository, carrying no path, no timestamp and no author. On a checkout with
seven sessions it identifies *content*, never *ownership*. It answers "I know what my text said" and
cannot answer "whose is this" — so it is a recovery path for the author who remembers, and no help
at all to anyone else. Named by `codescout-05`.

**So there is no known safe shape for committing a file a peer may be editing.** The honest
position is that this is unsolved at the tool level: `git add -p` cannot help (the edits are
interleaved in one file), and checking `git diff` immediately before committing only narrows the
window rather than closing it. What is available is a *social* remedy — do not commit a path
another session is known to be working, and report rather than repair when you find you have.
That is policy, not mechanism, and is recorded as such.

### Three sessions attributed this one event to three different authors

`codescout-69` said `codescout-20`; retracted to `codescout-05` on `codescout-20`'s correction; the
actual author was `codescout-26`'s subagent, which self-reported. Each attribution was made
carefully, and each was wrong. The index records no author, so every attribution was necessarily an
inference — which is the class claim (*"enumerating the peer does not help"*) arriving through a
second door: the peers were fully enumerated and positively identified all evening, and the index
still could not say who wrote it.

## Hypotheses tried

1. **Hypothesis:** the pre-commit stash (`docs/issues/2026-09-01-pre-commit-stash-removes-every-peers-unstaged-work.md`)
   removed the staged paths.
   **Test:** that mechanism stashes *unstaged* work and restores it; the observed loss was of
   *staged* paths, and `codescout-26` reported the explicit `git restore --staged`.
   **Verdict:** rejected — related and distinct. That bug is about unstaged work vanishing during
   someone else's hooks; this is about staged paths being removed outright.

2. **Hypothesis:** this is contention, and a protocol (announce before staging, hold a token) fixes
   it.
   **Test:** the removal was a deliberate, well-intentioned repair by a party that had correctly
   diagnosed the situation — not a collision. A protocol binds only the sessions that heard it, and
   subagents dispatched by a peer never hear it at all.
   **Verdict:** rejected. The remedy has to be a *shape* the operating session controls alone, which
   is what the single-invocation form is.

## Fix

Not a code change in this repo — the defect is in git's model and in the sequence this project's
docs and gates recommend. Three concrete items:

1. **Document the atomic shape** wherever a stage-then-check-then-commit sequence is prescribed —
   `CLAUDE.md` § *Git Workflow*, `docs/RELEASE.md`, and the `ledger-counts` gate's own failure text,
   which currently sends the reader to re-derive and commit as separate steps.
2. **Say it in the dispatch brief.** `codescout-26` has already changed its remaining subagent
   dispatches to *"if the shared index holds paths you did not stage, commit by pathspec and leave
   them alone — never `git restore --staged`, never `git reset`"*. That is the correct instruction
   and it belongs in the shared guidance rather than in one session's briefs.
3. **Consider whether the gate can read the worktree instead of the index** for an interactive run,
   so that verification does not *require* staging first. That is the same index-vs-worktree axis as
   `docs/issues/2026-09-01-cluster-count-gate-lists-the-index-but-reads-the-worktree.md` and should
   be decided with it, not separately — note the two want opposite things, which is the real
   question rather than an oversight.

## Tests added

None, and a regression test is not obviously available: the defect is a race between processes
against a shared file, and a test asserting "staging survives" would be asserting something git does
not promise. The honest guard is the documented shape plus the dispatch instruction, both of which
are policy rather than mechanism — recorded as such rather than claimed as covered.

Per `CLAUDE.md` § *Observer Blindness*, the mechanism-shaped version would be a wrapper that stages
and commits in one call, so the unsafe shape is not reachable by following the docs. Not built.

## Workarounds

Never hold a staged set across another command. Use:

```
git add <explicit paths> && git commit -F <msgfile> -- <the same explicit paths>
```

Never a directory token (`git add docs/issues/`) — that records ownership as `-` in
`.git/session-stage-log`, which reads as foreign and gets your own later commit refused
(`scripts/post-index-change-stage-log.sh:250-255`). Let the pre-commit hooks be the verification;
they read the index and run inside the commit.

**This closes the losing half only.** It does not stop you committing a peer's concurrent edits to
the paths you name — see § *The remedy solves one of two problems*. Before naming a path, check
whether a peer is editing it, and if you find after the fact that you swept someone's work:
**report it to them and do not revert.** The content is intact; only the authorship record is
wrong, and a revert or amend on a shared branch destroys work the defect merely mislabels.

**If you believe you lost staged content, it is probably still there.** A staged-then-bypassed
version is orphaned, not destroyed, until gc prunes it:

```
git fsck --unreachable | awk '/unreachable blob/{print $3}' \
  | while read b; do echo "=== $b"; git cat-file -p "$b"; done
```

Do this **before** any `git gc --prune=now`. The listing carries no path, timestamp or author, so it
identifies content and never ownership — usable only if you remember what your text said.

## Resume

Decide item 3 against
`docs/issues/2026-09-01-cluster-count-gate-lists-the-index-but-reads-the-worktree.md` — they pull in
opposite directions and the resolution is one decision, not two. Then add the atomic shape to
`CLAUDE.md` § *Git Workflow* and to the `ledger-counts` failure text.

## References

- `62d7fa4b` — the commit that closed the red window with the single-invocation form.
- `docs/issues/2026-09-01-pre-commit-stash-removes-every-peers-unstaged-work.md` — sibling
  mechanism, unstaged work rather than staged paths.
- `docs/issues/archive/2026-09-02-one-ledger-file-serializes-every-class-edit.md` — why the pair had to be
  atomic in the first place.
- `docs/trackers/issue-clusters.md` `IC-17`.
