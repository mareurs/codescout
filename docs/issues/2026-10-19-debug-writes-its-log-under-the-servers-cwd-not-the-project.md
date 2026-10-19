---
id: bc4fee088b7236c2
kind: bug
status: open
title: 'BUG: `--debug` writes `<cwd>/.codescout/debug.log` relative to the server''s working directory'
owners:
- marius
tags:
- cluster/unclassified
opened: 2026-10-19
owner: marius
severity: low
unverified: 'Reported by an independent review of bdf34bdb4..46329d271 (evidence: scratchpad/rv-pin2/probe1.json, probe3-base.json); NOT reproduced by the session that filed this file. The root cause is a hypothesis.'
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

Unknown. Hypothesis, not tested: the log appender is configured with a relative `.codescout` directory in `src/logging.rs`.

## Evidence

The reviewer's probe files only.

## Hypotheses tried

None by the filing session.

## Fix

Not implemented. Candidate: derive the debug log path from the resolved project root or the per-user state directory.

## Tests added

N/A: not fixed; this file records a finding.

## Workarounds

Do not pass `--debug` from a directory you do not want a `.codescout/` in.

## Resume

Reproduce with `codescout start --debug` in an empty directory, one tool call, then `ls -A`; read `src/logging.rs` for the path.

## References

- `src/logging.rs`
