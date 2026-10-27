---
id: bc4fee088b7236c2
kind: bug
status: fixed
title: 'BUG: `--debug` writes `<cwd>/.codescout/debug.log` relative to the server''s working directory'
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-10-27
fix_patch_id: 9f97f2a171ca36a7543084251de96fd2db615af0
fix_sha: ad4c6ed51 (experiments)
opened: 2026-10-19
owner: marius
severity: low
unverified: 'The unit test covers log_dir_for, not init: nothing runs `codescout start --project X --debug` from another directory and looks at where the files land. The original symptom was never reproduced end to end.'
---

# BUG: `--debug` writes `<cwd>/.codescout/debug.log` in the server's working directory

## Summary

Reported by the review of `bdf34bdb4..46329d271`: a server started with `--debug` writes its log to `<cwd>/.codescout/debug.log`, relative to the process's current working directory, not to the project root or a state directory. Tool output that reaches the log (including a refused pin's message) is therefore stored in whatever directory the server happened to be started in.

## Class fit

A location that should be derived from the project or a per-user state directory is derived from the process's cwd, so the file lands wherever the server runs. It is the same family as the pin bug that left `x/.codescout/` behind.

## Symptom (Effect)

`<cwd>/.codescout/debug.log` exists after a `--debug` run, in a cwd that is not a project.

## Reproduction

Not reproduced by the session that filed this. The reviewer's observations are in `scratchpad/rv-pin2/probe1.json` and `probe3-base.json` (of the session that ran the review).

## Environment

Linux, debug build of branch `fix/workspace-pin-2026-10-18` after commit `46329d271`.

## Root cause

Verified 2026-10-27 by reading the code, not by running `start --debug`: `init` in `src/logging.rs` built the directory from `std::env::current_dir()` alone, and `main` calls it before clap parses (`src/main.rs`, so that startup errors are captured). The original hypothesis, a relative `.codescout` directory, was close: the directory was absolute but derived from the cwd.

## Evidence

The reviewer's probe files only.

## Hypotheses tried

None by the filing session.

## Fix

Fixed on `experiments` in `ad4c6ed51` (patch-id `9f97f2a171ca36a7543084251de96fd2db615af0`). `log_dir_for(args, cwd)` reads `--project X` and `--project=X` from the raw arguments, because logging cannot ask clap. It returns `<project>/.codescout` when the path names an existing directory (a relative path is joined to the cwd), and `<cwd>/.codescout` otherwise. A typo or a file therefore cannot make a `.codescout` appear where no project exists. `init` calls it for `debug.log`, the diagnostic log and `crash.log`, which share one directory, so `crash.log` moves with them for every subcommand that takes `--project`.

Without `--project` the behaviour is unchanged: the cwd. That is the normal MCP launch, where the cwd is the project root.

## Tests added

`the_log_dir_follows_an_existing_project_argument_and_falls_back_to_the_cwd` in `src/logging.rs`: both spellings, a relative path, no argument, a dangling `--project`, a path that does not exist, and a path that is a file. RED was the stub returning the cwd's directory. The 14 `logging::tests` pass, and the full `./scripts/gate.sh` was green for the batch.

## Workarounds

Do not pass `--debug` from a directory you do not want a `.codescout/` in.

## Resume

Reproduce with `codescout start --debug` in an empty directory, one tool call, then `ls -A`; read `src/logging.rs` for the path.

## References

- `src/logging.rs`
