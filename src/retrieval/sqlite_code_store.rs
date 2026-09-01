//! In-process, daemon-free [`CodeVectorStore`] backed by **sqlite-vec** (`vec0`).
//!
//! This is the "lite" stack's code-search backend (see
//! `docs/plans/archive/2026-06-16-two-stack-retrieval-lite.md`): no Qdrant, no Docker —
//! just a per-project SQLite file with a statically-linked `vec0` virtual table.
//! It survives a locked-down VDI (no runtime DLL for an EDR to quarantine) and
//! needs only a remote OpenAI-compatible embedding endpoint.
//!
//! Dense-only by design: `vec0` ranks on the dense vector; the `sparse` /
//! `bm25_boost` / `disable_sparse` query args are ignored (the lite stack has no
//! sparse leg). This mirrors how memory recall and the librarian sqlite-vec
//! artifact store already behave.
//!
//! ## Storage layout
//! One DB per project id under a data dir resolved by
//! `crate::retrieval::config::resolve_sqlite_dir` and carried on
//! `RetrievalConfig::sqlite_dir` — `$CODESCOUT_SQLITE_DIR` if set, else
//! `<project_root>/.codescout/embeddings/`, else (rootless callers only)
//! `<home>/.codescout/embeddings/`. This store does NOT read the environment;
//! it is handed a directory. Tables are created lazily; the `vec0`
//! dimension is inferred from the first batch of embeddings (so a remote model's
//! native dim is adopted automatically). A dimension change requires a reindex —
//! same caveat as switching Qdrant embedders (WIN-22).

use crate::retrieval::code_store::CodeVectorStore;
use crate::retrieval::drift::ChunkRef;
use crate::retrieval::embedder::{EmbedOutput, SparseVector};
use crate::retrieval::payload::CodePayload;
use crate::retrieval::search::Hit;
use crate::sqlite_vec_ext::dense_blob;
use anyhow::{Context, Result};
use async_trait::async_trait;
use parking_lot::Mutex;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

pub struct SqliteVecCodeStore {
    dir: PathBuf,
    /// One cached connection per project id. `vec0` connections are `!Sync`, so
    /// each is wrapped in its own mutex; the outer mutex guards the cache map.
    conns: Mutex<HashMap<String, Arc<Mutex<Connection>>>>,
}

impl SqliteVecCodeStore {
    /// Construct a store rooted at `dir` (one DB file per project id beneath it).
    pub fn at(dir: PathBuf) -> Self {
        Self {
            dir,
            conns: Mutex::new(HashMap::new()),
        }
    }

    /// Open (once) and cache the connection for `project_id`, creating the base
    /// `code_chunk` table. The `vec0` table is created lazily on first upsert,
    /// when the embedding dimension is known.
    fn conn_for(&self, project_id: &str) -> Result<Arc<Mutex<Connection>>> {
        crate::sqlite_vec_ext::open_conn(
            &self.dir,
            &self.conns,
            project_id,
            ".db",
            "CREATE TABLE IF NOT EXISTS code_chunk (
                 chunk_id     TEXT PRIMARY KEY,
                 project_id   TEXT NOT NULL,
                 file_path    TEXT NOT NULL,
                 language     TEXT NOT NULL,
                 start_line   INTEGER NOT NULL,
                 end_line     INTEGER NOT NULL,
                 content      TEXT NOT NULL,
                 content_hash TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_code_chunk_project ON code_chunk(project_id);",
        )
    }

