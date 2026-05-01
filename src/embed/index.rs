//! sqlite-vec based embedding index with incremental updates.
//!
//! Inspired by cocoindex-code's SQLite + sqlite-vec approach:
//! zero external services, embedded in the project directory.
//!
//! Schema:
//!   files(path TEXT, hash TEXT, mtime INTEGER) — tracks indexed file hashes + mtime
//!   chunks(id, file_path, language, content,   — code chunks
//!          start_line, end_line, file_hash,
//!          source)
//!   chunk_embeddings(rowid, embedding)         — blob table (sqlite-vec for search)
//!   meta(key TEXT, value TEXT)                  — stores embed_model, last_indexed_commit
//!
//! Change detection fallback chain:
//!   1. git diff last_indexed_commit..HEAD (tracked files)
//!   2. mtime comparison (untracked files or git unavailable)
//!   3. SHA-256 hash (final arbiter)
//!

use anyhow::{Context as _, Result};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Once;

use super::schema::{CodeChunk, SearchResult};

/// Typed filter for the `source` column in embedding search.
///
/// Replaces raw `Option<&str>` to prevent typos and make the API self-documenting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceScope {
    /// No filter — search all sources (project + all libraries).
    All,
    /// Only project-owned chunks (`source = 'project'`).
    Project,
    /// All library chunks (any `source` that isn't `'project'`).
    Libraries,
    /// A specific library by name (`source = '<name>'`).
    Library(String),
}

impl SourceScope {
    /// Parse from the string format used by the tool layer.
    pub fn from_str_opt(s: Option<&str>) -> Self {
        match s {
            None => Self::All,
            Some("project") => Self::Project,
            Some("libraries") => Self::Libraries,
            Some(lib) => Self::Library(lib.to_string()),
        }
    }

    /// Convert back to the `Option<&str>` format for SQL queries.
    pub fn as_sql_param(&self) -> Option<&str> {
        match self {
            Self::All => None,
            Self::Project => Some("project"),
            Self::Libraries => Some("libraries"),
            Self::Library(name) => Some(name.as_str()),
        }
    }
}

/// Path to the embedding database within a project.
pub fn db_path(project_root: &Path) -> PathBuf {
    project_root.join(".codescout").join("embeddings.db")
}

/// Path to the project embedding database (new layout).
pub fn project_db_path(project_root: &Path) -> PathBuf {
    project_root
        .join(".codescout")
        .join("embeddings")
        .join("project.db")
}

/// Path to a library's embedding database.
pub fn lib_db_path(project_root: &Path, lib_name: &str) -> PathBuf {
    project_root
        .join(".codescout")
        .join("embeddings")
        .join("lib")
        .join(format!("{}.db", sanitize_lib_name(lib_name)))
}

/// Sanitize a library name for use as a filename.
/// Replaces `/` and `\` with `--`, always lowercases for cross-platform consistency.
fn sanitize_lib_name(name: &str) -> String {
    let s = name.replace(['/', '\\'], "--");
    s.to_lowercase()
}

/// Migrate from old single-DB layout to new embeddings/ directory layout.
/// Called from `open_db` before opening the connection.
///
/// Old: `.codescout/embeddings.db`
/// New: `.codescout/embeddings/project.db` + `.codescout/embeddings/lib/`
pub fn maybe_migrate_db_layout(project_root: &Path) -> Result<()> {
    let old_path = db_path(project_root); // .codescout/embeddings.db
    let new_path = project_db_path(project_root);

    // Already migrated or no old DB — nothing to do
    if new_path.exists() || !old_path.exists() {
        // Ensure lib/ directory exists regardless
        let lib_dir = project_root.join(".codescout/embeddings/lib");
        if !lib_dir.exists() {
            std::fs::create_dir_all(&lib_dir)?;
        }
        return Ok(());
    }

    tracing::info!("Migrating embedding storage to new layout...");

    // Create new directory structure
    std::fs::create_dir_all(new_path.parent().unwrap())?;
    std::fs::create_dir_all(project_root.join(".codescout/embeddings/lib"))?;

    // Rename old DB to new location
    std::fs::rename(&old_path, &new_path)?;

    tracing::info!(
        "Migration complete: {} → {}",
        old_path.display(),
        new_path.display()
    );

    // Extract any library chunks (source LIKE 'lib:%') into per-library DBs
    extract_library_chunks_from_project_db(project_root)?;

    Ok(())
}

/// Extract all `lib:*`-tagged chunks from project.db into per-library DBs.
///
/// For each distinct `lib:<name>` source found in project.db:
///   1. Copy the chunks + embeddings to `.codescout/embeddings/lib/<name>.db`
///   2. Delete those rows from project.db
///   3. VACUUM project.db to reclaim space
///
/// This is a one-time migration step called from `maybe_migrate_db_layout`.
fn extract_library_chunks_from_project_db(project_root: &Path) -> Result<()> {
    let proj_path = project_db_path(project_root);
    let conn = Connection::open(&proj_path)
        .with_context(|| format!("opening project.db at {}", proj_path.display()))?;

    // Find all distinct lib:* sources present in project.db
    let mut stmt = conn.prepare("SELECT DISTINCT source FROM chunks WHERE source LIKE 'lib:%'")?;
    let sources: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    if sources.is_empty() {
        return Ok(());
    }

    tracing::info!(
        "Extracting {} library source(s) from project.db",
        sources.len()
    );

    for source in &sources {
        // Strip the "lib:" prefix to get the library name
        let lib_name = source.strip_prefix("lib:").unwrap_or(source.as_str());
        let lib_path = lib_db_path(project_root, lib_name);
        if let Some(parent) = lib_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        copy_chunks_to_lib_db(&conn, &lib_path, source).with_context(|| {
            format!(
                "copying chunks for source '{source}' to {}",
                lib_path.display()
            )
        })?;
    }

    // Delete all lib:* chunks from project.db
    conn.execute("DELETE FROM chunks WHERE source LIKE 'lib:%'", [])?;
    // Cascading delete for embeddings: chunk_embeddings rows whose rowid no
    // longer matches any chunk.id.  Plain blob table — no FK cascade — so we
    // delete orphans explicitly.
    conn.execute(
        "DELETE FROM chunk_embeddings WHERE rowid NOT IN (SELECT id FROM chunks)",
        [],
    )?;
    conn.execute_batch("VACUUM")?;

    tracing::info!("Library chunk extraction complete");
    Ok(())
}

/// Create a library DB at `lib_path` (with the standard schema + a `lib_meta`
/// table) and copy all chunks + embeddings for `source` from `src_conn`.
fn copy_chunks_to_lib_db(src_conn: &Connection, lib_path: &Path, source: &str) -> Result<()> {
    use rusqlite::OptionalExtension;
    let lib_conn = Connection::open(lib_path)?;
    lib_conn.busy_timeout(std::time::Duration::from_secs(5))?;

    // Create the same schema as project.db
    lib_conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;

        CREATE TABLE IF NOT EXISTS chunks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_path TEXT NOT NULL,
            language TEXT NOT NULL,
            content TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            file_hash TEXT NOT NULL,
            source TEXT NOT NULL DEFAULT 'project',
            metadata TEXT
        );
        CREATE TABLE IF NOT EXISTS chunk_embeddings (
            rowid INTEGER PRIMARY KEY,
            embedding BLOB NOT NULL
        );
        CREATE TABLE IF NOT EXISTS files (
            path TEXT PRIMARY KEY,
            hash TEXT NOT NULL,
            mtime INTEGER
        );
        CREATE TABLE IF NOT EXISTS lib_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    )?;

    // Collect all chunk IDs for this source — query by source, then read each
    // row by PK to avoid cursor issues during the insert loop.
    let mut id_stmt = src_conn.prepare("SELECT id FROM chunks WHERE source = ?1")?;
    let ids: Vec<i64> = id_stmt
        .query_map([source], |r| r.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    lib_conn.execute_batch("BEGIN")?;
    for id in &ids {
        // Read the full chunk row by PK
        let row = src_conn.query_row(
            "SELECT file_path, language, content, start_line, end_line, file_hash, source, metadata \
             FROM chunks WHERE id = ?1",
            [id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, Option<String>>(7)?,
                ))
            },
        );
        let (file_path, language, content, start_line, end_line, file_hash, src_tag, metadata) =
            match row {
                Ok(r) => r,
                Err(rusqlite::Error::QueryReturnedNoRows) => continue,
                Err(e) => return Err(e.into()),
            };

        // Read the corresponding embedding blob (may not exist for old rows)
        let embedding: Option<Vec<u8>> = src_conn
            .query_row(
                "SELECT embedding FROM chunk_embeddings WHERE rowid = ?1",
                [id],
                |r| r.get(0),
            )
            .optional()
            .with_context(|| format!("reading embedding for chunk id={id}"))?;

        // Insert chunk, capturing the new rowid for the embedding
        lib_conn.execute(
            "INSERT INTO chunks \
             (file_path, language, content, start_line, end_line, file_hash, source, metadata) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                file_path, language, content, start_line, end_line, file_hash, src_tag, metadata
            ],
        )?;
        let new_rowid = lib_conn.last_insert_rowid();

        if let Some(blob) = embedding {
            lib_conn.execute(
                "INSERT INTO chunk_embeddings (rowid, embedding) VALUES (?1, ?2)",
                params![new_rowid, blob],
            )?;
        }
    }
    lib_conn.execute_batch("COMMIT")?;

    tracing::debug!(
        "Copied {} chunk(s) for source '{}' → {}",
        ids.len(),
        source,
        lib_path.display()
    );
    Ok(())
}

/// Fallback for users who had library chunks stored without a `source` tag.
/// For now this is a no-op stub — most users won't be in this state.
#[allow(dead_code)]
fn extract_by_file_path_fallback(project_root: &Path, _conn: &Connection) -> Result<()> {
    // Check if libraries.json exists; if not, nothing to do.
    let libs_json = project_root.join(".codescout/libraries.json");
    if !libs_json.exists() {
        return Ok(());
    }
    // Full implementation deferred: users with untagged library chunks will
    // need to re-index their libraries after the migration.
    Ok(())
}

/// Bump whenever the `chunks` table schema changes in a way that requires
/// re-embedding. On mismatch, `open_db` drops and recreates all chunk-related tables.
const SCHEMA_VERSION: u32 = 1;

/// Open (or create) the embedding database and apply the schema.
/// Register sqlite-vec globally so every SQLite connection in this process
/// gets `vec_distance_cosine`, `vec_f32`, and the `vec0` virtual table module.
/// Uses `sqlite3_auto_extension` so the init runs on every `Connection::open`.
/// Safe to call multiple times — the `Once` guard makes it idempotent.
pub(crate) fn init_sqlite_vec() {
    // Compile-time pin on the upstream signature of `sqlite3_vec_init`.
    // The transmute below relies on it being `unsafe extern "C" fn()` (the
    // "bare" SQLite-loadable-extension entry point); if a future version of
    // the `sqlite-vec` crate changes the exported signature, this const
    // initializer will fail to type-check and flag the transmute before it
    // silently miscompiles at runtime.
    const _UPSTREAM_SQLITE_VEC_INIT_SIG: unsafe extern "C" fn() = sqlite_vec::sqlite3_vec_init;

    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // SAFETY: sqlite3_vec_init is a valid SQLite extension entry point.
        // sqlite3_auto_extension expects the full extension init signature:
        // fn(db, pzErrMsg, pApi) -> i32, but sqlite3_vec_init is declared as
        // extern "C" fn() in the crate. The transmute is safe because SQLite
        // will call it with the correct arguments at the C level.
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute::<
                *const (),
                unsafe extern "C" fn(
                    *mut rusqlite::ffi::sqlite3,
                    *mut *mut i8,
                    *const rusqlite::ffi::sqlite3_api_routines,
                ) -> i32,
            >(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
    });
}

