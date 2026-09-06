---
kind: bug
status: fixed
tags:
- cluster/blast-radius-exceeds-visibility
closed: 2026-09-06
opened: 2026-09-03
owner: marius
related: []
severity: high
---

# BUG: on the Qdrant backend, editing an artifact silently removes it from semantic search, permanently

## Summary

> **FIXED at `6f032dbd` (patch-id `2605fac14725020fcd4fcb66e5a22d6d21d85f9a`, `experiments`).**
> The description below is the state as filed on 2026-09-03 and is kept verbatim — it is the
> premise § *Fix* discharges. Nothing in it describes current behaviour.

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

**Fixed — Qdrant chunk-grain parity. `6f032dbd` on `experiments`, patch-id
`2605fac14725020fcd4fcb66e5a22d6d21d85f9a`.**

The second fork was taken. The trait was widened rather than the deployment being moved to
sqlite-vec, so the `artifact_vec_v2` re-embed contemplated above was never needed.

Three changes that only work together:

- **`ArtifactVectorStore::upsert` carries both ids** (`src/librarian/artifact_store.rs:139-145`)
  — `chunk_id` is the vector's identity, `artifact_id` the catalog key `delete` matches on.
  The one-slot signature was the bug; the backend was not. sqlite-vec had merely been
  *surviving* it by joining `artifact_chunk`, which is why the defect read as Qdrant-specific.
- **The 16-hex grain guard is gone** (`src/librarian/artifact_store.rs:295-321`). Its absence
  is the fix rather than a relaxation: with both ids travelling, the input it refused is
  exactly the input that path is for. The comment at the old guard site says so, so a reader
  does not restore it.
- **`artifact_upsert` writes both payload fields** (`src/retrieval/artifact.rs:100-124`) —
  point id derived from `chunk_id`, payload carrying `chunk_id` *and* `artifact_id`. That is
  the second slot whose absence forced the one id to be spent twice.

The read-side instrument this file asked for first shipped in the same commit: `semantic_find`
counts what it discards as `SemanticPage::unresolved` (`src/librarian/catalog/find.rs:381-388`)
and `doc(action="find")` surfaces it with a hint (`src/librarian/tools/find.rs:1089-1094`).

Qdrant also went **one collection per project** (`artifact_chunks_<name>_<hash>`). That is what
retired the mixed-grain `artifacts` collection — it was abandoned rather than migrated, which
is why "the collection needs rebuilding, not patching" was satisfied without a rebuild step.

**Verified live 2026-09-06 14:2x against the running daemon at 127.0.0.1:6333**, because no
gate lane can reach this code (see § *Tests added*):

```
collection                                         exact point count
artifact_chunks_codescout_dc6a871595179329                    29435
artifact_chunks_claude_plugins_38b0719140f222d8                4818
artifact_chunks_backend_kotlin_dcb6382ad43d85ac                 164
artifacts                     (legacy, mixed-grain)            2425
```

The legacy `artifacts` collection still exists and is **unreachable, not stale**:
`artifact_collections` enumerates by `n.starts_with(prefix)` with `prefix = "artifact_chunks_"`
(`src/librarian/artifact_store.rs:219`, `src/retrieval/artifact.rs:230-244`), and
`"artifacts".starts_with("artifact_chunks_")` is false. No query path reads it. It is dead
storage, tracked separately — see § *Resume*.
## Tests added

`a_candidate_with_no_chunk_row_is_counted_not_silently_dropped` —
`src/librarian/catalog/find.rs:625`. Exactly the test this file prescribed while it was open:
seed a candidate whose id resolves to no `artifact_chunk` row, and assert `page.unresolved == 1`
rather than a silently-short page. Written against the catalog rather than a backend, so **both**
gate lanes run it — which was the point of choosing the read-side half as the testable one.

Its own comment (`find.rs:630`) records why it asserts on `unresolved` alone: a bare
`unresolved` check paired with `exhausted` would pass a change that set both, so the
discriminating assertion is the count, not the pair.

Two further behavioural guards run against `InMemoryArtifactStore` — the collection-naming rule
and the delete grain.

**What no test covers, stated plainly in `6f032dbd`'s own commit message and repeated here so it
is not rediscovered:** the Qdrant write keying, the filtered delete, and the fan-out merge.
`server-stack` is not a default feature, so neither gate lane compiles any of it —
`cargo clippy --features server-stack` (added to the gate by that commit) is the only thing that
type-checks it, and an end-to-end reindex is the only thing that runs it. That gap is
pre-existing and is now the largest in this area. It is precisely why the live point-count
census sits in § *Fix* rather than being left to the suite: for this code the running daemon
**is** the instrument, and a green gate is evidence about the other half of the file.
## Workarounds

N/A — fixed. While it was open the only honest advice was to treat artifact semantic search as
a decaying subset; that no longer holds.
## Resume

N/A — fixed and archived.

**One residual, deliberately not folded in here:** the legacy `artifacts` Qdrant collection
holds 2425 orphaned mixed-grain points that no code path enumerates. It is dead storage, not a
search defect, and it will never be reclaimed on its own — filed as
`docs/issues/2026-09-06-the-legacy-artifacts-qdrant-collection-is-orphaned-not-cleaned-up.md`
rather than kept open here, because leaving a *fixed* correctness bug open to carry an ops
chore is how a high-severity row stays live long after its mechanism is gone. That is the
failure this very file demonstrated: it sat at `status: open severity: high` for three days
after `6f032dbd` closed it, with a Resume sending the next reader to re-implement finished
work.

**Why it stayed open, recorded because no instrument caught it.** `librarian(action="doctor")`
has `non_terminal_status_with_fix_anchor`, which fires on an open bug that *records* a fix
anchor. `6f032dbd` never referenced this file, and this file never recorded a SHA, so there was
no anchor to notice and the check read 0 — a true answer to a question nobody had asked. The
signal that did exist was in the source: the fixing commit left three doc comments citing this
file by path (`artifact_store.rs:132`, `retrieval/artifact.rs:99`, `catalog/find.rs:306`).
A live bug file cited from a *source* doc comment is a cheap tell that its fix already shipped,
and it is not currently wired to anything.
## References

- `docs/superpowers/plans/2026-09-02-artifact-chunk-grain-retrieval.md` §
  *Deferred* — predicted "Qdrant is already storing chunk-keyed points whose
  payload claims they are artifact ids". Confirmed here, with the census.
- `docs/trackers/retrieval-benchmark.md` — the 2026-09-03 runs, 2/12 then 1/12.
- `docs/issues/archive/2026-09-02-chunk-line-ranges-are-body-relative-but-published-as-file-lines.md`
  — the sibling defect, fixed at `36afd405`.
