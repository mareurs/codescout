---
status: open
opened: 2026-05-21
closed:
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
Unknown — under investigation. Hypothesis: `edit_markdown`'s heading-range
parser drops the final heading (likely an off-by-one or EOF-boundary condition
where the last section has no following sibling to delimit it), while
`read_markdown`/`parse_all_headings` includes it. Needs a side-by-side of the
two heading enumerators.

## Evidence
The error string above (edit_markdown) vs the concurrent read_markdown result
on `docs/trackers/output-form-text-compaction.md`:

```
"lines": 68, "hint": "68 lines, 7 sections — ..."
```

read_markdown counts 7 sections (incl. `## Resume`); edit_markdown's hint lists
only 6 (omits `## Resume`).

## Hypotheses tried
1. **Hypothesis:** stale heading cache in edit_markdown. **Test:** re-read with
   read_markdown (fresh), retried edit_markdown batch. **Verdict:** rejected —
   second attempt failed identically; read showed the heading present.
2. **Hypothesis:** `## Done log` section range absorbs the `## Resume` body, so a
   scoped `edit` on "Done log" could reach it. **Test:** `edit` on "Done log"
   with old_string = Resume paragraph. **Verdict:** rejected — "old_string not
   found in section 'Done log'", so Done log is correctly bounded before Resume;
   Resume is genuinely unreachable.

## Fix
Not yet fixed. Likely in the edit_markdown heading-enumeration/range code
(`src/tools/markdown/` — the editor path, distinct from `read_markdown.rs`).
Align it with `parse_all_headings` so the last heading is always addressable.

## Tests added
N/A — not yet fixed. When fixed, add a regression: a file whose last section is
an H2, assert `edit_markdown(heading=<last>, action="edit", ...)` succeeds.

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
