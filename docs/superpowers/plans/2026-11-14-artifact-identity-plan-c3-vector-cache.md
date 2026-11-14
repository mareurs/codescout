# Artifact identity Plan C3: content-keyed vector cache Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A vector the embedder made once is reused whenever the same text is embedded again under the same model, so a file moved by git, a copied file or a rebuilt catalog costs no embed call, for text that was embedded after the cache existed.

**Architecture:** A new per-user file `vector-cache.db` sits in the directory of the catalog file and maps `(model_key, embed_input_hash)` to a vector. `EmbeddingService` gets one seam method that reads the cache before it calls the embedder and stages each fresh vector. Each site that already flushes vectors to the search store also flushes the cache, in one transaction. The search stores, the query path and the catalog schema do not change. Nothing evicts a row (C3-R4).

**Tech Stack:** Rust, `rusqlite` (a plain rowid table, WAL, no sqlite-vec), `parking_lot`, the existing `catalog::chunk::embed_input_hash`.

**Spec:** `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md` (v5): Constraints, "Search needs embeddings" (L99-101); "The catalog is derived", last bullet, "a rebuild reuses them" (L230); "Index inputs, guard, machine state and entry ids", machine state, "next to the derived catalog ... losing it is harmless" (L297-299). Evidence: scout `docs/research/2026-11-14-artifact-identity-plan-c3-scout.md` (id `561841b0ae4a01d8`). Review of the first draft: session log `abff63149e3c24e8`, F-19 to F-23 and W-12; this text applies all of them.

## Global Constraints

- `TOOL_SURFACE_CHAR_BUDGET` is exactly 57_664 (headroom 0, `src/server.rs`). No advertised tool description or schema changes: the new `embed_cache` fields are response output, and the `reembed` schema text (`src/librarian/tools/librarian.rs:89`) stays as it is. If a schema edit is unavoidable, it is character-neutral.
- Write manual pages in Simplified Technical English (STE). Agent-facing prompt text (guide text, `embed_note`, any error text a tool shows) states the current situation only: no "was added", "formerly", "now". No absolute path in any text a tool shows.
- Guide text names no path or Rust field that exists only in this repo (`no_guide_names_a_path_or_field_only_this_repo_has`, `src/prompts/mod.rs:2809`).
- A constant name must not contain `CAP`, `LIMIT`, `MAX`, `BUDGET` or `THRESHOLD`, or the line above it carries `// cap-class: NOT_A_CAP — <why>` (`is_cap_shaped`, `tests/result_caps.rs:680`; gate `every_cap_constant_is_classified`, `:1224`).
- Production code in `vector_cache.rs` uses none of `fs::read`, `fs::read_to_string`, `fs::OpenOptions`, `File::open`, `File::options`, `OpenOptions::new`, `io::read_to_string`, `tokio::fs` (read-door gate, `src/librarian/read_door.rs:1414`). Its tests are an inline `#[cfg(test)] mod tests`, which that gate blanks.
- A cache failure (open, get, put, corrupt file, read-only directory, busy lock, newer format) never fails a call and never returns a wrong vector. It degrades to a miss and is counted (C3-R7).
- No new dependency. No change to the catalog schema, the search stores, `semantic_find`, or the `embedded_input_hash` marker skip. No eviction code.
- Targeted tests: `scripts/with-slot.sh cargo test --lib <filter>`. Implementers do not run `./scripts/gate.sh`; the controller runs it in the last task.
- Shared checkout: stage paths, run `git diff --cached --name-only` as its own call, read `git diff --cached`, then `git commit -m "..." -- <paths>` (`-m` before `--`; `docs/conventions/shared-checkout-commit-sequence.md` § 4). Touch only the files a task names.

## File map

- Create `src/librarian/vector_cache.rs`: key, store, tally, status.
- Modify `src/librarian/embedding.rs`: the seam, the test double, the guard tests.
- Modify `src/librarian/mod.rs`: module line; the two `with_budget` sites (`build_tool_context_with`, and `reindex_cli`, which is `#[cfg(test)]`).
- Modify `src/librarian/tools/reindex.rs`: seam at the drain and the report.
- Modify `src/librarian/indexer.rs` and `src/cli/backfill_chunks.rs`: seam at backfill and `index_repo`, backfill report.
- Modify `docs/manual/src/concepts/librarian-embedded.md` and `src/prompts/guides/librarian-runtime.md`.

## Rulings (controller C3-R1..R8, then mine R9..R14; the operator may overrule any)

