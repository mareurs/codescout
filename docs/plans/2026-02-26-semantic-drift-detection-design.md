# Semantic Drift Detection Design

Detect *how much* code changed in meaning, not just *that* it changed. SHA-256 hashing (the existing gatekeeper) tells you which files differ; semantic drift scoring tells you which differences actually matter.

## Motivation

The incremental index pipeline (`find_changed_files` → `build_index`) already uses SHA-256 to skip unchanged files. But SHA-256 treats a whitespace reformatting the same as a complete function rewrite — both produce a hash mismatch. This matters for two consumers:

1. **Doc staleness filtering** — SHA-256 flags 20 files as changed, but only 3 had meaningful semantic shifts. Without drift scores, the Glossary/Documentation Management feature must present all 20 diffs to the LLM, wasting context window tokens on noise.

2. **Drift-aware re-indexing feedback** — After `index_project`, the agent has no signal about *what kind* of changes happened. Drift scores let `IndexReport` say "12 files re-indexed, 2 had major semantic changes" instead of just "12 files re-indexed."

## Architecture

SHA-256 is the gatekeeper; semantic comparison is the intelligent filter. They are layered, never redundant:

| Signal | SHA-256 hash | Semantic (embeddings) |
|--------|-------------|----------------------|
| "Did anything change?" | Yes/no per file | N/A (overkill) |
| "Is this a trivial change?" | Can't tell — hash differs either way | Embedding barely moves |
| "Did the *purpose* change?" | Can't tell | Large embedding distance |
| "Is the doc still accurate?" | "Something changed, check it" | "Meaning drifted significantly — update" |

### Data Flow

Drift computation hooks into the existing `build_index` Phase 3 transaction loop. No new pipeline phases — just extra logic per file:

```
build_index called
  │
  ├─ Phase 1: find_changed_files()
  │    └─ git diff → mtime → SHA-256 fallback chain
  │    └─ Returns ChangeSet { changed, deleted }
  │
  ├─ Phase 2: Chunk + embed changed files (concurrent)
  │    └─ Produces Vec<FileResult> with new chunks + embeddings
  │
  ├─ Phase 3: DB transaction
  │    For each FileResult:
  │      ┌─ 3a. Read old chunks + embeddings from DB   ← NEW
  │      ├─ 3b. delete_file_chunks()
  │      ├─ 3c. Insert new chunks + embeddings
  │      ├─ 3d. Compare old↔new, compute drift score   ← NEW
  │      └─ 3e. Upsert drift_report row                ← NEW
  │
  └─ Phase 4: Commit transaction, update meta, return IndexReport
```

Between steps 3a and 3b, both old and new embeddings are in memory simultaneously. The comparison is transient — no historical embedding storage needed.

## Chunk Matching Algorithm

When a file changes, the new chunking may produce a different number of chunks (added function, deleted function, split a large function). The matching algorithm handles this:

### Step 1: Content-hash exact matching (fast path)

Compare old and new chunk content strings directly. Exact matches get drift = 0.0. Remove matched chunks from both sets.

This handles the common case: most chunks in a file are unchanged. Only the actual edits proceed to the expensive step.

### Step 2: Semantic best-match (greedy, on remainder)

For each remaining old chunk, compute cosine similarity against all remaining new chunks. Greedily assign: best pair first, remove both, repeat.

- If best match < 0.3 similarity → treat as unmatched (the chunk was truly added/removed, not refactored)
- Each matched pair gets drift = 1.0 - cosine_similarity
- Complexity: O(n*m) where n, m are the *unmatched* remainder counts (typically small)

The 0.3 threshold reflects that embeddings of completely unrelated code still have non-zero cosine similarity (typically 0.1-0.3 for code embedding models).

### Step 3: Classify unmatched

- Remaining old chunks → "removed" (drift contribution = 1.0 each)
- Remaining new chunks → "added" (drift contribution = 1.0 each)

### Step 4: Aggregate

