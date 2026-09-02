# `edit_file` Miss Diagnostics

> ⚠ **Unreleased — on the `experiments` branch only.** Not in v0.15.0 and not on
> crates.io; the API may change without notice. The full cohort is listed under
> `[Unreleased]` in
> [CHANGELOG.md](https://github.com/mareurs/codescout/blob/experiments/CHANGELOG.md).

When an `action="edit"` `old_string` fails to match, the error says *why* rather
than only *that*.

## Why this exists

"old_string not found" leaves an agent with no move except to re-read the file
and guess again — and the two most common causes need opposite responses. If the
text drifted since it was read, re-reading is right. If the text is byte-wrong
in an invisible way (a tab where spaces were assumed, a non-breaking space, a
trailing space), re-reading returns the same bytes and the retry fails
identically. Telling those two apart is the whole point.

## The three tiers

The closest window in the target section is scored for similarity, then
classified. The tier is reported in `extra["scoped_miss_tier"]` so a caller can
route on it programmatically.

| Tier | Meaning | Hint |
|---|---|---|
| `whitespace_invisible` | Closest line differs *only* in whitespace or invisible characters | Copy the exact bytes shown for `have` |
| `visible_drift` | Closest text differs visibly — it probably changed since you read it | Re-read this section, then retry with the current value |
| `no_close` | Nothing in the section is close | Verify the heading, or re-read and retry |

For `whitespace_invisible`, the classification names what differs, and both
`want` and `have` are rendered with whitespace made visible — otherwise the
message would print two strings that look identical.

For `visible_drift`, `want` and `have` are shown as-is, truncated.

Either way, if the closest window sits inside a fenced code block, the message
adds a note that whitespace there is significant.

## Degradation

Very large inputs — an oversized `old_string`, or a section past the line/byte
caps — skip the scoring and fall back to `no_close`. The diagnostic is a
convenience on the error path, and is not worth an expensive scan on a section
big enough to make it slow.

## It survives into artifacts

The same diagnostic reaches `doc(action="update", patch={body_edits: […]})`,
so editing a managed tracker gets the same explanation as editing a plain
markdown file — the tiers do not stop at the tool boundary.

## Batch safety

Batch edits gained two related guarantees: overlapping edits are detected and
rejected up front, and non-overlapping ones are applied end-to-start. Applying
top-to-bottom would invalidate every later edit's offsets as soon as the first
one changed the document's length.

## Where this lives

`src/tools/markdown/edit_markdown.rs` — `diagnose_scoped_miss` builds the
error; `classify_whitespace_diff`, `render_visible_whitespace` and
`line_in_code_block` are the classifiers it uses.

## Related

- [Document Section Editing](document-section-editing.md)
- [Markdown Tools](markdown-tools.md)
