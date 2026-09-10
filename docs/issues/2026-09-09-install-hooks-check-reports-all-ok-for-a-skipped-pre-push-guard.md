---
status: open
opened: 2026-09-09
closed:
severity: medium
owner: marius
related: []
tags:
- cluster/selector-narrower-than-its-population
kind: bug
---

# BUG: install-hooks.sh --check reports all-ok for a checkout whose pre-push guard is silently skipped, because that branch's copy predates the hook

## Summary

`pre-push` fails open when `scripts/pre-push-foreign-session-guard.sh` is absent from the checked-out branch. `render_shim`'s header names that trigger and argues it is survivable because a second observer path — `install-hooks.sh --check` — reports `MISSING pre-push` independently. On a branch predating the hook, `--check` **runs, reports four `ok` lines, and exits 0**, never mentioning pre-push: `install-hooks.sh` is itself versioned, and that copy does not know pre-push exists. The rescue path does not fail; it certifies health. Measured live — a push from this checkout went out unguarded, exit 0.

## Symptom (Effect)

First push of the session, from branch `result-cap-marker-gate` at `2a32c043`:

```
warning: git hook pre-push is installed, but scripts/pre-push-foreign-session-guard.sh is missing or not executable - skipping
 * [new branch]        result-cap-marker-gate -> result-cap-marker-gate
```

Exit 0. Twenty commits belonging to another session were published with no refusal and no prompt. `--check`, run against that same state by a peer session:

```
ok      pre-commit stage      framework shim present
ok      post-index-change     shim present
ok      stage log             present
off     prepare-commit-msg    opt-in; not installed
CHECK_EXIT=0
pre-push mentioned in that report: 0
```

A session that did the responsible thing — run the status tool before pushing — is told everything is fine.

## Reproduction

1. Check out a branch whose merge-base predates the commit adding the pre-push shim (here `2a32c043`, merge-base `9d6d2c2b`, 2026-09-02).
2. `bash scripts/install-hooks.sh --check` → exit 0, four `ok` lines, no mention of pre-push.
3. `git push origin <branch>` → succeeds, emitting the `warning:` above.
4. Rebase onto `experiments`; push again → **refused**.

Nothing about the push changed between 3 and 4. The tree did.

## Environment

Linux, codescout v0.15.0, 2026-09-09. Linked worktree `.worktrees/result-cap-marker-gate`, 482 commits behind `experiments` at the time of the unguarded push.

## Root cause

Two versioned artifacts and one shared one, and the interaction is the defect.

- The hook **wiring** is shared: `git rev-parse --git-path hooks` from a linked worktree returns `/home/marius/work/claude/codescout/.git/hooks`, the common dir. The shim is installed there and runs for every branch.
- The **guard** it invokes is tracked per-branch, so `[ ! -x "$PROJECT_ROOT/$target" ]` is evaluated against the checked-out tree and fails open for a branch older than the script. This much `scripts/install-hooks.sh:200` already states: *"The realistic trigger is checking out a branch that predates the script."*
- The **status tool** is tracked per-branch too, and that is what has not been reckoned with. Measured on the triggering commit:

```
git cat-file -e 2a32c043:scripts/install-hooks.sh   -> EXISTS   (so --check IS runnable)
'pre-push' mentions in that version                 -> 0
install_shim call sites there                        -> 2  (post-index-change:170,
                                                            prepare-commit-msg:209 — opt-in)
'pre-push' mentions in the current version           -> 7
```

So `--check`'s selector is its own hardcoded `install_shim` list. On that branch the list has two entries and neither is pre-push. It iterates them, finds both healthy, and exits 0. The hook it cannot name is live in the shared dir and skipping its guard on every push.

The header's claim that path 2 fires *"independently of whether anyone saw path 1"* is true and beside the point. Path 2 is independent of path 1 in the dimension tested — does anyone read stderr — and **co-versioned with the thing it reports on** in the dimension that decides the case.

Measured 2026-09-09 by `cat-file` against `2a32c043` plus an observed `--check` run in the reproduced state (session `59112612-5fc8-4b31-8c8c-e19220d99eac`).

## Evidence

### The shim's own reasoning, verbatim

```
#   1. push time  -> this warning, on stderr. Swallowed by any caller that reads only the
#                    exit code -- and with fail-open that exit is 0, so open-and-unheard is
#                    the worst square of the matrix. Real, and not sufficient on its own.
#   2. `install-hooks.sh --check` -> reports `MISSING <hook>` and exits 1, from the
#                    `-x "$PROJECT_ROOT/$target"` precondition, independently of whether
#                    anyone saw path 1. ...
#                    That path has a test caller (tests/pre-push-foreign-session-guard.sh).
```

`scripts/install-hooks.sh:190-198`. The `-x` precondition is never reached for pre-push on that branch, because no call site names pre-push to reach it with.

### The hook did run — this is not a wiring defect

The same hook **refused** two later pushes from the same worktree, once naming one sessionId and once six. A hook that refuses is a hook that ran. A separate claim that worktree sessions run no hooks at all was filed and retracted the same day; this bug does not depend on it and is a different mechanism.

### Why silence is worse than breakage here

