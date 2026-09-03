---
kind: bug
status: open
tags:
- cluster/blast-radius-exceeds-visibility
closed: null
opened: 2026-09-03
owner: marius
related: []
severity: high
---

# BUG: on the Qdrant backend, editing an artifact silently removes it from semantic search, permanently

## Summary

The `artifacts` Qdrant collection is a **mixture** of two id grains, and
`semantic_find` can only read one of them. 2476 of 5388 points (46%) are
artifact-grain and are silently skipped by every query. The other 2912 are
chunk-keyed points written during the window between Task 6 (which made the
embed queue chunk-keyed) and Task 7's guard (which now refuses chunk ids at the
Qdrant boundary). That set is **frozen**: it can never be extended or
refreshed. So every time anyone edits a markdown artifact, that artifact's chunk
ids are re-minted, its old points go stale, the write of its new points is
refused — and it drops out of semantic search and cannot come back.

This is not the state the plan's § *Deferred* describes. That section says a
Qdrant deployment is one "this plan does not apply to yet", which reads as
*keeps working artifact-grain*. It does not: the artifact-grain half of the
collection is unreachable.

## Symptom (Effect)

Measured 2026-09-03 18:50 by scrolling the whole collection:

```
qdrant 'artifacts' collection, scrolled in full
  total points                    : 5388
  payload artifact_id = ARTIFACT id (16 hex, UNREACHABLE by semantic_find): 2476
  payload artifact_id = CHUNK id  (36 char, hydrates)                     : 2912
```

Observed end-to-end on `docs/trackers/bug-fix-session-log.md`. At 00:35 it
ranked **1** for the benchmark's AE-1 query. I appended entry `F-108` to it at
00:45. At 18:45 it does not appear in the top 10 for that query, nor for a query
built verbatim from the text I had just added to it. It still holds 564
`artifact_chunk` rows and exactly **1** Qdrant point — and that point is
artifact-grain, i.e. one `semantic_find` can never return.

The benchmark recorded the drop as `hits@5 2/12 -> 1/12` without naming a cause.

## Reproduction

```
# with ArtifactBackend::resolve -> Qdrant (the default on a server-stack build)
<edit any indexed markdown artifact>
librarian(action="reindex")
# -> embed_error_count: N, every one:
#    "QdrantArtifactStore is artifact-grain and was handed a non-artifact id ..."
<semantic query that previously returned that artifact>
# -> the artifact is absent, with no indication it was ever there
```

## Environment

`experiments` @ `f74f25ec`, release build (`server-stack`), Qdrant 127.0.0.1:6333
collection `artifacts`, embeddings `CodeRankEmbed` @ 127.0.0.1:48081. No
`[librarian] vector_backend` in `.codescout/project.toml`; `CODESCOUT_ARTIFACT_BACKEND`
unset. `artifact_vec_v2` (sqlite) holds 0 rows, so sqlite-vec serves nothing —
confirmed by forcing each backend:

```
CODESCOUT_ARTIFACT_BACKEND=qdrant     -> count=3
CODESCOUT_ARTIFACT_BACKEND=sqlite-vec -> count=0
```

## Root cause

`ArtifactVectorStore::upsert(&self, project_id, id, vector)` carries **exactly
one id** (`src/librarian/artifact_store.rs:92`). Task 6 changed the embed queue
to one item per chunk, so that slot now holds a *chunk* id on every backend.

The two backends survive that differently:

- **sqlite-vec survives** because its second id is recoverable. `upsert` writes
  `artifact_vec_v2`, which is keyed by `chunk_id`, and `artifact_chunk` carries
  the `artifact_id` column — hydration is a join
  (`src/librarian/artifact_store.rs:242`).
- **Qdrant cannot**, because `artifact_upsert` (`src/retrieval/artifact.rs:77-98`)
  spends that one id **twice**: as the point id via `artifact_point_id(id)`, and
  as the payload field literally named `artifact_id` — which is the value `knn`
  returns as "the catalog key". There is no second field to put the other id in.

`QdrantArtifactStore::upsert` therefore refuses a non-16-hex id
(`src/librarian/artifact_store.rs:~178`). The guard is exact rather than
heuristic — artifact ids are `sha256(abs_path)` hex[..16], chunk ids are UUID v4
— and it is correct as far as it goes.

