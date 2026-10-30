---
id: c72d1df9c825b2ed
kind: bug
status: fixed
title: The code_chunks Qdrant collection has no payload index, so every project_id filter scans all 737k points
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-10-30
opened: 2026-10-30
severity: high
unverified: 'The live-Qdrant regression tests are #[ignore] and no gate lane compiles server-stack, so only a manual --include-ignored run exercises them. The gate run of 2026-10-30 was red on two OP-2 tests caused by a peer''s uncommitted ledger edit (docs/trackers/operator-rules.md), so it is not evidence for this change. The live code_chunks indexes were created by REST with the same parameters, not by ensure_collection itself; the upgrade code path is proven only on a test collection.'
---

# BUG: The code_chunks Qdrant collection has no payload index, so every project_id filter scans all 737k points

## Summary

`QdrantWrap::ensure_collection` (`src/retrieval/qdrant.rs:180-204`) creates `code_chunks` with a dense and a sparse vector and no payload index. It also returns early when the collection exists, so no later run adds one. Every filtered call on `code_chunks` (search, sync scroll, existence probe, stats) filters on `project_id` and sometimes `file_path`, so Qdrant scans every point of a collection shared by many projects. The artifact and memory collections already index `project_id` (`src/retrieval/artifact.rs:63-82`, `src/retrieval/memory.rs:77-84`).

## Class fit

No class on the roster names a missing performance index, so the sanctioned `cluster/unclassified` is used. What the case shows: a setup step that is idempotent only for existence, so a property added to sibling collections never reaches the oldest and largest one.

## Symptom (Effect)

Measured 2026-10-30 with read-only REST calls against Qdrant 1.17.0 at `127.0.0.1:6333`:

```
GET /collections/code_chunks            -> points_count 737429, payload_schema {}
POST .../points/count  filter project_id=codescout exact=true -> count 100897, time 5.69 s (a second run: 2.66-2.77 s)
POST .../points/scroll filter project_id=codescout limit 1000 with_payload [file_path] -> 0.58 s per page
POST .../points/scroll filter project_id=codescout limit 1   -> 0.59 s   (project_has_chunks shape)
hybrid query filtered on project_id     -> 1.13 / 1.14 / 1.39 s
same hybrid query unfiltered            -> 0.028-0.039 s
```

A filtered query for a tiny project (`codescout-embed`) costs the same, so the cost is a fixed full scan, not proportional to the project.

Indexed control (`artifact_chunks_codescout_*`, 52k points, `project_id` keyword-indexed): exact count 0.005-0.017 s; a 1000-row scroll page 0.054-0.066 s.

## Reproduction

```
curl -s http://127.0.0.1:6333/collections/code_chunks | python3 -c 'import sys,json; print(json.load(sys.stdin)["result"]["payload_schema"])'
time curl -s -X POST http://127.0.0.1:6333/collections/code_chunks/points/count -H 'content-type: application/json' \
  -d '{"filter":{"must":[{"key":"project_id","match":{"value":"codescout"}}]},"exact":true}'
```

HEAD `b27329082` (2026-10-30).

## Environment

Linux, Qdrant 1.17.0 (docker, REST 6333 / gRPC 6334), qdrant-client crate 1.17.0, `code_chunks` shared by every project on this host, `on_disk_payload: true`.

## Root cause

- `src/retrieval/qdrant.rs:180-204` (`ensure_collection`): `create_collection` with vectors only; early return at `:181-183` when the collection exists.
- Filters on `code_chunks`: `project_id` at `src/retrieval/qdrant.rs:98, 114, 212, 285, 312`; `file_path` in the `must_not` exclusion at `:115`.
- measured 2026-10-30: the curl calls above.

## Evidence

Callers that pay the scan (read 2026-10-30 by the retrieval explorer of this session, spot-checked):

