---
kind: bug
status: open
tags:
- cluster/unclassified
closed: null
opened: 2026-10-02
owner: marius
related: []
severity: low
---

# BUG: edit_file insert_before / insert_after with a body that has no heading line reports ok and attaches the text to the previous section

## Summary

On a markdown file, `edit_file(action="insert_before", heading=H, body=B)` is documented as adding a
*sibling section*. When `B` carries no heading line it adds no section at all: the text lands as
extra paragraphs at the end of whichever section precedes `H`. The call returns `{"status": "ok"}`
with no warning, so the caller learns nothing went wrong until they re-read the file.

## Symptom (Effect)

Text intended as a new `##` section is silently absorbed into the previous section. In this
checkout the effect was three paragraphs of a new `CLAUDE.md` rule landing in the file's intro,
under the H1, instead of under their own `## ...` heading. Nothing refused it, nothing warned, and
the file's heading map did not change, which is the only place it would have been visible.

## Reproduction

Observed twice: once on the real `CLAUDE.md` (2026-10-02, this session), then reproduced on a
scratch file. Starting file:

```
# Title

Intro paragraph.

## First

First body.

## Second

Second body.
```

Call: `edit_file(action="insert_before", heading="## Second", body="A new section body with no heading line.\n")`

Result: `{"status": "ok"}`. File afterwards:

```
## First

First body.

A new section body with no heading line.

## Second
```

The inserted text is now part of `## First`. No new heading exists.

## Environment

codescout on `experiments` at the time of writing (`1568b777`), Linux. The insert actions are
exposed by `edit_file`'s markdown grammar; the implementation is under `src/tools/markdown/`.

## Root cause

Not investigated. Only the observable behavior was checked. The schema text for `body` reads
"the section's new body text for replace/insert actions (heading preserved on replace)", and the
action description says insert adds "a sibling section". Neither says that for insert the heading
must be written into `body`, and `replace` explicitly does NOT need it. So a caller who follows
the `replace` habit (body only, heading supplied by `heading=`) gets the wrong result without any
signal. For insert, `heading` names the ANCHOR, not the new section's title.

## Evidence

The two outputs above. The scratch repro is deterministic.

## Hypotheses tried

None. Not investigated.

## Fix

Not implemented. Two options, either alone would close it:

1. **Refuse or warn** when an insert action's `body` has no leading heading line at the level of
   the anchor, naming the fix ("insert adds a sibling section; put its heading in body").
   `RecoverableError` with a hint fits the repo's error convention.
2. **Make the schema say so.** State in the `body` description that for insert the new section's
   own heading line must be included. Cheaper, and does not help a caller who does not read it.

Option 1 is the stronger one: the failing case looks exactly like success, which is this repo's
own definition of a guard that nothing observes.

## Tests added

N/A: not fixed. A regression test for option 1 should assert that a headingless body on
`insert_before` and on `insert_after` is refused, and that a body starting with a heading is not.

## Workarounds

Include the heading line as the first line of `body`. Or add the heading in a second edit. In
this session the second form was used to repair `CLAUDE.md` after the first call misfiled it.

## Resume

Start at `src/tools/markdown/` and find where `insert_before` and `insert_after` splice `body`.
Check whether the splice looks at `body`'s first line at all. Then decide between the two options
above.

## References

- `docs/issues/_TEMPLATE.md`: the bug template this file follows.
