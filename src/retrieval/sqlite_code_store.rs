//! In-process, daemon-free [`CodeVectorStore`] backed by **sqlite-vec** (`vec0`).
//!
//! This is the "lite" stack's code-search backend (see
//! `docs/plans/2026-06-16-two-stack-retrieval-lite.md`): no Qdrant, no Docker —
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
//! One DB per project id under a data dir (`$CODESCOUT_SQLITE_DIR`, else
//! `<home>/.codescout/embeddings/`). Tables are created lazily; the `vec0`
//! dimension is inferred from the first batch of embeddings (so a remote model's
//! native dim is adopted automatically). A dimension change requires a reindex —
//! same caveat as switching Qdrant embedders (WIN-22).

use crate::retrieval::code_store::CodeVectorStore;
use crate::retrieval::drift::ChunkRef;
use crate::retrieval::embedder::{EmbedOutput, SparseVector};
use crate::retrieval::payload::CodePayload;
use crate::retrieval::search::Hit;
use crate::sqlite_vec_ext::{dense_blob, sanitize_db_name};
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
    /// Resolve the data dir from the environment and construct an empty store.
    /// No I/O until the first per-project operation.
    pub fn from_env() -> Result<Self> {
        let dir = match std::env::var("CODESCOUT_SQLITE_DIR")
            .ok()
            .filter(|s| !s.is_empty())
        {
            Some(d) => PathBuf::from(d),
            None => crate::platform::home_dir()
                .context("cannot resolve home dir for sqlite-vec store; set CODESCOUT_SQLITE_DIR")?
                .join(".codescout")
                .join("embeddings"),
        };
        Ok(Self::at(dir))
    }

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
        let mut cache = self.conns.lock();
        if let Some(c) = cache.get(project_id) {
            return Ok(Arc::clone(c));
        }
        crate::sqlite_vec_ext::register();
        std::fs::create_dir_all(&self.dir)
            .with_context(|| format!("create sqlite-vec dir {}", self.dir.display()))?;
        let path = self
            .dir
            .join(format!("{}.db", sanitize_db_name(project_id)));
        let conn = Connection::open(&path)
            .with_context(|| format!("open sqlite-vec db {}", path.display()))?;
        conn.execute_batch(
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
        .context("create code_chunk table")?;
        let arc = Arc::new(Mutex::new(conn));
        cache.insert(project_id.to_string(), Arc::clone(&arc));
        Ok(arc)
    }

    /// Ensure the `code_vec` virtual table exists with the given dim. Validates
    /// against the existing dim (reindex required on a mismatch).
    fn ensure_vec_table(conn: &Connection, dim: usize) -> Result<()> {
        use rusqlite::OptionalExtension;
        let existing: Option<i64> = conn
            .query_row("SELECT length(embedding) FROM code_vec LIMIT 1", [], |r| {
                r.get(0)
            })
            .optional()
            .unwrap_or(None);
        if let Some(blob_len) = existing {
            let existing_dim = (blob_len / 4) as usize;
            if existing_dim != dim {
                anyhow::bail!(
                    "sqlite-vec code index dim mismatch: existing={existing_dim}, batch={dim}. \
                     The embedding model/dim changed — reindex with force=true to rebuild."
                );
            }
            return Ok(());
        }
        // FLOAT[N] requires the dim as a literal at CREATE time.
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
        let mut stmt =
            conn.prepare("SELECT chunk_id, content_hash FROM code_chunk WHERE project_id = ?1")?;
        let rows = stmt
            .query_map(rusqlite::params![project_id], |row| {
                Ok(ChunkRef {
                    chunk_id: row.get(0)?,
                    content_hash: row.get(1)?,
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

    async fn delete_chunks(&self, _collection: &str, ids: &[String]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        // The DB is keyed per project; ids carry their project prefix, so deleting
        // by id across the project's DB is safe. Open via the first id's project.
        let project_id = ids
            .iter()
            .find_map(|id| id.split(':').next())
            .unwrap_or("default")
            .to_string();
        let conn = self.conn_for(&project_id)?;
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
        // KNN on the dense leg, then hydrate payload + filter language. We fetch
        // `limit` (the caller's overfetch) and let exclude_languages trim — the
        // caller applies the final top-k.
        let mut stmt = conn.prepare(
            "SELECT v.distance, c.chunk_id, c.file_path, c.language, c.start_line, c.end_line, c.content
               FROM code_vec v JOIN code_chunk c ON c.chunk_id = v.chunk_id
              WHERE v.embedding MATCH vec_f32(?1) AND k = ?3 AND c.project_id = ?2
              ORDER BY v.distance",
        )?;
        let rows = stmt
            .query_map(
                rusqlite::params![dense_blob(dense), project_id, limit as i64],
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
            .filter(|(_, _, lang)| !exclude_languages.contains(lang))
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
            ast_kind: String::new(),
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

        let refs = store.chunk_refs("c", "proj").await.unwrap();
        assert_eq!(refs.len(), 2);

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
            )
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].file_path, "a.rs");

        store
            .delete_chunks("c", &["proj:a.rs:h1".to_string()])
            .await
            .unwrap();
        assert_eq!(
            store.project_index_stats("c", "proj").await.unwrap(),
            (1, 1)
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
            )
            .await
            .unwrap();
        assert!(hits.is_empty());
    }
}
