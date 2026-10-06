---
id: 3c2bd2656becbf1d
kind: bug
status: fixed
title: 'BUG: the size-measured-in-one-unit, returned-in-another defect recurred across tools; every reached instance is fixed, the residue is latent or unmeasured'
tags:
- cluster/unclassified
- run_command
- read_file
- read_markdown
- progressive-disclosure
closed: 2026-10-06
opened: 2026-10-06
owner: marius
related:
- docs/issues/archive/2026-10-05-run-command-json-stdout-overflow-has-no-working-json-path-recovery.md
- docs/issues/archive/2026-10-05-run-command-test-envelope-failures-field-has-no-byte-bound.md
- docs/adrs/2026-10-05-a-result-keeps-the-one-handle-its-tool-gave-it.md
severity: medium
unverified: 'The residue in section Still open is NOT fixed: a latent false positive in carries_elision_marker (reproduced in a unit probe, unreachable through run_command), a coverage-unbounded skeleton in read_with_line_range (read, not run), unlimited echoes of the caller''s own path and json_path, and four unmeasured paths (YAML key read, source lib:<name>, an overlong json_path, the heading= arm of read_file on a non-markdown real file). The per-fix mutation counts in Evidence were reported by the fixing agents; the memory ones were also run by the owner session. Gate green on bd945569; nothing is pushed.'
---

# BUG: the size-measured-in-one-unit, returned-in-another defect recurred across tools; every reached instance is fixed, the residue is latent or unmeasured

## Summary