pub fn open_db(project_root: &Path) -> Result<Connection> {
    // Migrate from old layout if needed (no-op when already on new layout)
    maybe_migrate_db_layout(project_root)?;

    let path = project_db_path(project_root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    init_sqlite_vec();
    let conn = Connection::open(&path)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;

    // Read stored schema version; drop chunk tables if stale.
    {
        use rusqlite::OptionalExtension;
        let stored: Option<u32> = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |r| {
                    r.get::<_, String>(0)
                        .and_then(|s| s.parse::<u32>().map_err(|_| rusqlite::Error::InvalidQuery))
                },
            )
            .optional()
            .ok()
            .flatten();

        if stored != Some(SCHEMA_VERSION) {
            conn.execute_batch(
                "DROP TABLE IF EXISTS chunks;
                 DROP TABLE IF EXISTS chunk_embeddings;
                 DROP TABLE IF EXISTS files;",
            )
            .context("dropping stale schema")?;
        }
    }

    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;

        CREATE TABLE IF NOT EXISTS files (
            path  TEXT PRIMARY KEY,
            hash  TEXT NOT NULL,
            mtime INTEGER
        );

        CREATE TABLE IF NOT EXISTS chunks (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            file_path  TEXT NOT NULL,
            language   TEXT NOT NULL,
            content    TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line   INTEGER NOT NULL,
            file_hash  TEXT NOT NULL,
            source     TEXT NOT NULL DEFAULT 'project',
            metadata   TEXT
        );

        CREATE TABLE IF NOT EXISTS chunk_embeddings (
            rowid     INTEGER PRIMARY KEY,
            embedding BLOB NOT NULL
        );

        CREATE TABLE IF NOT EXISTS meta (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS drift_report (
            file_path       TEXT PRIMARY KEY,
            avg_drift       REAL NOT NULL,
            max_drift       REAL NOT NULL,
            max_drift_chunk TEXT,
            chunks_added    INTEGER NOT NULL,
            chunks_removed  INTEGER NOT NULL,
            indexed_at      TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS memories (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            bucket     TEXT NOT NULL DEFAULT 'unstructured',
            title      TEXT NOT NULL,
            content    TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_memories_bucket ON memories(bucket);
        ",
    )?;

    // Record current schema version so future opens can detect staleness.
    conn.execute(
        "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?1)",
        [&SCHEMA_VERSION.to_string()],
    )?;

    // Column-addition migrations wrapped in a SAVEPOINT so a crash mid-migration
    // leaves the DB consistent rather than partially applied. Probe queries run
    // outside the savepoint (read-only); only the writes need atomicity.
    conn.execute_batch("SAVEPOINT schema_migrations")?;
    let migrate = (|| -> Result<()> {
        // Migrate: add mtime column if missing (safe no-op if already present)
        let has_mtime: bool = conn.prepare("SELECT mtime FROM files LIMIT 0").is_ok();
        if !has_mtime {
            conn.execute_batch("ALTER TABLE files ADD COLUMN mtime INTEGER")?;
        }

        // Migrate: add source column to chunks if missing
        let has_source: bool = conn.prepare("SELECT source FROM chunks LIMIT 0").is_ok();
        if !has_source {
            conn.execute_batch(
                "ALTER TABLE chunks ADD COLUMN source TEXT NOT NULL DEFAULT 'project'",
            )?;
        }

        // Migrate: add project_id column (workspace multi-project support)
        let has_project_id: bool = conn
            .prepare("SELECT project_id FROM chunks LIMIT 0")
            .is_ok();
        if !has_project_id {
            conn.execute(
                "ALTER TABLE chunks ADD COLUMN project_id TEXT NOT NULL DEFAULT 'root'",
                [],
            )?;
        }

        // Migrate: add metadata column (searchable header prepended before embedding)
        let has_metadata: bool = conn.prepare("SELECT metadata FROM chunks LIMIT 0").is_ok();
        if !has_metadata {
            conn.execute_batch("ALTER TABLE chunks ADD COLUMN metadata TEXT")?;
        }

        Ok(())
    })();
    match migrate {
        Ok(()) => conn.execute_batch("RELEASE schema_migrations")?,
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK TO schema_migrations");
            let _ = conn.execute_batch("RELEASE schema_migrations");
            return Err(e);
        }
    }

    maybe_migrate_to_vec0(&conn)?;

    Ok(conn)
}

/// Open (or create) the embedding database for a specific library.
pub fn open_lib_db(project_root: &Path, lib_name: &str) -> Result<Connection> {
    let path = lib_db_path(project_root, lib_name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    init_sqlite_vec();
    let conn = Connection::open(&path)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;

    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;

        CREATE TABLE IF NOT EXISTS chunks (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            file_path  TEXT NOT NULL,
            language   TEXT NOT NULL,
            content    TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line   INTEGER NOT NULL,
            file_hash  TEXT NOT NULL,
            source     TEXT NOT NULL DEFAULT 'project',
            metadata   TEXT
        );

        CREATE TABLE IF NOT EXISTS chunk_embeddings (
            rowid     INTEGER PRIMARY KEY,
            embedding BLOB NOT NULL
        );

        CREATE TABLE IF NOT EXISTS files (
            path  TEXT PRIMARY KEY,
            hash  TEXT NOT NULL,
            mtime INTEGER
        );

        CREATE TABLE IF NOT EXISTS meta (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS lib_meta (
            key   TEXT PRIMARY KEY,
            value TEXT
        );
    ",
    )?;

    maybe_migrate_to_vec0(&conn)?;
    Ok(conn)
}

/// Migrate `chunk_embeddings` from a plain BLOB table to a `vec0` virtual
/// table if `embedding_dims` is stored in meta and the table is not yet
/// a virtual table. Safe to call multiple times (idempotent).
pub fn maybe_migrate_to_vec0(conn: &Connection) -> Result<()> {
    use rusqlite::OptionalExtension;

    let dims: usize = match get_meta(conn, "embedding_dims")? {
        Some(s) => s
            .parse()
            .context("embedding_dims in meta is not a valid integer — DB may be corrupted")?,
        None => return Ok(()),
    };
    if dims == 0 {
        return Ok(());
    }

    // sqlite-vec registers vec0 virtual tables as type='table' in sqlite_master
    // (same as plain tables). The sql column contains "USING vec0" which is how
    // we distinguish them. This is the correct detection idiom for this extension.
    let sql: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='chunk_embeddings'",
            [],
            |r| r.get(0),
        )
        .optional()?;

    match sql {
        None => return Ok(()), // table doesn't exist yet
        Some(s) if s.contains("USING vec0") => return Ok(()), // already migrated
        _ => {}
    }

    tracing::info!("Migrating chunk_embeddings to vec0 virtual table (dims={dims})");

    // Use BEGIN IMMEDIATE to serialise concurrent migration attempts.  Only
    // one connection can hold the reserved (write) lock at a time, so the
    // second caller blocks here until the first commits.  Without IMMEDIATE
    // the check-then-migrate sequence is a classic TOCTOU race: two
    // connections can both observe "plain table" outside any transaction,
    // then the loser corrupts the database by renaming the already-migrated
    // vec0 virtual table (ALTER TABLE succeeds) and then failing on CREATE
    // VIRTUAL TABLE because the shadow tables (e.g. chunk_embeddings_info)
    // still exist under their original names.
    conn.execute_batch("BEGIN IMMEDIATE")?;

    // Re-check inside the exclusive transaction: another connection may have
    // migrated between our initial fast-path check above and acquiring the
    // write lock here.
    let sql_after_lock: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='chunk_embeddings'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    match sql_after_lock {
        None => {
            conn.execute_batch("ROLLBACK")?;
            return Ok(());
        }
        Some(s) if s.contains("USING vec0") => {
            conn.execute_batch("ROLLBACK")?;
            return Ok(()); // another connection migrated first
        }
        _ => {} // still plain — proceed with migration
    }

    // Run all DDL inside a closure so we can ROLLBACK on any failure.
    // Without this guard, a mid-migration error (e.g. disk full during INSERT)
    // would leave `chunk_embeddings` renamed to `chunk_embeddings_v1` with no
    // replacement — an irrecoverable corruption state.
    let migrate = || -> Result<()> {
        conn.execute_batch("ALTER TABLE chunk_embeddings RENAME TO chunk_embeddings_v1")?;
        conn.execute_batch(&format!(
            "CREATE VIRTUAL TABLE chunk_embeddings USING vec0(embedding float[{dims}] distance_metric=cosine)"
        ))?;
        conn.execute_batch(
            "INSERT INTO chunk_embeddings(rowid, embedding) \
             SELECT rowid, embedding FROM chunk_embeddings_v1",
        )?;
        conn.execute_batch("DROP TABLE chunk_embeddings_v1")?;
        Ok(())
    };

    match migrate() {
        Ok(()) => {
            conn.execute_batch("COMMIT")?;
            tracing::info!("vec0 migration complete");
            Ok(())
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

/// Lazily create the `vec_memories` vec0 virtual table for semantic memory
/// search. Requires `embedding_dims` to be set in the `meta` table (which
/// happens after the first `build_index` run). Safe to call multiple times
/// (idempotent) — returns `Ok(())` if the table already exists. Returns a
/// `RecoverableError` if `embedding_dims` is not yet set.
pub fn ensure_vec_memories(conn: &Connection) -> Result<()> {
    use rusqlite::OptionalExtension;

    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='vec_memories'",
            [],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    if exists {
        return Ok(());
    }

    let dims = match get_meta(conn, "embedding_dims")? {
        Some(s) => s
            .parse::<usize>()
            .context("embedding_dims in meta is not a valid integer — DB may be corrupted")?,
        None => {
            return Err(crate::tools::RecoverableError::with_hint(
                "semantic index not built yet — cannot store memory embeddings",
                "Run index(action='build') first, then try remember again.",
            )
            .into());
        }
    };
    if dims == 0 {
        return Ok(());
    }

    conn.execute_batch(&format!(
        "CREATE VIRTUAL TABLE vec_memories USING vec0(\
         embedding float[{dims}] distance_metric=cosine)"
    ))?;
    Ok(())
}

/// A memory record returned by [`search_memories`].
pub struct MemoryResult {
    pub id: i64,
    pub bucket: String,
    pub title: String,
    pub content: String,
    pub similarity: f32,
    pub created_at: String,
}

/// Validate that `embedding.len()` matches the `embedding_dims` recorded in
/// the `meta` table. Surfaces a `RecoverableError` with a repair hint when
/// they diverge — the usual cause is a model change between the initial
/// `build_index` and the current `memory(action="write")` call, which would
/// otherwise be swallowed by sqlite-vec returning a cryptic INSERT error.
fn validate_memory_embedding_dims(conn: &Connection, embedding_len: usize) -> Result<()> {
    let Some(stored) = get_meta(conn, "embedding_dims")? else {
        // No dims recorded yet — ensure_vec_memories_table will create it.
        return Ok(());
    };
    let stored_dims: usize = stored
        .parse()
        .context("embedding_dims in meta is not a valid integer — DB may be corrupted")?;
    if stored_dims != 0 && stored_dims != embedding_len {
        return Err(crate::tools::RecoverableError::with_hint(
            format!(
                "embedding dim mismatch: model produced {} but the index was built \
                 with {} — the memory was not stored",
                embedding_len, stored_dims,
            ),
            "Run index(action='build', force=true) to rebuild the semantic index with \
             the current model, then retry memory(action=\"write\").",
        )
        .into());
    }
    Ok(())
}

/// Insert a new memory and its embedding into both `memories` and `vec_memories`.
///
/// Uses a savepoint to ensure both tables are written atomically.
/// Safe to call inside or outside an existing transaction.
/// Returns the row id of the newly inserted memory.
pub fn insert_memory(
    conn: &Connection,
    bucket: &str,
    title: &str,
    content: &str,
    embedding: &[f32],
) -> Result<i64> {
    validate_memory_embedding_dims(conn, embedding.len())?;
    let now = utc_now_display();
    conn.execute_batch("SAVEPOINT sp_insert_memory")?;

    let result = (|| -> Result<i64> {
        conn.execute(
            "INSERT INTO memories (bucket, title, content, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![bucket, title, content, now, now],
        )?;
        let row_id = conn.last_insert_rowid();

        // Serialize embedding as little-endian f32 bytes (sqlite-vec format)
        let blob: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
        conn.execute(
            "INSERT INTO vec_memories (rowid, embedding) VALUES (?1, ?2)",
            params![row_id, blob],
        )?;
        Ok(row_id)
    })();

    match &result {
        Ok(_) => conn.execute_batch("RELEASE sp_insert_memory")?,
        Err(_) => {
            let _ = conn.execute_batch("ROLLBACK TO sp_insert_memory");
            let _ = conn.execute_batch("RELEASE sp_insert_memory");
        }
    }
    result
}

/// Search memories by embedding similarity, optionally filtered by bucket.
///
/// Returns up to `limit` results sorted by descending similarity (1.0 = identical).
pub fn search_memories(
    conn: &Connection,
    query_embedding: &[f32],
    bucket_filter: Option<&str>,
    limit: usize,
) -> Result<Vec<MemoryResult>> {
    let query_blob: Vec<u8> = query_embedding
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();

    let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<MemoryResult> {
        let distance: f64 = row.get(5)?;
        let similarity = (1.0_f32 - distance as f32).clamp(0.0, 1.0);
        Ok(MemoryResult {
            id: row.get(0)?,
            bucket: row.get(1)?,
            title: row.get(2)?,
            content: row.get(3)?,
            created_at: row.get(4)?,
            similarity,
        })
    };

    match bucket_filter {
        None => {
            let knn = "SELECT rowid, distance FROM vec_memories \
                       WHERE embedding MATCH vec_f32(?1) ORDER BY distance LIMIT ?2";
            let sql = format!(
                "SELECT m.id, m.bucket, m.title, m.content, m.created_at, \
                 COALESCE(knn.distance, 1.0) AS distance \
                 FROM memories m JOIN ({knn}) knn ON m.id = knn.rowid \
                 ORDER BY distance ASC"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(params![query_blob, limit as i64], map_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        }
        Some(bucket) => {
            // Over-fetch from KNN (limit * 5) so that post-filtering by bucket
            // still returns `limit` results even when memories are skewed toward
            // other buckets. Factor of 5 is conservative: even if only 1-in-5
            // memories belong to the requested bucket, we still fill the limit.
            // The outer LIMIT then re-caps to the caller's requested count.
            let inner_limit = (limit * 5) as i64;
            let knn = "SELECT rowid, distance FROM vec_memories \
                       WHERE embedding MATCH vec_f32(?1) ORDER BY distance LIMIT ?2";
            let sql = format!(
                "SELECT m.id, m.bucket, m.title, m.content, m.created_at, \
                 COALESCE(knn.distance, 1.0) AS distance \
                 FROM memories m JOIN ({knn}) knn ON m.id = knn.rowid \
                 WHERE m.bucket = ?3 \
                 ORDER BY distance ASC \
                 LIMIT ?4"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(
                    params![query_blob, inner_limit, bucket, limit as i64],
                    map_row,
                )?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        }
    }
}

/// Delete a memory from both `vec_memories` and `memories` by id.
pub fn delete_memory(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM memories WHERE id = ?1", params![id])?;
    if conn.changes() == 0 {
        return Err(crate::tools::RecoverableError::with_hint(
            format!("memory with id {} not found", id),
            "Use recall to find memory IDs before deleting.",
        )
        .into());
    }
    // Then delete from vec_memories (best-effort, may not have embedding)
    let _ = conn.execute("DELETE FROM vec_memories WHERE rowid = ?1", params![id]);
    Ok(())
}

/// Insert or update a memory by title match within the same bucket.
///
/// If a memory with the same `title` already exists, its content, bucket, embedding,
/// and `updated_at` timestamp are updated. Otherwise a new memory is inserted.
/// Returns the row id.
pub fn upsert_memory_by_title(
    conn: &Connection,
    bucket: &str,
    title: &str,
    content: &str,
    embedding: &[f32],
) -> Result<i64> {
    use rusqlite::OptionalExtension;

    validate_memory_embedding_dims(conn, embedding.len())?;

    // Scope the lookup to the same bucket — two memories in different buckets
    // with the same title are independent and must not collide.
    let existing_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM memories WHERE title = ?1 AND bucket = ?2",
            params![title, bucket],
            |r| r.get(0),
        )
        .optional()?;

    match existing_id {
        Some(id) => {
            let now = utc_now_display();
            conn.execute(
                "UPDATE memories SET content = ?1, updated_at = ?2 WHERE id = ?3",
                params![content, now, id],
            )?;
            let blob: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
            conn.execute(
                "UPDATE vec_memories SET embedding = ?1 WHERE rowid = ?2",
                params![blob, id],
            )?;
            Ok(id)
        }
        None => insert_memory(conn, bucket, title, content, embedding),
    }
}

/// Hash the content of a file for change detection.
pub fn hash_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)?;
    let digest = Sha256::digest(&bytes);
    Ok(hex::encode(digest))
}

/// Get file modification time as Unix epoch seconds.
pub fn file_mtime(path: &Path) -> Result<i64> {
    let meta = std::fs::metadata(path)?;
    let modified = meta.modified()?;
    let duration = modified
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    Ok(duration.as_secs() as i64)
}

/// Insert a chunk and its embedding into the database.
///
/// Uses a savepoint to ensure both tables are written atomically.
/// Safe to call inside or outside an existing transaction.
pub fn insert_chunk(conn: &Connection, chunk: &CodeChunk, embedding: &[f32]) -> Result<i64> {
    conn.execute_batch("SAVEPOINT sp_insert_chunk")?;

    let result = (|| -> Result<i64> {
        conn.execute(
            "INSERT INTO chunks (file_path, language, content, start_line, end_line, file_hash, source, project_id, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                chunk.file_path,
                chunk.language,
                chunk.content,
                chunk.start_line as i64,
                chunk.end_line as i64,
                chunk.file_hash,
                chunk.source,
                chunk.project_id,
                chunk.metadata,
            ],
        )?;
        let row_id = conn.last_insert_rowid();

        // Serialize embedding as little-endian f32 bytes (sqlite-vec format)
        let blob: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
        conn.execute(
            "INSERT INTO chunk_embeddings (rowid, embedding) VALUES (?1, ?2)",
            params![row_id, blob],
        )?;
        Ok(row_id)
    })();

    match &result {
        Ok(_) => conn.execute_batch("RELEASE sp_insert_chunk")?,
        Err(_) => {
            let _ = conn.execute_batch("ROLLBACK TO sp_insert_chunk");
            let _ = conn.execute_batch("RELEASE sp_insert_chunk");
        }
    }
    result
}

/// Remove all chunks for a given file path.
pub fn delete_file_chunks(conn: &Connection, file_path: &str) -> Result<()> {
    // Delete embeddings for this file's chunks
    conn.execute(
        "DELETE FROM chunk_embeddings
         WHERE rowid IN (SELECT id FROM chunks WHERE file_path = ?1)",
        params![file_path],
    )?;
    conn.execute(
        "DELETE FROM chunks WHERE file_path = ?1",
        params![file_path],
    )?;
    conn.execute("DELETE FROM files WHERE path = ?1", params![file_path])?;
    Ok(())
}

/// Remove index entries for files that no longer exist on disk.
/// Returns the number of purged files.
pub fn purge_missing_files(conn: &Connection, project_root: &Path) -> Result<usize> {
    let mut stmt = conn.prepare("SELECT path FROM files")?;
    let paths: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut purged = 0;
    for path in &paths {
        let full = project_root.join(path);
        if !full.exists() {
            delete_file_chunks(conn, path)?;
            purged += 1;
        }
    }
    Ok(purged)
}

/// Get the stored hash for a file (for incremental indexing).
pub fn get_file_hash(conn: &Connection, file_path: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT hash FROM files WHERE path = ?1")?;
    let mut rows = stmt.query(params![file_path])?;
    Ok(rows.next()?.map(|r| r.get(0)).transpose()?)
}

/// Update or insert the file hash record.
pub fn upsert_file_hash(
    conn: &Connection,
    file_path: &str,
    hash: &str,
    mtime: Option<i64>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO files (path, hash, mtime) VALUES (?1, ?2, ?3)
         ON CONFLICT(path) DO UPDATE SET hash = excluded.hash, mtime = excluded.mtime",
        params![file_path, hash, mtime],
    )?;
    Ok(())
}

pub fn get_file_mtime(conn: &Connection, file_path: &str) -> Result<Option<i64>> {
    let mut stmt = conn.prepare("SELECT mtime FROM files WHERE path = ?1")?;
    let mut rows = stmt.query(params![file_path])?;
    match rows.next()? {
        Some(row) => Ok(row.get(0)?),
        None => Ok(None),
    }
}

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
                embedding: bytes_to_f32(&blob)
                    .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(chunks)
}

/// Search for the most similar chunks across all sources.
///
/// Shorthand for [`search_scoped`] with no source filter.
pub fn search(
    conn: &Connection,
    query_embedding: &[f32],
    limit: usize,
) -> Result<Vec<SearchResult>> {
    search_scoped(conn, query_embedding, limit, &SourceScope::All)
}
// Returns true when `chunk_embeddings` is a vec0 virtual table.
// Checked via sqlite_master DDL — O(1) index lookup.
//
// No global cache: this process may use multiple databases (project DB +
// per-library DBs), each at a different migration stage.  Caching `true`
// globally would incorrectly take the vec0 SQL path for unmigrated DBs.
// The sqlite_master query is fast and runs only once per search call.
fn is_vec0_active(conn: &Connection) -> bool {
    use rusqlite::OptionalExtension;

    conn.query_row(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='chunk_embeddings'",
        [],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .ok()
    .flatten()
    .map(|sql| sql.contains("USING vec0"))
    .unwrap_or(false)
}

pub fn search_scoped(
    conn: &Connection,
    query_embedding: &[f32],
    limit: usize,
    scope: &SourceScope,
) -> Result<Vec<SearchResult>> {
    if is_vec0_active(conn) {
        return search_scoped_vec0(conn, query_embedding, limit, scope);
    }

    let source_filter = scope.as_sql_param();

    // Encode the query as little-endian f32 bytes — the format vec_f32() expects.
    let query_blob: Vec<u8> = query_embedding
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();

    let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<SearchResult> {
        // vec_distance_cosine returns cosine distance ∈ [0, 1] (0 = identical).
        // COALESCE maps NULL (degenerate zero-vector) to 1.0 (maximum distance).
        let distance: f64 = row.get(6)?;
        let score = (1.0_f32 - distance as f32).clamp(0.0, 1.0);
        Ok(SearchResult {
            file_path: row.get(0)?,
            language: row.get(1)?,
            content: row.get(2)?,
            start_line: row.get::<_, i64>(3)? as usize,
            end_line: row.get::<_, i64>(4)? as usize,
            source: row.get(5)?,
            score,
            project_id: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
        })
    };

    // Common SELECT with sqlite-vec distance. ORDER BY + LIMIT pushed to SQLite.
    let sel = "SELECT c.file_path, c.language, c.content, c.start_line, c.end_line, c.source, \
               COALESCE(vec_distance_cosine(vec_f32(ce.embedding), vec_f32(?1)), 1.0) AS distance, \
               c.project_id \
               FROM chunks c JOIN chunk_embeddings ce ON c.id = ce.rowid";

    match source_filter {
        None => {
            let sql = format!("{sel} ORDER BY distance ASC LIMIT ?2");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(params![query_blob, limit as i64], map_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        }
        Some("libraries") => {
            let sql = format!("{sel} WHERE c.source != 'project' ORDER BY distance ASC LIMIT ?2");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(params![query_blob, limit as i64], map_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        }
        Some(source) => {
            let sql = format!("{sel} WHERE c.source = ?2 ORDER BY distance ASC LIMIT ?3");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(params![query_blob, source, limit as i64], map_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        }
    }
}

fn search_scoped_vec0(
    conn: &Connection,
    query_embedding: &[f32],
    limit: usize,
    scope: &SourceScope,
) -> Result<Vec<SearchResult>> {
    let source_filter = scope.as_sql_param();
    let query_blob: Vec<u8> = query_embedding
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();

    let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<SearchResult> {
        // COALESCE maps NULL distance (degenerate zero-vector) to 1.0 (maximum
        // distance), matching the behaviour of the full-scan path.
        let distance: f64 = row.get(6)?;
        let score = (1.0_f32 - distance as f32).clamp(0.0, 1.0);
        Ok(SearchResult {
            file_path: row.get(0)?,
            language: row.get(1)?,
            content: row.get(2)?,
            start_line: row.get::<_, i64>(3)? as usize,
            end_line: row.get::<_, i64>(4)? as usize,
            source: row.get(5)?,
            score,
            project_id: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
        })
    };

    // KNN subquery: bare `distance` column required — vec0's query planner must
    // see it to honour the LIMIT constraint. COALESCE is applied at the outer
    // SELECT level so zero-vector NULLs are mapped to 1.0 (maximum distance).
    //
    // `{limit}` / `{inner_limit}` are Rust format-interpolated (not `?N` bound
    // params) because sqlite-vec treats a bound `?N` in the KNN subquery as a
    // plain SQL LIMIT, bypassing the KNN-k optimisation entirely. The k value
    // must be a literal in the prepared SQL text. `limit` is a `usize` so there
    // is no injection risk.
    //
    // For source-filtered paths we over-fetch from vec0 (inner_limit = limit * 4)
    // because the WHERE clause at the outer level discards non-matching rows
    // *after* the KNN has already capped at `limit`. Without over-fetching,
    // a request for 10 results filtered to "lib:foo" could return fewer than 10
    // even when more matching rows exist. The outer LIMIT then re-caps to `limit`.
    // Factor of 4 matches the strategy already used in `search_memories`.
    let inner_limit = (limit * 4).max(20);

    let knn_exact = format!(
        "SELECT rowid, distance FROM chunk_embeddings \
         WHERE embedding MATCH vec_f32(?1) ORDER BY distance LIMIT {limit}"
    );
    let knn_over = format!(
        "SELECT rowid, distance FROM chunk_embeddings \
         WHERE embedding MATCH vec_f32(?1) ORDER BY distance LIMIT {inner_limit}"
    );

    let sel_exact = format!(
        "SELECT c.file_path, c.language, c.content, c.start_line, c.end_line, c.source, \
         COALESCE(knn.distance, 1.0) AS distance, c.project_id \
         FROM chunks c JOIN ({knn_exact}) knn ON c.id = knn.rowid"
    );
    let sel_over = format!(
        "SELECT c.file_path, c.language, c.content, c.start_line, c.end_line, c.source, \
         COALESCE(knn.distance, 1.0) AS distance, c.project_id \
         FROM chunks c JOIN ({knn_over}) knn ON c.id = knn.rowid"
    );

    match source_filter {
        None => {
            let sql = format!("{sel_exact} ORDER BY distance ASC");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(params![query_blob], map_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        }
        Some("libraries") => {
            let sql =
                format!("{sel_over} WHERE c.source != 'project' ORDER BY distance ASC LIMIT ?2");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(params![query_blob, limit as i64], map_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        }
        Some(source) => {
            let sql = format!("{sel_over} WHERE c.source = ?2 ORDER BY distance ASC LIMIT ?3");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(params![query_blob, source, limit as i64], map_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        }
    }
}

pub fn search_multi_db(
    project_root: &Path,
    query_embedding: &[f32],
    limit: usize,
    scope: &crate::library::scope::Scope,
    library_registry: &crate::library::registry::LibraryRegistry,
    project_filter: Option<&str>,
) -> Result<Vec<SearchResult>> {
    let mut db_paths: Vec<PathBuf> = Vec::new();

    match scope {
        crate::library::scope::Scope::Project => {
            db_paths.push(project_db_path(project_root));
        }
        crate::library::scope::Scope::Library(name) => {
            if library_registry.lookup(name).is_none() {
                let available: Vec<&str> = library_registry
                    .all()
                    .iter()
                    .map(|e| e.name.as_str())
                    .collect();
                let hint = if available.is_empty() {
                    "No libraries are registered. Use library(action='register') to add one."
                        .to_string()
                } else {
                    format!(
                        "Available libraries: {}. Use one of these as the scope.",
                        available.join(", ")
                    )
                };
                return Err(crate::tools::RecoverableError::with_hint(
                    format!("library '{}' is not registered", name),
                    hint,
                )
                .into());
            }
            let p = lib_db_path(project_root, name);
            if p.exists() {
                db_paths.push(p);
            }
        }
        crate::library::scope::Scope::Libraries => {
            let lib_dir = project_root.join(".codescout/embeddings/lib");
            if lib_dir.is_dir() {
                for entry in std::fs::read_dir(&lib_dir)?.flatten() {
                    let p = entry.path();
                    if p.extension().is_some_and(|e| e == "db") {
                        db_paths.push(p);
                    }
                }
            }
        }
        crate::library::scope::Scope::All => {
            db_paths.push(project_db_path(project_root));
            let lib_dir = project_root.join(".codescout/embeddings/lib");
            if lib_dir.is_dir() {
                for entry in std::fs::read_dir(&lib_dir)?.flatten() {
                    let p = entry.path();
                    if p.extension().is_some_and(|e| e == "db") {
                        db_paths.push(p);
                    }
                }
            }
        }
    }

    // Over-fetch candidates, then apply a per-file cap for diversity.
    //
    // Motivation: fine-grained AST chunks + metadata-enriched embeddings cause
    // one highly-matched file to saturate top-K with sibling methods, crowding
    // out other relevant files. Production systems (Cursor, Continue.dev) and
    // the research literature (arXiv:2510.20609, "Practical Code RAG at Scale")
    // resolve this with a per-source cap in post-processing. Oversample 3×
    // so there are enough candidates to fill `limit` after capping; keep the
    // 4× factor already used when a project filter drops rows.
    const MAX_PER_FILE: usize = 2;
    const OVERSAMPLE: usize = 3;
    let fetch_limit = if project_filter.is_some() {
        (limit * 4).max(20)
    } else {
        (limit * OVERSAMPLE).max(20)
    };

    let mut all_results: Vec<SearchResult> = Vec::new();

    for path in &db_paths {
        if !path.exists() {
            continue;
        }
        match Connection::open(path) {
            Ok(conn) => match search(&conn, query_embedding, fetch_limit) {
                Ok(results) => all_results.extend(results),
                Err(e) => {
                    tracing::warn!("Search failed for {}: {}", path.display(), e);
                }
            },
            Err(e) => {
                tracing::warn!("Failed to open {}: {}", path.display(), e);
            }
        }
    }

    // Apply project filter as a post-filter on the merged result set.
    if let Some(proj) = project_filter {
        all_results.retain(|r| r.project_id == proj);
    }

    // Sort by score descending so the per-file cap keeps the best chunks per file.
    all_results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Per-file cap: at most MAX_PER_FILE chunks from any single file.
    let mut per_file: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut capped: Vec<SearchResult> = Vec::with_capacity(limit);
    for r in all_results.into_iter() {
        if capped.len() >= limit {
            break;
        }
        let count = per_file.entry(r.file_path.clone()).or_insert(0);
        if *count < MAX_PER_FILE {
            *count += 1;
            capped.push(r);
        }
    }

    Ok(capped)
}

fn bytes_to_f32(bytes: &[u8]) -> Result<Vec<f32>> {
    if bytes.len() % 4 != 0 {
        anyhow::bail!(
            "embedding blob size {} is not aligned to 4 bytes",
            bytes.len()
        );
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|b| {
            f32::from_le_bytes(
                b.try_into()
                    .expect("chunks_exact(4) guarantees 4-byte slices"),
            )
        })
        .collect())
}

/// Result of change detection: which files need re-indexing and which were deleted.
#[derive(Debug)]
pub struct ChangeSet {
    /// Relative paths of files that need re-indexing (new or modified).
    pub changed: Vec<String>,
    /// Relative paths of files that were deleted and purged from the index.
    pub deleted: Vec<String>,
}

/// Detect which files changed since the last index, using the fallback chain:
/// 1. Git diff from last_indexed_commit to HEAD (tracked files)
/// 2. Mtime comparison (untracked or when git diff unavailable)
/// 3. SHA-256 hash as final arbiter
///
/// If `force` is true, returns all indexable files as changed.
pub fn find_changed_files(
    conn: &Connection,
    project_root: &Path,
    force: bool,
) -> Result<ChangeSet> {
    use crate::ast::detect_language;

    let config = crate::config::ProjectConfig::load_or_default(project_root)?;
    let ignored = config.ignored_paths.patterns.clone();

    // Walk all eligible files
    let walker = ignore::WalkBuilder::new(project_root)
        .hidden(true)
        .git_ignore(true)
        .filter_entry(move |entry| {
            let name = entry.file_name().to_string_lossy();
            !ignored.iter().any(|p| p.as_str() == name.as_ref())
        })
        .build();

    let mut all_files: Vec<String> = Vec::new();
    for entry in walker.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if detect_language(path).is_none() {
            continue;
        }
        let rel = path
            .strip_prefix(project_root)?
            .to_string_lossy()
            .replace('\\', "/");
        all_files.push(rel);
    }

    if force {
        return Ok(ChangeSet {
            changed: all_files,
            deleted: Vec::new(),
        });
    }

    // Try git-diff approach first
    let git_changed = try_git_diff_detection(conn, project_root);

    let mut changed = Vec::new();
    let mut deleted = Vec::new();

    if let Some(git_result) = git_changed {
        // Git told us which tracked files changed
        let git_changed_set: std::collections::HashSet<&str> =
            git_result.changed.iter().map(|s| s.as_str()).collect();
        let git_deleted_set: std::collections::HashSet<&str> =
            git_result.deleted.iter().map(|s| s.as_str()).collect();

        // Purge deleted files
        for path in &git_result.deleted {
            delete_file_chunks(conn, path)?;
            deleted.push(path.clone());
        }

        for rel in &all_files {
            if git_changed_set.contains(rel.as_str()) {
                // Git says changed -> trust it
                changed.push(rel.clone());
            } else if git_deleted_set.contains(rel.as_str()) {
                continue;
            } else {
                // Not in git diff -> check if it's untracked/new via mtime
                if is_file_changed_mtime_hash(conn, project_root, rel)? {
                    changed.push(rel.clone());
                }
            }
        }
    } else {
        // No git diff available -> fall back to mtime + hash for everything
        for rel in &all_files {
            if is_file_changed_mtime_hash(conn, project_root, rel)? {
                changed.push(rel.clone());
            }
        }
    }

    // Purge files in DB but not on disk (deleted untracked files)
    let purged = purge_missing_files(conn, project_root)?;
    if purged > 0 {
        tracing::debug!("Purged {} missing files from index", purged);
    }

    Ok(ChangeSet { changed, deleted })
}

struct GitDiffResult {
    changed: Vec<String>,
    deleted: Vec<String>,
}

/// Try to use git diff for change detection. Returns None if unavailable.
fn try_git_diff_detection(conn: &Connection, project_root: &Path) -> Option<GitDiffResult> {
    let last_commit = get_last_indexed_commit(conn).ok()??;
    let repo = crate::git::open_repo(project_root).ok()?;
    let head = repo.head().ok()?.peel_to_commit().ok()?;
    let head_sha = head.id().to_string();

    if last_commit == head_sha {
        return Some(GitDiffResult {
            changed: Vec::new(),
            deleted: Vec::new(),
        });
    }

    let entries = crate::git::diff_tree_to_tree(&repo, &last_commit, &head_sha).ok()?;

    let mut changed = Vec::new();
    let mut deleted = Vec::new();
    for entry in entries {
        match entry.status {
            crate::git::DiffStatus::Added | crate::git::DiffStatus::Modified => {
                changed.push(entry.path);
            }
            crate::git::DiffStatus::Deleted => {
                deleted.push(entry.path);
            }
            crate::git::DiffStatus::Renamed { ref old_path } => {
                deleted.push(old_path.clone());
                changed.push(entry.path);
            }
        }
    }
    Some(GitDiffResult { changed, deleted })
}

fn is_file_changed_mtime_hash(conn: &Connection, project_root: &Path, rel: &str) -> Result<bool> {
    let full_path = project_root.join(rel);
    let current_mtime = file_mtime(&full_path)?;
    let stored_mtime = get_file_mtime(conn, rel)?;

    // Fast path: if mtime matches stored value, assume unchanged (cheap check).
    // Mtime is the pre-filter; SHA-256 is only used when mtime differs.
    if Some(current_mtime) == stored_mtime {
        return Ok(false);
    }

    // Mtime differs or no stored mtime → hash to confirm actual content change
    let current_hash = hash_file(&full_path)?;
    let stored_hash = get_file_hash(conn, rel)?;

    Ok(stored_hash.as_deref() != Some(current_hash.as_str()))
}

/// Callback type for `build_index` progress reporting.
/// Arguments: `(done, total, eta_secs)`.
pub type ProgressCb = Box<dyn Fn(usize, usize, Option<u64>) + Send>;

// ── Producer/consumer pipeline types ─────────────────────────────────────────

/// A single file's work item: all the data needed to embed and then write it.
struct FileWork {
    rel: String,
    hash: String,
    mtime: i64,
    lang: String,
    chunks: Vec<super::chunker::RawChunk>,
}

/// One batch of files, ready to be written to the DB after embedding.
struct GroupReady {
    works: Vec<FileWork>,
    embeddings: Vec<codescout_embed::Embedding>,
}

// ── embed_producer ────────────────────────────────────────────────────────────

async fn embed_producer(
    works: Vec<FileWork>,
    embedder: std::sync::Arc<dyn codescout_embed::Embedder>,
    tx: tokio::sync::mpsc::Sender<GroupReady>,
    progress_cb: Option<ProgressCb>,
    total_files: usize,
    file_group_size: usize,
    max_inflight: usize,
) -> anyhow::Result<()> {
    use codescout_embed::Embedding;
    use std::sync::Arc;
    use tokio::sync::Semaphore;
    use tokio::task::JoinSet;

    const BATCH_SIZE: usize = 32;

    let embed_start = std::time::Instant::now();
    let mut files_embedded_so_far = 0usize;

    let mut works_iter = works.into_iter();
    loop {
        let group: Vec<FileWork> = works_iter.by_ref().take(file_group_size).collect();
        if group.is_empty() {
            break;
        }

        // Flatten chunks for this group only
        let mut flat_texts: Vec<String> = Vec::new();
        let mut file_chunk_counts: Vec<usize> = Vec::new();
        for work in &group {
            file_chunk_counts.push(work.chunks.len());
            for chunk in &work.chunks {
                flat_texts.push(match &chunk.metadata {
                    Some(m) => format!("{m}\n{}", chunk.content),
                    None => chunk.content.clone(),
                });
            }
        }
        let total_chunks = flat_texts.len();
        let total_batches = total_chunks.div_ceil(BATCH_SIZE);

        // Spawn embedding tasks for this group
        let sem = Arc::new(Semaphore::new(max_inflight));
        let mut tasks: JoinSet<anyhow::Result<(usize, Vec<Embedding>)>> = JoinSet::new();
        for (batch_idx, chunk) in flat_texts.chunks(BATCH_SIZE).enumerate() {
            let batch: Vec<String> = chunk.to_vec();
            let embedder = Arc::clone(&embedder);
            let sem = Arc::clone(&sem);
            tasks.spawn(async move {
                let _permit = sem
                    .acquire()
                    .await
                    .map_err(|_| anyhow::anyhow!("semaphore unexpectedly closed"))?;
                let refs: Vec<&str> = batch.iter().map(|s| s.as_str()).collect();
                let embeddings = embedder.embed(&refs).await?;
                Ok((batch_idx, embeddings))
            });
        }
        drop(flat_texts);

        // Cumulative chunk boundaries for progress reporting within this group
        let mut boundaries: Vec<usize> = Vec::with_capacity(file_chunk_counts.len());
        let mut cumul = 0;
        for &count in &file_chunk_counts {
            cumul += count;
            boundaries.push(cumul);
        }
        drop(file_chunk_counts);

        // Collect results in batch order
        let mut batch_results: Vec<Option<Vec<Embedding>>> = vec![None; total_batches];
        let mut batches_done = 0usize;
        while let Some(res) = tasks.join_next().await {
            let (idx, embs) = res.map_err(|e| anyhow::anyhow!(e))??;
            batch_results[idx] = Some(embs);
            batches_done += 1;

            if let Some(cb) = &progress_cb {
                let chunks_done_in_group = batches_done * BATCH_SIZE;
                let group_files_done = boundaries
                    .iter()
                    .filter(|&&b| b <= chunks_done_in_group)
                    .count();
                let files_done = files_embedded_so_far + group_files_done;
                let remaining = total_files.saturating_sub(files_done);
                let eta = (files_done > 0 && remaining > 0).then(|| {
                    let elapsed = embed_start.elapsed().as_secs_f64();
                    (elapsed / files_done as f64 * remaining as f64) as u64
                });
                cb(files_done, total_files, eta);
            }
        }

        // Test hook: simulate slow embedding so the overlap test can measure that
        // the writer runs concurrently.  Uses std::thread::sleep (not async sleep)
        // so the OS-level block is immune to tokio scheduler jitter under load.
        #[cfg(test)]
        if let Ok(ms_str) = std::env::var("CODESCOUT_TEST_EMBED_DELAY_MS") {
            if let Ok(ms) = ms_str.parse::<u64>() {
                std::thread::sleep(std::time::Duration::from_millis(ms));
            }
        }

        // Flatten embeddings in batch order
        let embeddings: Vec<Embedding> = batch_results
            .into_iter()
            .flat_map(|b| b.unwrap_or_default())
            .collect();

        files_embedded_so_far += group.len();

        // Send to writer; if writer has dropped rx (error path), stop early
        if tx
            .send(GroupReady {
                works: group,
                embeddings,
            })
            .await
            .is_err()
        {
            break;
        }
    }
    Ok(())
}

// ── db_writer ─────────────────────────────────────────────────────────────────

/// Receives `GroupReady` messages from `embed_producer` and writes each group
/// to the DB in its own transaction.  When the channel closes the writer runs
/// the finalize block (anchor staleness, meta, last-indexed-commit).
///
/// Returns `(indexed, drift_results)`.
///
/// Test hook: when the environment variable `CODESCOUT_TEST_WRITE_DELAY_MS` is
/// set to a positive integer, each group write is preceded by a
/// `std::thread::sleep` of that many milliseconds.  Uses a blocking sleep so
/// the delay is immune to tokio scheduler jitter under parallel test load.
/// Only compiled in `#[cfg(test)]` builds.
async fn db_writer(
    mut rx: tokio::sync::mpsc::Receiver<GroupReady>,
    conn: rusqlite::Connection,
    config: crate::config::ProjectConfig,
    project_root: std::path::PathBuf,
    discovered_projects: Vec<crate::workspace::DiscoveredProject>,
) -> anyhow::Result<(usize, Vec<crate::embed::drift::FileDrift>)> {
    let mut indexed = 0usize;
    let mut drift_results: Vec<crate::embed::drift::FileDrift> = Vec::new();
    let mut embedding_dims_set = false;

    while let Some(group) = rx.recv().await {
        // Test hook: simulate slow DB writes so the overlap test can detect that
        // the producer was already embedding the next group while this one wrote.
        // std::thread::sleep is used intentionally — it blocks the OS thread so
        // the delay cannot be shortened by the tokio scheduler under load.
        #[cfg(test)]
        if let Ok(ms_str) = std::env::var("CODESCOUT_TEST_WRITE_DELAY_MS") {
            if let Ok(ms) = ms_str.parse::<u64>() {
                std::thread::sleep(std::time::Duration::from_millis(ms));
            }
        }

        let GroupReady {
            works: group_works,
            embeddings: flat_embeddings,
        } = group;

        conn.execute_batch("BEGIN")?;
        // Derive embedding dims from first group if not yet known (remote embedder)
        if !embedding_dims_set {
            if let Some(dims) = flat_embeddings.first().map(|e| e.len()) {
                if dims > 0 {
                    set_meta(&conn, "embedding_dims", &dims.to_string())?;
                    embedding_dims_set = true;
                }
            }
        }
        let mut offset = 0;
        for work in group_works {
            let n = work.chunks.len();
            let embeddings = &flat_embeddings[offset..offset + n];
            offset += n;

            let old_chunks = if config.embeddings.drift_detection_enabled {
                read_file_embeddings(&conn, &work.rel)?
            } else {
                Vec::new()
            };
            delete_file_chunks(&conn, &work.rel)?;
            let project_id = crate::workspace::resolve_project_id(
                &discovered_projects,
                &project_root,
                std::path::Path::new(&work.rel),
            );
            for (raw, emb) in work.chunks.iter().zip(embeddings.iter()) {
                let chunk = CodeChunk {
                    id: None,
                    file_path: work.rel.clone(),
                    language: work.lang.clone(),
                    content: raw.content.clone(),
                    start_line: raw.start_line,
                    end_line: raw.end_line,
                    file_hash: work.hash.clone(),
                    source: "project".into(),
                    project_id: project_id.clone(),
                    metadata: raw.metadata.clone(),
                };
                insert_chunk(&conn, &chunk, emb)?;
            }
            upsert_file_hash(&conn, &work.rel, &work.hash, Some(work.mtime))?;

            if config.embeddings.drift_detection_enabled && !old_chunks.is_empty() {
                let new_chunks: Vec<crate::embed::drift::NewChunk> = work
                    .chunks
                    .iter()
                    .zip(embeddings.iter())
                    .map(|(raw, emb)| crate::embed::drift::NewChunk {
                        content: raw.content.clone(),
                        embedding: emb.clone(),
                    })
                    .collect();
                let drift = crate::embed::drift::compute_file_drift(
                    &conn,
                    &work.rel,
                    &old_chunks,
                    &new_chunks,
                )?;
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

            tracing::debug!("indexed {} ({} chunks)", work.rel, work.chunks.len());
            indexed += 1;
        }
        conn.execute_batch("COMMIT")?;
    }

    // Finalize: anchor staleness + metadata committed together
    conn.execute_batch("BEGIN")?;
    if config.embeddings.drift_detection_enabled {
        ensure_memory_anchors(&conn)?;
        let staleness_threshold = config.memory.staleness_drift_threshold;
        for drift in &drift_results {
            if drift.avg_drift >= staleness_threshold {
                let _ = mark_anchors_stale_for_file(&conn, &drift.file_path);
            }
        }
    }
    set_meta(&conn, "embed_model", &config.embeddings.model)?;
    set_meta(&conn, "last_indexed_at", &utc_now_display())?;

    if let Ok(repo) = crate::git::open_repo(&project_root) {
        if let Ok(head) = repo.head() {
            if let Ok(commit) = head.peel_to_commit() {
                set_last_indexed_commit(&conn, &commit.id().to_string())?;
            }
        }
    }
    conn.execute_batch("COMMIT")?;

    // If vec0 was dropped for a remote embedder (dims unknown at start),
    // Phase 3 has now stored the real dims — migrate to vec0 so semantic
    // search works immediately without waiting for next open_db.
    if !is_vec0_active(&conn) {
        maybe_migrate_to_vec0(&conn)?;
    }

    Ok((indexed, drift_results))
}

// ── lib_db_writer ─────────────────────────────────────────────────────────────

/// DB writer for `build_library_index`.  Simpler than `db_writer`: no drift
/// detection, no workspace project-id resolution, no staleness anchors.
/// Returns the count of indexed files.
async fn lib_db_writer(
    mut rx: tokio::sync::mpsc::Receiver<GroupReady>,
    conn: rusqlite::Connection,
    config: crate::config::ProjectConfig,
    source: String,
) -> anyhow::Result<usize> {
    let mut indexed = 0usize;
    let mut embedding_dims_set = false;

    while let Some(group) = rx.recv().await {
        let GroupReady {
            works: group_works,
            embeddings: flat_embeddings,
        } = group;

        conn.execute_batch("BEGIN")?;
        if !embedding_dims_set {
            if let Some(dims) = flat_embeddings.first().map(|e| e.len()) {
                if dims > 0 {
                    set_meta(&conn, "embedding_dims", &dims.to_string())?;
                    embedding_dims_set = true;
                }
            }
        }
        let mut offset = 0;
        for work in group_works {
            let n = work.chunks.len();
            let embeddings = &flat_embeddings[offset..offset + n];
            offset += n;

            delete_file_chunks(&conn, &work.rel)?;
            for (raw, emb) in work.chunks.iter().zip(embeddings.iter()) {
                let chunk = CodeChunk {
                    id: None,
                    file_path: work.rel.clone(),
                    language: work.lang.clone(),
                    content: raw.content.clone(),
                    start_line: raw.start_line,
                    end_line: raw.end_line,
                    file_hash: work.hash.clone(),
                    source: source.clone(),
                    project_id: "root".into(),
                    metadata: raw.metadata.clone(),
                };
                insert_chunk(&conn, &chunk, emb)?;
            }
            upsert_file_hash(&conn, &work.rel, &work.hash, None)?;
            tracing::debug!("indexed {} ({} chunks)", work.rel, work.chunks.len());
            indexed += 1;
        }
        conn.execute_batch("COMMIT")?;
    }

    // Finalize metadata
    conn.execute_batch("BEGIN")?;
    set_meta(&conn, "embed_model", &config.embeddings.model)?;
    set_meta(&conn, "last_indexed_at", &utc_now_display())?;
    conn.execute_batch("COMMIT")?;

    Ok(indexed)
}

pub async fn build_index(
    project_root: &Path,
    force: bool,
    progress_cb: Option<ProgressCb>,
) -> Result<IndexReport> {
    use crate::config::ProjectConfig;
    use codescout_embed::create_embedder_with_config;
    use std::sync::Arc;

    let config = ProjectConfig::load_or_default(project_root)?;
    let conn = open_db(project_root)?;
    if !force {
        check_model_mismatch(&conn, &config.embeddings.model)?;
    }
    let embedder: Arc<dyn codescout_embed::Embedder> = Arc::from(
        create_embedder_with_config(
            &config.embeddings.model,
            config.embeddings.url.as_deref(),
            config
                .embeddings
                .api_key
                .as_ref()
                .map(|k| k.as_str().to_string()),
        )
        .await?,
    );

    // When force-rebuilding with a different model, the vec0 table may have
    // the wrong dimensionality. Detect this and recreate it.
    if force {
        let new_dims = embedder.dimensions();
        let old_dims: Option<usize> =
            get_meta(&conn, "embedding_dims")?.and_then(|s| s.parse().ok());
        if new_dims == 0 && old_dims.is_some() && is_vec0_active(&conn) {
            // Remote embedder: dimensions unknown until first embed response.
            // Drop vec0 (wrong dims would reject new vectors) and fall back to
            // regular blob table.  Real dims are derived after Phase 2 and vec0
            // is recreated via maybe_migrate_to_vec0 at the end of this function.
            tracing::info!(
                "Remote model (dims unknown at construction), dropping vec0 for re-derivation"
            );
            conn.execute_batch("DROP TABLE IF EXISTS chunk_embeddings")?;
            conn.execute_batch(
                "CREATE TABLE chunk_embeddings (rowid INTEGER PRIMARY KEY, embedding BLOB NOT NULL)",
            )?;
            conn.execute("DELETE FROM meta WHERE key = 'embedding_dims'", [])?;
        } else if old_dims.is_some_and(|d| d != new_dims) && new_dims > 0 && is_vec0_active(&conn) {
            // Local embedder: dimensions known, recreate vec0 with new dims.
            tracing::info!(
                "Dimension change detected ({} → {}), recreating vec0 table",
                old_dims.unwrap(),
                new_dims
            );
            conn.execute_batch("DROP TABLE IF EXISTS chunk_embeddings")?;
            conn.execute_batch(&format!(
                "CREATE VIRTUAL TABLE chunk_embeddings USING vec0(\
                 embedding float[{new_dims}] distance_metric=cosine)"
            ))?;
            set_meta(&conn, "embedding_dims", &new_dims.to_string())?;
        }
        // Update model name early so a crash mid-index doesn't leave stale metadata
        set_meta(&conn, "embed_model", &config.embeddings.model)?;
    }

    // ── Phase 1: Detect changed files ─────────────────────────────────────────
    let change_set = find_changed_files(&conn, project_root, force)?;

    // Discover workspace sub-projects for project_id tagging
    let discovered_projects = crate::workspace::discover_projects(project_root, 3, &[]);

    let mut works: Vec<FileWork> = Vec::new();

    for rel in &change_set.changed {
        let path = project_root.join(rel);
        let Some(lang) = crate::ast::detect_language(&path) else {
            continue;
        };
        let hash = hash_file(&path)?;

        let source = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let chunks = super::ast_chunker::split_file(
            &source,
            lang,
            Path::new(rel),
            config.embeddings.effective_chunk_size(),
        );
        if chunks.is_empty() {
            continue;
        }

        works.push(FileWork {
            rel: rel.clone(),
            hash,
            mtime: file_mtime(&path).unwrap_or(0),
            lang: lang.to_string(),
            chunks,
        });
    }

    // ── Phase 2+3: Pipeline embed + write across groups ────────────────────────
    // embed_producer and db_writer run concurrently: while the writer commits
    // group N, the producer is already embedding group N+1 (overlap).
    // Channel capacity 1 provides one slot of buffering — enough for overlap
    // without wasting RAM on more than one group of embeddings at a time.
    let file_group_size = config.embeddings.effective_file_group_size();
    let max_inflight = config.embeddings.effective_max_inflight();

    // Always clear drift data so stale rows don't persist when the feature is toggled off
    conn.execute_batch("BEGIN")?;
    clear_drift_report(&conn)?;
    conn.execute_batch("COMMIT")?;

    let total_files = works.len();

    // Cancel-safety: When embed_producer returns or is cancelled, its local tx drops →
    // channel closes → writer sees rx.recv() == None → runs finalize + exits cleanly.
    // If build_index future is dropped (caller cancellation), the JoinHandle drop does NOT
    // cancel the spawned writer; it finishes in-flight groups, drains queued groups, runs
    // finalize, then exits naturally with DB lock released. Consistent with our JoinHandle drop posture.
    let (tx, rx) = tokio::sync::mpsc::channel::<GroupReady>(1);
    let project_root_buf = project_root.to_path_buf();
    let config_clone = config.clone();
    let writer = tokio::spawn(db_writer(
        rx,
        conn,
        config_clone,
        project_root_buf,
        discovered_projects,
    ));

    let embed_result = embed_producer(
        works,
        Arc::clone(&embedder),
        tx,
        progress_cb,
        total_files,
        file_group_size,
        max_inflight,
    )
    .await;

    // tx is dropped when embed_producer returns → writer sees channel close → runs finalize
    let writer_result = writer.await.map_err(|e| anyhow::anyhow!(e))?;

    embed_result?; // embed error takes precedence
    let (indexed, drift_results) = writer_result?;

    tracing::info!(
        "Index complete: {} files indexed, {} deleted",
        indexed,
        change_set.deleted.len()
    );
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
}

pub async fn build_library_index(
    project_root: &Path,
    library_path: &Path,
    source: &str,
    force: bool,
) -> Result<()> {
    use crate::ast::detect_language;
    use crate::config::ProjectConfig;
    use codescout_embed::create_embedder_with_config;
    use std::sync::Arc;

    let lib_name = source.strip_prefix("lib:").unwrap_or(source);
    let config = ProjectConfig::load_or_default(project_root)?;
    let conn = open_lib_db(project_root, lib_name)?;
    if !force {
        check_model_mismatch(&conn, &config.embeddings.model)?;
    }
    let embedder: Arc<dyn codescout_embed::Embedder> = Arc::from(
        create_embedder_with_config(
            &config.embeddings.model,
            config.embeddings.url.as_deref(),
            config
                .embeddings
                .api_key
                .as_ref()
                .map(|k| k.as_str().to_string()),
        )
        .await?,
    );

    // ── Phase 1: Walk library path, hash, chunk ───────────────────────────────
    let walker = ignore::WalkBuilder::new(library_path)
        .hidden(true)
        .git_ignore(true)
        .build();

    let mut works: Vec<FileWork> = Vec::new();
    let mut skipped = 0usize;

    for entry in walker.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(lang) = detect_language(path) else {
            continue;
        };

        // Use library-relative paths prefixed with source for uniqueness
        let rel = path
            .strip_prefix(library_path)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let rel = format!("[{}]/{}", source, rel);
        let hash = hash_file(path)?;

        if !force {
            if let Some(stored) = get_file_hash(&conn, &rel)? {
                if stored == hash {
                    skipped += 1;
                    continue;
                }
            }
        }

        let file_source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let chunks = super::ast_chunker::split_file(
            &file_source,
            lang,
            Path::new(&rel),
            config.embeddings.effective_chunk_size(),
        );
        if chunks.is_empty() {
            continue;
        }

        works.push(FileWork {
            rel,
            hash,
            mtime: 0, // libraries track hash only, not mtime
            lang: lang.to_string(),
            chunks,
        });
    }

    // ── Phase 2+3: Pipeline embed + write across groups ────────────────────────
    // Same producer/consumer pattern as build_index: embed(N+1) overlaps with
    // DB write(N), eliminating GPU idle at group boundaries.
    let file_group_size = config.embeddings.effective_file_group_size();
    let max_inflight = config.embeddings.effective_max_inflight();
    let total_files = works.len();
    let source_owned = source.to_string();

    // Cancel-safety: When embed_producer returns or is cancelled, its local tx drops →
    // channel closes → writer sees rx.recv() == None → runs finalize + exits cleanly.
    // If build_library_index future is dropped (caller cancellation), the JoinHandle drop does NOT
    // cancel the spawned writer; it finishes in-flight groups, drains queued groups, runs
    // finalize, then exits naturally with DB lock released. Consistent with our JoinHandle drop posture.
    let (tx, rx) = tokio::sync::mpsc::channel::<GroupReady>(1);
    let writer = tokio::spawn(lib_db_writer(rx, conn, config.clone(), source_owned));

    let embed_result = embed_producer(
        works,
        Arc::clone(&embedder),
        tx,
        None, // no progress callback for library indexing
        total_files,
        file_group_size,
        max_inflight,
    )
    .await;

    let writer_result = writer.await.map_err(|e| anyhow::anyhow!(e))?;

    embed_result?;
    let indexed = writer_result?;

    tracing::info!(
        "Library index complete: {} files indexed, {} unchanged (source={})",
        indexed,
        skipped,
        source
    );
    Ok(())
}

#[derive(Debug)]
pub struct IndexReport {
    pub indexed: usize,
    pub deleted: usize,
    pub skipped_msg: String,
    pub drift: Vec<crate::embed::drift::FileDrift>,
}

/// Statistics about the embedding index.
#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexStats {
    pub file_count: usize,
    pub chunk_count: usize,
    pub embedding_count: usize,
    /// Model string stored at index time, if any.
    pub model: Option<String>,
    /// Human-readable UTC timestamp of the last completed index run (e.g. "2026-03-01 14:22 UTC"), if any.
    pub indexed_at: Option<String>,
}

/// Query index statistics from the database.
pub fn index_stats(conn: &Connection) -> Result<IndexStats> {
    let file_count: usize = conn.query_row("SELECT COUNT(*) FROM files", [], |r| {
        r.get::<_, i64>(0).map(|v| v as usize)
    })?;
    let chunk_count: usize = conn.query_row("SELECT COUNT(*) FROM chunks", [], |r| {
        r.get::<_, i64>(0).map(|v| v as usize)
    })?;
    let embedding_count: usize =
        conn.query_row("SELECT COUNT(*) FROM chunk_embeddings", [], |r| {
            r.get::<_, i64>(0).map(|v| v as usize)
        })?;
    let model = get_meta(conn, "embed_model")?;
    let indexed_at = get_meta(conn, "last_indexed_at")?;
    Ok(IndexStats {
        file_count,
        chunk_count,
        embedding_count,
        model,
        indexed_at,
    })
}

/// Per-source statistics for the embedding index.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SourceStats {
    pub file_count: usize,
    pub chunk_count: usize,
}

/// Query index statistics grouped by source (e.g. "project", "lib:serde").
pub fn index_stats_by_source(
    conn: &Connection,
) -> Result<std::collections::HashMap<String, SourceStats>> {
    let mut stmt = conn.prepare(
        "SELECT source, COUNT(DISTINCT file_path), COUNT(*) FROM chunks GROUP BY source",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)? as usize,
            row.get::<_, i64>(2)? as usize,
        ))
    })?;
    let mut map = std::collections::HashMap::new();
    for row in rows {
        let (source, file_count, chunk_count) = row?;
        map.insert(
            source,
            SourceStats {
                file_count,
                chunk_count,
            },
        );
    }
    Ok(map)
}

