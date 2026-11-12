---
kind: convention
status: active
title: Removing a worktree — list its holders first, stop them only on purpose
owners: []
tags:
  - worktrees
  - shared-checkout
  - scripts
---

# Removing a worktree — list its holders first, stop them only on purpose

**Audience:** anyone about to run `git worktree remove` on a linked worktree of this repo. Use
[`scripts/worktree-remove.sh`](../../scripts/worktree-remove.sh) instead; its suite is
[`tests/worktree-remove.sh`](../../tests/worktree-remove.sh).

## Why a bare `git worktree remove` fails

A worktree that served `symbols` on a Rust file keeps a `codescout mux ... rust-analyzer` process
and its `cargo check` child. After git deregisters the worktree, `cargo check` rewrites `target/`,
so git exits 255 with `Directory not empty` and a directory stays behind. Neither process stops
when another project is activated or when the worktree is removed (bug `3dde1548c3e5c310`).

## The order

1. Activate the main checkout, so this session's active project is not the worktree. If the
   worktree was removed while active, activation still works (bug `d291b97d9e69edef`).
2. Run `scripts/worktree-remove.sh <path>`. It lists each holder as `PID<TAB>KIND<TAB>CMDLINE`. With
   no holder it removes the worktree at once.
3. A mux is shared by every session working in that checkout, and the script cannot see which
   session is a client. Ask the occupant first
   ([`reaching-a-peer-session.md`](reaching-a-peer-session.md), Step 4 of its skill).
4. If the holders are the expected mux, rust-analyzer and cargo, run
   `scripts/worktree-remove.sh --stop <path>`. It sends SIGTERM, rescans, removes the worktree
   and checks that the directory is gone.
5. Delete the branch yourself afterwards with `git branch -d`.

The script never uses `--force`, never `rm`, never SIGKILL, and never deletes the branch.

## A blocker is not stoppable

A `codescout` process that is not a mux (for example `codescout start`, a live session's MCP
server) has kind `other`, even when it runs from `<worktree>/target/`. So has any editor, shell or
test run, and so has the script's own parent when its cwd is inside the worktree. With `--stop`, a
blocker makes the script exit 4 and signal nothing. Stop that session by hand.

## Exit codes

| code | meaning |
|---|---|
| 0 | the worktree is removed and verified gone |
| 2 | refused: usage, unregistered path, main checkout, caller's cwd inside, no `/proc`, or the first scan timed out; nothing changed |
| 3 | holders exist and `--stop` was not given |
| 4 | `--stop`, but a blocker exists; nothing was signalled |
| 5 | `--stop`, but a holder survived, or a rescan timed out; nothing was removed |
| 6 | git refused (untracked files, a lock); never forced |
| 7 | git removed it but the directory is still there; listed, never deleted |

Knobs: `--wait SECS` (default 30) bounds the rescan after SIGTERM.
`WORKTREE_REMOVE_SCAN_TIMEOUT` (whole seconds, default 10) bounds each scan; a scan that times out
means the holders are unknown, never none.

## What it does not cover

Its own header lists the limits. A green run is not proof that nothing holds the worktree:

- a process that holds a file descriptor only, with its cwd elsewhere and no path in argv;
- processes owned by another user, whose `/proc` entries it cannot read;
- a command line that spells the worktree through a symlink;
- a process that starts after the last scan (it shows up as exit 6 or 7).

The suite uses fixture `/proc` trees plus real processes inside its own temp worktrees. It cannot
say whether a listed mux has a live client, so the occupancy question stays with you.
