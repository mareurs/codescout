---
status: fixed
opened: 2026-05-21
closed: 2026-05-21
severity: low
owner: marius
related: []
tags: [edit_markdown, markdown, tool-quirk]
kind: bug
---

# BUG: edit_markdown cannot address the last heading in a file

## Summary
`edit_markdown` refuses to target the final `##` section of a file ("heading
not found"), even though `read_markdown` lists that exact heading in its
heading map. The two markdown tools disagree on the set of addressable
headings, with the last one missing only from `edit_markdown`.

## Symptom (Effect)
On `docs/trackers/output-form-text-compaction.md` (file ended with a `## Resume`
section), every `edit_markdown` action targeting `Resume` failed:

```
edits[2]: heading 'Resume' not found — hint: Available headings:
# OutputForm::Text compaction sweep, ## Root cause, ## Classification rule,
## Candidates, ## Proposed guard, ## Done log
```

The available-headings hint stops at `## Done log` — the final `## Resume`
heading is absent. A `read_markdown` call on the same file at the same moment
returned `"7 sections"` and listed `## Resume` in its map.

## Reproduction
1. Create a markdown file whose **last** section is an H2 with body text and a
   trailing newline.
2. `read_markdown(path)` → heading map includes the last heading.
3. `edit_markdown(path, heading="<last heading>", action="edit", old_string=...,
   new_string=...)` → fails with "heading not found".
4. Scoped `edit` against the *second-to-last* heading also cannot reach the last
   section's body (its section range is correctly bounded before the last
   heading), so the last section is fully unreachable via edit_markdown.

Observed live this session; not yet reduced to a unit test. codescout build
v0.13.0, MCP transport, project code-explorer, branch `experiments`.

## Environment
codescout v0.13.0, release MCP server, Linux. Both markdown tools share
`parse_all_headings` (`src/tools/file_summary/file_summary.rs`) for read; the
edit path uses its own heading enumeration/range logic — that is the suspected
divergence point.

## Root cause

`parse_all_headings` (and the parallel `compute_section_end` in `edit_markdown.rs`) used a naive
fence-toggle: every line starting with ``` flipped `in_code_block`. When a batch `edit_markdown`
call's intermediate buffer contained an **unmatched** ``` (e.g. an `insert_after` whose
`new_string` opened a fence without closing it), the toggle stayed `true` to EOF, hiding every
heading that followed — including the final `## Resume`. CommonMark-conformant, but brittle for
an editor tool whose in-flight buffer can transiently hold unbalanced fences between batched
edits.

The bug file's original "EOF off-by-one in heading enumeration" hypothesis was wrong. The two
markdown tools really do share `parse_all_headings`; the divergence was in the **input**:
`read_markdown` re-reads from disk (balanced fences); `edit_markdown` parses the in-memory
buffer after each batch step (potentially unbalanced).
## Evidence
The error string above (edit_markdown) vs the concurrent read_markdown result
on `docs/trackers/output-form-text-compaction.md`:

```
"lines": 68, "hint": "68 lines, 7 sections — ..."
```

read_markdown counts 7 sections (incl. `## Resume`); edit_markdown's hint lists
only 6 (omits `## Resume`).

## Hypotheses tried

1. **Hypothesis:** stale heading cache in edit_markdown. **Test:** re-read with read_markdown
   (fresh), retried edit_markdown batch. **Verdict:** rejected.
2. **Hypothesis:** `## Done log` section range absorbs the `## Resume` body. **Test:** scoped
   `edit` on "Done log" with old_string = Resume paragraph. **Verdict:** rejected (Done log
   correctly bounded).
3. **Hypothesis:** EOF off-by-one in heading enumeration; edit_markdown's heading parser drops
   the final heading. **Test:** invoked edit_markdown live against the on-disk file with 7
   balanced headings. **Verdict:** rejected — last heading resolved fine. This was the bug
   file's stated root cause; it was wrong.
4. **Hypothesis:** the bug surfaced on a transient mid-batch buffer where an earlier edit
   left an unmatched ``` fence open. **Test:** unit test with explicit unbalanced fence +
   trailing heading. **Verdict:** confirmed — every heading after the open fence disappears.
   Real root cause.
## Fix

Two-line pre-scan in both `parse_all_headings` (`src/tools/file_summary/file_summary.rs`) and
`compute_section_end` (`src/tools/markdown/edit_markdown.rs`): count ``` lines first; if the
count is odd, treat every ``` line as plain text instead of as a fence boundary. The fence
toggle path is only active when fences are balanced.

Trade-off: a file with a genuinely unclosed code fence + a heading-shaped line below it
(`# looks like heading`) now exposes that line as a real heading. For an editor tool this
is the right call — silent heading-hiding is the worse failure mode (was: invisible
last-heading; now: surfaceable real heading the user can then close the fence around).

Commit SHA: `0e73ebbb`.
## Tests added

`src/tools/file_summary/tests.rs`:

- `resolve_section_range_last_h2_in_multi_heading_doc` — the bug-file shape (7 headings, last
  is `## Resume`); confirms the last heading resolves both with and without the `## ` prefix.
- `resolve_section_range_last_heading_with_unbalanced_code_fence` — the actual repro: an
  unmatched ``` followed by `## Hidden` and `## Resume`. All 4 headings must be visible.
- `resolve_section_range_balanced_code_fence_still_masks_inner_headings` — regression guard
  for the balanced case (don't surface heading-shaped lines inside real fenced blocks).

`src/tools/markdown/tests.rs::unclosed_code_fence` — updated to assert the new editor-friendly
behavior with an explanatory comment recording the behavior change.
## Workarounds
Rewrite the whole file via `create_file(overwrite=true)` (used this session), or
ensure the section you need to edit is never the last one (append a trailing
sentinel heading). Both sidestep the buggy enumerator.

## Resume
Diff the heading enumeration used by the edit_markdown path against
`parse_all_headings` in `src/tools/file_summary/file_summary.rs`. Build a
minimal file ending in an H2 with body + trailing newline; trace why the final
heading is excluded from the editor's addressable set. Likely an EOF/sibling
delimiter off-by-one.

## References
- `docs/trackers/output-form-text-compaction.md` — file where this surfaced.
- `src/tools/markdown/read_markdown.rs`, `src/tools/file_summary/file_summary.rs`.
