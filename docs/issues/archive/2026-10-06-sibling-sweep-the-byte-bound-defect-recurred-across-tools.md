---
id: '3c2bd2656becbf1d'
kind: bug
status: fixed
title: 'BUG: the size-measured-in-one-unit, returned-in-another defect recurred across tools; every known instance is fixed, what remains is a short list of unreached edges and a standing limit'
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
unverified: 'Not fixed, and not all reached by a probe through a real tool (section Still open): the interactive run_command arm is ungated; a huge custom dangerous pattern echoed in the pending-ack reason can still reach @tool_*; buffer_truncated is shed, not bounded; JobState::Failed text is unclipped; the Text render of a memory read is never measured; the memory path-clip step is reached only by a direct test; the reserved _cut_fields key would strip a tool''s own field of that name; a real file path in next longer than about 4 KB; a jobs race in run_command::call. The per-fix mutation counts in Evidence were reported by the fixing agents; the first memory ones were also run by the owner session. The libtest compaction boundary is a standing design limit. Gate green on 5a7a54ec, lean re-run on the landed tip 2ee8514c; nothing is pushed by this session.'
---

# BUG: the size-measured-in-one-unit, returned-in-another defect recurred across tools; every known instance is fixed, what remains is a short list of unreached edges and a standing limit

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

Third round, the residue this record listed as not fixed (all measured through the real tools before and after):

- The `carries_elision_marker` false positive is fixed by provenance, not by better text matching. A summarizer that cuts a field records its name under the reserved envelope key `_cut_fields`; `clip_prebuffered_envelope` reads that record, spares only the fields it names, and strips it on every path, so no response carries it and no program can forge it. The text matcher and its tests are deleted. Before: 14 of 24 envelopes holding program-printed marker lines were spared and stayed at 16,798 to 16,892 bytes; after: all 24 clip to 8,460 to 8,472 bytes. Commits `b7422986`, `e33e91ad`.
- `read_with_line_range` no longer keeps `coverage` unconditionally: a forced markdown range on a file with 600 unread sections returned 35,645 bytes with two handles; the largest response is now 10,003 bytes. One shared helper, `page_beside_coverage`, serves this arm and the markdown range arm: `442a47d4`.
- The `path`, `json_path` and `breadcrumb` echoes are bounded in escaped bytes by a new cap, `INPUT_ECHO_CLIP` (300; probe row `read_file.input_echo_bytes`): a 6 KB YAML key had produced 12,287 bytes with two handles: `bcb02dd8`, `60e26d68`.
- A heading echo quoted inside another string now costs at most its 200-byte clip as delivered (it cost 396 bytes for `"`). The multi-heading oversized error, found on the way, echoed every heading whole: 72,413 bytes for three 12 KB headings, now 1,568 to 1,589: `352587c0`, `25561143`, `e659e4c1`.

Fourth round, from a survey of every site that adds a key after a gate decided (22 sites: 9 counted, 13 harmless overshoot, 3 dangerous; `docs/adrs/2026-10-07-one-measure-of-the-delivered-response.md`):

- `memory` section reads: the `file_id` fallback arm was never measured and was about 87 bytes wider than the inline candidate it replaced, so a missing-section name of 9,950 bytes gave a 10,092-byte arm with two handles (653 breaches across seven classes and both layouts). The arm is now built with every key and measured, the `missing` echo is clipped (300 escaped bytes a name, at most 40 kept, the rest counted in `missing_names_omitted`), `missing` is dropped and counted if the arm still does not fit, and `extra` is applied once before the measure. The Text render also hid `missing` from the caller entirely; it now shows `sections not found: [...]`: `bc696175`, `cea82b1c`, `8706dabb`.
- `run_command`: `jobs` carried every background job's full command with no bound, and the pending-ack, timeout and background shapes had no gate, so three 4 KB jobs gave 12,592 to 12,896 bytes under `@tool_*` and the `@ack_*` handle (or the `timed_out` status) was visible only inside that buffer (493,027 bytes with 20 `\x01` jobs). `jobs` lists at most 8 jobs, each command clipped to 300 escaped bytes, with a count and a route that shows an omitted job; every shape is measured in `RunCommand::call` and sheds envelope keys in a fixed order (`buffer_truncated`, `jobs`, `timeout_hint`) when still over, never its own status key. The largest delivered response in the sweeps is 3,771 bytes: `d3e293cc`, `4d8d7680`.

## Evidence

