---
id: '65d727548a2b125a'
kind: bug
status: fixed
title: 'BUG: artifact(get, full=true) silently truncates body on large trackers with no truncation indicator'
owners:
- marius
tags:
- codescout-tool
- librarian
- windows-audit-tangent
- cluster/capped-result-presented-as-complete
closed: 2026-07-10
opened: 2026-07-07
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

Not a missing signal — a **buried** one. `artifact(get, full=true)` *does* cap the
body at `SOFT_CAP_LINES` (500) in `apply_soft_cap` (`src/librarian/tools/get.rs:40`)
and emits a loud sibling `overflow` object (`shown_lines`/`total_lines`/`hint`) plus
`body_meta.source_line_count` (`get.rs:456,480-489`). **But** any body over the
500-line cap also exceeds the 10 KB inline budget, so the whole `get` response is
buffered and `Tool::call_content` (`src/tools/core/types.rs:618-621`) substitutes a
generic `"Result stored in @tool_X (N bytes)"` summary — because `LibrarianAdapter`
(`src/librarian/adapter.rs`) never overrode `format_compact`. The truncation warning
was generated, then discarded at the progressive-disclosure boundary, in exactly the
case (large body) it exists to cover. `read_file(json_path="$.body")` then faithfully
returns the already-capped ~500 lines (`read_from_buffer`, `src/tools/read_file.rs`)
with no back-reference to `$.overflow`, so the guided flow never surfaces the cut.
The two reporters each extracted `$.body` and missed the sibling `$.overflow`.
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

Fixed at the summary layer (`src/librarian/adapter.rs`): added
`LibrarianAdapter::format_compact` delegating to a new free fn
`librarian_compact_summary(inner_name, result)`. When an `artifact` response carries
an `overflow` object, the compact summary that survives buffering now reads
`"artifact body TRUNCATED — only N of M lines are in $.body …"` with narrower-selector
guidance. `output_id`/`hint`/`buffered_bytes` are set independently of the summary, so
buffer navigation is unaffected. The 500-line cap itself is unchanged (it keeps the
buffered body navigable); only the *silence* is fixed. Shipped on `experiments`.
## Tests added

`src/librarian/adapter.rs` (new `#[cfg(test)] mod tests`):
- `compact_summary_surfaces_artifact_get_body_truncation` — overflow object → summary
  names shown/total lines and contains "TRUNCAT".
- `compact_summary_none_without_overflow` — in-cap body → `None` (generic fallback preserved).
- `compact_summary_none_for_non_artifact_tools` — defensive gate on tool name.

Live-verified through the reconnected server: `artifact(get, id=2dd9d90bc83f9f49,
full=true)` summary flipped from `"Result stored in @tool_… (47588 bytes)"` to the
TRUNCATED warning, same `buffered_bytes: 47588`.
## Workarounds

For large trackers, read in slices via `read_file` on the buffered `@tool_*`/`@file_*` reference (as this session did) rather than trusting a single `full=true` call to return the complete document. Cross-check suspiciously round-looking `body` lengths against `preview.line_count` when the two are available from the same or a sibling call.

## Resume

Fixed 2026-07-10. No further action. Sibling report
`docs/issues/2026-07-09-artifact-get-full-true-body-silent-truncation.md` (id
`98dc447e9c72eacc`) is the same defect from the `read_file(json_path="$.body")` angle —
closed by the same fix.
## References

- `docs/trackers/bug-fix-session-log.md` (id `2dd9d90bc83f9f49`) — the tracker on which this was found; see its `## W-21` entry for the reconnaissance-skill scout that caught this before it caused an F-N/W-N ID collision.
- `docs/issues/2026-07-07-doctor-ads-colon-verbatim-prefix-false-positive.md` — the unrelated Windows-path bug being fixed in the same session when this was found.
