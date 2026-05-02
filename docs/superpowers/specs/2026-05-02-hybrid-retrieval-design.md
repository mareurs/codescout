# Hybrid BM25 + Vector Retrieval Design

**Date:** 2026-05-02
**Status:** draft
**Branch:** experiments

## Overview

Replace pure cosine-similarity retrieval in `semantic_search` with always-on hybrid retrieval: Tantivy BM25 (text leg) + sqlite-vec KNN (vector leg), fused via Reciprocal Rank Fusion (RRF). Scope: project index only; library indexes remain vector-only for now.

**Motivation:** The existing 20-TC benchmark shows all embedding models fail TC-10, TC-19, TC-20 — queries where code doesn't use query vocabulary. These are exact identifier/keyword lookups where BM25 excels and vector search fundamentally cannot help. Best current score: nomic-embed-code 36/60. Target post-hybrid: ≥40/60.

---

## Architecture

### Storage layout

```
.codescout/
  embeddings/
    index.db          ← SQLite: chunks + chunk_embeddings (unchanged)
  tantivy/
    meta.json         ← Tantivy index files (segments, managed by Tantivy)
    ...
```

### New modules

**`src/embed/bm25.rs`** — Tantivy index management. Three public functions:

```rust
pub fn open_or_create(root: &Path) -> Result<Index>
pub fn build_from_db(root: &Path, conn: &Connection) -> Result<()>
pub fn search(index: &Index, query: &str, limit: usize) -> Result<Vec<BM25Result>>
```

**`src/embed/fusion.rs`** — RRF score fusion.

```rust
pub struct BM25Result {
    pub chunk_id: u64,
    pub score: f32,
    pub rank: usize,
}

pub fn rrf_fuse(
    vector: &[SearchResult],
    bm25: &[BM25Result],
    k: f32,           // 60.0 canonical
) -> Vec<u64>         // chunk_ids in fused rank order
```

### Tantivy schema

| Field | Type | Boost | Notes |
|---|---|---|---|
| `chunk_id` | u64 FAST stored | — | join key back to `chunks.id` |
| `content` | Text indexed+stored | 1.0 | chunk body |
| `file_path` | Text indexed+stored | 1.5 | path tokens |
| `metadata` | Text indexed+stored | 2.0 | AST header; empty string when NULL |

### `CodeTokenizer`

Pure-Rust `tantivy::tokenizer::Tokenizer` implementation. No unsafe, no C-API.

Pipeline per token:
1. Split on whitespace and punctuation
2. Split on `_` (snake_case)
3. Split on camelCase boundaries (uppercase after lowercase = split point)
4. Lowercase all tokens

Examples:
- `"parseJsonObject"` → `["parse", "json", "object"]`
- `"open_db"` → `["open", "db"]`
- `"src/embed/index.rs"` → `["src", "embed", "index", "rs"]`
- `"impl Tool for SemanticSearch / call"` → `["impl", "tool", "for", "semantic", "search", "call"]`

Registered with `TokenizerManager` at `open_or_create` time under the name `"code"`.

---

## Data Flow

### Build path

`build_index` is the single entry point for both indexes. After `db_writer` completes writing to SQLite:

```
db_writer finishes
        ↓
build_from_db(root, conn)
        ↓
SELECT id, content, file_path, metadata FROM chunks WHERE source = 'project'
        ↓
delete .codescout/tantivy/ (full rebuild)
        ↓
open fresh Tantivy index with CodeTokenizer
        ↓
batch-add all docs (chunk_id, content, file_path, metadata)
        ↓
commit + merge segments
        ↓
.codescout/tantivy/ written
```

Full rebuild every time. BM25 indexing is fast (~1s for 100k chunks) — no incremental complexity needed.

### Search path

Inside `spawn_blocking` (Tantivy is sync, already on the blocking thread):