Gate on the final tip `bd945569`: fmt, clippy, lean (4,146 tests) and default (6,399 tests) all exit 0; 84 `test result: ok` lines, none FAILED. The second Opus review of `a0930592` ran about 38,500 probes through the real `call_content` across every tool and arm; no fixed arm broke the one-handle rule or the 10,003-byte limit, and its 14 mutations were all killed. It also found the four gaps fixed in the second round above.

The fixing agents report per-fix mutation runs, each applied after committing, one at a time, restored with `git checkout`, and counted as inconclusive when no `test result:` line printed: 15 killed for `read_markdown` (first round) and 10 for the second; 14 for `run_command` (13 killed, one inconclusive and redone, one survivor killed by added tests) plus `M7` and `M11` killed later; 17 killed and 4 equivalent for the `read_file` buffer and real-file arms. The equivalent survivors are the raw pre-checks kept in `read_file.rs`, proven so by `json_escaped_len(s) >= s.len()`. `M6` (`or_insert` to `insert` in `run_command::call`) survived and is equivalent except for a race between a background job changing state and the response being built. The `memory` mutations were also run by the owner session: 3 killed, then 2 killed.

Correction to this record's first version: the whole read below the summary threshold (the `read_markdown` default tiers) did NOT reproduce. At `2d1e4052` it already measured the serialized candidate, and its sweep passed before any change; it is kept as a guard. The heading-not-found list and the single-heading success were real and are fixed.

Third round: gate on `5b9e844f` (the two branches merged onto `experiments`): fmt, clippy, lean (4,154 tests) and default (6,411 tests) all exit 0, 84 `ok` result lines, none FAILED. `experiments` then gained only docs; the lean step was re-run on the landed tip `4f2f2e3e` (40 `ok`, none FAILED). The fixing agents report 13 mutations killed for the `read_file` and `read_markdown` residuals and 21 of 21 killed for the provenance fix. Three of those survived at first (`fit_summary` counting the record in two places, and `record_cut` appending duplicates); each got a committed killing test and was re-run and killed. A test-first unit test of the false positive failed on the old code with the byte counts above. Measured clean without a production change: a YAML key read, a `source: "lib:<name>"` response (largest 10,003 bytes), and `heading=` on a non-markdown real file (refused with a 181-byte error). Each has a guard test that passes on the old code.

Fourth round: the full gate passed on `5a7a54ec` (the two branches merged onto `experiments`): fmt, clippy, lean (4,180 tests) and default (6,438 tests) all exit 0, 86 `ok` result lines, none FAILED; `experiments` then gained only a shell test, and the lean step was re-run on the landed tip `2ee8514c` (41 `ok`, none FAILED). The `memory` agent reports 7 mutations killed (one, `extra` applied after the measure in the `file_id` arm only, survived every real-tool test until a new one was written and committed) and the `run_command` agent reports 9 killed; the per-fix counts were reported by the agents, not re-run by the owner session. The survey's figures are its own measurements through `call_content`; sites 1 to 5 and 7 rest on existing tests passing.

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
- Third round: `b7422986` patch-id `c665484f254197870799e3eb83a437801a6aa651`, `e33e91ad` patch-id `9dcefd208df65d823a8857fb5332fee5773f3c02` (cut-record provenance); `442a47d4` patch-id `3a5a3bb283dddc4ddbdfba13f7213da8f01ac97e`, `bcb02dd8` patch-id `b1edf93e868dcb457ae20a89671888faddcace5b`, `352587c0` patch-id `5fef3c60ccd531c029457728a6c711f0edf24470`, `e659e4c1` patch-id `0c5ab2447191199ec24ed9c5418062e923805170`, `25561143` patch-id `27174c4adb486727da69582d6e2c1faf9035dc29`, `60e26d68` patch-id `0a4bb22db262bfd6b211631644a10b1c36fcf887` (`read_file` and `read_markdown` residuals)
- Fourth round: `bc696175` patch-id `1d7d7346aa482d3c1b3255635bc0ef134ed614f8`, `cea82b1c` patch-id `235df86c36227d1da1f9dfba6d30948186ede5e8`, `8706dabb` patch-id `43986f51bc5c46af355753b677ea6a0c581e76da` (`memory`); `d3e293cc` patch-id `3d10fb43b1ad9e0c0f8ae2fc3ff91edadfbb42d6`, `4d8d7680` patch-id `1bc4a7b28f86b1e8b3c88f9ff264121fcfc7dca3` (`run_command`)

## Tests added