- **C3-R1** Design A of the scout: `vector-cache.db` in the directory of the catalog file; the path is a function of `LibrarianEnv.db`; an in-memory catalog gets an in-memory cache. It ships alone against the old indexer. Cost if wrong: a per-user file shared by every repo, worktree and profile, with no "delete the repo, delete its vectors". Moving it later changes one function, `VectorCache::beside`.
- **C3-R2** Key is `(model_key, embed_input_hash)`. `model_key` covers the configured model string, `budget_chars` and `COMPOSITION_VERSION`. The vector dimension is covered per lookup, not in the key string: see R11. Cost if wrong: a model change that keeps the string reuses old vectors (R3).
- **C3-R3** No probe-vector guard. Named follow-up. Cost: a remote server that changes its weights under one model name, or two servers behind one model name, serve stale vectors until `reembed=true` or the file is deleted.
- **C3-R4 (replaced by the controller after F-19)** NO eviction in C3: no age rule, no `last_used` column, no eviction call. Cost if wrong: the file only grows, about 3 KB per distinct embed input and about 439 MB at today's corpus (142,888 chunks x 3,072 bytes, scout arithmetic), and shrinks only when the operator deletes it. The trap a later eviction must avoid is in "Still open".
- **C3-R5 (rewritten after F-20)** No seeding, and no change to the `embedded_input_hash` skip. Queueing is per artifact. `index_repo_sync` queues an artifact only when its `embedded_sha256` stamp differs from the file hash (`needs_embed`, `indexer.rs:671-675`) or when `reembed=true` (`force_embed`, `indexer.rs:696` and `:732`). The v14 migration clears no stamp (`catalog/mod.rs:504-519`), and the only `embedded_sha256 = NULL` is `clear_kept_row` (`indexer.rs:1042`). So an artifact embedded before C3 and unchanged is never queued, and **on the day C3 ships the cache is empty for the whole existing corpus**. It fills only from text embedded after C3: a new or changed file, or a `reembed=true` reindex. A file moved by git hits only if its text was embedded after C3 shipped. Cost if wrong: the spec's "a rebuild reuses them" (L230) is not met for the cutover rebuild unless each project first runs one `reindex` with `reembed=true` (full embed cost, once), or the follow-up "seeding" ships (sqlite: one `SELECT` over `artifact_vec_v2` joined to the markers; Qdrant: read-back code that does not exist, scout claim not re-verified). Seeding can export only chunks that carry a marker (26.6% of this repo's chunks, scout figure); `reembed=true` is the complete lever. This text is in the manual page.
- **C3-R6** The cache sits at the embed seam of every site that embeds a chunk (see Facts verified: two production sites plus `index_repo`). A hit is upserted into the search store under the chunk's own id. Cache writes are staged and written in one transaction per flush.
- **C3-R7** A cache failure degrades to a miss and the report carries a count, not a stack. A stored row of another dimension is a miss.
- **C3-R8 (changed after F-22)** One plain table `vec(model_key, input_hash, dim, vector)`: a normal rowid table with `PRIMARY KEY (model_key, input_hash)`. Not `WITHOUT ROWID`: that suits rows under about 200 bytes, and a row here is about 3.1 KB (sqlite.org/withoutrowid.html § 4). No `last_used` column, because nothing reads it without eviction. Little-endian f32 as `write_embeddings_v2_with` writes (`indexer.rs:1446`). No sqlite-vec.
- **C3-R9 (mine)** `reembed=true` skips cache reads and still stages and refreshes every row. It is already "the escape for a marker that lies"; without this it would be no escape from a bad cache row. Cost if wrong: re-pointing a store that lost its points with `reembed=true` pays full embed cost. Consequence: a model switch is applied only by `reembed=true` (an ordinary reindex does not queue a stamped artifact), so it never reads the cache; only a dimension change, which drops vectors and goes through `backfill-chunks`, does. The manual and the guide therefore make no claim that a switch back to an earlier model reuses vectors.
- **C3-R10 (mine, refines R2)** One builder, `with_vector_cache(model, cache)`, sets the cache and the model key together. A service holds a key only with a cache, and the default constructors hold neither, so the seam is a pass-through for every existing test. Tests that want a cache use the model string `"test-model"`. Cost: R2's "test constructors get a fixed key" is not followed; a default key would let a production site that forgot the model write into a shared namespace.
- **C3-R11 (mine, corrects R2 and the scout)** `RemoteEmbedder::dimensions()` is 0 until its first successful embed, so the dimension cannot be read at construction. `get` takes the live `dimensions()`: 0 is a miss; a row whose `dim` differs is a miss and the next put replaces it. Cost: for a remote embedder, with `LIBRARIAN_EMBED_CONCURRENCY=N`, up to N lookups of a process's first window can see dimension 0 and miss before any embed returns (the drain runs `buffer_unordered(svc.concurrency())`, `reindex.rs:635`): up to N extra embeds per process, not one.
- **C3-R12 (mine)** `index_repo` is wired although it has no production caller, so a future caller cannot bypass the cache. Its tallies are not reported. Backfill reports `embed_cache`.
- **C3-R13 (mine, changed after F-23)** Robustness. The file is opened lazily at the first call that needs it (no file and no I/O at construction, so CLI commands that never embed touch nothing). Busy timeout 250 ms, not the catalog's 5 s, because a cache must not stall a reindex. A failed flush drops its staged rows and counts them. `PRAGMA user_version` is 1; a file with a newer version is unavailable and left alone. A corrupt file is left untouched (no automatic delete). A failed open is retried at the start of each reindex, backfill or `index_repo` call (`rearm`), not at each lookup, because the MCP server keeps one `VectorCache` for the life of the process (`try_build_runtime_with` runs once, `src/server.rs:441`). Cost: a cache that cannot open costs one failed open per call.
- **C3-R14 (mine)** Report meaning. In the reindex response `embedded` and `embedded_top` count only vectors the embedder made; vector-cache hits are `embed_cache.hits`. In the backfill report `embedded` stays "vectors written" and `embed_cache.hits` says how many came from the cache. A landed hit is marked with `embedded_input_hash` like any landed vector (existing `flush_vectors` behaviour), so the next run skips it.

## Review Focus

1. The same text under another title, another model string, another budget or another composition version must never hit; a long input's segmentation and pooling are pinned to `COMPOSITION_VERSION` (Task 2).
2. An unknown or changed dimension must never return a vector of the wrong width; a lookup before the first embed is a miss (Tasks 1 and 2).
3. A corrupt, newer-format, read-only, locked or unavailable cache leaves reindex and backfill complete with the failure counted, and a failed open is retried by the next call (Tasks 1, 3, 4).
4. `reembed=true` is never served a stored vector, and a cache attached after the content was embedded stays empty until `reembed=true` fills it (Task 3).
5. Two processes share one file: a flushed row is visible to the other connection and an opening process takes no write lock (Task 1).
6. A rebuilt catalog and a moved file cost zero embed calls, the vector lands under the new chunk id, and no production `embed_artifact` call bypasses the seam (Tasks 3 and 4).

---

### Task 1: The cache store

**Files:**
- Create: `src/librarian/vector_cache.rs`
- Modify: `src/librarian/mod.rs` (add `pub mod vector_cache;` beside `pub mod embedding;`)
- Test: inline `mod tests` of `vector_cache.rs`

**Interfaces:**
- Produces, all in `crate::librarian::vector_cache`:

```rust
pub const VECTOR_CACHE_FILE: &str = "vector-cache.db";
/// Version of how (title, text) becomes the stored vector in `EmbeddingService::embed_artifact`:
/// the join, the segmentation and the pooling. Bump it when any of them changes.
pub const COMPOSITION_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelKey(String);
impl ModelKey {
    /// `format!("{model}|{budget_chars}|{COMPOSITION_VERSION}")`
    pub fn new(model: &str, budget_chars: usize) -> Self;
    pub fn as_str(&self) -> &str;
}

#[derive(Debug, PartialEq)]
pub enum CacheLookup { Hit(Vec<f32>), Miss, Failed }
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FlushOutcome { pub stored: usize, pub failed: usize }
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum CacheStatus { #[default] Off, On, Unavailable }
impl CacheStatus { pub fn as_str(self) -> &'static str } // "off" | "on" | "unavailable"

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CacheTally { pub status: CacheStatus, pub hits: usize, pub stored: usize, pub errors: usize }
impl CacheTally {
    pub fn note_item(&mut self, from_cache: bool, cache_failed: bool); // hits += from_cache; errors += cache_failed
    pub fn note_flush(&mut self, f: FlushOutcome);                     // stored += f.stored; errors += f.failed
    pub fn to_json(&self) -> serde_json::Value;                        // {"status","hits","stored","errors"}
}

pub struct VectorCache { /* Send + Sync; the lock is never held across an await */ }
impl VectorCache {
    pub fn beside(catalog_path: Option<&std::path::Path>) -> Self; // Some(p): file `VECTOR_CACHE_FILE` in p's parent; None, or no parent: in memory
    pub fn open(path: &std::path::Path) -> Self;                   // lazy: nothing is touched until first use
    pub fn in_memory() -> Self;
    pub(crate) fn open_with_busy_timeout(path: &std::path::Path, busy: std::time::Duration) -> Self;
    pub fn path(&self) -> Option<&std::path::Path>;
    pub fn status(&self) -> CacheStatus; // On, or Unavailable after an open failed
    /// Forget a failed open so the next call that needs the file opens it again. No effect otherwise.
    pub fn rearm(&self);
    pub fn get(&self, model: &ModelKey, dim: usize, input_hash: &str) -> CacheLookup;
    pub fn stage(&self, model: &ModelKey, input_hash: &str, vector: &[f32]) -> bool;
    pub fn flush(&self) -> FlushOutcome;
    pub(crate) fn transactions(&self) -> usize; // committed flush transactions only
    #[cfg(test)] pub(crate) fn len(&self) -> usize;
    #[cfg(test)] pub(crate) fn busy_timeout_ms(&self) -> Option<i64>;
}
```

