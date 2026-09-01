---
id: d5af3d3ceff1d08c
kind: bug
status: fixed
title: foreign-index refuses a commit and prescribes a remedy git refuses too
tags:
- cluster/gate-keyed-on-unobservable-event
topic: shared-checkout commit coordination
closed: 2026-09-02
fix_patch_id: 0e7feedf232c5ed9e22fd975c6fe36baa109e1d2
fix_sha: 74b9cc67 (experiments)
opened: 2026-09-02
owner: marius
related: []
severity: high
---

## Summary

`scripts/pre-commit-foreign-index.sh` refuses a bare commit whose index holds paths it
cannot attribute, and tells the caller to use the pathspec form instead. In a
sequencer state (`CHERRY_PICK_HEAD` or `MERGE_HEAD` present) git **refuses the pathspec
form outright**, so the guard's own prescribed remedy is unavailable and no commit form
is left. The guard never checks that the route it names exists.

Two independent things have to be true for the deadlock, and both were measured:

1. The guard reads the caller's **own** staged content as foreign, because git plumbing
   (rebase / cherry-pick replay) stages without writing a `session-stage-log` entry.
2. The escape it prescribes is refused by git in exactly that state.

## Symptom (Effect)

Reported by `codescout-8a` (session `bf44ba81`) 2026-09-01, hit inside a private linked
worktree during a `git rebase` that stopped mid-replay:

```
$ git commit -C 79a03aab
  Refusing a bare commit: the index holds paths staged by another session.
    theirs:  <the rebase's own replayed paths>
    Staged by:
      (unrecorded) — staged before this guard was installed, so no session claimed it.

  Commit your own paths by pathspec — that form ignores the shared index:
      git commit -- <path>

$ git commit -C 79a03aab -- <path>
fatal: cannot do a partial commit during a cherry-pick.
```

The reporter identified the staged content positively before proceeding —
`git patch-id --stable` of the staged diff matched commit `79a03aab`'s own patch-id byte
for byte — and used `--no-verify` for that one commit, disclosing it. That is the
correct standard for the identification; it is recorded here because the *guard* left no
compliant route, not as a finding against the caller.

## Reproduction

Measured 2026-09-02 in throwaway repos. **The precondition is not "a rebase is in
progress"** — that was the reporter's generalisation from their own error string, and it
is wider than the defect.

```
# A. conflicted `git rebase` INSIDE a linked worktree:
#    $GIT_DIR/rebase-merge PRESENT, CHERRY_PICK_HEAD ABSENT
git commit -m x -- f        ->  [detached HEAD b954c3f] x        # SUCCEEDS

# B. conflicted `git cherry-pick`: CHERRY_PICK_HEAD PRESENT
git commit -m x -- f        ->  fatal: cannot do a partial commit during a cherry-pick.
git commit -C <sha>         ->  [master 90703f1] on-topic        # bare SUCCEEDS

# C. conflicted `git merge`: MERGE_HEAD PRESENT
git commit -m x -- f        ->  fatal: cannot do a partial commit during a merge.
```

So the refusal is keyed on `CHERRY_PICK_HEAD` / `MERGE_HEAD`, which a rebase sets in some
stop modes and not others. **Pathspec is the only blocked form** — bare `git commit`
works in the same state, which is precisely what makes the pair a deadlock rather than a
stuck tree.

## Environment

codescout `experiments` @ `f75fc3b6`, 2026-09-02. git 2.x on Linux. Reported from a
linked worktree (`.worktrees/audit-shards-t7`, since merged at `bbee621c` and removed).

## Root cause

Two layers, and only the second is new.

**The attribution proxy (`IC-2`).** The guard cannot observe *who staged a path*, so it
substitutes *"does `$git_dir/session-stage-log` claim it?"*. Content staged by git
plumbing writes no log entry, so a session's own rebase replay reads as `(unrecorded)`
and the guard reports it as another session's work. Confirmed at the source:
`git rev-parse --git-dir` appears exactly once in the script, at line 100, solely to
locate that log — there is no reference to `--git-common-dir`, worktree, rebase,
`CHERRY_PICK_HEAD` or sequencer anywhere in the file.

**The unchecked remedy (the part worth filing).** The refusal path is written once and
names one escape. It is correct for the case the guard was built for — an entangled
shared index during ordinary work — and it is *unsatisfiable* in a state the guard does
not test for. A guard may refuse; what it may not do is refuse and then name a route git
will reject.

## Hypotheses tried

