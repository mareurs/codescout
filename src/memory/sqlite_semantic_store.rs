//! In-process, daemon-free [`SemanticMemoryStore`] backed by **sqlite-vec**.
//!
//! The memory-recall counterpart to `retrieval::sqlite_code_store` — the lite
//! stack's memory backend (see `docs/plans/2026-06-16-two-stack-retrieval-lite.md`).
//! One SQLite file per project under the shared data dir; the full
//! [`SemanticMemory`] is stored as JSON in `memory_item`, its dense vector in a
//! `vec0` `memory_vec` table. Dense-only ranking, no daemon, EDR-safe (statically
//! linked `vec0`, no foreign DLL).

use crate::memory::semantic_store::{MemoryFilter, MemoryOrder, SemanticMemoryStore};
use crate::retrieval::memory::MemoryHit;
use crate::retrieval::memory_payload::SemanticMemory;
use crate::sqlite_vec_ext::{dense_blob, sanitize_db_name};
use anyhow::{Context, Result};
use async_trait::async_trait;
use parking_lot::Mutex;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

pub struct SqliteVecSemanticMemoryStore {
    dir: PathBuf,
    conns: Mutex<HashMap<String, Arc<Mutex<Connection>>>>,
}

impl SqliteVecSemanticMemoryStore {
    /// Resolve the data dir from the environment (shared with the code store).
    pub fn from_env() -> Result<Self> {
        let dir = match std::env::var("CODESCOUT_SQLITE_DIR")
            .ok()
            .filter(|s| !s.is_empty())
        {
            Some(d) => PathBuf::from(d),
            None => crate::platform::home_dir()
                .context(
                    "cannot resolve home dir for sqlite-vec memory store; set CODESCOUT_SQLITE_DIR",
                )?
                .join(".codescout")
                .join("embeddings"),
        };
        Ok(Self::at(dir))
    }

    pub fn at(dir: PathBuf) -> Self {
        Self {
            dir,
            conns: Mutex::new(HashMap::new()),
        }
    }

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
            .join(format!("{}.memories.db", sanitize_db_name(project_id)));
        let conn = Connection::open(&path)
            .with_context(|| format!("open sqlite-vec memory db {}", path.display()))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS memory_item (
                 point_id   TEXT PRIMARY KEY,
                 project_id TEXT NOT NULL,
                 bucket     TEXT NOT NULL,
                 updated_at TEXT NOT NULL,
                 json       TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_memory_project ON memory_item(project_id);",
        )
        .context("create memory_item table")?;
        let arc = Arc::new(Mutex::new(conn));
        cache.insert(project_id.to_string(), Arc::clone(&arc));
        Ok(arc)
    }

    fn ensure_vec_table(conn: &Connection, dim: usize) -> Result<()> {
        use rusqlite::OptionalExtension;
        let existing: Option<i64> = conn
            .query_row(
                "SELECT length(embedding) FROM memory_vec LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()
            .unwrap_or(None);
        if let Some(blob_len) = existing {
            let existing_dim = (blob_len / 4) as usize;
            if existing_dim != dim {
                anyhow::bail!(
                    "sqlite-vec memory index dim mismatch: existing={existing_dim}, batch={dim}. \
                     The embedding model/dim changed — clear the memory index to rebuild."
                );
            }
            return Ok(());
        }
        conn.execute_batch(&format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS memory_vec USING vec0(
                 point_id TEXT PRIMARY KEY,
                 embedding FLOAT[{dim}]
             );"
        ))
        .context("create memory_vec table")?;
        Ok(())
    }

    fn vec_table_present(conn: &Connection) -> bool {
        conn.query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='memory_vec'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false)
    }
}