**What the guard does not cover is the read side.** `semantic_find`
(`src/librarian/catalog/find.rs:~355`) resolves every id `knn` returns through
`artifact_chunk` and skips anything with no row:

```rust
let Some(row) = chunk_rows.get(chunk_id) else {
    continue;
};
```

An artifact-grain id can never be a chunk id, so those 2476 points are skipped
on every query — no error, no count, no hint. The comment above that line reads
"A chunk id with no row is stale, not an error — skip it", which is true of the
case it was written for and silently absorbs this one too.

So the collection now decays monotonically: the guard stops new chunk points
going in, and ordinary editing turns existing chunk points into stale ones.

Measured 2026-09-03: the point-grain census above, the forced-backend
comparison, and the AE-1 before/after. The "editing removes it permanently"
mechanism is **inferred from those three plus the code path** — I have one
observed instance (`bug-fix-session-log.md`), not a rate.

## Hypotheses tried

1. **Hypothesis:** the backfill CLI writes to a store the backend never reads.
   **Verdict:** rejected — `SqliteVecArtifactStore::upsert` calls the same
   `write_embeddings_v2`, so the paths are identical on the one backend where
   chunk-grain works. A bug file was drafted 2026-09-03 and withdrawn unpushed.

2. **Hypothesis:** AE-1's artifact was re-ranked below the fold by the corpus
   growing from 55 to 121 chunked artifacts.
   **Verdict:** rejected — it is not merely low, it is unreachable: its only
   Qdrant point is artifact-grain.

3. **Hypothesis:** Qdrant's points are all artifact-grain and hits come from a
   fallback path.
   **Verdict:** rejected — `semantic_find` has no artifact-id fallback, and the
   census shows 2912 chunk-keyed points. The hits come from those.

## Fix

Two options; this is the plan's **open question 4** and it is a deployment
decision, not only a code one.

- **Point this project at sqlite-vec** — `[librarian] vector_backend = "sqlite-vec"`
  in `.codescout/project.toml`. Cheap and immediate, but `artifact_vec_v2` is
  empty, so it needs a full re-embed before search returns anything at all.
- **Implement Qdrant chunk-grain parity** — widen the trait to carry both ids
  (chunk id as the vector's identity, artifact id as the hydration/scope key),
  add the second payload field, and have `knn` return the pair. This reaches
  `semantic_find`'s read shape too.

**Either way the collection needs rebuilding, not patching**: 46% of its points
are in a grain nothing reads, and the rest are frozen at a stale snapshot.

Independently of the choice: `semantic_find` should **count** the candidates it
skips and surface that, the way it already surfaces `cap_suppressed`. A read
path that silently discards 46% of what the store returns is the reason this took
a full session to see.

## Tests added

None yet — filed at notice.

No gate lane compiles this path: `server-stack` is not a default feature, so
both test lanes build `ArtifactBackend::resolve`'s `SqliteVec` arm and the
Qdrant code is never exercised. A regression test for the read-side half can be
written feature-independently against `InMemoryArtifactStore`: seed it with an
id that has no `artifact_chunk` row, and assert `semantic_find` reports the skip
rather than returning a silently-short page.

## Workarounds

None that preserve current behaviour. Until the fork is decided, treat artifact
semantic search as returning a decaying subset, and expect any artifact you edit
to leave it.

## Resume

Decide the fork above. If sqlite-vec: set `vector_backend`, then re-embed
(`librarian(action="reindex", reembed=true)`), and expect the first run to
write ~7573 chunk vectors. If Qdrant parity: start at
`src/librarian/artifact_store.rs:92` (the trait), then
`src/retrieval/artifact.rs:77` (payload), then
`src/librarian/catalog/find.rs:~355` (read shape).

Either way, add the skip counter to `semantic_find` first — it is small,
independent of the fork, and it is the instrument whose absence hid this.

## References

- `docs/superpowers/plans/2026-09-02-artifact-chunk-grain-retrieval.md` §
  *Deferred* — predicted "Qdrant is already storing chunk-keyed points whose
  payload claims they are artifact ids". Confirmed here, with the census.
- `docs/trackers/retrieval-benchmark.md` — the 2026-09-03 runs, 2/12 then 1/12.
- `docs/issues/2026-09-02-chunk-line-ranges-are-body-relative-but-published-as-file-lines.md`
  — the sibling defect, fixed at `36afd405`.
