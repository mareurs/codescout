---
status: open
opened: 2026-05-09
closed:
severity: medium
owner: marius
related: []
tags: ["edit_markdown", "heading-insert", "semantics"]
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

The "insert_after a heading" semantic targets the END of that heading's section, not the line immediately following the heading. For an H1 spanning the whole doc, the section ends at EOF.

## Evidence

Direct observation in multiple session edits where new content was discovered at EOF rather than the expected position.

## Hypotheses tried

*N/A — migrated from compact form; root cause confirmed by inspecting the semantic without deeper investigation.*

## Fix

Open. No commit yet. Candidate approaches:

1. Add an `at="heading_line"` mode that inserts immediately after the heading line, leaving the existing `"section_end"` semantic intact for nested headings.
2. Change the default to "heading_line" for H1s and document the asymmetry.
3. Document the existing behavior more clearly and direct callers to `action="edit"`.

## Tests added

N/A — open.

## Workarounds

Use `action="edit"` with `old_string` matching the heading + the next non-blank line; place the new content between them in `new_string`.

## Resume

Decide between adding an `at=` mode (option 1 above) or changing the H1 default (option 2). Concrete next action: open `src/tools/markdown/edit_markdown.rs`, locate the `insert_after` dispatch, audit current behavior with a unit test, then pick the path with the smallest blast radius for existing callers.

## References

- Originally tracked as **#1** in `docs/issues/bug-tracker.md` (retired after migration to per-file system).
