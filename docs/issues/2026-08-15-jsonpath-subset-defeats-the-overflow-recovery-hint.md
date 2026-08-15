---
id: '875e5d03d980ceac'
kind: bug
status: open
title: 'BUG: the overflow hint''s own recommended recovery needs `[*]`, which json_path rejects — agents fall back to shelling out, 11% of handle recoveries fail'
owners:
- marius
tags:
- progressive-disclosure
- json_path
- buffer-handles
- agent-guidance
- usage-db-evidence
topic: progressive-disclosure
---

## Summary

A result too large to inline is replaced by a handle plus a hint that says to read it with
`read_file("@tool_xyz", json_path="$.field")`. But the overflowing results are overwhelmingly
**arrays** (`artifact(find)` → `items[]`, `symbols` → `symbols[]`), and projecting a field out of an
array needs `[*]` — which the json_path parser rejects. The recommended recovery path routes
straight into the unsupported case.

The parameter is named `json_path` and takes `$`-prefixed expressions, so it advertises JSONPath
and implements a proper subset. Agents reach for the most common JSONPath idiom there is, get an
error, and improvise: the single most frequent recovery after an `artifact` overflow is
`run_command` (shelling out to grep/jq), **ahead of** the `read_file` the hint recommends.

## Symptom (Effect)

The rejection, verbatim:

    unsupported json_path segment '[*]' — hint: Supported forms: '.key', '["key"]' / '['key']'
    (quoted key — use for keys containing '.'), '[N]' (non-negative integer), '[-N]' (negative
    integer), '[-N:]' (last N elements). Other slice/filter forms not supported.

A second class, when the handle's underlying content is not JSON:

    json_path parameter is only supported for JSON files — hint: For Markdown files use
    read_markdown, for TOML/YAML use toml_key

## Reproduction

Live, this session (2026-08-15), unmodified:

1. `artifact(action="find", kind="bug", filter={...}, limit=40)` overflowed → returned
   `@tool_f99fdcd8` with the hint *"read_file(\"@tool_f99fdcd8\", json_path=\"$.field\") to extract
   a specific field"*.
2. Following that hint to get the titles: `read_file("@tool_f99fdcd8", json_path="$.artifacts[*].title")`
   → **rejected**, `unsupported json_path segment '[*]'`.
3. Recovery was to abandon the tool entirely: `run_command("grep -o '\"title\": \"[^\"]*\"' @tool_f99fdcd8")`.

Note step 2 is not a creative misuse — it is the literal application of the hint printed in step 1
to a result whose payload is an array.

## Environment

codescout on `experiments` @ `16cae241`. Evidence drawn from 13 `.codescout/usage.db` files across
every project on this machine (~54,000 recorded tool calls), read 2026-08-15.

## Root cause

Two independent mechanisms that compose into the observed behaviour.

**1. The grammar is a proper subset of JSONPath, under a name that promises JSONPath.**
`parse_bracket` (`src/tools/file_summary/file_summary.rs:570-626`) accepts exactly: a quoted key
(`["key"]` / `['key']`), a non-negative index (`[N]`), a negative index (`[-N]`), and a negative
tail slice (`[-N:]`). Everything else falls through to the error at
`src/tools/file_summary/file_summary.rs:623-626`. So wildcard, forward slice, and filter — three of
JSONPath's defining features — are absent, while the parameter name and the `$.` prefix signal the
full language.

**2. The overflow hint recommends a single-value extraction for a payload that is a list.**
The compact envelope's `hint` and `get_guide("progressive-disclosure")` both model recovery as
`read_file("@tool_xyz", json_path="$.foo")`. That works for a scalar field. The results that
actually overflow are lists of records, where the useful projection is "this field from every
element" — the `[*]` the parser rejects. The guidance and the parser disagree about what the common
case is.

Measured 2026-08-15 across all 13 usage.db files (query in Evidence): rejected segments break down
as **`[*]` 22 (73%)**, forward slices (`[1:3]`, `[0:3]`, `[1:]`, `[45:56]`, `[55:63]`) 5, filters
(`[?(@.id=='SD-3')]`, `[?(@.name=='run')]`) 2. Plus 6 rejections of the second class
(`json_path` applied to non-JSON content — typically a `@file_*` handle from a markdown read).

## Evidence

### The grammar agents actually reach for

Across every project's usage.db, the rejected-segment histogram:

    22  '[*]'
     2  '[1:3]'
     1  '[?(@.name=='run')]'
     1  '[?(@.id=='SD-3')]'
     1  '[55:63]'
     1  '[45:56]'
     1  '[1:]'
     1  '[0:3]'

`[*]` alone is 73% of all rejections. This is not a long tail of exotic syntax — it is one idiom,
reached for repeatedly, by different sessions across different projects.

### Recovery after a handle is returned — scattered, and shell wins