- `avg_drift`: mean of all pair drifts (including 0.0 exact matches and 1.0 for added/removed)
- `max_drift`: max of all pair drifts
- `max_drift_chunk`: content snippet of the most-drifted chunk (new side if matched, old side if removed)
- `chunks_added`: count of unmatched new chunks
- `chunks_removed`: count of unmatched old chunks

## Storage

### `drift_report` table (in `embeddings.db`)

```sql
CREATE TABLE IF NOT EXISTS drift_report (
    file_path       TEXT PRIMARY KEY,
    avg_drift       REAL NOT NULL,
    max_drift       REAL NOT NULL,
    max_drift_chunk TEXT,               -- content snippet (first 200 chars)
    chunks_added    INTEGER NOT NULL,
    chunks_removed  INTEGER NOT NULL,
    indexed_at      TEXT NOT NULL        -- ISO 8601
);
```

**Lifecycle:** Cleared and rebuilt on each `build_index` run. Only files that changed in the *most recent* index operation have rows. Files skipped by `find_changed_files` have no row — they didn't drift.

### `IndexReport` extension

```rust
pub struct IndexReport {
    pub indexed: usize,
    pub deleted: usize,
    pub skipped_msg: String,
    pub drift: Vec<FileDrift>,      // NEW
}

pub struct FileDrift {
    pub file_path: String,
    pub avg_drift: f32,
    pub max_drift: f32,
    pub chunks_added: usize,
    pub chunks_removed: usize,
}
```

## Tools

### `check_drift` (new tool)

Reads from the `drift_report` table on demand.

**Parameters:**
- `threshold` (optional, default 0.1): minimum `avg_drift` to include
- `path` (optional): glob pattern to filter files (e.g. `"src/tools/**"`)

**Output:**
- Exploring mode: file_path, avg_drift, max_drift, chunks_added, chunks_removed
- Focused mode: adds max_drift_chunk content
- Sorted by max_drift descending (most-drifted files first)

### `index_project` response (extended)

After re-indexing, includes a `drift_summary` field with the top-N most-drifted files, giving the agent immediate feedback.

## Integration Points

| Consumer | How it uses drift | When |
|----------|-------------------|------|
| `index_project` response | Top-N drifted files after re-indexing | Immediate |
| `check_drift` tool | On-demand query of drift_report table | Anytime after index build |
| `check_docs` (future) | Filters SHA-256 staleness by semantic significance | When Glossary feature lands |
| `semantic_search` | Could annotate results from high-drift files | Optional, low priority |

### `check_docs` integration sketch

```
check_docs flow:
  1. SHA-256 flags files A, B, C, D as changed
  2. Join with drift_report:
     - A: avg_drift 0.02 → formatting only, skip
     - B: avg_drift 0.45 → meaningful change, flag for doc update
     - C: avg_drift 0.8  → major rewrite, flag urgently
     - D: not in drift_report → not re-indexed yet, fall back to SHA-256 signal
```

## What This Does NOT Do

- **No historical tracking** — only the latest index run's drift is stored
- **No automatic actions** — drift scores are informational, the agent decides
- **No cross-file drift** — each file is compared against its own previous version

## Future Improvement: File-Level Semantic Fingerprints

Deferred to a later iteration. Concept: compute a single vector per file (mean of chunk embeddings) and persist it.

```sql
CREATE TABLE IF NOT EXISTS file_fingerprints (
    file_path   TEXT PRIMARY KEY,
    fingerprint BLOB NOT NULL,      -- mean of chunk embeddings
    indexed_at  TEXT NOT NULL
);
```

**What this would enable:**
- **Codebase evolution over time** — compare fingerprints across git tags/releases
- **Module-level similarity** — find semantically similar files (duplication candidates)
- **Drift without re-embedding** — compare stored fingerprint against freshly computed one
- **Cross-session drift** — if fingerprints are versioned by commit SHA, ask "how much did `auth.rs` drift between v1.0 and v2.0?"

**Why defer:** Requires decisions about versioning strategy (keep all versions? last N?), storage growth, and which queries are actually useful. Ship the transient chunk-level comparison first, then decide if persistent fingerprints add value based on real usage.