A broken status tool fails and gets fixed. This one succeeds. Its output is not merely uninformative about pre-push — it is *more* reassuring than no tool at all, because four `ok` lines and exit 0 are what a healthy checkout looks like. The state is fully knowable (`ls .git/hooks`), so this is not an unknowable-total case; it is an unexamined-subset case.

## Hypotheses tried

1. **Hypothesis:** worktree sessions run no hooks (`.git/worktrees/<name>` has no `hooks/`).
   **Test:** `git rev-parse --git-path hooks` from the worktree; three observed firings, two refusals.
   **Verdict:** rejected. Git reads hooks from the common dir for a linked worktree.

2. **Hypothesis:** already filed, so this is a rediscovery.
   **Test:** `grep` over `docs/issues/**/*.md` for `per-branch|predates the script|branch that predates`.
   **Verdict:** rejected — 0 hits. The trigger is named in a script comment, not the bug corpus, and that comment concludes it is survivable.

3. **Hypothesis (MINE, WRONG — kept deliberately):** path 2 is unavailable on the stale branch because its test caller `tests/pre-push-foreign-session-guard.sh` is absent there.
   **Test:** `git cat-file -e 2a32c043:scripts/install-hooks.sh`.
   **Verdict:** **rejected.** `install-hooks.sh` exists on that branch, so `--check` is perfectly runnable. The absent test caller means path 2 is not *regression-tested* there — a different and weaker claim than "cannot be run". I had inferred unavailability from the absence of a test, which does not follow. Kept because the wrong version is the intuitive one and reads as sufficient; a later reader who reaches for it should find it already closed.

4. **Hypothesis:** `--check` runs but cannot report pre-push, because that copy predates the hook.
   **Test:** count `pre-push` mentions and `install_shim` call sites in `2a32c043:scripts/install-hooks.sh`; then run `--check` in the reproduced state.
   **Verdict:** **confirmed** — 0 mentions, 2 call sites (neither pre-push), and an observed run returning four `ok` lines with exit 0. This is the finding.

## Fix

Not fixed. A fix must make the fail-open observable from something **not** co-versioned with the guard or with the status tool:

- Have the shim record its fail-open into shared, non-versioned state (under `.git/`) that any later `--check`, gate, or `librarian(action="doctor")` run reads — so the report is a function of the installed hooks rather than of the branch's list of them; or
- Have `--check` enumerate the hooks actually present in the common dir and report any it does not recognise, inverting the selector so the population is the ground truth rather than the code's list; or
- Fail closed on a missing guard, making the remedy `install-hooks.sh` rather than silence.

The second is the one that addresses the class rather than this instance. Fail-open-vs-closed is a reasoned policy choice in the header, not an oversight, and is deliberately not relitigated here.

## Tests added

None. The regression test must assert that a tree with the guard removed produces an *observable* artifact, and there is currently no non-versioned surface for it to read — which is the bug. Named rather than left blank: the test is blocked on the fix's design, not overlooked. Note that `tests/pre-push-foreign-session-guard.sh` exercises path 2 on branches that have it, which is exactly the population where the defect is absent.

## Workarounds

`ls -l .git/hooks/` and compare against `scripts/install-hooks.sh`'s `install_shim` call sites by eye. Treat a `warning:` line on push as a stop. Neither is a mechanism, and the first requires knowing the tool under-reports — which is the thing the tool conceals.

## Resume

Implement the inverted selector: have `--check` list the common dir's hooks and report any present-but-unrecognised shim, so the report's population is the filesystem rather than the script's own list. Then the regression test becomes expressible — park the guard, assert `--check` names it — where today it cannot be written on any branch that would fail.

## References

- `scripts/install-hooks.sh:190-203` — `render_shim`'s two-observer-path argument and its trigger sentence.
- `docs/trackers/issue-clusters/IC-18-selector-narrower-than-its-population.md` — the class.
- `3f0d13b7` (patch-id `74db845b1fa599d0f736ba6dd23bfbf26c813fbb`) — the adjacent `install-hooks.sh` worktree-write fix, and the `--git-path` relative-vs-absolute asymmetry found while fixing it. **A complement to this bug's fix, not an overlap, and the boundary is worth stating so nobody reads that commit as closing this.** It makes every write land where git reads and gates the success line on the write actually happening — so `--check` no longer prints `ok` for a shim it did not install. It does **not** make the shim *list* self-deriving. A sixth hook added tomorrow is still invisible to `--check` on every branch that predates it, which is this defect unchanged with a different hook name in it. The inversion under *Fix* is what closes that, and `3f0d13b7` cannot; conversely this bug's fix does nothing about writes landing in the wrong directory. Two halves, neither sufficient. (Boundary supplied by the author of `3f0d13b7`, session `59112612-5fc8-4b31-8c8c-e19220d99eac`.)
- Cluster candidacy: `guard-narrower-than-its-name` (IC-14) fits the pre-push guard itself, but that half is documented and accepted in the header; the defect filed here is the status tool, whose selector — a hardcoded `install_shim` list — is narrower than the population its caller intends (the hooks actually installed). `floor-published-under-the-name-of-a-total` (IC-20) was rejected: nothing stopped mid-walk and the true state is knowable from `ls .git/hooks`.
- Reproduction of the `--check` output contributed by session `59112612-5fc8-4b31-8c8c-e19220d99eac`; the mechanism correction in Hypothesis 3 is theirs.
