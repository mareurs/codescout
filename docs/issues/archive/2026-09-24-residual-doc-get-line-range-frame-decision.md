---
id: b1abe1bb34d75c8b
kind: bug
status: fixed
title: 'RESIDUAL: Decide whether to make doc(get)''s start_line/end_line file-relative (a breaking change to a tested contract) and implement if accepted'
tags:
- cluster/addressing-without-an-escape-hatch
closed: 2026-10-30
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-31-artifact-get-line-numbers-are-body-relative-not-file-relative.md
severity: low
unverified: 'Checked live on the rebuilt server, 2026-10-30, Linux: a heading line from grep returned that heading; start_line 1 was refused with the new message; two doc(find) hits (360ba099bdb3c1e5, 29b9d923faeaa232) fed their matched.start_line and end_line to doc(get) and returned exactly the snippet find showed. That is two hits by hand, not a regression test. Not tested: macOS and Windows, CRLF files, and a caller that still counts from the body: such a caller gets other lines with no error unless its number lands inside the frontmatter. The usage log hides calls with empty input, so the measured caller count is a floor. The probe site_get in src/librarian/tools/read_door_tests.rs is filed as 4ffa75bf5548c47e.'
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-31-artifact-get-line-numbers-are-body-relative-not-file-relative.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Decide whether to make doc(get)'s start_line/end_line file-relative (a breaking change to a tested contract) and implement if accepted. Decided and done on 2026-10-30, see ## Fix.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-31-artifact-get-line-numbers-are-body-relative-not-file-relative.md` (status `fixed`):

> start_line/end_line composability with grep/link_scan remains unfixed (deliberately, per F-128) — only headings/occurrences are file-relative now

## Fix

Fixed in `e9eaf5f26` (`experiments`), patch-id `f36dc893e23baedf7762ea42fbe6f65c25173c42`. DECIDED BY THE USER ON 2026-10-30: option A of A (make the range file-relative), B (keep, close as mitigated) and C (opt-in `line_frame`). BREAKING.

`start_line`/`end_line` now count FILE lines, the frame that `grep`, `link_scan`, `preview.headings[].line` and `doc(find)`'s `matched.start_line` use. A line outside the body (inside the frontmatter, or past the end) is refused. The refusal names the valid span, `frontmatter_lines`, and tells a caller that still counts from the body to add `frontmatter_lines`. The `body_meta` echo and the soft-cap hint use the same frame. The tool schema says FILE line.

**Why A, from measurement.** At HEAD the range was still body-relative. A later guard refused only a number beyond the body. A usage-log count over 2026-10-16 to 2026-10-30 (read-only, by a subagent, with the held text checked against the returned slice) found four calls in one session (0ff90aa2, rows 189666 to 189669) that held file-relative numbers from a `grep` and got slices shifted by 19 lines, with no error. Two more calls were refused for the same reason (rows 189823 and 217382, logged with empty input). Every count is a floor: 415 of 1,012 `doc` rows in the window carry no input.

**Tests.** `line_slice_returns_requested_range` and `a_range_at_the_first_body_line_returns_it_not_the_blank_separator` were changed to the new frame. `a_line_outside_the_body_is_refused_naming_the_valid_span` replaces the old refusal test. `a_heading_maps_line_fed_back_as_start_line_returns_that_heading` is new. All four failed on the old code for the right reason (`L5` for `L1`, `## Beta` for `## Alpha`, a frontmatter range answered with body text) before the change.

**Gate.** `./scripts/gate.sh` on an isolated worktree holding only this change: `PYTHON_LIGHT=0 FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`. A first run failed `tool_surface_under_budget` (my longer schema text; fixed by shortening it) and `a_timed_out_filter_with_output_over_the_limit_stays_inline`, which passed alone and in the second full run; I did not find its cause, so I do not call it a proven flake.

## Fix provenance

- **SHA:** `e9eaf5f26` (`experiments`)
- **patch-id:** `f36dc893e23baedf7762ea42fbe6f65c25173c42`

## References

- `docs/issues/archive/2026-08-31-artifact-get-line-numbers-are-body-relative-not-file-relative.md` — parent
