---
status: fixed
opened: 2026-05-26
closed: 2026-05-26
severity: medium
owner: marius
related: [2026-05-26-edit-markdown-insert-after-fuses-heading]
tags: [edit_markdown, markdown, scoped-edit, silent-corruption, class-a-fusion]
kind: bug
---

# BUG: edit_markdown scoped edit (action="edit") fuses section onto the following heading

## Summary
Sibling of `2026-05-26-edit-markdown-insert-after-fuses-heading` — same Class-A
fusion root cause, different function. `perform_scoped_edit` (the `edit` action)
builds its result with `format!("{}{}{}", before, new_section, after)`. When the
caller's `old_string` consumes the section's trailing newline and `new_string`
does not restore it, `new_section` loses its trailing `\n` and fuses directly
onto the following heading, silently demoting it to body text.

## Symptom (Effect)
```
edit_markdown(action="edit", heading="## A",
              old_string="old line\n", new_string="new line")
```
on `"## A\nkeep\nold line\n## B\nbody\n"` produced:
```
## A
keep
new line## B
body
```
`## B` is no longer at line-start → parsed as body text. Tool returned `"ok"`.

## Reproduction
At commit on `experiments` (see Fix for SHA). Direct call:
```rust
let content = "## A\nkeep\nold line\n## B\nbody\n";
let result = perform_scoped_edit(content, "## A", "old line\n", "new line", false).unwrap();
// result == "## A\nkeep\nnew line## B\nbody\n"  — fused.
```
Trigger requires `old_string` to include the trailing newline and `new_string`
to omit it — less common than the insert_after trigger (any newline-less
content), hence severity medium rather than high.

## Environment
codescout v0.14.0, Linux. Pure markdown string manipulation, no LSP.

## Root cause
`perform_scoped_edit` (`src/tools/markdown/edit_markdown.rs`) builds
`section_text` with a synthetic trailing newline (`format!("{}\n", ...)`), then
applies `.replace(old_string, new_string)`. If `old_string` includes that
trailing `\n` and `new_string` omits it, the resulting `new_section` ends
without a newline. The final splice `format!("{}{}{}", before, new_section,
after)` then concatenates `new_section` directly with `after` (the next
heading). `heading_level` (`src/tools/file_summary/file_summary.rs`) recognizes
a heading only at line-start, so the fused `new line## B` parses as body.

Same mechanism as the insert_after bug: a string-concatenation builder placing
caller content adjacent to a heading without guaranteeing a separating newline.
Line-vector builders (`edit_code` via `write_lines`) are immune because they
always `\n`-join elements.

## Evidence

### Red reproduction test output (pre-fix)
```
scoped edit fused onto the following heading: "## A\nkeep\nnew line## B\nbody\n"
```

## Hypotheses tried
1. **Hypothesis:** scoped edit shares the insert_after Class-A fusion.
   **Test:** traced `section_text` construction + `.replace` + final splice;
   wrote a red test with `old_string="old line\n"`. **Verdict:** confirmed —
   reproduces the fusion.

## Fix
Wrap `new_section` in `ensure_trailing_newline(&new_section)` before the final
splice, matching the insert_after fix and the `replace` / `insert_before` arms:

```rust
let result = format!("{}{}{}", before, ensure_trailing_newline(&new_section), after);
```

Change in `src/tools/markdown/edit_markdown.rs` (`perform_scoped_edit`). Fix on
`experiments`; **master-side SHA to be recorded after cherry-pick** (CLAUDE.md
§ "After cherry-pick").

## Tests added
`scoped_edit_consuming_trailing_newline_preserves_following_heading`
(`src/tools/markdown/tests.rs`) — old_string consumes the trailing newline,
asserts no fusion and that `## B` survives in the parsed heading map. All 114
markdown tests pass; fails on pre-fix code (red → green).

## Workarounds
Don't include the trailing newline in `old_string` for scoped edits, or always
re-read the heading map after the edit. (Not needed post-fix.)

## Resume
N/A — fixed.

## References
- `src/tools/markdown/edit_markdown.rs` — `perform_scoped_edit`, `ensure_trailing_newline`
- `docs/issues/2026-05-26-edit-markdown-insert-after-fuses-heading.md` — sibling instance, same root cause
- `src/tools/markdown/tests.rs` — regression test
