---
id: '561841b0ae4a01d8'
kind: research
status: draft
title: 'Plan C3 scout — the content-keyed vector cache: what exists, what is missing, three designs'
owners:
- marius
tags:
- identity
- plan-c3
- scout
- vector-cache
- embeddings
---

# Plan C3 scout — the content-keyed vector cache: what exists, what is missing, three designs

Written 2026-11-14, after Plan C2 landed. Method: I read the spec sections "Constraints", "The store boundary", "The catalog is derived", "Cutover", "Contract with ledger formats" and "Index inputs, guard, machine state and entry ids". I read the code named in each finding. I also ran read-only probes: `sqlite3 -readonly` on the live catalog, GET requests to the local Qdrant HTTP API, `env`, `git ls-files` and `git check-ignore`. I wrote nothing outside this file. "Measured" means I ran it today. "Cited" means I took it from a comment or a document and did not re-measure it. Spec line numbers are from `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md`.

## What the spec asks of Plan C3

- Constraints (spec L99-101): vectors are derived data, expensive to rebuild, a cache keyed by content hash and never state. "Today reuse is within one artifact only, and vectors die with their artifact row; the content-keyed cache is Plan C work."
- The catalog is derived (spec L230): "Embedding vectors are a cache keyed by content hash, so a rebuild reuses them."
- Plan order (spec L380-383): Plan C holds the dual-write and "the content-keyed vector cache". C1 split it into C1, C2 and C3 (C1 plan L7).
- The C1 state document (L98) says C3 "needs a persistent derived-file location first".
- Earlier review findings (session log, lines 841, 842 and 845): a plain blob table inside the derived file passes the determinism test but breaks the schema's own rule that every stored row is deterministic. `embed_input_hash` ignores the model, the budget and the dimension. The derived file has no persistent location, no build trigger and no consumer.

## Findings

### C3-1. Where chunk embeddings are stored

**Claim.** The chunk text is in the shared catalog file. The vectors are in one of two backends. Qdrant is the default. On this machine Qdrant holds them.

**Evidence.**
- Text rows: table `artifact_chunk` in the per-user catalog file `<data_local_dir>/librarian/catalog.db` (`src/librarian/mod.rs:316-320`; override `LIBRARIAN_DB`, `mod.rs:57,124`). Columns: `chunk_id` TEXT PK (a UUID v4 minted at `catalog/chunk.rs:503`), `artifact_id` (FK, ON DELETE CASCADE), `chunk_ix`, `start_line`, `end_line`, `entry_token`, `entry_part`, `entry_parts`, `content`, `content_hash`, and `embedded_input_hash` (added by v14). `UNIQUE (artifact_id, chunk_ix)`. Source: `catalog/mod.rs:403-416,516`; `ChunkRow` at `catalog/chunk.rs:11-43`.
- Backend 1, Qdrant (default on a `server-stack` build; `artifact_store.rs:24-32,65-84`). One collection per project, named `artifact_chunks_<basename>_<hash>`, where the hash is `artifact_id_from_abs(project_root)` (`artifact_store.rs:112-125`). Named vector `dense`, cosine distance (`retrieval/artifact.rs:49-55`). Point id is UUIDv5 over `chunk_id` (`artifact.rs:41-43,134-151`). Payload: `project_id`, `artifact_id`, `chunk_id`. Keyword indexes on `project_id` and `artifact_id` (`artifact.rs:63-82`).
- Backend 2, sqlite-vec: virtual table `artifact_vec_v2` = `vec0(id TEXT PRIMARY KEY, embedding FLOAT[768])` in `catalog.db` (`catalog/mod.rs:424-427`). The id is the `chunk_id`. L2 distance (`artifact_store.rs:677-690`). The blob is little-endian f32 (`indexer.rs:1446`).
- The legacy `artifact_vec` (v1) was retired by v13 (`catalog/mod.rs:473-503`), yet the live file still lists `artifact_vec` and its trigger in `sqlite_master`. An older binary re-created them, as the v13 comment predicts.
- Which backend runs here (measured): my environment has no `CODESCOUT_ARTIFACT_BACKEND`. `.codescout/project.toml` holds only `[embeddings]` and `[ignored_paths]`, so no `vector_backend`. The default is Qdrant. `GET :6333/collections` lists 26 collections, including `artifact_chunks_codescout_dc6a871595179329`: 63,814 points, vector size 768, Cosine.
- Sizes (measured): catalog 142,888 `artifact_chunk` rows over 7,860 artifacts, 112,136,476 bytes of text. For this repo outside worktrees: 51,724 chunk rows over 3,090 artifacts. `artifact_vec_v2_rowids` holds 59,136 rows. One vector is 768 x 4 = 3,072 bytes.

