---
id: 7f06fabbc480a468
kind: bug
status: fixed
title: 'BUG: a path with a quote or a space breaks the routes read_file and run_command name'
tags:
- cluster/unclassified
- read_file
- progressive-disclosure
closed: 2026-10-10
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

The three outputs above, and the route sweep (first taken 2026-10-09 at `918b3d51`, CORRECTED after review at
`ea64a514`: every place that builds a `read_file(…)` or `run_command(…)` route, each read in the code, not
taken from a grep hit):

Built from a real file's path or a caller-given target, CHANGED:

| Where | Route shape | Path comes from |
|---|---|---|
| `src/tools/read_file.rs:346` `real_file_route` (in `read_from_buffer`) | `read_file("<source>", json_path\|toml_key="…")` | the `source_path` of a `@file_*` handle |
| `src/tools/read_file.rs:777` `ValueContinuation::new` / `route` | `read_file("<path>", <nav>="…", start_line=N, end_line=M)` | the path the caller gave (real file), or a `@tool_*` ref |
| `src/tools/read_file.rs` `read_with_line_range`, two uses of `path_literal` (the `widest` skeleton `next` and the result `next`) | `read_file("<path>", start_line=N, end_line=M[, force=true])` | the path the caller gave |
| `src/tools/markdown/read_markdown.rs:681` `read_markdown_line_range` `next_at` | `read_file("<path>", start_line=N, end_line=M)` | the path the caller gave |
| `src/tools/read_file.rs:716` `value_line_hint` (via `grep_o_route`, :738, and `hide_handle_tokens`, :766) | `run_command("grep -o 'PATTERN[^,]*' <path>")` | the path the caller gave (real-file value) or a `@tool_*` ref |
| `src/tools/read_file.rs:954` `line_hint` (via `grep_o_route`; both branches) and `:960` its field route `read_file("<path>", json_path="<jp>")` | both shapes | a buffer ref in production; JSON-quoted too |
| `src/tools/file_summary/file_summary.rs:1312` (`bound_summary`, wide-string marker), `:1429` (`bound_summary`, summary omitted), `:1515` `file_id_literal` (`cut_array_middle`, exact-range and generic notes) | `read_file(path="<id>", start_line=…, end_line=…)` | a handle from three callers; the REAL path from `read_markdown_single_heading` (heading not found), `src/tools/markdown/read_markdown.rs:431` |

The first sweep called the four `file_summary.rs` sites handle-only. That was wrong for one caller. Every caller
of `fit_envelope` (the only caller of `bound_summary`, which is the only caller of `cut_array_middle`), read:
`read_markdown.rs:431` passes `path` (the real path, as the caller gave it); `read_markdown.rs:586` passes
`file_id` from `whole_text_handle` (:476); `read_markdown.rs:872` passes `file_id` from `whole_text_handle`
(:837); `read_file.rs:1725` (`read_full_file`) passes `file_id` from `store_file_read`. A file named `q"uo te.md`
with 2,000 headings, read with `heading="## nope-not-here"`, printed a route a JSON parser read back as `/tmp/…/q`.

UNCHANGED, verified plain by construction (the argument is a generated handle, never a name):

- `src/tools/read_file.rs:458` `read_from_buffer` `route`: `path` is a ref that `get_stream` found. Refs are
  minted as `@cmd_`/`@file_`/`@tool_` + 8 hex digits (`src/tools/output_buffer.rs:431`, `:782`, `:832`) and
  looked up by exact key (`resolve` strips only a `.err` suffix).
- `src/tools/read_file.rs:1571`, `:1576` (`outline_hint`), `src/tools/markdown/read_markdown.rs:204`, `:207`,
  `:529`, `:540` (`{:?}` of a `file_id`), `src/tools/core/types.rs:1799`-`1800` (`ref_id`),
  `src/tools/libtest_compact.rs:62`, `src/tools/command_summary.rs:556`, `src/tools/run_command/inner.rs:126`,
  `:379`, `src/tools/core/write_ack.rs:51`, `:118`: all take a handle.
- `src/tools/run_command/output.rs:624` `wide_line_route`: `queried_stream` is restricted by regex to
  `@(cmd|file|tool|bg)_[A-Za-z0-9_]+`.
- Prose that names a path and is not a call, left as it is: `read_file.rs:348`, `:353`,
  `ValueContinuation::prose`, the `searched` words of `value_line_hint`.
- Examples with no path: `read_file.rs:115`, `:362`, `run_command/mod.rs:63`, `:108`, `:291`.

SAME DEFECT, other shapes, NOT changed (outside the two shapes this bug names; each puts a path or root between
double quotes raw, in a call or an argument fragment a caller copies):

- `src/tools/grep.rs:447`, `:540` and `src/tools/symbol/symbols.rs:62`: `path="{file}"` fragments of the
  "narrow with" hints; the file is a search result.
