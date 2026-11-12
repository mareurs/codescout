---
id: '9189852e75f80f08'
kind: spec
status: active
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
- No change to the superpowers plugin (checked on 2026-11-12: its templates need none).
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
3. Classify each holder by the basename of `argv[0]` from `/proc/<pid>/cmdline`, never by `comm`. Stoppable: `codescout` when `argv[1]` is exactly `mux`; any `rust-analyzer*` (a prefix match); `cargo`; `rustc`; `sccache`; and any other executable under `<path>/target/` (kind `build-output`). Anything else is a blocker unless it descends from a stoppable holder: then it is stoppable, kind `child` (the C compiler chain of a cargo build script is one). The script finds the ancestors by following `PPid:` in `/proc/<pid>/status`, also through processes that are not holders. The walk ends, and the row stays a blocker, at pid 1 or 0, at a repeated pid, at a `status` it cannot read, at a `PPid:` that is not a number, after 64 steps, and at an ancestor of the script. A `codescout` that is not a mux (an MCP server) is always a blocker, even under `<path>/target/` or under a stoppable holder: stopping it would break a session. Any listed holder that is an ancestor of the script (its parent included) is a blocker too, whatever its kind: the script never signals itself, its subshells or its ancestors, and no ancestor of the script makes another process a `child`. The script learns its ancestors from `PPid:` in `/proc/<pid>/status` with the same bounds as the `child` walk, so the ancestry is known only as far as `status` can be read along the chain.
4. Without `--stop`: print a table (pid, kind, command line) and the occupancy warning, then exit 3 if there is any holder.
5. With `--stop`: if a blocker exists, print it and exit 4 without signalling anything. Otherwise SIGTERM the mux first, then the other stoppable holders, and print one stderr line per signal (`worktree-remove: signalled <pid> <kind> with SIGTERM`, or `worktree-remove: could not signal <pid> <kind> (already gone, or not permitted)`); wait up to `--wait` seconds (default 30), re-scanning about every 0.5 s. The script signals new holders in at most three rounds, because a dying `cargo` can start children. A blocker found by the first scan of the stop loop (one that appeared after the first scan) ends the run with exit 4 before any signal. A survivor, or a rescan that timed out, is a failure (exit 5); there is no SIGKILL to a holder. Every scan is bounded by `WORKTREE_REMOVE_SCAN_TIMEOUT` (default 10 s), and a scan that fails means the holders are unknown, never none.
6. Run `git worktree remove <path>` without `--force`. If git fails, ask `git worktree list` whether the worktree is still registered. Still listed (untracked files, a lock): print git's message and `git status --porcelain -uall`, capped at 50 lines, and exit 6; do not force. Ignored files are not listed, because they never block git; git deletes them with the worktree. No longer listed: git removed the work tree and then failed (the live `cargo check` case), so go on to step 7.
7. Run `git worktree prune`. Verify that the directory is gone. If it is not, print its contents and exit 7; do not delete it.

**Activation fix.** Verified cause: `workspace(action="activate")` is a write (`Workspace::is_write`), so `scope_root` in `src/tools/provenance_fence_context.rs` fences it on the active root, and `canonical_checkout(root).or_else(|_| root.canonicalize())?` (line 183) fails with `No such file or directory (os error 2)` once that root is gone. The suspect `Fence::try_mode` named earlier was wrong. The fix: `scope_root` runs a call unfenced when it is a write, the tool is `workspace`, `action` is `activate`, and `root.try_exists()` is `Ok(false)`. Any other write on a missing root is still refused.

## Tests

- Script: `tests/worktree-remove.sh`, a shell suite (266 assertions). It uses a fixture `/proc` (`WORKTREE_REMOVE_PROC`, with pids above `pid_max`, and `status` files for the ancestor walk), real-process cases, and `git` shims. It asserts exit codes 0 to 7, that the directory is gone, which processes were signalled, and the signal lines. `tests/worktree_remove_script.rs` runs the suite from `cargo nextest` and is `#![cfg(target_os = "linux")]`, because the suite reads `/proc` and needs GNU `realpath` and `timeout`. A signal sent to a fixture pid cannot be observed; it shows as `could not signal`. A real-process case covers a `child` that is stopped and removed.
- Activation: `activation_proceeds_after_the_default_root_was_deleted` (`src/server/workspace_pin_tests.rs`) and three tests in `src/tools/provenance_fence_context_tests.rs`. The unit test `a_workspace_write_on_a_missing_root_runs_unfenced` and the dispatch test failed with `os error 2` before the fix and pass after it; the two "still refused" tests are controls that pass both before and after.
- Mutation sweeps on the classification and the re-scan loop were run by hand during review. No row was added to `docs/PROBES.md`.

## Left to the builder

The test harness location for the script; the exact process-matching code; the wording of the occupancy warning; where the pointer to the script lives (`docs/RELEASE.md` or a new `docs/conventions/` page); the seam for the activation fix.

## Revisit when

The `--stop` path runs often (it signals that approach B is worth its cost), or a peer reports a mux that the script stopped.