**Not established.** Whether the running MCP binary is a `server-stack` build (inferred from the collection existing). Why Qdrant holds 63,814 points for 51,724 chunk rows of this repo (stale or foreign points are possible; I did not scroll the collection). Which projects own the 59,136 sqlite rows. The point counts of the other 25 collections.

### C3-2. The reuse rule today

**Claim.** Reuse is per artifact and per chunk row, in three layers. No layer looks across artifacts. The key has no model, dimension, budget or format version.

**Evidence.**
1. Row reuse. `replace_chunks(cat, artifact_id, rows)` (`chunk.rs:416-603`) loads `chunks_for(cat, artifact_id)` (`chunk.rs:606`), which is that one artifact's rows. Pass 1 keeps a row with the same `chunk_ix` and the same `content_hash`. Pass 2 keeps any unclaimed row of the same artifact with an equal `content_hash` (`chunk.rs:448-470`). A kept row keeps its `chunk_id`, so its vector stays. `content_hash` is SHA-256 over the chunk bytes only (`chunk.rs:83-84,103`).
2. Skip at embed time. `embed_queue_items` (`indexer.rs:251-297`) sets `up_to_date` when the stored `embedded_input_hash` equals `embed_input_hash(title, composed_text)` (`chunk.rs:643-651`: SHA-256 over a length-prefixed title, then the text). Only the MCP reindex drain honours the flag (`tools/reindex.rs:628`). `reembed=true` ignores it (`reindex.rs:623`). The marker is written after the vector lands (`reindex.rs:708`). It is nulled when a vector is rewritten (`indexer.rs:1414-1462`) or deleted (`indexer.rs:1569-1596`).
3. Artifact gate. `needs_embed` is true when `artifact.embedded_sha256` differs from the file hash (`indexer.rs:671-675`). The stamp is written only when every queued chunk landed (`reindex.rs:728-748`).

What the key leaves out:
- The model id. `chunk.rs:631-642` says it: "The embedding model is not part of the hash: the embedder exposes no identity to hash."
- The dimension and `budget_chars`. The budget decides whether a text is segmented and mean-pooled (`embedding.rs:113-132`).
- The composition format. The text `"{token} — {title}\n\n{content}"` (built in `indexer.rs:300-357`) and the join `"{title}\n\n{body}"` (`embedding.rs:114`) carry no version. A change to either would not invalidate a marker.

Coverage of the marker (measured): 26,812 of 142,888 chunk rows carry one (18.8%). For this repo outside worktrees: 13,770 of 51,724 (26.6%). Three quarters of today's vectors cannot be tied to any input hash. The v14 comment says an unmarked chunk "queues once" (`catalog/mod.rs:504-513`).

Duplicate content (measured): 131,352 distinct `content_hash` values over 142,888 rows. 3,436 hashes appear under more than one artifact, covering 14,790 rows; 11,354 of those rows are redundant. Because the title is in the embed input, content duplicates are an upper bound on cache hits.

**Not established.** Whether any path other than the three embed call sites embeds a chunk: `reindex.rs:631`, backfill `indexer.rs:1948`, CLI `index_repo` `indexer.rs:2162`. The CLI path never writes a marker: `mark_embedded` has one production caller (`reindex.rs:708`).

### C3-3. What deletes vectors, and when

**Claim.** A vector lives and dies with its `artifact_chunk` row. Anything that creates a new chunk id costs a new embed. A file moved by git is a full re-embed.