/// Read a value from the `meta` key-value table.
pub fn get_meta(conn: &Connection, key: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT value FROM meta WHERE key = ?1")?;
    let mut rows = stmt.query([key])?;
    match rows.next()? {
        Some(row) => Ok(Some(row.get(0)?)),
        None => Ok(None),
    }
}

/// Return an error if the index was built with a different embedding model.
///
/// Call this at the start of `build_index` before processing any files.
/// Returns `Ok(())` when:
///   - no model has been stored yet (first run), OR
///   - the stored model matches `configured`
pub fn check_model_mismatch(conn: &Connection, configured: &str) -> Result<()> {
    match get_meta(conn, "embed_model")? {
        None => Ok(()), // first run
        Some(stored) if stored == configured => Ok(()),
        Some(stored) => anyhow::bail!(
            "Index was built with model '{stored}'.\n\
             Configured model is '{configured}'.\n\
             Delete .codescout/embeddings.db and re-run `index` to rebuild."
        ),
    }
}

/// Write (insert or replace) a value in the `meta` key-value table.
pub fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
        rusqlite::params![key, value],
    )?;
    Ok(())
}

/// Returns the current UTC time as a human-readable string like "2026-03-01 14:22 UTC".
/// Uses only `std::time` to avoid pulling in a chrono dependency.
fn utc_now_display() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mins = (secs / 60) % 60;
    let hours = (secs / 3600) % 24;
    let days_since_epoch = secs / 86400;
    let (year, month, day) = days_to_ymd(days_since_epoch);
    format!("{year:04}-{month:02}-{day:02} {hours:02}:{mins:02} UTC")
}

