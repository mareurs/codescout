---
id: '2c48dccab72c0509'
kind: bug
status: open
title: edit_file with an unparseable `edits` string is reported as "missing 'old_string' parameter"
tags:
- cluster/accepted-parameter-silently-dropped
opened: 2026-10-26
owner: marius
severity: medium
---

# BUG: `edit_file` with an `edits` string that is not valid JSON is reported as "missing 'old_string' parameter"

## Summary

When `edits` arrives as a JSON string that does not parse, `optional_array_param` returns `None`. `edit_file` then treats the call as a single edit and fails with `missing 'old_string' parameter`. The message never mentions `edits`, so the agent cannot tell its batch was dropped for a parse error.

## Class fit

A parameter the caller supplied is silently ignored, and a different, unrelated error is reported. That is the accepted-parameter-silently-dropped trait. It differs from the class's other members because the parameter is dropped only on a parse failure and the later error names a different parameter.

## Symptom (Effect)

The agent sends a large `edits` payload, gets `missing 'old_string' parameter`, and resends the same edits as a native array on the next call. One large payload is wasted per occurrence.

## Reproduction

Source path (read 2026-10-26): `optional_array_param` (`src/tools/core/params.rs:306`) returns `None` for a string that does not parse to an array. `EditFile::call` (`src/tools/edit_file/mod.rs:558`) takes `edits.is_none()` as the single-edit path, which requires `old_string` (`require_str_param_or_hint`, `params.rs:155`: `missing '{}' parameter`). A call `{path, edits: "[{\"new_string\">x"}` has no valid JSON, so it ends in `missing 'old_string' parameter`. The call itself was not executed in this session, to avoid a write.

## Environment

codescout `experiments` at `8df90f1b9`.

## Root cause

The coercion helper reports "not coercible" and "absent" the same way (`None`), and its caller cannot tell them apart.

## Evidence

`usage.db`, `tool_name='edit_file'`, `err_family='missing_required_param'`, message mentions `old_string`, 2026-09-27..2026-10-26: 41 rows in 20 sessions. In 25 of them `edits` is a string beginning `[{"new_string">` (the `":` after `new_string` is lost, so the JSON is invalid); in 9 `edits` is absent. Heads seen: `[{"new_string">                return Ok(interactive_respons`, `[{"new_string">    done < <(git log -n 300 …`. Peak 2026-10-16..18. The cause of the garble is client- or model-side and is not established.

## Hypotheses tried

None on why the string is garbled; the server cannot fix that.

## Fix

Open. When `edits` is present but not coercible to an array, return an error that names `edits` and the parse error position, instead of falling through.

## Tests added

None. A test would send `{path, edits: "[{\"new_string\">x"}` and assert the error names `edits` and not `old_string`.

## Workarounds

Resend `edits` as a native JSON array.

## Resume

Found by the 2026-10 friction deep dive (`docs/trackers/friction-deep-dive-2026-10.md`, group G5). Fix not started.

## References

- `docs/adrs/2026-07-10-repair-and-continue-input-handling.md`
- `src/tools/core/params.rs`, `src/tools/edit_file/mod.rs`
