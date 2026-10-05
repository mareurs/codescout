---
kind: bug
status: open
tags:
- cluster/hint-composed-without-the-request
- run_command
- progressive-disclosure
- json_path
closed: null
opened: 2026-10-05
owner: marius
related:
- docs/issues/archive/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md
- docs/issues/archive/2026-08-28-tool-buffer-grep-returns-envelope-not-stdout.md
- docs/issues/archive/2026-09-07-the-json-path-key-hint-caps-at-ten-keys-and-marks-no-cut.md
severity: high
unverified: "The mechanism is read in src/tools/core/types.rs, not stepped through; the symptom and every workaround were run. No fix attempted. The cluster/ tag is my judgment of the fit (the IC-22 **Members:** line was added in the same commit). The frequency in usage.db was not measured."
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

Inferred from the source below — not stepped through; the outputs above are consistent with it.

1. The `@tool_X` buffer holds the whole `run_command` result as JSON: an object `{"type", "exit_code", "output_id", "stdout"}` where `stdout` is one **string**. (Measured: the buffer's root is an object, `$.exit_code` is `0`, `$.stdout[*]` reports "found string".) The JSON the command printed is therefore text inside a string, and `json_path` addresses JSON values only.
2. `default_json_path_hint` (`src/tools/core/types.rs:947-955`) derives the hint from the payload's shape: the largest array reachable through object keys, as `path[*]`, else the constant `"$.field"`. The payload has no array, so the constant is returned. `scalar_shaped_payload_keeps_the_generic_placeholder` (`types.rs:1813-1818`) pins that fallback for scalar payloads on purpose.
3. `run_command` does not override `json_path_hint`. Measured by `grep "fn json_path_hint"`: the overrides are `librarian/adapter.rs:481`, `memory/mod.rs:1257`, `symbol/symbols.rs:405` and `read_file.rs:237`; the default is `types.rs:1286`.
4. `read_file` refuses `json_path` on any ref that does not start with `@tool_` (`src/tools/read_file.rs:295-300`). The `@cmd_X` handle is the one that holds the raw JSON text, so it is the handle that would work, and it is refused.
5. The shape-mismatch error (`src/tools/file_summary/file_summary.rs:651-656`) names the type it found and not its keys, so the caller cannot learn that the data sits under `stdout`.

The doc comment on `default_json_path_hint` states the standard this misses: *"A hint that cannot work for the result it is attached to is worse than no hint: it converts a lookup into a failed call."* The earlier fix (archived `2026-08-15`) made the hint right for results that hold a real array. A JSON document inside a string is not covered.

`src/usage/db.rs:692-696` already classifies both refusals above as `json_path_wrong_buffer_kind` and `json_path_shape_mismatch`, so `usage.db` should hold how often agents hit this. Not queried here.

## Evidence

See Symptom and Reproduction. The workarounds below were also run, which shows the data is intact and only the route is missing.

## Hypotheses tried

1. **Hypothesis:** `json_path="$.stdout"` is the route (the archived `2026-08-28` bug names it as the working escape). **Test:** ran it. **Verdict:** rejected for JSON output. It returns the string, but the string is one line as wide as the whole document, and reading it back is cut at the inline budget (last line of the Symptom block).
2. **Hypothesis:** `@cmd_X` accepts `json_path` for a single JSON document. **Test:** ran it. **Verdict:** rejected: "only supported on @tool_* refs".

## Fix

Not started. Options, none tried:

- **(a) Fix the hint.** When a `run_command` result's `stdout` parses as one JSON document, name the shell route in the envelope instead of `$.field`, for example `run_command("jq '.[] | .field' @cmd_X")`. Smallest change, and it works today (see Workarounds).
- **(b) Let `json_path` read a `@cmd_*` buffer whose content is one JSON document.** Then `read_file("@cmd_X", json_path="$[*].name")` works, and the documented `[*]` route in the progressive-disclosure guide holds for CLI output. Larger: the refusal at `read_file.rs:295-300` is deliberate, and parsing a large buffer on each call needs a size decision.
- **(c) Make the shape-mismatch error list the keys of the object it found** (`file_summary.rs:651-656`), as the key-miss hint already does (archived `2026-09-07`), so `stdout` is discoverable.

Recommendation, not measured: (a) now, because the harness already promises a route and the fix is one branch in the hint; (b) as the real fix.

## Tests added

N/A: filing only.

## Workarounds

All run 2026-10-05; each returned one line per job (the first form was run with four fields, not two):

```
run_command("glab api …/jobs | jq -r '.[] | \"\\(.name)\\t\\(.status)\"'")        # filter in the same command
run_command("jq -r '.[] | \"\\(.name) \\(.status)\"' @cmd_0b8d6874")              # on the raw handle
run_command("jq -r '.stdout | fromjson | .[] | \"\\(.name) \\(.status)\"' @tool_0b8d6875")   # on the envelope
```

## Resume

Open `src/tools/core/types.rs:947`, add a test next to `scalar_shaped_payload_keeps_the_generic_placeholder` that feeds a `run_command` result whose `stdout` is a JSON array, and decide between options (a) and (b). Re-run the Reproduction first on the current binary.

## References

- `src/tools/core/types.rs:947-955`, `:1286`, `:1813-1818`
- `src/tools/read_file.rs:237`, `:295-300`
- `src/tools/file_summary/file_summary.rs:651-656`
- `src/usage/db.rs:692-696`
- `docs/issues/archive/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md`
- `docs/issues/archive/2026-08-28-tool-buffer-grep-returns-envelope-not-stdout.md`
- `docs/issues/archive/2026-09-07-the-json-path-key-hint-caps-at-ten-keys-and-marks-no-cut.md`
