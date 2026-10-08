---
id: 5deb547504115de9
kind: bug
status: fixed
title: 'BUG: a read of a buffer made another buffer on every call, and one real file had a new handle per read'
tags:
- cluster/unclassified
- read_file
- read_markdown
- progressive-disclosure
- output-buffer
closed: 2026-10-08
opened: 2026-10-07
owner: marius
related:
- docs/adrs/2026-10-08-a-read-keeps-the-handle-it-reads-and-one-real-file-has-one-handle.md
- docs/adrs/2026-10-05-a-result-keeps-the-one-handle-its-tool-gave-it.md
- docs/issues/archive/2026-10-06-sibling-sweep-the-byte-bound-defect-recurred-across-tools.md
severity: medium
unverified: 'CLEARED 2026-10-08. Was: not landed on experiments and not probed on a rebuilt binary. Landed as a fast-forward to a2871af0 and probed live on the rebuilt binary the same day (section Fix provenance). What the live probe did not reach is listed there and in Still open.'
---

# BUG: a read of a buffer made another buffer on every call, and one real file had a new handle per read

## Summary

`read_file` of a `@cmd_*`, `@file_*` or `@tool_*` ref stored the slice it returned under a new `@file_*`
handle. The same happened to a `json_path` value. A real file read twice got two handles. The operator's
rule is that the one handle holding the whole buffer is the one every call refers to. Fixed in 18
commits; the rule and its limits are in
`docs/adrs/2026-10-08-a-read-keeps-the-handle-it-reads-and-one-real-file-has-one-handle.md`.

## Symptom (Effect)

- A ranged read of a buffer over the inline budget returned `Buffer: @file_…` and a `file_id`. The `next`
  route named the source ref, so the caller held two handles for one text.
- Every identical call returned a different handle. `store_file` has no dedup, and the pool holds 50
  entries, so each `next` of a long buffer evicted an older handle the caller might still hold.
- `json_path` with `start_line`/`end_line` on a `@tool_*` ref ignored the range and minted a handle.
- A real file read whole twice, or by range twice, gave two handles for unchanged bytes.

## Reproduction

On the live binary of 2026-10-07 (`OutputBuffer::new(50)`):

1. `run_command("seq 1 3000")` gives `@cmd_X`. `read_file("@cmd_X", start_line=1, end_line=2500)` returns a
   new `@file_`. `read_file("@cmd_X")` with no range returns none.
2. Repeat any ranged read: a different handle each time.
3. `read_file("docs/RELEASE.md", force=true)` twice gives two handles.

The merged tip has a guard that does this for every source kind and arm at a full pool:
`tools::read_file::buffer_edge_tests::no_read_of_an_existing_buffer_mints_a_handle`.

## Environment

codescout on `experiments` at `38265405`. Linux. Release binary rebuilt from that tree on 2026-10-07.

## Root cause

`read_from_buffer` stored its slice and its `json_path` value with `store_file`, and the real-file arms
called `store_file` or `store_file_excerpt` on every read. `store_file_inner` mints a new id each time
and has no lookup. The reason given in the code for the slice store (BUG-026, keep the response small
enough that `call_content` does not wrap it in `@tool_*`) was false under the current code: the page is
sized with `response_fits`, `response_room` and `buffer_page` and does not depend on the stored copy.

## Evidence

Before and after, from an independent reviewer on baseline `38265405` and merged `2dd3e106`, in-process,
pool of 50:

| Case | Baseline | Merged |
|---|---|---|
| clamped wide line of a buffer, twice | 1 new handle per call | 0 |
| `json_path` value, twice, then a range | 1 new handle per call; range ignored | 0; paged with `Next:` |
| 1,000-line buffer range, followed to the end | 12 new handles | 0 |
| 456-line markdown file, 11 reads | 8 handles | 1 |
| wide line of a real file, twice | 1 new handle per call | 1 for both |

Defects the independent reviews found in the first fixes, all repaired before landing:
- `json_path` `next` held `<your json_path: N bytes, not repeated here>`, which fails when copied.
- Following `next` from an unranged read stopped at line 3,046 of 20,000. This was on baseline too.
- A slow reader could overwrite the one handle with older text (the stale-write race). The new code
  stamps the entry with the mtime seen before the read and refuses an older read.
- A holder of the handle was no longer told of a refresh.
- A multi-heading error with duplicated headings carried no route.
- A file with a future mtime that was then edited kept serving old text. Baseline refreshed it.
- The Text form counted the cut-marker line (`[2 of 2 lines shown]` beside `shown_lines:[1,1]`), in
  `format_read_file_body` and again in `markdown::format_read`.
- The generic hint offered `json_path="$.stdout"` on any `@tool_*`; it fails on a ref that is not a
  `run_command` envelope.
- The delivered Text of a buffered markdown range page had no handle, no `next` and no line count. This
  was on baseline too.

## Hypotheses tried

- Dedup only, keeping a snapshot handle for ranges and sections: rejected by the operator, because a
  changed file still made a second handle.
- Size limit for the whole-file handle: offered, not chosen.

## Fix

Eighteen commits, two branches merged, listed with their patch-ids in the ADR (section Built):
`e62f1444`…`ee13088b`. Patch-ids were computed on the merged range, `git patch-id --stable`. In short:

- `read_from_buffer` stores nothing. All page shapes go through `page_of_buffer_text`.
- `json_path` plus `start_line`/`end_line` pages the value on the same ref (cap
  `read_file.json_path_route_room`).
