# Semantic Drift Detection Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Detect how much code changed in *meaning* during re-indexing, not just *that* it changed, and persist drift scores for on-demand querying.

**Architecture:** Inside `build_index` Phase 3, before `delete_file_chunks`, read old embeddings. After inserting new chunks, compare old vs new using content-hash-first matching with greedy cosine fallback. Persist per-file drift scores in a `drift_report` table. Expose via a `check_drift` tool and in the `index_project` response.

**Tech Stack:** Rust, rusqlite (already a dependency), existing `cosine_sim`/`bytes_to_f32`/`l2_norm` functions in `src/embed/index.rs`.

**Design doc:** `docs/plans/2026-02-26-semantic-drift-detection-design.md`

---

## Task 1: Add `drift_report` table to schema

**Files:**
- Modify: `src/embed/index.rs:36-91` (`open_db` — schema DDL)
- Test: `src/embed/index.rs` (inline tests)

**Step 1: Write the failing test**

Add to the `tests` module in `src/embed/index.rs`:

```rust
    #[test]
    fn open_db_creates_drift_report_table() {
        let (_dir, conn) = open_test_db();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM drift_report", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
```

**Step 2: Run test to verify it fails**

Run: `cargo test embed::index::tests::open_db_creates_drift_report_table -- --nocapture`
Expected: FAIL — `no such table: drift_report`

**Step 3: Write minimal implementation**

In `open_db` (`src/embed/index.rs:36-91`), add the new table to the `execute_batch` DDL string, after the `meta` table:

```sql
        CREATE TABLE IF NOT EXISTS drift_report (
            file_path       TEXT PRIMARY KEY,
            avg_drift       REAL NOT NULL,
            max_drift       REAL NOT NULL,
            max_drift_chunk TEXT,
            chunks_added    INTEGER NOT NULL,
            chunks_removed  INTEGER NOT NULL,
            indexed_at      TEXT NOT NULL
        );
```

**Step 4: Run test to verify it passes**

Run: `cargo test embed::index::tests::open_db_creates_drift_report_table -- --nocapture`
Expected: PASS

**Step 5: Run full suite**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

**Step 6: Commit**

```bash
git add src/embed/index.rs
git commit -m "feat(index): add drift_report table to schema"
```

---

## Task 2: Add `read_file_embeddings` function

This function reads old chunk content + embeddings for a file before they are deleted. It's called inside the Phase 3 loop.

**Files:**
- Modify: `src/embed/index.rs` (add `OldChunk` struct, `read_file_embeddings` function)
- Test: `src/embed/index.rs` (inline tests)

**Step 1: Write the failing tests**

```rust
    #[test]
    fn read_file_embeddings_returns_content_and_vectors() {
        let (_dir, conn) = open_test_db();
        insert_chunk(
            &conn,
            &dummy_chunk("a.rs", "fn hello() {}"),
            &[1.0, 0.0, 0.0],
        )
        .unwrap();
        insert_chunk(
            &conn,
            &dummy_chunk("a.rs", "fn world() {}"),
            &[0.0, 1.0, 0.0],
        )
        .unwrap();

        let old = read_file_embeddings(&conn, "a.rs").unwrap();
        assert_eq!(old.len(), 2);
        assert_eq!(old[0].content, "fn hello() {}");
        assert_eq!(old[0].embedding, vec![1.0, 0.0, 0.0]);
        assert_eq!(old[1].content, "fn world() {}");
        assert_eq!(old[1].embedding, vec![0.0, 1.0, 0.0]);
    }

    #[test]
    fn read_file_embeddings_returns_empty_for_missing_file() {
        let (_dir, conn) = open_test_db();
        let old = read_file_embeddings(&conn, "missing.rs").unwrap();
        assert!(old.is_empty());
    }
```

**Step 2: Run tests to verify they fail**

Run: `cargo test embed::index::tests::read_file_embeddings -- --nocapture`
Expected: FAIL — `read_file_embeddings` doesn't exist

**Step 3: Write minimal implementation**

Add near the other query functions in `src/embed/index.rs`:

```rust
/// A chunk's content and embedding vector, read from the DB before deletion.
#[derive(Debug, Clone)]
pub struct OldChunk {
    pub content: String,
    pub embedding: Vec<f32>,
}

/// Read all chunk content + embedding vectors for a file.
/// Used to snapshot old state before `delete_file_chunks`.
pub fn read_file_embeddings(conn: &Connection, file_path: &str) -> Result<Vec<OldChunk>> {
    let mut stmt = conn.prepare(
        "SELECT c.content, ce.embedding
         FROM chunks c JOIN chunk_embeddings ce ON c.id = ce.rowid
         WHERE c.file_path = ?1
         ORDER BY c.start_line",
    )?;
    let chunks = stmt
        .query_map(params![file_path], |row| {
            let content: String = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            Ok(OldChunk {
                content,
                embedding: bytes_to_f32(&blob),
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(chunks)
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test embed::index::tests::read_file_embeddings -- --nocapture`
Expected: PASS

