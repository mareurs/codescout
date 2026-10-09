---
id: 0d796d9cbcd2d7bd
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

The route builders wrote a file's path into the text with `format!("…{path}…")` and no escaping or
quoting, in a place where the text is a call a caller copies and runs. Two shapes carry a path:

- `read_file("<path>", json_path="…", start_line=N, end_line=M)`: the path sits inside a JSON string, so a
  `"` or `\` in it ends the string early or starts an escape. The key beside it was already a JSON
  literal (`ValueContinuation::new`), the path was not.
- `run_command("grep -o 'PATTERN[^,]*' <path>")`: the path sits inside a shell command that is itself inside
  a JSON string, so it needs shell quoting first (a space splits the argument, `$`, a backtick and `'` run
  or break it) and JSON escaping after.

Nothing shared existed to do either, so each builder wrote its own raw `{path}`. A buffer ref
(`@cmd_`, `@file_`, `@tool_` plus 8 hex digits) is a plain word, so every builder that is only ever handed a
ref was right by construction, and the defect only showed for a real file's path.

## Evidence

The three outputs above, and the route sweep of 2026-10-09 at `918b3d51` (every place that builds a
`read_file(…)` or `run_command(…)` route; each read in the code, not taken from a grep hit):

Built from a real file's path or a caller-given target, CHANGED:

| Where | Route shape | Path comes from |
|---|---|---|
| `src/tools/read_file.rs:346` `real_file_route` (in `read_from_buffer`) | `read_file("<source>", json_path\|toml_key="…")` | the `source_path` of a `@file_*` handle |
| `src/tools/read_file.rs:777` `ValueContinuation::new` / `route` | `read_file("<path>", <nav>="…", start_line=N, end_line=M)` | the path the caller gave (real file), or a `@tool_*` ref |
| `src/tools/read_file.rs:1499` `read_with_line_range`, two uses (the `widest` skeleton `next` and the result `next`) | `read_file("<path>", start_line=N, end_line=M[, force=true])` | the path the caller gave |
| `src/tools/markdown/read_markdown.rs:681` `read_markdown_line_range` `next_at` | `read_file("<path>", start_line=N, end_line=M)` | the path the caller gave |
| `src/tools/read_file.rs:716` `value_line_hint` (via the new `grep_o_route`, :738) | `run_command("grep -o 'PATTERN[^,]*' <path>")` | the path the caller gave (real-file value) or a `@tool_*` ref |
| `src/tools/read_file.rs:954` `line_hint` (via `grep_o_route`; both branches) and `:960` its field route `read_file("<path>", json_path="<jp>")` | both shapes | a buffer ref in production (every caller passes one); JSON-quoted too, so a caller handing it a path cannot reintroduce the defect |

UNCHANGED, verified plain by construction (the argument is a generated handle, never a name):

- `src/tools/read_file.rs:458` `read_from_buffer` `route`: `path` is a ref that `get_stream` found. Refs are
  minted as `@cmd_`/`@file_`/`@tool_` + 8 hex digits (`src/tools/output_buffer.rs:431`, `:782`, `:832`) and
  looked up by exact key (`resolve` strips only a `.err` suffix).
- `src/tools/read_file.rs:1571`, `:1576` (`outline_hint`), `src/tools/file_summary/file_summary.rs:1310`,
  `:1425`, `:1514`, `:1518`, `src/tools/markdown/read_markdown.rs:204`, `:207`, `:529`, `:540` (`{:?}` of a
  `file_id`), `src/tools/core/types.rs:1799`-`1800` (`ref_id`), `src/tools/libtest_compact.rs:62`,
  `src/tools/command_summary.rs:556`, `src/tools/run_command/inner.rs:126`, `:379`,
  `src/tools/core/write_ack.rs:51`, `:118`: all take a handle.
- `src/tools/run_command/output.rs:624` `wide_line_route`: `queried_stream` is restricted by regex to
  `@(cmd|file|tool|bg)_[A-Za-z0-9_]+`.
- Prose that names a path and is not a call, left as it is: `read_file.rs:348`, `:353`,
  `ValueContinuation::prose`, the `searched` words of `value_line_hint`.
- Examples with no path: `read_file.rs:115`, `:362`, `run_command/mod.rs:63`, `:108`, `:291`.

SAME DEFECT, other shapes, NOT changed here (outside the two shapes this bug names; each puts a path or
root between double quotes raw, in a call or an argument fragment a caller copies):

- `src/tools/grep.rs:447`, `:540` and `src/tools/symbol/symbols.rs:62`: `path="{file}"` fragments of the
  "narrow with" hints; the file is a search result.