    /// Ensure the `code_vec` virtual table exists with the given dim. Validates
    /// against the existing dim (reindex required on a mismatch).
    fn ensure_vec_table(conn: &Connection, dim: usize) -> Result<()> {
        use rusqlite::OptionalExtension;
        // Probe table existence via sqlite_master first so a genuine read error
        // (corruption, lock) propagates instead of being swallowed as "no table
        // yet" — only a missing or empty table yields None below.
        let present: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='code_vec'",
                [],
                |_| Ok(true),
            )
            .optional()
            .context("probe code_vec existence")?
            .unwrap_or(false);
        if present {
            let blob_len: Option<i64> = conn
                .query_row("SELECT length(embedding) FROM code_vec LIMIT 1", [], |r| {
                    r.get(0)
                })
                .optional()
                .context("read existing code_vec dim")?;
            if let Some(blob_len) = blob_len {
                let existing_dim = (blob_len / 4) as usize;
                if existing_dim != dim {
                    anyhow::bail!(
                        "sqlite-vec code index dim mismatch: existing={existing_dim}, batch={dim}. \
                         The embedding model/dim changed — reindex with force=true to rebuild."
                    );
                }
            }
            return Ok(());
        }
        // FLOAT[N] requires the dim as a literal at CREATE time. vec0 defaults to
        // L2 distance (not cosine); `query` maps it to a score via 1/(1+dist).
        // Ranking matches the server stack's cosine distance only for L2-normalized
        // embeddings (what OpenAI-compatible code embedders emit), which the lite
        // stack assumes. See the two-stack plan's quality tradeoff.
        conn.execute_batch(&format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS code_vec USING vec0(
                 chunk_id TEXT PRIMARY KEY,
                 embedding FLOAT[{dim}]
             );"
        ))
        .context("create code_vec table")?;
        Ok(())
    }
}

#[async_trait]
impl CodeVectorStore for SqliteVecCodeStore {
    async fn ensure_collection(&self, _collection: &str, _dim: u64) -> Result<()> {
        // Per-project tables are created lazily (conn_for / first upsert); the
        // dim is inferred from the embeddings, not this hint. Nothing to do.
        Ok(())
    }

