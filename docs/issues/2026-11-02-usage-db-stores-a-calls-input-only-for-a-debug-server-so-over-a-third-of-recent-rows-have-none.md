---
id: '2108589a1b5fab62'
kind: bug
status: mitigated
title: 'BUG: `usage.db` stores a call''s input and output only for a server started with `--debug`, so 36.7% of the rows since 2026-10-25 have no input, and every query that reads `input_json` silently sees a fraction'
owners:
- marius
tags:
- cluster/unclassified
- usage
closed: 2026-11-02
opened: 2026-11-02
owner: marius
severity: low
unverified: 'Fixed in code and tests only. No row from a server started without `--debug` on a build with the commit has been seen: the 30 rows seen on the rebuilt server (build `33ccee567`, `codescout_dirty` 1) carry both blobs, so that server runs with `--debug`. The dirty bit is consistent with the rebuild having included the then-uncommitted change; it does not prove it. The 36.7% empty is for the period before the change and stays true of it.'
---

# BUG: `usage.db` stores a call's input and output only for a server started with `--debug`, so 36.7% of the rows since 2026-10-25 have no input, and every query that reads `input_json` silently sees a fraction

## Summary

`UsageRecorder::write_content` (`src/usage/mod.rs`) sets `input_json` and `output_json` only when `self.debug` is true. `debug` is the `--debug` flag of `codescout start`. Both launch forms run on this machine now (`pgrep -a -f "[c]odescout start"` shows `start --debug` and plain `start` servers side by side), and each server writes to the same project database. In the codescout database 19,102 of 87,972 rows (21.7%) have an empty `input_json`, and 36.7% of the rows since 2026-10-25.

## Class fit

No class on the roster was walked. `cluster/unclassified` is the sanctioned escape. It resembles a recorder that omits a dimension for some callers and says nothing, which is the shape of `IC-21` and of the `selector-narrower-than-its-population` bugs.

## Symptom (Effect)

- A row with an empty `input_json` cannot say what was asked: no symbol, path, command or action. A query that groups by `json_extract(input_json, ...)` counts only the rows that have it, and returns a plausible number with no sign of the gap.
- The gap is per server process, so it is all-or-nothing per build and day: build `0eb6190ec` has 536 of 536 `grep` and `symbols` rows with no input (2026-10-30 to 2026-11-01), while `a6d175ee0` has 0 of 132 (2026-10-31 to 2026-11-01).
- A comment in `src/usage/mod.rs` (the doc of `extract_friction_target_ignores_shell_commands`) says "`input_json` is populated on ~99% of rows". That was measured on 2026-08-20. It is 63% since 2026-10-25.
- The `analyze-usage` skill's queries read `input_json` (query F, slow commands: `json_extract(input_json,'$.command')`) and do not say so.

## Reproduction

```
sqlite3 -readonly .codescout/usage.db "select count(*), sum(length(coalesce(input_json,''))=0), round(100.0*sum(length(coalesce(input_json,''))=0)/count(*),1) from tool_calls"
```

gives 87,972, 19,102 and 21.7 on 2026-11-02. Restricted to `called_at >= '2026-10-25'`: 36.7%. The per-build split:

```
sqlite3 -readonly .codescout/usage.db "select substr(codescout_sha,1,9), count(*), sum(length(coalesce(input_json,''))=0) from tool_calls where tool_name in ('grep','symbols') group by 1 order by 2 desc limit 10"
```

## Environment

codescout `experiments`, the codescout project database, 2026-11-02. Other databases were not queried.

## Root cause

`let input_json = if self.debug { serde_json::to_string(input).ok() } else { None };` and the same for `output_json` (`src/usage/mod.rs`, `write_content`). Columns that are extracted without the flag, so that they survive when it is off: the buffer linkage, `tool_use_id`, the deliveries, `observed_changed` and the attribution outcome. Why input and output are debug-only is not recorded there; size and the content of commands are likely reasons, but that is a reading, not something I found written.

## Evidence

The two queries above; the code above; the process list. Found while looking into `cd85dbae70c76670` (`call_graph` rows with no input): the empty input there is this, not a `call_graph` defect.

## Hypotheses tried

None.

## Fix

Decided by the operator on 2026-11-02: record `input_json` always. Commit `a72175ca5` on `experiments`: `write_content` (`src/usage/mod.rs`) stores `serde_json::to_string(input)` on every call. `output_json` stays debug-only, because it is the large half. Cost, measured on the rows since 2026-10-25 that have an input: 539 bytes on average, 59 KB at most, 11 MB over 20,809 rows. Rows written before the change stay without an input; a query over `input_json` should still print how many rows had none for any period before the rebuild.

## Tests added

- `record_content_records_input_in_normal_mode` (renamed from `record_content_no_input_in_normal_mode`): red before, `input_json` was `None`; it also pins `output_json` as `None` without debug.
- `record_content_records_tool_use_id_with_debug_off`: red before, it asserted a NULL input.
- `record_content_records_buffer_linkage_with_debug_off` guarded "debug is off" through `input_json`; it now guards it through `output_json`, the blob that held the emitted handle.
- Gate in a clean worktree: all 0, 8841 tests.

## Workarounds

Start servers with `--debug`, and report with every `input_json` figure how many rows had none.

## Resume

Code changed in `a72175ca5`; not yet shown live. The only rows seen on the rebuilt server (8 rows, build `33ccee567`) carry both blobs, so that server runs with `--debug` and proves nothing. Needed: a row from a server started WITHOUT `--debug` on a build that has the commit, with `input_json` present and `output_json` empty. Query: `select codescout_sha, count(*), sum(length(coalesce(input_json,''))>0), sum(length(coalesce(output_json,''))>0) from tool_calls where called_at >= '<time of the rebuild>' group by 1`. When such a row exists, mark `fixed`. The usage report of 2026-11-02 and the `analyze-usage` skill name the gap for the period before.

## References

- `docs/issues/2026-11-02-call-graph-fails-with-a-bare-disk-io-error-and-the-rows-record-no-input.md`
- `src/usage/mod.rs` (`write_content`)
