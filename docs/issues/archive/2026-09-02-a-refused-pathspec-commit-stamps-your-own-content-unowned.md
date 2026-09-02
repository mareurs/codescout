---
id: 9c716e7f97865056
kind: bug
status: fixed
title: 'BUG: a refused pathspec commit stamps the author''s own content as unowned, and no-op restaging cannot reclaim it'
tags:
- cluster/shared-resource-carries-no-owner
closed: 2026-09-02
opened: 2026-09-02
owner: marius
severity: medium
---

## Summary

When `git commit -- <paths>` runs pre-commit hooks, git points `GIT_INDEX_FILE` at a **temporary
partial-commit index** (`$GIT_DIR/next-index-<pid>.lock`) holding the pathspec content.
`post-index-change` inherits that variable, so its `git diff --cached --raw` reads the *temporary*
index while writing rows into the *durable* `$GIT_DIR/session-stage-log`. The rows therefore
describe a transient index as though it were the shared one, and because the writer is not a
recognised staging command they are stamped `-`.

If the hook then refuses, the author is left holding rows saying their own work is unowned — and
**they cannot reclaim them**, because re-`git add`ing byte-identical content is not an index
write, so `post-index-change` never fires. A bare `git commit` then refuses their own path under
`theirs:`.

The end state is the symptom of
`docs/issues/archive/2026-09-02-a-transiently-empty-index-destroys-stage-log-ownership.md`
reached by a different route. That bug was ownership **destroyed**; this is ownership **never
established**. Its fix (`5e522fa4`) does not reach here by construction: retention preserves rows
that exist and creates none.

> **Root cause corrected 2026-09-02, after filing.** This file first said *"`git commit -- <paths>`
> stages the pathspec into the real index before hooks run"*. That is **false** and is the reading
> that sends a reader into `staging_op()`, which is not where the defect is. See § *Root cause* for
> what actually happens and § *Hypotheses tried* for the two experiments that separated them. The
> fix direction inverted with it.
## Symptom (Effect)

A bare `git commit` refuses the author's own path under `theirs:`, with
`Staged by: (unrecorded) — not a staging command`. The printed remedy
(`git commit -- <paths>`) is the very command that produced the state.

## Reproduction

Live, on the shared checkout, 2026-09-02. Not a fixture — the repo's own hooks:

```
$ grep -c 'issue-clusters' .git/session-stage-log        # no row for my file yet
1                                                        # (a peer's retained row)

$ git commit -m "probe" -- docs/trackers/issue-clusters.md
   ... refuse a pathspec commit carrying unstaged content ... Failed
$ grep 'issue-clusters' .git/session-stage-log
-	f861250b	docs/trackers/issue-clusters.md	not-staging      # <- MY blob, owner `-`

$ git add docs/trackers/issue-clusters.md                 # the author's retry
$ grep 'issue-clusters' .git/session-stage-log
-	f861250b	docs/trackers/issue-clusters.md	not-staging      # <- UNCHANGED

$ bash scripts/pre-commit-foreign-index.sh; echo $?
Refusing a bare commit: the index holds paths staged by another session.
  theirs:
      docs/trackers/issue-clusters.md
1
```

`f861250b` is the author's own content. The file is staged (`git diff --cached --name-only`
lists it) and reads as a peer's.

## Environment

`experiments`, shared checkout, hooks installed via `scripts/install-hooks.sh`. Observed after
`5e522fa4`, so retention was live and is not implicated.

## Root cause

**One mechanism, plus one that makes it unrecoverable.**

**1. The recorder reads a temporary index and writes about it durably.** For a partial commit,
git prepares `$GIT_DIR/next-index-<pid>.lock` and exports `GIT_INDEX_FILE` to the hooks. Any
index-writing operation *inside* a pre-commit hook — which the `pre-commit` framework performs on
every run, stashing and restoring unstaged content — fires `post-index-change`, which inherits the
variable. Measured directly:

```
GIT_INDEX_FILE=[.../.git/next-index-25659.lock]
hook sees staged: [tracked.txt]          # <- the TEMP index, not the real one
-> session-stage-log gains: `-  207f2a3  tracked.txt  unnamed`
```

`staging_op()` is working correctly throughout: the parent genuinely is not a staging command, and
`-` is the conservative answer. The defect is that the recorder had no business writing a row at
all — the index it was describing is not the resource the log is about, and is deleted moments
later.

**2. A no-op `git add` is not an index write.** Restaging byte-identical content changes nothing,
so git never runs `post-index-change` and the row cannot be corrected. Only a *content change*
reclaims the pair — verified by appending a byte, after which the row came back `named` under the
author's id.

