//! Lossless, explicit maintenance of one SQLite memory vector index.
use super::memories::MigrationEmbedder;
use anyhow::Result;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct RebuildOptions {
    pub dimensions: usize,
    pub backup: PathBuf,
    pub dry_run: bool,
}

#[derive(Debug, Serialize)]
pub struct SqliteVectorRebuildReport {
    pub records: usize,
    pub previous_dimensions: Option<usize>,
    pub requested_dimensions: usize,
    pub published_dimensions: Option<usize>,
    pub vectors_written: usize,
    pub dry_run: bool,
    pub backup: Option<PathBuf>,
}

pub async fn rebuild_sqlite_memory_vectors(
    db: &Path,
    embedder: Option<&dyn MigrationEmbedder>,
    options: &RebuildOptions,
) -> Result<SqliteVectorRebuildReport> {
    use anyhow::{ensure, Context};
    use rusqlite::{Connection, OpenFlags};

    ensure!(
        options.dimensions > 0,
        "requested dimensions must be positive"
    );
    ensure!(
        !options.backup.exists(),
        "backup destination already exists: {}",
        options.backup.display()
    );
    let db = db
        .canonicalize()
        .with_context(|| format!("existing memory database {}", db.display()))?;
    ensure!(
        db.is_file(),
        "memory database must be an existing regular file"
    );
    crate::sqlite_vec_ext::register();
    let flags = if options.dry_run {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    };
    let mut conn = Connection::open_with_flags(&db, flags)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    let source_version = data_version(&conn)?;
    let previous_dimensions = vector_dimensions(&conn)?;
    let snapshot = read_snapshot(&conn)?;
    let records = snapshot.ids.len();
    let mut report = SqliteVectorRebuildReport {
        records,
        previous_dimensions,
        requested_dimensions: options.dimensions,
        published_dimensions: None,
        vectors_written: 0,
        dry_run: options.dry_run,
        backup: None,
    };
    if options.dry_run {
        return Ok(report);
    }
    let journal: String = conn.pragma_query_value(None, "journal_mode", |r| r.get(0))?;
    ensure!(!journal.eq_ignore_ascii_case("off"), "journal_mode=OFF cannot guarantee transaction rollback; use a coherent copy with journaling enabled");
    let embedder = embedder.context("an explicit document embedder is required")?;
    let mut replacements = Vec::with_capacity(records);
    for (id, content) in snapshot.ids.iter().zip(&snapshot.contents) {
        let vector = embedder
            .embed(content)
            .await
            .with_context(|| format!("embedding memory {id}; original index retained"))?;
        ensure!(
            vector.len() == options.dimensions,
            "memory {id}: expected {} dimensions, received {}; original index retained",
            options.dimensions,
            vector.len()
        );
        ensure!(
            vector.iter().all(|v| v.is_finite()),
            "memory {id}: embedding contains non-finite values; original index retained"
        );
        replacements.push((id.clone(), vector));
    }
    // A completed backup is required before any index DDL. Failed reserved files
    // remain for diagnosis: retry always chooses a fresh backup destination.
    verified_backup(&conn, &options.backup, &snapshot)?;
    report.vectors_written = publish_vectors(
        &mut conn,
        source_version,
        &snapshot,
        &replacements,
        options.dimensions,
    )
    .with_context(|| {
        format!(
            "publication refused or rolled back; backup retained at {}",
            options.backup.display()
        )
    })?;
    report.published_dimensions = Some(options.dimensions);
    report.backup = Some(options.backup.clone());
    Ok(report)
}

#[derive(Debug, PartialEq)]
struct Snapshot {
    columns: Vec<String>,
    rows: Vec<Vec<rusqlite::types::Value>>,
    ids: Vec<String>,
    contents: Vec<String>,
}

fn data_version(conn: &rusqlite::Connection) -> Result<i64> {
    Ok(conn.pragma_query_value(None, "data_version", |r| r.get(0))?)
}