**Evidence.**
- Cascade (measured on the live file): seven tables reference `artifact(id) ON DELETE CASCADE`: `artifact_augmentation`, `artifact_chunk`, `artifact_link`, `artifact_observation`, `entry_reservation`, `event_edges`, `events`. Trigger `artifact_vec_v2_cascade_delete AFTER DELETE ON artifact_chunk` deletes the sqlite vector row (`catalog/mod.rs:438`; present in `sqlite_master`).
- Qdrant has no foreign key. The async caller tells the store: `delete(artifact_id)` by payload filter (`retrieval/artifact.rs:183-195`) from `reindex.rs:496-505` (removed, escaping-kept and vanished-kept ids), `indexer.rs:2154-2156` (CLI) and `tools/delete.rs:223-225` (`doc(delete)`).
- Vanished file. Since `b53baf521` (bug `99a4d0be1318c3c1`, fixed 2026-11-12) a vanished row that carries events or an augmentation is kept. `clear_kept_row` deletes its chunk rows (`indexer.rs:1036-1043`), which drops its sqlite vectors, and `vanished_kept_ids` go to `store.delete`. A vanished row without history is deleted (`indexer.rs:881`) and cascades. The spec's citation `indexer.rs:810-854` is stale: the branch is now at `indexer.rs:826-893`.
- Moved by git, not by `doc(move)`. The old row vanishes and loses its vectors as above. The new path is a new row (`id = sha256(abs_path)`): `existing` is None, `needs_embed` is true, `replace_chunks` finds no rows for the new id, every chunk gets a fresh UUID (`chunk.rs:503`) and is embedded. No lookup by content across artifacts exists. Moved by `doc(move)`: `refile` re-points the chunk rows (sqlite, `artifact_store.rs:648-675`) or the payload (Qdrant, `artifact_store.rs:482-`) and keeps the vectors (`tools/mv.rs:473-486`).
- Changed chunk: unclaimed rows are deleted by `chunk_id` (`chunk.rs:513-528`) and the trigger drops the vector (test `a_changed_chunk_loses_its_vector_across_a_reindex`, `chunk.rs:1282`).
- Re-embedding one chunk: sqlite does DELETE then INSERT (`indexer.rs:1448-1462`). Qdrant upsert is idempotent on `chunk_id`.
- Dimension change (sqlite only): `ensure_artifact_vec_v2_dim` bails unless `LIBRARIAN_ARTIFACT_VEC_MIGRATE=1`. With it, the code takes a snapshot, drops and re-creates the table, and nulls every marker (`indexer.rs:1467-1510,2026-2090`).
- Rebuild from scratch. Deleting `catalog.db` removes `artifact_chunk` and `artifact_vec_v2` with it. Qdrant points stay under chunk ids that no row names. `semantic_find` counts them as `unresolved` (`catalog/find.rs:300-313,386-395`). A new catalog mints fresh UUIDs and embeds everything, because `embedded_sha256` is NULL. I found no sweep of orphan points.
- `catalog/gc.rs:1-3` says "no scope-based deletion, no automatic deletion", and the file holds no `DELETE FROM` (grep).
- Cost of a full re-embed (cited arithmetic, not measured): `reindex.rs:611-614` cites 12.2 ms per text at one worker and 17-20 ms in an earlier run. 142,888 chunks come to 29 to 48 minutes. The 1,112 renames into `archive/` since January (spec L36) each cost one such pass over the moved file.

**Not established.** The number of orphan points in Qdrant. Whether `doctor` checks for orphan vectors beyond `scan_retired_vector_collections` (`tools/doctor.rs:766`; I did not read its body).

### C3-4. The derived catalog: chunks yes, vectors no, location none

**Claim.** The derived file stores chunk text and a deterministic chunk id. It stores no vector and no embed-input hash. No persistent location exists. The only production creator is a scratch file in a temp directory.

