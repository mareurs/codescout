---
id: '17a9af180d2cf288'
kind: bug
status: taken
title: workspace and index status scroll every chunk of the project to count chunks and files (22-33 s per call)
owners:
- marius
tags:
- cluster/unclassified
claimed_by: f6c27cf1-225c-4791-9084-17cffa386faf
opened: 2026-10-30
related:
- docs/issues/2026-10-30-code-chunks-collection-has-no-payload-index-so-every-project-filter-scans-every-point.md
severity: high
---

# BUG: workspace and index status scroll every chunk of the project to count chunks and files (22-33 s per call)

## Summary

`ProjectStatus/call` (`src/tools/config/mod.rs:659-662`) and the `index` status path (`src/tools/semantic/index.rs:637-644`) call `RetrievalClient::project_index_stats`. On Qdrant that is `QdrantWrap::project_index_stats` (`src/retrieval/qdrant.rs:305-353`), which scrolls every chunk of the project 1000 at a time only to count chunks and distinct `file_path` values. For codescout (100,897 chunks) that is about 101 filtered requests. The 2026-08-08 fix moved activation onto the cheap `project_has_chunks` probe but left these callers on the full scroll.

## Class fit

`cluster/unclassified`: no roster class names an aggregate computed by enumeration. What the case shows: a fix applied to one caller of an expensive function, while sibling callers that ask a cheaper question (a count) keep paying the full enumeration.

## Symptom (Effect)

usage.db (`.codescout/usage.db`, 2026-09-30 to 2026-10-30):

```
workspace action=status : 48 calls, avg 22,231 ms, max 48,789 ms, 41 over 5 s
  2026-10-29 10:16  38,983 ms   2026-10-29 17:05  39,212 ms
  2026-10-29 20:01  30,433 ms   2026-10-29 22:19  33,642 ms
  2026-10-30 05:30  29,045 ms
index status            :  7 calls, avg 32.8 s
index verify            :  3 calls, avg 27.6 s
```

`post_compact=true` status calls return in 1-946 ms because they return before the index section.

## Reproduction

`workspace(action="status")` on the codescout project, with the retrieval stack up. HEAD `b27329082`.

## Environment

Linux, Qdrant 1.17.0, `code_chunks` 737,429 points with no payload index (see `docs/issues/2026-10-30-code-chunks-collection-has-no-payload-index-so-every-project-filter-scans-every-point.md`).

## Root cause

- `src/retrieval/qdrant.rs:305-353`: paged scroll, `limit(1000)`, payload `file_path`, loop until `next_page_offset` is `None`.
- Callers: `src/tools/config/mod.rs:659-662` (status), `src/tools/semantic/index.rs:637-644` (index status), `src/tools/onboarding.rs:963-966`, `src/dashboard/api/index.rs:36`, `src/tools/semantic/index.rs:400` (after a worktree sync).
- measured 2026-10-30: one filtered scroll page = 0.58 s (curl, read-only); 101 pages ≈ 59 s upper bound; status calls measured 29-39 s.

## Evidence

See Symptom. The page cost is in the payload-index issue above.

## Hypotheses tried

None.

## Fix

Plan: count chunks with an exact `count` request (indexed: milliseconds) and count distinct files with a `facet` on `file_path` filtered by `project_id` (needs the payload index; facet `exact=true`). Keep the trait signature `(chunk_count, file_count)`. Fall back to the scroll only if the facet call fails.

## Tests added

None yet.

## Workarounds

Use `workspace(action="activate")`, which reports index presence through the cheap probe.

## Resume

Implement count + facet in `QdrantWrap::project_index_stats`; re-time `workspace(action="status")` after the payload index exists.

## References

- `docs/issues/archive/2026-08-08-index-probe-scrolls-the-whole-corpus-to-answer-a-yes-no.md` (§ Fix: "`project_index_stats` genuinely needs `file_count`, and distinct-file counting requires enumeration" — a facet removes that premise)