#[async_trait]
impl SemanticMemoryStore for SqliteVecSemanticMemoryStore {
    async fn upsert(&self, m: &SemanticMemory, dense: &[f32]) -> Result<()> {
        let dim = dense.len();
        if dim == 0 {
            anyhow::bail!(
                "sqlite-vec memory upsert: embedding dim is 0 (embedder error sentinel?)"
            );
        }
        let conn = self.conn_for(&m.project_id)?;
        let mut conn = conn.lock();
        Self::ensure_vec_table(&conn, dim)?;
        let pid = m.point_id().to_string();
        let json = serde_json::to_string(m).context("serialize SemanticMemory")?;
        let tx = conn.transaction()?;
        // vec0 ignores INSERT OR REPLACE — DELETE then INSERT for idempotency.
        tx.execute(
            "DELETE FROM memory_item WHERE point_id = ?1",
            rusqlite::params![pid],
        )?;
        tx.execute(
            "DELETE FROM memory_vec WHERE point_id = ?1",
            rusqlite::params![pid],
        )?;
        tx.execute(
            "INSERT INTO memory_item (point_id, project_id, bucket, updated_at, json)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![pid, m.project_id, m.bucket, m.updated_at, json],
        )?;
        tx.execute(
            "INSERT INTO memory_vec (point_id, embedding) VALUES (?1, ?2)",
            rusqlite::params![pid, dense_blob(dense)],
        )?;
        tx.commit()?;
        Ok(())
    }

