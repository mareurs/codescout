---
status: fixed
opened: 2026-07-17
closed: 2026-07-17
severity: medium
owner: marius
related: []
tags: [edit_markdown, edit_file, crlf, windows]
kind: bug
---

# BUG: `edit_markdown`'s `edit` action doesn't tolerate CRLF across a multi-line `old_string`, unlike `edit_file`

## Summary
`edit_markdown(action="edit", old_string=..., new_string=...)` (scoped in-section text
replacement) failed with "old_string not found" on any Windows-checked-out (`\r\n`)
markdown file whenever `old_string` spanned more than one line and was built with bare
`\n` newlines (the normal MCP-payload shape). `edit_file` already has a CRLF-tolerant
fallback for exactly this case; `edit_markdown`'s `edit` action had no equivalent.

## Symptom (Effect)
```
old_string not found in section '<heading>'. The text must match exactly (whitespace-sensitive).
```
Returned even though `read_markdown`/`read_file` showed content that looked
byte-identical to `old_string` (the diff was invisible — a trailing `\r` per line).

## Reproduction
Commit `beba4a7033cd174a898f30777c1fd58c91814a4b` (pre-fix).
```rust
let content = "# Title\r\n## Setup\r\nfoo\r\nbar\r\nbaz\r\n";
perform_scoped_edit(content, "## Setup", "foo\nbar", "FOO\nBAR", false); // Err before fix
```
Or via the MCP tool: call `edit_markdown` with `action="edit"` against any `\r\n`
checked-out `.md` file, passing a multi-line `old_string` built with bare `\n`.

## Environment
Windows, `core.autocrlf` producing `\r\n` working-tree line endings. codescout repo,
`src/tools/markdown/edit_markdown.rs`.

## Root cause
`perform_scoped_edit` (src/tools/markdown/edit_markdown.rs) matched `old_string` against
the section text with a single exact `section_text.contains(old_string)` check and had no
fallback. `edit_file`'s `perform_edit` (src/tools/edit_file/mod.rs) already carries a
`find_crlf_tolerant_windows` fallback (added for exactly this class of bug, see that
function's doc comment) that tolerates a lone trailing `\r` per content line without
touching indentation — `edit_markdown`'s scoped-edit path never got the equivalent.

## Evidence
Failing test before fix (`cargo test --lib scoped_edit_crlf_tolerant_multiline_old_string`):
```
thread 'tools::markdown::tests::scoped_edit_crlf_tolerant_multiline_old_string' panicked:
called `Result::unwrap()` on an `Err` value: old_string not found in section '## Setup'.
The text must match exactly (whitespace-sensitive).
```

## Hypotheses tried
1. **Hypothesis**: `edit_markdown`'s `edit` action shares the same matching code as
   `edit_file`. **Test**: grepped for `find_crlf_tolerant_windows` usage across `src/`.
   **Verdict**: rejected — it's private to `src/tools/edit_file/mod.rs`, never called from
   `src/tools/markdown/edit_markdown.rs`.
2. **Hypothesis**: `perform_scoped_edit` has no CRLF fallback at all (root cause).
   **Test**: read `perform_scoped_edit`; only match logic is
   `section_text.contains(old_string)` → error. **Verdict**: confirmed.

## Fix
Added `find_crlf_tolerant_ranges` (src/tools/markdown/edit_markdown.rs), mirroring
`edit_file`'s `find_crlf_tolerant_windows` line-window algorithm but scoped to a section's
extracted `String` (byte ranges instead of `NormWindow`+file offsets, no AST syntax-error
gate since markdown has no such check). `perform_scoped_edit` now tries this fallback when
the exact match fails and requires exactly one match (same conservative uniqueness gate as
`edit_file`) before applying it, adapting the replacement's line endings to the matched
region's convention. Implemented in
`src/tools/markdown/edit_markdown.rs` (`find_crlf_tolerant_ranges`, and the fallback branch
in `perform_scoped_edit`). Not yet cherry-picked to master — working tree only as of this
writing.

## Tests added
`scoped_edit_crlf_tolerant_multiline_old_string` — src/tools/markdown/tests.rs:766.

## Workarounds
Before the fix: normalize the file to LF first (or build `old_string`/`new_string` with
literal `\r\n` matching the file), or fall back to single-line `old_string` values only.

## Resume
N/A — fixed, regression test passing, `cargo fmt`/`clippy -D warnings`/`cargo test --lib`
(full suite except the pre-existing unrelated
`docs/issues/2026-07-13-truncated-lsp-range-repair-test-fails-on-windows.md` flake) all
green. Not yet committed/cherry-picked to master.

## References
- `src/tools/edit_file/mod.rs::find_crlf_tolerant_windows` (the sibling fallback this
  mirrors).
- `src/tools/markdown/edit_markdown.rs::perform_scoped_edit`.
