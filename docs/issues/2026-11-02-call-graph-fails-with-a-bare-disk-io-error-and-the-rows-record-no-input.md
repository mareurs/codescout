---
id: cd85dbae70c76670
kind: bug
status: open
title: 'BUG: `call_graph` failed 3 times with a bare "disk I/O error" (2 of 57 calls in the 2026-10-25 window), and the rows record neither the input nor the output'
tags:
- cluster/unclassified
opened: 2026-11-02
owner: marius
severity: low
unverified: Three rows in one database. The cause, the failing store and the inputs are unknown; the 'local store, not the language server' reading rests on two latencies of 2 and 3 ms only.
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

Unknown, under investigation. Not checked: which store `call_graph` reads (a SQLite cache, the catalog, or the language server's), whether another process held or removed that file at 07:45:44 on 2026-10-28, and whether the same error occurs in other projects' databases.

## Evidence

The query above. The rows do not record the call's input, so the symbol and path are lost.

## Hypotheses tried

None.

## Fix

Not done.

## Tests added

N/A. Nothing is fixed.

## Workarounds

Retry the call.

## Resume

Open, low severity, `unverified`. Next step: find where `call_graph` can return this message in `src/`, and ask the recorder why `input_json` is empty on these rows.

## References

- `docs/usage-reports/2026-11-02-usage-analysis.md`
