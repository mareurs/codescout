---
kind: bug
status: fixed
tags:
- cluster/hint-composed-without-the-request
- run_command
- progressive-disclosure
- json_path
closed: 2026-10-05
opened: 2026-10-05
owner: marius
related:
- docs/issues/archive/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md
- docs/issues/archive/2026-08-28-tool-buffer-grep-returns-envelope-not-stdout.md
- docs/issues/archive/2026-09-07-the-json-path-key-hint-caps-at-ten-keys-and-marks-no-cut.md
severity: high
unverified: The fix is UNCOMMITTED in the working tree, so there is no fix SHA or patch-id and the file is not archived. The live MCP binary that ran the probes predates the fix; the Reproduction was not re-run on a rebuilt binary (the reach test covers the same path through call_content). Root cause items 1 to 5 (hint, json_path on @cmd_*, shape-mismatch error) are unchanged and unfixed by design. The frequency in usage.db was not measured. The cluster/ tag was the filer's judgment; the root cause is now closer to IC-13 (cf. the archived 2026-09-14 stderr bug) than to IC-22, and the owner should decide.
---

# BUG: when a `run_command` prints one big JSON document, the overflow recovery cannot work — the hint is the placeholder `$.field`, `json_path` cannot enter `stdout`, and `@cmd_*` refuses `json_path`

## Summary

A `run_command` whose stdout is one JSON document (`glab api`, `gh api`, `kubectl -o json`, `docker inspect`, `curl`) and is over the inline budget returns an overflow envelope. The envelope advertises `read_file("@tool_X", json_path="$.field")`. No `read_file` route can project a field of that JSON. The caller finds this out by failing calls. The only route that works is shell-side `jq`.

## Symptom (Effect)

Measured 2026-10-05, in a `lang-pal-engine` session. The command was `glab api projects/807/pipelines/32057/jobs --paginate` (one JSON array of 9 job records, 32,458 buffered bytes). The goal was to list `name` and `status` of every job.

```
run_command("glab api projects/807/pipelines/32057/jobs --paginate")
→ {"output_id": "@tool_0b8d6875",
   "summary": "✓ exit 0  (query @cmd_0b8d6874)",
   "hint": "read_file(\"@tool_0b8d6875\", json_path=\"$.field\") to extract a specific field, or read_file(\"@tool_0b8d6875\", start_line=N, end_line=M) to browse sections",
   "buffered_bytes": 32458}

read_file("@tool_0b8d6875", json_path="$[*].name")
→ json_path '[*]' needs an array, found object
  hint: Use '[*]' only where the value is an array. Drop it to address the value itself.

read_file("@tool_0b8d6875", json_path="$.stdout[*].name")
→ json_path '[*]' needs an array, found string

read_file("@cmd_0b8d6874", json_path="$[*].name")
→ json_path is only supported on @tool_* refs, not '@cmd_0b8d6874'
  hint: @cmd_*/@file_* buffers are raw text. Slice with start_line/end_line, or grep the ref.

read_file("@tool_0b8d6875", json_path="$.stdout")
→ Extracted value at $.stdout (1 lines), buffer @file_0b8f345f

read_file("@file_0b8f345f", start_line=1, end_line=1)
→ the array, cut mid-record, ending "…[truncated: this line is wider than the inline budget]"
```

The `summary` line carries no content and no shape: `✓ exit 0  (query @cmd_0b8d6874)`.

## Reproduction