/// Validate the baked vec0 schema, including an empty index. Blob lengths alone
/// cannot reveal the dimensions of an empty virtual table.
fn vector_dimensions(conn: &rusqlite::Connection) -> Result<Option<usize>> {
    use anyhow::{ensure, Context};
    use rusqlite::OptionalExtension;
    let schema: Option<(String, String)> = conn
        .query_row(
            "SELECT type, coalesce(sql, '') FROM sqlite_master WHERE name = 'memory_vec'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((kind, sql)) = schema else {
        return Ok(None);
    };
    ensure!(kind == "table", "memory_vec is not a supported vec0 table");
    static SCHEMA: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(?i)^\s*CREATE\s+VIRTUAL\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?memory_vec\s+USING\s+vec0\s*\(\s*point_id\s+TEXT\s+PRIMARY\s+KEY\s*,\s*embedding\s+FLOAT\s*\[\s*([0-9]+)\s*\]\s*\)\s*;?\s*$").expect("fixed vec0 schema regex")
    });
    let captures = SCHEMA.captures(&sql).context("unsupported memory_vec schema; expected point_id TEXT PRIMARY KEY, embedding FLOAT[N] only")?;
    let dimensions: usize = captures[1].parse()?;
    ensure!(
        dimensions > 0,
        "existing vector dimensions must be positive"
    );
    Ok(Some(dimensions))
}

fn read_snapshot(conn: &rusqlite::Connection) -> Result<Snapshot> {
    use anyhow::{ensure, Context};
    use rusqlite::types::Value;
    let kind: String = conn.query_row(
        "SELECT type FROM sqlite_master WHERE name = 'memory_item'", [], |r| r.get(0)
    ).context("missing memory_item table; this command accepts an existing SQLite semantic-memory database")?;
    ensure!(kind == "table", "memory_item must be a table");
    let mut stmt = conn.prepare("SELECT * FROM memory_item ORDER BY point_id")?;
    let columns: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    for required in ["point_id", "project_id", "bucket", "updated_at", "json"] {
        ensure!(
            columns.iter().any(|c| c == required),
            "memory_item lacks required column {required}"
        );
    }
    let id_column = columns
        .iter()
        .position(|c| c == "point_id")
        .expect("validated");
    let json_column = columns.iter().position(|c| c == "json").expect("validated");
    let width = columns.len();
    let rows: Vec<Vec<Value>> = stmt
        .query_map([], |r| (0..width).map(|n| r.get(n)).collect())?
        .collect::<rusqlite::Result<_>>()?;
    let mut ids = Vec::with_capacity(rows.len());
    let mut contents = Vec::with_capacity(rows.len());
    let mut seen = std::collections::HashSet::new();
    for row in &rows {
        let Value::Text(id) = &row[id_column] else {
            anyhow::bail!("memory_item point_id must be text");
        };
        uuid::Uuid::parse_str(id)
            .with_context(|| format!("invalid literal memory point_id {id:?}"))?;
        ensure!(
            seen.insert(id.clone()),
            "duplicate literal memory point_id {id}"
        );
        let Value::Text(json) = &row[json_column] else {
            anyhow::bail!("memory {id}: json must be text");
        };
        let memory: crate::retrieval::memory_payload::SemanticMemory = serde_json::from_str(json)
            .with_context(|| {
            format!("memory {id}: invalid SemanticMemory payload; no rows may be skipped")
        })?;
        ids.push(id.clone());
        contents.push(memory.content);
    }
    Ok(Snapshot {
        columns,
        rows,
        ids,
        contents,
    })
}

fn verified_backup(
    conn: &rusqlite::Connection,
    destination: &Path,
    expected: &Snapshot,
) -> Result<()> {
    use anyhow::{ensure, Context};
    use rusqlite::{Connection, OpenFlags};
    let mut open = std::fs::OpenOptions::new();
    open.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        open.mode(0o600);
    }
    let reserved = open
        .open(destination)
        .with_context(|| format!("reserve fresh backup {}", destination.display()))?;
    drop(reserved);
    let path = destination
        .to_str()
        .context("backup path must be UTF-8 for SQLite VACUUM INTO")?;
    conn.execute("VACUUM INTO ?1", [path]).with_context(|| {
        format!(
            "backup failed; reserved file {} may be incomplete; use a fresh destination on retry",
            destination.display()
        )
    })?;
    std::fs::File::open(destination)?.sync_all()?;
    let backup = Connection::open_with_flags(destination, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let integrity: String = backup.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    ensure!(integrity == "ok", "backup integrity failed: {integrity}");
    ensure!(
        &read_snapshot(&backup)? == expected,
        "source changed while preparing backup; original index retained"
    );
    Ok(())
}