Neither is wrong in isolation. Together they leave a state with **no route out** that does not
involve modifying the work.

**The codebase already knows this hazard from the other side.** `tests/hooks-discrimination.sh`'s
`guard()` unsets `GIT_INDEX_FILE` deliberately, with a comment explaining that an inherited value
makes git read the wrong index and the guard read everything as ours — *"silence for entirely the
wrong reason"*. The recorder has the mirror-image exposure and no such precaution.
## Evidence

### The failure direction is safe, and that is again what makes it expensive

`-` over-refuses, which is correct. But at the point of use it is indistinguishable from a real
capture, and here the refusal names a file the author has just written, in a session that has
touched nothing else. The measured cost on 2026-09-02 was a second forced round through the
documented commit sequence.

### Not a `run_command` / MCP artefact — hypothesis falsified

First read as "`git` invoked through the MCP `run_command` tool breaks `PPID` detection". **False.**
A throwaway repo using the *real* shim form recorded `named` with the correct session id for a
`git add` issued through `run_command`. `PPID` detection works; the route is `git commit`.

Recorded because the falsification is the useful part: it is the hypothesis that fits the first
observation and sends the reader into `staging_op()`, which is not where the defect is.

### Why the sibling bug's fix does not cover it

Retention carries forward rows for pairs absent from the staged set. Here **no owned row was ever
written** — the first index write touching the pair was the non-staging one. Retention has nothing
to preserve. The two bugs share a symptom and a class and need different remedies.

## Hypotheses tried

1. **Hypothesis** — the MCP `run_command` wrapper breaks `/proc/$PPID/cmdline` detection.
   **Test** — throwaway repo, real shim, `git add` through `run_command`.
   **Verdict** — rejected. Records `SESS-PPID … named`. This is the reading that fits the first
   observation and sends you into `staging_op()`; it is worth keeping precisely because it is
   wrong in an attractive way.

2. **Hypothesis** — `git commit -- <paths>` stages into the REAL index before hooks run.
   **Test** — throwaway repo, tracked file modified but never staged, pre-commit that is a bare
   `exit 1` doing no index writes at all.
   **Verdict** — **rejected, and this was the filed root cause.** No row appears, and nothing is
   staged afterwards. The real index is untouched. An earlier run of this test used an *untracked*
   file, where `git commit -- <path>` fails before hooks run — a fixture flaw that produced the
   right answer for the wrong reason and had to be redone.

3. **Hypothesis** — the stamping needs the `pre-commit` framework's stash/restore specifically.
   **Test** — pre-commit hook printing `GIT_INDEX_FILE` and `git diff --cached --name-only`, then
   performing an index write, then `exit 1`.
   **Verdict** — **confirmed, and generalised.** `GIT_INDEX_FILE` is the temp partial-commit index
   and the hook sees the pathspec content in it. It is not the framework that matters but *any*
   index write inside *any* pre-commit hook during a pathspec commit. The framework is merely the
   one that always performs one.

4. **Hypothesis** (raised by `codescout-cc`, sessionId `953b5e77`) — the hole is hook-independent,
   so a commit refused by `ledger-counts` stamps already-owned rows `-` just as one refused by
   `foreign-index` would.
   **Test** — throwaway repo, non-`foreign-index` refusing hook; stage and confirm ownership
   FIRST, then attempt the refused pathspec commit.
   **Verdict** — **rejected in the direction that matters.** A pre-staged, owned row *survives*:
   carry-over matches the pair and preserves the owner. The damage requires the pair to have **no
   prior owned row**. So `codescout-cc`'s step 4 succeeded because their rows were still theirs,
   not because a pathspec commit ignores ownership. Their report was explicit that this was
   inferred rather than measured, their evidence having expired — which is why it was testable at
   all.
## Fix

Implemented 2026-09-02, on `experiments` at `cd1b138e`, patch-id
`c374900d02eb47a131fc18c5e802e321ebf3dca4`. **The direction inverted when the root cause was
corrected**, and the superseded proposal is kept below because it is the one the wrong root cause
recommends.

**Shipped — the recorder must not describe an index that is not the shared one.**
`post-index-change-stage-log.sh` now exits early when `GIT_INDEX_FILE` is set and does not resolve
to `$GIT_DIR/index`. Relative and absolute forms are both normalised before comparison. It sits
beside the existing `CODESCOUT_STAGE_LOG_RUNNING` re-entry brake, is a few lines, and touches
neither `staging_op()` nor the claiming rule. It removes the stamping at its source rather than
making the recorder guess better about the parent.

