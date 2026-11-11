---
id: '9189852e75f80f08'
kind: spec
status: draft
title: Worktree removal helper and the activation-after-removal fix — design
owners:
- marius
tags:
- worktree
- tooling
---

# Worktree removal helper and the activation-after-removal fix — design

Bug: `3dde1548c3e5c310` (git worktree remove refuses / leaves a directory behind) and bug `d291b97d9e69edef` (activation fails once the active project's directory is gone). Fix 1 of that bug (the `.gitignore` rules) is already merged. This design covers fix 3, plus one defect the probes found.

## Intent and why

The operator said on 2026-11-12: "we can't leave dangling codescout/superpowers trash lying around". Three probes on 2026-11-12 showed that a worktree which served `symbols` on a Rust file keeps a live `codescout mux ... rust-analyzer` process and its `cargo check` child. After `git worktree remove` deregisters the worktree, `cargo check` writes `target/` again, so git fails with `Directory not empty` and a directory is left behind. Nothing stops those processes when the operator activates another project or removes the worktree.

## Goals

1. One script, `scripts/worktree-remove.sh`, that is the correct path for removing a linked worktree and ends in a safe state: no registered worktree, no directory, no process still using it.
2. `workspace(action="activate")` works when the current active project's directory no longer exists.

## Non-goals and anti-goals

- No change to the LSP manager, the mux protocol or `ActivateProject`'s handling of the project it leaves (approach B was rejected for now).
- No change to the superpowers plugin (bug fix 2 stays open there).
- The script never uses `git worktree remove --force`, never uses `rm -rf`, never sends SIGKILL, and never deletes the branch.
- Anti-goal: a script that kills a process a peer session is working with. A green run must not hide that risk.

## Constraints

- A mux is keyed by workspace and shared across sessions. The script cannot see which session is a client, so stopping holders needs an explicit `--stop` and a printed occupancy warning (`docs/conventions/reaching-a-peer-session.md`, Step 4: ask the occupant first).
- A graceful mux exit (SIGTERM) runs `killpg` on rust-analyzer's process group (`src/lsp/mux/process.rs`). Whether `cargo check` stays in that group is not verified; the script therefore re-scans after the stop and treats survivors as blockers.
- Follows the repo's wrapper pattern (`scripts/git-safe-reset.sh`, `scripts/gate.sh`): the safe path is the only easy path.
- Bash, Linux `/proc` (as `scripts/stale-servers.sh` already does). On a host without `/proc` the script refuses and says why.
- No new text in `CLAUDE.md` (byte budget); the pointer goes in `docs/RELEASE.md` or a `docs/conventions/` page.

## Approaches considered

| | What | Trade-off |
|---|---|---|
| A (chosen) | Script finds and stops holders, then removes the worktree. | Covers every holder and every harness; a path heuristic that cannot see peers. |
| B | `LspManager` releases the clients of the project that activation leaves. | Fixes the owner's cleanup; the mux still waits 180 s unless the mux protocol gains a stop message, and old mux binaries do not know it. |
| C | A now, B later. | B is revisited when the stop path of A runs often or a peer reports a killed mux. |

The operator chose A on 2026-11-12.

## Design

**Script behavior** (`scripts/worktree-remove.sh [--stop] [--wait SECS] <worktree path>`):

1. Resolve the path against `git worktree list --porcelain`. Refuse the main checkout, an unregistered path, and a path that contains the caller's current directory.
2. Scan `/proc/<pid>` for holders: a process whose `cwd` is under the path, or whose command line contains the path as a whole path component (not as a substring of a longer name).
3. Classify each holder. Stoppable: `codescout mux`, `rust-analyzer`, `cargo`, `rustc`, `sccache`, and any executable under `<path>/target/`. Anything else is a blocker.
4. Without `--stop`: print a table (pid, kind, command line) and the occupancy warning, then exit 3 if there is any holder.
5. With `--stop`: if a blocker exists, print it and exit 4 without signalling anything. Otherwise SIGTERM the mux first, then the other stoppable holders; wait up to `--wait` seconds (default 30); re-scan, at most three rounds, because a dying `cargo` can start children. A survivor is a failure (exit 5); there is no SIGKILL.
6. Run `git worktree remove <path>` without `--force`. If git refuses because of untracked files, print `git status --porcelain --ignored -uall` for the path and exit 6; do not force.
7. Run `git worktree prune`. Verify that the directory is gone. If it is not, print its contents and exit 7; do not delete it.

**Activation fix.** Suspected cause (not yet verified): `Fence::try_mode` in `src/tools/native_provenance_fence.rs` calls `root.canonicalize()?` on the active root, so every write-class tool, `workspace(activate)` included, fails with `No such file or directory (os error 2)` once that root is gone. The first plan task verifies this with a reproducing test. The fix lets activation proceed without the old root's fence. The builder decides the exact seam after the test shows where the error starts.

## Tests

- Script: a test that builds a temporary git repo with a linked worktree and fake holders (a `sleep` process with its cwd in the worktree as a blocker; a renamed copy of `sleep` named `rust-analyzer` as a stoppable holder). It asserts exit codes 3, 4 and 0, that the directory is gone, and that no process was signalled in the blocker case.
- Activation: a test that removes the active project's directory and then activates the main checkout. It must fail before the fix and pass after.
- Mutation check on the script's classification and on the re-scan loop, per `docs/PROBES.md`.

## Left to the builder

The test harness location for the script; the exact process-matching code; the wording of the occupancy warning; where the pointer to the script lives (`docs/RELEASE.md` or a new `docs/conventions/` page); the seam for the activation fix.

## Revisit when

The `--stop` path runs often (it signals that approach B is worth its cost), or a peer reports a mux that the script stopped.
