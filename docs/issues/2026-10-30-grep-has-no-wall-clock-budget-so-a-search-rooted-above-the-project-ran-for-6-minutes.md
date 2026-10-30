---
id: '731747939c0bb764'
kind: bug
status: open
title: 'grep has no wall-clock budget: a search rooted at a parent of many repos ran 386 s, one at $HOME ran 135 s'
tags:
- cluster/unclassified
opened: 2026-10-30
---

# BUG: grep has no wall-clock budget: a search rooted at a parent of many repos ran 386 s, one at $HOME ran 135 s

## Summary

`grep` with a `path` outside the active project walks the whole tree it is given and has no time budget. Three calls in `.codescout/usage.db` account for 552 of the 622 seconds spent in `mode=files` calls with a `path`: 386,711 ms (path `/home/marius/work/claude`, pattern `\bJev\b|\bjev\b|JEV`), 135,109 ms (path `/home/marius`, glob `**/*.d.ts`) and 30,602 ms (path the codescout repo, four globs). Every other call is fast: p50 59 ms, p90 495 ms, p99 6.5 s, n=344.

## Class fit

`cluster/unclassified`. The case shows an unbounded walk whose cost depends on a directory the caller chose, and whose absence of any ceiling turns a typo-sized request into a six-minute hang.

## Symptom (Effect)

A tool call that runs far past the MCP client's own timeout. The two long calls were made against trees that hold many repositories (build outputs, `node_modules`, `.d.ts` trees). The normal in-project call is not affected.

## Reproduction

Not run: it would take minutes and walk `$HOME`. The inputs are in `usage.db` (`tool_name='grep'`, `input_json` with `path` set to a parent directory).

## Environment

Linux, release build, 2026-10-06 to 2026-10-20 calls.

## Root cause

Unknown - under investigation. Not read this session: whether the walk honours `.gitignore` outside a git root, whether `max_results` stops the walk early when the match count is small, and whether any deadline exists.

## Evidence

usage.db query 2026-10-30 over all `grep` calls with `mode=files` and a `path` (n=344): sum 621.8 s; 18 calls over 1 s account for 580.2 s; 3 calls over 30 s.

## Hypotheses tried

None.

## Fix

Candidate, not chosen: a wall-clock budget (for example 20 s) after which the call returns the matches found so far with a scope line saying the walk was cut and where, per `docs/adrs/2026-08-27-negative-results-name-their-scope.md`. A partial result must never read as a complete one.

## Tests added

None yet.

## Workarounds

Pass a narrower `path`, or a `glob` that excludes build and dependency directories.

## Resume

Read the grep walk code to see whether a deadline or an early stop exists; reproduce on a bounded tree with many ignored directories and time it; then decide the budget and the wording of the cut notice.

## References

- `docs/adrs/2026-08-27-negative-results-name-their-scope.md`
