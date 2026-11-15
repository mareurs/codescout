//! A per-user cache of embedding vectors, keyed by model and embed input.
//!
//! The file sits beside the catalog file. A vector the embedder made once is
//! reused when the same text is embedded again under the same model, so a moved
//! or rebuilt document costs no embed call.
//!
//! A cache failure never fails a call and never returns a wrong vector. It
//! degrades to a miss and is counted. The lock is never held across an await:
//! this module has no awaits.

use std::path::{Path, PathBuf};
use std::time::Duration;

use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

/// File name of the cache, in the machine's librarian directory (`LibrarianEnv::machine_paths`).
pub const VECTOR_CACHE_FILE: &str = "vector-cache.db";

/// Version of how (title, text) becomes the stored vector in `EmbeddingService::embed_artifact`:
/// the join, the segmentation and the pooling. Bump it when any of them changes.
/// A change to what the embedder sends on the document side under the same model string
/// (a prefix, a truncation, a request parameter) has the same effect: bump it then too.
pub const COMPOSITION_VERSION: u32 = 1;

/// How long a statement waits for a lock held by another process. Short on purpose:
/// a cache must not stall a reindex.
const PRODUCTION_BUSY_MS: u64 = 250;

/// The `PRAGMA user_version` this code writes and reads. A newer file and a file with a
/// negative version are left alone.
const FORMAT_VERSION: i64 = 1;

/// A plain rowid table: a row is about 3 KB, too wide for `WITHOUT ROWID`.
const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS vec (model_key TEXT NOT NULL, input_hash TEXT NOT NULL, dim INTEGER NOT NULL, vector BLOB NOT NULL, PRIMARY KEY (model_key, input_hash))";

/// The model part of a cache key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelKey(String);

impl ModelKey {
    /// `format!("{model}|{budget_chars}|{COMPOSITION_VERSION}")`
    pub fn new(model: &str, budget_chars: usize) -> Self {
        Self(format!("{model}|{budget_chars}|{COMPOSITION_VERSION}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The answer to one lookup.
#[derive(Debug, PartialEq)]
pub enum CacheLookup {
    Hit(Vec<f32>),
    Miss,
    Failed,
}

/// What one flush did, in rows.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FlushOutcome {
    pub stored: usize,
    pub failed: usize,
}

/// Whether the cache is in use.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum CacheStatus {
    #[default]
    Off,
    On,
    Unavailable,
}

impl CacheStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            CacheStatus::Off => "off",
            CacheStatus::On => "on",
            CacheStatus::Unavailable => "unavailable",
        }
    }
}

/// What one run did with the cache. Reports print it.
///
/// `hits` counts the vectors the cache supplied to this run. `stored` counts the rows that
/// flushes wrote. The part of `errors` that comes from flushes counts the rows that flushes
/// dropped. A flush writes every row staged in the cache. The server keeps one cache per
/// process. So a flush can include rows staged by a cancelled or a concurrent call. Lookup
/// and stage failures reach `errors` through `note_item`. Do not read these counts as this
/// run's own embeds.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CacheTally {
    pub status: CacheStatus,
    pub hits: usize,
    pub stored: usize,
    pub errors: usize,
}

impl CacheTally {
    /// `hits += from_cache`, `errors += cache_failed`.
    pub fn note_item(&mut self, from_cache: bool, cache_failed: bool) {
        self.hits += usize::from(from_cache);
        self.errors += usize::from(cache_failed);
    }

    /// `stored += f.stored`, `errors += f.failed`.
    pub fn note_flush(&mut self, f: FlushOutcome) {
        self.stored += f.stored;
        self.errors += f.failed;
    }

    /// `{"status", "hits", "stored", "errors"}`.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "status": self.status.as_str(),
            "hits": self.hits,
            "stored": self.stored,
            "errors": self.errors,
        })
    }
}

struct Staged {
    model: String,
    hash: String,
    dim: usize,
    bytes: Vec<u8>,
}

enum State {
    Unopened,
    Open(Connection),
    Unavailable,
}

struct Inner {
    state: State,
    staged: Vec<Staged>,
    transactions: usize,
}