```
query string
  ├──→ search_scoped_vec0(conn, embedding, limit*3)
  │         → Vec<SearchResult>  [vector leg, ranked by cosine]
  │
  └──→ bm25::search(index, query, limit*3)
            → Vec<BM25Result>    [BM25 leg, ranked by Tantivy score]

               ↓ both legs complete

  rrf_fuse(vector_results, bm25_results, k=60)
               ↓
  re-ranked chunk_id list

               ↓ SQLite join for BM25-only hits

  SELECT * FROM chunks WHERE id IN (bm25_only_ids)
               ↓
  full Vec<SearchResult> in fused rank order
               ↓
  apply_file_diversity_cap → take(limit)
               ↓
  existing format + output pipeline (unchanged)
```

**Overfetch factor:** both legs fetch `limit * 3` candidates so RRF has material to promote BM25-only hits that vector missed.

**Fallback:** if `.codescout/tantivy/` is absent, `bm25::search` returns `vec![]` and RRF degrades to pure vector ordering. No error surfaced to the user.

---

## RRF Fusion

Formula: `rrf_score(chunk) = Σ 1 / (k + rank_i)` across all legs where chunk appears (1-indexed rank).

k=60 (canonical default). Chunks appearing in both legs outscore single-leg hits. `rank` is assigned as 1-indexed position in each leg's result list (sorted by cosine score descending for vector; by Tantivy score descending for BM25).

Example — chunk at vector rank 3, BM25 rank 1 (k=60):
`1/63 + 1/61 = 0.0159 + 0.0164 = 0.0323`

Beats a chunk only at vector rank 1: `1/61 = 0.0164`.

This is the desired behavior: BM25 rescues exact-identifier queries that vector places mid-list.

---

## Testing

### Unit tests — `src/embed/bm25.rs`

- `code_tokenizer_splits_camel_case` — `"parseJsonObject"` → `["parse", "json", "object"]`
- `code_tokenizer_splits_snake_case` — `"open_db"` → `["open", "db"]`
- `code_tokenizer_splits_file_path` — `"src/embed/index.rs"` → `["src", "embed", "index", "rs"]`
- `code_tokenizer_handles_mixed` — `"SemanticSearch/call"` → `["semantic", "search", "call"]`
- `build_and_search_roundtrip` — insert 3 synthetic chunks, query by exact identifier, assert rank #1
- `search_returns_empty_on_missing_index` — no tantivy dir → returns `vec![]`, no panic

### Unit tests — `src/embed/fusion.rs`

- `rrf_promotes_bm25_only_hit` — chunk absent from vector but BM25 rank 1 appears in fused top-5
- `rrf_promotes_dual_hit_above_single_leg` — vector rank 3 + BM25 rank 1 beats vector rank 1 only
- `rrf_stable_on_empty_legs` — one empty leg → equivalent to single-leg ranking

### Integration — benchmark harness

Run 20-TC benchmark before and after merge. Baseline: nomic-embed-code 36/60, AllMiniLML6V2Q 34/60.

Ship gate: ≥40/60 on best available model.

TC-10, TC-19, TC-20 (all currently 0/3 across all models) are primary targets — at least one must improve.

### Three-query staleness sandwich

1. Build index, query known identifier — assert top-5
2. Wipe `.codescout/tantivy/` without rebuild
3. Query again — assert degradation (proves BM25 leg contributed)
4. Rebuild — assert identifier returns to top-5

---

## Dependencies

Add to `Cargo.toml`:

```toml
tantivy = "0.22"
```

Binary size impact: ~4MB release build.

No new runtime dependencies (no server process, no Python, embedded).

---

## Out of Scope

- Library index hybrid search (libraries remain vector-only)
- Configurable BM25/vector weights (equal RRF weights for now)
- Parallel execution of the two search legs (sequential inside `spawn_blocking` is fast enough)
- Streaming/incremental Tantivy updates (full rebuild on every `build_index`)
- Usage DB per-query BM25 vs vector attribution (input/output only in debug mode anyway)
