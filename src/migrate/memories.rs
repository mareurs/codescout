//! Read legacy sqlite-vec memories and upsert them into Qdrant.
//!
//! ## Migration semantics
//!
//! - **Re-embed, don't copy.** Legacy embeddings used whatever local model was
//!   active at write time (typically fastembed bge-small, 384 dim). The new
//!   Qdrant `memories` collection is dimensioned to the HTTP embedder's output
//!   (e.g. CodeRankEmbed-Q4, 768 dim). Copying the legacy vector would either
//!   fail the dim check or produce semantically meaningless search results.
//! - **Idempotent.** Point ids in Qdrant are UUIDv5(project_id, bucket, title).
//!   Running migration twice overwrites the same point rather than duplicating.
//! - **Best-effort anchors.** `memory_anchors` rows are read by
//!   `(memory_type, memory_key) == (bucket, title)`. Missing or malformed
//!   anchor rows are skipped, never fatal.
//! - **Best-effort per memory.** A single memory's embed/upsert failure logs
//!   and increments `skipped`, but does not abort the run.
//!
//! ## On-disk layout
//!
//! The legacy database lives at `.codescout/embeddings/project.db` relative
//! to the project root (see `embed::index::project_db_path`). The CLI default
//! resolves this path; callers can override with `--db-path`.

use anyhow::{Context, Result};
use async_trait::async_trait;
use rusqlite::Connection;
use std::path::Path;

use crate::memory::semantic_store::SemanticMemoryStore;
use crate::retrieval::memory_payload::{MemoryAnchor, SemanticMemory};

/// Minimal embedder contract for migration. Production code wraps the
/// project's `EmbedderHttp` via [`HttpMigrationEmbedder`]; tests can supply
/// a fake without spinning up the retrieval stack.
#[async_trait]
pub trait MigrationEmbedder: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
}

/// Production embedder backed by the project's `EmbedderHttp`. Returns the
/// dense vector only — sparse is not stored on memory points.
pub struct HttpMigrationEmbedder {
    inner: std::sync::Arc<dyn crate::retrieval::embedder::CodeEmbedder>,
}

impl HttpMigrationEmbedder {
    pub fn new(inner: std::sync::Arc<dyn crate::retrieval::embedder::CodeEmbedder>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl MigrationEmbedder for HttpMigrationEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // `embed_document_one`, not `embed_one`: a migration re-embeds *stored*
        // content, so it must use the document side. `embed_one` is the query
        // seam and applies an asymmetric model's query prefix — routing a
        // migration through it re-creates the very defect a re-embed exists to
        // repair. docs/issues/archive/2026-08-11-memory-documents-stored-query-prefixed.md
        Ok(self.inner.embed_document_one(text).await?)
    }
}

/// Summary of a migration run, returned to the CLI for reporting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MigrationReport {
    /// Memory rows discovered in the source database.
    pub read: usize,
    /// Memories successfully upserted into Qdrant.
    pub upserted: usize,
    /// Memories that failed to embed or upsert. Logged at warn level.
    pub skipped: usize,
    /// Anchor rows attached to upserted memories.
    pub anchors_attached: usize,
    /// True if `dry_run` was requested — no writes were performed.
    pub dry_run: bool,
}

/// Run a memory migration end-to-end.
///
/// - `db_path` points at the legacy `embeddings.db`. If it does not exist the
///   function returns an empty report (no error) — re-running on a system that
///   has already been migrated and cleaned up is a no-op.
/// - `project_id` is the namespace stored in the new Qdrant points
///   (typically the active project's `config.project.name`).
/// - `dry_run` reads + counts but skips embedding and upsert. Use to preview.
pub async fn migrate_memories(
    db_path: &Path,
    store: &dyn SemanticMemoryStore,
    embedder: &dyn MigrationEmbedder,
    project_id: &str,
    dry_run: bool,
) -> Result<MigrationReport> {
    let mut report = MigrationReport {
        dry_run,
        ..Default::default()
    };

    if !db_path.exists() {
        tracing::info!(
            "migrate-memories: no legacy database at {} — nothing to migrate",
            db_path.display()
        );
        return Ok(report);
    }

    let conn = Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("open legacy db at {}", db_path.display()))?;

    let rows = read_legacy_memories(&conn).context("read legacy memories table")?;
    report.read = rows.len();

    for row in rows {
        let anchors = read_legacy_anchors(&conn, &row.bucket, &row.title).unwrap_or_else(|e| {
            tracing::warn!(
                "migrate-memories: anchor read failed for {}/{}: {e} (continuing without anchors)",
                row.bucket,
                row.title
            );
            Vec::new()
        });
        report.anchors_attached += anchors.len();

        if dry_run {
            report.upserted += 1;
            continue;
        }

        let dense = match embedder.embed(&row.content).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    "migrate-memories: embed failed for {}/{}: {e}",
                    row.bucket,
                    row.title
                );
                report.skipped += 1;
                continue;
            }
        };

        let mem = SemanticMemory {
            project_id: project_id.to_string(),
            bucket: row.bucket.clone(),
            title: row.title.clone(),
            content: row.content,
            anchors,
            created_at: row.created_at,
            updated_at: row.updated_at,
        };

        if let Err(e) = store.upsert(&mem, &dense).await {
            tracing::warn!(
                "migrate-memories: upsert failed for {}/{}: {e}",
                row.bucket,
                row.title
            );
            report.skipped += 1;
            continue;
        }
        report.upserted += 1;
    }

    Ok(report)
}