- [ ] **Step 1: Write the failing tests.** Add the module with every signature above as a `todo!()` stub, so the tests compile and fail by panic, not by a compile error. File-backed tests use a `TempDir`; permission cases are `#[cfg(unix)]` and skip loudly when the process ignores modes, as `a_failed_append_never_advances_the_watermark` in `src/librarian/catalog/audit/shard.rs` does.
  - `a_stored_vector_comes_back_bit_for_bit_after_a_flush`: stage `[0.5, -1.25, 3.0e-7, f32::MIN_POSITIVE / 2.0]`, `flush()` is `FlushOutcome { stored: 1, failed: 0 }`, `get(&key, 4, "h1")` is `Hit` with equal `to_bits()`.
  - `a_staged_vector_is_not_visible_before_its_flush`: `Miss` before, `Hit` after.
  - `the_key_is_the_model_and_the_input_hash`: another model string, another budget: `Miss`. `ModelKey::new("m", 8000).as_str()` equals `format!("m|8000|{COMPOSITION_VERSION}")`.
  - `a_row_of_another_dimension_is_a_miss_and_the_next_put_replaces_it`: 3-wide row; `get(.., 4, ..)` is `Miss`; stage a 4-wide vector for the same key, flush; `get(.., 4, ..)` is `Hit`, `get(.., 3, ..)` is `Miss`; `len() == 1`.
  - `an_unknown_dimension_is_a_miss`: `get(.., 0, ..)` on an existing row is `Miss` (C3-R11).
  - `a_row_whose_blob_is_not_dim_times_four_bytes_is_failed_not_returned`: file-backed; corrupt the row through a second `rusqlite::Connection` (`UPDATE vec SET vector = zeroblob(5)`); `get` is `Failed`.
  - `one_flush_is_one_transaction`: stage 400, flush: `transactions() == 1`; flush with nothing staged: still 1; stage 3, flush: 2. Mutation named: a per-row commit fails this.
  - `the_table_is_a_rowid_table_keyed_by_model_and_input_hash` (C3-R8): the `sql` of `vec` in `sqlite_master`, read through a second connection, contains `PRIMARY KEY (model_key, input_hash)` and does not contain `WITHOUT ROWID` or `last_used`.
  - `an_unused_cache_creates_no_file`: `beside(Some(tmp/catalog.db))`, dropped unused: no `vector-cache.db`.
  - `the_cache_file_sits_beside_the_catalog_file`: `beside(Some(tmp/sub/catalog.db))`, stage and flush: `tmp/sub/vector-cache.db` exists and `path()` names it; `beside(None).path()` is `None`; `beside(Some(Path::new("catalog.db"))).path()` is `Some("vector-cache.db")`.
  - `the_production_busy_timeout_is_250_ms`: `open(path).busy_timeout_ms() == Some(250)`.
  - `a_corrupt_file_degrades_to_a_miss_and_is_left_untouched`: 200 garbage bytes; `get` is `Failed`; `stage` is `false`; `status()` is `Unavailable`; the file bytes are unchanged (the test reads them).
  - `a_failed_open_is_retried_after_rearm_and_not_before`: garbage file, `get` is `Failed`; delete the file; `get` is still `Failed` (no retry per lookup); `rearm()`; `get` is `Miss` and `status()` is `On`.
  - `a_file_of_a_newer_format_is_unavailable_and_unchanged`: `PRAGMA user_version = 99` set through a raw connection; the cache is `Unavailable` and the pragma still reads 99.
  - `a_read_only_directory_gives_an_unavailable_cache`: directory 0555; `get` is `Failed`; nothing is created.
  - `an_opening_process_takes_no_write_lock_and_a_locked_file_fails_only_the_flush`: initialise a file cache, then hold `BEGIN IMMEDIATE` on a second connection; a new `open_with_busy_timeout(path, 50 ms)` answers `get` (`Miss`) at once; `stage` then `flush` gives `FlushOutcome { stored: 0, failed: 1 }`; after the lock is released, stage and flush give `stored: 1` (the cache is not off for good).
  - `a_second_connection_sees_the_flushed_rows`: two caches on one file; flush on one, `Hit` on the other.
  - `an_empty_or_non_finite_vector_is_not_staged`: `[]`, `[f32::NAN]`, `[f32::INFINITY]` give `false`.
  - `the_tally_counts_and_prints_what_a_run_did`: `note_item(true, false)`, `note_item(false, true)`, `note_flush({stored: 3, failed: 2})`: `hits 1`, `errors 3`, `stored 3`; `to_json()` has exactly the four keys and `"status": "off"` for the default.
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib librarian::vector_cache`. Expected: every test FAILS (panic in a `todo!()`), none is a compile error. State which, if any, passes.
- [ ] **Step 3: Implement** the module. Decisions the signatures do not fix:
  - **Open** (lazy, inside the one mutex, on the first `get`, `stage` or `flush`): `create_dir_all(parent)`; `Connection::open`; `journal_mode = WAL` and `synchronous = NORMAL` via `pragma_update`; `busy_timeout` from the field. Read `user_version`: 1 is done and takes no write lock; 0 runs `CREATE TABLE IF NOT EXISTS vec (model_key TEXT NOT NULL, input_hash TEXT NOT NULL, dim INTEGER NOT NULL, vector BLOB NOT NULL, PRIMARY KEY (model_key, input_hash))` and sets 1 in one `BEGIN IMMEDIATE`; above 1 is `Unavailable`. Any open error is `Unavailable` and is logged once with `tracing::warn!`. The in-memory cache skips WAL. `rearm` sets an `Unavailable` state back to unopened and touches no file.
  - **get**: `dim == 0` is `Miss`; `Unavailable` is `Failed`; no row is `Miss`; `row.dim != dim` is `Miss`; `blob.len() != dim * 4` or any SQL error is `Failed`. `get` never sees staged puts.
  - **stage**: `false` for an empty vector, a non-finite component, or an unavailable cache; otherwise append `(model.as_str(), hash, vector.len(), le-f32 bytes)` to the staging list.
  - **flush**: take the staging list; if empty, return the default and open nothing. In ONE transaction run each row as `INSERT OR REPLACE`. On commit, `stored` is the row count and `transactions` increments. On any error the transaction rolls back and `failed` is the row count (the rows are dropped, not retried).
  - Use `parking_lot::Mutex<Inner>` for the connection state and the staging list.
- [ ] **Step 4: Run** `scripts/with-slot.sh cargo test --lib librarian::vector_cache` (PASS), then `scripts/with-slot.sh cargo test --lib read_door` (the gate `no_librarian_production_read_bypasses_the_read_door` stays green with the new file), then `git add src/librarian/vector_cache.rs` and `scripts/with-slot.sh cargo test --test result_caps every_cap_constant_is_classified` (the gate lists tracked files, so stage first; PASS), then `scripts/with-slot.sh cargo clippy --lib --no-default-features -- -D warnings` (clean).
- [ ] **Step 5: Commit** by pathspec: `feat(librarian): a per-user vector cache file keyed by model and embed input (C3 task 1)`.

### Task 2: Model identity and the seam on `EmbeddingService`

**Files:**
- Modify: `src/librarian/embedding.rs` (struct, builder, seam, test double, tests)
- Modify: `src/librarian/mod.rs` (`build_tool_context_with` L338-343 and `reindex_cli` L625-629: bind the cache, chain `.with_vector_cache(model, cache)`; a behavioural test beside `embedding_concurrency_built_from`, L896-917)
- Test: `embedding.rs` `mod tests`, `mod.rs` `mod tests`

**Interfaces:**
- Consumes: Task 1's `VectorCache`, `ModelKey`, `CacheLookup`, `CacheStatus`, `FlushOutcome`, `COMPOSITION_VERSION`; `crate::librarian::catalog::chunk::embed_input_hash(title: Option<&str>, text: &str) -> String` (`chunk.rs:643`).
- Produces, in `embedding.rs`:

```rust
pub struct CachedEmbed { pub vector: anyhow::Result<Vec<f32>>, pub from_cache: bool, pub cache_failed: bool }
impl EmbeddingService {
    /// Sets the cache and the model key together (C3-R10). Chain it after `with_budget`.
    pub fn with_vector_cache(self, model: &str, cache: std::sync::Arc<VectorCache>) -> Self;
    pub fn model_key(&self) -> Option<&ModelKey>;
    pub fn vector_cache(&self) -> Option<&VectorCache>;
    pub fn cache_status(&self) -> CacheStatus; // Off without a cache
    pub fn rearm_cache(&self);                 // `VectorCache::rearm`; nothing without a cache
    /// The seam. Without a cache: `embed_artifact` and nothing else.
    pub async fn embed_artifact_cached(&self, title: Option<&str>, body: &str, read_cache: bool) -> CachedEmbed;
    pub fn flush_cache(&self) -> FlushOutcome; // default outcome without a cache
}
#[cfg(test)] pub(crate) mod test_support {
    /// Counts texts, can declare one width and return another, can learn its width late, can fail.
    pub(crate) struct CountingEmbedder { /* ... */ }
    impl CountingEmbedder {
        pub(crate) fn new(dim: usize) -> std::sync::Arc<Self>;                       // declares and returns `dim`, vector [1, 0, ...]
        pub(crate) fn declaring(declared: usize, returned: usize) -> std::sync::Arc<Self>;
        pub(crate) fn learning(dim: usize) -> std::sync::Arc<Self>;                  // `dimensions()` is 0 until its first embed
        pub(crate) fn lengths(dim: usize) -> std::sync::Arc<Self>;                   // v[0] = chars of the text, v[1] = 1.0, rest 0
        pub(crate) fn set_failing(&self, on: bool);
        pub(crate) fn calls(&self) -> usize;                                         // texts embedded
        pub(crate) fn seen(&self) -> Vec<String>;                                    // exact texts, in order
    }
}
```

- [ ] **Step 1: Write the failing tests.** In `embedding.rs`, each on `EmbeddingService::new(emb.clone()).with_vector_cache("test-model", cache.clone())` with `VectorCache::in_memory()` unless stated:
  - `a_service_without_a_cache_embeds_every_time`: two calls, `from_cache` false both, `calls() == 2`, `cache_status()` is `Off`.
  - `a_second_identical_input_is_served_from_the_cache_after_a_flush`: first call `from_cache == false`; `flush_cache()` is `stored: 1`; the cache holds the row under `embed_input_hash(Some("t"), "b")` (`cache.get(&model_key, 4, &hash)` is `Hit`); the second call is `from_cache == true`, `calls() == 1`, vector equal.
  - `an_unflushed_vector_is_not_served`: second call before any flush: `from_cache == false`, `calls() == 2`.
  - `the_model_string_and_the_budget_each_change_the_key`: one shared cache; a service with model `"m2"`, and one with `with_budget(.., 999)`: each misses the row the first service stored; a service with the same model and budget hits.
  - `the_same_text_under_another_title_is_a_miss`: `("A", "body")` stored; `("B", "body")` and `(None, "body")` miss.
  - `an_unknown_live_dimension_makes_the_first_lookup_a_miss`: `CountingEmbedder::learning(4)`; seed the cache with the rows for `("t", "b")` and `("t", "c")`; the first call is `from_cache == false` and embeds once (the double now declares 4); a call for `("t", "c")` hits and `calls()` stays 1. This is the `RemoteEmbedder` shape (C3-R11).
  - `a_vector_of_the_wrong_width_is_not_cached`: `CountingEmbedder::declaring(4, 3)`: `vector` is `Ok` (3 wide), `cache_failed == true`, `flush_cache()` is `stored: 0`.
  - `reading_the_cache_can_be_skipped_and_the_write_still_refreshes_it`: row seeded with `[9.0, 9.0, 9.0, 9.0]`; `read_cache = false` returns the embedder's vector and `from_cache == false`; after `flush_cache()` a `read_cache = true` call returns the embedder's vector, not the seeded one.
  - `a_failing_cache_degrades_to_the_embedder`: a `VectorCache::open` over a garbage file: `vector` is the embedder's, `from_cache == false`, `cache_failed == true`, `cache_status()` is `Unavailable`.
  - `an_embedder_error_is_returned_and_nothing_is_cached`: `set_failing(true)`: `vector` is `Err`, `flush_cache()` is `stored: 0`.
  - `the_text_sent_to_the_embedder_is_pinned_to_the_composition_version`: `embed_artifact(Some("T"), "B")` sends exactly `"T\n\nB"` and `embed_artifact(None, "B")` sends `"\n\nB"`. The failure message says: a change to the join, the segmentation or the pooling must bump `COMPOSITION_VERSION`.
  - `the_segmentation_and_pooling_of_a_long_input_are_pinned_to_the_composition_version`: `CountingEmbedder::lengths(4)`, `with_budget(emb, 50)`, `embed_artifact(None, &"x".repeat(500))`. Measure on the unchanged code, then write in as literals: the segment lengths in `seen()` and the pooled, normalised vector rounded to six decimals. Same failure message as above. (`a_pooled_artifact_vector_is_renormalised_to_unit_length` tests unit length only, so a changed segment size would pass it.)
  - `every_production_construction_site_attaches_the_vector_cache_with_its_model`, beside `every_production_construction_site_passes_a_budget` (`embedding.rs:360`), same text-scan device over `src/librarian/mod.rs`. Bind the cache to a local named `cache` before each builder chain, so the chained call is the short line `.with_vector_cache(model, cache)`. rustfmt cannot split a call that fits in one line at 16 columns, so the guard matches plain substrings like the two existing guards and survives `./scripts/fmt-mine.sh` (F-21: a one-line `.with_vector_cache(model, std::sync::Arc::new(...))` would be split and match 0 times). Assert: `src.matches(".with_vector_cache(model, cache)").count()` equals `src.matches("EmbeddingService::with_budget(").count()`; `src.matches("VectorCache::beside(Some(&db_path))").count()` equals it too; `src.matches(".with_vector_cache(\"").count() == 0` (a literal model string); and a paired positive `>= 1` so removing both sites cannot pass.

  In `mod.rs`, `#[cfg(feature = "remote-embed")]`, mirroring `embedding_concurrency_built_from`: `the_service_a_reindex_runs_on_carries_a_vector_cache_beside_the_catalog`: with `db: tmp/catalog.db` and `embed_model: "any-model"`, `ctx.embedding` has `cache_status() == On`, `vector_cache().unwrap().path() == Some(tmp/vector-cache.db)`, `model_key().unwrap().as_str().starts_with("any-model|")`, `embedder.dimensions() == 0` (the fact R11 rests on), and `tmp/vector-cache.db` does not exist yet (lazy).
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib librarian::embedding` and `scripts/with-slot.sh cargo test --lib librarian::tests`. Expected: the new tests FAIL (add stubs first so they fail by assertion). The existing `every_production_construction_site_*` tests stay green. Report which tests were RED and which were guards.
- [ ] **Step 3: Implement.** Fields: `seam: Option<CacheSeam { cache: Arc<VectorCache>, model: ModelKey }>`, `None` in `new` and `with_budget`. `with_vector_cache` builds `ModelKey::new(model, self.budget_chars)`. `embed_artifact_cached`: without a seam, return `embed_artifact`'s result; else `hash = embed_input_hash(title, body)`; if `read_cache`, `get(&model, self.embedder.dimensions(), &hash)` (`Hit` returns `from_cache: true`; `Failed` sets `cache_failed`); call `embed_artifact`; on `Ok(v)`, stage `(model, hash, v)` only when the live dimension is 0 or equals `v.len()`, and set `cache_failed` when staging is refused. No lock is held across an `.await`. `flush_cache` calls `flush()`. In `mod.rs`, in both functions, add `let cache = std::sync::Arc::new(vector_cache::VectorCache::beside(Some(&db_path)));` right after `db_path` is known (the cache is lazy, so an unused binding costs nothing) and chain `.with_vector_cache(model, cache)` after the existing `.with_concurrency(env.embed_concurrency.unwrap_or(1))`. Keep that `.with_concurrency(...)` text byte-identical: `every_production_construction_site_passes_the_concurrency_setting` counts it.
- [ ] **Step 4: Run** the two commands of Step 2 (PASS), `scripts/with-slot.sh cargo test --lib read_door`, and `scripts/with-slot.sh cargo clippy --lib --no-default-features -- -D warnings`. Then run `./scripts/fmt-mine.sh --check` (or the repo's documented fmt check) and re-run the guard, to prove it survives the formatter.
- [ ] **Step 5: Commit** by pathspec: `feat(librarian): EmbeddingService carries a vector cache and a model key (C3 task 2)`.

### Task 3: The reindex tool reads and writes the cache

**Files:**
- Modify: `src/librarian/tools/reindex.rs` (drain at L623-635; `flush_vectors` L49-101 and its two calls L643 and L690; the response L909-910 and the `embed_note`, L976-)
- Test: `mod tests` of `reindex.rs`

**Interfaces:**
- Consumes: Task 2's `embed_artifact_cached`, `flush_cache`, `rearm_cache`, `cache_status`, `CountingEmbedder`; Task 1's `CacheTally`.
- Produces: the reindex response gains `"embed_cache": {"status","hits","stored","errors"}` (`CacheTally::to_json`), always present (`"off"` when no cache is attached).

- [ ] **Step 1: Write the failing tests.** Add a helper `external_store_ctx_with(roots, store, svc: EmbeddingService) -> ToolContext` and make `external_store_ctx` (L1372-1400) call it with its present `UnitEmbedder` service, so no existing test changes. Use `ledger_text` (no file name inside, so a move keeps every chunk text) and `RecordingStore::inner.chunk(chunk_id) -> Option<(artifact_id, Vec<f32>)>`.
  - `a_moved_file_is_not_embedded_again_and_its_vectors_land_under_the_new_chunk_ids`: service `CountingEmbedder::new(4)` plus a cache `Arc` the test keeps. Run 1 on `docs/specs/old.md`: `embedded == n >= 5`, `embed_cache.hits == 0`, `embed_cache.stored == n`, `calls() == n`. `std::fs::rename` to `docs/specs/new.md`; run 2: `calls()` unchanged, `embedded == 0`, `embed_cache.hits == n`, `embed_error_count == 0`. For every `chunks_for(new_artifact_id)` row: `store.inner.chunk(&row.chunk_id)` is `Some((new_artifact_id, [1.0, 0.0, 0.0, 0.0]))`; no old chunk id is in the store. Mutation named: a seam that reads the cache but never upserts the hit fails the store assertion; a drain still calling `embed_artifact` fails `calls()`.
  - `a_rebuilt_catalog_reuses_the_cache`: after run 1, a second `ToolContext` with a fresh `Catalog::open_in_memory()`, a fresh `RecordingStore`, the same cache `Arc` and the same embedder: `calls()` unchanged, `hits == n`. This is the spec's "a rebuild reuses them" (L230) for text embedded after the cache existed.
  - `a_cache_attached_after_the_content_was_embedded_stays_empty_until_reembed` (F-20): run 1 with the cacheless `external_store_ctx` stamps every artifact. Replace `ctx.embedding` with `Some(Arc::new(EmbeddingService::new(CountingEmbedder::new(4)).with_vector_cache("test-model", cache.clone())))`. Run 2: `embedded == 0`, `embed_cache.hits == 0`, `embed_cache.stored == 0`, `cache.len() == 0`, `calls() == 0`. Run 3 with `reembed: true`: `embed_cache.stored == n`, `cache.len() == n`. Rename the file and run 4: `hits == n`, `calls()` unchanged since run 3.
  - `a_hit_chunk_is_marked_embedded_so_the_next_run_skips_it`: after the moved-file run `marked_chunks(&ctx) == n`; a third run reports `embedded 0`, `embed_cache.hits 0`, `embed_cache.stored 0`.
  - `embedded_counts_the_embedders_work_and_hits_are_counted_apart`: a moved file plus a new file in one run: `embedded` equals the new file's chunks, `hits` the moved file's; `embedded_top` names only the new file.
  - `the_note_names_reused_vectors`: the move fixture plus an UNCHANGED sibling `docs/specs/keep.md` (its own `ledger_text`), so `total_unchanged > 0` and the "nothing needed a new vector" branch (`reindex.rs:976`) is reachable. Run 2: `embed_note` contains `"reused from the vector cache"` and does not contain `"nothing needed a new vector"`. Mutation named: dropping the `hits == 0` condition fails the negative half.
  - `the_cache_is_written_in_one_transaction_per_store_batch`: the 70-file fixture of `a_reindex_writes_vectors_in_batches_not_one_request_per_chunk` (`write_specs`): `cache.transactions()` equals `store.batch_sizes.lock().len()` (64 vectors fill the first batch) and `cache.len() == embedded`.
  - `a_cache_that_cannot_open_never_fails_the_reindex_and_the_report_counts_it`: cache over a garbage file: `embedded == n`, `embed_error_count == 0`, `embed_cache.status == "unavailable"`, `embed_cache.errors == n`; the file bytes are unchanged.
  - `a_cache_that_failed_to_open_is_retried_by_the_next_reindex`: as above, then delete the garbage file and run again with `reembed: true`: `embed_cache.status == "on"` and `stored == n`.
  - `reembed_skips_cache_reads_and_refreshes_the_rows`: run 1; `call(json!({"reembed": true}))`: `calls() == 2n`, `hits == 0`, `stored == n`; the marker test `reembed_ignores_the_markers` stays green.
  - Extend `envelope_reports_embedding_state`: `v["embed_cache"]["status"] == "off"`.
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib tools::reindex`. Expected RED: all new tests (the field and the seam are missing); the existing ones PASS. State which.
- [ ] **Step 3: Implement.**
  - At the start of `call`, when `ctx.embedding` is `Some`, `svc.rearm_cache()`.
  - Drain: `svc.embed_artifact_cached(item.title.as_deref(), &item.text, !reembed).await`; keep a `CacheTally` for the whole call; `note_item` for each result.
  - `unwritten` becomes `Vec<(&EmbedQueueItem, Vec<f32>, bool)>` (the bool is `from_cache`). `flush_vectors` skips `total_embedded` and `embedded_by_artifact` for a landed hit and still counts it in `landed` and pushes its `marks` entry (C3-R14).
  - After each of the two `flush_vectors(...)` calls: `cache_tally.note_flush(svc.flush_cache())`.
  - Set `cache_tally.status` from `ctx.embedding` (`Off` when `None`). Response: add `"embed_cache": cache_tally.to_json()` after `embed_skipped_chunks`. `embed_note`: the "nothing needed a new vector" branch also requires `hits == 0`; the last branch reads `"{total_embedded} embedded, {hits} reused from the vector cache"` when `hits > 0`.