/// Open the file (or memory), set the pragmas and make sure the table exists.
///
/// The version is read before any pragma that could write, so a file of a newer
/// format, a file with a negative version or a file that is not a database is left as
/// it is. A file at the current
/// version takes no write lock here. A new file is initialised under the write lock,
/// which re-reads the version first.
fn open_connection(path: Option<&Path>, busy: Duration) -> Result<Connection, String> {
    let conn = match path {
        Some(p) => {
            if let Some(parent) = p.parent().filter(|d| !d.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("create {}: {e}", parent.display()))?;
            }
            Connection::open(p)
        }
        None => Connection::open_in_memory(),
    }
    .map_err(|e| e.to_string())?;
    conn.busy_timeout(busy).map_err(|e| e.to_string())?;
    let version = read_version(&conn)?;
    if let Some(why) = unsupported_format(version) {
        return Err(why);
    }
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(|e| e.to_string())?;
    if version < FORMAT_VERSION {
        if path.is_some() {
            conn.pragma_update(None, "journal_mode", "WAL")
                .map_err(|e| e.to_string())?;
        }
        initialise(&conn)?;
    }
    Ok(conn)
}

fn read_version(conn: &Connection) -> Result<i64, String> {
    conn.pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(|e| e.to_string())
}

fn newer_format(version: i64) -> String {
    format!("format version {version} is newer than {FORMAT_VERSION}")
}

/// Why a file of this `user_version` is left alone, or `None` when this code may use
/// it: version 0 (new) and `FORMAT_VERSION` are ours. A newer version belongs to a
/// newer binary. A negative one is no version this code or any release wrote.
fn unsupported_format(version: i64) -> Option<String> {
    if version > FORMAT_VERSION {
        Some(newer_format(version))
    } else if version < 0 {
        Some(format!("format version {version} is not a valid version"))
    } else {
        None
    }
}

