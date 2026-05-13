# L-01 — Port `memory.*` from sqlite-vec to Qdrant `memories` collection

**Status:** draft / placeholder · **Opened:** 2026-05-13 · **Owner:** TBD

## Why

`src/tools/memory/mod.rs` is the last load-bearing consumer of
`src/embed/index.rs`. Until memory is on Qdrant, codescout cannot drop
`tantivy`, `sqlite-vec`, `fastembed`, or the `local-embed` feature — even
though they are no longer needed for code search.

Concretely, the blocker is the call surface:

```
src/tools/memory/mod.rs
  ↳ crate::embed::index::open_db(&root)
  ↳ crate::embed::index::ensure_vec_memories(conn)
  ↳ crate::embed::index::upsert_memory_by_title(...)
  ↳ crate::embed::index::ensure_memory_anchors(conn)
  ↳ crate::embed::index::delete_semantic_anchors(...)
  ↳ crate::embed::index::insert_semantic_anchor(...)
  ↳ crate::embed::index::search(conn, &embedding, top_n)
  ↳ crate::embed::index::get_file_hash(conn, path)
  ↳ crate::embed::index::delete_memory(conn, id)
```

Nine call sites in `memory/mod.rs` plus two in `src/prompts/builders.rs`. The
embedding source itself already routes through `codescout_embed::embed_one`
(any `Embedder`), so the work is purely on the **storage** side.

## Scope

| Item | Today | Target |
|---|---|---|
| Storage | sqlite-vec table `vec_memories` + side tables | Qdrant collection `memories` (per project) |
| Dense vectors | embedded via active `Embedder` | unchanged |
| Schema | `bucket, title, content, anchor_path, anchor_hash, created_at, updated_at` | same fields in Qdrant payload |
| Operations | open_db, ensure_*, upsert_by_title, search, delete | upsert / search / delete on `RetrievalClient` |
| Anchors | `semantic_anchors` table | second collection `memory_anchors` *or* embedded array on the memory point |
| Migration | n/a | one-shot tool reading sqlite-vec and bulk-importing into Qdrant |

## Open questions

1. **One collection per project or one shared collection with `project_id` payload filter?**
   Code chunks use the latter — consistent design says memory should too.
2. **Anchors as a separate collection or as embedded array?**
   Embedded simplifies queries but caps anchor count per memory; separate is
   flexible but adds a join.
3. **Migration UX:** opt-in `codescout migrate-memories` subcommand, or
   automatic on first run if a legacy sqlite-vec memory store is detected?
4. **What happens to `index(action='build')`?** L-10 question — re-route to
   `sync_project` semantics, or delete the tool entirely.

## Suggested order

1. Design doc fleshing out the four questions above (this file).
2. New module `src/memory/store.rs` with a `MemoryStore` trait — implementations
   for sqlite-vec (legacy, deletable) and Qdrant (target).
3. Wire `RetrievalClient` to expose memory ops (`memory_upsert`, `memory_search`,
   `memory_delete`) — likely needs a small `qdrant.rs` extension.
4. Switch `memory/mod.rs` callers to the trait. Keep the legacy impl behind a
   feature flag for one release.
5. Write the migration tool (`src/bin/migrate_memories.rs`).
6. Run the full memory test suite against both backends.
7. Delete `src/embed/index.rs`, `bm25.rs`, `fusion.rs`, `local.rs`, `drift.rs`.
   Drop `tantivy`, `sqlite-vec`, `fastembed` from `Cargo.toml`. Flip
   `local-embed` off by default. (Mechanical once the rest is done.)

## Cross-references

- Parent tracker: `docs/trackers/2026-05-07-legacy-retrieval-removal.md` (L-01 row)
- Compose deliverable that lands first: `docker-compose.yml` (this PR)
- Retrieval benchmark history: `docs/trackers/retrieval-benchmark.md`
