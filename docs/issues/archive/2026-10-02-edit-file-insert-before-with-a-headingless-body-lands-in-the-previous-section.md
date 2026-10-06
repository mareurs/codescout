---
kind: bug
status: fixed
tags:
- cluster/unclassified
closed: 2026-10-06
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

Established while fixing it (`46e26fa3`): the insert splice lands at the anchor heading's line start, which is the END of the preceding section, so a body that does not open with its own heading line adds no section and its text joins whatever precedes the anchor. (Earlier text of this section: only the observable behavior had been checked.) The schema text for `body` reads
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

Option 1 was implemented, for `insert_before` only. `insert_after` was deliberately NOT changed, although the bug text asks for it too. A headingless `insert_after` body with the default `at="end-of-section"` lands at the end of the TARGET section, which is the documented append-to-section use, and the new refusal hint itself sends callers there; refusing it would break that use and would not stop any misfiled text, since the text goes where the caller pointed. `insert_before` has no such use, because its splice point is the end of a different section from the one named. `plan_section_edit` in `src/tools/markdown/edit_markdown.rs` now refuses an `insert_before` whose first non-blank body line is not an ATX heading (`opens_with_heading`, which reuses `heading_level`, so `#hashtag` and seven hashes do not count), with a `RecoverableError` whose hint shows a heading-led corrected call. Single edits, `edits[]` batches and `doc(update, body_edits)` all route through `plan_section_edit`, so all three are covered. Option 2 was applied as well: the `body` schema description in `src/tools/edit_file/mod.rs` states that insert_before's body must start with its own heading line, and the `LONG_DOCS` table says the same. To keep the tool-surface byte budget unchanged (the budget was not raised), an equal sentence, "'edit' performs scoped text replacement within the target section.", was removed from the `action` description.

## Tests added

In `src/tools/markdown/tests.rs`:

- `a_headingless_insert_before_is_refused_and_writes_nothing` pins the single-edit refusal and that the file is unchanged.
- `a_headingless_insert_before_in_a_batch_is_refused_and_writes_nothing` pins the same for an `edits[]` batch.
- `an_insert_before_with_a_leading_heading_still_adds_the_section` pins that a heading-led body is still accepted and adds the section.
- `insert_before_heading_detection_uses_the_first_non_blank_line` unit-tests the heading predicate.
- `a_headingless_insert_after_still_appends_to_the_section` pins that `insert_after` is unchanged.

Four existing tests that passed headingless `insert_before` bodies were changed to heading-led bodies: three in `src/tools/markdown/tests.rs` (`batch_coincident_insert_and_span_is_order_independent`, `plan_section_edit_insert_after_and_remove_match_legacy`, `every_advertised_batch_action_actually_dispatches`) and one in `src/librarian/tools/update.rs`. No dedicated test asserts the refusal through `doc(update, body_edits)`; that path is covered by sharing `plan_section_edit`.

## Fix provenance

- **SHA:** `46e26fa3` (`experiments`)
- **patch-id:** `cd9242bf78375eeb081746d877380dd600389421`

## Workarounds

Include the heading line as the first line of `body`. Or add the heading in a second edit. In
this session the second form was used to repair `CLAUDE.md` after the first call misfiled it.

## Resume

Closed on 2026-10-06 by `46e26fa3` (local on `experiments`, not pushed at the time of writing), for `insert_before` only; `insert_after` is unchanged on purpose (see Fix). Residual follow-ups, listed and not filed: (1) the wrappers re-wrap this error with the generic hint "Check heading name and action." (seen in `src/librarian/tools/update.rs`'s body_edits path), so the structured `hint` field reads generic for this refusal; the specific corrected-call hint is not what that field carries. (2) There is no dedicated `doc(update, body_edits)` test of the refusal.

## References

- `docs/issues/_TEMPLATE.md`: the bug template this file follows.