/// Re-embed every memory already in the store, in place, from the content the
/// store itself holds.
///
/// This is the repair half of
/// `docs/issues/archive/2026-08-11-memory-documents-stored-query-prefixed.md`. Fixing
/// the write path only fixes *future* writes; vectors already stored under the
/// old convention stay where they are, and once queries move to the corrected
/// convention those rows are stranded — a real mismatch where before there was
/// a harmless symmetric shift. Anything that changes embedding convention
/// (query-prefix policy, model, dimension) needs this, which is why it takes no
/// argument describing *what* changed: it just re-derives every vector from
/// current config.
///
/// Distinct from [`migrate_memories`], which reads a legacy sqlite db and writes
/// somewhere else. This reads and writes the same store.
///
/// **Safety properties**, each covered by a test below:
/// - **In-place, never duplicating.** The `SemanticMemory` is passed back
///   verbatim, so `point_id` (UUIDv5 over `project_id`/`bucket`/`title`) is
///   unchanged and the upsert overwrites its own row. Content, anchors and
///   timestamps are carried through untouched — only the vector changes.
/// - **Failure is non-destructive.** An embed or upsert error skips that row and
///   leaves its existing point intact, so a broken embedder cannot degrade the
///   corpus; it just does nothing. Counted in `skipped`.
/// - **Idempotent.** Re-running is a no-op beyond rewriting identical vectors.
///
/// A *partial* run does leave a mixed corpus (some rows re-embedded, some not) —
/// unavoidable without transactional writes across the store. The report
/// distinguishes the two, and re-running is safe.
///
/// Report fields in this mode: `read` = memories found, `upserted` = re-embedded
/// and written, `skipped` = embed-or-upsert failures. `anchors_attached` counts
/// anchors **carried through unchanged** — nothing is newly derived here, unlike
/// in [`migrate_memories`].
pub async fn reembed_memories_in_place(
    store: &dyn SemanticMemoryStore,
    embedder: &dyn MigrationEmbedder,
    project_id: &str,
    dry_run: bool,
) -> Result<MigrationReport> {
    let mut report = MigrationReport {
        dry_run,
        ..Default::default()
    };

    let hits = store
        .list(
            project_id,
            crate::memory::semantic_store::MemoryFilter::default(),
        )
        .await
        .with_context(|| format!("list memories for project {project_id}"))?;
    report.read = hits.len();

    for hit in hits {
        report.anchors_attached += hit.memory.anchors.len();

        if dry_run {
            report.upserted += 1;
            continue;
        }

        let dense = match embedder.embed(&hit.memory.content).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    "reembed-memories: embed failed for {}/{}: {e} (existing vector left in place)",
                    hit.memory.bucket,
                    hit.memory.title
                );
                report.skipped += 1;
                continue;
            }
        };

        // `&hit.memory` verbatim: same point_id, same content, same anchors and
        // timestamps. Only the vector is new.
        if let Err(e) = store.upsert(&hit.memory, &dense).await {
            tracing::warn!(
                "reembed-memories: upsert failed for {}/{}: {e} (existing vector left in place)",
                hit.memory.bucket,
                hit.memory.title
            );
            report.skipped += 1;
            continue;
        }
        report.upserted += 1;
    }

    Ok(report)
}