- `src/tools/symbol/edit_code.rs:269`, `:327`: `references(symbol="…", path="{rel_path}")`.
- `src/tools/config/mod.rs:798`, `:1065`, `:1083`, `src/tools/core/guards.rs:18`:
  `workspace(action='activate', path="{root}")`.
- `src/util/path_security.rs:475`: `approve_write(path="{dir}")`.
- `src/prompts/builders.rs:558`-`570`, `:624`, `:637`, `:957`-`1002`: onboarding instructions that name a
  prompt path inside `read_file("{p}", …)`.

One more handle problem, found by review and fixed: `run_command` swaps a handle-shaped token
(`@cmd_deadbeef`, `@file_`, `@tool_`, `@bg_`, optional `.err`) for the buffer's content BEFORE the shell parses
the command (`OutputBuffer::resolve_refs`, `REF_RE`, `src/tools/output_buffer.rs:1151`), so no quoting hides one
inside a file name. `grep_o_route` breaks every such token in the name with an empty `''` after the `@`
(`hide_handle_tokens`, `src/tools/read_file.rs:766`; the pattern is a copy of `REF_RE` plus `ack`, because that
file was not edited).

Not fixed and not part of this bug: the Windows compile break at `src/tools/read_file_one_handle_tests.rs:1008`
(`std::os::unix::fs::symlink` ungated).

## Hypotheses tried

None.

## Fix

Commits `4e69200e` (fix and first tests), `5744620d`, `923a1dd3` (tests), `ea64a514` (review round: file_summary
notes, handle tokens, platform gating of the tests), `8dc54e95` (a test case); branch `fix/route-paths-escaped`.