**Evidence.**
- Table `chunk(chunk_id PK, rel_path, chunk_ix, start_line, end_line, entry_token, entry_part, entry_parts, content, content_hash)` (`derived/schema.rs:74-85`), written at `derived/build.rs:572-590`. My first grep for `artifact_chunk` in the derived tree missed it because the table is named `chunk`.
- `chunk_id = sha256(rel_path "\n" chunk_ix "\n" content_hash)` (`build.rs:405-415`). It is deterministic and bound to position: a move or an ordinal shift changes it. It is a different id space from the old UUIDs.
- The `entry` table holds token and line numbers only (`schema.rs:55-61`). The entry titles, which the embed input needs (`indexer.rs:300-357`), are not stored. The embed input cannot be rebuilt from derived rows alone; the build needs the blob body.
- `parse_cache(sha256, rel_path, rules_hash, parser_version, ..., chunks_json)` (`schema.rs:96-107`) is excluded from the canonical dump as "a cache of parse products, not an answer" (`schema.rs:117-118`). `build_full` rewrites the seven `DERIVED_TABLES` in one transaction and leaves `parse_cache` alone (`build.rs:71-73,159`). "After each build the cache holds exactly that build's documents" (`incremental.rs:17-18`). This is the one precedent for a persistent cache inside the derived file.
- Schema rule: "All rows are deterministic: no default is a clock or a random value" (`schema.rs:15-18`).
- One derived file is one checkout: the `checkout` table has one row (`schema.rs:15-18,32-38`). The spec says "every other row joins on the checkout and the key" (spec L225-227). C1 did not build that.
- Placement and lifecycle. The only production caller that creates a derived file is `codescout derive-shadow`: it builds into a scratch file in a temp directory outside the repo and removes it on return (`cli/derive_shadow.rs:4-7,205-218`). `schema::open(path)` creates the file and its parents, sets WAL, applies the schema at `user_version` 0, and refuses a foreign file or a newer version (`schema.rs:123-130,144-216`). No build trigger and no consumer exist (session log L845).

**Not established.** The final location, the build trigger and the consumer; the spec leaves table and column names to the builder (spec L370) and names no path.

### C3-5. File locations and conventions

**Claim.** Per-repo derived data lives under `<root>/.codescout/` and is git-ignored. Per-user data lives under `dirs::data_local_dir()`. The spec names no path for the derived catalog or the machine-state file. There is no existing cache directory convention.

**Evidence: `.codescout/` today (measured with `du`, 4.3 GB in total).**

| Entry | Size | Git state |
|---|---|---|
| `ecc-*-private-*` (six dirs) | about 3.1 GB | not tracked, not ignored (no rule matched in `git check-ignore`) |
| `embeddings/` (code index DBs, its own `.gitignore` is `*`) | 700 MB | ignored (`.gitignore:88`) |
| `usage.db` (+wal, shm) | 361 MB | ignored (`:27-29`) |
| `resident-private/` | 139 MB | ignored (`:23`) |
| `audit/` (five jsonl shards) | 50 MB | four tracked; `202611` untracked |
| `tantivy/` | 13 MB | ignored (`:145`) |
| `measurements/` | 11 MB | partly tracked |
| `call_edges.db` | 7.0 MB | ignored (`:140`) |
| `memories/` | 2.7 MB | tracked |
| logs (`debug.log*`, `diagnostic-*.log`) | about 4 MB | ignored (`:85-86`) |
| `project.toml`, `index-state.json`, `workspace.toml`, `peers.toml`, `libraries.json`, `tmp/`, `private-memories/`, `write.lock*` | small | ignored (`:14,15,91,92,87,117,96,134,139`) |
| `key-space.toml`, `librarian.toml`, `repo.toml`, `requirements.toml`, `system-prompt.md`, `projects/*/memories` | small | tracked (`git ls-files`) |