/// Convert days since the Unix epoch (1970-01-01) to (year, month, day).
/// Algorithm: http://howardhinnant.github.io/date_algorithms.html (civil_from_days)
fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    let z = days as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as u64, m, d)
}

/// Get the SHA of the last commit that was fully indexed.
pub fn get_last_indexed_commit(conn: &Connection) -> Result<Option<String>> {
    get_meta(conn, "last_indexed_commit")
}

/// Record the SHA of the last commit that was fully indexed.
pub fn set_last_indexed_commit(conn: &Connection, sha: &str) -> Result<()> {
    set_meta(conn, "last_indexed_commit", sha)
}

#[derive(Debug, serde::Serialize)]
pub struct Staleness {
    pub stale: bool,
    pub behind_commits: usize,
}

/// Check if the index is behind HEAD.
/// Returns Ok with stale=false if up to date, stale=true with commit count if behind.
/// If no git repo or HEAD doesn't exist, returns stale=true with behind_commits=0.
pub fn check_index_staleness(conn: &Connection, project_root: &Path) -> Result<Staleness> {
    let repo = match crate::git::open_repo(project_root) {
        Ok(r) => r,
        Err(_) => {
            return Ok(Staleness {
                stale: true,
                behind_commits: 0,
            })
        }
    };
    let head_oid = match repo.head() {
        Ok(h) => match h.peel_to_commit() {
            Ok(c) => c.id().to_string(),
            Err(_) => {
                return Ok(Staleness {
                    stale: true,
                    behind_commits: 0,
                })
            }
        },
        Err(_) => {
            return Ok(Staleness {
                stale: true,
                behind_commits: 0,
            })
        }
    };

    let last_indexed = get_last_indexed_commit(conn)?;
    match last_indexed {
        None => Ok(Staleness {
            stale: true,
            behind_commits: 0,
        }),
        Some(ref stored) if stored == &head_oid => Ok(Staleness {
            stale: false,
            behind_commits: 0,
        }),
        Some(ref stored) => {
            let behind = count_commits_between(&repo, stored, &head_oid);
            Ok(Staleness {
                stale: true,
                behind_commits: behind,
            })
        }
    }
}