- `OutputBuffer::store_file_read` deduplicates by resolved path, updates a changed file in place, stamps it
  with the pre-read mtime capped at the clock, and marks the change for the holder.
- `read_markdown` reuses the file's handle for ranges and sections through `whole_text_handle`.

## Fix provenance

- **SHA:** `e62f14446f293d87cf44f6f3ad01c9fd51c2b7bd` (`experiments`)
- **patch-id:** `8898abac7ab0669fc3348a18aa0e4e1b7812da7b`

That commit is the one that stopped `read_from_buffer` from minting. It is the anchor of a series: the other
seventeen code commits and their patch-ids are in the ADR, section Built.

Landed on `experiments` as a fast-forward to `a2871af0` on 2026-10-08, after a gate on that exact tip:
`FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`, 86 `test result:` lines, 0 FAILED. Nothing is pushed by this session
(`origin/experiments` was `e0bdad97`). The eighteen code commits and their patch-ids are in the ADR, section
Built; the docs commit is `d37cf5ff`. The catalog rows made in the worktree were folded with
`librarian(action="merge_worktree")` before the worktree was removed (4 merged, 3 reseated, no conflicts).

**Live probe on the rebuilt binary, 2026-10-08** (real MCP calls, not in-process tests):

| Call | Result |
|---|---|
| `read_file("docs/RELEASE.md", force=true)`, twice, file unchanged | `@file_19bf4485` both times (baseline: two handles) |
| `read_file("docs/RELEASE.md", heading="## Standard Ship Sequence")` (oversized) | error carries `file_id` `@file_19bf4485`, the same handle; `next_actions` in the file's lines, `start_line=77, end_line=264` |
| `read_file("@cmd_19bf41cd", start_line=1, end_line=1)` on a 3,000-byte `\x01` line, twice | no `Buffer:` line, no new handle; the hint's `grep -o` route names `@cmd_19bf41cd` |
| `read_file("@tool_19bfb7fe", json_path="$.file_groups")` (764 lines) | first page `[246 of 764 lines shown]`, `Next: read_file("@tool_19bfb7fe", json_path="$.file_groups", start_line=247, end_line=764)`; no new handle |
| the same with `start_line=1, end_line=3` | the first 3 lines of the value; no handle |
| `read_file("@tool_19bfb7fe", end_line=5)` | refused: `end_line provided without start_line` |
| a 600-line file read twice, edited (`v1`→`v2`), read by path again | the same handle `@file_19bfb470`, now holding `v2` |
| `wc -l @file_19bfb470` after that edit, then again | first call prints `↻ @file_19bfb470 refreshed from disk (file changed since last read)`; second prints none; `read_file(handle, 1..2)` shows `v2` |
| `grep` tool over `docs/RELEASE.md` (189 matches) | still a new `@tool_19bfb7fe`: the class that stays |

Not reached by the live probe: the stale-write race, the future-mtime case, eviction at a full pool of 50,
`peer knowledge`, and a `json_path` longer than 7 KB (prose `next`). Those rest on the in-process tests and the
three independent reviews.

## Tests added

- `no_read_of_an_existing_buffer_mints_a_handle`: 54 cases, every source kind and arm, pool full, follows every `next`.
- `every_over_budget_read_of_one_file_names_its_one_handle` and its companions in
  `src/tools/read_file_one_handle_tests.rs`.
- Stale-write, future-mtime, equal-mtime, refresh-notice and read-window tests in `output_buffer.rs`.
- Route tests that execute every emitted `next` and hint through the real tool.

## Workarounds

None needed. Before the fix, a caller could grep the source ref and ignore the extra handle.

## Still open

Recorded in the ADR (Revisit when): no byte budget on the pool (12 files of 5 MB read by range keep 62.9 MB);
`read_file` `json_path`/`toml_key` of a real file still stores a snapshot handle; `peer knowledge` makes a
local `@tool_*` for a large remote buffer; the refresh notice reaches only `run_command`; `\r\n` and a trailing
newline are lost when `json_path` pages are joined; `largest_field_path` runs on every `@tool_*` read; the
`json_path` `path` echo is still clipped with a "bytes shown" note above 300 bytes; `read_file("@file_X.err",
heading=…)` returns a `file_id` with an empty stream. Filed separately: the `run_command` buffer-query
hint pages the wrong stream.

## Resume

Nothing owed on this defect. Remaining work is in Still open and in the ADR (Revisit when): a byte budget for the
buffer pool, paging a `json_path`/`toml_key` value of a real file on its own path, and `peer knowledge`. The
related `run_command` hint bug is filed separately
(`docs/issues/2026-10-08-the-cut-off-hint-of-a-run-command-over-a-buffer-pages-the-source-not-its-own-output.md`).

## References

- `docs/adrs/2026-10-08-a-read-keeps-the-handle-it-reads-and-one-real-file-has-one-handle.md`
- `docs/adrs/2026-10-05-a-result-keeps-the-one-handle-its-tool-gave-it.md`
- `docs/adrs/2026-10-07-one-measure-of-the-delivered-response.md`
- `docs/issues/archive/2026-10-06-sibling-sweep-the-byte-bound-defect-recurred-across-tools.md`
- `docs/issues/archive/2026-08-25-file-slice-handle-refreshes-to-whole-file.md` (why excerpts were snapshots)
