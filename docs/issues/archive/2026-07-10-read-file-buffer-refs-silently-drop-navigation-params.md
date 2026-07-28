---
id: null
kind: bug
status: fixed
title: null
owners: []
tags:
- read_file
- buffers
- progressive-disclosure
- json_path
- toml_key
- silent-failure
topic: null
time_scope: null
closed: '2026-07-13'
opened: '2026-07-10'
owner: marius
related: []
severity: high
---

# BUG: read_file buffer refs silently drop navigation params — toml_key ignored on all refs, json_path ignored on @file_/@cmd_

## Summary
`read_from_buffer` (`src/tools/read_file.rs:175-313`) never reads `toml_key` at all, and handles `json_path` only under `if path.starts_with("@tool_")` (:199). Passing `toml_key` on any buffer ref, or `json_path` on `@file_`/`@cmd_` refs, silently falls through to the plain line-range/paginated text return — no error, no warning, output indistinguishable from a successful navigation-less read.

## Symptom (Effect)
Live-verified (subagent B1, this session): `read_file("@tool_xxx", toml_key="content")` returned a paginated dump of the whole buffer, ignoring the param; `read_file("@file_yyy", json_path="$.services")` returned the full file from line 1. The agent has zero signal its navigation request was dropped.

## Reproduction
1. Read a large JSON file with plain `read_file(path)` so it buffers as `@file_*`.
2. `read_file("@file_<id>", json_path="$.<key>")` → raw paginated text, `json_path` ignored.
3. Any `read_file("@<anything>", toml_key="k")` → same silent fall-through.

## Environment
codescout MCP server, branch `experiments`, 2026-07-10.

## Root cause
- `toml_key` appears only in `call()`'s disk-file branch (`src/tools/read_file.rs:120`) and in `validate_read_nav_params` (:367) — the buffer branch (`call()` :68 → `read_from_buffer`) returns early and never consults it.
- `json_path` in the buffer path is gated on `@tool_*` (:199) with the comment "json_path navigation is only meaningful for @tool_* (always JSON)" — false for `@file_*` refs holding buffered JSON files. `read_full_file` is exactly what buffers a large JSON file as `@file_*`, so `json_path` is unreachable precisely in the large-file scenario it exists for (subagent C1's framing).
- `validate_read_nav_params` (which would at least reject bad combos) is only invoked on the disk path, so nothing errors either.

## Evidence
- Code read directly this session: `read_from_buffer` :189-231 (only `@tool_` json_path branch), no `toml_key` reference in the function; `call()` :67-70 early buffer return.
- Live runs by subagent B1 (recon arm) — two verified silent no-ops, outputs captured in its report.
- Independently found by all three D1 agents (A1 #3, B1 #3, C1 #1/#2) in the 2026-07-10 3×3 bug-hunt experiment.

## Hypotheses tried
1. **Hypothesis:** validation rejects the combination before the silent path. **Test:** trace `validate_read_nav_params` call sites. **Verdict:** rejected — disk path only.

## Fix

**Shipped on `experiments` in `3af52f1e`** (`fix(read_file): error on unsupported nav params for buffer refs`). Archive after cherry-pick to `master`.

`read_from_buffer` (`src/tools/read_file.rs`) now guards early: `toml_key` on any buffer ref, or `json_path` on a `@cmd_`/`@file_` ref, returns a `RecoverableError` with a hint (slice with start_line/end_line, or grep the ref) instead of silently falling through to a line-range/full read. `json_path` on `@tool_` refs is unchanged.
## Tests added

`read_file_toml_key_on_buffer_ref_errors_not_silently_ignored` and `read_file_json_path_on_non_tool_buffer_ref_errors` (`src/tools/read_file.rs` tests). Both RED before the guard (the call returned the buffer content), GREEN after. The existing `read_file_buffer_json_path_array_element_returns_value` (json_path on a `@tool_` ref) still passes.
## Workarounds
`grep(pattern, @ref)` or `read_file(@ref, start_line/end_line)`; for `@tool_*` refs json_path works as documented.

## Resume
Implement (b) in `read_from_buffer` (reject with hint before the pagination fall-through), then evaluate (a) for `@file_*` JSON buffers; add both regression tests.

## References
- `get_guide("progressive-disclosure")` — the buffer contract this violates.
- Experiment provenance: session 5efbda5f, agents A1/B1/C1 (triple independent discovery; B1 live-verified).
