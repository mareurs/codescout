---
id: '786b1826bab82b18'
kind: bug
status: fixed
title: worktree-remove.sh --stop signals a stoppable holder that is an ancestor of the script itself
tags:
- cluster/unclassified
closed: 2026-11-12
opened: 2026-11-12
owner: marius
related:
- '3dde1548c3e5c310'
severity: low
unverified: Tested on fixture /proc trees only; never run against a real process tree in which a stoppable holder is an ancestor of the script. The ancestry is known only as far as /proc/<pid>/status can be read along the chain. The ancestor skips in send_term and in the stop_holders collection loop have no test.
---

# BUG: worktree-remove.sh --stop signals a stoppable holder that is an ancestor of the script itself

## Summary

`scripts/worktree-remove.sh` protects one process from its own `--stop`: the script's parent (`$PPID`). `send_term` refuses to signal `$$`, `$BASHPID` and `$PPID`, and `blockers_of` makes the parent a blocker whatever its kind. A holder of a stoppable kind (`mux`, `rust-analyzer`, `cargo`, `rustc`, `sccache`, `build-output`, `child`) that is a HIGHER ancestor of the script, for example the grandparent, has no such protection. With `--stop`, and when no `other` blocker exists, the script sends it SIGTERM. The operator's own shell chain then ends under them.

Found in the final whole-branch review of the `child` kind (bug `3dde1548c3e5c310`, commit `82f36b06b`). That branch makes the script's ancestry never a `child`, but it does not change this older behavior: it only keeps a process that sits below an ancestor from becoming a `child` through it.

## Class fit

The guard names one process, the parent, where the hazard is the whole chain of ancestors. No cluster class in the roster describes that, so this bug uses `cluster/unclassified`.

## Symptom (Effect)

An operator who runs `scripts/worktree-remove.sh --stop <path>` from a shell that is itself a descendant of a stoppable holder of the same worktree (a `cargo run` or a cargo-driven task that started the shell) may see that holder terminated, and with it the shell. The script's cwd must be outside the worktree, because a caller whose cwd is inside it is refused with exit 2. That keeps the case narrow.

## Reproduction

Not run on a real process tree. A reviewer ran the script once on a fixture `/proc` tree: a `cargo` holder G (cwd in the worktree) that is the parent of the script's parent, plus two `vim` holders. The run listed G as `cargo`, and `--stop` tried to signal it. In that fixture the two `vim` holders were also made `child` by a defect that is now fixed, so the fixture does not isolate this bug.

To isolate it, the fixture needs only G and the script's parent chain, with no `other` holder. Expected by the code: G is signalled; wanted: the script exits 4 and signals nothing.

## Environment

`scripts/worktree-remove.sh` on `experiments` at `82f36b06b` or later. Linux `/proc`.

## Root cause

Read in `scripts/worktree-remove.sh` on 2026-11-12: `send_term` returns without signalling only for `$$`, `$BASHPID` and `$PPID`; `blockers_of` flags an `other` row, or the row whose pid is `$PPID`. Nothing treats a row whose pid is an ancestor of the script as a blocker. `list_holders` already builds the set of the script's ancestors for the `child` pass, but only uses it there.

## Evidence

The code as read; one fixture run by a reviewer (above). No real process tree.

## Hypotheses tried

None beyond reading the code.

## Fix

Applied 2026-11-12 (commits in Fix provenance). `ancestry_of START` builds the set of the script's ancestors: the start, then each ancestor found through `PPid:` in `/proc/<pid>/status`, at most 64 steps, with a seen set. The walk ends at pid 1 or 0, a repeated pid, a `status` it cannot read, and a `PPid:` that is not a number or has 11 or more digits. `main` computes the set once, from the script's parent. `blockers_of` makes a listed holder in that set a blocker whatever its kind, printed as `(an ancestor of this script)`; the parent keeps `(the parent of this script)`. `send_term` and the collection loop of `stop_holders` skip such a pid. `list_holders` uses the same function for the `child` pass, so there is one walk.

A review of the first commit found that it computed the set lazily and trusted any non-empty `ANCESTRY` in the environment, so an inherited variable of that name turned the guard off and the script signalled the grandparent again (reproduced on a fixture). The second commit computes the set in `main` before any use.

## Tests added

`tests/worktree-remove.sh` (passed=266 failed=0 on 2026-11-12): `case_a_stoppable_grandparent_of_the_script_is_a_blocker`, `case_a_stoppable_ancestor_three_levels_up_is_a_blocker`, `case_an_ancestor_blocks_the_whole_stop`, `case_without_stop_an_ancestor_is_listed_unchanged`, `case_a_holder_that_is_not_an_ancestor_is_still_stopped`, `case_a_mux_ancestor_of_the_script_is_a_blocker`, `case_an_inherited_ancestry_variable_does_not_disable_the_guard`, and an assertion in the older parent case that the parent keeps its own text. Before the fix the first three exited 5 instead of 4; the inherited-variable case exited 5 on the first commit.

Not covered: the ancestor skips in `send_term` and in the collection loop of `stop_holders` (no realistic path makes an ancestor appear after the first scan), and the check `lacks "worktree-remove: signalled "` in the mux case cannot fail with fixture pids; the exit-4 assertion pins that case.

## Fix provenance

- **SHA:** `2e3bdbb30` (`experiments`)
- **patch-id:** `65e62ea427a3`
- **SHA:** `2740f4798` (`experiments`)
- **patch-id:** `501ff47461ef`

## Workarounds

Not needed once the fix is in. Before it: run the script from a shell that is not a descendant of a process that holds the worktree.

## Resume

Fixed. Open: nothing was run against a real process tree (fixtures only); the ancestry is known only as far as `/proc/<pid>/status` can be read along the chain; the two skips named under Tests added have no test.

## References

- Bug `3dde1548c3e5c310` (`docs/issues/2026-11-12-git-worktree-remove-refuses-after-codescout-and-agent-briefs-leave-untracked-files-in-a-worktree.md`), section Fix, the ruling on this gap.
- `docs/conventions/removing-a-worktree.md`, section "A blocker is not stoppable".
