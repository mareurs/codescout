---
id: d29988a6e87331f7
kind: bug
status: fixed
title: grep and read_file refuse a live @bg_ handle with a false "expired" or "file not found" message
tags:
- cluster/doc-contradicted-by-code
closed: 2026-10-27
fix_patch_id: b62d05e32628ba8aa2a0b1c908cc156bb18d34fb
fix_sha: a8373725a (experiments)
opened: 2026-10-26
owner: marius
severity: medium
unverified: 'CLEARED 2026-10-27. Was: the gate was not re-run after one whitespace-only edit. The final gate on the committed tree exited 0: python 1369, lean 5288, default 7947 passed.'
---

# BUG: grep and read_file refuse a live `@bg_*` handle with a false "expired" or "file not found" message

## Summary

`grep(path="@bg_…")` answers `buffer reference not found … Buffer refs expire when the session resets`, and `read_file(path="@bg_…")` answers `file not found … (searched <root>/@bg_…)`. The job's log is alive: `run_command("tail -3 @bg_…")` returns it. `get_guide("progressive-disclosure")` says all buffer handles are "addressable by any tool that accepts a path".

## Class fit

The guide states a contract (every handle works in any path-accepting tool) that the code contradicts for one handle kind. That is the doc-contradicted-by-code trait. It differs from the class's other members because the wrong text is an error message that blames a cause (expiry, a missing file) that is not the cause.

## Symptom (Effect)

An agent that starts `run_command(..., run_in_background=true)` and later greps the handle gets an error that says the buffer expired. The agent re-runs the work or switches to `run_command "grep PATTERN @bg_x"`, which works.

## Reproduction

1. `run_command("echo a; echo b", run_in_background=true)` returned `@bg_0000008f` (2026-10-26, this session).
2. `grep(pattern="probe", path="@bg_0000008f")` returned `buffer reference not found: '@bg_0000008f'` with the hint `Buffer refs expire when the session resets…`.
3. `read_file(path="@bg_0000008f")` returned `file not found: '@bg_0000008f' (searched /home/marius/work/claude/codescout/@bg_0000008f)`.
4. `run_command("tail -3 @bg_0000008f")` returned both lines and `"state": "exited 0"`.

## Environment

codescout `experiments` at `8df90f1b9`, 2026-10-26.

## Root cause

Unknown — under investigation for the exact line. The friction deep-dive subagent reported that `grep.rs:1031` and `read_file.rs:307` call `output_buffer.get_stream`, which resolves pool entries only, while background logs live in `background_jobs` and are reached through `get_background_log` (`output_buffer.rs:~1731`), used only by the `run_command` interpolation path. The controller did not read those lines. Steps 2 to 4 above are reproduced directly.

## Evidence

- `usage.db` 2026-09-27..2026-10-26: 13 `grep` calls with `path=@bg_*` and 9 `read_file` calls on `@bg_*` handles, none succeeded (subagent query over the rows that kept `input_json`; `input_json` is NULL unless debug is on).
- The `read_file` calls are classified `path_not_found`; the `grep` calls `buffer_ref_expired`. So the two error families both carry this cause and neither names it.
- Gap from emit to use was 2 to 25 minutes with the same `codescout_sha` (subagent figure, unverified), so neither pool eviction nor a restart explains it.

## Hypotheses tried

The `@bg_` handle is looked up in the wrong store by `grep` and `read_file`. Confirmed in behaviour by the reproduction; the source lines are the subagent's.

## Fix

Fixed in `a8373725a` (experiments), patch-id `b62d05e32628ba8aa2a0b1c908cc156bb18d34fb`.

`OutputBuffer::get_stream_or_job_log` (`src/tools/output_buffer.rs`) reads a `@bg_*` log fresh from disk. `grep` and `read_file` call it. A log over `JOB_LOG_READ_LIMIT` (16 MiB, chosen without measurement) is refused with a route to `run_command("tail -50 @bg_x")`. Checked on the rebuilt binary on 2026-10-27: `grep` and `read_file` on a running job's handle returned its first line.

## Tests added

Six tests in `src/tools/read_file_job_log_tests.rs`. All six failed before the fix and passed after it.

## Workarounds

`run_command("grep PATTERN @bg_x")` or `run_command("tail -N @bg_x")`.

## Resume

Fixed. Found by the 2026-10 friction deep dive (`docs/trackers/friction-deep-dive-2026-10.md`, group G6). Archive after the final gate passes.


## Fix provenance

- **SHA:** `a8373725a` (`experiments`)
- **patch-id:** `b62d05e32628ba8aa2a0b1c908cc156bb18d34fb`

## References

- `docs/trackers/friction-deep-dive-2026-10.md`
- `get_guide("progressive-disclosure")`, § *The @ref buffer*