- [ ] **Step 4: Run** `scripts/with-slot.sh cargo test --lib tools::reindex` (PASS), then `scripts/with-slot.sh cargo test --lib catalog::find` (`an_id_from_the_production_embed_queue_hydrates_through_semantic_find` and `the_real_sqlite_path_writes_and_reads_the_same_table` stay green) and `scripts/with-slot.sh cargo clippy --lib --no-default-features -- -D warnings`.
- [ ] **Step 5: Commit** by pathspec: `feat(librarian): reindex reuses cached vectors and reports embed_cache (C3 task 3)`.

### Task 4: Backfill and `index_repo` read and write the cache; the bypass guard

**Files:**
- Modify: `src/librarian/indexer.rs` (`backfill_chunk_vectors_with` embed at L1948 and the page write; `index_repo` L2117-2193; `BackfillReport` L1614-1650)
- Modify: `src/cli/backfill_chunks.rs` (`report_json`)
- Modify: `src/librarian/embedding.rs` (one guard test)
- Test: `indexer.rs` `mod tests`, `backfill_chunks.rs` `mod tests`, `embedding.rs` `mod tests`

**Interfaces:**
- Consumes: Task 2's seam and `test_support::CountingEmbedder`, Task 1's `CacheTally`.
- Produces: `BackfillReport.embed_cache: CacheTally` (the struct already derives `Default, PartialEq, Eq`); the backfill CLI JSON gains `"embed_cache"`.

