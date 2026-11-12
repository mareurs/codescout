---
id: '786b1826bab82b18'
kind: bug
status: open
title: worktree-remove.sh --stop signals a stoppable holder that is an ancestor of the script itself
tags:
- cluster/unclassified
opened: 2026-11-12
owner: marius
related:
- '3dde1548c3e5c310'
severity: low
unverified: Read from the code and one fixture run by a reviewer; never run against a real process tree. No test covers an ancestor holder when no other blocker exists.
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

Not made. Direction: make a holder whose pid is an ancestor of the script a blocker (exit 4 before any signal, printed like the parent, with the note "an ancestor of this script"), and make `send_term` refuse it as well. `list_holders` already computes the ancestry set; expose it to `blockers_of` and `stop_holders` and add the test the code lacks: a stoppable ancestor with no other blocker, expecting exit 4 and no signal line.

## Tests added

N/A: the bug is open.

## Workarounds

Run the script from a shell that is not a descendant of a process that holds the worktree.

## Resume

Decide whether the narrow case justifies the change (it adds a blocker class), then add the failing test first.

## References

- Bug `3dde1548c3e5c310` (`docs/issues/2026-11-12-git-worktree-remove-refuses-after-codescout-and-agent-briefs-leave-untracked-files-in-a-worktree.md`), section Fix, the ruling on this gap.
- `docs/conventions/removing-a-worktree.md`, section "A blocker is not stoppable".
