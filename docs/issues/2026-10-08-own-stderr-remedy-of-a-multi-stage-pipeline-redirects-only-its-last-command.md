---
id: d830b95eb2a07ce2
kind: bug
status: open
title: 'BUG: the own-stderr remedy of a buffer query tells a multi-stage pipeline to append a redirection that only reaches its last command'
tags:
- cluster/unclassified
- run_command
- progressive-disclosure
opened: 2026-10-08
owner: marius
related:
- docs/issues/2026-10-08-the-cut-off-hint-of-a-run-command-over-a-buffer-pages-the-source-not-its-own-output.md
severity: low
---

# BUG: the own-stderr remedy of a buffer query tells a multi-stage pipeline to append a redirection that only reaches its last command

## Summary

When a buffer query's own stderr is cut, the marker says: rerun the query with `2>&1 >/dev/null | cut -c1-4000`
appended. The 2026-10-08 hint fix gates this advice with `can_append_stage`, which excludes lists. A pipeline
is not a list, but `2>&1` applies to its LAST command only, so for a multi-stage pipeline the advice reads the
wrong stderr.

## Symptom (Effect)

`cat @X | tr -d '\n' >&2 | cat` gets the marker "rerun the query with `2>&1 >/dev/null | cut -c1-4000`
appended". Running that route returns 0 bytes of stdout and a stderr cut again (1,997 bytes).

## Reproduction

Reproduced by the final reviewer (`RunCommand::call_content`, 2026-10-08): the query above on a buffer `@X`
with a stderr stream above the budget. The base before the hint fix behaves the same.

## Environment

codescout on `experiments`. Linux.

## Root cause

`can_append_stage` checks `is_one_pipeline` and the IL-3 gate for the joined command. Neither tests whether the
stage's `2>&1` reaches the stream that was cut.

## Evidence

The reviewer's run above.

## Hypotheses tried

None.

## Fix

Not started. Options: wrap the query (`{ q; } 2>&1 >/dev/null | cut -c1-4000`), or refuse when there is more
than one stage.

## Tests added

N/A — not fixed.

## Workarounds

Narrow the query so it prints less to stderr.

## Resume

`bound_buffer_stderr` and `can_append_stage` in `src/tools/run_command/output.rs`. Execute the chosen route in
a test.

## References

- `docs/issues/2026-10-08-the-cut-off-hint-of-a-run-command-over-a-buffer-pages-the-source-not-its-own-output.md`
