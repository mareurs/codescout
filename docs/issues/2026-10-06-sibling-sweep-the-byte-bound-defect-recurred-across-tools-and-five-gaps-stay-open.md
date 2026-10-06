---
id: 2d90c8f5401b93d4
kind: bug
status: open
title: 'BUG: the size-measured-in-one-unit, returned-in-another defect recurred across tools; most instances are fixed, five residual gaps stay open'
tags:
- cluster/unclassified
- run_command
- read_file
- read_markdown
- progressive-disclosure
opened: 2026-10-06
owner: marius
related:
- docs/issues/archive/2026-10-05-run-command-json-stdout-overflow-has-no-working-json-path-recovery.md
- docs/issues/archive/2026-10-05-run-command-test-envelope-failures-field-has-no-byte-bound.md
- docs/adrs/2026-10-05-a-result-keeps-the-one-handle-its-tool-gave-it.md
severity: medium
unverified: Five residual gaps are open and unfixed (section Still open). The byte-edge sweep covered run_command, read_file whole file, read_file heading= and memory read on ASCII, quote, backslash, control-character, euro and emoji payloads; the heading-not-found list of read_markdown was NOT probed, and no workspace notice was triggered. The full gate ran on 40dc79cd; the landed tip 327e5609 differs from it only by docs/issues files.
---

# BUG: the size-measured-in-one-unit, returned-in-another defect recurred across tools; most instances are fixed, five residual gaps stay open

## Summary