1. **Hypothesis (reporter's):** per-session worktrees dissolve this, per
   `docs/issues/2026-09-01-two-correct-pre-commit-guards-have-an-empty-intersection.md`
   § *Fix* direction 3.
   **Verdict: rejected, and this bug is the counter-example.** The deadlock occurred
   inside a private linked worktree, by a different route than the entangled-ledger case
   that file documents.

2. **Hypothesis (reporter's):** exempt when `git rev-parse --git-dir` differs from
   `--git-common-dir`, i.e. any linked worktree.
   **Verdict: rejected — it would disarm the guard where it currently works.** That test
   proves *linked worktree*, not *unshared index*. Two sessions working in the **same**
   worktree share its index exactly as two share the main checkout's, and the guard
   already handles that correctly, because `session-stage-log` lives at `$git_dir/…`
   which is per-worktree. Sixteen sessions were live on this machine at the time with
   worktrees as the isolation mechanism, so the co-located pair is not hypothetical.
   Accepted by the reporter on review.

3. **Hypothesis (reporter's):** exempt when `$GIT_DIR/rebase-merge` exists.
   **Verdict: rejected as too wide.** Reproduction A above commits by pathspec with
   `rebase-merge` present. During such a stop the guard's remedy works and it need not
   stand down. Accepted by the reporter on review.

## Fix

**FIXED on `experiments` at `74b9cc67`, patch-id
`0e7feedf232c5ed9e22fd975c6fe36baa109e1d2`** (2026-09-02). The SHA dies on the next rebase;
the patch-id is a content hash of the diff and survives rebase and cherry-pick both.

Stands down on one condition: `CHERRY_PICK_HEAD` or `MERGE_HEAD` exists. That is exactly
the set where the prescribed remedy is unsatisfiable. It needs no worktree special-case, it
is narrower than the `rebase-merge` test, and it leaves both the main checkout and shared
worktrees fully guarded.

Probed via `git rev-parse --git-path` rather than `"$git_dir/..."`. Sequencer state is
per-worktree today — verified 2026-09-02, `CHERRY_PICK_HEAD` lands in
`.git/worktrees/<name>/`, and a probe reading the common dir finds nothing — and `--git-path`
keeps being right if that ever changes.

The alternative shape — keep refusing, but *change the hint* to name a route that exists
in that state — is worse here, because in a sequencer stop the only available form is the
bare commit the guard is refusing. There is no third route to name.

**The fix was developed against a COPY of the hook and only applied once green.** The
discipline was right; **the reason first recorded here was wrong twice over, and both
errors are corrected in place rather than deleted, because each is easy to re-derive.**

*First error — the mechanism.* `core.hooksPath` is **unset** in this repo
(`git config --show-origin --get-all core.hooksPath` → exit 1, every scope, measured
2026-09-02). Hooks reach `scripts/` via pre-commit's generated `.git/hooks/pre-commit`
plus `entry: scripts/pre-commit-foreign-index.sh` with `language: system`
(`.pre-commit-config.yaml:78-84`). Unset is this repo's **healthy** state and
`tests/hook_config.rs::a_set_core_hookspath_must_point_at_a_directory_that_exists`
asserts it; the archived instance of a *set* `hooksPath` silently disabled every hook here
for a day (`docs/issues/archive/2026-08-30-core-hookspath-points-at-pre-rename-path.md`).
So the wrong sentence did not merely misdescribe — a reader reasoning forward from it
could have **set** the variable and reproduced that bug. Caught by `codescout-3e`.

*Second error — the conclusion, which survived the first correction.* "Saving the file
makes it live at that instant" is **false in the direction that matters**, and stays false
under the corrected mechanism. `pre-commit` clears unstaged changes before running hooks
(`staged_files_only.py:108` — see `8cc95806a7b5f37a`), so a `language: system` entry
executes the **index** copy of its own script. Measured 2026-09-02 in a throwaway repo:

| hook-script edit | version the hook actually ran |
|---|---|
| unstaged | the **index** version — the edit is inert |
| staged | the edit — live for every session |
| unstaged, over a committed edit | the **index** version again |

**The exposure moment is `git add`, not the editor save**, so the window this note claimed
to be managing was never open. Both halves of the refutation were already filed in this
repo — the `hooksPath` test and the stash bug — and two sessions each held one and neither
composed them.


## Tests added

`tests/hooks-discrimination.sh` § 7, four cases. Suite went 46 passed / 3 failed → **49 / 0**.

**Every case asserts the staged path is FOREIGN before invoking the guard.** Without that
the hook exits 0 for entirely the wrong reason — an unmatched `session-stage-log` key reads
as "all mine" and passes silently, which is the false green a probe upstream already paid
for. The fixtures derive ownership through the suite's own `post-index-change` shim rather
than hand-seeding the log, so they cannot drift from the parser under test.

| case | asserts |
|---|---|
| conflicted cherry-pick | stands down |
| **no sequencer** (control) | **still refuses** |
| conflicted merge | stands down |
| cherry-pick inside a linked worktree | stands down — the reported incident |

**Mutations**, run against a copy before the shared script was touched, each mutant
`diff`-verified to differ from baseline:

| mutation | killed by |
|---|---|
| remove the `CHERRY_PICK_HEAD` arm | the cherry-pick and worktree cases |
| remove the `MERGE_HEAD` arm | the merge case **only** — without it this mutation survives |
| stand down unconditionally | the control **only** |

The last row is why the control is not filler: an unconditional stand-down is the cheapest
wrong fix here, it passes every other case, and nothing else in the suite notices it.

One defect was caught in the test *while writing it*: the worktree case was first wrapped in
a `( ... )` subshell, which would have discarded its `PASS`/`FAIL` increments — the
assertions would still print while the suite's exit code stopped depending on them. Same
subshell trap this file's `new_repo` header already records.
## Provenance

Reported by `codescout-8a` via peer message, 2026-09-01. Reproductions, the
precondition narrowing, and the rejection of hypotheses 2 and 3 were measured in this
session (`0771abbc`) and accepted by the reporter. The reporter separately withdrew an
adjacency-based authorship claim they had made about these scripts — see `IC-10`.