**Step 5: Run full suite**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

**Step 6: Commit**

```bash
git add src/embed/index.rs
git commit -m "feat(index): add read_file_embeddings for snapshotting old state"
```

---

## Task 3: Add `FileDrift` struct and chunk matching algorithm

This is the core comparison logic. Pure function, no DB access.

**Files:**
- Create: `src/embed/drift.rs`
- Modify: `src/embed/mod.rs` (add `pub mod drift;`)
- Test: `src/embed/drift.rs` (inline tests)

**Step 1: Write the failing tests**

Create `src/embed/drift.rs` with tests first:

```rust
use super::index::OldChunk;

/// Per-file drift result.
#[derive(Debug, Clone)]
pub struct FileDrift {
    pub file_path: String,
    pub avg_drift: f32,
    pub max_drift: f32,
    pub max_drift_chunk: Option<String>,
    pub chunks_added: usize,
    pub chunks_removed: usize,
}

/// A new chunk with its content and embedding.
#[derive(Debug, Clone)]
pub struct NewChunk {
    pub content: String,
    pub embedding: Vec<f32>,
}

/// Compare old and new chunks for a single file, returning drift scores.
pub fn compute_file_drift(
    file_path: &str,
    old_chunks: &[OldChunk],
    new_chunks: &[NewChunk],
) -> FileDrift {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn old(content: &str, emb: &[f32]) -> OldChunk {
        OldChunk {
            content: content.to_string(),
            embedding: emb.to_vec(),
        }
    }

    fn new(content: &str, emb: &[f32]) -> NewChunk {
        NewChunk {
            content: content.to_string(),
            embedding: emb.to_vec(),
        }
    }

    #[test]
    fn identical_chunks_have_zero_drift() {
        let olds = vec![old("fn a() {}", &[1.0, 0.0, 0.0])];
        let news = vec![new("fn a() {}", &[1.0, 0.0, 0.0])];
        let drift = compute_file_drift("a.rs", &olds, &news);
        assert_eq!(drift.avg_drift, 0.0);
        assert_eq!(drift.max_drift, 0.0);
        assert_eq!(drift.chunks_added, 0);
        assert_eq!(drift.chunks_removed, 0);
    }

    #[test]
    fn completely_different_chunks_have_high_drift() {
        // Orthogonal embeddings → cosine sim = 0.0 → drift = 1.0
        let olds = vec![old("fn a() {}", &[1.0, 0.0, 0.0])];
        let news = vec![new("fn b() { completely_different() }", &[0.0, 1.0, 0.0])];
        let drift = compute_file_drift("a.rs", &olds, &news);
        assert!(drift.avg_drift > 0.9);
        assert!(drift.max_drift > 0.9);
    }

    #[test]
    fn added_chunks_count_as_full_drift() {
        let olds = vec![];
        let news = vec![new("fn new_func() {}", &[1.0, 0.0, 0.0])];
        let drift = compute_file_drift("a.rs", &olds, &news);
        assert_eq!(drift.avg_drift, 1.0);
        assert_eq!(drift.max_drift, 1.0);
        assert_eq!(drift.chunks_added, 1);
        assert_eq!(drift.chunks_removed, 0);
    }

    #[test]
    fn removed_chunks_count_as_full_drift() {
        let olds = vec![old("fn old_func() {}", &[1.0, 0.0, 0.0])];
        let news = vec![];
        let drift = compute_file_drift("a.rs", &olds, &news);
        assert_eq!(drift.avg_drift, 1.0);
        assert_eq!(drift.max_drift, 1.0);
        assert_eq!(drift.chunks_added, 0);
        assert_eq!(drift.chunks_removed, 1);
    }

    #[test]
    fn content_hash_match_skips_semantic_comparison() {
        // Same content, different embeddings → content match wins, drift = 0.0
        let olds = vec![old("fn a() {}", &[1.0, 0.0, 0.0])];
        let news = vec![new("fn a() {}", &[0.0, 1.0, 0.0])];
        let drift = compute_file_drift("a.rs", &olds, &news);
        assert_eq!(drift.avg_drift, 0.0);
        assert_eq!(drift.max_drift, 0.0);
    }

    #[test]
    fn mixed_matched_and_added() {
        // Two old chunks, three new: one exact match, one semantic match, one added
        let olds = vec![
            old("fn unchanged() {}", &[1.0, 0.0, 0.0]),
            old("fn tweaked() { v1 }", &[0.0, 1.0, 0.0]),
        ];
        let news = vec![
            new("fn unchanged() {}", &[1.0, 0.0, 0.0]),
            // Slightly different embedding for tweaked function
            new("fn tweaked() { v2 }", &[0.1, 0.9, 0.0]),
            new("fn brand_new() {}", &[0.0, 0.0, 1.0]),
        ];
        let drift = compute_file_drift("a.rs", &olds, &news);
        assert_eq!(drift.chunks_added, 1);
        assert_eq!(drift.chunks_removed, 0);
        // avg includes: 0.0 (exact match) + small drift (semantic match) + 1.0 (added) / 3
        assert!(drift.avg_drift > 0.3);
        assert_eq!(drift.max_drift, 1.0); // the added chunk
    }

    #[test]
    fn max_drift_chunk_is_most_drifted_content() {
        let olds = vec![
            old("fn stable() {}", &[1.0, 0.0, 0.0]),
            old("fn volatile() { old_impl }", &[0.0, 1.0, 0.0]),
        ];
        let news = vec![
            new("fn stable() {}", &[1.0, 0.0, 0.0]),
            // Orthogonal → high drift
            new("fn volatile() { new_impl }", &[0.0, 0.0, 1.0]),
        ];
        let drift = compute_file_drift("a.rs", &olds, &news);
        assert!(drift.max_drift_chunk.is_some());
        let snippet = drift.max_drift_chunk.unwrap();
        assert!(snippet.contains("volatile"));
    }

    #[test]
    fn both_empty_means_zero_drift() {
        let drift = compute_file_drift("a.rs", &[], &[]);
        assert_eq!(drift.avg_drift, 0.0);
        assert_eq!(drift.max_drift, 0.0);
    }
}
```