fn count_commits_between(repo: &git2::Repository, from: &str, to: &str) -> usize {
    let Ok(to_oid) = git2::Oid::from_str(to) else {
        return 0;
    };
    let Ok(from_oid) = git2::Oid::from_str(from) else {
        return 0;
    };
    let Ok(mut revwalk) = repo.revwalk() else {
        return 0;
    };
    if revwalk.push(to_oid).is_err() {
        return 0;
    }
    if revwalk.hide(from_oid).is_err() {
        return 0;
    }
    revwalk.count()
}

/// A row from the drift_report table.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DriftReportRow {
    pub file_path: String,
    pub avg_drift: f32,
    pub max_drift: f32,
    pub max_drift_chunk: Option<String>,
    pub chunks_added: usize,
    pub chunks_removed: usize,
    pub indexed_at: String,
}

/// Insert or update a drift report row for a file.
pub fn upsert_drift_report(
    conn: &Connection,
    file_path: &str,
    avg_drift: f32,
    max_drift: f32,
    max_drift_chunk: Option<&str>,
    chunks_added: usize,
    chunks_removed: usize,
) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string();
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
        params![file_path, avg_drift, max_drift, max_drift_chunk, chunks_added as i64, chunks_removed as i64, now],
    )?;
    Ok(())
}

/// Query drift report rows, optionally filtering by threshold and path glob.
///
/// - `threshold`: minimum `max_drift` value (default 0.0 when None, which means
///   only rows with max_drift > 0 are returned)
/// - `path_glob`: SQL LIKE pattern for file_path filtering
/// - Results sorted by `max_drift` DESC
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

/// Delete all rows from the drift_report table.
pub fn clear_drift_report(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM drift_report", [])?;
    Ok(())
}

// ── Memory anchors ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SemanticAnchor {
    pub file_path: String,
    pub file_hash: String,
    pub similarity: f32,
    pub stale: bool,
}

/// Create the `memory_anchors` table if it does not exist.
pub fn ensure_memory_anchors(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS memory_anchors (
            id INTEGER PRIMARY KEY,
            memory_type TEXT NOT NULL,
            memory_key TEXT NOT NULL,
            file_path TEXT NOT NULL,
            file_hash TEXT NOT NULL,
            similarity REAL NOT NULL,
            created_at TEXT NOT NULL,
            stale INTEGER NOT NULL DEFAULT 0,
            UNIQUE(memory_type, memory_key, file_path)
        )",
    )?;
    Ok(())
}

/// Insert or update a semantic anchor for a memory–file pair.
pub fn insert_semantic_anchor(
    conn: &Connection,
    memory_type: &str,
    memory_key: &str,
    file_path: &str,
    file_hash: &str,
    similarity: f32,
) -> Result<()> {
    let now = utc_now_display();
    conn.execute(
        "INSERT INTO memory_anchors (memory_type, memory_key, file_path, file_hash, similarity, created_at, stale)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)
         ON CONFLICT(memory_type, memory_key, file_path) DO UPDATE SET
            file_hash = excluded.file_hash,
            similarity = excluded.similarity,
            created_at = excluded.created_at,
            stale = 0",
        params![memory_type, memory_key, file_path, file_hash, similarity, now],
    )?;
    Ok(())
}

/// Get all semantic anchors for a given memory.
pub fn get_semantic_anchors(
    conn: &Connection,
    memory_type: &str,
    memory_key: &str,
) -> Result<Vec<SemanticAnchor>> {
    let mut stmt = conn.prepare(
        "SELECT file_path, file_hash, similarity, stale
         FROM memory_anchors
         WHERE memory_type = ?1 AND memory_key = ?2",
    )?;
    let rows = stmt.query_map(params![memory_type, memory_key], |row| {
        Ok(SemanticAnchor {
            file_path: row.get(0)?,
            file_hash: row.get(1)?,
            similarity: row.get(2)?,
            stale: row.get::<_, i32>(3)? != 0,
        })
    })?;
    let mut anchors = Vec::new();
    for row in rows {
        anchors.push(row?);
    }
    Ok(anchors)
}

/// Delete all semantic anchors for a given memory.
pub fn delete_semantic_anchors(
    conn: &Connection,
    memory_type: &str,
    memory_key: &str,
) -> Result<()> {
    conn.execute(
        "DELETE FROM memory_anchors WHERE memory_type = ?1 AND memory_key = ?2",
        params![memory_type, memory_key],
    )?;
    Ok(())
}