- [ ] **Step 1: Write the failing tests.** Use `backfill_recording_svc(4, 4, seen)` (`indexer.rs:2860`), then `.with_vector_cache("test-model", cache.clone())`; `index_files`, `store_backfill` (both in the `indexer.rs` tests, next to `a_store_backfill_writes_its_vectors_to_the_store_and_not_to_artifact_vec_v2`), `InMemoryArtifactStore`.
  - `a_backfill_of_text_embedded_before_embeds_nothing_and_stores_the_cached_vectors`: `index_files(&m, &[("a.md", body)])`, `store_backfill` (embeds; `report.embed_cache.stored == chunks`); `index_files(&m, &[("b.md", body)])` (same body, another artifact); `seen.lock().clear()`; `store_backfill`: `seen` stays empty, `report.embed_cache.hits == chunks`, `store.chunk(&b_chunk_id)` is `Some((b_artifact_id, [1.0, 0.0, 0.0, 0.0]))` for every chunk of `b.md`, `report.embedded == chunks`.
  - `a_backfill_flushes_the_cache_once_per_page`: two artifacts with different bodies, `backfill_chunk_vectors_with(&m, &svc, 1, None, false, BackfillTarget::Store { .. }, &[])` (batch 1, so two pages): `cache.transactions() == 2` and `cache.len() == chunks`. An end-of-run flush gives 1 and fails this.
  - `a_cache_failure_does_not_fail_a_backfill`: cache over a garbage file: `report.embedded == chunks`, `report.embed_failed == 0`, `report.embed_cache.status == Unavailable`, `report.embed_cache.errors == chunks`.
  - `the_dimension_probe_is_not_cached`: with `CountingEmbedder::learning(4)` (declares 0 first) the run probes once (`seen()` holds `"codescout backfill dimension probe"` once) and `cache.len() == chunks` after the run, so the probe is not cached; the probe stays a bare `embed_artifact` call.
  - `index_repo_reuses_the_vectors_of_a_moved_file`: setup as `a_vanished_file_has_its_vectors_dropped_from_the_store` (`indexer.rs` L6697-6780) with an `InMemoryArtifactStore`; `index_repo(&cat, &rules, root, &ignore, Some(&svc), Some(&store), "p")`, rename, `seen` cleared, second call: `seen` empty, and the store holds each new chunk id under the new artifact id.
  - In `backfill_chunks.rs`: `the_printed_report_carries_the_cache_counts`: `report_json` has `printed["embed_cache"]["hits"]` and `["status"]`.
  - In `embedding.rs`, `every_embed_call_outside_the_seam_is_the_dimension_probe` (the bypass guard): walk every `.rs` file under `src/` (with `walkdir`, as the read-door gate does); the production region of a file is its text before the first line that trims to `mod tests {`; ignore lines that trim to a `//` comment; count `.embed_artifact(` and `::embed_artifact(`. The counts must be exactly `src/librarian/embedding.rs: 1` (the seam's own call) and `src/librarian/indexer.rs: 1` (the probe), and the `indexer.rs` line holding the one bare call contains `"codescout backfill dimension probe"`. Paired positives: `.embed_artifact_cached(` occurs exactly twice in the `indexer.rs` production region and once in `src/librarian/tools/reindex.rs`, so removing a site cannot pass. Named blind spot (also in Still open): production code after an inline test module is not scanned; the three per-site behavioural tests cover the three sites. It is RED until this task's wiring is done, because `indexer.rs` then holds three bare calls.
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib librarian::indexer`, `scripts/with-slot.sh cargo test --lib cli::backfill_chunks` and `scripts/with-slot.sh cargo test --lib every_embed_call_outside_the_seam_is_the_dimension_probe`. Expected RED: the new tests; existing PASS.
- [ ] **Step 3: Implement.** Backfill: at entry, `svc.rearm_cache()`; the per-item future calls `svc.embed_artifact_cached(item.title.as_deref(), &item.text, true).await`; `report.embed_cache.note_item(...)` per result; after the page's write block (where `pending` is written and cleared), `report.embed_cache.note_flush(svc.flush_cache())`; before `Ok(report)`, `report.embed_cache.status = svc.cache_status()`. The probe at `indexer.rs:1774` stays `embed_artifact`. `index_repo`: `rearm_cache()` at entry, the same seam call, `svc.flush_cache()` after each batch write and after the last one; the result is not reported (C3-R12). `report_json` adds `"embed_cache": report.embed_cache.to_json()`.
- [ ] **Step 4: Run** the three commands of Step 2 (PASS), `scripts/with-slot.sh cargo test --lib read_door`, and `scripts/with-slot.sh cargo clippy --lib --no-default-features -- -D warnings`.
- [ ] **Step 5: Commit** by pathspec: `feat(librarian): backfill and index_repo reuse cached vectors (C3 task 4)`.

### Task 5: Manual page and guide text

**Files:**
- Modify: `docs/manual/src/concepts/librarian-embedded.md` (new `## The vector cache` section after `## First run`; no new page, so `tests/manual_toc.rs` is unaffected)
- Modify: `src/prompts/guides/librarian-runtime.md` (one paragraph at the end of `## Where catalog state lives (and what is *not* in the repo)`, before `## Per-project classifier overrides`)
- Test: `src/prompts/mod.rs` tests

**Interfaces:** none.

- [ ] **Step 1: Write the failing test** `the_runtime_guide_documents_the_vector_cache`, beside `no_guide_denies_the_augmentation_sidecar` (`src/prompts/mod.rs`): `topic_body("librarian-runtime")` contains `vector-cache.db`, `vector-cache.db-wal`, `embed_cache` and `reembed`, and does not contain `at any time`.
- [ ] **Step 2: Run** `scripts/with-slot.sh cargo test --lib the_runtime_guide_documents_the_vector_cache`. Expected: FAIL.
- [ ] **Step 3: Write the text.** Manual section, in STE, exactly:

  > ## The vector cache
  >
  > The librarian keeps the vectors it makes in a cache. The cache is one file, `vector-cache.db`. It is in the same directory as the catalog file. It is not in a repository.
  >
  > The cache key is the text that was embedded and the embedding setup. The setup is the model name, the text budget and the version of the text format.
  >
  > A vector goes into the cache only when the librarian embeds the text. The cache is empty when it is new. The librarian does not embed a file that has not changed. So the cache holds no vector for such a file.
  >
  > To fill the cache for a project, run `reindex` with `reembed=true` for that project. This embeds each chunk again. It takes the full embedding time, one time.
  >
  > When a file moves, its chunks get new identifiers, but the text does not change. If the cache holds the vectors of that text, the librarian takes them from the cache and does not call the embedding server. A rebuilt catalog does the same. If the cache does not hold them, the librarian embeds the text and stores the vectors.
  >
  > The cache only saves time. If the cache file is missing, damaged or locked, the librarian calls the embedding server for each chunk. The result of `reindex` shows the cache in `embed_cache`.
  >
  > The cache file only grows. To remove it, stop every codescout process. Then delete `vector-cache.db`, `vector-cache.db-wal` and `vector-cache.db-shm`.
  >
  > `reindex` with `reembed=true` does not read the cache. It writes a new vector for each chunk.

  Guide paragraph (repo-neutral, present tense), exactly:

  > **Vectors have a cache of their own.** Next to the catalog file sits `vector-cache.db`. It is keyed by the exact text a chunk is embedded from, the model, the text budget and the text-format version. The cache starts empty and fills only from text that is embedded: a new or changed file, or a `reembed=true` reindex. An unchanged file is not queued, so its vectors reach the cache only through `reembed=true`. A moved or copied file and a rebuilt catalog take vectors from the cache when their text is in it, and then call no embedder. The cache holds no fact about any document, so losing it costs embed calls and nothing else. To remove it, stop every codescout process, then delete `vector-cache.db`, `vector-cache.db-wal` and `vector-cache.db-shm`. `reindex` reports it as `embed_cache` (`status`, `hits`, `stored`, `errors`); `embedded` counts only vectors the embedder made. `reembed=true` skips cache reads and rewrites the rows. A failing cache never fails a reindex: each failure costs one embed call and is counted in `errors`. The file is never pruned and only grows.
- [ ] **Step 4: Run** `scripts/with-slot.sh cargo test --lib the_runtime_guide_documents_the_vector_cache`, `scripts/with-slot.sh cargo test --lib no_guide_names_a_path_or_field_only_this_repo_has`, `scripts/with-slot.sh cargo test --lib no_guide_claims_a_move_preserves_the_id`, and `scripts/with-slot.sh cargo test --test manual_toc`. Expected: PASS. Then re-read the manual section against the STE rules (short sentences, one idea each, active voice).
- [ ] **Step 5: Commit** by pathspec: `docs(librarian): the vector cache in the manual and the runtime guide (C3 task 5)`.

### Task 6: Gate (controller)

- [ ] **Step 1:** Run `./scripts/gate.sh` in the background on an untouched tree (no implementer holds an open round) and end the turn. Expected: Python lane green, `fmt-mine.sh` clean, clippy clean in both feature sets, lean and default nextest green, doc tests green. Then record the named follow-ups (below) in the artifact-identity ledger.

## Self-review

1. **Coverage.** Spec L99-101 and L230 (a rebuild and a move reuse vectors): Tasks 3 and 4, for text embedded after the cache exists; the cold start for already-embedded content is pinned by a test and stated in R5 and the manual. L297-299 (a small machine file beside the catalog, loss harmless): Tasks 1 and 5. C3-R1: T1. R2, R10, R11: T2. R3: not built (follow-up). R4: nothing built (no eviction; follow-up). R5: nothing built; the cost is tested in T3 and written in T5. R6: T3, T4. R7: T1 (store), T2 (seam), T3 and T4 (reports). R8: T1. R9: T3 (and the model-switch consequence in T5 text). R12: T4. R13: T1 (open, rearm, busy timeout), T3 and T4 (rearm calls). R14: T3, T4. F-19 to F-23: Rulings R4, R5, R8, R11, R13; Tasks 1 to 5 as marked.
2. **Step scan.** Each step names its command, files, test names and assertions. Bodies are left to the implementer except the open, get, stage and flush rules, which the signatures do not fix.
3. **Types.** `ModelKey`, `CacheLookup`, `FlushOutcome`, `CacheStatus`, `CacheTally`, `VectorCache` (Task 1) are the types Tasks 2 to 4 use; `CachedEmbed`, `embed_artifact_cached`, `flush_cache`, `rearm_cache`, `cache_status` (Task 2) are the names Tasks 3 and 4 call; `CountingEmbedder` (Task 2) is the double Tasks 3 and 4 use. `CacheTally` carries `status`, so `to_json()` takes no argument in both reports. No interface or step carries a `last_used` column, an eviction call, a `now_secs` parameter or an `evicted` count; the words appear only where a ruling or a test says they are absent.
4. **Review Focus.** Line 1: T2. Line 2: T1, T2. Line 3: T1, T3, T4. Line 4: T3. Line 5: T1. Line 6: T3, T4.
5. **Proportion.** One new module, one seam method, three small wirings, one guard and two prose edits. Most of the text is test lists. The eviction code, its constants, its column, its index and its tests are gone.

## Facts verified, and what this plan corrects in the scout

Read at the bytes in this session, then confirmed by the independent review (session log W-12).

- **Correction 1: `reindex_cli` is not production.** `src/librarian/mod.rs:600` is `#[cfg(test)]` on `reindex_cli` (L601); its callers are tests only (`mod.rs:826, 828, 1005, 1036`). The archived bug `docs/issues/archive/2026-08-30-reindex-cli-carries-a-broken-copy-of-a-deliberately-removed-delete.md` is titled "reindex_cli is test-only". So there is one production `EmbeddingService::with_budget` site (`build_tool_context_with`, `mod.rs:338`), not two. The text-scan guards (`embedding.rs:360, 390`) still count both because they count raw text, so the plan keeps both sites in step.
- **Correction 2: the third embed site has no production caller.** `index_repo` (`indexer.rs:2117`, embed at `:2162`) is called only by `reindex_cli` (`mod.rs:731`) and tests; it is `pub` in `pub mod librarian` (`src/lib.rs:51`), so the compiler does not flag it. The production embed sites are two: the reindex drain (`reindex.rs:631`) and backfill (`indexer.rs:1948`), reached by `codescout backfill-chunks` (`src/cli/backfill_chunks.rs:99`) through `open_ctx` → `build_tool_context` (`src/cli/mod.rs:68`). A third `embed_artifact` call, the dimension probe (`indexer.rs:1774`), embeds no chunk and stays bare. The plan wires `index_repo` anyway (C3-R12) and guards all three sites (Task 4).
- **Correction 3: the dimension is unknown at construction.** `RemoteEmbedder::dimensions()` reads `cached_dims`, 0 until the first embed (`crates/codescout-embed/src/remote.rs:458-463`, set at `:615-616`). Backfill probes for the same reason (`indexer.rs:1769-1777`). The scout's "model_key covers the dimension, set at the two construction sites" cannot be computed there (C3-R11).
- **Correction 4 (review F-20): "an unmarked chunk is embedded by the first reindex after C3" is false.** The queue is gated per artifact before any marker is read: `needs_embed` (`indexer.rs:671-675`) and `force_embed` (`:696`, `:732`). The v14 migration clears no stamp (`catalog/mod.rs:504-519`); the only `embedded_sha256 = NULL` is `clear_kept_row` (`indexer.rs:1042`). The cache is empty for the whole existing corpus on the day C3 ships.
- **A git move, traced:** the old row vanishes and loses its vectors; the new path is a new row (`id = sha256(abs_path)`), its `embedded_sha256` is NULL, so every chunk is queued with a fresh chunk id and no marker. Each chunk reaches the seam. It hits only if the cache holds that text, which means the text was embedded (or `reembed`ed) after C3 shipped. Otherwise it embeds once, and the cache then holds it.
- **The seam fits** (review W-12): a hit rides `unwritten` into `flush_vectors` and lands under the chunk's own id; the marker is pushed only on a landed upsert (`reindex.rs:49-101`); `reembed` is threaded at `reindex.rs:628`; the MCP server builds its librarian runtime once (`src/server.rs:441`).
- **Gate census for a new sqlite file under `src/librarian`** (the question raised by F-17), confirmed by the review: `no_librarian_production_read_bypasses_the_read_door` walks every `.rs` under `src/librarian` (`read_door.rs:1414-1430`) and counts the tokens at `read_door.rs:1322-1335` (`path_reads`) and `:1398` (`use_item_reads`). `rusqlite::Connection::open` and `create_dir_all` match none, and `#[cfg(test)]` items are blanked (`production_code`, `read_door.rs:1083`), so it does NOT trip. `every_cap_constant_is_classified` applies (it lists `git ls-files src`, so stage the file first) and rejects cap-shaped names; no plan constant is cap-shaped. The text-scan guards on `mod.rs` stay green while `.with_concurrency(...)` is untouched. No write-side gate exists (plan A-R11 names it a follow-up). `tests/inline_gates.rs`, `tool_surface_under_budget` (the budget stays 57_664: only response output changes) and the CLAUDE.md byte cap do not see this change. `tests/manual_toc.rs` is untouched (no new page).
- `librarian-runtime.md` is a pull-only guide with no `serves:` declaration, so `MAX_DECLARED_SECTION_BYTES` does not apply to the new paragraph.
- Confirmed as the scout states: `embed_input_hash` (`chunk.rs:643`) hashes a length-prefixed title and the composed text; `embed_queue_from_chunks` sets `input_hash` on every item (`indexer.rs:300-357`); the title is the frontmatter title or the first H1, not the file name (`indexer.rs:621-624`), so a moved file keeps its title and chunk text; `mark_embedded` has one production caller (`reindex.rs:708`); `flush_vectors` is called at L643 and L690 only; `UPSERT_BATCH` is 64 (`artifact_store.rs:170`); `in_one_commit` documents 4,691 ms against 12 ms for 400 chunks (`indexer.rs:195-210`); `catalog_db_path` returns `None` for an in-memory connection (`catalog/mod.rs:733`); `Catalog::open` sets WAL and a 5 s busy timeout (`catalog/mod.rs:773-792`); `InMemoryArtifactStore::chunk` returns `(artifact_id, vector)`; the fixtures named in Tasks 3 and 4 exist; every `BackfillReport { .. }` literal outside the struct uses `..Default::default()`.
- Not re-measured: the scout's corpus figures (marker coverage, duplicate counts, 439 MB). Cited, not run.

## Still open

- **Named follow-up: eviction.** Without it the file only grows: about 3 KB per distinct embed input, about 439 MB at today's corpus, removed only by the manual stop-and-delete. The trap a later design must avoid (F-19): an age rule keyed on `last_used` deletes the vectors of stable content, because an unchanged file is never looked up. An artifact whose stamp is current is not queued (`indexer.rs:671-697`), and a queued chunk with a current marker skips the seam (`reindex.rs:628`). So nothing refreshes a row for stable content. A correct rule needs a refresh of the rows of marked chunks (touch `(model_key, embedded_input_hash)` for every marked chunk under the reindexed targets, in one transaction), or a live-set rule (keep the hashes the catalog's chunks still name), plus a test that a row whose hash is a live marker survives.
- **Named follow-up: seeding.** Exports the vectors of marked chunks into the cache. It is the only way to make the cutover rebuild reuse vectors without a `reembed=true` pass per project. sqlite: one `SELECT` over `artifact_vec_v2`; Qdrant: new read-back code (the scout says every scroll passes `with_vectors(false)`; not re-verified). It covers marked chunks only (26.6% of this repo's chunks, scout figure); `reembed=true` is the complete lever. Until one of the two runs, the cache is empty for the existing corpus (R5, manual page).
- Other named follow-ups: (a) the probe-vector guard, which also covers two servers behind one model string (R3); (b) replacing the marker skip by "cache hit and the store holds the point" (scout question 5); (c) automatic recreate of a corrupt cache file (R13); (d) a `doctor` check for the cache.
- The bypass guard (Task 4) scans production code before the first `mod tests {` of each file, so production code after an inline test module is not scanned. The three per-site behavioural tests cover the three known sites.
- When the derived catalog gets a persistent location (Plans D and E), `VectorCache::beside` should follow it (spec L297-299). One function changes.
- Not tested in this plan: a Qdrant-backed reindex end to end with the cache (the Qdrant tests are `--ignored`; the plan uses `InMemoryArtifactStore`), sqlite WAL behaviour on Windows, and a real llama-server run. The implementer should run one local reindex on a copy of a small repo, then a rename and a second reindex, and read `embed_cache` in both responses.
- If a task adds a constant with a cap-shaped name, or a read-door token to production code, the matching gate goes red; fix the name or the call, not the gate.