Per-fix tests are named in each commit. Added for F1: `a_next_page_hint_on_an_err_query_names_the_err_stream_and_that_route_works` (follows the hint it is given) and `a_wide_line_remedy_on_an_err_query_names_the_err_stream` (killed a surviving mutation of `wide_line_remedy`).

## Still open

What remains is small, and nothing here was reached by a probe through a real tool:

1. **The reserved key.** `_cut_fields` is reserved in every tool's top-level result; a tool field of that name would be stripped by the backstop. No tool uses the name (grepped when it was chosen). A new tool must not.
2. **A real file path in `next` is quoted whole,** on purpose: the route has to work, and the widest skeleton counts it (a path is at most about 4 KB). A path longer than that is not handled.
3. **The `@tool_` `json_path` arm stores its handle under the name `{path}:{jp}`** through `store_file`. The name is never echoed, so it costs nothing today.
4. **`file_summary::bound_summary`** builds a `file_id` envelope and does not use the cut record; the backstop acts only on `output_id` envelopes, so it never sees one.
5. **The `jobs` race.** `M6` (`or_insert` to `insert` in `run_command::call`) is equivalent except for a background job changing state between the two builds of the response; testing it needs timing control.

6. **The interactive arm of `run_command` is not gated.** Its `stdout` is the whole accumulated output, so a large run goes to `@tool_*` (one handle). It also never gets the envelope keys, although the `envelope_keys` doc says "every response shape"; the doc and the code disagree.
7. **A huge custom dangerous pattern** from config is echoed in the pending-ack `reason`, which is never shed, so that response can still go to `@tool_*`.
8. **`buffer_truncated` has no count cap.** It carries one notice of about 265 bytes per truncated buffer named, up to the buffer capacity of 50. It is shed when the response is over the limit, not bounded.
9. **`JobState::Failed` text is not clipped** (it shows an OS error string), so it is bounded only in practice.
10. **The Text render of a `memory` read is never measured, only the JSON the gate judged.** The new `sections not found` note can make it a few bytes larger. The existing shadow warning has a bigger skew of the same kind.
11. **The path-clip step in `memory`** is reachable only by a direct test, because no fixture can root a project at a path that wide.

A behaviour change to know: a section whose own content does not fit inline once serialized now returns the oversized-section error with a `file_id`, where before it came back as a success buffered under `@tool_*`. Both cost one extra call.

Standing design limits, decided 2026-10-06 with the owner (not work): compaction of a libtest run is tried only when the raw output is 10,003 bytes or fewer, pinned by `compaction_is_considered_up_to_exactly_the_inline_limit_in_raw_bytes`, because removing the limit would send up to about 10 KB of compacted text for large passing runs; interactive stdout is uncapped by design; the inline path returns pretty JSON but measures compact JSON, so 10,003 is not a cap on delivered bytes; the progressive-disclosure guide's table says 10,000 where the edge is 10,003.

## Workarounds

For a wide single line in a `@cmd_*` buffer, `grep -o 'TEXT.\{0,200\}' @cmd_X` or `cut -c1-4000 @cmd_X` read a window of it. `sed -n` and a bare `grep` return the whole line or nothing. `jq` reads a buffer that holds one JSON document.

## Resume

The recurrence came from per-tool size measures, not from one bad line. Every review of this work found new siblings, so the sweep is not a proof that none remain. The structural follow-up is Phase A of `docs/adrs/2026-10-07-one-measure-of-the-delivered-response.md`, which landed on 2026-10-07: one module measures the delivered response, the text predicate is private to `core`, and `tests/inline_gates.rs` fails an unannotated length-form gate. It closes the unit root (a raw body measured, an escaped response returned). It does NOT close the late-key root: a key added after a gate decided is still each tool's job, and only Phases B (a backstop for `file_id` envelopes) and C (one carrier for late keys) address it. Both are undecided and recorded in the ADR. Promote the class to its own cluster: three bug records now carry it.

## References

- `docs/adrs/2026-10-05-a-result-keeps-the-one-handle-its-tool-gave-it.md`
- `docs/issues/archive/2026-10-05-run-command-json-stdout-overflow-has-no-working-json-path-recovery.md`
- `docs/issues/archive/2026-10-05-run-command-test-envelope-failures-field-has-no-byte-bound.md`
- `docs/adrs/2026-10-08-a-read-keeps-the-handle-it-reads-and-one-real-file-has-one-handle.md` (the follow-on: a read of a buffer keeps its handle; one real file has one handle)