/// Mark all anchors pointing to a file as stale. Returns count of rows affected.
pub fn mark_anchors_stale_for_file(conn: &Connection, file_path: &str) -> Result<usize> {
    let count = conn.execute(
        "UPDATE memory_anchors SET stale = 1 WHERE file_path = ?1",
        params![file_path],
    )?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::schema::CodeChunk;
    use rusqlite::OptionalExtension;
    use tempfile::tempdir;

    fn open_test_db() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();
        (dir, conn)
    }

    fn open_test_db_vec0(dims: usize) -> (tempfile::TempDir, Connection) {
        let (dir, conn) = open_test_db();
        set_meta(&conn, "embedding_dims", &dims.to_string()).unwrap();
        maybe_migrate_to_vec0(&conn).unwrap();
        (dir, conn)
    }

    #[test]
    fn vec0_migration_skips_when_no_dims() {
        let (_dir, conn) = open_test_db();
        // No embedding_dims in meta → migration is a no-op, plain table stays
        maybe_migrate_to_vec0(&conn).unwrap();
        let sql: Option<String> = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name='chunk_embeddings'",
                [],
                |r| r.get(0),
            )
            .optional()
            .unwrap();
        let sql = sql.unwrap();
        assert!(
            !sql.contains("USING vec0"),
            "expected plain table, got: {sql}"
        );
    }

    #[test]
    fn vec0_migration_upgrades_plain_table() {
        let (_dir, conn) = open_test_db();
        // Insert a chunk so there is data to migrate
        insert_chunk(
            &conn,
            &dummy_chunk("a.rs", "fn a() {}"),
            &[0.1_f32, 0.2_f32],
        )
        .unwrap();
        set_meta(&conn, "embedding_dims", "2").unwrap();

        maybe_migrate_to_vec0(&conn).unwrap();

        let sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name='chunk_embeddings'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            sql.contains("USING vec0"),
            "expected vec0 virtual table, got: {sql}"
        );

        // Data must survive the migration
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM chunk_embeddings", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn vec0_migration_is_idempotent() {
        let (_dir, conn) = open_test_db();
        insert_chunk(
            &conn,
            &dummy_chunk("a.rs", "fn a() {}"),
            &[0.1_f32, 0.2_f32],
        )
        .unwrap();
        set_meta(&conn, "embedding_dims", "2").unwrap();

        maybe_migrate_to_vec0(&conn).unwrap();
        // Second call must not error
        maybe_migrate_to_vec0(&conn).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM chunk_embeddings", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn vec0_migration_ddl_preserves_data() {
        let (_dir, conn) = open_test_db();
        insert_chunk(
            &conn,
            &dummy_chunk("a.rs", "fn a() {}"),
            &[0.1_f32, 0.2_f32],
        )
        .unwrap();
        set_meta(&conn, "embedding_dims", "2").unwrap();
        // Try wrapping the four migration steps in an explicit transaction
        conn.execute_batch("BEGIN").unwrap();
        conn.execute_batch("ALTER TABLE chunk_embeddings RENAME TO chunk_embeddings_v1")
            .unwrap();
        conn.execute_batch("CREATE VIRTUAL TABLE chunk_embeddings USING vec0(embedding float[2])")
            .unwrap();
        conn.execute_batch(
            "INSERT INTO chunk_embeddings(rowid, embedding) \
             SELECT rowid, embedding FROM chunk_embeddings_v1",
        )
        .unwrap();
        conn.execute_batch("DROP TABLE chunk_embeddings_v1")
            .unwrap();
        conn.execute_batch("COMMIT").unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM chunk_embeddings", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    fn dummy_chunk(file_path: &str, content: &str) -> CodeChunk {
        CodeChunk {
            id: None,
            file_path: file_path.to_string(),
            language: "rust".to_string(),
            content: content.to_string(),
            start_line: 1,
            end_line: 3,
            file_hash: "testhash".to_string(),
            source: "project".into(),
            project_id: "root".into(),
            metadata: None,
        }
    }

    fn dummy_chunk_with_source(file_path: &str, content: &str, source: &str) -> CodeChunk {
        CodeChunk {
            id: None,
            file_path: file_path.to_string(),
            language: "rust".to_string(),
            content: content.to_string(),
            start_line: 1,
            end_line: 3,
            file_hash: "testhash".to_string(),
            source: source.to_string(),
            project_id: "root".into(),
            metadata: None,
        }
    }

    #[test]
    fn old_schema_without_metadata_triggers_rebuild() {
        let dir = tempfile::tempdir().unwrap();
        let project_root = dir.path();
        let db_path = project_db_path(project_root);
        std::fs::create_dir_all(db_path.parent().unwrap()).unwrap();

        // Create a legacy DB without metadata column and without schema_version.
        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE chunks (
                    id INTEGER PRIMARY KEY,
                    file_path TEXT NOT NULL,
                    language TEXT NOT NULL,
                    content TEXT NOT NULL,
                    start_line INTEGER NOT NULL,
                    end_line INTEGER NOT NULL,
                    file_hash TEXT NOT NULL,
                    source TEXT NOT NULL DEFAULT 'project'
                );
                INSERT INTO chunks (file_path, language, content, start_line, end_line, file_hash)
                 VALUES ('a.rs', 'rust', 'fn x(){}', 1, 1, 'abc');",
            )
            .unwrap();
        }

        // Open via production path; it should detect missing version, drop & recreate.
        let conn = open_db(project_root).expect("open_db should migrate");

        // Verify new column exists.
        let cols: Vec<String> = conn
            .prepare("PRAGMA table_info(chunks)")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(
            cols.iter().any(|c| c == "metadata"),
            "metadata column missing after migration: cols={cols:?}"
        );

        // Old row is gone (table was dropped).
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0, "old rows should be dropped on schema migration");
    }

    #[test]
    fn open_db_creates_tables() {
        let (_dir, conn) = open_test_db();
        let files: i64 = conn
            .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))
            .unwrap();
        let chunks: i64 = conn
            .query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(files, 0);
        assert_eq!(chunks, 0);
    }

    #[test]
    fn open_db_auto_migrates_when_dims_present() {
        let dir = tempfile::tempdir().unwrap();
        let conn1 = open_db(dir.path()).unwrap();
        // Insert a chunk and store dims (simulates a post-indexing state)
        insert_chunk(
            &conn1,
            &dummy_chunk("x.rs", "fn x() {}"),
            &[1.0_f32, 0.0_f32],
        )
        .unwrap();
        set_meta(&conn1, "embedding_dims", "2").unwrap();
        drop(conn1);

        // Re-open — open_db should auto-migrate
        let conn2 = open_db(dir.path()).unwrap();
        let sql: String = conn2
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name='chunk_embeddings'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            sql.contains("USING vec0"),
            "expected vec0 after reopen, got: {sql}"
        );
    }

    /// Verify that vec0 shadow tables ARE properly rolled back on transaction ROLLBACK.
    /// If this test FAILS, it means shadow tables survive ROLLBACK, which is the root cause
    /// of the "chunk_embeddings_info already exists" error on subsequent open_db calls.
    #[test]
    fn vec0_create_virtual_table_rolls_back_shadow_tables_on_rollback() {
        let (_dir, conn) = open_test_db();
        insert_chunk(
            &conn,
            &dummy_chunk("a.rs", "fn a() {}"),
            &[0.1_f32, 0.2_f32],
        )
        .unwrap();
        set_meta(&conn, "embedding_dims", "2").unwrap();

        // Manually replay the migration steps, then ROLLBACK instead of COMMIT
        conn.execute_batch("BEGIN").unwrap();
        conn.execute_batch("ALTER TABLE chunk_embeddings RENAME TO chunk_embeddings_v1")
            .unwrap();
        conn.execute_batch("CREATE VIRTUAL TABLE chunk_embeddings USING vec0(embedding float[2])")
            .unwrap();
        // Rollback WITHOUT committing — simulates a failed migration
        conn.execute_batch("ROLLBACK").unwrap();

        // After ROLLBACK: chunk_embeddings must be the plain table again
        let sql: Option<String> = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name='chunk_embeddings'",
                [],
                |r| r.get(0),
            )
            .optional()
            .unwrap();
        assert!(
            sql.as_deref()
                .map(|s| !s.contains("USING vec0"))
                .unwrap_or(false),
            "chunk_embeddings should be plain table after ROLLBACK, got: {sql:?}"
        );

        // The shadow tables must NOT persist after ROLLBACK
        let info_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name='chunk_embeddings_info'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
            > 0;
        assert!(
            !info_exists,
            "chunk_embeddings_info must not exist after ROLLBACK of CREATE VIRTUAL TABLE"
        );

        // A subsequent migration must succeed (no leftover shadow tables to conflict)
        maybe_migrate_to_vec0(&conn).unwrap();
    }

    /// Regression test: calling open_db on a DB that was already migrated to vec0
    /// must not fail with "table chunk_embeddings_info already exists".
    /// The execute_batch DDL in open_db runs `CREATE TABLE IF NOT EXISTS chunk_embeddings`
    /// every time — it must be a true no-op when chunk_embeddings is already a vec0 VT.
    #[test]
    fn open_db_is_idempotent_after_vec0_migration() {
        let dir = tempfile::tempdir().unwrap();

        // Conn1: plain table setup + dims set (simulates post-index_project state)
        let conn1 = open_db(dir.path()).unwrap();
        insert_chunk(
            &conn1,
            &dummy_chunk("x.rs", "fn x() {}"),
            &[1.0_f32, 0.0_f32],
        )
        .unwrap();
        set_meta(&conn1, "embedding_dims", "2").unwrap();
        drop(conn1);

        // Conn2: open_db triggers migration → chunk_embeddings becomes vec0
        let conn2 = open_db(dir.path()).unwrap();
        drop(conn2);

        // Conn3: open_db on already-migrated DB must NOT error with
        // "Could not create '_info' shadow table: table chunk_embeddings_info already exists"
        let conn3 = open_db(dir.path())
            .expect("third open_db must succeed when chunk_embeddings is already vec0");
        let sql: String = conn3
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name='chunk_embeddings'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            sql.contains("USING vec0"),
            "expected vec0 to persist across opens, got: {sql}"
        );

        // Print all sqlite_master entries to understand vec0 table structure
        let mut stmt = conn3
            .prepare("SELECT type, name, sql FROM sqlite_master ORDER BY type, name")
            .unwrap();
        let rows: Vec<(String, String, Option<String>)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        eprintln!("sqlite_master after 3rd open:");
        for (t, n, s) in &rows {
            eprintln!("  type={t:?} name={n:?} sql={s:?}");
        }
    }

    /// Documents the SQLite behavior that caused the TOCTOU race:
    /// renaming a vec0 virtual table succeeds but does NOT rename its shadow
    /// tables, so a subsequent CREATE VIRTUAL TABLE with the original name
    /// fails because the shadow tables (e.g. chunk_embeddings_info) still
    /// exist.  This test is deterministic and always passes — it serves as a
    /// pinned understanding of the underlying SQLite quirk.
    #[test]
    fn migration_race_loser_exposes_shadow_table_conflict() {
        let dir = tempfile::tempdir().unwrap();

        // Set up: plain table + dims + data (post-index_project state)
        let conn = open_db(dir.path()).unwrap();
        insert_chunk(
            &conn,
            &dummy_chunk("a.rs", "fn a() {}"),
            &[0.1_f32, 0.2_f32],
        )
        .unwrap();
        set_meta(&conn, "embedding_dims", "2").unwrap();
        drop(conn);

        // First connection migrates successfully (plain → vec0)
        let conn_a = open_db(dir.path()).unwrap();
        drop(conn_a);

        // Simulate what the losing connection would do WITHOUT BEGIN IMMEDIATE:
        // it observed "plain table" before the lock, then acquired the write
        // lock after conn_a committed.  At that point chunk_embeddings is
        // already a vec0 VT.  ALTERing it succeeds (SQLite allows renaming VTs
        // since 3.26.0) but shadow tables are NOT renamed.
        let conn_b = open_db(dir.path()).unwrap();
        conn_b
            .execute_batch("ALTER TABLE chunk_embeddings RENAME TO chunk_embeddings_v1")
            .expect("ALTER TABLE on vec0 VT should succeed");

        // The shadow table chunk_embeddings_info still exists under its original
        // name → CREATE VIRTUAL TABLE must fail with the shadow-table conflict.
        let result = conn_b.execute_batch(
            "CREATE VIRTUAL TABLE chunk_embeddings USING vec0(embedding float[2] distance_metric=cosine)",
        );
        assert!(
            result.is_err(),
            "expected shadow-table conflict but CREATE succeeded"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("already exists"),
            "expected 'already exists' error, got: {err}"
        );
    }

    /// Regression test: two threads racing through open_db on a migration-ready
    /// database must both succeed.  Before the BEGIN IMMEDIATE fix the second
    /// thread could corrupt the database and return an error.
    #[test]
    fn concurrent_open_db_migrations_do_not_corrupt() {
        use std::sync::{Arc, Barrier};

        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().to_owned();

        // Set up: plain table + dims + data, then close the connection so the
        // DB is in the "ready to migrate" state (no open connections).
        {
            let conn = open_db(&db_path).unwrap();
            insert_chunk(
                &conn,
                &dummy_chunk("a.rs", "fn a() {}"),
                &[0.1_f32, 0.2_f32],
            )
            .unwrap();
            set_meta(&conn, "embedding_dims", "2").unwrap();
        }

        // Barrier ensures both threads call open_db at the same time, maximising
        // the probability of the TOCTOU race manifesting without the fix.
        let barrier = Arc::new(Barrier::new(2));

        let barrier_a = Arc::clone(&barrier);
        let path_a = db_path.clone();
        let handle_a = std::thread::spawn(move || -> anyhow::Result<()> {
            barrier_a.wait();
            open_db(&path_a).map(|_| ())
        });

        let barrier_b = Arc::clone(&barrier);
        let path_b = db_path.clone();
        let handle_b = std::thread::spawn(move || -> anyhow::Result<()> {
            barrier_b.wait();
            open_db(&path_b).map(|_| ())
        });

        let result_a = handle_a.join().expect("thread A panicked");
        let result_b = handle_b.join().expect("thread B panicked");

        assert!(
            result_a.is_ok(),
            "thread A open_db failed: {:?}",
            result_a.err()
        );
        assert!(
            result_b.is_ok(),
            "thread B open_db failed: {:?}",
            result_b.err()
        );

        // Verify the DB is in the correct final state: vec0 virtual table.
        let conn = open_db(&db_path).unwrap();
        let sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name='chunk_embeddings'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            sql.contains("USING vec0"),
            "expected vec0 after concurrent migration, got: {sql}"
        );
    }

    #[test]
    fn insert_chunk_assigns_row_id() {
        let (_dir, conn) = open_test_db();
        let id = insert_chunk(&conn, &dummy_chunk("a.rs", "fn a() {}"), &[0.1, 0.2]).unwrap();
        assert!(id > 0);
    }

    #[test]
    fn insert_chunk_stores_project_id() {
        let (_dir, conn) = open_test_db();
        let mut chunk = dummy_chunk("src/main.rs", "fn main() {}");
        chunk.project_id = "my-service".to_string();
        insert_chunk(&conn, &chunk, &[0.1]).unwrap();

        let pid: String = conn
            .query_row(
                "SELECT project_id FROM chunks WHERE file_path = 'src/main.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(pid, "my-service");
    }

    #[test]
    fn insert_multiple_chunks_for_same_file() {
        let (_dir, conn) = open_test_db();
        insert_chunk(&conn, &dummy_chunk("f.rs", "chunk 1"), &[0.1]).unwrap();
        insert_chunk(&conn, &dummy_chunk("f.rs", "chunk 2"), &[0.2]).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chunks WHERE file_path='f.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn file_hash_upsert_and_get() {
        let (_dir, conn) = open_test_db();
        upsert_file_hash(&conn, "src/lib.rs", "aabbcc", None).unwrap();
        assert_eq!(
            get_file_hash(&conn, "src/lib.rs").unwrap(),
            Some("aabbcc".to_string())
        );
    }

    #[test]
    fn file_hash_upsert_updates_on_conflict() {
        let (_dir, conn) = open_test_db();
        upsert_file_hash(&conn, "src/lib.rs", "hash1", None).unwrap();
        upsert_file_hash(&conn, "src/lib.rs", "hash2", None).unwrap();
        assert_eq!(
            get_file_hash(&conn, "src/lib.rs").unwrap(),
            Some("hash2".to_string())
        );
    }

    #[test]
    fn get_file_hash_missing_returns_none() {
        let (_dir, conn) = open_test_db();
        assert_eq!(get_file_hash(&conn, "nonexistent.rs").unwrap(), None);
    }

    #[test]
    fn delete_file_chunks_removes_chunks_and_hash() {
        let (_dir, conn) = open_test_db();
        insert_chunk(&conn, &dummy_chunk("del.rs", "fn x() {}"), &[0.5]).unwrap();
        upsert_file_hash(&conn, "del.rs", "abc", None).unwrap();

        delete_file_chunks(&conn, "del.rs").unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chunks WHERE file_path='del.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
        assert_eq!(get_file_hash(&conn, "del.rs").unwrap(), None);
    }

    #[test]
    fn delete_does_not_affect_other_files() {
        let (_dir, conn) = open_test_db();
        insert_chunk(&conn, &dummy_chunk("keep.rs", "fn keep() {}"), &[0.1]).unwrap();
        insert_chunk(&conn, &dummy_chunk("del.rs", "fn del() {}"), &[0.2]).unwrap();

        delete_file_chunks(&conn, "del.rs").unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chunks WHERE file_path='keep.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn cosine_search_returns_closest_vector() {
        let (_dir, conn) = open_test_db();
        // Two orthogonal 4-dim embeddings
        insert_chunk(
            &conn,
            &dummy_chunk("a.rs", "fn a() {}"),
            &[1.0, 0.0, 0.0, 0.0],
        )
        .unwrap();
        insert_chunk(
            &conn,
            &dummy_chunk("b.rs", "fn b() {}"),
            &[0.0, 1.0, 0.0, 0.0],
        )
        .unwrap();

        // Query aligned with a.rs
        let results = search(&conn, &[0.9, 0.1, 0.0, 0.0], 1).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_path, "a.rs");
        assert!(results[0].score > 0.9, "score was {}", results[0].score);
    }

    #[test]
    fn cosine_search_respects_limit() {
        let (_dir, conn) = open_test_db();
        for i in 0..5 {
            insert_chunk(
                &conn,
                &dummy_chunk(&format!("{}.rs", i), "fn f() {}"),
                &[i as f32, 0.0],
            )
            .unwrap();
        }
        let results = search(&conn, &[1.0, 0.0], 3).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn hash_file_produces_64_char_hex() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.rs");
        std::fs::write(&file, b"fn main() {}").unwrap();
        let hash = hash_file(&file).unwrap();
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_file_differs_for_different_content() {
        let dir = tempdir().unwrap();
        let f1 = dir.path().join("a.rs");
        let f2 = dir.path().join("b.rs");
        std::fs::write(&f1, b"fn a() {}").unwrap();
        std::fs::write(&f2, b"fn b() {}").unwrap();
        assert_ne!(hash_file(&f1).unwrap(), hash_file(&f2).unwrap());
    }

    #[test]
    fn open_db_creates_meta_table() {
        let (_dir, conn) = open_test_db();
        // meta table must exist (schema_version is always inserted by open_db)
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM meta", [], |r| r.get(0))
            .unwrap();
        assert!(
            count >= 1,
            "meta table should have at least schema_version row"
        );
        // Verify schema_version is set correctly
        let ver: String = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ver, SCHEMA_VERSION.to_string());
    }

    #[test]
    fn meta_get_missing_key_returns_none() {
        let (_dir, conn) = open_test_db();
        let val = get_meta(&conn, "embed_model").unwrap();
        assert!(val.is_none());
    }

    #[test]
    fn meta_set_then_get_roundtrip() {
        let (_dir, conn) = open_test_db();
        set_meta(&conn, "embed_model", "ollama:mxbai-embed-large").unwrap();
        let val = get_meta(&conn, "embed_model").unwrap();
        assert_eq!(val.as_deref(), Some("ollama:mxbai-embed-large"));
    }

    #[test]
    fn meta_set_overwrites_existing_value() {
        let (_dir, conn) = open_test_db();
        set_meta(&conn, "embed_model", "old-model").unwrap();
        set_meta(&conn, "embed_model", "new-model").unwrap();
        let val = get_meta(&conn, "embed_model").unwrap();
        assert_eq!(val.as_deref(), Some("new-model"));
    }

    #[test]
    fn check_model_mismatch_first_run_is_ok() {
        let (_dir, conn) = open_test_db();
        // No meta entry yet — first run should succeed
        assert!(check_model_mismatch(&conn, "ollama:mxbai-embed-large").is_ok());
    }

    #[test]
    fn check_model_mismatch_same_model_is_ok() {
        let (_dir, conn) = open_test_db();
        set_meta(&conn, "embed_model", "ollama:mxbai-embed-large").unwrap();
        assert!(check_model_mismatch(&conn, "ollama:mxbai-embed-large").is_ok());
    }

    #[test]
    fn check_model_mismatch_different_model_is_err() {
        let (_dir, conn) = open_test_db();
        set_meta(&conn, "embed_model", "ollama:mxbai-embed-large").unwrap();
        let err = check_model_mismatch(&conn, "local:JinaEmbeddingsV2BaseCode")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("ollama:mxbai-embed-large"),
            "error should name stored model"
        );
        assert!(
            err.contains("local:JinaEmbeddingsV2BaseCode"),
            "error should name new model"
        );
        assert!(
            err.contains("embeddings.db"),
            "error should hint at DB deletion"
        );
    }

    #[test]
    fn index_stats_returns_stored_model() {
        let (_dir, conn) = open_test_db();
        set_meta(&conn, "embed_model", "ollama:mxbai-embed-large").unwrap();
        let stats = index_stats(&conn).unwrap();
        assert_eq!(stats.model.as_deref(), Some("ollama:mxbai-embed-large"));
    }

    #[test]
    fn index_stats_model_is_none_when_unset() {
        let (_dir, conn) = open_test_db();
        let stats = index_stats(&conn).unwrap();
        assert!(stats.model.is_none());
    }

    #[test]
    fn normalize_rel_path_uses_forward_slashes() {
        use std::path::PathBuf;
        // Simulate what build_index does: strip prefix + to_string_lossy
        let root = PathBuf::from(if cfg!(windows) {
            "C:\\project"
        } else {
            "/project"
        });
        let file = root.join("src").join("tools").join("file.rs");
        let rel = file
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        assert_eq!(rel, "src/tools/file.rs");
    }

    #[test]
    fn insert_chunk_stores_source() {
        let (_dir, conn) = open_test_db();
        let chunk = dummy_chunk_with_source("lib.rs", "fn x() {}", "lib:serde");
        insert_chunk(&conn, &chunk, &[0.1, 0.2]).unwrap();
        let stored: String = conn
            .query_row(
                "SELECT source FROM chunks WHERE file_path = 'lib.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(stored, "lib:serde");
    }

    #[test]
    fn search_returns_source() {
        let (_dir, conn) = open_test_db();
        insert_chunk(
            &conn,
            &dummy_chunk_with_source("a.rs", "fn a() {}", "lib:tokio"),
            &[1.0, 0.0],
        )
        .unwrap();
        let results = search(&conn, &[1.0, 0.0], 1).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, "lib:tokio");
    }

    #[test]
    fn search_scoped_filters_by_source() {
        let (_dir, conn) = open_test_db();
        // Insert project chunk and library chunk with orthogonal embeddings
        insert_chunk(
            &conn,
            &dummy_chunk("proj.rs", "fn proj() {}"),
            &[1.0, 0.0, 0.0],
        )
        .unwrap();
        insert_chunk(
            &conn,
            &dummy_chunk_with_source("serde.rs", "fn serde() {}", "lib:serde"),
            &[0.0, 1.0, 0.0],
        )
        .unwrap();
        insert_chunk(
            &conn,
            &dummy_chunk_with_source("tokio.rs", "fn tokio() {}", "lib:tokio"),
            &[0.0, 0.0, 1.0],
        )
        .unwrap();

        // No filter → all 3
        let all = search_scoped(&conn, &[1.0, 1.0, 1.0], 10, &SourceScope::All).unwrap();
        assert_eq!(all.len(), 3);

        // Project only
        let proj = search_scoped(&conn, &[1.0, 1.0, 1.0], 10, &SourceScope::Project).unwrap();
        assert_eq!(proj.len(), 1);
        assert_eq!(proj[0].source, "project");

        // Libraries (all non-project)
        let libs = search_scoped(&conn, &[1.0, 1.0, 1.0], 10, &SourceScope::Libraries).unwrap();
        assert_eq!(libs.len(), 2);
        assert!(libs.iter().all(|r| r.source != "project"));

        // Specific library
        let serde_only = search_scoped(
            &conn,
            &[1.0, 1.0, 1.0],
            10,
            &SourceScope::Library("lib:serde".to_string()),
        )
        .unwrap();
        assert_eq!(serde_only.len(), 1);
        assert_eq!(serde_only[0].source, "lib:serde");
    }

    #[test]
    fn upsert_file_hash_stores_mtime() {
        let (_dir, conn) = open_test_db();
        upsert_file_hash(&conn, "a.rs", "abc123", Some(1700000000)).unwrap();
        let mtime: Option<i64> = conn
            .query_row("SELECT mtime FROM files WHERE path = 'a.rs'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(mtime, Some(1700000000));
    }

    #[test]
    fn upsert_file_hash_updates_mtime() {
        let (_dir, conn) = open_test_db();
        upsert_file_hash(&conn, "a.rs", "abc", Some(1000)).unwrap();
        upsert_file_hash(&conn, "a.rs", "def", Some(2000)).unwrap();
        let mtime: Option<i64> = conn
            .query_row("SELECT mtime FROM files WHERE path = 'a.rs'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(mtime, Some(2000));
    }

    #[test]
    fn get_file_mtime_returns_stored_value() {
        let (_dir, conn) = open_test_db();
        upsert_file_hash(&conn, "a.rs", "abc", Some(1700000000)).unwrap();
        assert_eq!(get_file_mtime(&conn, "a.rs").unwrap(), Some(1700000000));
    }

    #[test]
    fn get_file_mtime_returns_none_for_missing() {
        let (_dir, conn) = open_test_db();
        assert_eq!(get_file_mtime(&conn, "missing.rs").unwrap(), None);
    }

    #[test]
    fn index_stats_by_source_groups() {
        let (_dir, conn) = open_test_db();
        insert_chunk(&conn, &dummy_chunk("a.rs", "fn a() {}"), &[0.1]).unwrap();
        insert_chunk(&conn, &dummy_chunk("b.rs", "fn b() {}"), &[0.2]).unwrap();
        insert_chunk(
            &conn,
            &dummy_chunk_with_source("serde.rs", "fn serde() {}", "lib:serde"),
            &[0.3],
        )
        .unwrap();

        let by_source = index_stats_by_source(&conn).unwrap();
        assert_eq!(by_source.len(), 2);

        let project = by_source.get("project").unwrap();
        assert_eq!(project.file_count, 2);
        assert_eq!(project.chunk_count, 2);

        let serde = by_source.get("lib:serde").unwrap();
        assert_eq!(serde.file_count, 1);
        assert_eq!(serde.chunk_count, 1);
    }

    #[test]
    fn last_indexed_commit_roundtrip() {
        let (_dir, conn) = open_test_db();
        assert_eq!(get_last_indexed_commit(&conn).unwrap(), None);
        set_last_indexed_commit(&conn, "abc123def456").unwrap();
        assert_eq!(
            get_last_indexed_commit(&conn).unwrap(),
            Some("abc123def456".to_string())
        );
    }

    #[test]
    fn last_indexed_commit_updates() {
        let (_dir, conn) = open_test_db();
        set_last_indexed_commit(&conn, "aaa").unwrap();
        set_last_indexed_commit(&conn, "bbb").unwrap();
        assert_eq!(
            get_last_indexed_commit(&conn).unwrap(),
            Some("bbb".to_string())
        );
    }

    #[test]
    fn file_mtime_returns_epoch_seconds() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.rs");
        std::fs::write(&file, b"fn main() {}").unwrap();
        let mtime = file_mtime(&file).unwrap();
        // Should be a reasonable epoch timestamp (after 2020)
        assert!(mtime > 1_577_836_800); // 2020-01-01
        assert!(mtime < 2_000_000_000); // ~2033
    }

    #[test]
    fn purge_missing_files_removes_deleted() {
        let dir = tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();

        // Insert entries for two files, but only create one on disk
        let existing = dir.path().join("exists.rs");
        std::fs::write(&existing, "fn a() {}").unwrap();
        insert_chunk(&conn, &dummy_chunk("exists.rs", "fn a() {}"), &[0.5]).unwrap();
        upsert_file_hash(&conn, "exists.rs", "aaa", Some(1000)).unwrap();
        insert_chunk(&conn, &dummy_chunk("gone.rs", "fn b() {}"), &[0.5]).unwrap();
        upsert_file_hash(&conn, "gone.rs", "bbb", Some(1000)).unwrap();

        let purged = purge_missing_files(&conn, dir.path()).unwrap();
        assert_eq!(purged, 1);

        // gone.rs should be removed from files table
        assert_eq!(get_file_hash(&conn, "gone.rs").unwrap(), None);
        // exists.rs should remain
        assert!(get_file_hash(&conn, "exists.rs").unwrap().is_some());
    }

    #[test]
    fn purge_missing_files_returns_zero_when_all_exist() {
        let dir = tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();
        let file = dir.path().join("a.rs");
        std::fs::write(&file, "fn a() {}").unwrap();
        upsert_file_hash(&conn, "a.rs", "aaa", Some(1000)).unwrap();
        let purged = purge_missing_files(&conn, dir.path()).unwrap();
        assert_eq!(purged, 0);
    }

    #[test]
    fn staleness_no_commit_stored_is_stale() {
        let dir = tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@test.com").unwrap();
        // Create an initial commit
        let mut index = repo.index().unwrap();
        let tree_oid = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let sig = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .unwrap();

        let conn = open_db(dir.path()).unwrap();
        let staleness = check_index_staleness(&conn, dir.path()).unwrap();
        assert!(staleness.stale);
    }

    #[test]
    fn staleness_matching_commit_is_fresh() {
        let dir = tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@test.com").unwrap();
        let mut index = repo.index().unwrap();
        let tree_oid = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let sig = repo.signature().unwrap();
        let oid = repo
            .commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .unwrap();

        let conn = open_db(dir.path()).unwrap();
        set_last_indexed_commit(&conn, &oid.to_string()).unwrap();
        let staleness = check_index_staleness(&conn, dir.path()).unwrap();
        assert!(!staleness.stale);
        assert_eq!(staleness.behind_commits, 0);
    }

    #[test]
    fn find_changed_files_detects_new_file() {
        let dir = tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();

        // Create a file that's not in the index
        let file = dir.path().join("new.rs");
        std::fs::write(&file, "fn new() {}").unwrap();

        let candidates = find_changed_files(&conn, dir.path(), false).unwrap();
        assert_eq!(candidates.changed.len(), 1);
        assert_eq!(candidates.changed[0], "new.rs");
    }

    #[test]
    fn find_changed_files_skips_unchanged_mtime() {
        let dir = tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();

        // Create a file and index it with matching mtime + hash
        let file = dir.path().join("same.rs");
        std::fs::write(&file, "fn same() {}").unwrap();
        let mtime = file_mtime(&file).unwrap();
        let hash = hash_file(&file).unwrap();
        upsert_file_hash(&conn, "same.rs", &hash, Some(mtime)).unwrap();

        let candidates = find_changed_files(&conn, dir.path(), false).unwrap();
        assert!(candidates.changed.is_empty());
    }

    #[test]
    fn find_changed_files_mtime_match_skips_hash_check() {
        let dir = tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();

        let file = dir.path().join("mod.rs");
        std::fs::write(&file, "fn a() {}").unwrap();
        let mtime = file_mtime(&file).unwrap();
        // Store a different hash but matching mtime — mtime pre-filter should
        // short-circuit and treat the file as unchanged (by design: mtime is
        // the cheap gate, hash is only checked when mtime differs)
        upsert_file_hash(&conn, "mod.rs", "oldhash", Some(mtime)).unwrap();

        let candidates = find_changed_files(&conn, dir.path(), false).unwrap();
        assert!(
            candidates.changed.is_empty(),
            "mtime match should skip hash check"
        );
    }

    #[test]
    fn find_changed_files_detects_mtime_change() {
        let dir = tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();

        let file = dir.path().join("mod.rs");
        std::fs::write(&file, "fn a() {}").unwrap();
        let hash = hash_file(&file).unwrap();
        // Store correct hash but stale mtime — mtime differs so hash is checked,
        // hash matches so file should NOT be reported as changed
        upsert_file_hash(&conn, "mod.rs", &hash, Some(0)).unwrap();

        let candidates = find_changed_files(&conn, dir.path(), false).unwrap();
        assert!(
            candidates.changed.is_empty(),
            "hash match should prevent re-index even if mtime differs"
        );
    }

    #[test]
    fn open_db_creates_drift_report_table() {
        let (_dir, conn) = open_test_db();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM drift_report", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn find_changed_files_force_returns_all() {
        let dir = tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();

        let file = dir.path().join("a.rs");
        std::fs::write(&file, "fn a() {}").unwrap();
        let mtime = file_mtime(&file).unwrap();
        let hash = hash_file(&file).unwrap();
        upsert_file_hash(&conn, "a.rs", &hash, Some(mtime)).unwrap();

        let candidates = find_changed_files(&conn, dir.path(), true).unwrap();
        assert_eq!(candidates.changed.len(), 1); // force includes even unchanged
    }

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

    #[test]
    fn upsert_drift_report_inserts_and_queries() {
        let (_dir, conn) = open_test_db();
        upsert_drift_report(&conn, "a.rs", 0.25, 0.8, Some("fn changed() {}"), 1, 0).unwrap();

        let reports = query_drift_report(&conn, None, None).unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].file_path, "a.rs");
        assert!((reports[0].avg_drift - 0.25).abs() < 0.01);
        assert!((reports[0].max_drift - 0.8).abs() < 0.01);
        assert_eq!(
            reports[0].max_drift_chunk.as_deref(),
            Some("fn changed() {}")
        );
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

    #[test]
    fn vec0_search_returns_closest_vector() {
        let (_dir, conn) = open_test_db_vec0(4);
        insert_chunk(
            &conn,
            &dummy_chunk("a.rs", "fn a() {}"),
            &[1.0_f32, 0.0, 0.0, 0.0],
        )
        .unwrap();
        insert_chunk(
            &conn,
            &dummy_chunk("b.rs", "fn b() {}"),
            &[0.0_f32, 1.0, 0.0, 0.0],
        )
        .unwrap();

        let results = search(&conn, &[0.9_f32, 0.1, 0.0, 0.0], 1).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_path, "a.rs");
        assert!(results[0].score > 0.9, "score={}", results[0].score);
    }

    #[test]
    fn vec0_search_respects_limit() {
        let (_dir, conn) = open_test_db_vec0(2);
        for i in 1..=5u8 {
            insert_chunk(
                &conn,
                &dummy_chunk(&format!("{i}.rs"), "fn f() {}"),
                &[i as f32, 0.0_f32],
            )
            .unwrap();
        }
        let results = search(&conn, &[1.0_f32, 0.0], 3).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn vec0_search_scoped_filters_by_source() {
        let (_dir, conn) = open_test_db_vec0(2);

        // Insert 6 project chunks and 6 library chunks — enough that a naive
        // LIMIT applied before filtering (the bug) would under-return results.
        for i in 0..6u8 {
            let c = dummy_chunk_with_source(&format!("p{i}.rs"), "fn p() {}", "project");
            insert_chunk(&conn, &c, &[i as f32, 0.0_f32]).unwrap();
            let c = dummy_chunk_with_source(&format!("l{i}.rs"), "fn l() {}", "mylib");
            insert_chunk(&conn, &c, &[i as f32, 0.1_f32]).unwrap();
        }

        // No filter — all 12 chunks.
        let all = search_scoped(&conn, &[1.0_f32, 0.0], 20, &SourceScope::All).unwrap();
        assert_eq!(all.len(), 12);

        // Project-only — must return all 6 project chunks even though the KNN
        // over-fetches from a pool that is 50% library chunks.
        let proj_only = search_scoped(&conn, &[1.0_f32, 0.0], 6, &SourceScope::Project).unwrap();
        assert_eq!(proj_only.len(), 6);
        assert!(proj_only.iter().all(|r| r.source == "project"));

        // Libraries — must return all 6 library chunks.
        let libs_only = search_scoped(&conn, &[1.0_f32, 0.0], 6, &SourceScope::Libraries).unwrap();
        assert_eq!(libs_only.len(), 6);
        assert!(libs_only.iter().all(|r| r.source == "mylib"));

        // Specific lib — must return all 6 matching chunks.
        let mylib = search_scoped(
            &conn,
            &[1.0_f32, 0.0],
            6,
            &SourceScope::Library("mylib".to_string()),
        )
        .unwrap();
        assert_eq!(mylib.len(), 6);
        assert!(mylib.iter().all(|r| r.source == "mylib"));
    }

    #[test]
    fn open_db_creates_memories_table() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM memories", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn open_db_creates_vec_memories_table() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();
        set_meta(&conn, "embedding_dims", "384").unwrap();
        ensure_vec_memories(&conn).unwrap();
        let sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name='vec_memories'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            sql.contains("vec0"),
            "expected vec0 virtual table, got: {sql}"
        );
    }

    #[test]
    fn insert_and_search_memory() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();
        set_meta(&conn, "embedding_dims", "3").unwrap();
        ensure_vec_memories(&conn).unwrap();

        let embedding = vec![0.1_f32, 0.2, 0.3];
        let id = insert_memory(
            &conn,
            "code",
            "test title",
            "test content about patterns",
            &embedding,
        )
        .unwrap();
        assert!(id > 0);

        let results = search_memories(&conn, &embedding, None, 5).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "test title");
        assert_eq!(results[0].bucket, "code");
    }

    #[test]
    fn delete_memory_removes_from_both_tables() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();
        set_meta(&conn, "embedding_dims", "3").unwrap();
        ensure_vec_memories(&conn).unwrap();

        let embedding = vec![0.1_f32, 0.2, 0.3];
        let id = insert_memory(&conn, "code", "to delete", "content", &embedding).unwrap();
        delete_memory(&conn, id).unwrap();

        let results = search_memories(&conn, &embedding, None, 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn search_memories_filters_by_bucket() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();
        set_meta(&conn, "embedding_dims", "3").unwrap();
        ensure_vec_memories(&conn).unwrap();

        let e1 = vec![0.1_f32, 0.2, 0.3];
        let e2 = vec![0.3_f32, 0.2, 0.1];
        insert_memory(&conn, "code", "code mem", "patterns", &e1).unwrap();
        insert_memory(&conn, "system", "sys mem", "build stuff", &e2).unwrap();

        let code_only = search_memories(&conn, &e1, Some("code"), 5).unwrap();
        assert_eq!(code_only.len(), 1);
        assert_eq!(code_only[0].bucket, "code");
    }

    #[test]
    fn upsert_memory_updates_existing() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();
        set_meta(&conn, "embedding_dims", "3").unwrap();
        ensure_vec_memories(&conn).unwrap();

        let e1 = vec![0.1_f32, 0.2, 0.3];
        let id1 = insert_memory(&conn, "code", "my-topic", "old content", &e1).unwrap();

        let e2 = vec![0.4_f32, 0.5, 0.6];
        let id2 = upsert_memory_by_title(&conn, "code", "my-topic", "new content", &e2).unwrap();

        assert_eq!(id1, id2, "should update same row, not insert new");

        // Verify content updated
        let content: String = conn
            .query_row(
                "SELECT content FROM memories WHERE id = ?1",
                params![id1],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(content, "new content");
    }

    #[test]
    fn upsert_memory_different_buckets_do_not_collide() {
        // Two memories with the same title in different buckets must be stored
        // independently — an upsert into "system" must not overwrite "code".
        let dir = tempfile::tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();
        set_meta(&conn, "embedding_dims", "2").unwrap();
        ensure_vec_memories(&conn).unwrap();

        let e = vec![0.1_f32, 0.2];
        let code_id =
            upsert_memory_by_title(&conn, "code", "patterns", "code content", &e).unwrap();
        let sys_id =
            upsert_memory_by_title(&conn, "system", "patterns", "system content", &e).unwrap();

        assert_ne!(
            code_id, sys_id,
            "different buckets must produce different rows"
        );

        // A second upsert into "system"/"patterns" updates the system row only.
        let e2 = vec![0.3_f32, 0.4];
        let sys_id2 =
            upsert_memory_by_title(&conn, "system", "patterns", "updated system", &e2).unwrap();
        assert_eq!(sys_id, sys_id2, "should update same system row");

        // Verify the code memory is untouched.
        let code_content: String = conn
            .query_row(
                "SELECT content FROM memories WHERE id = ?1",
                params![code_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(code_content, "code content");
    }

    #[test]
    fn build_index_does_not_clear_memories() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();
        set_meta(&conn, "embedding_dims", "3").unwrap();
        ensure_vec_memories(&conn).unwrap();

        // Insert a memory
        insert_memory(
            &conn,
            "code",
            "keep me",
            "important knowledge",
            &[0.1, 0.2, 0.3],
        )
        .unwrap();

        // Also insert a code chunk so we have data in the chunks/files tables
        insert_chunk(
            &conn,
            &dummy_chunk("src/main.rs", "fn main() {}"),
            &[0.4_f32, 0.5, 0.6],
        )
        .unwrap();

        // Simulate what build_index does to code chunks (it clears and rebuilds)
        conn.execute("DELETE FROM chunks", []).unwrap();
        conn.execute("DELETE FROM files", []).unwrap();

        // Memories should survive — they live in separate tables
        let results = search_memories(&conn, &[0.1, 0.2, 0.3], None, 5).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "keep me");
        assert_eq!(results[0].content, "important knowledge");
    }

    #[test]
    fn search_memories_empty_db() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();
        set_meta(&conn, "embedding_dims", "3").unwrap();
        ensure_vec_memories(&conn).unwrap();

        let results = search_memories(&conn, &[0.1, 0.2, 0.3], None, 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn insert_memory_timestamps_are_set() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();
        set_meta(&conn, "embedding_dims", "3").unwrap();
        ensure_vec_memories(&conn).unwrap();

        insert_memory(&conn, "code", "test", "content", &[0.1, 0.2, 0.3]).unwrap();

        let (created, updated): (String, String) = conn
            .query_row(
                "SELECT created_at, updated_at FROM memories WHERE title = 'test'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();

        assert!(!created.is_empty());
        assert!(!updated.is_empty());
        assert_eq!(created, updated); // Same on first insert
    }

    #[test]
    fn upsert_memory_updates_timestamp() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();
        set_meta(&conn, "embedding_dims", "3").unwrap();
        ensure_vec_memories(&conn).unwrap();

        insert_memory(&conn, "code", "ts-test", "v1", &[0.1, 0.2, 0.3]).unwrap();

        let created1: String = conn
            .query_row(
                "SELECT created_at FROM memories WHERE title = 'ts-test'",
                [],
                |r| r.get(0),
            )
            .unwrap();

        // Upsert with new content
        upsert_memory_by_title(&conn, "code", "ts-test", "v2", &[0.4, 0.5, 0.6]).unwrap();

        let (created2, updated2): (String, String) = conn
            .query_row(
                "SELECT created_at, updated_at FROM memories WHERE title = 'ts-test'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();

        assert_eq!(created1, created2, "created_at should not change on upsert");
        // updated_at may or may not differ (depends on timing) but should exist
        assert!(!updated2.is_empty());
    }

    #[test]
    fn delete_nonexistent_memory_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();
        set_meta(&conn, "embedding_dims", "3").unwrap();
        ensure_vec_memories(&conn).unwrap();

        // Deleting a memory that doesn't exist should return a RecoverableError
        let result = delete_memory(&conn, 99999);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("not found"),
            "expected 'not found' in error: {err_msg}"
        );
    }

    #[test]
    fn memory_anchors_table_created() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();
        ensure_memory_anchors(&conn).unwrap();

        insert_semantic_anchor(
            &conn,
            "markdown",
            "architecture",
            "src/server.rs",
            "abc123",
            0.85,
        )
        .unwrap();
        let anchors = get_semantic_anchors(&conn, "markdown", "architecture").unwrap();
        assert_eq!(anchors.len(), 1);
        assert_eq!(anchors[0].file_path, "src/server.rs");
        assert!((anchors[0].similarity - 0.85).abs() < 0.01);
        assert!(!anchors[0].stale);
    }

    #[test]
    fn memory_anchors_upsert_on_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();
        ensure_memory_anchors(&conn).unwrap();

        insert_semantic_anchor(&conn, "markdown", "arch", "src/a.rs", "h1", 0.8).unwrap();
        insert_semantic_anchor(&conn, "markdown", "arch", "src/a.rs", "h2", 0.9).unwrap();

        let anchors = get_semantic_anchors(&conn, "markdown", "arch").unwrap();
        assert_eq!(anchors.len(), 1);
        assert_eq!(anchors[0].file_hash, "h2");
        assert!((anchors[0].similarity - 0.9).abs() < 0.01);
    }

    #[test]
    fn mark_anchors_stale_for_drifted_file() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();
        ensure_memory_anchors(&conn).unwrap();

        insert_semantic_anchor(&conn, "markdown", "arch", "src/server.rs", "h1", 0.9).unwrap();
        insert_semantic_anchor(&conn, "markdown", "conv", "src/server.rs", "h2", 0.8).unwrap();
        insert_semantic_anchor(&conn, "markdown", "arch", "src/other.rs", "h3", 0.7).unwrap();

        let count = mark_anchors_stale_for_file(&conn, "src/server.rs").unwrap();
        assert_eq!(count, 2);

        let arch = get_semantic_anchors(&conn, "markdown", "arch").unwrap();
        let server_anchor = arch
            .iter()
            .find(|a| a.file_path == "src/server.rs")
            .unwrap();
        assert!(server_anchor.stale);
        let other_anchor = arch.iter().find(|a| a.file_path == "src/other.rs").unwrap();
        assert!(!other_anchor.stale);
    }

    #[test]
    fn delete_semantic_anchors_clears_all() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();
        ensure_memory_anchors(&conn).unwrap();

        insert_semantic_anchor(&conn, "markdown", "arch", "src/a.rs", "h1", 0.9).unwrap();
        insert_semantic_anchor(&conn, "markdown", "arch", "src/b.rs", "h2", 0.8).unwrap();

        delete_semantic_anchors(&conn, "markdown", "arch").unwrap();
        let anchors = get_semantic_anchors(&conn, "markdown", "arch").unwrap();
        assert!(anchors.is_empty());
    }

    #[test]
    fn ensure_memory_anchors_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();
        // Call twice — should not error
        ensure_memory_anchors(&conn).unwrap();
        ensure_memory_anchors(&conn).unwrap();
    }

    #[test]
    fn project_db_path_uses_embeddings_dir() {
        let root = Path::new("/tmp/test-project");
        let path = project_db_path(root);
        assert_eq!(path, root.join(".codescout/embeddings/project.db"));
    }

    #[test]
    fn lib_db_path_basic() {
        let root = Path::new("/tmp/test-project");
        let path = lib_db_path(root, "tokio");
        assert_eq!(path, root.join(".codescout/embeddings/lib/tokio.db"));
    }

    #[test]
    fn lib_db_path_sanitizes_scoped_npm() {
        let root = Path::new("/tmp/test-project");
        let path = lib_db_path(root, "@scope/name");
        assert_eq!(path, root.join(".codescout/embeddings/lib/@scope--name.db"));
    }

    #[test]
    fn lib_db_path_sanitizes_backslash() {
        let root = Path::new("/tmp/test-project");
        let path = lib_db_path(root, "foo\\bar");
        assert_eq!(path, root.join(".codescout/embeddings/lib/foo--bar.db"));
    }

    #[test]
    fn migrate_db_layout_renames_old_db() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let old_path = root.join(".codescout/embeddings.db");
        let new_path = project_db_path(root);

        // Create a real (empty) old-style SQLite DB so the extraction step can open it
        std::fs::create_dir_all(old_path.parent().unwrap()).unwrap();
        {
            let conn = Connection::open(&old_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE chunks (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    file_path TEXT NOT NULL,
                    language TEXT NOT NULL,
                    content TEXT NOT NULL,
                    start_line INTEGER NOT NULL,
                    end_line INTEGER NOT NULL,
                    file_hash TEXT NOT NULL,
                    source TEXT NOT NULL DEFAULT 'project'
                );
                CREATE TABLE chunk_embeddings (rowid INTEGER PRIMARY KEY, embedding BLOB NOT NULL);
                CREATE TABLE files (path TEXT PRIMARY KEY, hash TEXT NOT NULL, mtime INTEGER);",
            )
            .unwrap();
        }

        assert!(!new_path.exists());
        maybe_migrate_db_layout(root).unwrap();
        assert!(new_path.exists());
        assert!(!old_path.exists());
        // lib/ directory created
        assert!(root.join(".codescout/embeddings/lib").is_dir());
    }

    #[test]
    fn migrate_db_layout_noop_when_new_layout_exists() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let new_path = project_db_path(root);
        std::fs::create_dir_all(new_path.parent().unwrap()).unwrap();
        std::fs::write(&new_path, b"already-migrated").unwrap();

        // Old file also exists (shouldn't be touched)
        let old_path = root.join(".codescout/embeddings.db");
        std::fs::write(&old_path, b"old-db").unwrap();

        maybe_migrate_db_layout(root).unwrap();
        // New file untouched
        assert_eq!(std::fs::read(&new_path).unwrap(), b"already-migrated");
    }

    #[test]
    fn migrate_extracts_library_chunks_to_separate_dbs() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Create a real old-style DB with project + library chunks
        let old_path = db_path(root);
        std::fs::create_dir_all(old_path.parent().unwrap()).unwrap();
        {
            let conn = Connection::open(&old_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE chunks (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    file_path TEXT NOT NULL,
                    language TEXT NOT NULL,
                    content TEXT NOT NULL,
                    start_line INTEGER NOT NULL,
                    end_line INTEGER NOT NULL,
                    file_hash TEXT NOT NULL,
                    source TEXT NOT NULL DEFAULT 'project',
                    metadata TEXT
                );
                CREATE TABLE chunk_embeddings (rowid INTEGER PRIMARY KEY, embedding BLOB NOT NULL);
                CREATE TABLE files (path TEXT PRIMARY KEY, hash TEXT NOT NULL, mtime INTEGER);
                INSERT INTO chunks (file_path, language, content, start_line, end_line, file_hash, source)
                    VALUES ('src/main.rs', 'rust', 'fn main() {}', 1, 1, 'aaa', 'project');
                INSERT INTO chunks (file_path, language, content, start_line, end_line, file_hash, source)
                    VALUES ('[lib:tokio]/src/runtime.rs', 'rust', 'pub struct Runtime', 1, 1, 'bbb', 'lib:tokio');
                INSERT INTO chunk_embeddings (rowid, embedding) VALUES (1, X'00000000');
                INSERT INTO chunk_embeddings (rowid, embedding) VALUES (2, X'00000000');",
            )
            .unwrap();
        }

        maybe_migrate_db_layout(root).unwrap();

        // project.db should only have the project chunk
        let proj_conn = Connection::open(project_db_path(root)).unwrap();
        let count: i64 = proj_conn
            .query_row(
                "SELECT COUNT(*) FROM chunks WHERE source = 'project'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        let lib_count: i64 = proj_conn
            .query_row(
                "SELECT COUNT(*) FROM chunks WHERE source LIKE 'lib:%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(lib_count, 0);

        // tokio.db should have the library chunk
        let lib_path = lib_db_path(root, "tokio");
        assert!(lib_path.exists(), "lib/tokio.db should exist");
        let lib_conn = Connection::open(&lib_path).unwrap();
        let lib_chunk_count: i64 = lib_conn
            .query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(lib_chunk_count, 1);
    }

    #[test]
    fn open_db_uses_new_embeddings_dir() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let conn = open_db(root).unwrap();
        // Verify the DB is at the new location
        assert!(
            project_db_path(root).exists(),
            "project.db should be at the new location"
        );
        // Old location should not exist (for a fresh project)
        assert!(
            !db_path(root).exists(),
            "old embeddings.db should not exist for a fresh project"
        );
        drop(conn);
    }

    #[test]
    fn open_lib_db_creates_with_lib_meta_table() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let conn = open_lib_db(root, "tokio").unwrap();
        // lib_meta table should exist
        conn.execute(
            "INSERT INTO lib_meta (key, value) VALUES ('test', 'ok')",
            [],
        )
        .unwrap();
        let val: String = conn
            .query_row("SELECT value FROM lib_meta WHERE key = 'test'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(val, "ok");
        // Standard tables should also exist
        conn.execute(
            "INSERT INTO chunks (file_path, language, content, start_line, end_line, file_hash, source) VALUES ('test.rs', 'rust', 'code', 1, 1, 'hash', 'lib:tokio')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn search_multi_db_missing_lib_graceful() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // Create project DB (needed for directory structure)
        let _conn = open_db(root).unwrap();

        let registry = crate::library::registry::LibraryRegistry::new();
        let query_emb = vec![0.0f32; 384]; // dummy embedding

        // Search for a non-existent library — should return a RecoverableError, not Ok
        let scope = crate::library::scope::Scope::Library("nonexistent".to_string());
        let err = search_multi_db(root, &query_emb, 10, &scope, &registry, None).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("nonexistent"),
            "error should name the missing library, got: {msg}"
        );
    }

    #[test]
    fn search_multi_db_project_scope_uses_project_db() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let _conn = open_db(root).unwrap();

        let registry = crate::library::registry::LibraryRegistry::new();
        let query_emb = vec![0.0f32; 384];

        let scope = crate::library::scope::Scope::Project;
        // Should succeed (empty results, no embeddings in the test DB)
        let results = search_multi_db(root, &query_emb, 10, &scope, &registry, None).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn search_multi_db_applies_per_file_cap() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let conn = open_db(root).unwrap();
        set_meta(&conn, "embedding_dims", "4").unwrap();
        maybe_migrate_to_vec0(&conn).unwrap();

        // Insert 5 chunks in file_a and 5 in file_b, all with the same embedding
        // so cosine distance ties — ordering will follow rowid, giving file_a
        // first. Without the cap that means 5× file_a + some file_b; with
        // MAX_PER_FILE=2 and limit=4, we expect exactly 2 from each file.
        let emb = [0.1_f32, 0.2, 0.3, 0.4];
        for (i, file) in (0..10).zip(
            std::iter::repeat("file_a.rs")
                .take(5)
                .chain(std::iter::repeat("file_b.rs").take(5)),
        ) {
            let mut chunk = dummy_chunk(file, &format!("chunk {i}"));
            chunk.start_line = i + 1;
            chunk.end_line = i + 2;
            insert_chunk(&conn, &chunk, &emb).unwrap();
        }
        drop(conn);

        let registry = crate::library::registry::LibraryRegistry::new();
        let scope = crate::library::scope::Scope::Project;
        let results = search_multi_db(root, &emb, 4, &scope, &registry, None).unwrap();

        assert_eq!(results.len(), 4, "expected exactly 4 results");
        let file_a = results
            .iter()
            .filter(|r| r.file_path == "file_a.rs")
            .count();
        let file_b = results
            .iter()
            .filter(|r| r.file_path == "file_b.rs")
            .count();
        assert!(
            file_a <= 2 && file_b <= 2,
            "per-file cap violated: a={file_a}, b={file_b}"
        );
        assert_eq!(file_a + file_b, 4);
    }

    #[test]
    fn open_db_adds_project_id_column() {
        let dir = tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();
        let has_col: bool = conn
            .prepare("SELECT project_id FROM chunks LIMIT 0")
            .is_ok();
        assert!(has_col, "chunks table should have project_id column");
    }

    #[test]
    fn open_db_adds_metadata_column_to_existing_v1_db() {
        // Verify the ALTER TABLE guard fires for a DB that has schema_version=1
        // but was created before the metadata column existed.
        let dir = tempdir().unwrap();
        let db_path = project_db_path(dir.path());
        std::fs::create_dir_all(db_path.parent().unwrap()).unwrap();

        // Seed a v1 DB without metadata column.
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE chunks (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    file_path TEXT NOT NULL,
                    language TEXT NOT NULL,
                    content TEXT NOT NULL,
                    start_line INTEGER NOT NULL,
                    end_line INTEGER NOT NULL,
                    file_hash TEXT NOT NULL,
                    source TEXT NOT NULL DEFAULT 'project',
                    project_id TEXT NOT NULL DEFAULT 'root'
                );
                CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                INSERT INTO meta (key, value) VALUES ('schema_version', '1');",
            )
            .unwrap();
        }

        // open_db should add metadata column via ALTER TABLE guard.
        let conn = open_db(dir.path()).unwrap();
        let has_col: bool = conn.prepare("SELECT metadata FROM chunks LIMIT 0").is_ok();
        assert!(
            has_col,
            "chunks table should have metadata column after migration"
        );
    }

    #[test]
    fn existing_chunks_get_default_root_project_id() {
        let dir = tempdir().unwrap();
        let conn = open_db(dir.path()).unwrap();

        conn.execute(
            "INSERT INTO chunks (file_path, start_line, end_line, content, language, file_hash, source)
         VALUES ('src/main.rs', 1, 10, 'fn main() {}', 'rust', 'abc123', 'project')",
            [],
        )
        .unwrap();

        let pid: String = conn
            .query_row(
                "SELECT project_id FROM chunks WHERE file_path = 'src/main.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(pid, "root");
    }

    #[test]
    fn chunks_roundtrip_metadata_column() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = project_db_path(dir.path());
        std::fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        let conn = open_db(dir.path()).unwrap();
        conn.execute(
            "INSERT INTO chunks (file_path, language, content, start_line, end_line, file_hash, source, metadata)
             VALUES ('a.rs', 'rust', 'body', 1, 5, 'hash', 'project', 'src/a.rs :: fn foo')",
            [],
        )
        .unwrap();

        let meta: Option<String> = conn
            .query_row(
                "SELECT metadata FROM chunks WHERE file_path = 'a.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();

        assert_eq!(meta.as_deref(), Some("src/a.rs :: fn foo"));
    }

    #[test]
    fn insert_chunk_persists_metadata() {
        let (_dir, conn) = open_test_db();
        let mut chunk = dummy_chunk("src/b.rs", "fn bar() {}");
        chunk.metadata = Some("src/b.rs :: fn bar".to_string());
        insert_chunk(&conn, &chunk, &[0.1, 0.2]).unwrap();

        let meta: Option<String> = conn
            .query_row(
                "SELECT metadata FROM chunks WHERE file_path = 'src/b.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();

        assert_eq!(meta.as_deref(), Some("src/b.rs :: fn bar"));
    }

    #[test]
    fn embed_text_format_includes_metadata_prefix() {
        // When a RawChunk has metadata, the text sent for embedding should be
        // "{metadata}\n{content}" — not just content.
        let chunk_with_meta = codescout_embed::chunker::RawChunk {
            content: "fn hello() {}".to_string(),
            start_line: 1,
            end_line: 1,
            metadata: Some("src/a.rs :: fn hello".to_string()),
        };
        let chunk_no_meta = codescout_embed::chunker::RawChunk {
            content: "fn world() {}".to_string(),
            start_line: 3,
            end_line: 3,
            metadata: None,
        };

        let text_with = match &chunk_with_meta.metadata {
            Some(m) => format!("{m}\n{}", chunk_with_meta.content),
            None => chunk_with_meta.content.clone(),
        };
        let text_without = match &chunk_no_meta.metadata {
            Some(m) => format!("{m}\n{}", chunk_no_meta.content),
            None => chunk_no_meta.content.clone(),
        };

        assert_eq!(text_with, "src/a.rs :: fn hello\nfn hello() {}");
        assert_eq!(text_without, "fn world() {}");
    }

    // ── Overlap test ──────────────────────────────────────────────────────────

    /// Verify the producer/consumer pipeline produces correct DB state across
    /// multiple file groups.
    ///
    /// This test exercises the full pipeline path (embed_producer → channel →
    /// db_writer) with a real DB and a mock zero-latency embedder.  It
    /// verifies correctness: all files are indexed, every chunk has exactly one
    /// embedding, and the total counts are consistent.
    ///
    /// Design note: a timing assertion (total < sequential lower bound) was
    /// attempted but proved unreliable under parallel test execution — the OS
    /// schedules ~3 500 tokio worker threads simultaneously during a full
    /// `cargo test` run, causing std::thread::sleep calls in both tasks to
    /// serialize rather than overlap.  The overlap is an architectural property
    /// (mpsc channel capacity-1 rendezvous + two tokio::spawn tasks) verified
    /// by code inspection; this test covers correctness only.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn producer_consumer_overlaps_embed_with_db_write() {
        use std::sync::Arc;

        struct InstantEmbedder;
        #[async_trait::async_trait]
        impl codescout_embed::Embedder for InstantEmbedder {
            fn dimensions(&self) -> usize {
                3
            }
            async fn embed(
                &self,
                texts: &[&str],
            ) -> anyhow::Result<Vec<codescout_embed::Embedding>> {
                Ok(texts.iter().map(|_| vec![1.0_f32, 0.0, 0.0]).collect())
            }
        }

        // Build a project with 60 small Rust source files to force 2 groups at
        // file_group_size=30.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("src")).unwrap();
        for i in 0..60 {
            let content = format!("fn func_{i}() {{ println!(\"hello {i}\"); }}\n");
            std::fs::write(root.join(format!("src/f{i:02}.rs")), content).unwrap();
        }

        // project.toml: file_group_size=30, drift off (simpler writes)
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        std::fs::write(
            root.join(".codescout/project.toml"),
            "[project]\nname = \"overlap-test\"\n\n[embeddings]\nfile_group_size = 30\ndrift_detection_enabled = false\n",
        )
        .unwrap();

        let conn = open_db(root).unwrap();
        let config = crate::config::ProjectConfig::load_or_default(root).unwrap();

        let change_set = find_changed_files(&conn, root, true).unwrap();
        let mut works: Vec<FileWork> = Vec::new();
        for rel in &change_set.changed {
            let path = root.join(rel);
            let Some(lang) = crate::ast::detect_language(&path) else {
                continue;
            };
            let hash = hash_file(&path).unwrap();
            let src = std::fs::read_to_string(&path).unwrap();
            let chunks = crate::embed::ast_chunker::split_file(
                &src,
                lang,
                std::path::Path::new(rel),
                config.embeddings.effective_chunk_size(),
            );
            if chunks.is_empty() {
                continue;
            }
            works.push(FileWork {
                rel: rel.clone(),
                hash,
                mtime: file_mtime(&path).unwrap_or(0),
                lang: lang.to_string(),
                chunks,
            });
        }

        conn.execute_batch("BEGIN").unwrap();
        clear_drift_report(&conn).unwrap();
        conn.execute_batch("COMMIT").unwrap();

        let total_files = works.len();
        assert!(
            total_files >= 60,
            "need at least 60 files for the 2-group overlap test, got {total_files}"
        );

        let file_group_size = config.embeddings.effective_file_group_size();
        assert_eq!(
            file_group_size, 30,
            "expected file_group_size=30 from project.toml"
        );

        let max_inflight = config.embeddings.effective_max_inflight();
        let discovered = crate::workspace::discover_projects(root, 3, &[]);

        let (tx, rx) = tokio::sync::mpsc::channel::<GroupReady>(1);
        let root_buf = root.to_path_buf();
        let writer = tokio::spawn(db_writer(rx, conn, config.clone(), root_buf, discovered));

        embed_producer(
            works,
            Arc::new(InstantEmbedder),
            tx,
            None,
            total_files,
            file_group_size,
            max_inflight,
        )
        .await
        .unwrap();

        writer.await.unwrap().unwrap();

        // Correctness: all 60 files indexed, every chunk has an embedding.
        let conn2 = open_db(root).unwrap();
        let stats = index_stats(&conn2).unwrap();
        assert_eq!(stats.file_count, 60, "all 60 files should be indexed");
        assert!(stats.chunk_count > 0, "chunks must be written");
        assert_eq!(
            stats.embedding_count, stats.chunk_count,
            "every chunk needs an embedding"
        );
    }
}
