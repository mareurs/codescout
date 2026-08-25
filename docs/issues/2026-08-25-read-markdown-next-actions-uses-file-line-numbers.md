---
kind: bug
status: fixed
tags:
- read_markdown
- progressive-disclosure
- hints
closed: 2026-08-25
opened: 2026-08-25
owner: marius
related:
- docs/issues/archive/2026-08-25-run-command-nested-buffer-recursion.md
severity: low
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

## Two more defects in the same block, found while fixing

The filed report named the `next_actions` line-range entry. Re-running the
reproduction with a fixture that *has* sub-headings (the original had
`section_map: []`) surfaced two more, both in the same ~15 lines and both
breaking the same thing — the caller cannot act on the steering payload.

**`section_map[].l` is file-relative too.** Built as `json!({"h": h.text, "l":
h.line})`, where `h.line` comes from `parse_all_headings(text)` over the whole
file. The hint tells the caller to "pick a sub-heading from `section_map` or
start_line/end_line", so these numbers read as addresses into `file_id` — and
they are out of range for it.

This one is not a judgment call about which frame reads better, because the
server already contradicts itself. Asking the same handle for a heading that
does not exist returns:

```
error: heading '### Nope' not found

available headings:
  ## Big  L1
    ### Sub A  L3
    ### Sub B  L106
```

`### Sub A` at **L3** of the handle, while `section_map` reports `l: 306` for the
same heading — offset by exactly `start_ln - 1 = 303`. The error path already
votes for the handle's frame.

**The sub-heading `next_action` is not pasteable.** Built with `{}` rather than
`{:?}` for the heading, it emits:

```
read_markdown("@file_3abc23a4", heading=### Sub A)
```

an unquoted argument containing spaces and `#`.

measured 2026-08-25: all three observed in one response against the live server,
fixture `deep2.md` (`## Big` at file line 304, 207 lines, two sub-headings).
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

One rule, applied to the whole block in
`read_markdown_single_heading` (`src/tools/markdown/read_markdown.rs`): **every
number that addresses `file_id` is stated in that handle's frame.**

- The line-range `next_action` becomes `start_line=1, end_line=100.min(section_lines)`
  — it addresses the handle, whose first line is 1.
- `section_map[].l` becomes `h.line - start_ln + 1`. The filter above it already
  guarantees `h.line > start_ln`, so this cannot underflow. It now agrees with
  the heading-miss listing quoted above.
- The sub-heading `next_action` uses `{:?}` for the heading, so it round-trips
  as a pasteable call.

`line_range` is deliberately left file-relative. It is the one field in this
payload that describes *where the section lives* rather than *how to address the
handle*, and it pairs with `breadcrumb`. A caller who needs a file line can
recover it as `line_range[0] + l - 1`. The reasoning is recorded as a comment at
the site, so the next reader does not "fix" it into the handle's frame.

Heading addressing was verified to work against an excerpt handle before relying
on it: the miss-listing above is produced by parsing the handle's own content,
which is what makes `heading=` position-independent and therefore the right
primary next action.
## Tests added

`oversized_section_steering_numbers_address_the_handle_not_the_file`
(`src/tools/markdown/tests.rs`) — RED before the change, failing on
`start_line=304` exactly as measured live.

Its fixture puts `## Big` at file line 304 with two sub-headings. That is
load-bearing: a section starting at line 1 passes under **both** the broken and
the correct arithmetic, because the two frames coincide there — which is exactly
why the pre-existing
`heading_on_large_section_returns_ok_false_with_hint_and_section_map` (whose
`# Root` is at line 1) never caught this.

The `section_map` assertion is end-to-end rather than arithmetic: it reads the
handle at the line `section_map` reports and asserts `### Sub A` is there. A test
that recomputed `h.line - start_ln + 1` and compared would pass against a fix
that got the formula wrong in the same way twice.

Gate: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
— 4456 passed, 0 failed.
## Workarounds

Ignore the numbers in `next_actions` and read the handle from its own start:
`read_markdown("@file_…", start_line=1, end_line=100)`. The rejection message is
accurate and names the valid range (`1..=202`), so the recovery is one call.

## Resume

N/A — fixed.
## References

- `docs/issues/archive/2026-08-25-run-command-nested-buffer-recursion.md` — same
  defect class (mixed coordinate frames), different mechanism.
- `docs/issues/archive/2026-08-25-file-slice-handle-refreshes-to-whole-file.md` — the
  sibling that made these excerpt handles snapshots; found in the same sweep.
- `src/tools/markdown/read_markdown.rs` — `read_markdown_single_heading`.