- `hybrid_query` (`src/retrieval/qdrant.rs:421-504`): filter on both prefetch legs → every `semantic_search` (usage.db: 12 calls, avg 1.29 s).
- `scroll_chunk_refs` (`src/retrieval/qdrant.rs:207-275`): every `sync_project` (`src/retrieval/sync.rs:1150`), twice per worktree sync (`:881`, `:890`), `verify_index_coverage` (`:609`).
- `project_has_chunks` (`src/retrieval/qdrant.rs:282`): every activation probe. Its doc comment says "constant work regardless of corpus size"; it measured 0.59 s.
- `project_index_stats` (`src/retrieval/qdrant.rs:305-353`): see `docs/issues/2026-10-30-workspace-and-index-status-scroll-every-chunk-of-the-project-to-count-them.md`.

## Hypotheses tried

1. **Hypothesis:** the slowness of `workspace(action="status")` is the scroll alone. **Test:** time one filtered scroll page and one filtered count. **Verdict:** each filtered request costs ~0.55 s by itself, so the missing index multiplies every page.

## Fix

Implemented on `experiments`:

- `b565ba72e` (patch-id `1270454f901b285eadfbf7c049749af705aef035`): `CODE_PAYLOAD_INDEXES` (`project_id` keyword with `is_tenant`, `file_path` keyword) and the pure `missing_code_payload_indexes`; `QdrantWrap::ensure_collection` (`src/retrieval/qdrant.rs`) now calls `ensure_code_payload_indexes` on new AND existing collections, `wait(false)`, and logs a refusal as a warning instead of failing the sync.
- `addb7e34d` (patch-id `8a3a02ae7f3473d004fd8fa482c96c6bfafb6cc5`): the status follow-up, see `docs/issues/2026-10-30-workspace-and-index-status-scroll-every-chunk-of-the-project-to-count-them.md`.

A live deployment gets the indexes at its next `index(action="build")` (the only production caller of `ensure_collection` is `sync_project`). On this host they were created on 2026-10-30 15:05 by REST with the same parameters (`PUT /collections/code_chunks/index`, `wait=false`); both reached 737,514 points and the collection was green after 12 s.

Measured after, same requests as § Symptom (2026-10-30, three runs each):

```
exact filtered count      5.69 s / 2.66-2.77 s  ->  0.015-0.017 s
scroll page (1000 points) 0.58 s                ->  0.006-0.007 s
facet file_path           refused               ->  0.039-0.046 s (2,986 files, same as the scroll)
filtered dense query      ~0.57 s per leg*      ->  0.002 s (first call 0.244 s)
```

*the before figure is one leg of a hybrid query measured by a subagent; the after figure is a dense-only query, so the shapes differ.

## Tests added

- `missing_code_payload_indexes_names_only_the_fields_the_schema_lacks` (`src/retrieval/qdrant.rs`, `tests`): pure, runs in a `server-stack` lib build.
- `qdrant_ensure_collection_adds_payload_indexes_to_a_collection_that_predates_them` (`#[ignore]`, live Qdrant): the upgrade path. Mutation check 2026-10-30: restoring the early return made it fail with `payload indexes ["project_id", "file_path"] never appeared`.
- `qdrant_project_index_stats_facet_agrees_with_the_scroll` and `qdrant_project_index_stats_falls_back_to_the_scroll_without_an_index` (`#[ignore]`, live Qdrant).

Run with `scripts/with-slot.sh cargo test --lib --features server-stack retrieval::qdrant::tests -- --include-ignored`: 12 passed on 2026-10-30.

## Workarounds

None for callers.

## Resume

N/A — fixed 2026-10-30. Archive once a gate run is green (the 2026-10-30 run was red on a peer's uncommitted `OP-2` ledger edit, unrelated to this change).

## References

- `docs/issues/archive/2026-08-08-index-probe-scrolls-the-whole-corpus-to-answer-a-yes-no.md`
- `src/retrieval/artifact.rs:63-82` (the same pattern, already applied to artifacts)
