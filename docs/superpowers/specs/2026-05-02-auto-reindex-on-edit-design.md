# Auto-Reindex on Edit — Design Spec

**Date:** 2026-05-02
**Status:** Draft

## Goal

Keep semantic search results current as files are edited, without requiring explicit `index(action='build')` calls. Re-embedding is triggered lazily — only when a search is actually issued — so write tools pay zero latency cost.

## Trigger

Only edits made through codescout write tools: `edit_file`, `edit_code`, `create_file`. External editor edits are out of scope (filesystem watcher is a separate future feature).

## Architecture

Three components:

### 1. Dirty Set (`Agent`)

Add `dirty_files: Arc<Mutex<HashSet<String>>>` to `Agent`. Holds relative file paths (forward-slash normalized) that have been written since the last search. HashSet semantics collapse duplicate writes to the same file.

```rust
// src/agent.rs
pub struct Agent {
    // ...existing fields...
    pub dirty_files: Arc<Mutex<HashSet<String>>>,
}
```

Initialize as empty on `Agent::new`.

### 2. Post-Write Hook (write tools)

After each successful write in `edit_file`, `edit_code`, and `create_file`:

1. Compute the relative path of the written file.
2. Read the stored hash from the `files` table via `get_file_hash(conn, rel_path)`.
3. Hash the file on disk via `hash_file(path)`.
4. If hashes differ (or file is absent from DB): insert `rel_path` into `dirty_files`.

The hash check is the gate — it's sub-millisecond (single file read + SHA-256) and prevents re-embedding no-op writes. The write tool still returns `"ok"` immediately; the dirty set insertion is synchronous but cheap.

If the file is unreadable after the write (unexpected), skip the dirty set insertion — the write tool already surfaced the error.

### 3. Pre-Search Drain (`SemanticSearch`)

At the start of `SemanticSearch::call()`, before the knn query:

1. Lock and drain `dirty_files` into a local `Vec<String>`.
2. If non-empty: call `reindex_files(project_root, &paths, &embedder)`.
3. On error: log a warning, continue — search must never be blocked by embedding failures.
4. Proceed with the knn query as normal.

## New Function: `reindex_files`

Add to `src/embed/index.rs`:

```rust
pub async fn reindex_files(
    project_root: &Path,
    rel_paths: &[String],
    embedder: &Arc<dyn Embedder>,
) -> Result<usize>
```

For each path:
1. Read file content; skip if unreadable.
2. `hash_file` — compare against DB. Skip if hash unchanged (defensive double-check).
3. `split_file` → chunks. Skip if empty.
4. Embed chunks via `embedder.embed_batch`.
5. DB transaction: `delete_file_chunks(conn, rel)` → `insert_chunk` for each new chunk → `upsert_file_hash`.

Returns count of files actually re-embedded. Reuses all existing primitives — no new DB schema needed.

## Data Flow

```
edit_code("src/foo.rs", ...)
  → write succeeds
  → hash(src/foo.rs) ≠ db_stored_hash → dirty_set.insert("src/foo.rs")
  → return "ok"

[LLM processes response, issues next tool call]

semantic_search("find all traits")
  → drain dirty_set → ["src/foo.rs"]
  → reindex_files(root, ["src/foo.rs"], embedder)
      → split_file → embed → delete old chunks → insert new → upsert hash
  → knn query on fresh index
  → return results
```

## Error Handling

| Failure | Behavior |
|---------|----------|
| Hash check fails (file unreadable after write) | Skip dirty set insertion; write tool already errored |
| `reindex_files` embedder error | Log warning, clear dirty path, proceed with stale data |
| DB locked during reindex | Same as above — search proceeds |
| Multiple writes before search | HashSet collapses duplicates; file re-embedded once |

## Testing

Three tests in `src/embed/index.rs`:

1. **`reindex_files_updates_single_file`** — write a file, populate chunks in DB, overwrite with new content, call `reindex_files`, assert DB chunks reflect new content.

2. **`reindex_files_skips_unchanged_hash`** — write a file, index it, call `reindex_files` again without touching the file, assert embed call count is zero (hash gate fires).

3. **`reindex_files_handles_unreadable_file`** — add a path to the dirty set for a file that doesn't exist, call `reindex_files`, assert it returns without error and count is 0.

Integration test in `src/tools/semantic.rs` (or a dedicated integration file):

4. **`dirty_set_drained_on_search`** — simulate write (insert into dirty set), call `SemanticSearch`, assert dirty set is empty afterward and result reflects updated content.

## Out of Scope

- External editor edits (file watcher — tracked in ROADMAP)
- Deletions (no delete tool exists)
- Opt-in config flag — always-on, no background tasks, no overhead unless you search
- Librarian (markdown) index — separate indexing pipeline, separate concern
