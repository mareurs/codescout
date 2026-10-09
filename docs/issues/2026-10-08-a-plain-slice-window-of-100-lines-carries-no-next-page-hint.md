---
id: d64d81d07ba58b6b
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
- docs/issues/archive/2026-10-08-the-cut-off-hint-of-a-run-command-over-a-buffer-pages-the-source-not-its-own-output.md
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

The hint is built in two places, and neither reaches a plain slice whose window fits whole. `capped_hint` builds it for a response that was CUT. `full_page_hint` builds it for a query that ends in a hint's own pager (`<base> | sed -n 'A,Bp'`), and only for that shape: it reads `trailing_pager`, which needs a `|`. A plain slice (`cat <ref>`, `sed -n 'A,Bp' <ref>`) is neither. Its 100-line window of short lines is returned whole, so the cut path never runs, and it has no `|`, so the pager path never matches. Nothing built a hint for it.

## Evidence

The reviewer's chain: lines 1 to 100, then 101 to 200, then no hint.

## Hypotheses tried

None.

## Fix

Fixed in `065caed6` (code and tests) and `d4eee531` (the tests that pin the mutation survivors), `fix/run-command-hint-followups`, `src/tools/run_command/output.rs`.

- `plain_slice_window` returns both ends of the window (`plain_slice_first_line` is now its first half). `next_window_hint(query, lines, stream_lines)` gives a plain slice whose window is exactly one page (`B - A + 1 == BUFFER_QUERY_INLINE_CAP`) that printed a full page (`lines == 100`) the hint `Next page: sed -n '{B+1},{B+100}p' <ref>`, while the queried stream has a line after `B` (`B < total`). A caller's own narrower or wider slice stays bare.
- A plain slice reads a stream the buffer holds, so its length is KNOWN and the last window names no further one (a trailing pager's `full_page_hint` has to say "may print more"). `stream_line_count` counts the stream as the shell sees it: `.err` is the stderr, a `@tool_*` ref is the pretty-printed JSON (`OutputBuffer::resolve_refs` writes it so), a `@bg_*` ref is the log on disk, `@cmd_*` and `@file_*` are the stored stdout. The stream is asked only once the window is a full page.
- `full_window_hint` joins the two rules. The inline gate counts the hint (`gate_keys`), so a page that fits only without it is cut, not re-buffered, and the room for a cut stdout prices it among the widest hints.

## Tests added

All in `src/tools/run_command/tests.rs` unless marked `output.rs`. Every one follows ONLY the hints, and runs the route a hint names.

- `a_full_plain_slice_window_names_the_next_one_and_the_hints_reach_the_last_line_once`: the reviewer's reproduction (3,000-line `@cmd_*`, `sed -n '101,200p'`), then `cat` followed by hints alone returns lines 1 to 3,000 once in 30 pages, the last page silent.
- `a_full_plain_slice_window_names_a_next_one_only_while_a_line_is_left`: streams of 199, 200, 201, 250 lines and the first window of 100 and 101.
- `only_a_window_of_exactly_100_lines_names_a_next_page`.
- `the_hints_alone_reach_the_last_line_once_when_pages_are_cut_by_bytes`: pages of a varying number of lines, some cut by bytes, some whole.
- One per ref kind: `a_full_window_of_the_err_stream_pages_that_stream`, `a_tool_ref_counts_its_lines_as_the_sed_that_reads_it_does` (compared with `sh` over the pretty-printed text), `a_file_ref_full_window_names_the_next_one`, `a_background_log_full_window_names_the_next_one`.
- `a_full_plain_slice_window_that_fits_only_without_its_hint_is_cut_not_rebuffered`: the hint is inside the inline gate.
- `output.rs`: `a_plain_slice_window_has_both_ends_in_every_quote_form`, `next_window_hint_wants_a_full_page_and_a_line_left`, `a_full_window_hint_never_outgrows_the_keys_a_cut_response_adds`.

Mutation round (`./scripts/mutation-probe.sh`, one mutant at a time, the `tools::run_command` suite, 343 tests): `<` to `<=` on the last line, the window width off by one, the reversed-range guard, the `lines` guard, the start and the end of the next window, the `.err` stream, the `@tool_*` pretty rule, the `@bg_*` branch, the unquoted-pair of the slice regex, dropping the hint from the gate, dropping the hint from the fit arm, dropping the whole rule: all KILLED. SURVIVED: dropping the hint from the room that prices a cut stdout, and from the arm that returns an uncut page after the summary gate. Both are unreachable today: the cut skeleton holds three keys the gate's measure does not (`truncated`, `stdout_shown`, `stdout_total`), so a window the gate calls too big is always cut. `a_full_window_hint_never_outgrows_the_keys_a_cut_response_adds` pins that margin and reds if a hint is ever worded longer than it.

## Workarounds

Read the buffer with `read_file("@X", start_line=N, end_line=M)`.

## Resume

`capped_hint` and `full_page_hint` in `src/tools/run_command/output.rs`.

## References

- `docs/issues/archive/2026-10-08-the-cut-off-hint-of-a-run-command-over-a-buffer-pages-the-source-not-its-own-output.md`
