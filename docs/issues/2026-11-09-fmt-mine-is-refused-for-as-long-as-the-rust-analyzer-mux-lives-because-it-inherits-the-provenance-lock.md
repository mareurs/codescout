---
id: '312cde2f4698874b'
kind: bug
status: open
title: 'BUG: fmt-mine is refused for as long as the rust-analyzer mux lives, because the mux inherits the provenance lock'
owners:
- marius
tags:
- cluster/transient-shared-state-lies-to-readers
opened: 2026-11-09
owner: marius
related:
- docs/trackers/issue-clusters/IC-12-transient-shared-state-lies-to-readers.md
severity: medium
unverified: 'Which run_command started the mux is not established, nor whether closing the inherited descriptors in the mux would weaken a guarantee the fence gives. The flock exit codes were taken inside a run_command, which itself holds a shared lock, so they do not isolate the mux, and the record no longer relies on them. What is established without them: the kernel records a shared lock owned by a pid that no longer exists, and the mux and its children hold that same description. The refusal count of five is from this session only. Owner 1731442 was first misidentified as a peer and is corrected in the body.'
---

## Summary

`./scripts/fmt-mine.sh` through `run_command` is refused with `native provenance coordination refused dispatch: lock acquisition failed because the operation would block`, and stays refused while no tool call is running. The formatter needs the provenance lock exclusively. A shared copy of that lock is held, through inherited file descriptors, by the long-lived `codescout mux` and its `rust-analyzer` children. They outlive the call that took the lock, and the process that took it no longer exists. The refusal names no holder, so it reads the same as "a peer is mid-call".

## Class fit

This is the class "one session's tooling holds shared state for an operation, and every other session's read is wrong for that window, reported as truth": here the shared state is a lock, and the refusal looks like a transient collision. It differs from the class's other members in that the window does not end with the operation. A daemon spawned inside a fenced shell keeps the lock for as long as it lives, which was about three hours at the time of measurement.

## Symptom (Effect)

Observed 2026-11-09, session 677278c4: `./scripts/fmt-mine.sh` refused five times through `run_command`, including a baseline call with no job of mine running. `./scripts/gate.sh` ran normally in the same period, because inside the gate `fmt-mine` is run by a script, not through a fenced `run_command`.

## Reproduction

Two parts, and the second is the stronger one.

1. Look at the lock without trying it:

```
F=/home/marius/.local/state/codescout/native-provenance-v2/.codescout/native-provenance-v1.lock
grep <inode of $F> /proc/locks                 # the owners, as the kernel records them
ps -p <owner pid>                              # an owner that no longer exists
grep -E '^(lock|flags)' /proc/<mux pid>/fdinfo/<fd>   # the mux holds that same lock
```

   Measured 2026-11-09: owner 3325677 does not exist, and `fdinfo` of the mux, its `rust-analyzer` and its `proc-macro-srv` children each names that lock.

2. A unit test rebuilds the mechanism without any peer: `a_lock_kept_by_a_child_of_a_dead_owner_is_named_as_such` in `src/tools/native_provenance_fence/tests.rs` runs `flock -s <file> sh -c 'sleep 30 & echo $!'`, so the owner exits and a background child keeps the lock, then calls `Fence::try_acquire`.

**Do not use `flock -n -x` run from inside `run_command` as a reproduction.** I did, and got exit 1, but the `run_command` that ran it holds a shared lock itself while it runs, so the exclusive try fails whatever else is true. That result is confounded and proves nothing about the mux. Run it from a terminal outside codescout, and read `/proc/locks` first to see which locks exist.

## Environment

Linux, codescout at `53a7d8ec6`, rust-analyzer 1.99.1 (toolchain path in the process list).

## Root cause

Established by reading code and by measurement:

- `is_foreground_formatter` (`src/tools/provenance_fence_context.rs:20`) matches exactly `./scripts/fmt-mine.sh` or `bash scripts/fmt-mine.sh`, in the foreground. For that call `scope_root` takes `Fence::try_acquire`, which is `file.try_lock()`: exclusive and non-blocking. It takes it twice: on the per-user coordination file (`~/.local/state/codescout/native-provenance-v2/.codescout/native-provenance-v1.lock`) and on the checkout's own fence. Every other write and `run_command` takes `try_participate`, which is `try_lock_shared`. A `flock` exclusive try fails while any shared holder exists.
- `apply_to_shell` (called only from the `run_command` spawn sites) duplicates both lock descriptors into the shell and clears close-on-exec in the child (`apply_to_command_named`, `src/tools/native_provenance_fence.rs`). Every descendant of the shell shares the same open file description, and a `flock` lasts while any descriptor on that description lives.
- Measured: `/proc/locks` shows three shared locks on the inode, owned by pids 3325677, 1520813 and 1731442. `ps` shows 3325677 does not exist. It was this session's own codescout server earlier: its pid is in the names of this session's earlier background-job records. The lock it took is still held by `codescout mux … -- rust-analyzer` (pid 3690455, up about 3 hours), its `rust-analyzer` (3690525) and two `rust-analyzer-proc-macro-srv` (3690992, 3690996): `/proc/<pid>/fdinfo/24` of each names that lock.
- The mux's environment carries `CODESCOUT_PROVENANCE_COORDINATOR_FD=24`, `CODESCOUT_PROVENANCE_FENCE_FD=25`, both roots and generations (5743 and 1353). So it was started through `apply_to_shell`, from a `run_command` shell.
- The other two owners: 1520813 is a live `codescout start` of another session, with a `python3 …/live_smoke.py` child 34 s old at the time. 1731442 is this session's own server (its pid prefixes the names of this session's background-job records): the `run_command` that made these measurements held a shared lock itself while it ran. I first called 1731442 a live peer; that was wrong.
- The mux is shared by design (a socket under `/run/user/1000`, `--idle-timeout 180`), so it lives while any session uses rust-analyzer in this checkout.

