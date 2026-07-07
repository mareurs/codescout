---
id: b0e3905454edcba7
kind: bug
status: open
title: 'BUG: artifact(get, full=true) silently truncates body on large trackers with no truncation indicator'
owners:
- marius
tags:
- codescout-tool
- librarian
- windows-audit-tangent
topic: null
time_scope: null
closed: null
opened: '2026-07-07'
related:
- docs/issues/2026-07-07-doctor-ads-colon-verbatim-prefix-false-positive.md
severity: med
---

## Summary

`artifact(action="get", full=true)` on a large tracker (`docs/trackers/bug-fix-session-log.md`, id `2dd9d90bc83f9f49`, 1739 real lines) returns a `body` field truncated at 499 lines / ~46.8KB with no truncation flag anywhere in the response. `preview.line_count` (returned by the same tool on `full=false`) correctly reports 1739, so the tool has the true count available and simply doesn't propagate a truncation signal on the `full=true` path.

## Symptom (Effect)

`artifact(action="get", id="2dd9d90bc83f9f49", full=true)` returns a JSON object whose `body` value ends mid-document (inside the F-6/F-7 prose section) despite the tracker file on disk continuing through F-27/W-20. No `truncated`, `body_truncated`, or similar field is present anywhere in the response to signal this.

Separately, `artifact(action="get", id="2dd9d90bc83f9f49", full=false)`'s `preview.headings` array stops at "F-7" (line 505) while `preview.line_count` correctly reports `1739` — the headings list and the line count disagree about how much of the document was actually scanned.

## Reproduction

```
git rev-parse HEAD  # a92c734fde3e5901e37b57904f5dd16f1cfc2113, branch experiments

artifact(action="get", id="2dd9d90bc83f9f49", full=true)
# → body field is 499 lines / ~46794 bytes, ends mid F-6/F-7

# Compare against the real file:
grep -oE "^## (F|W)-[0-9]+" docs/trackers/bug-fix-session-log.md | tail -1
# → ## F-27 (file continues to line ~1744)
```

Ruled out catalog staleness: `librarian(action="reindex", scope="project")` reported `updated: 10` but re-fetching `artifact(get, full=true)` for this id returned the identical 499-line / 46794-byte body — the truncation point did not move, so this is not a stale-catalog-row issue, it reproduces on freshly reindexed data.

## Environment

codescout MCP server, librarian `artifact` tool, project `codescout`, branch `experiments` @ `a92c734fde3e5901e37b57904f5dd16f1cfc2113`. Not platform-specific (unlike the sibling Windows-path bugs filed the same day) — this is a response-size behavior, observed on Linux.

## Root cause

Unknown — not yet traced to a specific `path:line` in the librarian tool's Rust source. Likely candidates (untested):
- A response-size cap applied when serializing `body` for the `get` MCP response, independent of the `output_buffer` progressive-disclosure layer (this response was itself buffered as `@tool_*`, i.e. the truncation happens *before* or *during* buffering, not as a consequence of it).
- The cap sits at a suspiciously round-ish byte count (~46.8KB) that didn't move across two identical calls and a reindex in between, suggesting a fixed limit rather than a race or cache issue.

## Evidence

Call 1 (`@tool_3cde2717`, before reindex): `"buffered_bytes": 46794`, extracted `$.body` → 499 lines.
Call 2 (`@tool_3cdf1cf9`, after `librarian(action="reindex", scope="project")` reported `updated: 10`): identical `"buffered_bytes": 46794`, identical 499-line body.
Raw-file grep (`grep -oE "^## (F|W)-[0-9]+" docs/trackers/bug-fix-session-log.md`): 47 matches, last three `## F-26`, `## W-20`, `## F-27` — confirms the real file's content extends far past what `get(full=true)` returned.

## Hypotheses tried

1. **Hypothesis:** Catalog row for this artifact is stale (body column not resynced with the on-disk file).
   **Test:** `librarian(action="reindex", scope="project")`, then re-fetch `artifact(get, full=true)`.
   **Verdict:** rejected — reindex reported 10 rows updated but this artifact's returned body was byte-for-byte identical before and after.
   **Evidence link:** see Evidence section, calls 1 and 2.

## Fix

Not investigated — out of scope for the session that found it (was auditing a Windows-path bug in `src/librarian/tools/doctor.rs` and hit this while trying to append a session-log entry to this same tracker). Filing per this repo's "capture on notice" bug-tracking convention (CLAUDE.md) rather than fixing now.

## Tests added

N/A — root cause not yet located in source, so no regression test written. A regression test should assert that `artifact(get, full=true)`'s returned body's line count matches (or the response explicitly flags a mismatch against) `preview.line_count`/the on-disk file's line count for a tracker exceeding whatever the real cap turns out to be.

## Workarounds

For large trackers, read in slices via `read_file` on the buffered `@tool_*`/`@file_*` reference (as this session did) rather than trusting a single `full=true` call to return the complete document. Cross-check suspiciously round-looking `body` lengths against `preview.line_count` when the two are available from the same or a sibling call.

## Resume

Grep the librarian `artifact` tool's `get` handler (likely `src/librarian/tools/get.rs`, per the tool-name-to-file convention used elsewhere in `src/librarian/tools/`) for any byte/line cap applied to `full=true` body serialization, and check whether it emits a truncation signal anywhere. Compare against how `preview` mode computes `line_count` (which correctly saw all 1739 lines) to find where the two code paths diverge.

## References

- `docs/trackers/bug-fix-session-log.md` (id `2dd9d90bc83f9f49`) — the tracker on which this was found; see its `## W-21` entry for the reconnaissance-skill scout that caught this before it caused an F-N/W-N ID collision.
- `docs/issues/2026-07-07-doctor-ads-colon-verbatim-prefix-false-positive.md` — the unrelated Windows-path bug being fixed in the same session when this was found.

