---
id: 81fc1c9337926953
kind: bug
status: fixed
title: 'RESIDUAL: Make the heading-ambiguity error report file-relative line numbers (Fix step 4), and dedupe the two ''### BL-43'' definitions in open-issue-work-queue.md'
tags:
- cluster/addressing-without-an-escape-hatch
closed: 2026-10-06
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-27-identical-headings-make-a-section-permanently-unaddressable.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-27-identical-headings-make-a-section-permanently-unaddressable.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Make the heading-ambiguity error report file-relative line numbers (Fix step 4), and dedupe the two '### BL-43' definitions in open-issue-work-queue.md.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-27-identical-headings-make-a-section-permanently-unaddressable.md` (status `fixed`):

> Fix step 4 (the file-relative vs body-relative line-number frame in the ambiguity error) was deliberately NOT done and remains open. A second, unrelated defect is left unrepaired by choice: open-issue-work-queue.md defines ### BL-43 twice, and choosing the authoritative copy is a content judgement about another work stream.

## Fix

Both halves of the work were overtaken by earlier commits; verified at the bytes on 2026-10-06.

**(a) BL-43 dedupe: done in `78dc0886`** (2026-08-27, "BL-43 had two definers; demote the second, keep the measurement"). The commit demoted the second `### BL-43` heading to a nested `#### Measurement (2026-08-18) — recorded while BL-43 was still open` rather than deleting it, so the measurement survives. At HEAD `docs/trackers/open-issue-work-queue.md` has exactly one `### BL-43` heading (line 1425; `grep -c '^### BL-43'` returns 1) and the demoted heading at line 1431. The parent caveat's "choosing the authoritative copy is a content judgement" was settled by the commit message's finding that the two copies were two successive states of one entry.

**(b) Line-number frame: the frames agree.** Fix step 4 asked that the ambiguity error report file-relative lines. The ambiguity message and its `occurrences` extra are built in `src/tools/file_summary/file_summary.rs` (`dup_error` inside `resolve_section_range`, lines ~345-367) from `headings[i].line`, i.e. relative to whatever string the caller passes. Each caller now passes, or converts to, the file frame:

- `doc(update)` `body_edits`: `src/librarian/tools/update.rs` reads the whole file (`original = std::fs::read_to_string(&full)`, line 573), sets `working = original.clone()` (line 652) and runs `apply_body_edits` on that, frontmatter included, so the lines in the ambiguity message are file-relative by construction.
- `doc(get)`: `resolve_section_range` runs on the frontmatter-stripped body, so its `occurrences` are body-relative; `src/librarian/tools/get.rs` `heading_miss_meta(name, err, line_offset)` (lines ~75-100) adds `line_offset` to each, giving file-relative numbers. That was the sibling bug `docs/issues/archive/2026-08-31-artifact-get-line-numbers-are-body-relative-not-file-relative.md`, fixed in `d26d3cd6` (its own record cites that SHA). `f237394e` is the later commit that archived that bug file and re-pointed its citations (6 lines in `src/librarian/tools/get.rs`, all doc-comment paths), not the code change. Tests: `ambiguous_heading_occurrences_are_file_relative` and `heading_map_lines_are_file_relative_not_body_relative` in `src/librarian/tools/get.rs`.
- `edit_file` markdown: operates on the file as given, so file-relative.

The verification of the `update` arm is by reading the code, not by running a call; the `apply_body_edits` tests I found (`body_edits_without_occurrence_still_refuses_identical_headings`) use a body without frontmatter and assert only `found 2 times`, so no test pins the update-side line numbers against a frontmatter-bearing file.

Two items a triage pass raised were checked and are NOT closed here:

- `src/tools/file_summary/file_summary.rs` line 358 still words the message `heading '…' found N times (lines a, b)`, with no label saying the numbers are file-relative. Cosmetic.
- The suspicion that `apply_body_edits`' `.map_err` for replace/insert (`update.rs` ~381-386, hint "Check heading name and action.") discards the `occurrence=` hint: refuted for the TEXT, by reading. `RecoverableError`'s `Display` appends `" — hint: <text>"` after the message (`src/tools/core/types.rs` ~838-846), and the arm formats `body_edits[{i}]: {e}`, so the `Pass occurrence=N …` sentence rides in the message. What that arm does drop is the structured data (`heading_ambiguous` / `occurrences` extras), and it appends a second, generic hint. The `edit` arm goes through `prefix_scoped_error`, which preserves the rich error. Not exercised at runtime.

## Tests added

None in this sweep; both halves shipped earlier. The line-frame coverage for `doc(get)` is the two tests named under Fix (added by `d26d3cd6`). The BL-43 half is a document edit and has no test.

## Fix provenance

- **SHA:** `78dc0886` (`experiments`)
- **patch-id:** `2fb3139580c1ae35276b4e4c0687a7b67af8d38c`
- **SHA:** `d26d3cd6` (`experiments`)
- **patch-id:** `c9c089987a516572054fb1c81292b47b3427e50b`

## Resume

Closed on 2026-10-06; nothing to resume. Residual follow-ups, listed and not filed: (1) label the line numbers in the `resolve_section_range` ambiguity message (`src/tools/file_summary/file_summary.rs` ~358) as file-relative; cosmetic. (2) A test asserting file-relative lines in the `doc(update)` `body_edits` ambiguity error against a frontmatter-bearing file, and a runtime check of what a replace/insert-arm ambiguity error returns (message carries the `occurrence=` hint; `extra` is dropped). (3) `f237394e` was the SHA suggested for the line-frame half; it is the archive/doc commit, and the code is `d26d3cd6`, so `d26d3cd6` is the one recorded above.

## References

- `docs/issues/archive/2026-08-27-identical-headings-make-a-section-permanently-unaddressable.md` — parent