- There is no `.codescout/.gitignore` at the root. `.codescout/librarian.db*` is ignored (`.gitignore:32-34`), but production code uses `data_local_dir()` instead; only tests use that name under a temp dir (`server.rs:13088,13135`).
- Every root ignore rule for `.codescout/` needs a twin for `crates/*/.codescout/`; test `every_root_codescout_ignore_rule_has_its_crates_twin` (`tests/hook_config.rs`) enforces it (`.gitignore` comment near L41-46). A new file under `.codescout/` costs two ignore lines and keeps that test green.
- Per-user conventions in `src/`: data `dirs::data_local_dir()/librarian/catalog.db` (`librarian/mod.rs:316-320,612-616`, `cli/derive_shadow.rs:340`). Config `dirs::config_dir()/librarian/workspace.toml` (`librarian/workspace.rs:39`) and `$XDG_CONFIG_HOME/codescout` (`config/global.rs:82-100`). State `util::fs::per_user_state_dir()` (`util/fs.rs:530-545`), used for `codescout/guide_hints`, `servers` and `guide_rearm` (`server.rs:500-516`). `dirs::cache_dir()` serves the Kotlin LSP (`lsp/servers/mod.rs:265`) and the heartbeat (`heartbeat.rs:148`). A grep finds no `.codescout/cache` and no use of the git common directory.
- Spec on machine state (spec L53-55 and L297-299): reindex progress rows, gc settings, the embed-error marker and the chunk backfill cursor "move to a small machine-state file next to the derived catalog. It is not derived and holds no fact about a document, so losing it is harmless." The spec also says the cutover builds "a new catalog file" and keeps the old one read-only (spec L350-351).