After the two `run_command` byte-bound bugs were fixed (`e833abeb`, `fac7abce`), four forked reviewers swept the sibling tools. They found the same defect again and again. A size is measured in one unit (raw bytes, pretty text, a summary alone, or a response before later keys are added). A different thing is returned: the compact serialized whole response, which the inline limit counts (`exceeds_inline_limit`: more than 10,003 bytes). JSON escaping makes the gap large: `\x01` and `\x1b` serialize to 6 bytes, `"` and `\` to 2. The sweep fixed the instances below. A final independent review of the merged tip found one more (F1). A second round of fixes and a second Opus review found further siblings in the same tools, each verified by running it. All are fixed (section Fixed). What remains is listed under Still open: nothing there was reached by a probe through a real tool, except where it says so.

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

Second round, from the five gaps this record listed as open and from the reviews of that work:

- `memory(read)` decides inline-or-buffered on the serialized response, provenance keys (`resolved_from`, `write_target`) included: `093376e7`, `1a7730ea`.
- `run_command`: a `.err` buffer query carries the stored stderr once: `c1743949`. One detector for every summarizer marker, backstop markers that name where each cut field lives, and a stderr last line that keeps its end: `d74a3f10`. `buffer_truncated`, `jobs` and `timeout_hint` are counted by the gate: `d1fd9624`. The `↻ refreshed from disk` line is priced, and delivered on summarized test runs: `66512179`, `c756986a`.
- `read_markdown`: single-heading success, the heading-not-found list, the line range and `HEADING_ECHO_CLIP` (now in escaped bytes, with headings quoted as JSON strings in `next_actions`): `3f4877fa`. The buffered line range pages against its widest skeleton, clamps a wide line, and the section route reaches the end of the section: `247510cd`, `5737635c`.
- `read_file`: `read_from_buffer` (line range, whole buffer, `json_path`), the over-wide-line clamp and `buffer_page_room`: `84b1a028`, `38bec0bf`. Real-file ranges, whole reads and the `buffer_truncated` notice: `ddf52cb5`. `json_path` and `toml_key` reads of a real file are gated, and `next` keeps `force=true`: `eaf787f1`.
- A peer's dispatch test relied on the defect (a 6000-quote file forced the outer `@tool_*` envelope); it now pins what `read_file` does: `a6953592`. The outer-envelope corrections address stays pinned by `correction_reaches_the_caller_on_the_buffered_path`.

## Evidence

Gate on the final tip `bd945569`: fmt, clippy, lean (4,146 tests) and default (6,399 tests) all exit 0; 84 `test result: ok` lines, none FAILED. The second Opus review of `a0930592` ran about 38,500 probes through the real `call_content` across every tool and arm; no fixed arm broke the one-handle rule or the 10,003-byte limit, and its 14 mutations were all killed. It also found the four gaps fixed in the second round above.

The fixing agents report per-fix mutation runs, each applied after committing, one at a time, restored with `git checkout`, and counted as inconclusive when no `test result:` line printed: 15 killed for `read_markdown` (first round) and 10 for the second; 14 for `run_command` (13 killed, one inconclusive and redone, one survivor killed by added tests) plus `M7` and `M11` killed later; 17 killed and 4 equivalent for the `read_file` buffer and real-file arms. The equivalent survivors are the raw pre-checks kept in `read_file.rs`, proven so by `json_escaped_len(s) >= s.len()`. `M6` (`or_insert` to `insert` in `run_command::call`) survived and is equivalent except for a race between a background job changing state and the response being built. The `memory` mutations were also run by the owner session: 3 killed, then 2 killed.

Correction to this record's first version: the whole read below the summary threshold (the `read_markdown` default tiers) did NOT reproduce. At `2d1e4052` it already measured the serialized candidate, and its sweep passed before any change; it is kept as a guard. The heading-not-found list and the single-heading success were real and are fixed.

## Hypotheses tried

None needed; each instance was reproduced on the pre-fix bytes before its fix.

## Fix

See Fixed. The follow-up proposed to the owner is one shared helper that measures the real response size, replacing the per-tool measures that caused the recurrence.

## Fix provenance

All on `experiments`. This is a sweep of many commits; the primary anchor is the last one that closed a reviewed finding, and every other commit is listed below it with its own patch-id (`git show <sha> | git patch-id --stable`).

- **SHA:** `eaf787f1` (`experiments`)
- **patch-id:** `7c2916bc4801533bdee753aeb4d1218c86e12559`

Other commits:

- `8bd5f6f0` patch-id `6061abd9adfe69e9ed4e0665416e2595a2da25b4` (buffer-query stored stderr by bytes)
- `d2ef26e9` patch-id `0959f9dd1c43a953bfe233de9da65ce6a4e0f893` (`read_file` whole-file summary)
- `58cebaf7` patch-id `8178c82c14f0deac1fc62256489dd7d06edf901e` (buffer query from measured size)
- `fd0bb946` patch-id `7c95d244584fe77c2fdc4639ac6a881422879a7b` (backstop never re-cuts a marked field)
- `e0edaeb1` patch-id `beeeff16aa84f50b038ff628fe3dbbb839bba3c6` (libtest judged by its compacted response)
- `e08e1a6c` patch-id `c32ae80c4993fffccc387f967ec700af2174e387` (`.err` hint names the `.err` stream)
- `093376e7` patch-id `437a57d4ad4d3ca707beebb2ded932b411a8cd27` and `1a7730ea` patch-id `bf890b1644c6dc474f67d8fe28f330b20e405486` (`memory`)
- `c1743949` patch-id `375988597fafbdb7e04e321fd15d591c040f26c2`, `d74a3f10` patch-id `4d829c1a67d159895c9f7334e0b3862339c4dd39`, `d1fd9624` patch-id `84ea31adc94d14f90b9820d8b50110602b5f9693`, `c756986a` patch-id `99ebb3104d260162bb51736c9d7ec18752d671eb` (`run_command`)
- `3f4877fa` patch-id `f76f96ccb65f23b07e8affc5b5bfbdbf37329d48`, `247510cd` patch-id `93c4a7f21263b12ffa16d225004bbdbc41bb050e` (`read_markdown`)
- `84b1a028` patch-id `92ae5d0a7e256fa83a3a3082d8d67096c0e69c1b`, `ddf52cb5` patch-id `399ffc69b0b8eb67fafbc0a220a577cb3bd550c4`, `eaf787f1` patch-id `7c2916bc4801533bdee753aeb4d1218c86e12559` (`read_file`)
- `a6953592` patch-id `311116e21e35d6eed1ccf48670cbe766c794903b` (peer dispatch test)

## Tests added

Per-fix tests are named in each commit. Added for F1: `a_next_page_hint_on_an_err_query_names_the_err_stream_and_that_route_works` (follows the hint it is given) and `a_wide_line_remedy_on_an_err_query_names_the_err_stream` (killed a surviving mutation of `wide_line_remedy`).

## Still open

Not fixed, with what is and is not established:

1. **`carries_elision_marker` false positive.** Reproduced in a unit probe: a program-printed whole line `--- 5 lines omitted ---` left a 16,068-byte envelope uncut in `clip_prebuffered_envelope`, where the same envelope with other text clipped to 8,909 bytes. It is NOT reachable through `run_command`: `fit_summary` fits that envelope first, so the backstop never sees it oversized; 294 runs pin that (`a_program_printing_the_summarizer_markers_keeps_one_handle`). A sound fix carries per-field cut provenance from every marker writer to the backstop (see `docs/conventions/parsers-over-a-namespace.md`). Do it if a tool starts passing program text into an `output_id` envelope without fitting it first.
2. **`read_with_line_range` puts `coverage` in its skeleton unconditionally.** A `force=true` range on a markdown file whose `coverage` alone exceeds the limit could still mint `@tool_*`. Read, not run.
3. **The `path`, `json_path` and `breadcrumb` echoes in the new `file_id` arms have no length limit.** They are the caller's own input, the same as in the existing `@tool_` `json_path` arm.
4. **A heading echo quoted inside another string can reach about twice its 200-byte clip** (about 400 bytes for `"`). Bounded.
5. **Unmeasured:** a YAML key read (shares the toml path), a `source: "lib:<name>"` response, a `json_path` long enough to overflow the `file_id` arm, the `heading=` arm of `read_file` on a non-markdown real file, and the `jobs` race above.