Not established: which `run_command` started the mux (the generations 5743 and 1353 would identify it if the generation log can be queried; I did not look). Whether closing the inherited descriptors in the mux would weaken any guarantee the fence gives; the tests named `formatter_excludes_a_foreign_workspace_shell_targeting_its_checkout` and `the_actual_shell_inherits_the_request_scoped_fence` show the inheritance is intended for shells.

## Evidence

- `lsof` on the lock file: six processes (the two codescout servers, the python3 smoke test, the mux and its two children).
- `/proc/locks` and `/proc/<pid>/fdinfo/<fd>` as above.
- `ps -o pid,ppid,etimes,cmd` for the pids named.
- `flock -n -x` (exit 1) and `flock -n -s` (exit 0), run from inside `run_command`: CONFOUNDED, see Reproduction. They show only that some shared lock existed, and the measuring call supplied one itself.


Re-measured a few minutes later in the same session: `/proc/locks` listed FIVE shared locks on the inode (owners 662110, 1520813, 3325677, 3647863, 1731442; the last is this session's own server), and `flock -n -x` still exited 1, which is confounded as above. Two owners were new, so other sessions take this lock continuously. The mux's lock (owner 3325677, a dead pid) was among the five. So the exclusive try could not be isolated to the mux by waiting. What the evidence does establish is the narrower claim: while the mux lives, the exclusive try cannot succeed, whatever the other holders do. A caller who gets "would block" cannot tell a peer that is mid-call from a lock that no call owns.

## Hypotheses tried

- "My own background job holds it": ruled out for the baseline call (no job of mine was running); it can still explain one refusal that overlapped my clippy job.
- "A peer's call is mid-flight": possible for the two live peers, but not for the mux's lock, whose owner is dead.

## Fix

Partly done. **Option 2 is in; option 1 is not, and this bug stays open.**

- Option 2, `4eceee5e7` and `0d47f5898`: a refused fence lock now names its holders (Linux): the processes that have the lock file open, inherited descriptors included, and the owners the kernel records, each marked "not running" when gone. A list cut by its 12-entry cap or by the 400 ms scan limit ends in "and more". Only the process name and a plain lowercase subcommand are shown, never other arguments.
- Verified: unit tests on the pure parts; a test that rebuilds the dead-owner case with `flock -s <file> sh -c 'sleep 30 & echo $!'`; a test that starts 14 holders; mutation runs for the holder text and for the cut flag; and live on the rebuilt server. A refused `./scripts/fmt-mine.sh` named the running gate and its children, a dead owner (this session's previous server, whose gate was still running on inherited descriptors, the mechanism of this bug reproduced within minutes), and the dead owner of the mux's lock.
- Option 1, not done: daemons such as `codescout mux` close the inherited provenance descriptors at startup. The fence is another effort's design, so this stays a decision for its owner.

What option 2 does and does not change: a caller who is refused can now tell a peer that is mid-call from a lock that no call owns. The refusal itself is unchanged, and the mux can still keep the lock for hours.

## Tests added

N/A — not fixed.

## Workarounds

Format the files you changed with `rustfmt --edition 2021 <file>` after reading `git diff` for each, then `rustfmt --check`. Used five times in this session. The commit hook runs `rustfmt --check` on the committed bytes either way.

## Resume

1. Decide option 1. Find which `run_command` started the mux, and whether the fence's own tests rely on a daemon holding the lock.
2. For option 1, add a test that spawns a daemon inside a fenced shell and checks the formatter can still be taken after the shell ends. The existing `a_lock_kept_by_a_child_of_a_dead_owner_is_named_as_such` rebuilds the dead-owner setup and can be extended.
3. Close this bug when the mux no longer keeps the lock, not when the refusal is more readable.

## References

- `src/tools/provenance_fence_context.rs` (`is_foreground_formatter`, `scope_root`, `apply_to_shell`), `src/tools/native_provenance_fence.rs` (`try_mode`, `apply_to_command_named`, `coordination_root`).
- `docs/trackers/local-embed-opt-in-session-log.md` F-8.
- `docs/trackers/issue-clusters/IC-12-transient-shared-state-lies-to-readers.md`.
