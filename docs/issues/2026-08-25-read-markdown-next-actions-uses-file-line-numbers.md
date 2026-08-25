---
status: open
opened: 2026-08-25
closed:
severity: low
owner: marius
related:
  - docs/issues/archive/2026-08-25-run-command-nested-buffer-recursion.md
tags:
  - read_markdown
  - progressive-disclosure
  - hints
kind: bug
---

# BUG: `read_markdown`'s oversized-section `next_actions` addresses the excerpt handle with the FILE's line numbers

## Summary

When a requested heading's section is too large to inline, `read_markdown`
returns a `RecoverableError` carrying a `@file_*` handle holding **that section**
plus a `next_actions` entry telling the caller how to page through it. The line
numbers in that entry are the section's position **in the file**, but the handle
contains only the section, whose own line 1 is the section's first line. For any
section that does not start at file line 1, the suggested call is out of range —
and the tool rejects the call it just told the caller to make.

## Symptom (Effect)

```
read_markdown(path="…/deep.md", heading="## Big")
→ {
    "error": "section \"## Big\" spans 202 lines — exceeds inline threshold",
    "file_id": "@file_3ab5eac1",
    "next_actions": ["read_markdown(\"@file_3ab5eac1\", start_line=304, end_line=404)"],
    "line_range": [304, 505]
  }
```

Following that suggestion verbatim:

```
read_markdown(path="@file_3ab5eac1", start_line=304, end_line=404)
→ {
    "error": "start_line 304 exceeds file length 202",
    "hint": "valid range is 1..=202; use read_markdown(path, start_line=N, end_line=M) within bounds",
    "lines": 202
  }
```

## Reproduction

Live MCP server, any commit through `7712d8e6`.

1. Build a markdown file whose oversized section does **not** start at line 1:

   ```
   { echo "# Doc"; echo; for i in $(seq 1 300); do echo "filler line $i"; done;
     echo; echo "## Big"; echo;
     for i in $(seq 1 200); do echo "content line $i aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"; done; } > /tmp/deep.md
   ```

   `## Big` lands at line 304; the file is 505 lines.

2. `read_markdown(path="/tmp/deep.md", heading="## Big")` — oversized, so the
   response carries `file_id` and `next_actions`.

3. Run the `next_actions` entry verbatim.

**Expected:** the first ~100 lines of the section.
**Got:** `start_line 304 exceeds file length 202`.

A section starting at file line 1 masks the bug entirely — the two frames
coincide there, which is why the existing test
`heading_on_large_section_returns_ok_false_with_hint_and_section_map`
(`src/tools/markdown/tests.rs`) does not catch it: its fixture puts `# Root` at
line 1.

## Environment

- codescout `0.15.0`, branch `experiments`, Linux, stdio MCP transport.
- Observed 2026-08-25 against the live server.

## Root cause

`read_markdown_single_heading` (`src/tools/markdown/read_markdown.rs`) builds
`next_actions` from `section_result.line_range`, which is the section's extent
**in the file**:

```rust
let (start_ln, end_ln) = section_result.line_range;
…
actions.push(format!(
    "read_markdown({:?}, start_line={}, end_line={})",
    file_id, start_ln, start_ln + 100.min(section_lines)
));
```

`file_id` addresses a buffer holding only `section_result.content`, so its line 1
is the file's `start_ln`. The two coordinate frames differ by `start_ln - 1`.

This is the same defect class as
`docs/issues/archive/2026-08-25-run-command-nested-buffer-recursion.md` — a
response mixing the original file's line numbers with a derived handle's own —
but a different mechanism: that one was `next` in the paginated read path, this
one is `next_actions` in the oversized-section error path. Fixing the first did
not touch this.

measured 2026-08-25: the reproduction above, run against the live server; the
suggested call returned `start_line 304 exceeds file length 202`.

## Evidence

See Symptom — both calls quoted verbatim from the session.

Note `section_map` was `[]` in this reproduction (the section has no
sub-headings), so `next_actions` held exactly one entry and it was the broken
one: there was no working alternative alongside it.

## Hypotheses tried

1. **Hypothesis:** the handle holds the whole file, so file-relative numbers
   would be right. **Test:** read the error's own `lines` field and the
   `store_file_excerpt` call site. **Verdict:** rejected — the handle holds 202
   lines (the section), not 505 (the file).

## Fix

Not yet fixed. The line numbers should be the excerpt's own — `start_line=1`,
`end_line=100.min(section_lines)` — since `file_id` is what they address.

Worth deciding at the same time whether the *other* `next_actions` entry (the
sub-heading one, `read_markdown(file_id, heading=…)`) is correct: heading
addressing is position-independent, so it likely is, but it was not exercised in
this reproduction because `section_map` was empty.

## Tests added

None yet — this file is the capture, not the fix. When fixed, the regression
test must place the target section away from line 1; a fixture with the heading
at line 1 passes under both the broken and the correct arithmetic.

## Workarounds

Ignore the numbers in `next_actions` and read the handle from its own start:
`read_markdown("@file_…", start_line=1, end_line=100)`. The rejection message is
accurate and names the valid range (`1..=202`), so the recovery is one call.

## Resume

Fix `next_actions` construction in `read_markdown_single_heading`
(`src/tools/markdown/read_markdown.rs`), then add a regression test in
`src/tools/markdown/tests.rs` next to
`heading_on_large_section_returns_ok_false_with_hint_and_section_map`, using a
fixture whose oversized section starts well past line 1.

## References

- `docs/issues/archive/2026-08-25-run-command-nested-buffer-recursion.md` — same
  defect class (mixed coordinate frames), different mechanism.
- `docs/issues/archive/2026-08-25-file-slice-handle-refreshes-to-whole-file.md` — the
  sibling that made these excerpt handles snapshots; found in the same sweep.
- `src/tools/markdown/read_markdown.rs` — `read_markdown_single_heading`.