- `src/util/text.rs`: two shared helpers, the string helpers' home. `json_quote(s)` is `s` as a JSON string
  literal, quotes included (serde_json's escaping, the one `json_escaped_len` counts). `shell_quote(s)` is
  `s` as ONE POSIX shell word: bare when only `[A-Za-z0-9_@%+=:,./-]`, else single-quoted with `'` written
  `'\''`, and `None` for a line break, a carriage return or a NUL, which no one-line command can carry.
- `src/tools/read_file.rs`: `grep_o_route(target)` is the one builder of the `run_command("grep -o …")` route
  (shell-quote, then `hide_handle_tokens`, then JSON-quote the whole command; `./` before a name that starts
  with `-`; `None` for a line break). `value_line_hint` and `line_hint` use it, and name NO route for a line
  break instead of a broken one. `real_file_route`, `ValueContinuation::route`, both `next` of
  `read_with_line_range`, and the field route of `line_hint` write the path with `json_quote`.
- `src/tools/markdown/read_markdown.rs`: the markdown range `next` writes the path with `json_quote`.
- `src/tools/file_summary/file_summary.rs`: the four cut notes write the file id with `json_quote`.
- Buffer refs are plain words, so their routes are byte-identical to before.

## Fix provenance

- **SHA:** `4e69200e293644d253e0bd469ea9471eac8fb095` (`experiments`)
- **patch-id:** `2ee01ecff7b5326b0407bf8958c09729bab2cb66`

That is the first commit of a series. The rest, in landing order, with their patch-ids:

| SHA | patch-id | What |
|---|---|---|
| `5744620dc03cfedb3ccded059e31175232e1692f` | `f8b663fd85d9af0ca3a307f7414ba55c1a8bb723` | test: every printable ASCII character through `shell_quote` |
| `923a1dd332cea916fe8333b7fc4ce1632c6f1c4c` | `61b0a03fc43289b8124ae6c9425f8dff65c0fd10` | test: the escapes of a path outweigh the range-page skeleton's slack |
| `ea64a51464e2357c3803c0bdb3bfae0e3d7162d7` | `8026cd6407aab94940b23988695b8ed741aa3a98` | fix: the file-summary cut notes quote the real path; handle tokens broken in `grep -o` routes |
| `8dc54e95ceb2bebf0497e59fe4f68ec051ae1f13` | `2ba73cad2d0de5d7fbaa2d3dc08323302a324e67` | test: a handle at the end of a longer name is not that handle |
| `5f8c56a8952dd72faf3efdaa1a16e9e8a95912be` | `0e6971f48ca9f87d5326d98755f52af7286f2e58` | test: the handle-token regex is built once (clippy) |
| `6fb4e25f7b76962da3fcaf024704a3e4edcf8fbb` | `d0586876adf07f69d81f8163c510a2eca7fbe19d` | fix: the other tools' paths: `grep` and `symbols` overflow hints, `edit_code` references call, `workspace` activate and pin calls, worktree notices, `approve_write`, the onboarding `read_file` of a project prompt; `activate_call` is the one builder of the activate call |
| `1780c38d9b25ea0919eb9f662c0c7ce8bda4dcc9` | `961eb3b3bf6ebf147119fa4debfa45b5e5fd0c0c` | fix: the roots the doctor prints (`rehome`, `merge_worktree`), the symbols overview's `symbols(path="<dir>/**/*")` call, the per-project onboarding `tree("<root>")` |

The last two came from a sweep for the same defect in every builder that writes a real path into text a caller copies. `6fb4e25f` also gates the test `every_spelling_of_one_path_names_its_one_handle` with `cfg(unix)`: it called `std::os::unix::fs::symlink` and broke the Windows lib-test build (found with `scripts/build-windows.sh check --lib --tests`).

## Tests added

In `src/tools/read_file_route_path_tests.rs` (module `read_file::route_path_tests`) and `src/util/text.rs`.
Every route is parsed back with a real JSON parser and the path compared byte for byte. The tests in
`route_path_tests::unix_only` EXECUTE them: a `read_file(...)` through `ReadFile` (every page must fit the inline
limit), a `run_command(...)` under the real `sh` (and `dash` when installed; `sh` must run, only `dash` may be
absent) and through the real `RunCommand` tool, against a file whose name holds a space, `"`, `'`, `\`, `$`, a
backtick, a tab, a line break, glob, brace and operator characters, handle-shaped tokens and non-ASCII text. They
are `cfg(unix)`: NTFS forbids those names and Windows has no POSIX shell. The tests outside `unix_only` build no
file and spawn no shell, so every lane runs them.

Platform-neutral:
- `the_clamp_advisory_quotes_its_target_in_both_routes` (literal expected commands)
- `grep_o_route_names_no_route_for_a_line_break_and_leaves_a_handle_alone`
- `a_handle_shaped_token_inside_a_name_is_broken_so_the_resolver_does_not_see_it`
- `the_cut_notes_of_a_summary_quote_the_file_they_name` (all four `file_summary` sites, with a handle control)
- `json_quote_parses_back_to_the_exact_name_with_a_real_json_parser`,
  `shell_quote_names_no_word_for_a_line_break_a_carriage_return_or_a_nul`,
  `shell_quote_leaves_a_plain_word_and_a_buffer_ref_as_they_are` (also `{a,b}` and `x{1..3}y`)

`cfg(unix)`:
- `a_value_next_of_an_awkward_path_is_a_call_that_parses_back_and_runs` (json_path and toml_key)
- `a_line_range_next_of_an_awkward_path_is_a_call_that_parses_back_and_runs` (`.txt`, `.rs` with `force=true`, `.md`)
- `a_path_full_of_escapes_still_leaves_every_page_inside_the_limit` (240 quotes or backslashes in the name)
- `a_heading_not_found_in_a_long_file_prints_routes_that_parse_back_and_run` (2,000 headings, `q"uo te.md`)
- `the_wide_line_hint_of_an_awkward_path_is_a_command_that_prints_the_match` (also: a line break gets no route)
- `grep_o_route_is_one_argument_that_names_the_file` (relative and absolute names, `-r`, `~`)
- `the_real_file_route_of_a_buffer_ref_error_parses_back` (compares the canonical path)
- `shell_quote_makes_one_word_that_the_shell_reads_back_byte_for_byte`,
  `shell_quote_reads_back_every_printable_ascii_character_alone_and_inside_a_word`

Mutation (`scripts/mutation-probe.sh`, one mutant at a time, committed bytes): 36 mutants at `8dc54e95`, all
killed. The 26 of the first round, plus: braces added to the bare set (H9), `json_quote` dropped at each of the
four `file_summary` sites, the handle-token break removed, the whole-handle exemption removed, `ack` dropped from
the pattern, the start-of-name check dropped, the `.err` strip dropped. One mutant, the raw key literal of
`ValueContinuation::new`, is killed by the existing `read_file::buffer_edge_tests` (3 tests), not by the new ones.
The first round's two survivors (the `widest` skeleton priced with the raw path, a raw `json_path` literal in
the field route) were fixed by 240-escape and quote-bearing-field cases. Not run: the Windows lanes (no
toolchain here), and `dash` (not installed here).

## Workarounds

Copy the route by hand and quote the path.

## Resume

Landed on `experiments` at `377b685b` by fast-forward from `918b3d51`, so the series keeps its SHAs. Nothing is pushed.

Verified live on the rebuilt release binary (2026-10-10, built after the landing): `read_file` of a file named `q"uo te.json` prints its `Next:` route with the path as `q\"uo te.json`, a JSON string literal that reads back as the file name.

Not covered: the Windows and macOS lanes (the file-name tests are `cfg(unix)`; `scripts/build-windows.sh check --lib --tests` type-checks them for Windows and passes), and `dash` (not installed here).

Known limits, not fixed:
- `grep` prints every file through `to_forward_slash`, so a file named `back\slash.txt` appears as `back/slash.txt` in its results and hints (probed 2026-10-10).
- The `project_id="{id}"` sites in `src/prompts/builders.rs` write a project id raw. An id is not a file path; whether an id can hold a quote or a backslash was not checked.

## References

- `docs/adrs/2026-10-08-a-read-keeps-the-handle-it-reads-and-one-real-file-has-one-handle.md`
