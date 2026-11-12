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
   worktree was removed while active, see "If the active worktree is already gone" below.
2. Run `scripts/worktree-remove.sh <path>`. It lists each holder as `PID<TAB>KIND<TAB>CMDLINE`. With
   no holder it removes the worktree at once.
3. A mux is shared by every session working in that checkout, and the script cannot see which
   session is a client. Ask the occupant first
   ([`reaching-a-peer-session.md`](reaching-a-peer-session.md), Step 4 of its skill).
4. If every holder is one of the stoppable kinds (`mux`, `rust-analyzer`, `cargo`, `rustc`,
   `sccache`, `build-output`, `child`), run `scripts/worktree-remove.sh --stop <path>`. It sends
   SIGTERM and
   prints one stderr line per signal (`worktree-remove: signalled <pid> <kind> with SIGTERM`, or
   `worktree-remove: could not signal <pid> <kind> (already gone, or not permitted)`), so the
   output names each process it signalled. It then rescans, removes the worktree and checks that
   the directory is gone.
5. Delete the branch yourself afterwards with `git branch -d`.

Git deletes the worktree's ignored files with it (`target/`, `.superpowers/`, `.codescout/`,
`.buddy/`): copy out anything you keep, for example `.superpowers/`, before the removal.

The script never uses `--force`, never runs `rm`, never sends SIGKILL to a holder, and never
deletes the branch. (Its own scan child runs under `timeout -k 1`, which may SIGKILL that child.)

## If the active worktree is already gone

Bug `d291b97d9e69edef`: once the active project's worktree directory was gone,
`workspace(action="activate")` failed with `No such file or directory (os error 2)` for every
path, the main checkout included. The fix is in `scope_root`
(`src/tools/provenance_fence_context.rs`): an activation whose active root no longer exists runs
without the provenance fence. A server binary built before that fix still shows the symptom until
it is rebuilt and reconnected (`./scripts/rb.sh`, then `/mcp`). On such a server, `mkdir` the
removed path, activate the main checkout, then delete the stub with `rmdir <path>`, which refuses a
non-empty directory. If `rmdir` reports "Directory not empty", list it and remove only
`<path>/.codescout` (a server built before the fix may create fence files there when the stub is
outside any git checkout; a stub under `.worktrees/` resolves to the main checkout and stays
empty), then `rmdir` again. That stub is the one directory you delete by hand. Activating the main
checkout first (step 1) avoids the case.

## A `child` is stoppable

A process that would be `other` by its command line is a `child` when a holder of a stoppable
kind (`mux`, `rust-analyzer`, `cargo`, `rustc`, `sccache`, `build-output`) is among its ancestors:
the C compiler chain of a cargo build script (`make`, `cc`, `cc1`, `as`, `sh configure`) is
one. The script finds the ancestors by following `PPid:` upward, also through processes that are
not holders. The walk ends, and the row stays `other`, at pid 1 or 0, at a pid it has seen, at a
`status` it cannot read, at a `PPid:` that is not a number, and after 64 steps. A long-lived
process that a stoppable holder started (for example `cargo run`) is a `child` too and is
signalled. A descendant that was re-parented to the nearest subreaper or to pid 1 after its
ancestor exited is `other` in the rescan: it is never signalled for that reason, and if it
survives, the run ends in exit 5. Never a `child`: any ancestor of the script (its parent and the
parent's ancestors), and a `codescout` that is not a mux.

## A blocker is not stoppable

A `codescout` process that is not a mux (for example `codescout start`, a live session's MCP
server) has kind `other` and stays one, even when it runs from `<worktree>/target/` or under a
stoppable holder. So has any editor, shell or test run that has no stoppable holder among its
ancestors. The script's own parent is a blocker too when its cwd is inside the worktree: its kind
comes from the command line like any row (it can be `cargo` or `rust-analyzer`), and it is a
blocker because the script compares each row's pid with its own parent's, whatever the kind. With
`--stop`, a blocker makes the script exit 4 and signal nothing, whether it was there at the first
scan or appeared before the stop loop's first scan. Stop that session by hand.

## Exit codes

| code | meaning |
|---|---|
| 0 | the worktree is removed and verified gone (also with `-h`/`--help`, which prints the usage and removes nothing) |
| 2 | refused: usage, unregistered path, main checkout, caller's cwd inside, no `/proc`, or the first scan timed out; nothing changed |
| 3 | holders exist and `--stop` was not given |
| 4 | `--stop`, but a blocker exists; nothing was signalled |
| 5 | `--stop`, but a holder survived, or a rescan timed out; nothing was removed, but SIGTERM may already have been sent |
| 6 | git refused and still lists the worktree (untracked files, a lock); never forced. Git's message and `git status --porcelain -uall` (first 50 lines, then `... and N more`) are printed; ignored files are not listed |
| 7 | the directory is still there, though git removed the worktree (it may have exited non-zero after deregistering it, the bug itself); its contents are listed, never deleted |

When git fails but no longer lists the worktree, the script prunes, then checks the path as after
a success: exit 7 if the directory remains, exit 0 only if it is absent.

Knobs: `--wait SECS` (a whole number without leading zeros, default 30) bounds the rescan after
SIGTERM; the pause after the mux signal (up to about 5 s on a busy `/proc`) is not counted in it.
`WORKTREE_REMOVE_SCAN_TIMEOUT` (whole seconds, default 10) bounds each scan; a scan that times out
means the holders are unknown, never none.

## What it does not cover

Its own header lists the limits. A green run is not proof that nothing holds the worktree:

- a process that holds a file descriptor only, with its cwd elsewhere and no path in argv;
- processes owned by another user, whose `/proc` entries it cannot read;
- a command line that spells the worktree through a symlink;
- a process that starts after the last scan (it may show up as exit 6 or 7, or not at all: a
  late process that writes nothing lets git succeed, and exit 0 leaves a process whose cwd is
  deleted).

The suite uses fixture `/proc` trees plus real processes inside its own temp worktrees. It cannot
say whether a listed mux has a live client, so the occupancy question stays with you.
