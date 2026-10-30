---
id: '17a9af180d2cf288'
kind: bug
status: fixed
title: workspace and index status scroll every chunk of the project to count chunks and files (22-33 s per call)
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-10-30
opened: 2026-10-30
related:
- docs/issues/2026-10-30-code-chunks-collection-has-no-payload-index-so-every-project-filter-scans-every-point.md
severity: high
unverified: 'Measured with a debug build of this code over stdio MCP, not with a release server; the old release server, given the same indexes, answered in 754 ms, so most of the gain comes from the payload index, not from the facet. The first status call in a fresh server took 14.7 s (45.2 s before the index); that cold cost is not explained. The live-Qdrant tests are #[ignore].'
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

Implemented on `experiments`:

- `b565ba72e` (patch-id `1270454f901b285eadfbf7c049749af705aef035`): `QdrantWrap::project_index_stats` (`src/retrieval/qdrant.rs`) tries `project_index_stats_by_facet` (exact `count`, then `facet` on `file_path` with `limit` = chunk count, so it cannot cut the file list) and falls back to `project_index_stats_by_scroll` (the old enumeration) when the facet is refused.
- `addb7e34d` (patch-id `8a3a02ae7f3473d004fd8fa482c96c6bfafb6cc5`): the facet path reads the payload schema first and goes straight to the scroll when `file_path` is not indexed. Without it the exact count was a wasted full scan: status measured 34.8 / 37.8 s steady state against 32.1 s for the old code.

The gain needs the payload index from `docs/issues/2026-10-30-code-chunks-collection-has-no-payload-index-so-every-project-filter-scans-every-point.md`.

Measured 2026-10-30, `workspace(action="status")` on codescout (100,897 chunks, 2,986 files), driver `status_timer.py` over stdio MCP, three calls in one server; the first call in a fresh server is listed separately:

| build | code_chunks indexes | first call | steady state |
|---|---|---|---|
| old release server (MCP, this session) | none | — | 32,108 ms |
| new debug, before `addb7e34d` | none | 52.6 s | 37.8 / 34.8 s |
| new debug, `addb7e34d` | none | 45.2 s | 30.7 / 30.9 s |
| new debug, `addb7e34d` | project_id + file_path | 14.7 s | **885 / 900 ms** |
| old release server (MCP, this session) | project_id + file_path | — | 754 ms |

Every call returned the same `files: 2986, chunks: 100897`.

## Tests added

- `qdrant_project_index_stats_facet_agrees_with_the_scroll` (`src/retrieval/qdrant.rs`, `#[ignore]`, live Qdrant): facet path == scroll path == public entry point for a project with chunks, one sharing files with another project, and an absent one.
- `qdrant_project_index_stats_falls_back_to_the_scroll_without_an_index` (`#[ignore]`): on an unindexed collection the facet path is refused and the public entry point still answers `(3, 2)`.

No test pins the time; the table in § Fix is the timing evidence.

## Workarounds

Use `workspace(action="activate")`, which reports index presence through the cheap probe.

## Resume

N/A — fixed 2026-10-30. Archive once a gate run is green. Open question for a later session: why the first status call of a fresh server costs ~14 s more than the next ones.

## References

- `docs/issues/archive/2026-08-08-index-probe-scrolls-the-whole-corpus-to-answer-a-yes-no.md` (§ Fix: "`project_index_stats` genuinely needs `file_count`, and distinct-file counting requires enumeration" — a facet removes that premise)