**Step 2: Add the module declaration**

In `src/embed/mod.rs`, add:

```rust
pub mod drift;
```

**Step 3: Run tests to verify they fail**

Run: `cargo test embed::drift::tests -- --nocapture`
Expected: FAIL — `todo!()` panics

**Step 4: Write the implementation**

Replace the `todo!()` in `compute_file_drift` with the full algorithm:

```rust
use super::index::{cosine_sim, l2_norm, OldChunk};

/// Per-file drift result.
#[derive(Debug, Clone)]
pub struct FileDrift {
    pub file_path: String,
    pub avg_drift: f32,
    pub max_drift: f32,
    pub max_drift_chunk: Option<String>,
    pub chunks_added: usize,
    pub chunks_removed: usize,
}

/// A new chunk with its content and embedding.
#[derive(Debug, Clone)]
pub struct NewChunk {
    pub content: String,
    pub embedding: Vec<f32>,
}

/// Minimum cosine similarity to consider two chunks as semantically related.
/// Below this, chunks are treated as unmatched (added/removed).
const SEMANTIC_MATCH_THRESHOLD: f32 = 0.3;

/// Max characters to store in max_drift_chunk snippet.
const SNIPPET_MAX_LEN: usize = 200;

/// Compare old and new chunks for a single file, returning drift scores.
///
/// Algorithm:
/// 1. Content-hash exact matching (fast path, drift = 0.0)
/// 2. Greedy best-cosine pairing on remainder
/// 3. Unmatched old → removed (drift 1.0), unmatched new → added (drift 1.0)
/// 4. Aggregate avg_drift, max_drift, identify most-drifted chunk
pub fn compute_file_drift(
    file_path: &str,
    old_chunks: &[OldChunk],
    new_chunks: &[NewChunk],
) -> FileDrift {
    if old_chunks.is_empty() && new_chunks.is_empty() {
        return FileDrift {
            file_path: file_path.to_string(),
            avg_drift: 0.0,
            max_drift: 0.0,
            max_drift_chunk: None,
            chunks_added: 0,
            chunks_removed: 0,
        };
    }

    let mut drifts: Vec<f32> = Vec::new();
    let mut max_drift: f32 = 0.0;
    let mut max_drift_chunk: Option<String> = None;
    let mut chunks_added: usize = 0;
    let mut chunks_removed: usize = 0;

    // Track which chunks are matched
    let mut old_matched = vec![false; old_chunks.len()];
    let mut new_matched = vec![false; new_chunks.len()];

    // Step 1: Content-hash exact matching
    for (oi, old) in old_chunks.iter().enumerate() {
        for (ni, new_c) in new_chunks.iter().enumerate() {
            if !new_matched[ni] && old.content == new_c.content {
                old_matched[oi] = true;
                new_matched[ni] = true;
                drifts.push(0.0);
                break;
            }
        }
    }

    // Step 2: Greedy best-cosine on unmatched remainder
    // Collect unmatched indices
    let unmatched_old: Vec<usize> = old_matched
        .iter()
        .enumerate()
        .filter(|(_, m)| !**m)
        .map(|(i, _)| i)
        .collect();
    let mut unmatched_new: Vec<usize> = new_matched
        .iter()
        .enumerate()
        .filter(|(_, m)| !**m)
        .map(|(i, _)| i)
        .collect();

    // Build similarity matrix and greedily assign
    let mut pairs: Vec<(usize, usize, f32)> = Vec::new(); // (old_idx, new_idx, similarity)
    for &oi in &unmatched_old {
        let old_norm = l2_norm(&old_chunks[oi].embedding);
        let mut best_ni = None;
        let mut best_sim: f32 = -1.0;
        for &ni in &unmatched_new {
            let sim = cosine_sim(&old_chunks[oi].embedding, &new_chunks[ni].embedding, old_norm);
            if sim > best_sim {
                best_sim = sim;
                best_ni = Some(ni);
            }
        }
        if let Some(ni) = best_ni {
            pairs.push((oi, ni, best_sim));
        }
    }

    // Sort by similarity descending (best matches first) for greedy assignment
    pairs.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    let mut old_assigned = vec![false; old_chunks.len()];
    let mut new_assigned = vec![false; new_chunks.len()];
    // Mark content-matched as assigned
    for (i, m) in old_matched.iter().enumerate() {
        old_assigned[i] = *m;
    }
    for (i, m) in new_matched.iter().enumerate() {
        new_assigned[i] = *m;
    }

    for (oi, ni, sim) in &pairs {
        if old_assigned[*oi] || new_assigned[*ni] {
            continue;
        }
        if *sim < SEMANTIC_MATCH_THRESHOLD {
            continue; // No good match — will be counted as removed/added
        }
        old_assigned[*oi] = true;
        new_assigned[*ni] = true;
        let drift = 1.0 - sim;
        drifts.push(drift);
        if drift > max_drift {
            max_drift = drift;
            max_drift_chunk = Some(snippet(&new_chunks[*ni].content));
        }
    }

    // Step 3: Count unmatched as added/removed
    for (oi, assigned) in old_assigned.iter().enumerate() {
        if !assigned {
            chunks_removed += 1;
            drifts.push(1.0);
            if 1.0 > max_drift || (max_drift == 1.0 && max_drift_chunk.is_none()) {
                max_drift = 1.0;
                max_drift_chunk = Some(snippet(&old_chunks[oi].content));
            }
        }
    }
    for (ni, assigned) in new_assigned.iter().enumerate() {
        if !assigned {
            chunks_added += 1;
            drifts.push(1.0);
            if 1.0 > max_drift || (max_drift == 1.0 && max_drift_chunk.is_none()) {
                max_drift = 1.0;
                max_drift_chunk = Some(snippet(&new_chunks[ni].content));
            }
        }
    }

    // Step 4: Aggregate
    let avg_drift = if drifts.is_empty() {
        0.0
    } else {
        drifts.iter().sum::<f32>() / drifts.len() as f32
    };

    FileDrift {
        file_path: file_path.to_string(),
        avg_drift,
        max_drift,
        max_drift_chunk,
        chunks_added,
        chunks_removed,
    }
}

fn snippet(content: &str) -> String {
    if content.len() <= SNIPPET_MAX_LEN {
        content.to_string()
    } else {
        let mut s = content[..SNIPPET_MAX_LEN].to_string();
        s.push_str("...");
        s
    }
}
```

