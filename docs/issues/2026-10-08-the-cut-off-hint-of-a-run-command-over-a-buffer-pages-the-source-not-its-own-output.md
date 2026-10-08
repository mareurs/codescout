---
id: '1b544eac8264477f'
kind: bug
status: fixed
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
unverified: 'Not landed on experiments and not probed on a rebuilt binary: every claim above comes from in-process tests through RunCommand::call_content and from reviewers. The hint tests were run under dash (1,166 targeted tests, 0 failed) but not on macOS or Windows. Mutation counts were reported by the fixing agents and reviewers.'
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

`capped_hint` in `src/tools/run_command/output.rs` always built `Next page: sed -n '{from},{to}p' {handle}`,
as if the command's output were a contiguous slice of the queried buffer. That holds only for `cat <ref>`
and `sed -n 'A,Bp' <ref>`. `query_first_line` also matched a `sed -n` anywhere in the command, so
`grep x @ref | sed -n '5,10p'` was read as a slice that starts at line 5. The same assumption sat in the
wide-line remedy (`cut -c1-4000 <ref>`), in the `@tool_*` arm, and in the own-stderr remedy.

## Evidence

Steps 1 to 3 above. The behaviour of making no new buffer for a command over a buffer is consistent with
`docs/adrs/2026-10-08-a-read-keeps-the-handle-it-reads-and-one-real-file-has-one-handle.md`; only the hint
is wrong.

## Hypotheses tried

None yet.

## Fix

Two populations. A **plain slice** (the whole command is `cat <ref>` or `sed -n 'A,Bp' <ref>`) keeps the
route on the handle, in the buffer's own numbering. **Every other command** printed its own lines, which no
buffer holds: the hint says the rest is stored nowhere and names a route that works.

- Append `| sed -n 'N,Mp'` to the same query, when the query is one pipeline and the real IL-3 gate
  (`detect_il3_violation`) accepts the joined command. Otherwise: narrow the query.
- A query that already ends in the pager the hint advised has that last stage replaced, never stacked, and
  the next window starts at the line after the last one actually shown. Following only the hints reaches
  every line once, also when pages are cut by bytes.
- A query containing `$'`, `$(`, a backtick, unbalanced quotes or a trailing newline is not offered a pager:
  the splitter does not model that quoting, and a list hidden by it paged the wrong command.
- The query is never echoed, so the hint stays bounded.
- The same rule applies to `@tool_*` refs and `@bg_*` refs, to the wide-line route, and to the own-stderr
  remedy.

## Fix provenance

- **SHA:** `2cd90b71f0c8b95d0ab115628063b16e243d9b86` (`experiments`)
- **patch-id:** `a6efa912705b8d25cd2148659560248233029a80`

That is the first commit of a series; the others and their patch-ids are in the ADR
(`docs/adrs/2026-10-08-a-read-keeps-the-handle-it-reads-and-one-real-file-has-one-handle.md`, section Built).
Two independent reviews found three blockers in the first version (a list hidden by `$'`, `$(` or backtick
quoting, a "step by 100" that skipped lines after a byte cut, a trailing newline) and a third review found two
tests that fail under dash, the `/bin/sh` of Ubuntu CI. All are fixed.

## Tests added

Through the real `RunCommand`: the reproduction (`grep -n ""` over a 3,000-line buffer), a sparse filtering
grep, following ONLY the emitted hints to the end for 1,000 rows of 200 B, variable widths, a 30-line stored
stderr, `grep`, `awk` and `rg` (compared byte for byte with `sh` over a real file), an adversarial table of 80
query shapes, the `@tool_*` and `@bg_*` routes, and the hint's size against a 4 KB query. 34 mutants were
killed in the last round of the implementer and 28 new ones were applied by the final reviewer, 22 killed by
the suite and 6 by tests written for them.

## Workarounds

Narrow the pattern, or use `grep -c` first and a tighter pattern. Use `read_file("@cmd_X", start_line=N,
end_line=M)` for a plain slice of the source.

## Resume

After landing: probe the live tools (`grep -n "" @cmd_X` over a 3,000-line buffer, then
follow its hint), clear the `unverified` caveat and archive this file. Remaining, filed separately: the
own-stderr remedy on a multi-stage pipeline, a plain-slice window of exactly 100 lines that carries no
next-page hint, and unescaped paths in routes. Not fixed and out of scope: `shuf`-style random output cannot
be re-run into the same lines; a `sed 's/a/x/'` query makes a new handle.

## References

- `docs/adrs/2026-10-08-a-read-keeps-the-handle-it-reads-and-one-real-file-has-one-handle.md`
- `docs/issues/archive/2026-10-08-a-read-of-a-buffer-made-another-buffer-and-one-real-file-had-many-handles.md`