    async fn search(
        &self,
        project_id: &str,
        query: &[f32],
        top_n: usize,
        bucket: Option<&str>,
    ) -> Result<Vec<MemoryHit>> {
        let conn = self.conn_for(project_id)?;
        let conn = conn.lock();
        if !Self::vec_table_present(&conn) {
            return Ok(Vec::new());
        }
        let blob = dense_blob(query);
        // (distance, point_id, json) — bucket filter folded into the SQL. Each
        // branch binds the collected rows to a local so `stmt` outlives the query.
        let raw: Vec<(f64, String, String)> = if let Some(b) = bucket {
            let mut stmt = conn.prepare(
                "SELECT v.distance, m.point_id, m.json
                   FROM memory_vec v JOIN memory_item m ON m.point_id = v.point_id
                  WHERE v.embedding MATCH vec_f32(?1) AND k = ?2 AND m.project_id = ?3 AND m.bucket = ?4
                  ORDER BY v.distance",
            )?;
            let rows = stmt
                .query_map(rusqlite::params![blob, top_n as i64, project_id, b], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        } else {
            let mut stmt = conn.prepare(
                "SELECT v.distance, m.point_id, m.json
                   FROM memory_vec v JOIN memory_item m ON m.point_id = v.point_id
                  WHERE v.embedding MATCH vec_f32(?1) AND k = ?2 AND m.project_id = ?3
                  ORDER BY v.distance",
            )?;
            let rows = stmt
                .query_map(rusqlite::params![blob, top_n as i64, project_id], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        Ok(raw
            .into_iter()
            .filter_map(|(dist, pid, json)| {
                let memory: SemanticMemory = serde_json::from_str(&json).ok()?;
                let id = Uuid::parse_str(&pid).ok()?;
                Some(MemoryHit {
                    id,
                    memory,
                    score: Some(1.0 / (1.0 + dist as f32)),
                })
            })
            .collect())
    }

    async fn delete(&self, project_id: &str, id: Uuid) -> Result<()> {
        let conn = self.conn_for(project_id)?;
        let conn = conn.lock();
        let pid = id.to_string();
        conn.execute(
            "DELETE FROM memory_item WHERE point_id = ?1",
            rusqlite::params![pid],
        )?;
        if Self::vec_table_present(&conn) {
            conn.execute(
                "DELETE FROM memory_vec WHERE point_id = ?1",
                rusqlite::params![pid],
            )?;
        }
        Ok(())
    }

    async fn list(&self, project_id: &str, filter: MemoryFilter) -> Result<Vec<MemoryHit>> {
        let conn = self.conn_for(project_id)?;
        let conn = conn.lock();
        let rows: Vec<(String, String)> = if let Some(b) = filter.bucket.as_deref() {
            let mut stmt = conn.prepare(
                "SELECT point_id, json FROM memory_item WHERE project_id = ?1 AND bucket = ?2",
            )?;
            let rows = stmt
                .query_map(rusqlite::params![project_id, b], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        } else {
            let mut stmt =
                conn.prepare("SELECT point_id, json FROM memory_item WHERE project_id = ?1")?;
            let rows = stmt
                .query_map(rusqlite::params![project_id], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        let mut hits: Vec<MemoryHit> = rows
            .into_iter()
            .filter_map(|(pid, json)| {
                let memory: SemanticMemory = serde_json::from_str(&json).ok()?;
                let id = Uuid::parse_str(&pid).ok()?;
                Some(MemoryHit {
                    id,
                    memory,
                    score: None,
                })
            })
            // anchor_path lives inside the JSON payload — filter in Rust (mirrors
            // the in-memory + Qdrant stores).
            .filter(|h| {
                filter
                    .anchor_path
                    .as_deref()
                    .is_none_or(|p| h.memory.anchors.iter().any(|a| a.path == p))
            })
            .collect();
        if filter.order_by == MemoryOrder::UpdatedAtDesc {
            hits.sort_by(|a, b| b.memory.updated_at.cmp(&a.memory.updated_at));
        }
        if let Some(n) = filter.limit {
            hits.truncate(n);
        }
        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::memory_payload::MemoryAnchor;

    fn mem(project: &str, bucket: &str, title: &str, anchor: Option<&str>) -> SemanticMemory {
        SemanticMemory {
            project_id: project.into(),
            bucket: bucket.into(),
            title: title.into(),
            content: format!("content of {title}"),
            anchors: anchor
                .map(|p| vec![MemoryAnchor { path: p.into() }])
                .unwrap_or_default(),
            created_at: "2026-06-16T00:00:00Z".into(),
            updated_at: format!("2026-06-16T00:00:0{}Z", title.len() % 10),
        }
    }

    #[tokio::test]
    async fn real_vec0_memory_upsert_search_orders_by_distance() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SqliteVecSemanticMemoryStore::at(tmp.path().to_path_buf());
        store
            .upsert(&mem("p", "notes", "a", None), &[1.0, 0.0])
            .await
            .unwrap();
        store
            .upsert(&mem("p", "notes", "b", None), &[0.0, 1.0])
            .await
            .unwrap();

        let hits = store.search("p", &[0.9, 0.1], 10, None).await.unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].memory.title, "a", "nearest vector ranks first");
        assert!(hits[0].score.is_some());
    }

    #[tokio::test]
    async fn real_vec0_memory_bucket_filter_delete_and_list() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SqliteVecSemanticMemoryStore::at(tmp.path().to_path_buf());
        store
            .upsert(&mem("p", "notes", "n1", Some("src/a.rs")), &[1.0, 0.0])
            .await
            .unwrap();
        store
            .upsert(&mem("p", "prefs", "p1", None), &[1.0, 0.0])
            .await
            .unwrap();

        // search scoped to a bucket
        let hits = store
            .search("p", &[1.0, 0.0], 10, Some("notes"))
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].memory.bucket, "notes");

        // list with anchor_path filter
        let listed = store
            .list(
                "p",
                MemoryFilter {
                    anchor_path: Some("src/a.rs".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].memory.title, "n1");

        // delete by id
        let id = mem("p", "notes", "n1", None).point_id();
        store.delete("p", id).await.unwrap();
        let all = store.list("p", MemoryFilter::default()).await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].memory.bucket, "prefs");
    }

    #[tokio::test]
    async fn memory_search_before_upsert_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SqliteVecSemanticMemoryStore::at(tmp.path().to_path_buf());
        let hits = store.search("p", &[1.0, 0.0], 10, None).await.unwrap();
        assert!(hits.is_empty());
    }
}