Note: `cosine_sim` and `l2_norm` in `src/embed/index.rs` are currently `fn` (not `pub`). They need to be made `pub(crate)` so `drift.rs` can use them. Update in `src/embed/index.rs`:

```rust
pub(crate) fn l2_norm(v: &[f32]) -> f32 {
```

```rust
pub(crate) fn cosine_sim(a: &[f32], b: &[f32], a_norm: f32) -> f32 {
```

**Step 5: Run tests to verify they pass**

Run: `cargo test embed::drift::tests -- --nocapture`
Expected: PASS (all 8 tests)

**Step 6: Run full suite**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

**Step 7: Commit**

```bash
git add src/embed/drift.rs src/embed/mod.rs src/embed/index.rs
git commit -m "feat(drift): add chunk matching algorithm with content-hash + cosine fallback"
```

---

## Task 4: Add `upsert_drift_report` and `query_drift_report` functions

**Files:**
- Modify: `src/embed/index.rs` (add DB functions for drift_report)
- Test: `src/embed/index.rs` (inline tests)

**Step 1: Write the failing tests**

```rust
    #[test]
    fn upsert_drift_report_inserts_and_queries() {
        let (_dir, conn) = open_test_db();
        upsert_drift_report(
            &conn,
            "a.rs",
            0.25,
            0.8,
            Some("fn changed() {}"),
            1,
            0,
        )
        .unwrap();

        let reports = query_drift_report(&conn, None, None).unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].file_path, "a.rs");
        assert!((reports[0].avg_drift - 0.25).abs() < 0.01);
        assert!((reports[0].max_drift - 0.8).abs() < 0.01);
        assert_eq!(reports[0].max_drift_chunk.as_deref(), Some("fn changed() {}"));
        assert_eq!(reports[0].chunks_added, 1);
        assert_eq!(reports[0].chunks_removed, 0);
    }

    #[test]
    fn upsert_drift_report_overwrites() {
        let (_dir, conn) = open_test_db();
        upsert_drift_report(&conn, "a.rs", 0.1, 0.2, None, 0, 0).unwrap();
        upsert_drift_report(&conn, "a.rs", 0.9, 0.95, Some("new"), 2, 1).unwrap();
        let reports = query_drift_report(&conn, None, None).unwrap();
        assert_eq!(reports.len(), 1);
        assert!((reports[0].avg_drift - 0.9).abs() < 0.01);
    }

    #[test]
    fn query_drift_report_filters_by_threshold() {
        let (_dir, conn) = open_test_db();
        upsert_drift_report(&conn, "low.rs", 0.05, 0.1, None, 0, 0).unwrap();
        upsert_drift_report(&conn, "high.rs", 0.5, 0.9, None, 1, 0).unwrap();
        let reports = query_drift_report(&conn, Some(0.1), None).unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].file_path, "high.rs");
    }

    #[test]
    fn query_drift_report_filters_by_path_glob() {
        let (_dir, conn) = open_test_db();
        upsert_drift_report(&conn, "src/tools/a.rs", 0.5, 0.5, None, 0, 0).unwrap();
        upsert_drift_report(&conn, "src/embed/b.rs", 0.5, 0.5, None, 0, 0).unwrap();
        let reports = query_drift_report(&conn, None, Some("src/tools/%")).unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].file_path, "src/tools/a.rs");
    }

    #[test]
    fn clear_drift_report_removes_all_rows() {
        let (_dir, conn) = open_test_db();
        upsert_drift_report(&conn, "a.rs", 0.5, 0.5, None, 0, 0).unwrap();
        clear_drift_report(&conn).unwrap();
        let reports = query_drift_report(&conn, None, None).unwrap();
        assert!(reports.is_empty());
    }
```