Sequenced with a window function over `tool_calls` partitioned by `session_id` (codescout project),
counting what immediately follows an overflowed `artifact` / `read_markdown` / `read_file` call:

    36  artifact(overflow)      -> run_command:success
    22  read_file(overflow)     -> read_file:success
    20  artifact(overflow)      -> read_file:success
    14  artifact(overflow)      -> artifact:success
    12  artifact(overflow)      -> read_file:ERROR
     8  read_markdown(overflow) -> read_file:success
     6  artifact(overflow)      -> grep:success

Six different recovery strategies. The most common single one is `run_command` — leaving the tool
surface for the shell — and `read_file`, the recommended path, produced 12 errors.

### Failure rate

    17 failed of 152 recoveries

**11% of all recoveries from an overflow handle fail on the first attempt** (codescout project,
`artifact` / `read_markdown` / `read_file` overflows). Each failure is a wasted round trip on a path
the tool itself recommended.

### Retry-after-rejection sometimes fails again

Sequencing the two calls after a `json_path` rejection shows recovery is not reliably one step:
`read_file:success -> read_file:ERROR` (2 occurrences, codescout) and `read_file:ERROR` as the
immediate next call 5 times in `code-explorer.old`. The agent is guessing at the grammar.

## Hypotheses tried

1. **Hypothesis:** this is the already-fixed dotted-key bug resurfacing
   (`docs/issues/archive/2026-07-01-read-file-jsonpath-dotted-object-keys-unreachable.md`).
   **Test:** read `parse_bracket`; check whether quoted keys are accepted.
   **Verdict:** rejected — quoted keys ARE accepted now; that fix landed and holds. This is the
   sibling gap in the same parser (wildcard/slice/filter), not a regression.

2. **Hypothesis:** it is the already-fixed "buffer refs silently drop navigation params" bug
   (`docs/issues/archive/2026-07-10-read-file-buffer-refs-silently-drop-navigation-params.md`).
   **Test:** check whether `json_path` reaches the parser at all for a `@tool_*` ref.
   **Verdict:** rejected — it reaches the parser and is evaluated; that fix landed. The failure is
   the grammar, not a dropped parameter.

3. **Hypothesis:** agents are misusing the parameter and the guidance is fine.
   **Test:** trace one live case end to end (Reproduction above) and check whether the input was a
   literal application of the printed hint.
   **Verdict:** confirmed as a guidance defect — it was. The hint's shape does not match the
   payload's shape.

## Fix

Not implemented. Three candidate levels, not mutually exclusive:

1. **Support `[*]`** in `parse_bracket` as a projection segment (map the remaining path over every
   element). This alone addresses 73% of observed rejections and makes the printed hint true for
   list payloads. Forward slices (`[a:b]`) are a smaller, mechanical follow-on; filter expressions
   (`[?(...)]`) are a much larger surface and should probably stay unsupported.
2. **Make the hint match the payload.** The envelope knows whether the buffered result's top level
   is an array; when it is, the emitted hint should show the list-shaped projection rather than
   `json_path="$.field"`. A hint that cannot work for the result it is attached to is worse than no
   hint — it converts a lookup into a failed call.
3. **Say what is supported at the point of offer, not only on rejection.** The full grammar is
   currently discoverable only by getting it wrong; `get_guide("progressive-disclosure")` documents
   the handle families but not the addressing grammar.

Whatever is chosen, `get_guide("progressive-disclosure")` and the envelope hint must be updated
together — they are the two surfaces that taught the wrong shape.

## Tests added

None yet — filed on discovery.

## Workarounds

- Project a field from a list with `run_command` against the handle:
  `grep -o '"title": "[^"]*"' @tool_xyz` — this is what agents already do, 36 times in one project.
- Or address elements one at a time: `json_path="$.items[0].title"`, `[1]`, … — correct but O(n)
  calls.
- `[-N:]` (last N elements) IS supported and is easy to miss; it is the one list-shaped form that
  works today.

## Resume

Add a `Segment::Wildcard` arm to `parse_bracket` (`src/tools/file_summary/file_summary.rs:570-626`)
and make evaluation map the remaining segments over each element, returning an array. Then re-run
the histogram query in Evidence against a fresh usage.db window and confirm `[*]` rejections drop to
zero — the 73% figure is the acceptance test. Separately, decide on fix (2): find where the overflow
envelope's `hint` is built and make the list case emit a list-shaped example.

## References

- `src/tools/file_summary/file_summary.rs:570-626` — `parse_bracket`, the grammar
- `docs/issues/archive/2026-07-01-read-file-jsonpath-dotted-object-keys-unreachable.md` — fixed ancestor (quoted keys)
- `docs/issues/archive/2026-07-10-read-file-buffer-refs-silently-drop-navigation-params.md` — fixed ancestor (params dropped on refs)
- `get_guide("progressive-disclosure")` — documents handle families, not the addressing grammar
- Evidence source: 13 `.codescout/usage.db` files, ~54,000 tool calls, read 2026-08-15

