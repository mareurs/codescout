---
id: '1b544eac8264477f'
kind: bug
status: open
title: 'BUG: the cut-off hint of a run_command over a buffer pages the source stream, not the command''s own output'
tags:
- cluster/unclassified
- run_command
- progressive-disclosure
opened: 2026-10-08
owner: marius
related:
- docs/adrs/2026-10-08-a-read-keeps-the-handle-it-reads-and-one-real-file-has-one-handle.md
severity: low
---

# BUG: the cut-off hint of a `run_command` over a buffer pages the source stream, not the command's own output

## Summary

When a `run_command` that reads a buffer (`grep … @cmd_X`) prints more than the inline line cap, the response
is cut and gets no handle of its own. Its hint says `Next page: sed -n '101,200p' @cmd_X`. That pages the
SOURCE buffer. A filtering command printed different lines, so the caller never reaches the rest of the
filtered output.

## Symptom (Effect)

The caller sees `truncated: true`, `stdout_shown: 100`, `stdout_total: 3000` and a route that returns other
text than the cut-off lines. For `grep -n "" @cmd_X` the route returns lines without the `N:` prefix. For
`grep PATTERN @cmd_X` it returns unrelated source lines.

## Reproduction

Live binary, 2026-10-08:

1. `run_command("seq 1 3000")` gives `@cmd_199face4`.
2. `run_command("grep -n \"\" @cmd_199face4")` returns `1:1` … `100:100` and
   `"hint": "Output capped at 100 lines (stdout 100/3000). Next page: sed -n '101,200p' @cmd_199face4. Or grep 'keyword' @cmd_199face4 for targeted search."`.
3. There is no `output_id` in that response. `sed -n '101,200p' @cmd_199face4` returns `101` … `200`, not
   `101:101` … `200:200`.

An independent reviewer saw the same on the baseline `38265405` and on the merged one-handle tip.

## Environment

codescout on `experiments`, release binary of 2026-10-07. Linux.

## Root cause

Unknown — under investigation. The hint builder names the first `@` ref in the command as the page source.
It does not know that the command transformed the stream. Not yet read in code.

## Evidence

Steps 1 to 3 above. The behaviour of making no new buffer for a command over a buffer is consistent with
`docs/adrs/2026-10-08-a-read-keeps-the-handle-it-reads-and-one-real-file-has-one-handle.md`; only the hint
is wrong.

## Hypotheses tried

None yet.

## Fix

Not started. Options: name no page route when the command is not a plain slice of the source; or tell the
caller to narrow the command (`grep PATTERN @cmd_X | …` is blocked, so a narrower pattern or
`grep -m N`); or store the command's own output under its own `@cmd_*` as any other command does.

## Tests added

N/A — not fixed.

## Workarounds

Narrow the pattern, or use `grep -c` first and a tighter pattern. Use `read_file("@cmd_X", start_line=N,
end_line=M)` for a plain slice of the source.

## Resume

Read the hint builder in `src/tools/run_command/`. Decide the route with the operator, because one option
creates a handle for a command over a buffer.

## References

- `docs/adrs/2026-10-08-a-read-keeps-the-handle-it-reads-and-one-real-file-has-one-handle.md`
- `docs/issues/2026-10-08-a-read-of-a-buffer-made-another-buffer-and-one-real-file-had-many-handles.md`
