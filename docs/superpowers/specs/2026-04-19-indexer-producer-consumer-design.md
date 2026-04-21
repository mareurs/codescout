# Indexer Producer/Consumer Refactor — Design Spec

**Date:** 2026-04-19
**Branch:** `experiments`
**Scope:** `src/embed/index.rs::build_index` and `build_library_index`

## Problem

Current Phase 2+3 of `build_index` runs strictly sequential across file groups:

```
embed group N → drain JoinSet → BEGIN → per-file writes → COMMIT → next group
```

`while let Some(res) = tasks.join_next().await` waits for **all** embed tasks in
a group before any DB write begins. While the `BEGIN…COMMIT` block runs, no
embedding is in flight. On remote-GPU embedders this appears as ~150 ms GPU idle
per group boundary (≈ 10 barriers on 466 files at `file_group_size=50`).

## Goal

Overlap embed(group N+1) with DB write(group N). No other behavior change.

## Non-goals

- Per-file transactions (keep per-group granularity for crash resilience)
- Parallel writers (SQLite is single-writer anyway)
- `spawn_blocking` migration for DB calls (preserve current status quo)
- Pipelining depth > 1 (diminishing returns, more RAM)

## Design

Two async tasks connected by a single `mpsc::channel(1)` rendezvous:

```
embed_producer ──GroupReady { works, embeddings }──► db_writer
  (owns embedder,                                     (owns conn,
   progress_cb, embed_start)                           drift_results)
```

Channel capacity **1** is deliberate — one slot of buffering lets the producer
prepare group N+1 while the writer commits group N. Any larger buffer wastes
RAM without adding overlap.

### Message

```rust
struct GroupReady {
    works: Vec<FileWork>,
    embeddings: Vec<Embedding>, // flat; same order as works[i].chunks
}
```

`FileWork` and `GroupReady` move to module scope (currently `FileWork` is
nested inside `build_index`).

### embed_producer

Transplants current lines ~1658–1734 (from `let mut works_iter` through the
`while let Some(res) = tasks.join_next()` loop), minus the DB section.

Responsibilities:
- Iterate `works` by `file_group_size`
- For each group: spawn `JoinSet` with `Semaphore(max_inflight)` over `BATCH_SIZE`-chunk batches
- Drain batch results in order → flat `Vec<Embedding>`
- Invoke `progress_cb` as batches complete
- `tx.send(GroupReady { works: group, embeddings }).await?`
- On `works_iter` exhaustion: drop `tx` → writer sees channel close

Owned state: `embedder`, `progress_cb`, `total_files`, `files_embedded_so_far`,
`embed_start`.

### db_writer

Transplants current lines ~1735–1828 (the `BEGIN…COMMIT` block) plus the
finalize block (lines ~1830–1860: anchor staleness, `set_meta`,
`set_last_indexed_commit`, `maybe_migrate_to_vec0`).

Responsibilities:
- Loop `while let Some(group) = rx.recv().await`
- Per group: `BEGIN` → `delete_file_chunks` + `insert_chunk` loop + `upsert_file_hash` + optional drift → `COMMIT`
- On channel close: run finalize block (under its own `BEGIN…COMMIT`)
- Return `(indexed: usize, drift_results: Vec<FileDrift>)`

Owned state: `conn`, `drift_results`, `indexed`, `embedding_dims_set`,
`discovered_projects`, `config` (for drift threshold + embed_model).

### Orchestration

```rust
let (tx, rx) = mpsc::channel::<GroupReady>(1);
let writer = tokio::spawn(db_writer(rx, conn, config_clone, project_root_buf, ...));

let embed_result = embed_producer(
    works, Arc::clone(&embedder), tx, progress_cb, total_files,
).await;

// tx dropped when embed_producer returns → writer sees close → runs finalize
let writer_result = writer.await.map_err(|e| anyhow::anyhow!(e))?;

embed_result?;     // embed error takes precedence
let (indexed, drift) = writer_result?;
```

### Error handling

- Embed error → `tx` drops → writer drains remaining groups already sent, runs
  finalize, returns Ok. Caller sees `embed_result?` error. Partial progress
  preserved (same as current behavior).
- Writer error → `tx.send(...).await` on producer returns `Err` → producer
  returns error. Writer's error surfaces via `writer.await?`.
- If both err: embed error reported first (matches user expectation — producer
  is the "work source").

## Constraints

- `Connection` is `Send`, moved into writer task, never shared. No Arc/Mutex.
- `embedder: Arc<dyn Embedder>` already shareable — cloned into each embed task.
- `progress_cb` currently `Option<Arc<dyn Fn(...) + Send + Sync>>` — stays in producer.
- `config` cloned once for writer (it's `Clone`).
- `discovered_projects: Vec<DiscoveredProject>` used only in writer → moved there.

## Testing

One new test, behavior-preserving otherwise:

**Test: `producer_consumer_overlaps_embed_with_db_write`** (in
`tests/integration/` or an integration-style test in `src/embed/index.rs`):

1. Build a project with >= 2 file groups worth of content (e.g. 60 small files,
   `file_group_size=30`).
2. Wrap the underlying embedder or DB path with an instrumented adapter that
   records `(event, timestamp)` pairs: `EmbedStart(group_id)`,
   `EmbedEnd(group_id)`, `WriteStart(group_id)`, `WriteEnd(group_id)`.
3. Run `build_index`.
4. Assert there exists at least one pair `(G, G+1)` such that
   `EmbedStart(G+1) < WriteEnd(G)` — proves overlap.

If instrumentation is too invasive, fall back to:
- Add a `#[cfg(test)]` atomic counter on `CODESCOUT_TEST_WRITE_DELAY_MS` env var
  read at writer entry; set it to 200ms; measure total time of a 2-group run
  vs. (group1_embed + 2 × write_delay + group2_embed). Overlap means total
  < sum-of-parts.

Existing tests must pass unchanged.

## Deliverable

- Single commit: `refactor(embed): pipeline embed and DB writes across groups`
- Extracted `FileWork` + `GroupReady` types
- `embed_producer` + `db_writer` async fns
- Identical public API for `build_index` / `build_library_index`
- One overlap test
- Cargo fmt, clippy -D warnings, cargo test all pass

## Risk

- SQLite calls block the tokio runtime thread inside the writer task. This is
  the same risk the current code has. Document in module comment but don't fix
  here (separate concern).
- If a future change makes `FileWork.chunks` not `Send`, the channel breaks.
  Currently `Vec<RawChunk>` is `Send` (only `String` / `Option<String>` fields).
