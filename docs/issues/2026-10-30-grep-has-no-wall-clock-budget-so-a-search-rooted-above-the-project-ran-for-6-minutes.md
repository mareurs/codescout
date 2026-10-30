---
id: '731747939c0bb764'
kind: bug
status: fixed
title: 'grep has no wall-clock budget: a search rooted at a parent of many repos ran 386 s, one at $HOME ran 135 s'
tags:
- cluster/unclassified
opened: 2026-10-30
closed: 2026-10-30
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

Confirmed 2026-10-30 by reading `src/tools/grep.rs` (`Grep::call`): the walk is `ignore::WalkBuilder` with `git_ignore(true)` and nothing that reads a clock. Lines mode stops only on the match cap (`collect_limit`) or the byte cap; files mode (`mode="files"`) has no early stop at all and reads every admitted file. A search with few matches over a huge tree therefore walks the whole tree. `require_git` defaults to true, so outside a git root `.gitignore` is not honoured and `node_modules`/build trees are walked in full.

## Evidence

usage.db query 2026-10-30 over all `grep` calls with `mode=files` and a `path` (n=344): sum 621.8 s; 18 calls over 1 s account for 580.2 s; 3 calls over 30 s.

## Hypotheses tried

None.

## Fix

Shipped on branch `worktree-agent-ac59e41db7b0d1eb3` (a worktree off `experiments`), SHA `59d870c9d`, patch-id `902b9b617a5970f593b4138330fdaf5be0d302e6`. `src/tools/grep.rs`: new `WalkClock` polled once per walk entry in both the lines and the files loop; `time_budget_ms` input (default `DEFAULT_WALK_BUDGET_MS` = 20000). When it is spent the walk stops and the result carries a `walk_cut` string naming the budget, the files read, the entry being walked and the root, and saying a missing match proves nothing about the rest. `format_grep` renders it in the zero-match early return and below the header otherwise, so a partial result cannot read as a complete one.

Follow-up `299c5e78b` (same branch): `DEFAULT_WALK_BUDGET_MS` carries `cap-class: RESULT_CAP grep.walk_budget`, a probe row in `src/tools/core/cap_probe.rs` (mutation `NotYet`: the emission was not deleted and the row observed red), and `TOOL_SURFACE_CHAR_BUDGET` ratcheted 57_607 to 57_688 (+81, the new schema property). Full `./scripts/gate.sh` green on the final tree: FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0.

Not done: the 20 s default is the candidate from this file, not derived from a measurement of the cost of a real in-project search (the usage sample's p99 was 6.5 s); a tree needing more passes `time_budget_ms`.

## Tests added

`tools::grep::tests::a_spent_walk_budget_cuts_the_walk_and_names_it_in_both_modes` (watched fail first: result had no `walk_cut`; budget 0 is the deterministic stand-in for a slow tree) and `tools::grep::tests::an_ordinary_search_carries_no_walk_cut`. The 6-minute reproduction itself was not run.

## Workarounds

Pass a narrower `path`, or a `glob` that excludes build and dependency directories.

## Resume

Read the grep walk code to see whether a deadline or an early stop exists; reproduce on a bounded tree with many ignored directories and time it; then decide the budget and the wording of the cut notice.

## References

- `docs/adrs/2026-08-27-negative-results-name-their-scope.md`