After the two `run_command` byte-bound bugs were fixed (`e833abeb`, `fac7abce`), four forked reviewers swept the sibling tools. They found the same defect again and again. A size is measured in one unit (raw bytes, pretty text, a summary alone, or a response before later keys are added). A different thing is returned: the compact serialized whole response, which the inline limit counts (`exceeds_inline_limit`: more than 10,003 bytes). JSON escaping makes the gap large: `\x01` and `\x1b` serialize to 6 bytes, `"` and `\` to 2. The sweep fixed the instances below. A final independent review of the merged tip found one more (F1, fixed) and the residual items listed under Still open.

## Symptom (Effect)

A response a tool had bounded still went over the inline limit. The caller then got a second `@tool_*` handle (or a content-free envelope), or a hint that named a route that returned nothing. Examples, all from the sweep:

- `read_file` of a whole file returned `@tool_*` and `@file_*` together because its summary was sized in raw bytes.
- A `run_command` buffer query on a wide first line returned zero bytes of a non-empty result.
- The `wip_authors` field and the libtest compacted response were bounded in raw bytes.
- A `.err` buffer query's next-page hint named the bare handle and paged stdout (F1, found by the final review).

## Reproduction

Each fix carries its own regression test (section Tests added). F1: store stdout `x\n` and 300 lines of stderr in a buffer, then run `cat @cmd_X.err`. Before `e08e1a6c` the hint said `sed -n '81,180p' @cmd_X`, which pages stdout.

## Environment

Linux, codescout on `experiments`. Source read at `fac7abce`; reviewed range `fac7abce..40dc79cd`.

## Root cause

One recurring shape, found independently in about five places: a budget helper measures its own part in a convenient unit, and nothing measures the final response. The fixes either move the unit (escaped bytes, `json_escaped_len`, `elide_middle_escaped`) or measure the response that will actually be returned (`inline_response_exceeds_limit`, `response_extras_len`, `fit_summary`, `fit_envelope`, `compacted_fits`).

## Fixed

Each line is a fix with its SHA on `experiments`; the patch-id is given for the ones cited as anchors.

- Buffer-query stored stderr bounded by bytes: `8bd5f6f0` (patch-id `6061abd9adfe69e9ed4e0665416e2595a2da25b4`).
- Summary-or-inline gate measures the serialized response: `ea084bdf` (`c21c93ce73ab827cd705f4e8c222c9ab27ec7de0`).
- `read_file` whole-file summary bounded by serialized bytes, one handle per read: `d2ef26e9` (`0959f9dd1c43a953bfe233de9da65ce6a4e0f893`), then `3880453d`, `e6ef4700`.
- `wip_authors` bounded at the source: `b0d6f675` (`1606227afecf60c27f65c51db3a788f3d1f491f1`), then `379ad1dc` (escaped bytes).
- `librarian` hint prefers the text of a text-heavy result: `53734054` (`197c175fdcde4c457f50946b22bd571f1e25de22`). `memory` hint names the array it has: `fea64f16` (`2533657f682d880c5227048ef67358ddc60c4e78`). `read_file` and `json_path` hints name routes that work: `38708292`, `53bab804`.
- Buffer query budgeted from its measured response: `58cebaf7` (`8178c82c14f0deac1fc62256489dd7d06edf901e`), then `7973517e`, `5a1828e7`.
- `read_markdown` heading map, oversized-section error and multi-heading read bounded by bytes: `0963b41b`, `8ff27467`, `27a3a7c4`, `56c5c162`.
- The prebuffered-envelope backstop never re-cuts a field that already carries a marker: `fd0bb946` (`7c95d244584fe77c2fdc4639ac6a881422879a7b`).
- A libtest run is judged by its compacted response: `e0edaeb1` (`beeeff16aa84f50b038ff628fe3dbbb839bba3c6`).
- F1, found by the final review: a next-page hint on an `.err` query names the `.err` stream: `e08e1a6c` (`c32ae80c4993fffccc387f967ec700af2174e387`), with the second call site pinned by `60349294`.

## Evidence

Verified by running, 2026-10-06, on the merged tip: about 3,000 real responses swept across seven arms and seven content classes at 9,300 to 10,500 compact bytes. No response carried two handles. No inline response exceeded 10,003 compact bytes. Largest inline sizes were exactly 10,003 for ASCII, quote and backslash payloads. Every `@tool_*` hint (`$.content`) worked when followed. Ten single mutations of the load-bearing guards were all killed, each run with a `test result:` line. Gate: fmt, clippy, lean (4,108 tests) and default (6,361 tests) all exit 0 on `40dc79cd`.

## Hypotheses tried

None needed; each instance was reproduced on the pre-fix bytes before its fix.

## Fix

See Fixed. The follow-up proposed to the owner is one shared helper that measures the real response size, replacing the per-tool measures that caused the recurrence.

## Tests added

Per-fix tests are named in each commit. Added for F1: `a_next_page_hint_on_an_err_query_names_the_err_stream_and_that_route_works` (follows the hint it is given) and `a_wide_line_remedy_on_an_err_query_names_the_err_stream` (killed a surviving mutation of `wide_line_remedy`).

## Still open

1. **Single-heading `read_markdown` success** (about 12 KB for 6,000 quotes), **a whole read below the summary threshold** (about 18 KB for 9,000 quotes), and **the heading-not-found list** (27.6 to 32.7 KB) are measured in raw bytes. Each still returns exactly one handle. The heading-not-found list was not re-probed by the final review.
2. **`memory(read)` decides on raw bytes** (`src/tools/memory/mod.rs`, `exceeds_inline_limit(&content)`). Content over the limit once serialized goes behind one `@tool_*`: from 9,969 raw bytes of ASCII, about 5,000 of quotes, about 1,650 of control characters. The hinted `$.content` works.
3. **`HEADING_ECHO_CLIP`** (`read_markdown.rs`) counts raw bytes. A heading echo can serialize to 1,200 B. The response limit still holds because `fit_envelope` measures it serialized.
4. **A `.err` buffer query returns the stored stderr twice**: as the query's stdout and again as the bounded `stderr` field. This shrinks the room left for stdout.
5. **Backstop and stderr edges**: the backstop spares only `bytes shown` markers, not `--- stderr TAIL:` or `--- N lines omitted ---`; its marker says "the tool's own buffer" for envelope-only fields; `summarize_stderr` clips an over-wide last line from its head; compaction-first applies only when raw bytes are 10,003 or fewer.

Not defects, recorded so nobody re-reads them as caps: interactive stdout is uncapped by design; the inline path returns pretty JSON but measures compact JSON, so 10,003 is not a cap on delivered bytes (about 9 bytes more for a small response); the progressive-disclosure guide's table still says 10,000 where the edge is 10,003.

## Workarounds

For a wide single line in a `@cmd_*` buffer, `grep -o 'TEXT.\{0,200\}' @cmd_X` or `cut -c1-4000 @cmd_X` read a window of it. `sed -n` and a bare `grep` return the whole line or nothing. `jq` reads a buffer that holds one JSON document.

## Resume

Decide whether to take the shared-helper follow-up before fixing items 1 to 3 one by one, since they share the cause. Promote the class to its own cluster if a third independent instance appears outside this sweep; three bugs now carry it (the two archived ones and this record).

## References

- `docs/adrs/2026-10-05-a-result-keeps-the-one-handle-its-tool-gave-it.md`
- `docs/issues/archive/2026-10-05-run-command-json-stdout-overflow-has-no-working-json-path-recovery.md`
- `docs/issues/archive/2026-10-05-run-command-test-envelope-failures-field-has-no-byte-bound.md`