No `glab` needed. Measured 2026-10-05 at `b3d8a484` (branch `experiments`, source read there; the running binary's build is not recorded):

```
run_command("python3 -c \"import json; print(json.dumps([{'name': 'n%d' % i, 'status': 'created'} for i in range(2000)]))\"")
→ output_id @tool_0b907a6a, buffered_bytes 94964, hint …json_path="$.field"…

read_file("@tool_0b907a6a", json_path="$[*].name")
→ json_path '[*]' needs an array, found object
```

## Environment

Linux, codescout MCP server used from Claude Code on 2026-10-05, project `lang-pal-engine` (Python workspace). Source read at `b3d8a484` on `experiments`.

## Root cause

Verified 2026-10-05 by reading the source and by live runs on the pre-fix binary. The first four items are the chain the issue was filed with. Item 0 is the cause that makes the chain run, and the original filing missed it.

0. **`summarize_generic` bounded its output by line count only** (`src/tools/command_summary.rs`, `HEAD_LINES` 20 + `TAIL_LINES` 10). A line has no length bound. One 95 KB line of compact JSON is "1 line", so it came back **verbatim** in the envelope's `stdout`, and the "summary" was the whole output. The envelope then exceeded `TOOL_OUTPUT_BUFFER_THRESHOLD`, so `call_content` buffered it a second time under `@tool_*`. Three probes on the live binary gave the same content-free envelope: one 95 KB line, 50 KB on stderr, and five 20 KB lines. So the defect is not specific to JSON: any output of 30 lines or fewer and over about 10 KB takes this path. The same defect had been fixed for the `stderr` field of the `test` and `build` envelopes (`STDERR_SUMMARY_BYTE_BUDGET`, archived `2026-09-14`) and not for this shape.
1. The `@tool_X` buffer holds the whole `run_command` result as JSON: an object `{"type", "exit_code", "output_id", "stdout"}` where `stdout` is one **string**. (Measured: the buffer's root is an object, `$.exit_code` is `0`, `$.stdout[*]` reports "found string".) The JSON the command printed is therefore text inside a string, and `json_path` addresses JSON values only.
2. `default_json_path_hint` (`src/tools/core/types.rs`) derives the hint from the payload's shape: the largest array reachable through object keys, as `path[*]`, else the constant `"$.field"`. The payload has no array, so the constant is returned. `scalar_shaped_payload_keeps_the_generic_placeholder` pins that fallback for scalar payloads on purpose.
3. `run_command` does not override `json_path_hint`. The overrides are in `librarian/adapter.rs`, `memory/mod.rs`, `symbol/symbols.rs` and `read_file.rs`.
4. `read_file` refuses `json_path` on any ref that does not start with `@tool_` (`read_from_buffer`, `src/tools/read_file.rs`). The `@cmd_X` handle is the one that holds the raw JSON text, so it is the handle that would work, and it is refused.
5. The shape-mismatch error (`eval_segments`, `src/tools/file_summary/file_summary.rs`) names the type it found and not its keys, so the caller cannot learn that the data sits under `stdout`.

Items 1 to 5 stay true and stay unfixed. They only matter because of item 0: with a bounded summary, the `run_command` response is the small `@cmd_*` envelope, and no `@tool_*` handle or `$.field` hint is produced for this class of output. The recovery route for a `@cmd_*` handle is `jq`, `grep` or `sed` through `run_command`, and that route already worked (see Workarounds).

The doc comment on `default_json_path_hint` states the standard the old output missed: *"A hint that cannot work for the result it is attached to is worse than no hint: it converts a lookup into a failed call."*

`src/usage/db.rs` already classifies both refusals above as `json_path_wrong_buffer_kind` and `json_path_shape_mismatch`, so `usage.db` should hold how often agents hit this. Not queried.

## Evidence

See Symptom and Reproduction. The workarounds below were also run, which shows the data is intact and only the route is missing.

## Hypotheses tried

1. **Hypothesis:** `json_path="$.stdout"` is the route (the archived `2026-08-28` bug names it as the working escape). **Test:** ran it. **Verdict:** rejected for JSON output. It returns the string, but the string is one line as wide as the whole document, and reading it back is cut at the inline budget (last line of the Symptom block).
2. **Hypothesis:** `@cmd_X` accepts `json_path` for a single JSON document. **Test:** ran it. **Verdict:** rejected: "only supported on @tool_* refs".
3. **Hypothesis:** the missing hint is the cause, so the fix belongs in `json_path_hint` (the original option (a)). **Test:** traced where the `@tool_*` envelope comes from. **Verdict:** rejected as the root. The hint is wrong, but only because the response was re-buffered, and it was re-buffered because `summarize_generic` returned a 95 KB line verbatim. A hint override would have left a content-free envelope and moved the caller to a different failed route.

## Fix

Fixed 2026-10-05, uncommitted (see `unverified`). The fix is at the cause (Root cause item 0), not at the hint.

`summarize_generic` (`src/tools/command_summary.rs`) now bounds each stream by bytes after its line summary:

- `bound_stream_bytes` keeps the first and last half of `GENERIC_FIELD_BYTE_BUDGET` (2,000 bytes) and puts a marker between them: `--- stdout: <shown> of <total> bytes shown; all of it: output_id ---`. For stderr the handle is `output_id.err`.
- `<total>` is the length of the stream as the command wrote it, not of the text after line elision.
- A stream within the budget is returned unchanged, so no short output changes.
- Cuts fall on `char` boundaries (`clip_to_bytes`, and the new `clip_tail_to_bytes`).

The response for the Reproduction is now the small `@cmd_*` envelope with the document's head and tail inline. No `@tool_*` handle and no `$.field` hint is produced. The `@cmd_*` handle holds the whole stream, and `jq`, `grep` and `sed` through `run_command` read it (Workarounds).

**Options (a), (b) and (c) from the filing were not implemented.** They repair the recovery route for a `@tool_*` handle, which this class of output no longer reaches. They stay open as separate work if another tool produces a scalar-shaped overflow. Not measured: whether any tool other than `run_command` does.

**Same defect, other fields.** The `failures` field of the `test` envelope and the `first_error` field of the `build` envelope had no byte bound either. They are fixed in the same change with `FAILURE_FIELD_BYTE_BUDGET` (5,000 bytes), tracked in `2026-10-05-run-command-test-envelope-failures-field-has-no-byte-bound.md`.

## Tests added

All in `src/tools/command_summary.rs` unless stated. Each was red before the fix. The reach tests reproduced the bug itself (a `@tool_*` envelope of 94,964 bytes in the live probe).

- `summarize_generic_bounds_one_enormous_stdout_line_by_bytes`: one 72 KB JSON line; checks both ends, `shown`, `total` and that the envelope stays under the inline limit.
- `summarize_generic_bounds_a_few_wide_lines_by_bytes`: five 20 KB lines, so only the byte bound can bind.
- `summarize_generic_reports_the_original_size_after_line_elision_too`: `total` names the original stream after the line summary has run.
- `summarize_generic_bounds_a_huge_stderr_and_leaves_a_short_stdout_alone`: the stderr site on its own.
- `summarize_generic_cuts_on_char_boundaries`: `€` fixture, with a trailing `\n\n` so the tail cut lands inside a character.
- `summarize_generic_byte_bound_is_inclusive_at_the_budget`: exactly the budget is verbatim, one byte over is elided.
- `a_huge_stdout_line_is_summarized_inline_not_rebuffered` (`src/tools/run_command/tests.rs`): a 95,000-byte line through `RunCommand.call_content`. It asserts a `@cmd_*` handle, no `buffered_bytes`, the marker, and that the handle holds all 95,000 bytes. This is the REACH test: only it shows that `handle_successful_output` routes the generic arm through the bound.

**Cap registration.** `command_summary.generic_field_bytes` is a new `RESULT_CAP` id, with a probe row in `src/tools/core/cap_probe.rs` citing `a_huge_stdout_line_is_summarized_inline_not_rebuffered`. `result_caps` passes (71 tests).

**Mutation run, 2026-10-05, on the final bytes, in a private worktree.** 14 mutations, one at a time, 14 killed:

- M1 and M2 delete the stdout and stderr bounds. M2 is killed only by the stderr test, so the two sites are covered separately.
- M3 drops the tail, M4 drops the head, M9 uses the whole budget as the half.
- M5 turns `<=` into `<`, M6 miscounts `shown`, M7 reports the elided length as `total`.
- M8 removes the tail's char-boundary search.
- M10 removes the marker and leaves the truncation standing.
- M11 to M14 are the `failures` and `first_error` sites, in the sibling bug.

**Two gaps the mutation run found, both closed.** M8 survived the first run: my `€` fixture was `3N+1` bytes, so the tail cut was always on a boundary and the test never reached the search it names. M14 survived: no test sat between the 2,000-byte and 5,000-byte budgets for `first_error`. Both were fixed with a test change and re-run.

**Gate.** `./scripts/gate.sh` 2026-10-05: `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`. Its own list of what it does not cover applies.

## Workarounds

All run 2026-10-05; each returned one line per job (the first form was run with four fields, not two):

```
run_command("glab api …/jobs | jq -r '.[] | \"\\(.name)\\t\\(.status)\"'")        # filter in the same command
run_command("jq -r '.[] | \"\\(.name) \\(.status)\"' @cmd_0b8d6874")              # on the raw handle
run_command("jq -r '.stdout | fromjson | .[] | \"\\(.name) \\(.status)\"' @tool_0b8d6875")   # on the envelope
```

## Resume

Commit the fix (it is in the working tree on `experiments`, with the two issue files). Then record the fix SHA labelled `experiments` and its patch-id (`git show <sha> > x.patch && git patch-id --stable < x.patch`), and archive both bug files with `doc(action="move")`. Re-run the Reproduction on the first binary built from that commit: the live MCP binary used for the probes predates the fix.

## References

- `src/tools/core/types.rs:947-955`, `:1286`, `:1813-1818`
- `src/tools/read_file.rs:237`, `:295-300`
- `src/tools/file_summary/file_summary.rs:651-656`
- `src/usage/db.rs:692-696`
- `docs/issues/archive/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md`
- `docs/issues/archive/2026-08-28-tool-buffer-grep-returns-envelope-not-stdout.md`
- `docs/issues/archive/2026-09-07-the-json-path-key-hint-caps-at-ten-keys-and-marks-no-cut.md`
