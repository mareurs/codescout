---
id: cd85dbae70c76670
kind: bug
status: mitigated
title: 'BUG: `call_graph` failed 3 times with a bare "disk I/O error" (2 of 57 calls in the 2026-10-25 window), and the rows record neither the input nor the output'
tags:
- cluster/unclassified
closed: 2026-11-02
opened: 2026-11-02
owner: marius
severity: low
unverified: The cause of the disk I/O error was not found and no occurrence has been seen since 2026-10-28. The lookup and upsert sites are covered by the message builder's test, not by a failing store. CI has not been read.
---

# BUG: `call_graph` failed 3 times with a bare "disk I/O error" (2 of 57 calls in the 2026-10-25 window), and the rows record neither the input nor the output

## Summary

Three `call_graph` rows in the codescout usage database have `error_msg = 'disk I/O error'` and nothing else: no hint, no path, and empty `input_json` and `output_json`. Two are from 2026-10-28 07:45:44, 0.4 s apart, with 3 ms and 2 ms latency. One is from 2026-10-14. That is the only tool with this message in the database. Cause not known.

## Class fit

No class on the roster is shown by three rows of one bare message. `cluster/unclassified` is the sanctioned escape; the roster was not walked class by class.

## Symptom (Effect)

A `call_graph` call returns an error whose text is only the SQLite-style message "disk I/O error". It names no file, and the caller cannot tell whether to retry. Both failures in the window ended in under 5 ms, which is too fast for a language-server round trip, so the error came from a local store, not from the language server. That is a reading of the latency, not shown.

## Reproduction

Not reproduced. Query:

```
sqlite3 -readonly .codescout/usage.db "SELECT called_at, substr(codescout_sha,1,9), latency_ms FROM tool_calls WHERE tool_name='call_graph' AND error_msg LIKE 'disk I/O error%' ORDER BY called_at;"
```

gives 2026-10-14 12:50:37 (build `223b83aa`, 1,419 ms), 2026-10-28 07:45:44.186 and 07:45:44.581 (build `f9b0fc9f8`, 3 ms and 2 ms).

## Environment

codescout `experiments`; the codescout project database. The window has 57 `call_graph` calls, 55 of them successful. The other databases were not queried.

## Root cause

Two separate things, one of them found.

**The rows record no input: found, and not a `call_graph` defect.** `UsageRecorder::write_content` (`src/usage/mod.rs`) stores `input_json` and `output_json` only when the server was started with `--debug`. Both launch forms run on this machine, and 36.7% of the rows since 2026-10-25 have no input in any tool. Filed as `docs/issues/archive/2026-11-02-usage-db-stores-a-calls-input-only-for-a-debug-server-so-over-a-third-of-recent-rows-have-none.md` (`20d94ebc7deec094`); fixed in `a72175ca5`: a call's input is now recorded without `--debug` (rows written before that stay without one).

**The "disk I/O error" itself: not found.** The error text is SQLite's own. `call_graph` has three places that pass a `rusqlite` error up with no context: the open of `.codescout/call_edges.db` (`open_db`: mkdir, SQLite open, PRAGMA and DDL), the lookup in `one_hop`, and the upsert in `one_hop`. A 2-3 ms failure is too fast for a language-server round trip, which fits the local store, but the rows do not say which of the three it was. Not checked: whether another process held or removed the file at 07:45:44 on 2026-10-28, and whether other projects' databases show the same error.

## Evidence

The query above. The rows do not record the call's input, so the symbol and path are lost.

## Hypotheses tried

None.

## Fix

Done on `experiments` at `83af8a574`, for the bare message only. In `src/tools/symbol/call_graph/mod.rs`, `cache_error(step, root, e)` builds "call_graph: <step> the call-edge cache at <root>/.codescout/call_edges.db failed: <cause>", and it is used at the three sites above (`open_cache` says "opening", the lookup "reading", the upsert "writing to"). The cause of the error is not addressed, so the next occurrence will say which file and which step, nothing more. It stays a hard error: a failing store is not an input mistake the caller can correct.


## Fix provenance

- **SHA:** `83af8a5744e617f367bc8bf9b446339f1811c9d7` (`experiments`)
- **patch-id:** `2dc6d3c2a33b2703fedffd0da0bd0043c72412ba`

## Tests added

In `src/tools/symbol/call_graph/mod.rs`: `a_cache_error_names_the_store_and_the_step` and `opening_the_cache_in_a_broken_project_names_the_store`. Red before the change, on the assertions (stubs kept the bare error), then green. Gate in a clean worktree: FMT, CLIPPY, LEAN and DEFAULT all 0, 8,838 tests; `cargo test --lib tools::symbol::` 469 passed. Not tested: the lookup and upsert sites through a real failing store (only the message builder and the open are). CI has not been read.

## Workarounds

Retry the call.

## Resume

Mitigated on `experiments` (`83af8a574`): the next "disk I/O error" names the file and the step. The cause is still unknown and no new occurrence has been seen since 2026-10-28. If one appears, read its message first: it now says whether it was the open, the read or the write.

## References

- `docs/usage-reports/2026-11-02-usage-analysis.md`