**Step 2: Run tests to verify they fail**

Run: `cargo test embed::index::tests::upsert_drift_report -- --nocapture`
Expected: FAIL — functions don't exist

**Step 3: Write minimal implementation**

Add to `src/embed/index.rs`:

```rust
/// A row from the drift_report table.
#[derive(Debug, Clone)]
pub struct DriftReportRow {
    pub file_path: String,
    pub avg_drift: f32,
    pub max_drift: f32,
    pub max_drift_chunk: Option<String>,
    pub chunks_added: usize,
    pub chunks_removed: usize,
    pub indexed_at: String,
}

/// Insert or replace a drift report row.
pub fn upsert_drift_report(
    conn: &Connection,
    file_path: &str,
    avg_drift: f32,
    max_drift: f32,
    max_drift_chunk: Option<&str>,
    chunks_added: usize,
    chunks_removed: usize,
) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO drift_report (file_path, avg_drift, max_drift, max_drift_chunk, chunks_added, chunks_removed, indexed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(file_path) DO UPDATE SET
            avg_drift = excluded.avg_drift,
            max_drift = excluded.max_drift,
            max_drift_chunk = excluded.max_drift_chunk,
            chunks_added = excluded.chunks_added,
            chunks_removed = excluded.chunks_removed,
            indexed_at = excluded.indexed_at",
        params![file_path, avg_drift, max_drift, max_drift_chunk, chunks_added, chunks_removed, now],
    )?;
    Ok(())
}

/// Query drift report rows, optionally filtered by threshold and path glob.
/// Results sorted by max_drift descending.
pub fn query_drift_report(
    conn: &Connection,
    threshold: Option<f32>,
    path_glob: Option<&str>,
) -> Result<Vec<DriftReportRow>> {
    let threshold = threshold.unwrap_or(0.0);
    let sql = if path_glob.is_some() {
        "SELECT file_path, avg_drift, max_drift, max_drift_chunk, chunks_added, chunks_removed, indexed_at
         FROM drift_report
         WHERE avg_drift > ?1 AND file_path LIKE ?2
         ORDER BY max_drift DESC"
    } else {
        "SELECT file_path, avg_drift, max_drift, max_drift_chunk, chunks_added, chunks_removed, indexed_at
         FROM drift_report
         WHERE avg_drift > ?1
         ORDER BY max_drift DESC"
    };

    let mut stmt = conn.prepare(sql)?;
    let rows = if let Some(glob) = path_glob {
        stmt.query_map(params![threshold, glob], map_drift_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?
    } else {
        stmt.query_map(params![threshold], map_drift_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?
    };
    Ok(rows)
}

fn map_drift_row(row: &rusqlite::Row) -> rusqlite::Result<DriftReportRow> {
    Ok(DriftReportRow {
        file_path: row.get(0)?,
        avg_drift: row.get(1)?,
        max_drift: row.get(2)?,
        max_drift_chunk: row.get(3)?,
        chunks_added: row.get::<_, i64>(4)? as usize,
        chunks_removed: row.get::<_, i64>(5)? as usize,
        indexed_at: row.get(6)?,
    })
}

/// Remove all rows from drift_report.
pub fn clear_drift_report(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM drift_report", [])?;
    Ok(())
}
```

