---
status: fixed
opened: 2026-05-26
closed: 2026-05-26
severity: high
owner: marius
related: []
tags: [edit_markdown, markdown, insert_after, silent-corruption]
kind: bug
---

# BUG: edit_markdown insert_after fuses inserted content onto the following heading

## Summary
`edit_markdown(action="insert_after", at="end-of-section")` did not guarantee a
newline after the inserted block. When the caller's `content` lacked a trailing
`\n`, the inserted block's last line fused directly onto the next heading line
(`new entry## Heading`), silently demoting that heading — and every sibling
after it — to body text. Every edit returned `"ok"`; nothing failed loudly.

## Symptom (Effect)
Inserting at end-of-section before a sibling/parent heading produced output like:

```
## Section A
content here

new entry## Constraint Stream Patterns
more content
```

`## Constraint Stream Patterns` is no longer at line-start, so the heading
parser treats it as body text. In the original session three consecutive
headings were demoted in one edit (`## Constraint Stream Patterns`,
`## Custom Moves`, `## Timefold Migration`). The tool returned `"ok"` for every
call — the corruption was only visible by re-reading the rendered heading map.

## Reproduction
At commit `09748dce8d64990f7dd8d283b55677b1669d708f` (branch `experiments`):

```rust
let content = "## Section A\ncontent here\n\n## Constraint Stream Patterns\nmore content\n";
let result = perform_section_edit(content, "## Section A", "insert_after", Some("new entry")).unwrap();
// result contains "new entry## Constraint Stream Patterns" — fused.
```

Note the inserted `content` ("new entry") has **no** trailing newline — that is
the trigger. Callers that happen to terminate `content` with `\n` never see it.

## Environment
codescout v0.14.0, Linux, MCP stdio transport, `experiments` branch. Language-
agnostic — pure markdown string manipulation, no LSP involved.

## Root cause
The `insert_after` arm of `perform_section_edit_ext`
(`src/tools/markdown/edit_markdown.rs`) built its result with the raw caller
content:

```rust
let result = format!("{}{}{}", before, new, after);
```

`after` begins with the next line of the document, which at end-of-section is
typically the next heading. With no newline between `new` and `after`, the
last line of `new` and the heading land on the same physical line. The
heading parser (`heading_level` in `src/tools/file_summary/file_summary.rs`)
recognizes a heading only when the line *starts with* `#` — a fused line
`new entry## Heading` starts with `n`, so it parses as body text.

The sibling arms `replace` and `insert_before` already wrapped their content in
`ensure_trailing_newline(new)`; only `insert_after` used the raw string. The
inconsistency was the bug.

## Evidence

### Red reproduction test output (pre-fix)
```
inserted content fused onto the following heading:
"## Section A\ncontent here\n\nnew entry## Constraint Stream Patterns\nmore content\n"
```

### Parser confirms no blank line is needed — only line-start
`parse_all_headings_basic` (`src/tools/file_summary/tests.rs`) parses
`"# Title\ntext\n## Setup\n…"` as 3 headings with `## Setup` directly after a
content line. So the demotion is purely fusion (heading not at line-start),
not a missing-blank-line issue.

## Hypotheses tried
1. **Hypothesis:** the bug is in the parser requiring a blank line before headings.
   **Test:** read `heading_level` + `parse_all_headings`; checked
   `parse_all_headings_basic`. **Verdict:** rejected — parser recognizes a
   heading immediately after a content line; only requires line-start.
2. **Hypothesis:** `insert_after` omits the trailing-newline normalization that
   `replace`/`insert_before` apply. **Test:** read all three match arms.
   **Verdict:** confirmed — `insert_after` used raw `new`; the others use
   `ensure_trailing_newline(new)`.

## Fix
Wrap the inserted content in `ensure_trailing_newline(new)` in the
`insert_after` arm, matching the `replace` / `insert_before` arms:

```rust
let result = format!("{}{}{}", before, ensure_trailing_newline(new), after);
```

Change lives in `src/tools/markdown/edit_markdown.rs` (`insert_after` arm of
`perform_section_edit_ext`). Fix verified on `experiments`; **master-side SHA
to be recorded after cherry-pick** (see CLAUDE.md § "After cherry-pick").

## Tests added
`insert_after_without_trailing_newline_preserves_following_heading`
(`src/tools/markdown/tests.rs`) — feeds `content` with no trailing newline,
asserts the result does NOT contain the fused string and that the following
heading is still recognized by `parse_all_headings`. All 113 markdown tests
pass; the test fails on the pre-fix code (confirmed red → green).

## Workarounds
Always terminate `insert_after` content with `\n`, and verify markdown
structure by re-reading the heading map (`read_markdown(path)`) rather than
trusting the per-edit `"ok"` — the corruption is silent. (No longer needed
post-fix, but the heading-map verification habit remains good practice for any
structural markdown edit.)

## Resume
N/A — fixed.

## References
- `src/tools/markdown/edit_markdown.rs` — `perform_section_edit_ext`, `ensure_trailing_newline`
- `src/tools/file_summary/file_summary.rs` — `heading_level`, `parse_all_headings`
- `src/tools/markdown/tests.rs` — regression test