It does **not** give the author a reclaim route — it removes the state that needed one. That is
the right shape: `-` over-refusing stays correct, and the bad row is simply never written.

Note it does **not** by itself give the author a reclaim route — it removes the state that needs
one. That is the right shape: `-` over-refusing stays correct, and the bad row is simply never
written.

**Superseded direction, kept for its derivation** — *"treat `commit` as a staging verb when it
carries a pathspec, since argv names the paths and `names_path` already has what it needs."* This
follows from the false root cause and would have been actively wrong: it teaches the recorder to
**claim** pairs read out of a temporary index, converting a conservative `-` into a confident
wrong owner — the silent direction the whole design avoids.

**Do not fix it by making the guard trust `-` more.** Over-refusing is the correct direction; the
defect is upstream, in what the recorder is willing to describe.
## Fix provenance

- **SHA:** `cd1b138e` (`experiments`) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `c374900d02eb47a131fc18c5e802e321ebf3dca4` — content hash of the diff; survives rebase and cherry-pick.

Structured because `structured_fix_pointers` in `src/librarian/tools/doctor.rs` reads
`- **SHA:**` / `- **patch-id:**` list items and nothing else, so the accurate prose form in
§ *Fix* above read as **no anchor declared** — and this file's prose carries four commit-like
hashes, of which only this one is the fix. Verified 2026-09-02 before archiving: the SHA
resolves to a commit contained in `experiments`, and `git show cd1b138e | git patch-id --stable`
reproduces the patch-id above.
## Tests added

`tests/hooks-discrimination.sh` § 10. Confirmed RED against the unguarded script first — **both**
assertions failed — then green at 81/81.

- a temp partial-commit index writes **no row**
- and the recorder **still claims on the real index**, in the same repo

**The second is not decoration.** The first is an absence assertion and therefore monotone under
removal: deleting the recorder outright produces exactly the same silence. Only the pair
distinguishes a working guard from a dead hook.

**That both failed before the fix is itself the bug's second half on display.** The temp-index
write stamps the pair `-`, after which the author's `git add` is a no-op and cannot reclaim it —
so the positive assertion failed at `-` rather than at the author's id. With the guard, no row is
written, the author's add is a first write for that pair, and it claims normally.

The fixture's refusing hook is a bare stash cycle, **not** the `pre-commit` framework, on purpose:
the defect is reached by any index write inside any pre-commit hook during a pathspec commit. The
framework is merely the one that always performs one.

End-to-end re-run of the original symptom: refused pathspec commit → no row; author's `git add` →
`SESS-AUTHOR … named`; `pre-commit-foreign-index.sh` → exit 0, silent.
## Workarounds

Stage **before** attempting the commit — `git add <paths>` then `git commit -- <paths>` — which
is already step 4 of `docs/conventions/shared-checkout-commit-sequence.md`. Following that
sequence avoids this entirely; the bug is only reachable by committing by pathspec first, which
is what the `unreviewed-content` refusal then teaches you not to do.

Once in the state, the only reclaim is a content change. `--no-verify` also works and is the
wrong habit.

## References

- `scripts/post-index-change-stage-log.sh` — `staging_op()` and the verb list.
- `scripts/pre-commit-foreign-index.sh` — reads the log; correctly over-refuses on `-`.
- `docs/issues/archive/2026-09-02-a-transiently-empty-index-destroys-stage-log-ownership.md` — the
  sibling: same symptom, ownership destroyed rather than never established. Fixed at `5e522fa4`.
- `docs/conventions/shared-checkout-commit-sequence.md` — step 4 is the workaround.
- `docs/trackers/issue-clusters.md` `IC-17` — the class; this is the residual hole its
  Mechanism status now names.

## Resume

Fixed; no resume owed.

What this does **not** close, and stays with `IC-17`: the working tree's *unstaged* state still
carries no owner and has no adjacent git primitive to extend. See the class's `Mechanism status`.

One thing worth a separate look, raised by `codescout-cc`: step 4 of
`docs/conventions/shared-checkout-commit-sequence.md` prescribes `git add <paths> && git commit
-- <paths>`, which is the pathspec form this bug was about. **Staging first was always the
correct avoidance** and the page was never wrong — but a reader who reached the page *after*
hitting this bug had no way to see why the order mattered. With the guard shipped the hazard is
gone, so the page owes nothing; recorded here in case the ordering rationale is ever thought
optional.