A behaviour change to know: a section whose own content does not fit inline once serialized now returns the oversized-section error with a `file_id`, where before it came back as a success buffered under `@tool_*`. Both cost one extra call.

Standing design limits, decided 2026-10-06 with the owner (not work): compaction of a libtest run is tried only when the raw output is 10,003 bytes or fewer, pinned by `compaction_is_considered_up_to_exactly_the_inline_limit_in_raw_bytes`, because removing the limit would send up to about 10 KB of compacted text for large passing runs; interactive stdout is uncapped by design; the inline path returns pretty JSON but measures compact JSON, so 10,003 is not a cap on delivered bytes; the progressive-disclosure guide's table says 10,000 where the edge is 10,003.

## Workarounds

For a wide single line in a `@cmd_*` buffer, `grep -o 'TEXT.\{0,200\}' @cmd_X` or `cut -c1-4000 @cmd_X` read a window of it. `sed -n` and a bare `grep` return the whole line or nothing. `jq` reads a buffer that holds one JSON document.

## Resume

The recurrence came from per-tool size measures, not from one bad line. Every review of this work found new siblings, so the sweep is not a proof that none remain. The structural follow-up is one helper that measures the real serialized response and is used by every tool that returns its own handle, plus a source-level check that forbids a raw-body `exceeds_inline_limit` beside a serialized return. Take it up before the next tool adds a gate of its own. Promote the class to its own cluster: three bug records now carry it.

## References

- `docs/adrs/2026-10-05-a-result-keeps-the-one-handle-its-tool-gave-it.md`
- `docs/issues/archive/2026-10-05-run-command-json-stdout-overflow-has-no-working-json-path-recovery.md`
- `docs/issues/archive/2026-10-05-run-command-test-envelope-failures-field-has-no-byte-bound.md`