    async fn chunk_refs(&self, _collection: &str, project_id: &str) -> Result<Vec<ChunkRef>> {
        let conn = self.conn_for(project_id)?;
        let conn = conn.lock();
        let mut stmt = conn.prepare(
            "SELECT chunk_id, content_hash, file_path FROM code_chunk WHERE project_id = ?1",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![project_id], |row| {
                Ok(ChunkRef {
                    chunk_id: row.get(0)?,
                    content_hash: row.get(1)?,
                    file_path: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<ChunkRef>>>()?;
        Ok(rows)
    }

    async fn upsert_chunks(
        &self,
        _collection: &str,
        chunks: &[(CodePayload, EmbedOutput)],
    ) -> Result<()> {
        if chunks.is_empty() {
            return Ok(());
        }
        let project_id = chunks[0].0.project_id.clone();
        let dim = chunks[0].1.dense.len();
        if dim == 0 {
            anyhow::bail!("sqlite-vec upsert: embedding dim is 0 (embedder error sentinel?)");
        }
        let conn = self.conn_for(&project_id)?;
        let mut conn = conn.lock();
        Self::ensure_vec_table(&conn, dim)?;
        let tx = conn.transaction()?;
        for (p, e) in chunks {
            if e.dense.len() != dim {
                anyhow::bail!(
                    "sqlite-vec upsert: ragged batch dims ({} vs {})",
                    e.dense.len(),
                    dim
                );
            }
            // vec0 ignores INSERT OR REPLACE — DELETE then INSERT for idempotency
            // (same contract as the librarian artifact store / BUG-045).
            tx.execute(
                "DELETE FROM code_chunk WHERE chunk_id = ?1",
                rusqlite::params![p.chunk_id],
            )?;
            tx.execute(
                "DELETE FROM code_vec WHERE chunk_id = ?1",
                rusqlite::params![p.chunk_id],
            )?;
            tx.execute(
                "INSERT INTO code_chunk
                   (chunk_id, project_id, file_path, language, start_line, end_line, content, content_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    p.chunk_id,
                    p.project_id,
                    p.file_path,
                    p.language,
                    p.start_line,
                    p.end_line,
                    p.content,
                    p.content_hash,
                ],
            )?;
            tx.execute(
                "INSERT INTO code_vec (chunk_id, embedding) VALUES (?1, ?2)",
                rusqlite::params![p.chunk_id, dense_blob(&e.dense)],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    async fn delete_chunks(
        &self,
        _collection: &str,
        project_id: &str,
        ids: &[String],
    ) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        // Locate the per-project DB by the caller's `project_id`, NOT by parsing
        // the chunk_id prefix: chunk_id is `{project_id}:{rel}:{hash}` and a
        // project_id can itself contain a colon (libraries are `lib:{name}`), so
        // splitting on ':' would open the wrong DB (`lib.db` instead of
        // `lib_<name>.db`) and silently delete nothing.
        let conn = self.conn_for(project_id)?;
        let mut conn = conn.lock();
        let tx = conn.transaction()?;
        for id in ids {
            tx.execute(
                "DELETE FROM code_chunk WHERE chunk_id = ?1",
                rusqlite::params![id],
            )?;
            tx.execute(
                "DELETE FROM code_vec WHERE chunk_id = ?1",
                rusqlite::params![id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    async fn query(
        &self,
        _collection: &str,
        project_id: &str,
        dense: &[f32],
        _sparse: &SparseVector,
        limit: usize,
        _bm25_boost: f32,
        _disable_sparse: bool,
        exclude_languages: &[String],
        exclude_paths: &[String],
    ) -> Result<Vec<Hit>> {
        let conn = self.conn_for(project_id)?;
        let conn = conn.lock();
        // No vec0 table yet → nothing indexed → no hits.
        let has_vec: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='code_vec'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);
        if !has_vec {
            return Ok(Vec::new());
        }
        // KNN on the dense leg, then hydrate payload + filter language/path in Rust.
        // `language` and `file_path` live in the JOINed code_chunk, not the vec0
        // table, so neither exclusion can be pushed into the KNN (the server stack
        // pre-filters both inside the search). vec0 returns exactly `k` nearest by
        // distance, so when either list is non-empty we widen `k` to give the
        // post-filter headroom and avoid under-returning when excluded
        // languages/paths dominate the neighborhood. The caller (`SearchOpts`)
        // already overfetches; this is extra cushion. Exact parity would require
        // storing language and file_path as vec0 metadata columns.
        let k = if exclude_languages.is_empty() && exclude_paths.is_empty() {
            limit
        } else {
            limit.saturating_mul(4)
        };
        let mut stmt = conn.prepare(
            "SELECT v.distance, c.chunk_id, c.file_path, c.language, c.start_line, c.end_line, c.content
                 FROM code_vec v JOIN code_chunk c ON c.chunk_id = v.chunk_id
                WHERE v.embedding MATCH vec_f32(?1) AND k = ?3 AND c.project_id = ?2
                ORDER BY v.distance",
        )?;
        let rows = stmt
            .query_map(
                rusqlite::params![dense_blob(dense), project_id, k as i64],
                |row| {
                    let distance: f64 = row.get(0)?;
                    Ok((
                        distance,
                        Hit {
                            chunk_id: row.get(1)?,
                            file_path: row.get(2)?,
                            start_line: row.get(4)?,
                            end_line: row.get(5)?,
                            content: row.get(6)?,
                            score: 1.0 / (1.0 + distance as f32),
                            rerank_score: None,
                        },
                        row.get::<_, String>(3)?, // language
                    ))
                },
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows
            .into_iter()
            .filter(|(_, hit, lang)| {
                !exclude_languages.contains(lang) && !exclude_paths.contains(&hit.file_path)
            })
            .map(|(_, hit, _)| hit)
            .collect())
    }

    async fn project_index_stats(
        &self,
        _collection: &str,
        project_id: &str,
    ) -> Result<(usize, usize)> {
        let conn = self.conn_for(project_id)?;
        let conn = conn.lock();
        let chunks: i64 = conn.query_row(
            "SELECT count(*) FROM code_chunk WHERE project_id = ?1",
            rusqlite::params![project_id],
            |r| r.get(0),
        )?;
        let files: i64 = conn.query_row(
            "SELECT count(DISTINCT file_path) FROM code_chunk WHERE project_id = ?1",
            rusqlite::params![project_id],
            |r| r.get(0),
        )?;
        Ok((chunks as usize, files as usize))
    }

    async fn project_has_chunks(&self, _collection: &str, project_id: &str) -> Result<bool> {
        let conn = self.conn_for(project_id)?;
        let conn = conn.lock();
        // EXISTS stops at the first row; count(*) would scan the project's rows to
        // produce a number the caller immediately discards.
        let present: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM code_chunk WHERE project_id = ?1)",
            rusqlite::params![project_id],
            |r| r.get(0),
        )?;
        Ok(present)
    }

    async fn collection_dim(&self, _collection: &str, project_id: &str) -> Result<Option<u64>> {
        use rusqlite::OptionalExtension;
        let conn = self.conn_for(project_id)?;
        let conn = conn.lock();
        let present: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='code_vec'",
                [],
                |_| Ok(true),
            )
            .optional()
            .context("probe code_vec existence")?
            .unwrap_or(false);
        if !present {
            return Ok(None);
        }
        let blob_len: Option<i64> = conn
            .query_row("SELECT length(embedding) FROM code_vec LIMIT 1", [], |r| {
                r.get(0)
            })
            .optional()
            .context("read existing code_vec dim")?;
        Ok(blob_len.map(|n| (n / 4) as u64))
    }

    /// Drop this project's vector table and chunk metadata so the next sync can
    /// recreate `code_vec` at a different dimension.
    ///
    /// Scoping is structural, not query-enforced: `conn_for(project_id)` opens one
    /// `.db` file per project (see its doc comment), so `DROP TABLE code_vec` here
    /// cannot reach a sibling project's vectors — there is no shared table to be
    /// careful about. The `DELETE` still carries its `project_id` predicate because
    /// `code_chunk` has the column and honouring it costs nothing.
    ///
    /// `DROP TABLE` on a `vec0` virtual table takes its shadow tables
    /// (`code_vec_chunks`, `code_vec_rowids`, `code_vec_info`,
    /// `code_vec_vector_chunks00`) with it — sqlite-vec implements `xDestroy`. If
    /// that ever regresses, the symptom is the subsequent `CREATE VIRTUAL TABLE`
    /// colliding with a leftover shadow, which
    /// `reset_then_reindex_migrates_the_vector_table_to_a_new_dimension` catches.
    async fn reset_project_index(&self, _collection: &str, project_id: &str) -> Result<()> {
        let conn = self.conn_for(project_id)?;
        let conn = conn.lock();
        conn.execute_batch("DROP TABLE IF EXISTS code_vec;")
            .context("drop code_vec for dimension migration")?;
        conn.execute(
            "DELETE FROM code_chunk WHERE project_id = ?1",
            rusqlite::params![project_id],
        )
        .context("clear code_chunk for dimension migration")?;
        Ok(())
    }

    /// The backend where this can genuinely be non-zero: `code_chunk` and `code_vec`
    /// are two tables written by two statements inside `upsert_chunks`, so an
    /// interrupted or partially-failed write can leave metadata without a vector.
    ///
    /// Joins on `code_vec.chunk_id`, which is a declared `TEXT PRIMARY KEY` column of
    /// the vec0 table and the same column `query` already joins on — deliberately
    /// NOT on sqlite-vec's `code_vec_rowids` shadow table, which is an
    /// implementation detail that a version bump may rename.
    ///
    /// A hole is invisible from every other surface: the chunk still answers
    /// `chunk_refs`, still counts toward `project_index_stats`, and still makes
    /// `index(action="status")` report `indexed: true` — it simply never matches a
    /// query. Measured on this repo 2026-08-26: 0 of 46 979.
    async fn count_chunks_without_vectors(
        &self,
        _collection: &str,
        project_id: &str,
    ) -> Result<usize> {
        let conn = self.conn_for(project_id)?;
        let conn = conn.lock();
        // No `code_vec` yet means nothing has been embedded, so every chunk row
        // (if any) is a hole — but a fresh project has no chunk rows either, so this
        // correctly returns 0 rather than erroring on a missing table.
        let vec_tables: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='code_vec'",
                [],
                |r| r.get(0),
            )
            .context("probe code_vec existence")?;
        if vec_tables == 0 {
            let orphans: i64 = conn
                .query_row(
                    "SELECT count(*) FROM code_chunk WHERE project_id = ?1",
                    rusqlite::params![project_id],
                    |r| r.get(0),
                )
                .context("count chunks with no vec table")?;
            return Ok(orphans as usize);
        }
        let missing: i64 = conn
            .query_row(
                "SELECT count(*) FROM code_chunk c
                   LEFT JOIN code_vec v ON v.chunk_id = c.chunk_id
                  WHERE c.project_id = ?1 AND v.chunk_id IS NULL",
                rusqlite::params![project_id],
                |r| r.get(0),
            )
            .context("count chunks without vectors")?;
        Ok(missing as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(id: &str, project: &str, file: &str, lang: &str, hash: &str) -> CodePayload {
        CodePayload {
            project_id: project.into(),
            file_path: file.into(),
            language: lang.into(),
            start_line: 1,
            end_line: 2,
            ast_header: String::new(),
            content: format!("content of {id}"),
            content_hash: hash.into(),
            last_indexed_commit: String::new(),
            chunk_id: id.into(),
        }
    }

    fn embed(dense: Vec<f32>) -> EmbedOutput {
        EmbedOutput {
            dense,
            sparse: SparseVector {
                indices: vec![],
                values: vec![],
            },
        }
    }

    fn empty_sparse() -> SparseVector {
        SparseVector {
            indices: vec![],
            values: vec![],
        }
    }

    #[tokio::test]
    async fn real_vec0_upsert_query_orders_by_distance() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SqliteVecCodeStore::at(tmp.path().to_path_buf());
        store
            .upsert_chunks(
                "code_chunks",
                &[
                    (
                        payload("proj:a.rs:h1", "proj", "a.rs", "rust", "h1"),
                        embed(vec![1.0, 0.0, 0.0]),
                    ),
                    (
                        payload("proj:b.rs:h2", "proj", "b.rs", "rust", "h2"),
                        embed(vec![0.0, 1.0, 0.0]),
                    ),
                ],
            )
            .await
            .unwrap();

        let hits = store
            .query(
                "code_chunks",
                "proj",
                &[0.9, 0.1, 0.0],
                &empty_sparse(),
                10,
                3.0,
                true,
                &[],
                &[],
            )
            .await
            .unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(
            hits[0].chunk_id, "proj:a.rs:h1",
            "nearest vector ranks first"
        );
        assert_eq!(hits[0].file_path, "a.rs");
    }

    #[tokio::test]
    async fn collection_dim_reports_none_then_the_baked_dim() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SqliteVecCodeStore::at(tmp.path().to_path_buf());
        assert_eq!(
            store.collection_dim("code_chunks", "proj").await.unwrap(),
            None,
            "no table yet must be None, not an error"
        );
        let p = payload("c1", "proj", "a.rs", "rust", "h1");
        let e = embed(vec![0.1, 0.2, 0.3]);
        store.upsert_chunks("code_chunks", &[(p, e)]).await.unwrap();
        assert_eq!(
            store.collection_dim("code_chunks", "proj").await.unwrap(),
            Some(3),
            "vec0 bakes the dim at creation — report what it baked"
        );
    }

    /// The migration this bug is about, against a REAL `vec0` table: build at one
    /// dimension, reset, rebuild at another.
    ///
    /// This is the test that answers the residual risk the reconnaissance pass
    /// identified (`bug-fix-session-log:F-64`). `DROP TABLE` on a `vec0` virtual
    /// table must take its shadow tables with it — `code_vec_chunks`,
    /// `code_vec_rowids`, `code_vec_info`, `code_vec_vector_chunks00`, all four
    /// observed in a live index. If any survives, the rebuild's
    /// `CREATE VIRTUAL TABLE` collides with it and this test fails at the second
    /// `upsert_chunks`. That is the whole point: fail here, in a tempdir, rather
    /// than in a production migration that has already dropped the old index.
    ///
    /// A `RecordingStore` double cannot cover this — the question is entirely about
    /// what sqlite-vec's `xDestroy` actually does.
    #[tokio::test]
    async fn reset_then_reindex_migrates_the_vector_table_to_a_new_dimension() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SqliteVecCodeStore::at(tmp.path().to_path_buf());

        // Build at 3 dimensions.
        store
            .upsert_chunks(
                "code_chunks",
                &[(
                    payload("c1", "proj", "a.rs", "rust", "h1"),
                    embed(vec![0.1, 0.2, 0.3]),
                )],
            )
            .await
            .unwrap();
        assert_eq!(
            store.collection_dim("code_chunks", "proj").await.unwrap(),
            Some(3)
        );

        // The reset `force=true` could not previously perform.
        store
            .reset_project_index("code_chunks", "proj")
            .await
            .unwrap();
        assert_eq!(
            store.collection_dim("code_chunks", "proj").await.unwrap(),
            None,
            "the vector table must be GONE, not merely emptied — its dim is baked at \
             creation, so an emptied table would still refuse the new width"
        );
        assert!(
            store
                .chunk_refs("code_chunks", "proj")
                .await
                .unwrap()
                .is_empty(),
            "chunk metadata must go too, or the rebuild's prune step would delete the \
             very ids it is about to re-create"
        );

        // Rebuild at 5 dimensions — the migration itself.
        store
            .upsert_chunks(
                "code_chunks",
                &[(
                    payload("c2", "proj", "b.rs", "rust", "h2"),
                    embed(vec![0.1, 0.2, 0.3, 0.4, 0.5]),
                )],
            )
            .await
            .unwrap();
        assert_eq!(
            store.collection_dim("code_chunks", "proj").await.unwrap(),
            Some(5),
            "the rebuilt table must carry the NEW width — a surviving vec0 shadow \
             table surfaces right here"
        );

        // And the migrated index is actually searchable at the new width.
        let hits = store
            .query(
                "code_chunks",
                "proj",
                &[0.1, 0.2, 0.3, 0.4, 0.5],
                &empty_sparse(),
                10,
                0.0,
                true,
                &[],
                &[],
            )
            .await
            .unwrap();
        assert_eq!(
            hits.len(),
            1,
            "a migrated index that is not queryable is not a migration"
        );
        assert_eq!(hits[0].file_path, "b.rs");
    }

    #[tokio::test]
    async fn real_vec0_refs_stats_delete_and_language_filter() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SqliteVecCodeStore::at(tmp.path().to_path_buf());
        store
            .upsert_chunks(
                "c",
                &[
                    (
                        payload("proj:a.rs:h1", "proj", "a.rs", "rust", "h1"),
                        embed(vec![1.0, 0.0]),
                    ),
                    (
                        payload("proj:m.md:h2", "proj", "m.md", "markdown", "h2"),
                        embed(vec![1.0, 0.0]),
                    ),
                ],
            )
            .await
            .unwrap();

        assert_eq!(
            store.project_index_stats("c", "proj").await.unwrap(),
            (2, 2)
        );

        let mut refs = store.chunk_refs("c", "proj").await.unwrap();
        refs.sort_by(|a, b| a.chunk_id.cmp(&b.chunk_id));
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].file_path, "a.rs");
        assert_eq!(refs[1].file_path, "m.md");

        // markdown excluded → only the rust chunk
        let hits = store
            .query(
                "c",
                "proj",
                &[1.0, 0.0],
                &empty_sparse(),
                10,
                3.0,
                true,
                &["markdown".to_string()],
                &[],
            )
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].file_path, "a.rs");