- `src/tools/symbol/edit_code.rs:269`, `:327`: `references(symbol="…", path="{rel_path}")`.
- `src/tools/config/mod.rs:798`, `:1065`, `:1083`, `src/tools/core/guards.rs:18`:
  `workspace(action='activate', path="{root}")`.
- `src/util/path_security.rs:475`: `approve_write(path="{dir}")`.

## Hypotheses tried

None.

## Fix

Commits `4e69200e` (fix and first tests), `5744620d` and `923a1dd3` (tests only), branch `fix/route-paths-escaped`.

- `src/util/text.rs`: two shared helpers, the string helpers' home. `json_quote(s)` is `s` as a JSON string
  literal, quotes included (serde_json's escaping, the one `json_escaped_len` counts). `shell_quote(s)` is
  `s` as ONE POSIX shell word: bare when only `[A-Za-z0-9_@%+=:,./-]`, else single-quoted with `'` written
  `'\''`, and `None` for a line break, a carriage return or a NUL, which no one-line command can carry.
- `src/tools/read_file.rs`: `grep_o_route(target)` is the one builder of the `run_command("grep -o …")` route
  (shell-quote, then JSON-quote the whole command; `./` before a name that starts with `-`; `None` for a
  line break). `value_line_hint` and `line_hint` use it, and name NO route for a line break instead of a
  broken one. `real_file_route`, `ValueContinuation::route`, both `next` of `read_with_line_range`, and the
  field route of `line_hint` write the path with `json_quote`.
- `src/tools/markdown/read_markdown.rs`: the markdown range `next` writes the path with `json_quote`.
- Buffer refs are plain words, so their routes are byte-identical to before.

## Tests added

In `src/tools/read_file_route_path_tests.rs` (module `read_file::route_path_tests`) and `src/util/text.rs`.
Every route is parsed back with a real JSON parser and EXECUTED: a `read_file(...)` through `ReadFile` (its
path must equal the file's byte for byte, and every page must fit the inline limit), a `run_command(...)`
under the real `sh` (and `dash` when installed) and through the real `RunCommand` tool, against a file whose
name holds a space, `"`, `'`, `\`, `$`, a backtick, a tab, a line break, glob and operator characters and
non-ASCII text.

- `a_value_next_of_an_awkward_path_is_a_call_that_parses_back_and_runs` (json_path and toml_key)
- `a_line_range_next_of_an_awkward_path_is_a_call_that_parses_back_and_runs` (`.txt`, `.rs` with `force=true`, `.md`)
- `a_path_full_of_escapes_still_leaves_every_page_inside_the_limit` (240 quotes or backslashes in the name)
- `the_wide_line_hint_of_an_awkward_path_is_a_command_that_prints_the_match` (also: a line break gets no route)
- `grep_o_route_is_one_argument_that_names_the_file` (relative and absolute names, `-r`, `~`)
- `the_clamp_advisory_quotes_its_target_in_both_routes`
- `the_real_file_route_of_a_buffer_ref_error_parses_back`
- `json_quote_parses_back_to_the_exact_name_with_a_real_json_parser`,
  `shell_quote_makes_one_word_that_the_shell_reads_back_byte_for_byte`,
  `shell_quote_reads_back_every_printable_ascii_character_alone_and_inside_a_word`,
  `shell_quote_names_no_word_for_a_line_break_a_carriage_return_or_a_nul`,
  `shell_quote_leaves_a_plain_word_and_a_buffer_ref_as_they_are`

Mutation (`scripts/mutation-probe.sh`, one mutant at a time, committed bytes): 26 mutants, 26 killed, after two
survivors were fixed by tests. Survived first: the `widest` skeleton of `read_with_line_range` priced with the
raw path (120 quotes were inside the ~330 B of slack that the `hint` and `line_truncated` keys give an uncut
page; 240 quotes are not), and a raw `json_path` literal in the field route of `line_hint` (inert for the
plain `$.ident` paths production hands it; the test now passes one with a quote and a backslash). The raw key
literal of `ValueContinuation::new` is killed by the existing `read_file::buffer_edge_tests`, not by the new
ones.

## Workarounds

Copy the route by hand and quote the path.

## Resume

`read_with_line_range`, `value_of_a_file`, `line_hint` and the clamp hint in `src/tools/read_file.rs`.
Execute each route in a test with a path that has a space and a quote.

## References

- `docs/adrs/2026-10-08-a-read-keeps-the-handle-it-reads-and-one-real-file-has-one-handle.md`
