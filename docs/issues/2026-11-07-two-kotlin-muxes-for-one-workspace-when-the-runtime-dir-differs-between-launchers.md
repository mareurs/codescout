---
id: eceae82e2fd45815
kind: bug
status: open
title: Two kotlin muxes and two JVMs serve one workspace when codescout servers start with different XDG_RUNTIME_DIR
owners:
- marius
tags:
- cluster/unclassified
- lsp
- mux
opened: 2026-11-07
severity: low
unverified: 'No controlled reproduction: both muxes were observed live, not started on purpose. The launcher that started the second one is only partly identified (its parent is `/home/marius/.cargo/bin/codescout start`, pid 2674124, itself a child of pid 646545, not traced further). No harm beyond memory was observed, and whether two JVMs on separate index dirs ever corrupt or starve each other was not tested.'
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

None. Candidates, none evaluated: resolve the runtime directory from the uid alone (`/run/user/<uid>`) when it exists, whatever the caller's environment says; or discover an existing mux by workspace hash across both directories before spawning. Each changes where sockets live for every launcher, so each needs its own design.

## Tests added

None.

## Workarounds

Start every codescout server with the same `XDG_RUNTIME_DIR`. A stale duplicate mux can be stopped by killing its pid; its JVM is its child.

## Resume

Find what starts `~/.cargo/bin/codescout start` without `XDG_RUNTIME_DIR`. Then decide between the two candidate fixes above, and add a test through `runtime_dir_from` with the variable absent.

## References

- `src/socket_discovery.rs` (`workspace_hash`, `per_user_runtime_dir`, `runtime_dir_from`)
- `docs/adrs/2026-06-11-mux-single-owner-invariant.md`
- `src/lsp/mux/mod.rs` (`socket_path_for_workspace`)
