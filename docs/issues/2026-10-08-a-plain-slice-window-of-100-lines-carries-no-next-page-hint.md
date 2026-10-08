---
id: '9665b32b963f464c'
kind: bug
status: open
title: 'BUG: a plain-slice window of exactly 100 lines carries no next-page hint, so following the hints stops at line 200'
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

# BUG: a plain-slice window of exactly 100 lines carries no next-page hint, so following the hints stops at line 200

## Summary

For a plain slice (`cat <ref>`, `sed -n 'A,Bp' <ref>`) the cut-off hint names `Next page: sed -n '101,200p' <ref>`.
That next page prints exactly 100 lines, which fit whole, so its response is not cut and carries no hint. A
reader who follows only the hints stops at line 200 of a longer buffer. The 2026-10-08 hint fix closed this
for a trailing pager on a non-slice query (`full_page_hint`), not for a plain slice.

## Symptom (Effect)

`cat @X` over 3,000 short lines is cut at 100 and names `sed -n '101,200p' @X`. Running that prints lines
101 to 200 with no hint. The same for a `@tool_*` ref: the chain ends at line 200 of 1,604.

## Reproduction

Reproduced by the final reviewer through `RunCommand::call_content` on the merged tip, and the same on the
baseline `840fba50`.

## Environment

codescout on `experiments`. Linux.

## Root cause

The hint is built only for a response that is cut. A window that fills the 100-line page exactly is not cut.

## Evidence

The reviewer's chain: lines 1 to 100, then 101 to 200, then no hint.

## Hypotheses tried

None.

## Fix

Not started. `full_page_hint` already names the next window for a trailing pager; the plain-slice path needs
the same rule, counted by the inline gate.

## Tests added

N/A — not fixed.

## Workarounds

Read the buffer with `read_file("@X", start_line=N, end_line=M)`.

## Resume

`capped_hint` and `full_page_hint` in `src/tools/run_command/output.rs`.

## References

- `docs/issues/2026-10-08-the-cut-off-hint-of-a-run-command-over-a-buffer-pages-the-source-not-its-own-output.md`
