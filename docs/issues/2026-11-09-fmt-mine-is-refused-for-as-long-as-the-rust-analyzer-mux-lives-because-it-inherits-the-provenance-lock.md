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
unverified: 'Which run_command started the mux is not established, nor whether closing the inherited descriptors in the mux would weaken a guarantee the fence gives. The two peer-owned shared locks (pids 1520813 and 1731442) may be transient and were not waited out, so it is not shown that the mux''s lock alone keeps the exclusive try failing: that would need the peers'' processes to end, or a test that holds only the mux''s lock. The refusal count of five is from this session only.'
---

## Summary

`./scripts/fmt-mine.sh` through `run_command` is refused with `native provenance coordination refused dispatch: lock acquisition failed because the operation would block`, and stays refused while no tool call is running. The formatter needs the provenance lock exclusively. A shared copy of that lock is held, through inherited file descriptors, by the long-lived `codescout mux` and its `rust-analyzer` children. They outlive the call that took the lock, and the process that took it no longer exists. The refusal names no holder, so it reads the same as "a peer is mid-call".

## Class fit

This is the class "one session's tooling holds shared state for an operation, and every other session's read is wrong for that window, reported as truth": here the shared state is a lock, and the refusal looks like a transient collision. It differs from the class's other members in that the window does not end with the operation. A daemon spawned inside a fenced shell keeps the lock for as long as it lives, which was about three hours at the time of measurement.

## Symptom (Effect)

Observed 2026-11-09, session 677278c4: `./scripts/fmt-mine.sh` refused five times through `run_command`, including a baseline call with no job of mine running. `./scripts/gate.sh` ran normally in the same period, because inside the gate `fmt-mine` is run by a script, not through a fenced `run_command`.

## Reproduction

```
F=/home/marius/.local/state/codescout/native-provenance-v2/.codescout/native-provenance-v1.lock
flock -n -x $F true; echo $?     # 1   (the exclusive try the formatter makes)
flock -n -s $F true; echo $?     # 0   (the shared try every other call makes)
grep <inode of $F> /proc/locks   # the holders
```

Measured 2026-11-09: exit 1, then exit 0.

## Environment

Linux, codescout at `53a7d8ec6`, rust-analyzer 1.99.1 (toolchain path in the process list).

## Root cause

Established by reading code and by measurement:

- `is_foreground_formatter` (`src/tools/provenance_fence_context.rs:20`) matches exactly `./scripts/fmt-mine.sh` or `bash scripts/fmt-mine.sh`, in the foreground. For that call `scope_root` takes `Fence::try_acquire`, which is `file.try_lock()`: exclusive and non-blocking. It takes it twice: on the per-user coordination file (`~/.local/state/codescout/native-provenance-v2/.codescout/native-provenance-v1.lock`) and on the checkout's own fence. Every other write and `run_command` takes `try_participate`, which is `try_lock_shared`. A `flock` exclusive try fails while any shared holder exists.
- `apply_to_shell` (called only from the `run_command` spawn sites) duplicates both lock descriptors into the shell and clears close-on-exec in the child (`apply_to_command_named`, `src/tools/native_provenance_fence.rs`). Every descendant of the shell shares the same open file description, and a `flock` lasts while any descriptor on that description lives.
- Measured: `/proc/locks` shows three shared locks on the inode, owned by pids 3325677, 1520813 and 1731442. `ps` shows 3325677 does not exist. It was this session's own codescout server earlier: its pid is in the names of this session's earlier background-job records. The lock it took is still held by `codescout mux … -- rust-analyzer` (pid 3690455, up about 3 hours), its `rust-analyzer` (3690525) and two `rust-analyzer-proc-macro-srv` (3690992, 3690996): `/proc/<pid>/fdinfo/24` of each names that lock.
- The mux's environment carries `CODESCOUT_PROVENANCE_COORDINATOR_FD=24`, `CODESCOUT_PROVENANCE_FENCE_FD=25`, both roots and generations (5743 and 1353). So it was started through `apply_to_shell`, from a `run_command` shell.
- The other two owners are live peers: 1520813 (`codescout start`, with a `python3 …/live_smoke.py` child 34 s old) and 1731442 (`codescout start --debug`, 494 s old). Those may end on their own.
- The mux is shared by design (a socket under `/run/user/1000`, `--idle-timeout 180`), so it lives while any session uses rust-analyzer in this checkout.

Not established: which `run_command` started the mux (the generations 5743 and 1353 would identify it if the generation log can be queried; I did not look). Whether closing the inherited descriptors in the mux would weaken any guarantee the fence gives; the tests named `formatter_excludes_a_foreign_workspace_shell_targeting_its_checkout` and `the_actual_shell_inherits_the_request_scoped_fence` show the inheritance is intended for shells.

## Evidence

- `lsof` on the lock file: six processes (the two codescout servers, the python3 smoke test, the mux and its two children).
- `/proc/locks` and `/proc/<pid>/fdinfo/<fd>` as above.
- `ps -o pid,ppid,etimes,cmd` for the pids named.
- `flock -n -x` and `flock -n -s` results above.


Re-measured a few minutes later in the same session: `/proc/locks` listed FIVE shared locks on the inode (owners 662110, 1520813, 3325677, 3647863, 1731442), and `flock -n -x` still exited 1. Two owners were new, so other sessions take this lock continuously. The mux's lock (owner 3325677, a dead pid) was among the five. So the exclusive try could not be isolated to the mux by waiting. What the evidence does establish is the narrower claim: while the mux lives, the exclusive try cannot succeed, whatever the other holders do. A caller who gets "would block" cannot tell a peer that is mid-call from a lock that no call owns.

## Hypotheses tried

- "My own background job holds it": ruled out for the baseline call (no job of mine was running); it can still explain one refusal that overlapped my clippy job.
- "A peer's call is mid-flight": possible for the two live peers, but not for the mux's lock, whose owner is dead.

## Fix

Not fixed. This fence is another effort's design, so these are options, not a change:

1. Daemons close the inherited provenance descriptors at startup: `codescout mux` closes the descriptors named by `CODESCOUT_PROVENANCE_*_FD` and drops those variables, so neither it nor `rust-analyzer` carries the lock. Smallest change. Its cost is whatever guarantee the fence wants from the mux, which is not a write effect of the shell.
2. Make the refusal name its holders: on `WouldBlock`, read `/proc/locks` and say which pids hold the lock and whether they are alive. Independent of option 1, and it makes this diagnosable in one call.

## Tests added

N/A — not fixed.

## Workarounds

Format the files you changed with `rustfmt --edition 2021 <file>` after reading `git diff` for each, then `rustfmt --check`. Used five times in this session. The commit hook runs `rustfmt --check` on the committed bytes either way.

## Resume

1. Decide option 1, option 2, or both.
2. For option 1, find which `run_command` started the mux and whether the fence's own tests rely on a daemon holding the lock.
3. For either option, add a test that spawns a daemon inside a fenced shell and checks the formatter can still be taken after the shell ends.

## References

- `src/tools/provenance_fence_context.rs` (`is_foreground_formatter`, `scope_root`, `apply_to_shell`), `src/tools/native_provenance_fence.rs` (`try_mode`, `apply_to_command_named`, `coordination_root`).
- `docs/trackers/local-embed-opt-in-session-log.md` F-8.
- `docs/trackers/issue-clusters/IC-12-transient-shared-state-lies-to-readers.md`.