**Not established.** Whether the six `ecc-*-private-*` directories are meant to stay unignored (not this plan's question; they matter only as proof that an unlisted file under `.codescout/` is offered to `git add -A`).

### C3-6. The embedder: model, dimension, identity

**Claim.** The model is `CodeRankEmbed`, 768 dimensions. The id is known only as an environment string at construction. It is not kept by the service, not recorded next to the vectors, and a same-dimension model swap is invisible.

**Evidence.**
- Model (measured in this session's environment): `LIBRARIAN_EMBED_MODEL=CodeRankEmbed`, `LIBRARIAN_EMBED_URL` a local llama-server, `CODESCOUT_EMBEDDING_DIM=768`, `CODESCOUT_EMBEDDER_MODEL_NAME=CodeRankEmbed-Q4_K_M.gguf` (the last two are code-retrieval variables). The librarian reads environment only (`LibrarianEnv::from_env`, `librarian/mod.rs:121-140`), not `project.toml`. The dimension is fixed in the schema (`catalog/mod.rs:426`) and in the Qdrant collection (measured).
- Construction: `build_tool_context_with` and `reindex_cli` call `create_embedder_with_config(model, url, key)` and `EmbeddingService::with_budget(embedder, chunk_size_for_model(model))` (`librarian/mod.rs:327-340,622-627`). The service keeps `embedder`, `budget_chars` and `concurrency` only (`embedding.rs:5-26`). The `Embedder` trait has `dimensions`, `embed` and `embed_query` (`crates/codescout-embed/src/embedder.rs:108-125`). No model id is reachable at index time through the service.
- Not recorded next to the librarian's vectors (`chunk.rs:631-642`). The code index does record it: `IndexState.indexed_with_model` in `.codescout/index-state.json` (`retrieval/index_state.rs:70`), with a dimension guard `migrate_or_guard_index_dim` (`retrieval/client.rs:504`). That field's doc names the same-dimension swap (CodeRankEmbed and Jina are both 768d).
- Model change today: a different dimension on sqlite bails with a message naming the migration variable, and the opt-in rebuild drops every project's vectors (`indexer.rs:1467-1510`). A same-dimension swap is not detected; old and new vectors mix until `reembed=true`. Qdrant: `ensure_artifacts_collection` returns early for an existing collection and compares no dimension (`retrieval/artifact.rs:50-52`, `artifact_store.rs:394-403`).

**Not established.** The environment of the MCP server process (I read my shell's). Qdrant's own error on a dimension mismatch. Whether a remote server can change weights under one model name: the session-log reviewer says the code index already distrusts a remote model name (L842); I did not find the code for that claim.

### C3-7. Query-time readers and the effect of re-keying

**Claim.** One path reads vectors, through `chunk_id`. Re-keying the search store by `(model, content hash)` breaks one-vector-one-chunk-one-artifact in five places.

**Evidence.**
- Production callers of `semantic_find` (`catalog/find.rs:338`): `tools/find.rs:919` (the query is embedded at `find.rs:761`) and `tools/context.rs:690` (`context.rs:648`). Flow: `store.knn(project_id, query, k)` returns `(chunk_id, distance)` (`artifact_store.rs:306-311`). `rows_by_chunk_ids` (`chunk.rs:690`) maps each chunk id to a `ChunkRow`. `find_by_ids_filtered` (`find.rs:184`) applies the scope filter and hides gc'd rows. The per-artifact cap is applied in KNN order. A chunk id with no row is counted as `unresolved`. Qdrant also pre-filters by `project_id` (one collection per project; `None` fans out over all).
- What breaks if the search store were keyed by `(model, content hash)` and joined through chunk rows:
  1. KNN would return a hash that maps to 0..N chunk rows. `rows_by_chunk_ids` returns a map with one row per key. `SemanticHit.chunk`, `max_per_artifact` and `unresolved` count one to one. N > 1 is real: 3,436 hashes sit under more than one artifact (measured).
  2. `ArtifactVectorStore::delete(artifact_id)` (`artifact_store.rs:242`) and `refile` (`:258`) assume one owner per vector (Qdrant payload filter; sqlite chunk join). A shared vector would vanish for its other owners, or need reference counts.
  3. Project scope: collections are per project and pre-filter on one `project_id` payload. One shared point cannot carry several.
  4. The title is inside the input. The same bytes under another title are another vector, so the key is `embed_input_hash`, never `content_hash` (`chunk.rs:631-640`).
  5. The sqlite cascade trigger (`catalog/mod.rs:438`) deletes a vector row on any chunk-row delete. It would delete a shared vector.
- What does not break: a cache consulted at write time, outside the search path. The search stores stay chunk-keyed.
- Tests that guard the current path and must stay green: `an_id_from_the_production_embed_queue_hydrates_through_semantic_find` (`catalog/find.rs:1218`) and `the_real_sqlite_path_writes_and_reads_the_same_table` (`find.rs:1309`).

**Not established.** The ranking of ties in `doc(find)`; callers of `semantic_find` outside the two named (grep shows no others in `src/`).

### C3-8. Fixtures a plan can use

**Claim.** There is no shared fake embedder. Each test module declares its own struct that implements `codescout_embed::Embedder`. There is a shared in-memory vector store and a real sqlite store over an in-memory catalog.

**Evidence.**
- `indexer.rs` (inside `mod tests`, private): `backfill_svc` (`:2805`, constant 768-d unit vector), `backfill_svc_at_dim` (`:2809`), `backfill_rejecting_svc` (`:2832`, fails on a marker substring), `backfill_recording_svc` (`:2860`, records every text sent and can declare one dimension and return another). The recording one is the shape for "no embed call on a cache hit".
- `tools/reindex.rs`: `OkEmbedder`, `UnitEmbedder`, `FailingEmbedder`, each declared inside a test (`:1085-1380`, `:2067`). `tools/find.rs`: `MockEmbedder` (`:2273`). `tools/context.rs`: `ConstEmbedder` (`:1107`). `embedding.rs`: `CeilingEmbedder` (`:146`) and `SeamRecorder` (`:433`). Integration: `tests/librarian/reindex_concurrency.rs`, `ConcurrentEmbedder` (`:26`).
- Stores: `artifact_store::test_support::{InMemoryArtifactStore, BatchRecordingStore}` (`artifact_store.rs:708-911`, `#[cfg(test)]`). The real `SqliteVecArtifactStore::new(Arc<Mutex<Catalog>>)` over `Catalog::open_in_memory()` (`catalog/mod.rs:794`). Tests inject a temp catalog path through `LibrarianEnv.db` (`server.rs:13088`).
- Derived: `derived/testkit.rs` (`build_files` at `:258`) builds a repository from file contents.
- Guards a plan should name: `a_reused_chunk_embeds_with_the_current_entry_title_not_the_stored_one` (`indexer.rs:2618`), `an_unchanged_chunk_keeps_its_vector_across_a_reindex` (`chunk.rs:1131`), `every_production_construction_site_passes_a_budget` (`embedding.rs:360`).

**Not established.** Whether any test drives a Qdrant-backed reindex end to end without a daemon; the Qdrant tests I saw are `--ignored` (`retrieval/qdrant.rs:1146`).

## Facts every design shares

- The cache key must be `(model_key, embed_input_hash)`. `embed_input_hash` exists (`chunk.rs:643`). `model_key` is new: it must cover the configured model string, the dimension, `budget_chars` and a composition-version constant (C3-2, C3-6).
- The cache sits at the embed seam, before the embedder call, at the three production call sites. A hit still has to be upserted to the search store under the chunk's own id. This differs from the marker skip (`reindex.rs:628`), which uploads nothing.
- The marker skip creates a hole. A chunk skipped as `up_to_date` never reaches the embedder, so it never reaches the cache. If a marked chunk's file later moves, its new chunks miss the cache and are embedded again. A seeding step is therefore needed for the marked chunks, or the skip must also feed the cache.
- Seeding needs a read of vectors back from the store. For Qdrant no such code exists: every scroll in `src/retrieval/` passes `with_vectors(false)` (`qdrant.rs:314,378,494`, `memory.rs:205`). For sqlite a `SELECT id, embedding FROM artifact_vec_v2` suffices. Only marked chunks can be seeded safely: 26,812 of 142,888 (13,770 of 51,724 for this repo). An unmarked chunk cannot be tied to its input; the first reindex after C3 embeds it anyway (`catalog/mod.rs:504-513`) and fills the cache as it goes.
- Writes must be batched into one commit. Per-row commits cost about 400 times more (cited: `indexer.rs:195-210`, 4,691 ms against 12 ms for 400 chunks).
- Size: 142,888 vectors x 3,072 bytes is about 439 MB (measured count, arithmetic).

## Candidate designs

### Design A. A separate per-user cache file next to the catalog (recommended)

- **File.** `vector-cache.db` in the directory of the catalog file; today `<data_local_dir>/librarian/vector-cache.db`. The path is a function of the catalog path (`LibrarianEnv.db`, `librarian/mod.rs:57`). A test that injects a temp catalog path gets an isolated cache. An in-memory catalog (`catalog_db_path` returns None, `catalog/mod.rs:733`) gets an in-memory cache. This follows the spec's own pattern of a machine file "next to the derived catalog" (spec L297-299): whatever location the derived catalog later gets, the cache follows.
- **Schema.** `vec(model_key, input_hash, dim, vector BLOB, last_used)`, primary key `(model_key, input_hash)`, `WITHOUT ROWID`. Plain BLOB, little-endian f32 as `indexer.rs:1446` writes. No sqlite-vec needed: lookup is by exact key, there is no KNN.
- **Key.** `(model_key, embed_input_hash)` as above.
- **Eviction.** By age: rows unused for N days are removed at the end of a reindex that touched the cache. Never on a query path. A row cannot be wrong, only unused. Row cap optional.
- **Rebuild reuse.** The embed seam asks `get(model_key, hash)` first. A hit goes to the normal flush into the search store under the new chunk id. A miss embeds, then `put`s. A derived-catalog rebuild computes the same hashes from its `chunk` rows plus the entry titles read from the blob.
- **Cost.** Model identity plumbing: two `EmbeddingService` construction sites (`librarian/mod.rs:338,625`) and the guard test pattern at `embedding.rs:360`. One new file with its own WAL and busy timeout, of about 439 MB at today's corpus. A policy for a model name that lies. Seeding code (see above). No change to the query path, the search stores or the derived schema.
- **Gain.** A file moved by git costs no embed call. Worktree copies and a catalog rebuild cost none. The cutover's new catalog file keeps its vectors. A switch back to an earlier model hits the old rows. Of today's rows, 11,354 are cross-artifact content duplicates (upper bound).
- **Delivery.** It ships alone against the old indexer, as the catalog scout's S4 says ("SHIPS ALONE: yes if additive", `2026-11-11-artifact-identity-catalog-scout.md:244`).

### Design B. A per-repo cache file in the git common directory

- **File.** `<git-common-dir>/codescout/vector-cache.db`. All worktrees of one repo share it. It moves and dies with the repo. No ignore rules are needed because it lies inside `.git`.
- **Key, eviction, rebuild reuse.** As in A.
- **Cost.** A new location convention: no code uses the git common directory (grep). A fallback is needed for a read-only `.git`. No reuse across repos or clones, and a fresh clone starts empty. The catalog is per user and spans repos (`scope=all`), so storage granularity would differ between the catalog and its cache.
- **Prefer it when** the operator wants "delete the repo, delete its vectors" and accepts no cross-repo sharing.

### Design C. A cache table inside the derived catalog file

- **File.** The undecided persistent derived file; one file per checkout (C3-4).
- **Schema.** A `vector_cache` table like `parse_cache`, excluded from the dump (`schema.rs:117-118`) and left alone by `build_full` (`build.rs:71-73`).
- **Eviction.** Free: prune to the build's live set after each build, as `parse_cache` does (`incremental.rs:17-18`).
- **Rebuild reuse.** A rebuild into the same file reuses the rows.
- **Cost.** Blocked until the derived-file location exists (C1 state document L98). Per checkout: a new worktree or clone starts empty, and so does the cutover's "new catalog file" (spec L350). Live-set pruning drops the vectors of content that exists only on another branch, so a branch switch re-embeds. It breaks the schema's determinism rule (`schema.rs:15-18`) unless the operator accepts a carve-out as for `parse_cache`; the session-log reviewer rejected the plain blob table (L841). Deleting the file deletes the cache, which contradicts "a rebuild reuses them" (spec L230) unless "rebuild" means rewriting the tables in the same file.
- **Prefer it when** the operator wants exactly one derived file and accepts per-checkout caches.

### Rejected

- Re-key the search stores by content. It breaks five things (C3-7).
- Put the cache table in the old `catalog.db`. The cutover builds a new catalog file and keeps the old one read-only (spec L350-351). `snapshot_catalog` backups (`catalog/mod.rs:684-720`) would copy the extra hundreds of MB. Older binaries share that file.

## Recommendation

**Design A.** Reasons:
1. It does not wait for the derived-file location, so C3 ships alone against the old indexer.
2. It is free of the checkout and of the path: it survives a move, a worktree, a catalog rebuild and the cutover's new file, which are the cases the spec's G3 and G5 name.
3. It is a cache outside the derived schema and outside the query path, so neither the determinism rule nor the five breakages of C3-7 apply.

## Open questions for the operator

1. **Location.** A (next to the catalog), B (git common directory) or C (inside the derived file)? A needs no other decision. C waits on the derived-file location.
2. **Model identity.** Trust the configured model string plus dimension (cheap; the code index does so through `indexed_with_model`), or add a probe-vector guard (one embed call per process, compared with a stored probe vector)? The guard also catches weights that change under one name.
3. **Eviction.** Age only (needs no consumer), a row cap, or the live set (needs the derived build as consumer)? I suggest age only first. The days are the operator's number.
4. **Seeding.** Should C3 export the vectors of marked chunks into the cache? It saves 27% of this repo's chunks (13,770 of 51,724). The sqlite side is one query. The Qdrant side needs new read-back code, because none exists. Without seeding, the first reindex fills the cache only for chunks it embeds, and marked chunks stay outside until they change.
5. **Marker skip.** Keep the `embedded_input_hash` skip next to the cache, or replace it by "cache hit and the store holds the point"? The second removes a column but needs a store lookup per chunk.

Ledger checked: 99a4d0be1318c3c1 (fixed; its `indexer.rs:810-854` citation is stale). A title search of open, taken, investigating and zombie bugs for vector, embed and chunk found ten rows (`d25024261adb4d7c`, `13d07bcbd859597d`, `5d362867555d6bfb`, `389d5f429f5194f3`, `4947d15998c9e32f`, `04b1b91a23c3a760`, `a25729bf7e260d81`, `96df68171dad9e18`, `5e12ad64ced01519`, `41b29b7b3a863e2c`). None concerns the librarian vector store: they cover code-retrieval chunking, the embedding runtime profile and the embedding relay.