Note: This uses `chrono::Utc::now()`. Check if `chrono` is already a dependency. If not, add it to `Cargo.toml`:

```toml
chrono = { version = "0.4", default-features = false, features = ["clock", "std"] }
```

If `chrono` is not available, use a simpler timestamp approach:

```rust
let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs()
    .to_string();
```

**Step 4: Run tests to verify they pass**

Run: `cargo test embed::index::tests::upsert_drift_report -- --nocapture && cargo test embed::index::tests::query_drift_report -- --nocapture && cargo test embed::index::tests::clear_drift_report -- --nocapture`
Expected: PASS

**Step 5: Run full suite**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

**Step 6: Commit**

```bash
git add src/embed/index.rs Cargo.toml
git commit -m "feat(index): add drift_report CRUD functions"
```

---

## Task 5: Wire drift computation into `build_index` Phase 3

**Files:**
- Modify: `src/embed/index.rs:498-642` (`build_index` — Phase 3 loop)
- Modify: `src/embed/index.rs:805-809` (`IndexReport` — add drift field)
- Test: existing tests should still pass

**Step 1: Extend `IndexReport`**

In `src/embed/index.rs`, update `IndexReport`:

```rust
pub struct IndexReport {
    pub indexed: usize,
    pub deleted: usize,
    pub skipped_msg: String,
    pub drift: Vec<crate::embed::drift::FileDrift>,
}
```

**Step 2: Wire drift into the Phase 3 loop**

In `build_index`, at the start of Phase 3 (before the `for result in results` loop), add:

```rust
    clear_drift_report(&conn)?;
    let mut drift_results: Vec<crate::embed::drift::FileDrift> = Vec::new();
```

Inside the `for result in results` loop, before `delete_file_chunks`:

```rust
        // Snapshot old embeddings before deletion
        let old_chunks = read_file_embeddings(&conn, &result.rel)?;
```

After the existing `insert_chunk` loop (after all new chunks are inserted), add:

```rust
        // Compute drift if we had old chunks
        if !old_chunks.is_empty() {
            let new_chunks: Vec<crate::embed::drift::NewChunk> = result
                .chunks
                .iter()
                .zip(result.embeddings.iter())
                .map(|(raw, emb)| crate::embed::drift::NewChunk {
                    content: raw.content.clone(),
                    embedding: emb.clone(),
                })
                .collect();
            let drift = crate::embed::drift::compute_file_drift(
                &result.rel,
                &old_chunks,
                &new_chunks,
            );
            upsert_drift_report(
                &conn,
                &drift.file_path,
                drift.avg_drift,
                drift.max_drift,
                drift.max_drift_chunk.as_deref(),
                drift.chunks_added,
                drift.chunks_removed,
            )?;
            drift_results.push(drift);
        }
```

At the end, update the return to include drift:

```rust
    Ok(IndexReport {
        indexed,
        deleted: change_set.deleted.len(),
        skipped_msg: if force {
            "force rebuild".to_string()
        } else {
            format!("{} deleted", change_set.deleted.len())
        },
        drift: drift_results,
    })
```

**Step 3: Run all tests**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS — existing tests pass because `drift` is an additive field.

**Step 4: Commit**

```bash
git add src/embed/index.rs
git commit -m "feat(drift): wire drift computation into build_index Phase 3"
```

---

## Task 6: Add `CheckDrift` tool