#[derive(Debug)]
struct LegacyRow {
    bucket: String,
    title: String,
    content: String,
    created_at: String,
    updated_at: String,
}

fn read_legacy_memories(conn: &Connection) -> Result<Vec<LegacyRow>> {
    let mut stmt = conn.prepare(
        "SELECT bucket, title, content, created_at, updated_at \
         FROM memories ORDER BY id ASC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(LegacyRow {
                bucket: r.get(0)?,
                title: r.get(1)?,
                content: r.get(2)?,
                created_at: r.get(3)?,
                updated_at: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn read_legacy_anchors(
    conn: &Connection,
    memory_type: &str,
    memory_key: &str,
) -> Result<Vec<MemoryAnchor>> {
    // memory_anchors is optional in older DBs — return empty if it doesn't
    // exist rather than failing the whole migration.
    let has_table: bool = conn.prepare("SELECT 1 FROM memory_anchors LIMIT 0").is_ok();
    if !has_table {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare(
        "SELECT file_path FROM memory_anchors \
         WHERE memory_type = ?1 AND memory_key = ?2 \
         ORDER BY file_path ASC",
    )?;
    let paths = stmt
        .query_map(rusqlite::params![memory_type, memory_key], |r| {
            r.get::<_, String>(0)
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(paths
        .into_iter()
        .map(|path| MemoryAnchor { path })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::semantic_store::test_support::InMemorySemanticMemoryStore;
    use crate::memory::semantic_store::{MemoryFilter, SemanticMemoryStore};
    use std::sync::Arc;
    use tempfile::tempdir;

    struct FixedEmbedder {
        vec: Vec<f32>,
    }

    #[async_trait]
    impl MigrationEmbedder for FixedEmbedder {
        async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
            Ok(self.vec.clone())
        }
    }

    struct ErrEmbedder;

    #[async_trait]
    impl MigrationEmbedder for ErrEmbedder {
        async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
            anyhow::bail!("embed offline")
        }
    }

    fn seed_legacy_db(path: &Path) -> Connection {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            "CREATE TABLE memories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                bucket TEXT NOT NULL DEFAULT 'unstructured',
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE memory_anchors (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                memory_type TEXT NOT NULL,
                memory_key TEXT NOT NULL,
                file_path TEXT NOT NULL,
                file_hash TEXT,
                similarity REAL,
                created_at TEXT,
                stale INTEGER NOT NULL DEFAULT 0,
                UNIQUE(memory_type, memory_key, file_path)
            );",
        )
        .unwrap();
        conn
    }

    #[tokio::test]
    async fn missing_db_returns_empty_report() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("does-not-exist.db");
        let store = InMemorySemanticMemoryStore::new();
        let emb = FixedEmbedder { vec: vec![0.0; 8] };
        let report = migrate_memories(&db, &store, &emb, "p", false)
            .await
            .unwrap();
        assert_eq!(report, MigrationReport::default());
    }

    #[tokio::test]
    async fn migrates_memories_and_anchors() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("legacy.db");
        let conn = seed_legacy_db(&db);
        conn.execute(
            "INSERT INTO memories (bucket, title, content, created_at, updated_at) \
             VALUES ('system', 'arch', 'architecture body', '2026-01-01', '2026-01-02')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO memories (bucket, title, content, created_at, updated_at) \
             VALUES ('preferences', 'rust-style', 'prefs body', '2026-02-01', '2026-02-02')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO memory_anchors \
               (memory_type, memory_key, file_path, file_hash, similarity, created_at, stale) \
             VALUES ('system', 'arch', 'src/lib.rs', 'h1', 0.9, '2026-01-02', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO memory_anchors \
               (memory_type, memory_key, file_path, file_hash, similarity, created_at, stale) \
             VALUES ('system', 'arch', 'src/agent/mod.rs', 'h2', 0.8, '2026-01-02', 0)",
            [],
        )
        .unwrap();
        drop(conn);

        let store: Arc<InMemorySemanticMemoryStore> = Arc::new(InMemorySemanticMemoryStore::new());
        let emb = FixedEmbedder { vec: vec![1.0; 8] };

        let report = migrate_memories(&db, store.as_ref(), &emb, "proj-x", false)
            .await
            .unwrap();
        assert_eq!(report.read, 2);
        assert_eq!(report.upserted, 2);
        assert_eq!(report.skipped, 0);
        assert_eq!(report.anchors_attached, 2);
        assert!(!report.dry_run);

        let hits = store.list("proj-x", MemoryFilter::default()).await.unwrap();
        assert_eq!(hits.len(), 2);
        let arch = hits.iter().find(|h| h.memory.title == "arch").unwrap();
        assert_eq!(arch.memory.bucket, "system");
        assert_eq!(arch.memory.anchors.len(), 2);
        assert_eq!(arch.memory.anchors[0].path, "src/agent/mod.rs");
    }

    #[tokio::test]
    async fn dry_run_does_not_write() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("legacy.db");
        let conn = seed_legacy_db(&db);
        conn.execute(
            "INSERT INTO memories (bucket, title, content, created_at, updated_at) \
             VALUES ('s', 't', 'c', '2026-01-01', '2026-01-01')",
            [],
        )
        .unwrap();
        drop(conn);

        let store = InMemorySemanticMemoryStore::new();
        let emb = ErrEmbedder; // would fail if invoked

        let report = migrate_memories(&db, &store, &emb, "p", true)
            .await
            .unwrap();
        assert_eq!(report.read, 1);
        assert_eq!(report.upserted, 1); // counted but not embedded
        assert_eq!(report.skipped, 0);
        assert!(report.dry_run);

        // Store must be empty — dry_run skips embed + upsert
        let hits = store.list("p", MemoryFilter::default()).await.unwrap();
        assert_eq!(hits.len(), 0);
    }

    #[tokio::test]
    async fn embed_failure_skips_row_without_aborting_run() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("legacy.db");
        let conn = seed_legacy_db(&db);
        for i in 0..3 {
            conn.execute(
                "INSERT INTO memories (bucket, title, content, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, '2026-01-01', '2026-01-01')",
                rusqlite::params!["s", format!("t{i}"), format!("c{i}")],
            )
            .unwrap();
        }
        drop(conn);

        let store = InMemorySemanticMemoryStore::new();
        let emb = ErrEmbedder;
        let report = migrate_memories(&db, &store, &emb, "p", false)
            .await
            .unwrap();
        assert_eq!(report.read, 3);
        assert_eq!(report.upserted, 0);
        assert_eq!(report.skipped, 3);
    }

    #[tokio::test]
    async fn migration_is_idempotent_via_deterministic_point_id() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("legacy.db");
        let conn = seed_legacy_db(&db);
        conn.execute(
            "INSERT INTO memories (bucket, title, content, created_at, updated_at) \
             VALUES ('s', 'only', 'c', '2026-01-01', '2026-01-01')",
            [],
        )
        .unwrap();
        drop(conn);

        let store = InMemorySemanticMemoryStore::new();
        let emb = FixedEmbedder { vec: vec![0.5; 8] };
        migrate_memories(&db, &store, &emb, "p", false)
            .await
            .unwrap();
        migrate_memories(&db, &store, &emb, "p", false)
            .await
            .unwrap();

        let hits = store.list("p", MemoryFilter::default()).await.unwrap();
        assert_eq!(hits.len(), 1, "second run must overwrite, not duplicate");
    }

    /// Seed one memory whose stored vector is `seed`, then re-embed in place with
    /// an embedder returning `fresh`.
    async fn seeded_store(
        seed: Vec<f32>,
    ) -> (
        Arc<InMemorySemanticMemoryStore>,
        crate::retrieval::memory_payload::SemanticMemory,
    ) {
        let store: Arc<InMemorySemanticMemoryStore> = Arc::new(InMemorySemanticMemoryStore::new());
        let mem = crate::retrieval::memory_payload::SemanticMemory {
            project_id: "p".into(),
            bucket: "structured".into(),
            title: "t".into(),
            content: "the stored content".into(),
            anchors: vec![MemoryAnchor {
                path: "src/lib.rs".into(),
            }],
            created_at: "2026-01-01".into(),
            updated_at: "2026-01-01".into(),
        };
        store.upsert(&mem, &seed).await.unwrap();
        (store, mem)
    }

    /// The vector is replaced; the identity and payload are not. Asserted through
    /// `search` rather than by reaching into the store: a query equal to the fresh
    /// vector must score ~1.0, and one equal to the (orthogonal) seed ~0.0.
    /// docs/issues/archive/2026-08-11-memory-documents-stored-query-prefixed.md
    #[tokio::test]
    async fn reembed_in_place_replaces_the_vector_and_preserves_everything_else() {
        let seed = vec![1.0_f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let fresh = vec![0.0_f32, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let (store, mem) = seeded_store(seed.clone()).await;

        let report = reembed_memories_in_place(
            store.as_ref(),
            &FixedEmbedder { vec: fresh.clone() },
            "p",
            false,
        )
        .await
        .unwrap();
        assert_eq!(report.read, 1);
        assert_eq!(report.upserted, 1);
        assert_eq!(report.skipped, 0);
        assert_eq!(report.anchors_attached, 1, "anchors carried through");

        let all = store.list("p", MemoryFilter::default()).await.unwrap();
        assert_eq!(all.len(), 1, "in place: must not duplicate the row");
        assert_eq!(all[0].id, mem.point_id(), "same deterministic point id");
        assert_eq!(all[0].memory.content, "the stored content");
        assert_eq!(all[0].memory.anchors.len(), 1);
        assert_eq!(all[0].memory.created_at, "2026-01-01");

        let by_fresh = store.search("p", &fresh, 1, None).await.unwrap();
        assert!(
            by_fresh[0].score.unwrap() > 0.99,
            "stored vector must now be the fresh one, got score {:?}",
            by_fresh[0].score
        );
        let by_seed = store.search("p", &seed, 1, None).await.unwrap();
        assert!(
            by_seed[0].score.unwrap() < 0.01,
            "the seed vector must no longer be what is stored, got score {:?}",
            by_seed[0].score
        );
    }

    /// A failing embedder must not degrade the corpus: the row keeps the vector it
    /// had. This is what makes the command safe to run against a live store with a
    /// possibly-misconfigured embedder.
    #[tokio::test]
    async fn reembed_in_place_leaves_the_existing_vector_when_embedding_fails() {
        let seed = vec![1.0_f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let (store, _mem) = seeded_store(seed.clone()).await;

        let report = reembed_memories_in_place(store.as_ref(), &ErrEmbedder, "p", false)
            .await
            .unwrap();
        assert_eq!(report.read, 1);
        assert_eq!(report.upserted, 0);
        assert_eq!(report.skipped, 1);

        let by_seed = store.search("p", &seed, 1, None).await.unwrap();
        assert!(
            by_seed[0].score.unwrap() > 0.99,
            "the original vector must survive a failed re-embed"
        );
    }

    /// Dry run counts without embedding or writing — `ErrEmbedder` would error if
    /// invoked, so a passing run proves the embedder was never called.
    #[tokio::test]
    async fn reembed_in_place_dry_run_neither_embeds_nor_writes() {
        let seed = vec![1.0_f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let (store, _mem) = seeded_store(seed.clone()).await;

        let report = reembed_memories_in_place(store.as_ref(), &ErrEmbedder, "p", true)
            .await
            .unwrap();
        assert_eq!(report.read, 1);
        assert_eq!(report.upserted, 1, "counted");
        assert_eq!(report.skipped, 0);
        assert!(report.dry_run);

        let by_seed = store.search("p", &seed, 1, None).await.unwrap();
        assert!(
            by_seed[0].score.unwrap() > 0.99,
            "dry run must not have rewritten the vector"
        );
    }

    /// Only the requested project is touched — the command is per-project, and a
    /// shared collection holds several.
    #[tokio::test]
    async fn reembed_in_place_ignores_other_projects() {
        let seed = vec![1.0_f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let fresh = vec![0.0_f32, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let (store, mem) = seeded_store(seed.clone()).await;

        let mut other = mem.clone();
        other.project_id = "other".into();
        store.upsert(&other, &seed).await.unwrap();

        let report =
            reembed_memories_in_place(store.as_ref(), &FixedEmbedder { vec: fresh }, "p", false)
                .await
                .unwrap();
        assert_eq!(report.read, 1, "only project p's memory is in scope");

        let other_hits = store.search("other", &seed, 1, None).await.unwrap();
        assert!(
            other_hits[0].score.unwrap() > 0.99,
            "the other project's vector must be untouched"
        );
    }

    /// An empty store is a clean no-op, not an error — matches
    /// `missing_db_returns_empty_report` for the legacy path.
    #[tokio::test]
    async fn reembed_in_place_on_an_empty_store_is_a_no_op() {
        let store = InMemorySemanticMemoryStore::new();
        let report = reembed_memories_in_place(&store, &ErrEmbedder, "p", false)
            .await
            .unwrap();
        assert_eq!(report, MigrationReport::default());
    }
}