/// Create the table and set the version, under the write lock.
///
/// The unlocked read said the file is new. Another process may have finished the job
/// or written a newer format since, so the version is read again inside the lock.
/// Only a file that still reads 0 is written. Any other outcome rolls back.
fn initialise(conn: &Connection) -> Result<(), String> {
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let version = read_version(&tx)?;
    if let Some(why) = unsupported_format(version) {
        return Err(why);
    }
    if version == 0 {
        tx.execute_batch(&format!(
            "{CREATE_TABLE}; PRAGMA user_version = {FORMAT_VERSION};"
        ))
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}

/// One `INSERT OR REPLACE` per row, all in one immediate transaction.
fn write_rows(conn: &Connection, rows: &[Staged]) -> rusqlite::Result<()> {
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    {
        let mut stmt = tx.prepare_cached(
            "INSERT OR REPLACE INTO vec (model_key, input_hash, dim, vector) VALUES (?1, ?2, ?3, ?4)",
        )?;
        for r in rows {
            stmt.execute(params![r.model, r.hash, r.dim as i64, r.bytes])?;
        }
    }
    tx.commit()
}

impl Inner {
    /// The open connection. Opens it on first use. A failed open is logged once and
    /// leaves the state `Unavailable` until `rearm`.
    fn conn(&mut self, path: Option<&Path>, busy: Duration) -> Option<&Connection> {
        if matches!(self.state, State::Unopened) {
            self.state = match open_connection(path, busy) {
                Ok(c) => State::Open(c),
                Err(e) => {
                    tracing::warn!(error = %e, "vector cache is unavailable; embedding without it");
                    State::Unavailable
                }
            };
        }
        match &self.state {
            State::Open(c) => Some(c),
            _ => None,
        }
    }
}

/// The cache. Opens its file lazily, on the first call that needs it.
pub struct VectorCache {
    path: Option<PathBuf>,
    busy: Duration,
    inner: Mutex<Inner>,
}

impl VectorCache {
    /// Lazy: nothing is touched until first use.
    pub fn open(path: &Path) -> Self {
        Self::open_with_busy_timeout(path, Duration::from_millis(PRODUCTION_BUSY_MS))
    }

    pub fn in_memory() -> Self {
        Self::new(None, Duration::from_millis(PRODUCTION_BUSY_MS))
    }

    pub(crate) fn open_with_busy_timeout(path: &Path, busy: Duration) -> Self {
        Self::new(Some(path.to_path_buf()), busy)
    }

    fn new(path: Option<PathBuf>, busy: Duration) -> Self {
        Self {
            path,
            busy,
            inner: Mutex::new(Inner {
                state: State::Unopened,
                staged: Vec::new(),
                transactions: 0,
            }),
        }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// `On`, or `Unavailable` after an open failed. A file not yet opened is `On`.
    pub fn status(&self) -> CacheStatus {
        match self.inner.lock().state {
            State::Unavailable => CacheStatus::Unavailable,
            State::Unopened | State::Open(_) => CacheStatus::On,
        }
    }

    /// Forget a failed open so the next call that needs the file opens it again.
    /// No effect otherwise.
    pub fn rearm(&self) {
        let mut g = self.inner.lock();
        if matches!(g.state, State::Unavailable) {
            g.state = State::Unopened;
        }
    }

    /// Look one vector up. Staged rows are not visible until `flush`.
    ///
    /// An unknown dimension (0) and a row of another dimension are a miss. A row
    /// whose blob is not `dim * 4` bytes, an SQL error and an unavailable cache
    /// are `Failed`.
    pub fn get(&self, model: &ModelKey, dim: usize, input_hash: &str) -> CacheLookup {
        if dim == 0 {
            return CacheLookup::Miss;
        }
        let (Ok(want_dim), Some(want_bytes)) = (i64::try_from(dim), dim.checked_mul(4)) else {
            return CacheLookup::Miss;
        };
        let mut g = self.inner.lock();
        let Some(conn) = g.conn(self.path.as_deref(), self.busy) else {
            return CacheLookup::Failed;
        };
        let row = conn
            .prepare_cached("SELECT dim, vector FROM vec WHERE model_key = ?1 AND input_hash = ?2")
            .and_then(|mut stmt| {
                stmt.query_row(params![model.as_str(), input_hash], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?))
                })
                .optional()
            });
        match row {
            Err(e) => {
                tracing::debug!(error = %e, "vector cache lookup failed");
                CacheLookup::Failed
            }
            Ok(None) => CacheLookup::Miss,
            Ok(Some((row_dim, _))) if row_dim != want_dim => CacheLookup::Miss,
            Ok(Some((_, blob))) if blob.len() != want_bytes => CacheLookup::Failed,
            Ok(Some((_, blob))) => CacheLookup::Hit(
                blob.as_chunks::<4>()
                    .0
                    .iter()
                    .map(|b| f32::from_le_bytes(*b))
                    .collect(),
            ),
        }
    }

    /// Queue one vector for the next `flush`. `false` for an empty vector, a
    /// non-finite component or an unavailable cache.
    pub fn stage(&self, model: &ModelKey, input_hash: &str, vector: &[f32]) -> bool {
        if vector.is_empty() || vector.iter().any(|x| !x.is_finite()) {
            return false;
        }
        let mut g = self.inner.lock();
        if g.conn(self.path.as_deref(), self.busy).is_none() {
            return false;
        }
        g.staged.push(Staged {
            model: model.as_str().to_owned(),
            hash: input_hash.to_owned(),
            dim: vector.len(),
            bytes: vector.iter().flat_map(|x| x.to_le_bytes()).collect(),
        });
        true
    }

    /// Write every staged row in one transaction. On any error the transaction
    /// rolls back and the rows are dropped and counted as failed, not retried.
    /// With nothing staged it opens nothing.
    pub fn flush(&self) -> FlushOutcome {
        let mut g = self.inner.lock();
        let rows = std::mem::take(&mut g.staged);
        if rows.is_empty() {
            return FlushOutcome::default();
        }
        let Some(conn) = g.conn(self.path.as_deref(), self.busy) else {
            return FlushOutcome {
                stored: 0,
                failed: rows.len(),
            };
        };
        match write_rows(conn, &rows) {
            Ok(()) => {
                g.transactions += 1;
                FlushOutcome {
                    stored: rows.len(),
                    failed: 0,
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, rows = rows.len(), "vector cache flush failed; rows dropped");
                FlushOutcome {
                    stored: 0,
                    failed: rows.len(),
                }
            }
        }
    }

    /// Committed flush transactions only. Read by tests.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn transactions(&self) -> usize {
        self.inner.lock().transactions
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        let mut g = self.inner.lock();
        g.conn(self.path.as_deref(), self.busy)
            .and_then(|c| {
                c.query_row("SELECT COUNT(*) FROM vec", [], |r| r.get::<_, i64>(0))
                    .ok()
            })
            .map_or(0, |n| n as usize)
    }

    #[cfg(test)]
    pub(crate) fn busy_timeout_ms(&self) -> Option<i64> {
        let mut g = self.inner.lock();
        g.conn(self.path.as_deref(), self.busy).and_then(|c| {
            c.pragma_query_value(None, "busy_timeout", |r| r.get(0))
                .ok()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn key() -> ModelKey {
        ModelKey::new("m", 8000)
    }

    fn file_cache(tmp: &TempDir) -> VectorCache {
        VectorCache::open(&tmp.path().join(VECTOR_CACHE_FILE))
    }

    fn put(c: &VectorCache, hash: &str, v: &[f32]) {
        assert!(c.stage(&key(), hash, v), "stage must accept {v:?}");
        assert_eq!(
            c.flush(),
            FlushOutcome {
                stored: 1,
                failed: 0
            }
        );
    }

    #[test]
    fn a_stored_vector_comes_back_bit_for_bit_after_a_flush() {
        let c = VectorCache::in_memory();
        let v = [0.5f32, -1.25, 3.0e-7, f32::MIN_POSITIVE / 2.0];
        assert!(c.stage(&key(), "h1", &v));
        assert_eq!(
            c.flush(),
            FlushOutcome {
                stored: 1,
                failed: 0
            }
        );
        match c.get(&key(), 4, "h1") {
            CacheLookup::Hit(got) => {
                assert_eq!(got.len(), 4);
                for (a, b) in got.iter().zip(v.iter()) {
                    assert_eq!(a.to_bits(), b.to_bits());
                }
            }
            other => panic!("expected a hit, got {other:?}"),
        }
    }

    #[test]
    fn a_staged_vector_is_not_visible_before_its_flush() {
        let c = VectorCache::in_memory();
        assert!(c.stage(&key(), "h1", &[1.0, 2.0]));
        assert_eq!(c.get(&key(), 2, "h1"), CacheLookup::Miss);
        c.flush();
        assert_eq!(c.get(&key(), 2, "h1"), CacheLookup::Hit(vec![1.0, 2.0]));
    }

    #[test]
    fn the_key_is_the_model_and_the_input_hash() {
        let c = VectorCache::in_memory();
        put(&c, "h1", &[1.0, 2.0]);
        assert_eq!(c.get(&key(), 2, "h1"), CacheLookup::Hit(vec![1.0, 2.0]));
        assert_eq!(c.get(&key(), 2, "h2"), CacheLookup::Miss, "another hash");
        assert_eq!(
            c.get(&ModelKey::new("other", 8000), 2, "h1"),
            CacheLookup::Miss,
            "another model string"
        );
        assert_eq!(
            c.get(&ModelKey::new("m", 4000), 2, "h1"),
            CacheLookup::Miss,
            "another budget"
        );
        assert_eq!(
            ModelKey::new("m", 8000).as_str(),
            format!("m|8000|{COMPOSITION_VERSION}")
        );
    }

    #[test]
    fn a_row_of_another_dimension_is_a_miss_and_the_next_put_replaces_it() {
        let c = VectorCache::in_memory();
        put(&c, "h1", &[1.0, 2.0, 3.0]);
        assert_eq!(c.get(&key(), 4, "h1"), CacheLookup::Miss);
        put(&c, "h1", &[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(
            c.get(&key(), 4, "h1"),
            CacheLookup::Hit(vec![1.0, 2.0, 3.0, 4.0])
        );
        assert_eq!(c.get(&key(), 3, "h1"), CacheLookup::Miss);
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn an_unknown_dimension_is_a_miss() {
        let c = VectorCache::in_memory();
        put(&c, "h1", &[1.0, 2.0]);
        assert_eq!(c.get(&key(), 0, "h1"), CacheLookup::Miss);
    }

    #[test]
    fn a_row_whose_blob_is_not_dim_times_four_bytes_is_failed_not_returned() {
        let tmp = tempfile::tempdir().unwrap();
        let c = file_cache(&tmp);
        put(&c, "h1", &[1.0, 2.0, 3.0, 4.0]);
        let raw = Connection::open(tmp.path().join(VECTOR_CACHE_FILE)).unwrap();
        raw.execute_batch("UPDATE vec SET vector = zeroblob(5)")
            .unwrap();
        assert_eq!(c.get(&key(), 4, "h1"), CacheLookup::Failed, "too short");
        raw.execute_batch("UPDATE vec SET vector = zeroblob(20)")
            .unwrap();
        assert_eq!(c.get(&key(), 4, "h1"), CacheLookup::Failed, "too long");
        raw.execute_batch("UPDATE vec SET vector = zeroblob(16)")
            .unwrap();
        assert_eq!(
            c.get(&key(), 4, "h1"),
            CacheLookup::Hit(vec![0.0; 4]),
            "exactly dim * 4 bytes is read"
        );
    }

    #[test]
    fn one_flush_is_one_transaction() {
        let c = VectorCache::in_memory();
        for i in 0..400 {
            assert!(c.stage(&key(), &format!("h{i}"), &[i as f32, 1.0]));
        }
        assert_eq!(
            c.flush(),
            FlushOutcome {
                stored: 400,
                failed: 0
            }
        );
        assert_eq!(c.transactions(), 1);
        assert_eq!(c.flush(), FlushOutcome::default());
        assert_eq!(c.transactions(), 1, "an empty flush commits nothing");
        for i in 0..3 {
            assert!(c.stage(&key(), &format!("x{i}"), &[1.0]));
        }
        c.flush();
        assert_eq!(c.transactions(), 2);
        assert_eq!(c.len(), 403);
    }
    #[test]
    fn a_flush_that_fails_on_one_row_stores_none_of_them() {
        let tmp = tempfile::tempdir().unwrap();
        let c = file_cache(&tmp);
        // Opens the file and creates the table.
        assert_eq!(c.get(&key(), 1, "none"), CacheLookup::Miss);
        let raw = Connection::open(tmp.path().join(VECTOR_CACHE_FILE)).unwrap();
        raw.execute_batch(
            "CREATE TRIGGER refuse BEFORE INSERT ON vec WHEN NEW.input_hash = 'bad' \
             BEGIN SELECT RAISE(ABORT, 'refused'); END",
        )
        .unwrap();
        for h in ["good1", "bad", "good2"] {
            assert!(c.stage(&key(), h, &[1.0]));
        }
        assert_eq!(
            c.flush(),
            FlushOutcome {
                stored: 0,
                failed: 3
            }
        );
        assert_eq!(c.len(), 0, "a per-row commit would leave the first row");
        assert_eq!(c.get(&key(), 1, "good1"), CacheLookup::Miss);
        assert_eq!(c.transactions(), 0, "a rolled back flush is not counted");
    }

    #[test]
    fn the_blob_is_little_endian_f32() {
        let tmp = tempfile::tempdir().unwrap();
        let c = file_cache(&tmp);
        let v = [0.5f32, -1.25, 3.0e-7];
        put(&c, "h1", &v);
        let raw = Connection::open(tmp.path().join(VECTOR_CACHE_FILE)).unwrap();
        let (dim, blob): (i64, Vec<u8>) = raw
            .query_row(
                "SELECT dim, vector FROM vec WHERE input_hash = 'h1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        let want: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
        assert_eq!(dim, 3);
        assert_eq!(blob, want);
    }

    #[test]
    fn the_table_is_a_rowid_table_keyed_by_model_and_input_hash() {
        let tmp = tempfile::tempdir().unwrap();
        let c = file_cache(&tmp);
        put(&c, "h1", &[1.0]);
        let raw = Connection::open(tmp.path().join(VECTOR_CACHE_FILE)).unwrap();
        let sql: String = raw
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name = 'vec'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(sql.contains("PRIMARY KEY (model_key, input_hash)"), "{sql}");
        assert!(!sql.contains("WITHOUT ROWID"), "{sql}");
        assert!(!sql.contains("last_used"), "{sql}");
        let mode: String = raw
            .pragma_query_value(None, "journal_mode", |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal", "a file-backed cache is in WAL mode");
    }

    #[test]
    fn an_unused_cache_creates_no_file() {
        let tmp = tempfile::tempdir().unwrap();
        {
            let c = VectorCache::open(&tmp.path().join(VECTOR_CACHE_FILE));
            let _ = c.path();
            let _ = c.status();
            c.rearm();
            assert_eq!(c.flush(), FlushOutcome::default());
            assert_eq!(c.get(&key(), 0, "h1"), CacheLookup::Miss);
        }
        assert!(!tmp.path().join(VECTOR_CACHE_FILE).exists());
    }

    #[test]
    fn a_cache_in_a_directory_that_does_not_exist_creates_it() {
        // mutation: drop the `create_dir_all` in `open_connection`.
        let tmp = tempfile::tempdir().unwrap();
        let want = tmp.path().join("sub").join(VECTOR_CACHE_FILE);
        let c = VectorCache::open(&want);
        put(&c, "h1", &[1.0]);
        assert!(want.exists());
        assert_eq!(c.path(), Some(want.as_path()));
    }

    #[test]
    fn the_production_busy_timeout_is_250_ms() {
        let tmp = tempfile::tempdir().unwrap();
        let c = file_cache(&tmp);
        assert_eq!(c.busy_timeout_ms(), Some(250));
    }

    #[test]
    fn a_corrupt_file_degrades_to_a_miss_and_is_left_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(VECTOR_CACHE_FILE);
        let garbage: Vec<u8> = (0..200u32).map(|i| (i * 7 + 3) as u8).collect();
        std::fs::write(&path, &garbage).unwrap();
        let c = VectorCache::open(&path);
        assert_eq!(c.get(&key(), 2, "h1"), CacheLookup::Failed);
        assert!(!c.stage(&key(), "h1", &[1.0, 2.0]));
        assert_eq!(c.status(), CacheStatus::Unavailable);
        assert_eq!(std::fs::read(&path).unwrap(), garbage);
    }

    #[test]
    fn a_failed_open_is_retried_after_rearm_and_not_before() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(VECTOR_CACHE_FILE);
        std::fs::write(&path, vec![0x5Au8; 200]).unwrap();
        let c = VectorCache::open(&path);
        assert_eq!(c.get(&key(), 2, "h1"), CacheLookup::Failed);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            c.get(&key(), 2, "h1"),
            CacheLookup::Failed,
            "no retry per lookup"
        );
        c.rearm();
        assert_eq!(c.get(&key(), 2, "h1"), CacheLookup::Miss);
        assert_eq!(c.status(), CacheStatus::On);
    }

    #[test]
    fn a_file_of_a_newer_format_is_unavailable_and_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(VECTOR_CACHE_FILE);
        {
            let raw = Connection::open(&path).unwrap();
            raw.pragma_update(None, "user_version", 99).unwrap();
        }
        let before = std::fs::read(&path).unwrap();
        let c = VectorCache::open(&path);
        assert_eq!(c.get(&key(), 2, "h1"), CacheLookup::Failed);
        assert_eq!(c.status(), CacheStatus::Unavailable);
        assert!(!c.stage(&key(), "h1", &[1.0, 2.0]));
        let raw = Connection::open(&path).unwrap();
        let v: i64 = raw
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, 99);
        drop(raw);
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
    #[test]
    fn a_file_with_a_negative_version_is_unavailable_and_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(VECTOR_CACHE_FILE);
        {
            let raw = Connection::open(&path).unwrap();
            raw.pragma_update(None, "user_version", -5).unwrap();
        }
        let before = std::fs::read(&path).unwrap();
        let c = VectorCache::open(&path);
        assert_eq!(c.get(&key(), 2, "h1"), CacheLookup::Failed);
        assert_eq!(
            c.status(),
            CacheStatus::Unavailable,
            "a version this code never wrote is not a cache of this format"
        );
        assert!(!c.stage(&key(), "h1", &[1.0, 2.0]));
        let raw = Connection::open(&path).unwrap();
        let v: i64 = raw
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, -5);
        let tables: i64 = raw
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = 'vec'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            tables, 0,
            "no table is created in a file of an unknown format"
        );
        drop(raw);
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[cfg(unix)]
    #[test]
    fn a_read_only_directory_gives_an_unavailable_cache() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("ro");
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        // Root and some filesystems ignore the mode bits: probe, and skip loudly.
        if std::fs::write(dir.join("probe"), b"x").is_ok() {
            eprintln!(
                "SKIPPED a_read_only_directory_gives_an_unavailable_cache: modes are ignored here"
            );
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
            return;
        }
        let c = VectorCache::open(&dir.join(VECTOR_CACHE_FILE));
        let got = c.get(&key(), 2, "h1");
        let entries = std::fs::read_dir(&dir).unwrap().count();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(got, CacheLookup::Failed);
        assert_eq!(c.status(), CacheStatus::Unavailable);
        assert_eq!(entries, 0, "nothing is created in a read-only directory");
    }

    #[test]
    fn an_opening_process_takes_no_write_lock_and_a_locked_file_fails_only_the_flush() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(VECTOR_CACHE_FILE);
        let first = VectorCache::open(&path);
        assert_eq!(first.get(&key(), 2, "h0"), CacheLookup::Miss);
        let holder = Connection::open(&path).unwrap();
        holder.execute_batch("BEGIN IMMEDIATE").unwrap();

        let c = VectorCache::open_with_busy_timeout(&path, Duration::from_millis(50));
        assert_eq!(c.get(&key(), 2, "h1"), CacheLookup::Miss);
        assert!(c.stage(&key(), "h1", &[1.0, 2.0]));
        assert_eq!(
            c.flush(),
            FlushOutcome {
                stored: 0,
                failed: 1
            }
        );
        assert_eq!(c.transactions(), 0, "a failed flush commits nothing");

        holder.execute_batch("ROLLBACK").unwrap();
        assert!(c.stage(&key(), "h1", &[1.0, 2.0]));
        assert_eq!(
            c.flush(),
            FlushOutcome {
                stored: 1,
                failed: 0
            }
        );
        assert_eq!(c.get(&key(), 2, "h1"), CacheLookup::Hit(vec![1.0, 2.0]));
        assert_eq!(c.transactions(), 1, "the next flush commits once");
    }
    #[test]
    fn a_new_file_is_initialised_only_if_it_still_reads_version_zero_under_the_lock() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(VECTOR_CACHE_FILE);
        // A newer binary holds the write lock on a new file and is about to set version 99.
        // WAL is set first, so the opener's own journal pragma needs no lock.
        let holder = Connection::open(&path).unwrap();
        holder.pragma_update(None, "journal_mode", "WAL").unwrap();
        holder
            .execute_batch("BEGIN IMMEDIATE; PRAGMA user_version = 99;")
            .unwrap();
        let p = path.clone();
        let opener = std::thread::spawn(move || {
            let c = VectorCache::open_with_busy_timeout(&p, Duration::from_secs(10));
            let got = c.get(&key(), 2, "h1");
            (got, c.status())
        });
        // The opener reads version 0 without the lock, then waits for the lock.
        std::thread::sleep(Duration::from_millis(400));
        holder.execute_batch("COMMIT").unwrap();
        let (got, status) = opener.join().unwrap();
        assert_eq!(got, CacheLookup::Failed);
        assert_eq!(status, CacheStatus::Unavailable);
        let v: i64 = holder
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, 99, "an older binary must not downgrade the file");
        let tables: i64 = holder
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = 'vec'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tables, 0);
    }

    #[test]
    fn a_second_connection_sees_the_flushed_rows() {
        let tmp = tempfile::tempdir().unwrap();
        let a = file_cache(&tmp);
        let b = file_cache(&tmp);
        assert!(a.stage(&key(), "h1", &[1.0, 2.0]));
        assert_eq!(b.get(&key(), 2, "h1"), CacheLookup::Miss);
        a.flush();
        assert_eq!(b.get(&key(), 2, "h1"), CacheLookup::Hit(vec![1.0, 2.0]));
    }

    #[test]
    fn an_empty_or_non_finite_vector_is_not_staged() {
        let c = VectorCache::in_memory();
        assert!(!c.stage(&key(), "h1", &[]));
        assert!(!c.stage(&key(), "h2", &[f32::NAN]));
        assert!(!c.stage(&key(), "h3", &[1.0, f32::INFINITY]));
        assert_eq!(c.flush(), FlushOutcome::default());
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn the_tally_counts_and_prints_what_a_run_did() {
        let mut t = CacheTally {
            status: CacheStatus::On,
            ..CacheTally::default()
        };
        t.note_item(true, false);
        t.note_item(true, false);
        t.note_item(false, true);
        t.note_flush(FlushOutcome {
            stored: 4,
            failed: 2,
        });
        assert_eq!((t.hits, t.errors, t.stored), (2, 3, 4));
        let j = t.to_json();
        assert_eq!(j["status"], "on");
        assert_eq!(j["hits"], 2);
        assert_eq!(j["stored"], 4);
        assert_eq!(j["errors"], 3);
        let d = CacheTally::default().to_json();
        let obj = d.as_object().unwrap();
        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["errors", "hits", "status", "stored"]);
        assert_eq!(d["status"], "off");
        assert_eq!(CacheStatus::On.as_str(), "on");
        assert_eq!(CacheStatus::Unavailable.as_str(), "unavailable");
    }
}
