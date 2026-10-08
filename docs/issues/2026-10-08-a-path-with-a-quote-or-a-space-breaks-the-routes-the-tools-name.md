---
id: '60a6160e9b229994'
kind: bug
status: open
title: 'BUG: a path with a quote or a space breaks the routes read_file and run_command name'
tags:
- cluster/unclassified
- read_file
- progressive-disclosure
opened: 2026-10-08
owner: marius
related:
- docs/adrs/2026-10-08-a-read-keeps-the-handle-it-reads-and-one-real-file-has-one-handle.md
severity: low
---

# BUG: a path with a quote or a space breaks the routes read_file and run_command name

## Summary

The routes in `next` and in the cut-line hints put the file path into the text raw. The key or JSON path in the
same route is JSON-escaped. A path containing `"` gives a `read_file("…")` call that does not parse. The
`grep -o` hint puts the path into a shell command unquoted, so a path with a space splits into two arguments.

## Symptom (Effect)

- A file named `a"b\c.json`: `next` is `read_file("/tmp/…/a"b\c.json", json_path="$.k", …)`.
- The same unescaped form already existed in the real-file range `next`, for example
  `read_file("q"uote.json", start_line=2, end_line=5000)`.
- A file in `sp ace/w.json` with a 30 KB value: the hint `run_command("grep -o 'PATTERN[^,]*' /tmp/…/sp ace/w.json")`
  fails with `exit_code 2`: `grep: /tmp/…/sp: No such file or directory`.

## Reproduction

Reproduced by the implementer and the final reviewer in tests through the real tools on 2026-10-08. The
`line_hint` of the buffer arm did the same before the value-paging change. Not reproduced on the live binary.

## Environment

codescout on `experiments`. Linux.

## Root cause

The route builders write `{path}` with `format!` and no escaping or quoting.

## Evidence

The three outputs above.

## Hypotheses tried

None.

## Fix

Not started. JSON-escape the path in `read_file(…)` routes, as the key already is, and shell-quote it in
`run_command(…)` routes.

## Tests added

N/A — not fixed.

## Workarounds

Copy the route by hand and quote the path.

## Resume

`read_with_line_range`, `value_of_a_file`, `line_hint` and the clamp hint in `src/tools/read_file.rs`.
Execute each route in a test with a path that has a space and a quote.

## References

- `docs/adrs/2026-10-08-a-read-keeps-the-handle-it-reads-and-one-real-file-has-one-handle.md`
