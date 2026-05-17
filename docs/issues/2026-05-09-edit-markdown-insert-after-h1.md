---
status: fixed
opened: 2026-05-09
closed: 2026-05-17
severity: medium
owner: marius
related: []
tags: ["edit_markdown", "heading-insert", "semantics"]
kind: bug
---

# BUG: `edit_markdown insert_after` on H1 places content at section end (EOF) not after the heading line

## Summary

`edit_markdown(action="insert_after", heading="# Title")` on a top-level H1 inserts content at the END of the file (EOF area), not immediately after the heading line. For files where the H1 wraps the entire document, the content lands at the bottom. Defensible interpretation of "insert after section" for nested headings but counter-intuitive for top-level wrappers.

## Symptom (Effect)

Caller expects:

```markdown
# Title
<new content here>

(original content)
```

Caller actually gets:

```markdown
# Title

(original content)
<new content here>
```

## Reproduction

Any markdown file with a top-level `# Title` and content below. Call `edit_markdown(path, action="insert_after", heading="# Title", content="X")` and inspect.

## Environment

- Date observed: 2026-05-09
- Tool: `mcp__codescout__edit_markdown` (`action="insert_after"`)

## Root cause

`compute_section_end` walks forward from the heading until it finds another heading of level ≤ the target heading's level, returning that index (or `lines.len()`/EOF if none found). For a sole top-level H1 wrapping the whole document, no other H1 exists below, so `compute_section_end` returns `lines.len()` and `insert_after` lands at EOF.

This is the correct semantic for nested H2/H3 sections (you want content after the whole subsection ends). It's an API surface mismatch only for whole-doc-wrap H1: callers expect "after the heading line" but get "after the section ends".
## Evidence

Direct observation in multiple session edits where new content was discovered at EOF rather than the expected position.

## Hypotheses tried

*N/A — migrated from compact form; root cause confirmed by inspecting the semantic without deeper investigation.*

## Fix

Additive `at` parameter on `edit_markdown` (Option 1 from the bug-file). Values:

- `at="end-of-section"` (default) — preserves the existing semantic. No breakage for current callers.
- `at="after-heading-line"` — inserts content immediately after the heading line.

Implementation: split `perform_section_edit` into a `#[cfg(test)]` 4-arg wrapper (preserves the test-suite signature, 45 existing test callers untouched) and a new `perform_section_edit_ext` that accepts `at: Option<&str>` and is called by `EditMarkdown::call` in both single-edit and batch modes. Schema gains `at: enum["end-of-section", "after-heading-line"]` at both the top-level and inside the `edits[]` array items.

Invalid `at` values surface as a `RecoverableError` naming the bad value.
## Tests added

Four new tests in `src/tools/markdown/tests.rs`:

- `insert_after_h1_default_appends_at_end_of_section` — regression pin for the existing end-of-section behavior on a sole-H1 doc.
- `insert_after_h1_with_at_after_heading_line_inserts_right_after_heading` — confirms the new `at="after-heading-line"` mode inserts the content directly after the heading line.
- `insert_after_with_explicit_end_of_section_matches_default` — confirms `at="end-of-section"` is equivalent to omitting `at`.
- `insert_after_invalid_at_value_errors` — confirms `at="nonsense"` returns a `RecoverableError` naming the invalid value.

All pass. `cargo clippy --all-targets -- -D warnings` clean. `cargo test --lib` 2336 passed / 7 ignored.
## Workarounds

Use `action="edit"` with `old_string` matching the heading + the next non-blank line; place the new content between them in `new_string`.

## Resume

Closed. To insert after an H1 line, callers pass `at="after-heading-line"`. The default remains end-of-section so all existing callers (chiefly nested H2/H3 inserts) are unaffected.
## References

- Originally tracked as **#1** in `docs/issues/bug-tracker.md` (retired after migration to per-file system).