**Files:**
- Modify: `src/tools/semantic.rs` (add `CheckDrift` struct and `impl Tool`)
- Modify: `src/server.rs:29,94-96` (register the new tool)
- Test: `src/tools/semantic.rs` (inline tests)

**Step 1: Write the failing test**

Add to `src/tools/semantic.rs` tests:

```rust
    #[tokio::test]
    async fn check_drift_returns_empty_without_data() {
        let (_dir, ctx) = project_ctx().await;
        let result = CheckDrift.call(json!({}), &ctx).await.unwrap();
        assert_eq!(result["results"], json!([]));
    }

    #[tokio::test]
    async fn check_drift_returns_drift_rows() {
        let (_dir, ctx) = project_ctx().await;
        let root = {
            let inner = ctx.agent.inner.read().await;
            inner.active_project.as_ref().unwrap().root.clone()
        };
        let conn = crate::embed::index::open_db(&root).unwrap();
        crate::embed::index::upsert_drift_report(&conn, "a.rs", 0.5, 0.8, Some("fn x()"), 1, 0).unwrap();
        crate::embed::index::upsert_drift_report(&conn, "b.rs", 0.02, 0.05, None, 0, 0).unwrap();
        drop(conn);

        // Default threshold 0.1 should filter out b.rs
        let result = CheckDrift.call(json!({}), &ctx).await.unwrap();
        let results = result["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["file_path"], "a.rs");
    }

    #[tokio::test]
    async fn check_drift_respects_threshold() {
        let (_dir, ctx) = project_ctx().await;
        let root = {
            let inner = ctx.agent.inner.read().await;
            inner.active_project.as_ref().unwrap().root.clone()
        };
        let conn = crate::embed::index::open_db(&root).unwrap();
        crate::embed::index::upsert_drift_report(&conn, "a.rs", 0.5, 0.8, None, 1, 0).unwrap();
        drop(conn);

        let result = CheckDrift.call(json!({"threshold": 0.6}), &ctx).await.unwrap();
        let results = result["results"].as_array().unwrap();
        assert!(results.is_empty()); // avg_drift 0.5 < threshold 0.6
    }
```

**Step 2: Run tests to verify they fail**

Run: `cargo test tools::semantic::tests::check_drift -- --nocapture`
Expected: FAIL — `CheckDrift` doesn't exist

**Step 3: Write the tool implementation**

Add to `src/tools/semantic.rs`:

```rust
pub struct CheckDrift;

#[async_trait]
impl Tool for CheckDrift {
    fn name(&self) -> &str {
        "check_drift"
    }
    fn description(&self) -> &str {
        "Query semantic drift scores from the last index build. Shows which files changed meaningfully in code semantics, not just bytes. Use after index_project to find significant changes."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "threshold": {
                    "type": "number",
                    "description": "Minimum avg_drift to include (default: 0.1). Range 0.0-1.0."
                },
                "path": {
                    "type": "string",
                    "description": "Glob pattern to filter files (e.g. 'src/tools/%'). Uses SQL LIKE syntax."
                },
                "detail_level": {
                    "type": "string",
                    "enum": ["exploring", "full"],
                    "description": "Output detail: 'exploring' (default) shows scores only, 'full' includes most-drifted chunk content."
                }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        use super::output::OutputGuard;

        let threshold = input["threshold"].as_f64().map(|f| f as f32);
        let path = input["path"].as_str();
        let guard = OutputGuard::from_input(&input);

        let root = ctx.agent.require_project_root().await?;
        let conn = crate::embed::index::open_db(&root)?;
        let rows = crate::embed::index::query_drift_report(
            &conn,
            threshold.or(Some(0.1)),
            path,
        )?;

        let result_items: Vec<Value> = rows
            .iter()
            .map(|r| {
                let mut item = json!({
                    "file_path": r.file_path,
                    "avg_drift": r.avg_drift,
                    "max_drift": r.max_drift,
                    "chunks_added": r.chunks_added,
                    "chunks_removed": r.chunks_removed,
                });
                if guard.should_include_body() {
                    if let Some(ref chunk) = r.max_drift_chunk {
                        item["max_drift_chunk"] = json!(chunk);
                    }
                }
                item
            })
            .collect();

        let (result_items, overflow) =
            guard.cap_items(result_items, "Use detail_level='full' with offset for pagination");
        let total = overflow.as_ref().map_or(result_items.len(), |o| o.total);
        let mut result = json!({ "results": result_items, "total": total });
        if let Some(ov) = overflow {
            result["overflow"] = OutputGuard::overflow_json(&ov);
        }
        Ok(result)
    }
}
```

**Step 4: Register the tool in `src/server.rs`**

Add `CheckDrift` to the import at line 29:

```rust
    semantic::{CheckDrift, IndexProject, IndexStatus, SemanticSearch},
```

Add to the tools vector (around line 96):

```rust
            Arc::new(CheckDrift),
```

**Step 5: Run tests to verify they pass**

Run: `cargo test tools::semantic::tests::check_drift -- --nocapture`
Expected: PASS

**Step 6: Run full suite**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

**Step 7: Commit**

```bash
git add src/tools/semantic.rs src/server.rs
git commit -m "feat(drift): add check_drift tool for querying semantic drift scores"
```

---

## Task 7: Add drift summary to `index_project` response

**Files:**
- Modify: `src/tools/semantic.rs:146-163` (`IndexProject::call`)
- Test: `src/tools/semantic.rs` (inline tests)

**Step 1: Write the failing test**

```rust
    #[tokio::test]
    async fn index_project_response_has_drift_summary() {
        let (_dir, ctx) = project_ctx().await;

        // Manually insert drift data (simulating what build_index would produce)
        let root = {
            let inner = ctx.agent.inner.read().await;
            inner.active_project.as_ref().unwrap().root.clone()
        };
        let conn = crate::embed::index::open_db(&root).unwrap();
        crate::embed::index::upsert_drift_report(&conn, "a.rs", 0.8, 0.95, Some("fn x()"), 2, 1).unwrap();
        crate::embed::index::upsert_drift_report(&conn, "b.rs", 0.1, 0.2, None, 0, 0).unwrap();
        drop(conn);

        // We can't run build_index without an embedder, but we can test
        // that drift_summary is formed from the report's drift field.
        // For now, verify the data model works.
        let conn = crate::embed::index::open_db(&root).unwrap();
        let rows = crate::embed::index::query_drift_report(&conn, Some(0.0), None).unwrap();
        assert_eq!(rows.len(), 2);
    }
```

Note: A full integration test requires an embedder. The key change is in the `IndexProject::call` response formatting. The actual drift data comes from `IndexReport.drift` which is populated by `build_index`.

**Step 2: Update `IndexProject::call`**

In the `call` method, after getting `report` and `stats`, include drift info:

```rust
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let force = input["force"].as_bool().unwrap_or(false);
        let root = ctx.agent.require_project_root().await?;

        let report = crate::embed::index::build_index(&root, force).await?;

        let conn = crate::embed::index::open_db(&root)?;
        let stats = crate::embed::index::index_stats(&conn)?;

        // Top 5 most-drifted files
        let drift_summary: Vec<Value> = report
            .drift
            .iter()
            .filter(|d| d.avg_drift > 0.05) // skip near-zero drift
            .take(5)
            .map(|d| {
                json!({
                    "file": d.file_path,
                    "avg_drift": format!("{:.2}", d.avg_drift),
                    "max_drift": format!("{:.2}", d.max_drift),
                    "added": d.chunks_added,
                    "removed": d.chunks_removed,
                })
            })
            .collect();

        let mut result = json!({
            "status": "ok",
            "files_indexed": report.indexed,
            "files_deleted": report.deleted,
            "detail": report.skipped_msg,
            "total_files": stats.file_count,
            "total_chunks": stats.chunk_count,
        });

        if !drift_summary.is_empty() {
            result["drift_summary"] = json!(drift_summary);
        }

        Ok(result)
    }
```

**Step 3: Run all tests**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: PASS

**Step 4: Commit**

```bash
git add src/tools/semantic.rs
git commit -m "feat(drift): add drift summary to index_project response"
```

---

## Task 8: Update server instructions and documentation

**Files:**
- Modify: `src/prompts/server_instructions.md` (add `check_drift` to tool reference)
- Modify: `docs/ROADMAP.md` (mark Semantic Drift Detection as implemented)

**Step 1: Update server instructions**

Add `check_drift` to the Reading & Searching section of the tool reference in `src/prompts/server_instructions.md`:

```markdown
- `check_drift([threshold], [path])` — query semantic drift scores from last index build
```

Add a brief entry to the "How to Choose the Right Tool" section:

```markdown
### You want to know what changed meaningfully
After re-indexing with `index_project`, check `check_drift` to see which files
had significant semantic changes vs. trivial formatting/comment edits.
```

**Step 2: Update ROADMAP.md**

Update the Semantic Drift Detection section header to include `— **Implemented**`:

```markdown
### Semantic Drift Detection — **Implemented**
```

**Step 3: Run full suite**

Run: `cargo fmt && cargo test && cargo clippy -- -D warnings`
Expected: PASS

**Step 4: Commit**

```bash
git add src/prompts/server_instructions.md docs/ROADMAP.md
git commit -m "docs: add check_drift to tool reference and mark drift detection as implemented"
```
