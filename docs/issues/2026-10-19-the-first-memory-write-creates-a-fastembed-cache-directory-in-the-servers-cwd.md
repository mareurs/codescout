---
id: '03652f9f373efd00'
kind: bug
status: open
title: 'BUG: the first memory write creates `.fastembed_cache/` in the server''s working directory'
owners:
- marius
tags:
- cluster/unclassified
opened: 2026-10-19
owner: marius
severity: low
unverified: 'Reported by an independent review of bdf34bdb4..46329d271 (evidence: scratchpad/rv-pin2/probe1.json, probe1.table); NOT reproduced by the session that filed this file. The root cause is a hypothesis.'
---

# BUG: the first memory write creates `.fastembed_cache/` in the server's working directory

## Summary

Reported by the review of `bdf34bdb4..46329d271`: the first `memory` write through the server creates a `.fastembed_cache/` directory in the process's current working directory, not under the project or a cache directory. With a pinned call the cwd has nothing to do with the project, so a stray directory appears next to wherever the server was started.

## Class fit

A path that should be derived from the project or a per-user cache directory is derived from the process's cwd, so the side effect lands wherever the server happens to run. It is the same family as the pin bug that left `x/.codescout/` behind: a build-up of state at an ambient location.

## Symptom (Effect)

`<cwd>/.fastembed_cache/` exists after the first memory write of a server session whose cwd is not a project.

## Reproduction

Not reproduced by the session that filed this. The reviewer's observations are in `scratchpad/rv-pin2/probe1.json` and `probe1.table` (of the session that ran the review).

## Environment

Linux, debug build of branch `fix/workspace-pin-2026-10-18` after commit `46329d271`.

## Root cause

Unknown. Hypothesis, not tested: the embedding library's default cache directory is relative (`.fastembed_cache`) and the memory write path initialises it without setting an explicit cache dir. The embedder construction is in `crates/codescout-embed/`.

## Evidence

The reviewer's probe tables only.

## Hypotheses tried

None by the filing session.

## Fix

Not implemented. Candidate: pass an explicit cache directory (the per-user cache dir) where the embedder is built.

## Tests added

N/A: not fixed; this file records a finding.

## Workarounds

Start the server from a directory where a stray `.fastembed_cache/` does no harm, and delete it.

## Resume

Reproduce with a server started in an empty directory, one `memory` write, then `ls -A`; find where the embedder takes its cache path.

## References

- `crates/codescout-embed/src/lib.rs`
