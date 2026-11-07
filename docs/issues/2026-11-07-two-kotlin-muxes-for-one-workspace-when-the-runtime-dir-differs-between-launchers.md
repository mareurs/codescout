---
id: eceae82e2fd45815
kind: bug
status: fixed
title: Two kotlin muxes and two JVMs serve one workspace when codescout servers start with different XDG_RUNTIME_DIR
owners:
- marius
tags:
- cluster/unclassified
- lsp
- mux
closed: 2026-11-08
opened: 2026-11-07
severity: low
unverified: Not seen live. The two-mux case was not reproduced, before or after the fix, and the fix was proved only through the pure function runtime_dir_from. A deliberate XDG_RUNTIME_DIR that differs from /run/user/<uid> still splits launchers. The launcher that dropped the variable was not traced.
---

# BUG: Two kotlin muxes and two JVMs serve one workspace when codescout servers start with different XDG_RUNTIME_DIR

## Summary

On 2026-11-07 two `codescout mux` processes were alive for the same kotlin workspace (`--cwd /home/marius/work/claude/codescout`, workspace hash starting `7e868829`), each with its own `kotlin-lsp` JVM. One mux had its socket in `/run/user/1000`, the other in `/tmp/codescout-1000`. Sessions on the same workspace therefore do not share one language server, which the mux exists to guarantee.

## Class fit

