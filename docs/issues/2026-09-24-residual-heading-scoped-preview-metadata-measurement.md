---
id: '043c453c34469482'
kind: bug
status: fixed
title: 'RESIDUAL: Measure whether envelope metadata pushes otherwise-inlinable heading-scoped sections over the 9 KB inline budget, and suppress preview.headings if it does'
tags:
- cluster/hint-composed-without-the-request
closed: 2026-10-06
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-01-heading-scoped-get-overflow-hint-points-at-metadata.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-01-heading-scoped-get-overflow-hint-points-at-metadata.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Measure whether envelope metadata pushes otherwise-inlinable heading-scoped sections over the 9 KB inline budget, and suppress preview.headings if it does.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-01-heading-scoped-get-overflow-hint-points-at-metadata.md` (status `fixed`):

> The preview.headings suppression proposed in the original Fix section was NOT measured and NOT done - whether envelope metadata pushes otherwise-inlinable sections over the 9KB inline budget is still unknown. Nothing in the shipped fix depends on it; recorded because a reader may otherwise assume the whole Fix section landed. Also: the gate's default lane showed 1 failure at commit time (tests/issue_clusters.rs::every_declared_class_has_an_index_row), which was a peer's uncommitted work on issue-clusters.md - three classes declaring a Slug with no Index row yet, all three verified absent from HEAD - and not this change.

## Fix

**Status: fixed (superseded).** The suppression this residual asked for ("suppress preview.headings if it does") was built by a sibling bug filed the same day as the parent, `docs/issues/archive/2026-09-01-a-scoped-read-is-billed-the-full-heading-map.md`, in three commits on 2026-09-01: `f3a76f81`, `aee9dd6b`, `b9bcfee4` (patch-ids under Fix provenance; each equals the one that sibling recorded). The parent's caveat ("NOT measured and NOT done") predates that fix's closeout and was never updated.

Verified at the bytes on 2026-10-06 in `src/librarian/tools/get.rs`:

- `body_selected` (line 638) is true for `full`, `heading`, `headings`, `start_line` or `end_line`; `stub_this_preview` starts as `body_selected` (line 706) and is set back to `false` only when a requested heading misses or is ambiguous (lines ~718, ~763).
- `out["preview"] = if stub_this_preview { stub_preview(&preview) } else { preview }` (lines 843-847).
- `stub_preview` (line 209) replaces the `headings` array with `HEADINGS_OMITTED_NOTE` ("omitted (body selector present) — call doc(get, id=…) with no body selector for the map"), drops `summary`, and backfills `total_headings`; a preview with no `headings` array (e.g. `memory`) passes through untouched.
- Tests in the same file: `stub_preview_backfills_total_headings_under_the_cap`, `stub_preview_still_strips_a_default_shape_with_a_headings_array`, `stub_preview_passes_through_a_shape_with_no_headings_array_untouched`, and the `HEADINGS_OMITTED_NOTE` assertions around lines 1102, 1642 and 1774.

One live sample, 2026-10-06 (not the optional 9 KB measurement): `doc(get, id=<this file>, heading="## Fix")` returned a 226-byte body in a 1752-byte pretty-printed response whose `preview` was `{shape, headings: <note>, line_count, total_headings}`, 160 bytes. That is one small file and says nothing about a section near the 9 KB inline budget.

What remains is the residual's measurement half, NOT run here: how much of the inline budget the remaining envelope (`body_meta`, the stub preview, `extra`, `provenance`) takes on a heading-scoped read of a section just under 9 KB, and whether that pushes it over. With `preview.headings` stubbed, the part of that overhead that scaled with the document (the heading map, which the comment above `stub_preview` in `get.rs` puts at up to ~2,400 bytes) is gone, so the question is now about a fixed-size remainder. No code change was made in this sweep.

## Tests added

None in this sweep. The stub is covered by tests added in the three commits above (see Fix for names).

## Fix provenance

- **SHA:** `f3a76f81` (`experiments`)
- **patch-id:** `69d5fda78f7fcaa292f0b1fcc419bd4bd50cefef`
- **SHA:** `aee9dd6b` (`experiments`)
- **patch-id:** `08bba53a99703b16f21aaed1d051fe0566d74e9f`
- **SHA:** `b9bcfee4` (`experiments`)
- **patch-id:** `6102147d08b38d0dbad1f4ee72984870335846ac`

## Resume

Closed on 2026-10-06 as superseded by `docs/issues/archive/2026-09-01-a-scoped-read-is-billed-the-full-heading-map.md`; nothing to resume. Residual follow-up, listed and not filed: the optional 9 KB measurement above (a heading-scoped read of a section just under the inline budget, to see whether the fixed envelope remainder still tips it into a buffer).

## References

- `docs/issues/archive/2026-09-01-heading-scoped-get-overflow-hint-points-at-metadata.md` — parent