        store
            .delete_chunks("c", "proj", &["proj:a.rs:h1".to_string()])
            .await
            .unwrap();
        assert_eq!(
            store.project_index_stats("c", "proj").await.unwrap(),
            (1, 1)
        );
    }

    #[tokio::test]
    async fn real_vec0_path_filter() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SqliteVecCodeStore::at(tmp.path().to_path_buf());
        store
            .upsert_chunks(
                "c",
                &[
                    (
                        payload("proj:a.rs:h1", "proj", "a.rs", "rust", "h1"),
                        embed(vec![1.0, 0.0]),
                    ),
                    (
                        payload("proj:b.rs:h2", "proj", "b.rs", "rust", "h2"),
                        embed(vec![1.0, 0.0]),
                    ),
                ],
            )
            .await
            .unwrap();

        // "b.rs" excluded by path → only "a.rs" remains. Both directions matter: a
        // guard asserted only in the negative direction passes whether or not it
        // discriminates.
        let hits = store
            .query(
                "c",
                "proj",
                &[1.0, 0.0],
                &empty_sparse(),
                10,
                3.0,
                true,
                &[],
                &["b.rs".to_string()],
            )
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].file_path, "a.rs");
    }

    /// Pins the `k`-widening in `query`: vec0 returns exactly `k` nearest by raw
    /// distance, so if the excluded chunk occupies one of the nearest `limit`
    /// slots, a naive fetch of exactly `limit` candidates would post-filter down
    /// to `limit - 1` even though enough non-excluded chunks exist to fill
    /// `limit`. Widening `k` when `exclude_paths` is non-empty is what prevents
    /// that under-return (sqlite_code_store.rs query, the `k` computation).
    #[tokio::test]
    async fn real_vec0_path_exclusion_does_not_starve_the_post_filter() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SqliteVecCodeStore::at(tmp.path().to_path_buf());

        // 5 chunks at strictly increasing L2 distance from the query vector
        // [1.0, 0.0]. "near.rs" is the single nearest neighbor.
        store
            .upsert_chunks(
                "c",
                &[
                    (
                        payload("p:near.rs:h0", "proj", "near.rs", "rust", "h0"),
                        embed(vec![1.0, 0.0]),
                    ),
                    (
                        payload("p:b.rs:h1", "proj", "b.rs", "rust", "h1"),
                        embed(vec![0.9, 0.1]),
                    ),
                    (
                        payload("p:c.rs:h2", "proj", "c.rs", "rust", "h2"),
                        embed(vec![0.8, 0.2]),
                    ),
                    (
                        payload("p:d.rs:h3", "proj", "d.rs", "rust", "h3"),
                        embed(vec![0.7, 0.3]),
                    ),
                    (
                        payload("p:e.rs:h4", "proj", "e.rs", "rust", "h4"),
                        embed(vec![0.6, 0.4]),
                    ),
                ],
            )
            .await
            .unwrap();

        // Excluding the nearest neighbor must not shrink the result below what's
        // actually available: 5 chunks total, 1 excluded, request 4 → 4 back.
        let hits = store
            .query(
                "c",
                "proj",
                &[1.0, 0.0],
                &empty_sparse(),
                4,
                3.0,
                true,
                &[],
                &["near.rs".to_string()],
            )
            .await
            .unwrap();

        assert!(
            hits.iter().all(|h| h.file_path != "near.rs"),
            "excluded path must not appear in results"
        );
        assert_eq!(
            hits.len(),
            4,
            "4 non-excluded chunks exist and `limit` is 4 — a naive top-`limit` \
         fetch (no k widening) would return only 3 because the excluded \
         chunk occupies the nearest slot"
        );
    }

    #[tokio::test]
    async fn delete_resolves_db_by_project_id_not_chunk_prefix() {
        // Regression: libraries use a colon-bearing project_id (`lib:foo`) and
        // chunk_id is `{project_id}:{rel}:{hash}`. delete_chunks must open the DB
        // for the FULL project_id (`lib_foo.db`), not the chunk_id's first
        // colon-delimited segment (`lib.db`), or it silently deletes nothing.
        let tmp = tempfile::tempdir().unwrap();
        let store = SqliteVecCodeStore::at(tmp.path().to_path_buf());
        let p = payload("lib:foo:a.rs:h1", "lib:foo", "a.rs", "rust", "h1");
        store
            .upsert_chunks("c", &[(p, embed(vec![1.0, 0.0]))])
            .await
            .unwrap();
        assert_eq!(
            store.project_index_stats("c", "lib:foo").await.unwrap(),
            (1, 1)
        );
        store
            .delete_chunks("c", "lib:foo", &["lib:foo:a.rs:h1".to_string()])
            .await
            .unwrap();
        assert_eq!(
            store.project_index_stats("c", "lib:foo").await.unwrap(),
            (0, 0)
        );
    }

    #[tokio::test]
    async fn real_vec0_reupsert_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SqliteVecCodeStore::at(tmp.path().to_path_buf());
        let p = payload("proj:a.rs:h1", "proj", "a.rs", "rust", "h1");
        store
            .upsert_chunks("c", &[(p.clone(), embed(vec![1.0, 0.0]))])
            .await
            .unwrap();
        store
            .upsert_chunks("c", &[(p, embed(vec![0.0, 1.0]))])
            .await
            .unwrap();
        // Re-upsert replaces, does not duplicate.
        assert_eq!(
            store.project_index_stats("c", "proj").await.unwrap(),
            (1, 1)
        );
    }

    #[tokio::test]
    async fn query_before_index_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SqliteVecCodeStore::at(tmp.path().to_path_buf());
        let hits = store
            .query(
                "c",
                "proj",
                &[1.0, 0.0],
                &empty_sparse(),
                10,
                3.0,
                true,
                &[],
                &[],
            )
            .await
            .unwrap();
        assert!(hits.is_empty());
    }
}