No class in the roster states this defect: one resource (the workspace's mux) is named by a path whose directory is computed from the caller's environment, so two callers that agree on the workspace still disagree on the name. `cluster/unclassified` is used rather than forcing a fit. Candidate class if a second member appears: a shared resource's identity includes an environment variable of the caller.

## Symptom (Effect)

Two muxes and two JVMs for one workspace. Measured RSS of the two JVMs: 356588 kB and 306784 kB. Both muxes use `--idle-timeout 300`. No tool failure was seen. The cost seen is memory and a second kotlin index, built under a second `--system-path`.

## Reproduction

Not reproduced on purpose. Observed with `ps -eo pid,ppid,rss,etimes,args` filtered on `kotlin-lsp.*7e868829c00fa9b2`. To reproduce, start one codescout server with `XDG_RUNTIME_DIR=/run/user/1000` and one with it unset, on one kotlin workspace, and use a kotlin tool from each. Untested.

## Environment

Observed 2026-11-07 on `experiments` at `01d93907f`, Linux. Muxes: pid 1206394 (parent pid 9487, `systemd --user`; up 8245 s) and pid 2674203 (parent pid 2674124, `/home/marius/.cargo/bin/codescout start`; up 3690 s).

## Root cause

Read from source and from the processes, not from a controlled test.

- `src/socket_discovery.rs` `runtime_dir_from`: when `XDG_RUNTIME_DIR` is set, absolute and existing, the socket directory is that path. Otherwise it is `<temp_dir>/codescout-<uid>`.
- `workspace_hash` hashes only the workspace path (`src/socket_discovery.rs:10`). So the file name is equal for both muxes and only the directory differs.
- Environment of the two muxes, read from `/proc/<pid>/environ`: pid 1206394 has `XDG_RUNTIME_DIR=/run/user/1000` and `TMPDIR=/home/marius/.cache/cc-tmp`. pid 2674203 has neither variable, so it used `/tmp/codescout-1000`.
- `--system-path` and `GRADLE_USER_HOME` follow the temp directory, so the two JVMs use separate index directories. That is why the RocksDB lock in `docs/adrs/2026-06-11-mux-single-owner-invariant.md` did not collide.

The ADR states that every codescout process for one user, workspace and language computes the same socket path. That holds only when their environments agree on the runtime directory.

## Evidence

- `ps` lines for both muxes and both JVMs, 2026-11-07 (pids above; JVM pids 1206468 and 2674277).
- `/proc/2674203/environ`: no match for `TMPDIR` or `XDG_RUNTIME_DIR` (`grep` exit 1). `/proc/1206394/environ`: both present.
- A grep of `docs/issues` for `XDG_RUNTIME_DIR` found six files, none about two muxes for one workspace. That is a literal-string search, not a semantic one.

## Hypotheses tried

None tested. The environment difference is read from two live processes. Which launcher omitted `XDG_RUNTIME_DIR` is not known.

## Fix

`runtime_dir_from` (`src/socket_discovery.rs`) takes a new input, `user_run`, and `per_user_runtime_dir` passes `/run/user/<uid>`. The order is now:

1. A usable `XDG_RUNTIME_DIR` (absolute and existing), checked as before.
2. `/run/user/<uid>`, when it is absolute and exists. If it exists and is not private to this user, the call refuses, as for the first step. The docstring of `RuntimeDirError` asks for a refusal here, because a different directory chosen quietly would split processes that must meet.
3. `<temp>/codescout-<uid>`, as before.

A launcher that lost the variable now meets the ones that kept it, on a host where systemd makes `/run/user/<uid>`. Where that directory does not exist (macOS, a container), nothing changes. A deliberate `XDG_RUNTIME_DIR` that points elsewhere still gives another directory: this fix does not cover that case.

The second candidate in the earlier text (finding a mux across both directories) was not done.

**Scope found while fixing.** `per_user_runtime_dir()` also sites the index lock (`src/retrieval/index_lock.rs`, `lock_path`, line 139). The same environment difference therefore gave two index-lock files for one project, so the single-writer lock did not hold across such launchers. The fix covers that too. No live case of it was seen.

## Fix provenance

- **SHA:** `b4c4cad537e2ac644e49eb72c53ed44fa88c8599` (`experiments`)
- **patch-id:** `e152fa234d371578723691adc40be2a7b3b34283`

## Tests added

In `runtime_dir_trust` (`src/socket_discovery.rs`), through the pure `runtime_dir_from`:
- `a_launcher_without_xdg_meets_one_that_has_it`: the defect itself. It failed before the fix.
- `a_per_user_run_directory_that_is_not_private_is_refused`: a 0777 directory is refused, and the temp name is not created. It failed before the fix.
- `an_unusable_xdg_value_falls_to_the_per_user_run_directory`: an empty, relative or missing `XDG_RUNTIME_DIR` falls to the run directory. It failed before the fix.
- `a_missing_per_user_run_directory_falls_to_the_temp_name` and `a_usable_xdg_directory_wins_over_the_per_user_run_directory`: guard the kept behaviour. Both passed before the fix.

Mutant: a variant where the run directory wins over a usable `XDG_RUNTIME_DIR` fails `a_usable_xdg_directory_wins_over_the_per_user_run_directory`, and nothing else. It was reverted.

## Workarounds

Start every codescout server with the same `XDG_RUNTIME_DIR`. A stale duplicate mux can be stopped by killing its pid; its JVM is its child.

## Resume

Gate green on 2026-11-08 (FMT, CLIPPY, LEAN 6233, DEFAULT 9125, Python 1761). Not seen live: the duplicate mux was gone when the fix was written (one kotlin mux for the workspace, in `/run/user/1000`), and the case was never reproduced on purpose. After a rebuild, a mux that an older binary left in `/tmp/codescout-<uid>` can live up to its 300 s idle timeout beside a new one in `/run/user/<uid>`.

Still open: which launcher omitted `XDG_RUNTIME_DIR` (`/home/marius/.cargo/bin/codescout start`, pid 2674124 in the report) was not traced. The fix does not need it, but it would show whether other variables go missing too.

## References

- `src/socket_discovery.rs` (`workspace_hash`, `per_user_runtime_dir`, `runtime_dir_from`)
- `docs/adrs/2026-06-11-mux-single-owner-invariant.md`
- `src/lsp/mux/mod.rs` (`socket_path_for_workspace`)