fn publish_vectors(
    conn: &mut rusqlite::Connection,
    expected_version: i64,
    expected: &Snapshot,
    replacements: &[(String, Vec<f32>)],
    dimensions: usize,
) -> Result<usize> {
    use anyhow::{ensure, Context};
    use rusqlite::{params, TransactionBehavior};
    let expected_bytes = dimensions
        .checked_mul(4)
        .and_then(|n| i64::try_from(n).ok())
        .context("requested vector dimensions exceed SQLite blob limits")?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    ensure!(data_version(&tx)? == expected_version && &read_snapshot(&tx)? == expected,
        "source changed during embedding; stop all writers and retry with a fresh backup destination");
    tx.execute_batch(&format!(
        "DROP TABLE IF EXISTS memory_vec;
         CREATE VIRTUAL TABLE memory_vec USING vec0(
             point_id TEXT PRIMARY KEY, embedding FLOAT[{dimensions}]);"
    ))?;
    let mut inserted = 0;
    for (id, vector) in replacements {
        inserted += tx.execute(
            "INSERT INTO memory_vec(point_id, embedding) VALUES (?1, ?2)",
            params![id, crate::sqlite_vec_ext::dense_blob(vector)],
        )?;
    }
    let mut stmt =
        tx.prepare("SELECT point_id, length(embedding) FROM memory_vec ORDER BY point_id")?;
    let observed: Vec<(String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    ensure!(
        inserted == expected.ids.len() && observed.len() == expected.ids.len(),
        "replacement vector count does not cover every memory record"
    );
    ensure!(
        observed
            .iter()
            .zip(&expected.ids)
            .all(|((id, bytes), expected_id)| id == expected_id && *bytes == expected_bytes),
        "replacement index has missing/orphan IDs or incorrect vector dimensions"
    );
    ensure!(
        &read_snapshot(&tx)? == expected,
        "memory records changed during publication"
    );
    drop(stmt);
    tx.commit()?;
    Ok(inserted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::semantic_store::SemanticMemoryStore;
    use crate::memory::sqlite_semantic_store::SqliteVecSemanticMemoryStore;
    use async_trait::async_trait;
    use rusqlite::{params, types::Value, Connection};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fixture {
        dir: tempfile::TempDir,
        db: PathBuf,
    }

    impl Fixture {
        fn new(count: usize, dimensions: usize) -> Self {
            crate::sqlite_vec_ext::register();
            let dir = tempfile::tempdir().unwrap();
            let db = dir.path().join("fixture.memories.db");
            let conn = Connection::open(&db).unwrap();
            conn.execute_batch(&format!(
                "CREATE TABLE memory_item (
                    point_id TEXT PRIMARY KEY, project_id TEXT NOT NULL,
                    bucket TEXT NOT NULL, updated_at TEXT NOT NULL,
                    json TEXT NOT NULL, future BLOB);
                 CREATE VIRTUAL TABLE memory_vec USING vec0(
                    point_id TEXT PRIMARY KEY, embedding FLOAT[{dimensions}]);"
            ))
            .unwrap();
            for n in 0..count {
                let id = uuid::Uuid::from_u128(100 + n as u128).to_string();
                let project = if n == 191 { "other-project" } else { "fixture" };
                let payload = serde_json::json!({
                    "project_id": project, "bucket": if n % 2 == 0 { "structured" } else { "code" },
                    "title": format!("title-{n}"), "content": format!("row-{n}"),
                    "anchors": [{"path": format!("src/original-{n}.rs")}],
                    "created_at":"2026-01-01T00:00:00Z", "updated_at":"2026-02-01T00:00:00Z",
                    "unknown": {"must_survive": [n, "literal"], "extra":true}
                });
                let json = format!(" \n{}\n ", serde_json::to_string_pretty(&payload).unwrap());
                conn.execute(
                    "INSERT INTO memory_item VALUES (?1,?2,?3,?4,?5,?6)",
                    params![
                        id,
                        project,
                        payload["bucket"].as_str().unwrap(),
                        "raw-sql-timestamp",
                        json,
                        vec![n as u8, 0, 255]
                    ],
                )
                .unwrap();
                let mut vector = vec![0.0; dimensions];
                vector[dimensions - 1] = 1.0;
                conn.execute(
                    "INSERT INTO memory_vec VALUES (?1,?2)",
                    params![id, crate::sqlite_vec_ext::dense_blob(&vector)],
                )
                .unwrap();
            }
            Self { dir, db }
        }

        fn options(&self) -> RebuildOptions {
            RebuildOptions {
                dimensions: 384,
                backup: self.dir.path().join("original.db"),
                dry_run: false,
            }
        }

        fn rows(&self) -> Vec<Vec<Value>> {
            raw_rows(&Connection::open(&self.db).unwrap())
        }

        fn vectors(&self) -> Vec<(String, Vec<u8>)> {
            raw_vectors(&Connection::open(&self.db).unwrap())
        }
    }

    fn raw_rows(conn: &Connection) -> Vec<Vec<Value>> {
        let mut stmt = conn
            .prepare("SELECT * FROM memory_item ORDER BY point_id")
            .unwrap();
        let count = stmt.column_count();
        stmt.query_map([], |r| (0..count).map(|n| r.get(n)).collect())
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    fn raw_vectors(conn: &Connection) -> Vec<(String, Vec<u8>)> {
        let mut stmt = conn
            .prepare("SELECT point_id, embedding FROM memory_vec ORDER BY point_id")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    struct Model {
        dimensions: usize,
        fail_at: Option<usize>,
        calls: AtomicUsize,
    }

    impl Model {
        fn good() -> Self {
            Self {
                dimensions: 384,
                fail_at: None,
                calls: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl MigrationEmbedder for Model {
        async fn embed(&self, text: &str) -> Result<Vec<f32>> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            anyhow::ensure!(self.fail_at != Some(call), "synthetic embedding failure");
            let mut vector = vec![0.0; self.dimensions];
            if !vector.is_empty() {
                vector[if text == "row-0" {
                    0
                } else {
                    self.dimensions - 1
                }] = 1.0;
            }
            Ok(vector)
        }
    }

    #[tokio::test]
    async fn rebuilds_192_literal_ids_and_raw_records_then_reopens_recall_and_rollback() {
        let f = Fixture::new(192, 768);
        let original_rows = f.rows();
        let original_vectors = f.vectors();
        let options = f.options();
        let model = Model::good();
        let report = rebuild_sqlite_memory_vectors(&f.db, Some(&model), &options)
            .await
            .unwrap();
        assert_eq!(report.records, 192);
        assert_eq!(report.vectors_written, 192);
        assert_eq!(report.previous_dimensions, Some(768));
        assert_eq!(report.published_dimensions, Some(384));
        assert_eq!(model.calls.load(Ordering::SeqCst), 192);
        assert_eq!(f.rows(), original_rows);
        let vectors = f.vectors();
        assert_eq!(vectors.len(), 192);
        assert!(vectors.iter().all(|(_, blob)| blob.len() == 384 * 4));
        assert_eq!(
            vectors.iter().map(|(id, _)| id).collect::<Vec<_>>(),
            original_vectors
                .iter()
                .map(|(id, _)| id)
                .collect::<Vec<_>>()
        );
        let backup = Connection::open(&options.backup).unwrap();
        assert_eq!(raw_rows(&backup), original_rows);
        assert_eq!(raw_vectors(&backup), original_vectors);
        drop(backup);
        let store = SqliteVecSemanticMemoryStore::at(f.dir.path().to_path_buf());
        let mut query = vec![0.0; 384];
        query[0] = 1.0;
        let hits = store.search("fixture", &query, 1, None).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, uuid::Uuid::from_u128(100));
        assert_eq!(hits[0].memory.content, "row-0");

        let restored = tempfile::tempdir().unwrap();
        std::fs::copy(&options.backup, restored.path().join("fixture.memories.db")).unwrap();
        let old_store = SqliteVecSemanticMemoryStore::at(restored.path().to_path_buf());
        let mut old_query = vec![0.0; 768];
        old_query[767] = 1.0;
        assert!(!old_store
            .search("fixture", &old_query, 192, None)
            .await
            .unwrap()
            .is_empty());
        assert_eq!(
            raw_rows(&Connection::open(restored.path().join("fixture.memories.db")).unwrap()),
            original_rows
        );
    }

    #[tokio::test]
    async fn failure_midway_keeps_old_rows_vectors_and_allows_complete_retry() {
        let f = Fixture::new(8, 768);
        let rows = f.rows();
        let vectors = f.vectors();
        let options = f.options();
        let broken = Model {
            fail_at: Some(4),
            ..Model::good()
        };
        assert!(
            rebuild_sqlite_memory_vectors(&f.db, Some(&broken), &options)
                .await
                .is_err()
        );
        assert_eq!(f.rows(), rows);
        assert_eq!(f.vectors(), vectors);
        assert!(!options.backup.exists());
        let good = Model::good();
        rebuild_sqlite_memory_vectors(&f.db, Some(&good), &options)
            .await
            .unwrap();
        assert_eq!(good.calls.load(Ordering::SeqCst), 8);
        assert_eq!(f.rows(), rows);
        assert!(f.vectors().iter().all(|(_, v)| v.len() == 1536));
    }

    #[tokio::test]
    async fn empty_baked_schema_and_same_width_rebuild_are_supported() {
        let empty = Fixture::new(0, 768);
        let options = empty.options();
        let report = rebuild_sqlite_memory_vectors(&empty.db, Some(&Model::good()), &options)
            .await
            .unwrap();
        assert_eq!(report.records, 0);
        assert_eq!(report.previous_dimensions, Some(768));
        assert_eq!(
            vector_dimensions(&Connection::open(&empty.db).unwrap()).unwrap(),
            Some(384)
        );
        let f = Fixture::new(2, 384);
        let old = f.vectors();
        let model = Model::good();
        let report = rebuild_sqlite_memory_vectors(&f.db, Some(&model), &f.options())
            .await
            .unwrap();
        assert_eq!(report.vectors_written, 2);
        assert_eq!(model.calls.load(Ordering::SeqCst), 2);
        assert_ne!(f.vectors()[0].1, old[0].1);
    }

    #[tokio::test]
    async fn dry_run_is_read_only_and_reports_schema_width_without_loading_a_model() {
        let f = Fixture::new(0, 768);
        let mut options = f.options();
        options.dry_run = true;
        let before = std::fs::read(&f.db).unwrap();
        let report = rebuild_sqlite_memory_vectors(&f.db, None, &options)
            .await
            .unwrap();
        assert_eq!(report.previous_dimensions, Some(768));
        assert_eq!(report.requested_dimensions, 384);
        assert_eq!(report.published_dimensions, None);
        assert_eq!(report.vectors_written, 0);
        assert_eq!(report.backup, None);
        assert!(!options.backup.exists());
        assert_eq!(std::fs::read(&f.db).unwrap(), before);
    }

    struct Constant(Vec<f32>);

    #[async_trait]
    impl MigrationEmbedder for Constant {
        async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
            Ok(self.0.clone())
        }
    }

    #[tokio::test]
    async fn invalid_vectors_refuse_without_a_backup_or_any_partial_publication() {
        for vector in [
            vec![],
            vec![1.0; 383],
            vec![1.0; 385],
            vec![f32::NAN; 384],
            vec![f32::INFINITY; 384],
            vec![f32::NEG_INFINITY; 384],
        ] {
            let f = Fixture::new(2, 768);
            let rows = f.rows();
            let old = f.vectors();
            let options = f.options();
            let error = rebuild_sqlite_memory_vectors(&f.db, Some(&Constant(vector)), &options)
                .await
                .unwrap_err();
            assert!(error.to_string().contains("memory "), "{error:#}");
            assert_eq!(f.rows(), rows);
            assert_eq!(f.vectors(), old);
            assert!(!options.backup.exists());
        }
    }

    #[tokio::test]
    async fn zero_dimensions_missing_database_and_missing_embedder_do_not_create_an_index() {
        let f = Fixture::new(2, 768);
        let before = f.vectors();
        let mut options = f.options();
        options.dimensions = 0;
        assert!(
            rebuild_sqlite_memory_vectors(&f.db, Some(&Model::good()), &options)
                .await
                .unwrap_err()
                .to_string()
                .contains("positive")
        );
        options.dimensions = 384;
        let missing = f.dir.path().join("does-not-exist.db");
        assert!(
            rebuild_sqlite_memory_vectors(&missing, Some(&Model::good()), &options)
                .await
                .is_err()
        );
        assert!(!missing.exists());
        assert!(rebuild_sqlite_memory_vectors(&f.db, None, &options)
            .await
            .is_err());
        assert_eq!(f.vectors(), before);
        assert!(!options.backup.exists());
    }

    #[tokio::test]
    async fn malformed_rows_and_foreign_schemas_are_not_silently_skipped() {
        for sql in [
            "UPDATE memory_item SET json = '{broken'",
            "UPDATE memory_item SET point_id = 'not-a-uuid'",
            "DROP TABLE memory_vec; CREATE TABLE memory_vec(point_id TEXT, embedding BLOB)",
            "DROP TABLE memory_item; CREATE TABLE memory_item(point_id TEXT, json TEXT)",
        ] {
            let f = Fixture::new(1, 768);
            Connection::open(&f.db).unwrap().execute_batch(sql).unwrap();
            let before = std::fs::read(&f.db).unwrap();
            let mut options = f.options();
            options.dry_run = true;
            assert!(
                rebuild_sqlite_memory_vectors(&f.db, None, &options)
                    .await
                    .is_err(),
                "{sql}"
            );
            assert_eq!(std::fs::read(&f.db).unwrap(), before);
            assert!(!options.backup.exists());
        }
        let f = Fixture::new(0, 768);
        Connection::open(&f.db)
            .unwrap()
            .execute_batch("DROP TABLE memory_item")
            .unwrap();
        assert!(
            rebuild_sqlite_memory_vectors(&f.db, Some(&Model::good()), &f.options())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn an_existing_backup_is_never_overwritten_and_a_failed_backup_prevents_publication() {
        let f = Fixture::new(2, 768);
        let before = f.vectors();
        let mut options = f.options();
        std::fs::write(&options.backup, b"another owners bytes").unwrap();
        let model = Model::good();
        assert!(rebuild_sqlite_memory_vectors(&f.db, Some(&model), &options)
            .await
            .is_err());
        assert_eq!(model.calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            std::fs::read(&options.backup).unwrap(),
            b"another owners bytes"
        );
        options.backup = f.dir.path().join("missing-parent/backup.db");
        assert!(rebuild_sqlite_memory_vectors(&f.db, Some(&model), &options)
            .await
            .is_err());
        assert_eq!(f.vectors(), before);
    }

    #[tokio::test]
    async fn a_wal_backup_contains_uncheckpointed_rows_and_original_vectors() {
        let f = Fixture::new(2, 768);
        let writer = Connection::open(&f.db).unwrap();
        writer.pragma_update(None, "journal_mode", "WAL").unwrap();
        writer.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
        writer
            .execute(
                "UPDATE memory_item SET future = ?1 WHERE point_id = ?2",
                params![vec![42_u8, 43], uuid::Uuid::from_u128(100).to_string()],
            )
            .unwrap();
        assert!(f.db.with_extension("db-wal").is_file());
        let original_rows = f.rows();
        let original_vectors = f.vectors();
        let options = f.options();
        rebuild_sqlite_memory_vectors(&f.db, Some(&Model::good()), &options)
            .await
            .unwrap();
        let backup = Connection::open(&options.backup).unwrap();
        assert_eq!(raw_rows(&backup), original_rows);
        assert_eq!(raw_vectors(&backup), original_vectors);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&options.backup)
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        assert_eq!(f.rows(), original_rows);
    }

    struct Writer {
        db: PathBuf,
        change_record: bool,
        calls: AtomicUsize,
    }

    #[async_trait]
    impl MigrationEmbedder for Writer {
        async fn embed(&self, text: &str) -> Result<Vec<f32>> {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                let mut conn = Connection::open(&self.db)?;
                if self.change_record {
                    conn.execute("UPDATE memory_item SET updated_at = 'external writer'", [])?;
                } else {
                    let tx = conn.transaction()?;
                    let id = uuid::Uuid::from_u128(100).to_string();
                    tx.execute("DELETE FROM memory_vec WHERE point_id = ?1", [&id])?;
                    tx.execute(
                        "INSERT INTO memory_vec VALUES (?1, ?2)",
                        params![id, crate::sqlite_vec_ext::dense_blob(&vec![0.5; 768])],
                    )?;
                    tx.commit()?;
                }
            }
            Model::good().embed(text).await
        }
    }

    #[tokio::test]
    async fn concurrent_record_and_vector_only_writers_are_preserved_and_rebuild_refused() {
        for change_record in [true, false] {
            let f = Fixture::new(2, 768);
            let original_rows = f.rows();
            let original_vectors = f.vectors();
            let writer = Writer {
                db: f.db.clone(),
                change_record,
                calls: AtomicUsize::new(0),
            };
            let error = rebuild_sqlite_memory_vectors(&f.db, Some(&writer), &f.options())
                .await
                .unwrap_err();
            assert!(format!("{error:#}").contains("source changed"), "{error:#}");
            assert_eq!(
                vector_dimensions(&Connection::open(&f.db).unwrap()).unwrap(),
                Some(768)
            );
            if change_record {
                assert_ne!(f.rows(), original_rows);
                assert_eq!(f.vectors(), original_vectors);
                let conn = Connection::open(&f.db).unwrap();
                let updated: String = conn
                    .query_row("SELECT updated_at FROM memory_item LIMIT 1", [], |r| {
                        r.get(0)
                    })
                    .unwrap();
                assert_eq!(updated, "external writer");
            } else {
                assert_eq!(f.rows(), original_rows);
                assert_eq!(
                    f.vectors()[0].1,
                    crate::sqlite_vec_ext::dense_blob(&vec![0.5; 768])
                );
                assert_eq!(f.vectors()[1].1, original_vectors[1].1);
            }
        }
    }

    #[test]
    fn a_real_insert_failure_after_vec0_ddl_restores_old_schema_rows_and_vector_bytes() {
        let f = Fixture::new(2, 768);
        let old_rows = f.rows();
        let old_vectors = f.vectors();
        let mut conn = Connection::open(&f.db).unwrap();
        let snapshot = read_snapshot(&conn).unwrap();
        let version = data_version(&conn).unwrap();
        let options = f.options();
        verified_backup(&conn, &options.backup, &snapshot).unwrap();
        let duplicate = snapshot.ids[0].clone();
        let replacements = vec![
            (duplicate.clone(), vec![0.2; 384]),
            (duplicate, vec![0.3; 384]),
        ];
        let error = publish_vectors(&mut conn, version, &snapshot, &replacements, 384).unwrap_err();
        assert!(
            error.to_string().contains("UNIQUE") || error.to_string().contains("unique"),
            "{error:#}"
        );
        assert_eq!(vector_dimensions(&conn).unwrap(), Some(768));
        assert_eq!(f.rows(), old_rows);
        assert_eq!(f.vectors(), old_vectors);
        assert_eq!(
            raw_vectors(&Connection::open(&options.backup).unwrap()),
            old_vectors
        );
    }

    #[tokio::test]
    async fn missing_or_orphan_old_vectors_are_replaced_with_exact_record_coverage() {
        let f = Fixture::new(2, 768);
        let conn = Connection::open(&f.db).unwrap();
        conn.execute(
            "DELETE FROM memory_vec WHERE point_id = ?1",
            [uuid::Uuid::from_u128(100).to_string()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO memory_vec VALUES (?1,?2)",
            params![
                uuid::Uuid::from_u128(999).to_string(),
                crate::sqlite_vec_ext::dense_blob(&vec![0.1; 768])
            ],
        )
        .unwrap();
        let rows = f.rows();
        rebuild_sqlite_memory_vectors(&f.db, Some(&Model::good()), &f.options())
            .await
            .unwrap();
        assert_eq!(f.rows(), rows);
        assert_eq!(
            f.vectors()
                .iter()
                .map(|(id, _)| id.clone())
                .collect::<Vec<_>>(),
            vec![
                uuid::Uuid::from_u128(100).to_string(),
                uuid::Uuid::from_u128(101).to_string()
            ]
        );
    }

    #[tokio::test]
    async fn accepted_uuid_spellings_stay_literal_in_both_tables_and_reopened_recall() {
        // One row per spelling ensures normalization reaches publication rather
        // than being caught incidentally by an ordering difference among IDs.
        for literal in [
            uuid::Uuid::from_u128(0xaabb).to_string().to_uppercase(),
            uuid::Uuid::from_u128(0xaabc).simple().to_string(),
            format!("{{{}}}", uuid::Uuid::from_u128(0xaabd)),
            format!("urn:uuid:{}", uuid::Uuid::from_u128(0xaabe)),
        ] {
            let f = Fixture::new(1, 768);
            let conn = Connection::open(&f.db).unwrap();
            uuid::Uuid::parse_str(&literal).unwrap();
            let old = uuid::Uuid::from_u128(100).to_string();
            let vector: Vec<u8> = conn
                .query_row(
                    "SELECT embedding FROM memory_vec WHERE point_id = ?1",
                    [&old],
                    |r| r.get(0),
                )
                .unwrap();
            conn.execute(
                "UPDATE memory_item SET point_id = ?1 WHERE point_id = ?2",
                params![literal, old],
            )
            .unwrap();
            conn.execute("DELETE FROM memory_vec WHERE point_id = ?1", [&old])
                .unwrap();
            conn.execute(
                "INSERT INTO memory_vec VALUES (?1, ?2)",
                params![literal, vector],
            )
            .unwrap();
            let original = f.rows();
            let report = rebuild_sqlite_memory_vectors(&f.db, Some(&Model::good()), &f.options())
                .await
                .unwrap();
            assert_eq!(report.vectors_written, 1);
            assert_eq!(f.rows(), original);
            assert_eq!(
                f.vectors()
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect::<Vec<_>>(),
                vec![literal.clone()]
            );
            let store = SqliteVecSemanticMemoryStore::at(f.dir.path().to_path_buf());
            let mut query = vec![0.0; 384];
            query[0] = 1.0;
            let hits = store.search("fixture", &query, 1, None).await.unwrap();
            assert_eq!(hits.len(), 1);
            assert_eq!(hits[0].id, uuid::Uuid::parse_str(&literal).unwrap());
            assert_eq!(hits[0].memory.content, "row-0");
        }
    }

    #[test]
    fn missing_or_unique_orphan_replacements_refuse_and_roll_back_after_real_inserts() {
        for orphan in [false, true] {
            let f = Fixture::new(2, 768);
            let old_rows = f.rows();
            let old_vectors = f.vectors();
            let mut conn = Connection::open(&f.db).unwrap();
            let snapshot = read_snapshot(&conn).unwrap();
            let version = data_version(&conn).unwrap();
            let options = f.options();
            verified_backup(&conn, &options.backup, &snapshot).unwrap();
            let mut replacements = vec![(snapshot.ids[0].clone(), vec![0.2; 384])];
            if orphan {
                replacements.push((uuid::Uuid::from_u128(999).to_string(), vec![0.3; 384]));
            }
            let error =
                publish_vectors(&mut conn, version, &snapshot, &replacements, 384).unwrap_err();
            let message = error.to_string();
            assert!(
                message.contains(if orphan { "missing/orphan" } else { "count" }),
                "{message}"
            );
            assert_eq!(vector_dimensions(&conn).unwrap(), Some(768));
            assert_eq!(f.rows(), old_rows);
            assert_eq!(f.vectors(), old_vectors);
            assert_eq!(
                raw_vectors(&Connection::open(&options.backup).unwrap()),
                old_vectors
            );
        }
    }

    #[tokio::test]
    async fn legacy_in_place_refuses_the_sqlite_dimension_change_and_reports_every_row_skipped() {
        let f = Fixture::new(2, 768);
        let old_rows = f.rows();
        let old_vectors = f.vectors();
        let store = SqliteVecSemanticMemoryStore::at(f.dir.path().to_path_buf());
        let report = crate::migrate::memories::reembed_memories_in_place(
            &store,
            &Model::good(),
            "fixture",
            false,
        )
        .await
        .unwrap();
        assert_eq!(report.read, 2);
        assert_eq!(report.upserted, 0);
        assert_eq!(report.skipped, 2);
        assert_eq!(f.rows(), old_rows);
        assert_eq!(f.vectors(), old_vectors);
    }
}
